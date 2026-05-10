// Engine — `TimerState` pomodoro state machine.
//
// Spec 001-leptos-migration §Phase 2 (T120-T146); ported from
// `src/core/pomodoro-timer.js`. Pure state machine — no `web-sys`,
// no DOM reads. All inputs (wall-clock time, activity signals,
// settings) are passed in via constructor / setters / `tick(now_ms)`.
//
// See `engine/mod.rs` for module-level Principle I rationale.

use crate::bridge::timer_mode::TimerMode;
use crate::engine::activity_signal::ActivitySignal;
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
    /// Smart-pause kicked in after the configured inactivity
    /// threshold elapsed (i.e. the bridge layer fed an
    /// `ActivitySignal::Idle` while a focus session was running).
    /// Mirrors the `autoPauseTimer` body at
    /// `pomodoro-timer.js:524-562`.
    AutoPaused,
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
    /// Cumulative focus-work in the current session, in seconds.
    /// Mirrors `currentSessionElapsedTime` at
    /// `pomodoro-timer.js:37` plus the per-tick integration at
    /// line 745-749. Read by the persistence layer on completion
    /// to record the real session duration (used by undo /
    /// session-save flows). Reset by `reset()` and on a clean
    /// session start; preserved across pause/resume.
    current_session_elapsed_secs: u32,
    /// Whether the engine is configured to auto-pause on idle
    /// signals. Mirrors `smartPauseEnabled` at
    /// `pomodoro-timer.js:20`. Toggled at runtime via
    /// `set_smart_pause_enabled`.
    smart_pause_enabled: bool,
    /// Whether the engine is currently in the smart-pause
    /// suspended state. Mirrors `isAutoPaused` at
    /// `pomodoro-timer.js:21`. Distinct from a manual pause
    /// (which would set `is_paused`); the bridge / UI layer
    /// distinguishes the two so the resume affordance is correct
    /// (manual resume vs. activity-driven auto-resume).
    is_auto_paused: bool,
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
            current_session_elapsed_secs: 0,
            smart_pause_enabled: false,
            is_auto_paused: false,
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

    /// Cumulative wall-clock seconds spent in the current focus
    /// session. Mirrors `currentSessionElapsedTime` at
    /// `pomodoro-timer.js:37`. Reset by `reset()` and on a clean
    /// session start; preserved across pause/resume.
    #[must_use]
    pub const fn current_session_elapsed_secs(&self) -> u32 {
        self.current_session_elapsed_secs
    }

    /// Whether smart-pause is currently enabled. The bridge layer
    /// gates its activity-monitor subscription on this flag.
    #[must_use]
    pub const fn smart_pause_enabled(&self) -> bool {
        self.smart_pause_enabled
    }

    /// Toggles smart-pause. Disabling while currently auto-paused
    /// resumes the timer (mirrors `enableSmartPause(false)` at
    /// `pomodoro-timer.js:628-664`).
    pub const fn set_smart_pause_enabled(&mut self, enabled: bool) {
        self.smart_pause_enabled = enabled;
        if !enabled && self.is_auto_paused {
            self.is_auto_paused = false;
        }
    }

    /// Whether the engine is currently in the smart-pause
    /// suspended state. Distinct from a manual pause.
    #[must_use]
    pub const fn is_auto_paused(&self) -> bool {
        self.is_auto_paused
    }

    /// Consume an `ActivitySignal` from the bridge layer.
    ///
    /// Idle while running a focus session triggers auto-pause.
    /// Active while auto-paused triggers auto-resume (T134-T135).
    /// Returns the events fired by the transition (empty `Vec` if
    /// the signal didn't transition state).
    ///
    /// Mirrors `handleUserActivity` + `autoPauseTimer` +
    /// `resumeFromAutoPause` at `pomodoro-timer.js:440-626`.
    pub fn observe_activity(&mut self, signal: ActivitySignal) -> Vec<TimerEvent> {
        let mut events = Vec::new();
        match signal {
            ActivitySignal::Idle => {
                if self.smart_pause_enabled
                    && self.is_running
                    && !self.is_auto_paused
                    && self.current_mode == TimerMode::Focus
                {
                    self.is_auto_paused = true;
                    self.is_running = false;
                    events.push(TimerEvent::AutoPaused);
                }
            }
            ActivitySignal::Active => {
                // Auto-resume path lands in T135.
            }
        }
        events
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

        // Accumulator: integrate the wall-clock seconds drained by
        // this tick into the focus-session counter (focus mode
        // only; break-mode accumulation is meaningless for the
        // persistence layer). Mirrors the
        // `currentSessionElapsedTime += timeDiff` line at
        // `pomodoro-timer.js:745-749`.
        if self.current_mode == TimerMode::Focus {
            let drained = old_remaining.saturating_sub(new_remaining);
            if drained > 0 {
                let drained_u32 = u32::try_from(drained).unwrap_or(u32::MAX);
                self.current_session_elapsed_secs = self
                    .current_session_elapsed_secs
                    .saturating_add(drained_u32);
            }
        }

        // Zero-cross from positive to non-positive triggers the
        // mode's completion transition. Mirrors the
        // `oldTimeRemaining > 0 && timeRemaining <= 0` check at
        // `pomodoro-timer.js:777`.
        if old_remaining > 0 && new_remaining <= 0 {
            match self.current_mode {
                TimerMode::Focus => {
                    self.completed_pomodoros = self.completed_pomodoros.saturating_add(1);
                    events.push(TimerEvent::PomodoroCompleted {
                        completed_pomodoros: self.completed_pomodoros,
                    });
                    // Every fourth focus completion enters
                    // `LongBreak`; otherwise short `Break`. Mirrors
                    // `pomodoro-timer.js:1195-1199`.
                    self.current_mode = if self.completed_pomodoros.is_multiple_of(4) {
                        TimerMode::LongBreak
                    } else {
                        TimerMode::Break
                    };
                }
                TimerMode::Break | TimerMode::LongBreak => {
                    // Break-mode completion returns to focus. Mirrors
                    // `pomodoro-timer.js:1213` (`this.currentMode =
                    // "focus"`).
                    self.current_mode = TimerMode::Focus;
                }
            }
            self.time_remaining_secs = i64::from(self.durations.for_mode(self.current_mode));
            self.is_running = false;
            self.timer_start_ms = None;
            self.timer_duration_secs = None;
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
    use crate::engine::activity_signal::ActivitySignal;
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

    /// Run the engine through a full focus → break cycle, returning
    /// it to focus mode and starting again. Used by T126 to drive
    /// four focus completions in sequence.
    fn cycle_focus_then_break(state: &mut TimerState, clock: &MockClock) {
        let durations = Durations::default();
        // Drive the focus countdown to zero.
        state.start(clock).expect("start focus");
        clock.advance(i64::from(durations.focus) * 1000);
        state.tick(clock);
        // Drive the break countdown to zero.
        state.start(clock).expect("start break");
        clock.advance(i64::from(state.current_mode_duration_secs()) * 1000);
        state.tick(clock);
    }

    /// T132: with smart-pause enabled, observing an `Idle` activity
    /// signal during a running focus session auto-pauses the timer.
    /// Mirrors `autoPauseTimer` at `pomodoro-timer.js:524-562`:
    /// the JS-side path checks `smartPauseEnabled && isRunning &&
    /// !isPaused && !isAutoPaused && currentMode === "focus"` and
    /// then sets `isAutoPaused = true; isPaused = true` and stops
    /// the tick loop. The engine port mirrors that gate exactly,
    /// emitting `AutoPaused` so the bridge can update tray + UI.
    #[test]
    fn smart_pause_pauses_after_inactive_timeout() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default());
        state.set_smart_pause_enabled(true);
        state.start(&clock).expect("start");
        clock.advance(1_000);
        state.tick(&clock);

        let events = state.observe_activity(ActivitySignal::Idle);

        assert!(state.is_auto_paused());
        assert!(!state.is_running());
        assert!(
            events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::AutoPaused)),
            "expected AutoPaused in {events:?}"
        );
    }

    /// T128: drift compensation recovers after an OS-suspend gap.
    /// SC-005, AS-1.3. Mirrors `updateTimerWithAccuracy` at
    /// `pomodoro-timer.js:730-789`, which computes elapsed time
    /// from the wall-clock anchor `timerStartTime` rather than
    /// counting tick callbacks (background-throttling robustness),
    /// AND maintains a `currentSessionElapsedTime` accumulator
    /// (line 745-749) that's later used for saving the completed
    /// session record.
    ///
    /// Scenario: timer started, 1 second of regular ticks elapse,
    /// then the OS suspends the process for 90 seconds. On
    /// resumption a single `tick` fires; the engine must report
    /// 91 seconds elapsed (1 + 90), not 1 + 1 = 2, AND must
    /// accumulate the 91 seconds of focus work into
    /// `current_session_elapsed_secs` so the persistence layer
    /// records the real session duration.
    #[test]
    fn drift_compensation_recovers_90s_of_os_suspend() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default());
        state.start(&clock).expect("start");

        // 1 regular tick — 1 second elapsed.
        clock.advance(1_000);
        state.tick(&clock);
        assert_eq!(state.time_remaining_secs(), 25 * 60 - 1);
        assert_eq!(
            state.current_session_elapsed_secs(),
            1,
            "first tick should accumulate 1 second of focus work"
        );

        // OS suspends for 90 seconds. No ticks fire during the
        // suspension. On resumption a single tick fires.
        clock.advance(90_000);
        state.tick(&clock);

        // Wall-clock anchor: 91 seconds elapsed since start.
        assert_eq!(state.time_remaining_secs(), 25 * 60 - 91);
        // Accumulator: the 90 lost seconds count as focus work
        // because the user was meant to be focusing during the
        // suspend gap (per the JS source's continuous
        // accumulation at `pomodoro-timer.js:745-749`).
        assert_eq!(
            state.current_session_elapsed_secs(),
            91,
            "accumulator should track wall-clock work, not tick count"
        );
        assert_eq!(state.completed_pomodoros(), 0);
        assert!(state.is_running());
    }

    /// T126: every fourth focus completion enters `LongBreak` instead
    /// of `Break`. Mirrors the `completedPomodoros % 4 === 0`
    /// branch at `pomodoro-timer.js:1195-1199`. Time remaining
    /// resets to the configured long-break duration (20 min).
    #[test]
    fn long_break_after_4_focus_sessions() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default());

        // Three full cycles return us to focus each time.
        cycle_focus_then_break(&mut state, &clock);
        cycle_focus_then_break(&mut state, &clock);
        cycle_focus_then_break(&mut state, &clock);
        assert_eq!(state.completed_pomodoros(), 3);
        assert_eq!(state.current_mode(), TimerMode::Focus);

        // Fourth focus completion → LongBreak.
        state.start(&clock).expect("start fourth focus");
        clock.advance(25 * 60 * 1000);
        state.tick(&clock);

        assert_eq!(state.completed_pomodoros(), 4);
        assert_eq!(state.current_mode(), TimerMode::LongBreak);
        assert_eq!(state.time_remaining_secs(), 20 * 60);
    }
}
