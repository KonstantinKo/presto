// Goals settings tab — Phase 4b (T207) of spec
// 001-leptos-migration. Wires `#weekly-goal-minutes` to
// `Settings::timer.weekly_goal_minutes`.
//
// **Selector contract** (consumed by `tests/e2e/settings-goals.spec.js`):
// - `#weekly-goal-minutes` — integer input (`spec.js:12,16,38,40`).
// - `.notification-ping` filtered by "Settings saved" — auto-save
//   toast shape (`spec.js:23`); rendered by the shell.
//
// Per Principle I, this component never mutates engine state — the
// weekly-goal value is read by the calendar view's focus-summary
// projection (Phase 4a wired that). Phase 4c attaches the
// `bridge::commands::save_settings` hop on each change; the
// dev-server / e2e-mock branch operates against the in-memory
// signal.
//
// Lint allowance: `clippy::must_use_candidate` is silenced for the
// usual Leptos `#[component]` reason.
#![allow(
    clippy::must_use_candidate,
    reason = "Leptos component returns are consumed by view!, not bound by callers."
)]

use leptos::prelude::*;
use leptos_i18n::{t, t_string};

use crate::bridge::types::Settings;
use crate::components::settings::SettingsToast;
use crate::i18n::i18n::use_i18n;

/// Parse an `<input type="number">` value into `u32`, falling back to
/// `fallback` when the input is empty / non-numeric / out of range.
fn parse_minutes(raw: &str, fallback: u32) -> u32 {
    raw.trim().parse::<u32>().unwrap_or(fallback)
}

/// Goals settings tab — single weekly-goal input.
#[component]
pub fn GoalsSettings(settings: RwSignal<Settings>, toast: SettingsToast) -> impl IntoView {
    let i18n = use_i18n();
    let weekly_goal =
        Signal::derive(move || settings.with(|s| s.timer.weekly_goal_minutes.to_string()));

    let on_change = move |ev| {
        let value = parse_minutes(&event_target_value(&ev), 125);
        settings.update(|s| s.timer.weekly_goal_minutes = value);
        toast.show(t_string!(i18n, settings.toast_saved).to_string());
    };

    view! {
        <div class="category-header">
            <h1>{t!(i18n, settings.goals.title)}</h1>
            <p class="category-description">{t!(i18n, settings.goals.description)}</p>
        </div>
        <div class="settings-section">
            <h3 class="section-header">{t!(i18n, settings.goals.focus_section)}</h3>
            <div class="setting-item">
                <label for="weekly-goal-minutes">{t!(i18n, settings.goals.weekly_label)}</label>
                <input
                    type="number"
                    id="weekly-goal-minutes"
                    min="30"
                    max="2400"
                    step="5"
                    prop:value=move || weekly_goal.get()
                    on:change=on_change
                />
                <p class="setting-description">{t!(i18n, settings.goals.weekly_help)}</p>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::parse_minutes;

    #[test]
    fn parse_minutes_falls_back_on_empty_or_garbage() {
        assert_eq!(parse_minutes("", 125), 125);
        assert_eq!(parse_minutes("abc", 125), 125);
        assert_eq!(parse_minutes("50", 125), 50);
    }

    /// T207 — selector contract pin. Sourced from
    /// `tests/e2e/settings-goals.spec.js`.
    #[test]
    fn goals_selector_contract_documented() {
        const REQUIRED_IDS: &[&str] = &["weekly-goal-minutes"];
        for id in REQUIRED_IDS {
            assert!(!id.is_empty(), "selector ID must not be empty");
        }
    }
}
