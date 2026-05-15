// `Distraction` + `DistractionParentRef` wire records.
//
// Feature 006-timer-controls-quicklog-distractions §`Distraction` in
// `data-model.md`. A mid-session interruption note. Captured in one of
// two contexts:
//
// - In-session — from the Running right-button modal. `parent_ref`
//   is `Some(_)` with a snapshot of the running session.
// - Retroactive — from the Inventory. `parent_ref` is `None`.
//
// `parent_ref` uses `#[serde(default)]` so retroactive entries (and
// any future forward-compat writes) deserialise cleanly with `None`.
// Validation (note length 1..=120, UUID v4 `id`, ISO-8601 timestamps,
// `%a %b %d %Y` date) happens at the Tauri command boundary per
// FR-022.

use serde::{Deserialize, Serialize};

use crate::timer::TimerMode;

/// User-entered distraction note, optionally tied to the parent
/// session that was running when the note was captured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Distraction {
    /// UUID v4 string.
    pub id: String,
    /// User-provided. 1..=120 UTF-8 chars. PII — never log in plain.
    pub note: String,
    /// ISO-8601 UTC. Modal-submit time for in-session captures;
    /// user-set or `now()` for retroactive entries.
    pub created_at: String,
    /// chrono `%a %b %d %Y`. Matches the `ManualSession.date`
    /// precedent.
    pub date: String,
    /// Snapshotted at modal-open time (per spec Clarifications +
    /// Edge Cases). `None` for retroactive entries from Inventory.
    #[serde(default)]
    pub parent_ref: Option<DistractionParentRef>,
}

/// Snapshot of the parent session at the moment the Distraction modal was opened.
///
/// Title is rendered as-snapshotted (never re-resolved); tag is
/// re-resolved against the current tag table at render time
/// (FR-024a).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DistractionParentRef {
    /// ISO-8601 — when the parent session started.
    pub parent_session_start_ts: String,
    /// The running mode at modal-open.
    pub parent_mode: TimerMode,
    /// The selected tag id at modal-open. `None` if no tag was set.
    pub parent_tag_id: Option<String>,
    /// The session title at modal-open. `None` if not set.
    pub parent_title: Option<String>,
}
