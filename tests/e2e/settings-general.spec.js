import { test, expect } from "./fixtures/index.js";
import { gotoTimer, openSettings, selectSettingsCategory, tapTab } from "./fixtures/screens.js";

test("general settings: locale switching, no mixed-locale frame, and focus duration", async ({
  page,
}) => {
  await gotoTimer(page);
  await openSettings(page);
  await selectSettingsCategory(page, "General");

  const localeSelector = page.locator("#locale-selector");
  const languageLabel = page.locator('label[for="locale-selector"]');
  await expect(localeSelector).toBeVisible();
  await expect(localeSelector).toHaveValue("en");
  await expect(languageLabel).toHaveText("Language:");

  await localeSelector.selectOption("de");
  await expect(languageLabel).toHaveText("Sprache:");
  const germanBody = await page.locator("body").textContent();
  for (const needle of ["Language:", "Notifications", "Timer Durations"]) {
    expect(germanBody, `English-only "${needle}" must not survive locale switch`).not.toContain(
      needle,
    );
  }
  for (const needle of ["Sprache:", "Benachrichtigungen", "Timer-Dauern"]) {
    expect(germanBody, `German "${needle}" must appear after locale switch`).toContain(needle);
  }

  await tapTab(page, "Timer");
  await tapTab(page, "Settings");
  await selectSettingsCategory(page, "General");
  await expect(localeSelector).toHaveValue("de");
  await expect(languageLabel).toHaveText("Sprache:");

  const localeFixtures = [
    { code: "it", label: "Lingua:" },
    { code: "tr", label: "Dil:" },
    { code: "de", label: "Sprache:" },
  ];
  for (const { code, label } of localeFixtures) {
    await localeSelector.selectOption(code);
    await expect(languageLabel).toHaveText(label);
  }

  await localeSelector.selectOption("en");
  await expect(languageLabel).toHaveText("Language:");

  await tapTab(page, "Timer");
  await expect(page.locator("#timer-minutes")).toHaveText("25");

  await openSettings(page);
  await selectSettingsCategory(page, "General");
  await page.locator("#focus-duration").fill("5");
  await page.locator("#focus-duration").press("Tab");

  await tapTab(page, "Timer");
  await expect(page.locator("#timer-minutes")).toHaveText("05", { timeout: 5000 });
  await expect(page.locator("#timer-seconds")).toHaveText("00");

  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#pause-icon")).toBeVisible();
  await expect(page.locator("#timer-seconds")).not.toHaveText("00", { timeout: 3000 });
  await page.locator("#stop-btn").click();
  await expect(page.locator("#timer-minutes")).toHaveText("05", { timeout: 3000 });

  await openSettings(page);
  await selectSettingsCategory(page, "General");
  await page.locator("#focus-duration").fill("25");
  await page.locator("#focus-duration").press("Tab");

  await tapTab(page, "Timer");
  await expect(page.locator("#timer-minutes")).toHaveText("25", { timeout: 5000 });
});
