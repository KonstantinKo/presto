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

use crate::bridge::types::ManualSession;

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
}
