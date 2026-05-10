// Engine — `ActivitySignal` reduction.
//
// Spec 001-leptos-migration §Phase 2 T130-T131: the engine consumes
// a normalised `ActivitySignal` stream rather than raw DOM events
// (Principle I). The bridge layer subscribes to `user-activity` /
// `user-inactivity` Tauri events and feeds the engine via
// `Timer::observe_activity(signal)`; this module owns the
// edge-detection logic so duplicate Active→Active or Idle→Idle
// emissions are folded into no-ops.

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
