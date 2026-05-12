// Daily drill-down view — Feature 003 Bundle B.
//
// Covers:
// - SC-006: `#daily-view` mounts and the sidebar `#daily-nav` button
//   routes to it; other view-hosts gain `.hidden`.
// - SC-007: clicking a day cell updates `#selected-day-title` and
//   re-binds the sessions timeline (empty-state label visible for
//   zero-session days).
import { test, expect } from "./fixtures/index.js";
import { gotoTimer, tapTab } from "./fixtures/screens.js";

test("Daily view routes from sidebar and selecting a day re-binds the timeline", async ({
  page,
}) => {
  await gotoTimer(page);

  // SC-006 — sidebar Daily nav routes to `#daily-view`.
  await tapTab(page, "Daily");

  await expect(page.locator("#daily-view")).toBeVisible();
  // Other view containers are hidden via the `.view-host.hidden`
  // wrapper. `#timer-view`, `#calendar-view`, and `#settings-view`
  // exist in the DOM but with the `.hidden` host class.
  await expect(page.locator("#timer-view")).toBeHidden();
  await expect(page.locator("#calendar-view")).toBeHidden();
  await expect(page.locator("#settings-view")).toBeHidden();

  // Two-column layout — month grid on the left, sessions timeline on
  // the right. Both should be present.
  await expect(page.locator("#calendar-grid")).toBeVisible();
  await expect(page.locator("#sessions-timeline")).toBeVisible();
  await expect(page.locator("#selected-day-title")).toBeVisible();

  // Today's cell carries aria-current="date" (FR-018).
  await expect(page.locator('#calendar-grid [aria-current="date"]')).toBeVisible({
    timeout: 5000,
  });

  // SC-007 — Click a non-today day cell (the first in-month, non-
  // today cell that exists in the rendered grid). The grid is 42
  // cells; today is highlighted. Pick the first cell that has a
  // day-number span and is NOT the today-cell.
  const candidateCells = page
    .locator("#calendar-grid .calendar-day:not(.today):not(.other-month)")
    .filter({ has: page.locator(".calendar-day-number") });

  const targetCell = candidateCells.first();
  // Capture the aria-label so we can verify the title updates after the click.
  const targetLabel = await targetCell.getAttribute("aria-label");
  await targetCell.click();

  // The clicked cell should gain the `.selected` modifier class.
  await expect(targetCell).toHaveClass(/selected/);

  // The selected-day-title should no longer be the default
  // "Today's Sessions" because the user picked a non-today cell.
  // Best-effort assertion: the title changed away from the default.
  const titleAfter = await page.locator("#selected-day-title").textContent();
  expect(titleAfter).not.toBeNull();
  expect(titleAfter?.trim()).not.toEqual("Today's Sessions");

  // Empty-state: a different day (one that has no sessions) shows
  // "No sessions completed" in the timeline track. The
  // `_blockExternal` fixture means no remote data lands; the cold-
  // start state is empty.
  await expect(page.locator("#timeline-track")).toContainText("No sessions completed");

  // Bonus: aria-label round-trip — the label string is the date
  // formatted via `format_session_date`; the click is bound to the
  // cell's underlying timestamp, not the label, so the assertion
  // here just confirms the label is non-empty.
  expect(targetLabel).not.toBeNull();
  expect((targetLabel || "").trim()).not.toEqual("");
});
