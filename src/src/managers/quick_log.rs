// `QuickLogManager` — feature 006 §Phase 5 (T042/T044).
//
// Mirrors `SessionManager` (`src/src/managers/session.rs:20-22`) per
// finding AG-7. Owns an in-memory `Vec<QuickLog>`; bulk-re-saves on
// every mutation. Reaches the Tauri side exclusively through
// `bridge::commands::{load_quick_logs, save_quick_logs}` (Principle VI).
//
// QuickLog entries are pure metric counters — they never touch the
// engine accumulators or `pomodoros_until_long_break` (FR-027,
// SC-005). The manager is therefore distinct from `SessionManager`
// which routes `ManualSession` writes through the engine first.
//
// Lint allowance: `clippy::future_not_send` is allowed at the module
// level for the same reason as on `bridge::commands` and the other
// managers — every async path here transitively awaits a `JsFuture`
// from `bridge::commands`, which is `!Send` on `wasm32-unknown-unknown`.
#![allow(
    clippy::future_not_send,
    reason = "Manager async paths await wasm32 Tauri bridge futures that carry !Send JsValue."
)]

use crate::bridge::commands;
use crate::bridge::types::BridgeError;
use crate::bridge::types::QuickLog;

/// In-memory authority over the user's quick-log entries. The on-disk
/// half is the bulk `save_quick_logs(entries)` rewrite per
/// data-model.md §`QuickLog` (matches the JS-era flat-file pattern).
#[derive(Debug, Clone, Default)]
pub struct QuickLogManager {
    entries: Vec<QuickLog>,
}

impl QuickLogManager {
    /// Construct an empty manager.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Borrow the current entries.
    #[must_use]
    pub fn entries(&self) -> &[QuickLog] {
        &self.entries
    }

    /// Build the bulk save payload — the `Vec<QuickLog>` shape the
    /// Tauri-side `save_quick_logs(quickLogs)` wrapper expects. Bulk
    /// re-save per mutation matches the `SessionManager` precedent.
    #[must_use]
    pub fn save_payload(&self) -> Vec<QuickLog> {
        self.entries.clone()
    }

    /// Append a new quick-log entry. Generates a UUID v4 id, an
    /// ISO-8601 `created_at` timestamp, and the chrono
    /// `%a %b %d %Y` `date` field. Boundary validation lives at the
    /// Tauri command (FR-022) — the manager accepts whatever the
    /// modal layer hands it.
    pub fn add(&mut self, title: String, elapsed_minutes: u32, now_ms: i64, id: String) {
        let date = crate::engine::date_format::format_session_date(now_ms);
        let created_at = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(now_ms)
            .unwrap_or_default()
            .to_rfc3339();
        self.entries.push(QuickLog {
            id,
            title,
            elapsed_minutes,
            created_at,
            date,
        });
    }

    /// Replace the matching quick-log entry by `id`. Update-of-unknown-id
    /// is a no-op (mirrors `SessionManager::update_manual`).
    pub fn update_by_id(&mut self, updated: QuickLog) {
        if let Some(slot) = self.entries.iter_mut().find(|q| q.id == updated.id) {
            *slot = updated;
        }
    }

    /// Remove the entry with the matching `id`. Delete-of-unknown-id
    /// is a no-op.
    pub fn delete_by_id(&mut self, id: &str) {
        self.entries.retain(|q| q.id != id);
    }

    /// Project the list to the entries whose `date` field matches
    /// `date_str`. The date string is the chrono format `%a %b %d %Y`
    /// produced by `engine::date_format::format_session_date`.
    /// Unknown date returns an empty `Vec`.
    #[must_use]
    pub fn entries_for_date(&self, date_str: &str) -> Vec<&QuickLog> {
        self.entries.iter().filter(|q| q.date == date_str).collect()
    }

    /// Build a manager from a previously-loaded list (typically the
    /// result of `bridge::commands::load_quick_logs()`). Only
    /// `BridgeUnavailable` falls back to an empty cold-start list;
    /// other errors propagate so the caller avoids silent data-loss
    /// overwrites. Matches the `SessionManager::from_loaded_or_default`
    /// precedent.
    ///
    /// # Errors
    /// Returns `Err(e)` for any `BridgeError` variant other than
    /// `BridgeUnavailable`.
    pub fn from_loaded_or_default(
        loaded: Result<Vec<QuickLog>, BridgeError>,
    ) -> Result<Self, BridgeError> {
        match loaded {
            Ok(entries) => Ok(Self { entries }),
            Err(BridgeError::BridgeUnavailable) => Ok(Self::new()),
            Err(e) => Err(e),
        }
    }

    /// Async cold-start path: ask the bridge for the persisted
    /// quick-log entries. Returns `Ok` (empty list) only when the
    /// bridge is unavailable.
    ///
    /// # Errors
    /// Returns `Err(e)` for any bridge or IO error other than
    /// `BridgeUnavailable`.
    pub async fn load() -> Result<Self, BridgeError> {
        Self::from_loaded_or_default(commands::load_quick_logs().await)
    }

    /// Async save path: hand the current bulk payload to
    /// `bridge::commands::save_quick_logs`.
    ///
    /// # Errors
    /// Returns whatever `bridge::commands::save_quick_logs` returns —
    /// `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
    /// not present, or whichever variant the Tauri-side handler maps
    /// boundary-validation / IO failure to.
    pub async fn save(&self) -> Result<(), BridgeError> {
        commands::save_quick_logs(self.save_payload()).await
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

    /// T042: `add()` populates the in-memory list with derived
    /// `id` / `created_at` / `date` fields and the user-supplied
    /// title + elapsed-minutes.
    #[test]
    fn add_then_load_round_trips_entry() {
        let mut mgr = QuickLogManager::new();
        // 2024-01-01T00:00:00Z → chrono local-tz fallback "Mon Jan 01 2024".
        let day_ms: i64 = 1_704_067_200_000;
        mgr.add("Reply to PR".to_string(), 5, day_ms, "qid-1".to_string());

        assert_eq!(mgr.entries().len(), 1);
        let q = &mgr.entries()[0];
        assert_eq!(q.id, "qid-1");
        assert_eq!(q.title, "Reply to PR");
        assert_eq!(q.elapsed_minutes, 5);
        // date format matches `format_session_date` precedent.
        assert_eq!(
            q.date,
            crate::engine::date_format::format_session_date(day_ms)
        );
        // created_at is RFC-3339 (ISO-8601 superset).
        assert!(q.created_at.contains('T'));

        let payload = mgr.save_payload();
        assert_eq!(payload.len(), 1);
        assert_eq!(payload[0].id, "qid-1");
    }

    /// T042: `update_by_id()` replaces the matching entry in place;
    /// list length unchanged. Unknown id is a no-op.
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
        let q2 = mgr.entries().iter().find(|q| q.id == "qid-2").unwrap();
        assert_eq!(q2.title, "b");
        assert_eq!(q2.elapsed_minutes, 7);

        // Update-of-unknown-id is a no-op.
        let mut ghost = sample("qid-nope", "ghost", 1, "Fri May 15 2026");
        ghost.id = "qid-nope".to_string();
        mgr.update_by_id(ghost);
        assert_eq!(mgr.entries().len(), 2, "update-of-unknown-id no-op");
    }

    /// T042: `delete_by_id()` removes the matching entry only.
    /// Unknown id is a no-op.
    #[test]
    fn delete_removes_only_target() {
        let mut mgr = QuickLogManager::new();
        mgr.add("a".to_string(), 5, 1_704_067_200_000, "qid-1".to_string());
        mgr.add("b".to_string(), 7, 1_704_067_200_000, "qid-2".to_string());
        mgr.add("c".to_string(), 9, 1_704_067_200_000, "qid-3".to_string());

        mgr.delete_by_id("qid-2");

        assert_eq!(mgr.entries().len(), 2);
        assert!(mgr.entries().iter().all(|q| q.id != "qid-2"));

        // Delete-of-unknown-id no-op.
        mgr.delete_by_id("qid-nope");
        assert_eq!(mgr.entries().len(), 2);
    }

    /// T042: `entries_for_date()` filters by the `date` field.
    /// Unknown date returns empty.
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

    /// T042: in-memory bulk-re-save round-trip via `save_payload` →
    /// `from_loaded_or_default`. Mimics the bridge handoff without
    /// crossing the wire.
    #[test]
    fn load_then_save_round_trip() {
        let mut mgr = QuickLogManager::new();
        mgr.add("a".to_string(), 5, 1_704_067_200_000, "qid-1".to_string());
        mgr.add("b".to_string(), 7, 1_704_067_200_000, "qid-2".to_string());
        let payload = mgr.save_payload();
        assert_eq!(payload.len(), 2);

        let reloaded = QuickLogManager::from_loaded_or_default(Ok(payload)).unwrap();
        assert_eq!(reloaded.entries().len(), 2);
        assert_eq!(reloaded.entries()[0].id, "qid-1");
        assert_eq!(reloaded.entries()[1].id, "qid-2");

        // BridgeUnavailable cold-start → empty.
        let cold =
            QuickLogManager::from_loaded_or_default(Err(BridgeError::BridgeUnavailable)).unwrap();
        assert!(cold.entries().is_empty());

        // Other errors propagate.
        let propagated = QuickLogManager::from_loaded_or_default(Err(BridgeError::Internal {
            msg: "disk".to_string(),
        }));
        assert!(propagated.is_err());
    }
}
