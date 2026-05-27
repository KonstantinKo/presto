import { test, expect } from "./fixtures/index.js";
import { openSettings, selectSettingsCategory, tapTab } from "./fixtures/screens.js";

test("advanced settings: autostart, system pause, status bar, debug mode, cancel reset", async ({
  page,
}) => {
  await page.goto("/index.html");
  await openSettings(page);
  await selectSettingsCategory(page, "Advanced");

  // Toggle autostart on then off
  await expect(page.locator("#autostart-enabled")).not.toBeChecked();
  await page.locator("#autostart-enabled").click();
  await expect(page.locator("#autostart-enabled")).toBeChecked();
  await page.locator("#autostart-enabled").click();
  await expect(page.locator("#autostart-enabled")).not.toBeChecked();

  // Toggle hide-icon-on-close
  await page.locator("#hide-icon-on-close").click();
  await expect(page.locator("#hide-icon-on-close")).toBeChecked();

  // Change status bar display to icon-only
  await page.locator("#status-bar-display").selectOption("icon-only");
  await expect(page.locator("#status-bar-display")).toHaveValue("icon-only");

  // System pause behaviors default on and can be toggled independently.
  await expect(page.locator("#pause-on-lock-screen")).toBeChecked();
  await expect(page.locator("#pause-on-system-suspension")).toBeChecked();
  await page.locator("#pause-on-lock-screen").click();
  await expect(page.locator("#pause-on-lock-screen")).not.toBeChecked();
  await expect(page.locator("#pause-on-system-suspension")).toBeChecked();
  await page.locator("#pause-on-system-suspension").click();
  await expect(page.locator("#pause-on-system-suspension")).not.toBeChecked();
  await page.locator("#pause-on-lock-screen").click();
  await expect(page.locator("#pause-on-lock-screen")).toBeChecked();
  await expect(page.locator("#pause-on-system-suspension")).not.toBeChecked();

  // Enable debug mode (3-second timers)
  await page.locator("#debug-mode").click();
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
