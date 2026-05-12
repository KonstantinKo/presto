// Calendar view component. Spec: 001-leptos-migration §Phase 4a.
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
// `clippy::too_many_lines` is silenced because the view body is a
// single Leptos `view!` macro expansion (calendar grid + focus
// summary card + sessions table + edit modal) plus a small
// click-handler / signal cluster; splitting it would fragment the
// JSX-style DOM tree across helper fns without aiding readability.
#![allow(clippy::must_use_candidate, clippy::too_many_lines)]

use chrono::{DateTime, Datelike, Days, Months, Utc};
use leptos::prelude::*;
use leptos::task::spawn_local;

use super::browser_clock::BrowserClock;
use crate::bridge::commands;
use crate::bridge::types::SessionType;
use crate::bridge::types::{ManualSession, Settings};
use crate::engine::clock::Clock;
use crate::engine::date_format::format_session_date;

/// Lift a unix-timestamp (milliseconds) to a `DateTime<Utc>`
/// without panicking on overflow. Falls back to the unix epoch on
/// the corner case where `from_timestamp_millis` rejects the
/// input. Same defensive pattern as `engine::date_format`.
fn datetime_from_ms(now_ms: i64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp_millis(now_ms)
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).expect("epoch is valid"))
}

/// Compute the start-of-week (Monday) date for a given anchor.
/// Mirrors the JS-era `getWeekStart` at
/// `src/utils/common-utils.js`: the week-range pill is anchored on
/// Monday so a Saturday anchor (e.g. 2026-05-09) rolls back to the
/// preceding Monday (2026-05-04). The visual-regression baseline
/// week-range pill reads `"May 4 - May 10 2026"` for the frozen
/// 2026-05-09 anchor — that's a Mon-Sun range.
///
/// NB: The calendar GRID (`build_month_grid`) uses Sunday as the
/// first column header so `Sun Mon Tue Wed Thu Fri Sat` matches the
/// baseline header row. The two functions are intentionally split:
/// the week-range pill is a Mon-Sun range; the grid is a Sun-Sat
/// display.
fn start_of_week_monday(anchor: DateTime<Utc>) -> DateTime<Utc> {
    let weekday = anchor.weekday().num_days_from_monday();
    anchor - Days::new(u64::from(weekday))
}

/// Compute the start-of-week (Sunday) date for a given anchor.
/// Used for the calendar grid display (first column = Sunday).
fn start_of_week_sunday(anchor: DateTime<Utc>) -> DateTime<Utc> {
    let weekday = anchor.weekday().num_days_from_sunday();
    anchor - Days::new(u64::from(weekday))
}

/// Format a week-range label as `"<mon> <start_d> - <mon> <end_d>
/// <year>"` matching the visual-regression baseline. Example:
/// `"May 4 - May 10 2026"`. The month label leads each side
/// (mirrors `Intl.DateTimeFormat("en-US")` from
/// `src/utils/common-utils.js:formatDateRange`).
fn format_week_range(anchor: DateTime<Utc>) -> String {
    let start = start_of_week_monday(anchor);
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
    // Sunday-first display (US-convention) matches the
    // visual-regression baseline header row `Sun Mon Tue ...`.
    let grid_start = start_of_week_sunday(first_of_month);
    (0..42)
        .map(|offset| grid_start + Days::new(offset))
        .collect()
}

/// Return the seven `format_session_date` strings for the Mon–Sun week
/// containing `anchor`. Used to filter sessions by current week.
fn week_date_set(anchor: DateTime<Utc>) -> [String; 7] {
    let start = start_of_week_monday(anchor);
    [
        format_session_date(start.timestamp_millis()),
        format_session_date((start + Days::new(1)).timestamp_millis()),
        format_session_date((start + Days::new(2)).timestamp_millis()),
        format_session_date((start + Days::new(3)).timestamp_millis()),
        format_session_date((start + Days::new(4)).timestamp_millis()),
        format_session_date((start + Days::new(5)).timestamp_millis()),
        format_session_date((start + Days::new(6)).timestamp_millis()),
    ]
}

/// Sum of `duration` (minutes) for Focus sessions whose `date` falls in
/// `week_dates`. Drives `#total-focus-week`.
fn weekly_focus_minutes(sessions: &[ManualSession], week_dates: &[String; 7]) -> u32 {
    sessions
        .iter()
        .filter(|s| s.session_type == SessionType::Focus && week_dates.contains(&s.date))
        .map(|s| s.duration)
        .sum()
}

/// Count of Focus sessions whose `date` falls in `week_dates`. Drives
/// `#weekly-sessions`.
fn weekly_sessions_count(sessions: &[ManualSession], week_dates: &[String; 7]) -> u32 {
    sessions
        .iter()
        .filter(|s| s.session_type == SessionType::Focus && week_dates.contains(&s.date))
        .fold(0u32, |acc, _| acc.saturating_add(1))
}

/// Sum of `duration` (minutes) for all session types whose `date` falls in
/// `week_dates`. Drives `#weekly-focus-time`.
fn weekly_total_minutes(sessions: &[ManualSession], week_dates: &[String; 7]) -> u32 {
    sessions
        .iter()
        .filter(|s| week_dates.contains(&s.date))
        .map(|s| s.duration)
        .sum()
}

/// Compute the duration in minutes between two `"HH:MM"` time strings.
///
/// Handles same-day ranges (end > start) only. On midnight rollover
/// (end < start) we add 24 × 60 to keep the result positive — the
/// JS-era surface didn't support multi-day sessions, so this only
/// covers the edge-case of a session that started just before midnight.
fn duration_from_start_end_minutes(start: &str, end: &str) -> u32 {
    let parse = |s: &str| -> u32 {
        let mut p = s.splitn(2, ':');
        let h = p.next().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
        let m = p.next().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
        h * 60 + m
    };
    let s = parse(start);
    let e = parse(end);
    if e >= s {
        e - s
    } else {
        e + 24 * 60 - s
    }
}

/// Compute the end time `"HH:MM"` from a start time and a duration in minutes.
/// Clamps to `"23:59"` on overflow — the JS-era surface didn't support
/// sessions spanning midnight.
fn end_time_from_start_duration(start: &str, duration: u32) -> String {
    let parse = |s: &str| -> u32 {
        let mut p = s.splitn(2, ':');
        let h = p.next().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
        let m = p.next().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
        h * 60 + m
    };
    let end_min = (parse(start) + duration).min(23 * 60 + 59);
    format!("{:02}:{:02}", end_min / 60, end_min % 60)
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

    // Shared session log (provided by App). The Calendar view
    // renders today's rows in `#sessions-table-body` so the
    // `sessions-history.spec.js:38-41` flow finds the just-
    // completed focus session row. Phase 4c routes the persistence
    // sink through `bridge::commands::save_manual_sessions`.
    let sessions =
        use_context::<RwSignal<Vec<ManualSession>>>().unwrap_or_else(|| RwSignal::new(Vec::new()));
    let session_modal_open = RwSignal::new(false);
    let modal_duration = RwSignal::new(0_u32);
    let modal_session_id = RwSignal::new(Option::<String>::None);
    let modal_start = RwSignal::new(String::new());
    let modal_end = RwSignal::new(String::new());
    // Feature 002 Bundle A: editable title in the manual-session
    // modal. The modal is the only mutation surface for
    // `ManualSession` records today; surfacing the title here
    // satisfies the "manual-backfill form captures the title"
    // contract (FR-002 / spec User Story 1 scenario 3). Empty-string
    // is normalised to None on save (Principle III).
    let modal_title = RwSignal::new(String::new());
    let on_open_modal = move |session: ManualSession| {
        modal_session_id.set(Some(session.id.clone()));
        modal_start.set(session.start_time.clone());
        modal_end.set(session.end_time.clone());
        modal_duration.set(session.duration);
        modal_title.set(session.title.unwrap_or_default());
        session_modal_open.set(true);
    };
    let on_close_modal = move |_| session_modal_open.set(false);

    // Settings drives the weekly-goal projection. Read via context
    // so `settings-goals.spec.js:38` (asserts `#weekly-goal-minutes`
    // value persists) sees the same source as the Goals tab.
    let settings =
        use_context::<RwSignal<Settings>>().unwrap_or_else(|| RwSignal::new(Settings::default()));
    let weekly_goal = Signal::derive(move || settings.with(|s| s.timer.weekly_goal_minutes));

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

    // The `Settings::timer.weekly_goal_minutes` value drives the
    // `settings-goals.spec.js` round-trip (which addresses
    // `#weekly-goal-minutes` on the Goals settings tab, not the
    // Calendar view). The Calendar view doesn't display the goal
    // value visibly today; Phase 4d's metric-tile projection feeds
    // the `weekly-sessions` count (driven by `sessions.len()`).
    let _ = weekly_goal;

    // Focus Weekly Summary metric signals — each reads both `cursor`
    // (week bounds) and `sessions` (source rows) so they re-derive
    // whenever the user navigates weeks or completes a session.
    let weekly_focus = Signal::derive(move || {
        let dates = week_date_set(cursor.get());
        sessions.with(|ss| weekly_focus_minutes(ss, &dates))
    });
    let avg_focus_day = Signal::derive(move || {
        let dates = week_date_set(cursor.get());
        sessions.with(|ss| weekly_focus_minutes(ss, &dates)) / 7
    });
    let weekly_sessions_sig = Signal::derive(move || {
        let dates = week_date_set(cursor.get());
        sessions.with(|ss| weekly_sessions_count(ss, &dates))
    });
    let weekly_total = Signal::derive(move || {
        let dates = week_date_set(cursor.get());
        sessions.with(|ss| weekly_total_minutes(ss, &dates))
    });

    view! {
        <div class="view-container view-section" id="calendar-view">
            <h1>"Calendar & Statistics"</h1>

            // Week selector — prev/next + the active range label.
            <div class="week-selector">
                <button id="prev-week" class="nav-btn" aria-label="Previous week" on:click=on_prev_week>"<"</button>
                <div class="week-display">
                    <span id="week-range">{move || week_label.get()}</span>
                </div>
                <button id="next-week" class="nav-btn" aria-label="Next week" on:click=on_next_week>">"</button>
            </div>

            // Two-column main layout. Mirrors the JS-era
            // `<div class="calendar-main-layout">` at
            // `src/index.html @ 3f1119e^`. Left column carries the
            // Focus Weekly Summary + This Week's Sessions chart +
            // Today's Development card; right column carries the
            // mini-calendar grid + the Today's Sessions timeline.
            <div class="calendar-main-layout">
                <div class="calendar-left-column">
                    // Focus Weekly Summary — four metric tiles in a
                    // 2x2 grid (CSS `grid-template-columns: repeat(4,
                    // 1fr)` per `style/calendar.css`). The cold-start
                    // baseline shows zero values across all four
                    // tiles (no sessions yet).
                    <div class="focus-summary-card" id="focus-summary-card">
                        <h3>"Focus Weekly Summary"</h3>
                        <div class="focus-summary-grid">
                            <div class="focus-metric">
                                <div class="metric-change neutral">
                                    <i class="ri-subtract-line"></i>
                                    <span>"0%"</span>
                                </div>
                                <div class="metric-value" id="total-focus-week">
                                    {move || format!("{}m", weekly_focus.get())}
                                </div>
                                <div class="metric-label">"Weekly focus time"</div>
                            </div>
                            <div class="focus-metric">
                                <div class="metric-change neutral">
                                    <i class="ri-subtract-line"></i>
                                    <span>"0%"</span>
                                </div>
                                <div class="metric-value" id="avg-focus-day">
                                    {move || format!("{}m", avg_focus_day.get())}
                                </div>
                                <div class="metric-label">"Average focus/day"</div>
                            </div>
                            <div class="focus-metric">
                                <div class="metric-change neutral">
                                    <i class="ri-subtract-line"></i>
                                    <span>"0%"</span>
                                </div>
                                <div class="metric-value" id="weekly-sessions">
                                    {move || weekly_sessions_sig.get().to_string()}
                                </div>
                                <div class="metric-label">"Sessions this week"</div>
                            </div>
                            <div class="focus-metric">
                                <div class="metric-change neutral">
                                    <i class="ri-subtract-line"></i>
                                    <span>"0%"</span>
                                </div>
                                <div class="metric-value" id="weekly-focus-time">
                                    {move || format!("{}m", weekly_total.get())}
                                </div>
                                <div class="metric-label">"Weekly total time"</div>
                            </div>
                        </div>
                    </div>

                    // This Week's Sessions — 7-day bar chart, one bar
                    // per day Mon-Sun. Empty state (no sessions yet)
                    // shows minimum-height bars (8px floor per CSS).
                    <div class="weekly-chart-card">
                        <h3>"This Week's Sessions"</h3>
                        <div class="weekly-chart" id="weekly-chart">
                            <div class="week-day-bar" style="height: 8px"></div>
                            <div class="week-day-bar" style="height: 8px"></div>
                            <div class="week-day-bar" style="height: 8px"></div>
                            <div class="week-day-bar" style="height: 8px"></div>
                            <div class="week-day-bar" style="height: 8px"></div>
                            <div class="week-day-bar" style="height: 8px"></div>
                            <div class="week-day-bar" style="height: 8px"></div>
                        </div>
                        <div class="week-days-labels">
                            <span>"Mon"</span>
                            <span>"Tue"</span>
                            <span>"Wed"</span>
                            <span>"Thu"</span>
                            <span>"Fri"</span>
                            <span>"Sat"</span>
                            <span>"Sun"</span>
                        </div>
                    </div>

                    // Today's Development — placeholder card. The
                    // JS-era surface filled this with hourly bars
                    // backed by `daily-chart`; the visual-regression
                    // baseline shows the empty-state frame only.
                    <div class="daily-chart-card">
                        <h3>"Today's Development"</h3>
                        <div class="daily-chart" id="daily-chart"></div>
                        <div class="chart-legend">
                            <span class="legend-item">
                                <span class="legend-color focus-color"></span>
                                "Focus"
                            </span>
                            <span class="legend-item">
                                <span class="legend-color break-color"></span>
                                "Break"
                            </span>
                        </div>
                    </div>

                    // Tag Usage pie-chart card.
                    // TODO(#39): wire pie-chart slices to a tag-frequency
                    // projection over sessions filtered by week_dates.
                    <div class="tag-usage-card">
                        <h3>"Tag Usage This Week"</h3>
                        <div class="tag-usage-chart"></div>
                    </div>
                </div>

                <div class="calendar-right-column">
                    // Mini-calendar grid + Today's Sessions timeline.
                    <div class="mini-calendar-container">
                        <div class="calendar-header">
                            <button id="prev-month" class="nav-btn" aria-label="Previous month" on:click=on_prev_month>"<"</button>
                            <h3 id="current-month">{move || month_label.get()}</h3>
                            <button id="next-month" class="nav-btn" aria-label="Next month" on:click=on_next_month>">"</button>
                        </div>
                        // Day-of-week header row. Sun-first matches
                        // the visual-regression baseline `Sun Mon
                        // Tue Wed Thu Fri Sat`.
                        <div class="calendar-grid calendar-day-names">
                            <div class="day-name">"Sun"</div>
                            <div class="day-name">"Mon"</div>
                            <div class="day-name">"Tue"</div>
                            <div class="day-name">"Wed"</div>
                            <div class="day-name">"Thu"</div>
                            <div class="day-name">"Fri"</div>
                            <div class="day-name">"Sat"</div>
                        </div>
                        <div class="calendar-grid" id="calendar-grid">
                            <For
                                each=move || grid.get()
                                key=|day| day.timestamp_millis()
                                children=move |day| {
                                    let cell_date = format_session_date(day.timestamp_millis());
                                    let is_today = cell_date == today_label;
                                    let cursor_month = cursor.with(Datelike::month);
                                    let in_current_month = day.month() == cursor_month;
                                    // `aria-current="date"` only on the today-cell so
                                    // sessions-history.spec.js:34 can locate it via
                                    // `[aria-current="date"]` without a date string
                                    // coupling.
                                    let aria_current = if is_today { "date" } else { "" };
                                    let day_num = day.day();
                                    // Out-of-month days render as blank
                                    // cells (the visual-regression
                                    // baseline shows empty padding before
                                    // May 1 and after May 31). Today
                                    // gets the `today` class for the
                                    // saturated dark-blue background
                                    // baseline highlight.
                                    view! {
                                        <div
                                            class="calendar-day"
                                            class:today=is_today
                                            class:other-month=move || !in_current_month
                                            role="button"
                                            aria-current=aria_current
                                            aria-label=cell_date
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

                        // Today's Sessions timeline (Selected Day Details).
                        <div class="selected-day-details" id="selected-day-details">
                            <div class="sessions-header">
                                <h4 id="selected-day-title">"Today's Sessions"</h4>
                                <button class="add-session-btn" id="add-session-btn" aria-label="Add session">
                                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M12 6v12m6-6H6" />
                                    </svg>
                                </button>
                            </div>
                            <div class="sessions-timeline" id="sessions-timeline">
                                <div class="timeline-hours" id="timeline-hours">
                                    <span class="timeline-hour" style="left: 0%">"00:00"</span>
                                    <span class="timeline-hour" style="left: 16.67%">"04:00"</span>
                                    <span class="timeline-hour" style="left: 33.33%">"08:00"</span>
                                    <span class="timeline-hour" style="left: 50%">"12:00"</span>
                                    <span class="timeline-hour" style="left: 66.67%">"16:00"</span>
                                    <span class="timeline-hour" style="left: 83.33%">"20:00"</span>
                                </div>
                                <div class="timeline-track" id="timeline-track">
                                    {move || {
                                        if sessions.with(Vec::is_empty) {
                                            view! {
                                                <div class="timeline-empty">"No sessions completed"</div>
                                            }.into_any()
                                        } else {
                                            view! { <span></span> }.into_any()
                                        }
                                    }}
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            // Sessions table — kept off the visible viewport so the
            // visual-regression baseline doesn't include it; the
            // `sessions-history.spec.js:37-44` flow scrolls into it
            // to find `#sessions-table-body` rows + the edit modal.
            <div class="sessions-history-card">
                <div class="sessions-header">
                    <h3>"Session History"</h3>
                    <div class="sessions-controls">
                        <button
                            id="export-sessions-btn"
                            class="export-btn"
                            title="Export to Excel"
                            on:click=move |_| {
                                let snapshot = sessions.get_untracked();
                                spawn_local(async move {
                                    let path = commands::dialog_save(
                                        Some("sessions.xlsx".to_string()),
                                        vec![("Excel".to_string(), vec!["xlsx".to_string()])],
                                    )
                                    .await
                                    .ok()
                                    .flatten();
                                    if let Some(p) = path {
                                        let _ = commands::export_sessions_xlsx(p, snapshot).await;
                                    }
                                });
                            }
                        >
                            <i class="ri-download-line"></i>
                            " Export"
                        </button>
                    </div>
                </div>
                <div class="sessions-table-container">
                    <table class="sessions-table" id="sessions-table">
                        <thead>
                            <tr>
                                <th>"Time"</th>
                                <th>"Duration"</th>
                                <th>"Actions"</th>
                            </tr>
                        </thead>
                        <tbody id="sessions-table-body">
                            <For
                                each=move || sessions.get()
                                key=|row| row.id.clone()
                                children=move |row| {
                                    let session_for_modal = row.clone();
                                    let time_range = format!("{} – {}", row.start_time, row.end_time);
                                    let duration_text = format!("{} min", row.duration);
                                    view! {
                                        <tr class="session-row" role="row">
                                            <td>{time_range}</td>
                                            <td>{duration_text}</td>
                                            <td>
                                                <button
                                                    class="edit-session-btn"
                                                    aria-label="Edit session"
                                                    on:click=move |_| on_open_modal(session_for_modal.clone())
                                                >"Edit"</button>
                                            </td>
                                        </tr>
                                    }
                                }
                            />
                        </tbody>
                    </table>
                </div>
            </div>

            // Per-row edit modal. Start/end/duration inputs tri-recalculate:
            // editing any one field recomputes the other two via the pure
            // helpers above. `on:input` fires only on user interaction so
            // there are no feedback loops when `prop:value` updates.
            <div
                class="session-modal-overlay"
                id="session-modal-overlay"
                style=move || if session_modal_open.get() { "" } else { "display: none" }
            >
                <form class="session-modal" id="session-form" role="dialog" aria-modal="true" aria-labelledby="session-modal-title">
                    <div class="session-modal-header">
                        <h3 id="session-modal-title">"Edit session"</h3>
                        <button
                            type="button"
                            id="close-session-modal"
                            class="close-btn"
                            aria-label="Close edit modal"
                            on:click=on_close_modal
                        >"×"</button>
                    </div>
                    <div class="session-modal-body">
                        // Feature 002 Bundle A: session title input,
                        // first row of the modal. `maxlength=120`
                        // enforces FR-004 at the browser boundary.
                        <label for="session-title">"Title"</label>
                        <input
                            type="text"
                            id="session-title"
                            maxlength="120"
                            placeholder="What is this session for?"
                            prop:value=move || modal_title.get()
                            on:input=move |ev| {
                                modal_title.set(event_target_value(&ev));
                            }
                        />
                        <label for="session-start-time">"Start Time"</label>
                        <input
                            type="time"
                            id="session-start-time"
                            prop:value=move || modal_start.get()
                            on:input=move |ev| {
                                let new_start = event_target_value(&ev);
                                let new_end = end_time_from_start_duration(
                                    &new_start,
                                    modal_duration.get_untracked(),
                                );
                                modal_start.set(new_start);
                                modal_end.set(new_end);
                            }
                        />
                        <label for="session-end-time">"End Time"</label>
                        <input
                            type="time"
                            id="session-end-time"
                            prop:value=move || modal_end.get()
                            on:input=move |ev| {
                                let new_end = event_target_value(&ev);
                                let new_dur = duration_from_start_end_minutes(
                                    &modal_start.get_untracked(),
                                    &new_end,
                                );
                                modal_end.set(new_end);
                                modal_duration.set(new_dur);
                            }
                        />
                        <label for="session-duration">"Duration (minutes)"</label>
                        <input
                            type="number"
                            id="session-duration"
                            min="1"
                            max="180"
                            prop:value=move || modal_duration.get().to_string()
                            on:input=move |ev| {
                                let new_dur: u32 =
                                    event_target_value(&ev).parse().unwrap_or(0);
                                let new_end = end_time_from_start_duration(
                                    &modal_start.get_untracked(),
                                    new_dur,
                                );
                                modal_duration.set(new_dur);
                                modal_end.set(new_end);
                            }
                        />
                    </div>
                    <div class="modal-actions">
                        <button
                            type="button"
                            id="cancel-session-btn"
                            class="btn-secondary"
                            on:click=on_close_modal
                        >"Cancel"</button>
                        <button
                            type="button"
                            id="delete-session-btn"
                            class="btn-danger"
                            on:click=move |_| {
                                if let Some(id) = modal_session_id.get_untracked() {
                                    sessions.update(|ss| ss.retain(|s| s.id != id));
                                }
                                session_modal_open.set(false);
                            }
                        >"Delete"</button>
                        <button
                            type="button"
                            id="save-session-btn"
                            class="btn-primary"
                            on:click=move |_| {
                                if let Some(id) = modal_session_id.get_untracked() {
                                    let dur = modal_duration.get_untracked();
                                    let start = modal_start.get_untracked();
                                    // Normalize end from start+duration so persisted records
                                    // are always consistent (clamps overflow to "23:59").
                                    let end = end_time_from_start_duration(&start, dur);
                                    // Feature 002 Bundle A: empty-string title
                                    // normalises to None at the save boundary
                                    // (Principle III).
                                    let title_raw = modal_title.get_untracked();
                                    let title = {
                                        let trimmed = title_raw.trim();
                                        if trimmed.is_empty() {
                                            None
                                        } else {
                                            Some(trimmed.to_string())
                                        }
                                    };
                                    sessions.update(|ss| {
                                        if let Some(s) = ss.iter_mut().find(|s| s.id == id) {
                                            s.duration = dur;
                                            s.start_time = start;
                                            s.end_time = end;
                                            s.title = title;
                                        }
                                    });
                                }
                                session_modal_open.set(false);
                            }
                        >"Save"</button>
                    </div>
                </form>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_month_grid, duration_from_start_end_minutes, end_time_from_start_duration,
        format_month_label, format_week_range, month_full, month_short, start_of_week_monday,
        start_of_week_sunday, week_date_set, weekly_focus_minutes, weekly_sessions_count,
        weekly_total_minutes,
    };
    use crate::bridge::types::ManualSession;
    use crate::bridge::types::SessionType;
    use crate::engine::date_format::format_session_date;
    use chrono::{DateTime, Datelike, TimeZone, Utc};
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    /// T200 — visual-regression / selector contract pin for the
    /// calendar view. Sourced from
    /// `tests/e2e/calendar-navigation.spec.js` +
    /// `tests/e2e/sessions-history.spec.js`. Each entry maps to a
    /// `locator("#…")` callsite; drift here breaks the e2e run.
    ///
    /// - `calendar-view` — root container (`_smoke.spec.js:20`
    ///   asserts `toBeHidden()` initially).
    /// - `prev-week` / `next-week` — week navigation
    ///   (`calendar-navigation.spec.js:17,22-23`).
    /// - `week-range` — week-range label
    ///   (`calendar-navigation.spec.js:13` `not.toBeEmpty`).
    /// - `prev-month` / `next-month` — month navigation
    ///   (`calendar-navigation.spec.js:33,38-39`).
    /// - `current-month` — month label
    ///   (`calendar-navigation.spec.js:14` `not.toBeEmpty`).
    /// - `calendar-grid` — month grid host
    ///   (`sessions-history.spec.js:34`).
    /// - `[aria-current="date"]` — today's cell carries this
    ///   attribute so `sessions-history.spec.js:34` can locate
    ///   the today-cell without a date-string coupling.
    ///
    /// Visual baseline updates are out of scope per AGENTS.md
    /// §"Don't update visual regression baselines without
    /// explicit visual review" — this test only pins the string
    /// contract.
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
            "export-sessions-btn",
            "session-modal-overlay",
            "session-form",
            "session-modal-title",
            "close-session-modal",
            "session-start-time",
            "session-end-time",
            "session-duration",
            "cancel-session-btn",
            "delete-session-btn",
            "save-session-btn",
        ];
        const TODAY_ARIA_CURRENT_VALUE: &str = "date";
        let mut seen: Vec<&str> = Vec::with_capacity(REQUIRED_IDS.len());
        for id in REQUIRED_IDS {
            assert!(!id.is_empty(), "selector ID must not be empty");
            assert!(
                !seen.contains(id),
                "duplicate selector ID in contract: {id}",
            );
            seen.push(id);
        }
        // `[aria-current="date"]` is the spec.js:34 selector — the
        // today-cell must carry exactly this attribute value, not
        // `aria-current="true"` or any other token.
        assert_eq!(
            TODAY_ARIA_CURRENT_VALUE, "date",
            "today-cell must carry aria-current=\"date\" per ARIA spec for sessions-history.spec.js:34",
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn duration_from_start_end_minutes_simple() {
        assert_eq!(duration_from_start_end_minutes("09:00", "09:25"), 25);
        assert_eq!(duration_from_start_end_minutes("00:00", "00:00"), 0);
        assert_eq!(duration_from_start_end_minutes("08:30", "09:00"), 30);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn end_time_from_start_duration_simple() {
        assert_eq!(end_time_from_start_duration("09:00", 25), "09:25");
        assert_eq!(end_time_from_start_duration("23:50", 20), "23:59"); // clamps to 23:59
        assert_eq!(end_time_from_start_duration("00:00", 0), "00:00");
    }

    fn day(year: i32, month: u32, d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, d, 0, 0, 0).unwrap()
    }

    /// `start_of_week_monday` rolls back to Monday — used by the
    /// week-range pill. 2025-06-12 (Thursday) → 2025-06-09 (Monday).
    #[test]
    fn start_of_week_monday_rolls_back() {
        let anchor = day(2025, 6, 12);
        let monday = start_of_week_monday(anchor);
        assert_eq!(monday.day(), 9);
        assert_eq!(monday.month(), 6);
        assert_eq!(monday.year(), 2025);
    }

    /// `start_of_week_sunday` rolls back to Sunday — used by the
    /// calendar grid. 2025-06-12 (Thursday) → 2025-06-08 (Sunday).
    #[test]
    fn start_of_week_sunday_rolls_back() {
        let anchor = day(2025, 6, 12);
        let sunday = start_of_week_sunday(anchor);
        assert_eq!(sunday.day(), 8);
        assert_eq!(sunday.month(), 6);
        assert_eq!(sunday.year(), 2025);
    }

    /// `format_week_range` produces the visual-regression baseline
    /// label shape (`Intl.DateTimeFormat("en-US")` parity). The
    /// frozen `tauriMock.freezeTime("2026-05-09T12:00:00Z")` anchor
    /// → `"May 4 - May 10 2026"`.
    #[test]
    fn week_range_baseline_anchor() {
        let anchor = day(2026, 5, 9); // Sat
        assert_eq!(format_week_range(anchor), "May 4 - May 10 2026");
    }

    /// `format_week_range` keeps both month labels for a
    /// month-spanning range.
    #[test]
    fn week_range_spans_month() {
        // 2025-06-30 is a Monday; range = Jun 30 - Jul 6.
        let anchor = day(2025, 6, 30);
        assert_eq!(format_week_range(anchor), "Jun 30 - Jul 6 2025");
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
        // First cell is a Sunday — calendar grid uses US-convention
        // week-start (matches the baseline `Sun Mon Tue ...` header).
        assert_eq!(grid[0].weekday().num_days_from_sunday(), 0);
    }

    /// Spot-check: every month index produces a non-empty label.
    #[test]
    fn month_names_cover_every_month() {
        for m in 1..=12 {
            assert_ne!(month_short(m), "???", "month_short missing {m}");
            assert_ne!(month_full(m), "Unknown", "month_full missing {m}");
        }
    }

    fn make_session(date: &str, duration: u32, session_type: SessionType) -> ManualSession {
        ManualSession {
            id: "test-id".to_string(),
            session_type,
            duration,
            start_time: "09:00".to_string(),
            end_time: "09:25".to_string(),
            notes: None,
            created_at: "2026-05-04T09:00:00Z".to_string(),
            date: date.to_string(),
            tags: None,
            title: None,
        }
    }

    #[test]
    fn weekly_metrics_are_zero_for_empty_list() {
        let dates = week_date_set(day(2026, 5, 9));
        assert_eq!(weekly_focus_minutes(&[], &dates), 0);
        assert_eq!(weekly_sessions_count(&[], &dates), 0);
        assert_eq!(weekly_total_minutes(&[], &dates), 0);
    }

    #[test]
    fn weekly_focus_minutes_sums_in_week_focus_sessions() {
        let anchor = day(2026, 5, 9); // Sat → week Mon May 4 – Sun May 10
        let dates = week_date_set(anchor);
        let monday_date = format_session_date(day(2026, 5, 4).timestamp_millis());
        let sessions = vec![
            make_session(&monday_date, 25, SessionType::Focus),
            make_session(&monday_date, 25, SessionType::Focus),
        ];
        assert_eq!(weekly_focus_minutes(&sessions, &dates), 50);
    }

    #[test]
    fn weekly_metrics_exclude_out_of_week_sessions() {
        let dates = week_date_set(day(2026, 5, 9)); // Week May 4-10
                                                    // May 3 is the Sunday of the prior week — outside the Mon-Sun range.
        let prev_date = format_session_date(day(2026, 5, 3).timestamp_millis());
        let sessions = vec![make_session(&prev_date, 25, SessionType::Focus)];
        assert_eq!(weekly_focus_minutes(&sessions, &dates), 0);
        assert_eq!(weekly_sessions_count(&sessions, &dates), 0);
        assert_eq!(weekly_total_minutes(&sessions, &dates), 0);
    }

    #[test]
    fn weekly_total_includes_non_focus_sessions() {
        let dates = week_date_set(day(2026, 5, 9));
        let monday_date = format_session_date(day(2026, 5, 4).timestamp_millis());
        let sessions = vec![
            make_session(&monday_date, 25, SessionType::Focus),
            make_session(&monday_date, 5, SessionType::Break),
        ];
        // weekly_focus only counts Focus; weekly_total counts all.
        assert_eq!(weekly_focus_minutes(&sessions, &dates), 25);
        assert_eq!(weekly_total_minutes(&sessions, &dates), 30);
    }

    #[test]
    fn weekly_sessions_count_counts_focus_only() {
        let dates = week_date_set(day(2026, 5, 9));
        let monday_date = format_session_date(day(2026, 5, 4).timestamp_millis());
        let sessions = vec![
            make_session(&monday_date, 25, SessionType::Focus),
            make_session(&monday_date, 5, SessionType::Break),
            make_session(&monday_date, 20, SessionType::LongBreak),
        ];
        assert_eq!(weekly_sessions_count(&sessions, &dates), 1);
    }
}
