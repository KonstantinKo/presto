import { test, expect } from "./fixtures/index.js";
import { gotoTimer, tapTab } from "./fixtures/screens.js";

test("calendar week and month navigation updates displayed ranges", async ({ page }) => {
  await gotoTimer(page);
  await tapTab(page, "Calendar");

  // Capture initial week range and month
  const initialWeekRange = await page.locator("#week-range").textContent();
  const initialMonth = await page.locator("#current-month").textContent();

  // Navigate to previous week
  await page.locator("#prev-week").click();
  const prevWeekRange = await page.locator("#week-range").textContent();
  expect(prevWeekRange).not.toEqual(initialWeekRange);

  // Navigate forward two weeks (should be one week ahead of initial)
  await page.locator("#next-week").click();
  await page.locator("#next-week").click();
  const nextWeekRange = await page.locator("#week-range").textContent();
  expect(nextWeekRange).not.toEqual(initialWeekRange);
  expect(nextWeekRange).not.toEqual(prevWeekRange);

  // Return to initial week
  await page.locator("#prev-week").click();
  await expect(page.locator("#week-range")).toHaveText(initialWeekRange || "");

  // Navigate to previous month
  await page.locator("#prev-month").click();
  const prevMonth = await page.locator("#current-month").textContent();
  expect(prevMonth).not.toEqual(initialMonth);

  // Navigate forward two months
  await page.locator("#next-month").click();
  await page.locator("#next-month").click();
  const nextMonth = await page.locator("#current-month").textContent();
  expect(nextMonth).not.toEqual(initialMonth);
  expect(nextMonth).not.toEqual(prevMonth);

  // Return to initial month
  await page.locator("#prev-month").click();
  await expect(page.locator("#current-month")).toHaveText(initialMonth || "");
});
