#![allow(
    clippy::redundant_pub_crate,
    reason = "Private module exposes pub(super) persistence helpers to lib.rs while avoiding plain pub unreachable_pub."
)]

use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Serializes `value` as pretty-printed JSON and atomically writes it to `path`.
///
/// Writes to a sibling `.tmp` file first, then renames on success, preventing
/// partial writes from corrupting the target file on crash or power loss.
///
/// # Errors
///
/// Returns an error string if serialization, temp-file write, or rename fails.
pub(super) fn write_json_atomic<T: Serialize + ?Sized>(
    path: &Path,
    value: &T,
) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| format!("Failed to serialize JSON: {e}"))?;
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, json.as_bytes())
        .map_err(|e| format!("Failed to write temp file: {e}"))?;
    std::fs::rename(&tmp_path, path).map_err(|e| format!("Failed to persist file: {e}"))?;
    Ok(())
}

/// Acquires a `Mutex` lock, recovering from a poisoned state if necessary.
///
/// If the mutex was poisoned by a prior panicking holder, logs a warning and
/// returns the inner value rather than propagating the panic.
pub(super) fn lock_or_recover<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| {
        log::warn!("recovering poisoned mutex");
        e.into_inner()
    })
}

/// Returns `true` when `action` was last called within `window` of `now`,
/// and records `now` as the latest call time otherwise.
///
/// Extracting this logic as a pure function (caller-supplied state) makes
/// it trivially testable without touching the global `SHORTCUT_DEBOUNCE` mutex.
#[must_use]
// Module-level `clippy::redundant_pub_crate` allowance covers this parent-module API:
// `pub(super)` records "lib.rs helper", while plain `pub` would trip `unreachable_pub`.
pub(super) fn is_debounced(
    map: &mut HashMap<String, Instant>,
    action: &str,
    now: Instant,
    window: Duration,
) -> bool {
    if let Some(last) = map.get(action) {
        if now.duration_since(*last) < window {
            return true;
        }
    }
    map.insert(action.to_owned(), now);
    false
}

// ── Settings ─────────────────────────────────────────────────────────────────

/// Reads `settings.json` from `dir`.
///
/// Returns `Ok(AppSettings::default())` when the file is absent or contains
/// malformed JSON. Returns `Err` for any other I/O error (e.g. permission
/// denied) so callers can surface unexpected failures.
pub(super) fn read_settings_from(dir: &Path) -> Result<super::AppSettings, std::io::Error> {
    let file_path = dir.join("settings.json");
    if !file_path.exists() {
        return Ok(super::AppSettings::default());
    }
    match fs::read_to_string(&file_path) {
        Ok(contents) => Ok(serde_json::from_str(&contents).unwrap_or_else(|e| {
            log::warn!("settings parse failed ({e}), using defaults");
            super::AppSettings::default()
        })),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(super::AppSettings::default()),
        Err(e) => Err(e),
    }
}

/// Creates `dir` if necessary, then atomically writes `settings` to
/// `settings.json`.
pub(super) fn write_settings_to(dir: &Path, settings: &super::AppSettings) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory: {e}"))?;
    write_json_atomic(&dir.join("settings.json"), settings)
}

// ── Session ───────────────────────────────────────────────────────────────────

/// Reads `session.json` from `dir`.
///
/// Returns `None` if the file is absent. Applies the date-rollover logic from
/// `load_session_data`: if the stored date is from a previous day, the session
/// counters are reset to fresh-day defaults before returning.
pub(super) fn read_session_from(dir: &Path) -> Result<Option<super::PomodoroSession>, String> {
    let file_path = dir.join("session.json");
    if !file_path.exists() {
        return Ok(None);
    }
    let content =
        fs::read_to_string(&file_path).map_err(|e| format!("Failed to read session file: {e}"))?;
    let mut session: super::PomodoroSession =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse session: {e}"))?;

    let now = chrono::Local::now();
    let today_legacy = now.format("%a %b %d %Y").to_string();
    let today_iso = now.format("%Y-%m-%d").to_string();

    let is_same_day = session.date == today_legacy
        || session.date == today_iso
        || chrono::NaiveDate::parse_from_str(&session.date, "%a %b %d %Y")
            .is_ok_and(|d| d.format("%Y-%m-%d").to_string() == today_iso);

    if is_same_day && session.date != today_legacy {
        session.date.clone_from(&today_legacy);
        write_json_atomic(&file_path, &session)?;
    } else if !is_same_day {
        session.completed_pomodoros = 0;
        session.total_focus_time = 0;
        session.current_session = 1;
        session.date.clone_from(&today_legacy);
        write_json_atomic(&file_path, &session)?;
    }

    Ok(Some(session))
}

/// Creates `dir` if necessary, then atomically writes `session` to
/// `session.json`.
pub(super) fn write_session_to(dir: &Path, session: &super::PomodoroSession) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory: {e}"))?;
    write_json_atomic(&dir.join("session.json"), session)
}

// ── Tasks ─────────────────────────────────────────────────────────────────────

/// Reads `tasks.json` from `dir`, returning an empty vec when absent.
pub(super) fn read_tasks_from(dir: &Path) -> Result<Vec<super::Task>, String> {
    let file_path = dir.join("tasks.json");
    if !file_path.exists() {
        return Ok(Vec::new());
    }
    let content =
        fs::read_to_string(file_path).map_err(|e| format!("Failed to read tasks file: {e}"))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse tasks: {e}"))
}

/// Creates `dir` if necessary, then atomically writes `tasks` to `tasks.json`.
pub(super) fn write_tasks_to(dir: &Path, tasks: &[super::Task]) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory: {e}"))?;
    write_json_atomic(&dir.join("tasks.json"), tasks)
}

// ── History ───────────────────────────────────────────────────────────────────

/// Backs up a corrupt file at `original_path` by trying three strategies:
///
///   1. rename to `<name>.corrupt`
///   2. rename to `<name>.corrupt.<unix_ts>`
///   3. write `content` to `<name>.corrupt.<unix_ts>`
///
/// Returns the path where the backup landed, or `Err` when every attempt fails.
fn backup_corrupt_file(original_path: &Path, content: &str) -> Result<std::path::PathBuf, String> {
    let file_name = original_path.file_name().map_or_else(
        || "unknown".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    let base_corrupt = original_path.with_file_name(format!("{file_name}.corrupt"));
    if fs::rename(original_path, &base_corrupt).is_ok() {
        return Ok(base_corrupt);
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let unique = original_path.with_file_name(format!("{file_name}.corrupt.{ts}"));
    if fs::rename(original_path, &unique).is_ok() {
        return Ok(unique);
    }
    fs::write(&unique, content.as_bytes())
        .map(|()| unique)
        .map_err(|e| format!("all backup attempts failed: {e}"))
}

/// Reads `history.json` from `dir`, returning an empty vec when absent.
///
/// On corrupt JSON, rescues the file via `backup_corrupt_file` and returns an
/// empty vec. Returns `Err` only when all backup attempts fail.
pub(super) fn read_history_from(dir: &Path) -> Result<Vec<super::PomodoroSession>, String> {
    let history_path = dir.join("history.json");
    if !history_path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(&history_path)
        .map_err(|e| format!("Failed to read history file: {e}"))?;
    match serde_json::from_str(&content) {
        Ok(h) => Ok(h),
        Err(e) => {
            match backup_corrupt_file(&history_path, &content) {
                Ok(backup_path) => {
                    log::warn!(
                        "history.json corrupt, preserved as {}: {e}",
                        backup_path.display()
                    );
                }
                Err(backup_err) => {
                    log::error!(
                        "history.json corrupt and all backup attempts failed ({backup_err}): {e}"
                    );
                    return Err(format!("history.json corrupt and backup failed: {e}"));
                }
            }
            Ok(Vec::new())
        }
    }
}

/// Converts a date string to canonical ISO `"%Y-%m-%d"` format.
///
/// Accepts both ISO `"%Y-%m-%d"` and the legacy JS `"%a %b %d %Y"` format
/// (e.g. "Mon Jan 01 2024"). Unrecognised strings are returned unchanged so
/// callers are never worse off than before normalization.
fn normalize_date(date: &str) -> String {
    if chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok() {
        return date.to_owned();
    }
    chrono::NaiveDate::parse_from_str(date, "%a %b %d %Y")
        .map_or_else(|_| date.to_owned(), |d| d.format("%Y-%m-%d").to_string())
}

/// Appends `session` to `history.json`, replacing any existing entry for the
/// same date, then trims to the most recent 30 entries.
///
/// All dates (both the incoming session and any existing history entries) are
/// normalized to ISO `"%Y-%m-%d"` before deduplication, sorting, and write-back
/// so that legacy-format entries and ISO-format entries for the same day are
/// always treated as identical.
pub(super) fn append_daily_stats_to(
    dir: &Path,
    session: &super::PomodoroSession,
) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory: {e}"))?;
    let history_path = dir.join("history.json");

    let mut history = read_history_from(dir)?;

    for entry in &mut history {
        entry.date = normalize_date(&entry.date);
    }

    let mut normalized_session = session.clone();
    normalized_session.date = normalize_date(&session.date);

    history.retain(|s| s.date != normalized_session.date);
    history.push(normalized_session);

    history.sort_by(|a, b| a.date.cmp(&b.date));
    if history.len() > 30 {
        let start_index = history.len() - 30;
        history.drain(0..start_index);
    }

    write_json_atomic(&history_path, &history)
}

// ── Manual sessions ───────────────────────────────────────────────────────────

/// Reads `manual_sessions.json` from `dir`, returning an empty vec when absent.
pub(super) fn read_manual_sessions_from(dir: &Path) -> Result<Vec<super::ManualSession>, String> {
    let file_path = dir.join("manual_sessions.json");
    if !file_path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read manual sessions file: {e}"))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse manual sessions: {e}"))
}

/// Creates `dir` if necessary, then atomically writes `sessions` to
/// `manual_sessions.json`.
pub(super) fn write_manual_sessions_to(
    dir: &Path,
    sessions: &[super::ManualSession],
) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory: {e}"))?;
    write_json_atomic(&dir.join("manual_sessions.json"), sessions)
}

/// Appends `session` to `manual_sessions.json`, trimming the oldest entries
/// when the list exceeds `MAX_MANUAL_SESSIONS` to bound file growth.
///
/// The append-then-cap pattern keeps the hot timer-completion path from
/// issuing an ever-growing bulk rewrite on every pomodoro.
pub(super) fn append_manual_session_in(
    dir: &Path,
    session: super::ManualSession,
) -> Result<(), String> {
    const MAX_MANUAL_SESSIONS: usize = 1_000;
    let mut sessions = read_manual_sessions_from(dir)?;
    sessions.push(session);
    if sessions.len() > MAX_MANUAL_SESSIONS {
        sessions.drain(..sessions.len() - MAX_MANUAL_SESSIONS);
    }
    write_manual_sessions_to(dir, &sessions)
}

// ── Quick logs ────────────────────────────────────────────────────────────────

/// Reads `quick_logs.json` from `dir`, returning an empty vec when absent.
///
/// Trims to the most recent `MAX_QUICK_LOGS` entries on load so cold-start
/// memory is bounded even if the file grew large before the cap was added.
///
/// On corrupt JSON, rescues the file by renaming it to `quick_logs.json.corrupt`
/// (matching the `history.json` convention in `append_daily_stats_to`) so the
/// next save does not silently clobber the user's data, then returns an empty
/// vec. The `serde_json::Error` text is logged but never persisted on the
/// wire — feeds AG-10's PII-safety contract.
pub(super) fn read_quick_logs_from(dir: &Path) -> Result<Vec<super::QuickLog>, String> {
    const MAX_QUICK_LOGS: usize = 1_000;
    let file_path = dir.join("quick_logs.json");
    if !file_path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read quick logs file: {e}"))?;
    if content.trim().is_empty() || content.trim() == "null" {
        return Ok(Vec::new());
    }
    match serde_json::from_str::<Vec<super::QuickLog>>(&content) {
        Ok(mut logs) => {
            if logs.len() > MAX_QUICK_LOGS {
                logs.drain(..logs.len() - MAX_QUICK_LOGS);
            }
            Ok(logs)
        }
        Err(e) => {
            match backup_corrupt_file(&file_path, &content) {
                Ok(backup_path) => {
                    log::warn!(
                        "quick_logs.json corrupt, preserved as {}: {e}",
                        backup_path.display()
                    );
                }
                Err(backup_err) => {
                    log::error!(
                        "quick_logs.json corrupt and all backup attempts failed ({backup_err}): {e}"
                    );
                    return Err(format!("quick_logs.json corrupt and backup failed: {e}"));
                }
            }
            Ok(Vec::new())
        }
    }
}

/// Creates `dir` if necessary, then atomically writes `logs` to
/// `quick_logs.json`.
pub(super) fn write_quick_logs_to(dir: &Path, logs: &[super::QuickLog]) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory: {e}"))?;
    write_json_atomic(&dir.join("quick_logs.json"), logs)
}

// ── Distractions ──────────────────────────────────────────────────────────────

/// Reads `distractions.json` from `dir`, returning an empty vec when absent.
///
/// Trims to the most recent `MAX_DISTRACTIONS` entries on load so cold-start
/// memory is bounded even if the file grew large before the cap was added.
///
/// On corrupt JSON, rescues the file by renaming it to
/// `distractions.json.corrupt` (matching the `history.json` convention in
/// `append_daily_stats_to`) so the next save does not silently clobber the
/// user's data, then returns an empty vec.
pub(super) fn read_distractions_from(dir: &Path) -> Result<Vec<super::Distraction>, String> {
    const MAX_DISTRACTIONS: usize = 1_000;
    let file_path = dir.join("distractions.json");
    if !file_path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read distractions file: {e}"))?;
    if content.trim().is_empty() || content.trim() == "null" {
        return Ok(Vec::new());
    }
    match serde_json::from_str::<Vec<super::Distraction>>(&content) {
        Ok(mut entries) => {
            if entries.len() > MAX_DISTRACTIONS {
                entries.drain(..entries.len() - MAX_DISTRACTIONS);
            }
            Ok(entries)
        }
        Err(e) => {
            match backup_corrupt_file(&file_path, &content) {
                Ok(backup_path) => {
                    log::warn!(
                        "distractions.json corrupt, preserved as {}: {e}",
                        backup_path.display()
                    );
                }
                Err(backup_err) => {
                    log::error!(
                        "distractions.json corrupt and all backup attempts failed ({backup_err}): {e}"
                    );
                    return Err(format!("distractions.json corrupt and backup failed: {e}"));
                }
            }
            Ok(Vec::new())
        }
    }
}

/// Creates `dir` if necessary, then atomically writes `entries` to
/// `distractions.json`.
pub(super) fn write_distractions_to(
    dir: &Path,
    entries: &[super::Distraction],
) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory: {e}"))?;
    write_json_atomic(&dir.join("distractions.json"), entries)
}

// ── Tags ──────────────────────────────────────────────────────────────────────

/// Reads `tags.json` from `dir`.
///
/// When the file is absent, bootstraps and persists a default "Focus" tag so
/// that subsequent reads are consistent.
pub(super) fn read_tags_from(dir: &Path) -> Result<Vec<super::Tag>, String> {
    let file_path = dir.join("tags.json");
    if !file_path.exists() {
        let default_tag = super::Tag {
            id: "default-focus".to_string(),
            name: "Focus".to_string(),
            icon: "ri-brain-line".to_string(),
            color: "#4CAF50".to_string(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .to_string(),
        };
        let tags = vec![default_tag];
        fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory: {e}"))?;
        write_json_atomic(&file_path, &tags)?;
        return Ok(tags);
    }
    let content =
        fs::read_to_string(&file_path).map_err(|e| format!("Failed to read tags: {e}"))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse tags.json: {e}"))
}

/// Creates `dir` if necessary, then atomically writes `tags` to `tags.json`.
pub(super) fn write_tags_to(dir: &Path, tags: &[super::Tag]) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory: {e}"))?;
    write_json_atomic(&dir.join("tags.json"), tags)
}

/// Inserts or replaces the entry matching `tag.id` in `tags.json`.
pub(super) fn upsert_tag_in(dir: &Path, tag: super::Tag) -> Result<(), String> {
    let mut tags = read_tags_from(dir)?;
    tags.retain(|t| t.id != tag.id);
    tags.push(tag);
    write_tags_to(dir, &tags)
}

/// Removes the entry matching `tag_id` from `tags.json`.
pub(super) fn delete_tag_in(dir: &Path, tag_id: &str) -> Result<(), String> {
    let mut tags = read_tags_from(dir)?;
    tags.retain(|t| t.id != tag_id);
    write_tags_to(dir, &tags)
}

// ── Session tags ──────────────────────────────────────────────────────────────

/// Reads `session_tags.json` from `dir`, returning an empty vec when absent.
pub(super) fn read_session_tags_from(dir: &Path) -> Result<Vec<super::SessionTag>, String> {
    let file_path = dir.join("session_tags.json");
    if !file_path.exists() {
        return Ok(Vec::new());
    }
    let content =
        fs::read_to_string(&file_path).map_err(|e| format!("Failed to read session tags: {e}"))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse session_tags.json: {e}"))
}

/// Creates `dir` if necessary, then atomically writes `session_tags` to
/// `session_tags.json`.
pub(super) fn write_session_tags_to(
    dir: &Path,
    session_tags: &[super::SessionTag],
) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory: {e}"))?;
    write_json_atomic(&dir.join("session_tags.json"), session_tags)
}

/// Appends `session_tag` to `session_tags.json`, trimming the oldest entries
/// when the list exceeds `MAX_SESSION_TAGS` to bound file growth.
pub(super) fn append_session_tag_in(
    dir: &Path,
    session_tag: super::SessionTag,
) -> Result<(), String> {
    const MAX_SESSION_TAGS: usize = 5_000;
    let mut session_tags = read_session_tags_from(dir)?;
    session_tags.push(session_tag);
    if session_tags.len() > MAX_SESSION_TAGS {
        session_tags.drain(..session_tags.len() - MAX_SESSION_TAGS);
    }
    write_session_tags_to(dir, &session_tags)
}

// ── Reset ─────────────────────────────────────────────────────────────────────

/// Deletes all known data files from `dir`. Files that do not exist are
/// silently skipped so the function is idempotent.
pub(super) fn delete_all_data_in(dir: &Path) -> Result<(), String> {
    const FILES: &[&str] = &[
        "session.json",
        "tasks.json",
        "history.json",
        "settings.json",
        "manual_sessions.json",
        "tags.json",
        "session_tags.json",
        "quick_logs.json",
        "distractions.json",
    ];
    for file_name in FILES {
        let file_path = dir.join(file_name);
        if let Err(e) = fs::remove_file(&file_path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(format!("Failed to delete {file_name}: {e}"));
            }
        }
    }
    // Remove any .corrupt / .corrupt.<ts> backup files to avoid leaking user data.
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.contains(".json.corrupt") {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::{
        append_daily_stats_to, append_manual_session_in, append_session_tag_in, delete_all_data_in,
        delete_tag_in, is_debounced, read_distractions_from, read_history_from,
        read_manual_sessions_from, read_quick_logs_from, read_session_from, read_session_tags_from,
        read_settings_from, read_tags_from, read_tasks_from, upsert_tag_in, write_distractions_to,
        write_manual_sessions_to, write_quick_logs_to, write_session_tags_to, write_session_to,
        write_settings_to, write_tags_to, write_tasks_to,
    };
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    // Re-use parent-module types (private to lib.rs but accessible from descendants).
    use super::super::{
        AppSettings, Distraction, ManualSession, PomodoroSession, QuickLog, SessionTag,
        SessionType, Tag, Task,
    };

    // ── helpers::is_debounced (pre-existing) ──────────────────────────────────

    #[test]
    fn first_call_records_time_and_returns_false() {
        let mut map = HashMap::new();
        let now = Instant::now();
        assert!(!is_debounced(
            &mut map,
            "action",
            now,
            Duration::from_millis(500)
        ));
        assert!(map.contains_key("action"));
    }

    #[test]
    fn immediate_second_call_is_debounced() {
        let mut map = HashMap::new();
        let now = Instant::now();
        let window = Duration::from_millis(500);
        assert!(!is_debounced(&mut map, "action", now, window));
        assert!(is_debounced(&mut map, "action", now, window));
    }

    #[test]
    fn call_after_window_expires_is_not_debounced() {
        let now = Instant::now();
        let window = Duration::from_millis(500);

        let mut map = HashMap::new();
        assert!(!is_debounced(&mut map, "action", now, window));
        let later = now + Duration::from_millis(600);
        assert!(!is_debounced(&mut map, "action", later, window));

        // elapsed == window (strict less-than boundary): also not debounced
        let mut map2 = HashMap::new();
        assert!(!is_debounced(&mut map2, "action", now, window));
        let equal_to_window = now + window;
        assert!(!is_debounced(&mut map2, "action", equal_to_window, window));
    }

    #[test]
    fn different_actions_are_independent() {
        let mut map = HashMap::new();
        let now = Instant::now();
        let window = Duration::from_millis(500);
        assert!(!is_debounced(&mut map, "a1", now, window));
        assert!(!is_debounced(&mut map, "a2", now, window));
        assert!(is_debounced(&mut map, "a1", now, window));
    }

    // ── Settings helpers ──────────────────────────────────────────────────────

    #[test]
    fn settings_missing_file_returns_defaults() {
        let dir = tempfile::tempdir().expect("tempdir");
        let result = read_settings_from(dir.path()).expect("read");
        let defaults = AppSettings::default();
        assert_eq!(result.timer.focus_duration, defaults.timer.focus_duration);
    }

    #[test]
    fn settings_write_then_read_round_trips() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut settings = AppSettings::default();
        settings.timer.focus_duration = 42;
        settings.autostart = true;
        write_settings_to(dir.path(), &settings).expect("write");
        let loaded = read_settings_from(dir.path()).expect("read");
        assert_eq!(loaded.timer.focus_duration, 42);
        assert!(loaded.autostart);
    }

    #[test]
    fn settings_malformed_json_returns_defaults() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("settings.json"), b"not json")
            .expect("write malformed file");
        let result = read_settings_from(dir.path()).expect("read");
        let defaults = AppSettings::default();
        assert_eq!(result.timer.focus_duration, defaults.timer.focus_duration);
    }

    #[test]
    fn settings_write_creates_parent_dir() {
        let dir = tempfile::tempdir().expect("tempdir");
        let nested = dir.path().join("nested").join("deep");
        write_settings_to(&nested, &AppSettings::default()).expect("write to nested dir");
        assert!(nested.join("settings.json").exists());
    }

    // ── Session helpers ───────────────────────────────────────────────────────

    fn make_session(date: &str) -> PomodoroSession {
        PomodoroSession {
            completed_pomodoros: 3,
            total_focus_time: 4500,
            current_session: 2,
            date: date.to_string(),
            title: None,
        }
    }

    #[test]
    fn session_missing_file_returns_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        let result = read_session_from(dir.path()).expect("read");
        assert!(result.is_none());
    }

    #[test]
    fn session_write_then_read_round_trips() {
        let dir = tempfile::tempdir().expect("tempdir");
        let today = chrono::Local::now().format("%a %b %d %Y").to_string();
        let session = make_session(&today);
        write_session_to(dir.path(), &session).expect("write");
        let loaded = read_session_from(dir.path()).expect("read").expect("Some");
        assert_eq!(loaded.completed_pomodoros, session.completed_pomodoros);
        assert_eq!(loaded.total_focus_time, session.total_focus_time);
        assert_eq!(loaded.current_session, session.current_session);
    }

    #[test]
    fn session_stale_date_resets_counters() {
        let dir = tempfile::tempdir().expect("tempdir");
        // A date clearly in the past triggers the rollover branch.
        let session = make_session("Mon Jan 01 2001");
        write_session_to(dir.path(), &session).expect("write");
        let loaded = read_session_from(dir.path()).expect("read").expect("Some");
        assert_eq!(loaded.completed_pomodoros, 0);
        assert_eq!(loaded.total_focus_time, 0);
        assert_eq!(loaded.current_session, 1);
    }

    // ── Malformed-input error-path regression guards ──────────────────────────

    #[test]
    fn read_session_from_malformed_json() {
        let dir = tempfile::tempdir().expect("tempdir");
        // Invalid UTF-8 bytes: fs::read_to_string returns Err before serde sees anything.
        std::fs::write(dir.path().join("session.json"), b"\xFF\xFE\x00")
            .expect("write malformed bytes");
        assert!(read_session_from(dir.path()).is_err());
    }

    #[test]
    fn read_tasks_from_wrong_schema() {
        let dir = tempfile::tempdir().expect("tempdir");
        // Valid JSON but wrong type — a bare string instead of Vec<Task>.
        std::fs::write(dir.path().join("tasks.json"), b"\"this is not an array\"")
            .expect("write wrong schema");
        assert!(read_tasks_from(dir.path()).is_err());
    }

    #[test]
    fn read_manual_sessions_from_empty_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        // Empty file: serde_json fails to parse an empty string as Vec<ManualSession>.
        std::fs::write(dir.path().join("manual_sessions.json"), b"").expect("write empty file");
        assert!(read_manual_sessions_from(dir.path()).is_err());
    }

    // ── Tasks helpers ─────────────────────────────────────────────────────────

    fn make_task(id: u64, text: &str) -> Task {
        Task {
            id,
            text: text.to_string(),
            completed: false,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            completed_at: None,
        }
    }

    #[test]
    fn tasks_missing_file_returns_empty_vec() {
        let dir = tempfile::tempdir().expect("tempdir");
        let result = read_tasks_from(dir.path()).expect("read");
        assert!(result.is_empty());
    }

    #[test]
    fn tasks_write_then_read_round_trips() {
        let dir = tempfile::tempdir().expect("tempdir");
        let tasks = vec![make_task(1, "Buy milk"), make_task(2, "Write tests")];
        write_tasks_to(dir.path(), &tasks).expect("write");
        let loaded = read_tasks_from(dir.path()).expect("read");
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].text, "Buy milk");
        assert_eq!(loaded[1].text, "Write tests");
    }

    #[test]
    fn tasks_empty_vec_round_trips() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_tasks_to(dir.path(), &[]).expect("write");
        let loaded = read_tasks_from(dir.path()).expect("read");
        assert!(loaded.is_empty());
    }

    // ── History helpers ───────────────────────────────────────────────────────

    #[test]
    fn history_missing_file_returns_empty_vec() {
        let dir = tempfile::tempdir().expect("tempdir");
        let result = read_history_from(dir.path()).expect("read");
        assert!(result.is_empty());
    }

    #[test]
    fn history_corrupt_json_is_renamed_and_returns_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("history.json"), b"not json").expect("write corrupt file");
        let result = read_history_from(dir.path()).expect("read");
        assert!(result.is_empty());
        assert!(dir.path().join("history.json.corrupt").exists());
        assert!(!dir.path().join("history.json").exists());
    }

    #[test]
    fn history_appending_31st_entry_prunes_oldest() {
        let dir = tempfile::tempdir().expect("tempdir");
        for i in 0u32..31u32 {
            let session = PomodoroSession {
                completed_pomodoros: i,
                total_focus_time: 0,
                current_session: 1,
                date: format!("2024-01-{:02}", i + 1),
                title: None,
            };
            append_daily_stats_to(dir.path(), &session).expect("append");
        }
        let history = read_history_from(dir.path()).expect("read");
        assert_eq!(history.len(), 30);
        // Entry for day 01 is pruned; day 02 is the oldest survivor.
        assert_eq!(history[0].date, "2024-01-02");
        assert_eq!(history[29].date, "2024-01-31");
    }

    #[test]
    fn history_duplicate_date_is_replaced() {
        let dir = tempfile::tempdir().expect("tempdir");
        let first = PomodoroSession {
            completed_pomodoros: 1,
            total_focus_time: 0,
            current_session: 1,
            date: "2024-06-01".to_string(),
            title: None,
        };
        let second = PomodoroSession {
            completed_pomodoros: 5,
            total_focus_time: 0,
            current_session: 1,
            date: "2024-06-01".to_string(),
            title: None,
        };
        append_daily_stats_to(dir.path(), &first).expect("append first");
        append_daily_stats_to(dir.path(), &second).expect("append second");
        let history = read_history_from(dir.path()).expect("read");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].completed_pomodoros, 5);
    }

    #[test]
    fn history_legacy_date_deduplicates_with_iso_date() {
        let dir = tempfile::tempdir().expect("tempdir");
        let legacy = PomodoroSession {
            completed_pomodoros: 1,
            total_focus_time: 0,
            current_session: 1,
            date: "Sat Jun 01 2024".to_string(),
            title: None,
        };
        let iso = PomodoroSession {
            completed_pomodoros: 5,
            total_focus_time: 0,
            current_session: 1,
            date: "2024-06-01".to_string(),
            title: None,
        };
        append_daily_stats_to(dir.path(), &legacy).expect("append legacy");
        append_daily_stats_to(dir.path(), &iso).expect("append iso");
        let history = read_history_from(dir.path()).expect("read");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].completed_pomodoros, 5);
        assert_eq!(history[0].date, "2024-06-01");
    }

    // ── Quick logs helpers ────────────────────────────────────────────────────

    #[test]
    fn quick_logs_missing_file_returns_empty_vec() {
        let dir = tempfile::tempdir().expect("tempdir");
        let result = read_quick_logs_from(dir.path()).expect("read");
        assert!(result.is_empty());
    }

    #[test]
    fn quick_logs_corrupt_json_is_backed_up_and_returns_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("quick_logs.json"), b"not json")
            .expect("write corrupt file");
        let result = read_quick_logs_from(dir.path()).expect("read");
        assert!(result.is_empty());
        assert!(dir.path().join("quick_logs.json.corrupt").exists());
        assert!(!dir.path().join("quick_logs.json").exists());
    }

    #[test]
    fn quick_logs_write_then_read_round_trips() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = QuickLog {
            id: "ql-test".to_string(),
            title: "Wrote docs".to_string(),
            elapsed_minutes: 30,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            date: "Mon Jan 01 2024".to_string(),
        };
        write_quick_logs_to(dir.path(), &[log]).expect("write");
        let loaded = read_quick_logs_from(dir.path()).expect("read");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "ql-test");
        assert_eq!(loaded[0].title, "Wrote docs");
        assert_eq!(loaded[0].elapsed_minutes, 30);
    }

    // ── Distractions helpers ──────────────────────────────────────────────────

    #[test]
    fn distractions_missing_file_returns_empty_vec() {
        let dir = tempfile::tempdir().expect("tempdir");
        let result = read_distractions_from(dir.path()).expect("read");
        assert!(result.is_empty());
    }

    #[test]
    fn distractions_corrupt_json_is_backed_up_and_returns_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("distractions.json"), b"not json")
            .expect("write corrupt file");
        let result = read_distractions_from(dir.path()).expect("read");
        assert!(result.is_empty());
        assert!(dir.path().join("distractions.json.corrupt").exists());
        assert!(!dir.path().join("distractions.json").exists());
    }

    #[test]
    fn distractions_write_then_read_round_trips() {
        let dir = tempfile::tempdir().expect("tempdir");
        let entry = Distraction {
            id: "d-test".to_string(),
            note: "Phone rang".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            date: "Mon Jan 01 2024".to_string(),
            parent_ref: None,
        };
        write_distractions_to(dir.path(), &[entry]).expect("write");
        let loaded = read_distractions_from(dir.path()).expect("read");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "d-test");
        assert_eq!(loaded[0].note, "Phone rang");
        assert!(loaded[0].parent_ref.is_none());
    }

    // ── Manual sessions helpers ───────────────────────────────────────────────

    fn make_manual_session(n: u32) -> ManualSession {
        ManualSession {
            id: format!("ms-{n}"),
            session_type: SessionType::Focus,
            duration: 25,
            start_time: "09:00".to_string(),
            end_time: "09:25".to_string(),
            notes: None,
            created_at: format!("2024-01-{:02}T09:00:00Z", (n % 28) + 1),
            date: format!("Mon Jan {:02} 2024", (n % 28) + 1),
            tags: None,
            title: None,
        }
    }

    #[test]
    fn manual_sessions_missing_file_returns_empty_vec() {
        let dir = tempfile::tempdir().expect("tempdir");
        let result = read_manual_sessions_from(dir.path()).expect("read");
        assert!(result.is_empty());
    }

    #[test]
    fn manual_sessions_append_then_read_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        append_manual_session_in(dir.path(), make_manual_session(1)).expect("append");
        let loaded = read_manual_sessions_from(dir.path()).expect("read");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "ms-1");
        assert_eq!(loaded[0].duration, 25);
        assert_eq!(loaded[0].session_type, SessionType::Focus);
    }

    #[test]
    fn manual_sessions_cap_trims_at_1000() {
        let dir = tempfile::tempdir().expect("tempdir");
        let sessions: Vec<ManualSession> = (0..1_001).map(make_manual_session).collect();
        write_manual_sessions_to(dir.path(), &sessions).expect("write");
        // Append one more to trigger the 1_000-entry cap.
        append_manual_session_in(dir.path(), make_manual_session(1_001)).expect("append");
        let loaded = read_manual_sessions_from(dir.path()).expect("read");
        assert_eq!(loaded.len(), 1_000);
        // Oldest entry is drained; newest is last.
        assert_eq!(loaded[0].id, "ms-2");
        assert_eq!(loaded[999].id, "ms-1001");
        // Recency ordering: last-appended entry is present, entry 0 is evicted.
        let last_id = "ms-1001";
        let first_id = "ms-0";
        assert!(
            loaded.iter().any(|s| s.id == last_id),
            "last-appended entry {last_id} must be retained after cap",
        );
        assert!(
            !loaded.iter().any(|s| s.id == first_id),
            "first entry {first_id} must be evicted after cap",
        );
    }

    // ── Tags helpers ──────────────────────────────────────────────────────────

    #[test]
    fn tags_missing_file_bootstraps_default_focus_tag() {
        let dir = tempfile::tempdir().expect("tempdir");
        let tags = read_tags_from(dir.path()).expect("read");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].id, "default-focus");
        assert_eq!(tags[0].name, "Focus");
        // Bootstrap persists the file for subsequent reads.
        assert!(dir.path().join("tags.json").exists());
    }

    #[test]
    fn tags_upsert_replaces_existing_id() {
        let dir = tempfile::tempdir().expect("tempdir");
        let original = Tag {
            id: "t-1".to_string(),
            name: "Work".to_string(),
            icon: "ri-briefcase-line".to_string(),
            color: "#3b82f6".to_string(),
            created_at: "0".to_string(),
        };
        write_tags_to(dir.path(), &[original]).expect("write");
        let updated = Tag {
            id: "t-1".to_string(),
            name: "Work Updated".to_string(),
            icon: "ri-briefcase-line".to_string(),
            color: "#ff0000".to_string(),
            created_at: "0".to_string(),
        };
        upsert_tag_in(dir.path(), updated).expect("upsert");
        let tags = read_tags_from(dir.path()).expect("read");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "Work Updated");
        assert_eq!(tags[0].color, "#ff0000");
    }

    #[test]
    fn tags_delete_removes_only_target() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = Tag {
            id: "t-a".to_string(),
            name: "A".to_string(),
            icon: String::new(),
            color: "#000".to_string(),
            created_at: "0".to_string(),
        };
        let b = Tag {
            id: "t-b".to_string(),
            name: "B".to_string(),
            icon: String::new(),
            color: "#000".to_string(),
            created_at: "0".to_string(),
        };
        write_tags_to(dir.path(), &[a, b]).expect("write");
        delete_tag_in(dir.path(), "t-a").expect("delete");
        let tags = read_tags_from(dir.path()).expect("read");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].id, "t-b");
    }

    // ── Session tags helpers ──────────────────────────────────────────────────

    #[test]
    fn session_tags_append_and_read_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let st = SessionTag {
            session_id: "s-1".to_string(),
            tag_id: "t-1".to_string(),
            duration: 1500,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        append_session_tag_in(dir.path(), st).expect("append");
        let loaded = read_session_tags_from(dir.path()).expect("read");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].session_id, "s-1");
    }

    // ── Reset helper ──────────────────────────────────────────────────────────

    #[test]
    fn delete_all_data_removes_present_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        for name in &[
            "session.json",
            "tasks.json",
            "settings.json",
            "history.json.corrupt",
            // AR-1 regression: feature 006's quick_logs.json and
            // distractions.json must also be wiped by reset-all-data.
            "quick_logs.json",
            "distractions.json",
            "quick_logs.json.corrupt",
            "distractions.json.corrupt",
        ] {
            std::fs::write(dir.path().join(name), b"{}").expect("write");
        }
        delete_all_data_in(dir.path()).expect("delete");
        assert!(!dir.path().join("session.json").exists());
        assert!(!dir.path().join("tasks.json").exists());
        assert!(!dir.path().join("settings.json").exists());
        assert!(!dir.path().join("history.json.corrupt").exists());
        assert!(!dir.path().join("quick_logs.json").exists());
        assert!(!dir.path().join("distractions.json").exists());
        assert!(!dir.path().join("quick_logs.json.corrupt").exists());
        assert!(!dir.path().join("distractions.json.corrupt").exists());
    }

    #[test]
    fn delete_all_data_removes_timestamped_corrupt_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("history.json.corrupt.1716985200"), b"{}").expect("write");
        std::fs::write(dir.path().join("quick_logs.json.corrupt.1716985200"), b"{}")
            .expect("write");
        delete_all_data_in(dir.path()).expect("delete");
        assert!(!dir.path().join("history.json.corrupt.1716985200").exists());
        assert!(!dir
            .path()
            .join("quick_logs.json.corrupt.1716985200")
            .exists());
    }

    #[test]
    fn delete_all_data_succeeds_when_no_files_present() {
        let dir = tempfile::tempdir().expect("tempdir");
        // Should not error even though no files exist.
        delete_all_data_in(dir.path()).expect("delete with no files");
    }

    // ── Cap tests ─────────────────────────────────────────────────────────────

    fn make_session_tag(n: u32) -> SessionTag {
        SessionTag {
            session_id: format!("s-{n}"),
            tag_id: "t-1".to_string(),
            duration: 1500,
            created_at: format!("2024-01-{:02}T00:00:00Z", (n % 28) + 1),
        }
    }

    #[test]
    fn session_tags_cap_trims_to_max_5000() {
        let dir = tempfile::tempdir().expect("tempdir");
        let tags: Vec<SessionTag> = (0..5_002).map(make_session_tag).collect();
        write_session_tags_to(dir.path(), &tags).expect("write");
        // Append one more to trigger the cap.
        append_session_tag_in(dir.path(), make_session_tag(5_002)).expect("append");
        let loaded = read_session_tags_from(dir.path()).expect("read");
        assert_eq!(loaded.len(), 5_000);
        // Oldest entries are drained; newest is last.
        assert_eq!(loaded[0].session_id, "s-3");
        assert_eq!(loaded[4999].session_id, "s-5002");
        // Recency ordering: last-appended entry is present, entry 0 is evicted.
        let last_id = "s-5002";
        let first_id = "s-0";
        assert!(
            loaded.iter().any(|s| s.session_id == last_id),
            "last-appended entry {last_id} must be retained after cap",
        );
        assert!(
            !loaded.iter().any(|s| s.session_id == first_id),
            "first entry {first_id} must be evicted after cap",
        );
    }

    fn make_quick_log(n: u32) -> QuickLog {
        QuickLog {
            id: format!("ql-{n}"),
            title: format!("log {n}"),
            elapsed_minutes: 25,
            created_at: format!("2024-01-{:02}T00:00:00Z", (n % 28) + 1),
            date: "Mon Jan 01 2024".to_string(),
        }
    }

    #[test]
    fn quick_logs_cold_start_cap_trims_to_max_1000() {
        let dir = tempfile::tempdir().expect("tempdir");
        let logs: Vec<QuickLog> = (0..1_002).map(make_quick_log).collect();
        write_quick_logs_to(dir.path(), &logs).expect("write");
        let loaded = read_quick_logs_from(dir.path()).expect("read");
        assert_eq!(loaded.len(), 1_000);
        assert_eq!(loaded[0].id, "ql-2");
        assert_eq!(loaded[999].id, "ql-1001");
        // Recency ordering: last entry is present, entry 0 is evicted.
        let last_id = "ql-1001";
        let first_id = "ql-0";
        assert!(
            loaded.iter().any(|s| s.id == last_id),
            "last entry {last_id} must be retained after cap",
        );
        assert!(
            !loaded.iter().any(|s| s.id == first_id),
            "first entry {first_id} must be evicted after cap",
        );
    }

    fn make_distraction(n: u32) -> Distraction {
        Distraction {
            id: format!("d-{n}"),
            note: format!("distraction {n}"),
            created_at: format!("2024-01-{:02}T00:00:00Z", (n % 28) + 1),
            date: "Mon Jan 01 2024".to_string(),
            parent_ref: None,
        }
    }

    #[test]
    fn distractions_cold_start_cap_trims_to_max_1000() {
        let dir = tempfile::tempdir().expect("tempdir");
        let entries: Vec<Distraction> = (0..1_002).map(make_distraction).collect();
        write_distractions_to(dir.path(), &entries).expect("write");
        let loaded = read_distractions_from(dir.path()).expect("read");
        assert_eq!(loaded.len(), 1_000);
        assert_eq!(loaded[0].id, "d-2");
        assert_eq!(loaded[999].id, "d-1001");
        // Recency ordering: last entry is present, entry 0 is evicted.
        let last_id = "d-1001";
        let first_id = "d-0";
        assert!(
            loaded.iter().any(|s| s.id == last_id),
            "last entry {last_id} must be retained after cap",
        );
        assert!(
            !loaded.iter().any(|s| s.id == first_id),
            "first entry {first_id} must be evicted after cap",
        );
    }
}
