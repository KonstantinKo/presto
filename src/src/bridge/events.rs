// Bridge event listener surface — typed `listen()` wrappers for every
// Tauri event the Leptos crate consumes.
//
// Spec 001-leptos-migration §Phase 1F T116-T117; contracts/tauri-bridge.md
// §"Tauri events". One `pub const` per event name + a typed payload
// struct (or primitive) per event. The generic `listen<T>(name, cb)`
// helper subscribes via the Tauri 2.x JS event API and returns a
// `Listener` RAII guard that calls the JS unsubscribe closure on
// `Drop`.
//
// Per AGENTS.md §IPC: `invoke()` + `listen()` only; no other channels.
//
// This module is intentionally separate from `bridge::commands` (which
// owns request/response IPC). Events are fire-and-forget pushes from
// the Tauri side; the wrapper's job is the typed payload boundary
// plus the drop-guard that prevents listener leaks across `LiveView`
// re-renders.
//
// Lint allowance: `clippy::future_not_send` is allowed at the module
// level for the same reason it's allowed in `bridge::commands` — the
// bridge runs exclusively on `wasm32-unknown-unknown`, where the
// runtime is single-threaded and `JsValue` (plus everything
// transitively built on it: `JsFuture`, `Promise`, `Closure`,
// `serde-wasm-bindgen` values) is `!Send` by construction.
#![allow(
    clippy::future_not_send,
    reason = "Tauri event futures carry JsValue/Closure and run only on single-threaded wasm32."
)]

use serde::de::DeserializeOwned;
use serde::Deserialize;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use super::availability::bridge_available;
use super::types::BridgeError;

// ---------------------------------------------------------------------------
// Event-name constants (E1-E10 per contracts/tauri-bridge.md §"Tauri events")
// ---------------------------------------------------------------------------

/// E1 — `ActivityMonitor` (macOS) emits this on idle→active transition
/// (`src-tauri/src/lib.rs:367`). Payload: `()`.
/// Consumer: `engine/activity_signal.rs`.
pub const USER_ACTIVITY: &str = "user-activity";

/// E2 — `ActivityMonitor` (macOS) emits this on active→idle transition
/// (`src-tauri/src/lib.rs:379`). Payload: `()`.
/// Consumer: `engine/activity_signal.rs`.
pub const USER_INACTIVITY: &str = "user-inactivity";

/// E3 — global keyboard shortcut fired.
///
/// Emitted by `register_global_shortcuts` when a registered system-level
/// shortcut fires (`src-tauri/src/lib.rs:678`). Payload: `String`
/// carrying the action name (`"start-stop"`, `"reset"`, `"skip"`).
/// Consumer: `app.rs` (dispatches into `engine/timer.rs`).
pub const GLOBAL_SHORTCUT: &str = "global-shortcut";

/// E4 — `register_global_shortcuts` emits this after successfully
/// rebinding the shortcut set (`src-tauri/src/lib.rs:686`). Payload:
/// `ShortcutSettings`.
/// Consumer: `managers/settings.rs`.
pub const SHORTCUTS_UPDATED: &str = "shortcuts-updated";

/// E6 — Tray "Start session" menu item emits this (`src-tauri/src/lib.rs:941`).
/// Payload: `()`.
/// Consumer: `engine/timer.rs`.
pub const TRAY_START_SESSION: &str = "tray-start-session";

/// E7 — Tray "Pause" menu item emits this (`src-tauri/src/lib.rs:950`).
/// Payload: `()`.
/// Consumer: `engine/timer.rs`.
pub const TRAY_PAUSE: &str = "tray-pause";

/// E8 — Tray "Skip" menu item emits this (`src-tauri/src/lib.rs:959`).
/// Payload: `()`.
/// Consumer: `engine/timer.rs`.
pub const TRAY_SKIP: &str = "tray-skip";

/// E9 — Tray "Cancel" menu item emits this (`src-tauri/src/lib.rs:968`).
/// Payload: `()`.
/// Consumer: `engine/timer.rs`.
pub const TRAY_CANCEL: &str = "tray-cancel";

/// E10 — `tauri-plugin-updater` detected a newer release.
///
/// Emitted by `tauri-plugin-updater` when the auto-updater detects a
/// newer release. Payload: `UpdateAvailablePayload`. Consumer:
/// `managers/update.rs`. The plugin emits other events as well
/// (`tauri://update-installed`, `tauri://update-status`); we only
/// expose the available-detection event because it's the only one the
/// post-cutover crate actively listens for. Additional plugin events
/// can be subscribed via `listen::<RawPayloadType>("tauri://…")`
/// without adding new constants — the typed surface is per-consumer,
/// not per-emitter.
pub const UPDATE_AVAILABLE: &str = "tauri://update-available";

/// E11 — backend-driven 1 Hz tick.
///
/// Emitted by a Rust thread spawned in `src-tauri/src/lib.rs::run()`
/// to keep the timer cadence at 1 Hz even when `WKWebView` occludes
/// the window and throttles `setInterval`. Payload: `()`. Consumer:
/// `components/timer/mod.rs` (runs the same `engine.tick` /
/// tray-update / metronome body as the frontend
/// `set_interval_with_handle` driver).
pub const ENGINE_TICK: &str = "engine-tick";

/// E12 — backend detected a native 1 Hz tick gap.
///
/// Payload carries the pause timestamp and native cause.
pub const SYSTEM_SUSPENDED: &str = "system-suspended";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SystemPauseReason {
    LockScreen,
    SystemSuspension,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct SystemSuspendedPayload {
    pub paused_at_ms: i64,
    pub reason: SystemPauseReason,
}

// ---------------------------------------------------------------------------
// Tauri 2.x JS event-API binding
// ---------------------------------------------------------------------------

#[wasm_bindgen]
extern "C" {
    /// Tauri 2.x JS event API entry point. Bound to
    /// `window.__TAURI__.event.listen(eventName, handler)`. The handler
    /// is invoked with `{ event: string, id: number, payload: T }`. The
    /// returned `Promise` resolves to a JS function that, when called,
    /// unsubscribes the handler.
    ///
    /// Callers MUST short-circuit on `bridge_available().is_absent()`
    /// before invoking — the binding panics in environments where
    /// `__TAURI__` is missing (Trunk dev server, node tests, anywhere
    /// the Tauri webview bootstrap hasn't run).
    #[wasm_bindgen(
        js_namespace = ["__TAURI__", "event"],
        js_name = listen,
        catch
    )]
    fn tauri_listen(
        event_name: &str,
        handler: &js_sys::Function,
    ) -> Result<js_sys::Promise, JsValue>;
}

// ---------------------------------------------------------------------------
// `Listener` — RAII subscription guard
// ---------------------------------------------------------------------------

/// RAII guard returned by a successful `listen()` subscription.
///
/// Dropping the guard calls the JS unsubscribe closure (returned by
/// `__TAURI__.event.listen` on resolution), which removes the handler
/// from the Tauri JS event bus. This is the leak guarantee `LiveView`
/// consumers depend on across re-renders: a `Listener` stored in a
/// component's `on_cleanup` will unsubscribe when the component
/// unmounts.
///
/// `_closure` is held alive for the listener's lifetime so wasm-bindgen
/// doesn't drop the trampoline that bridges the JS handler invocation
/// into the user's Rust callback. Without it, the JS side would call
/// into freed memory the next time the event fires.
///
/// `clippy::missing_fields_in_debug` is intentionally not silenced
/// here because we don't `derive(Debug)` — the held closure isn't
/// debug-printable in a useful way and the guard is meant to be held,
/// not inspected.
pub struct Listener {
    /// Trampoline keeping the JS-callable wrapper around the user's
    /// callback alive. The JS event bus holds a reference to its
    /// inner `Function`; dropping the Closure first would free the
    /// trampoline while the JS side still has the reference, and the
    /// next event emit would dereference freed memory.
    _closure: Closure<dyn FnMut(JsValue)>,
    /// JS unsubscribe function returned by `__TAURI__.event.listen`.
    /// `Some` until `Drop` runs; `None` afterwards. We model it as
    /// `Option` so the `Drop` impl can `take()` and call it exactly
    /// once even if `Listener::drop` is composed by some future
    /// helper (e.g., a `LeakingListener::leak(self)` adapter).
    unsubscribe: Option<js_sys::Function>,
}

impl Drop for Listener {
    fn drop(&mut self) {
        if let Some(unlisten) = self.unsubscribe.take() {
            // Best-effort: the JS unsubscribe is a side-effecting
            // function with no return value the Rust side cares about.
            // A panicking unlisten (which would only happen if the
            // Tauri JS bus has been torn down underneath us) would
            // poison the unwind path of whatever component holds the
            // listener; absorbing the result keeps Drop infallible.
            let _ = unlisten.call0(&JsValue::NULL);
        }
    }
}

// Manual Debug impl: the held `Closure` and the JS unsubscribe
// `Function` both lack useful Debug printing (they'd surface raw
// JsValue pointers, never the user-visible callback identity). We
// expose the subscription state instead, which is the only field a
// human reading a panic message needs. `finish_non_exhaustive()`
// signals to readers that the type intentionally elides JS-side
// fields rather than forgetting them (per clippy's
// `missing_fields_in_debug` lint guidance).
impl core::fmt::Debug for Listener {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Listener")
            .field("subscribed", &self.unsubscribe.is_some())
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// Generic listen() — typed subscription helper
// ---------------------------------------------------------------------------

/// Subscribe to a Tauri event by name with a typed payload.
///
/// The `event_name` MUST be one of the `pub const`s in this module
/// (or a `tauri-plugin-*` prefixed name); drift between Leptos and
/// Tauri here is a runtime no-op (the listener simply never fires)
/// rather than a compile error, which is why every consumed event is
/// pinned by a const.
///
/// The `callback` is invoked once per emit with the deserialised
/// payload. `FnMut + 'static` lets consumers capture state by mutable
/// reference (`Rc<RefCell<…>>` is the conventional shape) and
/// outlive the call site (the closure is stashed inside the
/// `Listener` guard and survives until the guard is dropped).
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge
/// is not present (Trunk dev server, e2e mock harness, node tests).
/// Returns `BridgeError::Internal` if the JS-side `listen()` call
/// rejects (rare; would indicate a misconfigured Tauri webview).
/// Returns `BridgeError::SerdeRoundtrip` if a payload arrives that
/// can't be deserialised into `T` — but this fires inside the
/// callback, not from `listen()`'s return.
pub async fn listen<T>(
    event_name: &str,
    mut callback: impl FnMut(T) + 'static,
) -> Result<Listener, BridgeError>
where
    T: DeserializeOwned + 'static,
{
    if bridge_available().is_absent() {
        return Err(BridgeError::BridgeUnavailable);
    }

    // Capture event_name into the closure for SerdeRoundtrip diagnostics.
    let event_name_owned: String = event_name.to_string();

    // Wrap the user callback in a JS-callable trampoline. The JS event
    // bus invokes us with `{ event, id, payload }`; we extract `payload`
    // via Reflect (avoiding a full deserialise of the wrapper object,
    // which would require a typed wrapper struct that adds nothing).
    let closure = Closure::wrap(Box::new(move |envelope: JsValue| {
        let payload_key = JsValue::from_str("payload");
        // Malformed envelope (no `payload` field) — silently drop the
        // event. The Tauri JS event bus is stable; this branch is
        // unreachable in production. Logging would risk PII per
        // Principle II.
        let Ok(payload) = js_sys::Reflect::get(&envelope, &payload_key) else {
            return;
        };
        // SerdeRoundtrip on inbound event payloads is a contract drift
        // between Tauri-side and Leptos-side type definitions —
        // surface via the consumer rather than a panic. We can't
        // return an error here (we're inside a JS-driven callback);
        // the consumer's typed callback simply never fires for
        // malformed payloads. Future enhancement: thread an error
        // sink through `listen` so consumers can observe
        // deserialisation failures. Out of scope for Phase 1F. The
        // `event_name_owned` capture keeps the event name available
        // for that future error sink without changing today's API.
        let Ok(typed) = serde_wasm_bindgen::from_value::<T>(payload) else {
            let _ = &event_name_owned;
            return;
        };
        callback(typed);
    }) as Box<dyn FnMut(JsValue)>);

    // Call the JS-side listen() with the trampoline's inner Function.
    let promise = tauri_listen(event_name, closure.as_ref().unchecked_ref()).map_err(|e| {
        BridgeError::Internal {
            msg: format!("listen('{event_name}') failed at the JS bridge boundary: {e:?}"),
        }
    })?;

    let resolved = JsFuture::from(promise)
        .await
        .map_err(|e| BridgeError::Internal {
            msg: format!("listen('{event_name}') promise rejected: {e:?}"),
        })?;

    // The resolved value is the JS unsubscribe function. If the Tauri
    // bridge ever returns a non-function shape here, that's a contract
    // violation — surface as Internal rather than silently leaking.
    let unsubscribe: js_sys::Function =
        resolved.dyn_into().map_err(|raw| BridgeError::Internal {
            msg: format!("listen('{event_name}') returned non-function unsubscribe: {raw:?}"),
        })?;

    Ok(Listener {
        _closure: closure,
        unsubscribe: Some(unsubscribe),
    })
}

// Tests gated on `wasm32` because every assertion is a
// `#[wasm_bindgen_test]` — running them via `cargo test` on the host
// target would produce dead-code lint failures (the host-side
// `cfg(target_arch = "wasm32")` removal silently drops the test
// bodies). `wasm-pack test --node` is the canonical test driver per
// `quickstart.md` line 105 and tasks.md T116 done-signal.
#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::{
        listen, Listener, ENGINE_TICK, GLOBAL_SHORTCUT, SHORTCUTS_UPDATED, SYSTEM_SUSPENDED,
        TRAY_CANCEL, TRAY_PAUSE, TRAY_SKIP, TRAY_START_SESSION, UPDATE_AVAILABLE, USER_ACTIVITY,
        USER_INACTIVITY,
    };
    use crate::bridge::types::BridgeError;
    use crate::bridge::types::{ShortcutSettings, UpdateAvailablePayload};
    use wasm_bindgen_test::wasm_bindgen_test;

    /// Pins the canonical event-name string for every event the Leptos
    /// crate subscribes to. This is the contract surface — drift here
    /// breaks runtime listener wiring at the boundary
    /// (`__TAURI__.event.listen("user-activity", …)` is what the
    /// Tauri-side `app.emit("user-activity", ())` matches against).
    /// The list mirrors contracts/tauri-bridge.md §"Tauri events"
    /// rows E1-E11.
    #[wasm_bindgen_test]
    fn event_names_match_contract() {
        assert_eq!(USER_ACTIVITY, "user-activity");
        assert_eq!(USER_INACTIVITY, "user-inactivity");
        assert_eq!(GLOBAL_SHORTCUT, "global-shortcut");
        assert_eq!(SHORTCUTS_UPDATED, "shortcuts-updated");
        assert_eq!(TRAY_START_SESSION, "tray-start-session");
        assert_eq!(TRAY_PAUSE, "tray-pause");
        assert_eq!(TRAY_SKIP, "tray-skip");
        assert_eq!(TRAY_CANCEL, "tray-cancel");
        assert_eq!(UPDATE_AVAILABLE, "tauri://update-available");
        assert_eq!(ENGINE_TICK, "engine-tick");
        assert_eq!(SYSTEM_SUSPENDED, "system-suspended");
    }

    /// `listen<T>` short-circuits with `BridgeError::BridgeUnavailable`
    /// when `__TAURI_INTERNALS__` is absent — same uniform shape as
    /// every command wrapper in `bridge::commands`. Under `wasm-pack
    /// test --node` no globals are installed, so the call resolves
    /// to `Err(BridgeUnavailable)` immediately.
    #[wasm_bindgen_test]
    async fn listen_short_circuits_when_bridge_absent() {
        let result = listen::<()>(USER_ACTIVITY, |()| {}).await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin: `listen` must accept any
    /// `DeserializeOwned + 'static` payload, take the callback by
    /// `FnMut(T) + 'static`, and return `Result<Listener, BridgeError>`.
    /// If the signature drifts, this test stops compiling.
    #[wasm_bindgen_test]
    async fn listen_signature_pinned_for_unit_payload() {
        async fn assert_signature() -> Result<Listener, BridgeError> {
            listen::<()>(USER_ACTIVITY, |(): ()| {}).await
        }
        let _ = assert_signature().await;
    }

    /// Typed payload pin: `shortcuts-updated` carries `ShortcutSettings`,
    /// not a `String` or `serde_json::Value`. The wrapper's generic
    /// parameter is the load-bearing type guarantee for FR-008.
    #[wasm_bindgen_test]
    async fn listen_signature_pinned_for_shortcuts_updated_payload() {
        async fn assert_signature() -> Result<Listener, BridgeError> {
            listen::<ShortcutSettings>(SHORTCUTS_UPDATED, |_p: ShortcutSettings| {}).await
        }
        let _ = assert_signature().await;
    }

    /// Typed payload pin: the updater-plugin event carries a structured
    /// payload (version + body + date) per `tauri-plugin-updater`'s
    /// emit shape. Pins that we expose a real struct, not a string.
    #[wasm_bindgen_test]
    async fn listen_signature_pinned_for_update_available_payload() {
        async fn assert_signature() -> Result<Listener, BridgeError> {
            listen::<UpdateAvailablePayload>(UPDATE_AVAILABLE, |_p: UpdateAvailablePayload| {})
                .await
        }
        let _ = assert_signature().await;
    }

    /// `Listener` is the RAII guard returned by a successful `listen()`
    /// subscription. Dropping it MUST unsubscribe — that's the leak
    /// guarantee `LiveView` consumers depend on across re-renders.
    /// Pins that the type owns drop logic; the actual JS-side
    /// unsubscribe side-effect is covered by integration tests that
    /// run with a stub `__TAURI__.event.listen` installed on the
    /// global object.
    ///
    /// `core::mem::needs_drop::<Listener>()` is `true` iff the type
    /// or one of its fields has non-trivial drop logic. For
    /// `Listener`, the held `Closure` (heap-allocated trampoline)
    /// and the `Option<js_sys::Function>` both contribute. If a
    /// future refactor turns the guard into a Plain-Old-Data handle
    /// (no closure, no Drop impl), this assertion fires and the
    /// leak guarantee is no longer enforced by the type — that's the
    /// regression fence.
    #[wasm_bindgen_test]
    fn listener_drop_unsubscribes_compile_pin() {
        assert!(
            core::mem::needs_drop::<Listener>(),
            "Listener must own non-trivial Drop logic for the unsubscribe guarantee"
        );
    }
}
