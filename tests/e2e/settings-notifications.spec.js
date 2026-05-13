import { test, expect } from "./fixtures/index.js";
import { openSettings, selectSettingsCategory } from "./fixtures/screens.js";

test("notification settings: permission granted, status shown, toggle sound, test button", async ({
  page,
  tauriMock,
}) => {
  // Grant notification permission before navigation
  await tauriMock.setNotificationPermission("granted");

  await page.goto("/index.html");
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

  // Feature 004: ambient-sound controls flow. Three additive
  // selectors below the metronome row — opt-in checkbox, track
  // dropdown, volume slider (0..=100). All three visible regardless
  // of checkbox state (FR-014).
  const ambientEnabled = page.locator("#ambient-sound-enabled");
  const ambientType = page.locator("#ambient-sound-type");
  const ambientVolume = page.locator("#ambient-sound-volume");

  // Cold-start defaults: off / "none" / "50".
  await expect(ambientEnabled).toBeVisible();
  await expect(ambientType).toBeVisible();
  await expect(ambientVolume).toBeVisible();
  await expect(ambientEnabled).not.toBeChecked();
  await expect(ambientType).toHaveValue("none");
  await expect(ambientVolume).toHaveValue("50");

  // Toggle the feature on; pick Rain; drag the slider to 30.
  await ambientEnabled.click();
  await expect(ambientEnabled).toBeChecked();
  await ambientType.selectOption("rain");
  await expect(ambientType).toHaveValue("rain");
  await ambientVolume.fill("30");
  await expect(ambientVolume).toHaveValue("30");

  // Round-trip persistence: leave Notifications and come back.
  await selectSettingsCategory(page, "General");
  await selectSettingsCategory(page, "Notifications");
  await expect(ambientEnabled).toBeChecked();
  await expect(ambientType).toHaveValue("rain");
  await expect(ambientVolume).toHaveValue("30");

  // Toggle off; controls stay visible (FR-014) and the dropdown +
  // slider remember their values (FR-005). With the parent-checkbox
  // dependent-control affordance shipped in feature 004, the
  // dropdown + slider must additionally read as `disabled` so users
  // (and screen-readers) understand the dependency.
  await ambientEnabled.click();
  await expect(ambientEnabled).not.toBeChecked();
  await expect(ambientType).toBeVisible();
  await expect(ambientVolume).toBeVisible();
  await expect(ambientType).toHaveValue("rain");
  await expect(ambientVolume).toHaveValue("30");
  await expect(ambientType).toBeDisabled();
  await expect(ambientVolume).toBeDisabled();

  // Re-enable; controls become interactive again. Pick "None"; the
  // slider value MUST be preserved across the track-to-None
  // transition (FR-005 / A11).
  await ambientEnabled.click();
  await expect(ambientEnabled).toBeChecked();
  await expect(ambientType).toBeEnabled();
  await expect(ambientVolume).toBeEnabled();
  await ambientType.selectOption("none");
  await expect(ambientType).toHaveValue("none");
  await expect(ambientVolume).toHaveValue("30");
});
