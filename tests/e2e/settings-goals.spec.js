import { test, expect } from "./fixtures/index.js";
import { openSettings, selectSettingsCategory, tapTab } from "./fixtures/screens.js";

test("setting weekly goal updates focus summary metrics on calendar view", async ({ page }) => {
  await page.goto("/index.html");

  await openSettings(page);
  await selectSettingsCategory(page, "Goals");

  const defaultGoal = await page.locator("#weekly-goal-minutes").inputValue();
  expect(parseInt(defaultGoal, 10)).toBeGreaterThan(0);

  // Set a low goal (50 minutes) and blur to trigger auto-save (debounced ~1s)
  await page.locator("#weekly-goal-minutes").fill("50");
  await page.locator("#weekly-goal-minutes").press("Tab");

  // Wait for the auto-save to complete; the "✓ Settings saved" notification ping
  // is the visible success signal. Filter by text to avoid colliding with the
  // welcome ping that may still be on screen.
  await expect(
    page.locator(".notification-ping").filter({ hasText: "Settings saved" })
  ).toBeVisible({ timeout: 5000 });

  await tapTab(page, "Calendar");
  await expect(page.locator("#focus-summary-card")).toBeVisible({ timeout: 3000 });

  await expect(page.locator("#total-focus-week")).toBeVisible();
  await expect(page.locator("#avg-focus-day")).toBeVisible();
  await expect(page.locator("#weekly-sessions")).toBeVisible();

  // Verify the goal was persisted before reverting
  await openSettings(page);
  await selectSettingsCategory(page, "Goals");
  await expect(page.locator("#weekly-goal-minutes")).toHaveValue("50");
  await page.locator("#weekly-goal-minutes").fill(defaultGoal);
  await page.locator("#weekly-goal-minutes").press("Tab");
});
