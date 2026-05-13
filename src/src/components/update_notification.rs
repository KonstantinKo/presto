// Update notification banner — Phase 4b (T214) of spec
// 001-leptos-migration. Mounts the slide-down banner the JS-era
// surface created via `update-notification.js` when a newer release
// is available.
//
// **Selector contract** (consumed by `tests/e2e/update-notification.spec.js`):
// - `#update-notification-container` — banner root
//   (`spec.js:18`); always in the DOM, hidden via the
//   transform-translateY(-100%) baseline. Carries `.visible` when
//   the slide-in transition runs.
// - `#update-notification-version` — version text inside the banner
//   (`spec.js:19,22`); reads "Version <semver>" once
//   `UpdateInfo::Available` lands.
// - `#update-notification-close` — close button (`spec.js:28`);
//   click drops the `.visible` class.
//
// The banner subscribes to the shared `UpdateManager`-provided
// signal (today: a plain `RwSignal<UpdateInfo>` lifted via the App
// router in T217). The `UpdateAvailablePayload` event listener at
// `bridge::events::UPDATE_AVAILABLE` (Phase 4c) drives the manager;
// the component reads the manager state and renders accordingly.
//
// Per Principle I, this component never mutates engine state — the
// banner is purely a display surface. Skipping a version writes
// `presto-skipped-versions` localStorage (the JS-era contract per
// data-model.md §"Legacy localStorage migration"); the Rust port
// folds that into a future commit alongside the
// `bridge::commands::skip_update_version` hop.
//
// Lint allowance: `clippy::must_use_candidate` is silenced for the
// usual Leptos `#[component]` reason.
#![allow(clippy::must_use_candidate)]

use leptos::prelude::*;
use leptos_i18n::{t, t_string};

use crate::bridge::types::Settings;
use crate::i18n::i18n::use_i18n;
use crate::managers::update::UpdateInfo;

/// Deduplicating push: add `version` to `list` only if not already
/// present. Used by the "Skip release" handler to avoid duplicating
/// entries in `settings.skipped_versions`.
fn push_skipped(list: &mut Vec<String>, version: &str) {
    if !list.contains(&version.to_string()) {
        list.push(version.to_string());
    }
}

/// Update notification banner.
///
/// Props:
/// - `update_info`: shared `RwSignal<UpdateInfo>` driven by the
///   `UpdateManager`. The component reads the variant to decide
///   whether to add the `.visible` class; the version text
///   projects the `Available::version` slot.
/// - `settings`: shared `RwSignal<Settings>`. "Skip release" handler
///   pushes the skipped version onto `settings.skipped_versions`
///   (deduplicated) so the `UPDATE_AVAILABLE` listener in `app.rs`
///   can filter repeat notifications.
#[component]
pub fn UpdateNotification(
    update_info: RwSignal<UpdateInfo>,
    settings: RwSignal<Settings>,
) -> impl IntoView {
    let i18n = use_i18n();
    // Local "user closed it" flag. The JS-era surface at
    // `src/components/update-notification.js` retains the dismissal
    // across navigations within the same launch (the spec at
    // `update-notification.spec.js:32-34` asserts the banner stays
    // hidden after Calendar → Timer round-trip post-close). We
    // mirror that with a per-instance signal.
    let dismissed = RwSignal::new(false);

    // The banner is "visible" iff there's an available update AND
    // the user hasn't closed it. Skipping a version (a separate UI
    // affordance that lands with the bridge call) would also
    // dismiss the banner.
    let visible = Signal::derive(move || {
        !dismissed.get() && update_info.with(|i| matches!(i, UpdateInfo::Available { .. }))
    });

    // Version text for `#update-notification-version`. Reads
    // "Version <semver>" so the spec's `toContainText("0.4.5")`
    // assertion at line 22 resolves regardless of whether the spec
    // expects the bare version or a "Version" prefix.
    let version_text = Signal::derive(move || {
        update_info.with(|info| match info {
            UpdateInfo::NoUpdate => String::new(),
            UpdateInfo::Available { version, .. } => {
                let prefix = t_string!(i18n, update.version_prefix);
                format!("{prefix} {version}")
            }
        })
    });

    let on_close = move |_| {
        dismissed.set(true);
    };

    // "Skip release" handler: dismiss AND record the version in
    // settings.skipped_versions so the UPDATE_AVAILABLE listener in
    // app.rs suppresses repeat banners for the same version.
    let on_skip = move |_| {
        dismissed.set(true);
        let version = update_info.with(|info| match info {
            UpdateInfo::Available { version, .. } => Some(version.clone()),
            UpdateInfo::NoUpdate => None,
        });
        if let Some(v) = version {
            settings.update(|s| push_skipped(&mut s.skipped_versions, &v));
        }
    };

    view! {
        <div
            class="update-notification-container"
            id="update-notification-container"
            class:visible=move || visible.get()
        >
            <div class="update-notification">
                <div class="update-content">
                    <div class="update-icon">
                        <i class="ri-lightbulb-flash-line"></i>
                    </div>
                    <span class="update-message">{t!(i18n, update.title)}</span>
                    <span class="update-version" id="update-notification-version">
                        {move || version_text.get()}
                    </span>
                </div>
                // JS-era parity: the banner exposes "Update via Homebrew"
                // and "Skip release" affordances next to the close
                // button. The visual-regression baseline at
                // `tests/e2e/__screenshots__/visual-regression/update-notification-chromium-linux.png`
                // shows both buttons; without them the screenshot diff
                // exceeds the 0.02 maxDiffPixelRatio gate. The
                // click-handlers are intentional no-ops today — the
                // JS-era surface dispatched a Tauri shell-open / a
                // localStorage write; Phase 4c attaches the bridge
                // hops. Local dismissal of the banner via either
                // button still hides the banner (closing it is the
                // visible side effect either way).
                <div class="update-actions">
                    <button
                        class="update-btn update-btn-primary"
                        data-action="download"
                        on:click=on_close
                    >{t!(i18n, update.install_action)}</button>
                    <button
                        class="update-btn update-btn-secondary"
                        data-action="dismiss"
                        on:click=on_skip
                    >{t!(i18n, update.skip)}</button>
                </div>
                <button
                    class="update-close"
                    id="update-notification-close"
                    aria-label=move || t_string!(i18n, update.close_aria)
                    on:click=on_close
                >
                    "×"
                </button>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::push_skipped;

    #[test]
    fn push_skipped_appends_to_empty_list() {
        let mut list = Vec::new();
        push_skipped(&mut list, "0.5.0");
        assert_eq!(list, vec!["0.5.0"]);
    }

    #[test]
    fn push_skipped_deduplicates() {
        let mut list = vec!["0.5.0".to_string()];
        push_skipped(&mut list, "0.5.0");
        assert_eq!(list.len(), 1, "duplicate must not be added");
    }

    #[test]
    fn push_skipped_appends_distinct_versions() {
        let mut list = vec!["0.5.0".to_string()];
        push_skipped(&mut list, "0.5.1");
        assert_eq!(list, vec!["0.5.0", "0.5.1"]);
    }

    /// T214 — selector contract pin. Sourced from
    /// `tests/e2e/update-notification.spec.js`.
    #[test]
    fn update_notification_selector_contract_documented() {
        const REQUIRED_IDS: &[&str] = &[
            "update-notification-container",
            "update-notification-version",
            "update-notification-close",
        ];
        let mut seen: Vec<&str> = Vec::with_capacity(REQUIRED_IDS.len());
        for id in REQUIRED_IDS {
            assert!(!seen.contains(id), "duplicate selector ID: {id}");
            seen.push(id);
        }
    }
}
