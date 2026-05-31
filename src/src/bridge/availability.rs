// `BridgeAvailable` — runtime check for `window.__TAURI_INTERNALS__`.
//
// Spec 001-leptos-migration §Phase 1B T030-T031; data-model.md §`BridgeAvailable`.
//
// AGENTS.md §Bridge availability and FR-009: every `invoke()` wrapper checks
// this signal; when `Absent`, it short-circuits to a sentinel return value
// (or `BridgeError::BridgeUnavailable`). The check is a single `Reflect::has`
// call against the global object; the function is intentionally cheap so
// wrappers can call it on every invocation rather than caching at process
// start (the cache would then misbehave under hot-reload / dev-server tab
// reload patterns, and the cost is one Reflect lookup per command — orders
// of magnitude under the IPC trip itself).
//
// The check probes `js_sys::global()` rather than only `web_sys::window()`
// so it works under both browser (where `globalThis === window`) and the
// `wasm-bindgen-test --node` runner (where `web_sys::window()` returns
// `None` but `js_sys::global()` resolves to the node global object). This
// keeps every command-wrapper test runnable under `wasm-pack test --node`.

use wasm_bindgen::JsValue;

/// Indicates whether the Tauri JS bridge (`window.__TAURI_INTERNALS__`) is
/// reachable from the current execution context.
///
/// `Available` ⇒ `invoke()` may be called.
/// `Absent` ⇒ wrappers short-circuit to `BridgeError::BridgeUnavailable`
/// (or to a documented sentinel for read-only commands; see Phase 1G).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeAvailable {
    Available,
    Absent,
}

impl BridgeAvailable {
    /// `true` iff the bridge is *not* present. Lets call sites read as
    /// `if bridge_available().is_absent() { ... }` without verbose match
    /// arms at every wrapper.
    #[must_use]
    pub const fn is_absent(self) -> bool {
        matches!(self, Self::Absent)
    }
}

/// Read the global object for `__TAURI_INTERNALS__` and report whether the
/// Tauri JS bridge is reachable.
///
/// The probe checks `js_sys::global()` (which resolves to `window` in browser
/// contexts and to the node global in the wasm-bindgen-test `--node` runner)
/// for a property named `__TAURI_INTERNALS__`. This matches the Tauri 2.x convention where the
/// internals bag is installed during the webview bootstrap and absent from
/// every other context (Trunk dev server, e2e mock harness, node tests).
#[must_use]
pub fn bridge_available() -> BridgeAvailable {
    let global = js_sys::global();
    let key = JsValue::from_str("__TAURI_INTERNALS__");
    // Use Reflect::has (presence check only); falsy values like 0/false/null
    // still pass since only property existence matters, not its value.
    match js_sys::Reflect::has(&global, &key) {
        Ok(true) => BridgeAvailable::Available,
        _ => BridgeAvailable::Absent,
    }
}

// Tests gated on `wasm32` because every assertion is a
// `#[wasm_bindgen_test]` — running them via `cargo test` on the host
// target would produce dead-code lint failures. `wasm-pack test --node`
// is the canonical test driver per `quickstart.md` line 105 and tasks.md
// T030 done-signal.
#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::{bridge_available, BridgeAvailable};
    use wasm_bindgen_test::wasm_bindgen_test;

    /// Under `wasm-pack test --node`, `web_sys::window()` returns `None` because
    /// node has no DOM. `bridge_available()` MUST report `Absent` in that
    /// environment — this is the documented short-circuit path that every
    /// command wrapper relies on.
    #[wasm_bindgen_test]
    fn reports_absent_when_window_missing() {
        let actual = bridge_available();
        assert_eq!(actual, BridgeAvailable::Absent);
    }

    /// When `window.__TAURI_INTERNALS__` is set on the global object, the
    /// function MUST report `Available`. We install it ourselves via
    /// `Reflect::set` against the node global (since wasm-bindgen's node runner
    /// exposes the global object; `web_sys::window()` still returns `None`
    /// there, so this test ALSO pins that the implementation must call into
    /// `js_sys::global()` / `globalThis` rather than only `web_sys::window()`).
    ///
    /// NOTE for the RED phase: this test will fail because (a) the function
    /// doesn't exist yet, and (b) the implementation must read whichever
    /// global is present.
    #[wasm_bindgen_test]
    fn reports_available_when_internals_present_on_global() {
        // Install the marker on globalThis (the wasm-bindgen-test node runner's
        // top-level scope). We use js_sys::global() because web_sys::window()
        // returns None under --node.
        let global = js_sys::global();
        let key = wasm_bindgen::JsValue::from_str("__TAURI_INTERNALS__");
        let stub = js_sys::Object::new();
        js_sys::Reflect::set(&global, &key, &stub).expect("install stub");

        let actual = bridge_available();

        // Clean up so other tests aren't poisoned.
        js_sys::Reflect::delete_property(&global, &key).expect("remove stub");

        assert_eq!(actual, BridgeAvailable::Available);
    }

    /// `BridgeAvailable` must implement `From<bool>` (or equivalent
    /// boolean-coercion ergonomics) so wrappers can write
    /// `if bridge_available().is_absent() { ... }` without verbose matches.
    #[wasm_bindgen_test]
    fn exposes_is_absent_helper() {
        // Pure compile-time + runtime assertion — proves the API shape exists.
        assert!(BridgeAvailable::Absent.is_absent());
        assert!(!BridgeAvailable::Available.is_absent());
    }
}
