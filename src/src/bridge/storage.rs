// Legacy localStorage migration — transition-only entry point.
//
// Spec 001-leptos-migration §Phase 1E T099-T115; data-model.md
// §"Legacy localStorage migration"; contracts/tauri-bridge.md
// §"Transition-only commands". This module owns the one-shot
// read-and-clear logic that walks the JS-era `window.localStorage`
// keys, hands the parsed payload to the matching `import_legacy_*`
// Tauri command, and clears the key on success. Idempotent: repeated
// invocations are safe (the Tauri-side handler skips when its
// authoritative store already has data, and the reader treats an
// absent localStorage key as an empty no-op).
//
// Sunset: this module and every `import_legacy_*` wrapper are
// slated for removal one minor version after cutover. The
// constitutional anchor is Principle VII — these are not "indefinite
// parallel surfaces", they are a one-shot migration with a defined
// end-of-life.
//
// Lint allowance: `clippy::future_not_send` is allowed at the module
// level for the same reason it's allowed in `bridge::commands` —
// every async fn here ultimately calls into a wasm-only `JsFuture`,
// and the runtime is single-threaded under `wasm32-unknown-unknown`.
#![allow(clippy::future_not_send)]

// Tests gated on `wasm32` because every assertion is a
// `#[wasm_bindgen_test]` — running them via `cargo test` on the host
// target would produce dead-code lint failures. `wasm-pack test --node`
// is the canonical test driver per quickstart.md line 105 and tasks.md
// T100 done-signal.
#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use wasm_bindgen_test::wasm_bindgen_test;

    /// T100 (RED). The Leptos-side reader for the `pomodoro-settings`
    /// / `theme-preference` / `timer-theme-preference` /
    /// `presto_auto_check_updates` localStorage keys MUST exist as
    /// `import_legacy_settings_from_storage()` and return
    /// `Result<(), BridgeError>`. Under `wasm-pack test --node` no
    /// localStorage is present; the reader must absorb the
    /// no-localStorage case as a successful no-op rather than
    /// surfacing it as a `BridgeUnavailable` error — the migration
    /// entry point runs unconditionally on first launch and a node
    /// test environment must not fail it.
    ///
    /// This test fails at the RED phase because the reader does not
    /// exist yet (compile error: unresolved import).
    #[wasm_bindgen_test]
    async fn imports_legacy_settings() {
        // The function under test is `import_legacy_settings_from_storage`
        // — a Leptos-side reader that walks the four legacy localStorage
        // keys, builds a `LegacySettingsPayload`, and hands it to the
        // `import_legacy_settings` Tauri wrapper. With no localStorage
        // entries (and no Tauri bridge under --node) the reader must
        // absorb both as the cold-start no-op shape.
        let result = super::import_legacy_settings_from_storage().await;
        // Cold-start (no localStorage entries) is the documented
        // success path — Ok(()) with nothing migrated.
        assert!(result.is_ok(), "expected Ok(()) for empty localStorage, got {result:?}");
    }
}
