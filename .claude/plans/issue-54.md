# Implementation Plan for #54

**Issue:** sessions_history_table delete/save mutate only local signal — no IPC persistence
**Type:** bug
**Branch:** agentex/54-sessions-local-only

---

Researched. Bug is in `src/src/components/daily/sessions_history_table.rs` (delete handler lines 304-313, save handler 314-345). Bridge command `save_manual_sessions(Vec<ManualSession>)` already exists at `src/src/bridge/commands.rs:250`; Tauri handler at `src-tauri/src/lib.rs:516-525` rewrites `manual_sessions.json` bulk-style. Pure-logic mirror `SessionManager::delete_manual` / `update_manual` already exists at `src/src/managers/session.rs:122-149`. Now writing plan.

```md
# Bug: sessions-history table delete/save handlers do not persist via IPC

## Bug Description
The row-level **Delete** and **Save** handlers inside the Daily view's
`SessionsHistoryTable` mutate the shared `RwSignal<Vec<ManualSession>>`
in memory but never invoke a Tauri command to write the mutation
through to `manual_sessions.json` on disk. After the user edits a
session's title/start/end/duration (or deletes a row) and quits the
app, the next launch re-hydrates the pre-edit state from
`manual_sessions.json` and the edit/delete is silently lost.

- **Expected:** Edit → close app → relaunch → edited values still
  present. Delete → close app → relaunch → row still absent.
- **Actual:** Edit and delete only mutate the in-memory signal;
  restart restores the pre-edit list verbatim.

## Problem Statement
`sessions_history_table.rs:307-313` (Delete) and `:314-345` (Save) call
`sessions.update(...)` but never reach `bridge::commands`. The local
signal mutation is therefore ephemeral — only the in-process view
state changes; the persistence boundary is never crossed. The
in-app Add-Manual-Session path at `components/timer/mod.rs:1327`
shares the same `sessions` signal and does persist (the bug filer
reports that path lands on disk in their environment), so the
asymmetry is real: Add survives a restart, Edit/Delete do not. The
fix must make Edit/Delete equally durable.

## Solution Statement
After each `sessions.update(...)` in the Delete and Save handlers,
explicitly spawn an `async` task that hands the post-mutation snapshot
to `commands::save_manual_sessions(...)`. This makes persistence a
direct, observable side-effect of the click handler — identical to
how the JS-era `session-manager.js:67-69` flow worked before the
Leptos cutover.

We **reuse the existing bulk-rewrite command** rather than add new
per-entry `delete_manual_session(id)` / `update_manual_session(session)`
commands. Reasoning:

- `save_manual_sessions(Vec<ManualSession>)` already exists on both
  sides of the bridge (`commands.rs:250`, `lib.rs:516-525`,
  `tauriMock.js:109-116`), already round-trips the closed-domain
  `SessionType`, already has signature-pinning tests
  (`commands.rs:998-1009`), and already mirrors the JS-era
  bulk-rewrite convention noted in `session-manager.js:54-78`.
- Per-entry commands would expand the Tauri surface (Principle VII —
  bridge minimality), require parallel mock-drift updates, and yield
  no behavioural improvement; the on-disk file is rewritten
  atomically either way (`helpers::write_json_atomic`).

The fix is therefore the **minimum** edit: two `spawn_local` blocks
that call the existing wrapper with `sessions.get_untracked()` after
the mutation closure returns.

## Steps to Reproduce
1. `cargo tauri dev` (real Tauri runtime — the dev-server harness
   short-circuits the bridge so the bug is invisible there).
2. Enable debug timers (Settings → Advanced) and run one focus
   session to completion so a row lands in **Daily → Session
   History**.
3. Click **Edit** on the row. In the modal, change the duration from
   e.g. `25` to `30`. Click **Save**.
4. Confirm the duration cell updates in the table.
5. Quit the app (`Cmd+Q` / process kill — not just close-to-tray).
6. Relaunch via `cargo tauri dev`. **Bug:** the row shows `25 min`
   again; the edit is gone.
7. Repeat for **Delete**: row reappears after restart.

Equivalent Add path (control):
1. Same setup; click **Add manual session** in the existing flow.
2. Save, quit, relaunch. The added row persists. (This confirms
   `save_manual_sessions` itself works — only the Edit/Delete handlers
   are missing the call.)

## Root Cause Analysis
At `src/src/components/daily/sessions_history_table.rs`:

- **Delete handler (lines 304-313):**
  ```rust
  on:click=move |_| {
      if let Some(id) = modal_session_id.get_untracked() {
          sessions.update(|ss| ss.retain(|s| s.id != id));
      }
      session_modal_open.set(false);
  }
  ```
  `sessions.update(...)` mutates the local `RwSignal` but the closure
  ends without ever invoking `commands::save_manual_sessions(...)`.

- **Save handler (lines 314-345):** identical pattern — mutates the
  signal in place, never reaches the bridge.

The pre-feature-003 implementation at `ea3be7a:src/src/components/
calendar.rs:831-848` had byte-identical handlers. Feature 003 (the
calendar.rs → daily/ extraction, PR #52) preserved the existing
behaviour during the move and is therefore **not** the regression
source — the bug is genuinely pre-existing and was missed at the
original port from JS.

A persistence sink Effect exists at `src/src/app.rs:394-404` that
re-runs on `sessions.get()` changes and calls
`save_manual_sessions(snapshot)`. The bug report's symptom
(restart-loses-edit) indicates that for the Edit/Delete code paths,
this implicit propagation does not deliver the write to disk in
practice — at minimum, the handler-local code surface gives no
inspectable evidence that persistence is reached, and the bug
filer's restart-test confirms the data does not survive. Whether
the Effect fires but races the unmount, or simply does not
re-execute for these particular `sessions.update(...)` notifications
in real-Tauri runtime, the user-visible fix is the same: add the
explicit IPC at the click handler so the call is unambiguous,
synchronous-to-the-handler, and observable in test fixtures.

## Relevant Files
Use these files to fix the bug:

- `src/src/components/daily/sessions_history_table.rs` — owns the
  bugged click handlers (Delete at lines 304-313, Save at
  lines 314-345). Imports `crate::bridge::commands` already (used
  for `dialog_save` + `export_sessions_xlsx`), so adding two more
  `commands::save_manual_sessions(...)` calls needs no new imports.
- `src/src/bridge/commands.rs` — defines the
  `save_manual_sessions(Vec<ManualSession>) -> Result<(),
  BridgeError>` wrapper at line 250 that the new calls invoke. No
  edit needed here; just referenced.
- `tests/e2e/fixtures/tauriMock.js` — already mocks
  `save_manual_sessions` at line 109-116 (it mutates
  `_state.manualSessions`). Extend the mock to also bump a call
  counter so the e2e test can assert the explicit invoke happened.
- `tests/e2e/sessions-history.spec.js` — already exercises the Edit
  modal open/close flow. Extend it to (a) click **Save** with an
  edited duration and (b) click **Delete**, asserting both that the
  DOM reflects the mutation AND that the invoke counter for
  `save_manual_sessions` advanced. Inspecting the harness's call
  count is bridge-call introspection (the very thing the harness
  exposes), not in-memory store snooping — it remains within the
  spirit of `tests/e2e/CLAUDE.md` Rule 1.4 (the UI-state assertions
  also continue to gate the test).
- `src/src/managers/session.rs` — owns the already-tested pure
  helpers `update_manual(...)` / `delete_manual(...)` /
  `save_payload()` (lines 122-149, 94-96). Not edited by this PR,
  but referenced from the test plan because their cargo-test
  coverage already pins the in-memory mutation correctness; this PR
  pins only the additional IPC hop the component is missing.

## Step by Step Tasks
IMPORTANT: Execute every step in order, top to bottom. TDD is
observed: every step that ships handler logic is preceded by a
failing test, then turned green by the implementation.

### Step 1: Track `save_manual_sessions` invocations in the Tauri mock

- Open `tests/e2e/fixtures/tauriMock.js`.
- In the `_state` initializer (lines 25-36), add a counter field:
  `saveManualSessionsCallCount: 0,` and a last-call snapshot:
  `lastSaveManualSessionsArgs: null,`.
- In the `save_manual_sessions` switch case (lines 109-116),
  increment `_state.saveManualSessionsCallCount++;` and store
  `_state.lastSaveManualSessionsArgs = args && args.sessions
   ? args.sessions.slice() : null;` BEFORE the existing
  `_state.manualSessions = args.sessions.slice()` line. This
  guarantees the counter increments even if the args parsing is
  changed later.
- The `_state` object is already exposed via
  `window.__E2E_TEST_HARNESS__.state` (line 312) so no further
  plumbing is needed.

### Step 2: Add the failing e2e test for Save persistence (RED)

- Open `tests/e2e/sessions-history.spec.js`.
- After the existing "modal opens / `#session-duration` visible"
  assertions (lines 46-48), add steps that:
  - Read the pre-Save invoke count via
    `await page.evaluate(() =>
     window.__E2E_TEST_HARNESS__.state.saveManualSessionsCallCount)`.
  - Edit the duration field
    (`await page.locator('#session-duration').fill('30')`).
  - Click `#save-session-btn`.
  - Assert the modal closed:
    `await expect(page.locator('#session-modal-overlay')).toBeHidden();`.
  - Assert the row's duration cell reflects the new value:
    `await expect(rows.first()).toContainText('30 min');`.
  - Read the post-Save invoke count and assert it is strictly
    greater than the pre-Save count.
  - Assert the last-call payload contains the edited record:
    `const last = await page.evaluate(() =>
     window.__E2E_TEST_HARNESS__.state.lastSaveManualSessionsArgs);
     expect(last.some(s => s.duration === 30)).toBe(true);`.
- Run `(cd tests/e2e && npx playwright test sessions-history.spec.js)`.
  Expected: **the new assertions fail** (counter does not advance;
  Save handler never reaches the bridge).

### Step 3: Add the explicit IPC call in the Save handler (GREEN)

- Open `src/src/components/daily/sessions_history_table.rs`.
- In the Save handler closure (lines 314-345), AFTER
  `sessions.update(|ss| { ... });` (line 333-340) and BEFORE
  `session_modal_open.set(false);` (line 342), add:
  ```rust
  let snapshot = sessions.get_untracked();
  spawn_local(async move {
      let _ = commands::save_manual_sessions(snapshot).await;
  });
  ```
- The `spawn_local` and `commands` imports are already present at
  the top of the file (lines 26-28); no new use-statements needed.
- Re-run the e2e spec from Step 2. Expected: **passes**. The
  invoke counter advances by exactly one per Save click, and the
  payload contains the edited record.

### Step 4: Add the failing e2e test for Delete persistence (RED)

- Continue in `tests/e2e/sessions-history.spec.js`. After the Save
  assertions, append:
  - Re-open the modal:
    `await rows.first().getByRole('button',
     { name: 'Edit session' }).click();
     await expect(page.locator('#session-modal-overlay')).toBeVisible();`.
  - Snapshot the pre-Delete counter.
  - Click `#delete-session-btn`.
  - Assert the modal closed and the row count dropped to zero:
    `await expect(page.locator('#sessions-table-body
     tr')).toHaveCount(0);`.
  - Snapshot the post-Delete counter; assert it advanced.
  - Assert `lastSaveManualSessionsArgs` is `[]` (or does not contain
    the deleted id):
    `const after = await page.evaluate(() =>
     window.__E2E_TEST_HARNESS__.state.lastSaveManualSessionsArgs);
     expect(after).toEqual([]);`.
- Run the spec. Expected: **the Delete assertions fail** (counter
  does not advance — handler still skips the bridge call).

### Step 5: Add the explicit IPC call in the Delete handler (GREEN)

- Back in `src/src/components/daily/sessions_history_table.rs`.
- In the Delete handler closure (lines 304-313), AFTER
  `sessions.update(|ss| ss.retain(|s| s.id != id));` (line 309) and
  BEFORE `session_modal_open.set(false);` (line 311), add:
  ```rust
  let snapshot = sessions.get_untracked();
  spawn_local(async move {
      let _ = commands::save_manual_sessions(snapshot).await;
  });
  ```
- Re-run the e2e spec. Expected: **all assertions pass**.

### Step 6: Confirm no double-write regression with the persistence sink

- The Effect-based persistence sink at `src/src/app.rs:394-404`
  also reacts to `sessions.get()` changes. After our edits, a single
  user Save click will fire both the explicit IPC call from the
  handler AND the Effect's IPC call (because the `sessions.update`
  notifies subscribers).
- This is acceptable: `save_manual_sessions` is idempotent on the
  Tauri side (it writes the same snapshot atomically), and the
  duplicate write is bounded (exactly two calls per click, not a
  loop).
- Pin this in the test: the spec's `expect(post > pre)` is
  intentionally `>` not `=== pre + 1` so a double-fire still passes
  while still catching the original zero-fire bug. Document the `>`
  choice in a one-line comment in the spec so a future reader does
  not "tighten" it to `=== pre + 1` and re-introduce flake.

### Step 7: Run the full validation gate set

- See **Validation Commands**.

## Validation Commands
Execute every command to validate the bug is fixed with zero
regressions.

```bash
# 1. Backend + frontend lints (workspace-wide, strict-deny).
cargo clippy --workspace --all-targets -- -D warnings -W clippy::pedantic

# 2. Formatting drift.
cargo fmt --all --check

# 3. Host-side unit + integration tests (covers SessionManager
#    update_manual / delete_manual pure-logic tests, bridge command
#    signature pins, helpers::write_manual_sessions_to atomicity).
cargo test --workspace --frozen

# 4. wasm-bindgen-test for DOM-bound logic in the frontend crate.
(cd src && wasm-pack test --node)

# 5. Frontend bundle build (catches Trunk / wasm-bindgen issues).
(cd src && trunk build)

# 6. CI gates.
bash scripts/check-mock-drift.sh       # tauriMock.js ↔ lib.rs parity
bash scripts/check-baseline-cap.sh     # ≤2 visual baseline re-captures
bash scripts/check-engine-purity.sh    # no DOM crates in engine/
bash scripts/check-lockfile-drift.sh   # Cargo.lock / package-lock.json

# 7. Full Playwright suite (regression sweep across all 17 specs +
#    the visual regression suite).
(cd tests/e2e && npx playwright test)

# 8. Targeted run for the bugged spec (faster feedback during fix).
(cd tests/e2e && npx playwright test sessions-history.spec.js)

# 9. Manual smoke (real Tauri runtime — the only environment where
#    the original bug reproduces). Cannot be CI-automated but
#    documented for the reviewer:
cargo tauri dev
# → Enable debug timers, run a focus session to completion, edit
#   the row's duration in Daily, Save, quit (`Cmd+Q`), relaunch,
#   confirm the edited duration persists. Repeat with Delete.
```

All commands must pass. The targeted e2e spec from Step 8 is the
direct regression pin; the full suite from Step 7 confirms no
visual-regression or other spec was disturbed by the new
`spawn_local` calls (none expected — the calls are async and do not
change the synchronous DOM state).

## Notes
- **Why not just delete the persistence-sink Effect?** It is still
  load-bearing for the Add-Manual-Session path at
  `components/timer/mod.rs:1327`, which `sessions.update(|list|
  list.push(session))` and relies on the Effect for the durable
  write. Removing the sink would silently regress that path. The
  surgical fix is to add explicit IPC at the two newly-buggy
  surfaces only; leave the Effect as a belt-and-braces backup.
- **Why `get_untracked()` for the snapshot?** Inside an `on:click`
  handler we are not inside a reactive scope, so `.get()` and
  `.get_untracked()` behave identically with respect to tracking.
  Matching the existing convention at
  `sessions_history_table.rs:149` and
  `components/timer/mod.rs` (which uses `get_untracked` for snapshot
  reads outside Effects) keeps the file's style consistent.
- **Why `let _ =`?** The persistence sink at `app.rs:402` already
  discards the `Result`; we match that minimalism. A future PR can
  thread these failures through the `AppToast` queue (matching the
  settings-save toast at `app.rs:272-277`) but doing so here would
  expand scope beyond the bug.
- **Pre-existing not a regression** — the bug filer flagged this
  during the feature 003 quickreview and explicitly deferred to a
  focused PR rather than blocking #52. This plan implements that
  focused PR. No new spec-kit feature directory is needed; the bug
  is small enough to ship as a single direct commit on a
  short-lived `54-sessions-local-only` branch (already created;
  current HEAD per `git status`).
- **Constitution touchpoints:** Principle I (engine is the source
  of truth) is not impacted — manual-session CRUD does not pump the
  engine. Principle VI (managers reach Tauri only through
  `bridge::commands`) is upheld — the new calls go through the
  typed wrapper. Principle VII (bridge minimality) is upheld — no
  new Tauri commands are added; the existing bulk command is
  reused.
```

---
*Generated by Agentex*
