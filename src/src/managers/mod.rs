// Manager state machines — Phase 3 of spec 001-leptos-migration.
//
// Each manager owns a piece of frontend state, talks to the bridge layer
// (`bridge::commands` for Tauri commands, `bridge::events` for `listen()`
// payloads), and exposes a typed API for the components layer (Phase 4)
// to consume. Per Principle V (Test-First For Stateful Engines), every
// behaviour is covered by a failing test before the implementation lands.
//
// Module breakdown (Phase 3a — settings & navigation; Phase 3b — tag &
// session; Phase 3c — auth, update, team per tasks.md §Phase 3):
// - `settings`  — `SettingsManager` over the `Settings` shared record.
//                 Carries the F1/M3 `hide_status_bar → status_bar_display`
//                 lockstep migration (custom deserializer with legacy
//                 fallback; see `bridge::types::deserialize_status_bar_display_with_legacy_fallback`).
// - `navigation` — `NavigationManager` over the `NavView` / `SettingsTab`
//                  router-style enums (any-to-any transitions allowed).
// - `tag`       — `TagManager` over the user's `Tag` list. Per-tag CRUD
//                 (no bulk save) per contracts/tauri-bridge.md §Deletions.
// - `session`   — `SessionManager` over the user's manual-session
//                 backfill records. Per Principle I, manual entries
//                 route through `engine::timer::TimerState::record_manual_session`
//                 before the bulk re-save lands on disk via
//                 `bridge::commands::save_manual_sessions`.
// - `auth`      — `AuthManager` over the `AuthState` enum (Guest /
//                 Unauthenticated / SignedIn). Per Principle II,
//                 the default state is `Unauthenticated` and sign-in
//                 is opt-in; `continue_as_guest` is a first-class
//                 transition that persists the `presto-guest-mode`
//                 localStorage flag.
// - `update`    — `UpdateManager` over the `UpdateInfo` enum + the
//                 polling cadence pin (1h, matching JS-era
//                 `update-manager-global.js:219`). Consumes the E10
//                 `tauri://update-available` event payload.
// - `team`      — `TeamManager` — parity-only port of the JS-era
//                 demo-fixture team-roster surface. No bridge calls.

pub mod auth;
pub mod navigation;
pub mod session;
pub mod settings;
pub mod tag;
