import { test, expect } from "./fixtures/index.js";
import { gotoTimer, enableDebugTimers, tapTab } from "./fixtures/screens.js";

test("debug sessions-history", async ({ page }) => {
  await gotoTimer(page);
  await enableDebugTimers(page);
  await tapTab(page, "Timer");
  await page.locator("#timer-status").click();
  await expect(page.locator("#tag-dropdown-menu")).toBeVisible();
  await page.locator("#tag-list .tag-item").first().click();
  await page.locator("#timer-status").click();
  await expect(page.locator("#tag-dropdown-menu")).toBeHidden({ timeout: 2000 });
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#pause-icon")).toBeVisible();
  await expect(page.locator("#status-text")).toHaveText("Break", { timeout: 12000 });
  await tapTab(page, "Daily");
  await expect(page.locator('#calendar-grid [aria-current="date"]')).toBeVisible({ timeout: 5000 });
  const rows = page.locator("#sessions-table-body").getByRole("row");
  await expect(rows.first()).toBeVisible({ timeout: 5000 });
  await rows.first().getByRole("button", { name: "Edit session" }).click();
  await expect(page.locator("#session-modal-overlay")).toBeVisible({ timeout: 3000 });
  await expect(page.locator("#session-duration")).toBeVisible();

  const preSaveCount = await page.evaluate(() => window.__E2E_TEST_HARNESS__.state.saveManualSessionsCallCount);
  console.log("preSaveCount:", preSaveCount);
  console.log("sessions in mock:", JSON.stringify(await page.evaluate(() => window.__E2E_TEST_HARNESS__.state.manualSessions)));
  
  await page.locator("#save-session-btn").click();
  await expect(page.locator("#session-modal-overlay")).toBeHidden();
  
  const postSaveCount = await page.evaluate(() => window.__E2E_TEST_HARNESS__.state.saveManualSessionsCallCount);
  const saveArgs = await page.evaluate(() => window.__E2E_TEST_HARNESS__.state.lastSaveManualSessionsArgs);
  console.log("postSaveCount:", postSaveCount);
  console.log("saveArgs:", JSON.stringify(saveArgs));
  console.log("manualSessions after:", JSON.stringify(await page.evaluate(() => window.__E2E_TEST_HARNESS__.state.manualSessions)));
});
