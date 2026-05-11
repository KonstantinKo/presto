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
