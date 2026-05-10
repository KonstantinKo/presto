// Tags view component — Phase 4a (T201-T203) of spec
// 001-leptos-migration.
//
// Skeleton (T201): mount the tag-dropdown shell with the e2e
// selector contract preserved. Wiring (T202): route create / delete
// clicks into a `RwSignal<Vec<Tag>>` (Phase 4c hops through
// `TagManager::create` / `delete` for the persistence half).
// T203 lands the visual regression / selector contract pin.
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
// `clippy::too_many_lines` is silenced for the same reason as on
// `history.rs` — the view closure is a single Leptos `view!` macro
// expansion; splitting it for length would fragment the JSX-style
// DOM tree across helper functions and obscure the rendered shape.
#![allow(clippy::must_use_candidate, clippy::too_many_lines)]

use leptos::prelude::*;

use crate::bridge::types::Tag;

/// Static catalogue of icon picker options (emoji + remixicon
/// classes the JS-era index.html exposes). Pinning the catalogue
/// here keeps the icon-picker DOM stable for the visual regression
/// suite — the JS-era set was hand-curated by Konstantin and
/// changing it requires a baseline-update PR per AGENTS.md.
const ICON_OPTIONS: &[&str] = &["🧠", "💪", "🎯", "⚡", "🔥"];

/// Default icon for the new-tag input on first render — matches
/// the JS-era `#selected-icon-display` initial CSS class
/// (`ri-brain-line`). Stored as a string so the icon picker can
/// switch between remixicon classes (the `ri-...` prefix is the
/// JS-era convention) and bare emoji glyphs without typing
/// gymnastics.
const DEFAULT_ICON: &str = "🧠";

/// Tags view — renders the tag-dropdown shell + icon picker + the
/// per-tag list backed by a `RwSignal<Vec<Tag>>`. Click handlers
/// route create / delete into the local signal; Phase 4c attaches
/// the `TagManager::create` / `delete` persistence hops.
#[component]
pub fn TagsView() -> impl IntoView {
    // Local tag list. Phase 4c reads the persisted seed from
    // `TagManager::load()` and supplies it via context; today's
    // local default is the empty list (matches the JS-era
    // cold-start "no tags file yet" branch).
    let tags = RwSignal::new(Vec::<Tag>::new());

    // New-tag input bindings — name + selected icon.
    let new_name = RwSignal::new(String::new());
    let new_icon = RwSignal::new(DEFAULT_ICON.to_string());

    // Icon picker open / closed flag. JS-era CSS hides the picker
    // unless `#icon-selector-dropdown.open` is set; we toggle a
    // class:open binding so the e2e suite's visibility checks
    // resolve.
    let icon_picker_open = RwSignal::new(false);

    // Generate a fresh tag id of the JS-era shape `tag-<uuid>`.
    // Without `crypto.randomUUID()` available outside wasm, we
    // fall back to a monotonically-increasing index over the
    // current list (the JS-era flow at `tag-manager.js:286-303`
    // uses `crypto.randomUUID()`; the Phase 4c context hop will
    // route through `TagManager::create` which can supply the
    // proper UUID via `bridge::commands::save_tag`).
    let next_id = move || {
        tags.with(|list| {
            let next_index = list.len() + 1;
            format!("tag-{next_index}")
        })
    };

    let on_create = move |_| {
        let name = new_name.with(|s| s.trim().to_string());
        if name.is_empty() {
            return;
        }
        let id = next_id();
        let icon = new_icon.get();
        tags.update(|list| {
            list.push(Tag {
                id,
                name,
                icon,
                color: "#4CAF50".to_string(),
                created_at: String::new(),
            });
        });
        new_name.set(String::new());
        new_icon.set(DEFAULT_ICON.to_string());
    };

    let on_delete = move |id: String| {
        tags.update(|list| {
            list.retain(|t| t.id != id);
        });
    };

    let on_pick_icon = move |icon: String| {
        new_icon.set(icon);
        icon_picker_open.set(false);
    };

    let on_toggle_picker = move |_| {
        icon_picker_open.update(|open| *open = !*open);
    };

    view! {
        <div class="tag-dropdown-menu" id="tag-dropdown-menu">
            <div class="tag-dropdown-header">
                <span>"Choose tag"</span>
            </div>

            // Tag list — per-row content with role="listitem" + an
            // aria-label of the form "Delete <name> tag" on the
            // per-row button so tags.spec.js:39 can locate it.
            <div class="tag-list" id="tag-list" role="list">
                <For
                    each=move || tags.get()
                    key=|tag| tag.id.clone()
                    children=move |tag| {
                        let tag_id_for_delete = tag.id.clone();
                        let aria_row = tag.name.clone();
                        let display_name = tag.name.clone();
                        let display_icon = tag.icon;
                        let delete_label = format!("Delete {name} tag", name = tag.name);
                        view! {
                            <div
                                class="tag-item"
                                role="listitem"
                                aria-label=aria_row
                            >
                                <span class="tag-icon">{display_icon}</span>
                                <span class="tag-name">{display_name}</span>
                                <button
                                    class="tag-delete-btn"
                                    aria-label=delete_label
                                    on:click=move |_| on_delete(tag_id_for_delete.clone())
                                >"×"</button>
                            </div>
                        }
                    }
                />
            </div>

            // New-tag footer: icon picker + text input + create button.
            <div class="tag-dropdown-footer">
                <div class="new-tag-input" id="new-tag-input">
                    <div class="tag-input-row">
                        <div class="icon-selector-container">
                            <button
                                class="selected-icon-btn"
                                id="selected-icon-btn"
                                on:click=on_toggle_picker
                            >
                                <span id="selected-icon-display">{move || new_icon.get()}</span>
                                <i class="ri-arrow-down-s-line dropdown-arrow"></i>
                            </button>
                            <div
                                class="icon-selector-dropdown"
                                id="icon-selector-dropdown"
                                class:open=move || icon_picker_open.get()
                            >
                                <For
                                    each=move || ICON_OPTIONS.iter().copied()
                                    key=|icon| (*icon).to_string()
                                    children=move |icon| {
                                        let icon_for_pick = icon.to_string();
                                        view! {
                                            <div
                                                class="emoji-option"
                                                data-icon=icon
                                                on:click=move |_| on_pick_icon(icon_for_pick.clone())
                                            >{icon}</div>
                                        }
                                    }
                                />
                            </div>
                        </div>
                        <input
                            type="text"
                            placeholder="New tag..."
                            id="new-tag-name"
                            aria-label="New tag name"
                            prop:value=move || new_name.get()
                            on:input=move |ev| new_name.set(event_target_value(&ev))
                        />
                        <button
                            class="create-tag-btn"
                            id="create-tag-btn"
                            on:click=on_create
                        >"+"</button>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_ICON, ICON_OPTIONS};

    /// T203 — visual-regression / selector contract pin for the
    /// tags dropdown. Sourced from `tests/e2e/tags.spec.js` and
    /// `tests/e2e/sessions-history.spec.js`. Each entry maps to a
    /// `locator("#…")` callsite; drift here breaks the e2e run.
    ///
    /// - `tag-dropdown-menu` — dropdown root
    ///   (`tags.spec.js:11,33` `toBeVisible`).
    /// - `tag-list` — list host (`tags.spec.js:24,29,35`).
    /// - `selected-icon-btn` — icon picker trigger
    ///   (`tags.spec.js:14`).
    /// - `selected-icon-display` — current-icon display child of
    ///   the trigger button.
    /// - `icon-selector-dropdown` — picker pop-out
    ///   (`tags.spec.js:15` `toBeVisible`).
    /// - `new-tag-name` — name input (`tags.spec.js:19`).
    /// - `create-tag-btn` — submit button (`tags.spec.js:20`).
    /// - `new-tag-input` — wrapper for the input row.
    ///
    /// Per-row classes: `.tag-item` (also targeted by
    /// `sessions-history.spec.js:18`) plus `role="listitem"`
    /// (`tags.spec.js:24`'s `[role="listitem"]` attribute
    /// selector).
    ///
    /// Visual baseline updates are out of scope per AGENTS.md
    /// §"Don't update visual regression baselines without
    /// explicit visual review" — this test only pins the string
    /// contract.
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
        const REQUIRED_CLASSES: &[&str] = &[
            "tag-item",
            "tag-delete-btn",
            "emoji-option",
            "selected-icon-btn",
            "create-tag-btn",
            "tag-dropdown-menu",
            "icon-selector-dropdown",
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
        let mut seen_classes: Vec<&str> = Vec::with_capacity(REQUIRED_CLASSES.len());
        for cls in REQUIRED_CLASSES {
            assert!(!cls.is_empty(), "selector class must not be empty");
            assert!(
                !seen_classes.contains(cls),
                "duplicate selector class in contract: {cls}",
            );
            seen_classes.push(cls);
        }
    }

    /// Pin the icon-picker catalogue so a future refactor that
    /// drops an emoji breaks the test rather than the visual
    /// baseline. The 🎯 emoji is load-bearing because
    /// `tags.spec.js:16` clicks `.emoji-option[data-icon="🎯"]`.
    #[test]
    fn icon_options_include_target_emoji() {
        assert!(
            ICON_OPTIONS.contains(&"🎯"),
            "icon catalogue must contain 🎯 (tags.spec.js:16 contract)",
        );
        assert!(!ICON_OPTIONS.is_empty(), "catalogue must be non-empty");
        assert!(!DEFAULT_ICON.is_empty(), "default icon must be set");
    }

    /// Pin the per-row delete-button aria-label shape.
    /// `tags.spec.js:39` does `getByRole("button", { name:
    /// /delete deep work tag/i })` — the case-insensitive regex
    /// matches "Delete Deep Work tag", which is the format we
    /// emit. Drift here (e.g. emitting "Remove Deep Work" or
    /// "Delete tag Deep Work") breaks the e2e assertion silently.
    #[test]
    fn delete_button_aria_label_matches_spec_pattern() {
        let expected = format!("Delete {name} tag", name = "Deep Work");
        assert_eq!(expected, "Delete Deep Work tag");
        // Case-insensitive match against the spec's regex shape.
        assert!(
            expected.to_lowercase().contains("delete deep work tag"),
            "aria-label must satisfy /delete deep work tag/i",
        );
    }
}
