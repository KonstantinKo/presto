import { test, expect } from "./fixtures/index.js";
import { gotoTimer, openSettings, selectSettingsCategory } from "./fixtures/screens.js";

test("notification settings: permission granted, status shown, toggle sound, test button", async ({
  page,
  tauriMock,
}) => {
  // Grant notification permission before navigation
  await tauriMock.setNotificationPermission("granted");

  await gotoTimer(page);
  await openSettings(page);
  await selectSettingsCategory(page, "Notifications");

  // The notification status panel should be visible (displayed during settings init)
  await expect(page.locator("#notification-status")).toBeVisible({ timeout: 3000 });

  // Desktop notifications checkbox should be checked by default
  await expect(page.locator("#desktop-notifications")).toBeChecked();

  // Toggle desktop notifications off
  await page.locator("#desktop-notifications").click();
  await expect(page.locator("#desktop-notifications")).not.toBeChecked();

  // Wait for status to update (500ms debounce after toggle)
  await expect(page.locator("#notification-status-text")).toContainText("Disabled", {
    timeout: 2000,
  });

  // Toggle back on
  await page.locator("#desktop-notifications").click();
  await expect(page.locator("#desktop-notifications")).toBeChecked();

  // Sound notifications should be checked by default; toggle it off
  await expect(page.locator("#sound-notifications")).toBeChecked();
  await page.locator("#sound-notifications").click();
  await expect(page.locator("#sound-notifications")).not.toBeChecked();

  // Click the Test button to trigger a test notification
  await page.locator("#test-notifications-btn").click();
  // After clicking, the dialog mock returns without error — just verify no error thrown
  // The primary assertion is that the UI is still functional after the test
  await expect(page.locator("#notification-status")).toBeVisible();
});
