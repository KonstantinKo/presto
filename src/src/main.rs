// Binary entry point for the Leptos-driven `presto-web` frontend.
//
// Spec 001-leptos-migration §Phase 4b T217: mounts the top-level
// `<App/>` router from `presto_web::app`. The router owns the
// sidebar, the per-view dispatch, the shared
// `RwSignal<Settings/AuthState/UpdateInfo>`, and the bridge-bus
// startup hops (legacy localStorage migration, settings load,
// `tauri://update-available` + `global-shortcut` event
// subscriptions).
//
// The component tree lives in the package's lib crate at
// `src/src/lib.rs`; the binary imports via the public crate path
// `presto_web::app::App`. The lib root carries the Phase 1A T024
// rationale for the lib + bin split.

use presto_web::app::App;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}
