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
