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

  // No detail UI exists today; assert visible member data within the first row
  // (per plan step 24 fallback: 'assert the card shows correct member info: name, role, avatar')
  const firstRow = memberRows.first();
  const memberName = firstRow.locator(".member-name");
  await expect(memberName).toBeVisible();
  const nameText = await memberName.textContent();
  expect(nameText && nameText.trim().length).toBeGreaterThan(0);

  const memberRole = firstRow.locator(".member-role-small");
  await expect(memberRole).toBeVisible();
  const roleText = await memberRole.textContent();
  expect(roleText && roleText.trim().length).toBeGreaterThan(0);

  const memberAvatar = firstRow.locator(".member-avatar-initials");
  await expect(memberAvatar).toBeVisible();
  const avatarText = await memberAvatar.textContent();
  expect(avatarText && avatarText.trim().length).toBeGreaterThan(0);
});
