// Theme subsystem — Phase 5 (T223-T224) of spec
// 001-leptos-migration.
//
// Two halves:
//
// - `themes` — the auto-generated catalogue
//   (`pub const ALL_THEMES: &[&str]`,
//   `pub const DEFAULT_THEME: &str`). Code-gen lives in
//   `tools/build-themes`; the Cargo build script at `src/build.rs`
//   regenerates `themes.rs` into `OUT_DIR` on every `cargo build`
//   or `cargo test`. Consumed by
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
pub mod metadata;
pub mod themes {
    // Generated into OUT_DIR by src/build.rs from `style/themes/*.css`.
    include!(concat!(env!("OUT_DIR"), "/themes.rs"));
}
