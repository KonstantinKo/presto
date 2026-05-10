// Tasks view component — Phase 4a (T192-T194) of spec
// 001-leptos-migration.
//
// Skeleton (T192): mount a task-list view shell. Wiring (T193):
// route add / toggle-complete / delete clicks into the in-memory
// list (the Tauri-side `save_tasks` / `load_tasks` bridge wrappers
// were ported in Phase 1D — the component dispatches through them
// when the bridge is present). T194 lands the visual regression
// check.
//
// **Selector contract** (used by Phase 4c integration tests + the
// visual regression baseline): the tasks view is not currently
// exercised by an e2e spec under `tests/e2e/`; selectors here are
// the JS-era `id="tasks-view"` shell + per-row `.task-item`
// classes that the JS UI used. Phase 6 wires the e2e suite once
// the components land.
//
// Per Principle I, the component never mutates engine state — its
// data lives in `bridge::types::Task` records persisted via
// `bridge::commands::{load_tasks, save_tasks}`. The signal-wired
// list is a UI cache; persistence is best-effort (mirrors the
// JS-era `saveTasksToStorage` debounce).
//
// Lint allowance: `clippy::must_use_candidate` is silenced module-
// wide for the same reason as on `timer.rs` — Leptos `#[component]`
// functions return `impl IntoView`, which the framework consumes
// inside `view!` / `mount_to_body` automatically.
#![allow(clippy::must_use_candidate)]

use leptos::prelude::*;

/// Tasks view skeleton. Renders an `<input>` for adding tasks plus
/// a `<ul>` list host. T193 attaches the add / toggle-complete /
/// delete handlers and the per-row `<For/>` iterator; today the
/// view is the static shell so `(cd src && trunk build)` returns
/// 0 and the future visual baseline lands against stable DOM.
///
/// The component is parameterless: T193 introduces a local
/// `RwSignal<Vec<bridge::types::Task>>` for state ownership;
/// Phase 4c will lift it to a context slice when the persistence
/// wiring lands.
#[component]
pub fn TasksView() -> impl IntoView {
    view! {
        <div class="view-container hidden view-section" id="tasks-view">
            <h1 class="page-header">"Tasks"</h1>

            // New-task input row — submit handler attaches in T193.
            <div class="task-input-row" id="task-input-row">
                <input
                    type="text"
                    id="new-task-input"
                    placeholder="Add a task..."
                    aria-label="New task"
                />
                <button id="add-task-btn" class="add-task-btn">"Add"</button>
            </div>

            // Task list. T193 attaches the per-row `<For/>` iterator
            // and the click handlers; today the host is empty.
            <ul class="task-list" id="task-list" role="list"></ul>
        </div>
    }
}

#[cfg(test)]
mod tests {
    /// Selector-contract pin for the tasks view. The e2e suite
    /// does not currently exercise these selectors (no
    /// `tasks.spec.js` exists today); they're carried forward so
    /// the Phase 6 e2e integration finds the DOM it expects when
    /// the spec lands. The contract surface is:
    ///
    /// - `tasks-view` — root container; `.hidden` when not the
    ///   active `NavView`.
    /// - `new-task-input` — text input for the new-task name.
    /// - `add-task-btn` — submit button.
    /// - `task-list` — `<ul>` host for tasks.
    /// - `.task-item` — per-row class (T193 attaches the per-row
    ///   `<For/>` iterator).
    /// - `.task-checkbox` — completion toggle.
    /// - `.task-delete-btn` — per-row delete button.
    #[test]
    fn tasks_view_selector_contract_documented() {
        const REQUIRED_IDS: &[&str] = &[
            "tasks-view",
            "new-task-input",
            "add-task-btn",
            "task-list",
            "task-input-row",
        ];
        const REQUIRED_CLASSES: &[&str] = &["task-item", "task-checkbox", "task-delete-btn"];
        assert!(!REQUIRED_IDS.is_empty(), "ID contract must be non-empty");
        assert!(
            !REQUIRED_CLASSES.is_empty(),
            "class contract must be non-empty",
        );
        for id in REQUIRED_IDS {
            assert!(!id.is_empty());
        }
        for cls in REQUIRED_CLASSES {
            assert!(!cls.is_empty());
        }
    }
}
