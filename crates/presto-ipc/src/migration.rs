// Legacy JS-era migration payloads.
//
// Spec 001-leptos-migration §Phase 1E T099-T115; data-model.md
// §"Legacy localStorage migration". Each payload mirrors the JS-era
// localStorage shape for one preserved domain. The Leptos-side
// reader (`bridge::storage`) parses the localStorage value into the
// matching payload and hands it to the matching `import_legacy_*`
// Tauri command. The Tauri handler is idempotent: if the
// authoritative Rust-side store already has data, the import is a
// successful no-op.
//
// Feature-gated under `migration` so both crates can drop the
// surface in a single PR once cutover ages out the JS-era code path.

use serde::{Deserialize, Serialize};

use crate::auth::AuthUser;
use crate::session::{ManualSession, Session};
use crate::settings::Settings;
use crate::tags::Tag;
use crate::tasks::Task;

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
/// existing `Settings` shape (theme/timer-theme are not yet
/// represented in `Settings` and are dropped on import — they live
/// as user preferences in a later phase, per data-model.md §"Legacy
/// localStorage migration" disposition table).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LegacySettingsPayload {
    /// `pomodoro-settings` localStorage key parsed as the post-cutover
    /// `Settings` JSON (FR-005 — round-trip without migration).
    pub settings: Option<Settings>,
    /// `theme-preference` localStorage key (e.g. `"auto"`, `"dark"`,
    /// `"light"`).
    pub theme_preference: Option<String>,
    /// `timer-theme-preference` localStorage key (e.g. `"espresso"`).
    pub timer_theme_preference: Option<String>,
    /// `presto_auto_check_updates` localStorage key, parsed as bool.
    pub auto_check_updates: Option<bool>,
}

/// JS-era `pomodoro-history` localStorage shape — a vec of `Session`
/// records, the same shape the post-cutover history.json on disk
/// uses (FR-005). Empty vec is the cold-start no-op shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LegacyHistoryPayload {
    pub history: Vec<Session>,
}

/// JS-era `pomodoro-tasks` localStorage shape — a vec of `Task`
/// records, identical to the post-cutover `tasks.json` shape on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LegacyTasksPayload {
    pub tasks: Vec<Task>,
}

/// JS-era `presto-tags` localStorage shape — a vec of `Tag` records,
/// identical to the post-cutover `tags.json` shape on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LegacyTagsPayload {
    pub tags: Vec<Tag>,
}

/// JS-era `presto_manual_sessions` localStorage shape — a vec of
/// `ManualSession` records, identical to the post-cutover
/// `manual_sessions.json` shape on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LegacyManualSessionsPayload {
    pub sessions: Vec<ManualSession>,
}

/// JS-era user-state flags.
///
/// The boolean / string preferences that live as bare localStorage
/// values rather than inside a JSON blob. Per data-model.md
/// §"Legacy localStorage migration", these fold into the `Settings`
/// user-state slice on the Rust side.
///
/// `pomodoro-session` is the active-session snapshot for cross-launch
/// resume (`Session` shape). Carried as `Option<Session>` so the
/// handler can persist it via the existing `save_session_data` path
/// when present.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LegacyUserStatePayload {
    /// `presto-guest-mode` localStorage key, parsed as bool.
    pub guest_mode: Option<bool>,
    /// `presto-auth-seen` localStorage key, parsed as bool.
    pub auth_seen: Option<bool>,
    /// `presto-skipped-versions` localStorage key — the JS era stored
    /// a JSON-encoded `Vec<String>` here. Empty vec when absent.
    pub skipped_versions: Vec<String>,
    /// `pomodoro-session` localStorage key parsed as the post-cutover
    /// `Session` shape.
    pub active_session: Option<Session>,
}

/// JS-era Supabase auth token shape persisted at
/// `window.localStorage["sb-<project-ref>-auth-token"]`.
///
/// Distinct from `AuthSession` in two ways: (a) it carries
/// `expires_at` (Unix epoch seconds, supabase-js convention) which
/// the Rust-side persisted shape does not yet store, and (b) it is
/// transition-only and slated for removal one minor version after
/// cutover.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SupabaseSessionPayload {
    pub access_token: String,
    pub refresh_token: String,
    /// Unix epoch seconds, supabase-js convention. Carried for wire
    /// fidelity; the Tauri handler ignores it (the post-cutover
    /// session re-derives expiry on next refresh).
    pub expires_at: u64,
    pub user: AuthUser,
}
