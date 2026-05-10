// Theme subsystem — Phase 5 (T223-T224) of spec
// 001-leptos-migration.
//
// Two halves:
//
// - `themes` — the auto-generated catalogue
//   (`pub const ALL_THEMES: &[&str]`,
//   `pub const DEFAULT_THEME: &str`). Code-gen lives in
//   `tools/build-themes`; the Trunk pre-build hook in
//   `src/Trunk.toml` regenerates `themes.rs` on every build, and
//   the file is `.gitignore`-tracked. Consumed by
//   `components::settings::theme::ThemeSettings`.
//
// - `loader` — runtime DOM application + system-theme detection.
//   `apply_theme(name)` sets `<html data-theme="...">`; the
//   `prefers-color-scheme` hop folds the OS-level light/dark
//   selection into the rendered class so the JS-era CSS rules
//   under `src/style/themes/*.css` continue to match unchanged
//   (FR-021).
//
// Per Principle I, this module is a thin DOM-binding wrapper. The
// rendering decision (which theme stem to pick) is the manager
// layer's concern; this module does the `set_attribute` write and
// nothing else stateful.

pub mod loader;
pub mod themes;
