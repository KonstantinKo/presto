// `QuickLog` wire record.
//
// Feature 006-timer-controls-quicklog-distractions §`QuickLog` in
// `data-model.md`. A small ad-hoc task log entry that the user wants
// to count, but doesn't justify starting a full pomodoro. Persisted
// as `Vec<QuickLog>` in `quick_logs.json` in the Tauri app-data
// directory.
//
// Conventions inherited from the wider `presto-ipc` crate: camelCase
// wire shape, opt-in `specta::Type` derive for cross-language binding
// generation. No validation logic in the struct — boundary checks
// (`title` length 1..=120, `elapsed_minutes` range 1..=720, UUID v4
// `id`, ISO-8601 `created_at`, `%a %b %d %Y` `date`) live at the
// Tauri command boundary per FR-022.

use serde::{Deserialize, Serialize};

/// User-entered quick-log entry. Counted by a per-period metric
/// distinct from `completed_pomodoros`; never affects
/// `pomodoros_until_long_break` (FR-027 + SC-005).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct QuickLog {
    /// UUID v4 string.
    pub id: String,
    /// User-provided. 1..=120 UTF-8 chars. PII — never log in plain.
    pub title: String,
    /// 1..=720 (1 min to 12 h).
    pub elapsed_minutes: u32,
    /// ISO-8601 UTC.
    pub created_at: String,
    /// chrono `%a %b %d %Y` (e.g. `"Fri May 15 2026"`). Matches the
    /// `ManualSession.date` precedent.
    pub date: String,
}
