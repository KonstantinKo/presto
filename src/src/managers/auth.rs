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

#[cfg(test)]
mod tests {
    use super::{AuthState, AuthUser};
    use crate::bridge::types::AuthSession;

    fn _sample_user() -> AuthUser {
        AuthUser {
            id: "user-uuid-1".to_string(),
            email: "test@example.com".to_string(),
            user_metadata: serde_json::json!({}),
        }
    }

    fn _sample_session() -> AuthSession {
        AuthSession {
            access_token: "access-token-redacted".to_string(),
            refresh_token: "refresh-token-redacted".to_string(),
            user: _sample_user(),
        }
    }

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
}
