// presto-ipc — single source of truth for every type that crosses
// the Tauri↔Leptos IPC boundary.
//
// **Why this crate exists**: `#[tauri::command]` auto-applies
// `rename_all = "camelCase"` to its generated arg-deserializer
// struct; the Leptos frontend, if it defines its own
// `#[derive(Serialize)] struct Args { ... }`, defaults to
// snake_case. Drift compiles on both sides, fails silently at
// runtime — the Tauri resolver rejects the request, the Promise
// rejects, the WASM call site `let _ = ...await` swallows the
// error. Five real bugs shipped this way before the project moved
// to this shared-crate model.
//
// **Feature flags**:
// - `specta`  enables `specta::Type` derives for `tauri-specta`
//             bindings codegen. Host crate enables; WASM crate
//             disables (keeps specta transitives out of the wasm
//             dep graph).
// - `migration` exposes the JS-era legacy payload types. Both
//             endpoints enable until the post-cutover sunset.

#![allow(clippy::module_name_repetitions)]
// Module-level glob re-exports below intentionally surface everything
// from each sub-module flat at the crate root. `unreachable_pub` fires
// on every empty module declaration; suppress at the use site rather
// than spelling out each symbol (which would defeat the point of a
// flat re-export hub).
#![allow(unreachable_pub)]

pub mod args;
pub mod auth;
pub mod error;
pub mod events;
pub mod session;
pub mod settings;
pub mod tags;
pub mod tasks;
pub mod timer;

#[cfg(feature = "migration")]
pub mod migration;

// Convenience re-exports — `use presto_ipc::*;` brings every wire
// type into scope at the call site. Keep the surface flat so call
// sites read like the original `bridge::types::*` imports.
pub use args::*;
pub use auth::*;
pub use error::*;
pub use events::*;
pub use session::*;
pub use settings::*;
pub use tags::*;
pub use tasks::*;
pub use timer::*;

#[cfg(feature = "migration")]
pub use migration::*;
