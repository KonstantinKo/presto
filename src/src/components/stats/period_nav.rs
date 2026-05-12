// Per-period navigator widget for the Statistics view.
//
// Branches on the active `Period` to emit the correct prev/next labels
// and range-label format. Preserves the existing `#prev-week` /
// `#next-week` / `#week-range` selectors for the Weekly variant (FR-009
// / A13); adds new selector IDs for Daily/Monthly/Yearly (FR-007).
//
// Selector contract (per FR-007 + FR-009):
// - Daily: `#prev-day` / `#next-day` / `#day-range`
// - Weekly: `#prev-week` / `#next-week` / `#week-range` (PRESERVED)
// - Monthly: `#prev-month-period` / `#next-month-period` / `#month-range`
// - Yearly: `#prev-year` / `#next-year` / `#year-range`

#![allow(
    clippy::must_use_candidate,
    clippy::too_many_lines,
    reason = "Leptos `#[component]` returning `impl IntoView`; body is a single `view!` macro expansion branching on `Period`. Matches `calendar.rs:32` precedent."
)]

use chrono::{DateTime, Datelike, Days, Months, Utc};
use leptos::prelude::*;

use super::period_selector::Period;

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

const MONTH_SHORT_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn month_short(month: u32) -> &'static str {
    let idx = month.saturating_sub(1) as usize;
    MONTH_SHORT_NAMES.get(idx).copied().unwrap_or("???")
}

fn month_full(month: u32) -> &'static str {
    let idx = month.saturating_sub(1) as usize;
    MONTH_FULL_NAMES.get(idx).copied().unwrap_or("Unknown")
}

/// Format the Daily range label: `"<Month> <Day> <Year>"`.
#[must_use]
pub fn format_day_range(anchor: DateTime<Utc>) -> String {
    format!(
        "{month} {day} {year}",
        month = month_short(anchor.month()),
        day = anchor.day(),
        year = anchor.year(),
    )
}

/// Format the Weekly range label: `"<MonStart> <D> - <MonEnd> <D> <Y>"`.
/// Anchor is treated as the Mon-Sun span containing it.
#[must_use]
pub fn format_week_range(anchor: DateTime<Utc>) -> String {
    let weekday = anchor.weekday().num_days_from_monday();
    let start = anchor - Days::new(u64::from(weekday));
    let end = start + Days::new(6);
    format!(
        "{start_month} {start_day} - {end_month} {end_day} {year}",
        start_day = start.day(),
        start_month = month_short(start.month()),
        end_day = end.day(),
        end_month = month_short(end.month()),
        year = end.year(),
    )
}

/// Format the Monthly range label: `"<Month> <Year>"`.
#[must_use]
pub fn format_month_range(anchor: DateTime<Utc>) -> String {
    format!(
        "{month} {year}",
        month = month_full(anchor.month()),
        year = anchor.year(),
    )
}

/// Format the Yearly range label: just the year.
#[must_use]
pub fn format_year_range(anchor: DateTime<Utc>) -> String {
    anchor.year().to_string()
}

/// Step a cursor by one unit of the given period. Direction `+1` for
/// next, `-1` for prev. Returns the original cursor if the step would
/// overflow (`checked_add_months` short-circuit).
#[must_use]
pub fn step_cursor(cursor: DateTime<Utc>, period: Period, forward: bool) -> DateTime<Utc> {
    match period {
        Period::Daily => {
            if forward {
                cursor + Days::new(1)
            } else {
                cursor - Days::new(1)
            }
        }
        Period::Weekly => {
            if forward {
                cursor + Days::new(7)
            } else {
                cursor - Days::new(7)
            }
        }
        Period::Monthly => {
            let step = Months::new(1);
            if forward {
                cursor.checked_add_months(step).unwrap_or(cursor)
            } else {
                cursor.checked_sub_months(step).unwrap_or(cursor)
            }
        }
        Period::Yearly => {
            let step = Months::new(12);
            if forward {
                cursor.checked_add_months(step).unwrap_or(cursor)
            } else {
                cursor.checked_sub_months(step).unwrap_or(cursor)
            }
        }
    }
}

/// Per-period navigator widget. Reads the current period and the
/// shared `cursor: RwSignal<DateTime<Utc>>` to render the right prev/
/// next selector IDs and the range-label format.
///
/// `StatisticsView` owns the cursor signal and reset-on-period-swap
/// behaviour (FR-008); this component is shape-only.
#[component]
pub fn PeriodNav(period: Signal<Period>, cursor: RwSignal<DateTime<Utc>>) -> impl IntoView {
    let range_label = Signal::derive(move || match period.get() {
        Period::Daily => format_day_range(cursor.get()),
        Period::Weekly => format_week_range(cursor.get()),
        Period::Monthly => format_month_range(cursor.get()),
        Period::Yearly => format_year_range(cursor.get()),
    });

    let on_prev = move |_| {
        let p = period.get();
        cursor.update(|c| *c = step_cursor(*c, p, false));
    };
    let on_next = move |_| {
        let p = period.get();
        cursor.update(|c| *c = step_cursor(*c, p, true));
    };

    view! {
        <div class="period-nav week-selector">
            {move || match period.get() {
                Period::Daily => view! {
                    <button id="prev-day" class="nav-btn" aria-label="Previous day" on:click=on_prev>"<"</button>
                    <div class="week-display">
                        <span id="day-range">{move || range_label.get()}</span>
                    </div>
                    <button id="next-day" class="nav-btn" aria-label="Next day" on:click=on_next>">"</button>
                }.into_any(),
                Period::Weekly => view! {
                    <button id="prev-week" class="nav-btn" aria-label="Previous week" on:click=on_prev>"<"</button>
                    <div class="week-display">
                        <span id="week-range">{move || range_label.get()}</span>
                    </div>
                    <button id="next-week" class="nav-btn" aria-label="Next week" on:click=on_next>">"</button>
                }.into_any(),
                Period::Monthly => view! {
                    <button id="prev-month-period" class="nav-btn" aria-label="Previous month" on:click=on_prev>"<"</button>
                    <div class="week-display">
                        <span id="month-range">{move || range_label.get()}</span>
                    </div>
                    <button id="next-month-period" class="nav-btn" aria-label="Next month" on:click=on_next>">"</button>
                }.into_any(),
                Period::Yearly => view! {
                    <button id="prev-year" class="nav-btn" aria-label="Previous year" on:click=on_prev>"<"</button>
                    <div class="week-display">
                        <span id="year-range">{move || range_label.get()}</span>
                    </div>
                    <button id="next-year" class="nav-btn" aria-label="Next year" on:click=on_next>">"</button>
                }.into_any(),
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::{format_day_range, format_month_range, format_week_range, format_year_range};
    use chrono::{TimeZone, Utc};

    #[test]
    fn day_range_formats_as_month_day_year() {
        let anchor = Utc.with_ymd_and_hms(2026, 5, 12, 12, 0, 0).unwrap();
        assert_eq!(format_day_range(anchor), "May 12 2026");
    }

    #[test]
    fn week_range_formats_mon_to_sun() {
        // 2026-05-09 is a Saturday; Mon-Sun span = May 4 - May 10.
        let anchor = Utc.with_ymd_and_hms(2026, 5, 9, 12, 0, 0).unwrap();
        assert_eq!(format_week_range(anchor), "May 4 - May 10 2026");
    }

    #[test]
    fn month_range_uses_full_name() {
        let anchor = Utc.with_ymd_and_hms(2026, 5, 12, 12, 0, 0).unwrap();
        assert_eq!(format_month_range(anchor), "May 2026");
    }

    #[test]
    fn year_range_is_year_only() {
        let anchor = Utc.with_ymd_and_hms(2026, 5, 12, 12, 0, 0).unwrap();
        assert_eq!(format_year_range(anchor), "2026");
    }
}
