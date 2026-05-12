// Sessions-history table for the Daily view — mirrors the
// pre-rework `#sessions-table-body` block in `components::calendar`
// (lines 644+, kept off-viewport so the visual-regression baseline
// doesn't include it). The selector string `#sessions-table-body` is
// preserved (FR-019 / A14 / CHK043).
//
// Hosts the edit modal too (preserved e2e contract per CHK043:
// `sessions-history.spec.js:41-48` uses `#session-modal-overlay` and
// `#close-session-modal` after the `tapTab("Daily")` migration in
// T011).
//
// Selector contract preserved: `#sessions-table-body`,
// `#sessions-table`, `#export-sessions-btn`,
// `#session-modal-overlay`, `#session-form`, `#session-modal-title`,
// `#close-session-modal`, `#session-title`, `#session-start-time`,
// `#session-end-time`, `#session-duration`, `#cancel-session-btn`,
// `#delete-session-btn`, `#save-session-btn`.

#![allow(
    clippy::must_use_candidate,
    clippy::too_many_lines,
    reason = "Leptos `#[component]` returning `impl IntoView`; body is a single `view!` macro expansion (table + modal). Matches `calendar.rs:32` precedent."
)]

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::bridge::commands;
use crate::bridge::types::ManualSession;

const TITLE_DISPLAY_CAP: usize = 40;

fn truncated_title(full: &str) -> String {
    let chars: Vec<char> = full.chars().collect();
    if chars.len() <= TITLE_DISPLAY_CAP {
        full.to_string()
    } else {
        let mut out: String = chars.into_iter().take(TITLE_DISPLAY_CAP).collect();
        out.push('\u{2026}');
        out
    }
}

fn title_cell_view(
    title: Option<&str>,
    tags: Option<&[serde_json::Value]>,
) -> impl IntoView + use<> {
    title.map_or_else(
        || {
            let joined = tags
                .map(|ts| {
                    ts.iter()
                        .filter_map(|v| v.get("name").and_then(serde_json::Value::as_str))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            if joined.is_empty() {
                view! { <td>"\u{00a0}"</td> }.into_any()
            } else {
                view! { <td>{joined}</td> }.into_any()
            }
        },
        |full| {
            let display = truncated_title(full);
            let full_owned = full.to_string();
            view! { <td title=full_owned>{display}</td> }.into_any()
        },
    )
}

fn duration_from_start_end_minutes(start: &str, end: &str) -> u32 {
    let parse = |s: &str| -> u32 {
        let mut p = s.splitn(2, ':');
        let h = p.next().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
        let m = p.next().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
        h * 60 + m
    };
    let s = parse(start);
    let e = parse(end);
    if e >= s {
        e - s
    } else {
        e + 24 * 60 - s
    }
}

/// Returns `(end_time_string, clamped_duration)` so callers persist a
/// duration consistent with the clamped end (never past 23:59).
fn end_time_from_start_duration(start: &str, duration: u32) -> (String, u32) {
    let parse = |s: &str| -> u32 {
        let mut p = s.splitn(2, ':');
        let h = p.next().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
        let m = p.next().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
        h * 60 + m
    };
    let start_min = parse(start);
    let end_min = (start_min + duration).min(23 * 60 + 59);
    let clamped_dur = end_min - start_min;
    (
        format!("{:02}:{:02}", end_min / 60, end_min % 60),
        clamped_dur,
    )
}

#[component]
pub fn SessionsHistoryTable() -> impl IntoView {
    let sessions =
        use_context::<RwSignal<Vec<ManualSession>>>().unwrap_or_else(|| RwSignal::new(Vec::new()));

    let session_modal_open = RwSignal::new(false);
    let modal_duration = RwSignal::new(0_u32);
    let modal_session_id = RwSignal::new(Option::<String>::None);
    let modal_start = RwSignal::new(String::new());
    let modal_end = RwSignal::new(String::new());
    let modal_title = RwSignal::new(String::new());

    let on_open_modal = move |session: ManualSession| {
        modal_session_id.set(Some(session.id.clone()));
        modal_start.set(session.start_time.clone());
        modal_end.set(session.end_time.clone());
        modal_duration.set(session.duration);
        modal_title.set(session.title.unwrap_or_default());
        session_modal_open.set(true);
    };
    let on_close_modal = move |_| session_modal_open.set(false);

    view! {
        <div class="sessions-history-card">
            <div class="sessions-header">
                <h3>"Session History"</h3>
                <div class="sessions-controls">
                    <button
                        id="export-sessions-btn"
                        class="export-btn"
                        title="Export to Excel"
                        on:click=move |_| {
                            let snapshot = sessions.get_untracked();
                            spawn_local(async move {
                                let path = commands::dialog_save(
                                    Some("sessions.xlsx".to_string()),
                                    vec![("Excel".to_string(), vec!["xlsx".to_string()])],
                                )
                                .await
                                .ok()
                                .flatten();
                                if let Some(p) = path {
                                    let _ = commands::export_sessions_xlsx(p, snapshot).await;
                                }
                            });
                        }
                    >
                        <i class="ri-download-line"></i>
                        " Export"
                    </button>
                </div>
            </div>
            <div class="sessions-table-container">
                <table class="sessions-table" id="sessions-table">
                    <thead>
                        <tr>
                            <th>"Time"</th>
                            <th>"Title"</th>
                            <th>"Duration"</th>
                            <th>"Actions"</th>
                        </tr>
                    </thead>
                    <tbody id="sessions-table-body">
                        <For
                            each=move || sessions.get()
                            key=|row| row.id.clone()
                            children=move |row| {
                                let session_for_modal = row.clone();
                                let time_range = format!("{} – {}", row.start_time, row.end_time);
                                let duration_text = format!("{} min", row.duration);
                                let title_cell = title_cell_view(
                                    row.title.as_deref(),
                                    row.tags.as_deref(),
                                );
                                view! {
                                    <tr class="session-row" role="row">
                                        <td>{time_range}</td>
                                        {title_cell}
                                        <td>{duration_text}</td>
                                        <td>
                                            <button
                                                class="edit-session-btn"
                                                aria-label="Edit session"
                                                on:click=move |_| on_open_modal(session_for_modal.clone())
                                            >"Edit"</button>
                                        </td>
                                    </tr>
                                }
                            }
                        />
                    </tbody>
                </table>
            </div>

            // Edit modal — same shape as the calendar.rs original.
            <div
                class="session-modal-overlay"
                id="session-modal-overlay"
                style=move || if session_modal_open.get() { "" } else { "display: none" }
            >
                <form class="session-modal" id="session-form" role="dialog" aria-modal="true" aria-labelledby="session-modal-title">
                    <div class="session-modal-header">
                        <h3 id="session-modal-title">"Edit session"</h3>
                        <button
                            type="button"
                            id="close-session-modal"
                            class="close-btn"
                            aria-label="Close edit modal"
                            on:click=on_close_modal
                        >"\u{00d7}"</button>
                    </div>
                    <div class="session-modal-body">
                        <label for="session-title">"Title"</label>
                        <input
                            type="text"
                            id="session-title"
                            maxlength="120"
                            placeholder="What is this session for?"
                            prop:value=move || modal_title.get()
                            on:input=move |ev| {
                                modal_title.set(event_target_value(&ev));
                            }
                        />
                        <label for="session-start-time">"Start Time"</label>
                        <input
                            type="time"
                            id="session-start-time"
                            prop:value=move || modal_start.get()
                            on:input=move |ev| {
                                let new_start = event_target_value(&ev);
                                let (new_end, new_dur) = end_time_from_start_duration(
                                    &new_start,
                                    modal_duration.get_untracked(),
                                );
                                modal_start.set(new_start);
                                modal_end.set(new_end);
                                modal_duration.set(new_dur);
                            }
                        />
                        <label for="session-end-time">"End Time"</label>
                        <input
                            type="time"
                            id="session-end-time"
                            prop:value=move || modal_end.get()
                            on:input=move |ev| {
                                let new_end = event_target_value(&ev);
                                let new_dur = duration_from_start_end_minutes(
                                    &modal_start.get_untracked(),
                                    &new_end,
                                );
                                modal_end.set(new_end);
                                modal_duration.set(new_dur);
                            }
                        />
                        <label for="session-duration">"Duration (minutes)"</label>
                        <input
                            type="number"
                            id="session-duration"
                            min="1"
                            max="180"
                            prop:value=move || modal_duration.get().to_string()
                            on:input=move |ev| {
                                let raw: u32 =
                                    event_target_value(&ev).parse().unwrap_or(1);
                                let clamped = raw.clamp(1, 180);
                                let (new_end, clamped_dur) = end_time_from_start_duration(
                                    &modal_start.get_untracked(),
                                    clamped,
                                );
                                modal_duration.set(clamped_dur);
                                modal_end.set(new_end);
                            }
                        />
                    </div>
                    <div class="modal-actions">
                        <button
                            type="button"
                            id="cancel-session-btn"
                            class="btn-secondary"
                            on:click=on_close_modal
                        >"Cancel"</button>
                        <button
                            type="button"
                            id="delete-session-btn"
                            class="btn-danger"
                            on:click=move |_| {
                                if let Some(id) = modal_session_id.get_untracked() {
                                    sessions.update(|ss| ss.retain(|s| s.id != id));
                                }
                                session_modal_open.set(false);
                            }
                        >"Delete"</button>
                        <button
                            type="button"
                            id="save-session-btn"
                            class="btn-primary"
                            on:click=move |_| {
                                if let Some(id) = modal_session_id.get_untracked() {
                                    let dur = modal_duration.get_untracked();
                                    let start = modal_start.get_untracked();
                                    let (end, clamped_dur) =
                                        end_time_from_start_duration(&start, dur);
                                    let title_raw = modal_title.get_untracked();
                                    let title = {
                                        let trimmed = title_raw.trim();
                                        if trimmed.is_empty() {
                                            None
                                        } else {
                                            Some(trimmed.to_string())
                                        }
                                    };
                                    sessions.update(|ss| {
                                        if let Some(s) = ss.iter_mut().find(|s| s.id == id) {
                                            s.duration = clamped_dur;
                                            s.start_time = start;
                                            s.end_time = end;
                                            s.title = title;
                                        }
                                    });
                                }
                                session_modal_open.set(false);
                            }
                        >"Save"</button>
                    </div>
                </form>
            </div>
        </div>
    }
}
