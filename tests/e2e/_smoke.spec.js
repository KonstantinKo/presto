import { test, expect } from "./fixtures/index.js";

test("page loads, timer view is default, no console errors", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  page.on("console", (msg) => {
    if (msg.type() === "error") {
      const text = msg.text();
      // "Failed to load resource" errors are expected — external URLs are blocked by _blockExternal
      if (!text.includes("Failed to load resource") && !text.includes("ERR_FAILED")) {
        errors.push(text);
      }
    }
  });

  await page.goto("/index.html");
  await expect(page.locator("#timer-minutes")).toHaveText("25");
  await expect(page.locator("#timer-seconds")).toHaveText("00");
  await expect(page.locator("#timer-view")).toBeVisible();
  await expect(page.locator("#calendar-view")).toBeHidden();
  await expect(page.locator("#settings-view")).toBeHidden();

  expect(errors, `unexpected console errors:\n${errors.join("\n")}`).toEqual([]);
});
