// TODO(stack-swap): on the new stack, verify the in-app Space-key fallback is wired
// equivalently. Global OS shortcuts (CommandOrControl+Alt+Space) cannot be triggered from a
// browser context and are tested separately via Cargo (#9 follow-up).
import { test, expect } from "./fixtures/index.js";
import { gotoTimer, openSettings, selectSettingsCategory, tapTab } from "./fixtures/screens.js";

test("shortcut recording: record Space key, verify saved, verify Space starts/pauses timer", async ({
  page,
}) => {
  await gotoTimer(page);
  await openSettings(page);
  await selectSettingsCategory(page, "Shortcuts");

  // Click the start/stop shortcut input to enter recording mode
  await page.locator("#start-stop-shortcut").click();
  await expect(page.locator("#start-stop-shortcut")).toHaveClass(/recording/);

  // Press Space — the shortcut handler records it; formatShortcut([" "]) = " " (space char)
  await page.keyboard.press(" ");

  // After the 500ms auto-finish delay, recording stops and the value is saved
  await expect(page.locator("#start-stop-shortcut")).not.toHaveClass(/recording/, {
    timeout: 2000,
  });
  // Space key is stored as the literal space character " "
  await expect(page.locator("#start-stop-shortcut")).toHaveValue(" ");

  // Navigate to Timer — Space key has a hardcoded fallback in pomodoro-timer.js (e.code === "Space")
  await tapTab(page, "Timer");
  await page.locator("body").press(" ");
  await expect(page.locator("#pause-icon")).toBeVisible({ timeout: 3000 });

  await page.locator("body").press(" ");
  await expect(page.locator("#play-icon")).toBeVisible({ timeout: 3000 });

  // Stop the timer
  await page.locator("#stop-btn").click();
});
