// `BridgeAvailable` — one-time read of `window.__TAURI_INTERNALS__`.
//
// Spec 001-leptos-migration §Phase 1B T030-T031; data-model.md §`BridgeAvailable`.
//
// AGENTS.md §Bridge availability and FR-009: every `invoke()` wrapper checks
// this signal; when `Absent`, it short-circuits to a sentinel return value
// (or `BridgeError::BridgeUnavailable`). The check is a single `Reflect::has`
// call against `window`; the function is intentionally cheap so wrappers can
// call it on every invocation rather than caching at process start (the cache
// would then misbehave under hot-reload / dev-server tab reload patterns).

#[cfg(test)]
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
    /// exposes the global object; web_sys::window() still returns None there,
    /// so this test ALSO pins that the implementation must call into js-sys
    /// `global()` / `globalThis` rather than only `web_sys::window()`).
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
