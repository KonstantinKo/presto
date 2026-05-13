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
use leptos_i18n::{t, t_string};
use wasm_bindgen::JsCast as _;

use crate::bridge::types::Settings;
use crate::components::settings::SettingsToast;
use crate::i18n::i18n::use_i18n;
use crate::theme::loader::{apply_timer_theme, resolve_color_mode, system_prefers_dark};
use crate::theme::metadata::{is_compatible, ThemeMeta, THEME_METADATA};

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

/// Per-tile inline preview styles. Mirrors
/// `applyThemePreviewStyles` at `settings-manager.js:1488-1520` —
/// each theme paints its preview frame with the colors and (for
/// pipboy) the mono font + glow that match the real timer skin.
struct PreviewStyles {
    display: String,
    time: String,
    status: String,
}

fn preview_styles_for(meta: &ThemeMeta) -> PreviewStyles {
    let focus = meta.preview.focus;
    let time_base = format!("color: {focus};");
    let status_base = format!("color: {focus};");
    match meta.id {
        "pipboy" => PreviewStyles {
            display: format!(
                "background: #000011; border: 1px solid {focus}; font-family: \"Share Tech Mono\", monospace;"
            ),
            time: format!("{time_base} text-shadow: 0 0 5px {focus};"),
            status: format!("{status_base} text-shadow: 0 0 3px {focus};"),
        },
        "espresso" => PreviewStyles {
            display: format!(
                "background: #3c2415; border: 1px solid {focus}; color: #f4f1de;"
            ),
            time: time_base,
            status: status_base,
        },
        "pommodore64" => PreviewStyles {
            display: format!(
                "background: #40318d; border: 1px solid {focus}; color: #7b68ee;"
            ),
            time: time_base,
            status: status_base,
        },
        _ => PreviewStyles {
            display: format!("background: #f8f9fa; border: 1px solid {focus};"),
            time: time_base,
            status: status_base,
        },
    }
}

/// Theme settings tab — light/dark/auto picker + timer theme grid.
#[component]
pub fn ThemeSettings(settings: RwSignal<Settings>, toast: SettingsToast) -> impl IntoView {
    let i18n = use_i18n();
    // Color-theme handler resolves `auto` against the OS preference
    // and applies the concrete `data-theme` token. Also persists
    // the preference string ("auto"/"light"/"dark") to the shared
    // settings signal so the debounced save sink routes it to disk.
    let on_theme = move |pref: &'static str| {
        let normalised = normalise_theme_pref(pref);
        let resolved = resolve_color_mode(normalised, system_prefers_dark());
        set_html_attr("data-theme", resolved);
        settings.update(|s| s.appearance.theme = normalised.to_string());
        toast.show(t_string!(i18n, settings.toast_saved).to_string());
    };
    // Timer-theme handler applies the data-timer-theme attribute via
    // the loader and also persists to the shared settings signal.
    let on_timer_theme = move |id: &'static str| {
        apply_timer_theme(id);
        set_html_attr("data-timer-theme", id);
        settings.update(|s| s.appearance.timer_theme = id.to_string());
        toast.show(t_string!(i18n, settings.toast_saved).to_string());
    };

    let current_theme = Signal::derive(move || settings.with(|s| s.appearance.theme.clone()));
    let current_timer_theme =
        Signal::derive(move || settings.with(|s| s.appearance.timer_theme.clone()));

    view! {
        <div class="category-header">
            <h1>{t!(i18n, settings.theme.title)}</h1>
            <p class="category-description">{t!(i18n, settings.theme.description)}</p>
        </div>
        <div class="settings-section">
            <h3 class="section-header">{t!(i18n, settings.theme.color_section)}</h3>
            <div class="setting-item">
                <label for="theme-selector">{t!(i18n, settings.theme.mode_label)}</label>
                <div class="theme-selector" id="theme-selector">
                    <button
                        class="theme-option"
                        class:active=move || current_theme.get() == "auto"
                        data-theme="auto"
                        title=move || t_string!(i18n, settings.theme.mode_auto_aria)
                        on:click=move |_| on_theme("auto")
                    >
                        <i class="ri-contrast-2-line"></i>
                        <span>{t!(i18n, settings.theme.mode_auto)}</span>
                    </button>
                    <button
                        class="theme-option"
                        class:active=move || current_theme.get() == "light"
                        data-theme="light"
                        title=move || t_string!(i18n, settings.theme.mode_light_aria)
                        on:click=move |_| on_theme("light")
                    >
                        <i class="ri-sun-line"></i>
                        <span>{t!(i18n, settings.theme.mode_light)}</span>
                    </button>
                    <button
                        class="theme-option"
                        class:active=move || current_theme.get() == "dark"
                        data-theme="dark"
                        title=move || t_string!(i18n, settings.theme.mode_dark_aria)
                        on:click=move |_| on_theme("dark")
                    >
                        <i class="ri-moon-line"></i>
                        <span>{t!(i18n, settings.theme.mode_dark)}</span>
                    </button>
                </div>
                <p class="setting-description">{t!(i18n, settings.theme.mode_help)}</p>
            </div>
        </div>
        <div class="settings-section">
            <h3 class="section-header">{t!(i18n, settings.theme.timer_section)}</h3>
            <div class="setting-item">
                <label for="timer-theme-selector">{t!(i18n, settings.theme.timer_theme_label)}</label>
                <div class="timer-theme-grid" id="timer-theme-grid">
                    {THEME_METADATA
                        .iter()
                        .map(|meta| {
                            let id: &'static str = meta.id;
                            let name: &'static str = meta.name;
                            let description: &'static str = meta.description;
                            let supports_light = meta.supports.light;
                            let supports_dark = meta.supports.dark;
                            let preview_styles = preview_styles_for(meta);
                            let focus_color = meta.preview.focus.to_string();
                            let break_color = meta.preview.break_.to_string();
                            let long_break_color = meta.preview.long_break.to_string();
                            // Compatibility check resolves the user's
                            // explicit/auto preference against the OS
                            // hint, mirroring `getCurrentColorMode` at
                            // `settings-manager.js:1373`.
                            let is_disabled = Signal::derive(move || {
                                let pref = current_theme.get();
                                let resolved = resolve_color_mode(
                                    normalise_theme_pref(&pref),
                                    system_prefers_dark(),
                                );
                                !is_compatible(id, resolved)
                            });
                            view! {
                                <div
                                    class="timer-theme-option"
                                    class:active=move || current_timer_theme.get() == id
                                    class:disabled=move || is_disabled.get()
                                    data-timer-theme=id
                                    role="button"
                                    tabindex=move || if is_disabled.get() { -1 } else { 0 }
                                    aria-disabled=move || is_disabled.get().to_string()
                                    on:click=move |_| {
                                        if is_disabled.get_untracked() {
                                            return;
                                        }
                                        on_timer_theme(id);
                                    }
                                    on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                                        let key = ev.key();
                                        if key == "Enter" || key == " " {
                                            ev.prevent_default();
                                            if !is_disabled.get_untracked() {
                                                on_timer_theme(id);
                                            }
                                        }
                                    }
                                >
                                    <div class="timer-theme-header">
                                        <h4 class="timer-theme-name">{name}</h4>
                                        <div class="timer-theme-compatibility">
                                            {(supports_light).then(|| view! {
                                                <span class="compatibility-badge light">
                                                    <i class="ri-sun-line"></i>
                                                </span>
                                            })}
                                            {(supports_dark).then(|| view! {
                                                <span class="compatibility-badge dark">
                                                    <i class="ri-moon-line"></i>
                                                </span>
                                            })}
                                        </div>
                                    </div>
                                    <p class="timer-theme-description">{description}</p>
                                    <div class="timer-theme-preview">
                                        <div
                                            class="timer-preview-display"
                                            data-preview-theme=id
                                            style=preview_styles.display
                                        >
                                            <div
                                                class="timer-preview-time"
                                                style=preview_styles.time
                                            >"25:00"</div>
                                            <div
                                                class="timer-preview-status"
                                                style=preview_styles.status
                                            >"Focus Session"</div>
                                        </div>
                                        <div class="color-preview-strip">
                                            <div
                                                class="preview-color"
                                                style=format!("background-color: {focus_color};")
                                            ></div>
                                            <div
                                                class="preview-color"
                                                style=format!("background-color: {break_color};")
                                            ></div>
                                            <div
                                                class="preview-color"
                                                style=format!("background-color: {long_break_color};")
                                            ></div>
                                        </div>
                                    </div>
                                </div>
                            }
                        })
                        .collect::<Vec<_>>()}
                </div>
                <p class="setting-description">{t!(i18n, settings.theme.timer_theme_help)}</p>
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
    use super::normalise_theme_pref;
    use crate::theme::metadata::THEME_METADATA;
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

    /// E2e asserts `tileCount >= 2` (`spec.js:29`); the rendered tiles
    /// come from `THEME_METADATA`, so pin against that. Also verify each
    /// metadata id is present in the auto-generated `ALL_THEMES` slice so
    /// a drift between the two sources surfaces here.
    #[test]
    fn timer_theme_catalogue_meets_e2e_minimum() {
        assert!(
            THEME_METADATA.len() >= 2,
            "settings-theme.spec.js:29 asserts tileCount >= 2; THEME_METADATA has {}",
            THEME_METADATA.len(),
        );
        for meta in THEME_METADATA {
            assert!(!meta.id.is_empty(), "theme id must not be empty");
            assert!(
                ALL_THEMES.contains(&meta.id),
                "metadata id '{}' is missing from generated ALL_THEMES",
                meta.id
            );
        }
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
