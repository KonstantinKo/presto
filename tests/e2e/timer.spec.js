import { test, expect } from "./fixtures/index.js";

test("timer play / pause / resume / stop flow", async ({ page }) => {
  await page.goto("/index.html");

  // Start timer
  await expect(page.locator("#play-icon")).toBeVisible();
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#pause-icon")).toBeVisible();
  await expect(page.locator("#play-icon")).toBeHidden();

  // Wait for the seconds counter to tick (timer is running)
  await expect(page.locator("#timer-seconds")).not.toHaveText("00", { timeout: 2000 });

  // Pause
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#play-icon")).toBeVisible();
  await expect(page.locator("#pause-icon")).toBeHidden();

  // Resume
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#pause-icon")).toBeVisible();

  // Stop / reset
  await page.locator("#stop-btn").click();

  // Timer should return to the initial focus duration (25:00)
  await expect(page.locator("#timer-minutes")).toHaveText("25", { timeout: 3000 });
  await expect(page.locator("#timer-seconds")).toHaveText("00");
  await expect(page.locator("#play-icon")).toBeVisible();
});
