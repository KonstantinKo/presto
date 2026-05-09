// TODO(stack-swap): This spec triggers the update notification via the localStorage test-mode
// flag that causes UpdateManagerV2.simulateUpdate() to run on the 5-second auto-check.
// Re-implement against the new stack's update notification boundary.
import { test, expect } from "./fixtures/index.js";
import { gotoTimer, tapTab } from "./fixtures/screens.js";

test("update notification banner: appears with version and close hides it", async ({
  page,
  tauriMock,
}) => {
  // Seed test mode — causes UpdateManagerV2 to simulate an update after its 5-second startup delay
  await tauriMock.setUpdateAvailable();

  await gotoTimer(page);

  // The banner container is always in DOM (just off-screen); wait for the version text to appear
  // which indicates UpdateManagerV2.simulateUpdate() has fired and shown the notification
  const banner = page.locator(".update-notification-container");
  await expect(banner.locator(".update-version")).not.toBeEmpty({ timeout: 12000 });

  // The version should be "Version 0.4.5" (0.4.4 incremented by simulateUpdate)
  await expect(banner.locator(".update-version")).toContainText("0.4.5");

  // The banner should now have the "visible" class (slid into view)
  await expect(banner).toHaveClass(/visible/, { timeout: 3000 });

  // Click the close button (×) to hide the banner
  await banner.locator(".update-close[data-action='close']").click();
  await expect(banner).not.toHaveClass(/visible/, { timeout: 1000 });

  // Navigate to Calendar and back — banner should remain hidden (just closed, not dismissed)
  await tapTab(page, "Calendar");
  await tapTab(page, "Timer");
  await expect(banner).not.toHaveClass(/visible/);
});
