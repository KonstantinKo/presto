// MonthlyPeakDay widget — 7-point weekday-totals line chart.
// Ported from ramazanberkozbek/presto `src/components/monthly-peak-day.js`.
//
// Visualises "which weekday accumulates the most focused time within
// the active month". Mounted on the Monthly period.

#![allow(
    clippy::must_use_candidate,
    reason = "Leptos `#[component]` returning `impl IntoView`; `#[must_use]` is implicit."
)]

use leptos::prelude::*;
use leptos_i18n::{t, t_string};

use crate::bridge::types::{ManualSession, SessionType};
use crate::components::stats::line_chart::{LineChart, LineChartConfig};
use crate::i18n::i18n::use_i18n;

/// 7-bucket weekday totals starting Monday=0..Sunday=6, in focus minutes.
///
/// `parse_weekday_index` reads the leading 3-letter day abbreviation
/// of `session.date` (`%a %b %d %Y` shape, e.g. `Sat May 10 2026`)
/// and dispatches to the index — non-conforming entries are skipped.
#[must_use]
pub fn compute_weekday_totals(sessions: &[ManualSession]) -> [f32; 7] {
    let mut totals = [0.0_f32; 7];
    for s in sessions {
        if !matches!(s.session_type, SessionType::Focus) {
            continue;
        }
        let Some(idx) = parse_weekday_index(&s.date) else {
            continue;
        };
        #[allow(
            clippy::cast_precision_loss,
            reason = "duration <= 1440 — no precision loss."
        )]
        let mins = s.duration as f32;
        totals[idx as usize] += mins;
    }
    totals
}

/// Parse the leading 3-letter weekday abbreviation of a `%a %b %d %Y`
/// date string to a Mon=0..Sun=6 index.
#[must_use]
pub fn parse_weekday_index(date_str: &str) -> Option<u32> {
    let prefix = date_str.get(..3)?;
    match prefix {
        "Mon" => Some(0),
        "Tue" => Some(1),
        "Wed" => Some(2),
        "Thu" => Some(3),
        "Fri" => Some(4),
        "Sat" => Some(5),
        "Sun" => Some(6),
        _ => None,
    }
}

/// Argmax-by-value across the 7-bucket weekday array. Returns
/// `(weekday_index, value)`. Ties go to the lower index. Returns
/// `(0, 0.0)` when all-zero.
#[must_use]
pub fn argmax_weekday(buckets: &[f32; 7]) -> (u32, f32) {
    let mut best = (0_u32, 0.0_f32);
    for (i, &v) in buckets.iter().enumerate() {
        if v > best.1 {
            #[allow(clippy::cast_possible_truncation, reason = "i < 7.")]
            let i_u32 = i as u32;
            best = (i_u32, v);
        }
    }
    best
}

/// 7-point weekday-totals line chart.
#[component]
pub fn MonthlyPeakDay(sessions: Signal<Vec<ManualSession>>) -> impl IntoView {
    let i18n = use_i18n();
    let cfg = Signal::derive(move || {
        let buckets = sessions.with(|ss| compute_weekday_totals(ss));
        let (peak_idx, _) = argmax_weekday(&buckets);
        let labels: Vec<String> = vec![
            t_string!(i18n, calendar.dow_mon).to_string(),
            t_string!(i18n, calendar.dow_tue).to_string(),
            t_string!(i18n, calendar.dow_wed).to_string(),
            t_string!(i18n, calendar.dow_thu).to_string(),
            t_string!(i18n, calendar.dow_fri).to_string(),
            t_string!(i18n, calendar.dow_sat).to_string(),
            t_string!(i18n, calendar.dow_sun).to_string(),
        ];
        LineChartConfig {
            points: buckets.to_vec(),
            x_labels: labels,
            y_max: None,
            peak_index: Some(peak_idx as usize),
            width_px: 680,
            height_px: 200,
        }
    });

    view! {
        <div class="line-chart-card">
            <h3 class="section-header">{t!(i18n, stats.monthly_peak_day_title)}</h3>
            <p class="line-chart-subhead">
                {t!(i18n, stats.monthly_peak_day_subhead)}
            </p>
            {move || view! { <LineChart cfg=cfg.get() /> }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::{argmax_weekday, compute_weekday_totals, parse_weekday_index};
    use crate::bridge::types::{ManualSession, SessionType};

    fn session_on(date: &str, duration: u32) -> ManualSession {
        ManualSession {
            id: "s".to_string(),
            session_type: SessionType::Focus,
            duration,
            start_time: "09:00".to_string(),
            end_time: "09:25".to_string(),
            notes: None,
            created_at: "2026-05-10T08:00:00Z".to_string(),
            date: date.to_string(),
            tags: None,
            title: None,
        }
    }

    #[test]
    fn weekday_totals_partitions_by_weekday_index() {
        let sessions = vec![
            session_on("Mon May 11 2026", 25),
            session_on("Mon May 18 2026", 30),
            session_on("Fri May 15 2026", 50),
        ];
        let out = compute_weekday_totals(&sessions);
        assert!((out[0] - 55.0).abs() < f32::EPSILON, "Mon got {}", out[0]);
        assert!((out[4] - 50.0).abs() < f32::EPSILON, "Fri got {}", out[4]);
    }

    #[test]
    fn argmax_weekday_picks_argmax() {
        let mut b = [0.0_f32; 7];
        b[3] = 100.0;
        let (idx, v) = argmax_weekday(&b);
        assert_eq!(idx, 3);
        assert!((v - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_weekday_index_known_abbrevs() {
        assert_eq!(parse_weekday_index("Mon May 10 2026"), Some(0));
        assert_eq!(parse_weekday_index("Sun May 17 2026"), Some(6));
        assert_eq!(parse_weekday_index("Xyz nope"), None);
        assert_eq!(parse_weekday_index(""), None);
    }

    #[test]
    fn weekday_totals_skips_break_sessions() {
        let mut s = session_on("Mon May 11 2026", 25);
        s.session_type = SessionType::Break;
        let out = compute_weekday_totals(std::slice::from_ref(&s));
        assert!(out.iter().all(|v| v.abs() < f32::EPSILON));
    }
}
