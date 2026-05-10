// `NavigationManager` — the Rust port of `src/managers/navigation-manager.js`.
//
// Spec 001-leptos-migration §Phase 3a (T157-T160). Owns the active
// view (`NavView`) and the active settings sub-tab (`SettingsTab`).
// Router-style state machine: any `NavView::X → NavView::Y`
// transition is allowed (data-model.md §`NavView`). Initial state is
// `NavView::Timer`.
//
// Phase 3a body lands the enums + transitions in T158/T160; this file
// is a placeholder until the RED tests for T157 land.

// Phase 3a placeholder — content lands in T157 RED followed by T158
// GREEN. Keeping the module empty here is fine because the parent
// `mod.rs` declares it; the workspace `unreachable_pub` lint won't
// fire on an empty module.
