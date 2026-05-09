import { test, expect } from "./fixtures/index.js";
import { gotoTimer, enableDebugTimers, tapTab } from "./fixtures/screens.js";

test("run debug-mode focus session to completion and verify it appears in calendar history", async ({
  page,
}) => {
  await gotoTimer(page);

  // Enable 3-second debug timers via Settings → Advanced
  await enableDebugTimers(page);
  await tapTab(page, "Timer");

  // Pick a tag (default "Focus" tag)
  await page.locator("#timer-status").click();
  await expect(page.locator("#tag-dropdown-menu")).toBeVisible();
  await page.locator("#tag-list .tag-item").first().click();
  await page.keyboard.press("Escape");
  await expect(page.locator("#tag-dropdown-menu")).toBeHidden({ timeout: 2000 });

  // Start the timer
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#pause-icon")).toBeVisible();

  // Wait for the 3-second focus session to complete: status changes to "Break"
  // when the mode transitions from focus to break
  await expect(page.locator("#status-text")).toHaveText("Break", { timeout: 12000 });

  // Navigate to the Calendar view
  await tapTab(page, "Calendar");

  // Today's date should be highlighted in the calendar grid
  await expect(page.locator("#calendar-grid .today")).toBeVisible({ timeout: 5000 });

  // At least one session row should appear in the history table for today
  const rows = page.locator("#sessions-table-body tr");
  await expect(rows.first()).toBeVisible({ timeout: 5000 });

  // Click the edit button in the first row to open the session edit modal
  await rows.first().locator(".session-action-btn.edit").click();
  await expect(page.locator("#session-modal-overlay")).toBeVisible({ timeout: 3000 });
  // Modal shows duration field
  await expect(page.locator("#session-duration")).toBeVisible();

  // Close the modal
  await page.locator("#close-session-modal").click();
  await expect(page.locator("#session-modal-overlay")).toBeHidden();
});
