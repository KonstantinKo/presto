# Implementation Plan for #4

**Issue:** Add a unit test suite (Vitest + Cargo tests)
**Type:** chore
**Branch:** agentex/4-add-unit-test-suite

---

Now I have enough context to draft the plan. The issue itself says only Phase 1 should land in this PR (the `wontfix` label is intentionally there to prevent the workflow from trying to do it all in one shot), so the plan targets Phase 1 only.

Key constraints I noticed during research:

- `src/core/pomodoro-timer.js` does `const { invoke } = window.__TAURI__.core;` at **module load** — tests must shim `window.__TAURI__` before import.
- `src/utils/common-utils.js`, `tag-statistics.js`, `pomodoro-timer.js` all import `logger.js`, which imports `@tauri-apps/plugin-log`. That plugin calls into the Rust runtime — must be mocked.
- Vitest needs a DOM (utilities call `document`, `navigator`, `requestAnimationFrame`). `happy-dom` is the lightest fit.
- On the Rust side, `should_debounce_shortcut` uses a `LazyLock<Mutex<HashMap>>` global. Testable as-is, but cleaner to extract a pure `is_debounced(map, action, now, duration)` helper.
- `.agentex.yml` already has `cargo test` in `qa.test`, but `npm test` is not present — must be added.
- ESLint ignores need updating so test files don't fail lint (test globals like `describe`, `it`, `expect` aren't in browser globals).
- `tsconfig.json` `include` is `src/**/*.js` — tests outside `src/` won't be type-checked, which is desired.

```md
# Chore: Add Phase 1 unit test suite (Vitest + Cargo tests)

## Chore Description

Presto currently has zero behavioral test coverage: `cargo test` passes
vacuously and `npm test` is not wired up at all. PR #1 enforced strict
linting/formatting but added no behavioral safety net, so any refactor (the
logger migration in #5 was a recent example) ships blind.

This chore implements **Phase 1** of issue #4 only. Phase 1 is the "get the
runners running" milestone:

1. Add Vitest as a devDependency and wire `npm test` to run it.
2. Add a Vitest test setup that stubs `window.__TAURI__` and the
   `@tauri-apps/plugin-log` module so files that touch them at import time
   load cleanly.
3. Write sanity tests for pure functions in:
   - `src/utils/common-utils.js` (`TimeUtils`, `KeyboardUtils`)
   - `src/utils/tag-statistics.js` (math/format helpers)
   - `src/core/pomodoro-timer.js` (initial state and `resetTimer` / `adjustTimer`
     state transitions)
4. Extract the pure pieces of `src-tauri/src/lib.rs` (debounce helper, weekly
   goal default, analytics default, `AppSettings::default()`) into a
   `mod helpers` so they can be unit-tested without a Tauri runtime, and add
   `#[cfg(test)] mod tests`.
5. Wire `npm test` into `.agentex.yml`'s `qa.test` section so CI runs both
   suites.
6. Update `eslint.config.js` and `tsconfig.json` so the new `tests/`
   directory does not trip the existing lint/typecheck gates.

**Out of scope (Phase 2/3, separate issues):**
- Manager unit tests (`SessionManager`, `SettingsManager`, `NavigationManager`)
  with mocked Tauri invokes.
- Tauri `MockRuntime`-based integration tests of `#[tauri::command]` functions.
- Playwright/WebdriverIO E2E.

**Acceptance:** ≥10 tests pass total, both `cargo test` and `npm test` exit 0
with non-empty output, and `.agentex.yml`'s test stage runs both. The
`wontfix` label on issue #4 must be removed before this lands, and Phase 2/3
sub-issues must be filed.

## Relevant Files

Use these files to resolve the chore:

- `package.json` — add `vitest` + `happy-dom` to `devDependencies`, add a
  `test` script. Existing scripts (`lint`, `format`, `typecheck`) are the
  template for shape/style.
- `.agentex.yml` — `qa.test` already runs `cargo build --all-targets` and
  `cargo test` from `src-tauri/`; we need to add `npm test` to that list.
- `eslint.config.js` — `ignores` already excludes `node_modules`,
  `src/styles`, `art`, `src/docs`. We need ESLint to either ignore `tests/`
  or recognize Vitest globals (`describe`, `it`, `expect`, `vi`, `beforeEach`,
  etc.) so test files don't fail `npx eslint src` (note: `eslint src` only
  lints `src/`, so this only matters if tests live under `src/`; we'll keep
  them under `tests/` to side-step it).
- `tsconfig.json` — `include` is `src/**/*.js`; tests under `tests/` are
  automatically excluded, which is what we want for Phase 1.
- `src/utils/common-utils.js` — exports `TimeUtils`, `KeyboardUtils`,
  `NotificationUtils`, `StorageUtils`, `DOMUtils`. Phase 1 covers
  `TimeUtils` (`formatTime`, `formatTimeDetailed`, `getWeekStart`,
  `isSameDay`, `formatDateRange`) and `KeyboardUtils` (`parseShortcut`,
  `matchesShortcut`). Imports `logger` — must mock `@tauri-apps/plugin-log`.
- `src/utils/tag-statistics.js` — Phase 1 covers `formatDuration`,
  `generatePieChartGradient`, and `getTagUsageStatistics` (the math: filtering
  by date, splitting duration across multi-tag sessions, "untagged" bucket,
  percentage rollup). Also imports `logger`.
- `src/utils/logger.js` — module under test indirectly; the `@tauri-apps/plugin-log`
  import is what we mock at the Vitest level.
- `src/core/pomodoro-timer.js` — line 2 does
  `const { invoke } = window.__TAURI__.core;` at module load. Tests must
  shim `window.__TAURI__` in `beforeAll` (or via the Vitest setup file)
  before importing this module. Phase 1 covers: constructor produces sane
  initial state (mode `focus`, `25*60` time remaining, `isRunning=false`),
  `resetTimer` clears running state and restores `durations[currentMode]`,
  `adjustTimer(+/-N)` changes `timeRemaining` and clamps at 0. The
  constructor reaches into the DOM (`getElementById`) — those calls must
  return `null` cleanly under happy-dom (they already do; the methods we
  test gate on `this.taskInput`, `this.timerMinutes`, etc.).
- `src-tauri/src/lib.rs` — extract the pure helpers below into a new
  module `src-tauri/src/helpers.rs`, expose them with `pub(crate)`, and
  re-route the existing call sites:
  - `should_debounce_shortcut` (lines ~191–204) → split into a pure
    `pub(crate) fn is_debounced(map: &mut HashMap<String, Instant>, action: &str, now: Instant, window: Duration) -> bool` and keep the global-state wrapper at the call site, OR keep the wrapper too and test the wrapper using a fresh `HashMap` (cleaner: pure variant).
  - `default_weekly_goal` (line 119), `default_analytics_enabled` (line 123).
  - `AppSettings::default()` impl (lines 157–188) — assert specific fields
    (`timer.focus_duration == 25`, `analytics_enabled == true`, etc.).
- `src-tauri/Cargo.toml` — should not need changes; `cargo test` already
  works against the existing lib target. If we want the helpers module to
  carry its own tests, just add `#[cfg(test)] mod tests { ... }` inside it.

### New Files

- `vitest.config.js` — configures `environment: "happy-dom"`, `setupFiles:
  ["./tests/setup.js"]`, and `globals: true` (so `describe`/`it`/`expect`
  are available without imports).
- `tests/setup.js` — runs before every test file. Stubs
  `globalThis.window.__TAURI__` with `core: { invoke: vi.fn() }`,
  `notification: {...}` etc., and `vi.mock("@tauri-apps/plugin-log", ...)`
  to no-op `debug`/`info`/`warn`/`error` so `logger.js` doesn't try to talk
  to the Rust runtime.
- `tests/utils/common-utils.test.js` — `TimeUtils` and `KeyboardUtils`
  cases.
- `tests/utils/tag-statistics.test.js` — `formatDuration`,
  `generatePieChartGradient`, `getTagUsageStatistics` cases.
- `tests/core/pomodoro-timer.test.js` — initial state, `resetTimer`,
  `adjustTimer`. Constructor calls `init()` which is async and calls
  `loadSessionData()` → Tauri `invoke`; the Tauri mock returns
  `null`/`undefined` and the manager handles that gracefully (existing code
  has fallbacks for missing data). We avoid awaiting `init()` and just
  poke at synchronous state.
- `src-tauri/src/helpers.rs` — extracted pure helpers + `#[cfg(test)] mod tests`.

## Step by Step Tasks

IMPORTANT: Execute every step in order, top to bottom.

### 1. Add Vitest tooling

- In `package.json`:
  - Add `"vitest": "^2"` and `"happy-dom": "^15"` to `devDependencies`
    (use the latest stable matching the existing devDeps' major-pinning
    style — `^` ranges).
  - Add `"test": "vitest run"` to `scripts` (use `run` mode for CI; users
    can still call `npx vitest` for watch mode locally).
- Run `npm install` to update `package-lock.json`. Commit the lockfile.
- Create `vitest.config.js` at the repo root:

  ```js
  import { defineConfig } from "vitest/config";

  export default defineConfig({
    test: {
      environment: "happy-dom",
      globals: true,
      setupFiles: ["./tests/setup.js"],
      include: ["tests/**/*.test.js"],
    },
  });
  ```

### 2. Create the Vitest setup file

- Create `tests/setup.js`:
  - `vi.mock("@tauri-apps/plugin-log", () => ({ debug: vi.fn(), info: vi.fn(), warn: vi.fn(), error: vi.fn() }))` — must be at module top so it hoists.
  - In `beforeAll`, attach to `globalThis.window`: `__TAURI__ = { core: { invoke: vi.fn() }, notification: { isPermissionGranted: vi.fn(async () => true), requestPermission: vi.fn(async () => "granted"), sendNotification: vi.fn() } }`.
  - This file runs before each test file but `vi.mock` must be at top-level so the hoist works for module-level imports.
- Verify by running `npx vitest run` — at this point it will report "no test files found", which is fine.

### 3. Add tests for `TimeUtils` and `KeyboardUtils`

- Create `tests/utils/common-utils.test.js` and import `{ TimeUtils, KeyboardUtils } from "../../src/utils/common-utils.js"`.
- `TimeUtils.formatTime`:
  - returns `"0m"` for `0`, negative, and `null`.
  - returns `"45s"` for `45`.
  - returns `"5m"` for `300`.
  - returns `"1h 30m"` for `5400`, `"2h"` for `7200`.
- `TimeUtils.formatTimeDetailed`:
  - returns `"0h 0m"` for `0`/`null`.
  - returns `"1h 30m"` for `5400`.
- `TimeUtils.getWeekStart`: pass a known Wednesday (e.g. `new Date("2026-05-06T12:00:00Z")` is a Wednesday), expect Monday of that week. Cover the Sunday edge case where `day === 0` triggers the `-6` branch.
- `TimeUtils.isSameDay`: same-day true, different-day false.
- `KeyboardUtils.parseShortcut`:
  - returns `null` for falsy input.
  - parses `"CommandOrControl+Alt+Space"` → `{ meta: true, ctrl: true, alt: true, shift: false, key: " " }`.
  - parses `"Shift+R"` → `{ shift: true, key: "r" }`.
- `KeyboardUtils.matchesShortcut`:
  - matches a `Space` keydown when `shortcutString === "CommandOrControl+Alt+Space"` and the event has `metaKey: true, altKey: true, code: "Space"`.
  - does NOT match if a required modifier is missing.

### 4. Add tests for `TagStatistics`

- Create `tests/utils/tag-statistics.test.js` and instantiate `new TagStatistics()`.
- `formatDuration`:
  - `30` → `"30s"`.
  - `90` → `"1m"` (Math.floor(90/60) = 1).
  - `3600` → `"1h"`.
  - `5400` → `"1h 30m"`.
- `generatePieChartGradient`:
  - empty array returns `"conic-gradient(#e5e7eb 0deg 360deg)"`.
  - single-stat 100% returns a gradient with `0deg 360deg`.
- `getTagUsageStatistics`:
  - Two tagged sessions (each `duration: 30`, one tag each) within range
    produce two stats summing to ~100% with totalDuration `60*60` seconds
    (note: code multiplies by 60 to convert minutes to seconds).
  - Untagged session falls into the synthetic `"untagged"` bucket.
  - Multi-tag session splits duration evenly (e.g. 2 tags, duration 30 →
    each gets 15 minutes).
  - Date-range filter excludes sessions outside `[startDate, endDate]`.

### 5. Add state-transition tests for `PomodoroTimer`

- Create `tests/core/pomodoro-timer.test.js`.
- The setup (`tests/setup.js`) already provides `window.__TAURI__.core.invoke`
  as a `vi.fn()` returning `undefined` — that lets the module-load
  `const { invoke } = window.__TAURI__.core;` succeed.
- `import { PomodoroTimer } from "../../src/core/pomodoro-timer.js";`.
- For each test, `beforeEach`:
  - `document.body.innerHTML = "";` (happy-dom resets between files but not necessarily between tests).
  - Construct `const timer = new PomodoroTimer();`. The constructor calls
    `init()` which is async and fires off `loadSessionData()` via `invoke`;
    we don't await it, we just synchronously poke at state. (`vi.mock`'d
    `invoke` resolves to `undefined`, and the existing fallbacks handle a
    missing return.)
- Assertions:
  - `timer.currentMode === "focus"`, `timer.timeRemaining === 25 * 60`,
    `timer.isRunning === false`, `timer.isPaused === false`,
    `timer.completedPomodoros === 0`.
  - `timer.adjustTimer(5)` → `timeRemaining === 25 * 60 + 300`.
  - `timer.adjustTimer(-1000)` → `timeRemaining === 0` (clamped).
  - `timer.resetTimer()` after `timer.timeRemaining = 60` →
    `timeRemaining === durations.focus` (1500), `isRunning === false`,
    `sessionStartTime === null`.
- After each test, call `timer.stopMidnightMonitoring()` to clear the
  `setInterval` so Vitest doesn't hang.

### 6. Extract pure helpers from `lib.rs` into `helpers.rs`

- Create `src-tauri/src/helpers.rs`:

  ```rust
  use std::collections::HashMap;
  use std::time::{Duration, Instant};

  pub(crate) fn is_debounced(
      map: &mut HashMap<String, Instant>,
      action: &str,
      now: Instant,
      window: Duration,
  ) -> bool {
      if let Some(last) = map.get(action) {
          if now.duration_since(*last) < window {
              return true;
          }
      }
      map.insert(action.to_string(), now);
      false
  }
  ```
- Add `mod helpers;` near the top of `lib.rs` (below the existing `use`
  statements).
- Replace the body of `should_debounce_shortcut` with:

  ```rust
  fn should_debounce_shortcut(action: &str) -> bool {
      let mut map = SHORTCUT_DEBOUNCE.lock().unwrap();
      helpers::is_debounced(&mut map, action, Instant::now(), Duration::from_millis(500))
  }
  ```

  This keeps the public behavior identical while making the core logic
  testable in isolation.
- Add `#[cfg(test)] mod tests` to `helpers.rs`:
  - First call returns `false` and inserts the entry.
  - Immediate second call with same action returns `true` (debounced).
  - Second call with `now` advanced past `window` returns `false`.
  - Different actions don't interfere with each other.

### 7. Add tests for `AppSettings` defaults in `lib.rs`

- At the very bottom of `src-tauri/src/lib.rs`, append:

  ```rust
  #[cfg(test)]
  mod tests {
      use super::{default_analytics_enabled, default_weekly_goal, AppSettings};

      #[test]
      fn weekly_goal_default_is_125() {
          assert_eq!(default_weekly_goal(), 125);
      }

      #[test]
      fn analytics_enabled_default_is_true() {
          assert!(default_analytics_enabled());
      }

      #[test]
      fn app_settings_default_has_expected_durations() {
          let s = AppSettings::default();
          assert_eq!(s.timer.focus_duration, 25);
          assert_eq!(s.timer.break_duration, 5);
          assert_eq!(s.timer.long_break_duration, 20);
          assert_eq!(s.timer.total_sessions, 10);
          assert_eq!(s.timer.weekly_goal_minutes, 125);
          assert!(s.analytics_enabled);
          assert!(!s.autostart);
          assert!(!s.hide_icon_on_close);
          assert!(!s.hide_status_bar);
          assert!(s.notifications.desktop_notifications);
          assert!(s.notifications.sound_notifications);
          assert!(!s.notifications.smart_pause);
          assert_eq!(s.notifications.smart_pause_timeout, 30);
      }
  }
  ```

- Note: `default_weekly_goal` and `default_analytics_enabled` are private
  module fns — they're already accessible to a sibling `tests` module via
  `use super::*`. No visibility changes needed.

### 8. Wire `npm test` into `.agentex.yml`

- In `.agentex.yml`, append `npm test` to the `qa.test` array so CI runs
  both suites:

  ```yaml
  qa:
    setup:
      - npm ci
    test:
      - cd src-tauri && cargo build --all-targets
      - cd src-tauri && cargo test
      - npm test
  ```

### 9. Sanity-check ESLint and Prettier coverage

- ESLint: `npx eslint src` — `eslint src` is scoped to `src/`, so
  `tests/**` is naturally out of scope. No `eslint.config.js` change needed
  for Phase 1. (Optional polish: add a Vitest globals block scoped to
  `tests/**` so future contributors who run `npx eslint tests` don't
  trip; defer to Phase 2.)
- Prettier: `npx prettier --check .` covers everything. Our new files must
  be Prettier-clean — run `npx prettier --write tests vitest.config.js`
  before committing.

### 10. Run the full validation suite (see "Validation Commands" below)

Confirm all gates green before opening the PR.

### 11. Issue hygiene

- After this PR is up, the `wontfix` label on issue #4 should be removed
  (per the issue's own instructions) and Phase 2 / Phase 3 sub-issues
  filed. This is a manual GitHub action and not part of the code change,
  but the PR description should call it out so the reviewer doesn't
  forget.

## Validation Commands

Execute every command to validate the chore is complete with zero regressions.

- `npm install` — verifies the new devDeps resolve cleanly.
- `npm test` — must exit 0 with **non-empty output** (i.e., Vitest reports the
  test files it ran). Expect ≥7 JS tests passing.
- `cd src-tauri && cargo test` — must exit 0 with non-empty output. Expect
  ≥7 Rust tests passing (4 in `helpers::tests`, 3 in `lib::tests`).
- Combined JS + Rust ≥10 tests passing — meets the issue's acceptance bar.
- `cd src-tauri && cargo build --all-targets` — confirms the helpers
  refactor compiles in test config too.
- `cd src-tauri && cargo clippy --all-targets -- -D warnings` — the new
  `helpers.rs` and the inline `mod tests` must satisfy the project's
  pedantic+nursery clippy gate. Particular care: avoid
  `clippy::missing_panics_doc` on test fns (tests are exempt by default,
  but `unwrap_or` patterns should be reviewed).
- `cd src-tauri && cargo fmt -- --check` — formatting clean.
- `npx eslint src` — must remain green; we didn't change anything `src/`
  is sensitive to, but confirm.
- `npx tsc --noEmit -p .` — must remain green; `tsconfig.json` `include`
  is `src/**/*.js`, so test files don't affect typecheck.
- `npx prettier --check .` — Prettier covers the whole repo by default,
  so the new test files and `vitest.config.js` must be formatted.

If any of these fail, fix root-causes (do not bypass with `--no-verify`,
`-A clippy::all`, or similar) before merging.

## Notes

- **Why happy-dom over jsdom?** Happy-dom is faster and lighter. The tests
  only need basic DOM (`document.createElement`, `getElementById`,
  `addEventListener`); both work, but happy-dom is the standard pairing
  with Vitest in 2025-era projects.
- **Why mock `@tauri-apps/plugin-log` instead of `logger.js`?** Mocking at
  the lowest layer (`plugin-log`) means we don't need a separate mock
  per-file and we exercise the real `logger.js` formatting code in tests.
- **Why a Vitest setup file instead of per-test mocks for `window.__TAURI__`?**
  `pomodoro-timer.js` destructures `window.__TAURI__.core` at the top of
  the module — if the global isn't there *before* the import is evaluated,
  the import throws. A setup file ensures it's there for every test.
- **Why extract `is_debounced` as pure?** The current
  `should_debounce_shortcut` mutates a `LazyLock<Mutex<HashMap>>` global,
  which makes assertions order-dependent across tests. A pure function with
  caller-owned state is trivially testable and idiomatic Rust.
- **Phase 2 scope (separate PR, separate sub-issue):** Manager unit tests
  with a richer Tauri invoke mock (success + error paths for
  `SessionManager.loadSessionsFromStorage`, `SettingsManager` defaults
  applied to a fresh instance, `NavigationManager` chart-data shaping).
  Phase 1 deliberately avoids these because the mock surface area required
  is non-trivial.
- **`pomodoro-timer.js` constructor side effects:** The constructor calls
  `init()` which kicks off async work (`loadSessionData`, `loadTasks`,
  `setupEventListeners`) and starts a midnight-monitoring `setInterval`.
  Tests must call `timer.stopMidnightMonitoring()` (and `clearInterval` on
  any timer interval if `isRunning`) in a teardown hook or Vitest will hang
  on completion. Use `afterEach` for safety.
- **Do not amend or split**: keep this as a single PR. The issue body says
  Phase 1 ships in 1 PR; subsequent phases get their own sub-issues.
```

---
*Generated by Agentex*
