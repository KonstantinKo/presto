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

    /// Borrow the current settings record.
    #[must_use]
    pub const fn current(&self) -> &Settings {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::SettingsManager;

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
