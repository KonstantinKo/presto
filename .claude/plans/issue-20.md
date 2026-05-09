# Implementation Plan for #20

**Issue:** Full-coverage E2E test suite — all screens + major flows
**Type:** feature
**Branch:** agentex/20-e2e-full-coverage-tests

---

I have enough context. Producing the complete plan now.

````md
# Feature: Full-coverage E2E test suite — all screens + major flows

## Feature Description

Land a Playwright-driven, browser-level E2E test suite under `tests/e2e/` that exercises every screen and major user flow in Presto by clicking and typing exactly like a real user. The suite is deliberately decoupled from the current vanilla-JS implementation so it survives the upcoming frontend stack swap (likely Leptos + WASM + Trunk): it asserts only on visible UI state, never on internal module structure or module imports, and isolates the unavoidable Tauri-bridge mocking to a single fixture annotated with `// TODO(stack-swap):` markers.

The suite complements but does not duplicate existing test layers:

- Phase 1 (#4) — Vitest pure unit tests for helpers/utilities.
- Phase 2 (#8) — Vitest manager unit tests with mocked `window.__TAURI__`.
- Phase 3 (#9) — Cargo MockRuntime IPC integration tests.
- **Phase 4 (this feature)** — Browser-level E2E tests driven by Playwright against the real `src/index.html`.

Concretely it adds: 15 multi-step spec files, 1 ground-rules doc (`tests/e2e/CLAUDE.md`), a Playwright config wired to a static-server-served `src/`, an automatic `_blockExternal` fixture that blocks all non-loopback HTTP, a Tauri-bridge mock fixture installed before any page script runs, an `npm run test:e2e` script, and a CI job that gates PR merges.

## User Story

As a maintainer of Presto preparing for a frontend stack swap (vanilla JS → likely Leptos/WASM)
I want a full-coverage E2E test suite that drives the app exactly like a real user would (UI clicks, no programmatic state injection, no JS evaluation mid-flow)
So that I can refactor or rewrite the entire frontend with confidence that user-visible behavior is preserved, because the same suite will continue to validate every screen and major flow on the new stack with no test changes beyond — at most — selector adjustments.

## Problem Statement

Today, all of Presto's automated tests run inside the JS module graph (Vitest + happy-dom) or inside Cargo's Rust runtime. Nothing exercises the actual rendered HTML, the actual click handlers, the actual navigation between screens, or the actual interplay between manager singletons that are wired together at `main.js` boot. Specifically:

- Manager unit tests construct managers in isolation against a stub DOM. They cannot catch regressions where, say, the calendar nav handler fails to refresh the focus-summary card after a session is saved on the timer view, because no test actually clicks the calendar tab after running a timer.
- The test pyramid has a gaping hole at the "user clicks a button on screen A, navigates to screen B, sees the result" level — exactly the level a frontend stack swap is most likely to break.
- Without browser-level tests, the only way to validate the swap is manual smoke testing on every screen — laborious and error-prone, and the swap PR will be huge.

There's a secondary risk: tests written naively (mocking specific module paths, asserting on JS class names, using `page.evaluate` to manipulate state) will die on the swap, leaving the repo with a debt of obsolete tests masquerading as coverage. The suite's design must aggressively avoid that.

## Solution Statement

Add a Playwright suite under `tests/e2e/` driven against a static HTTP-served copy of `src/` (Tauri's `frontendDist`). The suite uses three small but load-bearing fixtures:

1. **`_blockExternal`** (auto fixture): registers a `page.route('**/*', …)` handler that aborts every request whose host is not `localhost`/`127.0.0.1`. Wired as a Playwright `auto` fixture so every test inherits it without opt-in. This blocks Supabase, jsdelivr (Supabase UMD CDN), Google Fonts, GitHub releases (updater), and any analytics — even ones nobody anticipated yet.

2. **`tauriMock`** (auto fixture): uses `page.addInitScript()` to install a stub `window.__TAURI__` object before any page script runs, faking `core.invoke`, `dialog`, `event`, `notification`, and `updater`. The mock owns an in-memory store for tags / tasks / settings / sessions / history that persists across navigations (but not across `page.goto` reloads to a fresh context). Annotated `// TODO(stack-swap):` because the `window.__TAURI__` shape will not survive the swap — but the public UI behaviors driven through it will.

3. **Per-spec helpers** (in `tests/e2e/fixtures/`): tiny page-object-style helpers like `gotoTimer(page)`, `enableDebugTimers(page)` (UI-clicks through Settings → Advanced → Debug Mode), `runTimerToCompletion(page)`, etc. These keep specs short and intent-revealing, and centralize the few flows that depend on debug-mode 3-second timers.

The dev server is **Vite** with a 3-line `vite.config.js` rooted at `src/`. Vite is added because it correctly serves ESM (`<script type="module">`) with the right MIME types and integrates cleanly with Playwright's `webServer` block. Tauri's webview is **not** used because spawning `tauri dev` requires `tauri-driver` + WebKit2GTK + extra OS packages in CI, with no behavioral upside given that all backend calls are mocked anyway. The choice is documented at the top of `tests/e2e/CLAUDE.md`.

CI gets a third `e2e` job that runs `npm ci`, installs Playwright browsers (`npx playwright install --with-deps chromium`), and runs `npm run test:e2e`. Failures block PR merge.

Stack-swap survivability is the design constraint that drives every other choice. Selectors are resolved by ARIA role + accessible name where possible, falling back to stable `id` / `data-*` attributes that exist in `src/index.html` today and are reasonable to preserve in any reasonable Leptos rewrite. No selector targets a CSS class that is purely stylistic; no selector targets a JS-internal data attribute set at runtime by a manager.

## Relevant Files

Use these files to understand patterns and to extend during implementation:

- `src/index.html` — single-page HTML containing **all** screens (timer, calendar, team, settings + sub-categories) toggled via `view-container` + `hidden`. The selectors the suite drives (`#timer-status`, `#play-pause-btn`, `#stop-btn`, `#timer-minutes`, `#calendar-nav`, `#settings-nav`, `#prev-week`, `#focus-duration`, `#theme-selector`, etc.) are defined here. Read this first when authoring any spec.
- `src/main.js` — boot sequence; constructs `NavigationManager`, `SettingsManager`, `SessionManager`, `TeamManager`, `PomodoroTimer`, `AuthManager`. Useful to confirm initialization order — the page is fully usable only after the `DOMContentLoaded` chain finishes.
- `src/core/pomodoro-timer.js` — timer state machine; ticks every second; methods `startTimer`, `pauseTimer`, `resetTimer`, `skipSession`. Click handlers wired in constructor.
- `src/managers/navigation-manager.js` — view switching via `data-view`; calendar/week/month nav; lazy XLSX export; populates focus summary cards.
- `src/managers/session-manager.js` — manual session CRUD via Tauri commands (`load_manual_sessions`, `save_manual_sessions`); modal at `#session-modal-overlay`.
- `src/managers/settings-manager.js` — settings load/save; shortcut recording; theme switching; auto-save with 1s debounce.
- `src/managers/tag-manager.js` — tag list + create/delete via Tauri commands (`load_tags`, `save_tag`, `delete_tag`); falls back to localStorage when `window.__TAURI__` is absent. Note: no in-UI "edit tag" affordance today — only create + delete.
- `src/managers/team-manager.js` — synthesizes mock team members in `initializeDemoData()`; entirely client-side. **`#team-nav` button is `disabled` in `src/index.html` today**; the spec must enable it via fixture.
- `src/managers/auth-manager.js` — Supabase session bootstrap. The tests mock `window.supabase` (or block the CDN entirely and rely on `presto-guest-mode` localStorage flag for guest mode).
- `src/managers/update-manager-global.js` — assigns `window.UpdateManagerV2`; emits `update-available` events on its `eventTarget`. The `update-notification` spec mocks the `tauri::updater` plugin invoke.
- `src/components/update-notification.js` — listens on `window.updateManager.eventTarget`; renders the banner. Drives `update-notification.spec.js`.
- `src-tauri/tauri.conf.json` — `frontendDist: "../src"` confirms src/ can be served as-is.
- `package.json` — needs `test:e2e` script + `@playwright/test` + `vite` dev-dependencies.
- `.github/workflows/ci.yml` — needs a new `e2e` job.
- `.gitignore` — needs to ignore `tests/e2e/playwright-report/`, `tests/e2e/test-results/`, and Playwright's browser cache.
- `tests/setup/tauri-mock.js` — Vitest-side Tauri mock; reuse the _shape_ (handler dispatch by command name) but **not** the implementation; the E2E variant runs in the page context via `addInitScript`, not in node.

### New Files

- `tests/e2e/CLAUDE.md` — verbatim the four ground-rule blocks from the issue, plus a single short paragraph explaining the dev-server choice (Vite over `tauri dev`).
- `playwright.config.js` (project root) — Playwright config; defines projects, `webServer`, `use`, `testDir: "./tests/e2e"`, `reporter`.
- `vite.config.js` (project root) — minimal `defineConfig({ root: "src", server: { port: 1420 }, preview: { port: 1420 } })`.
- `tests/e2e/fixtures/index.js` — exports `test` (a `playwright.test` extended with `_blockExternal`, `tauriMock`, and `pageWithMocks` fixtures).
- `tests/e2e/fixtures/blockExternal.js` — implements the network-block fixture.
- `tests/e2e/fixtures/tauriMock.js` — implements the `window.__TAURI__` page init script as a string + the in-memory state model. Annotated `// TODO(stack-swap):`.
- `tests/e2e/fixtures/screens.js` — small page-object helpers (`gotoTimer`, `openSettings`, `selectSettingsCategory(name)`, `enableDebugTimers`, `runTimerToCompletion`, `tapTab`, `findSessionRowByTag`, etc.).
- `tests/e2e/timer.spec.js`
- `tests/e2e/tags.spec.js`
- `tests/e2e/sessions-history.spec.js`
- `tests/e2e/calendar-navigation.spec.js`
- `tests/e2e/settings-general.spec.js`
- `tests/e2e/settings-shortcuts.spec.js`
- `tests/e2e/settings-notifications.spec.js`
- `tests/e2e/settings-theme.spec.js`
- `tests/e2e/settings-automation.spec.js`
- `tests/e2e/settings-goals.spec.js`
- `tests/e2e/settings-advanced.spec.js`
- `tests/e2e/settings-updates.spec.js`
- `tests/e2e/auth.spec.js`
- `tests/e2e/team.spec.js`
- `tests/e2e/update-notification.spec.js`

## Implementation Plan

### Phase 1: Foundation

Wire up Vite + Playwright + the two automatic fixtures (`_blockExternal`, `tauriMock`), write `tests/e2e/CLAUDE.md` with the verbatim ground rules, and stand up an empty smoke spec to prove the runner boots, the dev server starts, the page loads, and the Tauri stub satisfies app initialization without console errors. No real coverage yet — just the rig.

### Phase 2: Core Implementation

Author the 15 spec files, each as a multi-step user journey per ground rule #3. Bundle related assertions; never use `waitForTimeout`; never use `page.evaluate` mid-flow; never use `goto` after the initial entry. Prefer ARIA roles + accessible text for selectors (`getByRole('button', { name: 'Start' })`, `getByText('Focus')`); fall back to stable IDs that exist in `src/index.html` for elements without an accessible name. Where the test needs to fire a Tauri-only bridge call (notifications, updater), extend the mock — never bypass it.

### Phase 3: Integration

Add the CI `e2e` job, ensure failures block merge, run the full local validation matrix (`npm run typecheck`, `npm run lint`, `npm test`, `cargo test`, `npm run test:e2e`), and verify the suite is green on a clean checkout. Update the existing test-strategy doc if one exists; otherwise leave the `tests/e2e/CLAUDE.md` as the canonical reference.

## Step by Step Tasks

IMPORTANT: Execute every step in order, top to bottom.

### Step 1: Add dev dependencies and scripts

- Edit `package.json`. Add to `devDependencies`: `"@playwright/test": "^1.49.0"`, `"vite": "^5.4.0"`. Pin majors to lockfile-resolved versions after `npm install`.
- Add to `scripts`:
  - `"test:e2e": "playwright test"`
  - `"test:e2e:ui": "playwright test --ui"`
  - `"e2e:install": "playwright install --with-deps chromium"`
- Update `validate` to also run e2e: `"validate": "npm run typecheck && npm run lint && npm test && npm run test:e2e"`.
- Run `npm install` to lock versions.

### Step 2: Add `vite.config.js`

- Create `vite.config.js` at the repo root:
  ```js
  import { defineConfig } from "vite";
  export default defineConfig({
    root: "src",
    server: { port: 1420, strictPort: true, host: "127.0.0.1" },
    preview: { port: 1420, strictPort: true, host: "127.0.0.1" },
    publicDir: false,
  });
  ```
````

- Verify by running `npx vite --port 1420 --host 127.0.0.1` and curl-ing `http://127.0.0.1:1420/index.html` in another terminal — must return 200 and contain `<title>Presto</title>`.
- Quit the server.

### Step 3: Add `playwright.config.js`

- Create `playwright.config.js` at the repo root:
  ```js
  import { defineConfig, devices } from "@playwright/test";
  export default defineConfig({
    testDir: "./tests/e2e",
    fullyParallel: true,
    forbidOnly: !!process.env.CI,
    retries: process.env.CI ? 2 : 0,
    workers: process.env.CI ? 2 : undefined,
    reporter: process.env.CI ? [["github"], ["html", { open: "never" }]] : "list",
    use: {
      baseURL: "http://127.0.0.1:1420",
      trace: "retain-on-failure",
      video: "retain-on-failure",
      screenshot: "only-on-failure",
      actionTimeout: 5000,
      navigationTimeout: 15000,
    },
    webServer: {
      command: "npx vite --port 1420 --host 127.0.0.1",
      url: "http://127.0.0.1:1420",
      reuseExistingServer: !process.env.CI,
      timeout: 30_000,
    },
    projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  });
  ```
- Note: only Chromium for now; cross-browser coverage is out of scope.

### Step 4: Update `.gitignore` and create `tests/e2e/`

- Append to `.gitignore`:
  ```
  /playwright-report/
  /test-results/
  /tests/e2e/playwright-report/
  /tests/e2e/test-results/
  ```
- Create the directory `tests/e2e/` and the subdirectory `tests/e2e/fixtures/`.

### Step 5: Write `tests/e2e/CLAUDE.md`

- Create the file with the verbatim ground rules from the issue (rules 1–3, including all sub-bullets), then append a one-paragraph **"Dev-server choice"** section explaining: (a) Vite is used to serve `src/` as a static HTTP server because `src/index.html` uses ESM `type="module"` imports, (b) `tauri dev` was rejected for E2E because it would require `tauri-driver` + WebKit2GTK + extra OS deps in CI for no behavioral upside given that all Tauri commands are mocked, (c) the suite is intentionally decoupled from the current frontend so it survives the planned Leptos/WASM stack swap; only `tests/e2e/fixtures/tauriMock.js` is implementation-coupled and is annotated accordingly.

### Step 6: Implement `tests/e2e/fixtures/blockExternal.js`

- Export a Playwright fixture that, on `page` setup, registers a route handler:
  ```js
  // tests/e2e/fixtures/blockExternal.js
  export async function applyBlockExternal(page) {
    await page.route("**/*", (route) => {
      const url = new URL(route.request().url());
      const isLoopback = url.hostname === "127.0.0.1" || url.hostname === "localhost";
      const isData = url.protocol === "data:" || url.protocol === "blob:";
      if (isLoopback || isData) return route.continue();
      return route.abort();
    });
  }
  ```
- The fixture is wired as `auto: true` in `fixtures/index.js` so every test inherits it. Tests must not unregister it.

### Step 7: Implement `tests/e2e/fixtures/tauriMock.js`

- Implement an `addInitScript` payload that runs **before** any of the page's bundled scripts. Its job is to install `window.__TAURI__` with:
  - `core.invoke(cmd, args)` — switch on `cmd` against an in-memory `state` object: `tags`, `tasks`, `manualSessions`, `sessionTags`, `settings`, `pomodoroSession`, `history`, `autostart`. Implements at minimum: `load_tags`, `save_tag`, `delete_tag`, `load_tasks`, `save_tasks`, `load_settings`, `save_settings`, `load_manual_sessions`, `save_manual_sessions`, `delete_manual_session`, `register_global_shortcuts`, `is_autostart_enabled`, `enable_autostart`, `disable_autostart`, `load_session_data`, `save_session_data`, `append_daily_stats`, `load_history`, `delete_all_data`, `write_excel_file`, `plugin:updater|check`, `plugin:app|version`. Unknown commands `console.warn` and reject with `Unmocked command: <cmd>` so a missing handler surfaces as a visible test failure rather than silent passing.
  - `dialog.{save, open, message}` — return a configurable canned value (default `null`). A test can override per-test via `tauriMock.setDialog(...)`.
  - `event.{listen, emit}` — minimal pub/sub backed by a shared `EventTarget`.
  - `notification.{isPermissionGranted, requestPermission, sendNotification}` — `requestPermission` returns whatever `state.notificationPermission` is set to.
  - `app.getVersion()` — returns the version configured by the test (default `"0.4.4"`).
  - Optional `updater` namespace mirroring `core.invoke('plugin:updater|check')` so `update-manager-global.js`'s `getTauriUpdaterAPI()` resolves.
- The script also exposes `window.__E2E_TEST_HARNESS__` with helpers like `seedTags(list)`, `seedSessions(map)`, `setUpdateAvailable(opts)`, `setNotificationPermission(state)`, etc., so a test fixture (not the test body) can configure state at goto time. **Mid-flow `page.evaluate` calls into this harness are forbidden by ground rule #1.2** — only the fixture (executed before navigation) may use it.
- Top of file: `// TODO(stack-swap): this fixture mocks the Tauri JS bridge by setting window.__TAURI__. After the Leptos/WASM swap, the bridge shape will change (or be replaced by a Trunk-served WASM binary that talks to Tauri via a different IPC). Re-implement this fixture against whatever the new bridge boundary is. The spec files do not depend on this file's internals; they depend only on user-visible UI and on the high-level seedX/setX configuration helpers exposed via window.__E2E_TEST_HARNESS__.`
- Export `applyTauriMock(page, options)` that calls `page.addInitScript(...)` once and stores the seed config on the test for later access.

### Step 8: Implement `tests/e2e/fixtures/index.js`

- Compose the two fixtures into a Playwright `test`:
  ```js
  import { test as base, expect } from "@playwright/test";
  import { applyBlockExternal } from "./blockExternal.js";
  import { applyTauriMock } from "./tauriMock.js";
  export const test = base.extend({
    _blockExternal: [
      async ({ page }, use) => {
        await applyBlockExternal(page);
        await use();
      },
      { auto: true },
    ],
    tauriMock: [
      async ({ page }, use) => {
        const harness = await applyTauriMock(page);
        await use(harness);
      },
      { auto: true },
    ],
  });
  export { expect };
  ```
- Re-export `screens.js` helpers (next step).

### Step 9: Implement `tests/e2e/fixtures/screens.js`

- Helpers (each composed of UI clicks, no `goto` after initial):
  - `gotoTimer(page)` — `await page.goto("/index.html")` then wait for `#timer-minutes` to be visible with text `25`.
  - `tapTab(page, name)` — clicks the sidebar icon by `title` attribute (`Timer`, `Calendar`, `Team`, `Settings`).
  - `openSettings(page)` — `tapTab(page, 'Settings')` and waits for `#settings-view` to be visible.
  - `selectSettingsCategory(page, name)` — clicks the matching `.settings-nav-item` by visible text.
  - `enableDebugTimers(page)` — opens settings → advanced → checks `#debug-mode`. Used by `sessions-history.spec.js`.
  - `setNotificationPermission(harness, state)` — fixture-level; sets the mock's permission state before the spec navigates.
  - `enableTeamButton(page)` — uses the harness to remove the `disabled` attribute from `#team-nav` _before initial navigation_ via init-script seeding (not mid-flow). Required because the button is hard-disabled in HTML today; treat this as an environment-condition fixture, akin to "feature flag on", per the "no JS injection" exception clause.

### Step 10: Stand up a smoke spec

- Create `tests/e2e/_smoke.spec.js`:
  ```js
  import { test, expect } from "./fixtures/index.js";
  test("page loads, timer view is default, no console errors", async ({ page }) => {
    const errors = [];
    page.on("pageerror", (e) => errors.push(String(e)));
    page.on("console", (msg) => {
      if (msg.type() === "error") errors.push(msg.text());
    });
    await page.goto("/index.html");
    await expect(page.locator("#timer-minutes")).toHaveText("25");
    await expect(page.locator("#timer-seconds")).toHaveText("00");
    expect(errors, `unexpected console errors: ${errors.join("\n")}`).toEqual([]);
  });
  ```
- Run `npm run e2e:install && npm run test:e2e`. Iterate on the Tauri mock until the smoke spec is green and console-error-free. **Do not proceed to step 11 until smoke passes.**

### Step 11: Author `timer.spec.js`

- One test, multi-step:
  1. `gotoTimer(page)`.
  2. Open the tag dropdown (click `#timer-status`); wait for `#tag-dropdown-menu` to be visible; click the first item in `#tag-list`; assert `#status-text` updates.
  3. Click `#play-pause-btn`; assert `#pause-icon` is visible (timer is now running).
  4. Wait for `#timer-seconds` to change to `59` (i.e. `expect(page.locator('#timer-seconds')).toHaveText('59', { timeout: 2_000 })`). **No `waitForTimeout`.**
  5. Click `#play-pause-btn` again; assert `#play-icon` is visible (paused).
  6. Click `#play-pause-btn` again; assert `#pause-icon` is visible (resumed).
  7. Click `#stop-btn` (which acts as reset/X during focus). Confirm any prompt via UI.
  8. Assert `#timer-minutes` is back to `25` and `#timer-seconds` is `00`.

### Step 12: Author `tags.spec.js`

- One test, multi-step:
  1. `gotoTimer(page)`.
  2. Click `#timer-status` to open the tag dropdown.
  3. Click `#selected-icon-btn` to open the icon picker; click an emoji option (e.g. `🎯`).
  4. Type `Deep Work` into `#new-tag-name`; click `#create-tag-btn`.
  5. Assert the new tag appears in `#tag-list` with name `Deep Work` and the chosen icon.
  6. Click outside to close the dropdown.
  7. Reload via UI navigation: `tapTab(page, 'Settings')` then `tapTab(page, 'Timer')` — re-open the dropdown — assert tag still present (UI-driven persistence check via the in-memory mock store, which is preserved across nav within the same page).
  8. Click the delete icon on the new tag (`.tag-item-delete[data-tag-id]`); confirm any prompt; assert tag removed from `#tag-list`.
- Note in a comment at the top: "There is no in-app 'edit tag' affordance today (verified against `src/index.html` and `src/managers/tag-manager.js`); the issue's edit-step is satisfied by toggling icon selection during creation. If an edit affordance is added, extend this spec." — record this discrepancy here, do not block on a UI feature that doesn't exist.

### Step 13: Author `sessions-history.spec.js`

- One test, multi-step:
  1. `gotoTimer(page)`.
  2. `enableDebugTimers(page)` (Settings → Advanced → check `#debug-mode`, then back to timer via tab).
  3. Pick a tag (open `#timer-status` dropdown → click first tag).
  4. Click `#play-pause-btn`; wait for `#timer-minutes` + `#timer-seconds` to reach `00:00` (i.e. assert with a generous-but-bounded timeout that matches debug 3-second timers — `{ timeout: 10_000 }`).
  5. Wait for the timer to transition to break or for the session to be saved (assert via UI, e.g. progress dot becomes filled, or status changes).
  6. `tapTab(page, 'Calendar')`.
  7. Assert today's date in `#calendar-grid` is highlighted.
  8. Assert at least one row appears in `#sessions-table-body` for today, with the chosen tag and a duration matching the debug timer.
  9. Click the row; assert the session edit modal at `#session-modal-overlay` becomes visible and shows correct duration/start/end values.

### Step 14: Author `calendar-navigation.spec.js`

- One test, multi-step:
  1. `gotoTimer(page)`.
  2. `tapTab(page, 'Calendar')`; capture initial `#week-range` text.
  3. Click `#prev-week`; assert `#week-range` text changes.
  4. Click `#next-week` twice; assert `#week-range` text matches a week ahead of initial.
  5. Click `#prev-month`; assert `#current-month` text changes.
  6. Click `#next-month` twice; assert `#current-month` text matches one month ahead.
  7. Return to current week by clicking `#prev-week` / `#next-week` until `#week-range` matches the captured initial value.

### Step 15: Author `settings-general.spec.js`

- One test, multi-step:
  1. `gotoTimer(page)`; assert `#timer-minutes` is `25`.
  2. `openSettings(page)`; `selectSettingsCategory(page, 'General')`.
  3. Clear `#focus-duration` and type `5`; press Tab to blur (triggers auto-save).
  4. Wait for the auto-save indicator (text "Settings are saved automatically" stays visible) — alternatively wait for the Tauri mock to receive a `save_settings` invocation by listening to a harness event, but ground rule #1.4 prefers a visible UI cue. Use the visible cue.
  5. `tapTab(page, 'Timer')`.
  6. Assert `#timer-minutes` is now `5`.
  7. Click `#play-pause-btn`; wait for `#timer-seconds` to change; click `#stop-btn` to reset; revert by going back into settings and setting `#focus-duration` back to `25`.

### Step 16: Author `settings-shortcuts.spec.js`

- One test, multi-step:
  1. `gotoTimer(page)`; `openSettings(page)`; `selectSettingsCategory(page, 'Shortcuts')`.
  2. Click `#start-stop-shortcut`; assert it has class `recording` (or aria-state) — recording mode active.
  3. Press `Space` (the in-app fallback shortcut handler will pick it up). Note: OS-level global shortcuts (`CommandOrControl+Alt+Space`) cannot be triggered from a browser context; the spec records `Space` and asserts the input value updates.
  4. Click outside to blur; assert the input value is `Space`.
  5. `tapTab(page, 'Timer')`; press `Space` on `body`; assert the timer starts (`#pause-icon` visible).
  6. Press `Space` again; assert paused.
  7. Annotate at top: `// TODO(stack-swap): on the new stack, verify the in-app Space-key fallback is wired equivalently. Global OS shortcuts are tested separately via Cargo (#9 follow-up).`

### Step 17: Author `settings-notifications.spec.js`

- One test, multi-step:
  1. Set the mock's `notificationPermission` to `granted` via the fixture (before navigation).
  2. `gotoTimer(page)`; `openSettings(page)`; `selectSettingsCategory(page, 'Notifications')`.
  3. Toggle `#desktop-notifications` off then on; assert UI state in `#notification-status` updates.
  4. Click `#test-notifications-btn`; assert a toast/notification UI appears (look for a generic notification rendered into the DOM by `NotificationUtils.show*`). The mock's `sendNotification` records the call so a follow-up assertion can verify it was invoked, but the _primary_ assertion is the visible toast.
  5. Toggle `#sound-notifications` off; assert it persists (re-open settings tab).

### Step 18: Author `settings-theme.spec.js`

- One test, multi-step:
  1. `gotoTimer(page)`; `openSettings(page)`; `selectSettingsCategory(page, 'Theme')`.
  2. Capture the `data-theme` (or class state) on `<html>` initially.
  3. Click `[data-theme="light"]` button under `#theme-selector`; assert `<html>` data-attribute / class reflects `light`.
  4. Click `[data-theme="dark"]`; assert dark.
  5. Click `[data-theme="auto"]`; assert auto.
  6. Click a non-default timer-color theme tile in `#timer-theme-grid` (the suite picks the second tile to avoid depending on a specific name); assert `<html>` (or `body`) gains the corresponding theme class.

### Step 19: Author `settings-automation.spec.js`

- One test, multi-step:
  1. `gotoTimer(page)`; `openSettings(page)`; `selectSettingsCategory(page, 'Automation')`.
  2. Toggle `#auto-start-timer`; toggle `#allow-continuous-sessions`; toggle `#smart-pause` (and assert `#smart-pause-timeout-setting` becomes visible); toggle `#auto-save-sessions`; toggle `#prevent-interruptions`. After each toggle, assert it stays toggled after navigating to Timer and back.
  3. With `#auto-start-timer` on, `enableDebugTimers(page)`, run a session to completion on Timer view; assert next session starts automatically (`#pause-icon` visible without user clicking play).

### Step 20: Author `settings-goals.spec.js`

- One test, multi-step:
  1. `gotoTimer(page)`; `openSettings(page)`; `selectSettingsCategory(page, 'Goals')`.
  2. Set `#weekly-goal-minutes` to `50`; blur to auto-save.
  3. `tapTab(page, 'Calendar')`.
  4. Assert the focus-summary card (`#total-focus-week`, `#avg-focus-day`, etc.) reflects a smaller goal (the _progress_ indicator within the card should show a higher % toward goal compared to the default).
  5. Run one debug session via the harness's seeded sessions (or via `enableDebugTimers` + run flow) and re-assert the progress percentage moved.

### Step 21: Author `settings-advanced.spec.js`

- One test, multi-step:
  1. `gotoTimer(page)`; `openSettings(page)`; `selectSettingsCategory(page, 'Advanced')`.
  2. Toggle `#autostart-enabled` — assert the mock's `is_autostart_enabled` / `enable_autostart` / `disable_autostart` calls flow through (UI-visible: the checkbox state updates and persists).
  3. Toggle `#hide-icon-on-close`; assert persistence.
  4. Change `#status-bar-display` from `default` to `icon-only`; assert persistence.
  5. Toggle `#analytics-enabled`; assert persistence.
  6. Toggle `#debug-mode`; navigate to Timer; assert timer durations now show 3-second debug values (e.g. starts at `00:03`).
  7. Click `#reset-all-data-btn`; cancel the confirmation modal at the first prompt (UI click on cancel); assert no data was reset (tags from earlier specs are preserved within the same test). **Do not actually accept the reset** — that would tear down state mid-spec.

### Step 22: Author `settings-updates.spec.js`

- One test, multi-step:
  1. Configure the fixture to mock `plugin:updater|check` to return `null` (no update available) for the first call, then `{ available: true, version: "9.9.9", currentVersion: "0.4.4", manualDownloadRequired: true }` for the second.
  2. `gotoTimer(page)`; `openSettings(page)`; `selectSettingsCategory(page, 'Updates')`.
  3. Assert `#current-version` text matches the mocked version.
  4. Toggle `#auto-check-updates`; toggle `#include-prerelease`; assert persistence.
  5. Click `#check-updates-btn`; assert `#update-status` shows "no updates" or equivalent.
  6. Click `#check-updates-btn` again; assert `#update-info` becomes visible with the new version.
- Note in a comment: "This spec mocks at the Tauri command boundary (`plugin:updater|check`) — annotated TODO(stack-swap) per the issue's policy. Never asserts on raw network response bodies."

### Step 23: Author `auth.spec.js`

- One test, multi-step:
  1. Configure the fixture: stub `window.supabase` (since the page tries to load Supabase from the jsdelivr CDN, which `_blockExternal` blocks; provide a stand-in via `addInitScript` that exposes `supabase.createClient(...)` returning a mock with the auth methods Presto calls).
  2. `gotoTimer(page)`; click `#user-avatar-btn`; assert `#user-dropdown` becomes visible.
  3. Click `#user-sign-in`; the auth UI overlay (or login modal) appears.
  4. Fill the email/password fields; submit. Mock returns a successful session.
  5. Assert `#user-name` updates from `Guest` to the mocked user's name; `#user-status` updates to non-guest.
  6. Click `#user-avatar-btn` again; click `#user-sign-out`. Mock signs out.
  7. Assert `#user-name` returns to `Guest`.

### Step 24: Author `team.spec.js`

- One test, multi-step:
  1. Use `enableTeamButton` fixture _before_ `goto` (removes the `disabled` attribute from `#team-nav` via `addInitScript` — environment setup, not mid-flow injection — and is the documented exception to rule #1.2 because the button is gated in HTML today).
  2. `gotoTimer(page)`; `tapTab(page, 'Team')`.
  3. Assert `#team-view` is visible.
  4. Assert team-stat cards (`#team-focusing`, `#team-on-break`, `#team-privacy`, `#team-offline`) have numeric content.
  5. Assert at least one team section renders inside `#team-members-grid`.
  6. Click on the first member card; assert a detail UI appears (or, if no detail UI exists, assert the card shows correct member info: name, role, avatar). Verify against the demo data in `team-manager.js`.

### Step 25: Author `update-notification.spec.js`

- One test, multi-step:
  1. Configure the fixture to seed `setUpdateAvailable({ version: "1.2.3", manualDownloadRequired: true })` so `UpdateManagerV2` emits `update-available` shortly after boot.
  2. `gotoTimer(page)`.
  3. Assert `.update-notification-container` becomes visible with text matching `1.2.3`.
  4. Click the `.update-close[data-action="close"]` button; assert the banner is hidden (class change or `display: none`).
  5. Re-trigger the seeded event via the harness (only allowed at fixture level — use a per-test helper that re-emits via init-script-installed `window.__E2E_TEST_HARNESS__.emitUpdateAvailable()` — annotate as `TODO(stack-swap)`).
  6. Assert banner reappears; click `[data-action="dismiss"]`; assert banner disappears and stays dismissed across navigation.

### Step 26: Wire the CI job

- Edit `.github/workflows/ci.yml`. Add a new job `e2e`:
  ```yaml
  e2e:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4
      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: "20"
          cache: "npm"
      - name: Install frontend dependencies
        run: npm ci
      - name: Cache Playwright browsers
        uses: actions/cache@v4
        with:
          path: ~/.cache/ms-playwright
          key: ${{ runner.os }}-playwright-${{ hashFiles('package-lock.json') }}
      - name: Install Playwright browsers
        run: npx playwright install --with-deps chromium
      - name: Run E2E tests
        run: npm run test:e2e
      - name: Upload Playwright report on failure
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: playwright-report
          path: playwright-report/
          retention-days: 7
  ```
- Confirm the job runs in parallel with `frontend` and `backend`. Failures in any job block PR merge per the existing `pull_request` trigger.

### Step 27: Run the full validation matrix locally

- `npm run typecheck` — passes (no new TS errors; spec files are JS, opt-out via `tsconfig.json` exclude if needed).
- `npm run lint` — passes (extend `eslint.config.js` if needed to mark `tests/e2e/**` as Playwright env so `test`, `expect` globals lint clean).
- `npm test` — passes (Vitest unaffected).
- `cd src-tauri && cargo test` — passes.
- `npm run test:e2e` — passes; review the HTML report to confirm each spec walks a full user journey with multiple assertions per ground rule #3.
- Cycle through any flake until all 15 specs are green for 3 consecutive runs.

### Step 28: Documentation pass

- Append to `README.md` under "Available Scripts": `- npm run test:e2e — Run Playwright E2E suite (UI-driven, browser-level)`.
- Spot-check `tests/e2e/CLAUDE.md` against the issue's verbatim ground-rule blocks; the exact text must match.
- No other docs changes.

## Testing Strategy

### Unit Tests

This feature _is_ a test suite. There are no new unit tests; existing unit tests (Vitest, cargo test) remain green and unmodified.

### Integration Tests

The 15 E2E spec files are themselves the integration tests. Each spec is a multi-step user journey driving multiple components together (timer + tags + navigation + persistence). They run in Playwright + Chromium against a Vite-served `src/`, with `_blockExternal` and `tauriMock` auto-fixtures applied to every test.

Coverage matrix:

| Layer                                        | Tested by                         |
| -------------------------------------------- | --------------------------------- |
| Pure helpers (Rust)                          | Cargo unit tests (#4)             |
| Pure helpers (JS)                            | Vitest unit tests (#4)            |
| Manager logic (JS, mocked Tauri)             | Vitest manager tests (#8)         |
| Tauri command surface (Rust IPC)             | Cargo MockRuntime tests (#9)      |
| **Rendered UI + screen interplay (browser)** | **Playwright E2E (this feature)** |

### Edge Cases

- **Timing flakes**: tests that rely on the timer ticking use bounded `expect(...).toHaveText(...)` waits with explicit timeouts (5–10 s ceiling) — never `waitForTimeout`. Debug-mode 3-second timers are used wherever real-time progression matters.
- **Persistence across navigation**: tags/sessions/settings persist across same-page navigations because the `tauriMock` keeps an in-memory state; they reset on `page.goto`. Specs are designed so no cross-spec state assumptions exist.
- **External script failures**: `_blockExternal` aborts Supabase/jsdelivr/Google Fonts requests; the page must still boot. Specs verify zero pageerrors (smoke spec).
- **Disabled team button**: handled by `enableTeamButton` fixture as documented exception.
- **No "edit tag" UI**: `tags.spec.js` includes a comment documenting the gap; does not assert on a feature that doesn't exist.
- **OS global shortcuts**: cannot be triggered from a browser; the shortcuts spec uses the in-app `Space` fallback and annotates the limitation.
- **CSP differences**: production-built Tauri uses CSP that limits external scripts. The Vite dev server has no such CSP — but `_blockExternal` enforces the same boundary at the network layer, so behavior parity is preserved.
- **Headless rendering**: visual elements that depend on layout (calendar grid, theme switcher) are asserted via state, not pixel-perfect screenshots. Visual regression is out of scope for this PR.
- **Reset all data**: spec must _not_ accept the reset confirmation, only verify the prompt UI works. Accidentally accepting would wipe the in-memory mock state mid-spec.

## Acceptance Criteria

- [ ] `tests/e2e/CLAUDE.md` exists with the issue's verbatim ground rules (rules 1–3 with all sub-bullets); a final paragraph documents the dev-server choice (Vite over `tauri dev`).
- [ ] One spec file per row in the issue's table (15 spec files); each is a single multi-step `test()` walking a user journey, asserting at multiple sub-steps per ground rule #3.
- [ ] No spec uses `page.evaluate` mid-flow to manipulate page state (rule #1.2). The only `addInitScript` usage is in fixtures, run before initial navigation (rule #1.2 exception clause).
- [ ] No spec uses `page.waitForTimeout()` (rule #1.3).
- [ ] All `goto` calls happen at the very start of a test; no spec navigates by URL after the first `goto` (rule #1.1).
- [ ] All assertions verify visible UI state (text, classes, visibility, attribute values), not in-memory store contents directly (rule #1.4).
- [ ] All non-loopback HTTP requests are blocked by the `_blockExternal` auto-fixture (rule #2). Verified by grepping the spec files for any `route.continue()` override targeting an external URL — there must be none.
- [ ] `npm run test:e2e` exits 0 locally on a clean checkout.
- [ ] `.github/workflows/ci.yml` includes a new `e2e` job; CI run is green; failures block PR merge (`pull_request` trigger).
- [ ] All Tauri-bridge mocking is contained in `tests/e2e/fixtures/tauriMock.js` and is annotated with a `// TODO(stack-swap):` comment block at the file top describing what needs to be re-implemented when the frontend stack swaps.
- [ ] No spec file imports from `src/utils/`, `src/managers/`, `src/core/`, or `src/components/`. Specs depend only on rendered HTML and the fixtures.
- [ ] `npm run typecheck`, `npm run lint`, `npm test`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt -- --check`, `npx prettier --check .` all remain green.
- [ ] No regression in existing unit/manager/cargo tests.
- [ ] HTML Playwright report uploads on CI failure for triage.

## Validation Commands

Execute every command to validate the feature works correctly with zero regressions.

```bash
# Install Playwright browsers (one-time per machine)
npm ci
npm run e2e:install

# Run the E2E suite locally
npm run test:e2e

# Aggregate validation matrix (mirrors CI)
npm run typecheck
npm run lint
npm test
cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt -- --check
cd .. && npx prettier --check .

# Inspect a specific spec interactively
npx playwright test tests/e2e/timer.spec.js --headed --debug

# Open the last HTML report
npx playwright show-report

# Verify the suite catches regressions: temporarily break src/index.html (rename #play-pause-btn to #play-pause-broken) and confirm timer.spec.js fails. Revert before commit.
```

For verifying CI parity locally (matches `.github/workflows/ci.yml` `e2e` job exactly):

```bash
npm ci && npx playwright install --with-deps chromium && npm run test:e2e
```

## Notes

- **Why Playwright over WebdriverIO**: Playwright has first-class Node TS/JS APIs, a built-in `webServer` runner, automatic browser binary management, parallel workers, and the `auto: true` fixture pattern that fits the issue's "every test inherits `_blockExternal`" requirement cleanly. WebdriverIO is more flexible for native automation (`tauri-driver` integration), but we explicitly do _not_ drive Tauri's native webview here — the suite runs in Chromium against Vite-served `src/`, which keeps CI minimal and matches the issue's "acceptable to drop the suite on `tauri dev` if vite-only is impractical" — the inverse, which is exactly the simpler choice.

- **Why mock the Tauri bridge instead of running real Tauri**: ground rule #2 (no internet calls) plus the abandoned-upstream/handoff context plus the swap context plus CI cost together favor a fully-isolated harness. Real Tauri integration is already covered by Phase 3 cargo MockRuntime tests (#9). The E2E suite's job is _user-visible behavior_, not the Tauri IPC contract.

- **Selector stability for the swap**: the suite leans on element IDs that already exist in `src/index.html` and on accessible roles/text. Authors of the Leptos rewrite are encouraged to preserve these IDs (`#play-pause-btn`, `#timer-minutes`, `#calendar-nav`, `#focus-duration`, etc.) — a one-page migration table lives in the post-swap commit's PR description rather than as a maintained doc here. If a Leptos component needs to drop an ID, the corresponding spec selector should be updated in the same PR; this is acceptable because the _flow_ is preserved, only the _handle_ changes.

- **What deliberately is not tested by this suite** (out of scope, tracked in follow-ups):
  - Visual regression / pixel-diff (worth investing in only after the swap settles).
  - Cross-browser coverage (Firefox, WebKit) — Chromium-only suffices given the production target uses WebKit2GTK on Linux and WebKit on macOS, but those are a Tauri concern, not a feature concern.
  - Real OS global shortcut handling (browser cannot trigger `CommandOrControl+Alt+Space`).
  - Real Supabase auth round-trip.
  - Real Tauri updater download/install.
  - Tray, autostart, native window management — Tauri-specific, no browser-driven path.

- **Annotation policy for stack-swap**: the Tauri-mock fixture is the _single_ file with implementation-detail coupling that needs to be revisited on swap. Spec files contain only ARIA/role/visible-text/stable-ID selectors. The shortcut spec carries one `TODO(stack-swap):` comment regarding the in-app `Space` fallback path. The update-notification spec carries one regarding the harness re-emit helper. No other spec needs `TODO(stack-swap):` markers.

- **Why include `team.spec.js` even though `#team-nav` is disabled**: the `TeamManager` already exists, the team view DOM already exists, and the demo data is exercised; the disabled state is a UX feature-flag, not a structural absence. Enabling it via fixture is justified — we are testing the team feature's _behavior_, not its current product gating.

- **Why no `goto`-mid-flow even though Playwright supports it**: rule #1.1 is the single most important guarantee for swap-survivability. If the URL routing scheme changes (e.g. Leptos uses fragment routing or its own route guards), every `goto`-using spec breaks. UI-driven nav survives.

```

---
*Generated by Agentex*
```
