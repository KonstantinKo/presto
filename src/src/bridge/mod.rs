// Bridge module — typed Tauri command boundary.
//
// Spec 001-leptos-migration §Phase 1: every Tauri command (existing + new)
// returns `Result<T, BridgeError>`. This module owns the closed-domain
// types that travel across the bridge:
//
// - `error` — `BridgeError` enum (Phase 1A T023-T025).
// - `session_type` — `SessionType` enum (Phase 1A T028-T029).
// - `timer_mode` — `TimerMode` enum (Phase 1C T076-T079; Tauri-side mirror
//   was T027).
//
// Per AGENTS.md §IPC: `invoke()` + `listen()` only; no other channels.

pub mod availability;
pub mod commands;
pub mod error;
pub mod events;
pub mod session_type;
pub mod storage;
pub mod timer_mode;
pub mod types;
