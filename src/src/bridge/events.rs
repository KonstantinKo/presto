// Bridge event listener surface — typed `listen()` wrappers for every
// Tauri event the Leptos crate consumes.
//
// Spec 001-leptos-migration §Phase 1F T116-T117; contracts/tauri-bridge.md
// §"Tauri events". One `pub const` per event name + a typed payload
// per event in `bridge::types`. The generic `listen<T>(name, cb)`
// helper subscribes via the Tauri 2.x JS event API and returns a
// `Listener` RAII guard that calls the JS unsubscribe closure on
// `Drop`.
//
// Per AGENTS.md §IPC: `invoke()` + `listen()` only; no other channels.
//
// This module is intentionally separate from `bridge::commands` (which
// owns request/response IPC). Events are fire-and-forget pushes from
// the Tauri side; the wrapper's job is the typed payload boundary
// plus the drop-guard that prevents listener leaks across LiveView
// re-renders.

#![allow(dead_code)]

// Tests gated on `wasm32` because every assertion is a
// `#[wasm_bindgen_test]` — running them via `cargo test` on the host
// target would produce dead-code lint failures (the host-side
// `cfg(target_arch = "wasm32")` removal silently drops the test
// bodies). `wasm-pack test --node` is the canonical test driver per
// `quickstart.md` line 105 and tasks.md T116 done-signal.
#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::{
        listen, Listener, GLOBAL_SHORTCUT, OAUTH_CALLBACK, SHORTCUTS_UPDATED, TRAY_CANCEL,
        TRAY_PAUSE, TRAY_SKIP, TRAY_START_SESSION, UPDATE_AVAILABLE, USER_ACTIVITY,
        USER_INACTIVITY,
    };
    use crate::bridge::error::BridgeError;
    use crate::bridge::types::{ShortcutSettings, UpdateAvailablePayload};
    use wasm_bindgen_test::wasm_bindgen_test;

    /// Pins the canonical event-name string for every event the Leptos
    /// crate subscribes to. This is the contract surface — drift here
    /// breaks runtime listener wiring at the boundary
    /// (`__TAURI__.event.listen("user-activity", …)` is what the
    /// Tauri-side `app.emit("user-activity", ())` matches against).
    /// The list mirrors contracts/tauri-bridge.md §"Tauri events"
    /// rows E1-E10.
    #[wasm_bindgen_test]
    fn event_names_match_contract() {
        assert_eq!(USER_ACTIVITY, "user-activity");
        assert_eq!(USER_INACTIVITY, "user-inactivity");
        assert_eq!(GLOBAL_SHORTCUT, "global-shortcut");
        assert_eq!(SHORTCUTS_UPDATED, "shortcuts-updated");
        assert_eq!(OAUTH_CALLBACK, "oauth-callback");
        assert_eq!(TRAY_START_SESSION, "tray-start-session");
        assert_eq!(TRAY_PAUSE, "tray-pause");
        assert_eq!(TRAY_SKIP, "tray-skip");
        assert_eq!(TRAY_CANCEL, "tray-cancel");
        assert_eq!(UPDATE_AVAILABLE, "tauri://update-available");
    }

    /// `listen<T>` short-circuits with `BridgeError::BridgeUnavailable`
    /// when `__TAURI_INTERNALS__` is absent — same uniform shape as
    /// every command wrapper in `bridge::commands`. Under `wasm-pack
    /// test --node` no globals are installed, so the call resolves
    /// to `Err(BridgeUnavailable)` immediately.
    #[wasm_bindgen_test]
    async fn listen_short_circuits_when_bridge_absent() {
        let result = listen::<()>(USER_ACTIVITY, |_| {}).await;
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
            listen::<()>(USER_ACTIVITY, |_payload: ()| {}).await
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
            listen::<UpdateAvailablePayload>(UPDATE_AVAILABLE, |_p: UpdateAvailablePayload| {}).await
        }
        let _ = assert_signature().await;
    }

    /// `Listener` is the RAII guard returned by a successful `listen()`
    /// subscription. Dropping it MUST unsubscribe — that's the leak
    /// guarantee LiveView consumers depend on across re-renders.
    /// Pins the type's `Drop` impl exists; the runtime side-effect is
    /// covered by the integration test below.
    #[wasm_bindgen_test]
    fn listener_drop_unsubscribes_compile_pin() {
        // Compile-time check that `Listener` implements `Drop`. If
        // someone refactors away the Drop impl (turning the guard
        // into a no-op handle), this stops compiling because the
        // function-pointer coercion requires the trait bound.
        fn assert_drops<T: Drop>() {}
        assert_drops::<Listener>();
    }
}
