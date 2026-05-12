// `SessionManager` — the Rust port of `src/managers/session-manager.js`.
//
// Spec 001-leptos-migration §Phase 3b (T167-T174). Owns the user's
// `ManualSession` backfill list and routes manual entries through
// `engine::timer::TimerState::record_manual_session` before the bulk
// re-save lands on disk via `bridge::commands::save_manual_sessions`
// (Principle I — manual entries flow through the same engine
// accumulators as live sessions). Per Principle VI, the async
// wrappers reach the Tauri side only through `bridge::commands` —
// the manager never touches `__TAURI_INTERNALS__` directly.
//
// Lint allowance: `clippy::future_not_send` is allowed at the module
// level for the same reason as on `bridge::commands`,
// `managers::settings`, and `managers::tag` — every async path here
// transitively awaits a `JsFuture` from `bridge::commands`, and
// `JsValue` (and everything built on it) is `!Send` by construction
// on `wasm32-unknown-unknown`. The runtime is single-threaded.
#![allow(clippy::future_not_send)]

use crate::bridge::commands;
use crate::bridge::types::BridgeError;
use crate::bridge::types::ManualSession;
use crate::engine::timer::{TimerEvent, TimerState};

/// Wrapper over the user's manual-session backfill list. Phase 3b
/// wires up the state machine; per-entry CRUD lands in
/// T168/T170/T172/T174.
#[derive(Debug, Clone, Default)]
pub struct SessionManager {
    /// Current authoritative manual-session list. Populated either
    /// by `load()` (cold-start path) or `from_loaded(...)` (test
    /// path / hand-fed list). `Default::default()` produces an
    /// empty `Vec`, matching the JS-side cold-start "no manual
    /// sessions file yet" convention.
    manual_sessions: Vec<ManualSession>,
}

impl SessionManager {
    /// Construct an empty manager.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            manual_sessions: Vec::new(),
        }
    }

    /// Borrow the current manual-session list.
    #[must_use]
    pub fn manual_sessions(&self) -> &[ManualSession] {
        &self.manual_sessions
    }

    /// Append a manual-session backfill, routing the entry through
    /// the engine's `record_manual_session(duration_secs)`
    /// accumulator path (Principle I — manual entries flow through
    /// the same engine accumulators as live sessions). The
    /// `ManualSession::duration` field is in minutes per
    /// data-model.md §`ManualSession`; the engine consumes
    /// seconds, so the conversion lands here.
    ///
    /// Returns the engine's emitted `TimerEvent`s so the caller
    /// (Phase 4 components) can fan them out to listeners (e.g.
    /// the tray icon's pomodoros-completed display). The bulk
    /// re-save through `save_manual` is the caller's
    /// responsibility — the manager updates state synchronously
    /// and the on-disk file catches up after the async hop.
    ///
    /// Spec 001-leptos-migration §Phase 3b T168.
    pub fn create_manual(
        &mut self,
        engine: &mut TimerState,
        manual: ManualSession,
    ) -> Vec<TimerEvent> {
        let duration_secs = manual.duration.saturating_mul(60);
        let events = engine.record_manual_session(duration_secs);
        self.manual_sessions.push(manual);
        events
    }

    /// Build the bulk save payload — the `Vec<ManualSession>` shape
    /// the Tauri-side `save_manual_sessions(sessions)` wrapper
    /// expects. Mirrors the JS-side `saveSessionsToStorage` flow at
    /// `src/managers/session-manager.js:54-78` where the entire
    /// on-disk file is rewritten on every mutation; the JS-era
    /// in-memory shape is a date-keyed map but the wire shape is a
    /// flat `Vec` (the JS code flattens with
    /// `Object.keys(...).forEach(date => ...push({...session,
    /// date}))`). Our in-memory shape is already the flat `Vec`
    /// because each `ManualSession` carries its own `date` field.
    ///
    /// Pure helper — used by tests and by the async `save_manual`
    /// wrapper for diagnostics.
    #[must_use]
    pub fn save_payload(&self) -> Vec<ManualSession> {
        self.manual_sessions.clone()
    }

    /// Async save path: hand the current bulk payload to
    /// `bridge::commands::save_manual_sessions` (per Principle VI —
    /// managers reach the Tauri side only through the typed bridge
    /// wrapper). Mirrors the JS-side `await invoke(
    /// "save_manual_sessions", { sessions })` at
    /// `src/managers/session-manager.js:67-69`.
    ///
    /// # Errors
    /// Returns whatever `bridge::commands::save_manual_sessions`
    /// returns — `BridgeError::BridgeUnavailable` when the Tauri
    /// JS bridge is not present, or whichever variant the Tauri-
    /// side handler maps its filesystem failure to.
    pub async fn save_manual(&self) -> Result<(), BridgeError> {
        commands::save_manual_sessions(self.save_payload()).await
    }

    /// Replace the matching manual-session entry by `id`. Pure
    /// mutation — update-of-unknown-id is a no-op (mirrors the
    /// JS-side `findIndex(...) !== -1` guard at
    /// `src/managers/session-manager.js:357-364`). Distinct from
    /// `create_manual`: the engine's accumulators are NOT pumped on
    /// update (the JS-era flow doesn't re-pump them either; only
    /// the persisted record changes). Spec
    /// 001-leptos-migration §Phase 3b T170.
    pub fn update_manual(&mut self, updated: ManualSession) {
        if let Some(slot) = self.manual_sessions.iter_mut().find(|s| s.id == updated.id) {
            *slot = updated;
        }
    }

    /// Remove the manual-session entry with the matching `id` from
    /// the in-memory list. Pure mutation — delete-of-unknown-id is
    /// a no-op (`Vec::retain` filters out only matches). The
    /// durable half of the flow is `save_manual` (T168 GREEN): the
    /// Tauri side has no per-entry delete command, so the
    /// wire-level write is a full
    /// `save_manual_sessions(remaining)` call — this is exactly
    /// what tasks.md T172 calls "bulk re-save with the entry
    /// omitted, matching the deleted `delete_manual_session` JS
    /// path".
    ///
    /// Engine accumulators are NOT decremented on delete: the
    /// historical pomodoros / focus-time accumulators are run-wide
    /// totals that don't shrink on retroactive edits. The JS-era
    /// `deleteCurrentSession` flow at
    /// `src/managers/session-manager.js:375-411` also doesn't
    /// touch the engine.
    ///
    /// Spec 001-leptos-migration §Phase 3b T172.
    pub fn delete_manual(&mut self, id: &str) {
        self.manual_sessions.retain(|s| s.id != id);
    }

    /// Project the manual-session list to the entries whose `date`
    /// field matches `date_str`. The date string is the chrono
    /// format `%a %b %d %Y` produced by
    /// `engine::date_format::format_session_date(timestamp_ms)` —
    /// the same format pinned in Phase 2 against JS
    /// `Date.prototype.toDateString()` parity (data-model.md
    /// §`Session.date`). Mirrors the JS-side `getSessionsForDate`
    /// flow at `src/managers/session-manager.js:413-417`.
    ///
    /// Returns borrowed references — callers that need owned values
    /// can `.cloned().collect()`. Unknown date returns an empty
    /// `Vec` (matches the JS-era `this.sessions[date] || []` shape
    /// at line 416).
    ///
    /// Spec 001-leptos-migration §Phase 3b T174.
    #[must_use]
    pub fn list_by_date(&self, date_str: &str) -> Vec<&ManualSession> {
        self.manual_sessions
            .iter()
            .filter(|s| s.date == date_str)
            .collect()
    }

    /// Build a manager from the result of
    /// `bridge::commands::load_manual_sessions()` (or any equivalent
    /// loader). Only `BridgeUnavailable` (dev/browser context where
    /// no Tauri runtime and therefore no file exists) falls back to
    /// an empty cold-start list; all other errors are propagated so
    /// the caller can avoid overwriting an unreadable but existing
    /// file with an empty list.
    ///
    /// # Errors
    /// Returns `Err(e)` for any `BridgeError` variant other than
    /// `BridgeUnavailable`.
    pub fn from_loaded_or_default(
        loaded: Result<Vec<ManualSession>, BridgeError>,
    ) -> Result<Self, BridgeError> {
        match loaded {
            Ok(sessions) => Ok(Self {
                manual_sessions: sessions,
            }),
            Err(BridgeError::BridgeUnavailable) => Ok(Self::new()),
            Err(e) => Err(e),
        }
    }

    /// Async cold-start path: ask the bridge for the persisted
    /// manual sessions. Returns `Ok` (empty list) only when the
    /// bridge is unavailable (dev/browser context); propagates all
    /// other errors so callers can avoid a silent data-loss
    /// overwrite. Mirrors the JS-side `loadSessionsFromStorage`
    /// flow at `src/managers/session-manager.js:25-52`, minus the
    /// localStorage fallback (Phase 1E
    /// `import_legacy_manual_sessions` migrated those records to
    /// the Rust-side store).
    ///
    /// # Errors
    /// Returns `Err(e)` for any bridge or I/O error other than
    /// `BridgeUnavailable`.
    pub async fn load() -> Result<Self, BridgeError> {
        Self::from_loaded_or_default(commands::load_manual_sessions().await)
    }
}

#[cfg(test)]
mod tests {
    use super::SessionManager;
    use crate::bridge::types::ManualSession;
    use crate::bridge::types::SessionType;
    use crate::engine::durations::Durations;
    use crate::engine::timer::{TimerEvent, TimerState};

    fn sample_manual(id: &str, duration_min: u32, date: &str) -> ManualSession {
        ManualSession {
            id: id.to_string(),
            session_type: SessionType::Focus,
            duration: duration_min,
            start_time: "09:00".to_string(),
            end_time: "09:25".to_string(),
            notes: None,
            created_at: "2026-05-10T09:00:00Z".to_string(),
            date: date.to_string(),
            tags: None,
        }
    }

    /// T167 [RED]: `create_manual(engine, manual)` MUST route the
    /// manual entry through the engine's
    /// `record_manual_session(duration_secs)` accumulator path
    /// (Principle I — manual entries flow through the same engine
    /// accumulators as live sessions, never bypass the engine
    /// straight to disk). The post-call `save_payload()` returns
    /// the bulk `Vec<ManualSession>` shape the bridge wrapper
    /// `save_manual_sessions` expects.
    ///
    /// Done-signal: this test currently fails because
    /// `SessionManager::create_manual` and `save_payload` do not
    /// yet exist. T168 GREEN attaches both alongside the async
    /// `save_manual` wrapper that hands the list to
    /// `bridge::commands::save_manual_sessions`.
    #[test]
    fn manual_session_create_round_trips_via_bridge() {
        let mut mgr = SessionManager::new();
        let mut engine = TimerState::new(Durations::default());

        let manual = sample_manual("m-1", 25, "Sat May 10 2026");
        let events = mgr.create_manual(&mut engine, manual.clone());

        // Engine accumulators reflect the manual entry.
        assert_eq!(
            engine.completed_pomodoros(),
            1,
            "engine.completed_pomodoros must increment per Principle I",
        );
        assert_eq!(
            engine.total_focus_secs(),
            25 * 60,
            "engine.total_focus_secs must integrate the manual duration in seconds",
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                TimerEvent::ManualSessionRecorded { duration_secs: 1500 }
            )),
            "engine must emit ManualSessionRecorded {{ duration_secs: 1500 }} (25 min); got {events:?}",
        );

        // Manager state reflects the manual entry.
        assert_eq!(
            mgr.manual_sessions().len(),
            1,
            "create_manual must append the entry to the manual-sessions list",
        );
        assert_eq!(mgr.manual_sessions()[0].id, "m-1");

        // The bulk save payload is the full list — the Tauri side's
        // `save_manual_sessions(sessions)` wrapper rewrites the
        // entire on-disk file (matches the JS-side
        // `saveSessionsToStorage` flow at `session-manager.js:54-78`).
        let payload = mgr.save_payload();
        assert_eq!(payload.len(), 1, "payload mirrors manager state");
        assert_eq!(payload[0].id, manual.id);
    }

    /// T169 [RED]: `update_manual(updated)` replaces the matching
    /// in-memory entry by `id` (mirrors the JS-side
    /// `updateSession` flow at
    /// `src/managers/session-manager.js:354-373` where the
    /// list-by-date is mutated in place at the matching index).
    /// Update-of-unknown-id is a no-op (no entry added; the JS-era
    /// `findIndex` returns `-1` and the splice is skipped).
    ///
    /// Distinct from `create_manual`: an update does NOT pump the
    /// engine accumulators — those reflect the original entry, and
    /// the JS-era flow at line 354 doesn't touch the timer engine
    /// either. Only the persisted record changes.
    ///
    /// Done-signal: this test currently fails because
    /// `SessionManager::update_manual` does not yet exist.
    /// T170 GREEN attaches the implementation.
    #[test]
    fn manual_session_update_replaces_by_id() {
        let mut mgr = SessionManager::new();
        let mut engine = TimerState::new(Durations::default());

        let m1 = sample_manual("m-1", 25, "Sat May 10 2026");
        let m2 = sample_manual("m-2", 50, "Sat May 10 2026");
        let _ = mgr.create_manual(&mut engine, m1);
        let _ = mgr.create_manual(&mut engine, m2);
        assert_eq!(mgr.manual_sessions().len(), 2);

        let pomodoros_before_update = engine.completed_pomodoros();
        let total_focus_before_update = engine.total_focus_secs();

        // Replace m-1 with a longer-duration entry.
        let mut updated = sample_manual("m-1", 40, "Sat May 10 2026");
        updated.notes = Some("revised".to_string());
        mgr.update_manual(updated);

        assert_eq!(
            mgr.manual_sessions().len(),
            2,
            "update must not change the list length",
        );

        let m1_after = mgr
            .manual_sessions()
            .iter()
            .find(|s| s.id == "m-1")
            .expect("m-1 still in the list");
        assert_eq!(m1_after.duration, 40, "duration replaced");
        assert_eq!(m1_after.notes.as_deref(), Some("revised"), "notes replaced");

        // Engine accumulators are unaffected by an update — only the
        // persisted record changes (mirrors the JS-era flow which
        // also doesn't re-pump the engine on update).
        assert_eq!(
            engine.completed_pomodoros(),
            pomodoros_before_update,
            "update must NOT pump engine.completed_pomodoros",
        );
        assert_eq!(
            engine.total_focus_secs(),
            total_focus_before_update,
            "update must NOT pump engine.total_focus_secs",
        );

        // Update-of-unknown-id is a no-op.
        let len_before_noop = mgr.manual_sessions().len();
        mgr.update_manual(sample_manual("m-nope", 99, "Sat May 10 2026"));
        assert_eq!(
            mgr.manual_sessions().len(),
            len_before_noop,
            "update of unknown id is a no-op (list unchanged)",
        );
    }

    /// T171 [RED]: `delete_manual(id)` removes the matching entry
    /// by `id` and the next `save_payload()` returns the bulk
    /// `Vec<ManualSession>` shape with the entry omitted.
    ///
    /// Per tasks.md T172, the on-disk-update path is "bulk re-save
    /// with the entry omitted, matching the deleted
    /// `delete_manual_session` JS path" — the Tauri side has no
    /// per-entry delete command; the wire-level write is a full
    /// `save_manual_sessions(remaining)` call. The manager's
    /// `delete_manual` mutation is the local half of that flow;
    /// `save_manual` is the durable half (already implemented in
    /// T168 GREEN; reused here).
    ///
    /// Engine accumulators are NOT decremented on delete — the JS-
    /// era flow at `deleteCurrentSession` doesn't touch the engine
    /// either, and the historical pomodoros / focus-time
    /// accumulators are run-wide totals that don't shrink on
    /// retroactive edits.
    ///
    /// Done-signal: this test currently fails because
    /// `SessionManager::delete_manual` does not yet exist.
    /// T172 GREEN attaches the implementation.
    #[test]
    fn manual_session_delete_removes_by_id() {
        let mut mgr = SessionManager::new();
        let mut engine = TimerState::new(Durations::default());

        let m1 = sample_manual("m-1", 25, "Sat May 10 2026");
        let m2 = sample_manual("m-2", 50, "Sat May 10 2026");
        let m3 = sample_manual("m-3", 30, "Sun May 11 2026");
        let _ = mgr.create_manual(&mut engine, m1);
        let _ = mgr.create_manual(&mut engine, m2);
        let _ = mgr.create_manual(&mut engine, m3);
        assert_eq!(mgr.manual_sessions().len(), 3);

        let pomodoros_before_delete = engine.completed_pomodoros();
        let total_focus_before_delete = engine.total_focus_secs();

        mgr.delete_manual("m-2");

        assert_eq!(mgr.manual_sessions().len(), 2, "one entry was removed");
        assert!(
            mgr.manual_sessions().iter().all(|s| s.id != "m-2"),
            "m-2 must not appear in the surviving list",
        );

        // The bulk save payload reflects the post-delete list — this
        // IS the wire-level write the JS-era `delete_manual_session`
        // rebuilds and re-saves at `session-manager.js:380-389`.
        let payload = mgr.save_payload();
        assert_eq!(
            payload.len(),
            2,
            "bulk save payload omits the deleted entry"
        );
        assert!(
            payload.iter().all(|s| s.id != "m-2"),
            "bulk save payload contains no record of m-2",
        );

        // Engine accumulators are unaffected — historical totals
        // don't shrink on retroactive deletes (matches the JS-era
        // flow which also doesn't decrement engine state).
        assert_eq!(
            engine.completed_pomodoros(),
            pomodoros_before_delete,
            "delete must NOT decrement engine.completed_pomodoros",
        );
        assert_eq!(
            engine.total_focus_secs(),
            total_focus_before_delete,
            "delete must NOT decrement engine.total_focus_secs",
        );

        // Delete-of-unknown-id is a no-op.
        let len_before_noop = mgr.manual_sessions().len();
        mgr.delete_manual("m-nope");
        assert_eq!(
            mgr.manual_sessions().len(),
            len_before_noop,
            "delete of unknown id is a no-op",
        );
    }

    /// T173 [RED]: `list_by_date(date_str)` returns only the manual
    /// sessions whose `date` field matches `date_str`. The date
    /// string is the chrono format `%a %b %d %Y` produced by
    /// `engine::date_format::format_session_date(timestamp_ms)` —
    /// the same format pinned in Phase 2 against JS
    /// `Date.prototype.toDateString()` parity (data-model.md
    /// §`Session.date`). Mirrors the JS-side `getSessionsForDate`
    /// flow at `src/managers/session-manager.js:413-417` where
    /// `this.sessions[date.toDateString()]` indexes the date-keyed
    /// in-memory map.
    ///
    /// Date strings are compared exactly — there's no timezone
    /// projection or mid-day rollover here, matching the JS-era
    /// flow which also key-equals on the `toDateString()` output.
    ///
    /// Done-signal: this test currently fails because
    /// `SessionManager::list_by_date` does not yet exist. T174
    /// GREEN attaches it.
    #[test]
    fn list_by_date_groups_correctly() {
        use crate::engine::date_format::format_session_date;

        let mut mgr = SessionManager::new();
        let mut engine = TimerState::new(Durations::default());

        // Three sessions across two distinct dates.
        // 2024-01-01 UTC → "Mon Jan 01 2024".
        // 2024-01-02 UTC → "Tue Jan 02 2024".
        let day_one_ms: i64 = 1_704_067_200_000; // 2024-01-01T00:00:00Z
        let day_two_ms: i64 = day_one_ms + 86_400_000;
        let day_one = format_session_date(day_one_ms);
        let day_two = format_session_date(day_two_ms);
        assert_ne!(day_one, day_two, "test fixture: two distinct date keys");

        let m1 = sample_manual("m-1", 25, &day_one);
        let m2 = sample_manual("m-2", 50, &day_one);
        let m3 = sample_manual("m-3", 30, &day_two);
        let _ = mgr.create_manual(&mut engine, m1);
        let _ = mgr.create_manual(&mut engine, m2);
        let _ = mgr.create_manual(&mut engine, m3);

        let day_one_sessions = mgr.list_by_date(&day_one);
        let day_two_sessions = mgr.list_by_date(&day_two);

        assert_eq!(
            day_one_sessions.len(),
            2,
            "two sessions on day_one ({day_one})",
        );
        assert!(
            day_one_sessions.iter().all(|s| s.date == day_one),
            "every session in the day_one bucket carries the day_one date",
        );
        assert!(
            day_one_sessions.iter().any(|s| s.id == "m-1")
                && day_one_sessions.iter().any(|s| s.id == "m-2"),
            "day_one bucket contains m-1 and m-2",
        );

        assert_eq!(
            day_two_sessions.len(),
            1,
            "one session on day_two ({day_two})",
        );
        assert_eq!(day_two_sessions[0].id, "m-3");

        // Unknown date returns an empty list (matches the JS-era
        // `this.sessions[date] || []` shape at line 416).
        let unknown_day = format_session_date(day_two_ms + 86_400_000);
        let none = mgr.list_by_date(&unknown_day);
        assert!(none.is_empty(), "unknown date returns an empty list");
    }

    #[test]
    fn from_loaded_or_default_only_swallows_bridge_unavailable() {
        use crate::bridge::types::BridgeError;

        // BridgeUnavailable (dev context) → cold-start empty list
        let result = SessionManager::from_loaded_or_default(Err(BridgeError::BridgeUnavailable));
        assert!(result.is_ok());
        assert!(result.unwrap().manual_sessions().is_empty());

        // Internal error (e.g. corrupt file) → propagated
        let err = BridgeError::Internal {
            msg: "disk error".to_string(),
        };
        let result = SessionManager::from_loaded_or_default(Err(err));
        assert!(result.is_err());

        // Ok with sessions → passes through
        let sessions = vec![sample_manual("m-1", 25, "Sat May 10 2026")];
        let result = SessionManager::from_loaded_or_default(Ok(sessions));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().manual_sessions().len(), 1);
    }
}
