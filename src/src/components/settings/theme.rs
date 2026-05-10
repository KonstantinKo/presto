// Theme settings tab — placeholder. Real implementation lands in a
// follow-up Phase 4b commit; today this stub mounts the empty
// category so the settings shell (T204) compiles.
#![allow(clippy::must_use_candidate)]

use leptos::prelude::*;

#[component]
pub fn ThemeSettings() -> impl IntoView {
    view! {
        <div class="category-header">
            <h1>"Theme"</h1>
        </div>
    }
}
