// Engine — `Durations` configurable per-mode duration set.
//
// Spec 001-leptos-migration §Phase 2: settings live outside the
// engine (`managers::settings`); the engine accepts a `Durations`
// at construction so it stays pure (Principle I). Defaults mirror
// `src/core/pomodoro-timer.js:48-52` — focus 25 min, short break
// 5 min, long break 20 min — expressed in seconds throughout.

use crate::bridge::timer_mode::TimerMode;

/// Per-mode duration set in seconds.
///
/// Constructed from `managers::settings::TimerSettings` at engine
/// boot or rebuilt and re-applied when settings change. The engine
/// reads but never writes this struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Durations {
    /// Focus session length, seconds.
    pub focus: u32,
    /// Short break length, seconds.
    pub short_break: u32,
    /// Long break length, seconds.
    pub long_break: u32,
}

impl Default for Durations {
    fn default() -> Self {
        Self {
            focus: 25 * 60,
            short_break: 5 * 60,
            long_break: 20 * 60,
        }
    }
}

impl Durations {
    /// Returns the duration in seconds for the given `TimerMode`.
    #[must_use]
    pub const fn for_mode(&self, mode: TimerMode) -> u32 {
        match mode {
            TimerMode::Focus => self.focus,
            TimerMode::Break => self.short_break,
            TimerMode::LongBreak => self.long_break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Durations;
    use crate::bridge::timer_mode::TimerMode;

    /// Default values mirror `pomodoro-timer.js:48-52`: focus 25 min,
    /// short break 5 min, long break 20 min. Pinned in seconds so a
    /// unit change (e.g. accidentally switching to minutes) fails loud.
    #[test]
    fn default_values_are_standard_pomodoro_durations() {
        let d = Durations::default();
        assert_eq!(d.focus, 25 * 60, "focus must be 25 min in seconds");
        assert_eq!(d.short_break, 5 * 60, "short break must be 5 min in seconds");
        assert_eq!(d.long_break, 20 * 60, "long break must be 20 min in seconds");
    }

    /// `for_mode` routes each `TimerMode` variant to the matching
    /// field. Pinned so a refactor that swaps two arms fails a test
    /// rather than producing a silent timing regression.
    #[test]
    fn for_mode_returns_correct_duration_for_each_mode() {
        let d = Durations {
            focus: 1500,
            short_break: 300,
            long_break: 1200,
        };
        assert_eq!(d.for_mode(TimerMode::Focus), 1500);
        assert_eq!(d.for_mode(TimerMode::Break), 300);
        assert_eq!(d.for_mode(TimerMode::LongBreak), 1200);
    }

    /// Custom durations (non-default values) route correctly — guards
    /// against a default-value coincidence masking a routing bug.
    #[test]
    fn for_mode_works_with_custom_durations() {
        let d = Durations {
            focus: 3000,
            short_break: 600,
            long_break: 1800,
        };
        assert_eq!(d.for_mode(TimerMode::Focus), 3000);
        assert_eq!(d.for_mode(TimerMode::Break), 600);
        assert_eq!(d.for_mode(TimerMode::LongBreak), 1800);
    }
}
