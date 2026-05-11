// Legacy localStorage migration — Tauri-side handlers.
//
// Spec 001-leptos-migration §Phase 1E T099-T115; data-model.md
// §"Legacy localStorage migration"; contracts/tauri-bridge.md
// §"Transition-only commands". This module owns the seven
// `import_legacy_*` handlers that fold JS-era localStorage payloads
// into the Rust-side authoritative stores. Each handler is
// idempotent: a re-import is a successful no-op when the matching
// authoritative file is already on disk.
//
// Sunset: this module and every `import_legacy_*` command is slated
// for removal one minor version after cutover. Principle VII anchor:
// this is a one-shot migration with a defined sunset, not an
// indefinite parallel surface.
//
// Per Principle II (Local-First, Privacy-Default), handlers MUST NOT
// log payload contents. Per-key counts (`history.len()`, etc.) are
// the only acceptable log signal.
//
// Lint allowance rationale — `clippy::redundant_pub_crate`: this
// module is `mod migration;` (private) at `lib.rs`, but every item
// below is referenced from `lib.rs` (each `import_legacy_*` Tauri
// command calls into `migration::*`). The same lint disagreement
// `auth.rs` resolves applies here; we follow the same allow-list at
// the module level rather than scattering per-item annotations.
#![allow(clippy::redundant_pub_crate)]

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{auth, helpers, AppSettings, BridgeError, ManualSession, PomodoroSession, Tag, Task};

// ── Payload mirrors ──────────────────────────────────────────────────────────
//
// Each `Legacy*Payload` mirrors the Leptos-side
// `presto-web::bridge::types::Legacy*Payload` byte-for-byte on the
// wire. The Tauri-side mirror is necessary because `tauri::command`
// macro deserialises the args into local types; we cannot import the
// Leptos crate here (it's a wasm-only crate). The types share the
// same JSON shape via `serde-default` field naming + identical field
// types.

/// Mirrors `presto-web::bridge::types::LegacySettingsPayload`.
#[derive(Debug, Deserialize, Serialize)]
pub(super) struct LegacySettingsPayload {
    pub settings: Option<AppSettings>,
    pub theme_preference: Option<String>,
    pub timer_theme_preference: Option<String>,
    pub auto_check_updates: Option<bool>,
}

/// Mirrors `presto-web::bridge::types::LegacyHistoryPayload`.
#[derive(Debug, Deserialize, Serialize)]
pub(super) struct LegacyHistoryPayload {
    pub history: Vec<PomodoroSession>,
}

/// Mirrors `presto-web::bridge::types::LegacyTasksPayload`.
#[derive(Debug, Deserialize, Serialize)]
pub(super) struct LegacyTasksPayload {
    pub tasks: Vec<Task>,
}

/// Mirrors `presto-web::bridge::types::LegacyTagsPayload`.
#[derive(Debug, Deserialize, Serialize)]
pub(super) struct LegacyTagsPayload {
    pub tags: Vec<Tag>,
}

/// Mirrors `presto-web::bridge::types::LegacyManualSessionsPayload`.
#[derive(Debug, Deserialize, Serialize)]
pub(super) struct LegacyManualSessionsPayload {
    pub sessions: Vec<ManualSession>,
}

/// Mirrors `presto-web::bridge::types::LegacyUserStatePayload`.
#[derive(Debug, Deserialize, Serialize)]
pub(super) struct LegacyUserStatePayload {
    pub guest_mode: Option<bool>,
    pub auth_seen: Option<bool>,
    pub skipped_versions: Vec<String>,
    pub active_session: Option<PomodoroSession>,
}

/// Mirrors `presto-web::bridge::types::SupabaseSessionPayload`. The
/// Tauri-side handler validates and re-persists into the existing
/// `auth::AuthSession` shape (dropping `expires_at` per
/// research.md §6 step 4 — the Rust-side session re-derives expiry
/// from the JWT on next refresh).
#[derive(Debug, Deserialize, Serialize)]
pub(super) struct SupabaseSessionPayload {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
    pub user: SupabaseUserMirror,
}

/// Subset of `auth::AuthUser` as shipped on the wire. We declare a
/// local mirror rather than re-exporting `auth::AuthUser` to avoid
/// re-publicising the auth module's internals; the field shape is
/// identical to `auth::AuthUser`.
#[derive(Debug, Deserialize, Serialize)]
pub(super) struct SupabaseUserMirror {
    pub id: String,
    pub email: String,
    pub user_metadata: serde_json::Value,
}

// ── User-state sentinel marker ───────────────────────────────────────────────

const USER_STATE_SENTINEL: &str = "legacy-user-state-imported.marker";

fn user_state_already_imported(dir: &Path) -> bool {
    dir.join(USER_STATE_SENTINEL).exists()
}

fn write_user_state_sentinel(dir: &Path) -> Result<(), BridgeError> {
    std::fs::create_dir_all(dir).map_err(|e| BridgeError::Internal {
        msg: format!("create app data dir: {e}"),
    })?;
    std::fs::write(dir.join(USER_STATE_SENTINEL), b"").map_err(|e| BridgeError::Internal {
        msg: format!("write user-state sentinel: {e}"),
    })
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// Idempotent: skip if `settings.json` already exists in the app-data
/// dir. Otherwise persist the embedded `Settings` (if present) using
/// the existing `helpers::write_settings_to` path. The four bare
/// preference flags (`theme_preference`, `timer_theme_preference`,
/// `auto_check_updates`) are accepted on the wire for non-lossiness
/// but only `auto_check_updates` has a target field today
/// (`AppSettings` has no theme-preference slot; per
/// data-model.md the theme work folds in a later phase). The handler
/// accepts the bare flags but drops them rather than failing the
/// import (lossless wire shape, lossy persistence — broadening the
/// persistence is a later phase's job).
//
// Takes the payload by reference: every helper call that consumes
// the inner `Settings` only borrows it (`helpers::write_settings_to`
// takes `&AppSettings`). Pass-by-value would force the early-return
// paths to drop a non-trivially-sized `LegacySettingsPayload` —
// clippy's `needless_pass_by_value` flags this and the by-ref form
// is unambiguously cheaper given we never move out of the payload.
pub(super) fn import_settings(
    app_data_dir: &Path,
    payload: &LegacySettingsPayload,
) -> Result<(), BridgeError> {
    if app_data_dir.join("settings.json").exists() {
        // Already imported (or set by post-cutover code) — skip.
        return Ok(());
    }
    let Some(legacy_settings) = payload.settings.as_ref() else {
        // No settings.json equivalent in the legacy payload; nothing
        // to write.
        return Ok(());
    };
    // Fold the legacy bare-preference flags into the `appearance` block.
    // The JS-era `theme_preference` / `timer_theme_preference` were stored
    // as separate localStorage keys outside the main settings object; the
    // payload carries them alongside `settings` so the import handler can
    // merge them into the post-cutover `appearance` block.
    let mut merged = legacy_settings.clone();
    if let Some(ref theme) = payload.theme_preference {
        merged.appearance.theme.clone_from(theme);
    }
    if let Some(ref timer_theme) = payload.timer_theme_preference {
        merged.appearance.timer_theme.clone_from(timer_theme);
    }
    helpers::write_settings_to(app_data_dir, &merged)?;
    Ok(())
}

/// Idempotent: skip if `history.json` already exists. Otherwise write
/// the imported vec via atomic-rename. Empty vec is a successful
/// no-op (no file written, but Ok(()) returned).
pub(super) fn import_history(
    app_data_dir: &Path,
    payload: &LegacyHistoryPayload,
) -> Result<(), BridgeError> {
    if app_data_dir.join("history.json").exists() || payload.history.is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(app_data_dir).map_err(|e| BridgeError::Internal {
        msg: format!("create app data dir: {e}"),
    })?;
    helpers::write_json_atomic(&app_data_dir.join("history.json"), &payload.history)?;
    Ok(())
}

/// Idempotent: skip if `tasks.json` already exists. Empty vec is a
/// successful no-op.
pub(super) fn import_tasks(
    app_data_dir: &Path,
    payload: &LegacyTasksPayload,
) -> Result<(), BridgeError> {
    if app_data_dir.join("tasks.json").exists() || payload.tasks.is_empty() {
        return Ok(());
    }
    helpers::write_tasks_to(app_data_dir, &payload.tasks)?;
    Ok(())
}

/// Idempotent: skip if `tags.json` already exists. The
/// `helpers::read_tags_from` cold-start bootstrap writes a default
/// "Focus" tag and the file on first read; we check the file on disk
/// (not `read_tags_from`) so we don't conflate "first launch" with
/// "user has imported".
pub(super) fn import_tags(
    app_data_dir: &Path,
    payload: &LegacyTagsPayload,
) -> Result<(), BridgeError> {
    if app_data_dir.join("tags.json").exists() {
        return Ok(());
    }
    helpers::write_tags_to(app_data_dir, &payload.tags)?;
    Ok(())
}

/// Idempotent: skip if `manual_sessions.json` already exists. Empty
/// vec is a successful no-op.
pub(super) fn import_manual_sessions(
    app_data_dir: &Path,
    payload: &LegacyManualSessionsPayload,
) -> Result<(), BridgeError> {
    if app_data_dir.join("manual_sessions.json").exists() || payload.sessions.is_empty() {
        return Ok(());
    }
    helpers::write_manual_sessions_to(app_data_dir, &payload.sessions)?;
    Ok(())
}

/// Idempotent via the user-state sentinel marker file (rather than
/// the `settings.json` file, because `import_settings` may run
/// independently of `import_user_state`).
///
/// **Phase 4e R-002 broadening**: `AppSettings` now carries the
/// `guest_mode` / `auth_seen` / `skipped_versions` slots, so this
/// handler folds the JS-era flags into the on-disk settings file.
/// The fold is read-modify-write: read the current `AppSettings`
/// (cold-start = `AppSettings::default()`), set the three fields
/// from the payload (only when the payload value is `Some` /
/// non-empty — never clobber a more-recent post-cutover value
/// with a stale legacy `false`), write the merged settings back.
/// The active session, if present, persists via the existing
/// `helpers::write_session_to` path; the sentinel guards against
/// a second-launch re-import overwriting newer data.
pub(super) fn import_user_state(
    app_data_dir: &Path,
    payload: &LegacyUserStatePayload,
) -> Result<(), BridgeError> {
    if user_state_already_imported(app_data_dir) {
        return Ok(());
    }
    if let Some(session) = payload.active_session.as_ref() {
        if !app_data_dir.join("session.json").exists() {
            helpers::write_session_to(app_data_dir, session)?;
        }
    }
    // R-002 fold: read current settings (or fall back to default
    // when the file is absent — the read helper does that), apply
    // the legacy flags, write back. Treat `Some(false)` as "user
    // explicitly opted out of guest mode" — that's still a
    // meaningful state distinct from "no signal at all", and a
    // 0.4.x user who toggled guest mode off should land at the
    // unauthenticated welcome state, not the cold-start guest
    // projection. Empty `skipped_versions` is also a meaningful
    // signal (the user has dismissed no updates yet); we copy it
    // verbatim.
    let mut settings = helpers::read_settings_from(app_data_dir).unwrap_or_default();
    if let Some(guest) = payload.guest_mode {
        settings.guest_mode = guest;
    }
    if let Some(seen) = payload.auth_seen {
        settings.auth_seen = seen;
    }
    settings
        .skipped_versions
        .clone_from(&payload.skipped_versions);
    helpers::write_settings_to(app_data_dir, &settings)?;

    write_user_state_sentinel(app_data_dir)?;
    Ok(())
}

/// Idempotent: skip if a Rust-side Supabase session is already
/// persisted. Otherwise persist the imported session in the same
/// shape `auth::persist_session` writes (dropping `expires_at` per
/// research.md §6 step 4 — the post-cutover session re-derives
/// expiry from the JWT on next refresh).
pub(super) fn import_supabase_session(
    app_data_dir: &Path,
    payload: &SupabaseSessionPayload,
) -> Result<(), BridgeError> {
    // Validate non-empty tokens before any disk touch — the JS-era
    // localStorage may carry a malformed blob.
    if payload.access_token.is_empty() {
        return Err(BridgeError::InvalidArgument {
            field: "access_token".to_string(),
            reason: "access_token is empty".to_string(),
        });
    }
    if payload.refresh_token.is_empty() {
        return Err(BridgeError::InvalidArgument {
            field: "refresh_token".to_string(),
            reason: "refresh_token is empty".to_string(),
        });
    }
    if auth::read_session(app_data_dir)?.is_some() {
        return Ok(());
    }
    // Re-shape into the `auth::AuthSession` type the rest of the
    // auth module persists. `expires_at` is intentionally not
    // forwarded — the post-cutover session re-derives expiry from
    // the JWT on next refresh.
    let session = auth::AuthSession {
        access_token: payload.access_token.clone(),
        refresh_token: payload.refresh_token.clone(),
        user: auth::AuthUser {
            id: payload.user.id.clone(),
            email: payload.user.email.clone(),
            user_metadata: payload.user.user_metadata.clone(),
        },
    };
    auth::persist_session(app_data_dir, &session)
}

#[cfg(test)]
mod tests {
    use super::{
        import_history, import_manual_sessions, import_supabase_session, import_tags, import_tasks,
        import_user_state, LegacyHistoryPayload, LegacyManualSessionsPayload,
        LegacySettingsPayload, LegacyTagsPayload, LegacyTasksPayload, LegacyUserStatePayload,
        SupabaseSessionPayload, SupabaseUserMirror,
    };
    use crate::{AppSettings, ManualSession, PomodoroSession, SessionType, Tag, Task};
    use tempfile::tempdir;

    // ── import_settings ─────────────────────────────────────────────────────

    #[test]
    fn import_settings_writes_settings_json_when_absent() {
        let dir = tempdir().unwrap();
        let payload = LegacySettingsPayload {
            settings: Some(AppSettings::default()),
            theme_preference: Some("dark".to_string()),
            timer_theme_preference: Some("espresso".to_string()),
            auto_check_updates: Some(true),
        };
        super::import_settings(dir.path(), &payload).unwrap();
        assert!(dir.path().join("settings.json").exists());
    }

    #[test]
    fn import_settings_skips_when_settings_json_already_exists() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("settings.json"), b"{\"sentinel\":true}").unwrap();
        let payload = LegacySettingsPayload {
            settings: Some(AppSettings::default()),
            theme_preference: None,
            timer_theme_preference: None,
            auto_check_updates: None,
        };
        super::import_settings(dir.path(), &payload).unwrap();
        // The sentinel content must survive — proves we did not
        // overwrite the existing settings.json.
        let after = std::fs::read_to_string(dir.path().join("settings.json")).unwrap();
        assert_eq!(after, "{\"sentinel\":true}");
    }

    #[test]
    fn import_settings_with_no_inner_settings_is_a_noop() {
        let dir = tempdir().unwrap();
        let payload = LegacySettingsPayload {
            settings: None,
            theme_preference: Some("dark".to_string()),
            timer_theme_preference: None,
            auto_check_updates: None,
        };
        super::import_settings(dir.path(), &payload).unwrap();
        assert!(!dir.path().join("settings.json").exists());
    }

    #[test]
    fn import_settings_folds_theme_preference_into_appearance() {
        let dir = tempdir().unwrap();
        let payload = LegacySettingsPayload {
            settings: Some(AppSettings::default()),
            theme_preference: Some("dark".to_string()),
            timer_theme_preference: Some("pipboy".to_string()),
            auto_check_updates: None,
        };
        super::import_settings(dir.path(), &payload).unwrap();
        let json = std::fs::read_to_string(dir.path().join("settings.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed["appearance"]["theme"].as_str().unwrap(),
            "dark",
            "theme_preference must win over default"
        );
        assert_eq!(
            parsed["appearance"]["timer_theme"].as_str().unwrap(),
            "pipboy",
            "timer_theme_preference must win over default"
        );
    }

    #[test]
    fn import_settings_no_theme_prefs_keeps_defaults() {
        let dir = tempdir().unwrap();
        let payload = LegacySettingsPayload {
            settings: Some(AppSettings::default()),
            theme_preference: None,
            timer_theme_preference: None,
            auto_check_updates: None,
        };
        super::import_settings(dir.path(), &payload).unwrap();
        let json = std::fs::read_to_string(dir.path().join("settings.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["appearance"]["theme"].as_str().unwrap(), "auto");
        assert_eq!(
            parsed["appearance"]["timer_theme"].as_str().unwrap(),
            "espresso"
        );
    }

    // ── import_history ──────────────────────────────────────────────────────

    fn sample_session() -> PomodoroSession {
        PomodoroSession {
            completed_pomodoros: 4,
            total_focus_time: 6_000,
            current_session: 5,
            date: "Sat May 10 2026".to_string(),
        }
    }

    #[test]
    fn import_history_writes_history_json_when_absent() {
        let dir = tempdir().unwrap();
        let payload = LegacyHistoryPayload {
            history: vec![sample_session()],
        };
        import_history(dir.path(), &payload).unwrap();
        assert!(dir.path().join("history.json").exists());
    }

    #[test]
    fn import_history_skips_when_history_json_already_exists() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("history.json"), b"[\"sentinel\"]").unwrap();
        let payload = LegacyHistoryPayload {
            history: vec![sample_session()],
        };
        import_history(dir.path(), &payload).unwrap();
        let after = std::fs::read_to_string(dir.path().join("history.json")).unwrap();
        assert_eq!(after, "[\"sentinel\"]");
    }

    #[test]
    fn import_history_with_empty_vec_is_a_noop() {
        let dir = tempdir().unwrap();
        let payload = LegacyHistoryPayload {
            history: Vec::new(),
        };
        import_history(dir.path(), &payload).unwrap();
        assert!(!dir.path().join("history.json").exists());
    }

    /// T103 named-test (per F4 idempotency design). A second-launch
    /// re-import MUST be a successful no-op even when the first
    /// launch already wrote the history file. Pins the entry-point
    /// contract that T115's `migrate_legacy_localstorage` relies on.
    #[test]
    fn import_history_is_idempotent_across_two_calls() {
        let dir = tempdir().unwrap();
        let payload = LegacyHistoryPayload {
            history: vec![sample_session()],
        };
        import_history(dir.path(), &payload).unwrap();
        let first_bytes = std::fs::read(dir.path().join("history.json")).unwrap();
        // Second call with a different payload must NOT overwrite —
        // the existence-check guarantees idempotency.
        let payload2 = LegacyHistoryPayload {
            history: vec![PomodoroSession {
                completed_pomodoros: 99,
                total_focus_time: 99,
                current_session: 99,
                date: "different".to_string(),
            }],
        };
        import_history(dir.path(), &payload2).unwrap();
        let second_bytes = std::fs::read(dir.path().join("history.json")).unwrap();
        assert_eq!(first_bytes, second_bytes, "second import must be a no-op");
    }

    // ── import_tasks ────────────────────────────────────────────────────────

    fn sample_task() -> Task {
        Task {
            id: 1,
            text: "ship".to_string(),
            completed: false,
            created_at: "2026-05-10T08:00:00Z".to_string(),
            completed_at: None,
        }
    }

    #[test]
    fn import_tasks_writes_when_absent() {
        let dir = tempdir().unwrap();
        let payload = LegacyTasksPayload {
            tasks: vec![sample_task()],
        };
        import_tasks(dir.path(), &payload).unwrap();
        assert!(dir.path().join("tasks.json").exists());
    }

    #[test]
    fn import_tasks_skips_when_already_present() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("tasks.json"), b"[]").unwrap();
        let payload = LegacyTasksPayload {
            tasks: vec![sample_task()],
        };
        import_tasks(dir.path(), &payload).unwrap();
        let after = std::fs::read_to_string(dir.path().join("tasks.json")).unwrap();
        assert_eq!(after, "[]");
    }

    /// T105 named-test (per F4 idempotency design). Two-call no-op:
    /// the second invocation must not overwrite the first's output,
    /// even with a mutated payload.
    #[test]
    fn import_tasks_is_idempotent_across_two_calls() {
        let dir = tempdir().unwrap();
        let payload = LegacyTasksPayload {
            tasks: vec![sample_task()],
        };
        import_tasks(dir.path(), &payload).unwrap();
        let first_bytes = std::fs::read(dir.path().join("tasks.json")).unwrap();
        let payload2 = LegacyTasksPayload {
            tasks: vec![Task {
                id: 999,
                text: "should-not-write".to_string(),
                completed: true,
                created_at: "2026-05-10T00:00:00Z".to_string(),
                completed_at: Some("2026-05-10T00:00:00Z".to_string()),
            }],
        };
        import_tasks(dir.path(), &payload2).unwrap();
        let second_bytes = std::fs::read(dir.path().join("tasks.json")).unwrap();
        assert_eq!(first_bytes, second_bytes);
    }

    #[test]
    fn import_tasks_with_empty_vec_is_a_noop() {
        let dir = tempdir().unwrap();
        let payload = LegacyTasksPayload { tasks: Vec::new() };
        import_tasks(dir.path(), &payload).unwrap();
        assert!(
            !dir.path().join("tasks.json").exists(),
            "empty tasks payload must not create tasks.json"
        );
    }

    // ── import_tags ─────────────────────────────────────────────────────────

    fn sample_tag() -> Tag {
        Tag {
            id: "t1".to_string(),
            name: "Focus".to_string(),
            icon: "ri-brain-line".to_string(),
            color: "#4CAF50".to_string(),
            created_at: "2026-05-10T08:00:00Z".to_string(),
        }
    }

    #[test]
    fn import_tags_writes_when_absent() {
        let dir = tempdir().unwrap();
        let payload = LegacyTagsPayload {
            tags: vec![sample_tag()],
        };
        import_tags(dir.path(), &payload).unwrap();
        assert!(dir.path().join("tags.json").exists());
    }

    #[test]
    fn import_tags_skips_when_already_present() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("tags.json"), b"[]").unwrap();
        let payload = LegacyTagsPayload {
            tags: vec![sample_tag()],
        };
        import_tags(dir.path(), &payload).unwrap();
        let after = std::fs::read_to_string(dir.path().join("tags.json")).unwrap();
        assert_eq!(after, "[]");
    }

    /// T107 named-test (per F4 idempotency design). Two-call no-op:
    /// the second invocation must not overwrite the first's output.
    #[test]
    fn import_tags_is_idempotent_across_two_calls() {
        let dir = tempdir().unwrap();
        let payload = LegacyTagsPayload {
            tags: vec![sample_tag()],
        };
        import_tags(dir.path(), &payload).unwrap();
        let first_bytes = std::fs::read(dir.path().join("tags.json")).unwrap();
        let payload2 = LegacyTagsPayload {
            tags: vec![Tag {
                id: "should-not-write".to_string(),
                name: "X".to_string(),
                icon: "x".to_string(),
                color: "#000".to_string(),
                created_at: "2026-05-10T00:00:00Z".to_string(),
            }],
        };
        import_tags(dir.path(), &payload2).unwrap();
        let second_bytes = std::fs::read(dir.path().join("tags.json")).unwrap();
        assert_eq!(first_bytes, second_bytes);
    }

    #[test]
    fn import_tags_with_empty_vec_writes_empty_tags_json() {
        let dir = tempdir().unwrap();
        let payload = LegacyTagsPayload { tags: Vec::new() };
        import_tags(dir.path(), &payload).unwrap();
        assert!(
            dir.path().join("tags.json").exists(),
            "empty tags payload must create tags.json to prevent synthetic bootstrap tag"
        );
        let content = std::fs::read_to_string(dir.path().join("tags.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(
            parsed.as_array().unwrap().is_empty(),
            "tags.json must contain empty array"
        );
    }

    // ── import_manual_sessions ──────────────────────────────────────────────

    fn sample_manual() -> ManualSession {
        ManualSession {
            id: "ms1".to_string(),
            session_type: SessionType::Focus,
            duration: 25,
            start_time: "10:00".to_string(),
            end_time: "10:25".to_string(),
            notes: None,
            created_at: "2026-05-10T10:25:00Z".to_string(),
            date: "Sat May 10 2026".to_string(),
            tags: None,
        }
    }

    #[test]
    fn import_manual_sessions_writes_when_absent() {
        let dir = tempdir().unwrap();
        let payload = LegacyManualSessionsPayload {
            sessions: vec![sample_manual()],
        };
        import_manual_sessions(dir.path(), &payload).unwrap();
        assert!(dir.path().join("manual_sessions.json").exists());
    }

    #[test]
    fn import_manual_sessions_skips_when_already_present() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("manual_sessions.json"), b"[]").unwrap();
        let payload = LegacyManualSessionsPayload {
            sessions: vec![sample_manual()],
        };
        import_manual_sessions(dir.path(), &payload).unwrap();
        let after = std::fs::read_to_string(dir.path().join("manual_sessions.json")).unwrap();
        assert_eq!(after, "[]");
    }

    /// T109 named-test (per F4 idempotency design). Two-call no-op:
    /// the second invocation must not overwrite the first's output.
    #[test]
    fn import_manual_sessions_is_idempotent_across_two_calls() {
        let dir = tempdir().unwrap();
        let payload = LegacyManualSessionsPayload {
            sessions: vec![sample_manual()],
        };
        import_manual_sessions(dir.path(), &payload).unwrap();
        let first_bytes = std::fs::read(dir.path().join("manual_sessions.json")).unwrap();
        let mut second = sample_manual();
        second.id = "should-not-write".to_string();
        let payload2 = LegacyManualSessionsPayload {
            sessions: vec![second],
        };
        import_manual_sessions(dir.path(), &payload2).unwrap();
        let second_bytes = std::fs::read(dir.path().join("manual_sessions.json")).unwrap();
        assert_eq!(first_bytes, second_bytes);
    }

    #[test]
    fn import_manual_sessions_with_empty_vec_is_a_noop() {
        let dir = tempdir().unwrap();
        let payload = LegacyManualSessionsPayload {
            sessions: Vec::new(),
        };
        import_manual_sessions(dir.path(), &payload).unwrap();
        assert!(
            !dir.path().join("manual_sessions.json").exists(),
            "empty sessions payload must not create manual_sessions.json"
        );
    }

    // ── import_user_state ───────────────────────────────────────────────────

    #[test]
    fn import_user_state_writes_sentinel_and_session_when_absent() {
        let dir = tempdir().unwrap();
        let payload = LegacyUserStatePayload {
            guest_mode: Some(true),
            auth_seen: Some(true),
            skipped_versions: vec!["1.2.3".to_string()],
            active_session: Some(sample_session()),
        };
        import_user_state(dir.path(), &payload).unwrap();
        assert!(dir.path().join("session.json").exists());
        assert!(dir
            .path()
            .join("legacy-user-state-imported.marker")
            .exists());
    }

    /// Phase 4e R-002 named-test: the user-state fold persists the
    /// JS-era `guest_mode` / `auth_seen` / `skipped_versions` flags
    /// into the on-disk `AppSettings` so the post-migration cold
    /// start projects the correct auth/guest state without the
    /// localStorage round-trip.
    ///
    /// Pins the FR-006 contract: a 0.4.x guest user who applies the
    /// auto-update MUST remain in guest mode post-update.
    #[test]
    fn import_user_state_persists_guest_mode_into_settings() {
        let dir = tempdir().unwrap();
        let payload = LegacyUserStatePayload {
            guest_mode: Some(true),
            auth_seen: Some(true),
            skipped_versions: vec!["1.2.3".to_string()],
            active_session: None,
        };
        import_user_state(dir.path(), &payload).unwrap();

        // Read back settings.json — the user-state fields must be
        // present with the imported values.
        let settings_text =
            std::fs::read_to_string(dir.path().join("settings.json")).expect("settings written");
        let parsed: AppSettings = serde_json::from_str(&settings_text).expect("valid settings");
        assert!(parsed.guest_mode, "guest_mode must persist as true");
        assert!(parsed.auth_seen, "auth_seen must persist as true");
        assert_eq!(
            parsed.skipped_versions,
            vec!["1.2.3".to_string()],
            "skipped_versions must persist verbatim",
        );
    }

    /// Round-trip: `AppSettings` JSON encoding carries the new R-002
    /// fields, and a deserialise of that JSON re-derives the same
    /// values. Pins the wire shape against drift.
    #[test]
    fn app_settings_round_trips_user_state_slice() {
        let original = AppSettings {
            guest_mode: true,
            auth_seen: true,
            skipped_versions: vec!["0.5.0".to_string(), "0.5.1".to_string()],
            ..AppSettings::default()
        };
        let json = serde_json::to_string(&original).expect("serialize");
        assert!(json.contains(r#""guest_mode":true"#));
        assert!(json.contains(r#""auth_seen":true"#));
        assert!(json.contains(r#""skipped_versions":["0.5.0","0.5.1"]"#));
        let decoded: AppSettings = serde_json::from_str(&json).expect("deserialize");
        assert!(decoded.guest_mode);
        assert!(decoded.auth_seen);
        assert_eq!(decoded.skipped_versions.len(), 2);
    }

    /// Backwards compatibility: a 0.4.x settings.json without the
    /// new R-002 fields must deserialise into `AppSettings` with the
    /// cold-start defaults for those fields. Pins the
    /// `#[serde(default)]` markers against drift.
    #[test]
    fn app_settings_legacy_shape_uses_default_user_state_slice() {
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
        let decoded: AppSettings =
            serde_json::from_str(legacy).expect("legacy minimal shape must deserialise");
        assert!(!decoded.guest_mode);
        assert!(!decoded.auth_seen);
        assert!(decoded.skipped_versions.is_empty());
    }

    #[test]
    fn import_user_state_skips_when_sentinel_present() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path()).unwrap();
        std::fs::write(dir.path().join("legacy-user-state-imported.marker"), b"").unwrap();
        let payload = LegacyUserStatePayload {
            guest_mode: Some(true),
            auth_seen: Some(true),
            skipped_versions: vec!["1.2.3".to_string()],
            active_session: Some(sample_session()),
        };
        import_user_state(dir.path(), &payload).unwrap();
        // The sentinel was already there; the active session must NOT
        // have been written because the whole import short-circuits.
        assert!(!dir.path().join("session.json").exists());
    }

    /// T111 named-test (per F4 idempotency design).
    ///
    /// Two-call no-op: after the first import writes the sentinel +
    /// session, the second invocation must not re-write the
    /// `session.json` (even with a different `active_session` payload).
    #[test]
    fn import_user_state_is_idempotent_across_two_calls() {
        let dir = tempdir().unwrap();
        let payload = LegacyUserStatePayload {
            guest_mode: Some(true),
            auth_seen: Some(true),
            skipped_versions: vec!["1.2.3".to_string()],
            active_session: Some(sample_session()),
        };
        import_user_state(dir.path(), &payload).unwrap();
        let first_session = std::fs::read(dir.path().join("session.json")).unwrap();
        // Second call with a different active_session must NOT
        // overwrite — the sentinel guarantees idempotency.
        let payload2 = LegacyUserStatePayload {
            guest_mode: Some(false),
            auth_seen: Some(false),
            skipped_versions: vec![],
            active_session: Some(PomodoroSession {
                completed_pomodoros: 99,
                total_focus_time: 99,
                current_session: 99,
                date: "should-not-write".to_string(),
            }),
        };
        import_user_state(dir.path(), &payload2).unwrap();
        let second_session = std::fs::read(dir.path().join("session.json")).unwrap();
        assert_eq!(first_session, second_session);
    }

    // ── import_supabase_session ─────────────────────────────────────────────

    fn sample_supabase_payload() -> SupabaseSessionPayload {
        SupabaseSessionPayload {
            access_token: "tok".to_string(),
            refresh_token: "rt".to_string(),
            expires_at: 9_999_999_999,
            user: SupabaseUserMirror {
                id: "uid".to_string(),
                email: "u@e.com".to_string(),
                user_metadata: serde_json::json!({}),
            },
        }
    }

    #[test]
    fn import_supabase_session_writes_when_absent() {
        let dir = tempdir().unwrap();
        import_supabase_session(dir.path(), &sample_supabase_payload()).unwrap();
        assert!(dir.path().join("supabase-session.json").exists());
    }

    #[test]
    fn import_supabase_session_skips_when_already_present() {
        let dir = tempdir().unwrap();
        // Pre-seed an existing supabase-session.json by going through
        // auth::persist_session.
        let session = serde_json::json!({
            "access_token": "preexisting",
            "refresh_token": "preexisting-rt",
            "user": {"id": "u", "email": "e", "user_metadata": {}}
        });
        let session: super::auth::AuthSession = serde_json::from_value(session).unwrap();
        super::auth::persist_session(dir.path(), &session).unwrap();
        // Now the import is a no-op.
        import_supabase_session(dir.path(), &sample_supabase_payload()).unwrap();
        let after = super::auth::read_session(dir.path()).unwrap().unwrap();
        assert_eq!(after.access_token, "preexisting");
    }

    #[test]
    fn import_supabase_session_rejects_empty_tokens() {
        let dir = tempdir().unwrap();
        let mut payload = sample_supabase_payload();
        payload.access_token = String::new();
        let err = import_supabase_session(dir.path(), &payload).unwrap_err();
        assert!(matches!(
            err,
            super::BridgeError::InvalidArgument { ref field, .. } if field == "access_token"
        ));
    }

    /// T113 named-test (per F4 idempotency design + research.md §6
    /// step 4).
    ///
    /// Two-call no-op: the second invocation must not overwrite the
    /// first's persisted session, even with a different payload.
    /// Pins the consumer-visible "first import wins" rule.
    #[test]
    fn import_supabase_session_is_idempotent_across_two_calls() {
        let dir = tempdir().unwrap();
        let payload1 = sample_supabase_payload();
        import_supabase_session(dir.path(), &payload1).unwrap();
        let first = super::auth::read_session(dir.path()).unwrap().unwrap();
        // Second call with mutated tokens must NOT overwrite.
        let mut payload2 = sample_supabase_payload();
        payload2.access_token = "different-tok".to_string();
        payload2.refresh_token = "different-rt".to_string();
        import_supabase_session(dir.path(), &payload2).unwrap();
        let second = super::auth::read_session(dir.path()).unwrap().unwrap();
        assert_eq!(first.access_token, second.access_token);
        assert_eq!(first.refresh_token, second.refresh_token);
    }

    #[test]
    fn import_supabase_session_preserves_tokens_and_user_drops_expires_at() {
        let dir = tempdir().unwrap();
        let payload = SupabaseSessionPayload {
            access_token: "access-tok-123".to_string(),
            refresh_token: "refresh-tok-456".to_string(),
            expires_at: 9_999_999_999,
            user: SupabaseUserMirror {
                id: "user-id-789".to_string(),
                email: "test@example.com".to_string(),
                user_metadata: serde_json::json!({"key": "value"}),
            },
        };
        import_supabase_session(dir.path(), &payload).unwrap();

        let session_file = dir.path().join("supabase-session.json");
        assert!(session_file.exists());
        let raw = std::fs::read_to_string(&session_file).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();

        assert_eq!(parsed["access_token"].as_str().unwrap(), "access-tok-123");
        assert_eq!(parsed["refresh_token"].as_str().unwrap(), "refresh-tok-456");
        assert_eq!(parsed["user"]["id"].as_str().unwrap(), "user-id-789");
        assert_eq!(
            parsed["user"]["email"].as_str().unwrap(),
            "test@example.com"
        );
        assert_eq!(
            parsed["user"]["user_metadata"]["key"].as_str().unwrap(),
            "value"
        );
        assert!(
            parsed.get("expires_at").is_none(),
            "expires_at must be dropped from persisted session"
        );
    }
}
