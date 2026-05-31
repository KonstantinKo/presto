import { test, expect } from "./fixtures/index.js";
import { gotoTimer, openSettings, selectSettingsCategory } from "./fixtures/screens.js";

test("theme selector: light/dark/auto modes update html data-theme; timer theme updates data-timer-theme", async ({
  page,
}) => {
  await gotoTimer(page);
  await openSettings(page);
  await selectSettingsCategory(page, "Theme");

  await page.locator("#theme-selector").getByRole("button", { name: /light/i }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light", { timeout: 3000 });

  await page.locator("#theme-selector").getByRole("button", { name: /dark/i }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark", { timeout: 3000 });

  await page.locator("#theme-selector").getByRole("button", { name: /auto/i }).click();
  // Auto resolves to either light or dark based on system; just verify attribute is set
  const htmlTheme = await page.locator("html").getAttribute("data-theme");
  expect(["light", "dark"]).toContain(htmlTheme);

  const themeTiles = page.locator("#timer-theme-grid [data-timer-theme]");
  await expect(themeTiles.first()).toBeVisible({ timeout: 3000 });
  const tileCount = await themeTiles.count();
  expect(tileCount).toBeGreaterThanOrEqual(2);

  const secondTile = themeTiles.nth(1);
  const themeId = await secondTile.getAttribute("data-timer-theme");
  expect(themeId).toBeTruthy();
  await secondTile.click();
  await expect(page.locator("html")).toHaveAttribute("data-timer-theme", themeId, {
    timeout: 3000,
  });
});
