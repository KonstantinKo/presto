import { test, expect } from "./fixtures/index.js";
import {
  gotoTimer,
  tapTab,
  selectSettingsCategory,
  dismissWelcomePing,
} from "./fixtures/screens.js";

test("visual baseline: timer, tags, calendar, team, all settings tabs, update banner, auth modal", async ({
  page,
  tauriMock,
}) => {
  // Pre-navigation setup: freeze time, enable update banner, enable team tab
  await tauriMock.freezeTime("2026-05-09T12:00:00Z");
  await tauriMock.setUpdateAvailable();
  await tauriMock.enableTeamButton();

  // Single navigation (Rule 1.1)
  await gotoTimer(page);

  // Wait for the welcome ping to leave the DOM before any screenshots
  await dismissWelcomePing(page);

  // --- 5a. Update notification banner ---
  // The banner appears ~5 s after boot due to the test-mode flag
  const banner = page.locator("#update-notification-container");
  await expect(banner).toHaveClass(/visible/, { timeout: 12000 });
  await expect(banner.locator("#update-notification-version")).toContainText("0.4.5");
  await expect(page).toHaveScreenshot(["visual-regression", "update-notification.png"]);
  await page.locator("#update-notification-close").click();
  await expect(banner).not.toHaveClass(/visible/, { timeout: 3000 });

  // --- 5b. Timer (clean state) ---
  await expect(page.locator("#timer-view")).toBeVisible();
  await expect(page.locator("#timer-minutes")).toHaveText("25");
  await expect(page.locator("#timer-seconds")).toHaveText("00");
  await expect(page.locator("#play-icon")).toBeVisible();
  await expect(page).toHaveScreenshot(["visual-regression", "timer.png"]);

  // --- 5c. Tag manager dropdown ---
  await page.locator("#timer-status").click();
  await expect(page.locator("#tag-dropdown-menu")).toBeVisible({ timeout: 5000 });
  await expect(page).toHaveScreenshot(["visual-regression", "tag-manager.png"]);
  await page.locator("#timer-status").click();
  await expect(page.locator("#tag-dropdown-menu")).toBeHidden({ timeout: 5000 });

  // --- 5d. Calendar ---
  await tapTab(page, "Calendar");
  await expect(page.locator("#calendar-view")).not.toHaveClass(/hidden/);
  await expect(page.locator("#week-range")).not.toBeEmpty();
  await expect(page.locator("#current-month")).not.toBeEmpty();
  await expect(page).toHaveScreenshot(["visual-regression", "calendar.png"]);

  // --- 5e. Team ---
  await tapTab(page, "Team");
  await expect(page.locator("#team-view")).toBeVisible();
  await expect(page.locator("#team-members-grid")).toBeVisible();
  await expect(page.locator("#team-members-grid").getByRole("group").first()).toBeVisible({
    timeout: 5000,
  });
  await expect(page).toHaveScreenshot(["visual-regression", "team.png"]);

  // --- 5f. Settings — eight sub-tabs ---
  await tapTab(page, "Settings");

  await selectSettingsCategory(page, "General");
  await expect(page).toHaveScreenshot(["visual-regression", "settings-general.png"]);

  await selectSettingsCategory(page, "Shortcuts");
  await expect(page).toHaveScreenshot(["visual-regression", "settings-shortcuts.png"]);

  await selectSettingsCategory(page, "Notifications");
  await expect(page).toHaveScreenshot(["visual-regression", "settings-notifications.png"]);

  await selectSettingsCategory(page, "Theme");
  await expect(page).toHaveScreenshot(["visual-regression", "settings-theme.png"]);

  await selectSettingsCategory(page, "Automation");
  await expect(page).toHaveScreenshot(["visual-regression", "settings-automation.png"]);

  await selectSettingsCategory(page, "Goals");
  await expect(page).toHaveScreenshot(["visual-regression", "settings-goals.png"]);

  await selectSettingsCategory(page, "Advanced");
  await expect(page).toHaveScreenshot(["visual-regression", "settings-advanced.png"]);

  await selectSettingsCategory(page, "Updates");
  await expect(page.locator("#current-version")).toContainText("0.4.4");
  await expect(page).toHaveScreenshot(["visual-regression", "settings-updates.png"]);

  // --- 5g. Auth modal (last — it dims the rest of the UI) ---
  await tapTab(page, "Timer");
  await page.locator("#user-avatar-btn").click();
  await expect(page.locator("#user-dropdown")).toBeVisible({ timeout: 3000 });
  await page.locator("#user-sign-in").click();
  await expect(page.locator("#auth-overlay")).toBeVisible({ timeout: 3000 });
  await expect(page).toHaveScreenshot(["visual-regression", "auth-modal.png"]);
});
