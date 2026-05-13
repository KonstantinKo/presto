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

// "Today's Sessions" matches pre-rework visual baseline.
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

#[component]
pub fn SessionsTimeline(
    selected_day: RwSignal<DateTime<Utc>>,
    today: DateTime<Utc>,
) -> impl IntoView {
    let sessions =
        use_context::<RwSignal<Vec<ManualSession>>>().unwrap_or_else(|| RwSignal::new(Vec::new()));

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
                            selected_sessions.with(|ss| {
                                ss.iter().map(|session| {
                                    let start_minutes = parse_hhmm_to_minutes(&session.start_time);
                                    let end_minutes = start_minutes.saturating_add(session.duration);
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
                                    let tt = format!(
                                        "{}{}{}",
                                        session.title.as_deref().unwrap_or(""),
                                        if session.title.is_some() { " · " } else { "" },
                                        session.start_time,
                                    );
                                    view! {
                                        <div class=class style=style title=tt></div>
                                    }
                                }).collect_view()
                            }).into_any()
                        }
                    }}
                </div>
            </div>
            {move || {
                if selected_sessions.with(Vec::is_empty) {
                    ().into_any()
                } else {
                    selected_sessions.with(|ss| {
                        view! {
                            <ul class="sessions-list">
                                {ss.iter().map(|session| {
                                    let kind_class = match session.session_type {
                                        SessionType::Focus | SessionType::Custom => "sessions-list-dot focus",
                                        SessionType::Break | SessionType::LongBreak => "sessions-list-dot break",
                                    };
                                    let time_text = format!("{} – {}", session.start_time, session.end_time);
                                    let dur_text = format!("{} min", session.duration);
                                    let title_text = session
                                        .title
                                        .clone()
                                        .filter(|t| !t.is_empty())
                                        .unwrap_or_else(|| match session.session_type {
                                            SessionType::Focus => "Focus".to_string(),
                                            SessionType::Break => "Break".to_string(),
                                            SessionType::LongBreak => "Long Break".to_string(),
                                            SessionType::Custom => "Custom".to_string(),
                                        });
                                    view! {
                                        <li class="sessions-list-item">
                                            <span class=kind_class></span>
                                            <span class="sessions-list-time">{time_text}</span>
                                            <span class="sessions-list-title">{title_text}</span>
                                            <span class="sessions-list-duration">{dur_text}</span>
                                        </li>
                                    }
                                }).collect_view()}
                            </ul>
                        }.into_any()
                    })
                }
            }}
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
        assert_eq!(parse_hhmm_to_minutes("12"), 0);
        assert_eq!(parse_hhmm_to_minutes("08:xx"), 0);
        assert_eq!(parse_hhmm_to_minutes("08:00:00"), 0);
    }

    #[test]
    fn out_of_range_falls_back_to_zero() {
        assert_eq!(parse_hhmm_to_minutes("24:00"), 0);
        assert_eq!(parse_hhmm_to_minutes("12:60"), 0);
    }
}
