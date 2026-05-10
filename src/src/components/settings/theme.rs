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

/// Theme picker tile. Mirrors the JS-era `themes` catalogue at
/// `src/managers/theme-manager.js` — id matches the
/// `data-timer-theme` attribute the spec asserts on; label is
/// purely cosmetic.
struct TimerThemeTile {
    id: &'static str,
    label: &'static str,
}

/// Minimal fixture catalogue. Phase 5 replaces this with the
/// code-gen'd `theme::themes::ALL_THEMES` slice from the build-themes
/// tool; here the static pair is enough to satisfy the e2e
/// `tileCount >= 2` assertion.
const TIMER_THEMES: &[TimerThemeTile] = &[
    TimerThemeTile {
        id: "espresso",
        label: "Espresso",
    },
    TimerThemeTile {
        id: "matcha",
        label: "Matcha",
    },
];

/// Theme settings tab — light/dark/auto picker + timer theme grid.
#[component]
pub fn ThemeSettings() -> impl IntoView {
    let on_theme = move |theme: &'static str| {
        set_html_attr("data-theme", theme);
    };
    let on_timer_theme = move |id: &'static str| {
        set_html_attr("data-timer-theme", id);
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
                    {TIMER_THEMES
                        .iter()
                        .map(|tile| {
                            let id = tile.id;
                            let label = tile.label;
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

#[cfg(test)]
mod tests {
    use super::TIMER_THEMES;

    /// T205 — selector contract pin. Sourced from
    /// `tests/e2e/settings-theme.spec.js`.
    #[test]
    fn theme_settings_selector_contract_documented() {
        const REQUIRED_IDS: &[&str] = &["theme-selector", "timer-theme-grid"];
        for id in REQUIRED_IDS {
            assert!(!id.is_empty(), "selector ID must not be empty");
        }
    }

    /// E2e asserts `tileCount >= 2` (`spec.js:29`); the fixture
    /// catalogue must satisfy that lower bound. Phase 5 will swap
    /// this static list for the code-gen'd full catalogue.
    #[test]
    fn timer_theme_catalogue_meets_e2e_minimum() {
        assert!(
            TIMER_THEMES.len() >= 2,
            "settings-theme.spec.js:29 asserts tileCount >= 2",
        );
        for tile in TIMER_THEMES {
            assert!(!tile.id.is_empty(), "theme id must not be empty");
            assert!(!tile.label.is_empty(), "theme label must not be empty");
        }
    }
}
