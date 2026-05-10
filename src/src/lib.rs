// Library root for the `presto-web` Leptos crate.
//
// Spec 001-leptos-migration §Phase 1A T024: declares the bridge module so
// `BridgeError` (and, later in Phase 1A, `SessionType`) is reachable from
// outside the crate. This pairs the existing `[[bin]]` with a `[lib]` so
// the unit tests under `cargo test -p presto-web` run against the lib's
// module tree, and `pub` items are not flagged by `unreachable_pub`
// (which fires on binary-only crates because nothing has an external
// reachability path). Without this lib root, `unreachable_pub` (warn)
// and `clippy::redundant_pub_crate` (nursery deny in workspace) disagree
// about how to mark every `pub`/`pub(crate)` item.
//
// The bin (`main.rs`) declares its own `mod bridge;` for self-contained
// compilation; the lib re-exposes the same tree for external testability
// and for downstream Tauri-side consumers (in this workspace) that may
// want to import shared types via a workspace path dependency in later
// phases. Today there are no other consumers — but the lib root is the
// correct shape for Phase 1C onwards when bridge wrappers grow.

pub mod bridge;
pub mod components;
pub mod engine;
pub mod managers;
