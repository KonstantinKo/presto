// Notifications settings tab — placeholder. Real implementation
// lands in a follow-up Phase 4b commit; today this stub mounts the
// empty category so the settings shell (T204) compiles.
#![allow(clippy::must_use_candidate)]

use leptos::prelude::*;

use crate::bridge::types::Settings;
use crate::components::settings::SettingsToast;

#[component]
pub fn NotificationsSettings(
    #[allow(unused_variables, reason = "wired in follow-up Notifications task")]
    settings: RwSignal<Settings>,
    #[allow(unused_variables, reason = "wired in follow-up Notifications task")]
    toast: SettingsToast,
) -> impl IntoView {
    view! {
        <div class="category-header">
            <h1>"Notifications"</h1>
        </div>
    }
}
