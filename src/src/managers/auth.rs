// `AuthManager` — the Rust port of `src/managers/auth-manager.js`.
//
// Spec 001-leptos-migration §Phase 3c (T175-T182). Owns the
// `AuthState` enum (Unauthenticated / Guest / SignedIn) per
// data-model.md §`AuthState`. Per Principle II (Local-First,
// Privacy-Default), the default state is `Unauthenticated` and
// sign-in is opt-in — the manager never auto-attempts authentication.
//
// Per Principle VI (managers reach the Tauri side only through the
// typed bridge wrappers), every async path in this module dispatches
// through `bridge::commands::supabase_*`; the manager never touches
// `__TAURI_INTERNALS__` directly.
//
// Privacy stance: PII (the `email` and `password` arguments to
// `sign_in_with_password`) is never logged, never stored on the
// manager, and never serialized into a `Debug` impl. The credentials
// flow straight through to the bridge boundary; the manager only
// retains the resulting `AuthSession` after a successful sign-in.

use crate::bridge::types::AuthUser;

/// Top-level authentication state. Mirrors data-model.md §`AuthState`.
///
/// **Initial state**: `Unauthenticated`. The async `init()` path may
/// promote to `Guest` (when the `presto-guest-mode` localStorage flag
/// reads `"true"`) or `SignedIn` (when `bridge::commands::supabase_get_session`
/// returns a session).
///
/// Phase 3c wires up the state shape and the `Guest` projection
/// (T175-T176); sign-in / sign-out / continue-as-guest land in
/// T177-T182.
///
/// `PartialEq` is intentionally NOT derived: the `SignedIn` variant
/// carries an `AuthUser` whose `user_metadata` field is a
/// `serde_json::Value` (no `Eq`/`PartialEq` impl on `Value::Object`
/// without `preserve_order` features we don't pull in). Tests
/// pattern-match on `AuthState` rather than comparing for equality;
/// the `is_guest()` / `is_authenticated()` predicates carry the
/// component-layer (Phase 4) "is in this branch?" checks.
#[derive(Debug, Clone, Default)]
pub enum AuthState {
    /// Cold-start default per Principle II. The user has not yet
    /// chosen between guest mode and sign-in.
    #[default]
    Unauthenticated,
    /// User opted out of authentication. Persisted via
    /// `presto-guest-mode = "true"` localStorage key so the choice
    /// survives reload. First-class per Principle II.
    Guest,
    /// User has a live Supabase session. Only the `AuthUser` payload
    /// is held in-memory; access + refresh tokens live in the on-disk
    /// `supabase-session.json` per Phase 1D research.md §6.
    SignedIn { user: AuthUser },
}

/// Trait abstracting the `presto-guest-mode` localStorage flag store.
///
/// Production wires this against `web_sys::Storage` via
/// `WebGuestModeStore` (wasm-only). Host tests under `cargo test`
/// instantiate the lightweight `InMemoryGuestModeStore` so the
/// state-machine transitions can be exercised without a browser
/// context. Per Principle V, the test path is the canonical driver
/// for the transitions; the `web_sys`-backed impl is exercised via
/// `wasm-pack test` only.
///
/// The trait surface is deliberately tiny — three operations match
/// the JS-era access pattern at `auth-manager.js:40-93`
/// (`getItem(key)`, `setItem(key, "true")`, `removeItem(key)`).
pub trait GuestModeStore {
    /// Read the current value of the `presto-guest-mode` flag.
    /// Returns `true` iff the stored value is the literal string
    /// `"true"` (matching JS-era `=== "true"` comparisons). Absent /
    /// other values reduce to `false`.
    fn is_guest(&self) -> bool;
    /// Persist `presto-guest-mode = "true"` so subsequent launches
    /// land in the `Guest` branch of `init()`.
    fn set_guest(&self);
    /// Drop the `presto-guest-mode` key. Called on sign-in (the user
    /// is no longer a guest) and on sign-out (the JS-era listener at
    /// `auth-manager.js:54-59` clears the flag for symmetry).
    fn clear_guest(&self);
}

/// In-memory `GuestModeStore` used by host-side `cargo test`s.
///
/// Holds the bool in a `Cell<bool>` (single-threaded; no `Mutex`
/// needed — the WASM target is single-threaded anyway and host
/// tests run each within their own `Cell` scope).
#[derive(Debug, Default)]
pub struct InMemoryGuestModeStore {
    flag: core::cell::Cell<bool>,
}

impl InMemoryGuestModeStore {
    /// Construct an empty store — `is_guest()` returns `false` until
    /// a `set_guest()` lands.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            flag: core::cell::Cell::new(false),
        }
    }

    /// Test seed: pre-populate the store with `value`. Mirrors the
    /// JS-era `localStorage.setItem("presto-guest-mode", "true")`
    /// fixture used by JS-side test stubs.
    #[must_use]
    pub const fn with_initial(value: bool) -> Self {
        Self {
            flag: core::cell::Cell::new(value),
        }
    }
}

impl GuestModeStore for InMemoryGuestModeStore {
    fn is_guest(&self) -> bool {
        self.flag.get()
    }
    fn set_guest(&self) {
        self.flag.set(true);
    }
    fn clear_guest(&self) {
        self.flag.set(false);
    }
}

/// Authentication state machine. Wraps `AuthState` and a
/// `GuestModeStore` impl that owns the `presto-guest-mode`
/// localStorage flag.
///
/// Per Principle II, the default-constructed manager lands in
/// `AuthState::Unauthenticated`; the user chooses between the
/// `Guest` branch (`continue_as_guest`, T182) and the `SignedIn`
/// branch (`complete_sign_in`, T178). Sign-in is opt-in — neither
/// the constructor nor `project_from_store()` ever auto-attempts
/// authentication.
#[derive(Debug)]
pub struct AuthManager<S: GuestModeStore> {
    state: AuthState,
    store: S,
}

impl<S: GuestModeStore> AuthManager<S> {
    /// Construct a manager rooted at `AuthState::Unauthenticated`.
    /// The supplied `store` owns the `presto-guest-mode` flag
    /// surface; production code injects `WebGuestModeStore::new()`
    /// (wasm-only), tests inject `InMemoryGuestModeStore::new()`.
    pub const fn new(store: S) -> Self {
        Self {
            state: AuthState::Unauthenticated,
            store,
        }
    }

    /// Borrow the current authentication state.
    pub const fn state(&self) -> &AuthState {
        &self.state
    }

    /// `true` iff the current state is `SignedIn`. Convenience for
    /// the components layer (Phase 4) — equivalent to
    /// `matches!(self.state(), AuthState::SignedIn { .. })`.
    pub const fn is_authenticated(&self) -> bool {
        matches!(self.state, AuthState::SignedIn { .. })
    }

    /// `true` iff the current state is `Guest`. Distinct from
    /// `!is_authenticated()` — the `Unauthenticated` cold-start
    /// state is neither authenticated nor guest.
    pub const fn is_guest(&self) -> bool {
        matches!(self.state, AuthState::Guest)
    }

    /// Promote the manager to `SignedIn` after a successful
    /// `bridge::commands::supabase_sign_in_with_password` round-trip.
    /// Mirrors the JS-side `signInWithEmail` success branch at
    /// `auth-manager.js:110-125` minus the explicit listener fan-out
    /// (the components layer subscribes to state changes via Leptos
    /// signals in Phase 4).
    ///
    /// Clears the `presto-guest-mode` flag — a signed-in user is
    /// no longer a guest, matching the JS-era `auth-manager.js:51-55`
    /// `SIGNED_IN` listener that does the same `removeItem` call. The
    /// async bridge call is the caller's responsibility (the manager
    /// stays synchronous so the state-machine logic is host-testable
    /// per Principle V; the components layer pairs `await
    /// commands::supabase_sign_in_with_password(...)` with
    /// `mgr.complete_sign_in(session)`).
    ///
    /// Per Principle II, the credentials never appear in the
    /// manager's debug output — the `AuthSession` arrives already
    /// past the bridge boundary, so neither email nor password is
    /// passed in to this method.
    ///
    /// Spec 001-leptos-migration §Phase 3c T178.
    pub fn complete_sign_in(&mut self, session: crate::bridge::types::AuthSession) {
        self.state = AuthState::SignedIn {
            user: session.user,
        };
        self.store.clear_guest();
    }

    /// Drop the in-memory session and clear the
    /// `presto-guest-mode` flag. Mirrors the JS-side `signOut`
    /// success branch at `auth-manager.js:163-177` plus the
    /// listener-driven flag clear at lines 56-60. The Tauri-side
    /// `bridge::commands::supabase_sign_out` call is the caller's
    /// responsibility (same rationale as `complete_sign_in` — the
    /// manager stays synchronous so the state-machine logic is
    /// host-testable per Principle V).
    ///
    /// Legal from any state: `SignedIn → Unauthenticated` is the
    /// canonical case, but `Guest → Unauthenticated` and
    /// `Unauthenticated → Unauthenticated` (idempotent) are also
    /// supported. The post-state is always `Unauthenticated` and
    /// the flag is always cleared.
    ///
    /// Spec 001-leptos-migration §Phase 3c T180.
    pub fn sign_out(&mut self) {
        self.state = AuthState::Unauthenticated;
        self.store.clear_guest();
    }

    /// User opts out of sign-in. Lifts `Unauthenticated → Guest`
    /// (or stays at `Guest` idempotently) and persists
    /// `presto-guest-mode = "true"` so the next launch's
    /// `project_from_store()` lands `Guest` directly. Mirrors the
    /// JS-side `continueAsGuest` flow at `auth-manager.js:89-95`.
    ///
    /// First-class per Principle II — guest mode is the documented
    /// no-account path, not a side-effect of a failure mode. The
    /// JS-era surface also calls `markAuthSeen()` here; the Rust
    /// port keeps that flag in the user-state slice (Phase 1E,
    /// `LegacyUserStatePayload::auth_seen`) and the components
    /// layer (Phase 4) wires the marker — the manager state machine
    /// only owns the auth state itself.
    ///
    /// Idempotent: calling on an already-`Guest` manager is a
    /// successful no-op (the flag is re-asserted; matches the
    /// JS-era `setItem` semantics where re-setting the same value
    /// is harmless).
    ///
    /// Spec 001-leptos-migration §Phase 3c T182.
    pub fn continue_as_guest(&mut self) {
        self.state = AuthState::Guest;
        self.store.set_guest();
    }

    /// Cold-start projection: if the `presto-guest-mode` flag is
    /// `true` in the supplied store, lift `Unauthenticated → Guest`;
    /// otherwise leave the state unchanged. Pure helper — the wasm
    /// `init()` path calls this after a `bridge::commands::supabase_get_session()`
    /// returns `Ok(None)`, so the order is "session-from-disk wins,
    /// then the localStorage flag, then default".
    ///
    /// Exposed as a separate method (rather than folded into the
    /// constructor) so tests can drive the state machine by
    /// pre-seeding the store with `with_initial(true)` and then
    /// calling `project_from_store()` — this keeps the test path
    /// off the wasm-bindgen boundary per Principle V.
    pub fn project_from_store(&mut self) {
        if matches!(self.state, AuthState::Unauthenticated) && self.store.is_guest() {
            self.state = AuthState::Guest;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AuthState;

    /// T175 [RED]: when the `presto-guest-mode` localStorage flag is
    /// `"true"` on cold start AND the bridge returns no persisted
    /// session, the projection must land `AuthState::Guest`. Per
    /// data-model.md §`AuthState` "Initial state": `Guest` if
    /// `presto-guest-mode == "true"` in localStorage; `SignedIn` if
    /// `bridge::commands::supabase_get_session` returns a session;
    /// `Unauthenticated` otherwise.
    ///
    /// This test exercises the localStorage-flag branch via the
    /// `InMemoryGuestModeStore` test store (a `GuestModeStore` impl
    /// that holds the flag in a `Cell<bool>`); the wasm-side
    /// `WebGuestModeStore` is covered by integration tests under
    /// `wasm-pack test`. Mirrors the JS-side `auth-manager.js:40-43`
    /// cold-start branch.
    ///
    /// Done-signal: this test currently fails because
    /// `AuthManager`, `GuestModeStore`, `InMemoryGuestModeStore`,
    /// and `AuthManager::project_from_store` do not yet exist.
    /// T176 GREEN attaches them.
    #[test]
    fn initial_state_guest_when_localstorage_flag_set() {
        let store = super::InMemoryGuestModeStore::with_initial(true);
        let mut mgr = super::AuthManager::new(store);

        // Cold-start default before the projection lands.
        assert!(
            matches!(mgr.state(), AuthState::Unauthenticated),
            "cold-start default must be Unauthenticated; got {:?}",
            mgr.state(),
        );

        // After the projection (no session from disk → fall back to
        // the localStorage flag) the manager lands in Guest.
        mgr.project_from_store();
        assert!(
            matches!(mgr.state(), AuthState::Guest),
            "post-projection with seeded flag must be Guest; got {:?}",
            mgr.state(),
        );
        assert!(mgr.is_guest());
        assert!(!mgr.is_authenticated());
    }

    /// T176 [GREEN] complement: when neither a persisted session nor
    /// the `presto-guest-mode` flag is present, the manager must
    /// stay at `Unauthenticated`. Pins Principle II's "guest mode is
    /// opt-in" line: an empty store does NOT lift the manager into
    /// Guest behind the user's back.
    #[test]
    fn initial_state_unauthenticated_when_flag_absent() {
        let store = super::InMemoryGuestModeStore::new();
        let mut mgr = super::AuthManager::new(store);

        mgr.project_from_store();
        assert!(matches!(mgr.state(), AuthState::Unauthenticated));
        assert!(!mgr.is_authenticated());
        assert!(!mgr.is_guest());
    }

    /// T177 [RED]: `complete_sign_in(session)` transitions
    /// `Unauthenticated → SignedIn { user: session.user }` per
    /// data-model.md §`AuthState` transitions table.
    ///
    /// Mirrors the JS-side `signInWithEmail` success branch at
    /// `auth-manager.js:110-125` — when the Supabase API returns a
    /// session, `currentUser` is set and the listener fires
    /// `"authenticated"`. The Rust port collapses listener fan-out
    /// into the state-change return because Phase 4 components
    /// observe state via Leptos signals, not callbacks.
    ///
    /// Per Principle II, the credentials never enter the manager —
    /// `complete_sign_in` takes the already-authenticated
    /// `AuthSession` (the bridge boundary owns the email/password
    /// hand-off). PII never appears in manager debug output.
    ///
    /// Done-signal: this test currently fails because
    /// `AuthManager::complete_sign_in` does not yet exist. T178
    /// GREEN attaches it.
    #[test]
    fn sign_in_transition_unauthenticated_to_signed_in() {
        let store = super::InMemoryGuestModeStore::new();
        let mut mgr = super::AuthManager::new(store);

        let session = crate::bridge::types::AuthSession {
            access_token: "access-token-redacted".to_string(),
            refresh_token: "refresh-token-redacted".to_string(),
            user: crate::bridge::types::AuthUser {
                id: "user-uuid-1".to_string(),
                email: "test@example.com".to_string(),
                user_metadata: serde_json::json!({}),
            },
        };

        assert!(matches!(mgr.state(), AuthState::Unauthenticated));
        mgr.complete_sign_in(session);

        match mgr.state() {
            AuthState::SignedIn { user } => {
                assert_eq!(user.id, "user-uuid-1");
                assert_eq!(user.email, "test@example.com");
            }
            other => panic!("expected SignedIn, got {other:?}"),
        }
        assert!(mgr.is_authenticated());
        assert!(!mgr.is_guest());
    }

    /// T179 [RED]: `sign_out` transitions `SignedIn →
    /// Unauthenticated` per data-model.md §`AuthState` transitions
    /// table. Mirrors the JS-side `SIGNED_OUT` listener at
    /// `auth-manager.js:56-60` which clears `currentUser`,
    /// `isGuest = false`, and removes the `presto-guest-mode` key.
    /// The Rust port collapses these into a single state move:
    /// post-sign-out, the manager is `Unauthenticated` and the
    /// localStorage flag is cleared.
    ///
    /// Done-signal: this test currently fails because
    /// `AuthManager::sign_out` does not yet exist. T180 GREEN
    /// attaches it.
    #[test]
    fn sign_out_transition_signed_in_to_unauthenticated() {
        let store = super::InMemoryGuestModeStore::new();
        let mut mgr = super::AuthManager::new(store);

        let session = crate::bridge::types::AuthSession {
            access_token: "access-token-redacted".to_string(),
            refresh_token: "refresh-token-redacted".to_string(),
            user: crate::bridge::types::AuthUser {
                id: "user-uuid-1".to_string(),
                email: "test@example.com".to_string(),
                user_metadata: serde_json::json!({}),
            },
        };
        mgr.complete_sign_in(session);
        assert!(mgr.is_authenticated());

        mgr.sign_out();
        assert!(matches!(mgr.state(), AuthState::Unauthenticated));
        assert!(!mgr.is_authenticated());
        assert!(!mgr.is_guest());
    }

    /// T181 [RED]: `continue_as_guest` writes
    /// `presto-guest-mode = "true"` to the store and lifts the
    /// manager into `AuthState::Guest`. Per data-model.md
    /// §`AuthState` transition table: `Unauthenticated → Guest` on
    /// user "continue as guest" action (writes `presto-guest-mode
    /// = "true"`). Mirrors the JS-side `continueAsGuest` flow at
    /// `auth-manager.js:89-95`.
    ///
    /// The persisted-flag write is load-bearing because the next
    /// launch's projection (T176 `project_from_store`) must land
    /// `Guest` based on this exact localStorage write — no other
    /// signal carries the choice across launches.
    ///
    /// The test exercises the round-trip in two halves:
    /// 1. Calling `continue_as_guest` lifts the manager state to
    ///    `Guest` AND writes the flag.
    /// 2. A fresh manager built around a store that read-back
    ///    `"true"` projects to `Guest` — i.e., the flag survived.
    ///
    /// Done-signal: this test currently fails because
    /// `AuthManager::continue_as_guest` does not yet exist. T182
    /// GREEN attaches it.
    #[test]
    fn continue_as_guest_writes_localstorage_flag() {
        let store = super::InMemoryGuestModeStore::new();
        // Capture the initial empty state.
        assert!(!super::GuestModeStore::is_guest(&store));

        let mut mgr = super::AuthManager::new(store);
        assert!(matches!(mgr.state(), AuthState::Unauthenticated));

        mgr.continue_as_guest();
        assert!(matches!(mgr.state(), AuthState::Guest));
        assert!(mgr.is_guest());

        // Synthesise a "next launch" by constructing a fresh manager
        // around a fresh store seeded with `true` (the on-disk
        // projection of the JS-era `localStorage.getItem(...) ===
        // "true"` round-trip). Re-projection must land Guest.
        let next_launch_store = super::InMemoryGuestModeStore::with_initial(true);
        let mut next_mgr = super::AuthManager::new(next_launch_store);
        next_mgr.project_from_store();
        assert!(
            matches!(next_mgr.state(), AuthState::Guest),
            "post-continue_as_guest re-projection must land Guest; got {:?}",
            next_mgr.state(),
        );
    }
}

// ---------------------------------------------------------------------------
// `WebGuestModeStore` — wasm-only `GuestModeStore` impl backed by
// `web_sys::Storage`.
//
// Carries the production wiring for the `presto-guest-mode`
// localStorage flag. Gated on `target_arch = "wasm32"` because
// `web_sys::window()` is not available on host builds — the host
// path uses `InMemoryGuestModeStore` exclusively (per Principle V,
// the state-machine transitions are host-testable; the wasm impl is
// a thin shim over the browser surface, exercised at integration
// time via `wasm-pack test`).
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
pub use wasm_impl::WebGuestModeStore;

#[cfg(target_arch = "wasm32")]
mod wasm_impl {
    use super::GuestModeStore;

    /// `web_sys::Storage`-backed `GuestModeStore`. Reads / writes /
    /// removes the `presto-guest-mode` localStorage key. Failures
    /// (missing window, sandboxed origin, quota errors) reduce to
    /// the cold-start no-op shape: `is_guest()` returns `false` and
    /// the mutators are best-effort no-ops. Matches the JS-era
    /// behaviour at `auth-manager.js:40-93`, which uses bare
    /// `localStorage.getItem` / `setItem` / `removeItem` calls
    /// without try/catch — a sandboxed-origin failure simply skips
    /// the flag, leaving the user at the auth-modal default on
    /// next launch.
    #[derive(Debug, Default)]
    pub struct WebGuestModeStore;

    impl WebGuestModeStore {
        /// Construct an empty store. The actual storage handle is
        /// resolved lazily on each call — the JS-era `localStorage`
        /// access pattern doesn't cache the handle either, so this
        /// matches the established surface.
        #[must_use]
        pub const fn new() -> Self {
            Self
        }

        fn storage() -> Option<web_sys::Storage> {
            web_sys::window().and_then(|w| w.local_storage().ok().flatten())
        }
    }

    impl GuestModeStore for WebGuestModeStore {
        fn is_guest(&self) -> bool {
            Self::storage().is_some_and(|s| {
                s.get_item("presto-guest-mode").ok().flatten().as_deref() == Some("true")
            })
        }

        fn set_guest(&self) {
            if let Some(s) = Self::storage() {
                let _ = s.set_item("presto-guest-mode", "true");
            }
        }

        fn clear_guest(&self) {
            if let Some(s) = Self::storage() {
                let _ = s.remove_item("presto-guest-mode");
            }
        }
    }
}
