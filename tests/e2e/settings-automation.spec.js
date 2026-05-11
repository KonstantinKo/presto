import { test, expect } from "./fixtures/index.js";
import {
  gotoTimer,
  openSettings,
  selectSettingsCategory,
  enableDebugTimers,
  tapTab,
} from "./fixtures/screens.js";

test("automation settings: toggles update UI state and smart-pause timeout shows when enabled", async ({
  page,
}) => {
  await gotoTimer(page);
  await openSettings(page);
  await selectSettingsCategory(page, "Automation");

  // Auto-start timer is checked by default; toggle it off
  await expect(page.locator("#auto-start-timer")).toBeChecked();
  await page.locator("#auto-start-timer").click();
  await expect(page.locator("#auto-start-timer")).not.toBeChecked();

  // Allow Continuous Sessions: toggle on
  await expect(page.locator("#allow-continuous-sessions")).not.toBeChecked();
  await page.locator("#allow-continuous-sessions").click();
  await expect(page.locator("#allow-continuous-sessions")).toBeChecked();

  // Smart Pause: enabling it should reveal the inactivity timeout setting
  await expect(page.locator("#smart-pause")).not.toBeChecked();
  await page.locator("#smart-pause").click();
  await expect(page.locator("#smart-pause")).toBeChecked();
  await expect(page.locator("#smart-pause-timeout-setting")).toBeVisible({ timeout: 2000 });

  // Auto-save sessions is checked by default; toggle it off
  await expect(page.locator("#auto-save-sessions")).toBeChecked();
  await page.locator("#auto-save-sessions").click();
  await expect(page.locator("#auto-save-sessions")).not.toBeChecked();

  // Prevent Interruptions: toggle on
  await expect(page.locator("#prevent-interruptions")).not.toBeChecked();
  await page.locator("#prevent-interruptions").click();
  await expect(page.locator("#prevent-interruptions")).toBeChecked();

  // Re-enable auto-start-timer (it was toggled off above; the behavioral test requires it on)
  await page.locator("#auto-start-timer").click();
  await expect(page.locator("#auto-start-timer")).toBeChecked();

  // Turn off allow-continuous-sessions: with it enabled the engine enters overtime instead
  // of transitioning to Break, which would block the Focus→Break→auto-restart behavioral test.
  await page.locator("#allow-continuous-sessions").click();
  await expect(page.locator("#allow-continuous-sessions")).not.toBeChecked();

  // Enable 3-second debug timers so the end-to-end flow completes quickly
  await enableDebugTimers(page);

  // Navigate to Timer and start a session
  await tapTab(page, "Timer");
  await page.locator("#play-pause-btn").click();

  // Wait for the 3-second focus session to complete — timer should transition to Break
  await expect(page.locator("#status-text")).toHaveText("Break", { timeout: 15000 });

  // Wait for the 3-second break to complete — auto-start-timer should start the next focus
  // session automatically, leaving the timer running (pause icon visible, no play needed)
  await expect(page.locator("#pause-icon")).toBeVisible({ timeout: 15000 });
});
