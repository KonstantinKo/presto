// `SettingsManager` — the Rust port of `src/managers/settings-manager.js`.
//
// Spec 001-leptos-migration §Phase 3a (T147-T156). Owns the user's
// `Settings` record, persists via `bridge::commands::{load_settings,
// save_settings}` (Principle VI — managers reach the Tauri side only
// through the typed bridge wrappers), and exposes per-field setters
// for the settings UI. Carries the F1/M3 lockstep migration from the
// legacy `hide_status_bar: bool` field to `status_bar_display:
// StatusBarDisplay` — the custom deserializer in
// `bridge::types::deserialize_status_bar_display_with_legacy_fallback`
// reads either shape on disk and emits only the new shape on save
// (legacy field is dropped; see `data-model.md` §"Settings legacy
// migration").
//
// Lint allowance: `clippy::future_not_send` is allowed at the module
// level for the same reason as on `bridge::commands` — every async
// path here transitively awaits a `JsFuture` from `bridge::commands`,
// and `JsValue` (and everything built on it) is `!Send` by
// construction on `wasm32-unknown-unknown`. The runtime is
// single-threaded; demanding `Send` would force a `!Send`-erasure
// shim that does nothing on the WASM target.
#![allow(clippy::future_not_send)]

use crate::bridge::commands;
use crate::bridge::error::BridgeError;
use crate::bridge::types::Settings;

/// Wrapper over the user's `Settings` record. Phase 3a wires up the
/// state machine; per-field setters and validators arrive in T151-T152.
#[derive(Debug, Clone, Default)]
pub struct SettingsManager {
    /// Current authoritative settings — `Default::default()` until
    /// `load()` lands.
    state: Settings,
}

impl SettingsManager {
    /// Construct a manager with the default settings record. Use
    /// `load()` to seed from disk; use `from_loaded_or_default(...)`
    /// to ingest a `bridge::commands::load_settings()` result while
    /// applying the FR-005 "fall back to default on error" rule.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Settings::default(),
        }
    }

    /// Build a manager from the result of
    /// `bridge::commands::load_settings()` (or any equivalent loader),
    /// falling back to `Settings::default()` on error. Mirrors the
    /// JS-side catch-and-default at
    /// `src/managers/settings-manager.js:125-128`: persistence
    /// failures (missing file, deserialise error, bridge unavailable)
    /// must not poison the manager's state — the user always sees a
    /// usable default until they edit a field that triggers a save.
    #[must_use]
    pub fn from_loaded_or_default(loaded: Result<Settings, BridgeError>) -> Self {
        Self {
            state: loaded.unwrap_or_default(),
        }
    }

    /// Borrow the current settings record.
    #[must_use]
    pub const fn current(&self) -> &Settings {
        &self.state
    }

    /// Async cold-start path: ask the bridge for the persisted settings,
    /// fall back to `Settings::default()` on any error (cold start, bridge
    /// unavailable, corrupted file). Mirrors the JS-side
    /// `SettingsManager.loadSettings` flow at
    /// `src/managers/settings-manager.js:103-129`.
    ///
    /// The wrapper is `async` because the underlying
    /// `bridge::commands::load_settings` is `async`. Tests for the pure
    /// merge logic exercise `from_loaded_or_default` directly to keep the
    /// host test path off the wasm bindgen boundary.
    pub async fn load() -> Self {
        Self::from_loaded_or_default(commands::load_settings().await)
    }
}

#[cfg(test)]
mod tests {
    use super::SettingsManager;

    /// T149 [RED]: an older 0.4.x settings JSON that predates the
    /// `weekly_goal_minutes`, `auto_start_focus`,
    /// `allow_continuous_sessions`, `advanced`, `analytics_enabled`,
    /// `hide_icon_on_close`, and the `status_bar_display` /
    /// `hide_status_bar` field cluster MUST deserialise — the
    /// `#[serde(default)]` markers on each field provide cold-start
    /// values matching `Settings::default()`. Mirrors the Tauri-side
    /// pin at
    /// `src-tauri/src/lib.rs::tests::app_settings_missing_serde_default_fields_use_defaults`.
    ///
    /// Done-signal: this test currently fails because the manager
    /// does not yet expose an `ingest_raw_json` path and because the
    /// `Settings` struct in `bridge::types` carries
    /// `hide_status_bar: bool` rather than the
    /// `status_bar_display: StatusBarDisplay` shape the test asserts.
    /// T150 GREEN lands the field-level migration on both
    /// `presto-web::Settings` and `presto::AppSettings` in lockstep.
    #[test]
    fn missing_serde_default_fields_use_defaults() {
        // Pre-cutover JSON: minimal shape, no nested defaults populated.
        let legacy = r#"{
            "shortcuts": {"start_stop": null, "reset": null, "skip": null},
            "timer": {"focus_duration": 25, "break_duration": 5,
                      "long_break_duration": 20, "total_sessions": 10},
            "notifications": {"desktop_notifications": true,
                              "sound_notifications": true,
                              "auto_start_timer": true, "smart_pause": false,
                              "smart_pause_timeout": 30},
            "autostart": false
        }"#;
        let mgr = SettingsManager::ingest_raw_json(legacy)
            .expect("legacy minimal-shape JSON must deserialise");
        let defaults = crate::bridge::types::Settings::default();

        assert_eq!(
            mgr.current().timer.weekly_goal_minutes,
            defaults.timer.weekly_goal_minutes,
            "weekly_goal_minutes serde default must fire",
        );
        assert_eq!(
            mgr.current().notifications.auto_start_focus,
            defaults.notifications.auto_start_focus,
        );
        assert_eq!(
            mgr.current().notifications.allow_continuous_sessions,
            defaults.notifications.allow_continuous_sessions,
        );
        assert_eq!(mgr.current().advanced.debug_mode, defaults.advanced.debug_mode);
        assert_eq!(mgr.current().analytics_enabled, defaults.analytics_enabled);
        assert_eq!(mgr.current().hide_icon_on_close, defaults.hide_icon_on_close);
        // T151 covers the migration cases for status_bar_display in
        // detail; here we only assert the "neither field present"
        // branch lands at `StatusBarDisplay::default()`.
        assert_eq!(
            mgr.current().status_bar_display,
            crate::bridge::types::StatusBarDisplay::default(),
            "status_bar_display must default when neither legacy nor new field present",
        );
    }

    /// T147 [RED]: when the bridge load returns `Err` (e.g. cold-start
    /// "no settings file" surfaced as `BridgeError::BridgeUnavailable`
    /// in the host test environment, or the Tauri-side fallback path
    /// in production), the manager MUST fall back to
    /// `Settings::default()` per the JS-side behaviour at
    /// `src/managers/settings-manager.js:125-128` (catch → default).
    ///
    /// Done-signal: this test currently fails because
    /// `from_loaded_or_default` does not yet exist. T148 lands the
    /// implementation.
    #[test]
    fn load_returns_default_when_missing_file() {
        // Simulate the Tauri-side cold-start signal: an Err return from
        // `bridge::commands::load_settings`. The manager must produce a
        // record matching `Settings::default()`.
        let manager = SettingsManager::from_loaded_or_default(Err(
            crate::bridge::error::BridgeError::BridgeUnavailable,
        ));
        let defaults = crate::bridge::types::Settings::default();
        assert_eq!(
            manager.current().timer.focus_duration,
            defaults.timer.focus_duration,
            "cold-start load must yield Settings::default()",
        );
        assert_eq!(
            manager.current().timer.weekly_goal_minutes,
            defaults.timer.weekly_goal_minutes,
        );
    }
}
