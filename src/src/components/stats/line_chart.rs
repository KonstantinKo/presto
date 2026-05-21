// Reusable LineChart primitive — Bundle Y of feature N (UI parity port
// from ramazanberkozbek/presto). SVG-based line chart consumed by
// `peak_focus_time.rs` (24-point hourly average) and
// `monthly_peak_day.rs` (7-point weekday peak).
//
// Mirrors `bar_chart.rs`'s separation: `compute_line_spec` is the pure
// projection (testable without a DOM), `LineChart` is the Leptos view
// shim. Tests pin the projection contract; the component is induced
// by the same logic.

#![allow(
    clippy::must_use_candidate,
    clippy::too_many_lines,
    reason = "Leptos `#[component]` returning `impl IntoView`; `#[must_use]` is implicit. Mirrors `bar_chart.rs` precedent."
)]

use leptos::prelude::*;
use std::fmt::Write as _;

/// Caller input for the line chart projection.
///
/// Invariants:
/// - `points.len() == x_labels.len()` (parallel slices).
/// - `peak_index`, if Some, is `< points.len()`.
/// - `width_px >= 120` and `height_px >= 80` (enough room for labels).
#[derive(Clone, Debug, PartialEq)]
pub struct LineChartConfig {
    /// Y-values, one per point along the X-axis.
    pub points: Vec<f32>,
    /// X-axis label per point. Parallel to `points`.
    pub x_labels: Vec<String>,
    /// Optional explicit Y-axis ceiling. When `None`, derived from the
    /// max of `points` via `nice_ticks`.
    pub y_max: Option<f32>,
    /// Optional argmax index — emits a dashed vertical guide and a
    /// highlighted dot at this point.
    pub peak_index: Option<usize>,
    /// SVG viewBox width in px.
    pub width_px: u32,
    /// SVG viewBox height in px.
    pub height_px: u32,
}

/// Pure projection of the rendered line shape.
#[derive(Clone, Debug, PartialEq)]
pub struct LineRenderSpec {
    /// SVG `<path d="...">` data (M + L segments).
    pub path_d: String,
    /// Numeric Y-axis tick values (top-down or bottom-up; see
    /// `tick_y_px` for paired pixel offsets).
    pub ticks: Vec<f32>,
    /// Pixel Y-coords for each tick, top-down (SVG y=0 is top).
    pub tick_y_px: Vec<f32>,
    /// Per-point pixel coordinates `(x, y)` inside the viewBox.
    pub point_xy_px: Vec<(f32, f32)>,
    /// Pixel X-coord of the peak point, if `peak_index` was Some.
    pub peak_x_px: Option<f32>,
    /// Effective Y-axis ceiling used to scale the points.
    pub y_max: f32,
}

const PADDING_LEFT: f32 = 40.0;
const PADDING_RIGHT: f32 = 20.0;
const PADDING_TOP: f32 = 16.0;
const PADDING_BOTTOM: f32 = 24.0;

/// "Nice" Y-axis tick policy mirroring
/// ramazanberkozbek/presto's `peak-focus-time.js`:
/// - `max == 0` → ticks `[0, 5, 10, 15]`, ceiling `15`.
/// - `max <= 15` → ceiling = ceil(max/5)*5, 4 ticks.
/// - `max <= 25` → ceiling = ceil(max/5)*5, 6 ticks.
/// - else → ceiling = ceil(max/5)*5, 5 ticks.
#[must_use]
pub fn nice_ticks(max_val: f32) -> (Vec<f32>, f32) {
    if max_val <= 0.0 {
        return (vec![0.0, 5.0, 10.0, 15.0], 15.0);
    }
    let nice_max = (max_val / 5.0).ceil() * 5.0;
    let tick_count = if nice_max <= 15.0 {
        4
    } else if nice_max <= 25.0 {
        6
    } else {
        5
    };
    #[allow(
        clippy::cast_precision_loss,
        reason = "Tick count <= 6 — no precision loss."
    )]
    let denom = (tick_count - 1) as f32;
    let step = nice_max / denom;
    let ticks = (0..tick_count)
        .map(|i| {
            #[allow(
                clippy::cast_precision_loss,
                reason = "Tick index <= 5 — no precision loss."
            )]
            let raw = step * i as f32;
            // Round to nearest integer for clean axis labels.
            raw.round()
        })
        .collect();
    (ticks, nice_max)
}

/// Compute the per-line render spec. Pure — no DOM, no signals.
#[must_use]
pub fn compute_line_spec(cfg: &LineChartConfig) -> LineRenderSpec {
    let raw_max = cfg.points.iter().copied().fold(0.0_f32, f32::max);
    // When `y_max` is set the ticks must follow it — otherwise the
    // labels can show values larger or smaller than the actual chart
    // ceiling. `nice_ticks(target).1` rounds the ceiling up to the
    // policy boundary; pass the override through it so ticks + scale
    // stay in sync regardless of whether the override or the data max
    // wins.
    let tick_target = cfg.y_max.unwrap_or(raw_max);
    let (ticks, derived_max) = nice_ticks(tick_target);
    let y_max = derived_max.max(1.0);

    #[allow(
        clippy::cast_precision_loss,
        reason = "SVG dimensions are UI pixels far below the f32 exact-integer limit."
    )]
    let chart_w = cfg.width_px as f32 - PADDING_LEFT - PADDING_RIGHT;
    #[allow(
        clippy::cast_precision_loss,
        reason = "Same SVG-dimension bound as chart_w."
    )]
    let chart_h = cfg.height_px as f32 - PADDING_TOP - PADDING_BOTTOM;

    let tick_y_px: Vec<f32> = ticks
        .iter()
        .map(|&t| {
            let frac = (t / y_max).clamp(0.0, 1.0);
            // y=0 is top in SVG; tick 0 sits at bottom of chart.
            chart_h.mul_add(1.0 - frac, PADDING_TOP)
        })
        .collect();

    let n = cfg.points.len();
    let point_xy_px: Vec<(f32, f32)> = if n == 0 {
        Vec::new()
    } else if n == 1 {
        // Single point: place it at the horizontal center.
        let frac = (cfg.points[0] / y_max).clamp(0.0, 1.0);
        let y = chart_h.mul_add(1.0 - frac, PADDING_TOP);
        vec![(PADDING_LEFT + chart_w / 2.0, y)]
    } else {
        #[allow(
            clippy::cast_precision_loss,
            reason = "UI chart point count is far below the f32 exact-integer limit."
        )]
        let step = chart_w / (n - 1) as f32;
        cfg.points
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                #[allow(
                    clippy::cast_precision_loss,
                    reason = "Point index is bounded by the same UI chart point count."
                )]
                let x = step.mul_add(i as f32, PADDING_LEFT);
                let frac = (v / y_max).clamp(0.0, 1.0);
                let y = chart_h.mul_add(1.0 - frac, PADDING_TOP);
                (x, y)
            })
            .collect()
    };

    let mut path_d = String::new();
    for (i, &(x, y)) in point_xy_px.iter().enumerate() {
        if i == 0 {
            let _ = write!(path_d, "M {x:.2} {y:.2}");
        } else {
            let _ = write!(path_d, " L {x:.2} {y:.2}");
        }
    }

    let peak_x_px = cfg
        .peak_index
        .and_then(|idx| point_xy_px.get(idx).map(|&(x, _)| x));

    LineRenderSpec {
        path_d,
        ticks,
        tick_y_px,
        point_xy_px,
        peak_x_px,
        y_max,
    }
}

/// Reusable line chart. Renders SVG `<svg>` with gridlines per tick,
/// a polyline `<path>` for the data, `<circle>` per point, and a
/// dashed vertical `<line>` at `peak_x_px` if set.
#[component]
pub fn LineChart(cfg: LineChartConfig) -> impl IntoView {
    let view_box = format!("0 0 {} {}", cfg.width_px, cfg.height_px);
    let spec = compute_line_spec(&cfg);
    let LineChartConfig {
        x_labels,
        peak_index,
        width_px,
        height_px,
        ..
    } = cfg;

    let ticks_with_y = spec
        .ticks
        .iter()
        .copied()
        .zip(spec.tick_y_px.iter().copied());

    let chart_left = PADDING_LEFT;
    #[allow(
        clippy::cast_precision_loss,
        reason = "SVG width is a UI pixel value far below the f32 exact-integer limit."
    )]
    let chart_right = width_px as f32 - PADDING_RIGHT;
    #[allow(
        clippy::cast_precision_loss,
        reason = "SVG height is a UI pixel value far below the f32 exact-integer limit."
    )]
    let chart_bottom = height_px as f32 - PADDING_BOTTOM;

    let labels_with_x: Vec<(String, f32)> = x_labels
        .into_iter()
        .zip(spec.point_xy_px.iter().map(|&(x, _)| x))
        .collect();
    // Show every Nth label so dense X-axes (24 hours) don't overlap.
    let label_stride = if labels_with_x.len() > 12 {
        4
    } else if labels_with_x.len() > 7 {
        2
    } else {
        1
    };

    let path_d = spec.path_d;
    let points = spec.point_xy_px;
    let peak_x = spec.peak_x_px;

    view! {
        <svg
            class="line-chart"
            viewBox=view_box
            preserveAspectRatio="xMidYMid meet"
            xmlns="http://www.w3.org/2000/svg"
        >
            // Y-axis gridlines + labels
            {ticks_with_y.map(|(tick, y)| {
                view! {
                    <g class="line-chart-tick">
                        <line
                            x1=chart_left
                            x2=chart_right
                            y1=y
                            y2=y
                            class="line-chart-gridline"
                        ></line>
                        <text
                            x=chart_left - 6.0
                            y=y + 4.0
                            text-anchor="end"
                            class="line-chart-tick-label"
                        >
                            {format!("{tick:.0}")}
                        </text>
                    </g>
                }
            }).collect_view()}

            // Peak vertical guide
            {peak_x.map(|x| view! {
                <line
                    x1=x
                    x2=x
                    y1=PADDING_TOP
                    y2=chart_bottom
                    class="line-chart-peak-guide"
                    stroke-dasharray="4 4"
                ></line>
            })}

            // Data path
            <path
                class="line-chart-path"
                d=path_d
                fill="none"
            ></path>

            // Per-point dots
            {points.into_iter().enumerate().map(|(i, (x, y))| {
                let is_peak = peak_index == Some(i);
                view! {
                    <circle
                        cx=x
                        cy=y
                        r=if is_peak { 5.0 } else { 3.0 }
                        class="line-chart-dot"
                        class:line-chart-dot-peak=is_peak
                    ></circle>
                }
            }).collect_view()}

            // X-axis labels (every Nth)
            {labels_with_x.into_iter().enumerate().filter_map(|(i, (label, x))| {
                if i % label_stride != 0 {
                    return None;
                }
                Some(view! {
                    <text
                        x=x
                        y=chart_bottom + 16.0
                        text-anchor="middle"
                        class="line-chart-x-label"
                    >
                        {label}
                    </text>
                })
            }).collect_view()}
        </svg>
    }
}

#[cfg(test)]
mod tests {
    use super::{compute_line_spec, nice_ticks, LineChartConfig};

    #[test]
    fn nice_ticks_zero_returns_0_5_10_15() {
        let (ticks, max) = nice_ticks(0.0);
        assert_eq!(ticks, vec![0.0, 5.0, 10.0, 15.0]);
        assert!((max - 15.0).abs() < f32::EPSILON);
    }

    #[test]
    fn nice_ticks_small_returns_4_ticks_step_5() {
        let (ticks, max) = nice_ticks(11.0);
        assert!((max - 15.0).abs() < f32::EPSILON);
        assert_eq!(ticks.len(), 4);
        // Values 0, 5, 10, 15 (round to nearest)
        assert!((ticks[0] - 0.0).abs() < 1.0);
        assert!((ticks[3] - 15.0).abs() < 1.0);
    }

    #[test]
    fn nice_ticks_mid_returns_6_ticks() {
        let (ticks, _) = nice_ticks(20.0);
        assert_eq!(ticks.len(), 6);
    }

    #[test]
    fn nice_ticks_large_returns_5_ticks_multiple_of_5() {
        let (_, max) = nice_ticks(47.0);
        // ceil(47/5)*5 = 50
        assert!((max - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn compute_line_spec_with_7_points_emits_7_pixel_coords() {
        let cfg = LineChartConfig {
            points: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0],
            x_labels: vec!["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            y_max: None,
            peak_index: None,
            width_px: 400,
            height_px: 200,
        };
        let spec = compute_line_spec(&cfg);
        assert_eq!(spec.point_xy_px.len(), 7);
        assert_eq!(spec.path_d.matches('L').count(), 6);
        assert_eq!(spec.path_d.matches('M').count(), 1);
    }

    #[test]
    fn compute_line_spec_peak_index_emits_peak_x() {
        let cfg = LineChartConfig {
            points: vec![1.0, 9.0, 2.0],
            x_labels: vec!["a".into(), "b".into(), "c".into()],
            y_max: None,
            peak_index: Some(1),
            width_px: 200,
            height_px: 100,
        };
        let spec = compute_line_spec(&cfg);
        let peak_x = spec.peak_x_px.expect("peak_x_px set when peak_index given");
        // For 3 points across a 200-wide chart, point[1] is at the center.
        assert!((peak_x - spec.point_xy_px[1].0).abs() < f32::EPSILON);
    }

    #[test]
    fn compute_line_spec_zero_points_returns_empty() {
        let cfg = LineChartConfig {
            points: vec![],
            x_labels: vec![],
            y_max: None,
            peak_index: None,
            width_px: 200,
            height_px: 100,
        };
        let spec = compute_line_spec(&cfg);
        assert!(spec.point_xy_px.is_empty());
        assert!(spec.path_d.is_empty());
    }

    #[test]
    fn compute_line_spec_clamps_value_above_y_max() {
        let cfg = LineChartConfig {
            points: vec![1000.0],
            x_labels: vec!["x".into()],
            y_max: Some(10.0),
            peak_index: None,
            width_px: 200,
            height_px: 100,
        };
        let spec = compute_line_spec(&cfg);
        // Clamped to ceiling — should sit at the top of the chart area
        // (y = PADDING_TOP exactly).
        assert!((spec.point_xy_px[0].1 - 16.0).abs() < 0.01);
    }
}
