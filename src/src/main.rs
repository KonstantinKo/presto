// Binary entry point for the Leptos-driven `presto-web` frontend.
//
// Per spec 001-leptos-migration tasks.md, Phase 4 mounts the
// component tree (Phase 4a: T189-T203 — five core screens; Phase 4b
// adds settings/auth/update/team). The full router lands at T217 in
// Phase 4c — until then, `main.rs` mounts the `TimerView` directly
// so the e2e suite's first-paint selectors (`#timer-view`,
// `#timer-minutes`, `#timer-seconds`) resolve under `trunk serve`.
//
// The component tree lives in the package's lib crate at
// `src/src/lib.rs`; the binary imports via the public crate path
// `presto_web::components::*`. The lib root carries the Phase 1A
// T024 rationale for the lib + bin split.

use presto_web::components::timer::TimerView;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(TimerView);
}
