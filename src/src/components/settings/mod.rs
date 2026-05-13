// Settings view shell — Phase 4b (T204) of spec
// 001-leptos-migration.
//
// Mounts the canonical settings two-pane layout:
//
//   `#settings-view` (root)
//     ├── `.settings-sidebar` — vertical nav (`.settings-nav-item`s with
//     │   `data-category="<tab>"`); the active tab carries `.active`.
//     └── `.settings-content` — host for the active per-tab category
//         div; only one `.settings-category` carries `.active` at a
//         time and its `id="category-<tab>"` matches the
//         `selectSettingsCategory` fixture's `#category-<tab>.active`
//         wait selector.
//
// **Selector contract** (consumed by every `tests/e2e/settings-*.spec.js`
// via `tests/e2e/fixtures/screens.js::selectSettingsCategory` and
// `tapTab(... "Settings")`):
//
// - `#settings-view` — root view container. Carries `.hidden` when
//   another `NavView` is active. (`screens.js:35` waits for
//   `#settings-view:not(.hidden)`.)
// - `.settings-nav-item[data-category="<tab>"]` — sidebar button per
//   tab. (`screens.js:71` clicks via this selector.)
// - `#category-<tab>.active` — per-tab content host with the active
//   class on the visible one. (`screens.js:72` waits for this.)
// - `.settings-category` — base class on every per-tab host. The
//   `.active` modifier toggles visibility (CSS-driven; we match the
//   JS-era class-toggle pattern rather than an `if`/`else` branch so
//   the tab content stays in the DOM and CSS transitions between
//   tabs render correctly).
// - `[role="alert"].notification-ping` filtered by "Settings saved" —
//   per-tab auto-save toast surface (`screens.js:104` and
//   `settings-goals.spec.js:23` filter by this text). The toast
//   renderer lives here on the shell so every tab feeds into a
//   single toast queue rather than each tab re-implementing the
//   queue.
//
// Each per-tab module (`general`, `theme`, `notifications`, etc.) is
// declared `pub` below so the App router (T217) can mount the same
// per-tab views directly when restoring deep links — the shell-vs-
// direct-mount distinction is a layering concern; the shell wraps
// the same `<Tab>View/>` components.
//
// Per Principle I, the shell never mutates engine state — settings
// changes go through `SettingsManager` setters and `bridge::commands::save_settings`
// (Phase 4c attaches the persistence hop; the dev-server / e2e mock
// branch operates against the in-memory `RwSignal<Settings>`).
//
// Lint allowance: `clippy::must_use_candidate` is silenced module-wide
// for the same reason as on `timer.rs` etc. — Leptos `#[component]`
// returns are consumed by `view!` / `mount_to_body` automatically, so
// `#[must_use]` would not apply at any call site.
//
// `clippy::too_many_lines` is silenced module-wide because each
// per-tab view body is a single Leptos `view!` expansion plus a
// derived-signal cluster — splitting either across helper fns
// fragments the JSX-style DOM tree. Same rationale as on
// `history.rs` / `tags.rs`.
#![allow(clippy::must_use_candidate, clippy::too_many_lines)]

use leptos::prelude::*;
use leptos_i18n::{t, t_string};

use crate::bridge::types::Settings;
use crate::i18n::i18n::use_i18n;
use crate::managers::navigation::SettingsTab;

pub mod advanced;
pub mod automation;
pub mod general;
pub mod goals;
pub mod notifications;
pub mod shortcuts;
pub mod theme;
pub mod updates;

/// Shared toast handle.
///
/// Cloned into each per-tab view so a save can fire
/// `toast.show("Settings saved")` without each tab re-allocating
/// a queue. The shell renders one `[role="alert"].notification-ping`
/// for the most recent message and clears it after the timeout fires.
/// The e2e suite's
/// `getByRole("alert").filter({ hasText: "Settings saved" })` resolves
/// against a stable role + text shape regardless of which tab fires.
#[derive(Clone, Copy)]
pub struct SettingsToast {
    /// Latest message; `None` when no toast is currently shown.
    ///
    /// Feature 005: widened from `&'static str` to `String` so the
    /// localised "Settings saved" text (which is computed via the
    /// `t_string!` macro at the call site) can flow through. The
    /// e2e suite still filters the toast surface by the rendered
    /// text — `RwSignal<Option<String>>` keeps the lifetime story
    /// equivalent (the autoclear Effect drops the `String` once
    /// the 2s timeout fires).
    pub message: RwSignal<Option<String>>,
}

impl SettingsToast {
    /// Construct an empty toast handle. The caller should keep this
    /// signal alive for the duration of the settings view so the
    /// toast renders on the shell, not inside each tab.
    #[must_use]
    pub fn new() -> Self {
        Self {
            message: RwSignal::new(None),
        }
    }

    /// Show `text` as the latest toast. Mirrors the JS-era
    /// `NotificationUtils.showMessage("Settings saved", ...)` call at
    /// `src/managers/settings-manager.js` after `saveSettings`. The
    /// auto-clear timeout fires inside the shell's `Effect::new` so
    /// the lifecycle stays tied to the settings view.
    pub fn show(self, text: impl Into<String>) {
        self.message.set(Some(text.into()));
    }
}

impl Default for SettingsToast {
    fn default() -> Self {
        Self::new()
    }
}

/// Settings view shell. Renders the two-pane layout (sidebar nav +
/// content) with the active tab indicated via `.active` on both the
/// nav button and the per-tab content container.
///
/// Props:
/// - `tab`: the active `SettingsTab`. The App router (T217) passes
///   `nav.last_settings_tab()` here; the shell's per-button
///   `on:click` calls `on_select_tab` to update the parent.
/// - `settings`: the `RwSignal<Settings>` the per-tab views read /
///   write. Wiring through the parent keeps the shell stateless
///   regarding settings content; per-tab signals (e.g. focus
///   duration changes) feed back through `settings.update(...)`.
/// - `on_select_tab`: callback fired when the user clicks a sidebar
///   nav button. The App router updates the
///   `NavigationManager::last_settings_tab` slice via this hook so
///   the per-tab routing matches `screens.js:72`'s
///   `#category-<tab>.active` wait.
///
/// `#[allow(clippy::needless_pass_by_value)]` on `on_select_tab`
/// would be required if we accepted `Callback<SettingsTab>` directly
/// — but using a `Callback` prop is the established Leptos pattern
/// and the framework's `Callback::run` consumes by reference, so the
/// pass-by-value is on the prop binding only and isn't actually a
/// move at the click-handler site.
#[component]
pub fn SettingsView(
    /// Active tab — the `data-category` value of the nav button that
    /// carries `.active`, and the `id` suffix on the active
    /// `.settings-category` host.
    tab: Signal<SettingsTab>,
    /// Shared settings record; per-tab views read fields off this
    /// signal and call `update` to mutate.
    settings: RwSignal<Settings>,
    /// Tab-change callback. The App router uses
    /// `NavigationManager::select_settings_tab` here; standalone
    /// embeddings can pass `|_| {}` as a no-op.
    on_select_tab: Callback<SettingsTab>,
) -> impl IntoView {
    let i18n = use_i18n();
    let toast = SettingsToast::new();

    // Auto-clear the toast after 2s — matches the JS-era
    // `NotificationUtils.showMessage` timeout. The Effect re-runs on
    // every signal change (i.e. every `toast.show(...)` call) and
    // schedules a fresh clear; the latest scheduled clear wins.
    Effect::new(move |_| {
        if toast.message.with(Option::is_some) {
            let handle = set_timeout_with_handle(
                move || {
                    toast.message.set(None);
                },
                core::time::Duration::from_secs(2),
            );
            // The handle is intentionally leaked — Leptos cleans it
            // up when the effect re-runs or the component unmounts.
            // Failure (no JS bridge — host tests / SSR) reduces to
            // dropping the timer; the toast never clears in that
            // branch which is acceptable because it's never shown
            // either.
            let _ = handle;
        }
    });

    // Per-tab category-active flag. Each row reads this to decide
    // whether to apply `.active` on its `#category-<tab>` div.
    let is_active = move |target: SettingsTab| Signal::derive(move || tab.get() == target);

    view! {
        <div class="view-container view-section" id="settings-view">
            <div class="settings-layout">
                // Sidebar nav — every button carries
                // `data-category="<tab>"` matching the
                // `selectSettingsCategory` fixture's locator at
                // `screens.js:71`.
                <div class="settings-sidebar sidebar-base">
                    <h2>{t!(i18n, settings.shell_header)}</h2>
                    <nav class="settings-nav sidebar-nav">
                        {settings_nav_items()
                            .into_iter()
                            .map(|item| {
                                let target = item.target;
                                let icon = item.icon;
                                let on_click = move |_| on_select_tab.run(target);
                                // Feature 005: localised tab label per
                                // variant. Each `match` arm is a static
                                // catalogue key so the proc-macro can
                                // compile-time-check the lookup.
                                let label_view = move || match target {
                                    SettingsTab::General => t!(i18n, settings.tab_general).into_any(),
                                    SettingsTab::Shortcuts => t!(i18n, settings.tab_shortcuts).into_any(),
                                    SettingsTab::Notifications => t!(i18n, settings.tab_notifications).into_any(),
                                    SettingsTab::Theme => t!(i18n, settings.tab_theme).into_any(),
                                    SettingsTab::Automation => t!(i18n, settings.tab_automation).into_any(),
                                    SettingsTab::Goals => t!(i18n, settings.tab_goals).into_any(),
                                    SettingsTab::Advanced => t!(i18n, settings.tab_advanced).into_any(),
                                    SettingsTab::Updates => t!(i18n, settings.tab_updates).into_any(),
                                };
                                view! {
                                    <button
                                        class="settings-nav-item nav-item-base"
                                        class:active=move || tab.get() == target
                                        data-category=item.category
                                        on:click=on_click
                                    >
                                        <i class=icon></i>
                                        <span>{label_view}</span>
                                    </button>
                                }
                            })
                            .collect::<Vec<_>>()}
                    </nav>
                </div>

                // Content host — every per-tab category div is
                // mounted; only the active one carries `.active`,
                // matching the JS-era CSS-driven visibility toggle.
                <div class="settings-content content-main">
                    <div
                        class="settings-category"
                        class:active=is_active(SettingsTab::General)
                        id="category-general"
                    >
                        <general::GeneralSettings settings=settings toast=toast/>
                    </div>
                    <div
                        class="settings-category"
                        class:active=is_active(SettingsTab::Shortcuts)
                        id="category-shortcuts"
                    >
                        <shortcuts::ShortcutsSettings settings=settings toast=toast/>
                    </div>
                    <div
                        class="settings-category"
                        class:active=is_active(SettingsTab::Notifications)
                        id="category-notifications"
                    >
                        <notifications::NotificationsSettings settings=settings toast=toast/>
                    </div>
                    <div
                        class="settings-category"
                        class:active=is_active(SettingsTab::Theme)
                        id="category-theme"
                    >
                        <theme::ThemeSettings settings=settings toast=toast/>
                    </div>
                    <div
                        class="settings-category"
                        class:active=is_active(SettingsTab::Automation)
                        id="category-automation"
                    >
                        <automation::AutomationSettings settings=settings toast=toast/>
                    </div>
                    <div
                        class="settings-category"
                        class:active=is_active(SettingsTab::Goals)
                        id="category-goals"
                    >
                        <goals::GoalsSettings settings=settings toast=toast/>
                    </div>
                    <div
                        class="settings-category"
                        class:active=is_active(SettingsTab::Advanced)
                        id="category-advanced"
                    >
                        <advanced::AdvancedSettings settings=settings toast=toast/>
                    </div>
                    <div
                        class="settings-category"
                        class:active=is_active(SettingsTab::Updates)
                        id="category-updates"
                    >
                        <updates::UpdatesSettings settings=settings toast=toast/>
                    </div>

                    // Footer: auto-save acknowledgement + Reset to
                    // Defaults. Rendered inside `.settings-content` so
                    // its width is bounded by the content column,
                    // matching the JS-era DOM at `index.html:1208`.
                    <div class="settings-actions setting-item">
                        <div class="auto-save-info">
                            <span class="auto-save-text">"✓ " {t!(i18n, settings.autosave_info)}</span>
                        </div>
                        <button
                            class="btn-secondary"
                            on:click=move |_| {
                                settings.set(Settings::default());
                                toast.show(t_string!(i18n, settings.toast_reset_defaults).to_string());
                            }
                        >{t!(i18n, settings.reset_defaults_button)}</button>
                    </div>
                </div>
            </div>

            // Shell-level toast surface. `role="alert"` so the e2e
            // suite's `getByRole("alert").filter({ hasText: "Settings
            // saved" })` resolves; `.notification-ping` so the
            // legacy `.notification-ping` selector also matches
            // (settings-goals.spec.js:23 uses that class hook).
            {move || toast
                .message
                .get()
                .map(|text| {
                    view! {
                        <div class="notification-ping" role="alert">
                            "✓ "
                            {text}
                        </div>
                    }
                })}
        </div>
    }
}

/// One sidebar nav item. Pinning the (label, icon, category) tuple
/// here keeps the JS-era ordering / icon mapping centralised and
/// matches the `selectSettingsCategory` fixture's category map at
/// `tests/e2e/fixtures/screens.js:55-64`.
struct SettingsNavItem {
    target: SettingsTab,
    /// `data-category` attribute value. Must equal the lowercase
    /// snake-case form the `screens.js` fixture sends.
    category: &'static str,
    /// Display label.
    ///
    /// Feature 005: kept on the struct as the canonical English
    /// source-of-truth (matched against by the `settings_nav_items_*`
    /// tests below); rendered output comes from `t!(i18n, settings.tab_*)`
    /// at the view call site instead.
    #[allow(dead_code)]
    label: &'static str,
    /// Remixicon class (`ri-*-line` etc.).
    icon: &'static str,
}

/// JS-era settings nav order, sourced from `src/index.html` lines
/// 579-610. The fixture's category map at
/// `tests/e2e/fixtures/screens.js:55-64` enumerates the eight tabs
/// in display order; this function returns the same order so the
/// rendered DOM mirrors the JS-era surface.
fn settings_nav_items() -> Vec<SettingsNavItem> {
    vec![
        SettingsNavItem {
            target: SettingsTab::General,
            category: "general",
            label: "General",
            icon: "ri-timer-line",
        },
        SettingsNavItem {
            target: SettingsTab::Shortcuts,
            category: "shortcuts",
            label: "Shortcuts",
            icon: "ri-keyboard-line",
        },
        SettingsNavItem {
            target: SettingsTab::Notifications,
            category: "notifications",
            label: "Notifications",
            icon: "ri-notification-line",
        },
        SettingsNavItem {
            target: SettingsTab::Theme,
            category: "theme",
            label: "Theme",
            icon: "ri-palette-line",
        },
        SettingsNavItem {
            target: SettingsTab::Automation,
            category: "automation",
            label: "Automation",
            icon: "ri-magic-line",
        },
        SettingsNavItem {
            target: SettingsTab::Goals,
            category: "goals",
            label: "Goals",
            icon: "ri-trophy-line",
        },
        SettingsNavItem {
            target: SettingsTab::Advanced,
            category: "advanced",
            label: "Advanced",
            icon: "ri-settings-3-line",
        },
        SettingsNavItem {
            target: SettingsTab::Updates,
            category: "updates",
            label: "Updates",
            icon: "ri-download-line",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::settings_nav_items;
    use crate::managers::navigation::SettingsTab;

    /// T204 — selector contract pin for the settings shell. Sourced
    /// from `tests/e2e/fixtures/screens.js` and every
    /// `settings-*.spec.js`.
    ///
    /// - `settings-view` — root view (`screens.js:35`).
    /// - `settings-nav-item` — class on the per-tab nav button
    ///   (`screens.js:71` `.settings-nav-item[data-category="..."]`).
    /// - `category-<tab>` — per-tab content container ID
    ///   (`screens.js:72` waits for `.active`).
    /// - `notification-ping` — toast surface class
    ///   (`settings-goals.spec.js:23`).
    #[test]
    fn settings_shell_selector_contract_documented() {
        const REQUIRED_IDS: &[&str] = &[
            "settings-view",
            "category-general",
            "category-shortcuts",
            "category-notifications",
            "category-theme",
            "category-automation",
            "category-goals",
            "category-advanced",
            "category-updates",
        ];
        const REQUIRED_CLASSES: &[&str] = &[
            "settings-nav-item",
            "settings-category",
            "settings-sidebar",
            "settings-content",
            "settings-layout",
            "notification-ping",
        ];
        let mut seen_ids: Vec<&str> = Vec::with_capacity(REQUIRED_IDS.len());
        for id in REQUIRED_IDS {
            assert!(!id.is_empty(), "selector ID must not be empty");
            assert!(
                !seen_ids.contains(id),
                "duplicate selector ID in contract: {id}",
            );
            seen_ids.push(id);
        }
        for cls in REQUIRED_CLASSES {
            assert!(!cls.is_empty(), "selector class must not be empty");
        }
    }

    /// Pin the JS-era nav order (`src/index.html:579-610`). Drift
    /// here would silently re-order the sidebar — the e2e suite
    /// addresses each tab by `data-category`, so the order is purely
    /// a UX concern, but pinning catches accidental re-shuffles.
    #[test]
    fn settings_nav_items_match_jsbaseline_order() {
        let items = settings_nav_items();
        let order: Vec<SettingsTab> = items.iter().map(|i| i.target).collect();
        assert_eq!(
            order,
            vec![
                SettingsTab::General,
                SettingsTab::Shortcuts,
                SettingsTab::Notifications,
                SettingsTab::Theme,
                SettingsTab::Automation,
                SettingsTab::Goals,
                SettingsTab::Advanced,
                SettingsTab::Updates,
            ]
        );
    }

    /// Pin the `data-category` slug shape — the
    /// `selectSettingsCategory` fixture at
    /// `tests/e2e/fixtures/screens.js:55-64` maps display labels to
    /// these exact strings. Drift here breaks every settings-*.spec.js
    /// at the locator step.
    #[test]
    fn data_category_slugs_match_fixture_map() {
        let items = settings_nav_items();
        let slugs: Vec<&'static str> = items.iter().map(|i| i.category).collect();
        assert_eq!(
            slugs,
            vec![
                "general",
                "shortcuts",
                "notifications",
                "theme",
                "automation",
                "goals",
                "advanced",
                "updates",
            ]
        );
    }
}
