// Phase 0 hello-world entry. Replaced wholesale in Phases 1-5.
// Per spec 001-leptos-migration plan.md §Phase 0 exit criteria and
// tasks.md T005 (`leptos::mount_to_body(|| view! { <p>"hello"</p> })`).

use leptos::prelude::*;

// Bridge module: typed Tauri command boundary. Phase 1A introduces the
// `BridgeError` and `SessionType` enums under this tree; later sub-phases
// add wrappers and storage helpers. Declared at the binary root so unit
// tests under `cargo test -p presto-web` are addressable as
// `bridge::error::tests::*`.
mod bridge;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(|| view! { <p>"hello"</p> });
}
