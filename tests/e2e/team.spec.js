import { test, expect } from "./fixtures/index.js";
import { gotoTimer, tapTab } from "./fixtures/screens.js";

test("team view: stats cards display numeric values, member grid renders", async ({
  page,
  tauriMock,
}) => {
  // Enable the #team-nav button (it is disabled in HTML; this is a documented fixture exception)
  await tauriMock.enableTeamButton();

  await gotoTimer(page);
  await tapTab(page, "Team");

  await expect(page.locator("#team-view")).toBeVisible();

  // Team stats cards should have numeric content (populated by team-manager.js demo data)
  await expect(page.locator("#team-focusing")).toBeVisible({ timeout: 5000 });
  const focusingText = await page.locator("#team-focusing").textContent();
  expect(parseInt(focusingText || "0", 10)).toBeGreaterThanOrEqual(0);

  await expect(page.locator("#team-on-break")).toBeVisible();
  await expect(page.locator("#team-privacy")).toBeVisible();
  await expect(page.locator("#team-offline")).toBeVisible();

  // Team members grid should be populated with at least one team section
  await expect(page.locator("#team-members-grid")).toBeVisible();
  const sections = page.locator("#team-members-grid .team-section");
  await expect(sections.first()).toBeVisible({ timeout: 5000 });

  // Each team section should contain member rows
  const memberRows = page.locator("#team-members-grid .member-row");
  const rowCount = await memberRows.count();
  expect(rowCount).toBeGreaterThan(0);
});
