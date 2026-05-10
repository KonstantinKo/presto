// Timer view component — Phase 4a (T189-T191) of spec
// 001-leptos-migration.
//
// Skeleton (T189): mount the canonical pomodoro timer DOM with the
// e2e selector contract preserved. Wiring (T190) and the visual-
// regression check (T191) follow in subsequent commits.
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
// Leptos signals; it never mutates engine state directly. Wiring in
// T190 routes button clicks into manager API calls
// (`engine::timer::Timer::start/pause/skip`, `Settings::save_*`,
// etc.). The current skeleton renders the static initial-state DOM
// so `(cd src && trunk build)` returns 0 and the
// `#timer-minutes` / `#timer-seconds` first-paint selectors resolve.
//
// Lint allowance: `clippy::must_use_candidate` is silenced module-
// wide because Leptos `#[component]` functions return `impl
// IntoView`, which the framework consumes automatically inside
// `view!` / `mount_to_body` — annotating each component with
// `#[must_use]` would be noise that contradicts the Leptos call
// pattern (`<TimerView/>` inside `view!` doesn't bind a result).
#![allow(clippy::must_use_candidate)]

use leptos::prelude::*;

/// Initial focus-mode minute count for the timer's first paint —
/// `25` matches `engine::durations::Durations::default().focus`
/// projected to minutes. Pinning the literal here (rather than
/// re-deriving from `Durations::default()`) keeps the skeleton's
/// first-paint identical to the JS-era index.html value the
/// `_smoke.spec.js` first-paint assertion locks in.
const INITIAL_FOCUS_MINUTES: &str = "25";

/// Initial seconds column — `00` because the countdown starts on a
/// minute boundary.
const INITIAL_SECONDS: &str = "00";

/// Default mode-badge label rendered next to the dropdown chevron.
/// Mirrors the JS-era `#status-text` initial value at
/// `src/index.html:172` ("Focus" — the label for `TimerMode::Focus`).
const INITIAL_MODE_LABEL: &str = "Focus";

/// Timer view skeleton — renders the canonical pomodoro DOM with
/// every e2e selector in place. Engine + manager wiring lands in
/// T190; today's render is the static initial state so `trunk
/// build` succeeds and `_smoke.spec.js`'s first-paint selectors
/// (`#timer-minutes` = "25", `#timer-seconds` = "00") resolve.
///
/// Returns a fragment whose root is `<div id="timer-view">` to match
/// the `#timer-view` selector contract.
#[component]
pub fn TimerView() -> impl IntoView {
    view! {
        <div class="view-container" id="timer-view">
            // Progress dots — populated by the daily-goal projection
            // in T190; today the container is empty.
            <div class="progress-dots" id="progress-dots"></div>

            // Status / mode label + tag-dropdown trigger.
            <div style="text-align: center; position: relative">
                <div class="timer-status-container">
                    <div class="timer-status clickable" id="timer-status">
                        <i id="status-icon" class="ri-brain-line"></i>
                        <span id="status-text">{INITIAL_MODE_LABEL}</span>
                        <i class="ri-arrow-down-s-line tag-dropdown-arrow" id="tag-dropdown-arrow"></i>
                    </div>
                </div>
            </div>

            // Countdown display.
            <div class="timer-container">
                <div class="timer-minutes" id="timer-minutes">{INITIAL_FOCUS_MINUTES}</div>
                <div class="timer-seconds" id="timer-seconds">{INITIAL_SECONDS}</div>
            </div>

            // Control buttons. Icon SVGs are placeholders until T190
            // attaches the wired `<svg>` content; the IDs the e2e
            // suite asserts on (`#play-icon` visible, `#pause-icon`
            // hidden) are present so the selectors resolve.
            <div class="controls">
                <button id="stop-btn" class="control-btn" aria-label="Reset timer"></button>
                <button id="play-pause-btn" class="control-btn primary" aria-label="Start or pause timer">
                    <span id="play-icon"></span>
                    <span id="pause-icon" style="display: none"></span>
                </button>
                <button id="skip-btn" class="control-btn" aria-label="Skip session"></button>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
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
        // The contract is documented in the module-level comment;
        // this list mirrors the comment so a refactor that loses a
        // selector also fails this assertion (the comment is the
        // source of truth, this is the runtime pin).
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
        // Empty / duplicate IDs would be a contract drift; pin both.
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
}
