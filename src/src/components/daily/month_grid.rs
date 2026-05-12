// Month-grid component for the Daily view — Bundle B of feature 003.
//
// Mirrors the structure of `components::calendar`'s mini-calendar
// grid block (lines 542–602 pre-rework) but with a `selected_day`
// dimension: the clicked cell carries a `.selected` CSS modifier
// distinct from the today-cell's `.today` highlight.
//
// Selector contract preserved per FR-019 / A14:
// - `#calendar-grid` — the 42-cell month-grid host
// - `#prev-month` / `#next-month` / `#current-month` — the navigation
//   header
// - `aria-current="date"` on the today-cell (FR-018)

#![allow(
    clippy::must_use_candidate,
    clippy::too_many_lines,
    reason = "Leptos `#[component]` functions returning `impl IntoView` are consumed by the parent `view!` macro; `#[must_use]` is implicit. The body is a single `view!` macro expansion (grid header + 42 day cells + click handler) plus a small derived-signal cluster; splitting it would fragment the JSX-style DOM tree without aiding readability — same justification as `calendar.rs:32`."
)]

use chrono::{DateTime, Datelike, Days, Months, Utc};
use leptos::prelude::*;

use crate::engine::date_format::format_session_date;

const DAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTH_FULL_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

fn start_of_week_sunday(anchor: DateTime<Utc>) -> DateTime<Utc> {
    let weekday = anchor.weekday().num_days_from_sunday();
    anchor - Days::new(u64::from(weekday))
}

fn build_month_grid(anchor: DateTime<Utc>) -> Vec<DateTime<Utc>> {
    let first_of_month = anchor.with_day(1).unwrap_or(anchor);
    let grid_start = start_of_week_sunday(first_of_month);
    (0..42)
        .map(|offset| grid_start + Days::new(offset))
        .collect()
}

fn format_month_label(anchor: DateTime<Utc>) -> String {
    let month_idx = anchor.month0() as usize;
    let name = MONTH_FULL_NAMES
        .get(month_idx)
        .copied()
        .unwrap_or("Unknown");
    format!("{name} {year}", year = anchor.year())
}

/// Month-grid component for the Daily view. The grid runs Sunday-
/// first to match the existing baseline header row (`Sun Mon Tue Wed
/// Thu Fri Sat`).
///
/// State:
/// - `month_cursor` drives which month's grid is rendered. Prev/next
///   buttons step it by one month via `Months::new(1)`.
/// - `selected_day` highlights the clicked cell with the `.selected`
///   modifier class. The today-cell carries the `.today` modifier;
///   when today and selected are different cells, both classes are
///   visually distinct.
/// - `today` is a constant per-render anchor (captured from the
///   caller's `BrowserClock` seed) used to mark the today-cell with
///   `aria-current="date"`.
///
/// Clicking a day cell calls `on_select_day` with the clicked cell's
/// `DateTime<Utc>`; the caller is responsible for routing this
/// through `clamp_day_to_month` on subsequent month-nav events
/// (handled in the parent `DailyView`).
#[component]
pub fn MonthGrid(
    month_cursor: RwSignal<DateTime<Utc>>,
    selected_day: RwSignal<DateTime<Utc>>,
    today: DateTime<Utc>,
    #[prop(into)] on_prev_month: Callback<()>,
    #[prop(into)] on_next_month: Callback<()>,
    #[prop(into)] on_select_day: Callback<DateTime<Utc>>,
) -> impl IntoView {
    let month_label = Signal::derive(move || format_month_label(month_cursor.get()));
    let grid = Signal::derive(move || build_month_grid(month_cursor.get()));
    let today_label = format_session_date(today.timestamp_millis());

    view! {
        <div class="mini-calendar-container">
            <div class="calendar-header">
                <button
                    id="prev-month"
                    class="nav-btn"
                    aria-label="Previous month"
                    on:click=move |_| on_prev_month.run(())
                >"<"</button>
                <h3 id="current-month">{move || month_label.get()}</h3>
                <button
                    id="next-month"
                    class="nav-btn"
                    aria-label="Next month"
                    on:click=move |_| on_next_month.run(())
                >">"</button>
            </div>
            // Day-of-week header row (Sun-first; FR-018).
            <div class="calendar-grid calendar-day-names">
                {DAY_NAMES.iter().map(|name| view! { <div class="day-name">{*name}</div> }).collect_view()}
            </div>
            <div class="calendar-grid" id="calendar-grid">
                <For
                    each=move || grid.get()
                    key=|day| day.timestamp_millis()
                    children=move |day| {
                        let cell_date = format_session_date(day.timestamp_millis());
                        let is_today = cell_date == today_label;
                        let cursor_month = month_cursor.with(Datelike::month);
                        let in_current_month = day.month() == cursor_month;
                        // `aria-current="date"` only on the today-cell so
                        // sessions-history.spec.js:34 can locate it without
                        // a date-string coupling. Per ARIA, an empty string
                        // is invalid; emit the attribute only on the
                        // today-cell via `Option<&str>` (None omits it).
                        let aria_current: Option<&'static str> =
                            if is_today { Some("date") } else { None };
                        let day_num = day.day();
                        let day_for_click = day;
                        let day_for_select = day;
                        let day_ts = day.timestamp_millis();
                        // `.selected` flips when the cell's date matches
                        // selected_day's date (per `format_session_date`).
                        let is_selected = Signal::derive(move || {
                            let sel_label = format_session_date(
                                selected_day.with(chrono::DateTime::timestamp_millis),
                            );
                            format_session_date(day_ts) == sel_label
                        });
                        view! {
                            <div
                                class="calendar-day"
                                class:today=is_today
                                class:selected=move || is_selected.get()
                                class:other-month=move || !in_current_month
                                role="button"
                                aria-current=aria_current
                                aria-label=cell_date
                                on:click=move |_| {
                                    let _ = day_for_click;
                                    on_select_day.run(day_for_select);
                                }
                            >
                                {if in_current_month {
                                    view! { <span class="calendar-day-number">{day_num}</span> }.into_any()
                                } else {
                                    view! { <span></span> }.into_any()
                                }}
                            </div>
                        }
                    }
                />
            </div>
        </div>
    }
}

/// Shared helper: step a cursor backward by one month, applying the
/// day-clamp helper to land on the same day-of-month (or the last
/// valid day if the target month is shorter).
pub fn prev_month(cursor: DateTime<Utc>) -> DateTime<Utc> {
    cursor.checked_sub_months(Months::new(1)).unwrap_or(cursor)
}

/// Step forward by one month — same clamp story as `prev_month`.
pub fn next_month(cursor: DateTime<Utc>) -> DateTime<Utc> {
    cursor.checked_add_months(Months::new(1)).unwrap_or(cursor)
}
