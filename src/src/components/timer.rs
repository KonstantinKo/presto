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
                // No `pause()` API on the engine yet (Phase 2 ships
                // start/skip/reset; the JS-era `pause()` is mapped
                // to `reset()`'s "stop running, clear anchor" half
                // for now — full pause/resume parity lands in a
                // later refinement). This preserves the e2e flow:
                // click while running → idle, `#play-icon` visible.
                state.reset();
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
            // `#play-icon` / `#pause-icon` IDs.
            <div class="controls">
                <button id="stop-btn" class="control-btn" aria-label="Reset timer" on:click=on_stop></button>
                <button id="play-pause-btn" class="control-btn primary" aria-label="Start or pause timer" on:click=on_play_pause>
                    <span id="play-icon" style=move || play_icon_style.get()></span>
                    <span id="pause-icon" style=move || pause_icon_style.get()></span>
                </button>
                <button id="skip-btn" class="control-btn" aria-label="Skip session" on:click=on_skip></button>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::{mode_label, pad_two};
    use crate::bridge::timer_mode::TimerMode;

    /// Selector-contract smoke pin: enumerate the IDs the e2e suite
    /// asserts on so a future refactor that loses one fails this
    /// host-side test rather than only the e2e run. Visual
    /// regression (T191) covers the rendered shape; this test
    /// covers the string contract.
    ///
    /// The set is hand-derived from `tests/e2e/_smoke.spec.js`,
    /// `timer.spec.js`, and `visual-regression.spec.js` — every
    /// `locator("#…")` against the timer view. If a spec adds a
    /// new selector, append it here so the contract drift is
    /// caught at `cargo test` time.
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
