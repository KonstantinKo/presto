// Sessions-timeline panel for the Daily view — mirrors the
// pre-rework `#sessions-timeline` block in `components::calendar`
// (lines 604–635). Shows the selected day's sessions as positioned
// blocks against a 00:00–24:00 horizontal axis; the empty-state
// label "No sessions completed" matches the existing baseline.
//
// Selector contract preserved per A14 / FR-019:
// - `#sessions-timeline`
// - `#timeline-hours`
// - `#timeline-track`
// - `#selected-day-title`

#![allow(
    clippy::must_use_candidate,
    clippy::too_many_lines,
    reason = "Leptos `#[component]` returning `impl IntoView`; body is a single `view!` macro expansion (hour markers + empty-state label) plus a derived signal. Matches `calendar.rs:32` precedent."
)]

use chrono::{DateTime, Datelike, Utc};
use leptos::prelude::*;

use crate::bridge::types::{ManualSession, SessionType};
use crate::engine::date_format::format_session_date;

/// Parse an `"HH:MM"` time string to minutes-since-midnight. Returns
/// 0 on malformed input — requires exactly one ':', both sides fully
/// numeric, h < 24, m < 60.
fn parse_hhmm_to_minutes(s: &str) -> u32 {
    let Some((h_str, m_str)) = s.split_once(':') else {
        return 0;
    };
    if m_str.contains(':') {
        return 0;
    }
    let Ok(h) = h_str.parse::<u32>() else {
        return 0;
    };
    let Ok(m) = m_str.parse::<u32>() else {
        return 0;
    };
    if h >= 24 || m >= 60 {
        return 0;
    }
    h * 60 + m
}

const WEEKDAY_NAMES: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

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

fn weekday_name(day: DateTime<Utc>) -> &'static str {
    let idx = day.weekday().num_days_from_monday() as usize;
    WEEKDAY_NAMES.get(idx).copied().unwrap_or("Unknown")
}

fn month_full(month: u32) -> &'static str {
    let idx = month.saturating_sub(1) as usize;
    MONTH_FULL_NAMES.get(idx).copied().unwrap_or("Unknown")
}

/// Render the title for the selected-day timeline header. When the
/// selected day is today, show "Today's Sessions" to match the
/// pre-rework baseline; otherwise show
/// `"<Weekday>, <Month> <Day> <Year>"` (e.g.
/// `"Tuesday, May 5 2026"`).
fn selected_day_title(selected: DateTime<Utc>, today: DateTime<Utc>) -> String {
    if format_session_date(selected.timestamp_millis())
        == format_session_date(today.timestamp_millis())
    {
        "Today's Sessions".to_string()
    } else {
        format!(
            "{weekday}, {month} {day} {year}",
            weekday = weekday_name(selected),
            month = month_full(selected.month()),
            day = selected.day(),
            year = selected.year(),
        )
    }
}

/// Sessions-timeline component. Reads the global `sessions` context
/// signal and projects to the selected day's session set.
#[component]
pub fn SessionsTimeline(
    selected_day: RwSignal<DateTime<Utc>>,
    today: DateTime<Utc>,
) -> impl IntoView {
    let sessions =
        use_context::<RwSignal<Vec<ManualSession>>>().unwrap_or_else(|| RwSignal::new(Vec::new()));

    // Filter the session log to entries whose `date` matches the
    // selected day (per `format_session_date`).
    let selected_sessions = Signal::derive(move || {
        let selected_label =
            format_session_date(selected_day.with(chrono::DateTime::timestamp_millis));
        sessions.with(|all| {
            all.iter()
                .filter(|s| s.date == selected_label)
                .cloned()
                .collect::<Vec<_>>()
        })
    });

    let title = Signal::derive(move || selected_day_title(selected_day.get(), today));

    view! {
        <div class="selected-day-details" id="selected-day-details">
            <div class="sessions-header">
                <h4 id="selected-day-title">{move || title.get()}</h4>
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
                        if selected_sessions.with(Vec::is_empty) {
                            view! {
                                <div class="timeline-empty">"No sessions completed"</div>
                            }.into_any()
                        } else {
                            // FR-014: each session as a positioned block on
                            // a 00:00–24:00 horizontal timeline. `left` and
                            // `width` are percentages of the 1440-minute day.
                            selected_sessions.with(|ss| {
                                ss.iter().map(|session| {
                                    let start_minutes = parse_hhmm_to_minutes(&session.start_time);
                                    let end_minutes = start_minutes + session.duration;
                                    let clamped_start = start_minutes.min(1440);
                                    let clamped_end = end_minutes.min(1440);
                                    let clamped_duration = clamped_end.saturating_sub(clamped_start);
                                    let left_pct = (f64::from(clamped_start) / 1440.0) * 100.0;
                                    let width_pct = (f64::from(clamped_duration) / 1440.0) * 100.0;
                                    let style = format!(
                                        "left: {left_pct:.2}%; width: {width_pct:.2}%"
                                    );
                                    let class = match session.session_type {
                                        SessionType::Focus | SessionType::Custom => {
                                            "session-block session-block-focus"
                                        }
                                        SessionType::Break | SessionType::LongBreak => {
                                            "session-block session-block-break"
                                        }
                                    };
                                    let title = session.id.clone();
                                    view! {
                                        <div class=class style=style title=title></div>
                                    }
                                }).collect_view()
                            }).into_any()
                        }
                    }}
                </div>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::parse_hhmm_to_minutes;

    #[test]
    fn parses_midnight() {
        assert_eq!(parse_hhmm_to_minutes("00:00"), 0);
    }

    #[test]
    fn parses_noon() {
        assert_eq!(parse_hhmm_to_minutes("12:00"), 720);
    }

    #[test]
    fn parses_last_minute() {
        assert_eq!(parse_hhmm_to_minutes("23:59"), 1439);
    }

    #[test]
    fn malformed_falls_back_to_zero() {
        assert_eq!(parse_hhmm_to_minutes("abc"), 0);
    }

    #[test]
    fn out_of_range_falls_back_to_zero() {
        assert_eq!(parse_hhmm_to_minutes("24:00"), 0);
        assert_eq!(parse_hhmm_to_minutes("12:60"), 0);
    }
}
