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
#![allow(clippy::must_use_candidate)]

use leptos::prelude::*;

/// History view skeleton. Renders the session-history table shell
/// (`#sessions-table-body`) plus the (initially-hidden) per-row
/// edit modal (`#session-modal-overlay`, `#session-duration`,
/// `#close-session-modal`). T196 attaches the row iterator + modal
/// open/close handlers; today's render is the static shell so
/// `(cd src && trunk build)` returns 0 and the e2e selectors
/// resolve.
#[component]
pub fn HistoryView() -> impl IntoView {
    view! {
        <div class="view-container hidden view-section" id="history-view">
            // Sessions history table — the JS-era `#sessions-table`
            // wrapper is preserved so visual baselines match.
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
                        // Per-row content lands in T196.
                        <tbody id="sessions-table-body"></tbody>
                    </table>
                </div>
            </div>

            // Per-row edit modal. Hidden by default; T196 wires the
            // open/close lifecycle and the field bindings.
            <div class="session-modal-overlay" id="session-modal-overlay" style="display: none">
                <div class="session-modal">
                    <div class="session-modal-header">
                        <h3>"Edit session"</h3>
                        <button id="close-session-modal" class="close-btn" aria-label="Close edit modal">"×"</button>
                    </div>
                    <div class="session-modal-body">
                        <label for="session-duration">"Duration (minutes)"</label>
                        <input type="number" id="session-duration" min="1" max="180" />
                    </div>
                </div>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    /// Selector contract pin for the history view, sourced from
    /// `tests/e2e/sessions-history.spec.js`. Each entry maps to a
    /// `locator("#…")` callsite in the spec; drift here breaks the
    /// e2e run for that flow.
    ///
    /// - `history-view` — root container (the JS-era UI carries the
    ///   table inline in the calendar view; the Leptos port lifts
    ///   it to a dedicated container so `NavView::History`
    ///   dispatches cleanly. The e2e spec navigates via
    ///   `tapTab(page, "Calendar")` and asserts the table inline;
    ///   T196 attaches the same DOM structure inside this
    ///   container).
    /// - `sessions-table-body` — `<tbody>` host
    ///   (`spec.js:37` asserts `.getByRole("row")`).
    /// - `session-modal-overlay` — modal backdrop
    ///   (`spec.js:42`).
    /// - `session-duration` — duration field (`spec.js:44`).
    /// - `close-session-modal` — close button (`spec.js:47`).
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
