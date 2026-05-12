import { test, expect } from "./fixtures/index.js";
import { gotoTimer, tapTab } from "./fixtures/screens.js";

// Feature 003 (FR-019, A14): the pre-rework Calendar tab combined a
// week-range navigator (Statistics-view selector contract) with a
// month-grid navigator (Daily-view selector contract). The two
// surfaces now live on separate views; this test walks both flows in
// sequence from a single page load.

test("week navigation under Statistics view and month navigation under Daily view", async ({ page }) => {
  await gotoTimer(page);

  // --- Statistics view: week navigation ---
  await tapTab(page, "Calendar");

  // Statistics view preserves `#week-range` on the Weekly variant
  // (FR-009 / A13). Cold-load default is Weekly.
  await expect(page.locator("#week-range")).not.toBeEmpty();

  const initialWeekRange = await page.locator("#week-range").textContent();

  // Navigate to previous week.
  await page.locator("#prev-week").click();
  const prevWeekRange = await page.locator("#week-range").textContent();
  expect(prevWeekRange).not.toEqual(initialWeekRange);

  // Navigate forward two weeks (should land one week ahead of initial).
  await page.locator("#next-week").click();
  await page.locator("#next-week").click();
  const nextWeekRange = await page.locator("#week-range").textContent();
  expect(nextWeekRange).not.toEqual(initialWeekRange);
  expect(nextWeekRange).not.toEqual(prevWeekRange);

  // Return to the initial week.
  await page.locator("#prev-week").click();
  await expect(page.locator("#week-range")).toHaveText(initialWeekRange);

  // --- Daily view: month navigation ---
  await tapTab(page, "Daily");

  // Daily view inherits `#current-month`, `#prev-month`, `#next-month`
  // from the pre-rework calendar.rs (A14 / FR-019).
  await expect(page.locator("#current-month")).not.toBeEmpty();

  const initialMonth = await page.locator("#current-month").textContent();

  await page.locator("#prev-month").click();
  const prevMonth = await page.locator("#current-month").textContent();
  expect(prevMonth).not.toEqual(initialMonth);

  await page.locator("#next-month").click();
  await page.locator("#next-month").click();
  const nextMonth = await page.locator("#current-month").textContent();
  expect(nextMonth).not.toEqual(initialMonth);
  expect(nextMonth).not.toEqual(prevMonth);

  await page.locator("#prev-month").click();
  await expect(page.locator("#current-month")).toHaveText(initialMonth);
});
