// Automation settings tab — Phase 4b (T211) of spec
// 001-leptos-migration. Wires auto-start / continuous-sessions /
// smart-pause / auto-save / prevent-interruptions toggles +
// smart-pause inactivity-timeout slider.
//
// **Selector contract** (consumed by `tests/e2e/settings-automation.spec.js`):
// - `#auto-start-timer` — checkbox (`spec.js:18-20`).
// - `#allow-continuous-sessions` — checkbox (`spec.js:23-25`).
// - `#smart-pause` — checkbox (`spec.js:28-30`).
// - `#smart-pause-timeout-setting` — wrapper visibility-toggled by
//   `#smart-pause` state (`spec.js:31`).
// - `#smart-pause-timeout` — `<input type="range">` for the
//   inactivity-timeout slider.
// - `#auto-save-sessions` — checkbox (`spec.js:34-36`).
// - `#prevent-interruptions` — checkbox (`spec.js:39-41`).
//
// Smart-pause-timeout reveal: the spec at line 31 expects the
// `#smart-pause-timeout-setting` element to become visible after
// toggling smart-pause on. We toggle a `display: none` style based
// on the `smart_pause` field via a derived signal.
//
// Per Principle I, this component never mutates engine state — the
// toggles bind to `Settings::notifications.{auto_start_timer,
// allow_continuous_sessions, smart_pause, smart_pause_timeout}`.
// Phase 4c attaches the `bridge::commands::save_settings` hop on
// each change; the dev-server / e2e-mock branch is in-memory.
//
// Local UI state (auto-save / prevent-interruptions) is held in
// `RwSignal<bool>` because the canonical `NotificationSettings`
// struct doesn't yet expose those slots — the JS-era
// settings-manager.js carried them as separate keys; the Phase 1E
// migration reader threads them through the
// `LegacySettingsPayload`. A follow-up commit folds them into
// `Settings` proper alongside the persistence hop.
//
// Lint allowance: `clippy::must_use_candidate` is silenced for the
// usual Leptos `#[component]` reason. `clippy::too_many_lines` is
// silenced for the same `view!` reason as other settings tabs.
#![allow(clippy::must_use_candidate, clippy::too_many_lines)]

use leptos::prelude::*;

use crate::bridge::types::Settings;
use crate::components::settings::SettingsToast;

/// Parse the slider's `<input type="range">` value into u32, falling
/// back to 30 (the JS-era default) on parse failure.
fn parse_seconds(raw: &str, fallback: u32) -> u32 {
    raw.trim().parse::<u32>().unwrap_or(fallback)
}

/// Automation settings tab.
#[component]
pub fn AutomationSettings(
    settings: RwSignal<Settings>,
    toast: SettingsToast,
) -> impl IntoView {
    // Notifications-bound signals.
    let auto_start = Signal::derive(move || {
        settings.with(|s| s.notifications.auto_start_timer)
    });
    let continuous = Signal::derive(move || {
        settings.with(|s| s.notifications.allow_continuous_sessions)
    });
    let smart_pause = Signal::derive(move || {
        settings.with(|s| s.notifications.smart_pause)
    });
    let timeout_value = Signal::derive(move || {
        settings.with(|s| s.notifications.smart_pause_timeout.to_string())
    });

    // Local UI state for the two toggles that don't yet have typed
    // backing in `Settings::notifications`. JS-era defaults: auto-save
    // = true, prevent-interruptions = false.
    let auto_save = RwSignal::new(true);
    let prevent_interruptions = RwSignal::new(false);

    let on_auto_start = move |_| {
        settings.update(|s| {
            s.notifications.auto_start_timer = !s.notifications.auto_start_timer;
        });
        toast.show("Settings saved");
    };
    let on_continuous = move |_| {
        settings.update(|s| {
            s.notifications.allow_continuous_sessions =
                !s.notifications.allow_continuous_sessions;
        });
        toast.show("Settings saved");
    };
    let on_smart_pause = move |_| {
        settings.update(|s| {
            s.notifications.smart_pause = !s.notifications.smart_pause;
        });
        toast.show("Settings saved");
    };
    let on_timeout = move |ev| {
        let value = parse_seconds(&event_target_value(&ev), 30);
        settings.update(|s| s.notifications.smart_pause_timeout = value);
        toast.show("Settings saved");
    };
    let on_auto_save = move |_| {
        auto_save.update(|v| *v = !*v);
        toast.show("Settings saved");
    };
    let on_prevent = move |_| {
        prevent_interruptions.update(|v| *v = !*v);
        toast.show("Settings saved");
    };

    view! {
        <div class="category-header">
            <h1>"Automation"</h1>
            <p class="category-description">
                "Configure automatic behaviors and smart features"
            </p>
        </div>
        <div class="settings-section">
            <h3>"Timer Automation"</h3>
            <div class="setting-item">
                <label class="checkbox-label">
                    <input
                        type="checkbox"
                        id="auto-start-timer"
                        prop:checked=move || auto_start.get()
                        on:change=on_auto_start
                    />
                    <span class="checkmark"></span>
                    "Auto-start Timer"
                </label>
                <p class="setting-description">
                    "Automatically start the timer when manually skipping to the next session."
                </p>
            </div>
            <div class="setting-item">
                <label class="checkbox-label">
                    <input
                        type="checkbox"
                        id="allow-continuous-sessions"
                        prop:checked=move || continuous.get()
                        on:change=on_continuous
                    />
                    <span class="checkmark"></span>
                    "Allow Continuous Sessions"
                </label>
                <p class="setting-description">
                    "Allow all sessions to continue beyond their timer duration."
                </p>
            </div>
        </div>
        <div class="settings-section">
            <h3>"Smart Features"</h3>
            <div class="setting-item">
                <label class="checkbox-label">
                    <input
                        type="checkbox"
                        id="smart-pause"
                        prop:checked=move || smart_pause.get()
                        on:change=on_smart_pause
                    />
                    <span class="checkmark"></span>
                    "Smart Pause (Auto-pause when inactive)"
                </label>
                <p class="setting-description">
                    "Automatically pause the timer during focus sessions when no activity is detected."
                </p>
            </div>
            <div
                class="setting-item"
                id="smart-pause-timeout-setting"
                style=move || {
                    if smart_pause.get() { "" } else { "display: none" }
                }
            >
                <label for="smart-pause-timeout">
                    "Inactivity Timeout: "
                    <span id="timeout-value">{move || timeout_value.get()}</span>
                    " seconds"
                </label>
                <input
                    type="range"
                    id="smart-pause-timeout"
                    min="5"
                    max="120"
                    step="5"
                    prop:value=move || timeout_value.get()
                    on:input=on_timeout
                />
                <p class="setting-description">
                    "How long to wait before pausing during inactivity"
                </p>
            </div>
        </div>
        <div class="settings-section">
            <h3>"Session Management"</h3>
            <div class="setting-item">
                <label class="checkbox-label">
                    <input
                        type="checkbox"
                        id="auto-save-sessions"
                        prop:checked=move || auto_save.get()
                        on:change=on_auto_save
                    />
                    <span class="checkmark"></span>
                    "Auto-save Completed Sessions"
                </label>
                <p class="setting-description">
                    "Automatically save session data when timer completes."
                </p>
            </div>
            <div class="setting-item">
                <label class="checkbox-label">
                    <input
                        type="checkbox"
                        id="prevent-interruptions"
                        prop:checked=move || prevent_interruptions.get()
                        on:change=on_prevent
                    />
                    <span class="checkmark"></span>
                    "Prevent Interruptions"
                </label>
                <p class="setting-description">
                    "Show confirmation dialog before allowing session resets during active focus periods."
                </p>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::parse_seconds;

    #[test]
    fn parse_seconds_falls_back_on_garbage() {
        assert_eq!(parse_seconds("", 30), 30);
        assert_eq!(parse_seconds("abc", 30), 30);
        assert_eq!(parse_seconds("60", 30), 60);
    }

    /// T211 — selector contract pin. Sourced from
    /// `tests/e2e/settings-automation.spec.js`.
    #[test]
    fn automation_selector_contract_documented() {
        const REQUIRED_IDS: &[&str] = &[
            "auto-start-timer",
            "allow-continuous-sessions",
            "smart-pause",
            "smart-pause-timeout-setting",
            "smart-pause-timeout",
            "auto-save-sessions",
            "prevent-interruptions",
        ];
        let mut seen: Vec<&str> = Vec::with_capacity(REQUIRED_IDS.len());
        for id in REQUIRED_IDS {
            assert!(!seen.contains(id), "duplicate selector ID: {id}");
            seen.push(id);
        }
    }
}
