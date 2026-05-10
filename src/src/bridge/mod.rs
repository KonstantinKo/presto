// Bridge module — typed Tauri command boundary.
//
// Spec 001-leptos-migration §Phase 1: every Tauri command (existing + new)
// returns `Result<T, BridgeError>`. This module owns the closed-domain
// types that travel across the bridge:
//
// - `error` — `BridgeError` enum (Phase 1A T023-T025).
// - `session_type` — `SessionType` enum (Phase 1A T028-T029).
//
// Per AGENTS.md §IPC: `invoke()` + `listen()` only; no other channels.

pub mod availability;
pub mod commands;
pub mod error;
pub mod session_type;
