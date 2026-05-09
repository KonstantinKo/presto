import { test, expect } from "./fixtures/index.js";
import { gotoTimer, openSettings, selectSettingsCategory, tapTab } from "./fixtures/screens.js";

test("advanced settings: autostart, hide-icon, status bar, analytics, debug mode, cancel reset", async ({
  page,
}) => {
  await gotoTimer(page);
  await openSettings(page);
  await selectSettingsCategory(page, "Advanced");

  // Toggle autostart on then off via label (custom checkmark overlay requires label click)
  await expect(page.locator("#autostart-enabled")).not.toBeChecked();
  await page.locator("label.checkbox-label:has(#autostart-enabled)").click();
  await expect(page.locator("#autostart-enabled")).toBeChecked();
  await page.locator("label.checkbox-label:has(#autostart-enabled)").click();
  await expect(page.locator("#autostart-enabled")).not.toBeChecked();

  // Toggle hide-icon-on-close
  await page.locator("label.checkbox-label:has(#hide-icon-on-close)").click();
  await expect(page.locator("#hide-icon-on-close")).toBeChecked();

  // Change status bar display to icon-only
  await page.locator("#status-bar-display").selectOption("icon-only");
  await expect(page.locator("#status-bar-display")).toHaveValue("icon-only");

  // Toggle analytics off (default is enabled)
  await expect(page.locator("#analytics-enabled")).toBeChecked();
  await page.locator("label.checkbox-label:has(#analytics-enabled)").click();
  await expect(page.locator("#analytics-enabled")).not.toBeChecked();

  // Enable debug mode (3-second timers)
  await page.locator("label.checkbox-label:has(#debug-mode)").click();
  await expect(page.locator("#debug-mode")).toBeChecked();

  // Navigate to Timer — timer should now show 0:03 (debug duration)
  await tapTab(page, "Timer");
  await expect(page.locator("#timer-minutes")).toHaveText("00", { timeout: 3000 });
  await expect(page.locator("#timer-seconds")).toHaveText("03");

  // Return to Advanced and click Reset All Data — cancel via the dialog mock (returns false)
  await openSettings(page);
  await selectSettingsCategory(page, "Advanced");

  await page.locator("#reset-all-data-btn").click();
  // The mock dialog.ask returns false by default — no reset should occur
  // Debug mode checkbox state should be preserved (data was not reset)
  await expect(page.locator("#debug-mode")).toBeChecked();
});
