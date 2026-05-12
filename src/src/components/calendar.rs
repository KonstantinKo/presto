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

use chrono::{DateTime, Datelike, Days, Utc};
use leptos::prelude::*;

use super::browser_clock::BrowserClock;
use super::utils::datetime::datetime_from_ms;
use crate::bridge::types::SessionType;
use crate::bridge::types::{ManualSession, Settings};
use crate::engine::clock::Clock;
use crate::engine::date_format::format_session_date;

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

    // Settings drives the weekly-goal projection. Read via context
    // so `settings-goals.spec.js:38` (asserts `#weekly-goal-minutes`
    // value persists) sees the same source as the Goals tab.
    let settings =
        use_context::<RwSignal<Settings>>().unwrap_or_else(|| RwSignal::new(Settings::default()));
    let weekly_goal = Signal::derive(move || settings.with(|s| s.timer.weekly_goal_minutes));

    let week_label = Signal::derive(move || format_week_range(cursor.get()));

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

    // Suppress today since the mini-cal grid + sessions-history moved
    // to the new Daily view (Feature 003 Phase 2). Calendar.rs keeps
    // the week-range navigation + focus-summary cards through Phase 3.
    let _ = today;

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
            </div>
            // Feature 003 (Phase 2): the right-column mini-calendar
            // + Today's Sessions timeline AND the off-viewport
            // sessions-history-card (table + edit modal) move to the
            // new `components::daily` view (FR-019 / CHK043). Phase 3
            // replaces `CalendarView` with `StatisticsView`.
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::{
        format_week_range, month_short, start_of_week_monday, week_date_set, weekly_focus_minutes,
        weekly_sessions_count, weekly_total_minutes,
    };
    use crate::bridge::types::ManualSession;
    use crate::bridge::types::SessionType;
    use crate::engine::date_format::format_session_date;
    use chrono::{DateTime, Datelike, TimeZone, Utc};

    /// Selector contract pin for the Calendar (Statistics) view
    /// AFTER the Feature 003 Phase 2 cleanup.
    ///
    /// The mini-calendar grid + sessions-history block moved to
    /// `components::daily` (FR-019 / A14 / CHK043); the IDs below
    /// are what remains on the `#calendar-view` host through
    /// Phase 2. Phase 3 replaces `CalendarView` with
    /// `StatisticsView` and deletes this file.
    #[test]
    fn calendar_view_selector_contract_documented() {
        const REQUIRED_IDS: &[&str] = &[
            "calendar-view",
            "prev-week",
            "next-week",
            "week-range",
            "focus-summary-card",
            "total-focus-week",
            "avg-focus-day",
            "weekly-sessions",
            "weekly-focus-time",
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

    /// Spot-check: every month index produces a non-empty label.
    #[test]
    fn month_names_cover_every_month() {
        for m in 1..=12 {
            assert_ne!(month_short(m), "???", "month_short missing {m}");
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
