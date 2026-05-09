// Phase 0 hello-world entry. Replaced wholesale in Phases 1-5.
// Per spec 001-leptos-migration plan.md §Phase 0 exit criteria and
// tasks.md T005 (`leptos::mount_to_body(|| view! { <p>"hello"</p> })`).
//
// The bridge module tree (typed Tauri command boundary) lives in the lib
// root at `src/src/lib.rs`; the binary uses it via the package's lib
// crate path (`presto_web::bridge::*`). Phase 1A T024 introduces this
// lib + bin layout; see `lib.rs` for rationale.

use leptos::prelude::*;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(|| view! { <p>"hello"</p> });
}
