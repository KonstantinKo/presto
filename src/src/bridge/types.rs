// Shared record types that travel across the Tauri bridge.
//
// Spec 001-leptos-migration §Phase 1C; data-model.md §"Shared types — bridge
// boundary". The Tauri-side mirrors live in `src-tauri/src/lib.rs` (today
// they are `PomodoroSession`, `Task`, `ManualSession` — same on-disk wire
// shape). Field-by-field byte-stable serde is the FR-005 invariant: every
// existing 0.4.x JSON file must round-trip through these structs without
// migration.
//
// Closed-domain enums (`SessionType`, `TimerMode`) live in their own modules
// (Phase 1A T028-T029); this file holds the *records* that embed them.

use serde::{Deserialize, Serialize};

use super::session_type::SessionType;
use super::timer_mode::TimerMode;

/// Pomodoro session record persisted in the user's app-data directory.
/// Mirrors `PomodoroSession` at `src-tauri/src/lib.rs:142-148`.
///
/// On-disk shape: `snake_case` JSON via serde's default field naming. The
/// `date` field is the chrono format `%a %b %d %Y` (matches JS
/// `Date.prototype.toDateString()` exact-byte; pinned by
/// `engine::date_format` in Phase 2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub completed_pomodoros: u32,
    /// Seconds.
    pub total_focus_time: u32,
    pub current_session: u32,
    /// `%a %b %d %Y` (e.g., "Sat May 10 2026").
    pub date: String,
}

/// Task record on the user's task list. Mirrors `Task` at
/// `src-tauri/src/lib.rs:184-191`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: u64,
    pub text: String,
    pub completed: bool,
    pub created_at: String,
    pub completed_at: Option<String>,
}

/// Per-session per-tag time-spent join row. Mirrors `SessionTag` at
/// `src-tauri/src/lib.rs:177-182`.
///
/// On-disk shape: `snake_case` JSON via serde's default field naming.
/// `duration` is wall-clock seconds spent on this tag during the named
/// session — distinct from `ManualSession::duration` (minutes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTag {
    pub session_id: String,
    pub tag_id: String,
    /// Seconds.
    pub duration: u32,
    pub created_at: String,
}

/// User-defined tag attached to sessions and manual entries. Mirrors `Tag`
/// at `src-tauri/src/lib.rs:167-174`.
///
/// On-disk shape: `snake_case` JSON via serde's default field naming. The
/// `icon` field carries either an emoji or a Remix icon class (e.g.,
/// `"ri-briefcase-line"`); `color` is a hex string (e.g., `"#3b82f6"`).
/// Both are pinned to `String` because the JS-era on-disk records mix
/// the two conventions and a closed-domain enum would force a migration
/// (FR-005 — no on-disk shape change).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub color: String,
    pub created_at: String,
}

/// User-entered manual session record.
///
/// Mirrors `ManualSession` at `src-tauri/src/lib.rs:154-165`. `session_type`
/// is the closed-domain `SessionType` per spec 001 T029 (was a
/// stringly-typed `String` pre-cutover).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualSession {
    pub id: String,
    pub session_type: SessionType,
    /// Minutes.
    pub duration: u32,
    /// `HH:MM`.
    pub start_time: String,
    /// `HH:MM`.
    pub end_time: String,
    pub notes: Option<String>,
    /// ISO-8601.
    pub created_at: String,
    /// `%a %b %d %Y`.
    pub date: String,
    /// Inline tag objects per the existing JS-era on-disk shape (FR-005).
    /// Kept loosely typed because the legacy records embed full tag objects
    /// rather than ID-only references; we normalise at consumption time
    /// without reshaping on disk.
    pub tags: Option<Vec<serde_json::Value>>,
}

/// Keyboard-shortcut bindings bundle. Mirrors `ShortcutSettings` at
/// `src-tauri/src/lib.rs:212-217`.
///
/// Each field is `Option<String>` because users can clear a binding
/// (the JS era stores `null` for cleared bindings, which serde maps to
/// `None`). Each string is a Tauri shortcut spec like
/// `"CommandOrControl+Alt+Space"`; parsing happens Rust-side at
/// `register_global_shortcuts` time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutSettings {
    pub start_stop: Option<String>,
    pub reset: Option<String>,
    pub skip: Option<String>,
}

impl Default for ShortcutSettings {
    fn default() -> Self {
        Self {
            start_stop: Some("CommandOrControl+Alt+Space".to_string()),
            reset: Some("CommandOrControl+Alt+R".to_string()),
            skip: Some("CommandOrControl+Alt+S".to_string()),
        }
    }
}

/// Appearance / theme preferences.
///
/// `theme` is the color-mode preference ("auto" / "light" / "dark");
/// `timer_theme` is the timer palette stem (e.g. "espresso"). Mirrors
/// `AppearanceSettings` in `src-tauri/src/lib.rs` byte-for-byte on the wire
/// (FR-005 / FR-008 lockstep discipline).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceSettings {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_timer_theme")]
    pub timer_theme: String,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            timer_theme: default_timer_theme(),
        }
    }
}

fn default_theme() -> String {
    "auto".to_string()
}

fn default_timer_theme() -> String {
    "espresso".to_string()
}

/// Timer durations & session count. Mirrors `TimerSettings` at
/// `src-tauri/src/lib.rs`.
///
/// `weekly_goal_minutes` and `max_session_time` carry
/// `#[serde(default = "...")]` because settings JSON written by
/// pre-widening builds lacks those fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerSettings {
    /// Minutes.
    pub focus_duration: u32,
    /// Minutes.
    pub break_duration: u32,
    /// Minutes.
    pub long_break_duration: u32,
    pub total_sessions: u32,
    #[serde(default = "default_weekly_goal")]
    pub weekly_goal_minutes: u32,
    /// Maximum continuous session time before auto-pause (minutes).
    #[serde(default = "default_max_session_time")]
    pub max_session_time: u32,
}

impl Default for TimerSettings {
    fn default() -> Self {
        Self {
            focus_duration: 25,
            break_duration: 5,
            long_break_duration: 20,
            total_sessions: 10,
            weekly_goal_minutes: default_weekly_goal(),
            max_session_time: default_max_session_time(),
        }
    }
}

const fn default_weekly_goal() -> u32 {
    125
}

const fn default_max_session_time() -> u32 {
    120
}

const fn default_analytics_enabled() -> bool {
    true
}

/// Notification preferences. Mirrors `NotificationSettings` at
/// `src-tauri/src/lib.rs:259-270`.
///
/// `auto_start_focus` and `allow_continuous_sessions` carry
/// `#[serde(default)]` because they were added after the `0.4.0`
/// settings shape and may be missing from older settings JSONs.
///
/// `clippy::struct_excessive_bools` is allowed targeted-fashion: every
/// bool here maps to an independent UI toggle (the same rationale the
/// Tauri-side mirror at `src-tauri/src/lib.rs:258` uses), so collapsing
/// them into a state-machine enum would not match either the JSON
/// shape on disk (FR-005) or the settings UI grouping.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSettings {
    pub desktop_notifications: bool,
    pub sound_notifications: bool,
    pub auto_start_timer: bool,
    #[serde(default)]
    pub auto_start_focus: bool,
    #[serde(default)]
    pub allow_continuous_sessions: bool,
    pub smart_pause: bool,
    /// Seconds.
    pub smart_pause_timeout: u32,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            desktop_notifications: true,
            sound_notifications: true,
            auto_start_timer: true,
            auto_start_focus: false,
            allow_continuous_sessions: false,
            smart_pause: false,
            smart_pause_timeout: 30,
        }
    }
}

/// Advanced / debug toggles. Mirrors `AdvancedSettings` at
/// `src-tauri/src/lib.rs:272-276`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AdvancedSettings {
    #[serde(default)]
    pub debug_mode: bool,
}

/// Status-bar visibility mode.
///
/// Replaces the legacy `hide_status_bar: bool` shape with a typed
/// enum so future "compact" or "hidden" modes don't fork the on-disk
/// encoding (data-model.md §"Settings legacy migration"; F1/M3
/// lockstep migration).
///
/// Wire shape: kebab-case strings (`"default"`, `"icon-only"`),
/// matching the JS-era on-disk values written by
/// `src/managers/settings-manager.js` after its `hide_status_bar →
/// status_bar_display` migration step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum StatusBarDisplay {
    /// Full status bar (timer text + icon).
    #[default]
    Default,
    /// Icon-only status bar — corresponds to the legacy
    /// `hide_status_bar: true` setting.
    IconOnly,
}

/// Full application settings record. Mirrors `AppSettings` at
/// `src-tauri/src/lib.rs`.
///
/// **Wire shape (post-Phase-3a)**: the legacy `hide_status_bar: bool`
/// field is replaced by `status_bar_display: StatusBarDisplay` per the
/// F1/M3 lockstep migration (Phase 3a T150 / T152 — the same commit
/// pair tightens the Tauri-side `AppSettings`). Legacy 0.4.x settings
/// JSONs that still carry `hide_status_bar` are read by the
/// `#[serde(from = "SettingsOnDisk")]` shim below: it accepts either
/// shape on the wire, projects through the legacy fallback, and the
/// derived `Serialize` impl on `Settings` then emits only the new
/// shape on next save (legacy field is gone — there is no field for
/// it on the struct).
///
/// `clippy::struct_excessive_bools` is allowed here for the same
/// reason as on `NotificationSettings` and on the Tauri-side mirror:
/// each bool is an independent settings toggle exposed in the UI;
/// restructuring would not match either the on-disk JSON shape or
/// the settings-page layout.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "SettingsOnDisk")]
pub struct Settings {
    pub shortcuts: ShortcutSettings,
    pub timer: TimerSettings,
    pub notifications: NotificationSettings,
    #[serde(default)]
    pub advanced: AdvancedSettings,
    #[serde(default)]
    pub appearance: AppearanceSettings,
    pub autostart: bool,
    #[serde(default = "default_analytics_enabled")]
    pub analytics_enabled: bool,
    #[serde(default)]
    pub hide_icon_on_close: bool,
    pub status_bar_display: StatusBarDisplay,
    // Phase 4e R-002: user-state slice — mirrors the Tauri-side
    // `AppSettings` widening byte-for-byte. The JS-era
    // `presto-guest-mode` / `presto-auth-seen` /
    // `presto-skipped-versions` localStorage flags fold into these
    // fields on first post-cutover launch (see
    // `bridge::storage::import_legacy_user_state_from_storage` and
    // `migration.rs::import_user_state`). Once migrated, `guest_mode`
    // is the canonical signal; the localStorage fallback at
    // `managers::auth::WebGuestModeStore` is kept only as a
    // belt-and-braces fallback for sandboxed origins where the
    // Tauri bridge round-trip didn't complete.
    //
    // Each field is `#[serde(default)]` so 0.4.x settings JSONs
    // predating this widening still deserialise into the cold-start
    // shape (`guest_mode: false`, `auth_seen: false`,
    // `skipped_versions: vec![]`).
    #[serde(default)]
    pub guest_mode: bool,
    #[serde(default)]
    pub auth_seen: bool,
    #[serde(default)]
    pub skipped_versions: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            shortcuts: ShortcutSettings::default(),
            timer: TimerSettings::default(),
            notifications: NotificationSettings::default(),
            advanced: AdvancedSettings::default(),
            appearance: AppearanceSettings::default(),
            autostart: false,
            analytics_enabled: true,
            hide_icon_on_close: false,
            status_bar_display: StatusBarDisplay::Default,
            // Phase 4e R-002 cold-start defaults — see field-level
            // doc-comment for the rationale.
            guest_mode: false,
            auth_seen: false,
            skipped_versions: Vec::new(),
        }
    }
}

/// On-disk shape of the settings JSON, accepting either the new
/// `status_bar_display: StatusBarDisplay` field or the legacy
/// `hide_status_bar: bool` field. Used as the
/// `#[serde(from = "SettingsOnDisk")]` source for `Settings`; the
/// `From<SettingsOnDisk> for Settings` impl below ports the legacy
/// fallback from `src/managers/settings-manager.js:109-119`:
///
/// 1. If `status_bar_display` is present, use it.
/// 2. Else if `hide_status_bar: true`, use `IconOnly`.
/// 3. Else if `hide_status_bar: false`, use `Default`.
/// 4. Else, use `StatusBarDisplay::default()` (i.e. `Default`).
///
/// Tie-breaker: when both fields are present, the new field wins.
/// This matches the JS-side behaviour at lines 111-113, where the
/// migration only runs when `loadedSettings.status_bar_display ===
/// undefined`.
///
/// Spec 001-leptos-migration §Phase 3a T152;
/// data-model.md §"Settings legacy migration".
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Deserialize)]
struct SettingsOnDisk {
    shortcuts: ShortcutSettings,
    timer: TimerSettings,
    notifications: NotificationSettings,
    #[serde(default)]
    advanced: AdvancedSettings,
    #[serde(default)]
    appearance: AppearanceSettings,
    autostart: bool,
    #[serde(default = "default_analytics_enabled")]
    analytics_enabled: bool,
    #[serde(default)]
    hide_icon_on_close: bool,
    #[serde(default)]
    status_bar_display: Option<StatusBarDisplay>,
    /// Legacy field — read-only fallback. The struct that
    /// `Settings` deserialises into never re-emits this on save
    /// because the post-conversion `Settings` has no field for it.
    #[serde(default)]
    hide_status_bar: Option<bool>,
    /// Phase 4e R-002 user-state slice (`#[serde(default)]` so the
    /// pre-widening shape still deserialises). See `Settings`
    /// doc-comment for per-field semantics.
    #[serde(default)]
    guest_mode: bool,
    #[serde(default)]
    auth_seen: bool,
    #[serde(default)]
    skipped_versions: Vec<String>,
}

impl From<SettingsOnDisk> for Settings {
    fn from(raw: SettingsOnDisk) -> Self {
        let status_bar_display = raw.status_bar_display.unwrap_or(match raw.hide_status_bar {
            Some(true) => StatusBarDisplay::IconOnly,
            Some(false) | None => StatusBarDisplay::Default,
        });
        Self {
            shortcuts: raw.shortcuts,
            timer: raw.timer,
            notifications: raw.notifications,
            advanced: raw.advanced,
            appearance: raw.appearance,
            autostart: raw.autostart,
            analytics_enabled: raw.analytics_enabled,
            hide_icon_on_close: raw.hide_icon_on_close,
            status_bar_display,
            guest_mode: raw.guest_mode,
            auth_seen: raw.auth_seen,
            skipped_versions: raw.skipped_versions,
        }
    }
}

/// Supabase auth session record. Mirrors data-model.md §`Session (Supabase
/// auth session — distinct from pomodoro `Session`)`.
///
/// Phase 1D T088-T089: replaces the JS `supabase-js` SDK's `Session` type.
/// Persisted Rust-side per research.md §6 (the JS-era localStorage
/// `sb-<project-ref>-auth-token` shape moves to the app-data dir on
/// first post-cutover launch). Distinct from `Session` (the pomodoro
/// session record above) by design — the data-model.md collision note
/// renames this to `AuthSession` so the two types never conflict at a
/// call site that imports both.
///
/// Wire shape: `snake_case` JSON via serde's default field naming. Matches
/// supabase-js's REST response shape directly so the Rust adapter
/// (`src-tauri/src/auth.rs`) can deserialise the `/auth/v1/token`
/// response into this struct without a translation layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSession {
    pub access_token: String,
    pub refresh_token: String,
    pub user: AuthUser,
}

/// Supabase auth user record embedded in `AuthSession`. Mirrors
/// data-model.md §`AuthUser`.
///
/// `user_metadata` is intentionally `serde_json::Value` (not a typed
/// struct) because Supabase's metadata is open-ended — apps store
/// per-tenant fields like `full_name`, `avatar_url`, OAuth-provider
/// claims, etc. The Leptos consumers (`managers/auth.rs`) read specific
/// keys via `.get("full_name")` rather than imposing a closed shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub id: String,
    pub email: String,
    pub user_metadata: serde_json::Value,
}

// -----------------------------------------------------------------------
// Legacy localStorage migration payloads (Phase 1E, transition-only).
//
// Spec 001-leptos-migration §Phase 1E T099-T115; data-model.md
// §"Legacy localStorage migration"; contracts/tauri-bridge.md
// §"Transition-only commands". Each `Legacy*Payload` mirrors the
// JS-era localStorage shape for one preserved domain. The Leptos-side
// reader (`bridge::storage`) parses the localStorage value into the
// matching payload and hands it to the matching `import_legacy_*`
// Tauri command. The Tauri handler is idempotent: if the
// authoritative Rust-side store already has data, the import is a
// successful no-op.
//
// Sunset: every `Legacy*Payload` and every `import_legacy_*` wrapper
// is slated for removal one minor version after cutover. Principle
// VII anchor: this is a one-shot migration with a defined sunset,
// not an indefinite parallel surface.
// -----------------------------------------------------------------------

/// JS-era `pomodoro-settings` localStorage shape, plus the four
/// preference flags split out into separate keys
/// (`theme-preference`, `timer-theme-preference`,
/// `presto_auto_check_updates`).
///
/// The core settings JSON shape is identical to the post-cutover
/// `Settings` record (FR-005 — no on-disk shape change), so we
/// reuse `Settings` as the embedded core. The four preference flags
/// are flattened on top because the JS era stored each in its own
/// localStorage key rather than inside the settings JSON; the
/// Tauri-side `import_legacy_settings` handler folds them into the
/// existing `AppSettings` shape (theme/timer-theme are not yet
/// represented in `AppSettings` and are dropped on import — they
/// live as user preferences in a later phase, per
/// data-model.md §"Legacy localStorage migration" disposition table).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacySettingsPayload {
    /// `pomodoro-settings` localStorage key parsed as the post-cutover
    /// `Settings` JSON (FR-005 — round-trip without migration). `None`
    /// when the key is absent.
    pub settings: Option<Settings>,
    /// `theme-preference` localStorage key (e.g. `"auto"`, `"dark"`,
    /// `"light"`). `None` when absent. Carried through to the handler
    /// so the import path is non-lossy; the handler today logs and
    /// drops it because `AppSettings` does not yet carry a theme
    /// preference field. A later phase folds it in.
    pub theme_preference: Option<String>,
    /// `timer-theme-preference` localStorage key (e.g. `"espresso"`).
    /// Same disposition as `theme_preference` — carried through, not
    /// yet folded into `AppSettings`.
    pub timer_theme_preference: Option<String>,
    /// `presto_auto_check_updates` localStorage key, parsed as bool
    /// (the JS era stored `"true"` / `"false"` strings; the reader
    /// parses to `bool`). `None` when absent.
    pub auto_check_updates: Option<bool>,
}

/// JS-era `pomodoro-history` localStorage shape — a vec of `Session`
/// records, the same shape the post-cutover history.json on disk
/// uses (FR-005). Empty vec is the cold-start no-op shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyHistoryPayload {
    pub history: Vec<Session>,
}

/// JS-era `pomodoro-tasks` localStorage shape — a vec of `Task`
/// records, identical to the post-cutover `tasks.json` shape on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyTasksPayload {
    pub tasks: Vec<Task>,
}

/// JS-era `presto-tags` localStorage shape — a vec of `Tag` records,
/// identical to the post-cutover `tags.json` shape on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyTagsPayload {
    pub tags: Vec<Tag>,
}

/// JS-era `presto_manual_sessions` localStorage shape — a vec of
/// `ManualSession` records, identical to the post-cutover
/// `manual_sessions.json` shape on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyManualSessionsPayload {
    pub sessions: Vec<ManualSession>,
}

/// JS-era user-state flags.
///
/// The boolean / string preferences that live as bare localStorage
/// values rather than inside a JSON blob. Per data-model.md
/// §"Legacy localStorage migration", these fold into the
/// `AppSettings` user-state slice on the Rust side
/// (`hide_icon_on_close` is unrelated — that's a pre-existing field).
///
/// `pomodoro-session` is the active-session snapshot for cross-launch
/// resume (`Session` shape). Carried as `Option<Session>` so the
/// handler can persist it via the existing `save_session_data` path
/// when present.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyUserStatePayload {
    /// `presto-guest-mode` localStorage key, parsed as bool. `None`
    /// when absent.
    pub guest_mode: Option<bool>,
    /// `presto-auth-seen` localStorage key, parsed as bool.
    pub auth_seen: Option<bool>,
    /// `presto-skipped-versions` localStorage key — the JS era stored
    /// a JSON-encoded `Vec<String>` here. Empty vec when absent.
    pub skipped_versions: Vec<String>,
    /// `pomodoro-session` localStorage key parsed as the post-cutover
    /// `Session` shape (FR-005 — round-trip without migration). `None`
    /// when absent.
    pub active_session: Option<Session>,
}

/// JS-era Supabase auth token shape persisted at
/// `window.localStorage["sb-<project-ref>-auth-token"]`. Mirrors
/// data-model.md §`SupabaseSessionPayload`.
///
/// Distinct from `AuthSession` in two ways: (a) it carries
/// `expires_at` (Unix epoch seconds, supabase-js convention) which
/// the Rust-side persisted shape does not yet store, and (b) it is
/// transition-only and slated for removal one minor version after
/// cutover.
///
/// The Tauri-side `import_legacy_supabase_session` handler validates
/// the payload, ignores `expires_at` (the Rust-side `AuthSession`
/// re-derives expiry from the JWT on next refresh), and writes
/// `AuthSession { access_token, refresh_token, user }` to
/// `<app_data_dir>/supabase-session.json`. Idempotent: skipped if the
/// Rust-side session file already exists.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupabaseSessionPayload {
    pub access_token: String,
    pub refresh_token: String,
    /// Unix epoch seconds, supabase-js convention. Carried for wire
    /// fidelity; the Tauri handler ignores it (the post-cutover
    /// session re-derives expiry on next refresh).
    pub expires_at: u64,
    pub user: AuthUser,
}

/// Argument bundle for `bridge::commands::update_tray_icon`.
///
/// Per contracts/tauri-bridge.md row 23 / data-model.md §`UpdateTrayIconArgs`,
/// the Tauri-side handler at `src-tauri/src/lib.rs:538` takes six positional
/// parameters (`timer_text`, `is_running`, `session_mode`, `current_session`,
/// `total_sessions`, `mode_icon`); the Leptos wrapper collapses them into
/// this single typed struct so the call site reads
/// `update_tray_icon(args)` rather than a six-arg sprawl.
///
/// The on-the-wire shape is preserved exactly: `serde-wasm-bindgen`
/// flattens the struct fields to top-level keys in the Tauri args bag,
/// matching the per-positional-arg shape Tauri 2.x expects. `session_mode`
/// is the closed-domain `TimerMode` enum (Phase 1A T027) — a `String`
/// drift here would not compile (FR-008).
///
/// `mode_icon: Option<String>` mirrors the Tauri-side handler's
/// `mode_icon: Option<String>`; the handler falls back to a hard-coded
/// emoji per `TimerMode` variant when `None`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTrayIconArgs {
    pub timer_text: String,
    pub is_running: bool,
    pub session_mode: TimerMode,
    pub current_session: u32,
    pub total_sessions: u32,
    pub mode_icon: Option<String>,
}

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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAvailablePayload {
    pub version: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{
        AppearanceSettings, AuthSession, AuthUser, LegacyHistoryPayload,
        LegacyManualSessionsPayload, LegacyTagsPayload, LegacyTasksPayload,
        LegacyUserStatePayload, ManualSession, NotificationSettings, Session, SessionTag, Settings,
        ShortcutSettings, StatusBarDisplay, SupabaseSessionPayload, Tag, Task, TimerSettings,
        UpdateAvailablePayload, UpdateTrayIconArgs,
    };
    use crate::bridge::session_type::SessionType;
    use crate::bridge::timer_mode::TimerMode;

    #[test]
    fn session_round_trips_snake_case() {
        let s = Session {
            completed_pomodoros: 4,
            total_focus_time: 6_000,
            current_session: 5,
            date: "Sat May 10 2026".to_string(),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(
            json,
            r#"{"completed_pomodoros":4,"total_focus_time":6000,"current_session":5,"date":"Sat May 10 2026"}"#
        );
        let decoded: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.completed_pomodoros, 4);
        assert_eq!(decoded.total_focus_time, 6_000);
        assert_eq!(decoded.current_session, 5);
        assert_eq!(decoded.date, "Sat May 10 2026");
    }

    #[test]
    fn task_round_trips_with_optional_completed_at() {
        let t = Task {
            id: 17,
            text: "ship the wrapper".to_string(),
            completed: false,
            created_at: "2026-05-10T08:00:00Z".to_string(),
            completed_at: None,
        };
        let json = serde_json::to_string(&t).unwrap();
        let decoded: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, 17);
        assert_eq!(decoded.text, "ship the wrapper");
        assert!(!decoded.completed);
        assert_eq!(decoded.completed_at, None);
    }

    #[test]
    fn settings_round_trips_default_shape() {
        // Pins today's `AppSettings` wire shape post-Phase-3a F1/M3
        // migration: `status_bar_display: StatusBarDisplay` (kebab-case
        // string on the wire) replaces the legacy `hide_status_bar: bool`
        // field. The legacy fallback path is exercised separately in
        // `managers::settings::tests::migrates_hide_status_bar_to_status_bar_display`.
        let s = Settings::default();
        let json = serde_json::to_string(&s).unwrap();
        let decoded: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.timer.focus_duration, 25);
        assert_eq!(decoded.timer.weekly_goal_minutes, 125);
        assert_eq!(decoded.timer.max_session_time, 120);
        assert!(decoded.notifications.desktop_notifications);
        assert!(decoded.analytics_enabled);
        assert_eq!(decoded.status_bar_display, StatusBarDisplay::Default);
        // Wire shape: kebab-case enum string, no `hide_status_bar` key.
        assert!(json.contains(r#""status_bar_display":"default""#));
        assert!(!json.contains("hide_status_bar"));
        assert_eq!(
            decoded.shortcuts.start_stop.as_deref(),
            Some("CommandOrControl+Alt+Space"),
        );
        // Phase 4e R-002 user-state slice — cold-start defaults
        // present on the wire.
        assert!(!decoded.guest_mode);
        assert!(!decoded.auth_seen);
        assert!(decoded.skipped_versions.is_empty());
        assert!(json.contains(r#""guest_mode":false"#));
        assert!(json.contains(r#""auth_seen":false"#));
        assert!(json.contains(r#""skipped_versions":[]"#));
        // Appearance block present on the wire.
        assert_eq!(decoded.appearance.theme, "auto");
        assert_eq!(decoded.appearance.timer_theme, "espresso");
        assert!(json.contains(r#""appearance":{"theme":"auto","timer_theme":"espresso"}"#));
        assert!(json.contains(r#""max_session_time":120"#));
    }

    #[test]
    fn settings_appearance_deserialises_from_legacy_json() {
        // Pre-widening 0.4.x JSONs lack `appearance` and `max_session_time`.
        // The serde defaults must fire for both.
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
        let decoded: Settings = serde_json::from_str(legacy).unwrap();
        assert_eq!(decoded.appearance.theme, "auto");
        assert_eq!(decoded.appearance.timer_theme, "espresso");
        assert_eq!(decoded.timer.max_session_time, 120);
    }

    #[test]
    fn appearance_settings_round_trips() {
        let a = AppearanceSettings {
            theme: "dark".to_string(),
            timer_theme: "pipboy".to_string(),
        };
        let json = serde_json::to_string(&a).unwrap();
        let decoded: AppearanceSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.theme, "dark");
        assert_eq!(decoded.timer_theme, "pipboy");
    }

    /// Phase 4e R-002 round-trip: a `Settings` with non-default
    /// user-state values serialises and deserialises stably.
    #[test]
    fn settings_round_trips_user_state_slice() {
        let s = Settings {
            guest_mode: true,
            auth_seen: true,
            skipped_versions: vec!["0.5.0".to_string(), "0.5.1".to_string()],
            ..Settings::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        let decoded: Settings = serde_json::from_str(&json).unwrap();
        assert!(decoded.guest_mode);
        assert!(decoded.auth_seen);
        assert_eq!(decoded.skipped_versions.len(), 2);
        assert_eq!(decoded.skipped_versions[0], "0.5.0");
    }

    #[test]
    fn settings_deserialises_from_minimal_legacy_json() {
        // Old `0.4.x` settings JSONs may lack `weekly_goal_minutes`,
        // `auto_start_focus`, `allow_continuous_sessions`, `advanced`,
        // `analytics_enabled`, `hide_icon_on_close`, and any
        // status-bar field. The serde defaults must fill those in
        // (FR-005 — round-trip every released 0.4.x JSON without
        // manual migration). The "legacy `hide_status_bar` projects
        // into `status_bar_display`" path lands in T152's custom
        // deserializer; here we cover only the "neither field
        // present" branch.
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
        let decoded: Settings = serde_json::from_str(legacy).unwrap();
        assert_eq!(decoded.timer.weekly_goal_minutes, 125);
        assert_eq!(decoded.timer.max_session_time, 120);
        assert!(!decoded.notifications.auto_start_focus);
        assert!(!decoded.notifications.allow_continuous_sessions);
        assert!(!decoded.advanced.debug_mode);
        assert!(decoded.analytics_enabled);
        assert!(!decoded.hide_icon_on_close);
        assert_eq!(decoded.status_bar_display, StatusBarDisplay::default());
        assert_eq!(decoded.appearance.theme, "auto");
        assert_eq!(decoded.appearance.timer_theme, "espresso");
    }

    #[test]
    fn auth_session_round_trips_snake_case() {
        // Pins the wire shape against supabase-js's `/auth/v1/token`
        // response so the Rust REST adapter (`src-tauri/src/auth.rs`)
        // can deserialise the upstream response directly into this
        // struct. `user_metadata` is `serde_json::Value` so apps can
        // carry arbitrary OAuth-provider claims without forcing a
        // closed-shape migration.
        let s = AuthSession {
            access_token: "eyJhbGciOi...".to_string(),
            refresh_token: "rt-abc-123".to_string(),
            user: AuthUser {
                id: "user-uuid".to_string(),
                email: "user@example.com".to_string(),
                user_metadata: serde_json::json!({"full_name": "Konstantin"}),
            },
        };
        let json = serde_json::to_string(&s).unwrap();
        // Round-trip stability: the encoded JSON must contain the
        // top-level keys supabase-js produces (snake_case via serde
        // default), with `user` nested as an object.
        assert!(json.contains(r#""access_token":"eyJhbGciOi...""#));
        assert!(json.contains(r#""refresh_token":"rt-abc-123""#));
        assert!(json.contains(r#""email":"user@example.com""#));
        assert!(json.contains(r#""full_name":"Konstantin""#));
        let decoded: AuthSession = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.access_token, "eyJhbGciOi...");
        assert_eq!(decoded.user.id, "user-uuid");
        assert_eq!(decoded.user.email, "user@example.com");
        assert_eq!(
            decoded
                .user
                .user_metadata
                .get("full_name")
                .and_then(|v| v.as_str()),
            Some("Konstantin"),
        );
    }

    #[test]
    fn auth_session_deserialises_with_empty_user_metadata() {
        // The mock entry in tauriMock.js (T087) emits `user_metadata: {}`;
        // make sure that shape decodes cleanly so the e2e short-circuit
        // path doesn't trip a SerdeRoundtrip error.
        let json = r#"{
            "access_token": "tok",
            "refresh_token": "rt",
            "user": {"id": "id", "email": "e@e", "user_metadata": {}}
        }"#;
        let decoded: AuthSession = serde_json::from_str(json).unwrap();
        assert!(decoded.user.user_metadata.is_object());
    }

    #[test]
    fn manual_session_carries_typed_session_type() {
        let m = ManualSession {
            id: "ms-1".to_string(),
            session_type: SessionType::LongBreak,
            duration: 15,
            start_time: "10:00".to_string(),
            end_time: "10:15".to_string(),
            notes: Some("walk".to_string()),
            created_at: "2026-05-10T10:15:00Z".to_string(),
            date: "Sat May 10 2026".to_string(),
            tags: None,
        };
        let json = serde_json::to_string(&m).unwrap();
        // The closed-domain enum serialises as the camelCase string per
        // SessionType's #[serde(rename_all = "camelCase")].
        assert!(json.contains(r#""session_type":"longBreak""#));
        let decoded: ManualSession = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.session_type, SessionType::LongBreak);
    }

    #[test]
    fn tag_round_trips_snake_case() {
        let t = Tag {
            id: "tag-abc".to_string(),
            name: "Work".to_string(),
            icon: "ri-briefcase-line".to_string(),
            color: "#3b82f6".to_string(),
            created_at: "2026-05-10T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains(r#""id":"tag-abc""#));
        assert!(json.contains(r#""name":"Work""#));
        assert!(json.contains("\"color\":\"#3b82f6\""));
        let decoded: Tag = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, "tag-abc");
        assert_eq!(decoded.name, "Work");
        assert_eq!(decoded.icon, "ri-briefcase-line");
        assert_eq!(decoded.color, "#3b82f6");
    }

    #[test]
    fn session_tag_round_trips_snake_case() {
        let st = SessionTag {
            session_id: "sess-1".to_string(),
            tag_id: "tag-abc".to_string(),
            duration: 300,
            created_at: "2026-05-10T09:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&st).unwrap();
        assert!(json.contains(r#""session_id":"sess-1""#));
        assert!(json.contains(r#""tag_id":"tag-abc""#));
        assert!(json.contains(r#""duration":300"#));
        let decoded: SessionTag = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.session_id, "sess-1");
        assert_eq!(decoded.tag_id, "tag-abc");
        assert_eq!(decoded.duration, 300);
        assert_eq!(decoded.created_at, "2026-05-10T09:00:00Z");
    }

    /// Pins the default keyboard-shortcut bindings so an accidental
    /// change to a default string fails a test rather than shipping
    /// silently broken shortcuts to users.
    #[test]
    fn shortcut_settings_default_values_match_legacy_js_bindings() {
        let s = ShortcutSettings::default();
        assert_eq!(
            s.start_stop.as_deref(),
            Some("CommandOrControl+Alt+Space"),
            "start_stop default must match the JS-era binding",
        );
        assert_eq!(
            s.reset.as_deref(),
            Some("CommandOrControl+Alt+R"),
            "reset default must match the JS-era binding",
        );
        assert_eq!(
            s.skip.as_deref(),
            Some("CommandOrControl+Alt+S"),
            "skip default must match the JS-era binding",
        );
    }

    /// `ShortcutSettings` with null bindings (user-cleared) round-trips
    /// through JSON with `null` on the wire per the JS-era convention.
    #[test]
    fn shortcut_settings_null_bindings_round_trip() {
        let json = r#"{"start_stop":null,"reset":null,"skip":null}"#;
        let decoded: ShortcutSettings = serde_json::from_str(json).unwrap();
        assert!(decoded.start_stop.is_none());
        assert!(decoded.reset.is_none());
        assert!(decoded.skip.is_none());
        let re_encoded = serde_json::to_string(&decoded).unwrap();
        assert_eq!(re_encoded, json);
    }

    /// Pins the `NotificationSettings` defaults — each bool field has
    /// a distinct value so a default-value swap would fail exactly one
    /// assertion here.
    #[test]
    fn notification_settings_default_values() {
        let n = NotificationSettings::default();
        assert!(n.desktop_notifications, "desktop_notifications defaults true");
        assert!(n.sound_notifications, "sound_notifications defaults true");
        assert!(n.auto_start_timer, "auto_start_timer defaults true");
        assert!(!n.auto_start_focus, "auto_start_focus defaults false");
        assert!(
            !n.allow_continuous_sessions,
            "allow_continuous_sessions defaults false",
        );
        assert!(!n.smart_pause, "smart_pause defaults false");
        assert_eq!(n.smart_pause_timeout, 30, "smart_pause_timeout defaults 30s");
    }

    /// Pins the `TimerSettings` defaults in isolation — the values are
    /// also covered by `settings_round_trips_default_shape` but isolating
    /// them here makes regressions in defaults immediately attributable.
    #[test]
    fn timer_settings_default_values() {
        let t = TimerSettings::default();
        assert_eq!(t.focus_duration, 25);
        assert_eq!(t.break_duration, 5);
        assert_eq!(t.long_break_duration, 20);
        assert_eq!(t.total_sessions, 10);
        assert_eq!(t.weekly_goal_minutes, 125);
        assert_eq!(t.max_session_time, 120);
    }

    /// `StatusBarDisplay` serialises as kebab-case strings per the
    /// wire contract (data-model.md §"Settings legacy migration").
    #[test]
    fn status_bar_display_serialises_kebab_case() {
        let json_default = serde_json::to_string(&StatusBarDisplay::Default).unwrap();
        assert_eq!(json_default, r#""default""#);
        let json_icon_only = serde_json::to_string(&StatusBarDisplay::IconOnly).unwrap();
        assert_eq!(json_icon_only, r#""icon-only""#);
    }

    #[test]
    fn status_bar_display_round_trips_both_variants() {
        for (wire, expected) in [
            (r#""default""#, StatusBarDisplay::Default),
            (r#""icon-only""#, StatusBarDisplay::IconOnly),
        ] {
            let decoded: StatusBarDisplay = serde_json::from_str(wire).unwrap();
            assert_eq!(decoded, expected, "failed for wire form {wire}");
            let re_encoded = serde_json::to_string(&decoded).unwrap();
            assert_eq!(re_encoded, wire);
        }
    }

    /// `UpdateAvailablePayload` round-trips with all optional fields
    /// populated — pins the wire shape the `tauri-plugin-updater`
    /// emits (`version`, `body`, `date`).
    #[test]
    fn update_available_payload_round_trips_with_all_fields() {
        let p = UpdateAvailablePayload {
            version: "0.6.0".to_string(),
            body: Some("Bug fixes.".to_string()),
            date: Some("2026-05-10T00:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains(r#""version":"0.6.0""#));
        let decoded: UpdateAvailablePayload = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.version, "0.6.0");
        assert_eq!(decoded.body.as_deref(), Some("Bug fixes."));
        assert_eq!(decoded.date.as_deref(), Some("2026-05-10T00:00:00Z"));
    }

    /// When `body` and `date` are absent from the plugin's emit, the
    /// `#[serde(default)]` markers must supply `None` so the
    /// `UpdateManager::handle_event` path doesn't fail deserialisation
    /// on minimal payloads.
    #[test]
    fn update_available_payload_handles_missing_optional_fields() {
        let json = r#"{"version":"0.6.1"}"#;
        let decoded: UpdateAvailablePayload = serde_json::from_str(json).unwrap();
        assert_eq!(decoded.version, "0.6.1");
        assert!(decoded.body.is_none());
        assert!(decoded.date.is_none());
    }

    /// `UpdateTrayIconArgs` round-trips through JSON — the struct is
    /// the single typed arg bundle handed to `update_tray_icon`; a
    /// wire-shape drift here would silence a tray-icon update without
    /// a compile error.
    #[test]
    fn update_tray_icon_args_round_trips() {
        let args = UpdateTrayIconArgs {
            timer_text: "24:59".to_string(),
            is_running: true,
            session_mode: TimerMode::Focus,
            current_session: 3,
            total_sessions: 10,
            mode_icon: Some("🍅".to_string()),
        };
        let json = serde_json::to_string(&args).unwrap();
        assert!(json.contains(r#""timer_text":"24:59""#));
        assert!(json.contains(r#""is_running":true"#));
        assert!(json.contains(r#""session_mode":"focus""#));
        assert!(json.contains(r#""current_session":3"#));
        assert!(json.contains(r#""total_sessions":10"#));
        let decoded: UpdateTrayIconArgs = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.timer_text, "24:59");
        assert!(decoded.is_running);
        assert_eq!(decoded.session_mode, TimerMode::Focus);
        assert_eq!(decoded.current_session, 3);
        assert_eq!(decoded.mode_icon.as_deref(), Some("🍅"));
    }

    #[test]
    fn update_tray_icon_args_without_mode_icon_round_trips() {
        let args = UpdateTrayIconArgs {
            timer_text: "05:00".to_string(),
            is_running: false,
            session_mode: TimerMode::Break,
            current_session: 1,
            total_sessions: 10,
            mode_icon: None,
        };
        let json = serde_json::to_string(&args).unwrap();
        let decoded: UpdateTrayIconArgs = serde_json::from_str(&json).unwrap();
        assert!(decoded.mode_icon.is_none());
        assert_eq!(decoded.session_mode, TimerMode::Break);
    }

    #[test]
    fn legacy_history_payload_round_trips() {
        let p = LegacyHistoryPayload {
            history: vec![Session {
                completed_pomodoros: 2,
                total_focus_time: 3000,
                current_session: 3,
                date: "Sat May 10 2026".to_string(),
            }],
        };
        let json = serde_json::to_string(&p).unwrap();
        let decoded: LegacyHistoryPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.history.len(), 1);
        assert_eq!(decoded.history[0].completed_pomodoros, 2);
    }

    #[test]
    fn legacy_tasks_payload_round_trips() {
        let p = LegacyTasksPayload {
            tasks: vec![Task {
                id: 1,
                text: "write tests".to_string(),
                completed: false,
                created_at: "2026-05-10T00:00:00Z".to_string(),
                completed_at: None,
            }],
        };
        let json = serde_json::to_string(&p).unwrap();
        let decoded: LegacyTasksPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.tasks.len(), 1);
        assert_eq!(decoded.tasks[0].text, "write tests");
    }

    #[test]
    fn legacy_tags_payload_round_trips() {
        let p = LegacyTagsPayload {
            tags: vec![Tag {
                id: "tag-1".to_string(),
                name: "Focus".to_string(),
                icon: "🎯".to_string(),
                color: "#ff0000".to_string(),
                created_at: "2026-05-10T00:00:00Z".to_string(),
            }],
        };
        let json = serde_json::to_string(&p).unwrap();
        let decoded: LegacyTagsPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.tags.len(), 1);
        assert_eq!(decoded.tags[0].id, "tag-1");
    }

    #[test]
    fn legacy_manual_sessions_payload_round_trips() {
        let p = LegacyManualSessionsPayload {
            sessions: vec![ManualSession {
                id: "ms-1".to_string(),
                session_type: SessionType::Focus,
                duration: 25,
                start_time: "09:00".to_string(),
                end_time: "09:25".to_string(),
                notes: None,
                created_at: "2026-05-10T09:00:00Z".to_string(),
                date: "Sat May 10 2026".to_string(),
                tags: None,
            }],
        };
        let json = serde_json::to_string(&p).unwrap();
        let decoded: LegacyManualSessionsPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.sessions.len(), 1);
        assert_eq!(decoded.sessions[0].id, "ms-1");
        assert_eq!(decoded.sessions[0].duration, 25);
    }

    #[test]
    fn legacy_user_state_payload_round_trips_with_all_fields() {
        let p = LegacyUserStatePayload {
            guest_mode: Some(true),
            auth_seen: Some(false),
            skipped_versions: vec!["0.5.0".to_string()],
            active_session: Some(Session {
                completed_pomodoros: 1,
                total_focus_time: 1500,
                current_session: 2,
                date: "Sat May 10 2026".to_string(),
            }),
        };
        let json = serde_json::to_string(&p).unwrap();
        let decoded: LegacyUserStatePayload = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.guest_mode, Some(true));
        assert_eq!(decoded.auth_seen, Some(false));
        assert_eq!(decoded.skipped_versions, vec!["0.5.0"]);
        assert!(decoded.active_session.is_some());
        assert_eq!(decoded.active_session.unwrap().completed_pomodoros, 1);
    }

    #[test]
    fn legacy_user_state_payload_round_trips_with_absent_optional_fields() {
        let json = r#"{"guest_mode":null,"auth_seen":null,"skipped_versions":[],"active_session":null}"#;
        let decoded: LegacyUserStatePayload = serde_json::from_str(json).unwrap();
        assert!(decoded.guest_mode.is_none());
        assert!(decoded.auth_seen.is_none());
        assert!(decoded.skipped_versions.is_empty());
        assert!(decoded.active_session.is_none());
    }

    /// `SupabaseSessionPayload` is the JS-era localStorage token shape
    /// (distinct from `AuthSession` — it carries `expires_at`). Pinned
    /// so the `import_legacy_supabase_session_from_storage` reader can
    /// deserialise the raw supabase-js localStorage value without a
    /// translation layer.
    #[test]
    fn supabase_session_payload_round_trips() {
        let p = SupabaseSessionPayload {
            access_token: "eyJ...".to_string(),
            refresh_token: "rt-xyz".to_string(),
            expires_at: 1_746_883_200,
            user: AuthUser {
                id: "user-1".to_string(),
                email: "test@example.com".to_string(),
                user_metadata: serde_json::json!({}),
            },
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains(r#""expires_at":1746883200"#));
        let decoded: SupabaseSessionPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.access_token, "eyJ...");
        assert_eq!(decoded.refresh_token, "rt-xyz");
        assert_eq!(decoded.expires_at, 1_746_883_200);
        assert_eq!(decoded.user.email, "test@example.com");
    }
}
