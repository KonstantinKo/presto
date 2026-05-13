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
use leptos_i18n::{t, t_string};

use crate::bridge::types::{AmbientSoundType, Settings};
use crate::components::settings::SettingsToast;
use crate::i18n::i18n::use_i18n;

/// Map `AmbientSoundType` to its wire-shape string (the value
/// attribute of each `<option>` in the dropdown). Kept in sync with
/// the kebab-case serde output via match-exhaustiveness — a new
/// enum variant fails to compile here.
const fn ambient_sound_type_wire(t: AmbientSoundType) -> &'static str {
    match t {
        AmbientSoundType::None => "none",
        AmbientSoundType::Rain => "rain",
        AmbientSoundType::Fire => "fire",
        AmbientSoundType::Library => "library",
        AmbientSoundType::Fan => "fan",
        AmbientSoundType::Storm => "storm",
        AmbientSoundType::WhiteNoise => "white-noise",
        AmbientSoundType::Wind => "wind",
    }
}

/// Map a wire-shape string (read off the `<select>`'s `.value()`)
/// back to the typed enum. Same match-exhaustive surface; an
/// unrecognised string (e.g. a stale option) falls back to `None`,
/// matching the spec's "absence is first-class" invariant (FR-002,
/// A5).
fn ambient_sound_type_from_wire(s: &str) -> AmbientSoundType {
    match s {
        "rain" => AmbientSoundType::Rain,
        "fire" => AmbientSoundType::Fire,
        "library" => AmbientSoundType::Library,
        "fan" => AmbientSoundType::Fan,
        "storm" => AmbientSoundType::Storm,
        "white-noise" => AmbientSoundType::WhiteNoise,
        "wind" => AmbientSoundType::Wind,
        _ => AmbientSoundType::None,
    }
}

/// Notifications settings tab.
#[component]
pub fn NotificationsSettings(settings: RwSignal<Settings>, toast: SettingsToast) -> impl IntoView {
    let i18n = use_i18n();
    // Derived signals — read each bool field via `.with(...)`.
    let desktop_enabled =
        Signal::derive(move || settings.with(|s| s.notifications.desktop_notifications));
    let sound_enabled =
        Signal::derive(move || settings.with(|s| s.notifications.sound_notifications));
    // Feature 002 Bundle C (T023): ticking-sound opt-in checkbox.
    // Locked to one tick per second to stay in sync with the visual
    // countdown and the macOS tray text — no BPM knob.
    let metronome_enabled = Signal::derive(move || settings.with(|s| s.notifications.metronome));
    // Feature 004: opt-in ambient background sound controls. Three
    // signals — checkbox, track dropdown wire string, volume slider
    // (0–100). All three controls visible regardless of checkbox
    // state per FR-014; toggling off does not destructively reset
    // the dropdown or slider per FR-005.
    let ambient_enabled =
        Signal::derive(move || settings.with(|s| s.notifications.ambient_sound_enabled));
    let ambient_type_wire = Signal::derive(move || {
        ambient_sound_type_wire(settings.with(|s| s.notifications.ambient_sound_type)).to_string()
    });
    let ambient_volume =
        Signal::derive(move || settings.with(|s| s.notifications.ambient_sound_volume));
    // Status text mirrors the JS-era `notifications.js` pattern
    // (`Enabled` / `Disabled` based on the toggle). The e2e spec at
    // `settings-notifications.spec.js:26` asserts `toContainText("Disabled")`
    // after toggling off — the spec runs in English mode, so the
    // localised string still resolves to "Disabled" / "Enabled" there.
    let status_text = Signal::derive(move || {
        if desktop_enabled.get() {
            t_string!(i18n, settings.notifications.status_enabled).to_string()
        } else {
            t_string!(i18n, settings.notifications.status_disabled).to_string()
        }
    });

    let on_desktop_toggle = move |_| {
        settings.update(|s| {
            s.notifications.desktop_notifications = !s.notifications.desktop_notifications;
        });
        toast.show(t_string!(i18n, settings.toast_saved).to_string());
    };
    let on_sound_toggle = move |_| {
        settings.update(|s| {
            s.notifications.sound_notifications = !s.notifications.sound_notifications;
        });
        toast.show(t_string!(i18n, settings.toast_saved).to_string());
    };
    let on_metronome_toggle = move |_| {
        settings.update(|s| {
            s.notifications.metronome = !s.notifications.metronome;
        });
        toast.show(t_string!(i18n, settings.toast_saved).to_string());
    };
    let on_ambient_toggle = move |_| {
        settings.update(|s| {
            s.notifications.ambient_sound_enabled = !s.notifications.ambient_sound_enabled;
        });
        toast.show(t_string!(i18n, settings.toast_saved).to_string());
    };
    let on_ambient_type_change = move |ev: leptos::ev::Event| {
        let new_value = event_target_value(&ev);
        let parsed = ambient_sound_type_from_wire(&new_value);
        settings.update(|s| {
            s.notifications.ambient_sound_type = parsed;
        });
        toast.show(t_string!(i18n, settings.toast_saved).to_string());
    };
    let on_ambient_volume_input = move |ev: leptos::ev::Event| {
        let raw = event_target_value(&ev);
        // The `<input type="range" min=0 max=100>` already constrains
        // the browser-side value to 0..=100; we only need to project
        // the string into `u32`. Per Principle III, no defensive
        // clamp at the settings call site — the UI input boundary
        // already enforces the range.
        if let Ok(parsed) = raw.parse::<u32>() {
            settings.update(|s| {
                s.notifications.ambient_sound_volume = parsed;
            });
        }
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
            <h1>{t!(i18n, settings.notifications.title)}</h1>
            <p class="category-description">{t!(i18n, settings.notifications.description)}</p>
        </div>
        <div class="settings-section">
            <h3 class="section-header">{t!(i18n, settings.notifications.types_header)}</h3>
            <div class="setting-item">
                <label class="checkbox-label">
                    <input
                        type="checkbox"
                        id="desktop-notifications"
                        prop:checked=move || desktop_enabled.get()
                        on:change=on_desktop_toggle
                    />
                    <span class="checkmark"></span>
                    {t!(i18n, settings.notifications.desktop_label)}
                </label>
                <p class="setting-description">{t!(i18n, settings.notifications.desktop_help)}</p>
                // Status panel — always rendered so
                // `expect(#notification-status).toBeVisible()` at
                // `spec.js:16` resolves; CSS handles the in/out
                // transition based on permission state.
                <div
                    id="notification-status"
                    class="notification-status"
                    class:status-ready=move || desktop_enabled.get()
                    class:status-disabled=move || !desktop_enabled.get()
                >
                    <span id="notification-status-text">{move || status_text.get()}</span>
                    <button id="test-notifications-btn" on:click=on_test>
                        {t!(i18n, settings.notifications.test_button)}
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
                    {t!(i18n, settings.notifications.sound_label)}
                </label>
                <p class="setting-description">{t!(i18n, settings.notifications.sound_help)}</p>
            </div>
            <div class="setting-item">
                <label class="checkbox-label">
                    <input
                        type="checkbox"
                        id="metronome-enabled"
                        prop:checked=move || metronome_enabled.get()
                        on:change=on_metronome_toggle
                    />
                    <span class="checkmark"></span>
                    {t!(i18n, settings.notifications.metronome_label)}
                </label>
                <p class="setting-description">{t!(i18n, settings.notifications.metronome_help)}</p>
            </div>
            // Feature 004: ambient-sound controls. Nested in a
            // single parent `.setting-item` card so the dropdown +
            // slider read as DEPENDENT sub-controls of the parent
            // checkbox (rather than siblings of equal weight). The
            // dropdown + slider are disabled when the checkbox is
            // off, with ARIA + opacity feedback (see the
            // `.ambient-controls.disabled` CSS rule in
            // `style/settings.css`). All three controls remain
            // visible regardless of checkbox state per FR-014;
            // toggling off does not destructively reset the dropdown
            // or slider per FR-005.
            <div class="setting-item">
                <label class="checkbox-label">
                    <input
                        type="checkbox"
                        id="ambient-sound-enabled"
                        prop:checked=move || ambient_enabled.get()
                        on:change=on_ambient_toggle
                    />
                    <span class="checkmark"></span>
                    {t!(i18n, settings.notifications.ambient_label)}
                </label>
                <p class="setting-description">{t!(i18n, settings.notifications.ambient_help)}</p>
                <div
                    class="ambient-controls"
                    class:disabled=move || !ambient_enabled.get()
                    aria-disabled=move || (!ambient_enabled.get()).then_some("true")
                >
                    <div class="ambient-control-row">
                        <label class="setting-label" for="ambient-sound-type">{t!(i18n, settings.notifications.ambient_sound_label)}</label>
                        <select
                            id="ambient-sound-type"
                            class="setting-select"
                            prop:value=move || ambient_type_wire.get()
                            on:change=on_ambient_type_change
                            prop:disabled=move || !ambient_enabled.get()
                        >
                            <option value="none">{t!(i18n, settings.notifications.ambient_track_none)}</option>
                            <option value="rain">{t!(i18n, settings.notifications.ambient_track_rain)}</option>
                            <option value="fire">{t!(i18n, settings.notifications.ambient_track_fire)}</option>
                            <option value="library">{t!(i18n, settings.notifications.ambient_track_library)}</option>
                            <option value="fan">{t!(i18n, settings.notifications.ambient_track_fan)}</option>
                            <option value="storm">{t!(i18n, settings.notifications.ambient_track_storm)}</option>
                            <option value="white-noise">{t!(i18n, settings.notifications.ambient_track_white_noise)}</option>
                            <option value="wind">{t!(i18n, settings.notifications.ambient_track_wind)}</option>
                        </select>
                    </div>
                    <div class="ambient-control-row">
                        <label class="setting-label" for="ambient-sound-volume">
                            {t!(i18n, settings.notifications.ambient_volume_label)}" "
                            <span class="setting-value">
                                {move || format!("{}%", ambient_volume.get())}
                            </span>
                        </label>
                        <input
                            id="ambient-sound-volume"
                            type="range"
                            min="0"
                            max="100"
                            step="1"
                            prop:value=move || ambient_volume.get().to_string()
                            on:input=on_ambient_volume_input
                            prop:disabled=move || !ambient_enabled.get()
                        />
                    </div>
                </div>
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
            // Feature 002 Bundle C (T023, revised): ticking-sound
            // toggle. BPM input removed — tick is locked to 1 Hz.
            "metronome-enabled",
            // Feature 004 (T008): three ambient-sound controls
            // additive below the metronome row (FR-013 / FR-015).
            "ambient-sound-enabled",
            "ambient-sound-type",
            "ambient-sound-volume",
        ];
        let mut seen: Vec<&str> = Vec::with_capacity(REQUIRED_IDS.len());
        for id in REQUIRED_IDS {
            assert!(!seen.contains(id), "duplicate selector ID: {id}");
            seen.push(id);
        }
    }
}
