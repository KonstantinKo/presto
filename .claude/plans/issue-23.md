# Implementation Plan for #23

**Issue:** Add automated visual regression testing (Playwright visual comparison)
**Type:** feature
**Branch:** agentex/23-add-visual-regression-tests

---

I have enough context to draft the plan. Here it is.

# Feature: Automated Visual Regression Testing (Playwright Visual Comparison)

## Feature Description

Add a Playwright visual-regression spec that captures pixel-level baselines of every major screen in the current Tauri/JS frontend. Each screen is reached via real UI taps (per the existing `tests/e2e/CLAUDE.md` ground rules) and committed as a PNG under `tests/e2e/__screenshots__/`. The suite runs on every CI build and uploads diff artifacts on failure. The baseline images become the visual contract that the upcoming Leptos/WASM rewrite must match before it can ship.

## User Story

As a maintainer about to swap the entire frontend tech stack
I want a committed pixel-level baseline of every screen of the current app and a CI gate that fails on visual drift
So that I can re-implement screens on the new stack and prove "the app looks the same" without doing the comparison by eye

## Problem Statement

We are about to rewrite the frontend in a different language and runtime (most likely Leptos + WASM). Any rewrite of this size will introduce subtle visual differences — wrong padding, off-by-one font weight, drifted colors — that no functional test will catch. Manual visual review across ~14 screens does not scale and is error-prone. Today the `tests/e2e/` Playwright suite asserts behavior only; it has zero pixel-level coverage. Without a frozen visual baseline taken _before_ the swap, post-swap drift is undetectable until users complain.

## Solution Statement

Add `tests/e2e/visual-regression.spec.js` that drives the app exactly like a real user (per Rule 1 of `tests/e2e/CLAUDE.md` — UI taps only, no mid-flow `page.evaluate`) and snapshots every major screen with `expect(page).toHaveScreenshot('<slug>.png')`. Configure Playwright tolerances (`maxDiffPixelRatio: 0.02`, `threshold: 0.2`) generous enough to absorb cross-machine font hinting noise but tight enough to catch real drift. Stabilize the page by disabling animations during snapshots, freezing `Date.now()` (so the calendar header and team-manager demo timers don't drift between runs), and waiting for the welcome notification ping to fade out before each shot. Commit baselines under `tests/e2e/__screenshots__/visual-regression/<slug>-chromium-linux.png` so they live alongside the spec; CI uploads `playwright-report/` and `test-results/` as artifacts on failure so reviewers can eyeball the diffs.

## Relevant Files

- `tests/e2e/CLAUDE.md` — Ground rules (UI-only, no `waitForTimeout`, no mid-flow `evaluate`, single `goto` per test, single `test()` per spec). Visual spec must comply. Will be extended with the "Updating visual baselines" section.
- `tests/e2e/fixtures/index.js` — Auto-fixtures: `_blockExternal` and `tauriMock`. Visual spec inherits both. No change required.
- `tests/e2e/fixtures/tauriMock.js` — Tauri bridge mock + pre-navigation harness helpers (`setUpdateAvailable`, `enableTeamButton`, `seedTags`, `setNotificationPermission`). Will gain a `freezeTime(isoString)` helper that addInitScripts a `Date` override before navigation, so the calendar header and team-manager demo timers render deterministically.
- `tests/e2e/fixtures/screens.js` — Page-object helpers: `gotoTimer`, `tapTab`, `openSettings`, `selectSettingsCategory`, `enableDebugTimers`. Will gain `dismissWelcomePing(page)` (waits for the boot-time `Welcome to Presto! 🍅` notification-ping to leave the DOM) so it does not contaminate timer-view snapshots.
- `tests/e2e/_smoke.spec.js`, `tests/e2e/timer.spec.js`, `tests/e2e/tags.spec.js`, `tests/e2e/auth.spec.js`, `tests/e2e/team.spec.js`, `tests/e2e/settings-*.spec.js`, `tests/e2e/update-notification.spec.js` — Existing spec patterns to mirror: how each screen is reached via UI, what wait predicate signals "screen is settled" (e.g. `#category-<cat>.active`, `#auth-overlay` visible, banner has `.visible` class). Read these to copy navigation steps verbatim into the visual spec.
- `playwright.config.js` — Currently sets `screenshot: 'only-on-failure'`, `trace: 'retain-on-failure'`, `video: 'retain-on-failure'`. Needs `expect.toHaveScreenshot` defaults (`maxDiffPixelRatio`, `threshold`, `animations: 'disabled'`) and a `snapshotPathTemplate` that produces `tests/e2e/__screenshots__/<spec>/<slug>-<projectName>-<platform>.png`.
- `.github/workflows/ci.yml` — `e2e` job already runs `npm run test:e2e` on `ubuntu-latest` and uploads `playwright-report/` on failure. Needs an additional artifact upload for `test-results/` (where Playwright writes the actual diff PNGs `*-actual.png`, `*-expected.png`, `*-diff.png`).
- `package.json` — Has `npm run test:e2e` which already covers any spec under `tests/e2e/`. Per the issue, leave as-is unless a separate `test:visual` script proves meaningful (it does not — the visual spec is just another file in the same dir).
- `src/index.html` — Stable selectors and IDs the visual spec navigates to (e.g. `#timer-nav`, `#calendar-nav`, `#team-nav`, `#settings-nav`, `.settings-nav-item[data-category="..."]`, `#timer-status` to open the tag manager, `#user-avatar-btn` + `#user-sign-in` to open the auth overlay, `#update-notification-container.visible`). No edits.
- `src/managers/team-manager.js` — Demo team data uses `Date.now()` for member timers and `Math.random()` for status churn at a 30 s `setTimeout` cadence. Time freezing fixes the per-member timer text; the 30 s cadence is longer than any single test and thus does not fire mid-snapshot, so the random branches are not entered.
- `src/managers/update-manager-global.js` — `localStorage.presto_force_update_test === 'true'` causes `simulateUpdate()` to fire ~5 s after boot, surfacing `#update-notification-container.visible` with version `0.4.5`. Already wired through `tauriMock.setUpdateAvailable()`.

### New Files

- `tests/e2e/visual-regression.spec.js` — Single spec, single `test()`, walks all 14 screens.
- `tests/e2e/__screenshots__/visual-regression/<slug>-chromium-linux.png` × 14 — Committed baseline images, generated locally with `--update-snapshots` and reviewed before commit.

## Implementation Plan

### Phase 1: Foundation

Land the configuration and stability primitives that the spec depends on, but ship no baselines and no spec yet.

1. Add `expect.toHaveScreenshot` defaults to `playwright.config.js` so every visual assertion in the repo (now and in future) shares the same tolerance and animation-handling policy.
2. Add `snapshotPathTemplate` so baselines land under `tests/e2e/__screenshots__/<spec>/<slug>-<projectName>-<platform>.png` instead of the default `*-snapshots/` directory next to the spec.
3. Extend `tests/e2e/fixtures/tauriMock.js` with a `freezeTime(isoString)` harness method that addInitScripts a `Date` constructor override before the first navigation. Keep it opt-in — existing specs continue to use real time.
4. Extend `tests/e2e/fixtures/screens.js` with `dismissWelcomePing(page)`, which waits (with a generous timeout) for the welcome `notification-ping` to be removed from the DOM so it does not appear in later screenshots.
5. Update `.github/workflows/ci.yml` `e2e` job: add a second `actions/upload-artifact@v4` step (under `if: failure()`) that uploads `test-results/`, the directory where Playwright writes per-failure `*-actual.png`, `*-expected.png`, `*-diff.png`. Keep the existing `playwright-report/` upload.

### Phase 2: Core Implementation

Write the spec, generate and commit baselines, verify the spec passes against its own baselines.

1. Create `tests/e2e/visual-regression.spec.js`. One `test()` (compliant with Rule 3) that:
   - Calls `await tauriMock.freezeTime('2026-05-09T12:00:00Z')`, `await tauriMock.setUpdateAvailable()`, `await tauriMock.enableTeamButton()` before navigation.
   - Calls `await gotoTimer(page)` once.
   - Calls `await dismissWelcomePing(page)`.
   - Captures the update-notification banner first (it appears ~5 s after boot due to the test-mode flag), then closes it via `#update-notification-close`. This sequencing prevents the banner from contaminating the other screenshots.
   - Then walks Timer → Tag manager → Calendar → Team → all 8 Settings sub-tabs → Auth modal (auth modal last because it dims the rest of the UI).
   - At each stop: wait for the screen-settled predicate that the existing spec for that screen uses (e.g. `#calendar-view:not(.hidden)`, `#category-general.active`), then `await expect(page).toHaveScreenshot('<slug>.png')`.
2. Generate baselines locally: `npx playwright test --update-snapshots tests/e2e/visual-regression.spec.js`. Visually review each generated PNG, confirm there is no welcome-ping or stray notification, no recording-state UI, no in-flight animation frame.
3. Commit the 14 PNGs under `tests/e2e/__screenshots__/visual-regression/`. Confirm the path matches the issue's spec format `<screen>-chromium-linux.png`.
4. Re-run `npm run test:e2e` clean (no `--update-snapshots`) and confirm zero diffs.

### Phase 3: Integration

Make the suite reproducible for other developers and CI, and document the baseline-update flow.

1. Push the branch and confirm the `e2e` job in CI passes against the committed baselines (this is the cross-machine confirmation that tolerances and time freezing handle non-determinism).
2. Add an "Updating visual baselines" section to `tests/e2e/CLAUDE.md` documenting the rule: baselines change only when an intentional design change has been made; regenerate with `npx playwright test --update-snapshots tests/e2e/visual-regression.spec.js`; visually review each updated PNG; commit images alongside the design-change PR; do not regenerate to "fix" CI noise — investigate the diff first.
3. Verify the failure path in CI by intentionally introducing a small CSS change on a feature branch, observing the `e2e` job fail with diff PNGs in the uploaded `test-results/` artifact, then revert.

## Step by Step Tasks

IMPORTANT: Execute every step in order, top to bottom.

### 1. Add visual-regression defaults to `playwright.config.js`

- Add an `expect` block with `toHaveScreenshot: { maxDiffPixelRatio: 0.02, threshold: 0.2, animations: 'disabled' }` so all visual assertions share the same tolerance and Playwright stops CSS animations/transitions before capturing.
- Add `snapshotPathTemplate: '{testDir}/__screenshots__/{testFileName}/{arg}{-projectName}-{platform}{ext}'` so generated baselines land at `tests/e2e/__screenshots__/visual-regression/<slug>-chromium-linux.png` (matches the issue's expected path).
- Do not change the existing `screenshot`, `trace`, `video`, or `webServer` blocks.

### 2. Extend `tests/e2e/fixtures/tauriMock.js` with `freezeTime`

- Add a method on the harness object (alongside `setUpdateAvailable`, `enableTeamButton`, etc.):
  - Accepts an ISO date string, e.g. `'2026-05-09T12:00:00Z'`.
  - Calls `page.addInitScript({ content: ... })` with a self-contained IIFE that overrides `globalThis.Date` so `new Date()` and `Date.now()` return the fixed instant. Do not patch `setTimeout`/`setInterval`/`requestAnimationFrame` — only the wall clock, so timers and animations still tick (animations will be frozen by the screenshot's `animations: 'disabled'` option, not by this script).
  - Mark the helper with a `// TODO(stack-swap):` comment if it touches anything Tauri-mock-internal; otherwise it lives at the harness boundary and survives the swap.
- Do not call `freezeTime` from any auto-fixture — it must remain opt-in so existing specs continue to use real time.

### 3. Add `dismissWelcomePing` to `tests/e2e/fixtures/screens.js`

- New exported function: takes `page`, awaits that the `.notification-ping` matching `Welcome to Presto! 🍅` is no longer attached. Use `expect(page.locator('.notification-ping').filter({ hasText: 'Welcome to Presto!' })).toHaveCount(0, { timeout: 8000 })`.
- This duplicates the natural fade-out (the ping auto-dismisses after a few seconds via `NotificationUtils.showNotificationPing` → `dismissNotification`). Waiting for `toHaveCount(0)` is the visible signal it has fully detached, including post-animation cleanup.

### 4. Update CI to upload `test-results/` on failure

- In `.github/workflows/ci.yml`, after the existing "Upload Playwright report on failure" step in the `e2e` job, add a second `actions/upload-artifact@v4` step also gated on `if: failure()`, named `playwright-test-results`, with `path: test-results/` and `retention-days: 7`. Use `if-no-files-found: ignore` so a passing run that produced no `test-results/` does not fail the artifact step.

### 5. Create `tests/e2e/visual-regression.spec.js`

- File header: import `{ test, expect }` from `./fixtures/index.js` and the screen helpers from `./fixtures/screens.js` (`gotoTimer`, `tapTab`, `openSettings`, `selectSettingsCategory`, `dismissWelcomePing`).
- Single `test()` body — title: `"visual baseline: timer, tags, calendar, team, all settings tabs, update banner, auth modal"`.
- Top of test (pre-navigation):
  - `await tauriMock.freezeTime('2026-05-09T12:00:00Z')`
  - `await tauriMock.setUpdateAvailable()`
  - `await tauriMock.enableTeamButton()`
- Navigation: `await gotoTimer(page)` (single `goto`, per Rule 1.1).
- `await dismissWelcomePing(page)`.
- Capture sequence (each step is: navigate via UI taps → wait for screen-settled predicate → `await expect(page).toHaveScreenshot('<slug>.png')`):

#### 5a. Update notification banner

- Wait for `#update-notification-container` to gain the `visible` class (use `expect(page.locator('#update-notification-container')).toHaveClass(/visible/, { timeout: 12000 })`, mirroring `update-notification.spec.js`).
- Wait for `#update-notification-version` to contain `0.4.5`.
- `await expect(page).toHaveScreenshot('update-notification.png')`.
- Click `#update-notification-close` and assert `not.toHaveClass(/visible/)`.

#### 5b. Timer (clean)

- `await expect(page.locator('#timer-view')).toBeVisible()`.
- Verify timer is in default state: `#timer-minutes` = `25`, `#timer-seconds` = `00`, `#play-icon` visible.
- `await expect(page).toHaveScreenshot('timer.png')`.

#### 5c. Tag manager dropdown

- Click `#timer-status`, wait for `#tag-dropdown-menu` visible.
- `await expect(page).toHaveScreenshot('tag-manager.png')`.
- Click `#timer-status` again, wait for `#tag-dropdown-menu` hidden.

#### 5d. Calendar

- `await tapTab(page, 'Calendar')`.
- Wait for `#calendar-view:not(.hidden)`, `#week-range` non-empty, `#current-month` non-empty.
- `await expect(page).toHaveScreenshot('calendar.png')`.

#### 5e. Team

- `await tapTab(page, 'Team')`.
- Wait for `#team-view` visible, `#team-members-grid` visible, first `[role="group"]` inside it visible (mirrors `team.spec.js` waits).
- `await expect(page).toHaveScreenshot('team.png')`.

#### 5f. Settings — eight sub-tabs

- `await tapTab(page, 'Settings')`. (Default tab is General — `#category-general.active` will already be set, but call `selectSettingsCategory(page, 'General')` for explicitness.)
- For each `(name, slug)` pair below, call `await selectSettingsCategory(page, name)` (which already waits for `#category-<cat>.active`), then `await expect(page).toHaveScreenshot('<slug>.png')`:
  - General → `settings-general.png`
  - Shortcuts → `settings-shortcuts.png`
  - Notifications → `settings-notifications.png`
  - Theme → `settings-theme.png`
  - Automation → `settings-automation.png`
  - Goals → `settings-goals.png`
  - Advanced → `settings-advanced.png`
  - Updates → `settings-updates.png` (also wait for `#current-version` to show `0.4.4` so the version field is populated before snapshot)

#### 5g. Auth modal (last — it dims the rest of the UI)

- `await tapTab(page, 'Timer')`.
- Click `#user-avatar-btn`, wait for `#user-dropdown` visible (mirrors `auth.spec.js`).
- Click `#user-sign-in`, wait for `#auth-overlay` visible.
- `await expect(page).toHaveScreenshot('auth-modal.png')`.

### 6. Generate baselines

- Run `npx playwright test --update-snapshots tests/e2e/visual-regression.spec.js` locally.
- Visually inspect every PNG under `tests/e2e/__screenshots__/visual-regression/`. Confirm:
  - No `notification-ping` still on screen (welcome or "Settings saved").
  - Update-notification screenshot shows the banner with version `0.4.5`.
  - Timer shows `25:00` with the play icon (not pause).
  - Calendar header is fixed to the May 2026 week containing 2026-05-09 (proves time freezing works).
  - Team grid shows deterministic per-member timers (proves time freezing works).
  - All settings shots show their respective active tab.
  - Auth modal shows the overlay populated with both the "Sign in" and "Continue as Guest" columns.

### 7. Commit baselines

- `git add tests/e2e/__screenshots__/visual-regression/*.png` plus the new spec, fixture changes, config changes, and CI workflow changes.

### 8. Verify locally

- Run `npm run test:e2e` (full suite, no flags) and confirm zero diffs and zero regressions to existing specs.
- Run `npm run test:e2e -- tests/e2e/visual-regression.spec.js` separately to confirm the spec passes in isolation.

### 9. Document the update flow in `tests/e2e/CLAUDE.md`

- Append a new H2 section titled `Updating visual baselines` with these bullets:
  - Baselines change only when an intentional design change has been made. Visual diffs are a _signal_, not noise to silence.
  - To regenerate: `npx playwright test --update-snapshots tests/e2e/visual-regression.spec.js`.
  - After regenerating, visually review each updated PNG with `git diff` (image diff in your IDE / `git difftool`) and confirm the change matches the design intent.
  - Commit the updated PNGs in the same PR as the design change.
  - If CI is failing on visual diffs and you did _not_ intentionally change the design, do not regenerate — investigate which CSS / DOM change is responsible. Use the `playwright-test-results` artifact in the failed CI run to see `*-actual.png`, `*-expected.png`, `*-diff.png`.
  - Baselines are committed for `chromium-linux` only because CI runs on `ubuntu-latest`; local runs on macOS / Windows may show diffs that pass on CI — trust the CI run.

### 10. Push branch and verify CI

- Open a PR. Confirm the `e2e` job is green on `ubuntu-latest` (same OS the baselines were generated on, given step 6 ran inside the same Linux Playwright image — see Notes for details if step 6 was run on macOS).

## Testing Strategy

### Unit Tests

None. Visual regression is end-to-end by construction.

### Integration Tests

The visual-regression spec _is_ the integration test. It exercises every major screen of the app via the same Playwright runner used by the rest of the e2e suite, with the same auto-fixtures (`_blockExternal`, `tauriMock`).

Verify the spec is wired correctly:

- `npx playwright test tests/e2e/visual-regression.spec.js` — runs the new spec against committed baselines, expects 0 diffs.
- `npm run test:e2e` — full suite still passes; no regression to existing 16 specs.

### Edge Cases

- **Welcome-ping race**: if `dismissWelcomePing(page)` is called before the ping has appeared, `toHaveCount(0)` resolves immediately and we screenshot before the ping shows. Mitigation: the timer-screen settle predicate (`#timer-view` visible) inside `gotoTimer` already runs after `app-loading` has been removed, which happens just before the ping is fired, so by the time `dismissWelcomePing` is called the ping is on screen. If still flaky, gate with `expect(page.locator('.notification-ping').filter({ hasText: 'Welcome to Presto!' })).toBeVisible({ timeout: 3000 })` first, then wait for `toHaveCount(0)`.
- **Update-banner re-fires**: `UpdateManagerV2` may re-check on a long interval. After the user closes the banner via `#update-notification-close`, the `visible` class is removed. Subsequent screenshots take longer than the snapshot interval but shorter than the next auto-check, so the banner stays hidden through Calendar/Team/Settings/Auth captures. If a re-fire is observed in practice, suppress by clearing `localStorage.presto_force_update_test` after the banner snapshot — but this requires UI navigation to a settings toggle; do not use a mid-flow `evaluate` to set localStorage. Better: shorten the auto-check interval irrelevantly by making the banner the first thing snapped.
- **Time freezing on a leap-second / DST boundary**: pick `2026-05-09T12:00:00Z` (a normal weekday in standard time) to avoid DST display ambiguity.
- **Font hinting drift between developer machines and CI**: `maxDiffPixelRatio: 0.02` and `threshold: 0.2` absorb sub-pixel anti-aliasing differences on text-heavy screens. If CI baselines fail locally on Mac/Windows (Linux Chromium uses different font stacks than macOS Chromium), document in `CLAUDE.md` that local diffs are expected — trust CI.
- **Snapshot saved at the wrong moment**: if Playwright's `animations: 'disabled'` does not catch a JS-driven animation (e.g. setInterval that mutates DOM styles), the fix is to wait for an explicit settle predicate before the snapshot, not to add `waitForTimeout` (forbidden by Rule 1.3).
- **CSS changes intentional vs noise**: `Updating visual baselines` section in `tests/e2e/CLAUDE.md` codifies the policy.
- **Calendar screen rendering before data load**: `calendar-navigation.spec.js` uses `expect(page.locator('#week-range')).not.toBeEmpty()` as the settle predicate. Reuse that.
- **Team screen mid-30s-tick**: mid-tick re-render is rare but possible; the `Math.random()` branches change per-member status. Time freezing keeps the timer text deterministic. The 30 s `setTimeout` is set on initialization; the snapshot completes well within 30 s of `team-view` becoming visible, so the random branches are not entered.

## Acceptance Criteria

- [ ] `tests/e2e/visual-regression.spec.js` exists and contains exactly one `test()` (Rule 3 compliant).
- [ ] The single test asserts `expect(page).toHaveScreenshot('<slug>.png')` exactly 14 times: `timer.png`, `tag-manager.png`, `calendar.png`, `team.png`, `settings-general.png`, `settings-shortcuts.png`, `settings-notifications.png`, `settings-theme.png`, `settings-automation.png`, `settings-goals.png`, `settings-advanced.png`, `settings-updates.png`, `update-notification.png`, `auth-modal.png`.
- [ ] All 14 baseline PNG files exist under `tests/e2e/__screenshots__/visual-regression/<slug>-chromium-linux.png`.
- [ ] `playwright.config.js` has `expect.toHaveScreenshot: { maxDiffPixelRatio: 0.02, threshold: 0.2, animations: 'disabled' }` and a `snapshotPathTemplate` that produces the path above.
- [ ] `tests/e2e/fixtures/tauriMock.js` exposes a `freezeTime(isoString)` method on the harness (used by the visual spec, not by anything else).
- [ ] `tests/e2e/fixtures/screens.js` exports `dismissWelcomePing(page)`.
- [ ] `tests/e2e/CLAUDE.md` contains an `Updating visual baselines` section describing when and how to regenerate.
- [ ] `.github/workflows/ci.yml` `e2e` job uploads both `playwright-report/` and `test-results/` on failure (two `upload-artifact` steps, both gated on `if: failure()`).
- [ ] `npm run test:e2e` passes on a clean checkout locally and in CI.
- [ ] No mid-flow `page.evaluate`, no `page.waitForTimeout`, exactly one `page.goto` in the visual spec (Rules 1.1–1.4 honored).
- [ ] No `// TODO(stack-swap):` annotations in the spec body itself; only in `tauriMock.js` if `freezeTime` reaches into bridge internals.
- [ ] An intentional CSS change on a probe branch causes the visual spec to fail in CI with diff images visible in the uploaded `test-results/` artifact. (Validate-and-revert step.)
- [ ] `package.json` is unchanged unless a separate `test:visual` script proves meaningful (it does not — leave `test:e2e` to cover both functional and visual).

## Validation Commands

```bash
# 1. Generate baselines fresh (Phase 2, step 6).
npx playwright test --update-snapshots tests/e2e/visual-regression.spec.js

# 2. Verify the visual spec passes against its committed baselines, in isolation.
npx playwright test tests/e2e/visual-regression.spec.js

# 3. Verify the full e2e suite still passes (no regressions to existing 16 specs).
npm run test:e2e

# 4. Verify the broader project gates still pass (Vitest + typecheck + lint + e2e).
npm run validate

# 5. Confirm baseline files exist and are committed.
git ls-files tests/e2e/__screenshots__/visual-regression/

# 6. Verify the spec is Rule 1 / Rule 3 compliant — exactly one goto and one test().
grep -n "page.goto" tests/e2e/visual-regression.spec.js   # must be exactly one match
grep -n "^test(" tests/e2e/visual-regression.spec.js      # must be exactly one match
grep -n "page.evaluate\|waitForTimeout" tests/e2e/visual-regression.spec.js  # must be zero matches

# 7. Confirm artifact upload wiring in CI.
grep -n "upload-artifact" .github/workflows/ci.yml         # must show TWO uploads inside the e2e job, both `if: failure()`

# 8. Push the branch and confirm the e2e job is green on CI.
git push -u origin <branch>
gh pr create
gh pr checks --watch
```

## Notes

- **Why the snapshot path template is custom**: Playwright's default `snapshotPathTemplate` would put images under `tests/e2e/visual-regression.spec.js-snapshots/<slug>-chromium-linux.png` (sibling of the spec, with a `-snapshots` suffix on the directory). The issue explicitly requests `tests/e2e/__screenshots__/<spec>/<slug>-chromium-linux.png`, hence the custom template. If Playwright's `{-projectName}-{platform}` token expansion produces unexpected double-dashes or different ordering on first run, tune the template until the resolved path is exactly `tests/e2e/__screenshots__/visual-regression/<slug>-chromium-linux.png`.
- **Why baselines are committed only for `chromium-linux`**: CI runs on `ubuntu-latest` with the bundled Playwright Chromium. Cross-platform pixel-perfect rendering of system fonts is impossible (macOS uses CoreText, Linux uses fontconfig+freetype). Anchoring on Linux gives a stable CI gate; local devs accept that local diffs may show but trust CI as the source of truth.
- **Why baselines must be generated in a Linux environment** (Playwright Docker image, CI, or a Linux dev VM): generating on macOS produces `*-chromium-darwin.png` which CI will not consume, _and_ the rendered pixels will not match Linux. If step 6 is run on macOS, drop the baselines and re-generate inside `mcr.microsoft.com/playwright:v1.49.0-noble` (or whichever Playwright image matches `package.json`). One-liner: `docker run --rm -v $PWD:/work -w /work mcr.microsoft.com/playwright:v1.49.0-noble npx playwright test --update-snapshots tests/e2e/visual-regression.spec.js`.
- **Why one big spec instead of many small ones**: Rule 3 says one `test()` per spec file. Splitting into 14 specs would create 14 spec files. One spec walking 14 screens is the cleaner Rule-3-compliant choice, and each `expect(page).toHaveScreenshot(...)` is a self-contained assertion that produces a useful diff regardless of where it sits in the journey.
- **Why the update banner is captured first**: with `setUpdateAvailable()` set as an init script, the banner appears ~5 s after `goto`. Capturing it first means the banner is on screen _before_ we have time to take any other shots (so it can't accidentally appear in them). After we close it via `#update-notification-close`, the rest of the journey is banner-free.
- **Why the auth modal is captured last**: `showAuthScreen()` calls `appContent.children[*].style.display = 'none'` and overlays an `#auth-overlay` over the entire app. After that, navigating back to other screens requires either a full page reload or a sign-in flow — too disruptive to put in the middle of a snapshot journey.
- **Stack-swap survivability**: the spec itself should contain zero implementation-detail coupling. All selectors used (`#timer-nav`, `.settings-nav-item[data-category="..."]`, `#auth-overlay`, etc.) are stable IDs and ARIA roles. The spec drives every transition through real UI clicks. The only stack-coupled piece is `freezeTime` if it ends up touching `tauriMock` internals — annotate with `// TODO(stack-swap):` per repo policy. After the swap, the spec body should only need selector-handle updates (if any IDs change) and the baseline images may need a one-time regeneration if the new stack renders identical-but-not-byte-identical output.
- **Future considerations**: if cross-machine font rendering proves too noisy even with `maxDiffPixelRatio: 0.02`, consider injecting a CSS override via `addInitScript` that forces `font-family: monospace` or a known web-safe stack — but this fundamentally changes the visual baseline's value as a "this is how the app looks" record. Prefer tuning tolerances first.

---

_Generated by Agentex_
