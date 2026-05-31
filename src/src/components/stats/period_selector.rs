// Period selector — four-tab UI for Daily/Weekly/Monthly/Yearly.
//
// `Period` is a closed sum type held in `RwSignal<Period>` by
// `StatisticsView`; this component owns the tab buttons that mutate it.
// Cold-load default is `Period::Weekly` (FR-003 / SC-001). Tab swap
// resets the cursor to the new period's "current" anchor (FR-008 /
// SC-005) — but the cursor reset is owned by `StatisticsView`, not by
// this component (the selector is shape-only).

#![allow(
    clippy::must_use_candidate,
    reason = "Leptos `#[component]` returning `impl IntoView`; `#[must_use]` is implicit."
)]

use leptos::prelude::*;
use leptos_i18n::t_string;

use crate::components::icon::{self, IconClass};
use crate::i18n::i18n::use_i18n;

/// Closed sum type for the four Statistics-view periods.
///
/// Held in `RwSignal<Period>` by `StatisticsView`. Drives the bar
/// chart, the per-period navigator, and the tag-usage pie. Never
/// persisted; never serialised across the Tauri bridge (FR-002).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Period {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

impl Period {
    /// Stable `data-period` attribute value for each variant. Used by
    /// the tab buttons' selectors so e2e specs can address each tab
    /// without coupling to display text.
    #[must_use]
    pub const fn data_attr(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
            Self::Yearly => "yearly",
        }
    }

    /// Leading-icon remix class name for the period pill. Matches the
    /// ramazanberkozbek fork's icon set for visual parity.
    #[must_use]
    pub const fn icon_name(self) -> &'static str {
        match self {
            Self::Daily => "ri-calendar-todo-line",
            Self::Weekly => "ri-calendar-line",
            Self::Monthly => "ri-calendar-event-line",
            Self::Yearly => "ri-calendar-check-line",
        }
    }
}

/// Period tab selector. The active period gets a `.active` modifier
/// class so CSS can highlight it; click on any other tab calls
/// `on_select` with the clicked variant.
#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::Period;

    const ALL: [Period; 4] = [
        Period::Daily,
        Period::Weekly,
        Period::Monthly,
        Period::Yearly,
    ];

    #[test]
    fn data_attr_values_are_stable() {
        assert_eq!(Period::Daily.data_attr(), "daily");
        assert_eq!(Period::Weekly.data_attr(), "weekly");
        assert_eq!(Period::Monthly.data_attr(), "monthly");
        assert_eq!(Period::Yearly.data_attr(), "yearly");
    }

    #[test]
    fn all_data_attrs_are_distinct() {
        let attrs: HashSet<_> = ALL.iter().map(|p| p.data_attr()).collect();
        assert_eq!(attrs.len(), ALL.len());
    }

    #[test]
    fn icon_names_are_non_empty_strings() {
        for p in ALL {
            assert!(!p.icon_name().is_empty(), "{p:?} has empty icon name");
        }
    }

    #[test]
    fn all_icon_names_are_distinct() {
        let icons: HashSet<_> = ALL.iter().map(|p| p.icon_name()).collect();
        assert_eq!(icons.len(), ALL.len());
    }
}

///
/// `StatisticsView` owns the cursor-reset behaviour (FR-008) — this
/// component is shape-only.
#[component]
pub fn PeriodSelector(
    current: RwSignal<Period>,
    #[prop(into)] on_select: Callback<Period>,
) -> impl IntoView {
    let i18n = use_i18n();
    let variants = [
        Period::Daily,
        Period::Weekly,
        Period::Monthly,
        Period::Yearly,
    ];

    view! {
        <div class="period-tabs" role="tablist" aria-label=move || t_string!(i18n, stats.period_aria_label).to_string()>
            {variants.into_iter().map(|period| {
                let is_active =
                    Signal::derive(move || current.with(|c| *c == period));
                let icon = IconClass::from_icon_name(period.icon_name());
                let label = Signal::derive(move || match period {
                    Period::Daily => t_string!(i18n, stats.period_daily).to_string(),
                    Period::Weekly => t_string!(i18n, stats.period_weekly).to_string(),
                    Period::Monthly => t_string!(i18n, stats.period_monthly).to_string(),
                    Period::Yearly => t_string!(i18n, stats.period_yearly).to_string(),
                });
                view! {
                    <button
                        class="period-btn"
                        class:active=move || is_active.get()
                        role="tab"
                        data-period=period.data_attr()
                        aria-selected=move || if is_active.get() { "true" } else { "false" }
                        on:click=move |_| on_select.run(period)
                    >
                        {icon::render(&icon)}
                        <span>{move || label.get()}</span>
                    </button>
                }
            }).collect_view()}
        </div>
    }
}
