import { test, expect } from "./fixtures/index.js";

test("timer play / pause / resume / stop flow", async ({ page }) => {
  await page.goto("/index.html");

  await expect(page.locator("#play-icon")).toBeVisible();
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#pause-icon")).toBeVisible();
  await expect(page.locator("#play-icon")).toBeHidden();

  await expect(page.locator("#timer-seconds")).not.toHaveText("00", { timeout: 2000 });

  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#play-icon")).toBeVisible();
  await expect(page.locator("#pause-icon")).toBeHidden();

  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#pause-icon")).toBeVisible();

  await page.locator("#stop-btn").click();

  await expect(page.locator("#timer-minutes")).toHaveText("25", { timeout: 3000 });
  await expect(page.locator("#timer-seconds")).toHaveText("00");
  await expect(page.locator("#play-icon")).toBeVisible();
});
