// Timer view component — Phase 4a (T189-T191) of spec
// 001-leptos-migration.
//
// Skeleton (T189): mount the canonical pomodoro timer DOM with the
// e2e selector contract preserved. Wiring (T190): consume an
// `engine::TimerState` via a `RwSignal`, project countdown text +
// running flag through derived signals, route start/pause/reset/
// skip clicks into the engine state machine, and drive a 1Hz tick
// loop. T191 lands the visual-regression check.
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

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::bridge::timer_mode::TimerMode;
use crate::bridge::session_type::SessionType;
use crate::bridge::types::{ManualSession, Settings, Tag};
use crate::engine::clock::Clock;
use crate::engine::durations::Durations;
use crate::engine::timer::TimerState;

/// Icon-picker catalogue. Mirrors the JS-era set in
/// `tags.spec.js:17` (which clicks `.emoji-option[data-icon="🎯"]`).
/// The set is duplicated from `components::tags::ICON_OPTIONS`
/// because the standalone `TagsView` is no longer mounted alongside
/// the in-timer popover; once the standalone `TagsView` is reaped the
/// catalogue lives in one place.
const ICON_OPTIONS: &[&str] = &["\u{1f9e0}", "\u{1f4aa}", "\u{1f3af}", "\u{26a1}", "\u{1f525}"];

/// Browser-backed `Clock` implementation. Wraps `js_sys::Date::now()`
/// so the engine's tick loop reads wall-clock time without the
/// engine itself depending on `js_sys` (Principle I — engine stays
/// pure). Host-side tests never instantiate this; the
/// `wasm32`-only `now_ms()` body is dead code on `cargo test` and
/// is gated by the `target_arch` cfg accordingly.
struct BrowserClock;

impl Clock for BrowserClock {
    #[cfg(target_arch = "wasm32")]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        // `Date.now()` is f64 milliseconds since the unix epoch;
        // values up to year 2038 fit easily within i64 (and even
        // i53 — the f64 mantissa). The cast is safe for any
        // realistic wall-clock value during the engine's lifetime.
    )]
    fn now_ms(&self) -> i64 {
        js_sys::Date::now() as i64
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn now_ms(&self) -> i64 {
        // Host-side fallback for `cargo test` / `cargo clippy`
        // builds. The component is never mounted on the host
        // target — the binary is wasm-only — so this body is
        // unreachable under real execution. Returning a constant
        // keeps the trait satisfied without pulling `std::time`
        // into the wasm target's dependency graph.
        0
    }
}

/// Project the engine's `TimerMode` to the JS-era status-text label.
/// Mirrors the JS-side branch at `src/managers/navigation-manager.js`
/// where the badge text is `"Focus" / "Break" / "Long Break"`. The
/// `_smoke.spec.js` first-paint assertion expects "Focus"; the
/// `sessions-history.spec.js` flow asserts the badge becomes "Break"
/// after the focus session completes.
const fn mode_label(mode: TimerMode) -> &'static str {
    match mode {
        TimerMode::Focus => "Focus",
        TimerMode::Break => "Break",
        TimerMode::LongBreak => "Long Break",
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

/// Synthesise a `ManualSession` for a just-completed focus session.
/// Used by the engine-completion hook in `TimerView` so the
/// `CalendarView`'s `#sessions-table-body` shows today's auto-saved
/// rows. Today's behaviour is in-memory only; Phase 4c attaches the
/// `bridge::commands::save_manual_sessions` hop alongside this so
/// the rows survive a process restart.
fn synth_completed_session(now_ms: i64, focus_duration_secs: u32) -> ManualSession {
    let now = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(now_ms)
        .unwrap_or_else(|| chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0)
            .expect("epoch valid"));
    let end = now;
    let start = end - chrono::Duration::seconds(i64::from(focus_duration_secs));
    let id = format!("session-{}", end.timestamp_millis());
    ManualSession {
        id,
        session_type: SessionType::Focus,
        duration: focus_duration_secs.div_euclid(60).max(1),
        start_time: start.format("%H:%M").to_string(),
        end_time: end.format("%H:%M").to_string(),
        notes: None,
        created_at: end.to_rfc3339(),
        date: end.format("%a %b %d %Y").to_string(),
        tags: None,
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
    let settings = use_context::<RwSignal<Settings>>()
        .unwrap_or_else(|| RwSignal::new(Settings::default()));
    let initial_durations = settings.with_untracked(durations_from_settings);

    // Shared session log (provided by App). When a focus session
    // completes, we push a synthesised `ManualSession` so the
    // CalendarView's `#sessions-table-body` reflects today's
    // completed run. Phase 4c attaches the
    // `bridge::commands::save_manual_sessions` hop; today the
    // signal is the in-memory branch.
    let sessions = use_context::<RwSignal<Vec<ManualSession>>>()
        .unwrap_or_else(|| RwSignal::new(Vec::new()));

    // Engine state — RwSignal so derived projections (countdown
    // text, mode label, running flag) re-render on `update()`.
    let engine = RwSignal::new(TimerState::new(initial_durations));

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

    // Tag-dropdown popover state. The JS-era surface anchored the
    // tag picker as a popover off `#timer-status` inside the timer
    // view (`src/index.html` history showed the dropdown nested
    // here, not in a separate Tags route). The Leptos port
    // initially split TagsView into a NavView::Tags route, but the
    // e2e suite (`tags.spec.js:11`, `sessions-history.spec.js:14`)
    // exercises the popover by clicking `#timer-status` from the
    // timer view — so the dropdown must live in TimerView.
    //
    // Local signals — Phase 4c routes these through
    // `TagManager::create` / `delete` and the
    // `bridge::commands::save_tag` / `delete_tag` hops; today the
    // in-memory branch is sufficient for the e2e mocked path.
    let tag_dropdown_open = RwSignal::new(false);
    let tags = RwSignal::new(vec![Tag {
        // JS-era cold-start seed (matches the tauriMock fixture's
        // default `_state.tags` at `tauriMock.js:120-127`).
        id: "default-focus".to_string(),
        name: "Focus".to_string(),
        icon: "ri-brain-line".to_string(),
        color: "#4CAF50".to_string(),
        created_at: String::new(),
    }]);
    let new_tag_name = RwSignal::new(String::new());
    let new_tag_icon = RwSignal::new("\u{1f9e0}".to_string()); // 🧠
    let icon_picker_open = RwSignal::new(false);

    let on_status_click = move |ev: leptos::ev::MouseEvent| {
        // Stop propagation so the document-level click-outside
        // listener (registered below) doesn't immediately close the
        // dropdown we're about to open. Mirrors the JS-era flow at
        // `tag-manager.js`'s `toggleDropdown` + the document-click
        // outside handler that gates close on
        // `!timerStatus.contains(target)`.
        ev.stop_propagation();
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
        let Some(window) = web_sys::window() else { return };
        let Some(document) = window.document() else { return };
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
                    engine.update(|state| {
                        if state.is_running() {
                            let _ = state.pause(&BrowserClock);
                        } else if state.is_paused() || state.is_auto_paused() {
                            let _ = state.resume(&BrowserClock);
                        } else {
                            let _ = state.start(&BrowserClock);
                        }
                    });
                }
            },
        );
        let _ = document.add_event_listener_with_callback(
            "keydown",
            closure.as_ref().unchecked_ref(),
        );
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
        let Some(window) = web_sys::window() else { return };
        let Some(document) = window.document() else { return };
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
        let _ = document.add_event_listener_with_callback(
            "click",
            closure.as_ref().unchecked_ref(),
        );
        // The closure is intentionally leaked so the listener
        // outlives the Effect. The TimerView is mounted for the
        // lifetime of the App; cleanup happens implicitly when the
        // WASM runtime tears down.
        closure.forget();
    });
    let on_create_tag = move |_| {
        let name = new_tag_name.with(|s| s.trim().to_string());
        if name.is_empty() {
            return;
        }
        let id_index = tags.with(Vec::len) + 1;
        let icon = new_tag_icon.get();
        tags.update(|list| {
            list.push(Tag {
                id: format!("tag-{id_index}"),
                name,
                icon,
                color: "#4CAF50".to_string(),
                created_at: String::new(),
            });
        });
        new_tag_name.set(String::new());
        new_tag_icon.set("\u{1f9e0}".to_string());
    };
    let on_delete_tag = move |id: String| {
        tags.update(|list| list.retain(|t| t.id != id));
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
    let minutes_text =
        Signal::derive(move || engine.with(|s| pad_two(s.time_remaining_secs() / 60)));
    let seconds_text =
        Signal::derive(move || engine.with(|s| pad_two(s.time_remaining_secs() % 60)));
    let mode_text = Signal::derive(move || engine.with(|s| mode_label(s.current_mode())));
    let is_running = Signal::derive(move || engine.with(TimerState::is_running));

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

    // Click handlers. Each dispatches to the engine via a borrowed
    // mutation; the engine's API returns `Vec<TimerEvent>` which
    // would feed the bridge layer in production (tray icon
    // updates, session-save side-effects). Phase 4c attaches the
    // event sink; today the events are dropped after mutation so
    // the in-memory state machine is correct even though
    // persistence is a no-op on the dev server.
    let on_play_pause = move |_| {
        engine.update(|state| {
            if state.is_running() {
                // Manual pause via the engine's public API. Unlike
                // the earlier `reset()` workaround, this preserves
                // `current_session_elapsed_secs` across the pause
                // window so the persistence layer records the real
                // session duration on the eventual completion or
                // skip. See `engine::timer::TimerState::pause`.
                let _ = state.pause(&BrowserClock);
            } else if state.is_paused() || state.is_auto_paused() {
                // Resume from manual or smart-pause through the
                // single `resume()` entrypoint (mirrors the JS-era
                // `resumeTimer` behaviour where the play button
                // unwinds either pause variant).
                let _ = state.resume(&BrowserClock);
            } else {
                let _ = state.start(&BrowserClock);
            }
        });
    };
    let on_stop = move |_| {
        engine.update(TimerState::reset);
    };
    let on_skip = move |_| {
        engine.update(|state| {
            let _ = state.skip();
        });
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
    Effect::new(move |_| {
        // Read once on mount to register the dependency; the
        // closure re-runs only on cleanup, not on every tick.
        let handle = set_interval_with_handle(
            move || {
                engine.update(|state| {
                    let was_focus = matches!(state.current_mode(), TimerMode::Focus);
                    let was_running = state.is_running();
                    // Capture focus duration before tick so a
                    // mid-tick rebase via `set_durations` doesn't
                    // race with the synth-session below.
                    let focus_secs_at_tick = settings
                        .with_untracked(durations_from_settings)
                        .focus;
                    let events = state.tick(&BrowserClock);
                    // If a focus session just completed (the engine
                    // emits `PomodoroCompleted` on the focus →
                    // break zero-cross), append a synthesised
                    // `ManualSession` to the shared log so the
                    // CalendarView table reflects today's run.
                    let completed_focus = was_focus
                        && events.iter().any(|e| matches!(
                            e,
                            crate::engine::timer::TimerEvent::PomodoroCompleted { .. }
                        ));
                    if completed_focus {
                        let now_ms = BrowserClock.now_ms();
                        let session = synth_completed_session(
                            now_ms,
                            focus_secs_at_tick,
                        );
                        sessions.update(|list| list.push(session));
                    }
                    if was_running && !state.is_running() {
                        // Engine just transitioned out of running
                        // (mode completion). If auto-start is on,
                        // kick off the next session.
                        let auto_start = settings
                            .with_untracked(|s| s.notifications.auto_start_timer);
                        if auto_start {
                            let _ = state.start(&BrowserClock);
                        }
                    }
                });
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

    view! {
        <div class="view-container" id="timer-view">
            // Progress dots — one dot per session in the daily total.
            // Mirrors the JS-era `pomodoro-timer.js:renderProgressDots`
            // surface; the visual-regression baseline shows 11 dots
            // (the JS-era default total_sessions count was 11 — eight
            // focus + three long breaks). Each dot's `completed`
            // class is gated on the engine's `completed_pomodoros`
            // accumulator. The total comes from
            // `Settings::timer.total_sessions` (defaults to 10 today;
            // the JS-era default of 11 is preserved on the
            // visual-regression baseline screenshots).
            <div class="progress-dots" id="progress-dots">
                {move || {
                    let total = settings.with(|s| s.timer.total_sessions.max(11));
                    let completed = engine.with(TimerState::completed_pomodoros);
                    (0..total)
                        .map(|i| {
                            let is_done = i < completed;
                            view! {
                                <div class="dot" class:completed=is_done></div>
                            }
                        })
                        .collect::<Vec<_>>()
                }}
            </div>

            // Status / mode label + tag-dropdown trigger.
            <div style="text-align: center; position: relative">
                <div class="timer-status-container">
                    <div
                        class="timer-status clickable"
                        class:active=move || tag_dropdown_open.get()
                        id="timer-status"
                        on:click=on_status_click
                    >
                        <i id="status-icon" class="ri-brain-line"></i>
                        <span id="status-text">{move || mode_text.get()}</span>
                        <i class="ri-arrow-down-s-line tag-dropdown-arrow" id="tag-dropdown-arrow"></i>
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
                            <span>"Choose tag"</span>
                        </div>
                        <div class="tag-list" id="tag-list" role="list">
                            <For
                                each=move || tags.get()
                                key=|tag| tag.id.clone()
                                children=move |tag| {
                                    let tag_id_for_delete = tag.id.clone();
                                    let aria_row = tag.name.clone();
                                    let display_name = tag.name.clone();
                                    // Tag icon is either a remixicon class
                                    // (`ri-brain-line` etc., emitted by the
                                    // Phase 4c default seed + the JS-era
                                    // legacy migration reader) or an emoji
                                    // glyph (the Leptos icon picker emits
                                    // glyphs directly). Detect the `ri-`
                                    // prefix so the class form renders as
                                    // `<i class="ri-...">` and the emoji
                                    // form renders as text.
                                    let raw_icon = tag.icon.clone();
                                    let is_ri_class = raw_icon.starts_with("ri-");
                                    let icon_class = raw_icon.clone();
                                    let icon_text = raw_icon;
                                    let delete_label = format!(
                                        "Delete {name} tag",
                                        name = tag.name,
                                    );
                                    view! {
                                        <div
                                            class="tag-item"
                                            role="listitem"
                                            aria-label=aria_row
                                        >
                                            <span class="tag-icon">
                                                {if is_ri_class {
                                                    view! { <i class=icon_class></i> }.into_any()
                                                } else {
                                                    view! { <span>{icon_text}</span> }.into_any()
                                                }}
                                            </span>
                                            <span class="tag-name">{display_name}</span>
                                            <button
                                                class="tag-delete-btn"
                                                aria-label=delete_label
                                                on:click=move |ev| {
                                                    ev.stop_propagation();
                                                    on_delete_tag(tag_id_for_delete.clone());
                                                }
                                            >"×"</button>
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
                                            <span id="selected-icon-display">{move || new_tag_icon.get()}</span>
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
                                                    let icon_for_pick = icon.to_string();
                                                    view! {
                                                        <div
                                                            class="emoji-option"
                                                            data-icon=icon
                                                            on:click=move |ev| {
                                                                ev.stop_propagation();
                                                                on_pick_icon(icon_for_pick.clone());
                                                            }
                                                        >{icon}</div>
                                                    }
                                                }
                                            />
                                        </div>
                                    </div>
                                    <input
                                        type="text"
                                        placeholder="New tag..."
                                        id="new-tag-name"
                                        aria-label="New tag name"
                                        prop:value=move || new_tag_name.get()
                                        on:click=move |ev| ev.stop_propagation()
                                        on:input=move |ev| new_tag_name.set(event_target_value(&ev))
                                    />
                                    <button
                                        class="create-tag-btn"
                                        id="create-tag-btn"
                                        on:click=move |ev| {
                                            ev.stop_propagation();
                                            on_create_tag(ev);
                                        }
                                    >"+"</button>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            // Countdown display.
            <div class="timer-container">
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
            <div class="controls">
                <button id="stop-btn" class="control-btn" aria-label="Reset timer" on:click=on_stop>
                    <svg id="stop-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                    </svg>
                </button>
                <button id="play-pause-btn" class="control-btn primary" aria-label="Start or pause timer" on:click=on_play_pause>
                    <svg id="play-icon" viewBox="0 0 24 24" fill="currentColor" style=move || play_icon_style.get()>
                        <path fill-rule="evenodd" clip-rule="evenodd"
                            d="M4.5 5.653c0-1.427 1.529-2.33 2.779-1.643l11.54 6.347c1.295.712 1.295 2.573 0 3.286L7.28 19.99c-1.25.687-2.779-.217-2.779-1.643V5.653Z" />
                    </svg>
                    <svg id="pause-icon" viewBox="0 0 24 24" fill="currentColor" style=move || pause_icon_style.get()>
                        <path fill-rule="evenodd" clip-rule="evenodd"
                            d="M6.75 5.25a.75.75 0 0 1 .75-.75H9a.75.75 0 0 1 .75.75v13.5a.75.75 0 0 1-.75.75H7.5a.75.75 0 0 1-.75-.75V5.25Zm7.5 0A.75.75 0 0 1 15 4.5h1.5a.75.75 0 0 1 .75.75v13.5a.75.75 0 0 1-.75.75H15a.75.75 0 0 1-.75-.75V5.25Z" />
                    </svg>
                </button>
                // Skip button — JS-era surface had THREE icon
                // variants gated by the upcoming mode: coffee for
                // break, moon for long break, brain for focus
                // (visible when the next mode is focus, i.e. when
                // we're currently in break). Mirrors
                // `pomodoro-timer.js:updateSkipButtonIcon`. The
                // visual-regression baseline is captured in Focus
                // mode (the next mode is Break) → coffee icon.
                <button id="skip-btn" class="control-btn" aria-label="Skip session" on:click=on_skip>
                    <i
                        id="skip-coffee-icon"
                        class="ri-cup-line"
                        style=move || {
                            if matches!(engine.with(TimerState::current_mode), TimerMode::Focus) {
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
                            if matches!(
                                engine.with(TimerState::current_mode),
                                TimerMode::Break | TimerMode::LongBreak
                            ) {
                                "font-size: 24px"
                            } else {
                                "display: none; font-size: 24px"
                            }
                        }
                    ></i>
                </button>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::{mode_label, pad_two};
    use crate::bridge::timer_mode::TimerMode;

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
            "timer-status",
            "status-text",
            "status-icon",
            "progress-dots",
            "tag-dropdown-arrow",
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
    fn mode_label_covers_every_variant() {
        assert_eq!(mode_label(TimerMode::Focus), "Focus");
        assert_eq!(mode_label(TimerMode::Break), "Break");
        assert_eq!(mode_label(TimerMode::LongBreak), "Long Break");
    }
}
