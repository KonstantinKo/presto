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
use leptos_i18n::{t, t_string};

use crate::bridge::types::Task;
use crate::i18n::i18n::use_i18n;

/// Tasks view — adds / toggles-complete / deletes tasks against an
/// in-memory `RwSignal<Vec<Task>>`. The Tauri-side `save_tasks` /
/// `load_tasks` bridge wrappers are reachable when the bridge is
/// present; Phase 4c attaches the persistence sink (the in-memory
/// signal is the dev-server / e2e-mock branch). Per Principle I,
/// the component never mutates engine state — tasks are independent
/// of the timer state machine.
#[component]
pub fn TasksView() -> impl IntoView {
    let i18n = use_i18n();
    // In-memory task list. Phase 4c reads the persisted seed from
    // `bridge::commands::load_tasks` and supplies it via context;
    // today's local default is the empty list — equivalent to the
    // JS-era cold-start "no tasks file yet" branch.
    let tasks = RwSignal::new(Vec::<Task>::new());

    // Bind for the new-task input field. `String` rather than `&str`
    // because the input value is owned and reset on submit.
    let new_text = RwSignal::new(String::new());

    // The next task id is the largest existing id + 1, mirroring
    // the JS-era `Math.max(...this.tasks.map(t => t.id), 0) + 1`
    // pattern at the JS-side task manager. Defensive against an
    // empty list (returns 1) and against duplicate ids on a
    // corrupted load (the +1 still produces a fresh slot).
    let next_id = move || {
        tasks.with(|list| {
            list.iter()
                .map(|t| t.id)
                .max()
                .map_or(1, |max| max.saturating_add(1))
        })
    };

    // Add-task handler. Trims the input; refuses empty strings
    // (JS-era `if (!text.trim()) return;` guard at
    // `src/main.js`). On success, appends a fresh `Task` and
    // clears the input.
    let on_add = move |_| {
        let text = new_text.with(|s| s.trim().to_string());
        if text.is_empty() {
            return;
        }
        let id = next_id();
        tasks.update(|list| {
            list.push(Task {
                id,
                text,
                completed: false,
                // ISO-8601 timestamp would normally come from the
                // bridge's clock; on the dev server we leave it
                // blank and Phase 4c attaches the real timestamp
                // when `save_tasks` lands.
                created_at: String::new(),
                completed_at: None,
            });
        });
        new_text.set(String::new());
    };

    // Per-row toggle-complete handler. Looked-up by id; missing
    // ids are no-ops (matches JS-era `findIndex(...) !== -1` guard).
    let on_toggle = move |id: u64| {
        tasks.update(|list| {
            if let Some(task) = list.iter_mut().find(|t| t.id == id) {
                task.completed = !task.completed;
            }
        });
    };

    // Per-row delete handler. Same retain-by-id pattern as
    // `TagManager::delete` (filter out the matching id; preserve
    // ordering of survivors).
    let on_delete = move |id: u64| {
        tasks.update(|list| {
            list.retain(|t| t.id != id);
        });
    };

    view! {
        <div class="view-container hidden view-section" id="tasks-view">
            <h1 class="page-header">{t!(i18n, tasks.page_header)}</h1>

            // New-task input row.
            <div class="task-input-row" id="task-input-row">
                <input
                    type="text"
                    id="new-task-input"
                    placeholder=move || t_string!(i18n, tasks.input_placeholder).to_string()
                    aria-label=move || t_string!(i18n, tasks.input_aria).to_string()
                    prop:value=move || new_text.get()
                    on:input=move |ev| new_text.set(event_target_value(&ev))
                />
                <button id="add-task-btn" class="add-task-btn" on:click=on_add>{t!(i18n, tasks.add_button)}</button>
            </div>

            // Task list.
            <ul class="task-list" id="task-list" role="list">
                <For
                    each=move || tasks.get()
                    key=|task| task.id
                    children=move |task| {
                        let task_id = task.id;
                        let completed = task.completed;
                        let delete_label = task.text.clone();
                        let label = task.text.clone();
                        let display = task.text;
                        view! {
                            <li
                                class="task-item"
                                class:completed=completed
                                role="listitem"
                                aria-label=label
                            >
                                <input
                                    type="checkbox"
                                    class="task-checkbox"
                                    prop:checked=completed
                                    on:change=move |_| on_toggle(task_id)
                                />
                                <span class="task-text">{display}</span>
                                <button
                                    class="task-delete-btn"
                                    aria-label=move || t_string!(i18n, tasks.delete_aria, name = delete_label.as_str())
                                    on:click=move |_| on_delete(task_id)
                                >"×"</button>
                            </li>
                        }
                    }
                />
            </ul>
        </div>
    }
}

#[cfg(test)]
mod tests {
    /// T194 — selector contract pin for the tasks view.
    ///
    /// The e2e suite does not currently exercise these selectors
    /// (no `tasks.spec.js` exists today); they're carried forward
    /// so the Phase 6 e2e integration finds the DOM it expects
    /// when the spec lands. Visual baseline updates are out of
    /// scope per AGENTS.md §"Don't update visual regression
    /// baselines without explicit visual review" — the test below
    /// only pins the string contract.
    ///
    /// Contract surface:
    ///
    /// - `tasks-view` — root container; `.hidden` when not the
    ///   active `NavView`.
    /// - `task-input-row` — input + add-button row.
    /// - `new-task-input` — text input for the new-task name.
    /// - `add-task-btn` — submit button.
    /// - `task-list` — `<ul>` host for tasks (role="list").
    /// - `.task-item` — per-row class (rendered by the `<For/>`
    ///   iterator added in T193).
    /// - `.task-checkbox` — completion toggle.
    /// - `.task-delete-btn` — per-row delete button.
    /// - `.task-text` — task body text span.
    #[test]
    fn tasks_view_selector_contract_documented() {
        const REQUIRED_IDS: &[&str] = &[
            "tasks-view",
            "new-task-input",
            "add-task-btn",
            "task-list",
            "task-input-row",
        ];
        const REQUIRED_CLASSES: &[&str] =
            &["task-item", "task-checkbox", "task-delete-btn", "task-text"];
        assert!(!REQUIRED_IDS.is_empty(), "ID contract must be non-empty");
        assert!(
            !REQUIRED_CLASSES.is_empty(),
            "class contract must be non-empty",
        );
        let mut seen_ids: Vec<&str> = Vec::with_capacity(REQUIRED_IDS.len());
        for id in REQUIRED_IDS {
            assert!(!id.is_empty(), "selector ID must not be empty");
            assert!(
                !seen_ids.contains(id),
                "duplicate selector ID in contract: {id}",
            );
            seen_ids.push(id);
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
}
