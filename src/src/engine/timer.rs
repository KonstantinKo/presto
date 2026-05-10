// Engine — `TimerState` pomodoro state machine.
//
// Spec 001-leptos-migration §Phase 2 (T120-T146); ported from
// `src/core/pomodoro-timer.js`. Pure state machine — no DOM-
// binding crate imports, no DOM reads. All inputs (wall-clock
// time, activity signals, settings) are passed in via constructor
// / setters / `tick(now_ms)`.
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
    /// Smart-pause auto-resumed — user activity observed while
    /// auto-paused. Mirrors `resumeFromAutoPause` at
    /// `pomodoro-timer.js:564-626`. The wall-clock anchor is
    /// re-recorded on emit so the suspend gap is not charged
    /// against the session.
    AutoResumed,
    /// A manual session backfill was recorded through the engine.
    /// Carries the recorded duration in seconds. Per Principle I,
    /// manual entries route through the same engine path as live
    /// sessions so the pomodoros + focus-time accumulators have a
    /// single source of truth.
    ManualSessionRecorded { duration_secs: u32 },
    /// `skip()` advanced the engine to the next mode. Carries the
    /// mode that was being skipped (so the persistence layer can
    /// gate "save this if focus > 1 minute" per
    /// `pomodoro-timer.js:1088-1090`). Distinct from
    /// `PomodoroCompleted` so consumers can disambiguate between
    /// a natural completion (save unconditionally) and a skip
    /// (conditional save).
    SessionSkipped {
        skipped_mode: TimerMode,
        elapsed_secs: u32,
    },
    /// Manual pause emitted by `pause()`. Distinct from
    /// `AutoPaused` so the bridge layer can disambiguate the
    /// pause source (manual button click vs. smart-pause / idle
    /// detection) for tray + UI affordances. Mirrors `pauseTimer`
    /// at `pomodoro-timer.js:790-822`.
    SessionPaused,
    /// Manual resume emitted by `resume()`. Distinct from
    /// `AutoResumed` (which fires only when smart-pause unwinds
    /// on observed activity). Mirrors `resumeTimer` at
    /// `pomodoro-timer.js:824-878`.
    SessionResumed,
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
// `clippy::struct_excessive_bools`: the JS-era state machine
// expresses four mutually-distinguishable boolean signals
// (`isRunning`, `isPaused`, `isAutoPaused`, `smartPauseEnabled`)
// at `pomodoro-timer.js:18-22`. Folding them into a `State` enum
// would conflate manual-pause + smart-pause (which the bridge
// layer renders with different tray icons) and break parity with
// the JS source. Keeping the bool fields preserves the 1:1
// behavioural-port mapping that Principle I demands.
#[allow(clippy::struct_excessive_bools)]
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
    /// Whether the engine is currently in a manual-pause state
    /// (user clicked the pause button). Mirrors `isPaused` at
    /// `pomodoro-timer.js:22`. Mutually exclusive with
    /// `is_running` — `pause()` flips running off, `resume()`
    /// flips it back on. Distinct from `is_auto_paused`: smart-
    /// pause and manual pause have different resume affordances
    /// (activity-driven vs. button-driven), and the bridge layer
    /// renders different tray icons for each.
    is_paused: bool,
    /// Cap on the number of focus pomodoros allowed in this run.
    /// Once `completed_pomodoros == total_sessions`, further
    /// `start()` calls return `MaxSessionCapReached`. Mirrors
    /// `totalSessions` at `pomodoro-timer.js:31` (default 10).
    total_sessions: u32,
    /// Cumulative focus time across the run, in seconds. Mirrors
    /// `totalFocusTime` at `pomodoro-timer.js:32`. Both completed
    /// live sessions and manual backfills (per Principle I)
    /// integrate into this accumulator.
    total_focus_secs: u32,
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
            is_paused: false,
            total_sessions: 10,
            total_focus_secs: 0,
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

    /// Whether the engine is currently in a manual-pause state.
    /// Distinct from smart-pause (`is_auto_paused`); the UI / tray
    /// layer reads both signals to render the correct resume
    /// affordance.
    #[must_use]
    pub const fn is_paused(&self) -> bool {
        self.is_paused
    }

    /// Configured max-session cap. Mirrors `totalSessions` at
    /// `pomodoro-timer.js:31`.
    #[must_use]
    pub const fn total_sessions(&self) -> u32 {
        self.total_sessions
    }

    /// Updates the max-session cap. Settings load applies this at
    /// boot; the UI's "more sessions" affordance can also bump it
    /// at runtime.
    pub const fn set_total_sessions(&mut self, total: u32) {
        self.total_sessions = total;
    }

    /// Cumulative focus time in seconds across both live and
    /// manual sessions. Mirrors `totalFocusTime` at
    /// `pomodoro-timer.js:32`.
    #[must_use]
    pub const fn total_focus_secs(&self) -> u32 {
        self.total_focus_secs
    }

    /// Skip the current mode and advance to the next.
    ///
    /// Mirrors `skipSession` at `pomodoro-timer.js:974-1150` (the
    /// "normal skip" branch — overtime / continuous-session
    /// handling is settings-driven and lives in the manager
    /// layer). Behaviour:
    ///
    /// - Focus skip: `completed_pomodoros++`,
    ///   `total_focus_secs += current_session_elapsed_secs`,
    ///   transition to `Break` (or `LongBreak` every fourth).
    ///   Emits `SessionSkipped { skipped_mode: Focus,
    ///   elapsed_secs }`.
    /// - `Break` / `LongBreak` skip: transition back to `Focus`.
    ///   Emits `SessionSkipped { skipped_mode: Break|LongBreak,
    ///   elapsed_secs: 0 }` (break-mode elapsed time isn't
    ///   tracked by the engine).
    ///
    /// `is_running` becomes false; the wall-clock anchor is
    /// cleared. Does NOT emit `PomodoroCompleted` — that event
    /// is reserved for natural countdown completions because
    /// the persistence layer reads the two events differently.
    pub fn skip(&mut self) -> Vec<TimerEvent> {
        let mut events = Vec::new();
        let skipped_mode = self.current_mode;
        let elapsed_secs = if skipped_mode == TimerMode::Focus {
            self.current_session_elapsed_secs
        } else {
            0
        };

        match skipped_mode {
            TimerMode::Focus => {
                self.completed_pomodoros = self.completed_pomodoros.saturating_add(1);
                self.total_focus_secs = self
                    .total_focus_secs
                    .saturating_add(self.current_session_elapsed_secs);
                self.current_session_elapsed_secs = 0;
                self.current_mode = if self.completed_pomodoros.is_multiple_of(4) {
                    TimerMode::LongBreak
                } else {
                    TimerMode::Break
                };
            }
            TimerMode::Break | TimerMode::LongBreak => {
                self.current_mode = TimerMode::Focus;
            }
        }

        self.time_remaining_secs = i64::from(self.durations.for_mode(self.current_mode));
        self.is_running = false;
        self.is_auto_paused = false;
        self.is_paused = false;
        self.timer_start_ms = None;
        self.timer_duration_secs = None;

        events.push(TimerEvent::SessionSkipped {
            skipped_mode,
            elapsed_secs,
        });
        events
    }

    /// Replace the per-mode `Durations` while the timer is idle.
    ///
    /// Mirrors the JS-era `pomodoro-timer.js:onSettingsChanged` flow:
    /// when settings update, the new durations are absorbed and the
    /// remaining time on the active mode is rebased to the new
    /// duration ONLY if the timer is idle (otherwise the user's
    /// current session continues against the old anchor — matches
    /// the JS-era behaviour where mid-session settings tweaks don't
    /// truncate the running session).
    ///
    /// Used by Phase 4c's `TimerView` ↔ `RwSignal<Settings>` bridge so
    /// `settings-general.spec.js` (focus-duration 25 → 5) and
    /// `settings-advanced.spec.js` (debug-mode → 3-second timers)
    /// see the timer display update without a process restart.
    pub const fn set_durations(&mut self, durations: Durations) {
        self.durations = durations;
        // Only rebase the displayed remaining time when the timer is
        // idle. A running / paused timer keeps its existing remaining
        // value so mid-session edits don't truncate the user's
        // progress. The check covers both `is_running` and
        // `current_session_elapsed_secs > 0` (which catches the
        // post-pause / pre-resume window).
        if !self.is_running
            && !self.is_paused
            && !self.is_auto_paused
            && self.current_session_elapsed_secs == 0
        {
            self.time_remaining_secs = match self.current_mode {
                TimerMode::Focus => durations.focus as i64,
                TimerMode::Break => durations.short_break as i64,
                TimerMode::LongBreak => durations.long_break as i64,
            };
        }
    }

    /// Resets the engine to its initial state.
    ///
    /// Idle in `Focus` mode with the focus duration's worth of
    /// time remaining; clears the per-session elapsed accumulator
    /// and the wall-clock anchor. The cumulative
    /// `completed_pomodoros` and `total_focus_secs` are NOT reset
    /// (those are run-wide; midnight monitoring at
    /// `pomodoro-timer.js:925-972` clears them — out of Phase 2
    /// scope). Mirrors `resetTimer` at `pomodoro-timer.js:854-878`.
    pub const fn reset(&mut self) {
        self.is_running = false;
        self.is_auto_paused = false;
        self.is_paused = false;
        self.current_mode = TimerMode::Focus;
        self.time_remaining_secs = self.durations.focus as i64;
        self.current_session_elapsed_secs = 0;
        self.timer_start_ms = None;
        self.timer_duration_secs = None;
    }

    /// Records a manual session backfill of `duration_secs`
    /// through the engine path (per Principle I — manual entries
    /// flow through the same accumulators as live sessions).
    ///
    /// Increments `completed_pomodoros`, integrates the duration
    /// into `total_focus_secs`, and emits
    /// `ManualSessionRecorded { duration_secs }`. The in-flight
    /// session's mode and countdown are unaffected — backfill
    /// adds to the historical record without disturbing the
    /// current run.
    pub fn record_manual_session(&mut self, duration_secs: u32) -> Vec<TimerEvent> {
        self.completed_pomodoros = self.completed_pomodoros.saturating_add(1);
        self.total_focus_secs = self.total_focus_secs.saturating_add(duration_secs);
        vec![TimerEvent::ManualSessionRecorded { duration_secs }]
    }

    /// Consume an `ActivitySignal` from the bridge layer.
    ///
    /// Idle while running a focus session triggers auto-pause.
    /// Active while auto-paused triggers auto-resume with a fresh
    /// wall-clock anchor (the suspend gap is NOT charged against
    /// the session). Returns the events fired by the transition
    /// (empty `Vec` if the signal didn't transition state).
    ///
    /// Mirrors `handleUserActivity` + `autoPauseTimer` +
    /// `resumeFromAutoPause` at `pomodoro-timer.js:440-626`.
    pub fn observe_activity(
        &mut self,
        signal: ActivitySignal,
        clock: &dyn Clock,
    ) -> Vec<TimerEvent> {
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
                if self.is_auto_paused {
                    self.is_auto_paused = false;
                    self.is_running = true;
                    // Re-anchor the wall clock to "now" with the
                    // current `time_remaining` snapshot so the
                    // auto-pause gap doesn't leak into the next
                    // tick's elapsed computation. Mirrors
                    // `pomodoro-timer.js:590-591` (`timerStartTime
                    // = Date.now()`, `timerDuration = timeRemaining`).
                    self.timer_start_ms = Some(clock.now_ms());
                    self.timer_duration_secs = Some(self.time_remaining_secs);
                    events.push(TimerEvent::AutoResumed);
                }
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
    /// Returns `TimerError::MaxSessionCapReached` when attempting
    /// to start a new focus session after the configured
    /// `total_sessions` cap has been hit. Mirrors `totalSessions`
    /// at `pomodoro-timer.js:31`.
    pub fn start(&mut self, clock: &dyn Clock) -> Result<(), TimerError> {
        if self.is_running {
            return Ok(());
        }
        // Cap-check: refuse a fresh focus start once the total has
        // been reached. The engine still permits in-progress
        // breaks to start (the cap is per focus session, not per
        // mode start) — the test exercises the focus-start
        // boundary because that's where the user-visible "no more
        // sessions" affordance fires.
        if self.current_mode == TimerMode::Focus && self.completed_pomodoros >= self.total_sessions
        {
            return Err(TimerError::MaxSessionCapReached);
        }
        let now = clock.now_ms();
        self.is_running = true;
        self.is_paused = false;
        self.timer_start_ms = Some(now);
        self.timer_duration_secs = Some(self.time_remaining_secs);
        Ok(())
    }

    /// Manually pause the countdown.
    ///
    /// Freezes `current_session_elapsed_secs` at its current value
    /// and clears the wall-clock anchor so subsequent `tick()`s
    /// short-circuit (the running flag flips off). Resuming via
    /// `resume()` re-anchors the wall clock to "now" so the pause
    /// gap doesn't leak into the next tick's elapsed computation.
    ///
    /// Mirrors `pauseTimer` at `pomodoro-timer.js:790-822`.
    /// Distinct from auto-pause (smart-pause); the bridge layer
    /// renders different tray icons for the two states.
    ///
    /// # Errors
    /// Returns `TimerError::NotRunning` if invoked while the engine
    /// is idle (not running, not already paused). Pausing while
    /// already paused is a no-op (`Ok(vec![])`) — the JS source
    /// silently ignores redundant pauses, and so does this.
    pub fn pause(&mut self, _clock: &dyn Clock) -> Result<Vec<TimerEvent>, TimerError> {
        if self.is_paused {
            // Already paused — no-op (idempotent). The JS source
            // mirrors this at `pauseTimer:792` (`if (this.isPaused)
            // return;`).
            return Ok(Vec::new());
        }
        if !self.is_running {
            return Err(TimerError::NotRunning);
        }
        self.is_running = false;
        self.is_paused = true;
        // Freeze the wall-clock anchor: clearing `timer_start_ms`
        // is what makes `tick()` short-circuit (it requires both
        // anchor + duration to advance). The accumulator is
        // preserved as-is by virtue of not being touched.
        self.timer_start_ms = None;
        self.timer_duration_secs = None;
        Ok(vec![TimerEvent::SessionPaused])
    }

    /// Resume from a manual pause.
    ///
    /// Re-anchors the wall clock to "now" with the current
    /// `time_remaining` snapshot so subsequent ticks count from the
    /// resume moment forward — the pause gap is NOT charged
    /// against the session. Mirrors `resumeTimer` at
    /// `pomodoro-timer.js:824-878`.
    ///
    /// `resume()` accepts both the manual-pause unwind path and
    /// the smart-pause (`is_auto_paused`) unwind path — the JS
    /// source mirrors the same single-entrypoint behaviour: a
    /// click on the play/pause button resumes the engine
    /// regardless of which pause variant put it there. Smart-
    /// pause's activity-driven unwind continues to flow through
    /// `observe_activity(Active)` (which still emits
    /// `AutoResumed`); the explicit-resume path emits
    /// `SessionResumed` for both.
    ///
    /// # Errors
    /// Returns `TimerError::NotPaused` if invoked while the engine
    /// is not in any pause state (manual or smart). Resuming while
    /// already running is a no-op (`Ok(vec![])`) — symmetric with
    /// the `pause()` no-op when already paused.
    pub fn resume(&mut self, clock: &dyn Clock) -> Result<Vec<TimerEvent>, TimerError> {
        if self.is_running && !self.is_paused {
            // Already running — no-op (idempotent).
            return Ok(Vec::new());
        }
        if !self.is_paused && !self.is_auto_paused {
            return Err(TimerError::NotPaused);
        }
        self.is_paused = false;
        self.is_auto_paused = false;
        self.is_running = true;
        self.timer_start_ms = Some(clock.now_ms());
        self.timer_duration_secs = Some(self.time_remaining_secs);
        Ok(vec![TimerEvent::SessionResumed])
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
        let (Some(start_ms), Some(duration_secs)) = (self.timer_start_ms, self.timer_duration_secs)
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
                    // Integrate the wall-clock-accumulated focus
                    // work into the run-wide total. Mirrors
                    // `totalFocusTime += actualElapsedTime` at
                    // `pomodoro-timer.js:1167`.
                    self.total_focus_secs = self
                        .total_focus_secs
                        .saturating_add(self.current_session_elapsed_secs);
                    self.current_session_elapsed_secs = 0;
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
/// The variant set is intentionally extensible — `non_exhaustive`
/// requires consumers to `match` with a fallback arm so adding
/// future variants doesn't break call sites.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TimerError {
    /// Attempted to start a focus session after the configured
    /// max-session cap (`total_sessions`) was reached. The
    /// caller's contract is to prompt the user to either reset
    /// the run or bump the cap. Mirrors `totalSessions` at
    /// `pomodoro-timer.js:31`.
    MaxSessionCapReached,
    /// Attempted to `pause()` while the engine was idle (not
    /// running, not already paused). Pausing while already paused
    /// is a no-op (`Ok(vec![])`); pausing from idle is a caller
    /// bug (the UI gates pause on the running flag).
    NotRunning,
    /// Attempted to `resume()` while the engine was not in a
    /// manual-pause state. Resuming while already running is a
    /// no-op (`Ok(vec![])`); resuming from a fresh idle state is
    /// a caller bug (start, not resume, is the right entrypoint).
    NotPaused,
}

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

        let events = state.observe_activity(ActivitySignal::Idle, &clock);

        assert!(state.is_auto_paused());
        assert!(!state.is_running());
        assert!(
            events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::AutoPaused)),
            "expected AutoPaused in {events:?}"
        );
    }

    /// T134: while auto-paused (smart-pause kicked in), an `Active`
    /// signal resumes the timer. Mirrors `resumeFromAutoPause` at
    /// `pomodoro-timer.js:564-626`. The wall-clock anchor is
    /// re-recorded so subsequent ticks count from the resume
    /// moment forward (the suspend gap is NOT charged against the
    /// session — that's the difference between an OS suspend
    /// (T128) and a deliberate auto-pause (T134)).
    #[test]
    fn smart_pause_resumes_on_activity() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default());
        state.set_smart_pause_enabled(true);
        state.start(&clock).expect("start");
        clock.advance(60_000);
        state.tick(&clock);
        // Auto-pause.
        state.observe_activity(ActivitySignal::Idle, &clock);
        assert!(state.is_auto_paused());
        let elapsed_at_pause = state.current_session_elapsed_secs();
        let remaining_at_pause = state.time_remaining_secs();

        // 5 minutes of inactivity tick by, then user moves.
        clock.advance(5 * 60 * 1000);
        let events = state.observe_activity(ActivitySignal::Active, &clock);

        assert!(!state.is_auto_paused());
        assert!(state.is_running());
        // The 5 minutes of suspended wall-clock are NOT charged
        // against the session: time_remaining and the elapsed
        // accumulator are unchanged at resume.
        assert_eq!(state.time_remaining_secs(), remaining_at_pause);
        assert_eq!(state.current_session_elapsed_secs(), elapsed_at_pause);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::AutoResumed)),
            "expected AutoResumed in {events:?}"
        );

        // After resume, a fresh tick advances the countdown again.
        clock.advance(1_000);
        state.tick(&clock);
        assert_eq!(state.time_remaining_secs(), remaining_at_pause - 1);
    }

    /// T136: after `total_sessions` focus pomodoros have been
    /// completed, further `start()` calls return
    /// `TimerError::MaxSessionCapReached` rather than re-arming
    /// the countdown. Mirrors the `totalSessions` cap at
    /// `pomodoro-timer.js:31` (default 10) plus the
    /// `currentSession`-display gating at line 1117-1119.
    #[test]
    fn max_session_cap_stops_at_total_sessions() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default());
        // Lower the cap for test speed.
        state.set_total_sessions(2);

        // Drive two full focus → break → focus return cycles to
        // hit the cap.
        for _ in 0..2 {
            state.start(&clock).expect("start focus");
            clock.advance(25 * 60 * 1000);
            state.tick(&clock);
            // Drive the break to completion.
            state.start(&clock).expect("start break");
            clock.advance(i64::from(state.current_mode_duration_secs()) * 1000);
            state.tick(&clock);
        }

        assert_eq!(state.completed_pomodoros(), 2);

        // Cap reached — `start` is rejected.
        let result = state.start(&clock);
        assert_eq!(result, Err(super::TimerError::MaxSessionCapReached));
        assert!(!state.is_running());
    }

    /// T138: manual session entries route through the engine path
    /// rather than bypassing into the persistence layer directly.
    /// Per Principle I rule "manual session entry must go through
    /// the same engine path as live sessions".
    ///
    /// `record_manual_session(secs)` increments
    /// `completed_pomodoros`, integrates the duration into the
    /// total focus time, and emits `ManualSessionRecorded`. The
    /// engine's mode + countdown are NOT disturbed (the user
    /// might be mid-focus when they add a backfill manual entry).
    #[test]
    fn manual_session_entry_routes_through_engine() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default());
        state.start(&clock).expect("start");
        clock.advance(60_000);
        state.tick(&clock);

        let pomodoros_before = state.completed_pomodoros();
        let total_focus_before = state.total_focus_secs();
        let mode_before = state.current_mode();
        let remaining_before = state.time_remaining_secs();

        let events = state.record_manual_session(20 * 60);

        assert_eq!(state.completed_pomodoros(), pomodoros_before + 1);
        assert_eq!(state.total_focus_secs(), total_focus_before + 20 * 60);
        // Mode + countdown unchanged — manual entry doesn't disturb
        // an in-flight live session.
        assert_eq!(state.current_mode(), mode_before);
        assert_eq!(state.time_remaining_secs(), remaining_before);
        assert!(state.is_running());
        assert!(
            events.iter().any(|e| matches!(
                e,
                super::TimerEvent::ManualSessionRecorded {
                    duration_secs: 1200
                }
            )),
            "expected ManualSessionRecorded {{ duration_secs: 1200 }} in {events:?}"
        );
    }

    /// T142: `skip()` advances to the next mode WITHOUT emitting
    /// `PomodoroCompleted`. This matters because the
    /// persistence layer treats `PomodoroCompleted` as the
    /// "save this session" trigger; a skipped session is recorded
    /// only if it ran for at least 1 minute (per
    /// `pomodoro-timer.js:1088-1090`), and that gating happens
    /// outside the engine. The skip-event itself is distinct so
    /// the bridge layer can disambiguate.
    ///
    /// Mirrors `skipSession` at `pomodoro-timer.js:974-1150`.
    /// The `completedPomodoros++` increment IS part of the JS
    /// skip path (line 1071), so the engine mirrors that — only
    /// the `PomodoroCompleted` event is suppressed in favour of
    /// `SessionSkipped`.
    #[test]
    fn skip_advances_to_next_mode_without_emitting_completed() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default());
        state.start(&clock).expect("start");
        clock.advance(60_000);
        state.tick(&clock);
        assert_eq!(state.current_mode(), TimerMode::Focus);
        assert_eq!(state.completed_pomodoros(), 0);

        let events = state.skip();

        assert_eq!(state.current_mode(), TimerMode::Break);
        assert_eq!(state.completed_pomodoros(), 1);
        assert_eq!(state.time_remaining_secs(), 5 * 60);
        assert!(!state.is_running());
        assert!(
            events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::SessionSkipped { .. })),
            "expected SessionSkipped in {events:?}"
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::PomodoroCompleted { .. })),
            "PomodoroCompleted must NOT fire on skip; events={events:?}"
        );
    }

    /// T140: `reset()` returns the engine to its initial state —
    /// idle in `Focus` mode with the focus duration's worth of
    /// time remaining and the per-session elapsed accumulator
    /// cleared. The cumulative `completed_pomodoros` and
    /// `total_focus_secs` are NOT reset (those are run-wide;
    /// midnight monitoring is what clears them in the JS source
    /// at `pomodoro-timer.js:925-972`).
    ///
    /// Mirrors `resetTimer` at `pomodoro-timer.js:854-878`.
    #[test]
    fn reset_returns_to_initial_state() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default());
        state.start(&clock).expect("start");
        clock.advance(60_000);
        state.tick(&clock);
        assert!(state.is_running());
        assert_eq!(state.current_session_elapsed_secs(), 60);

        state.reset();

        assert!(!state.is_running());
        assert!(!state.is_auto_paused());
        assert_eq!(state.current_mode(), TimerMode::Focus);
        assert_eq!(state.time_remaining_secs(), 25 * 60);
        assert_eq!(state.current_session_elapsed_secs(), 0);
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

    /// Engine backfill (Phase 4a/4b gap): explicit manual pause/resume
    /// must preserve the per-session elapsed accumulator across the
    /// pause window — the suspend gap does NOT count as focus work,
    /// and post-resume ticks accrue from the frozen value forward.
    ///
    /// Mirrors the JS-era `pauseTimer` / `resumeTimer` at
    /// `pomodoro-timer.js:790-878`. The contract pin is the test
    /// name itself (per AGENTS.md §"RED-first": the test name + the
    /// behaviour it asserts is the audit surface).
    ///
    /// The test currently fails because `pause()` / `resume()` /
    /// `is_paused()` aren't public on `TimerState` — only the
    /// existing `reset()` and `start()` exist (the
    /// `components/timer.rs` `on_play_pause` path stops the clock
    /// by calling `reset()`, which clobbers
    /// `current_session_elapsed_secs`).
    #[test]
    fn pause_preserves_remaining_time_and_resume_continues_from_same_point() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default());
        state.start(&clock).expect("start");

        // 5 minutes elapsed mid-focus (no completion yet — still
        // 20 minutes left in the focus countdown).
        clock.advance(5 * 60 * 1000);
        let _ = state.tick(&clock);
        let elapsed_before_pause = state.current_session_elapsed_secs();
        let remaining_before_pause = state.time_remaining_secs();
        assert_eq!(elapsed_before_pause, 5 * 60);

        // Manual pause. The wall-clock anchor freezes; the engine
        // is no longer running.
        let pause_events = state.pause(&clock).expect("pause should succeed");
        assert!(
            pause_events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::SessionPaused)),
            "expected SessionPaused in {pause_events:?}"
        );
        assert!(state.is_paused());
        assert!(!state.is_running());

        // 2 minutes pass during the pause — these MUST NOT count.
        clock.advance(2 * 60 * 1000);
        let _ = state.tick(&clock);
        assert_eq!(
            state.current_session_elapsed_secs(),
            elapsed_before_pause,
            "elapsed time frozen during pause"
        );
        assert_eq!(
            state.time_remaining_secs(),
            remaining_before_pause,
            "time remaining frozen during pause"
        );

        // Resume. Re-anchor the wall clock; subsequent ticks add to
        // the frozen accumulator going forward.
        let resume_events = state.resume(&clock).expect("resume should succeed");
        assert!(
            resume_events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::SessionResumed)),
            "expected SessionResumed in {resume_events:?}"
        );
        assert!(!state.is_paused());
        assert!(state.is_running());

        // 1 minute after resume — the accumulator and countdown
        // each move by exactly 60 seconds.
        clock.advance(60 * 1000);
        let _ = state.tick(&clock);
        assert_eq!(
            state.current_session_elapsed_secs(),
            elapsed_before_pause + 60,
            "elapsed time tracks again after resume"
        );
        assert_eq!(
            state.time_remaining_secs(),
            remaining_before_pause - 60,
            "countdown resumes from the same point"
        );
    }

    /// Engine backfill (Phase 4a/4b gap): when smart-pause kicks in
    /// from inactivity and the user then explicitly hits the resume
    /// button (rather than waiting for activity-driven auto-resume),
    /// the public `resume()` API must transition the engine back to
    /// running with the elapsed accumulator preserved across the
    /// pause window.
    ///
    /// The JS source `pomodoro-timer.js:824-878` (`resumeTimer`)
    /// handles BOTH manual-pause and auto-pause unwind paths through
    /// a single entrypoint — clicking the play/pause button while
    /// auto-paused resumes the engine the same way clicking it
    /// while manually-paused does. The Rust port must mirror that.
    ///
    /// This currently fails because `resume()` only handles
    /// `is_paused == true`; it returns `NotPaused` when the engine
    /// is in `is_auto_paused == true` (smart-pause) state, leaving
    /// the user stuck unless they generate activity.
    #[test]
    fn smart_pause_then_explicit_resume_works_correctly() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default());
        state.set_smart_pause_enabled(true);
        state.start(&clock).expect("start");

        // 90 seconds of focus work, then idle → smart-pause.
        clock.advance(90 * 1000);
        let _ = state.tick(&clock);
        let elapsed_before_pause = state.current_session_elapsed_secs();
        let remaining_before_pause = state.time_remaining_secs();
        assert_eq!(elapsed_before_pause, 90);

        let _ = state.observe_activity(ActivitySignal::Idle, &clock);
        assert!(state.is_auto_paused());
        assert!(!state.is_running());

        // 30 seconds of inactivity tick by — should NOT count.
        clock.advance(30 * 1000);
        let _ = state.tick(&clock);
        assert_eq!(
            state.current_session_elapsed_secs(),
            elapsed_before_pause,
            "smart-pause must freeze the elapsed accumulator"
        );

        // User clicks the resume button (not waiting for activity).
        let resume_events = state
            .resume(&clock)
            .expect("explicit resume from smart-pause should succeed");

        // The engine resumes — running flag back on, smart-pause
        // flag off — and the events surface the transition. The
        // event variant is implementation-defined: either
        // `SessionResumed` (treating manual + smart-pause unwinds
        // as one path, JS-source-style) or `AutoResumed` (preserving
        // the smart-pause unwind label) is acceptable. The
        // contract pin is just that ONE of the resume events fires.
        assert!(!state.is_auto_paused());
        assert!(state.is_running());
        assert!(
            resume_events.iter().any(|e| matches!(
                e,
                super::TimerEvent::SessionResumed | super::TimerEvent::AutoResumed
            )),
            "expected SessionResumed or AutoResumed in {resume_events:?}",
        );

        // Post-resume: a 1-second tick advances the countdown by 1
        // and accrues 1 to the elapsed accumulator (the 30s of
        // smart-pause inactivity is NOT charged).
        clock.advance(1_000);
        let _ = state.tick(&clock);
        assert_eq!(
            state.current_session_elapsed_secs(),
            elapsed_before_pause + 1,
            "post-resume tick accrues from the frozen accumulator"
        );
        assert_eq!(
            state.time_remaining_secs(),
            remaining_before_pause - 1,
            "post-resume countdown advances 1 second per tick"
        );
    }
}
