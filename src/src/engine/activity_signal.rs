// Engine — `ActivitySignal` reduction.
//
// Spec 001-leptos-migration §Phase 2 T130-T131: the engine consumes
// a normalised `ActivitySignal` stream rather than raw DOM events
// (Principle I). The bridge layer subscribes to `user-activity` /
// `user-inactivity` Tauri events and feeds the engine via
// `Timer::observe_activity(signal)`; this module owns the
// edge-detection logic so duplicate Active→Active or Idle→Idle
// emissions are folded into no-ops.

/// Two-state activity signal fed into the engine.
///
/// Wire form: the `bridge::events::USER_ACTIVITY` and
/// `USER_INACTIVITY` Tauri events normalise to this enum at the
/// bridge boundary; the engine never sees raw mousemove /
/// inactivity-timer-elapsed events. Mirrors the binary
/// `isAutoPaused`-control inputs at `pomodoro-timer.js:440-466`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivitySignal {
    /// User input observed — mousemove, keypress, scroll, etc.
    Active,
    /// Inactivity threshold elapsed without input.
    Idle,
}

/// Edge-detecting reducer over a stream of `ActivitySignal` events.
///
/// `observe(signal)` returns `Some(signal)` exactly on a state
/// transition; runs of duplicate signals fold into a single
/// reported transition. The reducer assumes the user is `Active`
/// at construction (the app just gained focus / launched), which
/// matches the JS-side `handleUserActivity` behaviour at
/// `pomodoro-timer.js:440` — the first activity event after a
/// fresh boot is informational, not a transition.
#[derive(Debug, Clone, Copy)]
pub struct ActivityReducer {
    last: ActivitySignal,
}

impl Default for ActivityReducer {
    fn default() -> Self {
        Self::new()
    }
}

impl ActivityReducer {
    /// Constructs a reducer in the `Active` state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            last: ActivitySignal::Active,
        }
    }

    /// Observes a raw signal and reports the transition (if any).
    ///
    /// Returns `Some(signal)` iff `signal != self.last`; otherwise
    /// `None`. After observation, the reducer's internal state
    /// always matches `signal`.
    pub fn observe(&mut self, signal: ActivitySignal) -> Option<ActivitySignal> {
        if signal == self.last {
            None
        } else {
            self.last = signal;
            Some(signal)
        }
    }

    /// Current observed state (the last signal seen, or the
    /// constructor default).
    #[must_use]
    pub const fn current(&self) -> ActivitySignal {
        self.last
    }
}

#[cfg(test)]
mod tests {
    use super::{ActivityReducer, ActivitySignal};

    /// T130: Idle ↔ Active edge detection. The reducer reports
    /// `Some(signal)` only on a transition; runs of duplicate
    /// raw events fold into a single signal. Mirrors the JS-side
    /// logic at `pomodoro-timer.js:440-466` which arms a fresh
    /// inactivity timer only on activity events that actually
    /// transition the state, not on every raw mousemove.
    #[test]
    fn idle_active_edge_detection() {
        let mut reducer = ActivityReducer::new();
        // Fresh reducer assumes Active (the user just opened the
        // app). First raw Active event is a no-op.
        assert_eq!(reducer.observe(ActivitySignal::Active), None);
        assert_eq!(reducer.observe(ActivitySignal::Active), None);
        // Active → Idle is reported.
        assert_eq!(
            reducer.observe(ActivitySignal::Idle),
            Some(ActivitySignal::Idle),
        );
        // Repeated Idle is folded.
        assert_eq!(reducer.observe(ActivitySignal::Idle), None);
        // Idle → Active is reported.
        assert_eq!(
            reducer.observe(ActivitySignal::Active),
            Some(ActivitySignal::Active),
        );
        // Repeated Active is folded.
        assert_eq!(reducer.observe(ActivitySignal::Active), None);
    }
}
