// Engine — `TimerState` pomodoro state machine.
//
// Spec 001-leptos-migration §Phase 2 (T120-T146); ported from
// `src/core/pomodoro-timer.js`. Pure state machine — no `web-sys`,
// no DOM reads. All inputs (wall-clock time, activity signals,
// settings) are passed in via constructor / setters / `tick(now_ms)`.
//
// See `engine/mod.rs` for module-level Principle I rationale.

use crate::bridge::timer_mode::TimerMode;
use crate::engine::clock::Clock;
use crate::engine::durations::Durations;

/// Internal events emitted by the state machine on transitions.
///
/// Distinct from the bridge events (E1-E10 in `bridge::events`):
/// those flow Tauri → Leptos. `TimerEvent` is the engine →
/// `app.rs` / managers signal — emitted by `tick()`, `start()`,
/// `pause()`, etc., and consumed by the persistence + UI layers
/// to drive session saves and tray-icon updates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimerEvent {
    /// A focus pomodoro just finished (countdown crossed zero or
    /// `skip()` was called during focus mode). Carries the
    /// pomodoros-completed count *after* the increment.
    PomodoroCompleted { completed_pomodoros: u32 },
}

/// Pomodoro state machine.
///
/// Mirrors the externally-visible behaviour of
/// `src/core/pomodoro-timer.js` line-for-line: starts in `Focus`
/// mode with the focus duration's worth of time remaining, idle
/// (not running, not paused). Subsequent commits in Phase 2 attach
/// `pause` / `resume` / `skip` / `reset` per behavioural tests
/// T122-T143.
#[derive(Debug, Clone)]
pub struct TimerState {
    /// Configured per-mode duration set in seconds.
    durations: Durations,
    /// Current mode (`Focus` / `Break` / `LongBreak`).
    current_mode: TimerMode,
    /// Time remaining in the current mode, in seconds. May go
    /// negative briefly inside `tick` before the
    /// completion-transition lands; the post-transition invariant
    /// is `>= 0`.
    time_remaining_secs: i64,
    /// Number of focus pomodoros completed since boot or last reset.
    completed_pomodoros: u32,
    /// `true` while the countdown is active. Mirrors `isRunning` in
    /// the JS source.
    is_running: bool,
    /// Wall-clock timestamp (ms since unix epoch) at which the
    /// current run-segment began. Mirrors `timerStartTime` at
    /// `pomodoro-timer.js:695`. Wall-clock-anchored elapsed
    /// computation is the drift-compensation primitive — see T128.
    timer_start_ms: Option<i64>,
    /// Time-remaining snapshot in seconds at the moment of `start`
    /// (or resume). Mirrors `timerDuration` at
    /// `pomodoro-timer.js:696`. `tick` computes
    /// `elapsed = (now - timer_start_ms) / 1000` and applies
    /// `time_remaining = timer_duration_secs - elapsed`.
    timer_duration_secs: Option<i64>,
}

impl TimerState {
    /// Constructs a fresh state machine in idle / `Focus` mode with
    /// `durations.focus` seconds remaining and zero completed
    /// pomodoros. Mirrors `PomodoroTimer` constructor at
    /// `src/core/pomodoro-timer.js:13-17`.
    #[must_use]
    pub fn new(durations: Durations) -> Self {
        let time_remaining_secs = i64::from(durations.focus);
        Self {
            durations,
            current_mode: TimerMode::Focus,
            time_remaining_secs,
            completed_pomodoros: 0,
            is_running: false,
            timer_start_ms: None,
            timer_duration_secs: None,
        }
    }

    /// Currently-active mode.
    #[must_use]
    pub const fn current_mode(&self) -> TimerMode {
        self.current_mode
    }

    /// Seconds remaining in the current mode. Always non-negative
    /// in steady state (never observed mid-`tick`).
    #[must_use]
    #[allow(
        clippy::cast_sign_loss,
        clippy::cast_possible_truncation,
        // Safe: `time_remaining_secs` is `>= 0` outside `tick`'s
        // internal arithmetic and bounded by the largest mode
        // duration (long break: 20 min = 1_200 < u32::MAX).
    )]
    pub const fn time_remaining_secs(&self) -> u32 {
        if self.time_remaining_secs < 0 {
            0
        } else {
            self.time_remaining_secs as u32
        }
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

    /// Whether the countdown is currently active.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.is_running
    }

    /// Begin (or resume) the countdown.
    ///
    /// Records the wall-clock anchor and the duration snapshot so
    /// subsequent `tick(now)` calls can compute elapsed time
    /// independent of host scheduler accuracy (drift compensation,
    /// per `pomodoro-timer.js:730-789`). No-op if already running.
    ///
    /// # Errors
    /// Currently infallible; returns `Result` so future
    /// preconditions (e.g. max-session cap, T136-T137) compose.
    pub fn start(&mut self, clock: &dyn Clock) -> Result<(), TimerError> {
        if self.is_running {
            return Ok(());
        }
        let now = clock.now_ms();
        self.is_running = true;
        self.timer_start_ms = Some(now);
        self.timer_duration_secs = Some(self.time_remaining_secs);
        Ok(())
    }

    /// Advance the state machine to wall-clock `now`, emitting any
    /// transition events that fire as a result.
    ///
    /// The tick computes elapsed time as
    /// `floor((now - timer_start_ms) / 1000)` (mirrors the
    /// `Math.floor` arithmetic at `pomodoro-timer.js:735`). When
    /// the countdown crosses zero, a focus mode emits
    /// `PomodoroCompleted` and increments `completed_pomodoros`;
    /// later commits attach the `Focus → Break` mode transition
    /// (T125), the long-break-every-fourth (T127), the
    /// drift-compensation arithmetic (T129) and break-mode
    /// completion semantics.
    pub fn tick(&mut self, clock: &dyn Clock) -> Vec<TimerEvent> {
        let mut events = Vec::new();
        if !self.is_running {
            return events;
        }
        let (Some(start_ms), Some(duration_secs)) =
            (self.timer_start_ms, self.timer_duration_secs)
        else {
            return events;
        };

        let now = clock.now_ms();
        let elapsed_ms = now.saturating_sub(start_ms);
        let elapsed_secs = elapsed_ms.div_euclid(1000);
        let new_remaining = duration_secs - elapsed_secs;
        let old_remaining = self.time_remaining_secs;
        self.time_remaining_secs = new_remaining;

        // Zero-cross from positive to non-positive triggers the
        // mode's completion transition. Mirrors the
        // `oldTimeRemaining > 0 && timeRemaining <= 0` check at
        // `pomodoro-timer.js:777`.
        if old_remaining > 0 && new_remaining <= 0 && self.current_mode == TimerMode::Focus {
            self.is_running = false;
            self.timer_start_ms = None;
            self.timer_duration_secs = None;
            self.completed_pomodoros = self.completed_pomodoros.saturating_add(1);
            events.push(TimerEvent::PomodoroCompleted {
                completed_pomodoros: self.completed_pomodoros,
            });

            // After a focus session, transition into the appropriate
            // break mode. The "every fourth" long-break rule lands in
            // T127; this commit covers only the short-break branch.
            // Mirrors `pomodoro-timer.js:1194-1205` (the
            // `completedPomodoros % 4 === 0` branch is filled in by
            // T127's GREEN commit).
            self.current_mode = TimerMode::Break;
            self.time_remaining_secs = i64::from(self.durations.for_mode(self.current_mode));
        }

        events
    }
}

/// Error variants returned by the public state-machine methods.
///
/// Currently empty; a placeholder for the max-session-cap (T136-T137)
/// and disallowed-transition errors that later commits attach.
/// The variant set is intentionally extensible — keeping the type
/// non-exhaustive so consumers must `match` with a fallback arm.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TimerError {}

#[cfg(test)]
mod tests {
    use super::TimerState;
    use crate::bridge::timer_mode::TimerMode;
    use crate::engine::clock::Clock;
    use crate::engine::durations::Durations;
    use core::cell::Cell;

    /// Deterministic test clock. `set(t)` jumps to absolute `t`,
    /// `advance(ms)` steps forward. Drift-compensation tests
    /// (T128) jump non-monotonically to simulate OS suspend.
    struct MockClock {
        now: Cell<i64>,
    }

    impl MockClock {
        fn new(start_ms: i64) -> Self {
            Self {
                now: Cell::new(start_ms),
            }
        }

        fn advance(&self, delta_ms: i64) {
            self.now.set(self.now.get() + delta_ms);
        }
    }

    impl Clock for MockClock {
        fn now_ms(&self) -> i64 {
            self.now.get()
        }
    }

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

    /// T122: after `start()` then a `tick()` 25 minutes later, the
    /// engine emits `PomodoroCompleted` and increments
    /// `completed_pomodoros`. Mirrors `completeSession` in
    /// `src/core/pomodoro-timer.js:1152` plus the
    /// `updateTimerWithAccuracy` trigger at line 777.
    #[test]
    fn focus_completes_after_25min_emits_pomodoro_completed() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default());
        state.start(&clock).expect("start");

        // Advance the wall clock 25 minutes.
        clock.advance(25 * 60 * 1000);
        let events = state.tick(&clock);

        assert!(
            events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::PomodoroCompleted { .. })),
            "expected PomodoroCompleted in {events:?}"
        );
        assert_eq!(state.completed_pomodoros(), 1);
    }

    /// T124: after a focus session completes (the first one), the
    /// next mode is `Break` (not `LongBreak` — that's every fourth,
    /// covered in T126). Time remaining resets to the configured
    /// short-break duration (5 min). Mirrors the
    /// `completedPomodoros % 4` branch at
    /// `src/core/pomodoro-timer.js:1195-1199`.
    #[test]
    fn break_after_focus() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default());
        state.start(&clock).expect("start");
        clock.advance(25 * 60 * 1000);
        state.tick(&clock);

        assert_eq!(state.current_mode(), TimerMode::Break);
        assert_eq!(state.time_remaining_secs(), 5 * 60);
    }
}
