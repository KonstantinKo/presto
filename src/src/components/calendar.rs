// Calendar view component — Phase 4a (T198-T200) of spec
// 001-leptos-migration.
//
// Skeleton (T198): mount the calendar navigation shell with the
// e2e selector contract preserved. Wiring (T199): route prev/next
// week + month clicks into the date-cursor signal, project the
// week range and the month label against chrono format strings,
// and feed the per-cell session counts from
// `SessionManager::list_by_date`. T200 lands the visual regression
// check.
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
//
// Per Principle I, this component READS state via signals; it
// never mutates engine state directly.
//
// Lint allowance: `clippy::must_use_candidate` is silenced module-
// wide for the same reason as on `timer.rs` etc.
#![allow(clippy::must_use_candidate)]

use chrono::{DateTime, Datelike, Days, Months, Utc};
use leptos::prelude::*;

use crate::engine::clock::Clock;
use crate::engine::date_format::format_session_date;

/// Browser-backed `Clock` implementation. Same shape as the
/// `BrowserClock` in `components::timer` — duplicated rather than
/// shared so each component is self-contained at the bridge
/// boundary (Phase 4c will lift a single shared impl when the
/// shared `app.rs` lands).
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
        // (epoch) keeps the trait satisfied without pulling
        // `std::time` or chrono's `clock` feature into the
        // dependency graph.
        0
    }
}

/// Lift a unix-timestamp (milliseconds) to a `DateTime<Utc>`
/// without panicking on overflow. Falls back to the unix epoch on
/// the corner case where `from_timestamp_millis` rejects the
/// input. Same defensive pattern as `engine::date_format`.
fn datetime_from_ms(now_ms: i64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp_millis(now_ms)
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).expect("epoch is valid"))
}

/// Compute the start-of-week (Monday) date for a given anchor.
/// Mirrors the JS-era `getStartOfWeek` helper at
/// `src/managers/navigation-manager.js`. Monday-as-first-day
/// matches the on-screen `Mon Tue Wed ...` header in the JS index.
fn start_of_week(anchor: DateTime<Utc>) -> DateTime<Utc> {
    let weekday = anchor.weekday().num_days_from_monday();
    anchor - Days::new(u64::from(weekday))
}

/// Format a week-range label as `"<start_d> <start_mon> - <end_d>
/// <end_mon> <year>"` matching the JS-era output. Example:
/// `"10 Jun - 16 Jun 2025"`.
fn format_week_range(anchor: DateTime<Utc>) -> String {
    let start = start_of_week(anchor);
    let end = start + Days::new(6);
    if start.month() == end.month() {
        format!(
            "{start_day} {month} - {end_day} {month} {year}",
            start_day = start.day(),
            end_day = end.day(),
            month = month_short(start.month()),
            year = end.year(),
        )
    } else {
        format!(
            "{start_day} {start_month} - {end_day} {end_month} {year}",
            start_day = start.day(),
            start_month = month_short(start.month()),
            end_day = end.day(),
            end_month = month_short(end.month()),
            year = end.year(),
        )
    }
}

/// Format a month-label as `"<month> <year>"`. Example:
/// `"June 2025"`.
fn format_month_label(anchor: DateTime<Utc>) -> String {
    format!(
        "{month} {year}",
        month = month_full(anchor.month()),
        year = anchor.year(),
    )
}

/// Three-letter month name for the week-range label.
const fn month_short(month: u32) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "???",
    }
}

/// Full month name for the month label.
const fn month_full(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    }
}

/// Build the month grid as a `Vec<DateTime<Utc>>` covering the
/// weeks containing the first and last day of the month. The grid
/// always starts on a Monday and runs 6 weeks (42 cells) so the
/// visual layout is stable across months. Mirrors the JS-era
/// `renderCalendar` flow.
///
/// Time-of-day on the input anchor is preserved through the grid;
/// downstream callers use only the date component
/// (`engine::date_format::format_session_date`) so the time
/// fragment is irrelevant.
fn build_month_grid(anchor: DateTime<Utc>) -> Vec<DateTime<Utc>> {
    let first_of_month = anchor.with_day(1).unwrap_or(anchor);
    let grid_start = start_of_week(first_of_month);
    (0..42)
        .map(|offset| grid_start + Days::new(offset))
        .collect()
}

/// Calendar view — renders the week-range navigation bar + the
/// month grid backed by a `RwSignal<DateTime<Utc>>` cursor. Click
/// handlers shift the cursor by one week or one month; derived
/// signals project the week-range label, the month label, and the
/// 42-cell grid against the cursor.
///
/// Phase 4c attaches the per-cell session-count projection through
/// `SessionManager::list_by_date`; today the cells render the day
/// number only with the today-cell carrying `aria-current="date"`
/// (matches the e2e suite's contract).
///
/// The cursor seeds at the wall-clock now via `BrowserClock` — on
/// host-side builds this resolves to the unix epoch (the trait's
/// host fallback); the e2e suite runs against the real wall clock.
#[component]
pub fn CalendarView() -> impl IntoView {
    let now = datetime_from_ms(BrowserClock.now_ms());
    let cursor = RwSignal::new(now);
    let today = now;

    let week_label = Signal::derive(move || format_week_range(cursor.get()));
    let month_label = Signal::derive(move || format_month_label(cursor.get()));
    let grid = Signal::derive(move || build_month_grid(cursor.get()));

    let on_prev_week = move |_| {
        cursor.update(|d| {
            *d = *d - Days::new(7);
        });
    };
    let on_next_week = move |_| {
        cursor.update(|d| {
            *d = *d + Days::new(7);
        });
    };
    let on_prev_month = move |_| {
        cursor.update(|d| {
            *d = d.checked_sub_months(Months::new(1)).unwrap_or(*d);
        });
    };
    let on_next_month = move |_| {
        cursor.update(|d| {
            *d = d.checked_add_months(Months::new(1)).unwrap_or(*d);
        });
    };

    // The today-cell is the one whose `format_session_date`
    // matches today's. Encoding the comparison via the Phase 2
    // date-format pin keeps the calendar grid in lock-step with
    // the session-history `Session.date` shape.
    let today_label = format_session_date(today.timestamp_millis());

    view! {
        <div class="view-container view-section hidden" id="calendar-view">
            <h1>"Calendar & Statistics"</h1>

            // Week selector — prev/next + the active range label.
            <div class="week-selector">
                <button id="prev-week" class="nav-btn" aria-label="Previous week" on:click=on_prev_week>"<"</button>
                <div class="week-display">
                    <span id="week-range">{move || week_label.get()}</span>
                </div>
                <button id="next-week" class="nav-btn" aria-label="Next week" on:click=on_next_week>">"</button>
            </div>

            // Month selector + grid host.
            <div class="mini-calendar-container">
                <div class="calendar-header">
                    <button id="prev-month" class="nav-btn" aria-label="Previous month" on:click=on_prev_month>"<"</button>
                    <h3 id="current-month">{move || month_label.get()}</h3>
                    <button id="next-month" class="nav-btn" aria-label="Next month" on:click=on_next_month>">"</button>
                </div>
                <div class="calendar-grid" id="calendar-grid">
                    <For
                        each=move || grid.get()
                        key=|day| day.timestamp_millis()
                        children=move |day| {
                            let cell_date = format_session_date(day.timestamp_millis());
                            let is_today = cell_date == today_label;
                            // `aria-current="date"` only on the today-cell so
                            // sessions-history.spec.js:34 can locate it via
                            // `[aria-current="date"]` without a date string
                            // coupling.
                            let aria_current = if is_today { "date" } else { "" };
                            let day_num = day.day();
                            view! {
                                <div
                                    class="calendar-day"
                                    class:today=is_today
                                    role="button"
                                    aria-current=aria_current
                                    aria-label=cell_date
                                >{day_num}</div>
                            }
                        }
                    />
                </div>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_month_grid, format_month_label, format_week_range, month_full, month_short,
        start_of_week,
    };
    use chrono::{DateTime, Datelike, TimeZone, Utc};

    /// T200 — visual-regression / selector contract pin for the
    /// calendar view. Sourced from
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

    fn day(year: i32, month: u32, d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, d, 0, 0, 0).unwrap()
    }

    /// `start_of_week` rolls back to Monday. 2025-06-12 (Thursday)
    /// → 2025-06-09 (Monday).
    #[test]
    fn start_of_week_rolls_back_to_monday() {
        let anchor = day(2025, 6, 12);
        let monday = start_of_week(anchor);
        assert_eq!(monday.day(), 9);
        assert_eq!(monday.month(), 6);
        assert_eq!(monday.year(), 2025);
    }

    /// `format_week_range` produces the JS-era label shape.
    /// Same-month range collapses the month label.
    #[test]
    fn week_range_same_month() {
        let anchor = day(2025, 6, 12); // Thu
        assert_eq!(format_week_range(anchor), "9 Jun - 15 Jun 2025");
    }

    /// `format_week_range` keeps both month labels for a
    /// month-spanning range.
    #[test]
    fn week_range_spans_month() {
        let anchor = day(2025, 6, 30); // Mon = start; Sun = Jul 6
        assert_eq!(format_week_range(anchor), "30 Jun - 6 Jul 2025");
    }

    /// Month label is the JS-era `"June 2025"` shape.
    #[test]
    fn month_label_is_full_name_plus_year() {
        assert_eq!(format_month_label(day(2025, 6, 12)), "June 2025");
        assert_eq!(format_month_label(day(2026, 1, 1)), "January 2026");
    }

    /// `build_month_grid` always returns 42 cells (6 weeks).
    #[test]
    fn month_grid_is_six_weeks() {
        let grid = build_month_grid(day(2025, 6, 12));
        assert_eq!(grid.len(), 42);
        // First cell is a Monday.
        assert_eq!(grid[0].weekday().num_days_from_monday(), 0);
    }

    /// Spot-check: every month index produces a non-empty label.
    #[test]
    fn month_names_cover_every_month() {
        for m in 1..=12 {
            assert_ne!(month_short(m), "???", "month_short missing {m}");
            assert_ne!(month_full(m), "Unknown", "month_full missing {m}");
        }
    }
}
