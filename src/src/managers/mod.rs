// Manager state machines — Phase 3 of spec 001-leptos-migration.
//
// Each manager owns a piece of frontend state, talks to the bridge layer
// (`bridge::commands` for Tauri commands, `bridge::events` for `listen()`
// payloads), and exposes a typed API for the components layer (Phase 4)
// to consume. Per Principle V (Test-First For Stateful Engines), every
// behaviour is covered by a failing test before the implementation lands.
//
// Module breakdown (Phase 3a — settings & navigation; subsequent batches
// add `tag`, `session`, `auth`, `update`, `team` per tasks.md §Phase 3):
// - `settings`  — `SettingsManager` over the `Settings` shared record.
//                 Carries the F1/M3 `hide_status_bar → status_bar_display`
//                 lockstep migration (custom deserializer with legacy
//                 fallback; see `bridge::types::deserialize_status_bar_display_with_legacy_fallback`).
// - `navigation` — `NavigationManager` over the `NavView` / `SettingsTab`
//                  router-style enums (any-to-any transitions allowed).

pub mod navigation;
pub mod settings;
