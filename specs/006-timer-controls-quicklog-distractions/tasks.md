# Tasks: Timer Control Rework + Quick Log + Distraction Capture

**Input**: Design documents from `/specs/006-timer-controls-quicklog-distractions/`
**Prerequisites**: spec.md, plan.md, data-model.md, contracts/, quickstart.md (all present).

**Tests**: Test-first **mandatory** for Principle V scope (timer engine, managers, Tauri-boundary helpers). UI plumbing & modal wiring covered by the Playwright e2e suite. **RED commit MUST precede GREEN commit** for every test-first task — AGENTS.md "Test-first commit ordering" is enforced inline on each engine / manager / persistence task pair.

**Organization**: 9 stack-ordered phases (domain → engine RED → engine GREEN → persistence → managers → UI → inventory/stats → catalogue → e2e/VR). Each task carries the user story (`[US1]`–`[US5]`) it advances for traceability. The phase ordering reflects the dependency graph (you cannot write engine GREEN before engine RED; you cannot wire a UI button before its target method exists; you cannot write an e2e spec before its mock command).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks).
- **[Story]**: User story tag (`[US1]`..`[US5]`). Some cross-cutting tasks span multiple stories — listed with the leading-priority story.
- Exact file paths included in every task. FR and SC mappings appended per task.

---

## Phase 1: Domain types + IPC shapes

**Goal**: Land the new typed structs and `TimerEvent` variants. No engine logic yet — just the closed-sum surface that downstream phases depend on.

**FRs covered**: FR-001 (RunState scaffolding, type-level), Key Entities (`QuickLog`, `Distraction`, `DistractionParentRef`).
**SCs advanced**: SC-006 (parent-ref typing), SC-007 (RunState scaffolding feeds the matrix).

- [ ] T001 [P] [US3] Create `crates/presto-ipc/src/quick_log.rs` with the `QuickLog` struct per `data-model.md` lines 17-46 (`id`, `title`, `elapsed_minutes`, `created_at`, `date` — `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]`, `#[cfg_attr(feature = "specta", derive(specta::Type))]`, `#[serde(rename_all = "camelCase")]`). No validation logic in the struct yet — boundary validation lives at the Tauri command. FR maps: FR-019, Key Entities (QuickLog).
- [ ] T002 [P] [US2] Create `crates/presto-ipc/src/distraction.rs` with the `Distraction` and `DistractionParentRef` structs per `data-model.md` lines 56-93. `parent_ref` uses `#[serde(default)]` so retroactive entries deserialize cleanly with `None`. FR maps: FR-020, Key Entities (Distraction, DistractionParentRef).
- [ ] T003 [US2] Edit `crates/presto-ipc/src/lib.rs` — add `pub mod quick_log; pub mod distraction;` and `pub use quick_log::QuickLog; pub use distraction::{Distraction, DistractionParentRef};`. **Done-signal**: `cargo build --frozen --manifest-path crates/presto-ipc/Cargo.toml` succeeds. FR maps: FR-021 (re-exports feed bridge/types).
- [ ] T004 [US1] Append two new variants to `TimerEvent` at `src/src/engine/timer.rs` (the in-process enum at line 24): `SessionAborted { aborted_mode: TimerMode, elapsed_secs: u32 }` and `SessionCompletedEarly { elapsed_secs: u32 }`. Variants placed at the end of the enum for readability. **Done-signal**: `cargo check --manifest-path src/Cargo.toml` succeeds (no use sites yet — additive enum). FR maps: FR-013, FR-017, FR-034.
- [ ] T005 [US1] Add the closed-sum `RunState` enum + `RunState::from_engine(is_running, is_paused, is_auto_paused) -> Self` constructor per `data-model.md` lines 132-157 to `src/src/components/timer/mod.rs` (new section near the top of the file; visibility `pub(super)`). Include the `debug_assert!(!(is_running && (is_paused || is_auto_paused)))` for illegal-state crashes in dev builds (Principle III; AG-1 finding). The mapping checks paused-or-autopaused **first** to prevent `(false, true, false)` rendering as Idle. **Done-signal**: `cargo check --manifest-path src/Cargo.toml`. FR maps: FR-012, Key Entities (RunState), SC-007.
- [ ] T006 [P] [US3] Edit `src/src/bridge/types.rs` to re-export the new IPC types (`QuickLog`, `Distraction`, `DistractionParentRef`) for the manager layer, following the existing re-export pattern. FR maps: FR-021, FR-032, FR-033 (manager imports follow SessionManager precedent per AG-7).
- [ ] T007 [P] [US3] Edit `src/src/bridge/commands.rs` to add the four new `invoke`-wrapper function signatures (`load_quick_logs`, `save_quick_logs`, `load_distractions`, `save_distractions`) returning `Result<…, BridgeError>` — wire-shape only; the bodies forward to the existing invoke helper. FR maps: FR-021.

**Phase 1 checkpoint**: All new typed surfaces compile. Engine `TimerEvent` has the two new variants but no emitter yet. `RunState` exists but no matcher yet. The build is green; no behaviour change visible to a user.

---

## Phase 2: Engine RED tests (failing-first)

**Goal**: Write the engine unit tests. All MUST fail with `unimplemented!()` / `compile-error` / panic before any GREEN code lands. Per AGENTS.md, the RED commit precedes the GREEN commit — no combined commit allowed.

**Process per task**: Write the test, run `cargo test --manifest-path src/Cargo.toml -- <test_name>`, confirm it fails for the **expected** reason (missing method / panicking call, not a typo). Commit RED. Only then proceed to Phase 3.

**FRs covered**: FR-013, FR-013a, FR-014, FR-015, FR-016, FR-017, FR-034, FR-035. **SCs advanced**: SC-001, SC-004, SC-010, SC-012.

- [ ] T008 [US1] Add stub method signatures `pub fn abort(&mut self, clock: &impl Clock) -> Vec<TimerEvent> { unimplemented!() }` and `pub fn complete(&mut self, clock: &impl Clock) -> Vec<TimerEvent> { unimplemented!() }` to `src/src/engine/timer.rs` so the test file compiles. **Done-signal**: `cargo check --manifest-path src/Cargo.toml`. Marked clearly as stubs (one-line comment). FR maps: FR-034.
- [ ] T009 [US1] Write RED test `abort_clears_elapsed_and_returns_to_idle_from_running` in `src/src/engine/timer.rs` `#[cfg(test)] mod tests` — asserts: `is_running=false, is_paused=false, is_auto_paused=false`, `current_session_elapsed_secs=0`, `current_mode` unchanged, emits exactly `[SessionAborted { aborted_mode, elapsed_secs }]`. **Done-signal**: `cargo test … abort_clears_elapsed_and_returns_to_idle_from_running` panics with `unimplemented!` (RED). FR maps: FR-017, FR-034.
- [ ] T010 [US1] Write RED test `abort_clears_elapsed_from_paused_and_autopaused` — same checks as T009 from `Paused` and `AutoPaused`; second `abort()` call returns `[]` (idempotence). **Done-signal**: test fails RED. FR maps: FR-017.
- [ ] T011 [US1] Write RED test `abort_does_not_touch_completed_pomodoros_or_cadence` — `completed_pomodoros` and `pomodoros_until_long_break` unchanged. FR maps: FR-017.
- [ ] T012 [US1] Write RED test `abort_does_not_trigger_auto_restart` — abort emits only `SessionAborted` (no `PomodoroCompleted`); the auto-restart gate at `src/src/components/timer/mod.rs:1471-1483` (after Phase 6 extension) does not fire. Asserts at the event-stream level only — UI-level assertion lives in `tests/e2e/timer-abort.spec.js`. FR maps: FR-017, SC-010.
- [ ] T013 [US1] Write RED test `abort_clears_session_completed_but_not_saved_flag` — from continuous-mode overtime (`session_completed_but_not_saved=true`), `abort` clears the flag (mirrors `skip` at lines 407-411). Prevents flag-leak into the next session. FR maps: FR-017, FR-034.
- [ ] T014 [US1] Write RED test `complete_from_paused_with_elapsed_30_increments_count` — pause at elapsed=30, `complete` increments `completed_pomodoros`, accumulates `total_focus_secs += 30`, emits `[PomodoroCompleted, SessionCompletedEarly { elapsed_secs: 30 }]`. FR maps: FR-013, FR-014, SC-001, SC-004.
- [ ] T015 [US1] Write RED test `complete_from_paused_with_elapsed_29_acts_as_abort` — pause at elapsed=29, `complete` returns `[SessionAborted { elapsed_secs: 29 }]`, no count, no advance, returns to Idle in same mode. FR maps: FR-015, SC-012.
- [ ] T016 [US1] Write RED test `complete_from_autopaused_same_as_paused` — from `AutoPaused` with elapsed≥30, identical effects to Paused-complete. FR maps: FR-013, Story 1 AC 3.
- [ ] T017 [US1] Write RED test `complete_in_continuous_mode_seals_with_overtime_elapsed` — continuous mode, pause at `focus_duration + 120` (overtime, post-zero-cross), `complete` seals the overtime portion into `total_focus_secs`, advances mode. Asserts `SessionCompletedEarly` IS emitted; `PomodoroCompleted` is NOT re-emitted (zero-cross already fired it). FR maps: FR-016, Story 1 AC 4.
- [ ] T018 [US1] Write RED test `complete_in_continuous_overtime_does_not_double_count` — full sequence (zero-cross → continued ticking → pause → complete): `completed_pomodoros` increments exactly 1. Regression test for the flag-driven branching. FR maps: FR-013, FR-016, SC-004.
- [ ] T019 [US1] Write RED test `complete_from_autopaused_in_continuous_overtime` — intersection of smart-pause + continuous mode: `complete` from auto-paused during overtime; count incremented exactly once, overtime integrated, flag cleared, mode advanced. FR maps: FR-013, FR-016.
- [ ] T020 [US1] Write RED test `complete_at_exactly_30s_wall_clock_counts_not_aborts` — start, wait 30.0 s wall-clock, pause (asserts `pause()` settles wall-clock delta per FR-013a), `complete` increments count (not discarded as Abort). The headline FR-013a test. FR maps: FR-013a, FR-014, SC-001, SC-012.
- [ ] T021 [US1] Write RED test `complete_advances_mode_via_cadence_check` (parameterised: `sessions_per_long_break ∈ {2, 3, 4}`) — `completed_pomodoros` reaching the cadence advances to `LongBreak`; else `Break`. FR maps: FR-013.
- [ ] T022 [US1] Write RED test `complete_idempotent_from_running_is_noop` — from Running (precondition fail), `complete` returns `[]` and does not mutate state. FR maps: FR-013, FR-034.
- [ ] T023 [US1] Write RED test `pause_at_zero_cross_lets_natural_completion_win` — pause clicked in the same tick the timer naturally hits zero: natural-completion sequence wins (`PomodoroCompleted` emitted, mode advanced); `complete` is unreachable. FR maps: FR-013, Edge Cases (zero-cross race), Story 1 AC 6.

**Phase 2 checkpoint**: All engine RED tests fail with `unimplemented!()` panics (or assertion failures against the `pause()` wall-clock-settle fix). **RED commit lands here.** No engine logic written yet. Per AGENTS.md "Test-first commit ordering," the next commit is the GREEN commit.

---

## Phase 3: Engine GREEN — implement `abort`, `complete`, `pause` wall-clock settling

**Goal**: Make the Phase 2 tests pass. One commit per logical chunk; the order below reflects the dependency graph.

**FRs covered**: FR-013, FR-013a, FR-014, FR-015, FR-016, FR-017, FR-034, FR-035. **SCs advanced**: SC-001, SC-004, SC-010, SC-012.

- [ ] T024 [US1] Extend `Timer::pause()` at `src/src/engine/timer.rs:664-683` to settle wall-clock delta into `current_session_elapsed_secs` before clearing `timer_start_ms` — extract or reuse the existing tick-drift compensation helper. Adds the `debug_assert!` for illegal state on entry (Principle III). **Done-signal**: test T020 (`complete_at_exactly_30s_wall_clock_counts_not_aborts`) passes. FR maps: FR-013a.
- [ ] T025 [US1] Extract the natural-completion sequence at `src/src/engine/timer.rs:808-872` into a private helper `fn complete_focus_session(&mut self) -> Vec<TimerEvent>`. The helper branches on `session_completed_but_not_saved`: flag=false ⇒ increment count + accumulate `total_focus_secs` + emit `PomodoroCompleted` + cadence check + advance mode; flag=true ⇒ accumulate overtime only, clear flag, **suppress** `PomodoroCompleted` re-emission, advance mode per zero-cross cadence. In both branches: emit `SessionCompletedEarly { elapsed_secs: <captured before zeroing> }`. Refactor the natural-completion zero-cross path (line 826 region) to call the helper. **Done-signal**: existing natural-completion tests still pass (regression). FR maps: FR-013, FR-016, AG-9 (path dedup).
- [ ] T026 [US1] Implement `Timer::abort(&mut self, clock: &impl Clock) -> Vec<TimerEvent>` per `contracts/timer-engine-actions.md` lines 11-49. Idempotent (no-op from Idle returns `[]`). Captures `aborted_mode` + `elapsed_secs` before zeroing. Clears the three run-state bools, `current_session_elapsed_secs`, `session_completed_but_not_saved`. **Does NOT** advance mode. **Done-signal**: tests T009–T013 pass. FR maps: FR-017, FR-034.
- [ ] T027 [US1] Implement `Timer::complete(&mut self, clock: &impl Clock) -> Vec<TimerEvent>` per `contracts/timer-engine-actions.md` lines 53-120. Precondition gate: `is_paused || is_auto_paused`; otherwise return `[]`. If `current_session_elapsed_secs < 30` ⇒ delegate to `self.abort(clock)`. If `>= 30` ⇒ call `complete_focus_session()` from T025. **Done-signal**: tests T014–T023 pass. FR maps: FR-013, FR-014, FR-015, FR-016, FR-034.
- [ ] T028 [US1] Run the full engine test suite: `cargo test --manifest-path src/Cargo.toml --lib engine::timer`. **Done-signal**: zero failures across all Phase 2 tests + all pre-existing engine tests. **GREEN commit lands here.** FR maps: full Phase 2 set, SC-001, SC-004, SC-010, SC-012.

**Phase 3 checkpoint**: Engine is fully spec'd. UI cannot yet reach the new methods (button matrix lives in Phase 6). Engine tests are 100% green.

---

## Phase 4: Persistence — Tauri commands + helpers + bridge mock

**Goal**: Land the four new Tauri commands behind a mock-first contract. Per AGENTS.md "Don't add Tauri commands without extending the mock first," the e2e mock gets the surface first; then RED tests; then real handlers.

**FRs covered**: FR-021, FR-022, FR-024a (parent-tag resolution begins here at the boundary level). **SCs advanced**: SC-005, SC-006.

### Mock-first

- [ ] T029 [US3] Extend `tests/e2e/fixtures/tauriMock.js` with four new commands: `load_quick_logs`, `save_quick_logs`, `load_distractions`, `save_distractions`. Each backed by module-scoped state (default `[]`); per-spec overrides supported. Default save returns `Ok`. **Done-signal**: a manual `mockTauri.invoke('load_quick_logs')` from a test fixture returns `[]`. FR maps: FR-021, AGENTS.md mock-first rule.

### RED tests (frontend wasm-bindgen-test side + backend `cargo test` side)

- [ ] T030 [P] [US3] Write RED test `save_quick_logs_round_trip` in `src-tauri/src/lib.rs` `#[cfg(test)] mod tests` — save a vec, load returns identical vec. **Done-signal**: test fails (command not implemented). FR maps: FR-021.
- [ ] T031 [P] [US3] Write RED test `save_quick_logs_rejects_out_of_range_minutes` — `elapsed_minutes=0` and `=721` both rejected with `BridgeError::InvalidArgument { field: "elapsedMinutes" }`. FR maps: FR-022.
- [ ] T032 [P] [US3] Write RED test `save_quick_logs_rejects_overlong_title` — 121-char title rejected with `field: "title"`. FR maps: FR-022.
- [ ] T033 [P] [US3] Write RED test `save_quick_logs_rejects_empty_title` — empty title rejected. FR maps: FR-022.
- [ ] T034 [P] [US2] Write RED test `save_distractions_round_trip` — round-trip including `parent_ref` payload. FR maps: FR-021, SC-006.
- [ ] T035 [P] [US2] Write RED test `save_distractions_rejects_overlong_note` — 121-char `note` rejected with `field: "note"`. FR maps: FR-022.
- [ ] T036 [P] [US2] Write RED test `save_distractions_rejects_overlong_parent_title` — when `parent_ref.parent_title.unwrap_or_default().chars().count() > 120`, rejected with `field: "parentRef.parentTitle"`. FR maps: FR-022.
- [ ] T037 [P] [US3] Write RED test `load_returns_empty_when_file_missing` for both `load_quick_logs` and `load_distractions`. FR maps: FR-021.
- [ ] T038 [P] [US3] Write RED test `load_handles_corrupt_file_with_bridge_error_internal` — non-JSON content yields `BridgeError::Internal` with a scrubbed reason string (asserts no characters from the corrupt payload appear in `msg` — PII conduit per Principle II). FR maps: FR-021, FR-022, AG-10 (PII-scrub conduit).

**RED commit for Phase 4 lands here.**

### GREEN — implementations

- [ ] T039 [US3] Add JSON IO helpers `read_quick_logs_from`, `write_quick_logs_to`, `read_distractions_from`, `write_distractions_to` to `src-tauri/src/helpers.rs`, mirroring `*_manual_sessions_*` (atomic write via `.tmp` rename). Missing files return `Ok(Vec::new())` via `#[serde(default)]`. Error formatter never receives payload bytes — only OS-level error strings (`format!("Failed to write …: {io_error}")`). FR maps: FR-021, FR-022 (boundary), Principle II.
- [ ] T040 [US3] Implement four `#[tauri::command] async fn` entries in `src-tauri/src/lib.rs` (`load_quick_logs`, `save_quick_logs`, `load_distractions`, `save_distractions`) per `contracts/persistence-commands.md`. Register all four in the existing `invoke_handler!` block alongside `load_manual_sessions` (lines 514-532). Save-side commands validate at the boundary in the order specified in the contract; first failure short-circuits with `BridgeError::InvalidArgument { field, reason }`. `parent_ref.parent_title` length validated when `Some`. FR maps: FR-021, FR-022.
- [ ] T041 [US3] Run `cargo test --manifest-path src-tauri/Cargo.toml`. **Done-signal**: all of T030–T038 pass. **GREEN commit lands here.** FR maps: full Phase 4 RED set, SC-005, SC-006.

**Phase 4 checkpoint**: Tauri command surface is real. Mock parity verified. Boundary validation is enforced. Managers can now wire to these commands in Phase 5.

---

## Phase 5: Managers — `QuickLogManager` + `DistractionManager`

**Goal**: Two new managers following the `SessionManager` precedent at `src/src/managers/session.rs:20-22` (per finding AG-7). Each owns a `RwSignal<Vec<T>>`, exposes `load/add/update/delete`, and bulk re-saves on every mutation. Test-first.

**FRs covered**: FR-032, FR-033, FR-035. **SCs advanced**: SC-005, SC-006.

### RED

- [ ] T042 [P] [US3] Write RED tests for `QuickLogManager` in `src/src/managers/quick_log.rs` (new file, test module): `add_then_load_round_trips_entry`, `update_replaces_in_place`, `delete_removes_only_target`, `validation_rejects_out_of_range_quick_log_minutes`, `validation_rejects_title_over_120`, `bridge_unavailable_short_circuits_gracefully`. Tests use a mock bridge fixture; structure mirrors `src/src/managers/session.rs` tests. **Done-signal**: tests fail (manager missing). FR maps: FR-032.
- [ ] T043 [P] [US2] Write RED tests for `DistractionManager` in `src/src/managers/distraction.rs` (new file, test module): `add_then_load_round_trips_entry`, `update_replaces_in_place`, `delete_removes_only_target`, `validation_rejects_note_over_120`, `parent_ref_snapshotted_at_modal_open_not_submit` (cooperation test — manager records what the modal hands it), `bridge_unavailable_short_circuits_gracefully`. **Done-signal**: tests fail. FR maps: FR-033, FR-035, SC-006.

**RED commit for Phase 5 lands here.**

### GREEN

- [ ] T044 [P] [US3] Implement `pub struct QuickLogManager { entries: RwSignal<Vec<QuickLog>> }` at `src/src/managers/quick_log.rs` with `new`, `load`, `add`, `update`, `delete` methods. Imports: `crate::bridge::types::QuickLog`, `crate::bridge::commands::{load_quick_logs, save_quick_logs}` (from T006/T007). Bulk re-save on every mutation. Graceful short-circuit when `window.__TAURI_INTERNALS__` is absent. **Done-signal**: T042 passes. FR maps: FR-032.
- [ ] T045 [P] [US2] Implement `pub struct DistractionManager` at `src/src/managers/distraction.rs` — identical shape to `QuickLogManager`. **Done-signal**: T043 passes. FR maps: FR-033, FR-035.
- [ ] T046 [US3] Edit `src/src/managers/mod.rs` to add `pub mod quick_log; pub mod distraction;`. **Done-signal**: `cargo check --manifest-path src/Cargo.toml`. FR maps: FR-032, FR-033.
- [ ] T047 [US3] Run `cargo test --manifest-path src/Cargo.toml --lib managers`. **Done-signal**: T042–T043 all green. **GREEN commit lands here.** FR maps: full Phase 5, SC-005, SC-006.

**Phase 5 checkpoint**: Managers exist and are tested. UI layer (Phase 6) can now consume them.

---

## Phase 6: UI components — combined pill, state-aware button matrix, modals, auto-restart UI gate fix

**Goal**: The visible rework. State-aware button matrix driven exhaustively off `RunState` × `TimerMode` (no flag-bool combinators). Combined `#timer-status-pill`. Two new modals. The critical auto-restart-UI-gate fix at `src/src/components/timer/mod.rs:1471-1483` (AG-2). Out of Principle V scope — covered by Phase 9 e2e.

**FRs covered**: FR-001 → FR-006, FR-012, FR-013 (UI side), FR-017 (UI side), FR-018, FR-019, FR-020, FR-025, FR-028, FR-035. **SCs advanced**: SC-001, SC-002, SC-003, SC-007, SC-010.

- [ ] T048 [US4] Refactor the combined pill at `src/src/components/timer/mod.rs:1727-1999`: wrap the existing `#timer-status` (chip + mode label + chevron) and `#session-title-input` in a new `#timer-status-pill` parent container. Both children keep their existing selector IDs (SC-007). In Focus Idle, both interactive. In Focus Running/Paused/AutoPaused: chevron hidden (chip click is a no-op), title input gets `readonly`. In Break/LongBreak Idle, the title region renders nothing — pill collapses to chip + mode label. Placeholder string sourced from new catalogue key `timer.pill_title_placeholder` (added in Phase 8). FR maps: FR-001, FR-002, FR-003, FR-004, FR-005, FR-006.
- [ ] T049 [US1] Replace today's three independent buttons at `src/src/components/timer/mod.rs:2034-2128` with a state-aware matrix driven by an **exhaustive match** on `(RunState, TimerMode)`. Each slot is a single `<button>` whose `label`, `icon`, `class` (ghost vs filled), `on:click`, and `aria-label` flip on the match. Drop the `StopButtonState` enum and its `Undo` variant entirely (~lines 225-238 + all downstream branches). Per FR-012: Idle ⇒ `+ Quick Log` · `▶ Play` · `→ Skip Mode`; Running ⇒ `✕ Abort` · `⏸ Pause` · `! Note Distraction`; Paused/AutoPaused ⇒ `✕ Abort` · `▶ Resume` · `✓ Complete`. Smoke check: existing engine `skip()` test suite remains green post-rename (`cargo test --lib engine::timer::tests::skip_`). FR maps: FR-012, FR-013, FR-017, FR-018, FR-028.
- [ ] T050 [US1] **AG-2 fix**: extend the auto-restart UI gate at `src/src/components/timer/mod.rs:1471-1483` to additionally require `events.iter().any(|e| matches!(e, TimerEvent::PomodoroCompleted { .. }))` — matching the existing event-check pattern at line 1429 (session-save gate). Under the new gate: natural completion + `complete`-with-count fire auto-restart; `abort` (which emits only `SessionAborted`) does NOT. Add a subscriber clearing pending auto-restart-countdown UI state on `TimerEvent::SessionAborted`. **Done-signal**: e2e `tests/e2e/timer-abort.spec.js` (added in Phase 9) asserts no countdown appears after Abort. FR maps: FR-017, SC-010, AG-2.
- [ ] T051 [P] [US3] Create `src/src/components/timer/quick_log_modal.rs` with a form: title field (auto-focused, required, `maxlength=120`), elapsed-minutes numeric field (default 5, min 1, max 720). Submit gated on title non-empty AND minutes in range. Submission calls `QuickLogManager::add` — never touches the engine. Modal closes on submit (no toast). Reachable from both the Idle left button AND the Inventory header `+ Quick Log` button (Phase 7). FR maps: FR-019, FR-025, SC-003, SC-005.
- [ ] T052 [P] [US2] Create `src/src/components/timer/distraction_modal.rs` with a single text field (auto-focused, `maxlength=120`). Enter submits, Escape cancels. Modal closes immediately on submit (no toast). **Parent-session ref is snapshotted at modal-open time, not submit time** — race-free per Edge Cases. Submission calls `DistractionManager::add` — never touches the engine (FR-035 pure side channel). FR maps: FR-020, FR-035, SC-002, SC-006.
- [ ] T053 [US3] Wire the Idle left-slot button to open the Quick Log modal. Wire the Inventory header button (Phase 7) to the same modal. FR maps: FR-019, FR-025.
- [ ] T054 [US2] Wire the Running right-slot button to open the Distraction modal. Confirm during wiring: the modal does not pause the engine, does not toggle smart-pause, does not touch `current_session_elapsed_secs` (FR-035 invariant — visible in the engine state assertion in `tests/e2e/timer-distraction.spec.js`). FR maps: FR-020, FR-035.

**Phase 6 checkpoint**: Timer view is rewired. Manual smoke per `quickstart.md` Exercise 1-4 succeeds in the dev shell. No e2e specs yet — those land in Phase 9.

---

## Phase 7: Inventory section + Stats tile widening

**Goal**: New consumption surface in the daily Stats / Calendar area. Stats tile label format widens with plural-aware suffixes.

**FRs covered**: FR-023, FR-024, FR-024a, FR-025, FR-026, FR-027. **SCs advanced**: SC-005, SC-006, SC-009.

- [ ] T055 [US5] Create `src/src/components/daily/inventory.rs` with the `<Inventory />` component. Two subsections: `Quick logs` and `Distractions`. Each row has Edit + Delete affordances reusing the `sessions_history_table.rs` edit/delete-via-modal pattern. Date-filter inherits the existing daily/weekly/monthly period selector — Inventory shows entries whose `date` field matches the selected day. Inventory header carries a `+ Quick Log` button opening the modal from T051. Empty-state lines render when the day has no entries (`No quick logs today.` / `No distractions today.`). FR maps: FR-023, FR-024, FR-025, FR-026, Edge Cases (zero-entries day).
- [ ] T056 [US5] **FR-024a render rule**: in the Distraction row component within `inventory.rs`, resolve `parent_ref.parent_tag_id` against the current tag table at render time. Tag exists ⇒ display the **current** tag name + colour (reflects renames). Tag deleted ⇒ display the `(deleted tag)` placeholder string from catalogue key `inventory.deleted_tag_placeholder` (added in Phase 8). `parent_title` is rendered as-snapshotted (never re-resolved). FR maps: FR-024a, SC-006.
- [ ] T057 [US5] Edit `src/src/components/daily/mod.rs` to render `<Inventory />` immediately below the existing sessions-history table. FR maps: FR-023.
- [ ] T058 [US5] Widen the stats tile label at `src/src/components/stats/mod.rs:431-457`. Append `· N quicklogs` when `N > 0` and `· M distractions` when `M > 0` to the pomodoro-count tile, in that order. Zero-suffixes hidden. Plural forms use the `_one` / `_other` key pairs from Phase 8 (FR-031). Apply the same widening to weekly + monthly tiles with period-specific catalogue keys. **Done-signal**: dev-shell daily Stats with seeded 5 pomodoros + 3 quicklogs + 2 distractions reads `5 pomodoros · 3 quicklogs · 2 distractions`. FR maps: FR-027, SC-009.

**Phase 7 checkpoint**: The full daily view renders the new Inventory section. Stats tile labels widen. The catalogue strings are still missing — Phase 8 fills them.

---

## Phase 8: Catalogue strings + dead-key pruning

**Goal**: Add new EN/DE/IT/TR catalogue keys; prune the dead `timer.ctrl_undo` / `timer.ctrl_undo_aria` keys per FR-028a. TR may fall back to EN per the existing deferral (spec Clarifications + AG-3).

**FRs covered**: FR-028a, FR-031. **SCs advanced**: SC-007 (the catalogue feeds visible chrome), SC-009.

- [ ] T059 [P] [US4] Edit `src/locales/en.json`: add new keys per FR-031 (`timer.ctrl_quick_log`, `timer.ctrl_skip_mode` — rename of today's `Skip session`, `timer.ctrl_abort`, `timer.ctrl_note_distraction`, `timer.ctrl_complete`, `timer.pill_title_placeholder`, `inventory.section_header`, `inventory.subsection_quicklogs`, `inventory.subsection_distractions`, `inventory.empty_quicklogs`, `inventory.empty_distractions`, `inventory.deleted_tag_placeholder`, `modal.quick_log_title`, `modal.note_distraction_title`, and the `_one` / `_other` pairs for `stats.tile_daily_quicklogs`, `stats.tile_daily_distractions`, `stats.tile_weekly_quicklogs`, `stats.tile_weekly_distractions`, `stats.tile_monthly_quicklogs`, `stats.tile_monthly_distractions`). **Prune** `timer.ctrl_undo` and `timer.ctrl_undo_aria` (FR-028a). FR maps: FR-028a, FR-031.
- [ ] T060 [P] [US4] Edit `src/locales/de.json`: same key additions with German translations (`Quick Log` ⇒ `Schnelleintrag` etc., DE inflected plural forms for `_one` / `_other`). Prune `timer.ctrl_undo*`. FR maps: FR-028a, FR-031.
- [ ] T061 [P] [US4] Edit `src/locales/it.json`: same key additions with Italian translations + IT inflected plural forms. Prune `timer.ctrl_undo*`. FR maps: FR-028a, FR-031.
- [ ] T062 [P] [US4] Edit `src/locales/tr.json`: add new keys with TR translations **or** EN fallback per spec Clarifications (TR contributor not in scope). **Prune** `timer.ctrl_undo*`. FR maps: FR-028a, FR-031.
- [ ] T063 [US4] Run `cargo check --manifest-path src/Cargo.toml`. The typed-key catalogue from feature 005 catches missing keys at compile time. **Done-signal**: all four locale files parse; no missing-key compile errors. FR maps: FR-031.

**Phase 8 checkpoint**: All visible chrome is localised across EN/DE/IT/TR. Dead `Undo` keys gone. The product is feature-complete pre-e2e.

---

## Phase 9: E2E + visual regression

**Goal**: New Playwright specs and regenerated VR baselines. Per FR-029/030 + Principle IV, each regenerated baseline carries a one-line PR note. Defer `timer-focus-running-chromium-linux.png` per plan.

**FRs covered**: FR-029, FR-030. **SCs advanced**: SC-007, SC-008, SC-009, SC-010 (UI level).

### New e2e specs

- [ ] T064 [P] [US3] Create `tests/e2e/timer-quick-log.spec.js` — Idle left-slot label is `+ Quick Log` across Focus/Break/LongBreak modes; modal opens auto-focused; title `maxlength=120`; minutes default=5, range `[1, 720]`; out-of-range submission rejected at form layer; valid submission appends to mocked `load_quick_logs` state; pomodoro counter unchanged; mode unchanged; `pomodoros_until_long_break` unchanged. FR maps: FR-019, FR-022, SC-003, SC-005.
- [ ] T065 [P] [US2] Create `tests/e2e/timer-distraction.spec.js` — Running right-slot label is `Distraction`; modal opens auto-focused; Enter submits; Escape cancels (no write); engine state (running, elapsed) unchanged post-submit; parent-session-ref persists snapshotted at modal-open time (not submit); two back-to-back submissions yield two distinct rows with identical parentRef but distinct createdAt. FR maps: FR-020, FR-035, SC-002, SC-006.
- [ ] T066 [P] [US1] Create `tests/e2e/timer-complete.spec.js` — Pause → ✓ Complete with elapsed≥30 increments count and advances; with elapsed<30 discards as Abort (no count, no advance, title preserved). Covers FR-013 / FR-015 at DOM level. **Plus regression**: after natural focus completion AND after Skip Mode, assert `#session-title-input.value === ''` — confirms FR-007 and FR-008 status-quo title-clear is preserved. FR maps: FR-013, FR-014, FR-015, SC-001, SC-004, SC-012.
- [ ] T067 [P] [US1] Create `tests/e2e/timer-abort.spec.js` — ✕ Abort from Running and from Paused; title persists in pill; no count, no advance; no auto-restart countdown appears even when `notifications.auto_start_timer = true` (the AG-2 fix at the DOM level). FR maps: FR-017, SC-010.
- [ ] T068 [P] [US5] Create `tests/e2e/inventory.spec.js` — Inventory subsection renders; per-row Edit + Delete; header `+ Quick Log` opens the identical modal from `timer-quick-log.spec.js`; day-filter swaps the row set; deleted tag in `parent_ref.parent_tag_id` renders `(deleted tag)` placeholder; renamed tag renders the current name. FR maps: FR-023, FR-024, FR-024a, FR-025, FR-026, SC-006.

### Visual regression baselines (one-line PR notes per FR-029/030)

- [ ] T069 [US4] Regenerate `tests/e2e/__screenshots__/visual-regression/timer-chromium-linux.png` — Idle Focus with the combined pill (single `#timer-status-pill` container around chip + title), left button `+ Quick Log`, right button `Skip Mode` (renamed from `Skip session`). PR note: `"Timer Idle: combined #timer-status-pill replaces separate controls; left button renamed + Quick Log; right button renamed Skip Mode."`. FR maps: FR-001, FR-029, FR-030, SC-008.
- [ ] T070 [US1] **Add new** baseline `tests/e2e/__screenshots__/visual-regression/timer-focus-paused-with-complete-chromium-linux.png` — Focus Paused state with the collapsed pill (chevron hidden, title `readonly`) and the new three-control triad `✕ Abort · ▶ Resume · ✓ Complete`. The single highest-value new visual surface for the feature (PR-10 fix). PR note: `"Timer Focus Paused (new baseline): combined pill collapsed read-only; new right-slot ✓ Complete button revealed."`. FR maps: FR-005, FR-012, FR-029, FR-030.
- [ ] T071 [US5] Regenerate `tests/e2e/__screenshots__/visual-regression/daily-chromium-linux.png` — Inventory subsection appended below sessions-history. Seeded day has zero entries so empty-state lines render. PR note: `"Daily view: new Inventory subsection appended below sessions-history table (empty-state lines for the seeded day)."`. FR maps: FR-023, FR-029, FR-030.
- [ ] T072 [US5] Regenerate `tests/e2e/__screenshots__/visual-regression/statistics-daily-chromium-linux.png` — daily Stats tile with the widened label format. **Canonical baseline keeps the zero-suffix scenario** (so the tile label is visually unchanged) per plan recommendation; non-zero case is DOM-asserted in `inventory.spec.js`. PR note: `"Daily Stats tile: label format widened (suffixes hidden when zero, so the canonical baseline is visually unchanged)."`. FR maps: FR-027, FR-029, FR-030, SC-009.
- [ ] T073 [US5] Run the full Playwright VR suite: `npx playwright test`. **Done-signal**: zero unexpected regressions outside the FR-029 set (SC-008). Any drift in `timer-focus-running-chromium-linux.png` (deferred per plan) is treated as a code regression, not absorbed. FR maps: FR-029, FR-030, SC-008.

**Phase 9 checkpoint**: All e2e + VR pass. PR description carries the four one-line baseline notes plus the `[BEST-GUESS PM DECISION]` deferral note for `timer-focus-running-chromium-linux.png`.

---

## Polish / cross-cutting (rolled into the relevant phases)

The standing constraint set (no new `#[allow]`, clippy-pedantic clean, lockfile drift zero) is verified once at the end of Phase 9 before opening the PR. No standalone "polish" tasks — each phase's done-signal already enforces the gate.

- [ ] T074 Final check: `cargo clippy --all-targets -- -D warnings -W clippy::pedantic` on both `src-tauri/Cargo.toml` and `src/Cargo.toml`; `cargo fmt --check` on both. Zero new `#[allow]` exceptions (SC-011). `cargo build --frozen` succeeds on both. `package-lock.json` ↔ `package.json` unchanged. FR maps: FR-036, SC-011, Principle IX, Principle X.

---

## Dependencies & Execution Order

### Phase dependencies

- **Phase 1** (Domain types): no dependencies — can start immediately.
- **Phase 2** (Engine RED): depends on T004, T005 (the new `TimerEvent` variants and `RunState` enum must compile).
- **Phase 3** (Engine GREEN): depends on Phase 2 RED commit per AGENTS.md "Test-first commit ordering."
- **Phase 4** (Persistence): the mock-first task T029 depends on T001, T002 (types must exist for the mock to reference them). T029 precedes RED tests T030–T038, which precede GREEN T039–T041. Phase 4 does NOT depend on Phase 3.
- **Phase 5** (Managers): depends on Phase 4 (managers wire to the real commands; RED tests use mock).
- **Phase 6** (UI): depends on Phase 3 (engine methods exist) AND Phase 5 (managers exist). T050 (auto-restart UI gate) depends on T004 (the `TimerEvent::SessionAborted` variant must exist for the match arm).
- **Phase 7** (Inventory + Stats): depends on Phase 5.
- **Phase 8** (Catalogue): can run in parallel with Phase 6 / Phase 7 once T005 (the matrix references catalogue keys, but compile-time-check is permissive until the keys actually render — runtime resolution at test time is what binds). Convention: land Phase 8 just before Phase 9 to avoid runtime-missing-key panics during manual exercises.
- **Phase 9** (e2e + VR): depends on **everything** (the full feature surface must render correctly).

### Parallel opportunities

- **Within Phase 1**: T001, T002 parallel; T006, T007 parallel.
- **Within Phase 2**: tests in the same file are sequential per file (Rust test ordering doesn't matter, but the file-level edit is serialised); engine tests run as a single suite — but writing them is a serial editing task on one file. No `[P]` markers.
- **Within Phase 4**: T030–T038 parallel as written (test functions in separate modules / separate test names).
- **Within Phase 5**: T042 vs T043 parallel; T044 vs T045 parallel (different files).
- **Within Phase 6**: T051 vs T052 parallel (different files).
- **Within Phase 8**: T059, T060, T061, T062 parallel (one per locale file).
- **Within Phase 9**: T064–T068 parallel (one per spec file); VR baselines T069–T072 parallel.

### Engine-tests sequencing nuance

Engine tests in Phase 2 all live in the **same file** (`src/src/engine/timer.rs` `#[cfg(test)] mod tests`). They are written sequentially as edits to one file but **run** as a parallel suite. The `[P]` marker is omitted because the editing dependency is on the same file — there is no parallelism opportunity at edit time.

---

## Implementation Strategy

### Test-first commit ordering (mandatory per AGENTS.md)

For every Principle V scope (engine, managers, Tauri-boundary helpers): **RED commit precedes GREEN commit**. No combined commit.

- Phase 2 commits: RED-engine.
- Phase 3 commits: GREEN-engine (broken into T024 wall-clock, T025 helper extraction, T026 abort, T027 complete, T028 suite-run; each can be its own commit or a single GREEN commit per AGENTS.md preference).
- Phase 4 commits: RED-persistence (after T029 mock-first), GREEN-persistence.
- Phase 5 commits: RED-managers, GREEN-managers.

Phase 6, 7, 8, 9 are out of Principle V scope — combined commits are fine, but the e2e specs in Phase 9 effectively serve as the regression-test contract.

### MVP scope

MVP = Phase 1 + Phase 2 + Phase 3 + Phase 4 + Phase 5 + Phase 6 (engine + persistence + managers + state-aware matrix + modals). Demonstrable: a user can Pause → ✓ Complete, capture a distraction, log a quick task. Inventory + Stats widening + VR baselines are deferrable polish on the path to merge, but the user-visible value of the headline three changes is intact.

In practice: ship the whole thing as one PR per the spec (the changes are tightly coupled — Inventory consumes Quick Log/Distraction; Stats tile depends on the same managers).

---

## Spec-coverage matrix (FRs ↔ Tasks)

Every functional requirement maps to ≥ 1 task; every success criterion is advanced by ≥ 1 phase.

| FR | Task(s) |
|---|---|
| **FR-001** | T048, T069 |
| **FR-002** | T048 |
| **FR-003** | T048, T059–T062 (`timer.pill_title_placeholder`) |
| **FR-004** | T048 |
| **FR-005** | T048, T070 |
| **FR-006** | T048 |
| **FR-007** | (status quo — verified by T066 / quickstart Ex 1) |
| **FR-008** | (status quo — verified by `tests/e2e/timer.spec.js` precedent) |
| **FR-009** | T049 (matrix wiring of `complete`), T066 (e2e) |
| **FR-010** | T026 (abort engine), T049 (wiring), T067 (e2e) |
| **FR-011** | T049 (matrix gates interactivity off `RunState`) |
| **FR-012** | T005 (RunState), T049, T064–T067 |
| **FR-013** | T014, T016, T017, T018, T019, T021, T022, T025, T027, T049, T066 |
| **FR-013a** | T020, T024, T066 |
| **FR-014** | T014, T020, T027, T066 |
| **FR-015** | T015, T027, T066 |
| **FR-016** | T017, T018, T019, T027, T066 |
| **FR-017** | T009, T010, T011, T012, T013, T026, T049, T050, T067 |
| **FR-018** | T049 (matrix continues to wire `Skip Mode`) |
| **FR-019** | T001, T040, T044, T051, T053, T064 |
| **FR-020** | T002, T040, T045, T052, T054, T065 |
| **FR-021** | T003, T006, T007, T029, T030, T034, T037, T039, T040 |
| **FR-022** | T031, T032, T033, T035, T036, T038, T039, T040 |
| **FR-023** | T055, T057, T068, T071 |
| **FR-024** | T055, T068 |
| **FR-024a** | T056, T059–T062 (`inventory.deleted_tag_placeholder`), T068 |
| **FR-025** | T051, T053, T055, T068 |
| **FR-026** | T055, T068 |
| **FR-027** | T058, T072 |
| **FR-028** | T049 (drops `StopButtonState::Undo`) |
| **FR-028a** | T059, T060, T061, T062, T063 |
| **FR-029** | T069, T070, T071, T072, T073 |
| **FR-030** | T069, T070, T071, T072 (PR notes attached) |
| **FR-031** | T058, T059–T063 |
| **FR-032** | T042, T044, T046, T047 |
| **FR-033** | T043, T045, T046, T047 |
| **FR-034** | T004, T008, T026, T027 |
| **FR-035** | T045, T052, T054, T065 |
| **FR-036** | T074 (final clippy gate) |

| SC | Phase advancing it |
|---|---|
| **SC-001** | Phase 2, 3, 6, 9 (T014, T020, T027, T049, T066) |
| **SC-002** | Phase 6, 9 (T052, T054, T065) |
| **SC-003** | Phase 6, 9 (T051, T053, T064) |
| **SC-004** | Phase 2, 3, 9 (T014, T018, T027, T028, T066) |
| **SC-005** | Phase 4, 5, 7, 9 (T040, T044, T058, T064) |
| **SC-006** | Phase 4, 5, 7, 9 (T034, T036, T045, T056, T065, T068) |
| **SC-007** | Phase 1, 6 (T005, T048, T049) |
| **SC-008** | Phase 9 (T073) |
| **SC-009** | Phase 7, 8, 9 (T058, T059–T062, T072) |
| **SC-010** | Phase 2, 3, 6, 9 (T012, T026, T050, T067) |
| **SC-011** | Phase 9 polish (T074) |
| **SC-012** | Phase 2, 3, 9 (T015, T020, T027, T066) |

No FR or SC is uncovered.

---

## Notes

- `[P]` markers indicate file-level independence at edit time. They do **not** guarantee runtime test-parallel safety — `cargo test` runs in-process; `playwright` parallelism is governed by `playwright.config.js`.
- All file paths are repository-relative absolute on disk under `/home/claude/projects/managed/KonstantinKo_presto/`.
- `--no-verify` is forbidden by CLAUDE.md "No `--no-verify` except in genuine emergencies." A hook failure means fix-then-recommit, not bypass.
- AG-1 through AG-10 finding references trace to the architecture-guard review embedded in `plan.md` and the spec Clarifications. AG-2 (auto-restart gate) ⇒ T050. AG-7 (manager precedent at `src/src/managers/session.rs:20-22`) ⇒ T044/T045. AG-9 (engine path dedup via `complete_focus_session` helper) ⇒ T025. AG-10 (PII-scrub conduit) ⇒ T038/T039.
- The `timer-focus-running-chromium-linux.png` baseline is deliberately deferred per `plan.md` Visual regression budget — a `[BEST-GUESS PM DECISION]` line in the PR description acknowledges this.

---

## Open questions for the PM (carried from plan.md, not blocking)

1. Should `TimerEvent::SessionCompletedEarly` be emitted unconditionally in branch B (current plan), or only when elapsed strictly less than `focus_duration`? Currently the helper at T025 emits it always; if the PM wants strict "early only" semantics, T017's assertion would need a flag check.
2. Confirm continuous-mode AutoPaused-during-overtime semantics (current plan: identical to Paused — exercised by T019). If the PM disagrees, T016 + T017 would be merged into a single test exercising the intersection, and the spec's Edge Cases section gets a new bullet.
