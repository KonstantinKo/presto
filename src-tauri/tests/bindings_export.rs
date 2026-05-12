//! CI gate against `src/bindings/tauri.ts` drift.
//!
//! Regenerates the bindings file in a temp directory and compares it
//! byte-for-byte against the checked-in `src/bindings/tauri.ts`. A
//! mismatch fails the test with the diff command the maintainer
//! should run:
//!
//!     cargo test -p presto --test bindings_export -- --nocapture
//!     # then `git diff src/bindings/tauri.ts` to inspect
//!     # then `cp <tmp>/tauri.ts src/bindings/tauri.ts` if desired
//!
//! This is the single defence against the IPC drift bug class. Every
//! `#[tauri::command]` flows through the same Builder used by
//! `run()`, so a missing `#[specta::specta]` annotation, a renamed
//! Args field, or a wire-shape change in `presto-ipc` all surface
//! here before they ship.

use std::path::{Path, PathBuf};

/// Configure the TS exporter to emit `string` for every BigInt-class
/// type so any future `u64`/`i64` field crossing the bridge serialises
/// safely.
fn ts_exporter() -> specta_typescript::Typescript {
    specta_typescript::Typescript::default().bigint(specta_typescript::BigIntExportBehavior::String)
}

fn presta_export(path: &Path) {
    presto_lib::build_specta_builder()
        .export(ts_exporter(), path)
        .expect("specta export must succeed");
}

fn repo_root() -> PathBuf {
    // `CARGO_MANIFEST_DIR` is `<repo>/src-tauri`; the bindings file
    // lives at `<repo>/src/bindings/tauri.ts`.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri has a parent (workspace root)")
        .to_path_buf()
}

#[test]
fn bindings_match_checked_in_copy() {
    let bindings_path = repo_root().join("src/bindings/tauri.ts");

    let tmp = tempfile::tempdir().expect("tempdir");
    let candidate_path = tmp.path().join("tauri.ts");

    presta_export(&candidate_path);

    let candidate = std::fs::read_to_string(&candidate_path).expect("read regenerated bindings");

    if !bindings_path.exists() {
        std::fs::write(&bindings_path, &candidate).expect("write initial bindings");
        panic!(
            "bindings file did not exist at {}; wrote initial copy. Re-run the test to verify.",
            bindings_path.display(),
        );
    }

    let checked_in = std::fs::read_to_string(&bindings_path).expect("read checked-in bindings");

    assert_eq!(
        candidate.trim_end(),
        checked_in.trim_end(),
        "src/bindings/tauri.ts is stale; regenerate via:\n\
         \n\
             cargo test -p presto --test bindings_export -- --nocapture\n\
         \n\
         then `git add src/bindings/tauri.ts`.\n\
         \n\
         If you intended the drift, this means a `#[tauri::command]` \n\
         signature or `presto-ipc` wire type changed and the IPC \n\
         contract evolved with it — review the diff carefully.",
    );
}
