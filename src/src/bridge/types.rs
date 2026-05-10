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

#[cfg(test)]
mod tests {
    use super::{ManualSession, Session, Task};
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
