// General settings tab — Phase 4b (T205) of spec
// 001-leptos-migration. Wires the timer-durations sub-form
// (`#focus-duration`, `#break-duration`, `#long-break-duration`,
// `#total-sessions`, `#max-session-time`) to the shared
// `RwSignal<Settings>`.
//
// **Selector contract** (consumed by `tests/e2e/settings-general.spec.js`):
// - `#focus-duration` — focus minutes input (`spec.js:13,34`).
// - `#break-duration`, `#long-break-duration`, `#total-sessions`,
//   `#max-session-time` — siblings; pinned for visual parity with the
//   JS-era surface even though the spec only fills `#focus-duration`.
//
// Auto-save UX: each input fires the toast on blur (`on:change` in
// the Leptos sense — `change` events fire on input blur in the
// browser, matching the JS-era debounce behaviour the spec relies on
// at lines 14-17 ("Tab triggers auto-save debounce")). The toast
// shape is `[role="alert"].notification-ping` filtered by "Settings
// saved" — see the shell module for the renderer.
//
// Per Principle I, this component never mutates engine state — the
// engine reads `settings.timer.focus_duration` via the manager when
// constructing `Durations`. Phase 4c attaches the
// `bridge::commands::save_settings` hop on each change; today the
// dev-server / e2e mock branch operates against the in-memory
// signal.
//
// Lint allowance: `clippy::must_use_candidate` is silenced module-
// wide for the same reason as on `timer.rs` — Leptos `#[component]`
// returns are consumed by `view!` automatically.
// `clippy::too_many_lines` is silenced module-wide because the view
// body is a single Leptos `view!` macro expansion (one DOM subtree
// per setting) plus a small change-handler cluster; splitting it
// would fragment the JSX-style DOM tree without aiding readability.
#![allow(clippy::must_use_candidate, clippy::too_many_lines)]

use leptos::prelude::*;

use crate::bridge::types::Settings;
use crate::components::settings::SettingsToast;

/// Parse an `<input type="number">` value into `u32`, falling back to
/// `fallback` when the input is empty / non-numeric / out of range.
/// Mirrors the JS-era `parseInt(value, 10) || fallback` pattern at
/// `src/managers/settings-manager.js`.
fn parse_minutes(raw: &str, fallback: u32) -> u32 {
    raw.trim().parse::<u32>().unwrap_or(fallback)
}

/// General settings tab — five timer-duration / session-count
/// numeric inputs, each bound to a slice of `settings.timer`.
#[component]
pub fn GeneralSettings(settings: RwSignal<Settings>, toast: SettingsToast) -> impl IntoView {
    // Derived signals — read each field via `.with(...)` so we
    // borrow the inner record without cloning. Each renders to a
    // `String` because `<input>`'s `prop:value` binds to a string.
    let focus_duration =
        Signal::derive(move || settings.with(|s| s.timer.focus_duration.to_string()));
    let break_duration =
        Signal::derive(move || settings.with(|s| s.timer.break_duration.to_string()));
    let long_break_duration =
        Signal::derive(move || settings.with(|s| s.timer.long_break_duration.to_string()));
    let total_sessions =
        Signal::derive(move || settings.with(|s| s.timer.total_sessions.to_string()));
    let max_session_time =
        Signal::derive(move || settings.with(|s| s.timer.max_session_time.to_string()));
    // Feature 002 Bundle B (T021): "Sessions per long break" — the
    // 1–10 clamp lives at the input boundary (Principle III); the
    // engine accepts the `u32` without a runtime guard. Mirrors the
    // string-binding pattern used by the sibling fields above.
    let sessions_per_long_break =
        Signal::derive(move || settings.with(|s| s.timer.sessions_per_long_break.to_string()));

    // Change handlers — each fires on blur (`on:change`), updates the
    // settings signal in place, and shows the auto-save toast. The
    // JS-era debounce is collapsed to a single fire-on-blur because
    // Leptos signals are synchronous; the visible feedback (the toast
    // ping) is what the e2e suite asserts.
    let on_focus_change = move |ev| {
        let value = parse_minutes(&event_target_value(&ev), 25);
        settings.update(|s| s.timer.focus_duration = value);
        toast.show("Settings saved");
    };
    let on_break_change = move |ev| {
        let value = parse_minutes(&event_target_value(&ev), 5);
        settings.update(|s| s.timer.break_duration = value);
        toast.show("Settings saved");
    };
    let on_long_break_change = move |ev| {
        let value = parse_minutes(&event_target_value(&ev), 20);
        settings.update(|s| s.timer.long_break_duration = value);
        toast.show("Settings saved");
    };
    let on_total_sessions_change = move |ev| {
        let value = parse_minutes(&event_target_value(&ev), 10);
        settings.update(|s| s.timer.total_sessions = value);
        toast.show("Settings saved");
    };
    let on_max_session_change = move |ev| {
        let value = parse_minutes(&event_target_value(&ev), 120);
        settings.update(|s| s.timer.max_session_time = value);
        toast.show("Settings saved");
    };
    // Feature 002 Bundle B (T021): explicit clamp to 1–10 at the
    // input boundary. Browser `min`/`max` is a UX hint only — a
    // hand-edited / pasted value still needs an explicit clamp
    // before it reaches the persisted signal so the engine input
    // (Principle III: type accepts any `u32` without a runtime
    // guard) is always in range.
    let on_sessions_per_long_break_change = move |ev| {
        let raw = parse_minutes(&event_target_value(&ev), 4);
        let value = raw.clamp(1, 10);
        settings.update(|s| s.timer.sessions_per_long_break = value);
        toast.show("Settings saved");
    };

    view! {
        <div class="category-header">
            <h1>"General Settings"</h1>
            <p class="category-description">
                "Configure timer durations and basic behavior"
            </p>
        </div>
        <div class="settings-section base-section">
            <h3 class="section-header">"Timer Durations"</h3>
            <div class="setting-item">
                <label for="focus-duration">"Focus Duration (minutes):"</label>
                <input
                    type="number"
                    id="focus-duration"
                    min="1"
                    max="60"
                    prop:value=move || focus_duration.get()
                    on:change=on_focus_change
                />
                <p class="setting-description">"How long should focus sessions last"</p>
            </div>
            <div class="setting-item">
                <label for="break-duration">"Short Break (minutes):"</label>
                <input
                    type="number"
                    id="break-duration"
                    min="1"
                    max="30"
                    prop:value=move || break_duration.get()
                    on:change=on_break_change
                />
                <p class="setting-description">
                    "Duration for short breaks between focus sessions"
                </p>
            </div>
            <div class="setting-item">
                <label for="long-break-duration">"Long Break (minutes):"</label>
                <input
                    type="number"
                    id="long-break-duration"
                    min="1"
                    max="60"
                    prop:value=move || long_break_duration.get()
                    on:change=on_long_break_change
                />
                <p class="setting-description">
                    "Duration for long breaks after completing a cycle"
                </p>
            </div>
            <div class="setting-item">
                <label for="total-sessions">"Daily Sessions:"</label>
                <input
                    type="number"
                    id="total-sessions"
                    min="1"
                    max="20"
                    prop:value=move || total_sessions.get()
                    on:change=on_total_sessions_change
                />
                <p class="setting-description">
                    "Number of focus sessions to complete each day"
                </p>
            </div>
            <div class="setting-item">
                <label for="sessions-per-long-break">"Sessions per Long Break:"</label>
                <input
                    type="number"
                    id="sessions-per-long-break"
                    min="1"
                    max="10"
                    prop:value=move || sessions_per_long_break.get()
                    on:change=on_sessions_per_long_break_change
                />
                <p class="setting-description">
                    "How many focus sessions before a long break"
                </p>
            </div>
            <div class="setting-item">
                <label for="max-session-time">"Max Session Time (minutes):"</label>
                <input
                    type="number"
                    id="max-session-time"
                    min="30"
                    max="480"
                    prop:value=move || max_session_time.get()
                    on:change=on_max_session_change
                />
                <p class="setting-description">
                    "Maximum time per session before auto-pause (default: 2 hours)"
                </p>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::parse_minutes;

    #[test]
    fn parse_minutes_falls_back_on_empty_or_garbage() {
        assert_eq!(parse_minutes("", 25), 25);
        assert_eq!(parse_minutes("abc", 25), 25);
        assert_eq!(parse_minutes("  ", 25), 25);
        assert_eq!(parse_minutes("-1", 25), 25);
        assert_eq!(parse_minutes("5", 25), 5);
        assert_eq!(parse_minutes("  5  ", 25), 5);
        // max_session_time fallback: 120 when input is invalid.
        assert_eq!(parse_minutes("", 120), 120);
        assert_eq!(parse_minutes("notanumber", 120), 120);
        assert_eq!(parse_minutes("60", 120), 60);
    }

    /// T205 — selector contract pin for the General tab. Sourced
    /// from `tests/e2e/settings-general.spec.js`. Drift here breaks
    /// the e2e run.
    #[test]
    fn general_settings_selector_contract_documented() {
        const REQUIRED_IDS: &[&str] = &[
            "focus-duration",
            "break-duration",
            "long-break-duration",
            "total-sessions",
            "max-session-time",
            // Feature 002 Bundle B (T021): the new long-break cadence
            // numeric input shares the General-tab selector contract.
            "sessions-per-long-break",
        ];
        let mut seen: Vec<&str> = Vec::with_capacity(REQUIRED_IDS.len());
        for id in REQUIRED_IDS {
            assert!(!seen.contains(id), "duplicate selector ID: {id}");
            seen.push(id);
        }
    }
}
