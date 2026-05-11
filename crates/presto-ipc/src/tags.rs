// Tag catalogue + per-session tag attribution wire records.

use serde::{Deserialize, Serialize};

/// User-defined tag attached to sessions and manual entries.
///
/// `icon` carries either an emoji or a Remix icon class (e.g.
/// `"ri-briefcase-line"`); `color` is a hex string (e.g. `"#3b82f6"`).
/// Both are pinned to `String` because the JS-era on-disk records mix
/// the two conventions and a closed-domain enum would force a
/// migration (FR-005 — no on-disk shape change).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub color: String,
    pub created_at: String,
}

/// Per-session per-tag time-spent join row.
///
/// `duration` is wall-clock seconds spent on this tag during the
/// named session — distinct from `ManualSession::duration` (minutes).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SessionTag {
    pub session_id: String,
    pub tag_id: String,
    /// Seconds.
    pub duration: u32,
    pub created_at: String,
}
