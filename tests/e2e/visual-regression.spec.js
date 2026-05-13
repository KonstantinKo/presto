import { test, expect } from "./fixtures/index.js";
import {
  gotoTimer,
  tapTab,
  selectSettingsCategory,
  dismissWelcomePing,
} from "./fixtures/screens.js";

test("visual baseline: timer, tags, statistics (4 periods), daily, all settings tabs, update banner", async ({
  page,
  tauriMock,
}) => {
  // Pre-navigation setup: freeze time, enable update banner.
  await tauriMock.freezeTime("2026-05-09T12:00:00Z");
  await tauriMock.setUpdateAvailable();

  // Single navigation (Rule 1.1)
  await gotoTimer(page);

  // Wait for the welcome ping to leave the DOM before any screenshots
  await dismissWelcomePing(page);

  // Feature 003 / FR-037: every non-sidebar baseline masks `nav.sidebar`
  // so the four-icons-vs-three-icons sidebar change does NOT cascade-
  // regenerate every baseline. The sidebar masking is intentionally
  // OMITTED only from the four Statistics frames + the Daily frame
  // because those screens are the touched surfaces for this feature.
  const sidebarMask = { mask: [page.locator("nav.sidebar")] };

  // --- 5a. Update notification banner ---
  // The banner appears ~5 s after boot due to the test-mode flag
  const banner = page.locator("#update-notification-container");
  await expect(banner).toHaveClass(/visible/, { timeout: 12000 });
  await expect(banner.locator("#update-notification-version")).toContainText("0.4.5");
  await expect(page).toHaveScreenshot(
    ["visual-regression", "update-notification.png"],
    sidebarMask
  );
  await page.locator("#update-notification-close").click();
  await expect(banner).not.toHaveClass(/visible/, { timeout: 3000 });

  // --- 5b. Timer (clean state) ---
  await expect(page.locator("#timer-view")).toBeVisible();
  await expect(page.locator("#timer-minutes")).toHaveText("25");
  await expect(page.locator("#timer-seconds")).toHaveText("00");
  await expect(page.locator("#play-icon")).toBeVisible();
  await expect(page).toHaveScreenshot(["visual-regression", "timer.png"], sidebarMask);

  // --- 5c. Tag manager dropdown ---
  // Feature 003 / FR-020 / FR-021 / FR-044: the picker now shows 12 icon
  // options (3 remixicon + 9 Phosphor; 5 emoji entries removed), so this
  // baseline legitimately differs and IS regenerated for this feature.
  await page.locator("#timer-status").click();
  await expect(page.locator("#tag-dropdown-menu")).toBeVisible({ timeout: 5000 });
  await expect(page).toHaveScreenshot(["visual-regression", "tag-manager.png"], sidebarMask);
  await page.locator("#timer-status").click();
  await expect(page.locator("#tag-dropdown-menu")).toBeHidden({ timeout: 5000 });

  // --- 5d. Statistics view — four per-period baselines (FR-043 / CHK040) ---
  // Each period is a materially different layout (24-hour bars vs 7
  // day-bars vs 28–31 day-bars vs 12 month-bars + different navigator
  // widgets); one baseline cannot catch a regression specific to e.g.
  // the Monthly bar chart's tick spacing. The Weekly frame is the
  // natural successor to the deleted `calendar-chromium-linux.png`
  // (Weekly is the cold-load default per FR-003 / SC-001).
  await tapTab(page, "Calendar");
  await expect(page.locator("#calendar-view")).not.toHaveClass(/hidden/);

  await page.locator('.period-btn[data-period="daily"]').click();
  await expect(page.locator('.period-btn[data-period="daily"]')).toHaveClass(/active/);
  await expect(page).toHaveScreenshot(["visual-regression", "statistics-daily.png"]);

  await page.locator('.period-btn[data-period="weekly"]').click();
  await expect(page.locator('.period-btn[data-period="weekly"]')).toHaveClass(/active/);
  await expect(page).toHaveScreenshot(["visual-regression", "statistics-weekly.png"]);

  await page.locator('.period-btn[data-period="monthly"]').click();
  await expect(page.locator('.period-btn[data-period="monthly"]')).toHaveClass(/active/);
  await expect(page).toHaveScreenshot(["visual-regression", "statistics-monthly.png"]);

  await page.locator('.period-btn[data-period="yearly"]').click();
  await expect(page.locator('.period-btn[data-period="yearly"]')).toHaveClass(/active/);
  await expect(page).toHaveScreenshot(["visual-regression", "statistics-yearly.png"]);

  // --- 5e. Daily view (new drill-down — FR-012 / FR-013) ---
  await tapTab(page, "Daily");
  await expect(page.locator("#daily-view")).not.toHaveClass(/hidden/);
  await expect(page).toHaveScreenshot(["visual-regression", "daily.png"]);

  // --- 5f. Settings — eight sub-tabs ---
  await tapTab(page, "Settings");

  await selectSettingsCategory(page, "General");
  await expect(page).toHaveScreenshot(["visual-regression", "settings-general.png"], sidebarMask);

  await selectSettingsCategory(page, "Shortcuts");
  await expect(page).toHaveScreenshot(["visual-regression", "settings-shortcuts.png"], sidebarMask);

  await selectSettingsCategory(page, "Notifications");
  await expect(page).toHaveScreenshot(
    ["visual-regression", "settings-notifications.png"],
    sidebarMask
  );

  await selectSettingsCategory(page, "Theme");
  await expect(page).toHaveScreenshot(["visual-regression", "settings-theme.png"], sidebarMask);

  await selectSettingsCategory(page, "Automation");
  await expect(page).toHaveScreenshot(
    ["visual-regression", "settings-automation.png"],
    sidebarMask
  );

  await selectSettingsCategory(page, "Goals");
  await expect(page).toHaveScreenshot(["visual-regression", "settings-goals.png"], sidebarMask);

  await selectSettingsCategory(page, "Advanced");
  await expect(page).toHaveScreenshot(["visual-regression", "settings-advanced.png"], sidebarMask);

  await selectSettingsCategory(page, "Updates");
  await expect(page.locator("#current-version")).toContainText("0.4.4");
  await expect(page).toHaveScreenshot(["visual-regression", "settings-updates.png"], sidebarMask);
});
