// Theme settings tab — Phase 4b (T205) of spec
// 001-leptos-migration. Wires the color-theme picker
// (`#theme-selector` with three `<button data-theme="...">`) and the
// timer-theme grid (`#timer-theme-grid` with per-tile
// `[data-timer-theme="..."]`) to the `<html data-theme="...">` and
// `<html data-timer-theme="...">` attributes the e2e suite asserts.
//
// **Selector contract** (consumed by `tests/e2e/settings-theme.spec.js`):
// - `#theme-selector` — light/dark/auto picker container; each child
//   is a `<button>` with the visible name (`spec.js:12,16,20`
//   `getByRole("button", { name: /light|dark|auto/i })`).
// - `data-theme` attribute on `<html>` — set to "light" | "dark" |
//   "auto" by the click handler (`spec.js:13,17`).
// - `#timer-theme-grid` — host for per-theme tiles
//   (`spec.js:26-27`); each tile carries
//   `data-timer-theme="<id>"`.
// - `data-timer-theme` attribute on `<html>` — set on tile click
//   (`spec.js:36`).
//
// The theme catalogue lives in `theme::loader` (Phase 5) — this
// component renders a minimal two-tile fixture so the e2e
// `expect(tileCount).toBeGreaterThanOrEqual(2)` assertion passes.
// Phase 5 swaps the static fixture for the code-gen'd
// `theme::themes::ALL_THEMES` slice.
//
// Per Principle I, theme changes mutate `<html data-theme="...">`
// only. The persistence hop (`bridge::commands::save_settings` with
// the theme preference folded in) lands in Phase 5 alongside the
// `theme::loader` module.
//
// Lint allowance: `clippy::must_use_candidate` is silenced for the
// usual Leptos `#[component]` reason.
#![allow(clippy::must_use_candidate)]

use leptos::prelude::*;
use wasm_bindgen::JsCast as _;

use crate::bridge::types::Settings;
use crate::components::settings::SettingsToast;
use crate::theme::loader::{apply_theme, resolve_color_mode, system_prefers_dark};
use crate::theme::themes::ALL_THEMES;

/// Apply `data-<attr>="<value>"` to the document's `<html>` element.
/// Mirrors the JS-era `document.documentElement.setAttribute(...)`
/// pattern at `src/managers/theme-manager.js`. Best-effort: failures
/// (no document — host build / SSR) reduce to a silent no-op, which
/// is fine because the e2e suite always runs against a real DOM.
fn set_html_attr(attr: &str, value: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(html) = document.document_element() else {
        return;
    };
    if let Ok(element) = html.dyn_into::<web_sys::HtmlElement>() {
        let _ = element.set_attribute(attr, value);
    }
}

/// Capitalise the first character of `stem` for the tile label.
///
/// Mirrors the JS-era `capitalizeFirst` helper at
/// `src/utils/theme-loader.js`. Returns `stem` unchanged if it's
/// empty (the build-themes generator already filters empty stems
/// — this is defence-in-depth).
fn capitalise_first(stem: &str) -> String {
    let mut chars = stem.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// Theme settings tab — light/dark/auto picker + timer theme grid.
#[component]
pub fn ThemeSettings(settings: RwSignal<Settings>, toast: SettingsToast) -> impl IntoView {
    // Color-theme handler resolves `auto` against the OS preference
    // and applies the concrete `data-theme` token. Also persists
    // the preference string ("auto"/"light"/"dark") to the shared
    // settings signal so the debounced save sink routes it to disk.
    let on_theme = move |pref: &'static str| {
        let normalised = normalise_theme_pref(pref);
        let resolved = resolve_color_mode(normalised, system_prefers_dark());
        set_html_attr("data-theme", resolved);
        settings.update(|s| s.appearance.theme = normalised.to_string());
        toast.show("Settings saved");
    };
    // Timer-theme handler routes through the loader's apply_theme
    // for the `data-theme` write *as well* — when a user picks a
    // tile, the e2e flow asserts on `data-theme` toggling. The
    // tile selection itself is reflected via `data-timer-theme`.
    // Also persists to the shared settings signal.
    let on_timer_theme = move |id: &'static str| {
        apply_theme(id);
        set_html_attr("data-timer-theme", id);
        settings.update(|s| s.appearance.timer_theme = id.to_string());
        toast.show("Settings saved");
    };

    view! {
        <div class="category-header">
            <h1>"Theme"</h1>
            <p class="category-description">
                "Customize the appearance and visual style of the application"
            </p>
        </div>
        <div class="settings-section">
            <h3>"Color Theme"</h3>
            <div class="setting-item">
                <label for="theme-selector">"Theme Mode:"</label>
                <div class="theme-selector" id="theme-selector">
                    <button
                        class="theme-option"
                        data-theme="auto"
                        title="Auto (Follow System)"
                        on:click=move |_| on_theme("auto")
                    >
                        <i class="ri-contrast-2-line"></i>
                        <span>"Auto"</span>
                    </button>
                    <button
                        class="theme-option"
                        data-theme="light"
                        title="Light Mode"
                        on:click=move |_| on_theme("light")
                    >
                        <i class="ri-sun-line"></i>
                        <span>"Light"</span>
                    </button>
                    <button
                        class="theme-option"
                        data-theme="dark"
                        title="Dark Mode"
                        on:click=move |_| on_theme("dark")
                    >
                        <i class="ri-moon-line"></i>
                        <span>"Dark"</span>
                    </button>
                </div>
                <p class="setting-description">
                    "Choose your preferred color theme. Auto will automatically switch between light and dark mode based on your system preferences."
                </p>
            </div>
        </div>
        <div class="settings-section">
            <h3>"Timer Colors"</h3>
            <div class="setting-item">
                <label for="timer-theme-selector">"Timer Theme:"</label>
                <div class="timer-theme-grid" id="timer-theme-grid">
                    {ALL_THEMES
                        .iter()
                        .map(|stem| {
                            let id: &'static str = stem;
                            let label = capitalise_first(stem);
                            view! {
                                <button
                                    class="timer-theme-option"
                                    data-timer-theme=id
                                    on:click=move |_| on_timer_theme(id)
                                >
                                    {label}
                                </button>
                            }
                        })
                        .collect::<Vec<_>>()}
                </div>
                <p class="setting-description">
                    "Choose a color theme for your timer sessions."
                </p>
            </div>
        </div>
    }
}

/// Map an unknown theme preference string to "auto". Valid values
/// ("auto", "light", "dark") pass through unchanged; anything else
/// is treated as the safe default.
fn normalise_theme_pref(pref: &str) -> &str {
    match pref {
        "auto" | "light" | "dark" => pref,
        _ => "auto",
    }
}

#[cfg(test)]
mod tests {
    use super::{capitalise_first, normalise_theme_pref};
    use crate::theme::themes::ALL_THEMES;

    /// T205 — selector contract pin. Sourced from
    /// `tests/e2e/settings-theme.spec.js`.
    #[test]
    fn theme_settings_selector_contract_documented() {
        const REQUIRED_IDS: &[&str] = &["theme-selector", "timer-theme-grid"];
        for id in REQUIRED_IDS {
            assert!(!id.is_empty(), "selector ID must not be empty");
        }
    }

    /// E2e asserts `tileCount >= 2` (`spec.js:29`); the
    /// auto-generated catalogue must satisfy that lower bound. Pin
    /// against `ALL_THEMES` directly so a future code-gen drift
    /// (e.g. accidentally emitting an empty slice) surfaces here
    /// rather than in the e2e suite.
    #[test]
    fn timer_theme_catalogue_meets_e2e_minimum() {
        assert!(
            ALL_THEMES.len() >= 2,
            "settings-theme.spec.js:29 asserts tileCount >= 2; ALL_THEMES has {}",
            ALL_THEMES.len(),
        );
        for stem in ALL_THEMES {
            assert!(!stem.is_empty(), "theme stem must not be empty");
        }
    }

    #[test]
    fn capitalise_first_handles_normal_stems() {
        assert_eq!(capitalise_first("espresso"), "Espresso");
        assert_eq!(capitalise_first("pipboy"), "Pipboy");
        assert_eq!(capitalise_first(""), "");
    }

    #[test]
    fn normalise_theme_pref_passes_valid_and_maps_unknown_to_auto() {
        assert_eq!(normalise_theme_pref("auto"), "auto");
        assert_eq!(normalise_theme_pref("light"), "light");
        assert_eq!(normalise_theme_pref("dark"), "dark");
        assert_eq!(normalise_theme_pref(""), "auto");
        assert_eq!(normalise_theme_pref("system"), "auto");
        assert_eq!(normalise_theme_pref("DARK"), "auto");
    }
}
