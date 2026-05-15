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
use crate::bridge::types::BridgeError;
use crate::bridge::types::Settings;

/// Wrapper over the user's `Settings` record. Phase 3a wires up the
/// state machine; per-field setters and validators arrive in T151-T152.
#[derive(Debug, Clone, Default)]
pub struct SettingsManager {
    /// Current authoritative settings — `Default::default()` until
    /// `load()` lands.
    state: Settings,
    /// `true` iff the in-memory state diverges from what the on-disk
    /// shape was when the manager last ingested it (FR-005 idempotent
    /// migration). Set by `ingest_raw_json` whenever the input JSON
    /// is missing a `#[serde(default)]`-marked field, carries a
    /// legacy `hide_status_bar` field instead of the new
    /// `status_bar_display`, or otherwise differs structurally from
    /// the canonical post-cutover wire shape. The components layer
    /// (Phase 4) honours this flag by triggering a save after the
    /// next user-driven mutation, mirroring the JS-side
    /// `scheduleAutoSave()` call at
    /// `src/managers/settings-manager.js:116`.
    needs_writeback: bool,
}

impl SettingsManager {
    /// Construct a manager with the default settings record. Use
    /// `load()` to seed from disk; use `from_loaded_or_default(...)`
    /// to ingest a `bridge::commands::load_settings()` result while
    /// applying the FR-005 "fall back to default on error" rule.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a manager from the result of
    /// `bridge::commands::load_settings()` (or any equivalent loader),
    /// falling back to `Settings::default()` on error. Mirrors the
    /// JS-side catch-and-default at
    /// `src/managers/settings-manager.js:125-128`: persistence
    /// failures (missing file, deserialise error, bridge unavailable)
    /// must not poison the manager's state — the user always sees a
    /// usable default until they edit a field that triggers a save.
    ///
    /// Cold-start fallback never sets `needs_writeback`: there is no
    /// legacy file to migrate when the load itself failed.
    #[must_use]
    pub fn from_loaded_or_default(loaded: Result<Settings, BridgeError>) -> Self {
        Self {
            state: loaded.unwrap_or_default(),
            needs_writeback: false,
        }
    }

    /// Borrow the current settings record.
    #[must_use]
    pub const fn current(&self) -> &Settings {
        &self.state
    }

    /// `true` iff the manager's last `ingest_raw_json` call observed a
    /// non-canonical on-disk shape (a missing `#[serde(default)]`
    /// field, or a legacy `hide_status_bar` carried over). The
    /// caller is expected to schedule a save once it's safe to do so;
    /// the next ingest of the resulting canonical payload returns
    /// `false`, matching the JS-side
    /// `scheduleAutoSave → save → re-load → no schedule` cycle.
    ///
    /// Spec 001-leptos-migration §Phase 3a T156; FR-005 idempotent
    /// migration path.
    #[must_use]
    pub const fn needs_writeback(&self) -> bool {
        self.needs_writeback
    }

    /// Ingest a raw on-disk settings JSON document. Mirrors the
    /// JS-side `mergeWithDefaults` flow at
    /// `src/managers/settings-manager.js:133-147` plus the
    /// `hide_status_bar → status_bar_display` migration step at
    /// lines 109-119: the typed deserialize on
    /// `bridge::types::Settings` carries the F1/M3 migration via
    /// `#[serde(default)]` markers (T150) and the custom legacy
    /// fallback (T152), so the resulting `Settings` is always in the
    /// post-cutover shape regardless of which 0.4.x revision wrote
    /// the file.
    ///
    /// Sets `needs_writeback` if the input JSON's normalised shape
    /// differs from the canonical post-cutover save shape (FR-005
    /// idempotent migration path; T156).
    ///
    /// # Errors
    /// Returns `serde_json::Error` if the input is not valid JSON or
    /// is missing required fields that have no `#[serde(default)]`
    /// (such as `shortcuts`, `timer`, `notifications`, `autostart` —
    /// every released 0.4.x build emits these).
    pub fn ingest_raw_json(raw: &str) -> Result<Self, serde_json::Error> {
        // Parse the input twice: once into the typed `Settings`
        // (which runs every projection: serde defaults, the
        // `SettingsOnDisk` legacy `hide_status_bar` fallback, etc.),
        // and once into a structural `serde_json::Value`. Re-emit the
        // typed value as JSON, parse THAT to a `Value`, and compare:
        // any structural divergence (missing serde-default field,
        // legacy `hide_status_bar` carried over, unknown extra keys)
        // means the next save would write a different shape — i.e.
        // the migration is non-idempotent for THIS particular
        // on-disk file and we owe it a writeback.
        let state: Settings = serde_json::from_str(raw)?;
        let input_value: serde_json::Value = serde_json::from_str(raw)?;
        let canonical_str = serde_json::to_string(&state)?;
        let canonical_value: serde_json::Value = serde_json::from_str(&canonical_str)?;
        let needs_writeback = input_value != canonical_value;
        Ok(Self {
            state,
            needs_writeback,
        })
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

    /// Serialise the current settings as a JSON document, matching the
    /// wire shape `bridge::commands::save_settings` will hand to the
    /// Tauri side. Pure helper — used by tests + by the async `save()`
    /// wrapper for diagnostics. The legacy `hide_status_bar` field is
    /// not emitted (the derived `Serialize` on `Settings` has no field
    /// for it), satisfying the FR-005 idempotent migration's second
    /// half: once read, the legacy shape is gone from disk on next save.
    ///
    /// # Errors
    /// Returns `serde_json::Error` if any nested value resists
    /// serialisation. In practice this never happens for the shapes
    /// we control, but we surface the error rather than panic so the
    /// caller can decide whether to retry or fall back.
    pub fn save_payload_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.state)
    }

    /// Async save path: hand the current settings to
    /// `bridge::commands::save_settings`. Per Principle VI, the manager
    /// reaches the Tauri side only through the typed bridge wrapper.
    /// Mirrors the JS-side `SettingsManager.saveSettings` flow at
    /// `src/managers/settings-manager.js` (the bridge invocation; the
    /// JS-side debounce is a UI concern that lives in the components
    /// layer in Phase 4).
    ///
    /// # Errors
    /// Returns whatever `bridge::commands::save_settings` returns —
    /// `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
    /// not present, or whichever variant the Tauri-side handler maps
    /// its filesystem failure to.
    pub async fn save(&self) -> Result<(), BridgeError> {
        commands::save_settings(self.state.clone()).await
    }
}

#[cfg(test)]
mod tests {
    use super::SettingsManager;

    /// T155 [RED]: FR-005 idempotent migration. When a settings JSON
    /// is read that:
    ///
    /// - lacks any `#[serde(default)]`-marked field, OR
    /// - carries the legacy `hide_status_bar` field instead of the
    ///   new `status_bar_display` field,
    ///
    /// the manager MUST flag the in-memory state as needing
    /// writeback. After the writeback, the on-disk shape is the
    /// post-cutover canonical shape, and a subsequent load is a
    /// no-op (no further writeback needed). Mirrors the JS-side
    /// `scheduleAutoSave()` call at
    /// `src/managers/settings-manager.js:116` that runs after the
    /// `hide_status_bar → status_bar_display` migration step.
    ///
    /// Done-signal: this test currently fails because
    /// `SettingsManager::needs_writeback` does not yet exist.
    /// T156 GREEN attaches the dirty-flag tracking via the
    /// `ingest_raw_json` path.
    #[test]
    fn idempotent_missing_field_migration_writes_back() {
        // Case A: legacy `hide_status_bar` field present.
        let legacy = r#"{
            "shortcuts": {"start_stop": null, "reset": null, "skip": null},
            "timer": {"focus_duration": 25, "break_duration": 5,
                      "long_break_duration": 20, "total_sessions": 10},
            "notifications": {"desktop_notifications": true,
                              "sound_notifications": true,
                              "auto_start_timer": true, "smart_pause": false,
                              "smart_pause_timeout": 30},
            "autostart": false,
            "hide_status_bar": true
        }"#;
        let mgr = SettingsManager::ingest_raw_json(legacy).expect("legacy shape must deserialise");
        assert!(
            mgr.needs_writeback(),
            "legacy hide_status_bar present must flag writeback",
        );

        // Now save and re-read — the second load is the canonical
        // shape and must NOT flag writeback.
        let canonical_payload = mgr.save_payload_json().expect("must serialise");
        let mgr2 = SettingsManager::ingest_raw_json(&canonical_payload)
            .expect("canonical shape must deserialise");
        assert!(
            !mgr2.needs_writeback(),
            "canonical post-save shape must NOT flag writeback (idempotent)",
        );

        // Case B: missing `weekly_goal_minutes` (a #[serde(default)]
        // field) on a settings JSON that's otherwise canonical.
        let missing_weekly_goal = r#"{
            "shortcuts": {"start_stop": null, "reset": null, "skip": null},
            "timer": {"focus_duration": 25, "break_duration": 5,
                      "long_break_duration": 20, "total_sessions": 10},
            "notifications": {"desktop_notifications": true,
                              "sound_notifications": true,
                              "auto_start_timer": true,
                              "auto_start_focus": false,
                              "allow_continuous_sessions": false,
                              "smart_pause": false,
                              "smart_pause_timeout": 30},
            "advanced": {"debug_mode": false},
            "autostart": false,
            "hide_icon_on_close": false,
            "status_bar_display": "default"
        }"#;
        let mgr = SettingsManager::ingest_raw_json(missing_weekly_goal).expect("must deserialise");
        assert!(
            mgr.needs_writeback(),
            "missing weekly_goal_minutes must flag writeback",
        );

        // Case C: full canonical shape with every field present —
        // must NOT flag writeback.
        let canonical = r#"{
            "shortcuts": {"start_stop": null, "reset": null, "skip": null, "abort": null},
            "timer": {"focus_duration": 25, "break_duration": 5,
                      "long_break_duration": 20, "total_sessions": 10,
                      "weekly_goal_minutes": 125, "max_session_time": 120,
                      "sessions_per_long_break": 4},
            "notifications": {"desktop_notifications": true,
                              "sound_notifications": true,
                              "auto_start_timer": true,
                              "auto_start_focus": false,
                              "allow_continuous_sessions": false,
                              "smart_pause": false,
                              "smart_pause_timeout": 30,
                              "metronome": false,
                              "ambient_sound_enabled": false,
                              "ambient_sound_type": "none",
                              "ambient_sound_volume": 50},
            "advanced": {"debug_mode": false},
            "appearance": {"theme": "auto", "timer_theme": "espresso", "locale": null},
            "autostart": false,
            "hide_icon_on_close": false,
            "status_bar_display": "default",
            "skipped_versions": []
        }"#;
        let mgr = SettingsManager::ingest_raw_json(canonical).expect("must deserialise");
        assert!(
            !mgr.needs_writeback(),
            "canonical full shape must NOT flag writeback",
        );
    }

    /// T153 [RED]: a save round-trip must produce the post-cutover
    /// wire shape only — the legacy `hide_status_bar` field never
    /// re-appears once the manager has read it. The JS-side
    /// behaviour at `src/managers/settings-manager.js:114-120` is
    /// "migrate, then save"; this Rust port achieves the same
    /// outcome via the `#[serde(from = "SettingsOnDisk")]` shim
    /// (T152) — `Settings` itself has no `hide_status_bar` field, so
    /// the derived `Serialize` impl cannot emit it.
    ///
    /// Done-signal: this test currently fails because
    /// `SettingsManager::save_payload_json` does not yet exist.
    /// T154 GREEN attaches the helper that the (future, async)
    /// `save()` wrapper will hand to `bridge::commands::save_settings`.
    #[test]
    fn save_writes_full_shape_drops_legacy_field() {
        // Start from a legacy on-disk shape that carries the old
        // field; ingest, then save. The serialized payload must
        // contain the new field (kebab-case) and not the legacy one.
        let legacy = r#"{
            "shortcuts": {"start_stop": null, "reset": null, "skip": null},
            "timer": {"focus_duration": 25, "break_duration": 5,
                      "long_break_duration": 20, "total_sessions": 10},
            "notifications": {"desktop_notifications": true,
                              "sound_notifications": true,
                              "auto_start_timer": true, "smart_pause": false,
                              "smart_pause_timeout": 30},
            "autostart": false,
            "hide_status_bar": true
        }"#;
        let mgr = SettingsManager::ingest_raw_json(legacy).expect("legacy shape must deserialise");
        let payload = mgr.save_payload_json().expect("must serialise");

        assert!(
            !payload.contains("hide_status_bar"),
            "save payload must not contain the legacy field; got {payload}",
        );
        assert!(
            payload.contains(r#""status_bar_display":"icon-only""#),
            "save payload must carry the post-migration kebab-case shape; got {payload}",
        );
    }

    /// T151 [RED]: F1/M3 migration coverage — the five cases from
    /// data-model.md §"Settings legacy migration":
    ///
    /// 1. `hide_status_bar: true → IconOnly`
    /// 2. `hide_status_bar: false → Default`
    /// 3. `status_bar_display: "icon-only" → IconOnly` (kebab-case
    ///    round-trip from a pre-cutover JS-era settings JSON)
    /// 4. `status_bar_display: "default" → Default`
    /// 5. neither field present → `Default`
    ///
    /// Mirrors the JS-side migration logic at
    /// `src/managers/settings-manager.js:109-119` ported to Rust.
    ///
    /// Done-signal: this test currently fails because cases 1 and 2
    /// require the custom deserializer
    /// `deserialize_status_bar_display_with_legacy_fallback` that
    /// T152 GREEN attaches to `Settings.status_bar_display`. The
    /// other three cases pass with the default-only T150 shape and
    /// land here so they regress loud if the deserializer ever drops
    /// them. T152 implements the fallback.
    #[test]
    fn migrates_hide_status_bar_to_status_bar_display() {
        use crate::bridge::types::StatusBarDisplay;

        // Builds a minimal-shape settings JSON with an arbitrary
        // status-bar fragment spliced in. Keeps every other field at
        // its default so each case isolates the migration logic.
        let make_json = |status_bar_fragment: &str| {
            format!(
                r#"{{
                    "shortcuts": {{"start_stop": null, "reset": null, "skip": null}},
                    "timer": {{"focus_duration": 25, "break_duration": 5,
                              "long_break_duration": 20, "total_sessions": 10}},
                    "notifications": {{"desktop_notifications": true,
                                      "sound_notifications": true,
                                      "auto_start_timer": true, "smart_pause": false,
                                      "smart_pause_timeout": 30}},
                    "autostart": false{status_bar_fragment}
                }}"#
            )
        };

        // Case 1: legacy `hide_status_bar: true → IconOnly`.
        let mgr = SettingsManager::ingest_raw_json(&make_json(r#", "hide_status_bar": true"#))
            .expect("legacy true must deserialise");
        assert_eq!(
            mgr.current().status_bar_display,
            StatusBarDisplay::IconOnly,
            "case 1: hide_status_bar:true must project to IconOnly",
        );

        // Case 2: legacy `hide_status_bar: false → Default`.
        let mgr = SettingsManager::ingest_raw_json(&make_json(r#", "hide_status_bar": false"#))
            .expect("legacy false must deserialise");
        assert_eq!(
            mgr.current().status_bar_display,
            StatusBarDisplay::Default,
            "case 2: hide_status_bar:false must project to Default",
        );

        // Case 3: kebab-case round-trip from a JS-era settings JSON
        // that already carries the new field. Pre-cutover JS-side
        // migration step had already started writing this shape.
        let mgr =
            SettingsManager::ingest_raw_json(&make_json(r#", "status_bar_display": "icon-only""#))
                .expect("new shape kebab-case must deserialise");
        assert_eq!(
            mgr.current().status_bar_display,
            StatusBarDisplay::IconOnly,
            "case 3: status_bar_display:\"icon-only\" must round-trip to IconOnly",
        );

        // Case 4: kebab-case `status_bar_display: "default" → Default`.
        let mgr =
            SettingsManager::ingest_raw_json(&make_json(r#", "status_bar_display": "default""#))
                .expect("new shape default kebab-case must deserialise");
        assert_eq!(
            mgr.current().status_bar_display,
            StatusBarDisplay::Default,
            "case 4: status_bar_display:\"default\" must round-trip to Default",
        );

        // Case 5: neither field present → `Default::default()`.
        let mgr = SettingsManager::ingest_raw_json(&make_json(""))
            .expect("neither field must deserialise");
        assert_eq!(
            mgr.current().status_bar_display,
            StatusBarDisplay::Default,
            "case 5: missing field must default to Default",
        );

        // Bonus pin: when both fields are present, the new field
        // wins (matches the JS-side behaviour of preferring the new
        // shape and only running the migration when
        // `status_bar_display === undefined`).
        let mgr = SettingsManager::ingest_raw_json(&make_json(
            r#", "hide_status_bar": true, "status_bar_display": "default""#,
        ))
        .expect("both fields must deserialise");
        assert_eq!(
            mgr.current().status_bar_display,
            StatusBarDisplay::Default,
            "tie-breaker: when both fields present, new field wins",
        );
    }

    /// T149 [RED]: an older 0.4.x settings JSON that predates the
    /// `weekly_goal_minutes`, `auto_start_focus`,
    /// `allow_continuous_sessions`, `advanced`,
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
        assert_eq!(
            mgr.current().advanced.debug_mode,
            defaults.advanced.debug_mode
        );
        assert_eq!(
            mgr.current().hide_icon_on_close,
            defaults.hide_icon_on_close
        );
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
            crate::bridge::types::BridgeError::BridgeUnavailable,
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
