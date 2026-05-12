// Toast + desktop-notification strings emitted by the timer view.
//
// Pure `const fn` lookups. Pre-refactor these were inlined match
// arms in the `handle_events` body; a regression where the strings
// got swapped (mode-specific → generic) shipped silently because
// nothing pinned the wire text. Each function below has a
// co-located test pinning every variant.

use crate::bridge::types::TimerMode;

/// Toast text shown when a focus pomodoro completes. The legacy
/// JS-era flow at `pomodoro-timer.js:1271-1281` distinguishes
/// between "next is short break" and "next is long break" — every
/// `sessions_per_long_break`-th completion lands in `LongBreak`.
#[must_use]
pub(super) const fn pomodoro_completed_toast(
    completed_pomodoros: u32,
    sessions_per_long_break: u32,
) -> &'static str {
    if completed_pomodoros.is_multiple_of(sessions_per_long_break) {
        "Great work! Take a long break \u{1f389}"
    } else {
        "Pomodoro completed! Take a short break \u{1f60c}"
    }
}

/// Desktop-notification body paired with [`pomodoro_completed_toast`].
#[must_use]
pub(super) const fn pomodoro_completed_desktop_body(
    completed_pomodoros: u32,
    sessions_per_long_break: u32,
) -> &'static str {
    if completed_pomodoros.is_multiple_of(sessions_per_long_break) {
        "Focus session complete \u{2014} take a long break"
    } else {
        "Focus session complete \u{2014} take a short break"
    }
}

/// Toast shown when a break / long-break session completes (mode
/// already flipped back to Focus on the engine side). Mirrors
/// `pomodoro-timer.js:1276-1281`.
#[must_use]
pub(super) const fn break_completed_toast(completed_mode: TimerMode) -> &'static str {
    match completed_mode {
        TimerMode::Break => "Break over! Ready to focus? \u{1f345}",
        TimerMode::LongBreak => "Long break over! Time to get back to work \u{1f680}",
        // Defensive default — `BreakCompleted` carries Break or
        // LongBreak only; `Focus` would be an engine regression.
        TimerMode::Focus => "Session completed",
    }
}

/// Desktop-notification body paired with [`break_completed_toast`].
#[must_use]
pub(super) const fn break_completed_desktop_body(completed_mode: TimerMode) -> &'static str {
    match completed_mode {
        TimerMode::Break => "Break finished \u{2014} back to focus",
        TimerMode::LongBreak => "Long break finished \u{2014} back to focus",
        TimerMode::Focus => "Session completed",
    }
}

/// Continuous-mode (`allow_continuous_sessions = true`) completion
/// messages — fires on `OvertimeStarted`. Pre-refactor the handler
/// always used the focus-mode strings even when a break entered
/// overtime; pin the per-mode wording.
///
/// Returns `(toast, desktop_body)`.
#[must_use]
pub(super) const fn overtime_started_messages(mode: TimerMode) -> (&'static str, &'static str) {
    match mode {
        TimerMode::Focus => (
            "Pomodoro completed! Continue working or take a break \u{1f345}",
            "Focus session complete \u{2014} overtime started",
        ),
        TimerMode::Break => (
            "Break time completed! Continue resting or ready to focus? \u{2615}",
            "Break over \u{2014} continue resting or get back to work",
        ),
        TimerMode::LongBreak => (
            "Long break completed! Continue resting or ready to work? \u{1f319}",
            "Long break over \u{2014} continue resting or get back to work",
        ),
    }
}

/// Per-mode skipped-session toast. Mirrors
/// `pomodoro-timer.js:1049-1058`.
#[must_use]
pub(super) const fn session_skipped_toast(skipped_mode: TimerMode) -> &'static str {
    match skipped_mode {
        TimerMode::Focus => "Focus session skipped \u{1f60c}",
        TimerMode::Break => "Break skipped \u{2014} ready to focus? \u{1f345}",
        TimerMode::LongBreak => "Long break skipped \u{2014} back to work \u{1f680}",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        break_completed_desktop_body, break_completed_toast, overtime_started_messages,
        pomodoro_completed_desktop_body, pomodoro_completed_toast, session_skipped_toast,
    };
    use crate::bridge::types::TimerMode;

    #[test]
    fn pomodoro_completed_toast_first_three_use_short_break_message() {
        for n in [1u32, 2, 3] {
            assert_eq!(
                pomodoro_completed_toast(n, 4),
                "Pomodoro completed! Take a short break \u{1f60c}",
                "completed_pomodoros={n} (not multiple of 4) must show short-break toast",
            );
        }
    }

    #[test]
    fn pomodoro_completed_toast_every_fourth_uses_long_break_message() {
        for n in [4u32, 8, 12] {
            assert_eq!(
                pomodoro_completed_toast(n, 4),
                "Great work! Take a long break \u{1f389}",
                "completed_pomodoros={n} (multiple of 4) must show long-break toast",
            );
        }
    }

    #[test]
    fn pomodoro_completed_toast_respects_configured_sessions_per_long_break() {
        // sessions_per_long_break=6: 5 is short, 6 is long.
        assert_eq!(
            pomodoro_completed_toast(5, 6),
            "Pomodoro completed! Take a short break \u{1f60c}",
        );
        assert_eq!(
            pomodoro_completed_toast(6, 6),
            "Great work! Take a long break \u{1f389}",
        );
    }

    #[test]
    fn pomodoro_completed_desktop_body_mirrors_toast_branch() {
        assert_eq!(
            pomodoro_completed_desktop_body(3, 4),
            "Focus session complete \u{2014} take a short break",
        );
        assert_eq!(
            pomodoro_completed_desktop_body(4, 4),
            "Focus session complete \u{2014} take a long break",
        );
    }

    #[test]
    fn break_completed_toast_per_mode() {
        assert_eq!(
            break_completed_toast(TimerMode::Break),
            "Break over! Ready to focus? \u{1f345}",
        );
        assert_eq!(
            break_completed_toast(TimerMode::LongBreak),
            "Long break over! Time to get back to work \u{1f680}",
        );
    }

    #[test]
    fn break_completed_desktop_body_per_mode() {
        assert_eq!(
            break_completed_desktop_body(TimerMode::Break),
            "Break finished \u{2014} back to focus",
        );
        assert_eq!(
            break_completed_desktop_body(TimerMode::LongBreak),
            "Long break finished \u{2014} back to focus",
        );
    }

    #[test]
    fn overtime_started_messages_focus_variant() {
        let (toast, desk) = overtime_started_messages(TimerMode::Focus);
        assert_eq!(
            toast,
            "Pomodoro completed! Continue working or take a break \u{1f345}",
        );
        assert!(desk.contains("Focus session"));
    }

    #[test]
    fn overtime_started_messages_break_variant_uses_break_specific_wording() {
        // Regression: pre-refactor this fired focus-mode wording even
        // when a break overtime started. Pin the break-specific text.
        let (toast, _desk) = overtime_started_messages(TimerMode::Break);
        assert!(
            toast.starts_with("Break time completed"),
            "break overtime must use break-specific toast; got: {toast}",
        );
    }

    #[test]
    fn overtime_started_messages_long_break_variant() {
        let (toast, _desk) = overtime_started_messages(TimerMode::LongBreak);
        assert!(toast.starts_with("Long break completed"));
    }

    #[test]
    fn session_skipped_toast_per_mode() {
        assert_eq!(
            session_skipped_toast(TimerMode::Focus),
            "Focus session skipped \u{1f60c}",
        );
        assert_eq!(
            session_skipped_toast(TimerMode::Break),
            "Break skipped \u{2014} ready to focus? \u{1f345}",
        );
        assert_eq!(
            session_skipped_toast(TimerMode::LongBreak),
            "Long break skipped \u{2014} back to work \u{1f680}",
        );
    }
}
