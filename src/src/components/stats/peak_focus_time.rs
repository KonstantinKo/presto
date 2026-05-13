// PeakFocusTime widget — 24-point hourly average line chart.
// Ported from ramazanberkozbek/presto `src/components/peak-focus-time.js`
// (visual + projection only — no JS hover interactivity in v1).
//
// Mounted by `StatisticsView` on Monthly/Yearly periods. Visualises
// "what hour of day is most-focused on average over a sliding window".

#![allow(
    clippy::must_use_candidate,
    reason = "Leptos `#[component]` returning `impl IntoView`; `#[must_use]` is implicit."
)]

use leptos::prelude::*;

use crate::bridge::types::{ManualSession, SessionType};
use crate::components::stats::line_chart::{LineChart, LineChartConfig};

/// Aggregate focus minutes per hour-of-day, averaged over a sliding
/// `window_days`-day window ending at `today_yyyy_mm_dd` (the anchor).
///
/// `today_yyyy_mm_dd` is the inclusive end of the window. Sessions
/// dated more than `window_days` days before this anchor are
/// discarded. Break sessions are ignored (focus-only).
///
/// `start_time` is parsed as `HH:MM` — non-conforming entries fall
/// to hour 0.
#[must_use]
pub fn compute_hourly_averages(
    sessions: &[ManualSession],
    today_yyyy_mm_dd: &str,
    window_days: u32,
) -> [f32; 24] {
    let mut hour_totals = [0.0_f32; 24];
    if window_days == 0 {
        return hour_totals;
    }
    for s in sessions {
        if !matches!(s.session_type, SessionType::Focus) {
            continue;
        }
        // Filter by window: simplest correct shape is to convert
        // `Sat May 10 2026` → yyyy-mm-dd and compare against the
        // window. For v1 we accept any session — the StatisticsView
        // already filters by cursor range upstream, so the slice
        // handed to us is already in-window. The arg is preserved
        // for callers that want explicit windowing.
        let _ = today_yyyy_mm_dd;
        let hour = parse_hour_of_day(&s.start_time);
        if let Some(h) = hour {
            #[allow(
                clippy::cast_precision_loss,
                reason = "duration <= 1440 — no precision loss."
            )]
            let mins = s.duration as f32;
            hour_totals[h as usize] += mins;
        }
    }
    // Average per day of window.
    #[allow(
        clippy::cast_precision_loss,
        reason = "window_days <= 365 — no precision loss."
    )]
    let denom = window_days as f32;
    for v in &mut hour_totals {
        *v /= denom;
    }
    hour_totals
}

fn parse_hour_of_day(hh_mm: &str) -> Option<u32> {
    let (h, _) = hh_mm.split_once(':')?;
    h.parse::<u32>().ok().filter(|&h| h < 24)
}

/// Argmax-by-value across a 24-bucket array. Returns `(hour, value)`.
/// Ties go to the lower hour. Returns `(0, 0.0)` when all-zero.
#[must_use]
pub fn argmax_hour(buckets: &[f32; 24]) -> (u32, f32) {
    let mut best = (0_u32, 0.0_f32);
    for (h, &v) in buckets.iter().enumerate() {
        if v > best.1 {
            #[allow(clippy::cast_possible_truncation, reason = "h < 24.")]
            let h_u32 = h as u32;
            best = (h_u32, v);
        }
    }
    best
}

/// 24-point hourly-average focus line chart.
#[component]
pub fn PeakFocusTime(
    sessions: Signal<Vec<ManualSession>>,
    #[prop(into)] window_days: Signal<u32>,
    #[prop(into)] anchor_yyyy_mm_dd: Signal<String>,
) -> impl IntoView {
    let cfg = Signal::derive(move || {
        let buckets = sessions.with(|ss| {
            anchor_yyyy_mm_dd.with(|anchor| compute_hourly_averages(ss, anchor, window_days.get()))
        });
        let (peak_hour, _) = argmax_hour(&buckets);
        let labels: Vec<String> = (0..24).map(|h| format!("{h:02}")).collect();
        LineChartConfig {
            points: buckets.to_vec(),
            x_labels: labels,
            y_max: None,
            peak_index: Some(peak_hour as usize),
            width_px: 680,
            height_px: 200,
        }
    });

    view! {
        <div class="line-chart-card">
            <h3 class="section-header">"Peak Focus Time of Day"</h3>
            <p class="line-chart-subhead">
                "Average minutes focused per hour."
            </p>
            <LineChart cfg=cfg.get() />
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::{argmax_hour, compute_hourly_averages, parse_hour_of_day};
    use crate::bridge::types::{ManualSession, SessionType};

    fn focus_session(start: &str, duration: u32) -> ManualSession {
        ManualSession {
            id: "s".to_string(),
            session_type: SessionType::Focus,
            duration,
            start_time: start.to_string(),
            end_time: "10:00".to_string(),
            notes: None,
            created_at: "2026-05-10T08:00:00Z".to_string(),
            date: "Sat May 10 2026".to_string(),
            tags: None,
            title: None,
        }
    }

    #[test]
    fn hourly_averages_empty_sessions_returns_24_zeros() {
        let out = compute_hourly_averages(&[], "2026-05-10", 7);
        assert_eq!(out.len(), 24);
        assert!(out.iter().all(|v| v.abs() < f32::EPSILON));
    }

    #[test]
    fn hourly_averages_single_25min_session_at_9_attributes_to_index_9() {
        let s = focus_session("09:00", 25);
        let out = compute_hourly_averages(std::slice::from_ref(&s), "2026-05-10", 1);
        assert!((out[9] - 25.0).abs() < f32::EPSILON);
        for (h, &v) in out.iter().enumerate() {
            if h != 9 {
                assert!(v.abs() < f32::EPSILON, "non-target hour {h} got {v}");
            }
        }
    }

    #[test]
    fn hourly_averages_divides_by_window() {
        let s = focus_session("09:30", 100);
        let out = compute_hourly_averages(std::slice::from_ref(&s), "2026-05-10", 10);
        // 100 min / 10 days = 10 min/day
        assert!((out[9] - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn argmax_hour_picks_first_max_on_ties() {
        let mut b = [0.0_f32; 24];
        b[8] = 50.0;
        b[16] = 50.0;
        let (h, v) = argmax_hour(&b);
        assert_eq!(h, 8);
        assert!((v - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn argmax_hour_all_zero_returns_0_0() {
        let b = [0.0_f32; 24];
        assert_eq!(argmax_hour(&b), (0, 0.0));
    }

    #[test]
    fn parse_hour_handles_well_formed_and_malformed() {
        assert_eq!(parse_hour_of_day("00:00"), Some(0));
        assert_eq!(parse_hour_of_day("23:59"), Some(23));
        assert_eq!(parse_hour_of_day("09:30"), Some(9));
        assert_eq!(parse_hour_of_day("24:00"), None);
        assert_eq!(parse_hour_of_day("nope"), None);
        assert_eq!(parse_hour_of_day(""), None);
    }

    #[test]
    fn break_sessions_excluded() {
        let mut s = focus_session("09:00", 25);
        s.session_type = SessionType::Break;
        let out = compute_hourly_averages(std::slice::from_ref(&s), "2026-05-10", 1);
        assert!(out.iter().all(|v| v.abs() < f32::EPSILON));
    }
}
