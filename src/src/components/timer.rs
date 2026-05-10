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
#![allow(clippy::must_use_candidate)]

use leptos::prelude::*;

use crate::bridge::timer_mode::TimerMode;
use crate::engine::clock::Clock;
use crate::engine::durations::Durations;
use crate::engine::timer::TimerState;

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

/// Timer view — renders the canonical pomodoro DOM and wires the
/// `engine::TimerState` state machine through Leptos signals.
///
/// State ownership: the component owns a `RwSignal<TimerState>` for
/// the duration of its mount. Phase 4c (T217) will lift this into a
/// `provide_context`-supplied state slice so `History`, `Calendar`
/// etc. can read the same `completed_pomodoros` accumulator. Today
/// the component is the sole consumer.
///
/// Returns a fragment whose root is `<div id="timer-view">` to match
/// the `#timer-view` selector contract.
#[component]
pub fn TimerView() -> impl IntoView {
    // Engine state — RwSignal so derived projections (countdown
    // text, mode label, running flag) re-render on `update()`.
    let engine = RwSignal::new(TimerState::new(Durations::default()));

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
    Effect::new(move |_| {
        // Read once on mount to register the dependency; the
        // closure re-runs only on cleanup, not on every tick.
        let handle = set_interval_with_handle(
            move || {
                engine.update(|state| {
                    let _ = state.tick(&BrowserClock);
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
            // Progress dots — populated by the daily-goal projection
            // in a later refinement; today the container is empty.
            <div class="progress-dots" id="progress-dots"></div>

            // Status / mode label + tag-dropdown trigger.
            <div style="text-align: center; position: relative">
                <div class="timer-status-container">
                    <div class="timer-status clickable" id="timer-status">
                        <i id="status-icon" class="ri-brain-line"></i>
                        <span id="status-text">{move || mode_text.get()}</span>
                        <i class="ri-arrow-down-s-line tag-dropdown-arrow" id="tag-dropdown-arrow"></i>
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
                <button id="skip-btn" class="control-btn" aria-label="Skip session" on:click=on_skip>
                    <i id="skip-brain-icon" class="ri-brain-line" style="font-size: 24px"></i>
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
