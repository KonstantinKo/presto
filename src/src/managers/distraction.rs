// `DistractionManager` — feature 006 §Phase 5 (T043/T045).
//
// Mirrors `QuickLogManager` (same file's twin) and the
// `SessionManager` precedent. Owns an in-memory `Vec<Distraction>`;
// bulk-re-saves on every mutation. Per FR-035 a Distraction never
// touches the engine — `add()` only mutates manager state.
//
// Lint allowance: `clippy::future_not_send` is allowed at the module
// level — `JsValue` is `!Send` on `wasm32-unknown-unknown`.
#![allow(
    clippy::future_not_send,
    reason = "Manager async paths await wasm32 Tauri bridge futures that carry !Send JsValue."
)]

use crate::bridge::commands;
use crate::bridge::types::BridgeError;
use crate::bridge::types::Distraction;
use crate::bridge::types::DistractionParentRef;

/// In-memory authority over the user's distraction entries.
#[derive(Debug, Clone, Default)]
pub struct DistractionManager {
    entries: Vec<Distraction>,
}

impl DistractionManager {
    /// Construct an empty manager.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Borrow the current entries.
    #[must_use]
    pub fn entries(&self) -> &[Distraction] {
        &self.entries
    }

    /// Bulk save payload — `Vec<Distraction>` matching the
    /// `save_distractions(distractions)` wire shape.
    #[must_use]
    pub fn save_payload(&self) -> Vec<Distraction> {
        self.entries.clone()
    }

    /// Append a new distraction entry. `id` is caller-supplied;
    /// derives ISO-8601 `created_at`, chrono `%a %b %d %Y` `date`.
    /// `parent_ref` is `Some(_)` for in-session captures
    /// (snapshotted at modal-open per spec Clarifications +
    /// Edge Cases) and `None` for retroactive entries.
    pub fn add(
        &mut self,
        note: String,
        parent_ref: Option<DistractionParentRef>,
        now_ms: i64,
        id: String,
    ) {
        let date = crate::engine::date_format::format_session_date(now_ms);
        let created_at = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(now_ms)
            .unwrap_or_default()
            .to_rfc3339();
        self.entries.push(Distraction {
            id,
            note,
            created_at,
            date,
            parent_ref,
        });
    }

    /// Replace the matching entry by `id`. Update-of-unknown-id is
    /// a no-op.
    pub fn update_by_id(&mut self, updated: Distraction) {
        if let Some(slot) = self.entries.iter_mut().find(|d| d.id == updated.id) {
            *slot = updated;
        }
    }

    /// Remove by `id`. Delete-of-unknown-id is a no-op.
    pub fn delete_by_id(&mut self, id: &str) {
        self.entries.retain(|d| d.id != id);
    }

    /// Entries whose `date` field equals `date_str`.
    #[must_use]
    pub fn entries_for_date(&self, date_str: &str) -> Vec<&Distraction> {
        self.entries.iter().filter(|d| d.date == date_str).collect()
    }

    /// Entries whose `parent_ref.parent_session_start_ts` matches
    /// the given timestamp. Used by Inventory views that want the
    /// "all distractions captured while this session was running"
    /// projection.
    #[must_use]
    pub fn parent_session_filter(&self, parent_session_start_ts: &str) -> Vec<&Distraction> {
        self.entries
            .iter()
            .filter(|d| {
                d.parent_ref
                    .as_ref()
                    .is_some_and(|p| p.parent_session_start_ts == parent_session_start_ts)
            })
            .collect()
    }

    /// Build a manager from a previously-loaded list. Only
    /// `BridgeUnavailable` falls back to empty; other errors
    /// propagate.
    ///
    /// # Errors
    /// Returns `Err(e)` for any `BridgeError` variant other than
    /// `BridgeUnavailable`.
    pub fn from_loaded_or_default(
        loaded: Result<Vec<Distraction>, BridgeError>,
    ) -> Result<Self, BridgeError> {
        match loaded {
            Ok(entries) => Ok(Self { entries }),
            Err(BridgeError::BridgeUnavailable) => Ok(Self::new()),
            Err(e) => Err(e),
        }
    }

    /// Async cold-start path.
    ///
    /// # Errors
    /// Returns `Err(e)` for any bridge or IO error other than
    /// `BridgeUnavailable`.
    pub async fn load() -> Result<Self, BridgeError> {
        Self::from_loaded_or_default(commands::load_distractions().await)
    }

    /// Async save path.
    ///
    /// # Errors
    /// Returns whatever `bridge::commands::save_distractions`
    /// returns.
    pub async fn save(&self) -> Result<(), BridgeError> {
        commands::save_distractions(self.save_payload()).await
    }
}

#[cfg(test)]
mod tests {
    use super::DistractionManager;
    use crate::bridge::types::{BridgeError, Distraction, DistractionParentRef, TimerMode};

    fn parent_ref(
        start_ts: &str,
        title: Option<&str>,
        tag_id: Option<&str>,
    ) -> DistractionParentRef {
        DistractionParentRef {
            parent_session_start_ts: start_ts.to_string(),
            parent_mode: TimerMode::Focus,
            parent_tag_id: tag_id.map(str::to_string),
            parent_title: title.map(str::to_string),
        }
    }

    fn sample(id: &str, note: &str, date: &str) -> Distraction {
        Distraction {
            id: id.to_string(),
            note: note.to_string(),
            created_at: "2026-05-15T09:00:00+00:00".to_string(),
            date: date.to_string(),
            parent_ref: None,
        }
    }

    /// T043: `add()` populates the in-memory list with derived
    /// `id`/`created_at`/`date` and the user-supplied note +
    /// `parent_ref`.
    #[test]
    fn add_with_parent_ref_round_trips_entry() {
        let mut mgr = DistractionManager::new();
        let day_ms: i64 = 1_704_067_200_000;
        let pref = parent_ref("2026-05-15T09:00:00Z", Some("Reading"), Some("tag-x"));
        mgr.add(
            "phone buzz".to_string(),
            Some(pref),
            day_ms,
            "did-1".to_string(),
        );

        assert_eq!(mgr.entries().len(), 1);
        let d = &mgr.entries()[0];
        assert_eq!(d.id, "did-1");
        assert_eq!(d.note, "phone buzz");
        assert_eq!(
            d.parent_ref.as_ref().unwrap().parent_tag_id.as_deref(),
            Some("tag-x")
        );
        assert_eq!(
            d.date,
            crate::engine::date_format::format_session_date(day_ms)
        );
        assert!(d.created_at.contains('T'));
    }

    /// T043: retroactive `add(note, None, ..)` — `parent_ref` absent.
    #[test]
    fn add_retroactive_without_parent_ref() {
        let mut mgr = DistractionManager::new();
        mgr.add(
            "retroactive".to_string(),
            None,
            1_704_067_200_000,
            "did-2".to_string(),
        );

        assert_eq!(mgr.entries().len(), 1);
        assert!(mgr.entries()[0].parent_ref.is_none());
    }

    /// T043: `update_by_id()` replaces in place.
    #[test]
    fn update_replaces_in_place() {
        let mut mgr = DistractionManager::new();
        mgr.add(
            "a".to_string(),
            None,
            1_704_067_200_000,
            "did-1".to_string(),
        );
        mgr.add(
            "b".to_string(),
            None,
            1_704_067_200_000,
            "did-2".to_string(),
        );

        let mut updated = mgr.entries()[0].clone();
        updated.note = "a (edited)".to_string();
        mgr.update_by_id(updated);

        assert_eq!(mgr.entries().len(), 2);
        let d1 = mgr.entries().iter().find(|d| d.id == "did-1").unwrap();
        assert_eq!(d1.note, "a (edited)");

        // Update-of-unknown-id no-op.
        mgr.update_by_id(sample("did-nope", "ghost", "Fri May 15 2026"));
        assert_eq!(mgr.entries().len(), 2);
    }

    /// T043: `delete_by_id()` removes only the match.
    #[test]
    fn delete_removes_only_target() {
        let mut mgr = DistractionManager::new();
        mgr.add(
            "a".to_string(),
            None,
            1_704_067_200_000,
            "did-1".to_string(),
        );
        mgr.add(
            "b".to_string(),
            None,
            1_704_067_200_000,
            "did-2".to_string(),
        );
        mgr.add(
            "c".to_string(),
            None,
            1_704_067_200_000,
            "did-3".to_string(),
        );

        mgr.delete_by_id("did-2");
        assert_eq!(mgr.entries().len(), 2);
        assert!(mgr.entries().iter().all(|d| d.id != "did-2"));

        mgr.delete_by_id("did-nope");
        assert_eq!(mgr.entries().len(), 2);
    }

    /// T043: `entries_for_date()` filters by `date`.
    #[test]
    fn entries_for_date_filters_by_date_field() {
        let mut mgr = DistractionManager::new();
        let day_one: i64 = 1_704_067_200_000;
        let day_two: i64 = day_one + 86_400_000;
        let day_one_str = crate::engine::date_format::format_session_date(day_one);

        mgr.add("a".to_string(), None, day_one, "did-1".to_string());
        mgr.add("b".to_string(), None, day_one, "did-2".to_string());
        mgr.add("c".to_string(), None, day_two, "did-3".to_string());

        let d1 = mgr.entries_for_date(&day_one_str);
        assert_eq!(d1.len(), 2);
    }

    /// T043: `parent_session_filter` matches on
    /// `parent_ref.parent_session_start_ts`. Retroactive entries
    /// (no `parent_ref`) are skipped.
    #[test]
    fn parent_session_filter_matches_only_matching_start_ts() {
        let mut mgr = DistractionManager::new();
        let p1 = parent_ref("2026-05-15T09:00:00Z", None, None);
        let p2 = parent_ref("2026-05-15T10:00:00Z", None, None);

        mgr.add(
            "a".to_string(),
            Some(p1.clone()),
            1_704_067_200_000,
            "did-1".to_string(),
        );
        mgr.add(
            "b".to_string(),
            Some(p1),
            1_704_067_200_000,
            "did-2".to_string(),
        );
        mgr.add(
            "c".to_string(),
            Some(p2),
            1_704_067_200_000,
            "did-3".to_string(),
        );
        mgr.add(
            "retro".to_string(),
            None,
            1_704_067_200_000,
            "did-4".to_string(),
        );

        let matches = mgr.parent_session_filter("2026-05-15T09:00:00Z");
        assert_eq!(matches.len(), 2);
        assert!(matches.iter().all(|d| d.id == "did-1" || d.id == "did-2"));

        let none = mgr.parent_session_filter("2026-05-15T11:00:00Z");
        assert!(none.is_empty());
    }

    /// T043: bulk-re-save round-trip via `save_payload` ↔
    /// `from_loaded_or_default`.
    #[test]
    fn load_then_save_round_trip() {
        let mut mgr = DistractionManager::new();
        mgr.add(
            "a".to_string(),
            None,
            1_704_067_200_000,
            "did-1".to_string(),
        );
        let payload = mgr.save_payload();
        let reloaded = DistractionManager::from_loaded_or_default(Ok(payload)).unwrap();
        assert_eq!(reloaded.entries().len(), 1);

        let cold = DistractionManager::from_loaded_or_default(Err(BridgeError::BridgeUnavailable))
            .unwrap();
        assert!(cold.entries().is_empty());

        let propagated = DistractionManager::from_loaded_or_default(Err(BridgeError::Internal {
            msg: "disk".to_string(),
        }));
        assert!(propagated.is_err());
    }
}
