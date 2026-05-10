// Engine module — the pomodoro state machine, ported from
// `src/core/pomodoro-timer.js` per spec 001-leptos-migration §Phase 2
// (T120-T146).
//
// **Principle I (The Timer Is Sacred)**: this module is a pure Rust
// state machine. No `web-sys` imports under `src/src/engine/` — the
// repo enforces this with the grep gate in T146 / Phase 7 CI. DOM
// inputs (real wall-clock time, raw activity events from the macOS
// `ActivityMonitor` plugin) enter the engine via the abstract
// `Clock` trait and the `ActivitySignal` enum, which are fed by the
// bridge layer. Tests inject deterministic implementations so every
// behavioural rule from the JS source is reproducible.
//
// Module breakdown:
// - `timer` — `TimerState` state machine + `TimerEvent` emissions.
// - `activity_signal` — Idle ↔ Active edge-detection reduction.
// - `clock` — abstract `Clock` trait for wall-clock time.
// - `durations` — `Durations` struct (focus / short / long break in seconds).
// - `date_format` — chrono format pin for `Session.date` parity.

pub mod activity_signal;
pub mod clock;
pub mod date_format;
pub mod durations;
pub mod timer;
