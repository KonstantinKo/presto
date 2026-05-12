// Bridge module — typed Tauri command boundary.
//
// Spec 001-leptos-migration §Phase 1: every Tauri command returns
// `Result<T, BridgeError>`. The wire types (`BridgeError`,
// `SessionType`, `TimerMode`, every record, every Args bundle)
// live in the shared `presto-ipc` crate; `bridge::types` is the
// single re-export hub on the Leptos side.
//
// Per AGENTS.md §IPC: `invoke()` + `listen()` only; no other
// channels.

pub mod availability;
pub mod commands;
pub mod events;
pub mod notification;
pub mod types;
