// Pomodoro session + manual-entry wire records.
//
// On-disk shape: `snake_case` JSON via serde defaults. The `date`
// field on `Session`/`ManualSession` is the chrono format
// `%a %b %d %Y` (matches JS `Date.prototype.toDateString()`
// exact-byte; pinned by `engine::date_format`).

use serde::{Deserialize, Serialize};

use crate::timer::SessionType;

/// Pomodoro session record persisted in the user's app-data
/// directory. Backend type alias: `PomodoroSession`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Session {
    pub completed_pomodoros: u32,
    /// Seconds.
    pub total_focus_time: u32,
    pub current_session: u32,
    /// `%a %b %d %Y` (e.g., "Sat May 10 2026").
    pub date: String,
    /// User-typed session title (≤120 user-perceived chars).
    /// `None` for sessions created before this field existed
    /// (feature 002), and for in-flight sessions that completed
    /// without a typed title. Empty-string is forbidden —
    /// normalised to `None` at the capture boundary per Principle
    /// III.
    #[serde(default)]
    pub title: Option<String>,
}

/// User-entered manual session record.
///
/// `session_type` is the closed-domain `SessionType` (was a
/// stringly-typed `String` pre-cutover per spec 001 T029).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
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
    /// Inline tag objects per the existing JS-era on-disk shape
    /// (FR-005). Loosely typed because legacy records embed full tag
    /// objects rather than ID-only references; we normalise at
    /// consumption time without reshaping on disk.
    pub tags: Option<Vec<serde_json::Value>>,
}

#[cfg(test)]
mod tests {
    use super::Session;

    /// T001 (RED → T002 GREEN): `Session::title` round-trips Some,
    /// None, and the legacy no-key shape. The legacy fixture mirrors
    /// pre-002 `history.json` records — feature 002 spec FR-001 +
    /// data-model.md §Evolution 1.
    #[test]
    fn title_round_trip_some_none_missing_key() {
        // Some — typed title round-trips byte-stable.
        let s1 = Session {
            completed_pomodoros: 3,
            total_focus_time: 4500,
            current_session: 4,
            date: "Sat May 10 2026".to_string(),
            title: Some("Spec 002 review".to_string()),
        };
        let json1 = serde_json::to_string(&s1).expect("serialise Some");
        let s1_back: Session = serde_json::from_str(&json1).expect("deserialise Some");
        assert_eq!(s1_back.title.as_deref(), Some("Spec 002 review"));

        // None — round-trips as None.
        let s2 = Session { title: None, ..s1 };
        let json2 = serde_json::to_string(&s2).expect("serialise None");
        let s2_back: Session = serde_json::from_str(&json2).expect("deserialise None");
        assert!(s2_back.title.is_none());

        // Legacy — pre-bundle JSON without the key deserialises as None.
        let legacy = r#"{"completed_pomodoros":3,"total_focus_time":4500,"current_session":4,"date":"Sat May 10 2026"}"#;
        let s3: Session = serde_json::from_str(legacy).expect("deserialise legacy");
        assert!(s3.title.is_none());
    }
}
