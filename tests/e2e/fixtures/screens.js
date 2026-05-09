import { expect } from "@playwright/test";

/**
 * Page-object-style helpers shared across spec files.
 * All helpers are pure UI: they use Playwright locators and never call page.evaluate().
 */

/**
 * Navigates to the timer view and waits for the timer display to be ready.
 * @param {import('@playwright/test').Page} page
 */
export async function gotoTimer(page) {
  await page.goto("/index.html");
  // Wait for the loading overlay to disappear and the timer to be visible
  await page.waitForSelector("#timer-minutes", { state: "visible", timeout: 15000 });
}

/**
 * Clicks a sidebar navigation button by its title attribute.
 * @param {import('@playwright/test').Page} page
 * @param {'Timer'|'Calendar'|'Team'|'Settings'} title
 */
export async function tapTab(page, title) {
  if (title === "Timer") {
    await page.locator("#timer-nav").click();
    await page.waitForSelector("#timer-view:not(.hidden)", { timeout: 5000 });
  } else if (title === "Calendar") {
    await page.locator("#calendar-nav").click();
    await page.waitForSelector("#calendar-view:not(.hidden)", { timeout: 5000 });
  } else if (title === "Team") {
    await page.locator("#team-nav").click();
    await page.waitForSelector("#team-view:not(.hidden)", { timeout: 5000 });
  } else if (title === "Settings") {
    await page.locator("#settings-nav").click();
    await page.waitForSelector("#settings-view:not(.hidden)", { timeout: 5000 });
  }
}

/**
 * Opens the settings view.
 * @param {import('@playwright/test').Page} page
 */
export async function openSettings(page) {
  await tapTab(page, "Settings");
}

/**
 * Clicks a settings category nav item by its visible text label.
 * @param {import('@playwright/test').Page} page
 * @param {'General'|'Shortcuts'|'Notifications'|'Theme'|'Automation'|'Goals'|'Advanced'|'Updates'} name
 */
export async function selectSettingsCategory(page, name) {
  const categoryMap = {
    General: "general",
    Shortcuts: "shortcuts",
    Notifications: "notifications",
    Theme: "theme",
    Automation: "automation",
    Goals: "goals",
    Advanced: "advanced",
    Updates: "updates",
  };
  const cat = categoryMap[name];
  await page.locator(`.settings-nav-item[data-category="${cat}"]`).click();
  await page.waitForSelector(`#category-${cat}.active`, { timeout: 5000 });
}

/**
 * Opens Settings → Advanced and enables debug mode (3-second timers).
 * Leaves the user on the Settings → Advanced view.
 * @param {import('@playwright/test').Page} page
 */
export async function enableDebugTimers(page) {
  await openSettings(page);
  await selectSettingsCategory(page, "Advanced");
  const debugCheckbox = page.locator("#debug-mode");
  const isChecked = await debugCheckbox.isChecked();
  if (!isChecked) {
    await debugCheckbox.click();
    await expect(page.locator(".notification-ping")).toContainText("Settings saved", {
      timeout: 3000,
    });
  }
}
