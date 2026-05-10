// Engine — `ActivitySignal` reduction.
//
// Spec 001-leptos-migration §Phase 2 T130-T131: the engine consumes
// a normalised `ActivitySignal` stream rather than raw DOM events
// (Principle I). The bridge layer subscribes to `user-activity` /
// `user-inactivity` Tauri events and feeds the engine via
// `Timer::observe_activity(signal)`; this module owns the
// edge-detection logic so duplicate Active→Active or Idle→Idle
// emissions are folded into no-ops.
