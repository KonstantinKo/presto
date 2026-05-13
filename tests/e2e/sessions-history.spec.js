import { test, expect } from "./fixtures/index.js";
import { gotoTimer, enableDebugTimers, tapTab } from "./fixtures/screens.js";

test("run debug-mode focus session to completion and verify it appears in calendar history", async ({
  page,
}) => {
  await gotoTimer(page);

  // Enable 3-second debug timers via Settings → Advanced
  await enableDebugTimers(page);
  await tapTab(page, "Timer");

  // Pick a tag (default "Focus" tag)
  await page.locator("#timer-status").click();
  await expect(page.locator("#tag-dropdown-menu")).toBeVisible();
  await page.locator("#tag-list .tag-item").first().click();
  // Toggle the dropdown closed by clicking the trigger again (clicking the
  // timer-status label re-invokes toggleDropdown(), which closes the open menu).
  await page.locator("#timer-status").click();
  await expect(page.locator("#tag-dropdown-menu")).toBeHidden({ timeout: 2000 });

  // Start the timer
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#pause-icon")).toBeVisible();

  // Wait for the 3-second focus session to complete: status changes to "Break"
  // when the mode transitions from focus to break
  await expect(page.locator("#status-text")).toHaveText("Break", { timeout: 12000 });

  // Navigate to the Daily view — Feature 003 moves the mini-calendar
  // grid + sessions-history table from the Calendar (Statistics) view
  // to the new Daily drill-down (FR-019 / A14 / CHK043). The selector
  // strings below (`#calendar-grid`, `#sessions-table-body`, etc.) are
  // preserved across the move.
  await tapTab(page, "Daily");

  // Today's date should be highlighted in the calendar grid (aria-current="date" marks today)
  await expect(page.locator('#calendar-grid [aria-current="date"]')).toBeVisible({ timeout: 5000 });

  // At least one session row should appear in the history table for today
  const rows = page.locator("#sessions-table-body").getByRole("row");
  await expect(rows.first()).toBeVisible({ timeout: 5000 });

  // Click the edit button in the first row to open the session edit modal
  await rows.first().getByRole("button", { name: "Edit session" }).click();
  await expect(page.locator("#session-modal-overlay")).toBeVisible({ timeout: 3000 });
  // Modal shows duration field
  await expect(page.locator("#session-duration")).toBeVisible();

  // --- Save persistence: click Save and verify IPC is called ---
  // We assert on IPC call count + payload rather than row DOM content: Leptos's
  // keyed <For> does not re-render rows whose key (id) is unchanged, so the
  // in-place update is not reliably visible via toContainText. The IPC assert is
  // the correct regression pin for the bug (no bridge call was made before the fix).
  const preSaveCount = await page.evaluate(
    () => window.__E2E_TEST_HARNESS__.state.saveManualSessionsCallCount
  );
  await page.locator("#save-session-btn").click();
  await expect(page.locator("#session-modal-overlay")).toBeHidden();
  const postSaveCount = await page.evaluate(
    () => window.__E2E_TEST_HARNESS__.state.saveManualSessionsCallCount
  );
  // Use > rather than === preSaveCount + 1: the persistence-sink Effect in app.rs
  // may fire a second write for the same mutation; both calls are idempotent and
  // the important invariant is that at least one bridge call was made.
  expect(postSaveCount).toBeGreaterThan(preSaveCount);
  const saveArgs = await page.evaluate(
    () => window.__E2E_TEST_HARNESS__.state.lastSaveManualSessionsArgs
  );
  // Payload must be a non-empty array containing the session record.
  expect(Array.isArray(saveArgs) && saveArgs.length > 0).toBe(true);

  // --- Delete persistence: open modal again and delete the row ---
  await rows.first().getByRole("button", { name: "Edit session" }).click();
  await expect(page.locator("#session-modal-overlay")).toBeVisible({ timeout: 3000 });
  const preDeleteCount = await page.evaluate(
    () => window.__E2E_TEST_HARNESS__.state.saveManualSessionsCallCount
  );
  await page.locator("#delete-session-btn").click();
  await expect(page.locator("#session-modal-overlay")).toBeHidden();
  await expect(page.locator("#sessions-table-body tr")).toHaveCount(0);
  const postDeleteCount = await page.evaluate(
    () => window.__E2E_TEST_HARNESS__.state.saveManualSessionsCallCount
  );
  // Same > convention: idempotent double-write is acceptable; zero-write is the bug.
  expect(postDeleteCount).toBeGreaterThan(preDeleteCount);
  const deleteArgs = await page.evaluate(
    () => window.__E2E_TEST_HARNESS__.state.lastSaveManualSessionsArgs
  );
  expect(deleteArgs).toEqual([]);
});
