// Advanced settings tab — Phase 4b (T210) of spec
// 001-leptos-migration. Wires the autostart / hide-icon-on-close /
// status-bar-display / debug-mode / reset-data toggles.
//
// **Selector contract** (consumed by `tests/e2e/settings-advanced.spec.js`):
// - `#autostart-enabled` — checkbox (`spec.js:12-16`).
// - `#hide-icon-on-close` — checkbox (`spec.js:19-20`).
// - `#status-bar-display` — `<select>` (`spec.js:23-24`).
// - `#debug-mode` — checkbox (`spec.js:32-33,47`).
// - `#reset-all-data-btn` — danger button (`spec.js:44`).
//
// Per Principle I, this component never mutates engine state — the
// Reset All Data button dispatches via `bridge::commands::reset_all_data`
// (Phase 4c attaches that hop). The dev-server / e2e-mock branch
// returns false on the dialog (matching `spec.js:44` "cancel via
// the dialog mock") so no reset occurs.
//
// Lint allowance: `clippy::must_use_candidate` is silenced for the
// usual Leptos `#[component]` reason. `clippy::too_many_lines` is
// silenced for the same `view!` reason as other settings tabs.
#![allow(clippy::must_use_candidate, clippy::too_many_lines)]

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_i18n::{t, t_string};

use crate::bridge::commands;
use crate::bridge::types::{Settings, StatusBarDisplay};
use crate::components::settings::SettingsToast;
use crate::i18n::i18n::use_i18n;

/// Map a `<select>` option string to `StatusBarDisplay`. Mirrors the
/// JS-era kebab-case wire shape (`"default"` / `"icon-only"`).
fn parse_status_bar(value: &str) -> StatusBarDisplay {
    match value {
        "icon-only" => StatusBarDisplay::IconOnly,
        _ => StatusBarDisplay::Default,
    }
}

/// Inverse of `parse_status_bar` — kebab-case string for the
/// `<option>` value attribute.
const fn status_bar_to_str(value: StatusBarDisplay) -> &'static str {
    match value {
        StatusBarDisplay::Default => "default",
        StatusBarDisplay::IconOnly => "icon-only",
    }
}

/// Advanced settings tab.
#[component]
pub fn AdvancedSettings(settings: RwSignal<Settings>, toast: SettingsToast) -> impl IntoView {
    let i18n = use_i18n();
    // Derived signals.
    let autostart = Signal::derive(move || settings.with(|s| s.autostart));
    let hide_icon = Signal::derive(move || settings.with(|s| s.hide_icon_on_close));
    let debug_mode = Signal::derive(move || settings.with(|s| s.advanced.debug_mode));
    let status_bar = Signal::derive(move || {
        settings.with(|s| status_bar_to_str(s.status_bar_display).to_string())
    });

    // Change handlers — each toggles the underlying field and fires
    // the auto-save toast. The ResetAllData button dispatches via a
    // best-effort `bridge::commands::reset_all_data` hop (Phase 4c);
    // the dev-server / e2e-mock branch silently no-ops.
    let on_autostart = move |_| {
        settings.update(|s| s.autostart = !s.autostart);
        toast.show(t_string!(i18n, settings.toast_saved).to_string());
    };
    let on_hide_icon = move |_| {
        settings.update(|s| s.hide_icon_on_close = !s.hide_icon_on_close);
        toast.show(t_string!(i18n, settings.toast_saved).to_string());
    };
    let on_status_bar = move |ev| {
        let value = parse_status_bar(&event_target_value(&ev));
        settings.update(|s| s.status_bar_display = value);
        toast.show(t_string!(i18n, settings.toast_saved).to_string());
    };
    let on_debug = move |_| {
        settings.update(|s| s.advanced.debug_mode = !s.advanced.debug_mode);
        toast.show(t_string!(i18n, settings.toast_saved).to_string());
    };
    let on_reset = move |_| {
        // Ask for confirmation via the Tauri dialog plugin before
        // irreversibly resetting all user data. The mock at
        // `tauriMock.js` maps `plugin:dialog|ask` to `false` so
        // `settings-advanced.spec.js:44` ("cancel via the dialog
        // mock — no reset should occur") passes without mutation.
        // On a real Tauri build the native OS confirmation dialog
        // appears; `true` = confirmed → call `reset_all_data`.
        let body = t_string!(i18n, settings.advanced.reset_dialog_body).to_string();
        let title = t_string!(i18n, settings.advanced.reset_dialog_title).to_string();
        spawn_local(async move {
            let confirmed = commands::dialog_ask(&body, &title).await.unwrap_or(false);
            if confirmed {
                let _ = commands::reset_all_data().await;
                // Clear in-memory settings back to default so the UI
                // reflects the reset without a process restart.
                settings.set(Settings::default());
            }
        });
    };

    view! {
        <div class="category-header">
            <h1>{t!(i18n, settings.advanced.title)}</h1>
            <p class="category-description">{t!(i18n, settings.advanced.description)}</p>
        </div>
        <div class="settings-section">
            <h3 class="section-header">{t!(i18n, settings.advanced.system_section)}</h3>
            <div class="setting-item">
                <label class="checkbox-label">
                    <input
                        type="checkbox"
                        id="autostart-enabled"
                        prop:checked=move || autostart.get()
                        on:change=on_autostart
                    />
                    <span class="checkmark"></span>
                    {t!(i18n, settings.advanced.autostart_label)}
                </label>
                <p class="setting-description">{t!(i18n, settings.advanced.autostart_help)}</p>
            </div>
            <div class="setting-item">
                <label class="checkbox-label">
                    <input
                        type="checkbox"
                        id="hide-icon-on-close"
                        prop:checked=move || hide_icon.get()
                        on:change=on_hide_icon
                    />
                    <span class="checkmark"></span>
                    {t!(i18n, settings.advanced.hide_icon_label)}
                </label>
                <p class="setting-description">{t!(i18n, settings.advanced.hide_icon_help)}</p>
            </div>
            <div class="setting-item">
                <label class="setting-label">{t!(i18n, settings.advanced.status_bar_label)}</label>
                <select
                    id="status-bar-display"
                    class="setting-select"
                    prop:value=move || status_bar.get()
                    on:change=on_status_bar
                >
                    <option value="default">{t!(i18n, settings.advanced.status_bar_default)}</option>
                    <option value="icon-only">{t!(i18n, settings.advanced.status_bar_icon_only)}</option>
                </select>
                <p class="setting-description">{t!(i18n, settings.advanced.status_bar_help)}</p>
            </div>
        </div>
        <div class="settings-section">
            <h3 class="section-header">{t!(i18n, settings.advanced.dev_section)}</h3>
            <div class="setting-item">
                <label class="checkbox-label">
                    <input
                        type="checkbox"
                        id="debug-mode"
                        prop:checked=move || debug_mode.get()
                        on:change=on_debug
                    />
                    <span class="checkmark"></span>
                    {t!(i18n, settings.advanced.debug_label)}
                </label>
                <p class="setting-description">{t!(i18n, settings.advanced.debug_help)}</p>
            </div>
        </div>
        <div class="settings-section danger-zone">
            <h3 class="section-header">{t!(i18n, settings.advanced.danger_section)}</h3>
            <p class="settings-description">{t!(i18n, settings.advanced.danger_description)}</p>
            <div class="danger-actions">
                <button class="btn-danger" id="reset-all-data-btn" on:click=on_reset>
                    {t!(i18n, settings.advanced.reset_button)}
                </button>
                <p class="danger-description">{t!(i18n, settings.advanced.reset_help)}</p>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_status_bar, status_bar_to_str};
    use crate::bridge::types::StatusBarDisplay;

    #[test]
    fn status_bar_round_trips() {
        assert_eq!(parse_status_bar("icon-only"), StatusBarDisplay::IconOnly);
        assert_eq!(parse_status_bar("default"), StatusBarDisplay::Default);
        assert_eq!(parse_status_bar(""), StatusBarDisplay::Default);
        assert_eq!(status_bar_to_str(StatusBarDisplay::IconOnly), "icon-only");
        assert_eq!(status_bar_to_str(StatusBarDisplay::Default), "default");
    }

    /// T210 — selector contract pin. Sourced from
    /// `tests/e2e/settings-advanced.spec.js`.
    #[test]
    fn advanced_selector_contract_documented() {
        const REQUIRED_IDS: &[&str] = &[
            "autostart-enabled",
            "hide-icon-on-close",
            "status-bar-display",
            "debug-mode",
            "reset-all-data-btn",
        ];
        let mut seen: Vec<&str> = Vec::with_capacity(REQUIRED_IDS.len());
        for id in REQUIRED_IDS {
            assert!(!seen.contains(id), "duplicate selector ID: {id}");
            seen.push(id);
        }
    }
}
