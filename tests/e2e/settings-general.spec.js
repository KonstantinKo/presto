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

// Feature 005 SC-007 — consecutive textContent snapshots straddling
// a locale switch must NOT contain English-only catalogue values
// after the switch lands (no mixed-locale frame). The snapshot is
// taken on the same Playwright auto-wait tick that observes the
// localised label flip, so any straggler English string in the DOM
// at that point would fail the assertion. Pairs with FR-007 / FR-012
// "single Leptos reactive tick" for locale propagation.
test("locale switch produces no mixed-locale frame in document body", async ({ page }) => {
  await gotoTimer(page);
  await openSettings(page);
  await selectSettingsCategory(page, "General");

  // Establish baseline: app boots in English (default locale per
  // Spec FR-009 / Locale::En) — confirm the language label reads in
  // English so the rest of the test starts from a known state.
  const languageLabel = page.locator('label[for="locale-selector"]');
  await expect(languageLabel).toHaveText("Language:");

  // Snapshot 1: English body text content.
  const beforeBody = await page.locator("body").textContent();
  expect(beforeBody).toContain("Language:");

  // Switch to German via the dropdown.
  await page.locator("#locale-selector").selectOption("de");
  // Wait for the label to flip to its German rendering. Playwright's
  // toHaveText auto-wait ensures the next textContent read happens on
  // a tick that has observed the locale change in the DOM.
  await expect(languageLabel).toHaveText("Sprache:");

  // Snapshot 2: post-switch body text content. By FR-007 / FR-012,
  // every `t!(i18n, ...)` call site re-renders in the same Leptos
  // reactive tick — so a German-rendered "Sprache:" label implies the
  // sibling "Timer Durations" / "Notifications" / etc. labels have
  // ALSO flipped to German.
  const afterBody = await page.locator("body").textContent();

  // English-only catalogue values that MUST NOT appear in the German
  // frame. Each is a string the German catalogue translates away from
  // its English form, so survival here would be a mixed-locale leak.
  const englishOnly = ["Language:", "Notifications", "Timer Durations"];
  for (const needle of englishOnly) {
    expect(afterBody, `English-only "${needle}" must not survive locale switch`).not.toContain(
      needle,
    );
  }

  // German-only strings that MUST appear in the German frame. At least
  // one of each label confirms the catalogue swap landed; "Sprache:" is
  // checked individually above via toHaveText for the auto-wait, and
  // the other two prove the global re-render reached non-General sections.
  const germanOnly = ["Sprache:", "Benachrichtigungen", "Timer-Dauern"];
  for (const needle of germanOnly) {
    expect(afterBody, `German "${needle}" must appear after locale switch`).toContain(needle);
  }

  // Restore English so the test env exits clean for sibling specs.
  await page.locator("#locale-selector").selectOption("en");
  await expect(languageLabel).toHaveText("Language:");
});

// Feature 005 SC-013 — parameterised locale-switch coverage. The
// existing single-test for de leaves it / tr unverified; this loop
// asserts that the "Language" label flips to its catalogue
// translation for each of the three non-default locales. Catches
// missing-key drift in any one catalogue file independent of the
// others.
const localeFixtures = [
  { code: "de", label: "Sprache:" },
  { code: "it", label: "Lingua:" },
  { code: "tr", label: "Dil:" },
];
for (const { code, label } of localeFixtures) {
  test(`locale switch to ${code} renders translated language label`, async ({ page }) => {
    await gotoTimer(page);
    await openSettings(page);
    await selectSettingsCategory(page, "General");

    const languageLabel = page.locator('label[for="locale-selector"]');
    await expect(languageLabel).toHaveText("Language:");

    await page.locator("#locale-selector").selectOption(code);
    await expect(languageLabel).toHaveText(label);

    // Restore English to exit the test env clean.
    await page.locator("#locale-selector").selectOption("en");
    await expect(languageLabel).toHaveText("Language:");
  });
}

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
