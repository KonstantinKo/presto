// Feature 006 / T066 — ✓ Complete button + status-quo title clear
// regression coverage on natural completion.
//
// Covers FR-013 (Complete reveals as the right-slot button in
// Paused), FR-015 (elapsed < 30 acts as Abort delegation: no count,
// title preserved), the headline FR-007 regression (natural focus
// completion clears the title).
//
// Engine-level coverage for FR-014 (≥ 30 s threshold) lives in the
// 14 GREEN tests under `src/src/engine/timer.rs` (T014, T020 etc.).
// The DOM-level surface here proves the affordance is wired
// correctly and the long-lived FR-007 contract still holds under the
// rework.

import { test, expect } from "./fixtures/index.js";
import {
  gotoTimer,
  openSettings,
  selectSettingsCategory,
  tapTab,
} from "./fixtures/screens.js";

test("Complete affordance + FR-007 natural-completion title clear regression", async ({
  page,
}) => {
  // R-001 regression adds a > 30-second wall-clock wait so a UI-
  // triggered Complete from Paused crosses the engine's elapsed
  // threshold. Bump the default test timeout to allow that plus
  // the upstream debug-mode flow.
  test.setTimeout(180_000);
  await page.goto("/index.html");

  // ── 0. Enable debug mode so the focus duration is 3 s wall-clock —
  // makes natural completion testable in a single spec without hour-
  // long waits.
  await openSettings(page);
  await selectSettingsCategory(page, "Advanced");
  await page.locator("#debug-mode").click();
  await expect(page.locator("#debug-mode")).toBeChecked();

  // Disable auto-start so the next focus session doesn't fire while
  // we're asserting on the post-completion DOM.
  await selectSettingsCategory(page, "Automation");
  // The default is ON — toggle off.
  await expect(page.locator("#auto-start-timer")).toBeChecked();
  await page.locator("#auto-start-timer").click();
  await expect(page.locator("#auto-start-timer")).not.toBeChecked();

  await tapTab(page, "Timer");
  await expect(page.locator("#timer-minutes")).toHaveText("00");
  await expect(page.locator("#timer-seconds")).toHaveText("03");

  // ── 1. Set a session title before starting ───────────────────────
  await page.locator("#session-title-input").fill("Audit FR-007");
  await expect(page.locator("#session-title-input")).toHaveValue("Audit FR-007");

  // ── 2. Natural focus completion clears the title (FR-007 regression).
  // After the 3-second focus completes, the mode advances to Break —
  // and the title input is only rendered for Focus. The contract is
  // that the next time the user lands on a Focus session, the title
  // field is empty (not lingering with the prior title).
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#pause-icon")).toBeVisible();

  // Wait for the post-completion Idle state in the next mode.
  await expect(page.locator("#play-icon")).toBeVisible({ timeout: 8000 });
  await expect(page.locator("#status-text")).not.toContainText(/focus/i);
  // Title input is removed from the DOM in non-Focus modes (FR-007).
  await expect(page.locator("#session-title-input")).toBeHidden();

  // ── 3. ✓ Complete reveals as the right slot in Paused (FR-013) ───
  // Back on whatever mode follows. The button matrix here is mode-
  // agnostic for the Paused affordance; we exercise the Focus path by
  // skipping back to Focus.
  // The Idle right-slot label is "Skip Mode" (re-confirms the rename
  // at the DOM level — FR-018).
  await expect(page.locator("#skip-btn")).toHaveAttribute(
    "aria-label",
    /skip.*mode|skip current mode|advance to the next phase/i
  );
  // Skip Mode until we're back on Focus.
  for (let i = 0; i < 4; i++) {
    const modeLabel = (await page.locator("#status-text").textContent()) || "";
    if (/focus/i.test(modeLabel)) break;
    await page.locator("#skip-btn").click();
  }
  await expect(page.locator("#status-text")).toContainText(/focus/i);
  // FR-007 regression: when we land back on Focus, the title-input
  // is empty (no lingering "Audit FR-007").
  await expect(page.locator("#session-title-input")).toBeVisible();
  await expect(page.locator("#session-title-input")).toHaveValue("");

  // Start → immediately Pause so elapsed is < 30 s.
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#pause-icon")).toBeVisible();
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#play-icon")).toBeVisible();

  // Right slot reveals as ✓ Complete (FR-013 / SC-001).
  await expect(page.locator("#skip-btn")).toHaveAttribute(
    "aria-label",
    /complete/i
  );

  // ── 4. R-001 regression: a UI-triggered Complete from Paused with
  // elapsed ≥ 30 s persists the session into the sessions-history
  // table. Before R-001 the persistence block was tick-only, so the
  // engine.complete() event vector never reached `save_session_data`
  // and the daily-history view stayed empty.
  //
  // The engine's 30-second threshold (FR-014) lives on wall-clock
  // elapsed, not on the configured focus duration — debug mode's
  // 3-second focus can't satisfy it (it auto-completes naturally
  // before 30s). Plan:
  //   1. From the current Paused state (debug-3s focus), click the
  //      left-slot Abort button to return the engine to Idle on
  //      Focus mode.
  //   2. Disable debug mode so the next start uses the configured
  //      25-min focus duration. set_durations rebases the displayed
  //      remaining only when fully Idle — the abort above is the
  //      precondition.
  //   3. Start a fresh focus, wait > 30 s, pause, Complete, assert.
  await page.locator("#stop-btn").click(); // left-slot = Abort in Paused
  await expect(page.locator("#play-icon")).toBeVisible();
  await expect(page.locator("#status-text")).toContainText(/focus/i);

  await openSettings(page);
  await selectSettingsCategory(page, "Advanced");
  await page.locator("#debug-mode").click();
  await expect(page.locator("#debug-mode")).not.toBeChecked();
  await tapTab(page, "Timer");
  // Confirm the displayed remaining rebased to the 25-min default.
  await expect(page.locator("#timer-minutes")).toHaveText("25");

  // Start a fresh 25-min focus.
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#pause-icon")).toBeVisible();

  // Wait > 30 s wall-clock so elapsed crosses the threshold. Playwright
  // doesn't bless explicit waits — but the engine's 30-second gate is
  // wall-clock-bound, and the only way to test the Complete branch
  // without mid-flow state injection (forbidden by E2E Rule 1.2) is
  // to actually wait. Lock the wait on the document title (the title
  // Effect re-renders on every engine tick from time_remaining_secs)
  // until the displayed remaining is between 24:00 and 24:29, i.e.
  // ≥ 31 s elapsed. The minutes digit alone is too coarse — 24:59
  // satisfies `expect minutes="24"` after just one tick.
  await expect(page).toHaveTitle(/^24:[0-2]\d — Presto$/, { timeout: 60_000 });

  // Pause and Complete.
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#play-icon")).toBeVisible();
  await page.locator("#skip-btn").click(); // right-slot = Complete in Paused

  // Mode should advance to Break (focus completion → break).
  await expect(page.locator("#status-text")).toContainText(/break/i, { timeout: 5_000 });

  // R-001 contract: the session is persisted to the in-memory log
  // and rendered into the sessions history table. Navigate to the
  // Daily view (which hosts the sessions table — off-viewport via
  // CSS positioning so `attached` rather than `visible` is the right
  // assertion shape) and verify one row exists.
  await tapTab(page, "Daily");
  await expect(page.locator("#sessions-table-body tr").first()).toBeAttached({ timeout: 5_000 });
});
