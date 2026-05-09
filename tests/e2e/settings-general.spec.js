import { test, expect } from "./fixtures/index.js";
import { gotoTimer, openSettings, selectSettingsCategory, tapTab } from "./fixtures/screens.js";

test("changing focus duration in general settings updates the timer display", async ({ page }) => {
  await gotoTimer(page);
  await expect(page.locator("#timer-minutes")).toHaveText("25");

  // Open Settings → General
  await openSettings(page);
  await selectSettingsCategory(page, "General");

  // Change focus duration from 25 to 5 minutes (Tab triggers auto-save debounce)
  await page.locator("#focus-duration").fill("5");
  await page.locator("#focus-duration").press("Tab");

  // Navigate to Timer — the 1s auto-save debounce fires during navigation wait
  await tapTab(page, "Timer");
  // Timer pads to 2 digits: "5" is displayed as "05"
  await expect(page.locator("#timer-minutes")).toHaveText("05", { timeout: 5000 });
  await expect(page.locator("#timer-seconds")).toHaveText("00");

  // Verify timer can be started with the new duration
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#pause-icon")).toBeVisible();
  await expect(page.locator("#timer-seconds")).not.toHaveText("00", { timeout: 3000 });

  // Stop/reset
  await page.locator("#stop-btn").click();
  await expect(page.locator("#timer-minutes")).toHaveText("05", { timeout: 3000 });

  // Revert to 25 minutes
  await openSettings(page);
  await selectSettingsCategory(page, "General");
  await page.locator("#focus-duration").fill("25");
  await page.locator("#focus-duration").press("Tab");

  await tapTab(page, "Timer");
  await expect(page.locator("#timer-minutes")).toHaveText("25", { timeout: 5000 });
});
