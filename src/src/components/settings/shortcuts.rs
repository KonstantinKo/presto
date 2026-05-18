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
use leptos_i18n::{t, t_string};

use crate::bridge::types::Settings;
use crate::components::settings::SettingsToast;
use crate::i18n::i18n::use_i18n;

/// Keyboard-shortcut slots. Mirrors `ShortcutSettings` field names;
/// the spec at `settings-shortcuts.spec.js:15` addresses the
/// start-stop slot via `#start-stop-shortcut`.
///
/// Feature 007: `Abort` slot added as the fourth row (FR-018).
/// Default binding is unbound per FR-019 — the slot ships with an
/// empty input, the user opts in.
///
/// R-003 note: `Reset` is the legacy alias for `Abort` — same engine
/// action, different binding row + default. The Undo-last-Pomodoro
/// affordance was removed by feature 006 (FR-028); the variant name
/// stays for backwards-compat with the `settings.json` field name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShortcutSlot {
    StartStop,
    Reset,
    Skip,
    Abort,
}

impl ShortcutSlot {
    /// HTML `id` for the `<input>` slot. Must equal the kebab-case
    /// `<slot>-shortcut` shape the e2e spec asserts.
    const fn input_id(self) -> &'static str {
        match self {
            Self::StartStop => "start-stop-shortcut",
            Self::Reset => "reset-shortcut",
            Self::Skip => "skip-shortcut",
            Self::Abort => "abort-shortcut",
        }
    }

    /// Display label for the row.
    ///
    /// Feature 005: kept on the impl as the English source-of-truth
    /// for any future audit; the rendered view dispatches via `t!`
    /// over the slot variant in `shortcut_row`.
    #[allow(dead_code)]
    const fn label(self) -> &'static str {
        match self {
            Self::StartStop => "Start/Stop Timer:",
            // R-003 fix: Reset is the legacy alias for Abort. Label
            // updated to match actual behaviour (the engine has no
            // `reset()` method since feature 006 removed it).
            Self::Reset => "Reset session:",
            Self::Skip => "Skip:",
            Self::Abort => "Abort Session:",
        }
    }

    const fn placeholder(self) -> &'static str {
        match self {
            Self::StartStop => "CommandOrControl+Alt+Space",
            Self::Reset => "CommandOrControl+Alt+R",
            Self::Skip => "CommandOrControl+Alt+S",
            // Feature 007: no canonical default — abort is opt-in. The
            // placeholder hints at the shape the user would type.
            Self::Abort => "CommandOrControl+Alt+W",
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
///
/// Feature 005: each row's localised label / description is dispatched
/// via a `match` over the slot variant so the `t!` macro can keep its
/// compile-time-checked static key paths. The "Clear ... shortcut"
/// aria-label is interpolated via `t_string!` with the slot's label
/// as the `{{ name }}` argument.
fn shortcut_row(
    slot: ShortcutSlot,
    settings: RwSignal<Settings>,
    recording: RwSignal<Option<ShortcutSlot>>,
    toast: SettingsToast,
) -> impl IntoView {
    let i18n = use_i18n();
    let value = Signal::derive(move || {
        settings
            .with(|s| match slot {
                ShortcutSlot::StartStop => s.shortcuts.start_stop.clone(),
                ShortcutSlot::Reset => s.shortcuts.reset.clone(),
                ShortcutSlot::Skip => s.shortcuts.skip.clone(),
                ShortcutSlot::Abort => s.shortcuts.abort.clone(),
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
                ShortcutSlot::Abort => &mut s.shortcuts.abort,
            };
            *target = Some(captured);
        });
        toast.show(t_string!(i18n, settings.toast_saved).to_string());
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

    let label_view = move || match slot {
        ShortcutSlot::StartStop => t!(i18n, settings.shortcuts.label_start_stop).into_any(),
        ShortcutSlot::Reset => t!(i18n, settings.shortcuts.label_reset).into_any(),
        ShortcutSlot::Skip => t!(i18n, settings.shortcuts.label_skip).into_any(),
        ShortcutSlot::Abort => t!(i18n, settings.shortcuts.label_abort).into_any(),
    };
    let description_view = move || match slot {
        ShortcutSlot::StartStop => t!(i18n, settings.shortcuts.desc_start_stop).into_any(),
        ShortcutSlot::Reset => t!(i18n, settings.shortcuts.desc_reset).into_any(),
        ShortcutSlot::Skip => t!(i18n, settings.shortcuts.desc_skip).into_any(),
        ShortcutSlot::Abort => t!(i18n, settings.shortcuts.desc_abort).into_any(),
    };
    let clear_aria = move || {
        let label_text: String = match slot {
            ShortcutSlot::StartStop => {
                t_string!(i18n, settings.shortcuts.label_start_stop).to_string()
            }
            ShortcutSlot::Reset => t_string!(i18n, settings.shortcuts.label_reset).to_string(),
            ShortcutSlot::Skip => t_string!(i18n, settings.shortcuts.label_skip).to_string(),
            ShortcutSlot::Abort => t_string!(i18n, settings.shortcuts.label_abort).to_string(),
        };
        t_string!(i18n, settings.shortcuts.clear_aria, name = label_text)
    };

    view! {
        <div class="shortcut-item">
            <label for=slot.input_id()>{label_view}</label>
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
                    aria-label=clear_aria
                    on:click=move |_| {
                        settings.update(|s| match slot {
                            ShortcutSlot::StartStop => s.shortcuts.start_stop = None,
                            ShortcutSlot::Reset => s.shortcuts.reset = None,
                            ShortcutSlot::Skip => s.shortcuts.skip = None,
                            ShortcutSlot::Abort => s.shortcuts.abort = None,
                        });
                        recording.set(None);
                        toast.show(t_string!(i18n, settings.toast_saved).to_string());
                    }
                >"×"</button>
            </div>
            <p class="setting-description">{description_view}</p>
        </div>
    }
}

/// Shortcuts settings tab.
#[component]
pub fn ShortcutsSettings(settings: RwSignal<Settings>, toast: SettingsToast) -> impl IntoView {
    let i18n = use_i18n();
    let recording = RwSignal::new(None::<ShortcutSlot>);

    view! {
        <div class="category-header">
            <h1>{t!(i18n, settings.shortcuts.title)}</h1>
            <p class="category-description">{t!(i18n, settings.shortcuts.description)}</p>
        </div>
        <div class="settings-section">
            <h3 class="section-header">{t!(i18n, settings.shortcuts.section_header)}</h3>
            {shortcut_row(ShortcutSlot::StartStop, settings, recording, toast)}
            {shortcut_row(ShortcutSlot::Reset, settings, recording, toast)}
            {shortcut_row(ShortcutSlot::Skip, settings, recording, toast)}
            // Feature 007 (T025, FR-018): fourth row — Abort. Default
            // binding is unbound (FR-019); the user opts in.
            {shortcut_row(ShortcutSlot::Abort, settings, recording, toast)}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::ShortcutSlot;

    /// T208 — selector contract pin. Sourced from
    /// `tests/e2e/settings-shortcuts.spec.js`.
    /// Feature 007 (T025): extended to include `#abort-shortcut`.
    #[test]
    fn shortcuts_selector_contract_documented() {
        const REQUIRED_IDS: &[&str] = &[
            "start-stop-shortcut",
            "reset-shortcut",
            "skip-shortcut",
            "abort-shortcut",
        ];
        let mut seen: Vec<&str> = Vec::with_capacity(REQUIRED_IDS.len());
        for id in REQUIRED_IDS {
            assert!(!seen.contains(id), "duplicate selector ID: {id}");
            seen.push(id);
        }
        assert_eq!(ShortcutSlot::StartStop.input_id(), "start-stop-shortcut");
        assert_eq!(ShortcutSlot::Reset.input_id(), "reset-shortcut");
        assert_eq!(ShortcutSlot::Skip.input_id(), "skip-shortcut");
        assert_eq!(ShortcutSlot::Abort.input_id(), "abort-shortcut");
    }
}
