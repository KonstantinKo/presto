//! Source-level invariant: every `#[tauri::command]` handler in
//! `src-tauri/src/lib.rs` is registered in
//! `build_specta_builder()::collect_commands![]`.
//!
//! A new handler added without the registration call silently
//! escapes the Tauri runtime — the binary 404s on invoke, the TS
//! bindings file omits the symbol, the drift gate is satisfied
//! because what's registered matches what's exported, and the
//! call site fails only at runtime when the user hits the
//! affected feature.
//!
//! This test parses the lib.rs source itself (via `include_str!`),
//! counts `#[tauri::command]` annotations, and asserts the count
//! equals the length of `collect_commands![…]`. Cheaper than a
//! macro that introspects the Builder; works against the source
//! tree without a Tauri runtime.

const LIB_SRC: &str = include_str!("../src/lib.rs");

/// Count `#[tauri::command]` annotations. Multi-line attribute
/// blocks are normalised at the line boundary — the annotation
/// always sits on its own line in this codebase.
fn count_command_annotations() -> usize {
    LIB_SRC
        .lines()
        .filter(|line| line.trim() == "#[tauri::command]")
        .count()
}

/// Count handler identifiers passed to `collect_commands![…]`.
/// The macro spans multiple lines; identifiers are separated by
/// commas. We slice the source between the `collect_commands![`
/// marker and its closing `]`, then count comma-separated tokens.
fn count_collected_commands() -> usize {
    let needle = "collect_commands![";
    let Some(start) = LIB_SRC.find(needle) else {
        panic!("could not locate `collect_commands![` in lib.rs source");
    };
    let after_open = &LIB_SRC[start + needle.len()..];
    let Some(end) = after_open.find(']') else {
        panic!("could not locate closing `]` after `collect_commands![`");
    };
    let body = &after_open[..end];
    body.split(',')
        .map(str::trim)
        .filter(|tok| !tok.is_empty())
        .count()
}

/// Pinning test: a new `#[tauri::command]` handler must appear in
/// `collect_commands![…]` or this test fails.
///
/// If you added a handler and got here, append its function name
/// inside the macro at
/// `src-tauri/src/lib.rs::build_specta_builder()`.
#[test]
fn every_tauri_command_handler_is_registered_in_collect_commands() {
    let annotations = count_command_annotations();
    let registered = count_collected_commands();
    assert_eq!(
        annotations, registered,
        "Found {annotations} `#[tauri::command]` annotation(s) in lib.rs \
         but {registered} entries in `collect_commands![…]`. Every \
         annotated handler must be registered with `tauri-specta`'s \
         Builder; otherwise the runtime 404s on invoke and the TS \
         bindings silently omit the symbol.",
    );
}

/// Sanity bound: handler count should not drop unexpectedly. Baseline
/// rebased to 25 after the local-only pivot removed auth/sync/team and
/// the transition-only legacy-migration command family.
#[test]
fn handler_count_meets_baseline() {
    let count = count_command_annotations();
    assert!(
        count >= 25,
        "Expected at least 25 `#[tauri::command]` handlers (local-only \
         baseline); found {count}. A drop indicates an accidental \
         handler deletion — verify intentionally.",
    );
}
