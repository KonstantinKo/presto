// Feature 006 / SC-006 — Multi-distraction-per-session flow.
//
// Two distractions captured within the same focus session must both
// appear in Inventory and share the same parent-session back-link
// (SC-006). The parent ref is snapshotted at modal-open time so both
// entries reference the session that was running when each was captured.

import { test, expect } from "./fixtures/index.js";
import { gotoTimer, tapTab } from "./fixtures/screens.js";

test("Distraction: two distractions in one session both appear in Inventory with shared parent ref (SC-006)", async ({
  page,
}) => {
  await gotoTimer(page);

  // ── 1. Start a Focus session ─────────────────────────────────────
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#pause-icon")).toBeVisible();

  // Wait for at least 1 s so the engine has a valid session-start timestamp.
  await expect(page.locator("#timer-seconds")).not.toHaveText("00", {
    timeout: 5000,
  });

  // ── 2. Open modal and submit first distraction ───────────────────
  await page.locator("#skip-btn").click();
  await expect(page.locator("#distraction-modal-overlay")).toBeVisible();
  await page.locator("#distraction-note").fill("first distraction");
  await page.keyboard.press("Enter");
  await expect(page.locator("#distraction-modal-overlay")).toBeHidden();

  // ── 3. Open modal again and submit second distraction ────────────
  await page.locator("#skip-btn").click();
  await expect(page.locator("#distraction-modal-overlay")).toBeVisible();
  await page.locator("#distraction-note").fill("second distraction");
  await page.keyboard.press("Enter");
  await expect(page.locator("#distraction-modal-overlay")).toBeHidden();

  // Engine still running after both captures (FR-035).
  await expect(page.locator("#pause-icon")).toBeVisible();

  // ── 4. Navigate to Daily (Inventory) ─────────────────────────────
  // Pause first so the Daily nav doesn't drag the timer's wall-clock
  // settle into the assertions below.
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#play-icon")).toBeVisible();

  await tapTab(page, "Daily");
  await expect(page.locator("#daily-view")).not.toHaveClass(/hidden/);

  // ── 5. Both distraction entries are visible ───────────────────────
  await expect(page.locator("#inventory-distractions-list")).toBeVisible();
  await expect(
    page.locator(`#inventory-distractions-list >> text="first distraction"`)
  ).toBeVisible();
  await expect(
    page.locator(`#inventory-distractions-list >> text="second distraction"`)
  ).toBeVisible();

  // ── 6. Both share the same parent session reference (SC-006) ──────
  // Distractions captured from a running session render with a clickable
  // parent-ref back-link (.inventory-parentref-clickable). Exactly two
  // such links confirms both entries are anchored to the same session.
  // The text content of both links must be identical — same session ref.
  const parentRefs = page.locator(
    "#inventory-distractions-list .inventory-parentref-clickable"
  );
  await expect(parentRefs).toHaveCount(2);
  const ref0 = await parentRefs.nth(0).innerText();
  const ref1 = await parentRefs.nth(1).innerText();
  expect(ref0).toBe(ref1);
});
