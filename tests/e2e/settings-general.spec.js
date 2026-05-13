import { test, expect } from "./fixtures/index.js";
import { gotoTimer, openSettings, selectSettingsCategory, tapTab } from "./fixtures/screens.js";

test("locale switcher in general settings flips localised strings and persists across navigation", async ({ page }) => {
  await gotoTimer(page);
  await openSettings(page);
  await selectSettingsCategory(page, "General");

  // The `#locale-selector` row is the first control on the General tab
  // (above the timer-durations section per FR-015).
  const localeSelector = page.locator("#locale-selector");
  await expect(localeSelector).toBeVisible();
  await expect(localeSelector).toHaveValue("en");

  // The surrounding `<label for="locale-selector">` is localised; in
  // English it reads "Language:".
  const languageLabel = page.locator('label[for="locale-selector"]');
  await expect(languageLabel).toHaveText("Language:");

  // Switch to German.
  await localeSelector.selectOption("de");
  // The same label now renders in German — confirms the i18n provider
  // sees the new locale and re-runs every `t!(...)` call site in a
  // single Leptos reactive tick (FR-007 / FR-012 / SC-007).
  await expect(languageLabel).toHaveText("Sprache:");

  // Persist-across-navigation check. Navigate away and back; the
  // dropdown reflects the saved selection (the debounced auto-save
  // Effect writes through the IPC settings signal).
  await tapTab(page, "Timer");
  await tapTab(page, "Settings");
  await selectSettingsCategory(page, "General");
  await expect(localeSelector).toHaveValue("de");
  await expect(languageLabel).toHaveText("Sprache:");

  // Switch back to English so the test env exits clean.
  await localeSelector.selectOption("en");
  await expect(languageLabel).toHaveText("Language:");
});

test("changing focus duration in general settings updates the timer display", async ({ page }) => {
  await gotoTimer(page);
  await expect(page.locator("#timer-minutes")).toHaveText("25");

  // Open Settings → General
  await openSettings(page);
  await selectSettingsCategory(page, "General");

  // Change focus duration from 25 to 5 minutes (Tab triggers auto-save debounce)
  await page.locator("#focus-duration").fill("5");
  await page.locator("#focus-duration").press("Tab");

  // Navigate to Timer — the 1s auto-save debounce fires during navigation wait
  await tapTab(page, "Timer");
  // Timer pads to 2 digits: "5" is displayed as "05"
  await expect(page.locator("#timer-minutes")).toHaveText("05", { timeout: 5000 });
  await expect(page.locator("#timer-seconds")).toHaveText("00");

  // Verify timer can be started with the new duration
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#pause-icon")).toBeVisible();
  await expect(page.locator("#timer-seconds")).not.toHaveText("00", { timeout: 3000 });

  // Stop/reset
  await page.locator("#stop-btn").click();
  await expect(page.locator("#timer-minutes")).toHaveText("05", { timeout: 3000 });

  // Revert to 25 minutes
  await openSettings(page);
  await selectSettingsCategory(page, "General");
  await page.locator("#focus-duration").fill("25");
  await page.locator("#focus-duration").press("Tab");

  await tapTab(page, "Timer");
  await expect(page.locator("#timer-minutes")).toHaveText("25", { timeout: 5000 });
});
