// Shortcuts settings tab — placeholder. Real implementation lands
// in a follow-up Phase 4b commit (Shortcuts task); today this stub
// mounts the empty category so the settings shell (T204) compiles.
#![allow(clippy::must_use_candidate)]

use leptos::prelude::*;

use crate::bridge::types::Settings;
use crate::components::settings::SettingsToast;

#[component]
pub fn ShortcutsSettings(
    #[allow(unused_variables, reason = "wired in follow-up Shortcuts task")]
    settings: RwSignal<Settings>,
    #[allow(unused_variables, reason = "wired in follow-up Shortcuts task")]
    toast: SettingsToast,
) -> impl IntoView {
    view! {
        <div class="category-header">
            <h1>"Global Shortcuts"</h1>
        </div>
    }
}
