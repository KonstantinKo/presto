# Implementation Plan: Timer Control Rework + Quick Log + Distraction Capture

**Branch**: `006-timer-controls-quicklog-distractions` | **Date**: 2026-05-15 | **Spec**: [`spec.md`](spec.md)
**Input**: Feature specification from `/specs/006-timer-controls-quicklog-distractions/spec.md`

> Thin, living plan. It points at spec FRs and code. It does not re-state them.
> Spec FR-NNN labels are normative; this plan binds them to file paths and engine entry points.

## Summary

Three converging UI/data changes anchored on the existing Tauri 2.x + Leptos stack and the existing engine in `src/src/engine/timer.rs`:

1. **State-aware control matrix** (FR-012 → FR-018). Three slots, one button each, label/icon/handler/aria flip on a UI-layer closed-sum `RunState` (Idle | Running | Paused). The engine learns two new entry points — `abort` and `complete` — that traverse the existing state machine rather than bypassing it (FR-013, FR-017).
2. **Combined `#timer-status-pill`** (FR-001 → FR-006). Wraps the existing `#timer-status` (chip + mode label + chevron) and `#session-title-input` into one container. DOM-minimal refactor; existing selectors preserved so e2e + VR stay anchored.
3. **Quick Log + Distraction capture surfaces and Inventory** (FR-019 → FR-027). Two new typed entities (`QuickLog`, `Distraction`) under `crates/presto-ipc/`. Four new Tauri commands mirroring the `load_manual_sessions` / `save_manual_sessions` precedent at `src-tauri/src/lib.rs:514-532`. Two new managers in `src/src/managers/`. One new `Inventory` subsection inside the existing daily Stats / Calendar area, reusing the `sessions_history_table.rs` edit/delete-via-modal pattern.

No new third-party dependencies (FR-036 / Principle X). No new IPC mechanism (Principle VI). All Principle V scope (engine, managers) is RED-then-GREEN.

## Technical Context

**Language/Version**: Rust 1.75+ (frontend WASM + backend native), TypeScript-flavour JS in the Playwright e2e suite.
**Primary Dependencies**: Leptos (CSR + WASM), Trunk, Tauri 2.x, `presto-ipc` (workspace), `serde`, `serde_json`, `chrono`, `uuid`, `specta` (feature-gated). All already in tree — no manifest churn expected.
**Storage**: Tauri app-data directory. New files alongside existing JSON: `quick_logs.json`, `distractions.json`. Read/write through helpers in `src-tauri/src/helpers.rs`. Missing files deserialise to `Vec::new()`.
**Testing**: `wasm-bindgen-test` for frontend unit + manager tests; `cargo test` for backend; Playwright (chromium) for e2e + visual regression at `tests/e2e/__screenshots__/visual-regression/`.
**Target Platform**: Tauri desktop (macOS, Linux, Windows). CI VR reference is `chromium-linux`.
**Project Type**: Single-user desktop app (Tauri 2.x). No backend service.
**Performance Goals**: Distraction modal interaction ≤ 1 s (SC-002). Quick Log end-to-end ≤ 10 s (SC-003). Abort → suppress-auto-restart within one engine tick (SC-010).
**Constraints**: Engine remains a pure state machine (Principle I). Local-only, no network egress (Principle II). Strict clippy/fmt gates (Principle X). No new `#[allow]` (SC-011).
**Scale/Scope**: Two new IPC types, four new Tauri commands, two new managers, three new modals (Quick Log, Distraction, Inventory row-edit), one new daily-view subsection, two new engine entry points. ~15 new catalogue keys. One existing tile-label format string widened. VR baselines: `timer-chromium-linux.png`, `daily-chromium-linux.png`, `statistics-daily-chromium-linux.png` (plus weekly/monthly statistics tiles).

## Constitution Check

Only violations and justified deviations are listed. Pass lines are omitted.

No principles violated. Notes on principle-brushed surfaces:

- **I. The Timer Is Sacred** — `abort` and `complete` are new entry points on the engine. Both traverse the existing event surface (`SessionPaused` / `PomodoroCompleted` / `SessionSkipped` patterns at `src/src/engine/timer.rs:664-872`). The natural-completion sequence at lines 808-872 is the reference for `complete`; in the normal (non-continuous-overtime) branch, `complete` reads `current_session_elapsed_secs`, increments `completed_pomodoros`, accumulates `total_focus_secs`, resets the per-session counter, emits `PomodoroCompleted`, runs the long-break cadence check, advances mode, stops running. In the continuous-overtime branch (flag-driven, see Architecture overview), `complete` seals + advances without re-incrementing the count. `abort` does NOT advance mode and is gated out of the auto-restart path at `src/src/components/timer/mod.rs:1471-1483` (gate extended in this PR to also require `PomodoroCompleted` in the events vec). Additionally, the existing `pause()` method is extended in this PR to settle wall-clock delta into `current_session_elapsed_secs` before clearing the start anchor (FR-013a).
- **III. Type Safety Over Defensive Code** — the UI `RunState` enum (Idle | Running | Paused; AutoPaused renders as Paused) is a closed sum, matched exhaustively. No flag-bool combinators in the matrix. `QuickLog`, `Distraction`, `DistractionParentRef` are closed typed structs; ranged ints validated at the Tauri boundary in `BridgeError::InvalidArgument` (FR-022).
- **IV. Visual Regression Is The UI Contract** — three baselines regenerate; one-line PR notes per baseline (see `Visual regression budget`). `[BEST-GUESS PM DECISION]` on whether to introduce new Running/Paused timer baselines now — recommendation in same section.
- **V. Test-First For Stateful Engines** — every engine entry point and both managers are Principle V scope. Failing test commit precedes implementation commit per `AGENTS.md` test-first ordering rule. UI plumbing and modal wiring are out of Principle V scope; the e2e suite covers them.
- **VI. The Tauri Boundary Is Stable** — four new typed `invoke`-pattern commands. Mock at `tests/e2e/fixtures/tauriMock.js` extended first per Principle V's mock-first rule. No new IPC mechanism.

## Project Structure

### Documentation (this feature)

```text
specs/006-timer-controls-quicklog-distractions/
├── plan.md                              # This file
├── spec.md                              # Already written
├── research.md                          # Phase 0 — only external/irreversible decisions
├── data-model.md                        # Phase 1 — new types + invariants
├── contracts/
│   ├── timer-engine-actions.md          # abort / complete contracts
│   └── persistence-commands.md          # Four new Tauri commands
└── quickstart.md                        # Phase 1 — dev exercise of the feature
```

### Source Code Touched (repository root)

```text
crates/presto-ipc/src/
├── lib.rs                  # +pub mod quick_log; +pub mod distraction; re-exports
├── quick_log.rs            # NEW — QuickLog struct, validation impl
└── distraction.rs          # NEW — Distraction + DistractionParentRef structs

src-tauri/src/
├── lib.rs                  # +4 commands (load/save quick_logs, load/save distractions)
│                           #  registered in the invoke_handler! block
└── helpers.rs              # +read_quick_logs_from / write_quick_logs_to / same pair for distractions

src/src/
├── engine/
│   └── timer.rs            # +abort(clock), +complete(clock); +TimerEvent::SessionAborted,
│                           #   +TimerEvent::SessionCompletedEarly
├── managers/
│   ├── mod.rs              # +pub mod quick_log; +pub mod distraction
│   ├── quick_log.rs        # NEW — QuickLogManager (load/add/update/delete; bulk re-save)
│   └── distraction.rs      # NEW — DistractionManager (same shape)
├── components/
│   ├── timer/
│   │   ├── mod.rs                       # Combined pill restructure (lines 1727-1999)
│   │   │                                # State-aware button matrix (lines 2034-2128 replaced)
│   │   │                                # Remove StopButtonState::Undo
│   │   ├── quick_log_modal.rs           # NEW — modal
│   │   └── distraction_modal.rs         # NEW — modal
│   ├── daily/
│   │   ├── mod.rs                       # +<Inventory /> render below sessions-history-table
│   │   ├── inventory.rs                 # NEW — Inventory subsection
│   │   └── sessions_history_table.rs    # Untouched — edit/delete pattern reused
│   └── stats/
│       └── mod.rs                       # Tile-label widening (lines 431-457)
└── i18n/                                # Catalogue keys added per FR-031

tests/e2e/
├── fixtures/
│   └── tauriMock.js                     # +4 commands (mock-first per Principle V)
├── timer-quick-log.spec.js              # NEW
├── timer-distraction.spec.js            # NEW
├── timer-complete.spec.js               # NEW
├── timer-abort.spec.js                  # NEW
└── inventory.spec.js                    # NEW

tests/e2e/__screenshots__/visual-regression/
├── timer-chromium-linux.png             # REGENERATED (Idle pill + new left label)
├── daily-chromium-linux.png             # REGENERATED (Inventory subsection)
├── statistics-daily-chromium-linux.png  # REGENERATED only if scenario has non-zero suffix
├── statistics-weekly-chromium-linux.png # REGENERATED only if scenario has non-zero suffix
└── statistics-monthly-chromium-linux.png# REGENERATED only if scenario has non-zero suffix
```

**Structure Decision**: Reuse the existing Tauri + Leptos layout. No new crate. No new top-level directory. New code slots into the established triad of `crates/presto-ipc/` (shared IPC), `src-tauri/src/` (backend), `src/src/{engine,managers,components}` (frontend).

## Architecture overview

The architecture has three concentric rings.

**Ring 1 — Engine (the deterministic state machine, Principle I scope).** Today the engine at `src/src/engine/timer.rs` exposes `pause` (lines 664-683), `resume` (708-722), `skip` (396-445), `reset` (493), `tick`, and the implicit natural-completion sequence (808-872). It distinguishes run-state via three orthogonal bools `is_running`, `is_paused`, `is_auto_paused` (lines 119-173). This feature adds two new entry points to that ring:

- `abort(clock) -> Vec<TimerEvent>` — idempotent; valid from Running OR Paused OR AutoPaused. Clears `current_session_elapsed_secs`, clears `session_completed_but_not_saved`, clears all three run-state bools, emits `TimerEvent::SessionAborted { aborted_mode, elapsed_secs }`. Does NOT advance mode. The auto-restart gate at `src/src/components/timer/mod.rs:1471-1483` is extended in this PR to also check `events.iter().any(|e| matches!(e, TimerEvent::PomodoroCompleted { .. }))` (matching the existing event-check pattern at line 1429 for the session-save gate). Under the new gate: `abort` emits no `PomodoroCompleted` → auto-restart does NOT fire.

- `complete(clock) -> Vec<TimerEvent>` — idempotent; precondition `is_paused || is_auto_paused`. Reads `current_session_elapsed_secs` (which `pause()` has settled to wall-clock — see FR-013a). **If `< 30`**: internally calls `abort` and returns its events (no `PomodoroCompleted`, no count). **If `>= 30`**: branches on `session_completed_but_not_saved` (set by the continuous-mode zero-cross at `src/src/engine/timer.rs:826`; read by `skip` at 407-417 to avoid double-counting):
  - **Flag `false` (normal paused-before-zero):** traverses the natural-completion side-effect sequence as written at lines 808-872 (increments `completed_pomodoros`, accumulates `total_focus_secs += current_session_elapsed_secs`, resets per-session counter, emits `PomodoroCompleted { completed_pomodoros }`, runs the long-break cadence check identically — same `sessions_per_long_break` consultation, same `LongBreak` vs `Break` selection — advances mode, stops running).
  - **Flag `true` (continuous-mode overtime):** the zero-cross already incremented `completed_pomodoros` and emitted `PomodoroCompleted`. Accumulator now holds ONLY the overtime portion. `complete` adds that overtime to `total_focus_secs`, resets the per-session counter, clears the flag, **suppresses re-emission of `PomodoroCompleted`** (canonical signal already fired), advances mode per the cadence already computed at the zero-cross, stops running.
  - In both sub-branches: emits `TimerEvent::SessionCompletedEarly { elapsed_secs }` per the uniform-emission rule. `SessionCompletedEarly` is engine-internal — never leaves the process, never serialised over the wire (Principle II).

To prevent path duplication, the natural-completion zero-cross path AND `complete`'s branch-B body share a new private helper `complete_focus_session(&mut self) -> Vec<TimerEvent>` (see Module breakdown). The natural-completion path itself does not change observable behaviour. The continuous-mode branch at line 810 is unchanged: in continuous mode the engine zero-crosses but does NOT auto-advance mode — it sets `session_completed_but_not_saved` and keeps ticking into overtime. `complete` from paused overtime is the only path that fully ends a continuous-mode session with a count. `skip` is unchanged and still does not count, even in continuous mode (consistent with today; FR-018) and continues to clear `session_completed_but_not_saved`.

The auto-restart at `src/src/components/timer/mod.rs:1471-1483` fires after natural completion and after branch-B.1 `complete`-with-count (intentional — `complete` is equivalent to natural completion). In branch B.2 (continuous-mode overtime), the auto-restart already fired at the zero-cross, so `complete` does not re-emit `PomodoroCompleted` and the gate does not re-fire.

**Pause wall-clock settling (FR-013a):** the existing `pause()` body at `src/src/engine/timer.rs:664-683` clears `timer_start_ms` without integrating the [0, 1) seconds since the last tick. The fix extends `pause()` to call the existing tick-drift compensation helper (or extract one) before clearing the anchor — settling `current_session_elapsed_secs` to the precise wall-clock value. Without this, a 30.0 s wall-clock pause-then-`complete` could be observed as 29 s elapsed and discarded as Abort.

**Ring 2 — Managers + persistence (Principle V scope; Principle II local-only; Principle VI Tauri-boundary stable).** Two new managers join `src/src/managers/`: `QuickLogManager` and `DistractionManager`. Both follow the `ManualSessionManager` pattern at `src/src/managers/session.rs:25-36`: in-memory `Vec<T>`, `load()` populates via the corresponding `invoke()` call, `add(t)` / `update(id, t)` / `delete(id)` mutate the vec and trigger a full bulk re-save via `save_*(vec.clone())`. No per-row updates, mirroring `save_manual_sessions`. Both managers gracefully short-circuit when `window.__TAURI_INTERNALS__` is absent (per `AGENTS.md` bridge-availability rule).

The four new Tauri commands at `src-tauri/src/lib.rs` mirror `save_manual_sessions` (lines 514-528) and `load_manual_sessions` (lines 529-532). Each follows `Result<T, BridgeError>` from `crates/presto-ipc/src/error.rs:29-65`. New helpers in `src-tauri/src/helpers.rs` for JSON file IO. Boundary validation per FR-022 returns `BridgeError::InvalidArgument { field, reason }` — the existing variant — for out-of-range fields. No new `BridgeError` variants needed.

**Ring 3 — UI (Principles III + IV scope).** The closed-sum UI-layer `RunState` (Idle | Running | Paused — AutoPaused folds into Paused at the matrix layer, per FR-012 paragraph 3 + Story 1 AC 3) derives once from the engine bools and drives every per-slot button. Each slot is a single `<button>` whose label, icon, ghost/filled styling, click handler, and `aria-label` flip on `(RunState, TimerMode)`. The combined pill replaces today's separate-controls layout with minimal DOM churn — both children (`#timer-status` and `#session-title-input`) keep their selector IDs, just under a new `#timer-status-pill` parent. Two new modals (Quick Log, Distraction). One new Inventory subsection in the daily Stats / Calendar area.

The engine bools stay as-is. There is no engine-wide refactor of `is_running`/`is_paused`/`is_auto_paused` into a single enum — that's explicitly **Out of Scope** per the spec.

## Module breakdown

| Module | Path | Why |
|---|---|---|
| `presto_ipc::quick_log` | `crates/presto-ipc/src/quick_log.rs` (new) | Shared `QuickLog` type + validation impl. `#[cfg_attr(feature = "specta", derive(specta::Type))]`, `#[serde(rename_all = "camelCase")]` per existing IPC convention. |
| `presto_ipc::distraction` | `crates/presto-ipc/src/distraction.rs` (new) | Shared `Distraction` + `DistractionParentRef` types. Same conventions. `parent_ref: Option<DistractionParentRef>`. |
| `crates/presto-ipc/src/lib.rs` | edit | `pub mod quick_log; pub mod distraction; pub use quick_log::QuickLog; pub use distraction::{Distraction, DistractionParentRef};` |
| `src-tauri/src/lib.rs` | edit | Four new `#[tauri::command]` async fns registered in the existing `invoke_handler!` block. Mirror `save_manual_sessions` / `load_manual_sessions`. |
| `src-tauri/src/helpers.rs` | edit | Four new JSON IO helpers: `read_quick_logs_from`, `write_quick_logs_to`, `read_distractions_from`, `write_distractions_to`. Mirror existing `*_manual_sessions_*` helpers. Filenames `quick_logs.json` and `distractions.json` alongside `manual_sessions.json`. |
| `src/src/engine/timer.rs` | edit | `pub fn abort(&mut self, clock: &impl Clock) -> Vec<TimerEvent>` and `pub fn complete(&mut self, clock: &impl Clock) -> Vec<TimerEvent>`. Plus a new private helper `fn complete_focus_session(&mut self) -> Vec<TimerEvent>` consumed by both the natural-completion zero-cross path (lines 808-872) AND `complete`'s branch-B body — single source of truth for the seal-and-advance sequence (emission order: `PomodoroCompleted` → `SessionCompletedEarly` in the count-incrementing path; in the continuous-mode overtime sub-branch the helper suppresses `PomodoroCompleted` re-emission, since the canonical one fired at the zero-cross). The existing `total_focus_secs` diff-capture UI subscriber at `src/src/components/timer/mod.rs:1420-1430` fires identically for both paths. Plus `TimerEvent::SessionAborted { aborted_mode: TimerMode, elapsed_secs: u32 }` and `TimerEvent::SessionCompletedEarly { elapsed_secs: u32 }` variants. Plus a small extension to `pause()` (lines 664-683) per FR-013a — settle wall-clock delta into `current_session_elapsed_secs` before clearing the start anchor. The location of `TimerEvent` is the in-process WASM enum at line 24 of this file — NOT `crates/presto-ipc/src/events.rs` (which contains only `UpdateAvailablePayload`). No Tauri-bridge crossing for the new variants. |
| `src/src/managers/quick_log.rs` | new | `pub struct QuickLogManager { entries: RwSignal<Vec<QuickLog>> }`. `load`, `add`, `update`, `delete` methods. Bulk re-save on every mutation. Imports follow the `SessionManager` precedent at `src/src/managers/session.rs:20-22` — IPC types via `crate::bridge::types` re-export; Tauri calls via `crate::bridge::commands::{save_quick_logs, load_quick_logs}`. Add the new types + commands to the existing re-export modules in `src/src/bridge/types.rs` and `src/src/bridge/commands.rs`. |
| `src/src/managers/distraction.rs` | new | `pub struct DistractionManager { entries: RwSignal<Vec<Distraction>> }`. Same shape. Imports follow the `SessionManager` precedent — IPC types via `crate::bridge::types` re-export; Tauri calls via `crate::bridge::commands::{save_distractions, load_distractions}`. Add the new types + commands to the existing re-export modules in `src/src/bridge/types.rs` and `src/src/bridge/commands.rs`. |
| `src/src/managers/mod.rs` | edit | `pub mod quick_log; pub mod distraction;` |
| `src/src/components/timer/mod.rs` | edit | (a) Wrap `#timer-status` + `#session-title-input` in `#timer-status-pill` (lines 1727-1999). (b) Replace today's three independent buttons (lines 2034-2128) with a matrix driven by `RunState` × `TimerMode`. (c) Remove the `StopButtonState::Undo` variant and all its branches (~lines 225-238 and downstream). (d) Subscribe to `TimerEvent::SessionAborted` to clear pending auto-restart-countdown UI state. |
| `src/src/components/timer/quick_log_modal.rs` | new | Form: title (auto-focused, required, `maxlength=120`), elapsed-minutes (numeric, min=1, max=720, default=5). Submit + cancel. Reachable from Idle left button AND Inventory header `+ Quick Log` button. |
| `src/src/components/timer/distraction_modal.rs` | new | Form: single text input (auto-focused, `maxlength=120`). Enter submits, Escape cancels. Captures the parent-session ref at modal-open time. |
| `src/src/components/daily/inventory.rs` | new | Subsections `Quick logs` and `Distractions`. Per-row Edit + Delete via the `sessions_history_table.rs` pattern. Date-filtered by the selected day. Carries the `+ Quick Log` header button. |
| `src/src/components/daily/mod.rs` | edit | Render `<Inventory />` below the existing sessions-history table. |
| `src/src/components/stats/mod.rs` | edit | Tile-label widening (lines 431-457). Append `· N quicklogs · M distractions` to the pomodoro-count tile when N > 0 / M > 0. Same widening applied to weekly + monthly tiles. New catalogue keys `stats.tile_daily_quicklogs`, `stats.tile_daily_distractions` (period-equivalent keys for weekly/monthly per FR-027) (each suffix-paired `_one`/`_other` per FR-031). |
| `src/locales/{en,de,it,tr}.json` | edit | New keys per FR-031 — typed-key catalogue from feature 005. EN/DE/IT translations in scope; TR may fall back to EN per spec Clarifications. |
| `tests/e2e/fixtures/tauriMock.js` | edit | Four new commands. Mock-first per Principle V + `AGENTS.md` "Don't add Tauri commands without extending the mock first." |

## Tauri command surface changes

Four new commands, all `async`, all returning `Result<T, BridgeError>`. Wired into the existing `invoke_handler!` block in `src-tauri/src/lib.rs`.

```rust
#[tauri::command]
async fn save_quick_logs(quick_logs: Vec<QuickLog>, app: AppHandle) -> Result<(), BridgeError>;

#[tauri::command]
async fn load_quick_logs(app: AppHandle) -> Result<Vec<QuickLog>, BridgeError>;

#[tauri::command]
async fn save_distractions(distractions: Vec<Distraction>, app: AppHandle) -> Result<(), BridgeError>;

#[tauri::command]
async fn load_distractions(app: AppHandle) -> Result<Vec<Distraction>, BridgeError>;
```

Save-side commands validate at the boundary per FR-022:

- `QuickLog::title` length in `1..=120` (UTF-8 char count, not bytes — matches the inline-input `maxlength` behaviour).
- `QuickLog::elapsed_minutes` in `1..=720`.
- `Distraction::note` length in `1..=120`.

Validation failures return `BridgeError::InvalidArgument { field, reason }` (existing variant in `crates/presto-ipc/src/error.rs:29-65`). No new error variants needed.

Files persisted alongside existing JSON in the Tauri app-data dir: `quick_logs.json`, `distractions.json`. Missing files deserialise to `Vec::new()` via `#[serde(default)]` on read, mirroring the `manual_sessions.json` precedent — no migration step.

Mock-first: `tests/e2e/fixtures/tauriMock.js` gets the four commands first (default empty-vec returns; per-spec overrides), then RED tests, then real handlers. Per `AGENTS.md`.

Full text contracts including argument/return/error shapes live in `contracts/persistence-commands.md`.

## Engine state-machine changes

The engine module is `src/src/engine/timer.rs`. Today's relevant surface:

- Run-state bools: `is_running`, `is_paused`, `is_auto_paused` (lines 119-173). Unchanged.
- `current_session_elapsed_secs: u32` (lines 145-152). Read by `complete`. Cleared by `abort`.
- `pause(clock)` (lines 664-683). Unchanged.
- `resume(clock)` (lines 708-722). Unchanged.
- `skip()` (lines 396-445). Unchanged. Continues to NOT count, including in continuous mode (FR-018).
- `reset()` (line 493). Unchanged.
- Natural-completion sequence (lines 808-872). The reference for `complete` side effects. Unchanged.
- Continuous-mode branch (line 810). Unchanged. `complete` is the only path that can count a paused overtime session.

New engine entry points (full preconditions, postconditions, and event emissions in `contracts/timer-engine-actions.md`):

- **`abort(clock)`** — idempotent. Valid from Running, Paused, AutoPaused. Clears the three run-state bools and `current_session_elapsed_secs`. Does NOT change `current_mode`. Does NOT touch `completed_pomodoros`, `total_focus_secs`, or `pomodoros_until_long_break`. Emits exactly `[TimerEvent::SessionAborted { aborted_mode, elapsed_secs }]` (the latter recorded before clearing, for observability — never reaches disk, Principle II).
- **`complete(clock)`** — idempotent. Precondition: `is_paused || is_auto_paused` (after `pause()` has settled wall-clock delta per FR-013a). From any other state, returns `[]` and is a no-op (matches the idempotence rule). Reads `current_session_elapsed_secs`. **If `< 30`**: internally calls `abort(clock)` and returns its events. **If `>= 30`**: branches on `session_completed_but_not_saved`. **Branch B.1 (flag false — normal paused-before-zero):** traverses the same side-effect sequence as natural completion at lines 808-872 via the shared helper `complete_focus_session` — `completed_pomodoros += 1`, `total_focus_secs += current_session_elapsed_secs`, `current_session_elapsed_secs = 0`, emits `TimerEvent::PomodoroCompleted { completed_pomodoros }` followed by `TimerEvent::SessionCompletedEarly { elapsed_secs }`, consults the long-break cadence (same `sessions_per_long_break` field at `crates/presto-ipc/src/settings.rs:260-307`), advances `current_mode` to either `Break` or `LongBreak`, sets `is_running = false`, clears `is_paused` and `is_auto_paused`. **Branch B.2 (flag true — continuous-mode overtime):** zero-cross already incremented `completed_pomodoros` + emitted `PomodoroCompleted`. The helper integrates only the overtime portion into `total_focus_secs`, resets the per-session accumulator, clears the flag, **suppresses** `PomodoroCompleted` re-emission, advances mode per the cadence already computed at the zero-cross, stops running, emits `SessionCompletedEarly` per the uniform-emission rule. The auto-restart path at `src/src/components/timer/mod.rs:1471-1483` (gate extended in this PR to additionally check the events vec for `PomodoroCompleted`) fires after B.1 and after natural completion; in B.2 the auto-restart already fired at the zero-cross.

**Continuous-mode interaction**: in continuous mode the engine never zero-crosses, so a Focus session past `focus_duration` paused into overtime is the only state from which a counted-end is reachable. `complete` reads the actual elapsed (including overtime), seals as exactly one pomodoro, runs the cadence check, advances mode. `abort` discards. `skip` advances without counting (status quo). This is the documented "the only way to end a continuous-mode session with a count" semantics (FR-016 + Story 1 AC 4).

**Smart-pause / AutoPaused interaction**: `complete` from AutoPaused is identical to `complete` from Paused (FR-013, Story 1 AC 3 + Clarifications). The < 30 s rule applies equally — if the user auto-pauses within the first 30 s of Focus, `complete` discards as Abort (Edge Cases). `[BEST-GUESS PM DECISION]` — see Best-guess decisions section on whether continuous-mode AutoPaused-during-overtime is a real concern.

## UI surface changes

Driven exclusively by the closed-sum `RunState` × existing `TimerMode` (`crates/presto-ipc/src/timer.rs:20-27`). Derivation:

```rust
// src/src/components/timer/mod.rs — pseudocode at the top of the matrix module
let run_state = Signal::derive(move || {
    engine.with(|t| RunState::from_engine(t.is_running, t.is_paused, t.is_auto_paused))
});
```

`RunState::from_engine` (defined in `data-model.md`) checks paused-or-autopaused **first** (these strictly imply not-running), then running, then defaults to Idle. The mapping carries a `debug_assert!` to crash dev builds on illegal `(true, true|true, *)` states per Principle III (illegal states impossible).

The matrix is wired off this signal as an exhaustive `match`, never via string comparisons or flag-bool conditions (Principle III).

**Combined pill** (FR-001 → FR-006) — refactor at `src/src/components/timer/mod.rs:1727-1999`. Wrap `#timer-status` (chip + mode label + chevron) and `#session-title-input` in a new `#timer-status-pill` container. Both selector IDs preserved so existing e2e selectors continue to resolve (SC-007). In Focus Idle, both children interactive. In Running/Paused (and the AutoPaused-renders-as-Paused case), chevron hides and title input carries `readonly`. In Break/LongBreak Idle, the title region renders nothing and the pill collapses to chip + mode label (FR-006). Per FR-031, the placeholder string comes from a new catalogue key `timer.pill_title_placeholder`.

**State-aware button matrix** (FR-012) — replace today's three independent buttons at `src/src/components/timer/mod.rs:2034-2128`. Each slot is a single `<button>` whose `label`, `icon`, `class` (ghost vs filled), `on:click`, and `aria-label` flip on `(run_state, mode)`. Per slot:

- **Left slot** — Idle ⇒ `+ Quick Log` (`timer.ctrl_quick_log`, ghost). Running ⇒ `✕ Abort` (`timer.ctrl_abort`, ghost). Paused ⇒ `✕ Abort` (`timer.ctrl_abort`, ghost).
- **Center slot** — Idle ⇒ `▶ Play` (existing key, filled). Running ⇒ `⏸ Pause` (existing key, filled). Paused ⇒ `▶ Resume` (existing key, filled).
- **Right slot** — Idle ⇒ `→ Skip Mode` (`timer.ctrl_skip_mode`, renamed from today's `timer.ctrl_skip_session`, ghost). Running ⇒ `! Note Distraction` (`timer.ctrl_note_distraction`, ghost, label `Distraction`). Paused ⇒ `✓ Complete` (`timer.ctrl_complete`, filled).

Today's `StopButtonState` and its `Undo` variant at `src/src/components/timer/mod.rs:225-238` is removed entirely (FR-028 + Removal note in spec). The `t_string!(i18n, timer.ctrl_undo)` / `timer.ctrl_undo_aria` lookups go with it. Catalogue cleanup: dead `ctrl_undo*` keys MUST be pruned in this PR per FR-028a.

**Quick Log modal** (`src/src/components/timer/quick_log_modal.rs`, FR-019) — title field auto-focused, `maxlength=120`, required. Minutes field numeric, min=1, max=720, default=5. Submit + cancel buttons. Submit gated on title non-empty AND minutes in range. Submission calls `QuickLogManager::add(QuickLog::new(title, minutes))` — never touches the engine. Reachable from the Idle left button (timer view) AND the Inventory header `+ Quick Log` button (FR-025).

**Distraction modal** (`src/src/components/timer/distraction_modal.rs`, FR-020) — single text field auto-focused, `maxlength=120`, required. Enter submits, Escape cancels. Modal closes immediately on submit (no toast). The parent-session ref is snapshotted **at modal-open time** (not submit time) — guards the natural-completion race per Edge Cases. Reachable from the Running right button only. Submission calls `DistractionManager::add(Distraction::new_with_ref(note, parent_ref))` — never touches the engine (FR-035).

**Inventory subsection** (`src/src/components/daily/inventory.rs`, FR-023 → FR-026) — new subsection inside the existing Stats / Calendar area, positioned after the existing sessions-history table. Two sub-subsections: `Quick logs` and `Distractions`. Each row gets Edit + Delete per the `sessions_history_table.rs` pattern (modal-based row-edit; bulk re-save on delete). Date-filter inherits the existing daily/weekly/monthly period selector — Inventory shows entries whose `date` matches the selected day. The Inventory header carries a `+ Quick Log` button (FR-025) opening the identical Quick Log modal.

**Stats tile widening** (`src/src/components/stats/mod.rs:431-457`, FR-027) — append `· N quicklogs` when `N > 0` and `· M distractions` when `M > 0` to the pomodoro-count tile label. Period-specific catalogue keys: `stats.tile_daily_quicklogs`, `stats.tile_daily_distractions` (plus weekly/monthly equivalents per FR-027) (each suffix-paired `_one`/`_other` per FR-031). Counts are computed from `QuickLogManager.entries` filtered by the period's date range; same for distractions.

**Removed surface** — `StopButtonState::Undo` and its UI branches (FR-028). The equivalent outcome (undo last pomodoro) remains reachable via per-row delete in the sessions-history table.

## Visual regression budget

Per FR-029 / FR-030 and Principle IV.

| Baseline | Change | One-line PR note (draft) |
|---|---|---|
| `timer-chromium-linux.png` | Combined pill (single container around chip+title), left button label `+ Quick Log` (was the Idle/Focus state's existing left button), right button still `→ Skip Mode` (renamed `Skip session` → `Skip Mode`). | "Timer Idle: combined `#timer-status-pill` replaces separate controls; left button renamed `+ Quick Log`; right button renamed `Skip Mode`." |
| `timer-focus-paused-with-complete-chromium-linux.png` (NEW) | Paused state baseline — collapsed pill (chevron hidden, title `readonly`), three-control triad `✕ Abort · ▶ Resume · ✓ Complete`. Single highest-value new visual surface for the feature. | "Timer Focus Paused (new baseline): combined pill collapsed read-only; new right-slot `✓ Complete` button revealed." |
| `daily-chromium-linux.png` | Inventory subsection appended below sessions-history. Empty-state lines render when the seeded day has no entries. | "Daily view: new Inventory subsection appended below sessions-history table (empty-state lines for the seeded day)." |
| `statistics-daily-chromium-linux.png` | The daily Stats tile label format changes from `K pomodoros` to `K pomodoros[· N quicklogs][· M distractions]`. **Recommendation**: keep the canonical baseline at the zero-suffix scenario (no quick logs, no distractions for the seeded day) so the tile label is visually unchanged. If the seeded day has non-zero quick logs/distractions, regenerate with the new suffix visible. | "Daily Stats tile: label format widened (suffixes hidden when zero, so the canonical baseline is visually unchanged)." |
| `statistics-weekly-chromium-linux.png` / `statistics-monthly-chromium-linux.png` | Same widening as daily, per FR-027. Same zero-suffix recommendation. | "Weekly/Monthly Stats tile: label format widened (suffixes hidden when zero)." |

**No baselines outside this set are expected to regenerate** (SC-008). Any unrelated diff is a code regression, not a baseline absorption.

**Decision**: **add `timer-focus-paused-with-complete-chromium-linux.png` baseline in this PR** — the Paused state with the new `✓ Complete` button revealed is the single highest-value new visual surface for the feature. **Defer `timer-focus-running-chromium-linux.png`** to a follow-up. Rationale: Principle IV — VR is the UI contract; the most novel visual surface MUST be baselined this PR. The Running-state collapsed-pill change is smaller and DOM-assertable via `timer-distraction.spec.js`. Update the PR-note checklist to include the new Paused baseline + a `[BEST-GUESS PM DECISION]` note for the deferred Running baseline. If `speckit-architecture-guard` pushes back on the Running deferral in the post-tasks review, revisit.

## Test plan

Test-first commit ordering per **V. Test-First For Stateful Engines** (AGENTS.md "Test-first commit ordering"). RED commit precedes GREEN commit for all engine + manager + Tauri-boundary tests. UI plumbing is out of Principle V scope and is covered by the e2e suite.

**Engine RED tests** in `src/src/engine/timer.rs` (`#[cfg(test)] mod tests`):

| Test | Asserts |
|---|---|
| `abort_clears_elapsed_and_returns_to_idle_from_running` | From Running, `abort()` zeros `current_session_elapsed_secs`, clears `is_running` (and the pause bools), does NOT change `current_mode`, emits exactly `[SessionAborted { … }]`, clears `session_completed_but_not_saved`. |
| `abort_clears_elapsed_from_paused_and_autopaused` | Same as above from `Paused` and `AutoPaused`. Idempotent: calling `abort()` again returns `[]`. |
| `abort_does_not_touch_completed_pomodoros_or_cadence` | `completed_pomodoros` and `pomodoros_until_long_break` unchanged across abort. |
| `abort_does_not_trigger_auto_restart` | The auto-restart gate at `src/src/components/timer/mod.rs:1471-1483` (extended in this PR to also require `PomodoroCompleted` in the events vec) never fires after `abort` — `abort` emits only `SessionAborted`. |
| `complete_from_paused_with_elapsed_30_increments_count` | Elapsed=30 ⇒ `completed_pomodoros += 1`, `total_focus_secs += 30`, emits `PomodoroCompleted` + `SessionCompletedEarly { elapsed_secs: 30 }`. |
| `complete_from_paused_with_elapsed_29_acts_as_abort` | Elapsed=29 ⇒ no count, no advance, returns to Idle in same mode, emits `[SessionAborted { elapsed_secs: 29 }]`. |
| `complete_from_autopaused_same_as_paused` | Pre-condition matrix: AutoPaused triggers the same code path as Paused. |
| `complete_in_continuous_mode_seals_with_overtime_elapsed` | In continuous mode with elapsed=`focus_duration + 120` (post-zero-cross overtime), `complete` seals at the overtime portion, advances mode. Asserts `SessionCompletedEarly` appears in the returned event vec **in addition to** the seal behaviour. Asserts `PomodoroCompleted` is NOT re-emitted (canonical one already fired at the zero-cross). |
| `complete_in_continuous_overtime_does_not_double_count` | Asserts `completed_pomodoros` increments by exactly 1 across the full zero-cross-then-`complete` sequence (regression test for the flag-driven branching). |
| `complete_from_autopaused_in_continuous_overtime` | Intersection: smart-pause during continuous-mode overtime + then-`complete`. Count incremented exactly once across the whole sequence, overtime elapsed integrated into `total_focus_secs`, `session_completed_but_not_saved` cleared, mode advanced to break. |
| `complete_at_exactly_30s_wall_clock_counts_not_aborts` | Pause 30.0 s of wall-clock after start (asserts `pause()` settles wall-clock delta per FR-013a), complete, assert count incremented (not discarded as Abort due to a 29 s read). |
| `complete_advances_mode_via_cadence_check` | With `sessions_per_long_break=4` and `completed_pomodoros` reaching 4 after `complete`, mode advances to `LongBreak`; otherwise `Break`. |
| `complete_idempotent_from_running_is_noop` | From Running (precondition fail), `complete` returns `[]` and does not mutate state. |
| `pause_at_zero_cross_lets_natural_completion_win` | Pause clicked in the same tick the timer naturally hits zero: the natural-completion sequence wins. User lands in next-mode Idle with the pomodoro counted. `complete` is unreachable. |

**Manager RED tests** for `QuickLogManager` and `DistractionManager` in their respective module test blocks:

- `add_then_load_round_trips_entry` — `add` then `load` (with a mock Tauri bridge) yields the entry.
- `update_replaces_in_place` — `update(id, new_t)` replaces by id, preserves vec order.
- `delete_removes_only_target` — `delete(id)` removes only the target row; bulk re-save fires.
- `validation_rejects_out_of_range_quick_log_minutes` — `add(QuickLog { elapsed_minutes: 0 })` and `=721` rejected before save.
- `validation_rejects_title_or_note_over_120` — character count enforced.
- `bridge_unavailable_short_circuits_gracefully` — when `window.__TAURI_INTERNALS__` is absent (per `AGENTS.md`), manager methods do not panic; they no-op or return an in-memory result.
- `parent_ref_snapshotted_at_modal_open_not_submit` — manager-level cooperation test (the modal captures, the manager records).

**Tauri command RED tests** (`src-tauri/src/lib.rs` test module + wasm-bindgen-test from the frontend side):

- `save_quick_logs_round_trip` — save then load returns identical vec.
- `save_quick_logs_rejects_out_of_range_minutes` — `BridgeError::InvalidArgument { field: "elapsedMinutes", … }`.
- `save_quick_logs_rejects_overlong_title` — same with `field: "title"`.
- `save_distractions_round_trip` — save then load returns identical vec.
- `save_distractions_rejects_overlong_note` — `BridgeError::InvalidArgument { field: "note", … }`.
- `load_returns_empty_when_file_missing` — both commands return `Vec::new()` when the JSON file is absent. (Equivalent to `#[serde(default)]` deserialisation.)
- `load_handles_corrupt_file_with_bridge_error_internal` — corrupt non-JSON / malformed payload yields `BridgeError::Internal { msg }` whose reason string contains no characters from the corrupt payload (PII-scrub conduit; see contracts/persistence-commands.md).

**E2E RED tests** in `tests/e2e/` (mock-first per Principle V; `tauriMock.js` extended first):

- `tests/e2e/timer-quick-log.spec.js` — Idle left-slot label, modal open, validation, submission, entry appears in mocked Inventory; pomodoro counter untouched.
- `tests/e2e/timer-distraction.spec.js` — Running right-slot label, modal open, Enter submits, Escape cancels, timer keeps ticking, distraction persists with parent-ref.
- `tests/e2e/timer-complete.spec.js` — Pause → ✓ Complete (elapsed ≥ 30) increments count and advances; (elapsed < 30) discards as Abort.
- `tests/e2e/timer-abort.spec.js` — ✕ Abort suppresses pending auto-restart, title persists, no count, no advance. Covers `abort_does_not_trigger_auto_restart`: the auto-restart gate at `src/src/components/timer/mod.rs:1471-1483` extended in this PR to also require `PomodoroCompleted` in the events vec → Abort (which emits only `SessionAborted`) does not trigger.
- `tests/e2e/inventory.spec.js` — Subsection renders, per-row Edit + Delete, header `+ Quick Log` opens the identical modal, day-filter swaps the row set.

**Visual-regression scenario seeding** — the existing daily-stats baselines are regenerated using a fixture that yields zero quick logs and zero distractions for the seeded day, so the tile label suffix stays hidden by default. A separate non-baseline e2e assertion (`inventory.spec.js`) covers the non-zero case via DOM text.

## Constitution mapping

Only deviations + justifications. (Pass-affirmations omitted per standing constraint.)

No principle violations. No `Complexity Tracking` entries.

Notes:

- The new `TimerEvent::SessionCompletedEarly` variant is engine-internal observability — it is never serialised to disk and never sent over the Tauri bridge. Local-only (Principle II) holds.
- The free-text `note` and `title` fields are PII per Principle II "PII never appears in plain logs." Persisted JSON on disk is fine (local store). Stderr / panic messages MUST elide the content — mirror the existing `ManualSession.notes` / `title` redaction pattern. (Spec Edge Cases item.)

## CI / quality gates touched

- **Backend clippy + fmt** (per Principle X / `AGENTS.md`): new code in `crates/presto-ipc/`, `src-tauri/`, and the Leptos crate clears `cargo clippy --all-targets -- -D warnings -W clippy::pedantic` with zero new `#[allow]` (SC-011).
- **Frontend wasm-bindgen-test**: new manager tests and engine tests run under the existing test runner.
- **Playwright + VR**: new specs and regenerated baselines (Visual regression budget table).
- **Pre-commit hook**: lockfile-drift check unchanged. Manifest changes (none expected) would require `Cargo.lock` in the same commit per Principle IX.
- **`.agentex.yml` pipeline**: no changes — the existing pipeline executes the new tests + VR suite.

## Migration / lockfile notes

- **No data migration.** Missing `quick_logs.json` and `distractions.json` deserialise to `Vec::new()` via `#[serde(default)]` on read. Mirrors the `manual_sessions.json` precedent at install/upgrade time.
- **No new Cargo dependencies expected.** `chrono`, `uuid`, `serde`, `serde_json`, `specta` are already in the workspace.
- **If a transitive bump is incidentally pulled in**, `Cargo.lock` lands in the same commit per Principle IX. CI uses `cargo build --frozen` — drift fails loudly.
- **i18n catalogue files** (per FR-031 + feature 005) gain new keys; the typed-key compile-time check catches missing keys. No library swap.
- **`StopButtonState::Undo` removal** — the unused `timer.ctrl_undo` and `timer.ctrl_undo_aria` keys MUST be pruned from the catalogue files (`src/locales/{en,de,it,tr}.json`) in this PR per FR-028a. Dead keys would be silently load-bearing and drift-prone; Principle VII says no upstream compatibility burden, so pruning is unambiguously correct.
- **Updater path** (Principle VII): existing presto users on the current release get the new `quick_logs.json` / `distractions.json` created lazily on first save; their existing data (`sessions.json`, `manual_sessions.json`, `tags.json`, etc.) is untouched. No back-compat work.

## Risk register

| Rank | Risk | Mitigation |
|---|---|---|
| 1 | `complete`'s long-break cadence check diverges from the natural-completion path under continuous mode, mis-counting overtime pomodoros toward the cadence (e.g., one overtime session being treated as two cadence ticks). | RED tests `complete_in_continuous_mode_seals_with_overtime_elapsed` + `complete_in_continuous_overtime_does_not_double_count` + `complete_advances_mode_via_cadence_check` all with continuous-mode fixtures. Implementation MUST factor the natural-completion sequence at `src/src/engine/timer.rs:808-872` into a private helper `complete_focus_session(&mut self) -> Vec<TimerEvent>` consumed by both the natural-completion zero-cross path and `complete`'s branch-B body — no path duplication. The helper branches on `session_completed_but_not_saved` to decide whether to increment / emit `PomodoroCompleted`, sealing the overtime sub-branch as additive-only. |
| 2 | The auto-restart at `src/src/components/timer/mod.rs:1471-1483` is currently gated **only** on the running-transition predicate `was_running && !state.is_running()` with no event check — so `abort` from Running would unintentionally trigger auto-restart. | Extend the gate in this PR to also require `events.iter().any(|e| matches!(e, TimerEvent::PomodoroCompleted { .. }))` (matching the existing event-check pattern used at line 1429 for the session-save gate). Under the new gate: natural completion + branch-B.1 `complete` emit `PomodoroCompleted` → auto-restart fires (intentional). Branch-B.2 `complete` does not re-emit, but auto-restart already fired at the zero-cross. `abort` emits only `SessionAborted` → auto-restart does NOT fire (intentional). RED test `abort_does_not_trigger_auto_restart` asserts at the event-stream + UI-gate level. E2E `timer-abort.spec.js` asserts at the DOM level. SC-010 is the contract. |
| 3 | Combined-pill restructure breaks selector stability for existing e2e tests targeting `#timer-status` and `#session-title-input`, even though the IDs are preserved — CSS positioning or z-index changes cause an unexpected baseline diff outside the FR-029 set. | Refactor wraps both children **inside** `#timer-status-pill` without moving them in the layout grid. Visual diff on a non-FR-029 baseline is treated as a code regression per SC-008. Pre-PR design QA loop (see user-memory `feedback_design_qa_loop`) inspects light+dark screenshots before opening PR. |

## Best-guess decisions made / `[BEST-GUESS PM DECISION]` markers

1. **`[BEST-GUESS PM DECISION]`** Defer new `timer-running-chromium-linux.png` / `timer-paused-chromium-linux.png` VR baselines to a follow-up. — Rationale: today's timer-view VR coverage is Idle-only; adding two more baselines triples the count for one view. Per-state visual changes are covered by e2e DOM assertions. If `architecture-guard` pushes back in the post-tasks review, revisit. (Visual regression budget section.)
2. **`[BEST-GUESS PM DECISION]`** Name the UI-layer enum `RunState` (variants `Idle | Running | Paused`; AutoPaused folds into Paused at the matrix layer). — Rationale: closest to the spec's "run-state" vocabulary; keeps the engine bools untouched (engine refactor is explicit Out of Scope); the AutoPaused-folds-into-Paused rule is per FR-012 paragraph 3 + Story 1 AC 3.
3. **`[BEST-GUESS PM DECISION]`** `complete` from continuous-mode AutoPaused (smart-pause kicks in during overtime) is treated identically to AutoPaused-in-normal-mode — engine seals with the actual elapsed (which exceeds `focus_duration`), counts one, advances. — Rationale: continuous-mode's documented "the only way to count an overtime session" semantics doesn't carve out smart-pause; AutoPaused-as-Paused parity (FR-012, Story 1 AC 3) carries through. If the PM disagrees, the RED tests `complete_from_autopaused_same_as_paused` + `complete_in_continuous_mode_seals_with_overtime_elapsed` should be combined into a single test that exercises the intersection, and the spec's Edge Cases section gets a new bullet.
4. Pruning the now-dead `timer.ctrl_undo` / `timer.ctrl_undo_aria` catalogue keys in the same PR as the `StopButtonState::Undo` removal is now mandated by FR-028a (not a hedge). The catalogue files `src/locales/{en,de,it,tr}.json` MUST drop these keys. Dead keys would be silently load-bearing.
5. **`[BEST-GUESS PM DECISION]`** Use the existing `BridgeError::InvalidArgument { field, reason }` variant for FR-022 validation failures. No new `BridgeError` variants. — Rationale: the existing variant carries enough semantic context. New variants would be churn without payoff.
6. **`[BEST-GUESS PM DECISION]`** Statistics tiles for weekly and monthly periods follow the same `· N quicklogs · M distractions` suffix rule as daily, but with period-aggregated counts and period-specific catalogue keys (`stats.tile_weekly_quicklogs`, `stats.tile_monthly_quicklogs`, etc.) (each suffix-paired `_one`/`_other` per FR-031). — Rationale: spec FR-027 mandates "same widening applies to weekly and monthly tiles". The catalogue key names follow the daily-pattern naming.

## Unresolved questions for the PM

(Soft — won't block tasks/implement.)

- Should the engine emit `SessionCompletedEarly` for every `complete`-with-count, or only when elapsed strictly less than `focus_duration`? Plan currently emits it always in branch B (overtime counts here too — including the B.2 continuous-mode overtime sub-branch). If the PM wants strict "early only" semantics, the test `complete_in_continuous_mode_seals_with_overtime_elapsed` would need a flag check.
- Confirm `[BEST-GUESS PM DECISION]` item 3 (continuous-mode AutoPaused) — now exercised by the new RED test `complete_from_autopaused_in_continuous_overtime`.
