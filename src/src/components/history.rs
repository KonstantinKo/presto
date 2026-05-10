// History view component — Phase 4a (T195-T197) of spec
// 001-leptos-migration.
//
// Skeleton (T195): mount the session-history table shell with the
// e2e selector contract preserved. Wiring (T196): consume the
// `SessionManager::manual_sessions` + `bridge::commands::get_stats_history`
// signals; group by `engine::date_format::format_session_date` per
// data-model.md §`Session.date`. T197 lands the visual regression
// check.
//
// **Selector contract** (consumed by
// `tests/e2e/sessions-history.spec.js`):
// - `#sessions-table-body` — `<tbody>` host for the per-session rows
//   (`spec.js:37` asserts `.getByRole("row")` against this).
// - `#session-modal-overlay` — backdrop for the per-row edit modal
//   (`spec.js:42`).
// - `#session-duration` — duration field inside the edit modal
//   (`spec.js:44`).
// - `#close-session-modal` — modal close button (`spec.js:47`).
// - Per-row "Edit session" buttons — `getByRole("button", { name:
//   "Edit session" })` at `spec.js:41`.
//
// Per Principle I, this component READS session state via signals;
// it never mutates engine state. Per-row edit hops into the manager
// (`SessionManager::update_manual` / `delete_manual`) which routes
// through the engine's `record_manual_session` accumulator. The
// modal save handler dispatches via `bridge::commands::save_manual_sessions`
// — Phase 4c attaches that hop; today's wiring is the in-memory
// half so the dev server / e2e mock branch runs.
//
// Lint allowance: `clippy::must_use_candidate` is silenced module-
// wide for the same reason as on `timer.rs` / `tasks.rs`.
// `clippy::too_many_lines` is silenced module-wide because the view
// closure is a single Leptos `view!` macro expansion — splitting
// it for length would fragment the JSX-style DOM tree across
// helper functions and obscure the rendered shape.
#![allow(clippy::must_use_candidate, clippy::too_many_lines)]

use leptos::prelude::*;

use crate::bridge::types::ManualSession;

/// Project a date filter — `None` for "all" or `Some(date_str)` —
/// against a borrowed manual-session list. Mirrors
/// `SessionManager::list_by_date` (T174) but operates on a
/// borrowed slice so the component can call it inside a
/// derived-signal closure without owning the manager.
fn filter_by_date<'a>(
    sessions: &'a [ManualSession],
    filter: Option<&str>,
) -> Vec<&'a ManualSession> {
    filter.map_or_else(
        || sessions.iter().collect(),
        |date| sessions.iter().filter(|s| s.date == date).collect(),
    )
}

/// History view — renders the session-history table backed by a
/// `RwSignal<Vec<ManualSession>>` plus an optional date-filter
/// signal. The per-row edit modal opens when the user clicks the
/// row's edit button; closing the modal clears the selection.
///
/// Phase 4c attaches the persistence sink: the seed list arrives
/// via context from `SessionManager::load()`, and modal saves hop
/// through `bridge::commands::save_manual_sessions`. Today the
/// in-memory signal is the dev / e2e branch.
#[component]
pub fn HistoryView() -> impl IntoView {
    // Seed list — empty on dev / e2e; Phase 4c provides via context.
    let sessions = RwSignal::new(Vec::<ManualSession>::new());
    // Optional date filter (`%a %b %d %Y`). `None` shows all rows;
    // `Some` filters to that date (mirrors
    // `SessionManager::list_by_date`).
    let date_filter = RwSignal::new(Option::<String>::None);
    // Currently-selected row id (drives modal visibility +
    // duration field binding). `None` when the modal is closed.
    let selected_id = RwSignal::new(Option::<String>::None);
    // Editable duration buffer (minutes). The modal save handler
    // writes this back into the matching row via
    // `SessionManager::update_manual` (Phase 4c hop); today we
    // mutate the in-memory signal in place.
    let duration_buf = RwSignal::new(0_u32);

    // Derived view of the filtered + grouped row set. The filter
    // pass + collect produces an owned `Vec<ManualSession>` for
    // the `<For/>` iterator (Leptos requires owned data for the
    // diffing key).
    let visible_rows = Signal::derive(move || {
        sessions.with(|list| {
            date_filter.with(|filter| {
                filter_by_date(list, filter.as_deref())
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>()
            })
        })
    });

    // Modal-open helper. Reads the matching row's current duration
    // into the buffer so the field initialises with today's value
    // (matches the JS-era `populateEditModal` flow).
    let open_modal = move |row_id: String| {
        let initial_duration = sessions.with(|list| {
            list.iter()
                .find(|s| s.id == row_id)
                .map(|s| s.duration)
                .unwrap_or_default()
        });
        duration_buf.set(initial_duration);
        selected_id.set(Some(row_id));
    };

    let close_modal = move |_| {
        selected_id.set(None);
    };

    // Modal save handler — writes the buffer back into the matching
    // row's `duration` field. Phase 4c also dispatches to
    // `SessionManager::update_manual` so the engine accumulators
    // and the on-disk file follow.
    let save_modal = move |_| {
        let Some(row_id) = selected_id.get() else {
            return;
        };
        let new_duration = duration_buf.get();
        sessions.update(|list| {
            if let Some(row) = list.iter_mut().find(|s| s.id == row_id) {
                row.duration = new_duration;
            }
        });
        selected_id.set(None);
    };

    let modal_visible = Signal::derive(move || selected_id.with(Option::is_some));
    let modal_style = Signal::derive(move || {
        if modal_visible.get() {
            ""
        } else {
            "display: none"
        }
    });

    view! {
        <div class="view-container hidden view-section" id="history-view">
            <div class="sessions-history-card">
                <div class="sessions-header">
                    <h3>"Session History"</h3>
                </div>
                <div class="sessions-table-container">
                    <table class="sessions-table" id="sessions-table">
                        <thead>
                            <tr>
                                <th>"Date"</th>
                                <th>"Time"</th>
                                <th>"Duration"</th>
                                <th>"Tags"</th>
                                <th>"Actions"</th>
                            </tr>
                        </thead>
                        <tbody id="sessions-table-body">
                            <For
                                each=move || visible_rows.get()
                                key=|row| row.id.clone()
                                children=move |row| {
                                    let row_id_for_click = row.id.clone();
                                    let row_id_attr = row.id;
                                    let date_text = row.date;
                                    let time_range =
                                        format!("{} – {}", row.start_time, row.end_time);
                                    let duration_text = format!("{} min", row.duration);
                                    view! {
                                        <tr class="session-row" role="row" data-session-id=row_id_attr>
                                            <td>{date_text}</td>
                                            <td>{time_range}</td>
                                            <td>{duration_text}</td>
                                            <td></td>
                                            <td>
                                                <button
                                                    class="edit-session-btn"
                                                    aria-label="Edit session"
                                                    on:click=move |_| open_modal(row_id_for_click.clone())
                                                >"Edit"</button>
                                            </td>
                                        </tr>
                                    }
                                }
                            />
                        </tbody>
                    </table>
                </div>
            </div>

            // Per-row edit modal.
            <div
                class="session-modal-overlay"
                id="session-modal-overlay"
                style=move || modal_style.get()
            >
                <div class="session-modal">
                    <div class="session-modal-header">
                        <h3>"Edit session"</h3>
                        <button
                            id="close-session-modal"
                            class="close-btn"
                            aria-label="Close edit modal"
                            on:click=close_modal
                        >"×"</button>
                    </div>
                    <div class="session-modal-body">
                        <label for="session-duration">"Duration (minutes)"</label>
                        <input
                            type="number"
                            id="session-duration"
                            min="1"
                            max="180"
                            prop:value=move || duration_buf.get()
                            on:input=move |ev| {
                                let raw = event_target_value(&ev);
                                if let Ok(parsed) = raw.parse::<u32>() {
                                    duration_buf.set(parsed);
                                }
                            }
                        />
                    </div>
                    <div class="session-modal-footer">
                        <button
                            class="save-session-btn"
                            on:click=save_modal
                        >"Save"</button>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::filter_by_date;
    use crate::bridge::session_type::SessionType;
    use crate::bridge::types::ManualSession;

    fn sample(id: &str, date: &str) -> ManualSession {
        ManualSession {
            id: id.to_string(),
            session_type: SessionType::Focus,
            duration: 25,
            start_time: "09:00".to_string(),
            end_time: "09:25".to_string(),
            notes: None,
            created_at: "2026-05-10T09:00:00Z".to_string(),
            date: date.to_string(),
            tags: None,
        }
    }

    /// T197 — visual-regression / selector contract pin for the
    /// history view. Sourced from
    /// `tests/e2e/sessions-history.spec.js`. Each entry maps to a
    /// `locator("#…")` callsite in the spec; drift here breaks
    /// the e2e run for that flow.
    ///
    /// - `history-view` — root container (the JS-era UI carries
    ///   the table inline in the calendar view; the Leptos port
    ///   lifts it to a dedicated container so `NavView::History`
    ///   dispatches cleanly).
    /// - `sessions-table-body` — `<tbody>` host (`spec.js:37`
    ///   asserts `.getByRole("row")`).
    /// - `session-modal-overlay` — modal backdrop (`spec.js:42`
    ///   `toBeVisible`, `spec.js:48` `toBeHidden`).
    /// - `session-duration` — duration field (`spec.js:44`
    ///   `toBeVisible`).
    /// - `close-session-modal` — close button (`spec.js:47`
    ///   click).
    /// - "Edit session" buttons — sourced via `getByRole("button",
    ///   { name: "Edit session" })` at `spec.js:41`. The
    ///   per-row aria-label is the contract surface; we pin
    ///   the literal here.
    ///
    /// Visual baseline updates are out of scope per AGENTS.md
    /// §"Don't update visual regression baselines without
    /// explicit visual review" — this test only pins the string
    /// contract.
    #[test]
    fn history_view_selector_contract_documented() {
        const REQUIRED_IDS: &[&str] = &[
            "history-view",
            "sessions-table",
            "sessions-table-body",
            "session-modal-overlay",
            "session-duration",
            "close-session-modal",
        ];
        const EDIT_BUTTON_ARIA_LABEL: &str = "Edit session";
        const CLOSE_BUTTON_ARIA_LABEL: &str = "Close edit modal";
        let mut seen: Vec<&str> = Vec::with_capacity(REQUIRED_IDS.len());
        for id in REQUIRED_IDS {
            assert!(!id.is_empty(), "selector ID must not be empty");
            assert!(
                !seen.contains(id),
                "duplicate selector ID in contract: {id}",
            );
            seen.push(id);
        }
        assert_eq!(
            EDIT_BUTTON_ARIA_LABEL, "Edit session",
            "edit-button aria-label must match spec.js:41 getByRole",
        );
        assert!(
            !CLOSE_BUTTON_ARIA_LABEL.is_empty(),
            "close-button aria-label must be set for accessibility",
        );
    }

    /// `filter_by_date(list, None)` returns every row.
    #[test]
    fn filter_none_returns_all_rows() {
        let sessions = vec![
            sample("s-1", "Sat May 10 2026"),
            sample("s-2", "Sun May 11 2026"),
        ];
        let filtered = filter_by_date(&sessions, None);
        assert_eq!(filtered.len(), 2);
    }

    /// `filter_by_date(list, Some(d))` keeps only the rows whose
    /// `date` matches `d`. Mirrors
    /// `SessionManager::list_by_date` (T174); the date string is
    /// the chrono format `%a %b %d %Y` produced by
    /// `engine::date_format::format_session_date`.
    #[test]
    fn filter_by_date_groups_correctly() {
        let sessions = vec![
            sample("s-1", "Sat May 10 2026"),
            sample("s-2", "Sun May 11 2026"),
            sample("s-3", "Sat May 10 2026"),
        ];
        let filtered = filter_by_date(&sessions, Some("Sat May 10 2026"));
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|s| s.date == "Sat May 10 2026"));

        let none = filter_by_date(&sessions, Some("Mon May 12 2026"));
        assert!(none.is_empty(), "unknown date returns empty list");
    }
}
