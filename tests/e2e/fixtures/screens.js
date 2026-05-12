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
 * @param {'Timer'|'Calendar'|'Daily'|'Settings'} title
 */
export async function tapTab(page, title) {
  if (title === "Timer") {
    await page.locator("#timer-nav").click();
    await page.waitForSelector("#timer-view:not(.hidden)", { timeout: 5000 });
  } else if (title === "Calendar") {
    await page.locator("#calendar-nav").click();
    await page.waitForSelector("#calendar-view:not(.hidden)", { timeout: 5000 });
  } else if (title === "Daily") {
    // Feature 003 Bundle B: new Daily drill-down view. `#daily-nav`
    // is the new fourth nav button (FR-012); `#daily-view` is its
    // route container (FR-013).
    await page.locator("#daily-nav").click();
    await page.waitForSelector("#daily-view:not(.hidden)", { timeout: 5000 });
  } else if (title === "Settings") {
    await page.locator("#settings-nav").click();
    await page.waitForSelector("#settings-view:not(.hidden)", { timeout: 5000 });
  } else {
    throw new Error(`tapTab: unsupported title "${title}"`);
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
  if (!cat) {
    throw new Error(
      `selectSettingsCategory: unrecognized name "${name}". Valid names: ${Object.keys(categoryMap).join(", ")}`
    );
  }
  await page.locator(`.settings-nav-item[data-category="${cat}"]`).click();
  await page.waitForSelector(`#category-${cat}.active`, { timeout: 5000 });
}

/**
 * Waits for the boot-time "Welcome to Presto! 🍅" notification ping to leave the DOM
 * so it does not appear in subsequent screenshots.
 * @param {import('@playwright/test').Page} page
 */
export async function dismissWelcomePing(page) {
  await expect(page.getByRole("alert").filter({ hasText: "Welcome to Presto!" })).toHaveCount(0, {
    timeout: 8000,
  });
}

/**
 * Opens Settings → Advanced and enables debug mode (3-second timers).
 * Leaves the user on the Settings → Advanced view.
 *
 * Settings auto-save is debounced ~1 s, so we wait for the visible "✓ Settings
 * saved" notification ping as the success signal. We filter for that exact text
 * because the welcome notification ("Welcome to Presto! 🍅") shown on initial
 * boot may still be on screen, which would otherwise cause a strict-mode
 * violation against `.notification-ping`.
 * @param {import('@playwright/test').Page} page
 */
export async function enableDebugTimers(page) {
  await openSettings(page);
  await selectSettingsCategory(page, "Advanced");
  const debugCheckbox = page.locator("#debug-mode");
  const isChecked = await debugCheckbox.isChecked();
  if (!isChecked) {
    await debugCheckbox.click();
    await expect(page.getByRole("alert").filter({ hasText: "Settings saved" })).toBeVisible({
      timeout: 5000,
    });
  }
}
