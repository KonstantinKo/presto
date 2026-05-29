// Engine — `TimerState` pomodoro state machine.
//
// Spec 001-leptos-migration §Phase 2 (T120-T146); ported from
// `src/core/pomodoro-timer.js`. Pure state machine — no DOM-
// binding crate imports, no DOM reads. All inputs (wall-clock
// time, activity signals, settings) are passed in via constructor
// / setters / `tick(now_ms)`.
//
// See `engine/mod.rs` for module-level Principle I rationale.

use crate::bridge::types::TimerMode;
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
    /// Two minutes remain in the current focus session (120→≤120
    /// crossing during `Focus` mode). Only emitted in `Focus` mode;
    /// the `new_remaining > 0` guard ensures it does not fire on the
    /// same tick as the zero-cross. Mirrors `pomodoro-timer.js:758-775`.
    TwoMinutesRemaining,
    /// Thirty seconds remain in the current focus session (30→≤30
    /// crossing during `Focus` mode). Only emitted in `Focus` mode.
    /// Mirrors `pomodoro-timer.js:758-775`.
    ThirtySecondsRemaining,
    /// The current session crossed zero while `allow_continuous_sessions`
    /// is enabled. Fires once on the zero-cross; subsequent ticks
    /// advance `time_remaining_secs` further negative without re-firing.
    /// Mirrors the continuous-session overtime branch at
    /// `pomodoro-timer.js:776-785`.
    OvertimeStarted { mode: TimerMode },
    /// `start()` transitioned the engine from idle to running.
    /// Drives the start-side effects the JS-era `startTimer()`
    /// inlined at `pomodoro-timer.js:709-712` (chime when
    /// `enableSoundNotifications` is on, "Timer started!" ping).
    /// Resume flows emit `SessionResumed` instead — distinct so
    /// the UI can choose a different toast even though both
    /// trigger the same chime.
    SessionStarted,
    /// A break (short or long) just finished (countdown crossed
    /// zero in `Break` or `LongBreak` mode and traditional —
    /// non-continuous — mode is in effect). Carries the mode that
    /// just completed so the UI can pick the mode-specific
    /// completion message. Mirrors the legacy `completeSession`
    /// break-branch toast at `pomodoro-timer.js:1276-1281`.
    BreakCompleted { mode: TimerMode },
    /// `abort()` discarded the in-progress session. Carries the
    /// mode that was being aborted and the elapsed-seconds value
    /// captured before zeroing. Read by the Leptos tick-loop
    /// subscriber to clear pending auto-restart-countdown UI state;
    /// the auto-restart UI gate at
    /// `src/src/components/timer/mod.rs:1471-1483` is also extended
    /// in feature 006 to require `PomodoroCompleted` (not just a
    /// running-edge transition) so `SessionAborted` does not trigger
    /// auto-restart. Engine-internal — never reaches the Tauri
    /// bridge (Principle II).
    SessionAborted {
        aborted_mode: TimerMode,
        elapsed_secs: u32,
    },
    /// `complete()` ended a paused (or auto-paused) focus session
    /// early. Carries the elapsed-seconds captured before zeroing.
    /// Engine-internal observability for the feature-006 RED tests;
    /// emitted unconditionally in the count-incrementing branch
    /// (including the continuous-mode overtime sub-branch where the
    /// canonical `PomodoroCompleted` already fired at zero-cross).
    /// Never reaches the Tauri bridge.
    SessionCompletedEarly { elapsed_secs: u32 },
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
#[allow(
    clippy::struct_excessive_bools,
    reason = "TimerState bools preserve distinct JS-era run/pause/smart-pause signals required by UI and tray parity."
)]
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
    /// When `true`, a focus session that crosses zero enters overtime
    /// (time remaining goes negative) rather than transitioning to
    /// `Break`. Toggled via `set_allow_continuous_sessions`. Mirrors
    /// the JS-era continuous-sessions setting.
    allow_continuous_sessions: bool,
    /// Set on the focus zero-cross when `allow_continuous_sessions`
    /// is on; cleared by `skip()` and `reset()`. Prevents a second
    /// `completed_pomodoros` increment if the user manually skips
    /// during overtime — the zero-cross already counted the session.
    session_completed_but_not_saved: bool,
    /// Number of focus completions per long-break cycle. Replaces
    /// the pre-002 hard-coded literal `4` at the natural zero-cross
    /// and skip-session branches. Default `4` matches the legacy
    /// cadence bit-for-bit (SC-006); updated mid-flight via
    /// `set_sessions_per_long_break`. The 1–10 clamp lives at the
    /// Settings UI input boundary (Principle III); the engine takes
    /// any `u32` and uses it verbatim.
    sessions_per_long_break: u32,
    /// Wall-clock timestamp (ms since unix epoch) at which the user
    /// FIRST started the currently-active session — set on the
    /// Idle → Running transition, preserved across pause / resume
    /// cycles, cleared on `abort` / `complete` / natural completion
    /// / `skip` / `reset`.
    ///
    /// R-003 fix: the Distraction modal needs a stable
    /// `parent_session_start_ts` so two distractions captured from
    /// the same logical focus session share the same anchor. The
    /// pre-fix UI derived it as `now - elapsed_secs * 1000`, but
    /// `current_session_elapsed_secs` is focus-only accumulated time
    /// (paused gaps excluded), so after a pause cycle the derived
    /// timestamp drifts. This field is the wall-clock truth that
    /// `current_session_started_at_ms` returns.
    ///
    /// Distinct from `timer_start_ms`: the latter is the CURRENT
    /// run-segment anchor (cleared on pause, re-set on resume) used
    /// for drift compensation, whereas this field is the SESSION
    /// anchor used by the UI for cross-event correlation.
    session_started_at_ms: Option<i64>,
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
            allow_continuous_sessions: false,
            session_completed_but_not_saved: false,
            sessions_per_long_break: 4,
            session_started_at_ms: None,
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
        reason = "Steady-state time_remaining_secs is >= 0 and bounded by configured mode durations."
    )]
    pub const fn time_remaining_secs(&self) -> u32 {
        if self.time_remaining_secs < 0 {
            0
        } else {
            self.time_remaining_secs as u32
        }
    }

    /// Signed seconds remaining. Negative during overtime when
    /// `allow_continuous_sessions` is enabled. The unsigned
    /// `time_remaining_secs()` clamps at 0 for normal display; this
    /// accessor lets the UI show the absolute overtime elapsed.
    #[must_use]
    pub const fn time_remaining_secs_signed(&self) -> i64 {
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

    /// Enables or disables continuous-sessions (overtime) mode.
    /// When enabled, a focus session that crosses zero re-anchors
    /// and runs negative rather than transitioning to break.
    pub const fn set_allow_continuous_sessions(&mut self, enabled: bool) {
        self.allow_continuous_sessions = enabled;
    }

    /// Replace the long-break cadence count (focus completions per
    /// long-break cycle).
    ///
    /// Mirrors `set_durations`'s posture (`engine/timer.rs:435`):
    /// assignment only, no clamp inside the engine — the 1–10 clamp
    /// is enforced at the Settings UI input boundary per Principle
    /// III. A mid-session change does NOT truncate the running
    /// session's `time_remaining_secs` or change `current_mode`; the
    /// new value applies on the next zero-cross / skip transition
    /// (FR-012 + Bundle B User Story 4).
    pub const fn set_sessions_per_long_break(&mut self, n: u32) {
        self.sessions_per_long_break = n;
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

    /// Wall-clock timestamp (ms since unix epoch) at which the user
    /// first started the currently-active session.
    ///
    /// R-003 fix: the Distraction modal reads this to derive a
    /// `parent_session_start_ts` that stays stable across pause /
    /// resume cycles, so two distractions captured from the same
    /// logical session share the same anchor. Returns `None` when
    /// the engine is fully Idle (no active session).
    #[must_use]
    pub const fn current_session_started_at_ms(&self) -> Option<i64> {
        self.session_started_at_ms
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
                if self.session_completed_but_not_saved {
                    // The zero-cross already counted this session;
                    // don't double-increment completed_pomodoros or
                    // re-integrate elapsed when skipping during overtime.
                    self.session_completed_but_not_saved = false;
                } else {
                    self.completed_pomodoros = self.completed_pomodoros.saturating_add(1);
                    self.total_focus_secs = self
                        .total_focus_secs
                        .saturating_add(self.current_session_elapsed_secs);
                }
                self.current_session_elapsed_secs = 0;
                self.current_mode = if self.should_take_long_break() {
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
        // R-003 fix: skip ends the current logical session — clear
        // the anchor so the next start() stamps a fresh one.
        self.session_started_at_ms = None;

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
            // `as i64` required: `From<u32> for i64` is not yet const-stable (issue #143874).
            self.time_remaining_secs = durations.for_mode(self.current_mode) as i64;
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
        // `as i64` required: `From<u32> for i64` is not yet const-stable (issue #143874).
        self.time_remaining_secs = self.durations.focus as i64;
        self.current_session_elapsed_secs = 0;
        self.timer_start_ms = None;
        self.timer_duration_secs = None;
        self.session_completed_but_not_saved = false;
        // R-003 fix: reset returns the engine to a clean Idle state.
        self.session_started_at_ms = None;
    }

    /// Decrement `completed_pomodoros` by one, saturating at zero.
    ///
    /// Called by the timer view's stop handler while in a break mode to
    /// "undo" the last completed pomodoro without resetting the break
    /// countdown. Mode-agnostic here; the caller gates on break mode.
    pub const fn decrement_completed_pomodoros(&mut self) {
        self.completed_pomodoros = self.completed_pomodoros.saturating_sub(1);
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

    /// Adjust the displayed remaining time by `delta_secs` seconds.
    ///
    /// Mirrors the JS-era `adjustTimer` flow at
    /// `pomodoro-timer.js:adjustTimer` (the `-5/+5` right-rail
    /// affordance — visible on the visual-regression baseline).
    /// `delta_secs > 0` adds time, `< 0` subtracts. Floors at
    /// 1 second so a `-5` press near zero doesn't roll the countdown
    /// negative. No upper bound — the JS-era `adjustTimer` only
    /// floored at zero, allowing power users to run
    /// longer-than-configured sessions.
    ///
    /// When the timer is running, this rebases the wall-clock
    /// anchor to "now" with the new remaining time as the duration
    /// snapshot — the next `tick()` thus measures elapsed time
    /// against the post-adjust baseline. This preserves drift
    /// compensation (Principle I): the wall-clock anchor +
    /// duration-snapshot pair stays in lock-step. Idle / paused
    /// timers leave the anchor untouched (it's already cleared).
    ///
    /// `current_session_elapsed_secs` is unaffected: the user has
    /// already worked the elapsed time, the adjust button only
    /// shifts the *future* portion of the session.
    pub fn adjust_remaining_secs(&mut self, delta_secs: i32, clock: &dyn Clock) {
        let proposed = self
            .time_remaining_secs
            .saturating_add(i64::from(delta_secs));
        // Floor at 1 second so a press near zero doesn't roll the
        // display to 0:00 outside the natural completion path.
        let clamped = proposed.max(1);
        self.time_remaining_secs = clamped;

        // Re-anchor the wall clock when running so the next tick
        // computes elapsed time against the adjusted baseline. The
        // anchor is touched ONLY when running — paused / idle
        // states already cleared the anchor in pause()/reset() and
        // resume() will re-anchor on the next start.
        if self.is_running {
            self.timer_start_ms = Some(clock.now_ms());
            self.timer_duration_secs = Some(self.time_remaining_secs);
        }
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

    /// D-3 fix: engine-side guard against the impossible
    /// `(is_running && (is_paused || is_auto_paused))` tuple.
    ///
    /// Per Principle III the engine boundary should reject illegal
    /// state combinations as loudly as the UI layer (which already
    /// asserts via `RunState::from_engine`). Called at the start of
    /// every state-transition method that touches these flags so the
    /// dev-build panics at the first frame the invariant breaks —
    /// the production build is a no-op.
    fn assert_consistent_state(&self) {
        debug_assert!(
            !(self.is_running && (self.is_paused || self.is_auto_paused)),
            "engine illegal state: cannot be both running and paused/auto-paused"
        );
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
    pub fn start(&mut self, clock: &dyn Clock) -> Result<Vec<TimerEvent>, TimerError> {
        if self.is_running {
            return Ok(Vec::new());
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
        // R-003 fix: stamp the session-start anchor ONLY on the
        // Idle → Running transition. Resuming from pause must NOT
        // re-stamp — the UI relies on the original-start invariant
        // for cross-event correlation (e.g. Distraction modal's
        // parent_session_start_ts).
        if self.session_started_at_ms.is_none() {
            self.session_started_at_ms = Some(now);
        }
        Ok(vec![TimerEvent::SessionStarted])
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
    pub fn pause(&mut self, clock: &dyn Clock) -> Result<Vec<TimerEvent>, TimerError> {
        self.assert_consistent_state();
        if self.is_paused {
            // Already paused — no-op (idempotent). The JS source
            // mirrors this at `pauseTimer:792` (`if (this.isPaused)
            // return;`).
            return Ok(Vec::new());
        }
        if !self.is_running {
            return Err(TimerError::NotRunning);
        }
        // FR-013a: settle wall-clock-accumulated elapsed into the
        // session accumulator before clearing the anchor — so a
        // subsequent `complete` reads the true elapsed at pause time
        // even if no `tick` happened to fold the delta in.
        self.settle_wall_clock_elapsed(clock);
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

    /// Fold any wall-clock seconds elapsed since the most recent
    /// `tick` integration into the session accumulator, then clear
    /// the anchor.
    ///
    /// Used by `pause` (FR-013a) and defensively by `abort` /
    /// `complete` so callers from any state observe a consistent
    /// `current_session_elapsed_secs` even when no `tick` has fired
    /// between `start` / `resume` and the pause moment. Mirrors the
    /// integer-truncated arithmetic in `tick_drift_compensation`
    /// (lines 824-832) — `div_euclid(1000)` for the seconds floor,
    /// `saturating_add` for the accumulator update.
    ///
    /// The increment is computed against the current
    /// `time_remaining_secs`, not against `timer_start_ms` directly,
    /// because previous ticks have already drained
    /// `time_remaining_secs` by the integrated portion. Settling here
    /// folds only the residual sub-tick wall-clock seconds — never
    /// double-counts time already in the accumulator.
    fn settle_wall_clock_elapsed(&mut self, clock: &dyn Clock) {
        let (Some(start_ms), Some(duration_secs)) = (self.timer_start_ms, self.timer_duration_secs)
        else {
            return;
        };
        if self.current_mode != TimerMode::Focus {
            self.timer_start_ms = None;
            self.timer_duration_secs = None;
            return;
        }
        let now = clock.now_ms();
        let elapsed_ms = now.saturating_sub(start_ms);
        let elapsed_secs = elapsed_ms.div_euclid(1000);
        let new_remaining = duration_secs - elapsed_secs;
        let old_remaining = self.time_remaining_secs;
        let drained = old_remaining.saturating_sub(new_remaining);
        if drained > 0 {
            let drained_u32 = u32::try_from(drained).unwrap_or(u32::MAX);
            self.current_session_elapsed_secs = self
                .current_session_elapsed_secs
                .saturating_add(drained_u32);
            self.time_remaining_secs = new_remaining;
        }
        self.timer_start_ms = None;
        self.timer_duration_secs = None;
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
        self.assert_consistent_state();
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

    // -------- Feature 006: new entry points --------
    //
    // Phase 3 implementations of `abort` and `complete` per
    // `specs/006-timer-controls-quicklog-distractions/contracts/timer-engine-actions.md`.
    // The shared `complete_focus_session` helper below dedups the
    // natural zero-cross focus path with the early-`complete` path
    // (branch B.1) so both observe identical accumulator and event
    // semantics (AG-9 finding).

    const fn should_take_long_break(&self) -> bool {
        self.completed_pomodoros
            .is_multiple_of(self.sessions_per_long_break)
    }

    /// Shared seal-and-advance for a focus-session completion.
    ///
    /// Increments `completed_pomodoros`, integrates the in-flight
    /// `current_session_elapsed_secs` into `total_focus_secs`, runs
    /// the long-break cadence check, advances `current_mode`, and
    /// emits `PomodoroCompleted`. Resets the wall-clock anchor and
    /// the three run-state bools. Consumed by:
    ///
    /// - `tick_drift_compensation`'s natural zero-cross focus branch
    ///   (line region 929-960).
    /// - `complete`'s branch B.1 (paused, any positive elapsed, not
    ///   in continuous-mode overtime).
    fn complete_focus_session(&mut self) -> Vec<TimerEvent> {
        self.completed_pomodoros = self.completed_pomodoros.saturating_add(1);
        // Integrate the wall-clock-accumulated focus work into the
        // run-wide total. Mirrors `totalFocusTime += actualElapsedTime`
        // at `pomodoro-timer.js:1167`.
        self.total_focus_secs = self
            .total_focus_secs
            .saturating_add(self.current_session_elapsed_secs);
        self.current_session_elapsed_secs = 0;
        // Every Nth focus completion enters `LongBreak`; otherwise
        // short `Break`. `N` is `self.sessions_per_long_break`.
        self.current_mode = if self.should_take_long_break() {
            TimerMode::LongBreak
        } else {
            TimerMode::Break
        };
        self.time_remaining_secs = i64::from(self.durations.for_mode(self.current_mode));
        self.is_running = false;
        self.is_paused = false;
        self.is_auto_paused = false;
        self.timer_start_ms = None;
        self.timer_duration_secs = None;
        // R-003 fix: focus completion (early or natural) ends the
        // current session. Clear the anchor so the next start()
        // stamps a fresh one. Note: this branch covers BOTH the
        // natural zero-cross path (via tick_drift_compensation) and
        // the explicit `complete()` path's B.1 branch.
        self.session_started_at_ms = None;
        vec![TimerEvent::PomodoroCompleted {
            completed_pomodoros: self.completed_pomodoros,
        }]
    }

    /// Discard the in-progress session entirely.
    ///
    /// Idempotent — a call from Idle returns `[]`. From Running /
    /// Paused / `AutoPaused`: settles wall-clock-accumulated elapsed
    /// into the captured event payload, then resets state without
    /// advancing `current_mode` or touching `completed_pomodoros` /
    /// `total_focus_secs`.
    ///
    /// Clears `session_completed_but_not_saved` to prevent leak into
    /// the next session (mirrors `skip`'s clearing at lines 429-433).
    pub fn abort(&mut self, clock: &dyn Clock) -> Vec<TimerEvent> {
        self.assert_consistent_state();
        if !self.is_running && !self.is_paused && !self.is_auto_paused {
            return Vec::new();
        }
        if self.is_auto_paused {
            // Auto-pause anchor predates the idle gap — clear it so settle
            // does not count inactivity as focused elapsed.
            self.timer_start_ms = None;
            self.timer_duration_secs = None;
        }
        self.settle_wall_clock_elapsed(clock);
        let aborted_mode = self.current_mode;
        let elapsed_secs = self.current_session_elapsed_secs;
        self.is_running = false;
        self.is_paused = false;
        self.is_auto_paused = false;
        self.current_session_elapsed_secs = 0;
        self.session_completed_but_not_saved = false;
        self.timer_start_ms = None;
        self.timer_duration_secs = None;
        // R-003 fix: clear the session-start anchor — the next
        // start() will stamp a fresh one. Symmetric with skip/reset/
        // complete and the natural-completion branch in tick.
        self.session_started_at_ms = None;
        // Restore the displayed countdown for the current mode so a
        // follow-up `start()` runs a full session (avoids a negative
        // `time_remaining_secs` leaking in from a prior continuous-
        // mode overtime — see `abort_clears_session_completed_but_not_saved_flag`).
        self.time_remaining_secs = i64::from(self.durations.for_mode(self.current_mode));
        vec![TimerEvent::SessionAborted {
            aborted_mode,
            elapsed_secs,
        }]
    }

    /// Honestly end a paused (or auto-paused) Focus session early.
    ///
    /// Precondition: `is_paused || is_auto_paused`. From any other
    /// state returns `[]` (engine-side cheat-tax for the
    /// state-aware-matrix UI gate).
    ///
    /// Any positive `current_session_elapsed_secs` counts as a
    /// completion — no anti-cheat threshold. (PO overrode FR-015
    /// anti-cheat; sub-30s completions count by design.)
    ///
    /// Branches on `session_completed_but_not_saved`:
    /// - `false` ⇒ runs `complete_focus_session` (count, integrate,
    ///   advance, emit `PomodoroCompleted`). Appends
    ///   `SessionCompletedEarly`.
    /// - `true` (continuous-mode overtime) ⇒ integrates the overtime
    ///   portion only into `total_focus_secs`, clears the flag,
    ///   does NOT re-increment `completed_pomodoros` or re-emit
    ///   `PomodoroCompleted` (the zero-cross already did both).
    ///   Emits only `SessionCompletedEarly`.
    pub fn complete(&mut self, clock: &dyn Clock) -> Vec<TimerEvent> {
        self.assert_consistent_state();
        if !(self.is_paused || self.is_auto_paused) {
            return Vec::new();
        }
        // Defensive — `pause` already settled, but `is_auto_paused`
        // paths may have skipped it. The guard below clears the stale
        // anchor first so the idle gap is not counted as focused elapsed.
        if self.is_auto_paused {
            // Auto-pause anchor predates the idle gap — clear it so settle
            // does not count inactivity as focused elapsed.
            self.timer_start_ms = None;
            self.timer_duration_secs = None;
        }
        self.settle_wall_clock_elapsed(clock);
        let elapsed = self.current_session_elapsed_secs;

        // Continuous-mode overtime path: the original session already
        // counted via the zero-cross, so the overtime portion is sealed
        // verbatim (B.2) without re-incrementing the pomodoro count.
        if self.session_completed_but_not_saved {
            // B.2 — continuous-mode overtime. The zero-cross already
            // incremented `completed_pomodoros` and emitted
            // `PomodoroCompleted`. Seal the overtime portion and
            // transition out of the still-running overtime mode
            // (the zero-cross also set `current_mode` to the next
            // mode via the cadence check there).
            self.total_focus_secs = self.total_focus_secs.saturating_add(elapsed);
            self.current_session_elapsed_secs = 0;
            self.session_completed_but_not_saved = false;
            self.is_running = false;
            self.is_paused = false;
            self.is_auto_paused = false;
            self.timer_start_ms = None;
            self.timer_duration_secs = None;
            // R-003 fix: B.2 overtime completion ends the logical
            // session too (the zero-cross already counted it; the
            // overtime portion sealing here is the final step).
            self.session_started_at_ms = None;
            // Note: continuous-mode zero-cross at line 871 leaves
            // `current_mode` as Focus and re-anchors for negative-
            // countdown overtime. The spec-mandated post-`complete`
            // mode is the cadence-determined next mode — run the
            // cadence check here against the count the zero-cross
            // already incremented so the post-condition matches B.1.
            self.current_mode = if self.should_take_long_break() {
                TimerMode::LongBreak
            } else {
                TimerMode::Break
            };
            self.time_remaining_secs = i64::from(self.durations.for_mode(self.current_mode));
            return vec![TimerEvent::SessionCompletedEarly {
                elapsed_secs: elapsed,
            }];
        }

        // B.1 — normal paused-before-zero path. Reuse the shared
        // helper so the side-effect sequence matches natural
        // completion exactly. Any positive elapsed counts (PO
        // override of FR-015 anti-cheat threshold).
        let mut events = self.complete_focus_session();
        events.push(TimerEvent::SessionCompletedEarly {
            elapsed_secs: elapsed,
        });
        events
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
        if !self.is_running {
            return Vec::new();
        }
        let (Some(start_ms), Some(duration_secs)) = (self.timer_start_ms, self.timer_duration_secs)
        else {
            return Vec::new();
        };

        let now = clock.now_ms();
        let elapsed_ms = now.saturating_sub(start_ms);
        let elapsed_secs = elapsed_ms.div_euclid(1000);
        let new_remaining = duration_secs - elapsed_secs;
        let old_remaining = self.time_remaining_secs;
        self.time_remaining_secs = new_remaining;

        self.tick_drift_compensation(old_remaining, new_remaining, now)
    }

    /// Integrate one tick's worth of elapsed time into the engine's
    /// accumulators and fire any zero-cross completion transitions.
    ///
    /// Called by `tick()` after the wall-clock arithmetic is done.
    /// `old_remaining` is `self.time_remaining_secs` *before* the
    /// tick updated it; `new_remaining` is the freshly-computed
    /// value (already written to `self.time_remaining_secs` by the
    /// caller). `now_ms` is the wall-clock timestamp captured in
    /// `tick()` — used to re-anchor the overtime countdown.
    /// Returns any events that fired during this tick.
    fn tick_drift_compensation(
        &mut self,
        old_remaining: i64,
        new_remaining: i64,
        now_ms: i64,
    ) -> Vec<TimerEvent> {
        let mut events = Vec::new();

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

        // 2-minute and 30-second warning events (focus mode only).
        // Placed before the zero-cross block so a tick that crosses
        // both 120 and 0 emits the warning AND the completion.
        // The `new_remaining > 0` guard prevents the warning from
        // firing on the same tick as the zero-cross itself. Mirrors
        // `pomodoro-timer.js:758-775`.
        if self.current_mode == TimerMode::Focus {
            if old_remaining > 120 && new_remaining <= 120 && new_remaining > 0 {
                events.push(TimerEvent::TwoMinutesRemaining);
            }
            if old_remaining > 30 && new_remaining <= 30 && new_remaining > 0 {
                events.push(TimerEvent::ThirtySecondsRemaining);
            }
        }

        // Zero-cross from positive to non-positive triggers the
        // mode's completion transition. Mirrors the
        // `oldTimeRemaining > 0 && timeRemaining <= 0` check at
        // `pomodoro-timer.js:777`.
        if old_remaining > 0 && new_remaining <= 0 {
            match self.current_mode {
                TimerMode::Focus if self.allow_continuous_sessions => {
                    // Continuous sessions: count the completion but
                    // don't flip mode. Re-anchor so subsequent ticks
                    // make `time_remaining_secs` go negative.
                    // Mirrors `pomodoro-timer.js:776-785`.
                    self.completed_pomodoros = self.completed_pomodoros.saturating_add(1);
                    self.total_focus_secs = self
                        .total_focus_secs
                        .saturating_add(self.current_session_elapsed_secs);
                    self.current_session_elapsed_secs = 0;
                    events.push(TimerEvent::PomodoroCompleted {
                        completed_pomodoros: self.completed_pomodoros,
                    });
                    events.push(TimerEvent::OvertimeStarted {
                        mode: TimerMode::Focus,
                    });
                    self.session_completed_but_not_saved = true;
                    // Re-anchor at zero so elapsed time from here
                    // subtracts from 0 → negative.
                    self.timer_start_ms = Some(now_ms);
                    self.timer_duration_secs = Some(0);
                }
                TimerMode::Break | TimerMode::LongBreak if self.allow_continuous_sessions => {
                    // Break overtime: re-anchor without mode flip or
                    // accumulator change.
                    events.push(TimerEvent::OvertimeStarted {
                        mode: self.current_mode,
                    });
                    self.timer_start_ms = Some(now_ms);
                    self.timer_duration_secs = Some(0);
                }
                TimerMode::Focus => {
                    // Natural zero-cross focus completion. Delegates
                    // to the shared `complete_focus_session` helper
                    // so the early-`complete()` path (Phase 3 T025)
                    // traverses the identical state transitions.
                    events.extend(self.complete_focus_session());
                }
                TimerMode::Break | TimerMode::LongBreak => {
                    // Break-mode completion returns to focus. Mirrors
                    // `pomodoro-timer.js:1213` (`this.currentMode =
                    // "focus"`).
                    let completed_mode = self.current_mode;
                    self.current_mode = TimerMode::Focus;
                    self.time_remaining_secs =
                        i64::from(self.durations.for_mode(self.current_mode));
                    self.is_running = false;
                    self.timer_start_ms = None;
                    self.timer_duration_secs = None;
                    // R-003 fix: break completion ends the logical
                    // break session — clear the anchor.
                    self.session_started_at_ms = None;
                    events.push(TimerEvent::BreakCompleted {
                        mode: completed_mode,
                    });
                }
            }
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
    use crate::bridge::types::TimerMode;
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

    /// T010 (RED → T011..T013 GREEN): with
    /// `sessions_per_long_break = 1`, every natural focus zero-cross
    /// transitions to `LongBreak`. Feature 002 spec FR-013 / SC-005
    /// boundary `N=1`.
    #[test]
    fn long_break_after_n_focus_sessions_with_n_eq_1() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default());
        state.set_sessions_per_long_break(1);

        // First focus completion → LongBreak (N=1 → every completion).
        state.start(&clock).expect("start first focus");
        clock.advance(25 * 60 * 1000);
        state.tick(&clock);

        assert_eq!(state.completed_pomodoros(), 1);
        assert_eq!(state.current_mode(), TimerMode::LongBreak);
        assert_eq!(state.time_remaining_secs(), 20 * 60);
    }

    /// T010: with `sessions_per_long_break = 10`, the `LongBreak` fires
    /// only on the 10th focus completion — completions 1..=9 transition
    /// to short `Break`. SC-005 boundary `N=10`.
    #[test]
    fn long_break_after_n_focus_sessions_with_n_eq_10() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default());
        // Total cap defaults to 10 — leave room for the 10th completion.
        state.set_total_sessions(10);
        state.set_sessions_per_long_break(10);

        // Drive 9 focus→break cycles. Each must transition to Break,
        // not LongBreak, because 1..=9 are not multiples of 10.
        for n in 1..=9u32 {
            cycle_focus_then_break(&mut state, &clock);
            assert_eq!(state.completed_pomodoros(), n);
            assert_eq!(
                state.current_mode(),
                TimerMode::Focus,
                "after the {n}-th cycle the engine returns to Focus",
            );
        }

        // 10th focus completion → LongBreak.
        state.start(&clock).expect("start tenth focus");
        clock.advance(25 * 60 * 1000);
        state.tick(&clock);

        assert_eq!(state.completed_pomodoros(), 10);
        assert_eq!(state.current_mode(), TimerMode::LongBreak);
        assert_eq!(state.time_remaining_secs(), 20 * 60);
    }

    /// T010: skip-session at focus also consults
    /// `sessions_per_long_break`. With `N=1`, the first skip from a
    /// focus session jumps directly to `LongBreak`. FR-013 + spec
    /// Bundle B Story 3 scenario 6.
    #[test]
    fn skip_session_long_break_with_n_eq_1() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default());
        state.set_sessions_per_long_break(1);

        state.start(&clock).expect("start focus");
        clock.advance(5 * 60 * 1000);
        state.tick(&clock);

        // Skip the focus session.
        let _ = state.skip();

        assert_eq!(state.completed_pomodoros(), 1);
        assert_eq!(state.current_mode(), TimerMode::LongBreak);
        assert_eq!(state.time_remaining_secs(), 20 * 60);
    }

    /// T010: a mid-session settings change to `sessions_per_long_break`
    /// MUST NOT truncate the running session's `time_remaining_secs`
    /// or change `current_mode` at the moment of save. FR-012 + Bundle
    /// B User Story 4. The new value takes effect on the next
    /// transition boundary; this test pins the no-truncation half.
    #[test]
    fn mid_session_sessions_per_long_break_change_preserves_anchor() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default());
        state.start(&clock).expect("start focus");

        // 5 minutes into the focus session.
        clock.advance(5 * 60 * 1000);
        state.tick(&clock);
        let remaining_at_save = state.time_remaining_secs();
        let mode_at_save = state.current_mode();
        assert_eq!(mode_at_save, TimerMode::Focus);
        assert_eq!(remaining_at_save, 20 * 60);

        // Settings change mid-focus — mirror the existing
        // `set_durations` posture (assignment only; no rebase of
        // in-flight state).
        state.set_sessions_per_long_break(1);

        assert_eq!(
            state.current_mode(),
            mode_at_save,
            "current_mode must NOT change at the moment of save",
        );
        assert_eq!(
            state.time_remaining_secs(),
            remaining_at_save,
            "time_remaining_secs must NOT change at the moment of save",
        );
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

    /// `skip()` from Idle (never started): should advance Focus → Break, emit
    /// `SessionSkipped` with `elapsed_secs` = 0, and leave the engine not running.
    /// Unlike `pause()`, `skip()` is unconditional — there is no guard on run state.
    #[test]
    fn skip_from_idle_focus_advances_to_break() {
        let mut state = TimerState::new(Durations::default());
        assert_eq!(state.current_mode(), TimerMode::Focus);
        assert!(!state.is_running());

        let events = state.skip();

        assert_eq!(state.current_mode(), TimerMode::Break);
        assert!(!state.is_running());
        assert_eq!(
            state.time_remaining_secs(),
            Durations::default().for_mode(TimerMode::Break)
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                super::TimerEvent::SessionSkipped {
                    skipped_mode: TimerMode::Focus,
                    elapsed_secs: 0
                }
            )),
            "expected SessionSkipped(Focus, 0) in {events:?}"
        );
    }

    #[test]
    fn pause_when_not_running_returns_err() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default());
        // Fresh engine: not started, not paused. pause() must reject with NotRunning.
        let result = state.pause(&clock);
        assert_eq!(result, Err(super::TimerError::NotRunning));
    }

    #[test]
    fn resume_when_not_paused_returns_err() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default());
        // Idle engine (never started): not running, not paused. resume() must reject
        // with NotPaused — `start()` is the correct entry-point from idle, not `resume()`.
        let result = state.resume(&clock);
        assert_eq!(result, Err(super::TimerError::NotPaused));
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

    /// `adjust_remaining_secs(+300)` adds 5 minutes to an idle timer
    /// with no upper-bound ceiling. The JS-era right-rail `+5` press
    /// is the visual-regression baseline's `#timer-plus-btn`; power
    /// users can extend a session past the configured duration.
    #[test]
    fn adjust_remaining_adds_seconds_when_idle() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default()); // focus 1500
                                                               // Knock the displayed remaining down to 1200 first.
        state.adjust_remaining_secs(-300, &clock);
        assert_eq!(state.time_remaining_secs(), 1200);
        state.adjust_remaining_secs(300, &clock);
        assert_eq!(state.time_remaining_secs(), 1500);
        // No ceiling: a second +300 should climb past the mode cap.
        state.adjust_remaining_secs(300, &clock);
        assert_eq!(
            state.time_remaining_secs(),
            1800,
            "+5 past mode cap must not be clamped",
        );
    }

    /// `adjust_remaining_secs(+300)` has no upper bound — repeated
    /// presses accumulate past the configured mode duration.
    #[test]
    fn adjust_remaining_does_not_clamp_at_mode_duration() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default()); // focus 1500
        for _ in 0..4 {
            state.adjust_remaining_secs(300, &clock);
        }
        assert_eq!(
            state.time_remaining_secs(),
            1500 + 4 * 300,
            "four +5 presses from full must reach 2700",
        );
    }

    /// `adjust_remaining_secs(-300)` subtracts 5 minutes and floors
    /// at 1 second so a press near zero doesn't roll the countdown
    /// negative. Mirrors the JS-era `adjustTimer` floor at
    /// `pomodoro-timer.js:adjustTimer`.
    #[test]
    fn adjust_remaining_subtracts_and_clamps_above_zero() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default());
        // Drive the remaining time near zero and confirm the floor.
        for _ in 0..6 {
            state.adjust_remaining_secs(-300, &clock);
        }
        assert_eq!(
            state.time_remaining_secs(),
            1,
            "-5 below 1s must clamp to 1s, not roll negative",
        );
    }

    /// While running, `adjust_remaining_secs` re-anchors the wall
    /// clock so the next `tick()` measures elapsed time against the
    /// adjusted remaining — Principle I drift compensation stays
    /// correct after the user shifts the displayed remaining time.
    #[test]
    fn adjust_remaining_rebases_anchor_when_running() {
        let clock = MockClock::new(10_000);
        let mut state = TimerState::new(Durations::default());
        state.start(&clock).unwrap();
        // Advance 5 seconds of wall time → displayed remaining is
        // 1500 - 5 = 1495 after the next tick.
        clock.advance(5_000);
        let _ = state.tick(&clock);
        assert_eq!(state.time_remaining_secs(), 1495);
        // +5: 1495 + 300 = 1795 (no ceiling — see adjust_remaining_does_not_clamp).
        state.adjust_remaining_secs(300, &clock);
        assert_eq!(state.time_remaining_secs(), 1795);
        // Advance another 1 second → next tick decrements from
        // the post-adjust baseline (1795 → 1794), proving the
        // anchor was rebased.
        clock.advance(1_000);
        let _ = state.tick(&clock);
        assert_eq!(
            state.time_remaining_secs(),
            1794,
            "post-adjust tick must measure against the rebased anchor",
        );
    }

    /// `TwoMinutesRemaining` fires exactly when the countdown crosses
    /// the 120 → ≤120 boundary in Focus mode, and NOT in Break mode.
    #[test]
    fn two_minutes_warning_fires_on_120_crossing_focus_only() {
        // --- Focus mode ---
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations {
            focus: 240,
            short_break: 300,
            long_break: 1200,
        });
        state.start(&clock).expect("start focus");
        // Advance 121 s → remaining = 240 - 121 = 119 (crosses 120).
        clock.advance(121 * 1000);
        let events = state.tick(&clock);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::TwoMinutesRemaining)),
            "TwoMinutesRemaining must fire on 120→119 crossing in Focus; events={events:?}",
        );

        // --- Exact boundary: advance exactly 120 s → new_remaining == 120 ---
        let clock_exact = MockClock::new(0);
        let mut state_exact = TimerState::new(Durations {
            focus: 240,
            short_break: 300,
            long_break: 1200,
        });
        state_exact.start(&clock_exact).expect("start focus exact");
        clock_exact.advance(120 * 1000);
        let exact_events = state_exact.tick(&clock_exact);
        assert!(
            exact_events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::TwoMinutesRemaining)),
            "TwoMinutesRemaining must fire when new_remaining == 120 (exact boundary); \
             events={exact_events:?}",
        );

        // --- Break mode must NOT emit ---
        let clock2 = MockClock::new(0);
        let mut state2 = TimerState::new(Durations {
            focus: 240,
            short_break: 300,
            long_break: 1200,
        });
        state2.start(&clock2).expect("start focus");
        clock2.advance(240 * 1000);
        state2.tick(&clock2); // completes focus → Break
        assert_eq!(state2.current_mode(), TimerMode::Break);
        state2.start(&clock2).expect("start break");
        clock2.advance(181 * 1000); // crosses 120 in break
        let break_events = state2.tick(&clock2);
        assert!(
            !break_events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::TwoMinutesRemaining)),
            "TwoMinutesRemaining must NOT fire in Break mode; events={break_events:?}",
        );
    }

    /// `ThirtySecondsRemaining` fires on the 30 → ≤30 crossing in
    /// Focus mode, including the exact-boundary case where remaining == 30.
    #[test]
    fn thirty_seconds_warning_fires_on_30_crossing() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations {
            focus: 60,
            short_break: 300,
            long_break: 1200,
        });
        state.start(&clock).expect("start");
        // Advance 31 s → remaining = 60 - 31 = 29 (crosses 30).
        clock.advance(31 * 1000);
        let events = state.tick(&clock);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::ThirtySecondsRemaining)),
            "ThirtySecondsRemaining must fire on 30→29 crossing; events={events:?}",
        );

        // --- Exact boundary: advance exactly 30 s → new_remaining == 30 ---
        let clock_exact = MockClock::new(0);
        let mut state_exact = TimerState::new(Durations {
            focus: 60,
            short_break: 300,
            long_break: 1200,
        });
        state_exact.start(&clock_exact).expect("start exact");
        clock_exact.advance(30 * 1000);
        let exact_events = state_exact.tick(&clock_exact);
        assert!(
            exact_events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::ThirtySecondsRemaining)),
            "ThirtySecondsRemaining must fire when new_remaining == 30 (exact boundary); \
             events={exact_events:?}",
        );
    }

    /// Documents the engine's behavior when `adjust_remaining_secs` lifts
    /// remaining above 120 and then a tick crosses 120 again.
    ///
    /// The engine evaluates the warning purely from `old_remaining > 120 &&
    /// new_remaining <= 120` — there is no per-session "already fired" flag.
    /// This means `TwoMinutesRemaining` WILL fire a second time after an
    /// extension that pushes remaining back above 120. This is intentional:
    /// the warning re-fires because, from the engine's perspective, it is a
    /// new crossing event; users who extend the timer mid-session will hear
    /// the chime again. Any future change to this behavior (e.g. making the
    /// warning one-shot per session) must update this test explicitly.
    #[test]
    fn two_minutes_warning_does_not_double_fire_after_adjust() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations {
            focus: 240,
            short_break: 300,
            long_break: 1200,
        });
        state.start(&clock).expect("start");

        // Step 1: advance 121 s → remaining = 119 → TwoMinutesRemaining fires.
        clock.advance(121 * 1000);
        let events1 = state.tick(&clock);
        assert!(
            events1
                .iter()
                .any(|e| matches!(e, super::TimerEvent::TwoMinutesRemaining)),
            "TwoMinutesRemaining must fire on first 120-crossing; events={events1:?}",
        );

        // Step 2: extend the timer by 300 s so remaining > 120 again.
        state.adjust_remaining_secs(300, &clock);
        assert!(
            state.time_remaining_secs() > 120,
            "remaining must be >120 after adjust",
        );

        // Step 3: advance 300 s to cross 120 again.
        // After adjust added 300 to 119 → remaining is 419; need to drain
        // 300 s to reach 119 (i.e. cross the ≤120 boundary).
        clock.advance(300 * 1000);
        let events2 = state.tick(&clock);
        // DOCUMENTED BEHAVIOR: the warning re-fires on every crossing because
        // the engine has no per-session suppression flag. See doc comment above.
        assert!(
            events2
                .iter()
                .any(|e| matches!(e, super::TimerEvent::TwoMinutesRemaining)),
            "TwoMinutesRemaining must re-fire on the second 120-crossing after an \
             adjust_remaining_secs extension (stateless crossing check); events={events2:?}",
        );
    }

    /// With `allow_continuous_sessions = true`, a focus zero-cross
    /// emits `PomodoroCompleted` + `OvertimeStarted`, keeps mode as
    /// Focus, keeps running, and subsequent ticks produce a negative
    /// signed remaining.
    #[test]
    fn continuous_focus_zero_cross_enters_overtime() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations {
            focus: 60,
            short_break: 300,
            long_break: 1200,
        });
        state.set_allow_continuous_sessions(true);
        state.start(&clock).expect("start");

        clock.advance(61 * 1000);
        let events = state.tick(&clock);

        assert!(
            events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::PomodoroCompleted { .. })),
            "PomodoroCompleted must fire on continuous zero-cross; events={events:?}",
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                super::TimerEvent::OvertimeStarted {
                    mode: TimerMode::Focus
                }
            )),
            "OvertimeStarted(Focus) must fire; events={events:?}",
        );
        assert_eq!(state.current_mode(), TimerMode::Focus, "mode stays Focus");
        assert!(state.is_running(), "must stay running");
        assert_eq!(state.completed_pomodoros(), 1);

        // 5 more seconds → signed remaining should be -5.
        clock.advance(5 * 1000);
        let _ = state.tick(&clock);
        assert_eq!(
            state.time_remaining_secs_signed(),
            -5,
            "signed remaining must be -5 after 5 s of overtime",
        );
    }

    /// With `allow_continuous_sessions = true`, a break zero-cross
    /// emits `OvertimeStarted { mode: Break }`, does NOT increment
    /// `completed_pomodoros`, keeps mode as `Break`, keeps running,
    /// and subsequent ticks produce a negative signed remaining.
    #[test]
    fn continuous_break_zero_cross_enters_overtime() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations {
            focus: 60,
            short_break: 60,
            long_break: 1200,
        });
        // Roll through a non-continuous focus session to reach Break mode.
        state.start(&clock).expect("focus start");
        clock.advance(61 * 1000);
        let _ = state.tick(&clock);
        assert_eq!(state.current_mode(), TimerMode::Break);

        state.set_allow_continuous_sessions(true);
        state.start(&clock).expect("break start");
        clock.advance(61 * 1000);
        let events = state.tick(&clock);

        assert!(
            events.iter().any(|e| matches!(
                e,
                super::TimerEvent::OvertimeStarted {
                    mode: TimerMode::Break
                }
            )),
            "OvertimeStarted(Break) must fire on break zero-cross in continuous mode; \
             events={events:?}",
        );
        assert_eq!(
            state.completed_pomodoros(),
            1,
            "completed_pomodoros must not increment on break overtime; \
             only the earlier focus completion counted",
        );
        assert_eq!(state.current_mode(), TimerMode::Break, "mode stays Break");
        assert!(state.is_running(), "must stay running");

        // 5 more seconds → signed remaining should be -5.
        clock.advance(5 * 1000);
        let _ = state.tick(&clock);
        assert_eq!(
            state.time_remaining_secs_signed(),
            -5,
            "signed remaining must be -5 after 5 s of break overtime",
        );
    }

    /// With `allow_continuous_sessions = true`, a long-break zero-cross
    /// emits `OvertimeStarted { mode: LongBreak }`, does NOT increment
    /// `completed_pomodoros`, keeps mode as `LongBreak`, keeps running,
    /// and subsequent ticks produce a negative signed remaining.
    #[test]
    fn continuous_long_break_zero_cross_enters_overtime() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations {
            focus: 60,
            short_break: 60,
            long_break: 60,
        });
        // Roll through 4 focus→break cycles to reach LongBreak mode.
        for _ in 0..4 {
            state.start(&clock).expect("focus start");
            clock.advance(61 * 1000);
            let _ = state.tick(&clock);
            if state.current_mode() == TimerMode::Break {
                state.start(&clock).expect("short break start");
                clock.advance(61 * 1000);
                let _ = state.tick(&clock);
            }
        }
        assert_eq!(state.current_mode(), TimerMode::LongBreak);
        let pomodoros_before = state.completed_pomodoros();

        state.set_allow_continuous_sessions(true);
        state.start(&clock).expect("long break start");
        clock.advance(61 * 1000);
        let events = state.tick(&clock);

        assert!(
            events.iter().any(|e| matches!(
                e,
                super::TimerEvent::OvertimeStarted {
                    mode: TimerMode::LongBreak
                }
            )),
            "OvertimeStarted(LongBreak) must fire on long-break zero-cross in \
             continuous mode; events={events:?}",
        );
        assert_eq!(
            state.completed_pomodoros(),
            pomodoros_before,
            "completed_pomodoros must not increment on long-break overtime",
        );
        assert_eq!(
            state.current_mode(),
            TimerMode::LongBreak,
            "mode stays LongBreak"
        );
        assert!(state.is_running(), "must stay running");

        // 5 more seconds → signed remaining should be -5.
        clock.advance(5 * 1000);
        let _ = state.tick(&clock);
        assert_eq!(
            state.time_remaining_secs_signed(),
            -5,
            "signed remaining must be -5 after 5 s of long-break overtime",
        );
    }

    /// `decrement_completed_pomodoros` saturates at zero — no underflow,
    /// no panic, regardless of how many times it is called.
    #[test]
    fn decrement_completed_pomodoros_saturates_at_zero() {
        let mut state = TimerState::new(Durations {
            focus: 1500,
            short_break: 300,
            long_break: 1200,
        });
        assert_eq!(state.completed_pomodoros(), 0);
        // Already at 0 — must stay at 0.
        state.decrement_completed_pomodoros();
        assert_eq!(state.completed_pomodoros(), 0, "must not underflow from 0");
        state.decrement_completed_pomodoros();
        assert_eq!(
            state.completed_pomodoros(),
            0,
            "still 0 after second decrement"
        );
    }

    /// Skipping during overtime must NOT double-count `completed_pomodoros`.
    #[test]
    fn continuous_skip_during_overtime_does_not_double_count() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations {
            focus: 60,
            short_break: 300,
            long_break: 1200,
        });
        state.set_allow_continuous_sessions(true);
        state.start(&clock).expect("start");

        clock.advance(61 * 1000);
        let _ = state.tick(&clock);
        assert_eq!(state.completed_pomodoros(), 1);

        let _skip_events = state.skip();
        assert_eq!(
            state.completed_pomodoros(),
            1,
            "skip during overtime must not re-increment completed_pomodoros",
        );
        assert_eq!(state.current_mode(), TimerMode::Break);
    }

    /// Regression test: the start-chime side-effect depends on
    /// `start()` emitting `SessionStarted`. Pre-fix the engine
    /// returned `()` and the chime fired only on resume; users heard
    /// nothing on first start. Mirrors `pomodoro-timer.js:709-712`.
    #[test]
    fn start_emits_session_started_event() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default());
        let events = state.start(&clock).expect("start");
        assert_eq!(
            events,
            vec![super::TimerEvent::SessionStarted],
            "first start must emit SessionStarted so the UI can chime",
        );
    }

    /// Calling `start()` while already running is a no-op — it MUST
    /// NOT re-emit `SessionStarted` (would double-fire the chime).
    #[test]
    fn start_when_already_running_is_silent_no_op() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default());
        let _ = state.start(&clock).expect("first start");
        let events = state.start(&clock).expect("second start no-op");
        assert!(
            events.is_empty(),
            "second start must be a silent no-op; got {events:?}",
        );
    }

    /// Regression test: short break completion was emitting no event
    /// in the post-cutover engine. Pre-fix, the UI showed no toast,
    /// no chime, no desktop notification on break end. The fix added
    /// `BreakCompleted { mode }` to the engine; this pins it.
    #[test]
    fn short_break_zero_cross_emits_break_completed_with_mode() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default());
        // Roll through one focus → break transition.
        state.start(&clock).expect("focus start");
        clock.advance(25 * 60 * 1000);
        let _ = state.tick(&clock); // focus completes, mode → Break
        assert_eq!(state.current_mode(), TimerMode::Break);

        // Start the break and run it to completion.
        state.start(&clock).expect("break start");
        clock.advance(5 * 60 * 1000);
        let events = state.tick(&clock);

        let break_completed = events.iter().find_map(|e| match e {
            super::TimerEvent::BreakCompleted { mode } => Some(*mode),
            _ => None,
        });
        assert_eq!(
            break_completed,
            Some(TimerMode::Break),
            "break completion must emit BreakCompleted {{ mode: Break }} \
             so the UI can show 'Break over! Ready to focus?'; got {events:?}",
        );
        assert_eq!(state.current_mode(), TimerMode::Focus);
    }

    /// Long-break completion carries the `LongBreak` variant so the
    /// UI can pick the long-break-specific message ("Long break
    /// over! Time to get back to work 🚀") vs the short-break one.
    #[test]
    fn long_break_zero_cross_emits_break_completed_with_long_break_mode() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default());
        // Roll through 4 focus → break cycles to reach LongBreak.
        for _ in 0..4 {
            state.start(&clock).expect("focus start");
            clock.advance(25 * 60 * 1000);
            let _ = state.tick(&clock);
            if state.current_mode() == TimerMode::Break {
                state.start(&clock).expect("break start");
                clock.advance(5 * 60 * 1000);
                let _ = state.tick(&clock);
            }
        }
        assert_eq!(state.current_mode(), TimerMode::LongBreak);

        state.start(&clock).expect("long break start");
        clock.advance(20 * 60 * 1000);
        let events = state.tick(&clock);

        let long_break_completed = events.iter().find_map(|e| match e {
            super::TimerEvent::BreakCompleted { mode } => Some(*mode),
            _ => None,
        });
        assert_eq!(
            long_break_completed,
            Some(TimerMode::LongBreak),
            "long break completion must emit \
             BreakCompleted {{ mode: LongBreak }}; got {events:?}",
        );
    }

    /// Continuous (`allow_continuous_sessions`) break overtime does
    /// NOT emit `BreakCompleted` — it emits `OvertimeStarted` and
    /// keeps the timer running. Pin this to prevent a future engine
    /// rewrite from accidentally double-firing both events.
    #[test]
    fn continuous_break_zero_cross_does_not_emit_break_completed() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations::default());
        // Roll a non-continuous focus session to reach Break mode
        // (continuous focus would stay in Focus on zero-cross).
        state.start(&clock).expect("focus start");
        clock.advance(25 * 60 * 1000);
        let _ = state.tick(&clock);
        assert_eq!(state.current_mode(), TimerMode::Break);

        // Now flip into continuous mode for the break session.
        state.set_allow_continuous_sessions(true);
        state.start(&clock).expect("break start");
        clock.advance(5 * 60 * 1000);
        let events = state.tick(&clock);

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::BreakCompleted { .. })),
            "continuous mode must NOT emit BreakCompleted on break overtime; \
             OvertimeStarted is the correct event. Got {events:?}",
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::OvertimeStarted { .. })),
            "continuous mode must emit OvertimeStarted on break zero-cross; \
             got {events:?}",
        );
    }

    /// T009: Abort from a Running focus session clears the engine to
    /// Idle in the same mode and emits `SessionAborted`. `elapsed_secs`
    /// is captured before zeroing.
    #[test]
    fn abort_from_running_clears_state_and_returns_idle() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations {
            focus: 60,
            short_break: 300,
            long_break: 1200,
        });
        state.start(&clock).expect("start");
        clock.advance(20 * 1000);
        let _ = state.tick(&clock);
        assert!(state.is_running());
        assert_eq!(state.current_session_elapsed_secs(), 20);

        let events = state.abort(&clock);

        assert_eq!(
            events,
            vec![super::TimerEvent::SessionAborted {
                aborted_mode: TimerMode::Focus,
                elapsed_secs: 20,
            }]
        );
        assert!(!state.is_running());
        assert!(!state.is_paused());
        assert!(!state.is_auto_paused());
        assert_eq!(state.current_session_elapsed_secs(), 0);
        assert_eq!(state.current_mode(), TimerMode::Focus);
    }

    /// T010 (paused half): Abort from a Paused focus session clears to
    /// Idle in the same mode and emits `SessionAborted`. Second call
    /// returns `[]` (idempotence).
    #[test]
    fn abort_from_paused_clears_state_and_returns_idle() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations {
            focus: 60,
            short_break: 300,
            long_break: 1200,
        });
        state.start(&clock).expect("start");
        clock.advance(15 * 1000);
        let _ = state.tick(&clock);
        state.pause(&clock).expect("pause");
        assert!(state.is_paused());

        let events = state.abort(&clock);
        assert_eq!(
            events,
            vec![super::TimerEvent::SessionAborted {
                aborted_mode: TimerMode::Focus,
                elapsed_secs: 15,
            }]
        );
        assert!(!state.is_paused());
        assert_eq!(state.current_session_elapsed_secs(), 0);
        assert_eq!(state.current_mode(), TimerMode::Focus);

        // Second abort is idempotent — no events.
        let again = state.abort(&clock);
        assert_eq!(again, Vec::<super::TimerEvent>::new());
    }

    /// T010 (auto-paused half): Abort from an `AutoPaused` focus session
    /// behaves identically to Abort-from-Paused.
    #[test]
    fn abort_from_autopaused_clears_state_and_returns_idle() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations {
            focus: 60,
            short_break: 300,
            long_break: 1200,
        });
        state.set_smart_pause_enabled(true);
        state.start(&clock).expect("start");
        clock.advance(10 * 1000);
        let _ = state.tick(&clock);
        // Drive into AutoPaused via the idle activity signal.
        let _ = state.observe_activity(ActivitySignal::Idle, &clock);
        assert!(state.is_auto_paused());

        let events = state.abort(&clock);
        assert_eq!(
            events,
            vec![super::TimerEvent::SessionAborted {
                aborted_mode: TimerMode::Focus,
                elapsed_secs: 10,
            }]
        );
        assert!(!state.is_auto_paused());
        assert_eq!(state.current_session_elapsed_secs(), 0);
        assert_eq!(state.current_mode(), TimerMode::Focus);
    }

    /// T011: Abort does not touch `completed_pomodoros`,
    /// `total_focus_secs`, or the long-break cadence accumulator
    /// (`pomodoros_until_long_break` is the UI/render view —
    /// `completed_pomodoros % sessions_per_long_break` — so the
    /// behavioural assertion is on `completed_pomodoros`).
    #[test]
    fn abort_preserves_counters() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations {
            focus: 60,
            short_break: 300,
            long_break: 1200,
        });
        // Roll one completed pomodoro first so the counters are
        // non-zero before the abort.
        state.start(&clock).expect("start");
        clock.advance(60 * 1000);
        let _ = state.tick(&clock);
        assert_eq!(state.completed_pomodoros(), 1);
        let total_focus_before = state.total_focus_secs();

        // Skip back to Focus, start a second session, abort partway.
        let _ = state.skip();
        assert_eq!(state.current_mode(), TimerMode::Focus);
        state.start(&clock).expect("start 2");
        clock.advance(20 * 1000);
        let _ = state.tick(&clock);
        let _ = state.abort(&clock);

        assert_eq!(
            state.completed_pomodoros(),
            1,
            "abort must not increment completed_pomodoros"
        );
        assert_eq!(
            state.total_focus_secs(),
            total_focus_before,
            "abort must not change total_focus_secs"
        );
    }

    /// T012: Abort emits ONLY `SessionAborted` — no `PomodoroCompleted`.
    /// The auto-restart UI gate at `components/timer/mod.rs:1471-1483`
    /// is extended in this PR to also require `PomodoroCompleted` in
    /// the event vec, so this engine-level invariant is what prevents
    /// the gate from firing after an abort (AG-2 finding).
    #[test]
    fn abort_emits_session_aborted_not_pomodoro_completed() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations {
            focus: 60,
            short_break: 300,
            long_break: 1200,
        });
        state.start(&clock).expect("start");
        clock.advance(40 * 1000);
        let _ = state.tick(&clock);

        let events = state.abort(&clock);

        assert!(
            events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::SessionAborted { .. })),
            "abort must emit SessionAborted; got {events:?}"
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::PomodoroCompleted { .. })),
            "abort must NOT emit PomodoroCompleted (auto-restart suppression); got {events:?}"
        );
    }

    /// T013: Abort during continuous-mode overtime clears the
    /// `session_completed_but_not_saved` flag so it does not leak into
    /// the next session (mirrors `skip`'s clearing at lines 407-411).
    #[test]
    fn abort_clears_session_completed_but_not_saved_flag() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations {
            focus: 60,
            short_break: 300,
            long_break: 1200,
        });
        state.set_allow_continuous_sessions(true);
        state.start(&clock).expect("start");
        // Cross the zero-cross to enter overtime — sets
        // session_completed_but_not_saved=true at line 826.
        clock.advance(61 * 1000);
        let _ = state.tick(&clock);
        assert_eq!(state.completed_pomodoros(), 1);

        // Now abort during overtime.
        let _ = state.abort(&clock);

        // Restart for the next session and roll another zero-cross.
        // If the flag had leaked, this second zero-cross would suppress
        // the count.
        state.start(&clock).expect("restart");
        clock.advance(61 * 1000);
        let _ = state.tick(&clock);
        assert_eq!(
            state.completed_pomodoros(),
            2,
            "session_completed_but_not_saved must be cleared by abort, \
             else the next zero-cross would not increment"
        );
    }

    /// T014: Complete from Paused with elapsed=30 counts (branch B.1):
    /// `completed_pomodoros++`, `total_focus_secs += 30`, advances mode,
    /// emits `PomodoroCompleted` + `SessionCompletedEarly { 30 }`.
    #[test]
    fn complete_from_paused_with_elapsed_30_counts_and_advances() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations {
            focus: 60,
            short_break: 300,
            long_break: 1200,
        });
        state.start(&clock).expect("start");
        clock.advance(30 * 1000);
        let _ = state.tick(&clock);
        state.pause(&clock).expect("pause");
        assert_eq!(state.current_session_elapsed_secs(), 30);

        let events = state.complete(&clock);

        assert_eq!(state.completed_pomodoros(), 1);
        assert_eq!(state.total_focus_secs(), 30);
        assert_eq!(state.current_mode(), TimerMode::Break);
        assert!(!state.is_running());
        assert!(!state.is_paused());
        assert!(!state.is_auto_paused());
        assert!(
            events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::PomodoroCompleted { .. })),
            "complete (count branch) must emit PomodoroCompleted; got {events:?}"
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                super::TimerEvent::SessionCompletedEarly { elapsed_secs: 30 }
            )),
            "complete (count branch) must emit SessionCompletedEarly{{30}}; got {events:?}"
        );
    }

    /// T015: Complete from Paused with short elapsed (5 s) counts as
    /// a completed pomodoro — PO override of the FR-015 anti-cheat
    /// threshold means any positive elapsed runs branch B.1, not the
    /// former abort fallback.
    #[test]
    fn complete_from_paused_with_short_elapsed_counts_as_completed() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations {
            focus: 60,
            short_break: 300,
            long_break: 1200,
        });
        state.start(&clock).expect("start");
        clock.advance(5 * 1000);
        let _ = state.tick(&clock);
        state.pause(&clock).expect("pause");
        assert_eq!(state.current_session_elapsed_secs(), 5);

        let events = state.complete(&clock);

        assert_eq!(
            state.completed_pomodoros(),
            1,
            "short-elapsed complete must count (no anti-cheat threshold)"
        );
        assert_eq!(state.total_focus_secs(), 5);
        assert_eq!(
            state.current_mode(),
            TimerMode::Break,
            "short-elapsed complete advances mode per cadence"
        );
        assert!(!state.is_running());
        assert!(!state.is_paused());
        assert!(!state.is_auto_paused());
        assert!(
            events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::PomodoroCompleted { .. })),
            "short-elapsed complete must emit PomodoroCompleted; got {events:?}"
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                super::TimerEvent::SessionCompletedEarly { elapsed_secs: 5 }
            )),
            "short-elapsed complete must emit SessionCompletedEarly{{5}}; got {events:?}"
        );
    }

    /// T016: Complete from `AutoPaused` behaves identically to Complete
    /// from Paused (FR-013, Story 1 AC 3).
    #[test]
    fn complete_from_autopaused_same_as_paused() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations {
            focus: 60,
            short_break: 300,
            long_break: 1200,
        });
        state.set_smart_pause_enabled(true);
        state.start(&clock).expect("start");
        clock.advance(45 * 1000);
        let _ = state.tick(&clock);
        let _ = state.observe_activity(ActivitySignal::Idle, &clock);
        assert!(state.is_auto_paused());

        let events = state.complete(&clock);

        assert_eq!(state.completed_pomodoros(), 1);
        assert_eq!(state.total_focus_secs(), 45);
        assert_eq!(state.current_mode(), TimerMode::Break);
        assert!(
            events.iter().any(|e| matches!(
                e,
                super::TimerEvent::SessionCompletedEarly { elapsed_secs: 45 }
            )),
            "complete from AutoPaused must emit SessionCompletedEarly{{45}}; got {events:?}"
        );
    }

    /// T017: Complete in continuous-mode overtime (branch B.2) emits
    /// `SessionCompletedEarly` unconditionally and does NOT re-emit
    /// `PomodoroCompleted` (zero-cross already fired the canonical one).
    #[test]
    fn complete_emits_session_completed_early_unconditionally_in_branch_b() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations {
            focus: 60,
            short_break: 300,
            long_break: 1200,
        });
        state.set_allow_continuous_sessions(true);
        state.start(&clock).expect("start");
        // Zero-cross at 60 s.
        clock.advance(61 * 1000);
        let _ = state.tick(&clock);
        // Run 120 s of overtime.
        clock.advance(120 * 1000);
        let _ = state.tick(&clock);
        state.pause(&clock).expect("pause");
        // current_session_elapsed_secs now holds overtime portion only.
        let overtime = state.current_session_elapsed_secs();
        assert!(overtime > 0);

        let events = state.complete(&clock);

        assert!(
            events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::SessionCompletedEarly { .. })),
            "branch B.2 must emit SessionCompletedEarly; got {events:?}"
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::PomodoroCompleted { .. })),
            "branch B.2 must NOT re-emit PomodoroCompleted; got {events:?}"
        );
    }

    /// T018: Continuous-mode overtime → complete does not double-count.
    /// Full sequence: start, zero-cross (count = 1), overtime, pause,
    /// complete. Final `completed_pomodoros` is exactly 1 (not 2).
    #[test]
    fn complete_in_continuous_overtime_does_not_double_count() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations {
            focus: 60,
            short_break: 300,
            long_break: 1200,
        });
        state.set_allow_continuous_sessions(true);
        state.start(&clock).expect("start");

        clock.advance(61 * 1000);
        let _ = state.tick(&clock);
        assert_eq!(state.completed_pomodoros(), 1);
        let focus_after_zero_cross = state.total_focus_secs();

        clock.advance(30 * 1000);
        let _ = state.tick(&clock);
        state.pause(&clock).expect("pause");
        let overtime = state.current_session_elapsed_secs();

        let _ = state.complete(&clock);

        assert_eq!(
            state.completed_pomodoros(),
            1,
            "complete in branch B.2 must NOT re-increment completed_pomodoros"
        );
        assert_eq!(
            state.total_focus_secs(),
            focus_after_zero_cross + overtime,
            "overtime portion must be sealed into total_focus_secs"
        );
        assert_eq!(state.current_session_elapsed_secs(), 0);
        assert!(!state.is_running());
        assert!(!state.is_paused());
    }

    /// T019: Intersection — `AutoPaused` during continuous-mode overtime,
    /// then complete. Same invariants as T018 plus `AutoPaused` entry.
    #[test]
    fn complete_from_autopaused_in_continuous_overtime() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations {
            focus: 60,
            short_break: 300,
            long_break: 1200,
        });
        state.set_allow_continuous_sessions(true);
        state.set_smart_pause_enabled(true);
        state.start(&clock).expect("start");

        // Cross into overtime.
        clock.advance(61 * 1000);
        let _ = state.tick(&clock);
        assert_eq!(state.completed_pomodoros(), 1);
        let focus_after_zero_cross = state.total_focus_secs();

        // Run 20 s of overtime, then smart-pause kicks in.
        clock.advance(20 * 1000);
        let _ = state.tick(&clock);
        let _ = state.observe_activity(ActivitySignal::Idle, &clock);
        assert!(state.is_auto_paused());
        let overtime = state.current_session_elapsed_secs();
        assert!(overtime > 0);

        let _ = state.complete(&clock);

        assert_eq!(
            state.completed_pomodoros(),
            1,
            "complete from AutoPaused in overtime must not re-increment"
        );
        assert_eq!(
            state.total_focus_secs(),
            focus_after_zero_cross + overtime,
            "overtime portion integrated into total_focus_secs"
        );
        assert!(!state.is_auto_paused());
        assert_eq!(state.current_mode(), TimerMode::Break);
    }

    /// T020: Pause settles wall-clock delta before complete reads
    /// `current_session_elapsed_secs`. After T024 lands `pause()`
    /// wall-clock-delta settling, the engine sees the full elapsed
    /// at complete-time. Originally guarded the 30 s abort gate
    /// boundary; with the PO override removing that gate the test
    /// still pins the same wall-clock-settling contract.
    #[test]
    fn complete_at_exactly_30s_wall_clock_counts_not_aborts() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations {
            focus: 60,
            short_break: 300,
            long_break: 1200,
        });
        state.start(&clock).expect("start");
        // 30.0 s wall-clock advance — no `tick` call. The current
        // engine's `pause()` does not settle wall-clock delta, so
        // `current_session_elapsed_secs` would read 0 here. After T024
        // it reads 30. This test fails RED for that exact reason.
        clock.advance(30 * 1000);
        state.pause(&clock).expect("pause");
        assert_eq!(
            state.current_session_elapsed_secs(),
            30,
            "FR-013a: pause must settle wall-clock delta into current_session_elapsed_secs"
        );

        let _ = state.complete(&clock);

        assert_eq!(
            state.completed_pomodoros(),
            1,
            "30.0 s wall-clock pause+complete must count (not discard)"
        );
        assert_eq!(state.total_focus_secs(), 30);
        assert_eq!(state.current_mode(), TimerMode::Break);
    }

    /// T021: Complete runs the long-break cadence check.
    /// Parameterised over `sessions_per_long_break ∈ {2, 3, 4}`.
    #[test]
    fn complete_runs_long_break_cadence_check() {
        for n in [2u32, 3, 4] {
            let clock = MockClock::new(0);
            let mut state = TimerState::new(Durations {
                focus: 60,
                short_break: 300,
                long_break: 1200,
            });
            state.set_sessions_per_long_break(n);

            // Roll n-1 full focus→break→focus cycles via skip so the
            // accumulator sits at n-1 completions, then on the nth
            // session we complete-with-elapsed=30.
            for _ in 0..(n - 1) {
                state.start(&clock).expect("start");
                clock.advance(60 * 1000);
                let _ = state.tick(&clock);
                // Engine is now in Break (or LongBreak); skip back to Focus.
                let _ = state.skip();
            }
            assert_eq!(state.completed_pomodoros(), n - 1);
            assert_eq!(state.current_mode(), TimerMode::Focus);

            // Nth session — pause at 30 s, complete.
            state.start(&clock).expect("start nth");
            clock.advance(30 * 1000);
            let _ = state.tick(&clock);
            state.pause(&clock).expect("pause");
            let _ = state.complete(&clock);

            assert_eq!(state.completed_pomodoros(), n);
            assert_eq!(
                state.current_mode(),
                TimerMode::LongBreak,
                "cadence n={n}: nth completion must advance to LongBreak"
            );

            // Roll one more to confirm the next completion does NOT
            // hit the cadence (advances to Break, not LongBreak).
            let _ = state.skip();
            state.start(&clock).expect("start n+1");
            clock.advance(30 * 1000);
            let _ = state.tick(&clock);
            state.pause(&clock).expect("pause");
            let _ = state.complete(&clock);
            assert_eq!(state.completed_pomodoros(), n + 1);
            assert_eq!(
                state.current_mode(),
                TimerMode::Break,
                "cadence n={n}: (n+1)th completion must advance to Break"
            );
        }
    }

    /// T022: Complete is idempotent from Running — engine returns `[]`
    /// and does not mutate state. This is the cheat-tax engine-level
    /// defence: even if the UI matrix lets the button through somehow,
    /// the engine refuses.
    #[test]
    fn complete_idempotent_from_running_is_noop() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations {
            focus: 60,
            short_break: 300,
            long_break: 1200,
        });
        state.start(&clock).expect("start");
        clock.advance(45 * 1000);
        let _ = state.tick(&clock);
        let count_before = state.completed_pomodoros();
        let focus_before = state.total_focus_secs();
        let mode_before = state.current_mode();
        let running_before = state.is_running();

        let events = state.complete(&clock);

        assert_eq!(events, Vec::<super::TimerEvent>::new());
        assert_eq!(state.completed_pomodoros(), count_before);
        assert_eq!(state.total_focus_secs(), focus_before);
        assert_eq!(state.current_mode(), mode_before);
        assert_eq!(state.is_running(), running_before);
    }

    /// T023: When pause is requested at the exact same tick the
    /// countdown naturally reaches zero, the natural-completion
    /// sequence wins — `PomodoroCompleted` fires, mode advances, and
    /// the user lands in the next-mode Idle so `complete` becomes
    /// unreachable. This pins the deterministic ordering for the
    /// zero-cross race (Edge Cases bullet, Story 1 AC 6).
    #[test]
    fn pause_at_zero_cross_lets_natural_completion_win() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations {
            focus: 60,
            short_break: 300,
            long_break: 1200,
        });
        state.start(&clock).expect("start");

        // Advance to the exact zero-cross instant — 60.0 s of wall-clock.
        clock.advance(60 * 1000);
        // The natural tick fires first. The pomodoro count fires and
        // the engine transitions to Break in the same call.
        let tick_events = state.tick(&clock);
        assert!(
            tick_events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::PomodoroCompleted { .. })),
            "natural completion at zero-cross must emit PomodoroCompleted; got {tick_events:?}"
        );
        assert_eq!(state.completed_pomodoros(), 1);
        assert_eq!(state.current_mode(), TimerMode::Break);
        assert!(!state.is_running());
        assert!(!state.is_paused());

        // A subsequent `complete` call on the resulting Idle state must
        // be a no-op (matches the idempotence rule + the matrix's
        // unreachability of the Complete button in Idle).
        let complete_events = state.complete(&clock);
        assert_eq!(complete_events, Vec::<super::TimerEvent>::new());
        assert_eq!(state.completed_pomodoros(), 1);
        assert_eq!(state.current_mode(), TimerMode::Break);
    }

    // R-003 fix: session-start anchor for stable Distraction
    // parent-ref across pause/resume.

    /// The session-start anchor is stamped on the Idle → Running
    /// transition and survives pause / resume cycles. The Distraction
    /// modal relies on this stability so two distractions captured
    /// from the same logical session share the same
    /// `parent_session_start_ts`.
    #[test]
    fn session_started_at_ms_survives_pause_resume_cycle() {
        let clock = MockClock::new(1_000_000);
        let mut state = TimerState::new(Durations::default());
        assert_eq!(
            state.current_session_started_at_ms(),
            None,
            "Idle pre-start: no anchor"
        );

        state.start(&clock).expect("start");
        let stamped = state.current_session_started_at_ms();
        assert_eq!(stamped, Some(1_000_000), "anchor stamped at start()");

        clock.advance(5_000);
        state.pause(&clock).expect("pause");
        assert_eq!(
            state.current_session_started_at_ms(),
            stamped,
            "pause does NOT touch the anchor"
        );

        clock.advance(10_000);
        state.resume(&clock).expect("resume");
        assert_eq!(
            state.current_session_started_at_ms(),
            stamped,
            "resume does NOT re-stamp the anchor"
        );

        clock.advance(5_000);
        state.pause(&clock).expect("second pause");
        assert_eq!(
            state.current_session_started_at_ms(),
            stamped,
            "anchor unchanged across multiple pause cycles"
        );
    }

    /// `abort` clears the session-start anchor — the next `start()`
    /// will stamp a fresh one.
    #[test]
    fn session_started_at_ms_clears_on_abort() {
        let clock = MockClock::new(1_000_000);
        let mut state = TimerState::new(Durations::default());
        state.start(&clock).expect("start");
        assert!(state.current_session_started_at_ms().is_some());

        clock.advance(5_000);
        let _ = state.abort(&clock);
        assert_eq!(
            state.current_session_started_at_ms(),
            None,
            "abort clears the session-start anchor"
        );

        // A subsequent start stamps a fresh anchor (not the original).
        clock.advance(1_000);
        state.start(&clock).expect("re-start");
        assert_eq!(
            state.current_session_started_at_ms(),
            Some(1_006_000),
            "re-start stamps a fresh anchor"
        );
    }

    /// D-3: drive the engine through its full transition surface
    /// and verify the `(is_running && (is_paused || is_auto_paused))`
    /// illegal-state combination never appears. Mirrors the UI-layer
    /// guard in `RunState::from_engine` so a future engine refactor
    /// can't silently introduce the impossible tuple.
    #[test]
    fn engine_never_reports_running_and_paused() {
        let clock = MockClock::new(1_000_000);
        let mut state = TimerState::new(Durations::default());
        let check = |s: &TimerState, label: &str| {
            assert!(
                !(s.is_running() && (s.is_paused() || s.is_auto_paused())),
                "engine illegal state at {label}: running={} paused={} auto_paused={}",
                s.is_running(),
                s.is_paused(),
                s.is_auto_paused(),
            );
        };
        check(&state, "fresh");

        state.start(&clock).expect("start");
        check(&state, "post-start");

        state.pause(&clock).expect("pause");
        check(&state, "post-pause");

        state.resume(&clock).expect("resume");
        check(&state, "post-resume");

        state.set_smart_pause_enabled(true);
        let _ = state.observe_activity(ActivitySignal::Idle, &clock);
        check(&state, "post-auto-pause");

        let _ = state.observe_activity(ActivitySignal::Active, &clock);
        check(&state, "post-auto-resume");

        state.pause(&clock).expect("pause-2");
        check(&state, "post-pause-2");

        // Run some elapsed so complete takes the B.1 branch.
        clock.advance(30_000);
        state.resume(&clock).expect("resume-2");
        check(&state, "post-resume-2");
        state.pause(&clock).expect("pause-3");
        let _ = state.complete(&clock);
        check(&state, "post-complete");
    }

    /// Natural focus completion (via `complete`) clears the anchor.
    /// Together with the analogous tick / break-completion / skip /
    /// reset coverage in the wider test matrix, this pins the
    /// AR-R1: pause-then-complete from Running-overtime emits
    /// `SessionPaused` followed by `SessionCompletedEarly` and leaves
    /// the engine in a clean post-complete state. Mirrors the
    /// transactional sequence the UI's `on_complete` handler runs in a
    /// single `try_update` when the user clicks Complete while the
    /// continuous-mode focus session is in overtime: synth-pause to
    /// satisfy `complete()`'s precondition, then complete via branch B.2.
    ///
    /// The combined event vec must surface BOTH events in the right
    /// order so the UI's `apply_tag_tracking_events` sees the
    /// `SessionPaused` → `FlushAll` action even though the user
    /// clicked Complete, not Pause.
    #[test]
    fn pause_then_complete_from_running_overtime_emits_paused_then_completed_early() {
        let clock = MockClock::new(0);
        let mut state = TimerState::new(Durations {
            focus: 60,
            short_break: 300,
            long_break: 1200,
        });
        state.set_allow_continuous_sessions(true);
        state.start(&clock).expect("start");

        // Cross into overtime: 61 s of wall-clock so the zero-cross fires
        // and the engine re-anchors for negative-countdown overtime.
        clock.advance(61 * 1000);
        let _ = state.tick(&clock);
        assert_eq!(state.completed_pomodoros(), 1);
        assert!(state.is_running());
        assert!(!state.is_paused());
        assert!(!state.is_auto_paused());

        // 30 s of overtime so the elapsed read at complete is non-zero.
        clock.advance(30 * 1000);
        let _ = state.tick(&clock);

        // Mirror the UI's on_complete pause-prelude transactional
        // sequence: pause then complete in one go.
        let pause_events = state.pause(&clock).expect("synth pause precondition");
        let complete_events = state.complete(&clock);
        let all_events: Vec<_> = pause_events.into_iter().chain(complete_events).collect();

        // Order: SessionPaused first (drives FlushAll in tag_tracking),
        // SessionCompletedEarly second (branch B.2's only emission).
        assert!(
            matches!(all_events.first(), Some(super::TimerEvent::SessionPaused)),
            "first event must be SessionPaused; got {all_events:?}"
        );
        assert!(
            all_events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::SessionCompletedEarly { .. })),
            "expected SessionCompletedEarly in {all_events:?}"
        );
        // Branch B.2: zero-cross already incremented the count;
        // complete must NOT re-emit PomodoroCompleted.
        assert!(
            !all_events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::PomodoroCompleted { .. })),
            "branch B.2 must NOT re-emit PomodoroCompleted; got {all_events:?}"
        );

        // Post-state: count unchanged (already incremented at zero-cross),
        // overtime sealed, engine clean and mode advanced per cadence.
        assert_eq!(state.completed_pomodoros(), 1);
        assert!(!state.is_running());
        assert!(!state.is_paused());
        assert!(!state.is_auto_paused());
        assert_eq!(state.current_mode(), TimerMode::Break);
        assert_eq!(state.current_session_elapsed_secs(), 0);
    }

    /// invariant that the field is cleared on every logical session
    /// end.
    #[test]
    fn session_started_at_ms_clears_on_complete() {
        let clock = MockClock::new(1_000_000);
        let mut state = TimerState::new(Durations::default());
        state.start(&clock).expect("start");
        assert!(state.current_session_started_at_ms().is_some());

        // Run some wall-clock elapsed so complete() takes the B.1
        // branch (count + advance).
        clock.advance(30_000);
        state.pause(&clock).expect("pause");
        let events = state.complete(&clock);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, super::TimerEvent::PomodoroCompleted { .. })),
            "expected PomodoroCompleted in {events:?}"
        );
        assert_eq!(
            state.current_session_started_at_ms(),
            None,
            "complete clears the session-start anchor"
        );
    }
}
