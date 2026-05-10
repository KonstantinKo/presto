// Engine — `TimerState` pomodoro state machine.
//
// Spec 001-leptos-migration §Phase 2 (T120-T146); ported from
// `src/core/pomodoro-timer.js`. Pure state machine — no `web-sys`,
// no DOM reads. All inputs (wall-clock time, activity signals,
// settings) are passed in via constructor / setters / `tick(now_ms)`.
//
// See `engine/mod.rs` for module-level Principle I rationale.

use crate::bridge::timer_mode::TimerMode;
use crate::engine::durations::Durations;

/// Pomodoro state machine.
///
/// Mirrors the externally-visible behaviour of
/// `src/core/pomodoro-timer.js` line-for-line: starts in `Focus`
/// mode with the focus duration's worth of time remaining, idle
/// (not running, not paused). Subsequent commits in Phase 2 attach
/// `start` / `pause` / `resume` / `tick` / `skip` / `reset` per
/// behavioural tests T122-T143.
#[derive(Debug, Clone)]
pub struct TimerState {
    /// Configured per-mode duration set in seconds.
    durations: Durations,
    /// Current mode (`Focus` / `Break` / `LongBreak`).
    current_mode: TimerMode,
    /// Time remaining in the current mode, in seconds.
    time_remaining_secs: u32,
    /// Number of focus pomodoros completed since boot or last reset.
    completed_pomodoros: u32,
}

impl TimerState {
    /// Constructs a fresh state machine in idle / `Focus` mode with
    /// `durations.focus` seconds remaining and zero completed
    /// pomodoros. Mirrors `PomodoroTimer` constructor at
    /// `src/core/pomodoro-timer.js:13-17`.
    #[must_use]
    pub const fn new(durations: Durations) -> Self {
        let time_remaining_secs = durations.focus;
        Self {
            durations,
            current_mode: TimerMode::Focus,
            time_remaining_secs,
            completed_pomodoros: 0,
        }
    }

    /// Currently-active mode.
    #[must_use]
    pub const fn current_mode(&self) -> TimerMode {
        self.current_mode
    }

    /// Seconds remaining in the current mode.
    #[must_use]
    pub const fn time_remaining_secs(&self) -> u32 {
        self.time_remaining_secs
    }

    /// Count of focus pomodoros completed since construction or last
    /// reset.
    #[must_use]
    pub const fn completed_pomodoros(&self) -> u32 {
        self.completed_pomodoros
    }

    /// Configured duration in seconds for the current mode.
    ///
    /// Used by the upcoming `reset` / mode-switch transitions
    /// (T125, T141) to restore the time-remaining to a fresh
    /// per-mode value. Exposed publicly because `app.rs` and the
    /// tray-update plumbing both need to display the configured
    /// length when the engine is idle.
    #[must_use]
    pub const fn current_mode_duration_secs(&self) -> u32 {
        self.durations.for_mode(self.current_mode)
    }
}

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
