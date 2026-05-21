// Global mute toggle — silences ticks, chimes, and ambient music.
//
// Two surfaces:
//   * `Muted` — Copy newtype around `RwSignal<bool>`, provided via
//     context at the App root so any view can read/toggle and so the
//     ambient-audio Effect picks up changes reactively.
//   * `is_muted()` — non-reactive snapshot for callers that can't (or
//     shouldn't) subscribe — `play_chime` and `play_metronome_tick`
//     fire from imperative call sites, not Effects, so they read the
//     atomic mirror instead of subscribing to the signal.
//
// The atomic mirror is kept in sync by an Effect installed in
// `App` (see `provide_mute_state`). Persistence is best-effort via
// `localStorage` key `presto.muted` — same flavour as other UI prefs
// in the JS-era code (no Tauri command, no cold-start race).

use std::sync::atomic::{AtomicBool, Ordering};

use leptos::prelude::*;

#[cfg(target_arch = "wasm32")]
const STORAGE_KEY: &str = "presto.muted";

static MUTED_ATOMIC: AtomicBool = AtomicBool::new(false);

/// Context carrier — `Copy` so descendants can `use_context` without
/// borrow gymnastics. The inner signal is the source of truth; the
/// atomic in this module mirrors it for non-reactive readers.
#[derive(Clone, Copy)]
pub struct Muted(pub RwSignal<bool>);

/// Non-reactive snapshot. Use from callers that fire outside a
/// reactive scope (`play_chime`, `play_metronome_tick`).
#[cfg(target_arch = "wasm32")]
#[inline]
pub(super) fn is_muted() -> bool {
    MUTED_ATOMIC.load(Ordering::Relaxed)
}

/// Create the mute signal, hydrate from `localStorage`, install the
/// atomic-mirror + persistence Effect, and return the `Muted` wrapper
/// ready to drop into `provide_context`.
///
/// Idempotent only at the *Effect* level — the atomic store is
/// overwritten on each App mount, which is the desired behaviour:
/// remounting the App in dev (HMR) should re-sync to the persisted
/// value rather than carry stale state.
pub fn provide_mute_state() -> Muted {
    let initial = read_persisted().unwrap_or(false);
    MUTED_ATOMIC.store(initial, Ordering::Relaxed);
    let signal = RwSignal::new(initial);

    Effect::new(move |_| {
        let v = signal.get();
        MUTED_ATOMIC.store(v, Ordering::Relaxed);
        write_persisted(v);
    });

    Muted(signal)
}

#[cfg(target_arch = "wasm32")]
fn read_persisted() -> Option<bool> {
    let storage = web_sys::window()?.local_storage().ok().flatten()?;
    let raw = storage.get_item(STORAGE_KEY).ok().flatten()?;
    match raw.as_str() {
        "1" | "true" => Some(true),
        "0" | "false" => Some(false),
        _ => None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
const fn read_persisted() -> Option<bool> {
    None
}

#[cfg(target_arch = "wasm32")]
fn write_persisted(v: bool) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return;
    };
    let _ = storage.set_item(STORAGE_KEY, if v { "1" } else { "0" });
}

#[cfg(not(target_arch = "wasm32"))]
const fn write_persisted(_v: bool) {}
