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

use chrono::DateTime;
use chrono::Utc;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_i18n::{t, t_string};

use crate::bridge::commands;
use crate::bridge::types::{ManualSession, SessionType};
use crate::engine::date_format::format_session_date;
use crate::i18n::i18n::use_i18n;

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

fn parse_time_minutes(value: &str) -> Option<u32> {
    let mut p = value.splitn(2, ':');
    let h = p.next()?.parse::<u32>().ok()?;
    let m = p.next()?.parse::<u32>().ok()?;
    if h < 24 && m < 60 {
        Some(h * 60 + m)
    } else {
        None
    }
}

#[component]
pub fn SessionsHistoryTable(selected_day: RwSignal<DateTime<Utc>>) -> impl IntoView {
    let i18n = use_i18n();
    let sessions =
        use_context::<RwSignal<Vec<ManualSession>>>().unwrap_or_else(|| RwSignal::new(Vec::new()));

    // Match the timeline's scoping: only sessions whose `date` field
    // equals the JS-era `toDateString()` projection of `selected_day`
    // are shown. Sessions store local-time dates (see
    // `engine::date_format`), so we compare label-equal rather than
    // by chrono components.
    let scoped_sessions = Signal::derive(move || {
        let selected_label =
            format_session_date(selected_day.with(chrono::DateTime::timestamp_millis));
        sessions.with(|all| {
            all.iter()
                .filter(|s| s.date == selected_label)
                .cloned()
                .collect::<Vec<_>>()
        })
    });

    let session_modal_open = RwSignal::new(false);
    let modal_session_id = RwSignal::new(Option::<String>::None);
    let modal_start = RwSignal::new(String::new());
    let modal_end = RwSignal::new(String::new());
    let modal_title = RwSignal::new(String::new());

    // Duration is a derived display — single source of truth is the
    // (start_time, end_time) pair. The persisted `ManualSession.duration`
    // field is a denormalised cache that we always overwrite at save
    // time via this same computation, so reads of the cached value
    // never disagree with (end - start).
    let modal_duration_minutes = Signal::derive(move || {
        duration_from_start_end_minutes(&modal_start.get(), &modal_end.get())
    });

    let on_open_modal = move |session: ManualSession| {
        modal_session_id.set(Some(session.id.clone()));
        modal_start.set(session.start_time.clone());
        modal_end.set(session.end_time.clone());
        let fallback = match session.session_type {
            SessionType::Focus => t_string!(i18n, timer.mode_focus).to_string(),
            SessionType::Break => t_string!(i18n, timer.mode_break).to_string(),
            SessionType::LongBreak => t_string!(i18n, timer.mode_long_break).to_string(),
            SessionType::Custom => t_string!(i18n, daily.session_type_custom).to_string(),
        };
        modal_title.set(
            session
                .title
                .filter(|t| !t.trim().is_empty())
                .unwrap_or(fallback),
        );
        session_modal_open.set(true);
    };
    let on_close_modal = move |_| session_modal_open.set(false);

    view! {
        <div class="sessions-history-card">
            <div class="sessions-header">
                <h3>{t!(i18n, daily.history_title)}</h3>
                <div class="sessions-controls">
                    <button
                        id="export-sessions-btn"
                        class="export-btn"
                        title=move || t_string!(i18n, daily.history_export_title).to_string()
                        style:display=move || if scoped_sessions.with(Vec::is_empty) { "none" } else { "" }
                        on:click=move |_| {
                            // Export only the visible (selected-day) scope so the
                            // user gets what's on screen, matching the table.
                            let snapshot = scoped_sessions.get_untracked();
                            spawn_local(async move {
                                let path = commands::dialog_save(
                                    Some("sessions.csv".to_string()),
                                    vec![("CSV".to_string(), vec!["csv".to_string()])],
                                )
                                .await
                                .ok()
                                .flatten();
                                if let Some(p) = path {
                                    let _ = commands::export_sessions_csv(p, snapshot).await;
                                }
                            });
                        }
                    >
                        <i class="ri-download-line"></i>
                        " "
                        {t!(i18n, daily.history_export_label)}
                    </button>
                </div>
            </div>
            // Empty-state placeholder — matches the timeline's
            // `--card-bg-subtle` track look. The table (with its column
            // header row) is only useful when there's data to label.
            <div
                class="sessions-history-empty"
                id="sessions-history-empty"
                style:display=move || if scoped_sessions.with(Vec::is_empty) { "" } else { "none" }
            >
                {t!(i18n, daily.history_empty)}
            </div>
            <div
                class="sessions-table-container"
                style:display=move || if scoped_sessions.with(Vec::is_empty) { "none" } else { "" }
            >
                <table class="sessions-table" id="sessions-table">
                    <thead>
                        <tr>
                            <th>{t!(i18n, daily.history_col_time)}</th>
                            <th>{t!(i18n, daily.history_col_title)}</th>
                            <th>{t!(i18n, daily.history_col_duration)}</th>
                            <th>{t!(i18n, daily.history_col_actions)}</th>
                        </tr>
                    </thead>
                    <tbody id="sessions-table-body">
                        <For
                            each=move || {
                                // Most-recent first: sort by `created_at` ISO
                                // timestamp descending. Lexicographic order
                                // matches chronological order for the
                                // RFC 3339 / ISO 8601 strings written by the
                                // engine, so a string compare is sufficient.
                                let mut rows = scoped_sessions.get();
                                rows.sort_by(|a, b| b.created_at.cmp(&a.created_at));
                                rows
                            }
                            key=|row| row.id.clone()
                            children=move |row| {
                                let session_for_modal = row.clone();
                                let time_range = format!("{} – {}", row.start_time, row.end_time);
                                // Bind as a reactive closure so the
                                // `minutes_unit` translation re-resolves
                                // when the locale changes — `<For>`'s
                                // children fn runs once per row at mount,
                                // so an eager `t_string!` would freeze the
                                // label until the row is recreated.
                                let duration_minutes = row.duration;
                                let duration_text = move || {
                                    format!("{} {}", duration_minutes, t_string!(i18n, stats.minutes_unit))
                                };
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
                                                aria-label=move || t_string!(i18n, daily.history_edit_aria).to_string()
                                                on:click=move |_| on_open_modal(session_for_modal.clone())
                                            >{t!(i18n, daily.history_edit_button)}</button>
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
                        <h3 id="session-modal-title">{t!(i18n, daily.modal_title)}</h3>
                        <button
                            type="button"
                            id="close-session-modal"
                            class="close-btn"
                            aria-label=move || t_string!(i18n, daily.modal_close_aria).to_string()
                            on:click=on_close_modal
                        >"\u{00d7}"</button>
                    </div>
                    <div class="session-modal-body">
                        <label for="session-title">{t!(i18n, daily.modal_field_title)}</label>
                        <input
                            type="text"
                            id="session-title"
                            maxlength="120"
                            placeholder=move || t_string!(i18n, daily.modal_title_placeholder).to_string()
                            prop:value=move || modal_title.get()
                            on:input=move |ev| {
                                modal_title.set(event_target_value(&ev));
                            }
                        />
                        <label for="session-start-time">{t!(i18n, daily.modal_field_start)}</label>
                        <input
                            type="time"
                            id="session-start-time"
                            prop:value=move || modal_start.get()
                            on:input=move |ev| modal_start.set(event_target_value(&ev))
                        />
                        <label for="session-end-time">{t!(i18n, daily.modal_field_end)}</label>
                        <input
                            type="time"
                            id="session-end-time"
                            prop:value=move || modal_end.get()
                            on:input=move |ev| modal_end.set(event_target_value(&ev))
                        />
                        <label>{t!(i18n, daily.modal_field_duration)}</label>
                        <div
                            id="session-duration"
                            class="session-duration-readout"
                            aria-live="polite"
                        >
                            {move || format!(
                                "{} {}",
                                modal_duration_minutes.get(),
                                t_string!(i18n, stats.minutes_unit),
                            )}
                        </div>
                    </div>
                    <div class="modal-actions">
                        <button
                            type="button"
                            id="cancel-session-btn"
                            class="btn-secondary"
                            on:click=on_close_modal
                        >{t!(i18n, daily.modal_cancel)}</button>
                        <button
                            type="button"
                            id="delete-session-btn"
                            class="btn-danger"
                            on:click=move |_| {
                                if let Some(id) = modal_session_id.get_untracked() {
                                    sessions.update(|ss| ss.retain(|s| s.id != id));
                                    let snapshot = sessions.get_untracked();
                                    spawn_local(async move {
                                        let _ = commands::save_manual_sessions(snapshot).await;
                                    });
                                }
                                session_modal_open.set(false);
                            }
                        >{t!(i18n, daily.modal_delete)}</button>
                        <button
                            type="button"
                            id="save-session-btn"
                            class="btn-primary"
                            on:click=move |_| {
                                if let Some(id) = modal_session_id.get_untracked() {
                                    // Duration is derived from
                                    // (start, end). Clamp to the
                                    // [1, 180] minute window the
                                    // legacy modal enforced — beyond
                                    // that the saved row would render
                                    // outside the 24h timeline track.
                                    let start = modal_start.get_untracked();
                                    let end = modal_end.get_untracked();
                                    if parse_time_minutes(&start).is_none()
                                        || parse_time_minutes(&end).is_none()
                                    {
                                        return;
                                    }
                                    let raw_dur =
                                        duration_from_start_end_minutes(&start, &end);
                                    let clamped_dur = raw_dur.clamp(1, 180);
                                    let (end, clamped_dur) =
                                        end_time_from_start_duration(&start, clamped_dur);
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
                                    let snapshot = sessions.get_untracked();
                                    spawn_local(async move {
                                        let _ = commands::save_manual_sessions(snapshot).await;
                                    });
                                }
                                session_modal_open.set(false);
                            }
                        >{t!(i18n, daily.modal_save)}</button>
                    </div>
                </form>
            </div>
        </div>
    }
}
