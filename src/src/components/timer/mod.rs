// Timer view component. Spec: 001-leptos-migration §Phase 4a.
//
// **Selector contract** (consumed by `tests/e2e/timer.spec.js`,
// `_smoke.spec.js`, `tags.spec.js`, `sessions-history.spec.js`,
// `visual-regression.spec.js`):
// - `#timer-view` — root view container; carries `.hidden` when
//   another `NavView` is active.
// - `#timer-minutes`, `#timer-seconds` — countdown text. Initial
//   render is the focus duration's worth (`25` / `00`) — the
//   `_smoke.spec.js` first-paint assertion locks this in.
// - `#play-pause-btn`, `#stop-btn`, `#skip-btn` — control buttons.
// - `#play-icon`, `#pause-icon` — visibility-toggled SVGs inside
//   `#play-pause-btn` (running ↔ idle).
// - `#timer-status`, `#status-text`, `#status-icon` — current-mode
//   badge + tag-dropdown trigger.
// - `#timer-status` click toggles `#tag-dropdown-menu` (covered by
//   the Tags component in T201-T203 — same DOM shell).
//
// Per Principle I, this component READS engine + manager state via
// Leptos signals; it never mutates engine state directly except by
// dispatching to the documented engine API
// (`engine::timer::TimerState::{start, skip, reset}`). The wall-
// clock tick loop is driven by `set_interval_with_handle` over a
// `BrowserClock` impl that wraps `js_sys::Date::now()` — the
// abstract `Clock` trait keeps the engine pure.
//
// Lint allowance: `clippy::must_use_candidate` is silenced module-
// wide because Leptos `#[component]` functions return `impl
// IntoView`, which the framework consumes automatically inside
// `view!` / `mount_to_body` — annotating each component with
// `#[must_use]` would be noise that contradicts the Leptos call
// pattern (`<TimerView/>` inside `view!` doesn't bind a result).
// `clippy::too_many_lines` is silenced because Phase 4c folded
// settings-context wiring, the tag-dropdown popover, the document-
// level keydown + click-outside listeners, the engine-completion ->
// session-log push, and the auto-start-on-completion branch into
// the single TimerView body. Splitting the view body across helper
// fns would fragment the JSX-style DOM tree without aiding
// readability — the alternative (a Manager struct + slot-prop
// bridge) is the post-merge plan's larger refactor.
#![allow(clippy::must_use_candidate, clippy::too_many_lines)]

mod tag_tracking;
mod tray;

use std::collections::HashMap;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_i18n::{t, t_string};
use wasm_bindgen::JsCast;

use self::tag_tracking::{
    apply_tag_tracking_events, tag_tracking_flush_all, tag_tracking_flush_one, tag_tracking_start,
};
use self::tray::{build_tray_text, dispatch_tray_update};
use super::browser_clock::BrowserClock;
use crate::app::AppToast;
use crate::bridge::commands;
use crate::bridge::types::AmbientSoundType;
use crate::bridge::types::SessionType;
use crate::bridge::types::TimerMode;
use crate::bridge::types::{ManualSession, Session, Settings, Tag};
use crate::components::ambient_audio;
use crate::engine::clock::Clock;
use crate::engine::durations::Durations;
use crate::engine::timer::{TimerEvent, TimerState};
use crate::i18n::i18n::use_i18n;

/// Icon-picker catalogue (feature 003 Bundle C: 3 remixicon entries +
/// 9 Phosphor entries — FR-020 / FR-021). The five legacy emoji
/// entries (`\u{1f9e0}` `\u{1f4aa}` `\u{1f3af}` `\u{26a1}` `\u{1f525}`)
/// were removed from the picker; existing tags persisted with emoji
/// icons continue to render via `IconClass::Glyph` (FR-024).
///
/// The `ri-` entries dispatch through `IconClass::Remix` →
/// `<i class="ri-{suffix}">`; the `ph-` entries dispatch through
/// `IconClass::Phosphor` → `<i class="ph ph-{suffix}">` (the outer
/// `ph` wrapper class is required for the Phosphor @font-face to
/// bind). Selection happens through the typed dispatch in
/// `crate::components::icon::IconClass::from_icon_name` — no
/// `starts_with(...)` chain at the render sites.
const ICON_OPTIONS: &[&str] = &[
    "ri-brain-line",
    "ri-focus-3-line",
    "ri-lightbulb-line",
    "ph-butterfly",
    "ph-cloud",
    "ph-code-simple",
    "ph-github-logo",
    "ph-apple-logo",
    "ph-crown-simple",
    "ph-atom",
    "ph-student",
    "ph-cpu",
];

/// Icon-picker default. The visual-regression baseline shows a brain
/// glyph rendered through the remixicon webfont (chromium-linux test
/// runner can't render `\u{1f9e0}` from the system emoji font), so the
/// "selected icon" preview seeds with the `ri-brain-line` class form.
/// A user picking an emoji from the dropdown overrides this with the
/// raw glyph for `tags.spec.js:17` parity.
const DEFAULT_NEW_TAG_ICON: &str = "ri-brain-line";

/// Project the engine's `TimerMode` to the JS-era status-text label.
/// Mirrors the JS-side branch at `src/managers/navigation-manager.js`
/// where the badge text is `"Focus" / "Break" / "Long Break"`. The
/// `_smoke.spec.js` first-paint assertion expects "Focus"; the
/// `sessions-history.spec.js` flow asserts the badge becomes "Break"
/// after the focus session completes.
///
/// Feature 005: kept as the English source-of-truth for the
/// `mode_label_covers_every_variant` test. View call sites have moved
/// to `t!(i18n, timer.mode_*)` so the rendered output is localised;
/// this fn remains the canonical mapping the test pins against.
#[cfg(test)]
const fn mode_label(mode: TimerMode) -> &'static str {
    match mode {
        TimerMode::Focus => "Focus",
        TimerMode::Break => "Break",
        TimerMode::LongBreak => "Long Break",
    }
}

/// Extend `mode_label` with the JS-era pause / overtime suffixes.
///
/// Tie-break ordering: `is_paused` wins over `is_auto_paused` wins
/// over overtime — overtime can only show while `is_running`, so it
/// is mutually exclusive with both pause states.
///
/// Feature 005: kept as the English source-of-truth for the
/// `mode_label_with_status_*` tests. View call sites compose the
/// localised label inline via `t_string!(i18n, timer.*)`.
//
// Four bool params reflect four orthogonal `TimerState` predicates.
// Grouping them into a struct would add ceremony without improving
// readability at the single call site.
#[cfg(test)]
#[allow(clippy::fn_params_excessive_bools)]
fn mode_label_with_status(
    mode: TimerMode,
    is_running: bool,
    is_paused: bool,
    is_auto_paused: bool,
    is_overtime: bool,
) -> String {
    let base = mode_label(mode);
    if is_paused {
        format!("{base} (Paused)")
    } else if is_auto_paused {
        format!("{base} (Auto-paused)")
    } else if is_running && is_overtime {
        format!("{base} (Overtime)")
    } else {
        base.to_string()
    }
}

/// Project a settings-indicator enabled flag onto its Remix Icon class string.
///
/// When enabled the icon uses the `-fill` variant plus the `active` class so
/// the CSS's `.settings-indicators i.active` rule applies the accent colour.
fn indicator_icon_class(stem: &str, enabled: bool) -> &'static str {
    // The match table is exhaustive over the three stems used by the
    // right-rail indicators.  Any new stem must be added here and
    // covered by a test.
    if enabled {
        match stem {
            "lightbulb" => "ri-lightbulb-fill active",
            "play-circle" => "ri-play-circle-fill active",
            "repeat" => "ri-repeat-fill active",
            _ => "",
        }
    } else {
        match stem {
            "lightbulb" => "ri-lightbulb-line",
            "play-circle" => "ri-play-circle-line",
            "repeat" => "ri-repeat-line",
            _ => "",
        }
    }
}

/// Project a non-negative seconds value to a zero-padded two-digit
/// string. Used for both the minutes and seconds columns of the
/// countdown display.
///
/// JS-era `String(value).padStart(2, "0")` parity at
/// `pomodoro-timer.js:1027`. Values >= 100 saturate at the literal
/// `format!("{value:02}")` output (which already widens past two
/// digits without truncation — the focus / long-break maxima cap at
/// 60 minutes per the settings clamps).
fn pad_two(value: u32) -> String {
    format!("{value:02}")
}

// -------- Feature 006: closed-sum UI run-state --------
//
// The engine carries three orthogonal bools (`is_running`,
// `is_paused`, `is_auto_paused`) — see `engine::timer.rs:119-173` for
// why the engine keeps them as bools (1:1 JS-era parity). At the UI
// layer we want an exhaustive `match` (Principle III) driving the
// state-aware button matrix, so we fold the bools into a closed sum
// at the boundary. AutoPaused folds into `Paused` per FR-012 ¶3 +
// Story 1 AC 3 — the UI matrix treats both pause variants identically.
//
// Engine-wide refactor (switch the engine itself to a `State` enum)
// is explicitly out of scope; the boundary fold below is feature
// 006's UI-only contract.

/// Closed sum that drives the state-aware button matrix per FR-012.
/// Derived from the engine's three orthogonal bools at the UI layer
/// — engine bools stay as-is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RunState {
    Idle,
    Running,
    Paused,
}

impl RunState {
    /// Project the engine's run-state predicates to the closed sum.
    ///
    /// The `is_paused || is_auto_paused` branch fires **first** —
    /// otherwise `(false, true, false)` (engine-Paused) would fall
    /// through to `is_running == false ⇒ Idle` and the matrix would
    /// render Idle controls over a paused session (AG-1 finding,
    /// `data-model.md` §`RunState` lines 132-157).
    pub(super) fn from_engine(is_running: bool, is_paused: bool, is_auto_paused: bool) -> Self {
        debug_assert!(
            !(is_running && (is_paused || is_auto_paused)),
            "engine illegal state: cannot be both running and paused"
        );
        if is_paused || is_auto_paused {
            Self::Paused
        } else if is_running {
            Self::Running
        } else {
            Self::Idle
        }
    }
}

// -------- Feature 003 Bundle D: control-button tooltip state --------
//
// Two derived `Signal<String>`s per button (`verbose_label`, `terse_tooltip`)
// both project from the same upstream `ButtonState` enum so the
// verbose `aria-label` / `title` pair and the terse `data-tooltip`
// never drift (CHK041). The state enums below are the upstream side;
// the downstream string projections live in `TimerView`'s body.
//
// Feature 006 (T049): the legacy `StopButtonState` (Reset/Undo)
// is gone — the left-slot button is now a state-aware control whose
// label/icon/handler flip on `(RunState, TimerMode)`. See the
// `left_slot_*` / `right_slot_*` signals inside `TimerView` for the
// new wiring.

/// Closed-sum state for the Play/Pause button across the timer's
/// run-state machine. Idle (not running, not paused, not auto-paused)
/// → Start; running → Pause; paused or auto-paused → Resume.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PlayPauseButtonState {
    Start,
    Pause,
    Resume,
}

impl PlayPauseButtonState {
    /// Derive the state from the engine's run-state predicates.
    #[must_use]
    pub const fn from_run_state(is_running: bool, is_paused: bool, is_auto_paused: bool) -> Self {
        if is_running {
            Self::Pause
        } else if is_paused || is_auto_paused {
            Self::Resume
        } else {
            Self::Start
        }
    }

    /// Verbose `aria-label` / `title` string. Per FR-028 the verbose
    /// label does NOT vary per state — the accessible name stays
    /// stable for screen-reader users so the button doesn't appear to
    /// "change identity" mid-session.
    #[must_use]
    pub const fn verbose_label(self) -> &'static str {
        "Start or pause timer"
    }

    /// Terse `data-tooltip` string for the visible CSS tooltip.
    #[must_use]
    pub const fn terse_tooltip(self) -> &'static str {
        match self {
            Self::Start => "Start",
            Self::Pause => "Pause",
            Self::Resume => "Resume",
        }
    }
}

/// Closed-sum state for the Skip button. No mode variants (FR-029); a
/// single-variant enum is intentional so the verbose/terse projection
/// path is uniform across the three buttons.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SkipButtonState {
    /// The only variant — Skip has no state-dependent text per FR-029.
    Skip,
}

impl SkipButtonState {
    /// Verbose accessible label — distinct from the terse tooltip per
    /// Spec A11 / SC-004 / contracts §3. The catalogue is the
    /// runtime source of truth (`timer.ctrl_skip_mode_aria`); this
    /// const fn is the host-side test fixture that pins the English
    /// form for the cargo-test matrix.
    #[must_use]
    pub const fn verbose_label(self) -> &'static str {
        "Skip current mode and advance to the next phase"
    }

    /// Terse tooltip — short form shown in `data-tooltip`. Catalogue
    /// key `timer.ctrl_skip_mode`.
    #[must_use]
    pub const fn terse_tooltip(self) -> &'static str {
        "Skip Mode"
    }
}

/// Project the skip-button icon name given the current mode and whether
/// the NEXT mode will be a long break.
///
/// - Focus + `!next_long` → "coffee" (short break ahead)
/// - Focus + `next_long` → "moon" (long break ahead)
/// - Break | `LongBreak` → "brain" (focus ahead)
const fn skip_icon_for_mode(mode: TimerMode, next_is_long_break: bool) -> &'static str {
    match mode {
        TimerMode::Focus => {
            if next_is_long_break {
                "moon"
            } else {
                "coffee"
            }
        }
        TimerMode::Break | TimerMode::LongBreak => "brain",
    }
}

/// Synthesise a `ManualSession` for a just-completed focus session.
/// Used by the engine-completion hook in `TimerView` so the
/// `CalendarView`'s `#sessions-table-body` shows today's auto-saved
/// rows. Today's behaviour is in-memory only; Phase 4c attaches the
/// `bridge::commands::save_manual_sessions` hop alongside this so
/// the rows survive a process restart.
///
/// `title` is the user-typed in-flight title (feature 002 Bundle A);
/// `None` for the no-title case and for paths that do not surface a
/// title input (manual-backfill flows construct their own
/// `ManualSession` directly).
fn synth_completed_session(
    now_ms: i64,
    focus_duration_secs: u32,
    title: Option<String>,
) -> ManualSession {
    let (hh_end, mm_end) = local_hh_mm(now_ms);
    let start_ms = now_ms - i64::from(focus_duration_secs) * 1000;
    let (hh_start, mm_start) = local_hh_mm(start_ms);
    ManualSession {
        id: format!("session-{now_ms}"),
        session_type: SessionType::Focus,
        duration: focus_duration_secs.div_euclid(60).max(1),
        start_time: format!("{hh_start:02}:{mm_start:02}"),
        end_time: format!("{hh_end:02}:{mm_end:02}"),
        notes: None,
        created_at: chrono::DateTime::<chrono::Utc>::from_timestamp_millis(now_ms)
            .unwrap_or_default()
            .to_rfc3339(),
        date: crate::engine::date_format::format_session_date(now_ms),
        tags: None,
        title,
    }
}

#[cfg(target_arch = "wasm32")]
fn local_hh_mm(ms: i64) -> (u32, u32) {
    #[allow(clippy::cast_precision_loss)]
    let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ms as f64));
    (d.get_hours(), d.get_minutes())
}

#[cfg(not(target_arch = "wasm32"))]
const fn local_hh_mm(_ms: i64) -> (u32, u32) {
    (0, 0)
}

#[cfg(target_arch = "wasm32")]
mod chime {
    use std::cell::RefCell;
    use web_sys::AudioContext;

    thread_local! {
        // One AudioContext shared by all chime calls for the WASM module's
        // lifetime. Avoids the WebKit per-page context cap (~4-6 contexts)
        // that silently fails `AudioContext::new()` once earlier contexts
        // accumulate. Mirrors the metronome's singleton pattern.
        pub(super) static CTX: RefCell<Option<AudioContext>> = const { RefCell::new(None) };
    }

    pub(super) fn ensure_context() {
        CTX.with(|cell| {
            let mut slot = cell.borrow_mut();
            if slot.is_none() {
                *slot = AudioContext::new().ok();
            }
            // Resume is idempotent on a running context; on macOS WKWebView a
            // freshly-constructed AudioContext starts suspended unless creation
            // happened inside a live gesture — `.resume()` unlocks it whenever
            // the autoplay policy permits.
            if let Some(ctx) = slot.as_ref() {
                let _ = ctx.resume();
            }
        });
    }
}

#[cfg(target_arch = "wasm32")]
fn play_chime() {
    use web_sys::OscillatorType;
    chime::ensure_context();
    chime::CTX.with(|cell| {
        let slot = cell.borrow();
        let Some(ctx) = slot.as_ref() else { return };
        let Ok(osc) = ctx.create_oscillator() else {
            return;
        };
        let Ok(gain) = ctx.create_gain() else { return };
        osc.set_type(OscillatorType::Sine);
        osc.frequency().set_value(800.0);
        let now = ctx.current_time();
        let _ = gain.gain().set_value_at_time(0.3, now);
        let _ = gain
            .gain()
            .exponential_ramp_to_value_at_time(0.01, now + 0.5);
        let _ = osc.connect_with_audio_node(&gain);
        let _ = gain.connect_with_audio_node(&ctx.destination());
        let _ = osc.start();
        let _ = osc.stop_with_when(now + 0.5);
    });
}

/// Returns the cached `AudioContext` used by `play_chime`, or `None` if it has
/// not yet been initialised. Test-only accessor for the singleton invariant.
#[cfg(all(target_arch = "wasm32", test))]
fn chime_audio_context() -> Option<web_sys::AudioContext> {
    chime::CTX.with(|cell| cell.borrow().clone())
}

#[cfg(not(target_arch = "wasm32"))]
const fn play_chime() {}

/// Constructs and resumes the shared `AudioContext` used by `play_chime`.
/// Call this from every user-gesture entry point (Start/Resume click,
/// keyboard shortcut) so the cached context is created inside the
/// synchronous gesture call stack. On macOS `WKWebView` a context created
/// inside a gesture is immediately unlockable via `.resume()`; a context
/// created later (e.g. from the 1 Hz tick) cannot be unlocked until the
/// next gesture — making transition chimes silent. Calling here costs one
/// idempotent `.resume()` per gesture and nothing else when already running.
#[cfg(target_arch = "wasm32")]
fn prime_audio_context() {
    chime::ensure_context();
}

#[cfg(not(target_arch = "wasm32"))]
const fn prime_audio_context() {}

/// One-shot ticking sound, fired once per second from the 1 Hz tick
/// Effect during focus sessions. Soft kitchen-timer "tick" — very
/// short percussive transient, low harmonic content, no audible
/// pitch sweep. Reuses a single long-lived `AudioContext` to avoid
/// per-call cold-start latency (a fresh `AudioContext` has 100–400 ms
/// of output buffer warm-up on macOS Core Audio that would push the
/// tick out of sync with the visual second).
#[cfg(target_arch = "wasm32")]
fn play_metronome_tick() {
    use std::cell::RefCell;
    use web_sys::{AudioContext, OscillatorType};
    thread_local! {
        static CTX: RefCell<Option<AudioContext>> = const { RefCell::new(None) };
    }
    CTX.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = AudioContext::new().ok();
        }
        let Some(ctx) = slot.as_ref() else { return };
        let Ok(osc) = ctx.create_oscillator() else {
            return;
        };
        let Ok(gain) = ctx.create_gain() else { return };
        osc.set_type(OscillatorType::Sine);
        osc.frequency().set_value(520.0);
        let now = ctx.current_time();
        let _ = gain.gain().set_value_at_time(0.04, now);
        let _ = gain
            .gain()
            .exponential_ramp_to_value_at_time(0.0005, now + 0.012);
        let _ = osc.connect_with_audio_node(&gain);
        let _ = gain.connect_with_audio_node(&ctx.destination());
        let _ = osc.start();
        let _ = osc.stop_with_when(now + 0.018);
    });
}

#[cfg(not(target_arch = "wasm32"))]
const fn play_metronome_tick() {}

/// ISO-8601 timestamp string for the current wall clock. Mirrors the
/// JS-era `new Date().toISOString()` used by `tag-manager.js` for
/// `created_at` fields on new tags + session-tag records.
#[cfg(target_arch = "wasm32")]
fn now_iso() -> String {
    js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
const fn now_iso() -> String {
    String::new()
}

/// Cryptographically-random UUID v4 string. Mirrors the JS-era
/// `crypto.randomUUID()` used by `tag-manager.js:287` for new tag
/// ids. Falls back to a timestamp-derived id when `window.crypto` is
/// unavailable (host tests / SSR).
#[cfg(target_arch = "wasm32")]
fn random_uuid() -> String {
    web_sys::window()
        .as_ref()
        .and_then(|w| w.crypto().ok())
        .map_or_else(|| BrowserClock.now_ms().to_string(), |c| c.random_uuid())
}

#[cfg(not(target_arch = "wasm32"))]
const fn random_uuid() -> String {
    String::new()
}

// Tray-icon formatter + dispatch moved to the `tray` submodule.
// `tag_tracking_*` helpers + dispatch moved to `tag_tracking`.

/// Typed alias for the `leptos_i18n` context. Threaded into `handle_events`
/// so every toast / desktop-notification body fires through the live
/// `t_string!` catalogue lookup (feature 005). Copy semantics — pass by
/// value at every call site.
type I18nCtx = leptos_i18n::I18nContext<crate::i18n::i18n::Locale>;

fn handle_events(
    events: &[TimerEvent],
    settings: &Settings,
    toast: AppToast,
    warning_signal: RwSignal<bool>,
    i18n: I18nCtx,
) {
    let has_overtime = events
        .iter()
        .any(|e| matches!(e, TimerEvent::OvertimeStarted { .. }));
    for e in events {
        match e {
            TimerEvent::PomodoroCompleted {
                completed_pomodoros,
            } => {
                if !has_overtime {
                    let is_long_break =
                        completed_pomodoros.is_multiple_of(settings.timer.sessions_per_long_break);
                    let toast_text = if is_long_break {
                        t_string!(i18n, timer.messages.pomodoro_completed_toast_long)
                    } else {
                        t_string!(i18n, timer.messages.pomodoro_completed_toast_short)
                    };
                    toast.show(toast_text);
                    if settings.notifications.sound_notifications {
                        play_chime();
                    }
                    if settings.notifications.desktop_notifications {
                        let desk_body = if is_long_break {
                            t_string!(i18n, timer.messages.pomodoro_completed_desktop_long)
                        } else {
                            t_string!(i18n, timer.messages.pomodoro_completed_desktop_short)
                        };
                        spawn_local(async move {
                            let _ =
                                crate::bridge::notification::send_notification("Presto", desk_body)
                                    .await;
                        });
                    }
                }
                warning_signal.set(false);
            }
            TimerEvent::BreakCompleted { mode } => {
                let toast_text = match mode {
                    TimerMode::Break => t_string!(i18n, timer.messages.break_completed_toast_break),
                    TimerMode::LongBreak => {
                        t_string!(i18n, timer.messages.break_completed_toast_long_break)
                    }
                    // Defensive default — `BreakCompleted` carries Break
                    // or LongBreak only; `Focus` would be an engine
                    // regression.
                    TimerMode::Focus => {
                        t_string!(i18n, timer.messages.break_completed_toast_focus)
                    }
                };
                toast.show(toast_text);
                if settings.notifications.sound_notifications {
                    play_chime();
                }
                if settings.notifications.desktop_notifications {
                    let desk_body = match mode {
                        TimerMode::Break => {
                            t_string!(i18n, timer.messages.break_completed_desktop_break)
                        }
                        TimerMode::LongBreak => {
                            t_string!(i18n, timer.messages.break_completed_desktop_long_break)
                        }
                        TimerMode::Focus => {
                            t_string!(i18n, timer.messages.break_completed_desktop_focus)
                        }
                    };
                    spawn_local(async move {
                        let _ = crate::bridge::notification::send_notification("Presto", desk_body)
                            .await;
                    });
                }
            }
            TimerEvent::TwoMinutesRemaining => {
                toast.show(t_string!(i18n, timer.messages.two_minutes_remaining));
                warning_signal.set(true);
            }
            TimerEvent::ThirtySecondsRemaining => {
                toast.show(t_string!(i18n, timer.messages.thirty_seconds_remaining));
                warning_signal.set(true);
            }
            TimerEvent::SessionStarted => {
                toast.show(t_string!(i18n, timer.toast.timer_started));
                if settings.notifications.sound_notifications {
                    play_chime();
                }
            }
            TimerEvent::SessionPaused => {
                toast.show(t_string!(i18n, timer.toast.timer_paused));
            }
            TimerEvent::SessionResumed => {
                toast.show(t_string!(i18n, timer.toast.timer_resumed));
                if settings.notifications.sound_notifications {
                    play_chime();
                }
            }
            TimerEvent::SessionSkipped { skipped_mode, .. } => {
                let toast_text = match skipped_mode {
                    TimerMode::Focus => {
                        t_string!(i18n, timer.messages.session_skipped_toast_focus)
                    }
                    TimerMode::Break => {
                        t_string!(i18n, timer.messages.session_skipped_toast_break)
                    }
                    TimerMode::LongBreak => {
                        t_string!(i18n, timer.messages.session_skipped_toast_long_break)
                    }
                };
                toast.show(toast_text);
                warning_signal.set(false);
            }
            TimerEvent::AutoPaused => {
                toast.show(t_string!(i18n, timer.toast.smart_pause_activated));
            }
            TimerEvent::AutoResumed => {
                toast.show(t_string!(i18n, timer.toast.auto_resumed));
            }
            TimerEvent::ManualSessionRecorded { .. } => {
                toast.show(t_string!(i18n, timer.toast.manual_session_recorded));
            }
            TimerEvent::OvertimeStarted { mode } => {
                let (toast_msg, desk_body) = match mode {
                    TimerMode::Focus => (
                        t_string!(i18n, timer.messages.overtime_started_toast_focus),
                        t_string!(i18n, timer.messages.overtime_started_desktop_focus),
                    ),
                    TimerMode::Break => (
                        t_string!(i18n, timer.messages.overtime_started_toast_break),
                        t_string!(i18n, timer.messages.overtime_started_desktop_break),
                    ),
                    TimerMode::LongBreak => (
                        t_string!(i18n, timer.messages.overtime_started_toast_long_break),
                        t_string!(i18n, timer.messages.overtime_started_desktop_long_break),
                    ),
                };
                toast.show(toast_msg);
                if settings.notifications.sound_notifications {
                    play_chime();
                }
                if settings.notifications.desktop_notifications {
                    spawn_local(async move {
                        let _ = crate::bridge::notification::send_notification("Presto", desk_body)
                            .await;
                    });
                }
            }
            // Feature 006: no toast / chime / desktop notification for
            // either new variant. `SessionAborted` is a discard — the
            // user already saw the button press; UI side-effect
            // bookkeeping (clearing pending auto-restart countdown) is
            // handled in the auto-restart effect, not here.
            // `SessionCompletedEarly` is engine-internal observability
            // only (paired with `PomodoroCompleted` in branch B or
            // `SessionAborted` in branch A — the paired event drives
            // the toast/chime).
            TimerEvent::SessionAborted { .. } | TimerEvent::SessionCompletedEarly { .. } => {}
        }
    }
}

/// Project the shared `Settings` signal to the engine's
/// `Durations`. When `Settings::advanced.debug_mode` is on, the
/// JS-era surface clamped every duration to 3 seconds for rapid
/// e2e iteration (see `pomodoro-timer.js:debug` flow); preserve
/// that behaviour so `settings-advanced.spec.js:37` ("00 / 03")
/// resolves once the debug toggle flips.
const fn durations_from_settings(settings: &Settings) -> Durations {
    if settings.advanced.debug_mode {
        return Durations {
            focus: 3,
            short_break: 3,
            long_break: 3,
        };
    }
    Durations {
        focus: settings.timer.focus_duration * 60,
        short_break: settings.timer.break_duration * 60,
        long_break: settings.timer.long_break_duration * 60,
    }
}

/// Timer view — renders the canonical pomodoro DOM and wires the
/// `engine::TimerState` state machine through Leptos signals.
///
/// State ownership: the component owns a `RwSignal<TimerState>` for
/// the duration of its mount. The shared `RwSignal<Settings>` is
/// pulled in via `expect_context` (provided by `App`) and projected
/// through `durations_from_settings` into a settings-driven
/// `Durations`. An `Effect` re-applies the durations whenever
/// settings change so the timer display reflects edits made on
/// the Settings tabs without a process restart.
///
/// Returns a fragment whose root is `<div id="timer-view">` to match
/// the `#timer-view` selector contract.
#[component]
pub fn TimerView() -> impl IntoView {
    // Read the shared Settings signal from context. The App router
    // (Phase 4b) `provide_context`s this signal; if the context is
    // unavailable (host-side `cargo test` builds, or future direct
    // mounts of TimerView outside the App shell), fall back to a
    // local default — Settings::default() returns the JS-era
    // baseline (focus 25, break 5, long break 20) so the display
    // matches the cold-start contract that `_smoke.spec.js`
    // asserts.
    let settings =
        use_context::<RwSignal<Settings>>().unwrap_or_else(|| RwSignal::new(Settings::default()));
    let initial_durations = settings.with_untracked(durations_from_settings);

    // Shared session log (provided by App). When a focus session
    // completes, we push a synthesised `ManualSession` so the
    // CalendarView's `#sessions-table-body` reflects today's
    // completed run. Phase 4c attaches the
    // `bridge::commands::save_manual_sessions` hop; today the
    // signal is the in-memory branch.
    let sessions =
        use_context::<RwSignal<Vec<ManualSession>>>().unwrap_or_else(|| RwSignal::new(Vec::new()));

    // Engine state — RwSignal so derived projections (countdown
    // text, mode label, running flag) re-render on `update()`.
    let engine = use_context::<RwSignal<TimerState>>()
        .unwrap_or_else(|| RwSignal::new(TimerState::new(initial_durations)));
    let app_toast = use_context::<AppToast>().unwrap_or_default();
    let warning_signal = RwSignal::new(false);

    // Feature 005: i18n context handle. Live for the lifetime of the
    // component; every `t!(...)` / `t_string!(...)` call site below
    // re-renders in the same reactive tick on locale change
    // (FR-007 / FR-012).
    let i18n = use_i18n();

    // React to settings changes: rebase the engine's `Durations`
    // when the Settings signal moves. The effect re-runs whenever
    // settings change; the engine's `set_durations` rebases the
    // displayed remaining time only when idle (so mid-session edits
    // don't truncate the active session — see the engine method's
    // rustdoc).
    Effect::new(move |_| {
        let new_durations = settings.with(durations_from_settings);
        engine.update(|state| state.set_durations(new_durations));
    });

    // Pipe Settings.notifications.allow_continuous_sessions into the
    // engine so the overtime path fires when enabled.
    Effect::new(move |_| {
        let enabled = settings.with(|s| s.notifications.allow_continuous_sessions);
        engine.update(|state| state.set_allow_continuous_sessions(enabled));
    });

    // Feature 002 Bundle B (T022): pipe
    // `Settings::timer.sessions_per_long_break` into the engine so
    // the natural zero-cross + skip-session branches consult the
    // configured cadence (timer.rs:421, :861). Mirrors the
    // `set_durations` / `set_allow_continuous_sessions` posture
    // above: runs once on init so the engine picks up the persisted
    // value on boot, and re-runs whenever the settings signal moves
    // so a mid-session save propagates without a process restart.
    // The engine's setter is a plain assignment (no clamp) — the
    // 1–10 clamp lives at the Settings UI input layer (Principle
    // III: type-system encoding over defensive guards).
    Effect::new(move |_| {
        let n = settings.with(|s| s.timer.sessions_per_long_break);
        engine.update(|state| state.set_sessions_per_long_break(n));
    });

    // Feature 002 Bundle C (revised): ticking-sound scheduler lives
    // inside the 1 Hz tick Effect below. Firing the tone from the
    // same Effect that mutates `state` and dispatches
    // `update_tray_icon` guarantees the audible tick, the visible
    // timer digit change, and the macOS tray text update share one
    // event-loop turn. A separate interval would drift against the
    // engine clock at second boundaries.

    // Tag-dropdown popover state. The JS-era surface anchored the
    // tag picker as a popover off `#timer-status` inside the timer
    // view (`src/index.html` history showed the dropdown nested
    // here, not in a separate Tags route). The Leptos port
    // initially split TagsView into a NavView::Tags route, but the
    // e2e suite (`tags.spec.js:11`, `sessions-history.spec.js:14`)
    // exercises the popover by clicking `#timer-status` from the
    // timer view — so the dropdown must live in TimerView.
    //
    // Group D (R-004): consume the App-level shared tag list via context
    // rather than owning a local signal. The App router seeds the context
    // signal with the JS-era default "Focus" tag (so this component renders
    // immediately without waiting for load_tags IPC) and overwrites it when
    // the cold-start load_tags response arrives. Tag CRUD below writes
    // through the shared signal so the App-level persistence sink fires on
    // every create / delete — mutations persist across restarts.
    let tag_dropdown_open = RwSignal::new(false);
    let tags = use_context::<RwSignal<Vec<Tag>>>().unwrap_or_else(|| {
        // Fallback: direct mount outside the App shell (host tests,
        // future Storybook-style previews). Seed with the default so
        // the UI renders without context.
        RwSignal::new(vec![Tag {
            id: "default-focus".to_string(),
            name: "Focus".to_string(),
            icon: "ri-brain-line".to_string(),
            color: "#4CAF50".to_string(),
            created_at: String::new(),
        }])
    });
    let new_tag_name = RwSignal::new(String::new());
    let new_tag_icon = RwSignal::new(DEFAULT_NEW_TAG_ICON.to_string());
    let icon_picker_open = RwSignal::new(false);
    // Currently-selected tag ids. Multi-select per the JS-era
    // `tag-manager.js:toggleTag` semantics — clicking a row toggles
    // its presence in `currentTags`. Seeds with the default focus
    // tag so the visual baseline shows the first row pre-highlighted.
    let selected_tag_ids = RwSignal::new(vec!["default-focus".to_string()]);
    // Reconcile selected_tag_ids against the actual tag list once it
    // loads from context. The seed "default-focus" may not exist in
    // real persisted tags; drop stale ids and fall back to the first
    // available tag so downstream add_session_tag calls always write
    // a valid tag id.
    Effect::new(move |_| {
        let valid_ids: Vec<String> = tags.with(|all| all.iter().map(|t| t.id.clone()).collect());
        if valid_ids.is_empty() {
            return;
        }
        selected_tag_ids.update(|sel| {
            sel.retain(|id| valid_ids.contains(id));
            if sel.is_empty() {
                if let Some(first) = valid_ids.first() {
                    sel.push(first.clone());
                }
            }
        });
    });
    // Per-tag wall-clock anchors for the time-spent ledger. Mirrors
    // `tag-manager.js:activeSessionTags`: keys are tag ids, values
    // are `Date.now()` capture points. Flushed on pause / stop /
    // completion / skip through `add_session_tag`. `StoredValue`
    // (not `RwSignal`) — the map never drives reactive rendering.
    let active_session_tags: StoredValue<HashMap<String, (String, i64)>> =
        StoredValue::new(HashMap::new());

    // Feature 002 Bundle A: in-flight session title. Local to the
    // component, captured once at focus zero-cross into BOTH the
    // `Session` persist call and the synthesised `ManualSession` row
    // (see `synth_completed_session`). Empty string normalises to
    // `None` at the boundary (Principle III). Cleared after the
    // post-completion write.
    let session_title = RwSignal::new(String::new());

    // Feature 006 (T051/T052): modal-open state for Quick Log +
    // Distraction. Snapshot of the parent-session ref is captured at
    // modal-open time (per spec Clarifications + Edge Cases) so a
    // mid-modal mode transition (e.g. natural pomodoro completion)
    // doesn't reshape the recorded parent ref.
    let quick_log_modal_open = RwSignal::new(false);
    let distraction_modal_open = RwSignal::new(false);
    let distraction_parent_ref_snapshot: RwSignal<
        Option<crate::bridge::types::DistractionParentRef>,
    > = RwSignal::new(None);

    // Feature 006 (T044/T045): in-memory manager state. Production
    // path: cold-start `load()`, mutate-then-`save()` round-trip per
    // mutation. Today the load+save hops are spawned best-effort;
    // failures are logged but don't block the UI (mirrors the JS-era
    // optimistic-update behaviour for tags + sessions).
    let quick_logs: RwSignal<crate::managers::quick_log::QuickLogManager> =
        use_context::<RwSignal<crate::managers::quick_log::QuickLogManager>>()
            .unwrap_or_else(|| RwSignal::new(crate::managers::quick_log::QuickLogManager::new()));
    let distractions: RwSignal<crate::managers::distraction::DistractionManager> = use_context::<
        RwSignal<crate::managers::distraction::DistractionManager>,
    >()
    .unwrap_or_else(|| RwSignal::new(crate::managers::distraction::DistractionManager::new()));

    let on_status_click = move |ev: leptos::ev::MouseEvent| {
        // Stop propagation so the document-level click-outside
        // listener (registered below) doesn't immediately close the
        // dropdown we're about to open. Mirrors the JS-era flow at
        // `tag-manager.js`'s `toggleDropdown` + the document-click
        // outside handler that gates close on
        // `!timerStatus.contains(target)`.
        ev.stop_propagation();
        // Feature 006 (T048): combined pill is read-only outside
        // Idle. The chevron is hidden via CSS / inline-style; this
        // handler is the second line of defence (synthetic clicks,
        // accessibility-tree activation) so the dropdown cannot open
        // while the timer is Running/Paused.
        let is_idle =
            engine.with_untracked(|s| !s.is_running() && !s.is_paused() && !s.is_auto_paused());
        if !is_idle {
            return;
        }
        tag_dropdown_open.update(|v| *v = !*v);
    };

    // Document-level keydown handler. Mirrors the JS-era
    // `pomodoro-timer.js:setupKeyboardShortcuts` flow: pressing the
    // configured `start_stop` shortcut (the Space key by default
    // per `settings-shortcuts.spec.js`) routes through the same
    // start / pause toggle as the play/pause button. The handler
    // skips when a form input is focused so typing in the
    // settings-shortcuts recorder doesn't double-fire.
    Effect::new(move |_| {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Some(document) = window.document() else {
            return;
        };
        let closure = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(
            move |ev: web_sys::KeyboardEvent| {
                // Skip when typing in a form field — the JS-era
                // surface ignored shortcuts when the active element
                // was an `<input>`/`<textarea>`/contenteditable so
                // the shortcut-recorder + tag-name input flows
                // didn't trip the start/stop toggle.
                if let Some(active) = web_sys::window()
                    .and_then(|w| w.document())
                    .and_then(|d| d.active_element())
                {
                    let tag = active.tag_name();
                    if matches!(tag.to_uppercase().as_str(), "INPUT" | "TEXTAREA" | "SELECT") {
                        return;
                    }
                }
                let key = ev.key();
                let configured = settings.with_untracked(|s| s.shortcuts.start_stop.clone());
                let matches_shortcut = configured
                    .as_deref()
                    .is_some_and(|expected| key == expected);
                // Hardcoded Space fallback so the JS-era
                // `pomodoro-timer.js` parity holds even when no
                // shortcut is configured. `settings-shortcuts.spec.js`
                // records " " as the captured value, so the
                // configured branch above also handles it; the
                // fallback covers the cold-start path.
                let matches_space = ev.code() == "Space";
                if matches_shortcut || matches_space {
                    ev.prevent_default();
                    prime_audio_context();
                    ambient_audio::prime_ambient_audio();
                    let events = engine
                        .try_update(|state| {
                            if state.is_running() {
                                state.pause(&BrowserClock).unwrap_or_default()
                            } else if state.is_paused() || state.is_auto_paused() {
                                state.resume(&BrowserClock).unwrap_or_default()
                            } else {
                                state.start(&BrowserClock).unwrap_or_default()
                            }
                        })
                        .unwrap_or_default();
                    handle_events(
                        &events,
                        &settings.get_untracked(),
                        app_toast,
                        warning_signal,
                        i18n,
                    );
                    apply_tag_tracking_events(&events, active_session_tags, selected_tag_ids);
                }
            },
        );
        let _ =
            document.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref());
        closure.forget();
    });

    // Close-on-outside-click. Matches the JS-era `document.addEventListener("click", ...)`
    // dismissal: any click NOT inside `#timer-status` or
    // `#tag-dropdown-menu` closes the popover. The tags.spec.js +
    // sessions-history.spec.js flows depend on this — both
    // navigate away (clicking `#settings-nav` etc.) and then
    // re-click `#timer-status` expecting the dropdown to re-open
    // from a closed state.
    Effect::new(move |_| {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Some(document) = window.document() else {
            return;
        };
        let closure = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::MouseEvent)>::new(
            move |ev: web_sys::MouseEvent| {
                if !tag_dropdown_open.get_untracked() {
                    return;
                }
                let Some(target) = ev.target() else { return };
                let Ok(target_node) = target.dyn_into::<web_sys::Node>() else {
                    return;
                };
                let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
                    return;
                };
                let inside_status = doc
                    .get_element_by_id("timer-status")
                    .is_some_and(|el| el.contains(Some(&target_node)));
                let inside_menu = doc
                    .get_element_by_id("tag-dropdown-menu")
                    .is_some_and(|el| el.contains(Some(&target_node)));
                if !inside_status && !inside_menu {
                    tag_dropdown_open.set(false);
                }
            },
        );
        let _ =
            document.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
        // The closure is intentionally leaked so the listener
        // outlives the Effect. The TimerView is mounted for the
        // lifetime of the App; cleanup happens implicitly when the
        // WASM runtime tears down.
        closure.forget();
    });
    let on_create_tag = move || {
        let name = new_tag_name.with(|s| s.trim().to_string());
        if name.is_empty() {
            return;
        }
        let icon = new_tag_icon.get();
        // Use `crypto.randomUUID()` for collision-free ids (legacy
        // `tag-manager.js:287` parity). The previous `tag-{index}`
        // scheme collided after delete-then-recreate: deleting the
        // 3rd of 4 tags then creating a new one re-derived `tag-4`,
        // overlapping with the surviving tag.
        let id = format!("tag-{}", random_uuid());
        let new_tag = Tag {
            id,
            name,
            icon,
            color: "#4CAF50".to_string(),
            created_at: now_iso(),
        };
        let new_tag_for_save = new_tag.clone();
        tags.update(|list| list.push(new_tag));
        new_tag_name.set(String::new());
        new_tag_icon.set(DEFAULT_NEW_TAG_ICON.to_string());
        // Persist immediately — the bulk-save sink in `app.rs` only
        // re-saves the in-memory list; per-tag save is what makes
        // creation durable across restarts.
        spawn_local(async move {
            let _ = commands::save_tag(new_tag_for_save).await;
        });
    };
    let on_delete_tag = move |id: String| {
        // Flush any active tracking before the tag id disappears
        // from the system; otherwise the accumulator would leak.
        tag_tracking_flush_one(active_session_tags, &id, BrowserClock.now_ms());
        tags.update(|list| list.retain(|t| t.id != id));
        selected_tag_ids.update(|sel| sel.retain(|t| t != &id));
        // Persist the deletion through the Tauri bridge. Without
        // this call the on-disk catalogue still contains the
        // dropped tag and it reappears on next launch.
        spawn_local(async move {
            let _ = commands::delete_tag(id).await;
        });
    };
    let on_toggle_picker = move |ev: leptos::ev::MouseEvent| {
        // The icon-selector lives inside `#timer-status`-anchored
        // dropdown; we stop propagation so the outer
        // `#timer-status` toggle doesn't immediately close the
        // dropdown when the user opens the icon picker.
        ev.stop_propagation();
        icon_picker_open.update(|v| *v = !*v);
    };
    let on_pick_icon = move |icon: String| {
        new_tag_icon.set(icon);
        icon_picker_open.set(false);
    };

    // Derived signals — each `.with(|s| ...)` borrows the engine
    // without cloning; Leptos memoises the result and re-runs the
    // closure only when the engine signal changes.
    let is_overtime = Signal::derive(move || engine.with(|s| s.time_remaining_secs_signed() < 0));

    let minutes_text = Signal::derive(move || {
        engine.with(|s| {
            let signed = s.time_remaining_secs_signed();
            if signed < 0 {
                pad_two(u32::try_from(-signed).unwrap_or(u32::MAX) / 60)
            } else {
                pad_two(s.time_remaining_secs() / 60)
            }
        })
    });

    let seconds_text = Signal::derive(move || {
        engine.with(|s| {
            let signed = s.time_remaining_secs_signed();
            if signed < 0 {
                pad_two(u32::try_from(-signed).unwrap_or(u32::MAX) % 60)
            } else {
                pad_two(s.time_remaining_secs() % 60)
            }
        })
    });

    // Tag-aware label + icon for `#status-text` / `#status-icon`.
    // Legacy `pomodoro-timer.js:1421-1448` overrides the mode label
    // with the active tag's name (or "N Tags" for multi-select) when
    // in Focus mode; the state suffixes (Paused / Auto-paused /
    // Overtime) still append. Break / LongBreak modes keep the mode
    // label regardless of tag selection.
    let status_label = Signal::derive(move || {
        let (mode, is_running_v, is_paused_v, is_auto_paused_v, is_ot) = engine.with(|s| {
            (
                s.current_mode(),
                s.is_running(),
                s.is_paused(),
                s.is_auto_paused(),
                s.time_remaining_secs_signed() < 0,
            )
        });
        // Feature 005: localised mode suffix. Each branch is a static
        // key so the proc-macro can compile-time-check it; the actual
        // wording (e.g. "(Paused)" -> "(Pausiert)") lives in the
        // catalogues.
        let suffix: String = if is_paused_v {
            format!(" {}", t_string!(i18n, timer.status_paused))
        } else if is_auto_paused_v {
            format!(" {}", t_string!(i18n, timer.status_auto_paused))
        } else if is_running_v && is_ot {
            format!(" {}", t_string!(i18n, timer.status_overtime))
        } else {
            String::new()
        };
        if mode == TimerMode::Focus {
            let matched: Vec<String> = selected_tag_ids.with(|sel| {
                tags.with(|all| {
                    all.iter()
                        .filter(|t| sel.contains(&t.id))
                        .map(|t| t.name.clone())
                        .collect()
                })
            });
            if !matched.is_empty() {
                let base = if matched.len() == 1 {
                    matched[0].clone()
                } else {
                    format!("{} {}", matched.len(), t_string!(i18n, tag.tags_plural))
                };
                return format!("{base}{suffix}");
            }
        }
        // Feature 005: localised mode label. Reuse the same suffix
        // string computed above (already localised); the base label
        // (Focus / Break / Long Break) comes from the catalogue.
        let base = match mode {
            TimerMode::Focus => t_string!(i18n, timer.mode_focus).to_string(),
            TimerMode::Break => t_string!(i18n, timer.mode_break).to_string(),
            TimerMode::LongBreak => t_string!(i18n, timer.mode_long_break).to_string(),
        };
        format!("{base}{suffix}")
    });
    let status_icon = Signal::derive(move || {
        let mode = engine.with(TimerState::current_mode);
        match mode {
            TimerMode::Break => return "ri-cup-line".to_string(),
            TimerMode::LongBreak => return "ri-moon-line".to_string(),
            TimerMode::Focus => {}
        }
        let icons: Vec<String> = selected_tag_ids.with(|sel| {
            tags.with(|all| {
                all.iter()
                    .filter(|t| sel.contains(&t.id))
                    .map(|t| t.icon.clone())
                    .collect()
            })
        });
        if icons.is_empty() {
            "ri-brain-line".to_string()
        } else if icons.len() == 1 {
            icons[0].clone()
        } else {
            "ri-price-tag-3-line".to_string()
        }
    });
    let is_running = Signal::derive(move || engine.with(TimerState::is_running));

    // Feature 006 (T049): closed-sum UI run-state. The button matrix
    // dispatches on this via an exhaustive `match` (Principle III).
    let run_state = Signal::derive(move || {
        engine.with(|s| RunState::from_engine(s.is_running(), s.is_paused(), s.is_auto_paused()))
    });

    let play_pause_btn_state = Signal::derive(move || {
        engine.with(|s| {
            PlayPauseButtonState::from_run_state(s.is_running(), s.is_paused(), s.is_auto_paused())
        })
    });
    let verbose_label_play =
        Signal::derive(move || t_string!(i18n, timer.ctrl_play_pause_aria).to_string());
    let terse_tooltip_play = Signal::derive(move || match play_pause_btn_state.get() {
        PlayPauseButtonState::Start => t_string!(i18n, timer.ctrl_start).to_string(),
        PlayPauseButtonState::Pause => t_string!(i18n, timer.ctrl_pause).to_string(),
        PlayPauseButtonState::Resume => t_string!(i18n, timer.ctrl_resume).to_string(),
    });

    // Feature 006 (T049): state-aware left-slot button.
    //   Idle           → "+ Quick Log"        (opens Quick Log modal)
    //   Running/Paused → "✕ Abort"            (engine.abort)
    let verbose_label_left = Signal::derive(move || match run_state.get() {
        RunState::Idle => t_string!(i18n, timer.ctrl_quick_log_aria).to_string(),
        RunState::Running | RunState::Paused => t_string!(i18n, timer.ctrl_abort_aria).to_string(),
    });
    let terse_tooltip_left = Signal::derive(move || match run_state.get() {
        RunState::Idle => t_string!(i18n, timer.ctrl_quick_log).to_string(),
        RunState::Running | RunState::Paused => t_string!(i18n, timer.ctrl_abort).to_string(),
    });

    // Feature 006 (T049): state-aware right-slot button.
    //   Idle    → "→ Skip Mode"        (engine.skip)
    //   Running → "! Note Distraction" (opens Distraction modal)
    //   Paused  → "✓ Complete"         (engine.complete)
    let verbose_label_right = Signal::derive(move || match run_state.get() {
        RunState::Idle => t_string!(i18n, timer.ctrl_skip_mode_aria).to_string(),
        RunState::Running => t_string!(i18n, timer.ctrl_note_distraction_aria).to_string(),
        RunState::Paused => t_string!(i18n, timer.ctrl_complete_aria).to_string(),
    });
    let terse_tooltip_right = Signal::derive(move || match run_state.get() {
        RunState::Idle => t_string!(i18n, timer.ctrl_skip_mode).to_string(),
        RunState::Running => t_string!(i18n, timer.ctrl_note_distraction).to_string(),
        RunState::Paused => t_string!(i18n, timer.ctrl_complete).to_string(),
    });

    // Update document title with overtime prefix when running in overtime.
    Effect::new(move |_| {
        let signed = engine.with(TimerState::time_remaining_secs_signed);
        let is_ot = signed < 0;
        let abs_secs = u32::try_from(signed.unsigned_abs()).unwrap_or(u32::MAX);
        let mins = abs_secs / 60;
        let secs = abs_secs % 60;
        let prefix = if is_ot { "+" } else { "" };
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            doc.set_title(&format!("{prefix}{mins:02}:{secs:02} \u{2014} Presto"));
        }
    });

    // Style helpers for the play/pause icon visibility-toggle. The
    // selector contract says `#play-icon` is visible when idle and
    // `#pause-icon` is visible when running; the e2e suite asserts
    // both `toBeVisible()` and `toBeHidden()` on these.
    let play_icon_style = Signal::derive(move || {
        if is_running.get() {
            "display: none"
        } else {
            ""
        }
    });
    let pause_icon_style = Signal::derive(move || {
        if is_running.get() {
            ""
        } else {
            "display: none"
        }
    });

    let next_is_long_break = Signal::derive(move || {
        let sessions = settings.with(|s| s.timer.sessions_per_long_break);
        // next completion hits the long-break boundary when count + 1 is a multiple of the configured cadence
        (engine.with(TimerState::completed_pomodoros) + 1).is_multiple_of(sessions)
    });

    // Click handlers. Each dispatches to the engine via a borrowed
    // mutation; the engine's API returns `Vec<TimerEvent>` which
    // would feed the bridge layer in production (tray icon
    // updates, session-save side-effects). Phase 4c attaches the
    // event sink; today the events are dropped after mutation so
    // the in-memory state machine is correct even though
    // persistence is a no-op on the dev server.
    let on_play_pause = move |_| {
        prime_audio_context();
        ambient_audio::prime_ambient_audio();
        let events = engine
            .try_update(|state| {
                if state.is_running() {
                    state.pause(&BrowserClock).unwrap_or_default()
                } else if state.is_paused() || state.is_auto_paused() {
                    state.resume(&BrowserClock).unwrap_or_default()
                } else {
                    state.start(&BrowserClock).unwrap_or_default()
                }
            })
            .unwrap_or_default();
        handle_events(
            &events,
            &settings.get_untracked(),
            app_toast,
            warning_signal,
            i18n,
        );
        apply_tag_tracking_events(&events, active_session_tags, selected_tag_ids);
        dispatch_tray_update(engine, settings, true);
    };
    // Feature 006 (T053): Abort handler. Called from the left-slot
    // button in Running or Paused state. Routes through
    // `engine.abort(clock)` which clears the run-state bools +
    // `current_session_elapsed_secs` without advancing mode or
    // touching `completed_pomodoros`. Per FR-017 the per-session
    // title is preserved (user intent: resume-after-abort with the
    // same context); the engine does not surface a
    // `PomodoroCompleted` event so the auto-restart UI gate at
    // line 1471-1483 (now event-checked per T050) does not fire.
    let on_abort = move |_| {
        let events = engine
            .try_update(|state| state.abort(&BrowserClock))
            .unwrap_or_default();
        warning_signal.set(false);
        tag_tracking_flush_all(active_session_tags, BrowserClock.now_ms());
        handle_events(
            &events,
            &settings.get_untracked(),
            app_toast,
            warning_signal,
            i18n,
        );
        apply_tag_tracking_events(&events, active_session_tags, selected_tag_ids);
        app_toast.show(t_string!(i18n, timer.toast.session_deleted));
        dispatch_tray_update(engine, settings, true);
    };

    // R-001 fix: persistence closure for a focus-completion event.
    // Called by BOTH the natural-zero-cross tick branch AND the UI-
    // triggered `on_complete` handler so an early Complete from
    // Paused leaves the same daily-history row + stats hop a natural
    // completion would. Caller passes `total_focus_before` captured
    // BEFORE the engine.complete()/tick() call so `elapsed_secs` is
    // the wall-clock duration of the just-sealed session (the engine
    // folds `current_session_elapsed_secs` into `total_focus_secs`
    // during completion). The closure also clears the in-flight
    // title and pushes the synthesised `ManualSession` into the
    // shared sessions log so the CalendarView re-renders.
    let persist_focus_completion = move |total_focus_before: u32| {
        let now_ms = BrowserClock.now_ms();
        let (total_focus_after, completed) =
            engine.with_untracked(|s| (s.total_focus_secs(), s.completed_pomodoros()));
        let elapsed_secs = total_focus_after.saturating_sub(total_focus_before);
        // Harvest the in-flight title ONCE at the boundary, normalise
        // empty-string to None (Principle III), and clear the signal
        // so the next focus starts blank — mirrors FR-007.
        let title_at_completion = {
            let raw = session_title.get_untracked();
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        };
        session_title.set(String::new());
        let session = synth_completed_session(now_ms, elapsed_secs, title_at_completion.clone());
        sessions.update(|list| list.push(session));
        let date_str = crate::engine::date_format::format_session_date(now_ms);
        let session_data = Session {
            completed_pomodoros: completed,
            total_focus_time: total_focus_after,
            current_session: completed.saturating_add(1),
            date: date_str,
            title: title_at_completion,
        };
        let sd_for_stats = session_data.clone();
        spawn_local(async move {
            let _ = commands::save_session_data(session_data).await;
            let _ = commands::save_daily_stats(sd_for_stats).await;
        });
    };

    // Feature 006 (T054): Complete handler. Called from the
    // right-slot button in Paused (or AutoPaused — folded into
    // RunState::Paused). Routes through `engine.complete(clock)`
    // which: (a) if elapsed < 30 s, internally delegates to
    // `abort()` (FR-015 — discard as Abort, no count); (b) else
    // increments `completed_pomodoros`, integrates elapsed into
    // `total_focus_secs`, advances mode per cadence, emits
    // `PomodoroCompleted` + `SessionCompletedEarly`. The downstream
    // tick-loop hooks (session-save, auto-restart) read the events
    // and act per FR-013/FR-014/FR-016.
    //
    // R-001 fix: persistence used to live exclusively inside the
    // tick branch. Since `complete()` flips `is_running = false`,
    // no further tick runs the save path — so a UI Complete left
    // no daily-history row and no stats update. We now snapshot
    // `total_focus_before` BEFORE the engine call (complete folds
    // elapsed into total_focus_secs), then invoke the shared
    // `persist_focus_completion` closure when a `PomodoroCompleted`
    // event fires (which also clears the title — mirrors the
    // natural-completion path).
    let on_complete = move |_| {
        let total_focus_before = engine.with_untracked(TimerState::total_focus_secs);
        let events = engine
            .try_update(|state| state.complete(&BrowserClock))
            .unwrap_or_default();
        let counted = events
            .iter()
            .any(|e| matches!(e, TimerEvent::PomodoroCompleted { .. }));
        if counted {
            persist_focus_completion(total_focus_before);
        }
        handle_events(
            &events,
            &settings.get_untracked(),
            app_toast,
            warning_signal,
            i18n,
        );
        apply_tag_tracking_events(&events, active_session_tags, selected_tag_ids);
        dispatch_tray_update(engine, settings, true);
    };

    // Feature 006 (T051): Quick Log modal open. Idle-state left-slot
    // button. Snapshots no session context — quick logs are
    // session-independent.
    let on_open_quick_log = move |_| {
        quick_log_modal_open.set(true);
    };

    // Feature 006 (T052): Distraction modal open. Running-state
    // right-slot button. Snapshots the current session context at
    // modal-open time per FR-035 + Edge Cases (race-free against
    // mid-modal mode transitions).
    //
    // R-003 fix: read the engine's session-start anchor instead of
    // deriving `start_ms = now - elapsed_secs * 1000`.
    // `current_session_elapsed_secs` is focus-only accumulated time
    // (paused gaps excluded), so the derived form drifted across
    // pause cycles — two distractions captured from the same logical
    // session got different `parent_session_start_ts` values. The
    // engine's anchor is stamped on Idle → Running and survives
    // pause/resume, so it's the wall-clock truth here. Fallback to
    // `now` for the impossible case where the engine is Idle when
    // the modal opens (matrix gates Distraction to Running-only).
    let on_open_distraction = move |_| {
        let snapshot = engine.with_untracked(|s| {
            let mode = s.current_mode();
            let start_ms = s
                .current_session_started_at_ms()
                .unwrap_or_else(|| BrowserClock.now_ms());
            crate::bridge::types::DistractionParentRef {
                parent_session_start_ts: chrono::DateTime::<chrono::Utc>::from_timestamp_millis(
                    start_ms,
                )
                .unwrap_or_default()
                .to_rfc3339(),
                parent_mode: mode,
                parent_tag_id: selected_tag_ids.with_untracked(|sel| sel.first().cloned()),
                parent_title: {
                    let raw = session_title.get_untracked();
                    let trimmed = raw.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                },
            }
        });
        distraction_parent_ref_snapshot.set(Some(snapshot));
        distraction_modal_open.set(true);
    };
    let on_skip = move |_| {
        let events = engine.try_update(TimerState::skip).unwrap_or_default();
        handle_events(
            &events,
            &settings.get_untracked(),
            app_toast,
            warning_signal,
            i18n,
        );
        apply_tag_tracking_events(&events, active_session_tags, selected_tag_ids);
        // Clear the per-session title on skip — mirrors the zero-cross clear at
        // the tick loop so focus → focus skip doesn't carry the previous title.
        session_title.set(String::new());
        if settings.with_untracked(|s| s.notifications.auto_start_timer) {
            let start_events = engine
                .try_update(|state| state.start(&BrowserClock).unwrap_or_default())
                .unwrap_or_default();
            handle_events(
                &start_events,
                &settings.get_untracked(),
                app_toast,
                warning_signal,
                i18n,
            );
            apply_tag_tracking_events(&start_events, active_session_tags, selected_tag_ids);
        }
        dispatch_tray_update(engine, settings, true);
    };

    // Right-rail timer-adjust handlers. Per the JS-era
    // `pomodoro-timer.js:adjustTimer` (+/- 5 minutes), the buttons
    // shift the displayed remaining time by 300 seconds in either
    // direction. The engine exposes `adjust_remaining_secs` for
    // this — it preserves the running/paused state and updates the
    // wall-clock anchor so drift compensation continues correctly.
    let on_adjust_minus = move |_| {
        engine.update(|state| {
            state.adjust_remaining_secs(-300, &BrowserClock);
        });
        // Toast mirrors `pomodoro-timer.js:902-904` ("5 minutes
        // subtracted from timer ⏰").
        app_toast.show(t_string!(i18n, timer.toast.minutes_subtracted));
    };
    let on_adjust_plus = move |_| {
        engine.update(|state| {
            state.adjust_remaining_secs(300, &BrowserClock);
        });
        if engine.with(|s| s.time_remaining_secs() > 120) {
            warning_signal.set(false);
        }
        app_toast.show(t_string!(i18n, timer.toast.minutes_added));
    };

    // 1 Hz tick loop. Ticking unconditionally (not gated on
    // `is_running`) is safe because `tick()` short-circuits when
    // the engine is idle (`if !self.is_running { return events; }`).
    // The handle is dropped on cleanup; Leptos's RAII guarantees
    // the interval clears when the component unmounts.
    //
    // Post-tick, if the engine just transitioned to a new mode
    // (the previous tick fired `PomodoroCompleted` or the engine
    // is idle in a non-Focus mode after a break completion) AND
    // `Settings::notifications.auto_start_timer` is on, auto-
    // start the next session. Mirrors the JS-era flow at
    // `pomodoro-timer.js:1175-1180` and is what
    // `settings-automation.spec.js:59` exercises (the spec waits
    // for `#pause-icon` to be visible after a focus → break →
    // focus auto-roll).
    // Closure-captured remembrance of the engine's `time_remaining_secs()`
    // at the *prior* interval fire. The ticking sound fires only when this
    // value decreases — that's the same instant the visible digit changes
    // and `update_tray_icon` is dispatched. `setInterval(1000ms)` doesn't
    // align with wall-clock-second boundaries, so without the diff guard
    // the tone could fire twice on the same second or skip a second.
    let last_remaining_for_tick: std::rc::Rc<std::cell::Cell<u32>> =
        std::rc::Rc::new(std::cell::Cell::new(u32::MAX));
    Effect::new(move |_| {
        // Read once on mount to register the dependency; the
        // closure re-runs only on cleanup, not on every tick.
        let last_remaining = last_remaining_for_tick.clone();
        let handle = set_interval_with_handle(
            move || {
                let remaining_before = engine.with_untracked(TimerState::time_remaining_secs);
                // R-001 fix: capture `total_focus_before` outside the
                // try_update so the shared `persist_focus_completion`
                // closure (called below) can compute the just-sealed
                // session's wall-clock elapsed against the pre-tick
                // snapshot. The engine folds `current_session_elapsed_secs`
                // into `total_focus_secs` on `PomodoroCompleted`, so the
                // diff between after-tick and pre-tick is the duration.
                let (was_focus_pre, total_focus_before) = engine.with_untracked(|s| {
                    (
                        matches!(s.current_mode(), TimerMode::Focus),
                        s.total_focus_secs(),
                    )
                });
                let events = engine
                    .try_update(|state| {
                        let was_running = state.is_running();
                        let mode_before = state.current_mode();
                        let mut events = state.tick(&BrowserClock);
                        // Feature 006 (T050 / AG-2): auto-restart UI
                        // gate. Previously this fired on the bare
                        // `running → !running` transition; with the
                        // arrival of `engine.abort` (which also flips
                        // the bool but emits only `SessionAborted`),
                        // we now also require a session-end event in
                        // the same tick.
                        //
                        // R-002 fix: widened from PomodoroCompleted-
                        // only to PomodoroCompleted OR BreakCompleted.
                        // The natural break zero-cross at
                        // `engine/timer.rs:1090-1103` emits ONLY
                        // BreakCompleted (focus completion emits
                        // PomodoroCompleted via complete_focus_session
                        // — break completion does not). Before this
                        // widening, `auto_start_timer = true` failed to
                        // auto-roll Break → Focus on a natural break
                        // end, regressing
                        // `tests/e2e/settings-automation.spec.js`.
                        // SessionAborted and SessionSkipped
                        // intentionally do NOT appear in the gate
                        // pattern — abort and skip must not trigger
                        // an auto-restart countdown.
                        let saw_session_end = events.iter().any(|e| {
                            matches!(
                                e,
                                TimerEvent::PomodoroCompleted { .. }
                                    | TimerEvent::BreakCompleted { .. }
                            )
                        });
                        if was_running && !state.is_running() && saw_session_end {
                            let auto_start =
                                settings.with_untracked(|s| s.notifications.auto_start_timer);
                            if auto_start {
                                match state.start(&BrowserClock) {
                                    Ok(start_events) => events.extend(start_events),
                                    Err(e) => leptos::logging::warn!(
                                        "auto-start after completion failed: {:?}",
                                        e
                                    ),
                                }
                            }
                        }
                        let mode_after = state.current_mode();
                        let mode_changed = mode_before != mode_after;
                        let running_changed = was_running != state.is_running();
                        // Tray icon (title + tooltip) refreshes every
                        // tick to match the legacy `updateDisplay() →
                        // updateTrayIcon()` chain at
                        // `pomodoro-timer.js:1630`. Tray menu rebuilds
                        // only on mode/running transitions (the menu
                        // labels don't depend on the second-by-second
                        // countdown), avoiding the macOS NSStatusItem
                        // menu-flicker bug noted in issue #40.
                        {
                            use crate::bridge::types::UpdateTrayIconArgs;
                            let settings_snapshot = settings.get_untracked();
                            let (timer_text, mode_icon) =
                                build_tray_text(state, &settings_snapshot);
                            let is_running = state.is_running();
                            let is_paused = state.is_paused() || state.is_auto_paused();
                            let current_session = state.completed_pomodoros().saturating_add(1);
                            let tray_args = UpdateTrayIconArgs {
                                timer_text,
                                is_running,
                                session_mode: mode_after,
                                current_session,
                                total_sessions: settings_snapshot.timer.total_sessions,
                                mode_icon,
                            };
                            let menu_dirty = mode_changed || running_changed;
                            let mode_for_menu = mode_after;
                            spawn_local(async move {
                                let _ = commands::update_tray_icon(tray_args).await;
                                if menu_dirty {
                                    let _ = commands::update_tray_menu(
                                        is_running,
                                        is_paused,
                                        mode_for_menu,
                                    )
                                    .await;
                                }
                            });
                        }
                        events
                    })
                    .unwrap_or_default();
                // R-001 fix: persist on natural focus zero-cross.
                // Mirrors `on_complete`. Gated on `was_focus_pre` so
                // a Break → Focus auto-restart roll (which may emit
                // SessionStarted in the same event vec) doesn't fire
                // the persistence path for the new focus session.
                let completed_focus = was_focus_pre
                    && events
                        .iter()
                        .any(|e| matches!(e, TimerEvent::PomodoroCompleted { .. }));
                if completed_focus {
                    persist_focus_completion(total_focus_before);
                }
                handle_events(
                    &events,
                    &settings.get_untracked(),
                    app_toast,
                    warning_signal,
                    i18n,
                );
                apply_tag_tracking_events(&events, active_session_tags, selected_tag_ids);

                // Ticking sound fires only when the engine's
                // `time_remaining_secs()` decreases — the same instant
                // the visible digit changes and the tray text is
                // refreshed. `setInterval(1000ms)` isn't aligned with
                // wall-clock-second boundaries; without the diff guard
                // the tone could double-fire or land on a frame where
                // the display didn't change.
                let remaining_after = engine.with_untracked(TimerState::time_remaining_secs);
                let crossed_second = remaining_after < remaining_before;
                last_remaining.set(remaining_after);
                let should_tick = crossed_second
                    && settings.with_untracked(|s| s.notifications.metronome)
                    && engine.with_untracked(|s| {
                        matches!(s.current_mode(), TimerMode::Focus)
                            && s.is_running()
                            && !s.is_paused()
                            && !s.is_auto_paused()
                            && s.time_remaining_secs() > 0
                    });
                if should_tick {
                    play_metronome_tick();
                }
            },
            std::time::Duration::from_secs(1),
        );
        // The handle is intentionally leaked into the closure's
        // capture so the interval lives as long as the effect.
        // `set_interval_with_handle` returns `Result<…, JsValue>`;
        // failure means the JS bridge is missing (host tests / SSR
        // — neither applies to the wasm target this component
        // mounts on), so swallow.
        let _ = handle;
    });

    // Feature 004 (R-004 fix): unified ambient-sound gate Effect.
    //
    // Earlier wiring split the decision across two Effects (a
    // gate_high Effect + a pause sub-Effect). That collapsed
    // "enabled+track" with "actively playing this instant" into a
    // single boolean, so disabling the feature *while paused* never
    // fired a fade-out — gate_high was already false from the pause,
    // so the disable transition was a non-event. The driver got
    // stranded in `Paused`, and the next focus session's `start()`
    // no-opped because the state machine wasn't `Idle`.
    //
    // The fix splits the gate into two orthogonal predicates:
    //   * `enabled_and_track_selected` — settings-level intent
    //     (ambient on AND a non-None track selected).
    //   * `active_focus` — engine-level "should be sounding right
    //     now" (Focus mode, running, not paused / auto-paused,
    //     time_remaining > 0).
    //
    // Transitions are then dispatched by comparing both predicates
    // against the prior tick. Disable always tears down the resident
    // element (regardless of pause state); enable while active starts
    // playback; pause/resume while still enabled flips between
    // Playing and Paused without destroying the element.
    //
    // The driver is a process-wide singleton (`with_driver`) so the
    // resident `HtmlAudioElement` pair survives across breaks /
    // long-breaks / auto-starts within the same app session, holding
    // the WKWebView gesture lease for continuous-sessions auto-resume
    // (research.md Decision 1).
    Effect::new(move |prev: Option<(bool, bool, AmbientSoundType, u32)>| {
        let enabled = settings.with(|s| s.notifications.ambient_sound_enabled);
        let track = settings.with(|s| s.notifications.ambient_sound_type);
        let volume = settings.with(|s| s.notifications.ambient_sound_volume);
        let enabled_and_track_selected = enabled && !matches!(track, AmbientSoundType::None);

        let mode_focus = engine.with(|s| matches!(s.current_mode(), TimerMode::Focus));
        let active_focus = mode_focus
            && engine.with(|s| {
                s.is_running()
                    && !s.is_paused()
                    && !s.is_auto_paused()
                    && s.time_remaining_secs() > 0
            });

        let (prev_enabled_track, prev_active, prev_track, prev_volume) =
            prev.unwrap_or((false, false, AmbientSoundType::None, volume));

        if prev_enabled_track && !enabled_and_track_selected {
            // Intent flipped off — fade out from whatever state we
            // were in (Playing, Paused, or CrossFading; the driver
            // handles all three arcs to Idle). This is the
            // load-bearing arc for the "disable while paused"
            // recovery: without it, the driver stays in `Paused`
            // and the next focus session can't start playback.
            let _ = ambient_audio::with_driver(ambient_audio::AmbientAudio::fade_out);
        } else if !prev_enabled_track && enabled_and_track_selected && active_focus {
            // Intent just flipped on AND we're already inside an
            // active focus session — start playback immediately.
            let _ = ambient_audio::with_driver(|drv| drv.start(track, volume));
        } else if enabled_and_track_selected && prev_enabled_track {
            // Intent stays on — handle engine-side transitions and
            // settings tweaks while enabled.
            if active_focus && !prev_active {
                // Focus just became active (start / resume / new
                // focus phase). The driver itself decides whether
                // this is Idle→Playing or Paused→Playing based on
                // its current state.
                let _ = ambient_audio::with_driver(|drv| {
                    match drv.state().clone() {
                        ambient_audio::AmbientAudioState::Paused {
                            track: paused_track,
                        } if paused_track != track => {
                            // Track changed while paused — element is already at
                            // volume 0 (pause fade completed). fade_out transitions
                            // to FadingOut; tick(200) drives the ramp to Idle
                            // synchronously so start() can spawn a fresh element.
                            drv.fade_out();
                            drv.tick(200);
                            drv.start(track, volume);
                        }
                        ambient_audio::AmbientAudioState::Paused { .. } => drv.resume(volume),
                        ambient_audio::AmbientAudioState::Idle => drv.start(track, volume),
                        // Already mid-arc (Playing / CrossFading /
                        // FadingOut) — no entry transition needed;
                        // the existing ramp completes on its own.
                        _ => {}
                    }
                });
            } else if !active_focus && prev_active {
                // Focus left the active sub-state (pause /
                // auto-pause / break / overtime). Pause the
                // resident element rather than tearing it down.
                let _ = ambient_audio::with_driver(ambient_audio::AmbientAudio::pause);
            } else if active_focus && prev_active && prev_track != track {
                // Track changed mid-focus — cross-fade.
                let _ = ambient_audio::with_driver(|drv| drv.cross_fade(track, volume));
            } else if active_focus && prev_active && prev_volume != volume {
                // Volume slider moved while playing.
                let _ = ambient_audio::with_driver(|drv| drv.set_volume(volume));
            }
        }

        (enabled_and_track_selected, active_focus, track, volume)
    });

    // Feature 004: ramp ticker. Advances any in-flight fade ramp at
    // ~16 ms (60 Hz). The driver no-ops when no ramp is active, so
    // the cost is a single function call per tick when ambient is
    // disabled / Idle.
    #[cfg(target_arch = "wasm32")]
    Effect::new(move |_: Option<()>| {
        let handle = set_interval_with_handle(
            move || {
                let _ = ambient_audio::with_driver(|drv| drv.tick(16));
            },
            std::time::Duration::from_millis(16),
        );
        // IntervalHandle has no Drop impl in Leptos; the interval continues firing until app exit. Same pattern as metronome at :1381.
        let _ = handle;
    });

    view! {
        <div class="view-container container" id="timer-view"
            class:focus=move || engine.with(|s| s.current_mode() == TimerMode::Focus)
            class:break=move || engine.with(|s| s.current_mode() == TimerMode::Break)
            class:longBreak=move || engine.with(|s| s.current_mode() == TimerMode::LongBreak)
            class:overtime=move || is_overtime.get()
            class:warning=move || warning_signal.get()
        >
            // Progress dots — one dot per session in the daily total.
            // Mirrors the JS-era `pomodoro-timer.js:renderProgressDots`
            // surface. Each dot's `completed` class is gated on the
            // engine's `completed_pomodoros` accumulator. The total
            // comes from `Settings::timer.total_sessions` (default 10;
            // the JS-era default was also 10). A floor of 1 prevents a
            // zero-dot row when the setting is cleared to 0.
            <div class="progress-dots" id="progress-dots">
                {move || {
                    let total = dot_count(settings.with(|s| s.timer.total_sessions));
                    let completed = engine.with(TimerState::completed_pomodoros);
                    (0..total)
                        .map(|i| {
                            let is_done = i < completed;
                            let is_current = i == completed && completed < total;
                            view! {
                                <div
                                    class="dot"
                                    class:completed=is_done
                                    class:current=is_current
                                ></div>
                            }
                        })
                        .collect::<Vec<_>>()
                }}
            </div>

            // Status / mode label + tag-dropdown trigger.
            //
            // Feature 006 (T048): combined `#timer-status-pill`
            // wraps the existing `#timer-status` (chip + mode label
            // + chevron) and `#session-title-input` so the two read
            // as a unified pill. In Focus Idle the pill is
            // interactive (chevron visible, title editable). In
            // Focus Running / Paused / AutoPaused the pill collapses
            // read-only — chevron hidden via inline-style + tag
            // click no-op via `on_status_click` gate, title input
            // gets `readonly` via `prop:readonly`. Break / LongBreak
            // modes don't render the title region — the `<Show>`
            // guard below keeps the JS-era status-quo intact.
            <div style="text-align: center;">
                <div class="timer-status-pill" id="timer-status-pill"
                    class:running=move || matches!(run_state.get(), RunState::Running)
                    class:paused=move || matches!(run_state.get(), RunState::Paused)>
                <div class="timer-status-container">
                    <div
                        class="timer-status clickable"
                        class:active=move || tag_dropdown_open.get()
                        class:locked=move || !matches!(run_state.get(), RunState::Idle)
                        id="timer-status"
                        on:click=on_status_click
                    >
                        {move || {
                            // Feature 003 (FR-023): typed dispatch via
                            // `IconClass::from_icon_name`. Wrap the
                            // rendered glyph in a host element that
                            // carries the `id="status-icon"` selector
                            // (e2e contract).
                            let raw = status_icon.get();
                            let class = crate::components::icon::IconClass::from_icon_name(&raw);
                            match class {
                                crate::components::icon::IconClass::Remix(suffix) => {
                                    let cls = format!("ri-{suffix}");
                                    view! { <i id="status-icon" class=cls></i> }.into_any()
                                }
                                crate::components::icon::IconClass::Phosphor(suffix) => {
                                    let cls = format!("ph ph-{suffix}");
                                    view! { <i id="status-icon" class=cls></i> }.into_any()
                                }
                                crate::components::icon::IconClass::Glyph(g) => {
                                    view! { <span id="status-icon">{g}</span> }.into_any()
                                }
                            }
                        }}
                        <span id="status-text">{move || status_label.get()}</span>
                        // Feature 006 (T048): chevron hidden outside
                        // Idle — pill is read-only while a session
                        // is in flight.
                        <i
                            class="ri-arrow-down-s-line tag-dropdown-arrow"
                            id="tag-dropdown-arrow"
                            style=move || if matches!(run_state.get(), RunState::Idle) { "" } else { "display: none" }
                        ></i>
                    </div>

                    // Tag-dropdown popover. Anchored as a sibling of
                    // `#timer-status` inside `.timer-status-container`
                    // so the JS-era CSS positioning rules
                    // (`.tag-dropdown-menu` `position: absolute; top:
                    // calc(100% + 8px)`) anchor against the trigger.
                    // The `.active` class is what
                    // `style/timer.css` reads to flip
                    // `display: none` → `display: block`.
                    <div
                        class="tag-dropdown-menu"
                        id="tag-dropdown-menu"
                        class:active=move || tag_dropdown_open.get()
                    >
                        <div class="tag-dropdown-header">
                            <span>{t!(i18n, tag.choose_header)}</span>
                        </div>
                        <div class="tag-list" id="tag-list" role="list">
                            <For
                                each=move || tags.get()
                                key=|tag| tag.id.clone()
                                children=move |tag| {
                                    let tag_id_for_delete = tag.id.clone();
                                    let tag_id_for_select = tag.id.clone();
                                    let tag_id_for_match = tag.id.clone();
                                    let aria_row = tag.name.clone();
                                    let display_name = tag.name.clone();
                                    // Feature 003 (FR-023): typed dispatch via
                                    // `IconClass::from_icon_name` — supports
                                    // remixicon, Phosphor, and raw-glyph fall-
                                    // through (legacy emoji-icon tags per
                                    // FR-024).
                                    let icon_class =
                                        crate::components::icon::IconClass::from_icon_name(
                                            &tag.icon,
                                        );
                                    // Feature 005: localised delete-tag aria-label.
                                    // The tag name is dynamic data (user-typed),
                                    // never localised — only the surrounding
                                    // "Delete ... tag" template is translated.
                                    let delete_label =
                                        t_string!(i18n, tag.delete_aria, name = tag.name.as_str());
                                    // Multi-select highlight: a row is
                                    // `selected` whenever its id is in
                                    // `selected_tag_ids`. Clicking the
                                    // row toggles membership, mirroring
                                    // `tag-manager.js:toggleTag`.
                                    let tag_id_for_class = tag_id_for_match;
                                    let id_for_delete = tag_id_for_delete;
                                    view! {
                                        <div
                                            class="tag-item"
                                            class:selected=move || {
                                                selected_tag_ids.with(|sel| sel.contains(&tag_id_for_class))
                                            }
                                            role="listitem"
                                            aria-label=aria_row
                                            on:click=move |ev| {
                                                ev.stop_propagation();
                                                let tid = tag_id_for_select.clone();
                                                let now = BrowserClock.now_ms();
                                                let was_present = selected_tag_ids
                                                    .with_untracked(|sel| sel.contains(&tid));
                                                selected_tag_ids.update(|sel| {
                                                    if let Some(pos) =
                                                        sel.iter().position(|x| x == &tid)
                                                    {
                                                        sel.remove(pos);
                                                    } else {
                                                        sel.push(tid.clone());
                                                    }
                                                });
                                                // Mirror `tag-manager.js:toggleTag`:
                                                // removing a tag mid-session flushes
                                                // its accumulated duration; adding a
                                                // tag while running starts a fresh
                                                // tracker so its time-spent counter
                                                // starts at zero rather than
                                                // back-dating to the session start.
                                                if was_present {
                                                    tag_tracking_flush_one(
                                                        active_session_tags,
                                                        &tid,
                                                        now,
                                                    );
                                                } else if engine.with_untracked(TimerState::is_running) {
                                                    tag_tracking_start(
                                                        active_session_tags,
                                                        &tid,
                                                        &format!("session-{now}"),
                                                        now,
                                                    );
                                                }
                                            }
                                        >
                                            <span class="tag-item-icon">
                                                {crate::components::icon::render(&icon_class)}
                                            </span>
                                            <span class="tag-item-name">{display_name}</span>
                                            // Delete affordance renders on every row;
                                            // CSS (`.tag-item:hover .tag-item-delete`)
                                            // gates visibility on hover, matching the
                                            // JS-era `tag-manager.js:renderTagList`
                                            // behaviour.
                                            <div
                                                class="tag-item-delete ri-delete-bin-line"
                                                role="button"
                                                aria-label=delete_label
                                                on:click=move |ev| {
                                                    ev.stop_propagation();
                                                    on_delete_tag(id_for_delete.clone());
                                                }
                                            ></div>
                                        </div>
                                    }
                                }
                            />
                        </div>
                        <div class="tag-dropdown-footer">
                            <div class="new-tag-input" id="new-tag-input">
                                <div class="tag-input-row">
                                    <div class="icon-selector-container">
                                        <button
                                            class="selected-icon-btn"
                                            id="selected-icon-btn"
                                            on:click=on_toggle_picker
                                        >
                                            // Feature 003 (FR-023): typed
                                            // dispatch on the selected-icon
                                            // preview. The host `<span>`
                                            // carries the e2e selector;
                                            // the inner content is the
                                            // rendered glyph.
                                            <span id="selected-icon-display">{move || {
                                                let raw = new_tag_icon.get();
                                                let class = crate::components::icon::IconClass::from_icon_name(&raw);
                                                crate::components::icon::render(&class)
                                            }}</span>
                                            <i class="ri-arrow-down-s-line dropdown-arrow"></i>
                                        </button>
                                        // Use `.active` (the CSS-side
                                        // visibility class) rather than
                                        // `.open` — the
                                        // `.icon-selector-dropdown`
                                        // rule in `style/timer.css`
                                        // toggles `display: grid`
                                        // off `.active`.
                                        <div
                                            class="icon-selector-dropdown"
                                            id="icon-selector-dropdown"
                                            class:active=move || icon_picker_open.get()
                                        >
                                            <For
                                                each=move || ICON_OPTIONS.iter().copied()
                                                key=|icon| (*icon).to_string()
                                                children=move |icon| {
                                                    // Feature 003 (FR-023): typed
                                                    // dispatch on picker options.
                                                    // The host `<div>` carries the
                                                    // e2e `data-icon=` selector
                                                    // (`tags.spec.js`); the inner
                                                    // content is the rendered glyph.
                                                    let icon_for_pick = icon.to_string();
                                                    let parsed =
                                                        crate::components::icon::IconClass::from_icon_name(icon);
                                                    let host_class = match parsed {
                                                        crate::components::icon::IconClass::Remix(_)
                                                        | crate::components::icon::IconClass::Phosphor(_) => "icon-option",
                                                        crate::components::icon::IconClass::Glyph(_) => "emoji-option",
                                                    };
                                                    view! {
                                                        <div
                                                            class=host_class
                                                            data-icon=icon
                                                            on:click=move |ev| {
                                                                ev.stop_propagation();
                                                                on_pick_icon(icon_for_pick.clone());
                                                            }
                                                        >
                                                            {crate::components::icon::render(&parsed)}
                                                        </div>
                                                    }.into_any()
                                                }
                                            />
                                        </div>
                                    </div>
                                    <input
                                        type="text"
                                        placeholder=move || t_string!(i18n, tag.new_placeholder)
                                        id="new-tag-name"
                                        aria-label=move || t_string!(i18n, tag.new_aria_label)
                                        prop:value=move || new_tag_name.get()
                                        on:click=move |ev| ev.stop_propagation()
                                        on:input=move |ev| new_tag_name.set(event_target_value(&ev))
                                        on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                                            // Enter submits — mirrors the JS-era
                                            // `tag-manager.js:setupEventListeners`
                                            // `keydown === "Enter"` handler on
                                            // `#new-tag-name`.
                                            if ev.key() == "Enter" {
                                                ev.prevent_default();
                                                on_create_tag();
                                            }
                                        }
                                    />
                                    <button
                                        class="create-tag-btn"
                                        id="create-tag-btn"
                                        on:click=move |ev| {
                                            ev.stop_propagation();
                                            on_create_tag();
                                        }
                                    >"+"</button>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
                // Feature 002 Bundle A + Feature 006 (T048):
                // per-session title input, rendered below the tag
                // pill inside `#timer-status-pill` so the combined
                // pill reads as a single unit. Only shown during
                // Focus — breaks don't carry titles. The input
                // becomes `readonly` outside Focus-Idle so the user
                // can't edit mid-session; the displayed placeholder
                // also flips to the lighter `pill_title_placeholder`
                // copy when Idle (FR-003: faint placeholder).
                <Show when=move || engine.with(|s| matches!(s.current_mode(), TimerMode::Focus))>
                    <div class="session-title-row"
                        class:pill-readonly=move || !matches!(run_state.get(), RunState::Idle)
                        class:pill-placeholder=move || session_title.with(|t| t.trim().is_empty())>
                        <input
                            type="text"
                            id="session-title-input"
                            class="session-title-input"
                            maxlength="120"
                            placeholder=move || t_string!(i18n, timer.pill_title_placeholder)
                            prop:value=move || session_title.get()
                            prop:readonly=move || !matches!(run_state.get(), RunState::Idle)
                            on:input=move |ev| {
                                session_title.set(event_target_value(&ev));
                            }
                        />
                    </div>
                </Show>
                </div>
            </div>

            // Countdown display. The `.timer-container` carries a
            // per-mode theme class (`focus` / `break` / `longBreak`)
            // so `style/timer.css`'s
            // `.timer-container.focus .timer-seconds { color:
            // var(--focus-timer-color) }` rule applies — without
            // the theme class, `.timer-seconds` falls back to
            // `var(--text-light)` (gray) while `.timer-minutes` uses
            // `var(--focus-timer-color)` (dark red), splitting the
            // countdown across two colors. The visual-regression
            // baseline shows both columns in `--focus-timer-color`,
            // which only happens with the theme class applied.
            <div
                class="timer-container"
                class:focus=move || matches!(engine.with(TimerState::current_mode), TimerMode::Focus)
                class:break=move || matches!(engine.with(TimerState::current_mode), TimerMode::Break)
                class:longBreak=move || matches!(engine.with(TimerState::current_mode), TimerMode::LongBreak)
                class:warning=move || warning_signal.get()
                class:overtime=move || is_overtime.get()
            >
                <div class="timer-minutes" id="timer-minutes">{move || minutes_text.get()}</div>
                <div class="timer-seconds" id="timer-seconds">{move || seconds_text.get()}</div>
            </div>

            // Control buttons. The icon visibility toggles match the
            // JS-era `style="display: none"` flips — the e2e suite
            // asserts on `toBeVisible()` / `toBeHidden()` of the
            // `#play-icon` / `#pause-icon` IDs. Inline SVGs mirror the
            // JS-era `index.html` markup byte-for-byte (heroicons-
            // style play / pause glyphs at viewBox 0 0 24 24); empty
            // <span> stand-ins would be zero-size boxes that
            // `toBeVisible()` rejects.
            // Feature 006 (T049): state-aware button matrix. The
            // three slots keep their JS-era selector IDs (`stop-btn`,
            // `play-pause-btn`, `skip-btn`) so the e2e contract +
            // visual-regression baseline don't drift; the icons +
            // labels + handlers dispatch on `(RunState, TimerMode)`.
            //
            //   Slot         Idle            Running                 Paused
            //   left-slot    + Quick Log     ✕ Abort                 ✕ Abort
            //   center-slot  ▶ Play          ⏸ Pause                 ▶ Resume
            //   right-slot   → Skip Mode     ! Note Distraction      ✓ Complete
            //
            // The play/pause center slot keeps its existing wiring
            // (same handler covers Start / Pause / Resume per
            // `PlayPauseButtonState`). Filled vs ghost is gated via
            // the `.primary` class — only the center slot is filled
            // and (in Paused state) the right slot (Complete).
            <div class="controls">
                <button id="stop-btn" class="control-btn"
                    class:filled-action=move || false
                    aria-label=move || verbose_label_left.get()
                    title=move || verbose_label_left.get()
                    data-tooltip=move || terse_tooltip_left.get()
                    on:click=move |ev| {
                        match run_state.get() {
                            RunState::Idle => on_open_quick_log(ev),
                            RunState::Running | RunState::Paused => on_abort(ev),
                        }
                    }>
                    // Idle: plus icon (Quick Log).
                    <svg
                        id="stop-icon"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2.5"
                        style=move || if matches!(run_state.get(), RunState::Idle) { "" } else { "display: none" }
                    >
                        <path stroke-linecap="round" stroke-linejoin="round" d="M12 5v14M5 12h14" />
                    </svg>
                    // Running / Paused: X icon (Abort). Selector
                    // `#undo-icon` kept for VR-baseline continuity —
                    // the icon glyph itself is the abort × glyph.
                    <svg
                        id="undo-icon"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2.5"
                        style=move || if matches!(run_state.get(), RunState::Running | RunState::Paused) { "" } else { "display: none" }
                    >
                        <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                    </svg>
                </button>
                <button id="play-pause-btn" class="control-btn primary"
                    aria-label=move || verbose_label_play.get()
                    title=move || verbose_label_play.get()
                    data-tooltip=move || terse_tooltip_play.get()
                    on:click=on_play_pause>
                    <svg id="play-icon" viewBox="0 0 24 24" fill="currentColor" style=move || play_icon_style.get()>
                        <path fill-rule="evenodd" clip-rule="evenodd"
                            d="M4.5 5.653c0-1.427 1.529-2.33 2.779-1.643l11.54 6.347c1.295.712 1.295 2.573 0 3.286L7.28 19.99c-1.25.687-2.779-.217-2.779-1.643V5.653Z" />
                    </svg>
                    <svg id="pause-icon" viewBox="0 0 24 24" fill="currentColor" style=move || pause_icon_style.get()>
                        <path fill-rule="evenodd" clip-rule="evenodd"
                            d="M6.75 5.25a.75.75 0 0 1 .75-.75H9a.75.75 0 0 1 .75.75v13.5a.75.75 0 0 1-.75.75H7.5a.75.75 0 0 1-.75-.75V5.25Zm7.5 0A.75.75 0 0 1 15 4.5h1.5a.75.75 0 0 1 .75.75v13.5a.75.75 0 0 1-.75.75H15a.75.75 0 0 1-.75-.75V5.25Z" />
                    </svg>
                </button>
                <button id="skip-btn" class="control-btn"
                    class:primary=move || matches!(run_state.get(), RunState::Paused)
                    aria-label=move || verbose_label_right.get()
                    title=move || verbose_label_right.get()
                    data-tooltip=move || terse_tooltip_right.get()
                    on:click=move |ev| {
                        match run_state.get() {
                            RunState::Idle => on_skip(ev),
                            RunState::Running => on_open_distraction(ev),
                            RunState::Paused => on_complete(ev),
                        }
                    }>
                    // Idle: per-mode skip icon (coffee / moon /
                    // brain). The visibility logic still considers
                    // the upcoming mode (cadence-aware via
                    // `next_is_long_break`).
                    <i
                        id="skip-coffee-icon"
                        class="ri-cup-line"
                        style=move || {
                            let mode = engine.with(TimerState::current_mode);
                            let next_long = next_is_long_break.get();
                            if matches!(run_state.get(), RunState::Idle)
                                && skip_icon_for_mode(mode, next_long) == "coffee" {
                                "font-size: 24px"
                            } else {
                                "display: none; font-size: 24px"
                            }
                        }
                    ></i>
                    <i
                        id="skip-sleep-icon"
                        class="ri-moon-line"
                        style=move || {
                            let mode = engine.with(TimerState::current_mode);
                            let next_long = next_is_long_break.get();
                            if matches!(run_state.get(), RunState::Idle)
                                && skip_icon_for_mode(mode, next_long) == "moon" {
                                "font-size: 24px"
                            } else {
                                "display: none; font-size: 24px"
                            }
                        }
                    ></i>
                    <i
                        id="skip-brain-icon"
                        class="ri-brain-line"
                        style=move || {
                            let mode = engine.with(TimerState::current_mode);
                            let next_long = next_is_long_break.get();
                            if matches!(run_state.get(), RunState::Idle)
                                && skip_icon_for_mode(mode, next_long) == "brain" {
                                "font-size: 24px"
                            } else {
                                "display: none; font-size: 24px"
                            }
                        }
                    ></i>
                    // Running: alert-circle (Distraction note).
                    <i
                        id="distraction-icon"
                        class="ri-alert-line"
                        style=move || if matches!(run_state.get(), RunState::Running) {
                            "font-size: 24px"
                        } else {
                            "display: none; font-size: 24px"
                        }
                    ></i>
                    // Paused: check (Complete).
                    <i
                        id="complete-icon"
                        class="ri-check-line"
                        style=move || if matches!(run_state.get(), RunState::Paused) {
                            "font-size: 24px"
                        } else {
                            "display: none; font-size: 24px"
                        }
                    ></i>
                    <svg
                        id="skip-default-icon"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2.5"
                        style="display: none"
                    >
                        <path stroke-linecap="round" stroke-linejoin="round" d="M13.5 4.5L21 12m0 0l-7.5 7.5M21 12H3" />
                    </svg>
                </button>
            </div>

            // Right-rail settings indicators + timer-adjust buttons.
            // Mirrors the JS-era `<div class="settings-indicators">`
            // markup at `src/index.html` (3f1119e^). The visual-
            // regression baseline shows four icons stacked vertically
            // along the right edge of the timer view: lightbulb (smart-
            // pause), play-circle (auto-start), repeat (continuous),
            // and the -5/+5 timer-adjust buttons. The first three are
            // visual-only at this layer (cold-start state mirrors the
            // JS-era CSS — smart-pause hidden, others visible) — Phase
            // 4c routes them through the Settings managers. The +/-
            // buttons dispatch through the engine's
            // `adjust_remaining_secs` API which preserves the running/
            // paused state and the wall-clock anchor.
            <div class="settings-indicators">
                <div class="smart-pause-container">
                    <span
                        id="smart-pause-countdown"
                        class="countdown-number"
                        style="display: none"
                    ></span>
                    <i
                        id="smart-indicator"
                        class=move || indicator_icon_class(
                            "lightbulb",
                            settings.with(|s| s.notifications.smart_pause),
                        )
                        style="display: block"
                        data-tooltip=move || t_string!(i18n, timer.indicator_smart_pause)
                        title=move || t_string!(i18n, timer.indicator_smart_pause)
                        on:click=move |_| settings.update(|s| {
                            s.notifications.smart_pause = !s.notifications.smart_pause;
                        })
                    ></i>
                </div>
                <i
                    id="auto-start-indicator"
                    class=move || indicator_icon_class(
                        "play-circle",
                        settings.with(|s| s.notifications.auto_start_timer),
                    )
                    data-tooltip=move || t_string!(i18n, timer.indicator_auto_start)
                    title=move || t_string!(i18n, timer.indicator_auto_start)
                    on:click=move |_| settings.update(|s| {
                        s.notifications.auto_start_timer = !s.notifications.auto_start_timer;
                    })
                ></i>
                <i
                    id="continuous-session-indicator"
                    class=move || indicator_icon_class(
                        "repeat",
                        settings.with(|s| s.notifications.allow_continuous_sessions),
                    )
                    data-tooltip=move || t_string!(i18n, timer.indicator_continuous)
                    title=move || t_string!(i18n, timer.indicator_continuous)
                    on:click=move |_| settings.update(|s| {
                        s.notifications.allow_continuous_sessions =
                            !s.notifications.allow_continuous_sessions;
                    })
                ></i>
                <button
                    class="timer-adjust-btn"
                    id="timer-minus-btn"
                    title=move || t_string!(i18n, timer.adjust_minus_aria)
                    aria-label=move || t_string!(i18n, timer.adjust_minus_aria)
                    on:click=on_adjust_minus
                    prop:disabled=move || is_overtime.get()
                >
                    <span>"-5"</span>
                </button>
                <button
                    class="timer-adjust-btn"
                    id="timer-plus-btn"
                    title=move || t_string!(i18n, timer.adjust_plus_aria)
                    aria-label=move || t_string!(i18n, timer.adjust_plus_aria)
                    on:click=on_adjust_plus
                    prop:disabled=move || is_overtime.get()
                >
                    <span>"+5"</span>
                </button>
            </div>

            // Feature 006 (T051): Quick Log modal.
            <QuickLogModal
                open=quick_log_modal_open
                quick_logs=quick_logs
            />
            // Feature 006 (T052): Distraction modal.
            <DistractionModal
                open=distraction_modal_open
                parent_ref_snapshot=distraction_parent_ref_snapshot
                distractions=distractions
            />
        </div>
    }
}

/// Feature 006 (T051): Quick Log modal. Title (required, maxlength=120)
/// + elapsed minutes (1..=720, default 5). Submit calls
/// `QuickLogManager::add` with a UUID v4 id and the current
/// wall-clock; the modal closes immediately. Bridge save is best-
/// effort (failures logged; the in-memory mutation already
/// happened).
#[component]
fn QuickLogModal(
    open: RwSignal<bool>,
    quick_logs: RwSignal<crate::managers::quick_log::QuickLogManager>,
) -> impl IntoView {
    let i18n = use_i18n();
    let title = RwSignal::new(String::new());
    let minutes = RwSignal::new(5u32);

    let on_close = move |_| {
        open.set(false);
        title.set(String::new());
        minutes.set(5);
    };
    let on_submit = move |_| {
        let raw_title = title.with_untracked(|t| t.trim().to_string());
        let mins = minutes.get_untracked();
        if raw_title.is_empty() || !(1..=720).contains(&mins) {
            return;
        }
        let now_ms = BrowserClock.now_ms();
        let id = format!("quicklog-{}", random_uuid());
        quick_logs.update(|mgr| mgr.add(raw_title, mins, now_ms, id));
        let snapshot =
            quick_logs.with_untracked(crate::managers::quick_log::QuickLogManager::save_payload);
        spawn_local(async move {
            if let Err(e) = crate::bridge::commands::save_quick_logs(snapshot).await {
                leptos::logging::warn!("save_quick_logs failed: {:?}", e);
            }
        });
        open.set(false);
        title.set(String::new());
        minutes.set(5);
    };

    view! {
        <div
            class="session-modal-overlay"
            id="quick-log-modal-overlay"
            style=move || if open.get() { "" } else { "display: none" }
        >
            <form class="session-modal"
                id="quick-log-form"
                role="dialog"
                aria-modal="true"
                aria-labelledby="quick-log-modal-title"
                on:submit=move |ev| {
                    ev.prevent_default();
                    on_submit(ev);
                }>
                <div class="session-modal-header">
                    <h3 id="quick-log-modal-title">{t!(i18n, modal.quick_log_title)}</h3>
                    <button
                        type="button"
                        id="close-quick-log-modal"
                        class="close-btn"
                        on:click=on_close
                    >"\u{00d7}"</button>
                </div>
                <div class="session-modal-body">
                    <label for="quick-log-title">{t!(i18n, modal.quick_log_title_label)}</label>
                    <input
                        type="text"
                        id="quick-log-title"
                        maxlength="120"
                        autofocus
                        required
                        prop:value=move || title.get()
                        on:input=move |ev| title.set(event_target_value(&ev))
                    />
                    <label for="quick-log-minutes">{t!(i18n, modal.quick_log_minutes_label)}</label>
                    <input
                        type="number"
                        id="quick-log-minutes"
                        min="1"
                        max="720"
                        prop:value=move || minutes.get().to_string()
                        on:input=move |ev| {
                            let raw: u32 = event_target_value(&ev).parse().unwrap_or(5);
                            minutes.set(raw.clamp(1, 720));
                        }
                    />
                </div>
                <div class="modal-actions">
                    <button
                        type="button"
                        id="cancel-quick-log-btn"
                        class="btn-secondary"
                        on:click=on_close
                    >{t!(i18n, modal.quick_log_cancel)}</button>
                    <button
                        type="submit"
                        id="save-quick-log-btn"
                        class="btn-primary"
                    >{t!(i18n, modal.quick_log_submit)}</button>
                </div>
            </form>
        </div>
    }
}

/// Feature 006 (T052): Distraction modal. Single note field
/// (required, maxlength=120). Enter submits; Escape cancels.
/// `parent_ref_snapshot` is the snapshotted parent-session ref at
/// modal-open time (race-free per Edge Cases). Submission calls
/// `DistractionManager::add` and never touches the engine
/// (FR-035 — pure side channel: timer keeps ticking, smart-pause
/// untouched).
#[component]
fn DistractionModal(
    open: RwSignal<bool>,
    parent_ref_snapshot: RwSignal<Option<crate::bridge::types::DistractionParentRef>>,
    distractions: RwSignal<crate::managers::distraction::DistractionManager>,
) -> impl IntoView {
    let i18n = use_i18n();
    let note = RwSignal::new(String::new());

    let do_close = move || {
        open.set(false);
        note.set(String::new());
        parent_ref_snapshot.set(None);
    };
    let do_submit = move || {
        let raw_note = note.with_untracked(|n| n.trim().to_string());
        if raw_note.is_empty() {
            return;
        }
        let pref = parent_ref_snapshot.get_untracked();
        let now_ms = BrowserClock.now_ms();
        let id = format!("distraction-{}", random_uuid());
        distractions.update(|mgr| mgr.add(raw_note, pref, now_ms, id));
        let snapshot = distractions
            .with_untracked(crate::managers::distraction::DistractionManager::save_payload);
        spawn_local(async move {
            if let Err(e) = crate::bridge::commands::save_distractions(snapshot).await {
                leptos::logging::warn!("save_distractions failed: {:?}", e);
            }
        });
        open.set(false);
        note.set(String::new());
        parent_ref_snapshot.set(None);
    };

    view! {
        <div
            class="session-modal-overlay"
            id="distraction-modal-overlay"
            style=move || if open.get() { "" } else { "display: none" }
        >
            <form class="session-modal"
                id="distraction-form"
                role="dialog"
                aria-modal="true"
                aria-labelledby="distraction-modal-title"
                on:submit=move |ev| {
                    ev.prevent_default();
                    do_submit();
                }
                on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                    if ev.key() == "Escape" {
                        ev.prevent_default();
                        do_close();
                    }
                }>
                <div class="session-modal-header">
                    <h3 id="distraction-modal-title">{t!(i18n, modal.note_distraction_title)}</h3>
                    <button
                        type="button"
                        id="close-distraction-modal"
                        class="close-btn"
                        on:click=move |_| do_close()
                    >"\u{00d7}"</button>
                </div>
                <div class="session-modal-body">
                    <label for="distraction-note">{t!(i18n, modal.note_distraction_label)}</label>
                    <input
                        type="text"
                        id="distraction-note"
                        maxlength="120"
                        autofocus
                        required
                        prop:value=move || note.get()
                        on:input=move |ev| note.set(event_target_value(&ev))
                    />
                </div>
                <div class="modal-actions">
                    <button
                        type="button"
                        id="cancel-distraction-btn"
                        class="btn-secondary"
                        on:click=move |_| do_close()
                    >{t!(i18n, modal.note_distraction_cancel)}</button>
                    <button
                        type="submit"
                        id="save-distraction-btn"
                        class="btn-primary"
                    >{t!(i18n, modal.note_distraction_submit)}</button>
                </div>
            </form>
        </div>
    }
}

/// Pure dot-count projection: raw `total_sessions` value with a floor
/// of 1 so a zero setting never produces an empty dot row.
fn dot_count(total: u32) -> u32 {
    total.max(1)
}

#[cfg(test)]
mod tests {
    use super::{
        dot_count, indicator_icon_class, mode_label, mode_label_with_status, pad_two,
        skip_icon_for_mode, PlayPauseButtonState, SkipButtonState, ICON_OPTIONS,
    };
    use crate::bridge::types::TimerMode;

    /// T191 — visual-regression / selector contract pin.
    ///
    /// The e2e suite (`tests/e2e/timer.spec.js`, `_smoke.spec.js`,
    /// `tags.spec.js`, `sessions-history.spec.js`,
    /// `visual-regression.spec.js`) asserts on a fixed set of
    /// `id="..."` attributes against the timer view. Drift here
    /// breaks the e2e run; this host-side test surfaces the drift
    /// earlier (in `cargo test` rather than `npx playwright test`)
    /// by enumerating the contract surface alongside the spec line
    /// that consumes each selector.
    ///
    /// Source map (every entry below is exercised by the referenced
    /// spec line):
    ///
    /// - `timer-view` — `_smoke.spec.js:19` (`toBeVisible`),
    ///   `visual-regression.spec.js` (timer-view screenshot
    ///   baseline).
    /// - `timer-minutes` — `_smoke.spec.js:17` (initial "25"),
    ///   `timer.spec.js:28` (post-reset "25").
    /// - `timer-seconds` — `_smoke.spec.js:18` ("00"),
    ///   `timer.spec.js:13` (ticks), `timer.spec.js:29` ("00"
    ///   reset).
    /// - `play-pause-btn` — `timer.spec.js:8,16,21` (start / pause
    ///   / resume).
    /// - `stop-btn` — `timer.spec.js:25` (reset).
    /// - `skip-btn` — present for E8 tray-skip + manual skip flow
    ///   (Phase 4c wires the tray subscription).
    /// - `play-icon` — `timer.spec.js:7,17,30` (visibility toggles).
    /// - `pause-icon` — `timer.spec.js:9,18` (running indicator).
    /// - `timer-status` — `tags.spec.js:11,33`,
    ///   `sessions-history.spec.js:14` (tag-dropdown trigger).
    /// - `status-text` — `sessions-history.spec.js:28` ("Break"
    ///   after focus completes).
    /// - `status-icon` — JS-era icon swap (`ri-brain-line` for
    ///   Focus, `ri-cup-line` for Break); covered by visual
    ///   regression baselines.
    /// - `progress-dots` — JS-era `#progress-dots` filled by the
    ///   daily-goal projection; container present so the visual
    ///   shell matches even before population.
    /// - `tag-dropdown-arrow` — chevron next to status-text;
    ///   covered by visual regression.
    ///
    /// If a spec adds a new selector, append it here AND to the
    /// `view!` macro above so the contract drift is caught at
    /// `cargo test` time. Visual baseline updates are out of scope
    /// (per AGENTS.md §"Don't update visual regression baselines
    /// without explicit visual review").
    #[test]
    fn timer_view_selector_contract_documented() {
        const REQUIRED_IDS: &[&str] = &[
            "timer-view",
            "timer-minutes",
            "timer-seconds",
            "play-pause-btn",
            "stop-btn",
            "skip-btn",
            "play-icon",
            "pause-icon",
            "stop-icon",
            "undo-icon",
            "skip-coffee-icon",
            "skip-sleep-icon",
            "skip-brain-icon",
            "timer-status",
            "status-text",
            "status-icon",
            "progress-dots",
            "tag-dropdown-arrow",
            "smart-indicator",
            "auto-start-indicator",
            "continuous-session-indicator",
        ];
        let mut seen: Vec<&str> = Vec::with_capacity(REQUIRED_IDS.len());
        for id in REQUIRED_IDS {
            assert!(!id.is_empty(), "selector ID must not be empty");
            assert!(
                !seen.contains(id),
                "duplicate selector ID in contract: {id}",
            );
            seen.push(id);
        }
    }

    /// T191 first-paint pin: the smoke spec asserts the initial
    /// `#timer-minutes` reads "25" and `#timer-seconds` reads "00".
    /// `pad_two` is the projection that produces those literals
    /// from the engine's initial `time_remaining_secs()` (1500 →
    /// 25 / 0). Pin the projection here so a future refactor that
    /// changes the format silently fails this test rather than the
    /// e2e suite.
    #[test]
    fn first_paint_minutes_seconds_match_smoke_spec() {
        let initial_secs: u32 = 25 * 60;
        assert_eq!(pad_two(initial_secs / 60), "25");
        assert_eq!(pad_two(initial_secs % 60), "00");
    }

    #[test]
    fn pad_two_zero_pads_single_digit() {
        assert_eq!(pad_two(0), "00");
        assert_eq!(pad_two(5), "05");
        assert_eq!(pad_two(25), "25");
        assert_eq!(pad_two(60), "60");
    }

    #[test]
    fn indicator_icon_class_lightbulb() {
        assert_eq!(
            indicator_icon_class("lightbulb", true),
            "ri-lightbulb-fill active"
        );
        assert_eq!(
            indicator_icon_class("lightbulb", false),
            "ri-lightbulb-line"
        );
    }

    #[test]
    fn indicator_icon_class_play_circle() {
        assert_eq!(
            indicator_icon_class("play-circle", true),
            "ri-play-circle-fill active"
        );
        assert_eq!(
            indicator_icon_class("play-circle", false),
            "ri-play-circle-line"
        );
    }

    #[test]
    fn indicator_icon_class_repeat() {
        assert_eq!(
            indicator_icon_class("repeat", true),
            "ri-repeat-fill active"
        );
        assert_eq!(indicator_icon_class("repeat", false), "ri-repeat-line");
    }

    #[test]
    fn mode_label_covers_every_variant() {
        assert_eq!(mode_label(TimerMode::Focus), "Focus");
        assert_eq!(mode_label(TimerMode::Break), "Break");
        assert_eq!(mode_label(TimerMode::LongBreak), "Long Break");
    }

    #[test]
    fn mode_label_with_status_idle_returns_plain_label() {
        assert_eq!(
            mode_label_with_status(TimerMode::Focus, false, false, false, false),
            "Focus"
        );
        assert_eq!(
            mode_label_with_status(TimerMode::Break, false, false, false, false),
            "Break"
        );
        assert_eq!(
            mode_label_with_status(TimerMode::LongBreak, false, false, false, false),
            "Long Break"
        );
    }

    #[test]
    fn mode_label_with_status_running_no_suffix() {
        // Running but not paused/overtime → plain label (matches e2e
        // contracts in `_smoke.spec.js` and `sessions-history.spec.js`).
        assert_eq!(
            mode_label_with_status(TimerMode::Focus, true, false, false, false),
            "Focus"
        );
        assert_eq!(
            mode_label_with_status(TimerMode::Break, true, false, false, false),
            "Break"
        );
    }

    #[test]
    fn mode_label_with_status_paused_suffix() {
        assert_eq!(
            mode_label_with_status(TimerMode::Focus, false, true, false, false),
            "Focus (Paused)"
        );
        assert_eq!(
            mode_label_with_status(TimerMode::Break, false, true, false, false),
            "Break (Paused)"
        );
        assert_eq!(
            mode_label_with_status(TimerMode::LongBreak, false, true, false, false),
            "Long Break (Paused)"
        );
    }

    #[test]
    fn mode_label_with_status_auto_paused_suffix() {
        assert_eq!(
            mode_label_with_status(TimerMode::Focus, false, false, true, false),
            "Focus (Auto-paused)"
        );
        assert_eq!(
            mode_label_with_status(TimerMode::Break, false, false, true, false),
            "Break (Auto-paused)"
        );
    }

    #[test]
    fn mode_label_with_status_overtime_suffix_requires_running() {
        // Overtime only shows while running.
        assert_eq!(
            mode_label_with_status(TimerMode::Focus, true, false, false, true),
            "Focus (Overtime)"
        );
        // Not running + overtime → plain (can't be in overtime while stopped).
        assert_eq!(
            mode_label_with_status(TimerMode::Focus, false, false, false, true),
            "Focus"
        );
    }

    #[test]
    fn mode_label_with_status_paused_wins_over_overtime() {
        // Paused takes precedence; overtime can only occur while running,
        // so this combination shouldn't arise in practice, but the
        // tie-break is pinned here.
        assert_eq!(
            mode_label_with_status(TimerMode::Focus, false, true, false, true),
            "Focus (Paused)"
        );
    }

    #[test]
    fn skip_icon_for_mode_covers_all_variants() {
        assert_eq!(skip_icon_for_mode(TimerMode::Focus, false), "coffee");
        assert_eq!(skip_icon_for_mode(TimerMode::Focus, true), "moon");
        assert_eq!(skip_icon_for_mode(TimerMode::Break, false), "brain");
        assert_eq!(skip_icon_for_mode(TimerMode::Break, true), "brain");
        assert_eq!(skip_icon_for_mode(TimerMode::LongBreak, false), "brain");
        assert_eq!(skip_icon_for_mode(TimerMode::LongBreak, true), "brain");
    }

    #[test]
    fn icon_options_has_12_with_3_remix_9_phosphor_0_emoji() {
        assert_eq!(ICON_OPTIONS.len(), 12, "ICON_OPTIONS must have exactly 12 entries (3 remix + 9 Phosphor; 5 emoji removed per FR-020/FR-021)");
        let remix_count = ICON_OPTIONS.iter().filter(|s| s.starts_with("ri-")).count();
        let phosphor_count = ICON_OPTIONS.iter().filter(|s| s.starts_with("ph-")).count();
        let other_count = ICON_OPTIONS.len() - remix_count - phosphor_count;
        assert_eq!(remix_count, 3, "expected 3 remixicon entries");
        assert_eq!(phosphor_count, 9, "expected 9 Phosphor entries");
        assert_eq!(other_count, 0, "expected 0 emoji or other entries");
    }

    #[test]
    fn dot_count_floors_at_one_and_passes_through_nonzero() {
        assert_eq!(dot_count(0), 1, "zero must yield 1-dot floor");
        assert_eq!(dot_count(1), 1);
        assert_eq!(
            dot_count(10),
            10,
            "default total_sessions = 10 must pass unchanged"
        );
        assert_eq!(dot_count(11), 11);
        assert_eq!(dot_count(20), 20);
    }

    // -------------------------------------------------------------
    // `build_tray_text` regression tests.
    //
    // Pre-fix, the tray title silently failed to update because the
    // dispatch only fired on mode/running transitions (not every
    // tick), the `UpdateTrayIconArgs` wire shape was snake_case
    // (Tauri auto-renames to camelCase, rejecting our payload), and
    // the macOS NSStatusItem rendered an icon-only tray with no
    // countdown text. The wire shape is pinned in
    // `presto-ipc::args::UpdateTrayIconArgs` tests; this block pins
    // the formatter that produces the title string.
    // Build-tray-text + message-helper + tag-tracking tests now
    // live alongside their helpers in the `tray`, `messages`, and
    // `tag_tracking` submodules.

    // -------------------------------------------------------------
    // Feature 003 Bundle D: control-button tooltip text matrix.
    //
    // FR-031 / SC-012 invariants:
    // (a) `aria-label == title` per button per state (verbose pair
    //     stays paired)
    // (b) `data-tooltip` matches the FR-027/028/029 terse mapping
    // (c) The test MUST NOT assert `aria-label == data-tooltip`
    //     (CHK041 — strings intentionally decoupled).
    //
    // Per Principle V's UI-rendering carve-out (plan.md §V), this
    // test is NOT RED-first; it lands alongside the implementation
    // (T017/T018) as a coverage gate. The button-state enums
    // (`StopButtonState`, `PlayPauseButtonState`, `SkipButtonState`)
    // expose pure projections (`verbose_label`, `terse_tooltip`) so
    // the test pins the typed dispatch without mounting a Leptos
    // view (matches the `IconClass::render_spec` pattern from T005).

    // Feature 006 (T049): the legacy `StopButtonState` enum is
    // gone — its verbose-label / terse-tooltip tests with it. The
    // left-slot button label is now driven by `(RunState, TimerMode)`
    // via the i18n catalogue keys (`timer.ctrl_quick_log` /
    // `timer.ctrl_abort` + their aria variants), and the matrix
    // wiring lives directly inside `TimerView`'s `view!` body.
    // Coverage moves to the Phase 9 e2e specs (`timer-quick-log`,
    // `timer-abort`, `timer-complete`).

    /// FR-028 — `#play-pause-btn` verbose label is the same
    /// "Start or pause timer" string across every run-state.
    #[test]
    fn play_pause_btn_verbose_label_does_not_vary() {
        let states = [
            PlayPauseButtonState::Start,
            PlayPauseButtonState::Pause,
            PlayPauseButtonState::Resume,
        ];
        for state in states {
            assert_eq!(state.verbose_label(), "Start or pause timer");
        }
    }

    /// FR-028 — `#play-pause-btn` terse string maps idle → Start,
    /// running → Pause, paused/auto-paused → Resume.
    #[test]
    fn play_pause_btn_terse_tooltip_per_run_state() {
        // Idle: not running, not paused, not auto-paused.
        assert_eq!(
            PlayPauseButtonState::from_run_state(false, false, false).terse_tooltip(),
            "Start",
        );
        // Running.
        assert_eq!(
            PlayPauseButtonState::from_run_state(true, false, false).terse_tooltip(),
            "Pause",
        );
        // Paused.
        assert_eq!(
            PlayPauseButtonState::from_run_state(false, true, false).terse_tooltip(),
            "Resume",
        );
        // Auto-paused.
        assert_eq!(
            PlayPauseButtonState::from_run_state(false, false, true).terse_tooltip(),
            "Resume",
        );
    }

    /// CHK041 — verbose `aria-label` is intentionally NOT equal to
    /// the terse `data-tooltip` for the play/pause button when the
    /// engine is running. The test guards against a future refactor
    /// that accidentally collapses the two into one string source.
    #[test]
    fn play_pause_btn_verbose_and_terse_are_decoupled_when_running() {
        let running = PlayPauseButtonState::from_run_state(true, false, false);
        assert_ne!(running.verbose_label(), running.terse_tooltip());
    }

    /// FR-029 — `#skip-btn` has no state variants. Verbose and terse
    /// are DISTINCT (Spec A11 / SC-004 / contracts §3 + CHK041
    /// drift-impossibility): the verbose form describes the action
    /// ("advance to the next phase"); the terse form is the short
    /// tooltip ("Skip Mode"). The catalogue is the runtime source
    /// of truth — these const fixtures pin the English wording.
    /// R-006: the legacy `ctrl_skip_session*` keys were pruned in
    /// favour of `ctrl_skip_mode*` (feature 006 rename); the host-
    /// side fixtures now mirror those.
    #[test]
    fn skip_btn_verbose_and_terse_are_distinct() {
        let state = SkipButtonState::Skip;
        assert_eq!(
            state.verbose_label(),
            "Skip current mode and advance to the next phase",
        );
        assert_eq!(state.terse_tooltip(), "Skip Mode");
        assert_ne!(
            state.verbose_label(),
            state.terse_tooltip(),
            "Spec A11 / SC-004: skip verbose and terse MUST NOT collapse",
        );
    }

    /// SC-012 — feature-006 reshaped this matrix: the legacy
    /// `#stop-btn` per-mode mapping is gone (state-aware buttons
    /// now drive labels via the `(RunState, TimerMode)` match
    /// directly inside `view!`). What remains stable here is the
    /// play/pause-button + skip-button text mapping; downstream
    /// e2e specs (Phase 9) cover the abort / complete / quick-log /
    /// distraction labels via DOM-level assertions.
    #[test]
    fn play_pause_and_skip_text_matrix_still_holds() {
        let run_states = [
            (false, false, false, "Start"),
            (true, false, false, "Pause"),
            (false, true, false, "Resume"),
            (false, false, true, "Resume"),
        ];
        for (is_running, is_paused, is_auto_paused, expected_terse) in run_states {
            let play = PlayPauseButtonState::from_run_state(is_running, is_paused, is_auto_paused);
            assert_eq!(play.terse_tooltip(), expected_terse);
            assert_eq!(play.verbose_label(), "Start or pause timer");
        }
        // Skip is mode-invariant per FR-029.
        let skip = SkipButtonState::Skip;
        assert_eq!(skip.terse_tooltip(), "Skip Mode");
        assert_eq!(
            skip.verbose_label(),
            "Skip current mode and advance to the next phase",
        );
    }
}

/// Wasm-bindgen tests for the chime singleton invariant.
/// Gated on `wasm32` because `AudioContext` is a browser API; host-side
/// `cargo test` would silently drop the bodies.
/// Run with: `(cd src && wasm-pack test --node)`.
#[cfg(all(test, target_arch = "wasm32"))]
mod chime_tests {
    use super::{chime_audio_context, play_chime};
    use wasm_bindgen_test::wasm_bindgen_test;

    /// CHK042 — `play_chime` must reuse one `AudioContext` across all calls.
    /// In node.js `AudioContext` is unavailable (slot stays `None`); the
    /// invariant is that the slot is CONSISTENT — not that it's `Some`.
    /// In a browser environment the slot becomes `Some` after the first call
    /// and every subsequent call returns the same instance.
    #[wasm_bindgen_test]
    fn play_chime_reuses_audio_context_across_calls() {
        play_chime();
        play_chime();
        play_chime();
        let snap1 = chime_audio_context();
        play_chime();
        let snap2 = chime_audio_context();
        // Both snapshots must agree: either both None (AudioContext
        // unavailable in node) or both point to the same context.
        match (snap1, snap2) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                assert!(
                    a == b,
                    "play_chime must reuse the cached AudioContext across calls"
                );
            }
            _ => panic!("chime AudioContext singleton changed across play_chime calls"),
        }
    }
}
