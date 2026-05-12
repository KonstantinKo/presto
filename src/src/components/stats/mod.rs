// Statistics view — Bundle A of feature 003. Replaces the pre-rework
// single-week `CalendarView` with a four-tab period selector backed by
// a single reusable `BarChart`.
//
// Selector contract (preserved from `calendar.rs` per FR-001 / FR-009
// / A13):
// - `#calendar-view` — root view container (`_smoke.spec.js:20`)
// - `#prev-week` / `#next-week` / `#week-range` — Weekly variant nav
// - `#focus-summary-card` / `#total-focus-week` / `#avg-focus-day` /
//   `#weekly-sessions` / `#weekly-focus-time` — focus-summary tiles
//
// New selectors (FR-007):
// - `#prev-day` / `#next-day` / `#day-range`
// - `#prev-month-period` / `#next-month-period` / `#month-range`
// - `#prev-year` / `#next-year` / `#year-range`
//
// The right-column mini-calendar + Today's Sessions + off-viewport
// sessions-history-card block from the pre-rework `calendar.rs` is
// REMOVED here (FR-019). Those surfaces now live in the Daily view
// (Phase 2 / Bundle B).

#![allow(
    clippy::must_use_candidate,
    clippy::too_many_lines,
    reason = "Leptos `#[component]` returning `impl IntoView`; body is a single `view!` macro expansion (period selector + navigator + focus-summary tiles + bar chart + tag-usage pie). Matches `calendar.rs:32` precedent."
)]

pub mod bar_chart;
pub mod period_nav;
pub mod period_selector;
pub mod tag_usage_pie;

use chrono::{DateTime, Datelike, Days, TimeZone, Utc};
use leptos::prelude::*;

use self::bar_chart::{BarChart, BarChartConfig};
use self::period_nav::PeriodNav;
use self::period_selector::{Period, PeriodSelector};
use self::tag_usage_pie::TagUsagePie;
use super::browser_clock::BrowserClock;
use super::utils::datetime::datetime_from_ms;
use crate::bridge::types::{ManualSession, SessionType, Settings, Tag};
use crate::engine::clock::Clock;
use crate::engine::date_format::format_session_date;

const MONTH_SHORT_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Compute the Monday of the week containing `anchor`.
fn start_of_week_monday(anchor: DateTime<Utc>) -> DateTime<Utc> {
    let weekday = anchor.weekday().num_days_from_monday();
    anchor - Days::new(u64::from(weekday))
}

/// Round `value` up to the nearest multiple of `step`. Used for the
/// readable-tick-label rounding policy on `BarChartProps::max_scale`
/// (Weekly/Monthly nearest-10; Yearly nearest-50 — see
/// `data-model.md §BarChartProps`).
const fn round_up_to_nearest(value: u32, step: u32) -> u32 {
    if step == 0 || value == 0 {
        return value;
    }
    let remainder = value % step;
    if remainder == 0 {
        value
    } else {
        value.saturating_add(step - remainder)
    }
}

/// Number of days in `month` of `year`. Walks down from 31 until
/// chrono validates the candidate. Always terminates in ≤ 4 iterations.
fn days_in_month(year: i32, month: u32) -> u32 {
    (28u32..=31u32)
        .rev()
        .find(|d| chrono::NaiveDate::from_ymd_opt(year, month, *d).is_some())
        .unwrap_or(28)
}

/// Set of `format_session_date` strings for the 7-day Mon-Sun span
/// containing `anchor`. Drives the focus-summary metric tiles.
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

/// Sum Focus-session minutes whose `date` is in `week_dates`.
fn weekly_focus_minutes(sessions: &[ManualSession], week_dates: &[String; 7]) -> u32 {
    sessions
        .iter()
        .filter(|s| s.session_type == SessionType::Focus && week_dates.contains(&s.date))
        .map(|s| s.duration)
        .sum()
}

/// Count Focus sessions whose `date` is in `week_dates`.
fn weekly_sessions_count(sessions: &[ManualSession], week_dates: &[String; 7]) -> u32 {
    sessions
        .iter()
        .filter(|s| s.session_type == SessionType::Focus && week_dates.contains(&s.date))
        .fold(0_u32, |acc, _| acc.saturating_add(1))
}

/// Sum all-session minutes whose `date` is in `week_dates`.
fn weekly_total_minutes(sessions: &[ManualSession], week_dates: &[String; 7]) -> u32 {
    sessions
        .iter()
        .filter(|s| week_dates.contains(&s.date))
        .map(|s| s.duration)
        .sum()
}

/// Filter sessions whose `date` matches a specific day.
fn sessions_for_day(sessions: &[ManualSession], day: DateTime<Utc>) -> Vec<&ManualSession> {
    let day_label = format_session_date(day.timestamp_millis());
    sessions.iter().filter(|s| s.date == day_label).collect()
}

/// Filter sessions whose `date` falls in the Mon-Sun span containing
/// `anchor`.
fn sessions_for_week(sessions: &[ManualSession], anchor: DateTime<Utc>) -> Vec<ManualSession> {
    let dates = week_date_set(anchor);
    sessions
        .iter()
        .filter(|s| dates.contains(&s.date))
        .cloned()
        .collect()
}

/// Filter sessions whose `date` parses to the same year+month as
/// `anchor`.
fn sessions_for_month(sessions: &[ManualSession], anchor: DateTime<Utc>) -> Vec<ManualSession> {
    let year = anchor.year();
    let month = anchor.month();
    let dim = days_in_month(year, month);
    let mut day_labels: Vec<String> = Vec::with_capacity(dim as usize);
    for d in 1..=dim {
        if let Some(dt) = Utc.with_ymd_and_hms(year, month, d, 0, 0, 0).single() {
            day_labels.push(format_session_date(dt.timestamp_millis()));
        }
    }
    sessions
        .iter()
        .filter(|s| day_labels.contains(&s.date))
        .cloned()
        .collect()
}

/// Filter sessions whose `date` parses to the same year as `anchor`.
fn sessions_for_year(sessions: &[ManualSession], anchor: DateTime<Utc>) -> Vec<ManualSession> {
    let year = anchor.year();
    let mut all_days: Vec<String> = Vec::with_capacity(366);
    for month in 1..=12_u32 {
        let dim = days_in_month(year, month);
        for d in 1..=dim {
            if let Some(dt) = Utc.with_ymd_and_hms(year, month, d, 0, 0, 0).single() {
                all_days.push(format_session_date(dt.timestamp_millis()));
            }
        }
    }
    sessions
        .iter()
        .filter(|s| all_days.contains(&s.date))
        .cloned()
        .collect()
}

/// Aggregate the 24 hourly Focus-minute totals for `anchor_day`. Per
/// hour buckets are derived from each session's `start_time` (HH:MM)
/// truncated to the hour. Sessions without a `start_time` are skipped.
#[must_use]
pub fn aggregate_hourly_focus(sessions: &[ManualSession], anchor_day: DateTime<Utc>) -> Vec<u32> {
    let mut buckets = vec![0_u32; 24];
    let day_label = format_session_date(anchor_day.timestamp_millis());
    for session in sessions {
        if session.date != day_label || session.session_type != SessionType::Focus {
            continue;
        }
        let hour = session
            .start_time
            .split(':')
            .next()
            .and_then(|h| h.parse::<u32>().ok())
            .unwrap_or(0);
        if let Some(bucket) = buckets.get_mut(hour as usize) {
            *bucket = bucket.saturating_add(session.duration);
        }
    }
    buckets
}

/// Aggregate the 7 weekday Focus-minute totals for the Mon-Sun span
/// containing `anchor`. Result is indexed Monday..Sunday.
#[must_use]
pub fn aggregate_weekly_focus(sessions: &[ManualSession], anchor: DateTime<Utc>) -> Vec<u32> {
    let dates = week_date_set(anchor);
    let mut buckets = vec![0_u32; 7];
    for session in sessions {
        if session.session_type != SessionType::Focus {
            continue;
        }
        if let Some(idx) = dates.iter().position(|d| *d == session.date) {
            if let Some(bucket) = buckets.get_mut(idx) {
                *bucket = bucket.saturating_add(session.duration);
            }
        }
    }
    buckets
}

/// Aggregate the per-day Focus-minute totals for the month containing
/// `anchor`. Result length is the days-in-month for that month.
#[must_use]
pub fn aggregate_monthly_focus(sessions: &[ManualSession], anchor: DateTime<Utc>) -> Vec<u32> {
    let year = anchor.year();
    let month = anchor.month();
    let dim = days_in_month(year, month);
    let mut buckets = vec![0_u32; dim as usize];
    for d in 1..=dim {
        if let Some(dt) = Utc.with_ymd_and_hms(year, month, d, 0, 0, 0).single() {
            let day_label = format_session_date(dt.timestamp_millis());
            let total: u32 = sessions
                .iter()
                .filter(|s| s.session_type == SessionType::Focus && s.date == day_label)
                .map(|s| s.duration)
                .sum();
            if let Some(bucket) = buckets.get_mut((d - 1) as usize) {
                *bucket = total;
            }
        }
    }
    buckets
}

/// Aggregate the 12 month Focus-minute totals for the year containing
/// `anchor`.
#[must_use]
pub fn aggregate_yearly_focus(sessions: &[ManualSession], anchor: DateTime<Utc>) -> Vec<u32> {
    let year = anchor.year();
    let mut buckets = vec![0_u32; 12];
    for month in 1..=12_u32 {
        let dim = days_in_month(year, month);
        let mut month_total: u32 = 0;
        for d in 1..=dim {
            if let Some(dt) = Utc.with_ymd_and_hms(year, month, d, 0, 0, 0).single() {
                let day_label = format_session_date(dt.timestamp_millis());
                let day_total: u32 = sessions
                    .iter()
                    .filter(|s| s.session_type == SessionType::Focus && s.date == day_label)
                    .map(|s| s.duration)
                    .sum();
                month_total = month_total.saturating_add(day_total);
            }
        }
        if let Some(bucket) = buckets.get_mut((month - 1) as usize) {
            *bucket = month_total;
        }
    }
    buckets
}

fn month_short(month: u32) -> &'static str {
    let idx = month.saturating_sub(1) as usize;
    MONTH_SHORT_NAMES.get(idx).copied().unwrap_or("???")
}

/// Anchor a cursor to the current period's "start" point per FR-008:
/// Daily → today; Weekly → this week's Monday; Monthly → first-of-month;
/// Yearly → first-of-year (Jan 1).
fn anchor_to_period(period: Period, now: DateTime<Utc>) -> DateTime<Utc> {
    match period {
        Period::Daily => now,
        Period::Weekly => start_of_week_monday(now),
        Period::Monthly => Utc
            .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
            .single()
            .unwrap_or(now),
        Period::Yearly => Utc
            .with_ymd_and_hms(now.year(), 1, 1, 0, 0, 0)
            .single()
            .unwrap_or(now),
    }
}

/// Build the per-period `BarChartProps` shape. The caller-applied
/// per-period floor + rounding policy lives here so the `BarChart`
/// component itself stays shape-agnostic.
fn build_bar_props(
    period: Period,
    cursor: DateTime<Utc>,
    sessions: &[ManualSession],
) -> BarChartConfig {
    match period {
        Period::Daily => {
            let labels: Vec<String> = (0..24).map(|h| format!("{h:02}:00")).collect();
            let values = aggregate_hourly_focus(sessions, cursor);
            BarChartConfig {
                max_scale: 60,
                x_axis_labels: labels,
                bar_values: values,
                min_bar_height_px: 4,
            }
        }
        Period::Weekly => {
            let labels: Vec<String> = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
                .iter()
                .map(|s| (*s).to_string())
                .collect();
            let values = aggregate_weekly_focus(sessions, cursor);
            let observed = values.iter().copied().max().unwrap_or(0);
            let rounded = round_up_to_nearest(observed, 10);
            let max_scale = rounded.max(20);
            BarChartConfig {
                max_scale,
                x_axis_labels: labels,
                bar_values: values,
                min_bar_height_px: 4,
            }
        }
        Period::Monthly => {
            let year = cursor.year();
            let month = cursor.month();
            let dim = days_in_month(year, month);
            let labels: Vec<String> = (1..=dim).map(|d| d.to_string()).collect();
            let values = aggregate_monthly_focus(sessions, cursor);
            let observed = values.iter().copied().max().unwrap_or(0);
            let rounded = round_up_to_nearest(observed, 10);
            let max_scale = rounded.max(50);
            BarChartConfig {
                max_scale,
                x_axis_labels: labels,
                bar_values: values,
                min_bar_height_px: 4,
            }
        }
        Period::Yearly => {
            let labels: Vec<String> = (1..=12_u32).map(|m| month_short(m).to_string()).collect();
            let values = aggregate_yearly_focus(sessions, cursor);
            let observed = values.iter().copied().max().unwrap_or(0);
            let rounded = round_up_to_nearest(observed, 50);
            let max_scale = rounded.max(100);
            BarChartConfig {
                max_scale,
                x_axis_labels: labels,
                bar_values: values,
                min_bar_height_px: 4,
            }
        }
    }
}

/// Statistics view. Holds the active `Period` + the per-period cursor,
/// projects period-scoped session slices for the bar chart and the
/// tag-usage pie, and renders the focus-summary metric tiles for the
/// Weekly variant only (selector contract per FR-009).
#[component]
pub fn StatisticsView() -> impl IntoView {
    let now = datetime_from_ms(BrowserClock.now_ms());

    // Cold-load default per FR-003 / SC-001.
    let period = RwSignal::new(Period::Weekly);
    let cursor = RwSignal::new(anchor_to_period(Period::Weekly, now));

    let sessions =
        use_context::<RwSignal<Vec<ManualSession>>>().unwrap_or_else(|| RwSignal::new(Vec::new()));
    let tags = use_context::<RwSignal<Vec<Tag>>>().unwrap_or_else(|| RwSignal::new(Vec::new()));

    // `settings` is read so that future feature work (weekly-goal
    // projection on the focus-summary card) can hook in without
    // re-introducing context plumbing. Today we suppress the binding
    // to keep clippy quiet without dropping the read pattern.
    let settings =
        use_context::<RwSignal<Settings>>().unwrap_or_else(|| RwSignal::new(Settings::default()));
    let _ = settings;

    let on_select_period = Callback::new(move |new_period: Period| {
        // FR-008 / SC-005: reset cursor on period swap.
        period.set(new_period);
        cursor.set(anchor_to_period(new_period, now));
    });

    let period_signal: Signal<Period> = period.into();

    // Period-scoped session slice. The bar chart and tag-usage pie
    // both consume the same projection.
    let period_sessions = Signal::derive(move || {
        let p = period.get();
        let c = cursor.get();
        sessions.with(|all| match p {
            Period::Daily => sessions_for_day(all, c).into_iter().cloned().collect(),
            Period::Weekly => sessions_for_week(all, c),
            Period::Monthly => sessions_for_month(all, c),
            Period::Yearly => sessions_for_year(all, c),
        })
    });

    let tags_signal: Signal<Vec<Tag>> = Signal::derive(move || tags.get());

    // Focus-summary metric signals — the e2e selector contract per
    // FR-009 / A13. These derive from `cursor` (week bounds) regardless
    // of the active period; the focus-summary card is rendered only on
    // the Weekly variant. The signals stay live so the period-swap
    // re-anchor keeps the card values fresh once Weekly is re-selected.
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
            <h1>"Statistics"</h1>

            <PeriodSelector current=period on_select=on_select_period />

            <PeriodNav period=period_signal cursor=cursor />

            // Focus Weekly Summary metric tiles — selector contract
            // preserved per FR-009. Rendered on every period; the
            // signals stay anchored on the period cursor so Daily
            // (e.g.) shows the metrics for the week containing the
            // active day.
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

            // Bar chart — single reusable definition (SC-002); the
            // props are recomputed on every cursor or period change.
            {move || {
                let props = build_bar_props(period.get(), cursor.get(), &sessions.get());
                view! { <BarChart max_scale=props.max_scale x_axis_labels=props.x_axis_labels bar_values=props.bar_values min_bar_height_px=props.min_bar_height_px /> }
            }}

            <TagUsagePie sessions=period_sessions tags=tags_signal />
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::Period;
    use super::{
        aggregate_hourly_focus, aggregate_monthly_focus, aggregate_weekly_focus,
        aggregate_yearly_focus, anchor_to_period, build_bar_props, days_in_month,
        round_up_to_nearest,
    };
    use crate::bridge::types::{ManualSession, SessionType};
    use chrono::{Datelike, TimeZone, Utc};

    fn session(date: &str, start: &str, duration: u32) -> ManualSession {
        ManualSession {
            id: format!("s-{date}-{start}"),
            session_type: SessionType::Focus,
            duration,
            start_time: start.to_string(),
            end_time: "00:00".to_string(),
            notes: None,
            created_at: format!("{date}T{start}:00Z"),
            date: date.to_string(),
            tags: None,
            title: None,
        }
    }

    #[test]
    fn round_up_zero_step_is_identity() {
        assert_eq!(round_up_to_nearest(91, 0), 91);
        assert_eq!(round_up_to_nearest(0, 10), 0);
    }

    #[test]
    fn round_up_nearest_10() {
        assert_eq!(round_up_to_nearest(87, 10), 90);
        assert_eq!(round_up_to_nearest(91, 10), 100);
        assert_eq!(round_up_to_nearest(90, 10), 90);
    }

    #[test]
    fn round_up_nearest_50() {
        assert_eq!(round_up_to_nearest(91, 50), 100);
        assert_eq!(round_up_to_nearest(151, 50), 200);
        assert_eq!(round_up_to_nearest(100, 50), 100);
    }

    #[test]
    fn days_in_month_handles_feb_leap_and_non_leap() {
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2025, 2), 28);
        assert_eq!(days_in_month(2026, 4), 30);
        assert_eq!(days_in_month(2026, 7), 31);
    }

    #[test]
    fn anchor_weekly_rolls_to_monday() {
        // 2026-05-09 is a Saturday; Monday is May 4.
        let sat = Utc.with_ymd_and_hms(2026, 5, 9, 12, 0, 0).unwrap();
        let anchor = anchor_to_period(Period::Weekly, sat);
        assert_eq!(anchor.day(), 4);
        assert_eq!(anchor.month(), 5);
    }

    #[test]
    fn anchor_monthly_rolls_to_first_of_month() {
        let mid = Utc.with_ymd_and_hms(2026, 5, 15, 12, 0, 0).unwrap();
        let anchor = anchor_to_period(Period::Monthly, mid);
        assert_eq!(anchor.day(), 1);
        assert_eq!(anchor.month(), 5);
    }

    #[test]
    fn anchor_yearly_rolls_to_jan_1() {
        let mid = Utc.with_ymd_and_hms(2026, 5, 15, 12, 0, 0).unwrap();
        let anchor = anchor_to_period(Period::Yearly, mid);
        assert_eq!(anchor.day(), 1);
        assert_eq!(anchor.month(), 1);
    }

    #[test]
    fn hourly_aggregation_buckets_by_start_hour() {
        let day = Utc.with_ymd_and_hms(2026, 5, 9, 0, 0, 0).unwrap();
        let date_label = crate::engine::date_format::format_session_date(day.timestamp_millis());
        let sessions = vec![
            session(&date_label, "09:30", 25),
            session(&date_label, "09:45", 25),
            session(&date_label, "14:00", 50),
        ];
        let buckets = aggregate_hourly_focus(&sessions, day);
        assert_eq!(buckets.len(), 24);
        assert_eq!(buckets[9], 50);
        assert_eq!(buckets[14], 50);
        assert_eq!(buckets[0], 0);
    }

    #[test]
    fn weekly_aggregation_indexes_mon_first() {
        // Mon May 4 2026.
        let mon = Utc.with_ymd_and_hms(2026, 5, 4, 12, 0, 0).unwrap();
        let mon_label = crate::engine::date_format::format_session_date(mon.timestamp_millis());
        let sessions = vec![session(&mon_label, "09:00", 25)];
        let buckets = aggregate_weekly_focus(&sessions, mon);
        assert_eq!(buckets.len(), 7);
        assert_eq!(buckets[0], 25);
    }

    #[test]
    fn monthly_aggregation_length_matches_dim() {
        let anchor = Utc.with_ymd_and_hms(2026, 5, 15, 0, 0, 0).unwrap();
        let buckets = aggregate_monthly_focus(&[], anchor);
        assert_eq!(buckets.len(), 31);
    }

    #[test]
    fn yearly_aggregation_has_12_buckets() {
        let anchor = Utc.with_ymd_and_hms(2026, 5, 15, 0, 0, 0).unwrap();
        let buckets = aggregate_yearly_focus(&[], anchor);
        assert_eq!(buckets.len(), 12);
    }

    #[test]
    fn build_bar_props_daily_uses_fixed_60_ceiling() {
        let anchor = Utc.with_ymd_and_hms(2026, 5, 9, 12, 0, 0).unwrap();
        let props = build_bar_props(Period::Daily, anchor, &[]);
        assert_eq!(props.max_scale, 60);
        assert_eq!(props.bar_values.len(), 24);
    }

    #[test]
    fn build_bar_props_weekly_floor_20_with_empty_data() {
        let anchor = Utc.with_ymd_and_hms(2026, 5, 9, 12, 0, 0).unwrap();
        let props = build_bar_props(Period::Weekly, anchor, &[]);
        assert_eq!(props.max_scale, 20);
        assert_eq!(props.bar_values.len(), 7);
    }

    #[test]
    fn build_bar_props_monthly_floor_50_with_empty_data() {
        let anchor = Utc.with_ymd_and_hms(2026, 5, 9, 12, 0, 0).unwrap();
        let props = build_bar_props(Period::Monthly, anchor, &[]);
        assert_eq!(props.max_scale, 50);
        assert!((28..=31).contains(&props.bar_values.len()));
    }

    #[test]
    fn build_bar_props_yearly_floor_100_with_empty_data() {
        let anchor = Utc.with_ymd_and_hms(2026, 5, 9, 12, 0, 0).unwrap();
        let props = build_bar_props(Period::Yearly, anchor, &[]);
        assert_eq!(props.max_scale, 100);
        assert_eq!(props.bar_values.len(), 12);
    }
}
