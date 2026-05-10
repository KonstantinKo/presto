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

/// Timer durations & session count. Mirrors `TimerSettings` at
/// `src-tauri/src/lib.rs:219-227`.
///
/// `weekly_goal_minutes` carries a `#[serde(default = "...")]` because
/// settings JSON written by pre-`weekly_goal` builds lacks the field.
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
}

impl Default for TimerSettings {
    fn default() -> Self {
        Self {
            focus_duration: 25,
            break_duration: 5,
            long_break_duration: 20,
            total_sessions: 10,
            weekly_goal_minutes: default_weekly_goal(),
        }
    }
}

const fn default_weekly_goal() -> u32 {
    125
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

/// Full application settings record. Mirrors `AppSettings` at
/// `src-tauri/src/lib.rs:196-210`.
///
/// **Wire shape note**: this matches the Tauri-side `AppSettings`
/// exactly today, including `hide_status_bar: bool`. Spec
/// data-model.md §`Settings / AppSettings` describes a planned
/// migration to a typed `status_bar_display: StatusBarDisplay` enum
/// with a custom deserializer that falls back to `hide_status_bar`
/// for legacy JSONs; that migration is out of scope for Phase 1C and
/// will be done in a later phase that touches both crates in lockstep.
/// Today's wrapper round-trips the existing shape (FR-005 — no
/// on-disk shape change in this phase).
///
/// `clippy::struct_excessive_bools` is allowed here for the same
/// reason as on `NotificationSettings` and on the Tauri-side mirror
/// at `src-tauri/src/lib.rs:195`: each bool is an independent
/// settings toggle exposed in the UI; restructuring would not match
/// either the on-disk JSON shape or the settings-page layout.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub shortcuts: ShortcutSettings,
    pub timer: TimerSettings,
    pub notifications: NotificationSettings,
    #[serde(default)]
    pub advanced: AdvancedSettings,
    pub autostart: bool,
    #[serde(default = "default_analytics_enabled")]
    pub analytics_enabled: bool,
    #[serde(default)]
    pub hide_icon_on_close: bool,
    #[serde(default)]
    pub hide_status_bar: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            shortcuts: ShortcutSettings::default(),
            timer: TimerSettings::default(),
            notifications: NotificationSettings::default(),
            advanced: AdvancedSettings::default(),
            autostart: false,
            analytics_enabled: true,
            hide_icon_on_close: false,
            hide_status_bar: false,
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
    use super::{AuthSession, AuthUser, ManualSession, Session, Settings, Task};
    use crate::bridge::session_type::SessionType;

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
        // Pins today's Tauri-side AppSettings wire shape including the
        // legacy `hide_status_bar` field. The forward migration to
        // `status_bar_display` per data-model.md is a separate phase;
        // this test documents the baseline so a future drift fails loud.
        let s = Settings::default();
        let json = serde_json::to_string(&s).unwrap();
        let decoded: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.timer.focus_duration, 25);
        assert_eq!(decoded.timer.weekly_goal_minutes, 125);
        assert!(decoded.notifications.desktop_notifications);
        assert!(decoded.analytics_enabled);
        assert!(!decoded.hide_status_bar);
        assert_eq!(
            decoded.shortcuts.start_stop.as_deref(),
            Some("CommandOrControl+Alt+Space"),
        );
    }

    #[test]
    fn settings_deserialises_from_minimal_legacy_json() {
        // Old `0.4.x` settings JSONs may lack `weekly_goal_minutes`,
        // `auto_start_focus`, `allow_continuous_sessions`, `advanced`,
        // `analytics_enabled`, `hide_icon_on_close`, and `hide_status_bar`.
        // The serde defaults must fill those in (FR-005 — round-trip
        // every released 0.4.x JSON without manual migration).
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
        assert!(!decoded.notifications.auto_start_focus);
        assert!(!decoded.notifications.allow_continuous_sessions);
        assert!(!decoded.advanced.debug_mode);
        assert!(decoded.analytics_enabled);
        assert!(!decoded.hide_icon_on_close);
        assert!(!decoded.hide_status_bar);
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
            decoded.user.user_metadata.get("full_name").and_then(|v| v.as_str()),
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
}
