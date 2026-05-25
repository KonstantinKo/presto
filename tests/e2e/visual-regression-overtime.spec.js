import { test, expect } from "./fixtures/index.js";
import {
  dismissWelcomePing,
  enableDebugTimers,
  openSettings,
  selectSettingsCategory,
  tapTab,
} from "./fixtures/screens.js";

// Feature 007 (T034): timer-focus-continuous-overtime VR baseline.
// Lives in its own spec file because the screenshot requires wall-clock time
// to advance, while the main visual-regression spec freezes Date.now().
test("visual baseline: timer-focus-continuous-overtime", async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto("/index.html");
  await dismissWelcomePing(page);

  await openSettings(page);
  await selectSettingsCategory(page, "Automation");
  if (await page.locator("#auto-start-timer").isChecked()) {
    await page.locator("#auto-start-timer").click();
  }
  await expect(page.locator("#auto-start-timer")).not.toBeChecked();
  if (!(await page.locator("#allow-continuous-sessions").isChecked())) {
    await page.locator("#allow-continuous-sessions").click();
  }
  await expect(page.locator("#allow-continuous-sessions")).toBeChecked();
  await enableDebugTimers(page);
  await tapTab(page, "Timer");

  await expect(page.locator("#timer-minutes")).toHaveText("00");
  await expect(page.locator("#timer-seconds")).toHaveText("03");
  await page.locator("#play-pause-btn").click();
  await expect(page.locator(".overtime-cta.visible")).toBeVisible({
    timeout: 10_000,
  });

  await expect(page).toHaveScreenshot(
    ["visual-regression", "timer-focus-continuous-overtime.png"],
    {
      mask: [
        page.locator("nav.sidebar"),
        page.locator("#timer-minutes"),
        page.locator("#timer-seconds"),
      ],
    }
  );
});
