import { test, expect } from "./fixtures/index.js";
import { gotoTimer, openSettings, selectSettingsCategory } from "./fixtures/screens.js";

test("theme selector: light/dark/auto modes update html data-theme; timer theme updates data-timer-theme", async ({
  page,
}) => {
  await gotoTimer(page);
  await openSettings(page);
  await selectSettingsCategory(page, "Theme");

  // Switch to Light theme
  await page.locator('#theme-selector .theme-option[data-theme="light"]').click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light", { timeout: 3000 });

  // Switch to Dark theme
  await page.locator('#theme-selector .theme-option[data-theme="dark"]').click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark", { timeout: 3000 });

  // Switch to Auto theme
  await page.locator('#theme-selector .theme-option[data-theme="auto"]').click();
  // Auto resolves to either light or dark based on system; just verify attribute is set
  const htmlTheme = await page.locator("html").getAttribute("data-theme");
  expect(["light", "dark"]).toContain(htmlTheme);

  // Timer theme grid should be populated with .timer-theme-option elements
  const themeTiles = page.locator("#timer-theme-grid .timer-theme-option");
  await expect(themeTiles.first()).toBeVisible({ timeout: 3000 });
  const tileCount = await themeTiles.count();
  expect(tileCount).toBeGreaterThan(0);

  // Click the second timer theme tile to change timer color theme
  if (tileCount >= 2) {
    const secondTile = themeTiles.nth(1);
    const themeId = await secondTile.getAttribute("data-timer-theme");
    await secondTile.click();
    if (themeId) {
      await expect(page.locator("html")).toHaveAttribute("data-timer-theme", themeId, {
        timeout: 3000,
      });
    }
  }
});
