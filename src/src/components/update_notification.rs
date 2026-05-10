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

use crate::managers::update::UpdateInfo;

/// Update notification banner.
///
/// Props:
/// - `update_info`: shared `RwSignal<UpdateInfo>` driven by the
///   `UpdateManager`. The component reads the variant to decide
///   whether to add the `.visible` class; the version text
///   projects the `Available::version` slot.
#[component]
pub fn UpdateNotification(
    update_info: RwSignal<UpdateInfo>,
) -> impl IntoView {
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
        !dismissed.get()
            && update_info.with(|i| matches!(i, UpdateInfo::Available { .. }))
    });

    // Version text for `#update-notification-version`. Reads
    // "Version <semver>" so the spec's `toContainText("0.4.5")`
    // assertion at line 22 resolves regardless of whether the spec
    // expects the bare version or a "Version" prefix.
    let version_text = Signal::derive(move || {
        update_info.with(|info| match info {
            UpdateInfo::NoUpdate => String::new(),
            UpdateInfo::Available { version, .. } => format!("Version {version}"),
        })
    });

    let on_close = move |_| {
        dismissed.set(true);
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
                    <span class="update-message">"Update available"</span>
                    <span class="update-version" id="update-notification-version">
                        {move || version_text.get()}
                    </span>
                </div>
                <button
                    class="update-close"
                    id="update-notification-close"
                    aria-label="Close update notification"
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
