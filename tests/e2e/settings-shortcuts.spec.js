// TODO(stack-swap): on the new stack, verify the in-app Space-key fallback is wired
// equivalently. Global OS shortcuts (CommandOrControl+Alt+Space) cannot be triggered from a
// browser context and are tested separately via Cargo (#9 follow-up).
import { test, expect } from "./fixtures/index.js";
import { openSettings, selectSettingsCategory, tapTab } from "./fixtures/screens.js";

test("shortcut recording: record Space key, verify saved, verify Space starts/pauses timer", async ({
  page,
}) => {
  await page.goto("/index.html");
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

// Feature 007 (T033, FR-018, FR-019, FR-020, SC-010): fourth Abort row.
// The new row appears in Settings > Shortcuts; the binding can be recorded
// and persists across a settings reload via the existing settings storage
// mechanism. Default is unbound per FR-019 — the user opts in.
test("Abort shortcut: fourth row recording + persistence across reload", async ({
  page,
}) => {
  await page.goto("/index.html");
  await openSettings(page);
  await selectSettingsCategory(page, "Shortcuts");

  // FR-018: the Abort row exists as the fourth bindable shortcut row.
  await expect(page.locator("#abort-shortcut")).toBeVisible();
  // FR-019: default is unbound — the input starts empty.
  await expect(page.locator("#abort-shortcut")).toHaveValue("");

  // Record a binding: Ctrl+Alt+W (matches the spec's reference shape).
  await page.locator("#abort-shortcut").click();
  await expect(page.locator("#abort-shortcut")).toHaveClass(/recording/);
  await page.keyboard.press("Control+Alt+W");

  // After the 500ms auto-finish delay, recording stops and the binding
  // is captured. formatShortcut emits the "CommandOrControl+Alt+w" shape
  // for a Ctrl+Alt+W keypress.
  await expect(page.locator("#abort-shortcut")).not.toHaveClass(/recording/, {
    timeout: 2000,
  });
  await expect(page.locator("#abort-shortcut")).toHaveValue(
    /CommandOrControl\+Alt\+w/i
  );

  // FR-020: navigate away and back; the binding persists via the
  // settings RwSignal (mock-backed in e2e) so the input still carries
  // the captured value.
  await tapTab(page, "Timer");
  await tapTab(page, "Settings");
  await selectSettingsCategory(page, "Shortcuts");
  await expect(page.locator("#abort-shortcut")).toHaveValue(
    /CommandOrControl\+Alt\+w/i
  );
});
