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
#[allow(clippy::redundant_pub_crate)]
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
#[allow(clippy::redundant_pub_crate)]
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
// pub(super) is the correct visibility here: the function is used by the parent module
// (lib.rs) and its descendants (the test module), but not from anywhere else in the crate.
// clippy::redundant_pub_crate fires because the enclosing module is private; however,
// pub(super) is intentionally more restrictive than pub(crate).
#[allow(clippy::redundant_pub_crate)]
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
#[allow(clippy::redundant_pub_crate)]
pub(super) fn read_settings_from(dir: &Path) -> Result<super::AppSettings, std::io::Error> {
    let file_path = dir.join("settings.json");
    if !file_path.exists() {
        return Ok(super::AppSettings::default());
    }
    match fs::read_to_string(&file_path) {
        Ok(contents) => Ok(serde_json::from_str(&contents).unwrap_or_default()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(super::AppSettings::default()),
        Err(e) => Err(e),
    }
}

/// Creates `dir` if necessary, then atomically writes `settings` to
/// `settings.json`.
#[allow(clippy::redundant_pub_crate)]
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
#[allow(clippy::redundant_pub_crate)]
pub(super) fn read_session_from(dir: &Path) -> Result<Option<super::PomodoroSession>, String> {
    let file_path = dir.join("session.json");
    if !file_path.exists() {
        return Ok(None);
    }
    let content =
        fs::read_to_string(&file_path).map_err(|e| format!("Failed to read session file: {e}"))?;
    let mut session: super::PomodoroSession =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse session: {e}"))?;

    let today_legacy = chrono::Local::now().format("%a %b %d %Y").to_string();
    let today_iso = chrono::Local::now().format("%Y-%m-%d").to_string();

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
#[allow(clippy::redundant_pub_crate)]
pub(super) fn write_session_to(dir: &Path, session: &super::PomodoroSession) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory: {e}"))?;
    write_json_atomic(&dir.join("session.json"), session)
}

// ── Tasks ─────────────────────────────────────────────────────────────────────

/// Reads `tasks.json` from `dir`, returning an empty vec when absent.
#[allow(clippy::redundant_pub_crate)]
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
#[allow(clippy::redundant_pub_crate)]
pub(super) fn write_tasks_to(dir: &Path, tasks: &[super::Task]) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory: {e}"))?;
    write_json_atomic(&dir.join("tasks.json"), tasks)
}

// ── History ───────────────────────────────────────────────────────────────────

/// Reads `history.json` from `dir`, returning an empty vec when absent.
#[allow(clippy::redundant_pub_crate)]
pub(super) fn read_history_from(dir: &Path) -> Result<Vec<super::PomodoroSession>, String> {
    let history_path = dir.join("history.json");
    if !history_path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(history_path)
        .map_err(|e| format!("Failed to read history file: {e}"))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse history: {e}"))
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
#[allow(clippy::redundant_pub_crate)]
pub(super) fn append_daily_stats_to(
    dir: &Path,
    session: &super::PomodoroSession,
) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory: {e}"))?;
    let history_path = dir.join("history.json");

    let mut history: Vec<super::PomodoroSession> = if history_path.exists() {
        let content = fs::read_to_string(&history_path)
            .map_err(|e| format!("Failed to read history: {e}"))?;
        match serde_json::from_str(&content) {
            Ok(h) => h,
            Err(e) => {
                let corrupt_path = history_path.with_extension("json.corrupt");
                match fs::rename(&history_path, &corrupt_path) {
                    Ok(()) => log::warn!(
                        "history.json could not be parsed, preserved as {}: {e}",
                        corrupt_path.display()
                    ),
                    Err(rename_err) => log::warn!(
                        "history.json could not be parsed and rename to .corrupt failed ({rename_err}): {e}"
                    ),
                }
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

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
#[allow(clippy::redundant_pub_crate)]
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
#[allow(clippy::redundant_pub_crate)]
pub(super) fn write_manual_sessions_to(
    dir: &Path,
    sessions: &[super::ManualSession],
) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory: {e}"))?;
    write_json_atomic(&dir.join("manual_sessions.json"), sessions)
}

/// Inserts or replaces the entry matching `session.id` in
/// `manual_sessions.json`.
#[allow(clippy::redundant_pub_crate)]
pub(super) fn upsert_manual_session_in(
    dir: &Path,
    session: super::ManualSession,
) -> Result<(), String> {
    let mut sessions = read_manual_sessions_from(dir)?;
    sessions.retain(|s| s.id != session.id);
    sessions.push(session);
    write_manual_sessions_to(dir, &sessions)
}

/// Removes the entry matching `session_id` from `manual_sessions.json`.
#[allow(clippy::redundant_pub_crate)]
pub(super) fn delete_manual_session_in(dir: &Path, session_id: &str) -> Result<(), String> {
    let mut sessions = read_manual_sessions_from(dir)?;
    sessions.retain(|s| s.id != session_id);
    write_manual_sessions_to(dir, &sessions)
}

// ── Tags ──────────────────────────────────────────────────────────────────────

/// Reads `tags.json` from `dir`.
///
/// When the file is absent, bootstraps and persists a default "Focus" tag so
/// that subsequent reads are consistent.
#[allow(clippy::redundant_pub_crate)]
pub(super) fn read_tags_from(dir: &Path) -> Result<Vec<super::Tag>, String> {
    let file_path = dir.join("tags.json");
    if file_path.exists() {
        let content =
            fs::read_to_string(&file_path).map_err(|e| format!("Failed to read tags: {e}"))?;
        return serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse tags.json: {e}"));
    }
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
    Ok(tags)
}

/// Creates `dir` if necessary, then atomically writes `tags` to `tags.json`.
#[allow(clippy::redundant_pub_crate)]
pub(super) fn write_tags_to(dir: &Path, tags: &[super::Tag]) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory: {e}"))?;
    write_json_atomic(&dir.join("tags.json"), tags)
}

/// Inserts or replaces the entry matching `tag.id` in `tags.json`.
#[allow(clippy::redundant_pub_crate)]
pub(super) fn upsert_tag_in(dir: &Path, tag: super::Tag) -> Result<(), String> {
    let mut tags = read_tags_from(dir)?;
    tags.retain(|t| t.id != tag.id);
    tags.push(tag);
    write_tags_to(dir, &tags)
}

/// Removes the entry matching `tag_id` from `tags.json`.
#[allow(clippy::redundant_pub_crate)]
pub(super) fn delete_tag_in(dir: &Path, tag_id: &str) -> Result<(), String> {
    let mut tags = read_tags_from(dir)?;
    tags.retain(|t| t.id != tag_id);
    write_tags_to(dir, &tags)
}

// ── Session tags ──────────────────────────────────────────────────────────────

/// Reads `session_tags.json` from `dir`, returning an empty vec when absent.
#[allow(clippy::redundant_pub_crate)]
pub(super) fn read_session_tags_from(dir: &Path) -> Result<Vec<super::SessionTag>, String> {
    let file_path = dir.join("session_tags.json");
    if file_path.exists() {
        let content = fs::read_to_string(&file_path)
            .map_err(|e| format!("Failed to read session tags: {e}"))?;
        return serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse session_tags.json: {e}"));
    }
    Ok(Vec::new())
}

/// Creates `dir` if necessary, then atomically writes `session_tags` to
/// `session_tags.json`.
#[allow(clippy::redundant_pub_crate)]
pub(super) fn write_session_tags_to(
    dir: &Path,
    session_tags: &[super::SessionTag],
) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory: {e}"))?;
    write_json_atomic(&dir.join("session_tags.json"), session_tags)
}

/// Appends `session_tag` to `session_tags.json`.
#[allow(clippy::redundant_pub_crate)]
pub(super) fn append_session_tag_in(
    dir: &Path,
    session_tag: super::SessionTag,
) -> Result<(), String> {
    let mut session_tags = read_session_tags_from(dir)?;
    session_tags.push(session_tag);
    write_session_tags_to(dir, &session_tags)
}

// ── Reset ─────────────────────────────────────────────────────────────────────

/// Deletes all known data files from `dir`. Files that do not exist are
/// silently skipped so the function is idempotent.
#[allow(clippy::redundant_pub_crate)]
pub(super) fn delete_all_data_in(dir: &Path) -> Result<(), String> {
    const FILES: &[&str] = &[
        "session.json",
        "tasks.json",
        "history.json",
        "settings.json",
        "manual_sessions.json",
        "tags.json",
        "session_tags.json",
    ];
    for file_name in FILES {
        let file_path = dir.join(file_name);
        if let Err(e) = fs::remove_file(&file_path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(format!("Failed to delete {file_name}: {e}"));
            }
        }
    }
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::{
        append_daily_stats_to, append_session_tag_in, delete_all_data_in, delete_manual_session_in,
        delete_tag_in, is_debounced, read_history_from, read_manual_sessions_from,
        read_session_from, read_session_tags_from, read_settings_from, read_tags_from,
        read_tasks_from, upsert_manual_session_in, upsert_tag_in, write_manual_sessions_to,
        write_session_to, write_settings_to, write_tags_to, write_tasks_to,
    };
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    // Re-use parent-module types (private to lib.rs but accessible from descendants).
    use super::super::{AppSettings, ManualSession, PomodoroSession, SessionTag, Tag, Task};

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
        assert_eq!(result.analytics_enabled, defaults.analytics_enabled);
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
    fn history_appending_31st_entry_prunes_oldest() {
        let dir = tempfile::tempdir().expect("tempdir");
        for i in 0u32..31u32 {
            let session = PomodoroSession {
                completed_pomodoros: i,
                total_focus_time: 0,
                current_session: 1,
                date: format!("2024-01-{:02}", i + 1),
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
        };
        let second = PomodoroSession {
            completed_pomodoros: 5,
            total_focus_time: 0,
            current_session: 1,
            date: "2024-06-01".to_string(),
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
        };
        let iso = PomodoroSession {
            completed_pomodoros: 5,
            total_focus_time: 0,
            current_session: 1,
            date: "2024-06-01".to_string(),
        };
        append_daily_stats_to(dir.path(), &legacy).expect("append legacy");
        append_daily_stats_to(dir.path(), &iso).expect("append iso");
        let history = read_history_from(dir.path()).expect("read");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].completed_pomodoros, 5);
        assert_eq!(history[0].date, "2024-06-01");
    }

    // ── Manual session helpers ────────────────────────────────────────────────

    fn make_manual_session(id: &str, date: &str) -> ManualSession {
        ManualSession {
            id: id.to_string(),
            session_type: "focus".to_string(),
            duration: 25,
            start_time: "09:00".to_string(),
            end_time: "09:25".to_string(),
            notes: None,
            created_at: "2024-01-01T09:00:00Z".to_string(),
            date: date.to_string(),
            tags: None,
        }
    }

    #[test]
    fn manual_session_upsert_replaces_existing_id() {
        let dir = tempfile::tempdir().expect("tempdir");
        let original = make_manual_session("s-1", "2024-06-01");
        let updated = ManualSession {
            duration: 50,
            ..make_manual_session("s-1", "2024-06-01")
        };
        let other = make_manual_session("s-2", "2024-06-01");
        write_manual_sessions_to(dir.path(), &[original, other]).expect("write");
        upsert_manual_session_in(dir.path(), updated).expect("upsert");
        let sessions = read_manual_sessions_from(dir.path()).expect("read");
        assert_eq!(sessions.len(), 2);
        let s1 = sessions.iter().find(|s| s.id == "s-1").expect("s-1");
        assert_eq!(s1.duration, 50);
        assert!(sessions.iter().any(|s| s.id == "s-2"));
    }

    #[test]
    fn manual_session_delete_removes_only_target() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = make_manual_session("s-a", "2024-06-01");
        let b = make_manual_session("s-b", "2024-06-01");
        write_manual_sessions_to(dir.path(), &[a, b]).expect("write");
        delete_manual_session_in(dir.path(), "s-a").expect("delete");
        let sessions = read_manual_sessions_from(dir.path()).expect("read");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "s-b");
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
        for name in &["session.json", "tasks.json", "settings.json"] {
            std::fs::write(dir.path().join(name), b"{}").expect("write");
        }
        delete_all_data_in(dir.path()).expect("delete");
        assert!(!dir.path().join("session.json").exists());
        assert!(!dir.path().join("tasks.json").exists());
        assert!(!dir.path().join("settings.json").exists());
    }

    #[test]
    fn delete_all_data_succeeds_when_no_files_present() {
        let dir = tempfile::tempdir().expect("tempdir");
        // Should not error even though no files exist.
        delete_all_data_in(dir.path()).expect("delete with no files");
    }
}
