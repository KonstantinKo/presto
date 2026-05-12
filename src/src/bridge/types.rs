// Shared record types that travel across the Tauri bridge.
//
// Spec 001-leptos-migration §Phase 1C; data-model.md §"Shared types — bridge
// boundary". The Tauri-side mirrors live in `src-tauri/src/lib.rs` (today
// they are `PomodoroSession`, `Task`, `ManualSession` — same on-disk wire
// shape). Field-by-field byte-stable serde is the FR-005 invariant: every
// existing 0.4.x JSON file must round-trip through these structs without
// migration.
//
// Closed-domain enums (BridgeError/SessionType/TimerMode) +
// domain records (Session/Task/SessionTag/Tag/ManualSession) live
// in `presto-ipc`. Re-exported here so `bridge::types` is the
// single hub for every type that crosses the IPC boundary.
pub use presto_ipc::{
    BridgeError, ManualSession, Session, SessionTag, SessionType, Tag, Task, TimerMode,
};

// Settings tree (Shortcut/Appearance/Timer/Notification/Advanced
// substructs + `StatusBarDisplay` + `Settings` + `SettingsOnDisk`)
// lives in `presto-ipc::settings`. Re-exported here for path
// stability across the codebase.
pub use presto_ipc::{
    default_analytics_enabled, default_max_session_time, default_weekly_goal, AdvancedSettings,
    AppearanceSettings, NotificationSettings, ShortcutSettings, StatusBarDisplay, TimerSettings,
};

// `Settings` + `SettingsOnDisk` migration shim live in
// `presto-ipc::settings`. Re-exported here for path stability.
pub use presto_ipc::{Settings, SettingsOnDisk};

// `AuthSession`, `AuthUser` live in `presto-ipc::auth`.
pub use presto_ipc::{AuthSession, AuthUser};

// Legacy localStorage migration payloads live in
// `presto-ipc::migration` (feature-gated; both endpoints opt in
// until the post-cutover sunset).
pub use presto_ipc::{
    LegacyHistoryPayload, LegacyManualSessionsPayload, LegacySettingsPayload, LegacyTagsPayload,
    LegacyTasksPayload, LegacyUserStatePayload, SupabaseSessionPayload,
};

// Command Args bundles live in `presto-ipc::args`. Every top-level
// Args struct that crosses the IPC boundary is single-sourced here
// so the camelCase wire shape (Tauri-auto-renames the args bag)
// cannot drift between client and handler.
pub use presto_ipc::{
    AddSessionTagArgs, DeleteTagArgs, StartActivityMonitoringArgs, SupabaseRefreshSessionArgs,
    SupabaseSignOutArgs, UpdateActivityTimeoutArgs, UpdateTrayIconArgs, UpdateTrayMenuArgs,
};

// -----------------------------------------------------------------------
// Tauri event payloads (Phase 1F)
//
// Spec 001-leptos-migration §Phase 1F T116-T117; contracts/tauri-bridge.md
// §"Tauri events". Most events carry a `()` payload (the activity and
// tray-menu emits) or a primitive (`String` for `global-shortcut` and
// `oauth-callback`) — those don't need a dedicated struct. The two
// non-trivial event payloads (`shortcuts-updated` reuses the existing
// `ShortcutSettings` record, defined above; `tauri://update-available`
// carries the updater plugin's emit shape) live here.
// -----------------------------------------------------------------------

/// Payload for the `tauri://update-available` event emitted by
/// `tauri-plugin-updater` when the auto-updater detects a newer
/// release. Mirrors the plugin's `Update` JSON shape.
///
/// Fields mirror the upstream plugin's emit; we deserialise only the
/// three the Leptos consumer (`managers/update.rs`) needs. `serde`'s
/// default-on-unknown-field behaviour silently drops anything else
/// (`available`, `current_version`, etc.) — the contract is the named
/// fields below; future plugin additions are non-breaking.
///
/// `body` is the changelog/release-notes blob (markdown). `date` is
/// the release publish date as the upstream-emitted RFC-2822-ish
/// string; we keep it as `String` to avoid pulling chrono into the
/// event-payload surface.
pub use presto_ipc::UpdateAvailablePayload;

#[cfg(test)]
mod tests {
    //! Lockstep coverage for the `presto-ipc` re-exports.
    //!
    //! The canonical wire-shape tests live with the types they
    //! cover (`crates/presto-ipc/src/*.rs::tests`). This module
    //! holds a single thin lockstep proof — re-exported types
    //! must round-trip through `serde_json` on the WASM-consumer
    //! side too. A failure here means the re-export wiring broke
    //! (very unlikely for `pub use`, but the gate is cheap).

    use super::{Settings, StatusBarDisplay};

    /// Round-trips `Settings::default()` to confirm the re-export
    /// surface is intact and the JSON shape's stable. Catches a
    /// hypothetical drift where `presto-ipc` builds with different
    /// serde features under `wasm32` than under host (the
    /// `migration` feature is the only divergence today; this test
    /// pins the shared default path).
    #[test]
    fn settings_re_export_round_trips_via_serde_json() {
        let s = Settings::default();
        let json = serde_json::to_string(&s).unwrap();
        let decoded: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.timer.focus_duration, 25);
        assert_eq!(decoded.status_bar_display, StatusBarDisplay::Default);
        assert!(json.contains(r#""status_bar_display":"default""#));
    }
}
