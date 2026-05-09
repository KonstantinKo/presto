# Implementation Plan for #9

**Issue:** Add integration tests (MockRuntime or Playwright/WebdriverIO E2E) (Phase 3)
**Type:** feature
**Branch:** agentex/9-add-integration-tests

---

I have enough context. Outputting the plan below.

```md
# Feature: Phase 3 Integration Tests via Tauri MockRuntime

## Feature Description
Add backend integration tests that exercise the `#[tauri::command]` surface of Presto end-to-end using Tauri 2's `MockRuntime`, plus tempdir-isolated round-trip tests for the I/O helpers those commands delegate to. These tests validate the Rust command contract — the stable boundary between any frontend (current vanilla JS, future Leptos/WASM) and the persistence layer — and run in CI under the existing `cargo test` step.

This is **Phase 3** of the test pyramid started in #4 (Phase 1 — runners + pure unit tests) and #8 (Phase 2 — manager unit tests with mocked Tauri). Phase 3 goes one level deeper: instead of mocking `window.__TAURI__.invoke()` from JS, it constructs a real Tauri app with `MockRuntime` and dispatches IPC requests through Tauri's actual invoke pipeline.

## User Story
As a maintainer of Presto preparing for a frontend stack swap (vanilla JS → likely Leptos/WASM)
I want backend integration tests that exercise `#[tauri::command]` functions through the real Tauri IPC machinery and tempdir-isolated I/O helper tests
So that the persistence and command contract — which is the stable seam between any frontend and the Rust backend — is verified by tests that survive the swap unchanged, without paying the cost of a full Playwright/WebDriver E2E rig.

## Problem Statement
After Phase 2, Presto has unit-test coverage at two levels:
1. **Pure-helper Rust tests** in `src-tauri/src/helpers.rs` (debounce, atomic write).
2. **Frontend manager tests** that drive `SessionManager` / `SettingsManager` / `NavigationManager` against a mocked `window.__TAURI__`.

What's missing — and what issue #9 calls out — is coverage that crosses the IPC boundary:
- The frontend manager tests **trust** that `invoke("save_manual_sessions", { sessions })` does the right thing on the Rust side. Nothing actually validates the Rust commands' behavior end-to-end.
- The thirty-plus `#[tauri::command]` functions in `src-tauri/src/lib.rs` are wholly untested. The current `cargo test` in `src-tauri` runs only the four pure-helper tests in `helpers.rs`.
- After the upcoming frontend stack swap, the Phase 2 JS manager tests die (they import `src/managers/*.js` and mock `window.__TAURI__`). The only artifacts that survive are pure Rust tests and any tests that exercise the command surface or visible UI — neither of which exists today.

## Solution Statement
Take the **MockRuntime path** from issue #9's two acceptance options. Concretely:

1. **Extract pure I/O helpers into `helpers.rs`** (or a new `storage` submodule) that accept a `&Path` for the data directory. Existing `#[tauri::command]` functions become thin glue that resolves `app.path().app_data_dir()` and delegates to the helper. This is a refactor with no behavior change; the command names, argument names, and return shapes stay identical.

2. **Add Cargo tests for the helpers** using `tempfile::TempDir` to isolate each test. These verify the meaty business logic — JSON round-trip, atomic write, date-rollover in `load_session_data`, history pruning to 30 days, default-tag bootstrap, settings deserialization with `#[serde(default)]` fallbacks — without needing any Tauri runtime.

3. **Add at least one MockRuntime integration test** at `src-tauri/tests/commands.rs` that exercises a `#[tauri::command]` end-to-end via `tauri::test::get_ipc_response`. The first target is `write_excel_file`, which is uniquely well-suited because it already accepts the destination path as an argument — no path-resolver override needed; just point it at a tempdir. This satisfies the "at least one integration test exercises a `#[tauri::command]` end-to-end" acceptance gate.

4. **Wire it into CI**. The existing `.github/workflows/ci.yml` already runs `cd src-tauri && cargo test`, which automatically picks up both `src/` unit tests and the new `tests/` integration tests. No CI changes needed beyond verifying the new dev-dependencies build.

**Why MockRuntime over Playwright/WebdriverIO**:
- Lower CI cost (no `tauri dev`, no WebKit2GTK browser harness, no extra container deps).
- The Rust command surface is what *actually* persists across the Leptos swap; testing it directly tests the contract that matters most.
- Acceptance only requires *one* of the two paths.
- README states the upstream project is abandoned; this fork is in maintenance/handoff mode and a heavyweight E2E rig is hard to justify.
- Helper extraction also benefits readability and matches the pattern set by `helpers.rs::is_debounced` (called out approvingly in issue #4 as the right way to structure testable Rust).

## Relevant Files

- `src-tauri/src/lib.rs` — contains all 30+ `#[tauri::command]` functions. Most use `app.path().app_data_dir()` + `serde_json` + `helpers::write_json_atomic`. Refactor target: extract the I/O bodies into `&Path`-taking helpers; commands shrink to glue.
- `src-tauri/src/helpers.rs` — current home of `write_json_atomic`, `lock_or_recover`, `is_debounced` plus their `#[cfg(test)] mod tests`. New helpers will live here (or in a new sibling module if the file gets too large).
- `src-tauri/Cargo.toml` — needs new `[dev-dependencies]` (`tempfile`, `tauri = { version = "2", features = ["test"] }`).
- `src-tauri/Cargo.lock` — will update on `cargo build`; commit it.
- `src-tauri/src/main.rs` — no changes; just calls `presto_lib::run()`.
- `src-tauri/tauri.conf.json` — no changes (mock context will be built in code, not via this file).
- `.github/workflows/ci.yml` — verify `cd src-tauri && cargo test` continues to pass; no edit expected unless we discover the WebKit deps need additions for the `tauri/test` feature.
- `.agentex.yml` — already runs `cargo test` in the `qa.test` section; no edit expected.
- `tests/setup/tauri-mock.js` — JS-side mock; do **not** touch (Phase 2 artifact, will die on swap).
- `src-tauri/.gitignore` — `target/` already ignored.

### New Files

- `src-tauri/tests/commands.rs` — new integration-test target. Uses `tauri::test::mock_builder` + `get_ipc_response` to drive `write_excel_file` (and any other practically-isolatable commands) through real IPC against `MockRuntime`.
- *(Optional, deferred)* `src-tauri/src/storage.rs` — only if `helpers.rs` grows past ~300 lines after the helper extractions. Default is to keep everything in `helpers.rs`.

## Implementation Plan

### Phase 1: Foundation
Add the dev-dependencies and the helper-extraction scaffolding without changing public command behavior. This is a refactor that should leave `cargo test` and the existing manager tests passing unchanged.

### Phase 2: Core Implementation
Write the helper-level tempdir tests covering each storage operation's round-trip + edge cases (date-rollover, missing files, malformed JSON, history pruning, default-tag bootstrap). These are the bulk of the actual coverage.

### Phase 3: Integration
Add the MockRuntime IPC test that drives `write_excel_file` end-to-end through `tauri::test::get_ipc_response`. Verify CI green, document in `.claude/docs/` if conventions exist.

## Step by Step Tasks

IMPORTANT: Execute every step in order, top to bottom.

### Step 1: Add dev-dependencies
- Edit `src-tauri/Cargo.toml`. Add a `[dev-dependencies]` section (or extend if present):
  ```toml
  [dev-dependencies]
  tempfile = "3"
  tauri = { version = "2", features = ["test"] }
  ```
- Run `cd src-tauri && cargo build --tests` to verify the dev-deps resolve and the `tauri::test` module is available. (The `test` feature unlocks `tauri::test::{mock_builder, mock_app, mock_context, get_ipc_response, MockRuntime, INVOKE_KEY}`.)
- Commit `Cargo.lock` updates.

### Step 2: Extract a settings I/O helper pair
- In `src-tauri/src/helpers.rs`, add:
  ```rust
  pub(super) fn read_settings_from(dir: &Path) -> Result<AppSettings, String> { ... }
  pub(super) fn write_settings_to(dir: &Path, settings: &AppSettings) -> Result<(), String> { ... }
  ```
- The helpers reproduce the exact behavior currently in `load_settings` / `save_settings`: `read_settings_from` returns `AppSettings::default()` when `settings.json` is missing or malformed; `write_settings_to` calls `fs::create_dir_all(dir)` then `write_json_atomic(dir.join("settings.json"), settings)`.
- Move `AppSettings` (and its sub-structs) visibility from private to `pub(crate)` so the helpers can reference them. Alternatively, define the helpers inline in `lib.rs` next to the types — the choice is whichever keeps the diff minimal; do not move types into `helpers.rs` if it requires a large visibility cascade.
- Refactor `load_settings` / `save_settings` in `lib.rs` to delegate: resolve `app.path().app_data_dir()`, then call the helper.
- Run `cargo test` and `cargo clippy --all-targets -- -D warnings` to confirm no regression.

### Step 3: Extract session/tasks/history/manual-sessions/tags I/O helpers
- Repeat the Step 2 pattern for the rest of the file-touching commands. Group by domain:
  - `read_session_from(dir) -> Result<Option<PomodoroSession>, String>` (preserves the date-rollover branch from `load_session_data`)
  - `write_session_to(dir, &PomodoroSession) -> Result<(), String>`
  - `read_tasks_from`, `write_tasks_to`
  - `read_history_from`, `append_daily_stats_to` (preserves the 30-day truncation + dedupe-by-date logic)
  - `read_manual_sessions_from`, `write_manual_sessions_to`, `upsert_manual_session_in`, `delete_manual_session_in`
  - `read_tags_from` (preserves the default-tag bootstrap), `write_tags_to`, `upsert_tag_in`, `delete_tag_in`
  - `read_session_tags_from`, `write_session_tags_to`, `append_session_tag_in`
  - `delete_all_data_in(dir) -> Result<(), String>` (mirrors `reset_all_data`'s file list)
- Refactor each `#[tauri::command]` to delegate. Public command signatures, names, and JSON shapes do not change.
- Run `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `npm test` (frontend manager tests should still pass — they don't care about Rust internals).

### Step 4: Add helper-level tempdir round-trip tests
- Inside `helpers.rs`'s existing `#[cfg(test)] mod tests`, add tests using `tempfile::TempDir`. Cover:
  - **Settings**: round-trip default → write → read returns equal; missing-file returns defaults; malformed JSON returns defaults (matches the `unwrap_or_else` branch in `load_settings_sync`).
  - **Session**: write → read returns `Some(session)` with same fields; missing file returns `None`; date-rollover: stored session with stale `date` field returns `Some` with `completed_pomodoros = 0` and updated date.
  - **Tasks**: round-trip empty vec, round-trip multi-element vec.
  - **History**: appending the 31st entry prunes the oldest; appending an entry whose date matches an existing entry replaces it.
  - **Manual sessions**: upsert by `id` replaces; delete by `id` removes; both leave unrelated rows untouched.
  - **Tags**: missing-file path bootstraps the default "Focus" tag and persists it; `upsert` by `id` replaces; `delete` removes.
  - **Reset**: with all data files present, `delete_all_data_in` removes them; with no files, returns Ok and does not error on non-existence.
- Each test owns its own `TempDir` so they can run in parallel.
- Aim for ~15–20 helper-level tests total.

### Step 5: Create the MockRuntime integration test crate
- Create `src-tauri/tests/commands.rs`. (Cargo automatically picks up integration tests under `tests/`.)
- Set up the harness:
  ```rust
  use tauri::test::{mock_builder, mock_context, noop_assets, get_ipc_response, INVOKE_KEY};
  use tauri::WebviewUrl;
  use tauri::webview::InvokeRequest;

  fn make_app() -> tauri::App<tauri::test::MockRuntime> {
      mock_builder()
          .invoke_handler(tauri::generate_handler![
              presto_lib::write_excel_file,
              // ...add more as they become test-friendly
          ])
          .build(mock_context(noop_assets()))
          .expect("failed to build mock app")
  }
  ```
  Note: `presto_lib::write_excel_file` requires the function to be `pub` (or `pub(crate)` re-exported). Adjust visibility accordingly.

### Step 6: Write the first end-to-end MockRuntime test
- Test name: `write_excel_file_writes_decoded_bytes_to_provided_path`.
- Body:
  - Build the mock app via `make_app()`.
  - Get the main webview: `let webview = app.get_webview_window("main").unwrap();` (or use `app.webviews()` — verify the API in Tauri 2.6 docs/source; `mock_builder` constructs a default window).
  - Create a `tempfile::TempDir`; compute a target path like `tmp.path().join("out.xlsx")`.
  - Base64-encode a known byte sequence (e.g. `b"hello-presto"`) and assemble an `InvokeRequest` with `cmd = "write_excel_file"` and the args `{ path: <tempdir path>, data: <base64> }`.
  - Call `get_ipc_response(&webview, request)` and assert the response is `Ok`.
  - Read the file from disk and assert the bytes equal the original input.
- Annotate at the top of the file:
  ```rust
  // Integration tests for #[tauri::command] functions via Tauri 2's MockRuntime.
  // These tests survive a frontend stack swap because they target the Rust IPC
  // surface, not any specific frontend's invoke shape.
  ```
- Run `cd src-tauri && cargo test --test commands` to verify it passes in isolation, then `cargo test` to verify nothing regressed.

### Step 7: (Stretch) Add a second MockRuntime test for a settings round-trip
- *Only attempt if `app.path().app_data_dir()` under `MockRuntime` resolves to a path we can override or sandbox without invasive lib.rs changes.* Try one of:
  - **Approach A (preferred if it works)**: configure the mock context with a unique `bundle.identifier` per test (e.g. `format!("com.presto.test-{}", uuid)`) and clean up the resulting OS-specific app-data directory in a teardown.
  - **Approach B**: skip — helper-level tests already cover the behavior, and the acceptance gate requires only one IPC integration test.
- If Approach A works, add `save_settings_then_load_settings_round_trips` that invokes both commands through IPC and asserts equality.
- If Approach A does not work cleanly, document why in a comment in `commands.rs` and stop. Do not refactor commands to take a path-injection parameter purely to unlock this test — it's not required for acceptance.

### Step 8: Run the full QA suite
- `cd src-tauri && cargo build --all-targets` — ensures all targets including new tests compile.
- `cd src-tauri && cargo test` — runs unit + helper + integration tests.
- `cd src-tauri && cargo clippy --all-targets -- -D warnings` — lint must be clean (note: the project enables `clippy::pedantic` and `clippy::nursery` as `deny`; the new test code must conform).
- `cd src-tauri && cargo fmt -- --check`.
- `npm test` — frontend Phase 2 tests must still pass.
- `npm run typecheck`, `npm run lint`, `npx prettier --check .` — no frontend regressions.

### Step 9: Verify CI integration
- Read `.github/workflows/ci.yml`. Confirm the `backend` job runs `cd src-tauri && cargo test`. New integration tests are automatically included by Cargo's default test discovery.
- If `tauri/test` requires extra system libraries on Ubuntu beyond what's already installed (`libwebkit2gtk-4.1-dev`, etc.), document the addition. Most likely none are needed because `MockRuntime` does not spawn a real webview.
- *Do not* push commits until Step 8 is fully green locally.

### Step 10: Document the test layout
- Add a brief comment at the top of `src-tauri/tests/commands.rs` explaining the harness pattern (one paragraph, what `MockRuntime` does, why `write_excel_file` is the entrypoint).
- *Do not* create new top-level docs; keep the explanation in the file. Conform to project rule "default to writing no comments — only when WHY is non-obvious"; here, "why MockRuntime + why this specific command" is non-obvious.

## Testing Strategy

### Unit Tests
Helper-level tests in `src-tauri/src/helpers.rs` `#[cfg(test)] mod tests`:
- **Settings helpers**: read-default-on-missing-file, write-then-read round-trip, read-default-on-malformed-JSON, write creates parent dir if absent, atomic write resilience (write_json_atomic already covered indirectly).
- **Session helpers**: write-then-read round-trip, missing-file returns `None`, stale-date triggers reset to fresh-day defaults, same-day with legacy date format normalizes to today.
- **Tasks helpers**: empty-vec round-trip, multi-element round-trip, missing-file returns empty vec.
- **History helpers**: appending past-30 prunes oldest, appending duplicate-date replaces, sort-by-date semantics preserved.
- **Manual session helpers**: upsert-by-id replaces (not duplicates), delete-by-id removes only that row.
- **Tags helpers**: missing-file bootstraps default "Focus" tag and persists it, upsert-by-id replaces, delete-by-id removes.
- **Reset helper**: removes all listed files when present, no error when files absent.

### Integration Tests
At `src-tauri/tests/commands.rs`:
- **`write_excel_file_writes_decoded_bytes_to_provided_path`** — drives `write_excel_file` end-to-end through `tauri::test::get_ipc_response` against `MockRuntime`; asserts the file is written with correctly base64-decoded bytes. **This is the test that satisfies issue #9's acceptance criterion.**
- *(Optional)* `save_settings_then_load_settings_round_trips` if a clean tempdir-isolation approach materializes for `app_data_dir()`.

### Edge Cases
- Helper called with a non-existent directory: writers must create it; readers must return defaults/empty/None as appropriate.
- Malformed JSON on disk: readers do not panic; settings reader returns defaults (matching current `unwrap_or_else` behavior).
- Concurrent writers: not introducing new locking; relying on the existing `write_json_atomic` (rename is atomic on the same filesystem).
- Date-format compatibility: `load_session_data` accepts both legacy `"%a %b %d %Y"` and ISO `"%Y-%m-%d"`; the helper test must cover both.
- `write_excel_file` with invalid base64: command returns `Err("Failed to decode base64 data: ...")` — assert the IPC response surfaces the error string and the file is not created.
- `write_excel_file` to a path whose parent does not exist: command returns `Err("Failed to write Excel file to ...")` — assert error propagation.

## Acceptance Criteria
- [ ] At least one integration test (`src-tauri/tests/commands.rs`) drives a `#[tauri::command]` end-to-end through `tauri::test::get_ipc_response` against `MockRuntime` and asserts a user-visible outcome (a file was written with the correct bytes).
- [ ] The chosen runner (`cargo test`) is wired into CI; the `backend` job in `.github/workflows/ci.yml` exercises the new tests on every push and pull request.
- [ ] At least 10 new helper-level tempdir round-trip tests exist in `src-tauri/src/helpers.rs` covering settings, session, tasks, history, manual sessions, tags, and reset.
- [ ] All `#[tauri::command]` functions that previously inlined I/O now delegate to a `&Path`-taking helper. Command function bodies become "resolve `app_data_dir`, call helper, return result" (plus analytics tracking where present). Public command names, JSON argument names, and return shapes are unchanged.
- [ ] No regression in existing tests: `cargo test`, `npm test`, `npm run typecheck`, `npm run lint`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt -- --check`, `prettier --check .` all pass locally.
- [ ] The Phase 2 JS manager tests at `tests/managers/*.test.js` are untouched and still pass.
- [ ] Each test that retains coupling to a specific command-name string (e.g. asserts `"write_excel_file"` literally) carries a `// TODO(stack-swap):` comment naming what would need updating on stack swap, per the issue's stated convention. (Note: the IPC integration test by definition references the command name; this is acceptable because it's testing the Rust contract that survives the swap.)

## Validation Commands
Execute every command to validate the feature works correctly with zero regressions.

```bash
# Backend builds clean (including tests target)
cd src-tauri && cargo build --all-targets

# Backend tests pass: includes existing helpers tests + new helper tests + new integration test
cd src-tauri && cargo test

# Backend lints clean (project enforces clippy::pedantic + nursery as deny)
cd src-tauri && cargo clippy --all-targets -- -D warnings

# Backend fmt clean
cd src-tauri && cargo fmt -- --check

# Frontend tests still pass (Phase 2 JS manager tests must not regress)
npm test

# Frontend type-check / lint / format clean
npm run typecheck
npm run lint
npx prettier --check .

# End-to-end aggregate (mirrors .agentex.yml qa.test pipeline)
cd src-tauri && cargo build --all-targets && cargo test && cd .. && npm test
```

For verifying the *integration* test specifically (so a reviewer can see it in isolation):
```bash
cd src-tauri && cargo test --test commands -- --nocapture
```

For verifying CI parity locally (matches `.github/workflows/ci.yml` backend job):
```bash
cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings
```

## Notes

- **Why MockRuntime over Playwright** is documented in the Solution Statement above. The TL;DR for future maintainers: lighter CI, validates the contract that survives the swap, and the acceptance criterion only requires one of the two paths.
- **Helper extraction is the load-bearing change.** The MockRuntime IPC test on its own would only meet the bare letter of the acceptance criterion; it is the helper-level tests that provide real coverage and survive the stack swap most cleanly. Reviewers should weight helpers tests as the "mass" of the PR, the IPC test as the "shape" of it.
- **Visibility considerations.** `presto_lib::write_excel_file` (and any other commands referenced from `tests/commands.rs`) must be `pub` (or re-exported via `pub use`). This is a small visibility concession to support integration testing; document the rationale with one short comment so future readers don't widen visibility further.
- **Future work, not in scope here**:
  - A second integration test covering `save_settings`/`load_settings` round-trip if a clean `app_data_dir` override pattern emerges (e.g. Tauri exposes a path-resolver injection point in a later 2.x release).
  - Visual regression / Playwright suite — if Leptos lands and the team wants UI-level coverage, that becomes worth the investment then; revisit once the new stack is in place.
  - Coverage for tray, global-shortcut, autostart, and macOS-only commands — each requires runtime-specific scaffolding beyond what `MockRuntime` provides; tracked in follow-up issues if desired.
- **Stack-swap annotation policy** (from issue #9): the IPC test references the command name `"write_excel_file"` as the IPC entry — this is *correct*, because it is testing the Rust IPC contract that survives the swap. The `// TODO(stack-swap):` annotations in this PR's helper tests are unnecessary because the tests don't reference any frontend module path or DOM detail. The annotation rule applies only to JS-side tests.
- **Rust toolchain**: project pins `rust 1.89.0` via `.tool-versions`. `tempfile` 3.x and `tauri/test` are both compatible.
```

---
*Generated by Agentex*
