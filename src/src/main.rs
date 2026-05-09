// Phase 0 hello-world entry. Replaced wholesale in Phases 1-5.
// Per spec 001-leptos-migration plan.md §Phase 0 exit criteria and
// tasks.md T005 (`leptos::mount_to_body(|| view! { <p>"hello"</p> })`).

use leptos::prelude::*;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(|| view! { <p>"hello"</p> });
}
