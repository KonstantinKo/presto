// Reusable BarChart component — Bundle A of feature 003.
//
// Single `pub fn BarChart` definition consumed four times across the
// Statistics view's period tabs (Daily / Weekly / Monthly / Yearly).
// Caller-applied per-period max_scale floor + rounding policy lives in
// `stats::mod` (see `BarChartProps` constructors there); this module
// is shape-agnostic.
//
// Contract: `specs/003-stats-redesign/contracts/components.md`
// §Contract 2. Optional non-RED-first tests cover SC-002 (single
// definition), SC-003 (bar count == labels.len()), SC-004 (min-height
// floor on all-zero rows).

#![allow(
    clippy::must_use_candidate,
    clippy::too_many_lines,
    reason = "Leptos `#[component]` functions returning `impl IntoView` are consumed by the parent `view!` macro; `#[must_use]` is implicit. Body is a single `view!` macro expansion. Matches `calendar.rs:32` precedent."
)]

use leptos::prelude::*;

/// Reusable bar-chart input contract.
///
/// Mirrors the `data-model.md §BarChartProps` shape but lives under a
/// distinct name (`BarChartConfig`) because Leptos's `#[component]`
/// macro auto-generates a type named `BarChartProps` for the
/// `BarChart` component's prop builder; the two names cannot collide.
/// The `BarChart` component below accepts the struct's fields spread
/// as individual parameters (the macro requires this);
/// `StatisticsView` constructs a `BarChartConfig` to keep the
/// caller-side bundle clean.
///
/// Invariants (caller-enforced — the component trusts the inputs):
/// - `x_axis_labels.len() == bar_values.len()` (parallel slices)
/// - `bar_values.iter().max() <= max_scale`
/// - `min_bar_height_px >= 4`
///
/// See `data-model.md §BarChartProps` for the rounding policy on
/// `max_scale` (Daily fixed 60; Weekly/Monthly nearest-10 with ≥20/≥50
/// floors; Yearly nearest-50 with ≥100 floor).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BarChartConfig {
    /// Y-axis ceiling in focus-minutes. Bars normalise as
    /// `bar_values[i] / max_scale` (clamped to 0..=1).
    pub max_scale: u32,
    /// One label per bar; parallel to `bar_values`.
    pub x_axis_labels: Vec<String>,
    /// Focus-minute total per bar; parallel to `x_axis_labels`.
    pub bar_values: Vec<u32>,
    /// Visual floor; bars with value 0 render at this height so the
    /// chart never has zero-height bars (FR-006).
    pub min_bar_height_px: u32,
}

/// Pure projection of the rendered chart shape.
///
/// Exposed so tests can pin the bar count + height floor without
/// mounting a Leptos view (no `Document` is available under
/// `wasm-pack test --node`). Mirrors the `IconClass::render_spec`
/// pattern from `components::icon` (T005).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BarRenderSpec {
    /// Label for the bar's x-axis tick.
    pub label: String,
    /// Computed pixel height after applying the `min_bar_height_px`
    /// floor when `value == 0` and the per-bar `value / max_scale`
    /// fraction otherwise.
    pub height_px: u32,
    /// The raw focus-minute total for the bar (pre-floor).
    pub value: u32,
}

const CHART_HEIGHT_PX: u32 = 200;

/// Compute the per-bar render spec for a given props instance. Pure
/// function — no DOM, no signals. The Leptos `BarChart` component
/// below renders one `<div class="bar">` per entry returned here.
#[must_use]
pub fn compute_render_spec(props: &BarChartConfig) -> Vec<BarRenderSpec> {
    let max = props.max_scale.max(1);
    let floor = props.min_bar_height_px;
    props
        .x_axis_labels
        .iter()
        .zip(props.bar_values.iter())
        .map(|(label, &value)| {
            // Saturating ratio: value ≤ max_scale by caller contract,
            // but `min(max)` defends against caller drift without
            // panicking.
            let clamped = value.min(max);
            // `clamped <= max` guarantees `(clamped * CHART_HEIGHT_PX) / max
            // <= CHART_HEIGHT_PX`, which fits in `u32` by construction.
            let scaled_u64 = u64::from(clamped) * u64::from(CHART_HEIGHT_PX) / u64::from(max);
            let scaled = u32::try_from(scaled_u64).unwrap_or(CHART_HEIGHT_PX);
            let height_px = scaled.max(floor);
            BarRenderSpec {
                label: label.clone(),
                height_px,
                value,
            }
        })
        .collect()
}

/// Reusable bar chart. Single `pub fn BarChart` per the SC-002 grep
/// check; instantiated four times from `StatisticsView`.
///
/// The props are spread as individual parameters here (Leptos's
/// `#[component]` macro generates a builder per parameter; a single
/// `props: BarChartProps` would collapse into one inaccessible field).
/// `BarChartProps` remains the documented data-model shape and is the
/// canonical bundle at the call site — `StatisticsView` constructs one
/// and passes its fields through to `<BarChart .../>`.
///
/// Layout:
/// - `.bar-chart-container` flex row holds the bars
/// - one `<div class="bar">` per (label, value) pair with inline
///   `height: <px>px` styling
/// - per-bar `<span class="bar-label">` below carries the x-axis label
/// - total-time footer below the chart projects the sum of
///   `bar_values` (for a quick at-a-glance summary)
#[component]
pub fn BarChart(
    max_scale: u32,
    x_axis_labels: Vec<String>,
    bar_values: Vec<u32>,
    min_bar_height_px: u32,
) -> impl IntoView {
    let props = BarChartConfig {
        max_scale,
        x_axis_labels,
        bar_values,
        min_bar_height_px,
    };
    let total: u32 = props.bar_values.iter().copied().sum();
    let spec = compute_render_spec(&props);

    view! {
        <div class="bar-chart-card">
            <div class="bar-chart-container">
                {spec.into_iter().map(|bar| {
                    let style = format!("height: {}px", bar.height_px);
                    let title_text = format!("{}: {} min", bar.label, bar.value);
                    view! {
                        <div class="bar-column">
                            <div class="bar" style=style title=title_text></div>
                            <span class="bar-label">{bar.label}</span>
                        </div>
                    }
                }).collect_view()}
            </div>
            <div class="bar-chart-footer">
                <span class="bar-chart-total">{format!("Total: {total} min")}</span>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::{compute_render_spec, BarChartConfig};

    /// SC-003: Daily period renders 24 bars (one per hour).
    #[test]
    fn daily_period_has_24_bars() {
        let labels: Vec<String> = (0..24).map(|h| format!("{h:02}:00")).collect();
        let values = vec![0_u32; 24];
        let props = BarChartConfig {
            max_scale: 60,
            x_axis_labels: labels,
            bar_values: values,
            min_bar_height_px: 4,
        };
        let spec = compute_render_spec(&props);
        assert_eq!(spec.len(), 24, "Daily period must render 24 hourly bars");
    }

    /// SC-003: Weekly period renders 7 bars (one per day).
    #[test]
    fn weekly_period_has_7_bars() {
        let labels: Vec<String> = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        let values = vec![0_u32; 7];
        let props = BarChartConfig {
            max_scale: 20,
            x_axis_labels: labels,
            bar_values: values,
            min_bar_height_px: 4,
        };
        let spec = compute_render_spec(&props);
        assert_eq!(spec.len(), 7, "Weekly period must render 7 day bars");
    }

    /// SC-003: Monthly period spans 28..=31 bars. Test with a 31-day
    /// shape since the longer end is the more demanding for layout.
    #[test]
    fn monthly_period_has_28_to_31_bars() {
        let labels: Vec<String> = (1..=31).map(|d| d.to_string()).collect();
        let values = vec![0_u32; 31];
        let props = BarChartConfig {
            max_scale: 50,
            x_axis_labels: labels,
            bar_values: values,
            min_bar_height_px: 4,
        };
        let spec = compute_render_spec(&props);
        assert!(
            (28..=31).contains(&spec.len()),
            "Monthly period bar count must be 28..=31, got {}",
            spec.len()
        );
    }

    /// SC-003: Yearly period renders 12 bars (Jan..Dec).
    #[test]
    fn yearly_period_has_12_bars() {
        let labels: Vec<String> = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
        let values = vec![0_u32; 12];
        let props = BarChartConfig {
            max_scale: 100,
            x_axis_labels: labels,
            bar_values: values,
            min_bar_height_px: 4,
        };
        let spec = compute_render_spec(&props);
        assert_eq!(spec.len(), 12, "Yearly period must render 12 month bars");
    }

    /// SC-004: all-zero values render at the `min_bar_height_px` floor
    /// rather than zero-height. Matches the FR-006 floor invariant.
    #[test]
    fn all_zero_values_render_at_floor() {
        let labels: Vec<String> = vec!["Mon".to_string(); 7];
        let values = vec![0_u32; 7];
        let props = BarChartConfig {
            max_scale: 20,
            x_axis_labels: labels,
            bar_values: values,
            min_bar_height_px: 8,
        };
        let spec = compute_render_spec(&props);
        for bar in &spec {
            assert_eq!(
                bar.height_px, 8,
                "all-zero bars must render at the min_bar_height_px floor"
            );
        }
    }

    /// SC-004: max-scale value renders at the chart's full pixel
    /// height (`CHART_HEIGHT_PX` constant in this module = 200 px).
    #[test]
    fn max_value_renders_at_full_height() {
        let labels: Vec<String> = vec!["Mon".to_string()];
        let values = vec![20_u32];
        let props = BarChartConfig {
            max_scale: 20,
            x_axis_labels: labels,
            bar_values: values,
            min_bar_height_px: 4,
        };
        let spec = compute_render_spec(&props);
        assert_eq!(
            spec[0].height_px, 200,
            "value == max_scale should render at full chart height"
        );
    }
}
