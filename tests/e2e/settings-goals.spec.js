import { test, expect } from "./fixtures/index.js";
import { openSettings, selectSettingsCategory, tapTab } from "./fixtures/screens.js";

test("setting weekly goal updates focus summary metrics on calendar view", async ({ page }) => {
  await page.goto("/index.html");

  // Open Settings → Goals and set a low weekly goal of 50 minutes
  await openSettings(page);
  await selectSettingsCategory(page, "Goals");

  // Capture the default value
  const defaultGoal = await page.locator("#weekly-goal-minutes").inputValue();
  expect(parseInt(defaultGoal, 10)).toBeGreaterThan(0);

  // Set a low goal (50 minutes) and blur to trigger auto-save
  await page.locator("#weekly-goal-minutes").fill("50");
  await page.locator("#weekly-goal-minutes").press("Tab");

  // Navigate to Calendar view — focus summary card should be visible
  await tapTab(page, "Calendar");
  await expect(page.locator("#focus-summary-card")).toBeVisible({ timeout: 3000 });

  // Weekly summary metrics should be present
  await expect(page.locator("#total-focus-week")).toBeVisible();
  await expect(page.locator("#avg-focus-day")).toBeVisible();
  await expect(page.locator("#weekly-sessions")).toBeVisible();

  // Verify the goal was persisted before reverting
  await openSettings(page);
  await selectSettingsCategory(page, "Goals");
  await expect(page.locator("#weekly-goal-minutes")).toHaveValue("50");
  // Now revert
  await page.locator("#weekly-goal-minutes").fill(defaultGoal);
  await page.locator("#weekly-goal-minutes").press("Tab");
});
