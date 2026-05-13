// Tray-icon title / tooltip formatting + Tauri dispatch.
//
// The formatter mirrors the JS-era `updateTrayIcon` at
// `pomodoro-timer.js:2680-2754`. Pre-Phase F the WASM rewrite
// silently shipped a snake_case `UpdateTrayIconArgs` and dispatched
// only on mode/running transitions, so the tray neither showed a
// countdown nor accepted icon-only mode. Both fixes live here, so
// the formatter and the dispatch boundary stay co-located.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::bridge::commands;
use crate::bridge::types::{Settings, StatusBarDisplay, TimerMode, UpdateTrayIconArgs};
use crate::engine::timer::TimerState;

/// Zero-width space padding baseline so the macOS status bar
/// reserves stable width for the longest possible "+99:99 (99/99)"
/// string (legacy `pomodoro-timer.js:2742`).
const TRAY_TEXT_MAX_LEN: usize = 14;

/// Build the `UpdateTrayIconArgs` payload from the current engine
/// state + settings.
///
/// Returns `(timer_text, mode_icon_override)`:
/// - **Default mode**: `timer_text` = `"+mm:ss (c/t)"` padded to 14
///   characters with zero-width spaces; `mode_icon_override =
///   Some(emoji)` so the backend renders `"{emoji} {timer_text}"`.
/// - **Icon-only mode**: `timer_text` = the emoji itself,
///   `mode_icon_override = Some("")` so the backend renders just the
///   emoji (matches legacy line 2712 zeroing `modeIcon` after copying
///   it into `displayText`).
#[must_use]
pub(super) fn build_tray_text(state: &TimerState, settings: &Settings) -> (String, Option<String>) {
    let is_paused = state.is_paused() || state.is_auto_paused();
    let time_signed = state.time_remaining_secs_signed();
    let allow_continuous = settings.notifications.allow_continuous_sessions;
    let is_overtime = time_signed < 0 && allow_continuous;
    let mode = state.current_mode();

    // Glyphs lifted from ramazanberkozbek/presto — chosen so they
    // render as a single monospace cell in the macOS menu bar (no
    // emoji shaping that would push the title off-baseline).
    // Pause is the only state override; overtime drops to the mode
    // icon to match the reference's minimal set.
    let mode_icon: &str = if is_paused {
        // ⏸ pause symbol forced to text presentation via U+FE0E so
        // macOS renders the slim outlined pair (not the chunky emoji
        // variant) at the same baseline as the mode glyphs.
        "\u{23f8}\u{fe0e}"
    } else {
        match mode {
            // ◉ filled circle = focus
            TimerMode::Focus => "\u{25c9}",
            // ☼ sun = short break (daytime rest)
            TimerMode::Break => "\u{263c}",
            // ☾ moon = long break (night rest)
            TimerMode::LongBreak => "\u{263e}",
        }
    };

    match settings.status_bar_display {
        StatusBarDisplay::IconOnly => (mode_icon.to_string(), Some(String::new())),
        StatusBarDisplay::Default => {
            let abs_time = time_signed.unsigned_abs();
            let mins = abs_time / 60;
            let secs = abs_time % 60;
            let prefix = if is_overtime { "+" } else { "" };
            let completed = state.completed_pomodoros();
            let total = settings.timer.total_sessions;
            let real = format!("{prefix}{mins:02}:{secs:02} ({completed}/{total})");
            let pad = TRAY_TEXT_MAX_LEN.saturating_sub(real.chars().count());
            let padding: String = "\u{200b}".repeat(pad);
            (format!("{real}{padding}"), Some(mode_icon.to_string()))
        }
    }
}

/// Push a tray-icon + (optionally) tray-menu refresh through the
/// Tauri bridge. Reads engine + settings untracked so callers can
/// invoke it from any closure without subscribing.
///
/// `menu_dirty=true` re-emits `update_tray_menu` (the start /
/// pause / skip / cancel item labels). Tick loop sets this only on
/// mode/running transitions; explicit start/pause/skip/stop
/// handlers always set it because the user just changed the
/// running state.
pub(super) fn dispatch_tray_update(
    engine: RwSignal<TimerState>,
    settings: RwSignal<Settings>,
    menu_dirty: bool,
) {
    let settings_snapshot = settings.get_untracked();
    let (timer_text, mode_icon, is_running, is_paused, mode_after, current_session) = engine
        .with_untracked(|state| {
            let (text, icon) = build_tray_text(state, &settings_snapshot);
            (
                text,
                icon,
                state.is_running(),
                state.is_paused() || state.is_auto_paused(),
                state.current_mode(),
                state.completed_pomodoros().saturating_add(1),
            )
        });
    let tray_args = UpdateTrayIconArgs {
        timer_text,
        is_running,
        session_mode: mode_after,
        current_session,
        total_sessions: settings_snapshot.timer.total_sessions,
        mode_icon,
    };
    spawn_local(async move {
        let _ = commands::update_tray_icon(tray_args).await;
        if menu_dirty {
            let _ = commands::update_tray_menu(is_running, is_paused, mode_after).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::build_tray_text;
    use crate::bridge::types::{Settings, StatusBarDisplay, TimerMode};
    use crate::engine::durations::Durations;
    use crate::engine::timer::TimerState;

    /// Idle focus session (no clock advance) at default durations
    /// renders the legacy-shape title text plus the focus emoji
    /// override.
    #[test]
    fn default_focus_renders_legacy_format() {
        let state = TimerState::new(Durations::default());
        let settings = Settings::default();
        let (text, icon) = build_tray_text(&state, &settings);
        assert!(
            text.starts_with("25:00 (0/10)"),
            "expected 'mm:ss (c/t)' real text prefix; got {text:?}"
        );
        // ◉ glyph override for Focus mode (consumed by backend's
        // `format!("{icon} {timer_text}")`).
        assert_eq!(icon.as_deref(), Some("\u{25c9}"));
        // 14-char padding to a zero-width-space-suffixed string —
        // backend reserves stable width for "+99:99 (99/99)".
        assert_eq!(
            text.chars().count(),
            14,
            "title must pad to 14 chars (longest possible '+99:99 (99/99)'); got {text:?}",
        );
    }

    /// `IconOnly` mode replaces the timer text with the mode emoji
    /// itself and clears the icon override (backend renders just the
    /// glyph).
    #[test]
    fn icon_only_mode_returns_emoji_only() {
        let state = TimerState::new(Durations::default());
        let settings = Settings {
            status_bar_display: StatusBarDisplay::IconOnly,
            ..Settings::default()
        };
        let (text, icon) = build_tray_text(&state, &settings);
        assert_eq!(
            text, "\u{25c9}",
            "IconOnly must put the mode glyph into the text slot"
        );
        assert_eq!(
            icon.as_deref(),
            Some(""),
            "IconOnly must clear the icon override so backend doesn't double-render"
        );
    }

    /// Paused state overrides the mode emoji with the pause glyph,
    /// matching legacy `pomodoro-timer.js:2700`.
    #[test]
    fn paused_uses_pause_glyph() {
        use crate::engine::clock::Clock;

        struct ZeroClock;
        impl Clock for ZeroClock {
            fn now_ms(&self) -> i64 {
                0
            }
        }
        let mut state = TimerState::new(Durations::default());
        let _ = state.start(&ZeroClock).unwrap();
        let _ = state.pause(&ZeroClock).unwrap();
        assert!(state.is_paused(), "test precondition: state must be paused");
        let (_, icon) = build_tray_text(&state, &Settings::default());
        assert_eq!(
            icon.as_deref(),
            Some("\u{23f8}\u{fe0e}"),
            "paused state must override the mode glyph with the text-variant pause glyph"
        );
    }

    /// Mode emoji table covers `Focus` (brain), `Break` (coffee),
    /// and `LongBreak` (moon). Drives the engine via `skip()`
    /// through the pomodoro cycle: every 4th focus completion lands
    /// in `LongBreak`.
    #[test]
    fn default_mode_glyphs() {
        let mut state = TimerState::new(Durations::default());
        let settings = Settings::default();

        // Focus — ◉ filled circle.
        let (_, icon) = build_tray_text(&state, &settings);
        assert_eq!(icon.as_deref(), Some("\u{25c9}"));

        // Skip 1: Focus → Break (count=1).
        let _ = state.skip();
        assert_eq!(state.current_mode(), TimerMode::Break);
        let (_, icon) = build_tray_text(&state, &settings);
        assert_eq!(icon.as_deref(), Some("\u{263c}"), "Break = sun");

        // Cycle six more skips; after the 7th skip,
        // completed_pomodoros == 4 → LongBreak.
        for _ in 0..6 {
            let _ = state.skip();
        }
        assert_eq!(state.completed_pomodoros(), 4);
        assert_eq!(state.current_mode(), TimerMode::LongBreak);
        let (_, icon) = build_tray_text(&state, &settings);
        assert_eq!(icon.as_deref(), Some("\u{263e}"), "LongBreak = moon");
    }
}
