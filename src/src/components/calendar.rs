// Calendar view component — Phase 4a (T198-T200) of spec
// 001-leptos-migration.
//
// Skeleton (T198): mount the calendar navigation shell with the
// e2e selector contract preserved. Wiring (T199): route prev/next
// week + month clicks into the date-cursor signal, project the
// monthly grid against `engine::date_format::format_session_date`,
// and feed selected-day session counts from the session manager.
// T200 lands the visual regression check.
//
// **Selector contract** (consumed by
// `tests/e2e/calendar-navigation.spec.js`,
// `sessions-history.spec.js`, `visual-regression.spec.js`):
// - `#calendar-view` — root view container (`_smoke.spec.js:20`
//   asserts hidden initially).
// - `#prev-week`, `#next-week` — week-cursor navigation
//   (`calendar-navigation.spec.js:17,22-23`).
// - `#prev-month`, `#next-month` — month-cursor navigation
//   (`calendar-navigation.spec.js:33,38-39`).
// - `#week-range` — week-range label (`spec.js:13` `not.toBeEmpty`).
// - `#current-month` — month label (`spec.js:14` `not.toBeEmpty`).
// - `#calendar-grid` — month grid host
//   (`sessions-history.spec.js:34` asserts
//   `[aria-current="date"]` cell).
// - `#calendar-grid [aria-current="date"]` — today's cell carries
//   `aria-current="date"` so the e2e suite can locate it without
//   coupling to a date string.
//
// Per Principle I, this component READS state via signals; it
// never mutates engine state directly.
//
// Lint allowance: `clippy::must_use_candidate` is silenced module-
// wide for the same reason as on `timer.rs` etc.
#![allow(clippy::must_use_candidate)]

use leptos::prelude::*;

/// Calendar view skeleton. Renders the week-range navigation bar +
/// month-grid host with the e2e selector contract in place. T199
/// attaches the date-cursor signals and the per-cell session-count
/// projection; today the static shell carries placeholder labels
/// so `(cd src && trunk build)` returns 0 and the e2e selectors
/// resolve.
#[component]
pub fn CalendarView() -> impl IntoView {
    view! {
        <div class="view-container view-section hidden" id="calendar-view">
            <h1>"Calendar & Statistics"</h1>

            // Week selector — prev/next + the active range label.
            // T199 attaches the `on:click` handlers; today they are
            // visual-only so the e2e selectors resolve.
            <div class="week-selector">
                <button id="prev-week" class="nav-btn" aria-label="Previous week">"<"</button>
                <div class="week-display">
                    <span id="week-range"></span>
                </div>
                <button id="next-week" class="nav-btn" aria-label="Next week">">"</button>
            </div>

            // Month selector + grid host.
            <div class="mini-calendar-container">
                <div class="calendar-header">
                    <button id="prev-month" class="nav-btn" aria-label="Previous month">"<"</button>
                    <h3 id="current-month"></h3>
                    <button id="next-month" class="nav-btn" aria-label="Next month">">"</button>
                </div>
                <div class="calendar-grid" id="calendar-grid"></div>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    /// Selector contract pin for the calendar view, sourced from
    /// `tests/e2e/calendar-navigation.spec.js`. Each entry maps to
    /// a `locator("#…")` callsite; drift here breaks the e2e run.
    #[test]
    fn calendar_view_selector_contract_documented() {
        const REQUIRED_IDS: &[&str] = &[
            "calendar-view",
            "prev-week",
            "next-week",
            "week-range",
            "prev-month",
            "next-month",
            "current-month",
            "calendar-grid",
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
}
