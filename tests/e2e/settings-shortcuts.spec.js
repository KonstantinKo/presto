// TODO(stack-swap): global OS shortcuts cannot be triggered from a browser
// context and are tested separately via Cargo (#9 follow-up). This spec covers
// in-app recording UI plus the Space-key fallback.
import { test, expect } from "./fixtures/index.js";
import { openSettings, selectSettingsCategory, tapTab } from "./fixtures/screens.js";

test("shortcut settings: record start/stop and abort bindings, verify Space fallback", async ({
  page,
}) => {
  await page.goto("/index.html");
  await openSettings(page);
  await selectSettingsCategory(page, "Shortcuts");

  await page.locator("#start-stop-shortcut").click();
  await expect(page.locator("#start-stop-shortcut")).toHaveClass(/recording/);
  await page.keyboard.press(" ");
  await expect(page.locator("#start-stop-shortcut")).not.toHaveClass(/recording/, {
    timeout: 2000,
  });
  await expect(page.locator("#start-stop-shortcut")).toHaveValue(" ");

  await tapTab(page, "Timer");
  await page.locator("body").press(" ");
  await expect(page.locator("#pause-icon")).toBeVisible({ timeout: 3000 });
  await page.locator("body").press(" ");
  await expect(page.locator("#play-icon")).toBeVisible({ timeout: 3000 });

  await tapTab(page, "Settings");
  await selectSettingsCategory(page, "Shortcuts");
  await expect(page.locator("#abort-shortcut")).toBeVisible();
  await expect(page.locator("#abort-shortcut")).toHaveValue("");

  await page.locator("#abort-shortcut").click();
  await expect(page.locator("#abort-shortcut")).toHaveClass(/recording/);
  await page.keyboard.press("Control+Alt+W");
  await expect(page.locator("#abort-shortcut")).not.toHaveClass(/recording/, {
    timeout: 2000,
  });
  await expect(page.locator("#abort-shortcut")).toHaveValue(/CommandOrControl\+Alt\+w/i);

  await tapTab(page, "Timer");
  await tapTab(page, "Settings");
  await selectSettingsCategory(page, "Shortcuts");
  await expect(page.locator("#abort-shortcut")).toHaveValue(/CommandOrControl\+Alt\+w/i);
});
