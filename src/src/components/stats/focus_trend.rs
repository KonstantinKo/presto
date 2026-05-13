// FocusTrend widget — three horizontal-bar comparison card.
// Ported from ramazanberkozbek/presto `src/components/focus-trend.js`.
//
// Three rows of "<label> | <bar fill> | <minutes>" stacked
// vertically. Each row shows that day's focus minutes; the bar fill
// is normalised against the max across the window. A small comparison
// dot on each non-first row sits at the previous row's percentage so
// the reader can eyeball the trend.

#![allow(
    clippy::must_use_candidate,
    reason = "Leptos `#[component]` returning `impl IntoView`; `#[must_use]` is implicit."
)]

use leptos::prelude::*;
use leptos_i18n::t;

use crate::bridge::types::{ManualSession, SessionType};
use crate::i18n::i18n::use_i18n;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrendRow {
    pub label: String,
    /// Minutes for this row.
    pub minutes: u32,
    /// Comparison anchor — minutes from the previous (newer) row.
    /// `None` for the first/top row.
    pub comparison_minutes: Option<u32>,
}

/// Compute the trend rows from a session slice + a parallel list of labels.
///
/// `rows` is most-recent first; each entry pairs a label with a
/// `%a %b %d %Y` date prefix (e.g. `Sat May 10 2026`). The slice is
/// matched to each row by date-prefix equality. Returns one
/// `TrendRow` per pair, in input order. `comparison_minutes`
/// references the previous row's minutes.
#[must_use]
pub fn compute_focus_trend(sessions: &[ManualSession], rows: &[(String, String)]) -> Vec<TrendRow> {
    let mut out = Vec::with_capacity(rows.len());
    let mut prev_minutes: Option<u32> = None;
    for (label, date_prefix) in rows {
        let minutes: u32 = sessions
            .iter()
            .filter(|s| matches!(s.session_type, SessionType::Focus))
            .filter(|s| s.date == *date_prefix)
            .map(|s| s.duration)
            .sum();
        out.push(TrendRow {
            label: label.clone(),
            minutes,
            comparison_minutes: prev_minutes,
        });
        prev_minutes = Some(minutes);
    }
    out
}

/// Format minutes as `Xh Ym` (drops the hour segment when zero).
#[must_use]
pub fn format_minutes(minutes: u32) -> String {
    let h = minutes / 60;
    let m = minutes % 60;
    if h == 0 {
        format!("{m}m")
    } else {
        format!("{h}h {m}m")
    }
}

/// Per-row fill percentage normalised against the max minutes in the
/// window. Returns 0.0 for an all-zero window (renders an empty bar).
#[must_use]
pub fn row_fill_percent(row_minutes: u32, max_minutes: u32) -> f32 {
    if max_minutes == 0 {
        return 0.0;
    }
    #[allow(
        clippy::cast_precision_loss,
        reason = "Minutes fit in f32 for any realistic day."
    )]
    let frac = row_minutes as f32 / max_minutes as f32;
    (frac * 100.0).clamp(0.0, 100.0)
}

/// Three horizontal-bar comparison card.
#[component]
pub fn FocusTrend(
    sessions: Signal<Vec<ManualSession>>,
    #[prop(into)] rows: Signal<Vec<(String, String)>>,
) -> impl IntoView {
    let i18n = use_i18n();
    let trend =
        Signal::derive(move || sessions.with(|ss| rows.with(|rs| compute_focus_trend(ss, rs))));

    view! {
        <div class="focus-trend-card">
            <h3 class="section-header">{t!(i18n, stats.focus_trend_title)}</h3>
            <div class="trend-days-container">
                {move || {
                    let snapshot = trend.get();
                    let max_minutes = snapshot.iter().map(|r| r.minutes).max().unwrap_or(0);
                    snapshot.into_iter().enumerate().map(|(idx, row)| {
                        let fill_pct = row_fill_percent(row.minutes, max_minutes);
                        let compare_pct = row
                            .comparison_minutes
                            .map(|m| row_fill_percent(m, max_minutes));
                        let bar_style = format!("width: {fill_pct:.1}%;");
                        let dot_style = compare_pct.map(|p| format!("left: {p:.1}%;"));
                        let bar_variant = match idx {
                            0 => "trend-current",
                            1 => "trend-previous",
                            _ => "trend-older",
                        };
                        let bar_class = format!("trend-bar-fill {bar_variant}");
                        view! {
                            <div class="trend-day">
                                <div class="trend-day-header">
                                    <div class="trend-day-label">{row.label.to_uppercase()}</div>
                                    <div class="trend-day-meta">
                                        <span class="trend-day-time">{format_minutes(row.minutes)}</span>
                                    </div>
                                </div>
                                <div class="trend-bar-wrapper">
                                    <div class="trend-bar-bg">
                                        <div class=bar_class style=bar_style></div>
                                        {dot_style.map(|s| view! {
                                            <div class="trend-comparison-dot" style=s>
                                                <div class="trend-comparison-dot-inner"></div>
                                            </div>
                                        })}
                                    </div>
                                </div>
                            </div>
                        }
                    }).collect_view()
                }}
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::{compute_focus_trend, row_fill_percent, TrendRow};
    use crate::bridge::types::{ManualSession, SessionType};

    fn session(date: &str, duration: u32) -> ManualSession {
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
    fn focus_trend_returns_one_row_per_input() {
        let sessions = vec![
            session("Tue May 12 2026", 25),
            session("Mon May 11 2026", 50),
        ];
        let rows = vec![
            ("Today".to_string(), "Tue May 12 2026".to_string()),
            ("Yesterday".to_string(), "Mon May 11 2026".to_string()),
            ("Day before".to_string(), "Sun May 10 2026".to_string()),
        ];
        let out = compute_focus_trend(&sessions, &rows);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].minutes, 25);
        assert_eq!(out[1].minutes, 50);
        assert_eq!(out[2].minutes, 0);
    }

    #[test]
    fn focus_trend_comparison_chains_previous_row() {
        let sessions = vec![
            session("Tue May 12 2026", 25),
            session("Mon May 11 2026", 50),
        ];
        let rows = vec![
            ("Today".to_string(), "Tue May 12 2026".to_string()),
            ("Yesterday".to_string(), "Mon May 11 2026".to_string()),
        ];
        let out = compute_focus_trend(&sessions, &rows);
        assert_eq!(out[0].comparison_minutes, None);
        assert_eq!(out[1].comparison_minutes, Some(25));
    }

    #[test]
    fn focus_trend_skips_break_sessions() {
        let mut s = session("Tue May 12 2026", 25);
        s.session_type = SessionType::Break;
        let out = compute_focus_trend(
            std::slice::from_ref(&s),
            &[("Today".to_string(), "Tue May 12 2026".to_string())],
        );
        assert_eq!(out[0].minutes, 0);
    }

    #[test]
    fn row_fill_percent_normalises_against_max() {
        assert!((row_fill_percent(25, 100) - 25.0).abs() < f32::EPSILON);
        assert!((row_fill_percent(100, 100) - 100.0).abs() < f32::EPSILON);
        assert!((row_fill_percent(0, 0) - 0.0).abs() < f32::EPSILON);
        // Clamps overflow.
        assert!((row_fill_percent(200, 100) - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn trend_row_is_clonable() {
        let r = TrendRow {
            label: "Today".to_string(),
            minutes: 25,
            comparison_minutes: None,
        };
        let r2 = r.clone();
        assert_eq!(r, r2);
    }
}
