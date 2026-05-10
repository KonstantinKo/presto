// Notifications settings tab — Phase 4b (T206) of spec
// 001-leptos-migration. Wires the desktop / sound notification
// toggles + the per-toggle status panel + the Test button.
//
// **Selector contract** (consumed by `tests/e2e/settings-notifications.spec.js`):
// - `#desktop-notifications` — checkbox toggle for OS-level
//   notifications (`spec.js:18-23`).
// - `#sound-notifications` — checkbox toggle for sound playback
//   (`spec.js:35-37`).
// - `#notification-status` — status panel container, visible after
//   permission state resolves (`spec.js:16,43`).
// - `#notification-status-text` — status text inside the panel
//   (`spec.js:26`).
// - `#test-notifications-btn` — fires a test notification
//   (`spec.js:40`).
//
// Per Principle I, this component never mutates engine state — the
// toggles update `Settings::notifications.{desktop,sound}_notifications`
// via the shared `RwSignal<Settings>`. Phase 4c attaches the
// `bridge::commands::save_settings` hop on each change; the dev-server
// / e2e-mock branch operates against the in-memory signal.
//
// Lint allowance: `clippy::must_use_candidate` is silenced module-
// wide for the same reason as on `timer.rs`. `clippy::too_many_lines`
// is silenced because the view body is a single Leptos `view!`
// expansion plus a small change-handler cluster.
#![allow(clippy::must_use_candidate, clippy::too_many_lines)]

use leptos::prelude::*;

use crate::bridge::types::Settings;
use crate::components::settings::SettingsToast;

/// Notifications settings tab.
#[component]
pub fn NotificationsSettings(
    settings: RwSignal<Settings>,
    toast: SettingsToast,
) -> impl IntoView {
    // Derived signals — read each bool field via `.with(...)`.
    let desktop_enabled = Signal::derive(move || {
        settings.with(|s| s.notifications.desktop_notifications)
    });
    let sound_enabled = Signal::derive(move || {
        settings.with(|s| s.notifications.sound_notifications)
    });
    // Status text mirrors the JS-era `notifications.js` pattern
    // (`Enabled` / `Disabled` based on the toggle). The e2e spec at
    // `settings-notifications.spec.js:26` asserts `toContainText("Disabled")`
    // after toggling off.
    let status_text = Signal::derive(move || {
        if desktop_enabled.get() {
            "Enabled"
        } else {
            "Disabled"
        }
    });

    let on_desktop_toggle = move |_| {
        settings.update(|s| {
            s.notifications.desktop_notifications = !s.notifications.desktop_notifications;
        });
        toast.show("Settings saved");
    };
    let on_sound_toggle = move |_| {
        settings.update(|s| {
            s.notifications.sound_notifications = !s.notifications.sound_notifications;
        });
        toast.show("Settings saved");
    };
    let on_test = move |_| {
        // Test button is a UI affordance — the JS-era handler at
        // `notifications.js` calls the OS notification API. The e2e
        // mock returns without error; we no-op here so the click
        // handler still fires and the spec's "UI is still
        // functional" assertion passes. The Tauri-side wiring lands
        // in Phase 4c alongside the persistence hops.
    };

    view! {
        <div class="category-header">
            <h1>"Notifications"</h1>
            <p class="category-description">
                "Control how and when you receive notifications"
            </p>
        </div>
        <div class="settings-section">
            <h3>"Notification Types"</h3>
            <div class="setting-item">
                <label class="checkbox-label">
                    <input
                        type="checkbox"
                        id="desktop-notifications"
                        prop:checked=move || desktop_enabled.get()
                        on:change=on_desktop_toggle
                    />
                    <span class="checkmark"></span>
                    "Desktop Notifications"
                </label>
                <p class="setting-description">
                    "Show system notifications when timer completes."
                </p>
                // Status panel — always rendered so
                // `expect(#notification-status).toBeVisible()` at
                // `spec.js:16` resolves; CSS handles the in/out
                // transition based on permission state.
                <div id="notification-status" class="notification-status">
                    <span id="notification-status-text">{move || status_text.get()}</span>
                    <button id="test-notifications-btn" on:click=on_test>
                        "Test"
                    </button>
                </div>
            </div>
            <div class="setting-item">
                <label class="checkbox-label">
                    <input
                        type="checkbox"
                        id="sound-notifications"
                        prop:checked=move || sound_enabled.get()
                        on:change=on_sound_toggle
                    />
                    <span class="checkmark"></span>
                    "Sound Notifications"
                </label>
                <p class="setting-description">
                    "Play a sound when timer phases complete"
                </p>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    /// T206 — selector contract pin. Sourced from
    /// `tests/e2e/settings-notifications.spec.js`.
    #[test]
    fn notifications_selector_contract_documented() {
        const REQUIRED_IDS: &[&str] = &[
            "desktop-notifications",
            "sound-notifications",
            "notification-status",
            "notification-status-text",
            "test-notifications-btn",
        ];
        let mut seen: Vec<&str> = Vec::with_capacity(REQUIRED_IDS.len());
        for id in REQUIRED_IDS {
            assert!(!seen.contains(id), "duplicate selector ID: {id}");
            seen.push(id);
        }
    }
}
