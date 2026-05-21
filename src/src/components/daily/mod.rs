// Daily drill-down view — Bundle B of feature 003.
//
// Selector contract (preserved across the calendar.rs → daily/
// migration per FR-019 / A14 / CHK043):
// - `#daily-view` — root view container (FR-013)
// - `#calendar-grid` — month-grid host
// - `#prev-month` / `#next-month` / `#current-month` — month nav
// - `#sessions-timeline` / `#timeline-hours` / `#timeline-track`
// - `#selected-day-title` — timeline header
// - `#sessions-table-body` — sessions-history table (off-viewport)
// - `#session-modal-overlay` and the modal's inner IDs — the edit
//   modal moves with the table per CHK043.

#![allow(
    clippy::must_use_candidate,
    clippy::too_many_lines,
    reason = "Leptos `#[component]` returning `impl IntoView`; body is a single `view!` macro expansion (two-column layout). Matches `calendar.rs:32` precedent."
)]

pub mod day_clamp;
pub mod inventory;
pub mod month_grid;
pub mod sessions_history_table;
pub mod sessions_timeline;

use chrono::{DateTime, Datelike, Utc};
use leptos::prelude::*;
use leptos_i18n::t;

use self::day_clamp::clamp_day_to_month;
use self::inventory::Inventory;
use self::month_grid::{next_month, prev_month, MonthGrid};
use self::sessions_history_table::SessionsHistoryTable;
use self::sessions_timeline::SessionsTimeline;
use super::browser_clock::BrowserClock;
use super::utils::datetime::datetime_from_ms;
use crate::engine::clock::Clock;
use crate::i18n::i18n::use_i18n;

/// Daily drill-down view. Two-column layout: month-grid on the left,
/// sessions-timeline on the right; the off-viewport
/// sessions-history-card (table + edit modal) sits below.
///
/// State (per `data-model.md §Daily view state`):
/// - `month_cursor: RwSignal<DateTime<Utc>>` drives the grid.
/// - `selected_day: RwSignal<DateTime<Utc>>` drives the timeline and
///   the `.selected` cell highlight.
/// - Both seed from `datetime_from_ms(BrowserClock.now_ms())` on
///   cold-load.
///
/// Month-navigation rolls `selected_day` via `clamp_day_to_month` so
/// the same day-of-month carries over when the new month is long
/// enough, and clamps to the last day otherwise (FR-017, SC-008).
#[component]
pub fn DailyView() -> impl IntoView {
    let i18n = use_i18n();
    let now = datetime_from_ms(BrowserClock.now_ms());
    let month_cursor = RwSignal::new(now);
    let selected_day = RwSignal::new(now);
    let today = now;

    let on_prev_month = Callback::new(move |()| {
        month_cursor.update(|c| *c = prev_month(*c));
        // Roll selected_day to the same day-of-month in the new
        // month, clamping if the new month is shorter (FR-017).
        let new_cursor = month_cursor.get_untracked();
        let dom = selected_day.with_untracked(Datelike::day);
        selected_day.set(clamp_day_to_month(dom, new_cursor));
    });
    let on_next_month = Callback::new(move |()| {
        month_cursor.update(|c| *c = next_month(*c));
        let new_cursor = month_cursor.get_untracked();
        let dom = selected_day.with_untracked(Datelike::day);
        selected_day.set(clamp_day_to_month(dom, new_cursor));
    });
    let on_select_day = Callback::new(move |day: DateTime<Utc>| {
        // Set the selected-day signal unconditionally so the
        // `.selected` highlight follows the click. Only roll
        // `month_cursor` if the click lands in a *different* month —
        // mutating it on every in-month click drifts hours/minutes
        // forward each time the today-cell is re-clicked.
        selected_day.set(day);
        let current = month_cursor.get_untracked();
        if day.month() != current.month() || day.year() != current.year() {
            month_cursor.set(day);
        }
    });

    view! {
        <div class="view-container view-section" id="daily-view">
            <h1>{t!(i18n, daily.header)}</h1>

            <div class="daily-main-layout">
                <div class="daily-left-column">
                    <MonthGrid
                        month_cursor=month_cursor
                        selected_day=selected_day
                        today=today
                        on_prev_month=on_prev_month
                        on_next_month=on_next_month
                        on_select_day=on_select_day
                    />
                </div>
                <div class="daily-right-column">
                    <SessionsTimeline
                        selected_day=selected_day
                        today=today
                    />
                </div>
            </div>

            // Off-viewport sessions-history-card. Matches the
            // pre-rework calendar.rs position so the existing CSS
            // (`.sessions-history-card { ... off-viewport positioning ... }`)
            // still applies.
            <SessionsHistoryTable selected_day=selected_day />

            // Feature 006 (T055-T057): Inventory subsection — quick
            // logs + distractions for the selected day. Rendered
            // below the sessions-history-card per the spec.
            <Inventory selected_day=selected_day />
        </div>
    }
}
