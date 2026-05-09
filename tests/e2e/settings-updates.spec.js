// This spec mocks at the Tauri command boundary (plugin:updater|check via simulateUpdate in
// test mode) — annotated TODO(stack-swap) per the issue's policy. Never asserts on raw
// network response bodies.
// TODO(stack-swap): Re-implement against whatever update check mechanism the new stack uses.
import { test, expect } from "./fixtures/index.js";
import { gotoTimer, openSettings, selectSettingsCategory } from "./fixtures/screens.js";

test("updates settings: version display, toggle checkboxes, check-updates shows result", async ({
  page,
  tauriMock,
}) => {
  // First call (startup auto-check) returns null; second call (button click) returns update
  await tauriMock.configureUpdaterCalls({ version: "0.4.5", currentVersion: "0.4.4" });

  await gotoTimer(page);
  await openSettings(page);
  await selectSettingsCategory(page, "Updates");

  // Current version should be populated by the app version mock (0.4.4)
  await expect(page.locator("#current-version")).toHaveText("0.4.4", { timeout: 3000 });

  // Auto-check updates is checked by default; toggle it off and back on
  await expect(page.locator("#auto-check-updates")).toBeChecked();
  await page.locator("#auto-check-updates").click();
  await expect(page.locator("#auto-check-updates")).not.toBeChecked();
  await page.locator("#auto-check-updates").click();
  await expect(page.locator("#auto-check-updates")).toBeChecked();

  // Include pre-release is unchecked by default; toggle it on
  await expect(page.locator("#include-prerelease")).not.toBeChecked();
  await page.locator("#include-prerelease").click();
  await expect(page.locator("#include-prerelease")).toBeChecked();

  // First click: startup auto-check was call #1 (null), button click is call #2 (null) — no update yet
  await page.locator("#check-updates-btn").click();
  await expect(page.locator("#update-info")).not.toBeVisible({ timeout: 5000 });

  // Second click: now call #3 (> 2), the update object is returned
  await page.locator("#check-updates-btn").click();

  // The update-info panel should become visible showing the simulated version
  await expect(page.locator("#update-info")).toBeVisible({ timeout: 10000 });
  await expect(page.locator("#latest-version-display")).toHaveText("0.4.5", { timeout: 5000 });

  // Update status should indicate an update is available
  await expect(page.locator("#update-status")).toContainText("available", { timeout: 3000 });
});
