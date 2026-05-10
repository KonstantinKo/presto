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
use crate::bridge::error::BridgeError;
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
        if let Some(slot) = self
            .manual_sessions
            .iter_mut()
            .find(|s| s.id == updated.id)
        {
            *slot = updated;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SessionManager;
    use crate::bridge::session_type::SessionType;
    use crate::bridge::types::ManualSession;
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
        assert_eq!(
            m1_after.notes.as_deref(),
            Some("revised"),
            "notes replaced",
        );

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
}
