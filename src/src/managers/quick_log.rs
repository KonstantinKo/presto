// `QuickLogManager` — feature 006 §Phase 5 (T042 RED skeleton).
//
// Skeleton only. T044 GREEN swaps the `todo!()` bodies for the real
// in-memory CRUD + bridge plumbing.
#![allow(clippy::future_not_send)]
#![allow(dead_code)]
#![allow(unused_variables)]

use crate::bridge::types::BridgeError;
use crate::bridge::types::QuickLog;

#[derive(Debug, Clone, Default)]
pub struct QuickLogManager {
    entries: Vec<QuickLog>,
}

impl QuickLogManager {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    #[must_use]
    pub fn entries(&self) -> &[QuickLog] {
        todo!("T044 GREEN")
    }

    #[must_use]
    pub fn save_payload(&self) -> Vec<QuickLog> {
        todo!("T044 GREEN")
    }

    pub fn add(&mut self, title: String, elapsed_minutes: u32, now_ms: i64, id: String) {
        todo!("T044 GREEN")
    }

    pub fn update_by_id(&mut self, updated: QuickLog) {
        todo!("T044 GREEN")
    }

    pub fn delete_by_id(&mut self, id: &str) {
        todo!("T044 GREEN")
    }

    #[must_use]
    pub fn entries_for_date(&self, date_str: &str) -> Vec<&QuickLog> {
        todo!("T044 GREEN")
    }

    /// # Errors
    /// Placeholder for the GREEN impl.
    pub fn from_loaded_or_default(
        loaded: Result<Vec<QuickLog>, BridgeError>,
    ) -> Result<Self, BridgeError> {
        todo!("T044 GREEN")
    }
}

#[cfg(test)]
mod tests {
    use super::QuickLogManager;
    use crate::bridge::types::{BridgeError, QuickLog};

    fn sample(id: &str, title: &str, elapsed: u32, date: &str) -> QuickLog {
        QuickLog {
            id: id.to_string(),
            title: title.to_string(),
            elapsed_minutes: elapsed,
            created_at: "2026-05-15T09:00:00+00:00".to_string(),
            date: date.to_string(),
        }
    }

    #[test]
    fn add_then_load_round_trips_entry() {
        let mut mgr = QuickLogManager::new();
        let day_ms: i64 = 1_704_067_200_000;
        mgr.add("Reply to PR".to_string(), 5, day_ms, "qid-1".to_string());

        assert_eq!(mgr.entries().len(), 1);
        let q = &mgr.entries()[0];
        assert_eq!(q.id, "qid-1");
        assert_eq!(q.title, "Reply to PR");
        assert_eq!(q.elapsed_minutes, 5);
        assert_eq!(
            q.date,
            crate::engine::date_format::format_session_date(day_ms)
        );
        assert!(q.created_at.contains('T'));

        let payload = mgr.save_payload();
        assert_eq!(payload.len(), 1);
        assert_eq!(payload[0].id, "qid-1");
    }

    #[test]
    fn update_replaces_in_place() {
        let mut mgr = QuickLogManager::new();
        mgr.add("a".to_string(), 5, 1_704_067_200_000, "qid-1".to_string());
        mgr.add("b".to_string(), 7, 1_704_067_200_000, "qid-2".to_string());
        assert_eq!(mgr.entries().len(), 2);

        let mut updated = mgr.entries()[0].clone();
        updated.title = "a (edited)".to_string();
        updated.elapsed_minutes = 9;
        mgr.update_by_id(updated);

        assert_eq!(mgr.entries().len(), 2, "update must not change length");
        let q1 = mgr.entries().iter().find(|q| q.id == "qid-1").unwrap();
        assert_eq!(q1.title, "a (edited)");
        assert_eq!(q1.elapsed_minutes, 9);

        mgr.update_by_id(sample("qid-nope", "ghost", 1, "Fri May 15 2026"));
        assert_eq!(mgr.entries().len(), 2, "update-of-unknown-id no-op");
    }

    #[test]
    fn delete_removes_only_target() {
        let mut mgr = QuickLogManager::new();
        mgr.add("a".to_string(), 5, 1_704_067_200_000, "qid-1".to_string());
        mgr.add("b".to_string(), 7, 1_704_067_200_000, "qid-2".to_string());
        mgr.add("c".to_string(), 9, 1_704_067_200_000, "qid-3".to_string());

        mgr.delete_by_id("qid-2");

        assert_eq!(mgr.entries().len(), 2);
        assert!(mgr.entries().iter().all(|q| q.id != "qid-2"));

        mgr.delete_by_id("qid-nope");
        assert_eq!(mgr.entries().len(), 2);
    }

    #[test]
    fn entries_for_date_filters_by_date_field() {
        let mut mgr = QuickLogManager::new();
        let day_one: i64 = 1_704_067_200_000;
        let day_two: i64 = day_one + 86_400_000;
        let day_one_str = crate::engine::date_format::format_session_date(day_one);
        let day_two_str = crate::engine::date_format::format_session_date(day_two);

        mgr.add("a".to_string(), 5, day_one, "qid-1".to_string());
        mgr.add("b".to_string(), 7, day_one, "qid-2".to_string());
        mgr.add("c".to_string(), 9, day_two, "qid-3".to_string());

        let d1 = mgr.entries_for_date(&day_one_str);
        let d2 = mgr.entries_for_date(&day_two_str);
        let unknown = mgr.entries_for_date("Sun Jan 01 1970");

        assert_eq!(d1.len(), 2);
        assert!(d1.iter().all(|q| q.date == day_one_str));
        assert_eq!(d2.len(), 1);
        assert_eq!(d2[0].id, "qid-3");
        assert!(unknown.is_empty());
    }

    #[test]
    fn load_then_save_round_trip() {
        let mut mgr = QuickLogManager::new();
        mgr.add("a".to_string(), 5, 1_704_067_200_000, "qid-1".to_string());
        mgr.add("b".to_string(), 7, 1_704_067_200_000, "qid-2".to_string());
        let payload = mgr.save_payload();
        assert_eq!(payload.len(), 2);

        let reloaded = QuickLogManager::from_loaded_or_default(Ok(payload.clone())).unwrap();
        assert_eq!(reloaded.entries().len(), 2);
        assert_eq!(reloaded.entries()[0].id, "qid-1");
        assert_eq!(reloaded.entries()[1].id, "qid-2");

        let cold =
            QuickLogManager::from_loaded_or_default(Err(BridgeError::BridgeUnavailable)).unwrap();
        assert!(cold.entries().is_empty());

        let propagated = QuickLogManager::from_loaded_or_default(Err(BridgeError::Internal {
            msg: "disk".to_string(),
        }));
        assert!(propagated.is_err());
    }
}
