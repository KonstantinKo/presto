// Engine — `TimerState` pomodoro state machine.
//
// Spec 001-leptos-migration §Phase 2 (T120-T146); ported from
// `src/core/pomodoro-timer.js`. Pure state machine — no `web-sys`,
// no DOM reads. All inputs (wall-clock time, activity signals,
// settings) are passed in via constructor / setters / `tick(now_ms)`.
//
// See `engine/mod.rs` for module-level Principle I rationale.

#[cfg(test)]
mod tests {
    use super::TimerState;
    use crate::bridge::timer_mode::TimerMode;
    use crate::engine::durations::Durations;

    /// T120: a freshly-constructed `TimerState` is in `Focus` mode with
    /// the focus duration's worth of time remaining and zero
    /// completed pomodoros. Mirrors `PomodoroTimer` constructor at
    /// `src/core/pomodoro-timer.js:13-17`.
    #[test]
    fn starts_in_focus_mode() {
        let state = TimerState::new(Durations::default());
        assert_eq!(state.current_mode(), TimerMode::Focus);
        assert_eq!(state.time_remaining_secs(), 25 * 60);
        assert_eq!(state.completed_pomodoros(), 0);
    }
}
