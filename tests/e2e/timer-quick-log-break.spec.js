// Feature 006 / FR-019 — Quick Log left-slot affordance in Break state.
//
// Covers the Break-idle variant of the left-slot affordance: after a
// focus session completes with auto-start-timer OFF, the break timer
// stays idle and the left slot (#stop-btn) must map to "Quick Log"
// (not "Abort"), opening the same modal as the Focus-idle path.

import { test, expect } from "./fixtures/index.js";
import {
  enableDebugTimers,
  openSettings,
  selectSettingsCategory,
  tapTab,
} from "./fixtures/screens.js";

test("Quick Log: Break-state left-slot opens modal", async ({ page }) => {
  // Debug timers fire in ~3 seconds. Auto-start-timer must be OFF so the
  // break stays idle after focus completes — when the break is running the
  // left slot maps to Abort, not Quick Log.
  test.setTimeout(30_000);
  await page.goto("/index.html");

  await openSettings(page);
  await selectSettingsCategory(page, "Automation");
  if (await page.locator("#auto-start-timer").isChecked()) {
    await page.locator("#auto-start-timer").click();
  }
  await expect(page.locator("#auto-start-timer")).not.toBeChecked();

  await enableDebugTimers(page);
  await tapTab(page, "Timer");

  // ── 1. Start a Focus session ──────────────────────────────────────
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#pause-icon")).toBeVisible();

  // ── 2. Focus completes (3 s); mode advances to idle Break ─────────
  // With auto-start off the break timer does not run; #status-text
  // shows the break mode and #play-icon reappears (timer is idle).
  await expect(page.locator("#status-text")).toContainText(/break/i, {
    timeout: 10_000,
  });
  await expect(page.locator("#play-icon")).toBeVisible();

  // ── 3. Left slot (#stop-btn) opens the Quick Log modal in Break ───
  await expect(page.locator("#stop-btn")).toHaveAttribute(
    "aria-label",
    /quick log/i
  );
  await page.locator("#stop-btn").click();
  await expect(page.locator("#quick-log-modal-overlay")).toBeVisible();
});
