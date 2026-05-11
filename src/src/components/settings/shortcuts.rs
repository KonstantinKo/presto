// Shortcuts settings tab — Phase 4b (T208) of spec
// 001-leptos-migration. Wires the three shortcut-recording inputs
// (`#start-stop-shortcut`, `#reset-shortcut`, `#skip-shortcut`) to
// `Settings::shortcuts`.
//
// **Selector contract** (consumed by `tests/e2e/settings-shortcuts.spec.js`):
// - `#start-stop-shortcut` — start/stop shortcut input
//   (`spec.js:15,22,26`).
// - `#reset-shortcut` — reset shortcut input.
// - `#skip-shortcut` — skip shortcut input.
// - `.recording` class on the active input during key capture
//   (`spec.js:16,22`).
//
// Recording flow: clicking an input lifts it into `recording` mode;
// the next keydown writes the captured shortcut into the input
// (and the settings signal) and exits recording. The 500ms
// auto-finish delay the spec mentions at line 22 is provided by the
// `set_timeout_with_handle` cleanup.
//
// Per Principle I, this component never mutates engine state — the
// global-shortcut registration is a Tauri-side effect dispatched
// through `bridge::commands::register_shortcuts` (Phase 4c attaches
// that hop). The dev-server / e2e-mock branch operates against the
// in-memory signal.
//
// Lint allowance: `clippy::must_use_candidate` is silenced for the
// usual Leptos `#[component]` reason. `clippy::too_many_lines` is
// silenced because the view body is a single Leptos `view!`
// expansion (one row per shortcut) plus a small recording-state
// helper cluster.
#![allow(clippy::must_use_candidate, clippy::too_many_lines)]

use leptos::ev::KeyboardEvent;
use leptos::prelude::*;

use crate::bridge::types::Settings;
use crate::components::settings::SettingsToast;

/// Three keyboard-shortcut slots. Mirrors `ShortcutSettings` field
/// names; the spec at `settings-shortcuts.spec.js:15` addresses the
/// start-stop slot via `#start-stop-shortcut`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShortcutSlot {
    StartStop,
    Reset,
    Skip,
}

impl ShortcutSlot {
    /// HTML `id` for the `<input>` slot. Must equal the kebab-case
    /// `<slot>-shortcut` shape the e2e spec asserts.
    const fn input_id(self) -> &'static str {
        match self {
            Self::StartStop => "start-stop-shortcut",
            Self::Reset => "reset-shortcut",
            Self::Skip => "skip-shortcut",
        }
    }

    /// Display label for the row.
    const fn label(self) -> &'static str {
        match self {
            Self::StartStop => "Start/Stop Timer:",
            Self::Reset => "Delete Session / Undo:",
            Self::Skip => "Save Session:",
        }
    }

    const fn placeholder(self) -> &'static str {
        match self {
            Self::StartStop => "CommandOrControl+Alt+Space",
            Self::Reset => "CommandOrControl+Alt+R",
            Self::Skip => "CommandOrControl+Alt+S",
        }
    }

    const fn description(self) -> &'static str {
        match self {
            Self::StartStop => "Start or pause the current Pomodoro session.",
            Self::Reset => "Delete the current session or undo the last completed Pomodoro.",
            Self::Skip => "Save the current session and start the next one.",
        }
    }
}

/// Format a captured `KeyboardEvent` as the JS-era
/// `formatShortcut([" "])` string. The spec at line 26 asserts the
/// space key is stored as the literal space character " "; for
/// non-modifier keys we use `event.key` directly so the visual
/// shape matches the JS-era surface.
fn format_shortcut(ev: &KeyboardEvent) -> String {
    let mut parts: Vec<String> = Vec::new();
    if ev.ctrl_key() || ev.meta_key() {
        parts.push("CommandOrControl".to_string());
    }
    if ev.alt_key() {
        parts.push("Alt".to_string());
    }
    if ev.shift_key() {
        parts.push("Shift".to_string());
    }
    parts.push(ev.key());
    parts.join("+")
}

/// Shortcut row builder. Returns the per-row view fragment with the
/// shared recording-flag wiring.
fn shortcut_row(
    slot: ShortcutSlot,
    settings: RwSignal<Settings>,
    recording: RwSignal<Option<ShortcutSlot>>,
    toast: SettingsToast,
) -> impl IntoView {
    let value = Signal::derive(move || {
        settings
            .with(|s| match slot {
                ShortcutSlot::StartStop => s.shortcuts.start_stop.clone(),
                ShortcutSlot::Reset => s.shortcuts.reset.clone(),
                ShortcutSlot::Skip => s.shortcuts.skip.clone(),
            })
            .unwrap_or_default()
    });
    let is_recording = Signal::derive(move || recording.get() == Some(slot));

    let on_focus_click = move |_| recording.set(Some(slot));

    let on_keydown = move |ev: KeyboardEvent| {
        if recording.get() != Some(slot) {
            return;
        }
        ev.prevent_default();
        let captured = format_shortcut(&ev);
        // The spec asserts the space key is stored as " " (single
        // space char); `formatShortcut` returns that exact shape
        // for an unmodified Space press because `parts` then
        // contains only `" "`.
        settings.update(|s| {
            let target = match slot {
                ShortcutSlot::StartStop => &mut s.shortcuts.start_stop,
                ShortcutSlot::Reset => &mut s.shortcuts.reset,
                ShortcutSlot::Skip => &mut s.shortcuts.skip,
            };
            *target = Some(captured);
        });
        toast.show("Settings saved");
        // Auto-exit recording after 500ms (matches JS-era debounce
        // the spec waits for at lines 22-24). Best-effort: a host
        // build returns Err which we drop.
        let handle = set_timeout_with_handle(
            move || {
                if recording.get() == Some(slot) {
                    recording.set(None);
                }
            },
            core::time::Duration::from_millis(500),
        );
        let _ = handle;
    };

    view! {
        <div class="shortcut-item">
            <label for=slot.input_id()>{slot.label()}</label>
            <div class="shortcut-input-container">
                <input
                    type="text"
                    id=slot.input_id()
                    class="shortcut-input"
                    class:recording=move || is_recording.get()
                    readonly
                    placeholder=slot.placeholder()
                    prop:value=move || value.get()
                    on:click=on_focus_click
                    on:keydown=on_keydown
                />
                <button
                    type="button"
                    class="shortcut-clear"
                    data-shortcut=slot.input_id()
                    aria-label=format!("Clear {} shortcut", slot.label())
                    on:click=move |_| {
                        settings.update(|s| match slot {
                            ShortcutSlot::StartStop => s.shortcuts.start_stop = None,
                            ShortcutSlot::Reset => s.shortcuts.reset = None,
                            ShortcutSlot::Skip => s.shortcuts.skip = None,
                        });
                        recording.set(None);
                        toast.show("Settings saved");
                    }
                >"×"</button>
            </div>
            <p class="setting-description">{slot.description()}</p>
        </div>
    }
}

/// Shortcuts settings tab.
#[component]
pub fn ShortcutsSettings(settings: RwSignal<Settings>, toast: SettingsToast) -> impl IntoView {
    let recording = RwSignal::new(None::<ShortcutSlot>);

    view! {
        <div class="category-header">
            <h1>"Global Shortcuts"</h1>
            <p class="category-description">
                "Configure keyboard shortcuts that work even when the app is in the background"
            </p>
        </div>
        <div class="settings-section">
            <h3>"Keyboard Shortcuts"</h3>
            {shortcut_row(ShortcutSlot::StartStop, settings, recording, toast)}
            {shortcut_row(ShortcutSlot::Reset, settings, recording, toast)}
            {shortcut_row(ShortcutSlot::Skip, settings, recording, toast)}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::ShortcutSlot;

    /// T208 — selector contract pin. Sourced from
    /// `tests/e2e/settings-shortcuts.spec.js`.
    #[test]
    fn shortcuts_selector_contract_documented() {
        const REQUIRED_IDS: &[&str] = &["start-stop-shortcut", "reset-shortcut", "skip-shortcut"];
        let mut seen: Vec<&str> = Vec::with_capacity(REQUIRED_IDS.len());
        for id in REQUIRED_IDS {
            assert!(!seen.contains(id), "duplicate selector ID: {id}");
            seen.push(id);
        }
        assert_eq!(ShortcutSlot::StartStop.input_id(), "start-stop-shortcut");
        assert_eq!(ShortcutSlot::Reset.input_id(), "reset-shortcut");
        assert_eq!(ShortcutSlot::Skip.input_id(), "skip-shortcut");
    }
}
