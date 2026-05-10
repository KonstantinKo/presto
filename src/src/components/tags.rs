// Tags view component — Phase 4a (T201-T203) of spec
// 001-leptos-migration.
//
// Skeleton (T201): mount the tag-dropdown shell with the e2e
// selector contract preserved. Wiring (T202): route create / delete
// clicks into a `RwSignal<Vec<Tag>>` (Phase 4c hops through
// `TagManager::create` / `delete` for the persistence half).
// T203 lands the visual regression check.
//
// **Selector contract** (consumed by `tests/e2e/tags.spec.js` and
// `sessions-history.spec.js`):
// - `#tag-dropdown-menu` — dropdown root (`spec.js:11,33`
//   `toBeVisible`; click on `#timer-status` toggles).
// - `#selected-icon-btn` — icon picker button (`spec.js:14`
//   click).
// - `#icon-selector-dropdown` — icon picker pop-out (`spec.js:15`
//   `toBeVisible`).
// - `.emoji-option[data-icon="..."]` — per-emoji option
//   (`spec.js:16` `data-icon="🎯"`).
// - `#new-tag-name` — text input (`spec.js:19`).
// - `#create-tag-btn` — submit button (`spec.js:20`).
// - `#tag-list` — `<ul>`-style host (`spec.js:24,29,35`).
// - `#tag-list [role="listitem"]` — per-tag row (`spec.js:24`).
// - `#tag-list .tag-item` — alternative class hook used by
//   `sessions-history.spec.js:18`.
// - Per-row delete button — `getByRole("button", { name: /delete
//   <tag name> tag/i })` at `spec.js:39`. The aria-label is the
//   contract surface; we pin its shape inline.
//
// Per Principle I, this component never mutates engine state —
// tags are independent of the timer state machine.
//
// Lint allowance: `clippy::must_use_candidate` is silenced module-
// wide for the same reason as on `timer.rs` etc.
#![allow(clippy::must_use_candidate)]

use leptos::prelude::*;

/// Tags view skeleton. Renders the tag-dropdown shell + icon picker
/// + new-tag input row with the e2e selector contract preserved.
/// T202 attaches the per-row `<For/>` iterator and the create /
/// delete handlers; today the static shell carries placeholder
/// emoji options so the icon-picker selector resolves under
/// visual regression.
///
/// The dropdown is rendered in the DOM tree even when "closed";
/// JS-era CSS hides `#tag-dropdown-menu:not(.open)` via display
/// rules. The e2e suite asserts `toBeVisible()` after clicking
/// `#timer-status` — that click handler lives in the Timer
/// component (T190); the wiring between the two lands in Phase 4c
/// via a shared "dropdown is open" context signal.
#[component]
pub fn TagsView() -> impl IntoView {
    view! {
        <div class="tag-dropdown-menu" id="tag-dropdown-menu">
            <div class="tag-dropdown-header">
                <span>"Choose tag"</span>
            </div>

            // Tag list — per-row content lands in T202.
            <div class="tag-list" id="tag-list" role="list"></div>

            // New-tag footer: icon picker + text input + create button.
            <div class="tag-dropdown-footer">
                <div class="new-tag-input" id="new-tag-input">
                    <div class="tag-input-row">
                        <div class="icon-selector-container">
                            <button class="selected-icon-btn" id="selected-icon-btn">
                                <i class="ri-brain-line" id="selected-icon-display"></i>
                                <i class="ri-arrow-down-s-line dropdown-arrow"></i>
                            </button>
                            <div class="icon-selector-dropdown" id="icon-selector-dropdown">
                                // Emoji options match the JS-era index.html shape.
                                // T202 attaches the `on:click` handler that
                                // updates `#selected-icon-display`.
                                <div class="emoji-option" data-icon="🧠">"🧠"</div>
                                <div class="emoji-option" data-icon="💪">"💪"</div>
                                <div class="emoji-option" data-icon="🎯">"🎯"</div>
                                <div class="emoji-option" data-icon="⚡">"⚡"</div>
                                <div class="emoji-option" data-icon="🔥">"🔥"</div>
                            </div>
                        </div>
                        <input
                            type="text"
                            placeholder="New tag..."
                            id="new-tag-name"
                            aria-label="New tag name"
                        />
                        <button class="create-tag-btn" id="create-tag-btn">"+"</button>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    /// Selector contract pin for the tags dropdown, sourced from
    /// `tests/e2e/tags.spec.js` and `sessions-history.spec.js`.
    /// Each entry maps to a `locator("#…")` callsite; drift here
    /// breaks the e2e run.
    #[test]
    fn tags_view_selector_contract_documented() {
        const REQUIRED_IDS: &[&str] = &[
            "tag-dropdown-menu",
            "tag-list",
            "selected-icon-btn",
            "selected-icon-display",
            "icon-selector-dropdown",
            "new-tag-input",
            "new-tag-name",
            "create-tag-btn",
        ];
        let mut seen: Vec<&str> = Vec::with_capacity(REQUIRED_IDS.len());
        for id in REQUIRED_IDS {
            assert!(!id.is_empty(), "selector ID must not be empty");
            assert!(
                !seen.contains(id),
                "duplicate selector ID in contract: {id}",
            );
            seen.push(id);
        }
    }
}
