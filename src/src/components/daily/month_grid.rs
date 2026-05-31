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
use leptos_i18n::t_string;

use crate::engine::date_format::format_session_date;
use crate::i18n::i18n::{use_i18n, Locale as I18nLocale};

type I18nCtx = leptos_i18n::I18nContext<I18nLocale>;

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

/// Localised month name lookup per `month_idx` (0 = January).
///
/// Feature 005: each branch is a static catalogue key so the
/// proc-macro can compile-time-check the lookup. Returns owned
/// `String` because the caller composes it into `"{name} {year}"`.
fn localised_month_name(i18n: I18nCtx, month_idx: usize) -> String {
    match month_idx {
        0 => t_string!(i18n, calendar.month_jan).to_string(),
        1 => t_string!(i18n, calendar.month_feb).to_string(),
        2 => t_string!(i18n, calendar.month_mar).to_string(),
        3 => t_string!(i18n, calendar.month_apr).to_string(),
        4 => t_string!(i18n, calendar.month_may).to_string(),
        5 => t_string!(i18n, calendar.month_jun).to_string(),
        6 => t_string!(i18n, calendar.month_jul).to_string(),
        7 => t_string!(i18n, calendar.month_aug).to_string(),
        8 => t_string!(i18n, calendar.month_sep).to_string(),
        9 => t_string!(i18n, calendar.month_oct).to_string(),
        10 => t_string!(i18n, calendar.month_nov).to_string(),
        11 => t_string!(i18n, calendar.month_dec).to_string(),
        _ => String::from("Unknown"),
    }
}

fn format_month_label(i18n: I18nCtx, anchor: DateTime<Utc>) -> String {
    let month_idx = anchor.month0() as usize;
    let name = localised_month_name(i18n, month_idx);
    format!("{name} {year}", year = anchor.year())
}

/// Day-of-week header row labels — Sunday-first. Per Fix B the column
/// order stays Sun-first across all locales; only the labels change.
fn day_name_for(i18n: I18nCtx, idx: usize) -> String {
    match idx {
        0 => t_string!(i18n, calendar.dow_sun).to_string(),
        1 => t_string!(i18n, calendar.dow_mon).to_string(),
        2 => t_string!(i18n, calendar.dow_tue).to_string(),
        3 => t_string!(i18n, calendar.dow_wed).to_string(),
        4 => t_string!(i18n, calendar.dow_thu).to_string(),
        5 => t_string!(i18n, calendar.dow_fri).to_string(),
        _ => t_string!(i18n, calendar.dow_sat).to_string(),
    }
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
    let i18n = use_i18n();
    let month_label = Signal::derive(move || format_month_label(i18n, month_cursor.get()));
    let grid = Signal::derive(move || build_month_grid(month_cursor.get()));
    let today_label = format_session_date(today.timestamp_millis());

    view! {
        <div class="mini-calendar-container">
            <div class="calendar-header">
                <button
                    id="prev-month"
                    class="nav-btn"
                    aria-label=move || t_string!(i18n, calendar.prev_month_aria)
                    on:click=move |_| on_prev_month.run(())
                >"<"</button>
                <h3 id="current-month">{move || month_label.get()}</h3>
                <button
                    id="next-month"
                    class="nav-btn"
                    aria-label=move || t_string!(i18n, calendar.next_month_aria)
                    on:click=move |_| on_next_month.run(())
                >">"</button>
            </div>
            // Day-of-week header row (Sun-first; FR-018). Per Fix B the
            // column order stays Sun-first across all locales; only the
            // labels change.
            <div class="calendar-grid calendar-day-names">
                {(0..7usize).map(|idx| {
                    view! { <div class="day-name">{move || day_name_for(i18n, idx)}</div> }
                }).collect_view()}
            </div>
            <div class="calendar-grid" id="calendar-grid">
                <For
                    each=move || grid.get()
                    key=|day| day.timestamp_millis()
                    children=move |day| {
                        let cell_date = format_session_date(day.timestamp_millis());
                        let is_today = cell_date == today_label;
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
                        let day_for_keydown = day;
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
                                class:other-month=move || day.month() != month_cursor.with(Datelike::month)
                                role="button"
                                tabindex="0"
                                aria-current=aria_current
                                aria-label=cell_date
                                on:click=move |_| {
                                    let _ = day_for_click;
                                    on_select_day.run(day_for_select);
                                }
                                on:keydown=move |ev| {
                                    let key = ev.key();
                                    if key == "Enter" || key == " " {
                                        ev.prevent_default();
                                        on_select_day.run(day_for_keydown);
                                    }
                                }
                            >
                                {move || if day.month() == month_cursor.with(Datelike::month) {
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

#[cfg(test)]
mod tests {
    use chrono::{Datelike, TimeZone, Utc};

    use super::{build_month_grid, next_month, prev_month, start_of_week_sunday};

    fn utc(year: i32, month: u32, day: u32) -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, 12, 0, 0)
            .single()
            .unwrap()
    }

    // ── start_of_week_sunday ─────────────────────────────────────────

    #[test]
    fn start_of_week_sunday_is_sunday_for_sunday_input() {
        // 2024-01-14 is a Sunday.
        let sun = utc(2024, 1, 14);
        let result = start_of_week_sunday(sun);
        assert_eq!(result.weekday().num_days_from_sunday(), 0);
        assert_eq!(result.day(), 14);
    }

    #[test]
    fn start_of_week_sunday_for_wednesday_returns_preceding_sunday() {
        // 2024-01-17 is a Wednesday; preceding Sunday is Jan 14.
        let wed = utc(2024, 1, 17);
        let result = start_of_week_sunday(wed);
        assert_eq!(result.weekday().num_days_from_sunday(), 0);
        assert_eq!(result.day(), 14);
        assert_eq!(result.month(), 1);
    }

    #[test]
    fn start_of_week_sunday_for_saturday_returns_preceding_sunday() {
        // 2024-01-20 is a Saturday; preceding Sunday is Jan 14.
        let sat = utc(2024, 1, 20);
        let result = start_of_week_sunday(sat);
        assert_eq!(result.day(), 14);
    }

    // ── build_month_grid ─────────────────────────────────────────────

    #[test]
    fn build_month_grid_always_42_cells() {
        // Test across several months to cover edge cases.
        for (year, month) in [(2024, 1), (2024, 2), (2023, 2), (2024, 12)] {
            let anchor = utc(year, month, 15);
            let grid = build_month_grid(anchor);
            assert_eq!(
                grid.len(),
                42,
                "expected 42 cells for {year}-{month:02}"
            );
        }
    }

    #[test]
    fn build_month_grid_first_cell_is_always_sunday() {
        let anchor = utc(2024, 1, 15);
        let grid = build_month_grid(anchor);
        assert_eq!(grid[0].weekday().num_days_from_sunday(), 0);
    }

    #[test]
    fn build_month_grid_cells_are_consecutive_days() {
        let anchor = utc(2024, 1, 15);
        let grid = build_month_grid(anchor);
        for window in grid.windows(2) {
            let diff = window[1] - window[0];
            assert_eq!(
                diff,
                chrono::Duration::days(1),
                "grid cells must be consecutive days"
            );
        }
    }

    #[test]
    fn build_month_grid_for_jan_2024_starts_dec_31_2023() {
        // Jan 1 2024 is a Monday; the preceding Sunday is Dec 31 2023.
        let anchor = utc(2024, 1, 1);
        let grid = build_month_grid(anchor);
        assert_eq!(grid[0].day(), 31);
        assert_eq!(grid[0].month(), 12);
        assert_eq!(grid[0].year(), 2023);
    }

    // ── prev_month / next_month ──────────────────────────────────────

    #[test]
    fn prev_month_steps_back_one_month() {
        let mar = utc(2024, 3, 15);
        let feb = prev_month(mar);
        assert_eq!(feb.month(), 2);
        assert_eq!(feb.year(), 2024);
    }

    #[test]
    fn prev_month_wraps_january_to_december() {
        let jan = utc(2024, 1, 15);
        let dec = prev_month(jan);
        assert_eq!(dec.month(), 12);
        assert_eq!(dec.year(), 2023);
    }

    #[test]
    fn next_month_steps_forward_one_month() {
        let jan = utc(2024, 1, 15);
        let feb = next_month(jan);
        assert_eq!(feb.month(), 2);
        assert_eq!(feb.year(), 2024);
    }

    #[test]
    fn next_month_wraps_december_to_january() {
        let dec = utc(2023, 12, 15);
        let jan = next_month(dec);
        assert_eq!(jan.month(), 1);
        assert_eq!(jan.year(), 2024);
    }
}
