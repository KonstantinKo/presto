// Inventory subsection for the Daily view — feature 006 §Phase 7
// (T055-T057). Two stacked lists:
//
// - `Quick logs` — header `+ Quick Log` button reuses the same modal
//   the timer view exposes (`#quick-log-modal-overlay`). Per-row
//   inline Edit/Delete pair.
// - `Distractions` — per-row inline Edit/Delete. The parent-session
//   reference per FR-024a re-resolves `parent_tag_id` against the
//   current tag table on every render: if the tag still exists, the
//   row shows the **current** name + colour (reflects renames). If
//   the tag was deleted, it shows the `(deleted tag)` placeholder
//   from catalogue key `inventory.deleted_tag_placeholder`.
//   `parent_title` is rendered as-snapshotted (never re-resolved).
//
// Date filter inherits the existing daily-view `selected_day`
// signal — the inventory shows only entries whose `date` field
// matches the currently-selected day.

#![allow(
    clippy::must_use_candidate,
    clippy::too_many_lines,
    reason = "Leptos `#[component]` returning `impl IntoView`; body is a single `view!` macro expansion (two lists + two edit modals). Matches `sessions_history_table.rs:108` precedent."
)]

use chrono::{DateTime, Utc};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_i18n::{t, t_string};

#[cfg(target_arch = "wasm32")]
use super::super::browser_clock::BrowserClock;
use crate::app::AppToast;
use crate::bridge::commands;
use crate::bridge::types::{Distraction, QuickLog, Tag};
#[cfg(target_arch = "wasm32")]
use crate::engine::clock::Clock;
use crate::engine::date_format::format_session_date;
use crate::i18n::i18n::use_i18n;
use crate::managers::distraction::DistractionManager;
use crate::managers::quick_log::QuickLogManager;

/// Inventory subsection. Renders below the `SessionsHistoryTable`.
#[component]
pub fn Inventory(selected_day: RwSignal<DateTime<Utc>>) -> impl IntoView {
    let i18n = use_i18n();
    let app_toast = use_context::<AppToast>().unwrap_or_default();
    let quick_logs: RwSignal<QuickLogManager> = use_context::<RwSignal<QuickLogManager>>()
        .unwrap_or_else(|| RwSignal::new(QuickLogManager::new()));
    let distractions: RwSignal<DistractionManager> = use_context::<RwSignal<DistractionManager>>()
        .unwrap_or_else(|| RwSignal::new(DistractionManager::new()));
    let tags: RwSignal<Vec<Tag>> =
        use_context::<RwSignal<Vec<Tag>>>().unwrap_or_else(|| RwSignal::new(Vec::new()));

    // `selected_day` -> chrono-formatted "%a %b %d %Y" date string;
    // matches the `QuickLog.date` / `Distraction.date` schema.
    let selected_date_str = Signal::derive(move || {
        let day = selected_day.get();
        format_session_date(day.timestamp_millis())
    });

    // Date-filtered projections. Cloned out of the manager so the
    // signal carries owned values (the `entries_for_date` borrow
    // would otherwise outlive the `with` closure).
    let quick_logs_today = Signal::derive(move || {
        let day = selected_date_str.get();
        quick_logs.with(|mgr| {
            mgr.entries_for_date(&day)
                .into_iter()
                .cloned()
                .collect::<Vec<QuickLog>>()
        })
    });
    let distractions_today = Signal::derive(move || {
        let day = selected_date_str.get();
        distractions.with(|mgr| {
            mgr.entries_for_date(&day)
                .into_iter()
                .cloned()
                .collect::<Vec<Distraction>>()
        })
    });

    // Header `+ Quick Log` button shares the same modal state as the
    // timer view. The Inventory hosts its own modal instance so the
    // Daily view doesn't depend on `<TimerView />` being mounted.
    let inv_quick_log_modal_open = RwSignal::new(false);

    // Edit modal state for quick logs.
    let ql_edit_open = RwSignal::new(false);
    let ql_edit_id = RwSignal::new(String::new());
    let ql_edit_title = RwSignal::new(String::new());
    let ql_edit_minutes = RwSignal::new(5u32);

    let ql_open_edit = move |ql: QuickLog| {
        ql_edit_id.set(ql.id.clone());
        ql_edit_title.set(ql.title.clone());
        ql_edit_minutes.set(ql.elapsed_minutes);
        ql_edit_open.set(true);
    };
    let ql_close_edit = move |_| ql_edit_open.set(false);
    let ql_save_edit = move |_| {
        let id = ql_edit_id.get_untracked();
        let new_title = ql_edit_title.with_untracked(|t| t.trim().to_string());
        let new_minutes = ql_edit_minutes.get_untracked().clamp(1, 720);
        if new_title.is_empty() {
            return;
        }
        quick_logs.update(|mgr| {
            if let Some(existing) = mgr.entries().iter().find(|q| q.id == id).cloned() {
                mgr.update_by_id(QuickLog {
                    id: existing.id,
                    title: new_title,
                    elapsed_minutes: new_minutes,
                    created_at: existing.created_at,
                    date: existing.date,
                });
            }
        });
        let snapshot = quick_logs.with_untracked(QuickLogManager::save_payload);
        spawn_local(async move {
            if let Err(e) = commands::save_quick_logs(snapshot).await {
                leptos::logging::warn!("save_quick_logs (edit) failed: {:?}", e);
            }
        });
        ql_edit_open.set(false);
    };

    let ql_delete = move |id: String| {
        quick_logs.update(|mgr| mgr.delete_by_id(&id));
        let snapshot = quick_logs.with_untracked(QuickLogManager::save_payload);
        spawn_local(async move {
            if let Err(e) = commands::save_quick_logs(snapshot).await {
                leptos::logging::warn!("save_quick_logs (delete) failed: {:?}", e);
            }
        });
    };

    // Edit modal state for distractions.
    let d_edit_open = RwSignal::new(false);
    let d_edit_id = RwSignal::new(String::new());
    let d_edit_note = RwSignal::new(String::new());

    let d_open_edit = move |d: Distraction| {
        d_edit_id.set(d.id.clone());
        d_edit_note.set(d.note);
        d_edit_open.set(true);
    };
    let d_close_edit = move |_| d_edit_open.set(false);
    let d_save_edit = move |_| {
        let id = d_edit_id.get_untracked();
        let new_note = d_edit_note.with_untracked(|n| n.trim().to_string());
        if new_note.is_empty() {
            return;
        }
        distractions.update(|mgr| {
            if let Some(existing) = mgr.entries().iter().find(|d| d.id == id).cloned() {
                mgr.update_by_id(Distraction {
                    id: existing.id,
                    note: new_note,
                    created_at: existing.created_at,
                    date: existing.date,
                    parent_ref: existing.parent_ref,
                });
            }
        });
        let snapshot = distractions.with_untracked(DistractionManager::save_payload);
        spawn_local(async move {
            if let Err(e) = commands::save_distractions(snapshot).await {
                leptos::logging::warn!("save_distractions (edit) failed: {:?}", e);
            }
        });
        d_edit_open.set(false);
    };

    let d_delete = move |id: String| {
        distractions.update(|mgr| mgr.delete_by_id(&id));
        let snapshot = distractions.with_untracked(DistractionManager::save_payload);
        spawn_local(async move {
            if let Err(e) = commands::save_distractions(snapshot).await {
                leptos::logging::warn!("save_distractions (delete) failed: {:?}", e);
            }
        });
    };

    // Quick-log add modal (header `+ Quick Log` button).
    let ql_add_title = RwSignal::new(String::new());
    let ql_add_minutes = RwSignal::new(5u32);
    let ql_add_close = move |_| {
        inv_quick_log_modal_open.set(false);
        ql_add_title.set(String::new());
        ql_add_minutes.set(5);
    };
    let ql_add_submit = move |_| {
        let raw = ql_add_title.with_untracked(|t| t.trim().to_string());
        let mins = ql_add_minutes.get_untracked();
        if raw.is_empty() || !(1..=720).contains(&mins) {
            return;
        }
        let now_ms = selected_day.get_untracked().timestamp_millis();
        let id = format!("quicklog-{}", inventory_uuid());
        quick_logs.update(|mgr| mgr.add(raw, mins, now_ms, id));
        let snapshot = quick_logs.with_untracked(QuickLogManager::save_payload);
        spawn_local(async move {
            if let Err(e) = commands::save_quick_logs(snapshot).await {
                leptos::logging::warn!("save_quick_logs (inventory add) failed: {:?}", e);
            }
        });
        // Optimistic confirmation toast — same key as the timer-view
        // QuickLogModal so the two add paths feel identical.
        app_toast.show(t_string!(i18n, timer.toast.quick_log_added).to_string());
        inv_quick_log_modal_open.set(false);
        ql_add_title.set(String::new());
        ql_add_minutes.set(5);
    };

    view! {
        <div class="inventory-card" id="inventory">
            <div class="inventory-header">
                <h3>{t!(i18n, inventory.section_header)}</h3>
                <button
                    type="button"
                    id="inventory-add-quicklog-btn"
                    class="btn-primary"
                    on:click=move |_| inv_quick_log_modal_open.set(true)
                >{t!(i18n, inventory.btn_add_quicklog)}</button>
            </div>

            // ── Quick logs subsection ───────────────────────────────
            <div class="inventory-subsection" id="inventory-quicklogs-section">
                <h4>{t!(i18n, inventory.subsection_quicklogs)}</h4>
                <Show
                    when=move || !quick_logs_today.get().is_empty()
                    fallback=move || view! {
                        <p class="inventory-empty" id="inventory-quicklogs-empty">
                            {t!(i18n, inventory.empty_quicklogs)}
                        </p>
                    }
                >
                    <ul class="inventory-list" id="inventory-quicklogs-list">
                        <For
                            each=move || quick_logs_today.get()
                            key=|q| q.id.clone()
                            children=move |q| {
                                let q_for_edit = q.clone();
                                let id_for_delete = q.id.clone();
                                // Reactive closure so `minutes_unit` re-
                                // resolves on locale change — `<For>`'s
                                // children fn runs once per row at mount.
                                let mins = q.elapsed_minutes;
                                let mins_label = move || {
                                    format!("{} {}", mins, t_string!(i18n, stats.minutes_unit))
                                };
                                view! {
                                    <li class="inventory-row" data-quicklog-id=q.id>
                                        <span class="inventory-row-title">{q.title}</span>
                                        <span class="inventory-row-meta">{mins_label}</span>
                                        <span class="inventory-row-actions">
                                            <button
                                                type="button"
                                                class="edit-quicklog-btn"
                                                on:click=move |_| ql_open_edit(q_for_edit.clone())
                                            >{t!(i18n, daily.history_edit_button)}</button>
                                            <button
                                                type="button"
                                                class="delete-quicklog-btn"
                                                on:click=move |_| ql_delete(id_for_delete.clone())
                                            >{t!(i18n, daily.modal_delete)}</button>
                                        </span>
                                    </li>
                                }
                            }
                        />
                    </ul>
                </Show>
            </div>

            // ── Distractions subsection ─────────────────────────────
            <div class="inventory-subsection" id="inventory-distractions-section">
                <h4>{t!(i18n, inventory.subsection_distractions)}</h4>
                <Show
                    when=move || !distractions_today.get().is_empty()
                    fallback=move || view! {
                        <p class="inventory-empty" id="inventory-distractions-empty">
                            {t!(i18n, inventory.empty_distractions)}
                        </p>
                    }
                >
                    <ul class="inventory-list" id="inventory-distractions-list">
                        <For
                            each=move || distractions_today.get()
                            key=|d| d.id.clone()
                            children=move |d| {
                                let d_for_edit = d.clone();
                                let id_for_delete = d.id.clone();
                                let parent_title = d
                                    .parent_ref
                                    .as_ref()
                                    .and_then(|p| p.parent_title.clone());
                                let parent_tag_id = d
                                    .parent_ref
                                    .as_ref()
                                    .and_then(|p| p.parent_tag_id.clone());
                                // FR-024a: resolve parent_tag_id
                                // against the *current* tag table.
                                // Tag exists -> render current name +
                                // colour swatch. Tag deleted -> render
                                // (deleted tag) placeholder.
                                let parent_ref_view = move || -> AnyView {
                                    match (parent_title.clone(), parent_tag_id.clone()) {
                                        (None, None) => view! { <span class="inventory-parentref-none">""</span> }.into_any(),
                                        (title_opt, tag_opt) => {
                                            let tag_view = tag_opt
                                                .map(|tid| {
                                                    tags.with(|all| {
                                                        all.iter().find(|t| t.id == tid).cloned()
                                                    })
                                                })
                                                .map(|maybe_tag| match maybe_tag {
                                                    Some(tag) => view! {
                                                        <span class="inventory-parentref-tag">
                                                            <span class="inventory-parentref-tag-swatch" style=format!("background:{}", tag.color)></span>
                                                            <span class="inventory-parentref-tag-name">{tag.name}</span>
                                                        </span>
                                                    }.into_any(),
                                                    None => view! {
                                                        <span class="inventory-parentref-tag inventory-parentref-tag-deleted">
                                                            {t!(i18n, inventory.deleted_tag_placeholder)}
                                                        </span>
                                                    }.into_any(),
                                                });
                                            view! {
                                                <span class="inventory-parentref">
                                                    {title_opt.map(|t| view! { <span class="inventory-parentref-title">{t}</span> }.into_any())}
                                                    {tag_view}
                                                </span>
                                            }.into_any()
                                        }
                                    }
                                };
                                view! {
                                    <li class="inventory-row" data-distraction-id=d.id>
                                        <span class="inventory-row-title">{d.note}</span>
                                        <span class="inventory-row-parentref">{parent_ref_view()}</span>
                                        <span class="inventory-row-actions">
                                            <button
                                                type="button"
                                                class="edit-distraction-btn"
                                                on:click=move |_| d_open_edit(d_for_edit.clone())
                                            >{t!(i18n, daily.history_edit_button)}</button>
                                            <button
                                                type="button"
                                                class="delete-distraction-btn"
                                                on:click=move |_| d_delete(id_for_delete.clone())
                                            >{t!(i18n, daily.modal_delete)}</button>
                                        </span>
                                    </li>
                                }
                            }
                        />
                    </ul>
                </Show>
            </div>

            // ── Add Quick Log modal (header button) ─────────────────
            <div
                class="session-modal-overlay"
                id="inventory-quick-log-modal-overlay"
                style=move || if inv_quick_log_modal_open.get() { "" } else { "display: none" }
            >
                <form class="session-modal"
                    id="inventory-quick-log-form"
                    role="dialog"
                    aria-modal="true"
                    aria-labelledby="inventory-quick-log-modal-title"
                    on:submit=move |ev| {
                        ev.prevent_default();
                        ql_add_submit(ev);
                    }>
                    <div class="session-modal-header">
                        <h3 id="inventory-quick-log-modal-title">{t!(i18n, modal.quick_log_title)}</h3>
                        <button
                            type="button"
                            id="inventory-close-quick-log-modal"
                            class="close-btn"
                            on:click=ql_add_close
                        >"\u{00d7}"</button>
                    </div>
                    <div class="session-modal-body">
                        <label for="inventory-quick-log-title">{t!(i18n, modal.quick_log_title_label)}</label>
                        <input
                            type="text"
                            id="inventory-quick-log-title"
                            maxlength="120"
                            autofocus
                            required
                            prop:value=move || ql_add_title.get()
                            on:input=move |ev| ql_add_title.set(event_target_value(&ev))
                        />
                        <label for="inventory-quick-log-minutes">{t!(i18n, modal.quick_log_minutes_label)}</label>
                        <input
                            type="number"
                            id="inventory-quick-log-minutes"
                            min="1"
                            max="720"
                            prop:value=move || ql_add_minutes.get().to_string()
                            on:input=move |ev| {
                                let raw: u32 = event_target_value(&ev).parse().unwrap_or(5);
                                ql_add_minutes.set(raw.clamp(1, 720));
                            }
                        />
                    </div>
                    <div class="modal-actions">
                        <button
                            type="button"
                            id="inventory-cancel-quick-log-btn"
                            class="btn-secondary"
                            on:click=ql_add_close
                        >{t!(i18n, modal.quick_log_cancel)}</button>
                        <button
                            type="submit"
                            id="inventory-save-quick-log-btn"
                            class="btn-primary"
                        >{t!(i18n, modal.quick_log_submit)}</button>
                    </div>
                </form>
            </div>

            // ── Edit Quick Log modal ─────────────────────────────────
            <div
                class="session-modal-overlay"
                id="inventory-edit-quicklog-overlay"
                style=move || if ql_edit_open.get() { "" } else { "display: none" }
            >
                <form class="session-modal"
                    id="inventory-edit-quicklog-form"
                    role="dialog"
                    aria-modal="true"
                    aria-labelledby="inventory-edit-quicklog-title"
                    on:submit=move |ev| {
                        ev.prevent_default();
                        ql_save_edit(ev);
                    }>
                    <div class="session-modal-header">
                        <h3 id="inventory-edit-quicklog-title">{t!(i18n, modal.quick_log_title)}</h3>
                        <button
                            type="button"
                            class="close-btn"
                            on:click=ql_close_edit
                        >"\u{00d7}"</button>
                    </div>
                    <div class="session-modal-body">
                        <label for="inventory-edit-quicklog-title-input">{t!(i18n, modal.quick_log_title_label)}</label>
                        <input
                            type="text"
                            id="inventory-edit-quicklog-title-input"
                            maxlength="120"
                            required
                            prop:value=move || ql_edit_title.get()
                            on:input=move |ev| ql_edit_title.set(event_target_value(&ev))
                        />
                        <label for="inventory-edit-quicklog-minutes-input">{t!(i18n, modal.quick_log_minutes_label)}</label>
                        <input
                            type="number"
                            id="inventory-edit-quicklog-minutes-input"
                            min="1"
                            max="720"
                            prop:value=move || ql_edit_minutes.get().to_string()
                            on:input=move |ev| {
                                let raw: u32 = event_target_value(&ev).parse().unwrap_or(5);
                                ql_edit_minutes.set(raw.clamp(1, 720));
                            }
                        />
                    </div>
                    <div class="modal-actions">
                        <button
                            type="button"
                            class="btn-secondary"
                            on:click=ql_close_edit
                        >{t!(i18n, modal.quick_log_cancel)}</button>
                        <button
                            type="submit"
                            class="btn-primary"
                        >{t!(i18n, daily.modal_save)}</button>
                    </div>
                </form>
            </div>

            // ── Edit Distraction modal ──────────────────────────────
            <div
                class="session-modal-overlay"
                id="inventory-edit-distraction-overlay"
                style=move || if d_edit_open.get() { "" } else { "display: none" }
            >
                <form class="session-modal"
                    id="inventory-edit-distraction-form"
                    role="dialog"
                    aria-modal="true"
                    aria-labelledby="inventory-edit-distraction-title"
                    on:submit=move |ev| {
                        ev.prevent_default();
                        d_save_edit(ev);
                    }>
                    <div class="session-modal-header">
                        <h3 id="inventory-edit-distraction-title">{t!(i18n, modal.note_distraction_title)}</h3>
                        <button
                            type="button"
                            class="close-btn"
                            on:click=d_close_edit
                        >"\u{00d7}"</button>
                    </div>
                    <div class="session-modal-body">
                        <label for="inventory-edit-distraction-note-input">{t!(i18n, modal.note_distraction_label)}</label>
                        <input
                            type="text"
                            id="inventory-edit-distraction-note-input"
                            maxlength="120"
                            required
                            prop:value=move || d_edit_note.get()
                            on:input=move |ev| d_edit_note.set(event_target_value(&ev))
                        />
                    </div>
                    <div class="modal-actions">
                        <button
                            type="button"
                            class="btn-secondary"
                            on:click=d_close_edit
                        >{t!(i18n, modal.note_distraction_cancel)}</button>
                        <button
                            type="submit"
                            class="btn-primary"
                        >{t!(i18n, daily.modal_save)}</button>
                    </div>
                </form>
            </div>
        </div>
    }
}

/// UUID v4 helper. Crypto-backed on wasm, timestamp-derived fallback
/// on native (host tests). Mirrors `timer/mod.rs::random_uuid` so the
/// id-shape stays identical regardless of which surface created the
/// entry.
#[cfg(target_arch = "wasm32")]
fn inventory_uuid() -> String {
    web_sys::window()
        .as_ref()
        .and_then(|w| w.crypto().ok())
        .map_or_else(|| BrowserClock.now_ms().to_string(), |c| c.random_uuid())
}

#[cfg(not(target_arch = "wasm32"))]
const fn inventory_uuid() -> String {
    String::new()
}
