---
description: "Task list for Overtime Button Treatment (feature 007)"
---

# Tasks: Overtime Button Treatment

**Feature**: `007-overtime-button-treatment`
**Branch**: `007-overtime-button-treatment`
**Input**: `/specs/007-overtime-button-treatment/` — spec.md (24 FRs, 10 SCs), plan.md, data-model.md, contracts/shortcut-registration.md, quickstart.md
**Prerequisites**: Feature 006 engine path (`engine.complete(clock)` branch B.2 at `src/src/engine/timer.rs:998-1040`) must be in tree.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: User story this task belongs to (US1 = P1 overtime treatment, US2 = P2 abort shortcut)
- Exact file paths given throughout

---

## Phase 1: Catalogue Keys + i18n Hygiene

**Purpose**: Add all new i18n keys before any consuming code is written; fix the one remaining test-helper literal. These tasks are all [P] — they touch different files.

**FRs covered**: FR-012, FR-013

- [ ] T001 [P] Add `timer.overtime_cta` key (`"Wrap it up!"`) to `src/locales/en.json` (source-of-truth)
- [ ] T002 [P] Add `timer.overtime_cta` good-faith translation to `src/locales/de.json`
- [ ] T003 [P] Add `timer.overtime_cta` good-faith translation to `src/locales/it.json`
- [ ] T004 [P] Add `timer.overtime_cta` EN-fallback (or good-faith TR) to `src/locales/tr.json` per feature-005 hedge
- [ ] T005 [P] Add `settings.shortcuts.label_abort` and `settings.shortcuts.desc_abort` keys to `src/locales/en.json` — values: `"Abort Session:"` and `"Discard the current focus session without logging it."`
- [ ] T006 [P] Add `settings.shortcuts.label_abort` and `settings.shortcuts.desc_abort` good-faith translations to `src/locales/de.json`
- [ ] T007 [P] Add `settings.shortcuts.label_abort` and `settings.shortcuts.desc_abort` good-faith translations to `src/locales/it.json`
- [ ] T008 [P] Add `settings.shortcuts.label_abort` and `settings.shortcuts.desc_abort` EN-fallback to `src/locales/tr.json`
- [ ] T009 Replace hard-coded `"(Overtime)"` literal in the `#[cfg(test)]` helper `mode_label_with_status` at `src/src/components/timer/mod.rs:154` with `t_string!(i18n, timer.status_overtime)` — one-line hygiene fix (FR-013). Done-signal: `cargo test -p presto-web --target wasm32-unknown-unknown mode_label_with_status` passes; the resolved string under the EN locale equals `"(Overtime)"` (proves catalogue substitution is real, not just key-pass-through).

**Checkpoint**: All new catalogue keys exist in all four locales; test-helper literal replaced. Build compiles with typed-key check passing.

---

## Phase 2: IPC Widening — RED-then-GREEN

**Purpose**: Widen `ShortcutSettings` with the `abort` field and prove the wire format is correct before any consuming code is written. Tests are written first (RED), then the implementation makes them GREEN.

**FRs covered**: FR-018, FR-019, FR-020
**Test-first ordering applies**: `ShortcutSettings` is a persistent IPC contract treated as a stateful engine per AGENTS.md.

- [ ] T010 Write RED test `shortcut_settings_with_abort_roundtrips` in `crates/presto-ipc/src/settings.rs` `#[cfg(test)] mod tests` — assert `ShortcutSettings { start_stop: Some(_), reset: Some(_), skip: Some(_), abort: Some("CommandOrControl+Alt+W") }` serialises + deserialises identically. Run `cargo test -p presto-ipc` and confirm it FAILS (field does not exist yet). Done-signal: test compiles and fails with a missing-field error.
- [ ] T011 Write RED test `shortcut_settings_with_unbound_abort_roundtrips` in same file — assert `abort: None` serialises to JSON `null` and deserialises back to `None`. Done-signal: test fails as expected.
- [ ] T012 Write RED test `shortcut_settings_missing_abort_field_defaults_to_none` in same file — assert a pre-feature settings JSON string (no `abort` key) deserialises with `abort: None`. Done-signal: test fails as expected.
- [ ] T013 Add `pub abort: Option<String>` field to `ShortcutSettings` struct in `crates/presto-ipc/src/settings.rs:113-127`. Add the doc-comment above the field in `Default::default()` explaining the intentional asymmetry (abort defaults to `None` while sibling fields are pre-bound — per FR-019, do not "fix" without spec revision). Done-signal: `cargo test -p presto-ipc` passes all three RED tests above and no existing tests regress.

- [ ] T013b [P] Write RED tests in `src-tauri/src/lib.rs` `#[cfg(test)] mod tests`:
  - `register_global_shortcuts_widened_arg_accepts_abort` — call with `ShortcutSettings { abort: Some("CommandOrControl+Alt+W"), .. }`, assert Ok.
  - `register_global_shortcuts_widened_arg_skips_unbound_abort` — call with `abort: None`, assert no registration emitted for `"abort"`.
  - `register_global_shortcuts_widened_arg_invalid_abort_returns_internal_error` — call with `abort: Some("not-a-shortcut")`, assert `BridgeError::Internal { msg }` and msg contains `"abort"`.
  Run `cargo test -p presto_lib --lib` and confirm tests fail because the loop at `:432-473` doesn't yet handle the `abort` slot. Done-signal: three tests written, compile, fail with the expected reason. T023 will make them GREEN.

- [ ] T013c Update the two existing settings-deserialisation test fixture strings in `src-tauri/src/lib.rs:1151` and `:1237` (legacy roundtrip + `make_json` helper) to add `"abort": null` to the `"shortcuts"` object. Two-line edit. Done-signal: `cargo test -p presto_lib --lib` passes; pre-existing fixture-using tests stay green after the IPC widening.

**Checkpoint**: `ShortcutSettings` carries `abort: Option<String>`; three round-trip tests green; `cargo clippy --all-targets -- -D warnings -W clippy::pedantic` clean on `presto-ipc`.

---

## Phase 3: User Story 1 — Gentle Nudge Out of Overtime (Priority: P1)

**Goal**: When a continuous-mode focus session crosses zero, all three button slots show orange `✓ Complete`; center filled, outer two ghost; "Wrap it up!" CTA visible between countdown and buttons; clicking any slot ends the session via the engine's existing `complete(clock)` path; pausing reverts to normal Paused matrix; exiting returns to normal treatment.

**Independent Test**: Start a continuous-mode focus session (short duration), advance past zero, observe three orange Complete buttons and "Wrap it up!" CTA; click any slot and verify session ends and break begins. Pause during overtime and confirm matrix reverts; resume and confirm overtime returns.

**FRs covered**: FR-001, FR-002, FR-003, FR-004, FR-005, FR-006, FR-007, FR-008, FR-009, FR-010, FR-011, FR-012, FR-014, FR-015, FR-016, FR-022, FR-023, FR-024
**SCs covered**: SC-001, SC-002, SC-003, SC-004, SC-006, SC-007, SC-008, SC-009

### Implementation for User Story 1

- [ ] T014 [US1] Add CSS `.control-btn.overtime` modifier to `src/style/timer.css`: `border-color` and `color` equal `var(--warning-color)`. Add `.control-btn.overtime.primary` rule: `background` equals `var(--warning-color)`, foreground inverted (white/dark contrast per light/dark token). Add `.overtime-cta` base rule: `display: none; text-align: center; color: var(--warning-color);` plus sizing/margin to sit between countdown and button row. Add `.overtime-cta.visible` rule: `display: block`. Done-signal: CSS lint passes; no new CSS variables introduced (reuse `--warning-color` already at `src/style/variables.css:22,48,72`).

- [ ] T015 [US1] Add the `on_center_click` named closure at `src/src/components/timer/mod.rs` co-located with `on_play_pause` around line 1327-1349. Body: `if is_overtime.get_untracked() && matches!(run_state.get_untracked(), RunState::Running) { on_complete(ev); } else { on_play_pause(ev); }`. This is the single 2D gate for the center slot — no JSX-level conditional. Done-signal: code compiles; `cargo clippy` clean.

- [ ] T016 [US1] Extend the left-slot (`#stop-btn`) click-dispatch `match` at `src/src/components/timer/mod.rs:2273-2277` to a 2-tuple `(RunState, bool)` match: `(RunState::Running, true) => on_complete(ev)` (overtime collapses to Complete), `(RunState::Running, false) => on_abort(ev)`, `(RunState::Idle, _) => on_open_quick_log(ev)`, `(RunState::Paused, _) => on_abort(ev)`. Done-signal: exhaustive match; `cargo clippy --all-targets -- -D warnings -W clippy::pedantic` clean.

- [ ] T017 [US1] Extend the right-slot (`#skip-btn`) click-dispatch `match` at `src/src/components/timer/mod.rs:2323-2328` with the same overtime-collapse arm: `(RunState::Running, true) => on_complete(ev)`. All three slots now share `on_complete` in the `(Running, true)` row (FR-007, FR-008, SC-002). Done-signal: exhaustive 2-tuple match; clippy clean.

- [ ] T018 [US1] Bind `on:click=on_center_click` on `#play-pause-btn` in `src/src/components/timer/mod.rs` around line 2308, replacing the previous `on_play_pause` direct binding. Done-signal: center button compiles with the named closure; no JSX-level wrapper.

- [ ] T019 [US1] Add `class:overtime` reactive binding to all three `.control-btn` elements in `src/src/components/timer/mod.rs`, bound to the inline predicate `matches!(run_state.get(), RunState::Running) && is_overtime.get()`. Left slot (`#stop-btn`), center (`#play-pause-btn`), right (`#skip-btn`). Done-signal: three buttons carry `class:overtime` at the right moments; clippy clean.

- [ ] T020 [US1] Update the label/icon dispatch for all three button slots to emit `✓ Complete` when `(RunState::Running, is_overtime == true)` — re-use the existing `timer.ctrl_complete` catalogue key and the `✓` glyph already used by the Paused-Complete path (feature 006). Extend the `verbose_label_play` and sibling terse/tooltip signals (around line 2305) so the center button's `aria-label` returns `timer.ctrl_complete_aria` in `(Running, true)` state (FR-016, SC-003). Done-signal: labels correct for all three slots in overtime; `cargo clippy` clean.

- [ ] T021 [US1] Add `aria-hidden` and `tabindex` reactive bindings on `#stop-btn` and `#skip-btn` only in `src/src/components/timer/mod.rs`: `aria-hidden=move || matches!(run_state.get(), RunState::Running) && is_overtime.get()` and `tabindex=move || if matches!(run_state.get(), RunState::Running) && is_overtime.get() { -1 } else { 0 }`. Pattern from `src/src/components/settings/theme.rs:217`. Center `#play-pause-btn` untouched. Done-signal: FR-014, FR-015; clippy clean.

- [ ] T022 [US1] Insert the `<p class="overtime-cta">` element in the timer view at `src/src/components/timer/mod.rs` between the `.timer-container` closing tag (~line 2241) and the `.controls` opening tag (~line 2267): `<p class="overtime-cta" class:visible=move || matches!(run_state.get(), RunState::Running) && is_overtime.get()>{ move || t_string!(i18n, timer.overtime_cta) }</p>`. Done-signal: CTA element exists in DOM; visibility predicate is `Running && is_overtime` (same gate as buttons — synchronous appear/disappear per SC-001, SC-006, SC-009; FR-010, FR-011).

**Checkpoint**: User Story 1 is fully functional. Run `cargo build --frozen` for backend + frontend. Open the app in continuous mode with a 1-minute focus session, advance past zero, and observe the three orange Complete buttons + "Wrap it up!" CTA. Click any slot — session ends. Pause during overtime — matrix reverts; resume — overtime returns. `cargo clippy --all-targets -- -D warnings -W clippy::pedantic` clean on all crates.

---

## Phase 4: User Story 2 — No Accidental Discard (Priority: P2)

**Goal**: Extend the global shortcut mechanism with an Abort action: new `ShortcutSettings.abort` field wired through the Tauri registration loop, the frontend listener, and the Settings > Shortcuts panel with a fourth bindable row. The shortcut persists across restarts and remains active during overtime.

**Independent Test**: With the overtime treatment on screen, emit `"abort"` on the `global-shortcut` channel (mock or real) and verify the session is discarded, the timer returns to idle, and the orange treatment is gone. Verify the Abort row appears in Settings > Shortcuts and the binding persists across reload.

**FRs covered**: FR-017, FR-018, FR-019, FR-020, FR-021
**SCs covered**: SC-005, SC-010
**Depends on**: T013 (Phase 2 IPC widening)

### Implementation for User Story 2

- [ ] T023 [US2] Widen the registration loop in `src-tauri/src/lib.rs:432-473` by adding `("abort", &shortcuts.abort)` as a fourth entry in the iterator slice (after `"skip"`). Same `on_shortcut` closure body, same `should_debounce_shortcut` gate, same `app_handle.emit("global-shortcut", action_owned.as_str())` line. No new branch, no new command. Done-signal: `cargo build --frozen` on the `src-tauri` crate passes; `cargo clippy --all-targets -- -D warnings -W clippy::pedantic` clean; all three T013b tests now pass.

- [ ] T024 [US2] Rewrite the listener at `src/src/app.rs:613-624` (currently a no-op stub) with the full four-arm dispatch: `"start-stop"` → `engine.try_update(state.start_stop)`, `"reset"` → `engine.try_update(state.reset)`, `"skip"` → `engine.try_update(state.skip)`, `"abort"` → `engine.try_update(state.abort)`. Each branch mirrors the side-effect pipeline of the corresponding UI handler: `"start-stop"` calls `engine.try_update(state.start_stop)` then mirrors `on_play_pause` (`src/src/components/timer/mod.rs:1327-1349`); `"reset"` mirrors the existing reset handler; `"skip"` mirrors the existing skip handler; `"abort"` calls `engine.try_update(state.abort)` then mirrors `on_abort` (`src/src/components/timer/mod.rs:1360-1442`) — `tag_tracking_flush_all`, `app_toast.show(...)`, `handle_events`, `apply_tag_tracking_events`, `dispatch_tray_update`. Wire names are kebab-case throughout; add `_ => {}` wildcard arm for forward-compatibility. Done-signal: all four arms compile, each calls the corresponding side-effect chain, listener no longer a no-op; clippy clean.

- [ ] T025 [US2] Add `ShortcutSlot::Abort` variant to the `ShortcutSlot` enum in `src/src/components/settings/shortcuts.rs`. Add `input_id` → `"abort-shortcut"`, `placeholder`, `label` (from `settings.shortcuts.label_abort`), and `description` (from `settings.shortcuts.desc_abort`) arms matching the existing kebab-case convention (`"start-stop-shortcut"`, `"reset-shortcut"`, `"skip-shortcut"`). Render a fourth `shortcut_row(ShortcutSlot::Abort, …)` call at the end of the Shortcuts section view. Extend the `shortcuts_selector_contract_documented` test (line ~259) to cover `#abort-shortcut`. Done-signal: Settings > Shortcuts panel shows four rows; selector-contract test passes; clippy clean.

- [ ] T026 [P] [US2] Check `tests/e2e/fixtures/tauriMock.js` for an existing `global-shortcut` event-emit helper (look for `emit("global-shortcut", …)` or equivalent from feature 006). If absent, add a small mock helper that allows e2e tests to call `window.__TAURI_INTERNALS__.emit("global-shortcut", payload)` (or the correct Tauri 2.x mock path). Done-signal: tauriMock.js has a confirmed emit path for `"global-shortcut"` payloads usable by Playwright tests.

**Checkpoint**: User Story 2 functional. With the app running: bind a key to Abort in Settings > Shortcuts, enter overtime, press the key, and confirm the session discards and the timer returns to idle. Restart the app and confirm the Abort binding persisted. `cargo clippy` clean on all crates.

---

## Phase 5: e2e Tests + Visual Regression Baseline + Polish

**Purpose**: Full end-to-end coverage for both user stories, new VR baseline, dark-mode QA loop, final lint/fmt/clippy gates.

**FRs covered**: All (verification sweep). Key: FR-001–FR-024.
**SCs covered**: All 10.

### e2e — timer-overtime.spec.js (new)

- [ ] T027 Create `tests/e2e/timer-overtime.spec.js` with test: *Triple-Complete dispatch* — continuous mode, advance clock past zero (mock engine to expose overtime via `is_overtime` signal or advance clock), click left ghost slot → assert engine `complete` was called (session ends, break begins). Re-enter overtime, click right ghost slot → same. Re-enter, click center filled slot → same. Assert all three produce identical state (FR-007, FR-008, SC-002).

- [ ] T028 Add test to `tests/e2e/timer-overtime.spec.js`: *Orange tint + CTA visible* — in overtime, assert three `.control-btn` elements carry `class` containing `overtime`, `.overtime-cta` is visible with text matching the `timer.overtime_cta` catalogue value for the active locale (FR-005, FR-010, SC-001, SC-006, SC-008).

- [ ] T029 Add test to `tests/e2e/timer-overtime.spec.js`: *A11y removal of outer slots* — in overtime, assert `#stop-btn[aria-hidden="true"]` and `#stop-btn[tabindex="-1"]`; same for `#skip-btn`. Assert `#play-pause-btn` has `aria-hidden` absent (or `false`) and `tabindex="0"`. Separately assert `page.getByRole('button', { name: <ctrl_complete_aria text> })` returns exactly one result (role-based SC-003 check) (FR-014, FR-015, FR-016, SC-003, SC-004).

- [ ] T030 Add test to `tests/e2e/timer-overtime.spec.js`: *Exit via Complete clears treatment* — after any of the three slot clicks, assert `.control-btn` elements no longer carry `class*=overtime`, `.overtime-cta` is not visible, and the timer view shows normal break treatment (FR-024, SC-009).

- [ ] T031 Add test to `tests/e2e/timer-overtime.spec.js`: *Exit via Abort keyboard clears treatment* — emit `"global-shortcut"` event with payload `"abort"` using the mock helper from T026 while in overtime, assert engine returns to idle in current focus mode, overtime treatment gone, CTA gone (FR-021, SC-005, SC-009).

- [ ] T032 Add test to `tests/e2e/timer-overtime.spec.js`: *Pause during overtime reverts to Paused matrix* — while in overtime, emit `"global-shortcut"` with `"start-stop"` (pause), assert matrix shows `✕ Abort | ▶ Resume | ✓ Complete` (Paused trio from feature 006), CTA hidden, `.control-btn.overtime` classes absent. Resume (emit `"start-stop"` again), assert overtime treatment returns (FR-022, FR-023).

### e2e — settings-shortcuts.spec.js (extend)

- [ ] T033 Add test to `tests/e2e/settings-shortcuts.spec.js`: *Fourth-row Abort* — assert `#abort-shortcut` input element exists in Settings > Shortcuts, accepts a key binding, the binding appears in the `register_global_shortcuts` mock payload under the `abort` field, and persists across a settings reload (FR-018, FR-019, FR-020, SC-010).

### Visual Regression Baseline

- [ ] T034 Capture the new VR baseline `tests/e2e/__screenshots__/visual-regression/timer-focus-continuous-overtime-chromium-linux.png` by running `npx playwright test --update-snapshots --grep "overtime"` in continuous mode at ~14 minutes past zero (three orange `✓ Complete` slots, center filled, outer ghost, "Wrap it up!" CTA, pulsating-orange countdown). Done-signal: baseline file exists and the subsequent `npx playwright test` VR run passes within the 2% pixel-diff cap.

### Polish Gate

- [ ] T035 [P] Run `cargo fmt --all` across the workspace; fix any formatting issues in changed files (`crates/presto-ipc/src/settings.rs`, `src-tauri/src/lib.rs`, `src/src/app.rs`, `src/src/components/timer/mod.rs`, `src/src/components/settings/shortcuts.rs`). Done-signal: `cargo fmt --all -- --check` exits 0.

- [ ] T036 [P] Run `cargo clippy --all-targets -- -D warnings -W clippy::pedantic` across the full workspace. Fix any warnings introduced by this feature. No new `#[allow]` carve-outs. Done-signal: clippy exits 0 with no new warnings.

- [ ] T037 Run `npx playwright test` (full suite). All existing tests pass; new `timer-overtime.spec.js` tests pass; extended `settings-shortcuts.spec.js` test passes; VR baseline matches within 2%. Done-signal: Playwright exits 0.

- [ ] T038 Pre-PR design QA loop (per user-memory `feedback_design_qa_loop`): capture screenshots of (a) continuous-mode overtime in light theme — center saturated orange, outer ghost orange, CTA visible; (b) same in dark theme — `#f59e0b` warning palette; (c) paused-during-overtime light + dark — Paused matrix, countdown stays orange, CTA hidden. Iterate with styling sub-agent if any diverges from spec intent before opening the PR. Done-signal: light + dark screenshots both render cleanly per SC-007.

**Checkpoint**: All 40 tasks complete. Full test suite green. `cargo clippy` clean. One new VR baseline committed with one-line PR note. Design QA loop passed.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Catalogue + i18n)**: No dependencies — start immediately. All T001–T009 are [P].
- **Phase 2 (IPC Widening)**: No dependencies on Phase 1 — can run concurrently. T010–T012 (RED tests) before T013 (GREEN impl). RED-before-GREEN ordering is mandatory for the wire-format contract.
- **Phase 3 (US1 — overtime treatment)**: T014–T022. Depends on Phase 1 (locale keys needed at compile time) and T013 (IPC type widened). T014 (CSS) and T009 (i18n hygiene) can run in parallel with Phase 2.
- **Phase 4 (US2 — abort shortcut)**: T023–T026. Depends on T013. T023, T024, T025, T026 are largely [P] (different files).
- **Phase 5 (e2e + VR + polish)**: T027–T038. Depends on all implementation phases complete. T026 (mock check) must precede T031. T034 (VR capture) must follow T027–T033.

### User Story Dependencies

- **US1 (P1)** can begin as soon as Phase 1 + Phase 2 complete.
- **US2 (P2)** can begin as soon as T013 (Phase 2) completes — independent of US1 implementation tasks.
- US1 and US2 implementation tasks touch different files and can be worked in parallel once Phase 2 is done.

### Within Each Phase

- Phase 2: T010, T011, T012 (write RED tests, can be parallel) → T013 (make GREEN) → T013b (Tauri-bridge RED tests, can be parallel with T013c) → T013c (fixture strings) → T023 (make T013b GREEN).
- Phase 3: T014 (CSS) and T015 (`on_center_click`) can run in parallel. T016, T017 (click-dispatch) can run in parallel. T018–T022 can follow in any order after T015 is in place.
- Phase 4: T023, T024, T025, T026 can all run in parallel (different files).
- Phase 5: T027–T033 (e2e tests) can run in parallel per test file. T034 (VR) after T027–T033. T035, T036 in parallel. T037 after T035, T036. T038 last.

### Parallel Opportunities

```bash
# Phase 1: all locale edits in parallel
Tasks: T001–T009 (all touch different files)

# Phase 2: RED tests in parallel, then GREEN
Tasks: T010, T011, T012 in parallel → T013

# Phase 3 + Phase 4: after T013, run in parallel
Phase 3 (US1): T014, T015 in parallel → T016, T017 in parallel → T018–T022
Phase 4 (US2): T023, T024, T025, T026 in parallel

# Phase 5: e2e tests in parallel per spec file
Tasks: T027–T033 in parallel (same spec file, group by file) → T034 → T035, T036 in parallel → T037 → T038
```

---

## Spec-Coverage Matrix

| FR | Task(s) |
|---|---|
| FR-001 | T016, T017, T019, T022 |
| FR-002 | T016, T017, T019, T022 (gate: `Running && is_overtime` only) |
| FR-003 | T020 |
| FR-004 | T014, T020 |
| FR-005 | T014, T019 |
| FR-006 | T014 (`var(--warning-color)` light/dark) |
| FR-007 | T016, T017, T018 |
| FR-008 | T016, T017, T018 (dispatch via `on_complete` → engine `complete(clock)`) |
| FR-009 | T016, T017, T018 (no new tally increment — engine B.2 path unchanged) |
| FR-010 | T022 |
| FR-011 | T022 (same visibility predicate as buttons) |
| FR-012 | T001–T004 |
| FR-013 | T009 |
| FR-014 | T021 |
| FR-015 | T021 |
| FR-016 | T020 (`timer.ctrl_complete_aria` on center) |
| FR-017 | T024 (keyboard-only discard path) |
| FR-018 | T025 (Settings > Shortcuts fourth row) |
| FR-019 | T013 (`abort: None` default with asymmetry comment) |
| FR-020 | T023, T025, T033 (persistence via existing settings storage) |
| FR-021 | T024, T031 |
| FR-022 | T016, T017 (`Paused, *` row falls back to normal matrix) |
| FR-023 | T032 (smart-pause ≡ manual pause for matrix) |
| FR-024 | T016, T017, T022, T030 |

All 24 FRs are covered by at least one task. ✓

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1 (catalogue keys) — T001–T009
2. Complete Phase 2 (IPC widening RED-then-GREEN) — T010–T013
3. Complete Phase 3 (US1 overtime treatment) — T014–T022
4. **STOP and VALIDATE**: test overtime treatment manually per quickstart.md steps 1–6
5. US1 is independently shippable; US2 and e2e can follow

### Incremental Delivery

1. Phase 1 + Phase 2 → Foundation ready
2. Phase 3 (US1) → overtime treatment live → validate per quickstart.md
3. Phase 4 (US2) → Abort shortcut wired → validate keyboard discard per quickstart.md exercise 3
4. Phase 5 → full test + VR + polish → PR ready

---

## Notes

- `[P]` tasks touch different files and have no unmet dependencies — safe to launch in parallel.
- **Test-first ordering** (Phase 2 only): T010–T012 must be written and run RED before T013 makes them GREEN. Engine tests are not in scope (engine is untouched).
- **Mock-first**: T026 verifies the Tauri mock before e2e tests rely on it — no new mock command needed (mock accepts `register_global_shortcuts` payload-agnostically); only the `global-shortcut` emit helper needs verification.
- **No new engine logic**: branch B.2 (`complete(clock)` continuous-mode path) is fully covered by feature 006's tests. This feature touches only the UI, IPC, and settings layers.
- **VR budget**: 1 new baseline. Default cap suffices; no `.agentex.yml` carve-out required.
- **Kebab-case wire names throughout**: `"start-stop"`, `"reset"`, `"skip"`, `"abort"` — match arms in T024 MUST use kebab-case to match the Tauri emitter at `src-tauri/src/lib.rs:442-446`.
- **No `--no-verify`**: pre-commit hook runs on every commit.
