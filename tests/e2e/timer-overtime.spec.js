// Feature 007 — Overtime button treatment.
//
// Covers the user-facing surface of the wrap-up nudge:
//   * triple-Complete dispatch (FR-007, FR-008, SC-002)
//   * orange tint + CTA visibility (FR-005, FR-010, SC-001, SC-006, SC-008)
//   * a11y removal of the outer slots (FR-014, FR-015, FR-016, SC-003, SC-004)
//   * exit via Complete clears the treatment (FR-024, SC-009)
//   * exit via Abort keyboard shortcut clears the treatment (FR-021, SC-005)
//   * pause during overtime reverts to the Paused matrix (FR-022, FR-023)
//
// Engine-level coverage for branch B.2 (continuous-mode overtime
// complete) lives in feature 006's RED tests under
// `src/src/engine/timer.rs`. This spec proves the UI wiring is
// correct around that engine path.
//
// Mock harness: continuous-mode overtime is reached via debug-mode's
// 3-second focus duration so the test crosses zero in a few wall-clock
// seconds. Each block re-enters overtime so the three triggers can be
// exercised independently.

import { test, expect } from "./fixtures/index.js";
import {
  enableDebugTimers,
  gotoTimer,
  openSettings,
  selectSettingsCategory,
  tapTab,
} from "./fixtures/screens.js";

/**
 * Configure the timer for continuous-mode debug operation: 3-second
 * debug timers + allow-continuous-sessions on + auto-start-timer off
 * (so a post-overtime Complete doesn't immediately roll into the next
 * mode while we're asserting).
 */
async function enableContinuousDebugMode(page) {
  await openSettings(page);
  await selectSettingsCategory(page, "Automation");
  if (await page.locator("#auto-start-timer").isChecked()) {
    await page.locator("#auto-start-timer").click();
  }
  await expect(page.locator("#auto-start-timer")).not.toBeChecked();
  if (!(await page.locator("#allow-continuous-sessions").isChecked())) {
    await page.locator("#allow-continuous-sessions").click();
  }
  await expect(page.locator("#allow-continuous-sessions")).toBeChecked();
  await enableDebugTimers(page);
  await tapTab(page, "Timer");
}

/**
 * Start a focus session and wait until the timer crosses zero into
 * overtime. Locks the wait on the `.overtime-cta.visible` selector —
 * Feature 007 ties the CTA's visibility to the same `(Running,
 * is_overtime)` predicate as the button-row orange treatment, so a
 * visible CTA proves the matrix is in overtime mode.
 */
async function enterOvertime(page) {
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#pause-icon")).toBeVisible();
  // Wait for the 3-second focus to cross zero; the CTA appears the
  // same UI tick the countdown flips to its overtime colour
  // (SC-001 / SC-006).
  await expect(page.locator(".overtime-cta.visible")).toBeVisible({
    timeout: 10_000,
  });
}

test("Overtime treatment: orange tint + CTA + a11y + triple-Complete + Abort exit + Pause revert", async ({
  page,
}) => {
  test.setTimeout(180_000);
  await page.goto("/index.html");
  await enableContinuousDebugMode(page);

  // ── Block 1. T028 — orange tint visible + CTA visible ─────────────
  // Enter overtime and assert all three control-btn elements carry
  // the `overtime` class; the CTA renders the localised
  // `timer.overtime_cta` string (default EN = "Wrap it up!"); the
  // timer-container itself still carries the `overtime` class so the
  // countdown pulses in the same warning colour (FR-005, FR-006).
  await enterOvertime(page);
  await expect(page.locator("#stop-btn")).toHaveClass(/overtime/);
  await expect(page.locator("#play-pause-btn")).toHaveClass(/overtime/);
  await expect(page.locator("#skip-btn")).toHaveClass(/overtime/);
  await expect(page.locator(".overtime-cta")).toHaveText("Wrap it up!");
  await expect(page.locator(".timer-container.overtime")).toBeVisible();

  // ── Block 2. T029 — a11y removal of outer slots ───────────────────
  // FR-014 / FR-015: the outer two slots are hidden from the
  // accessibility tree and excluded from the tab order during
  // overtime. The center slot keeps its normal tab order and reads
  // the ctrl_complete aria label (FR-016, SC-003).
  await expect(page.locator("#stop-btn")).toHaveAttribute("aria-hidden", "true");
  await expect(page.locator("#stop-btn")).toHaveAttribute("tabindex", "-1");
  await expect(page.locator("#skip-btn")).toHaveAttribute("aria-hidden", "true");
  await expect(page.locator("#skip-btn")).toHaveAttribute("tabindex", "-1");
  await expect(page.locator("#play-pause-btn")).toHaveAttribute(
    "aria-label",
    /complete/i
  );

  // ── Block 3. T027 + T030 — triple-Complete dispatch (left slot) ───
  // Click the left ghost slot. The button is `aria-hidden` so
  // pointer events still dispatch; the click maps to on_complete and
  // the focus session ends → break begins. After the click, the
  // overtime treatment is gone (FR-024, SC-009).
  await page.locator("#stop-btn").click({ force: true });
  await expect(page.locator("#status-text")).toContainText(/break/i, {
    timeout: 5_000,
  });
  await expect(page.locator(".overtime-cta.visible")).toBeHidden();
  await expect(page.locator("#stop-btn")).not.toHaveClass(/overtime/);
  await expect(page.locator("#play-pause-btn")).not.toHaveClass(/overtime/);
  await expect(page.locator("#skip-btn")).not.toHaveClass(/overtime/);

  // Return to a fresh Focus session for the next exercise. Skip
  // forward until we land back on Focus.
  for (let i = 0; i < 4; i++) {
    const text = (await page.locator("#status-text").textContent()) || "";
    if (/focus/i.test(text)) break;
    await page.locator("#skip-btn").click();
  }
  await expect(page.locator("#status-text")).toContainText(/focus/i);

  // ── Block 4. T027 — triple-Complete dispatch (right slot) ─────────
  await enterOvertime(page);
  await page.locator("#skip-btn").click({ force: true });
  await expect(page.locator("#status-text")).toContainText(/break/i, {
    timeout: 5_000,
  });
  await expect(page.locator(".overtime-cta.visible")).toBeHidden();

  for (let i = 0; i < 4; i++) {
    const text = (await page.locator("#status-text").textContent()) || "";
    if (/focus/i.test(text)) break;
    await page.locator("#skip-btn").click();
  }
  await expect(page.locator("#status-text")).toContainText(/focus/i);

  // ── Block 5. T027 — triple-Complete dispatch (center slot) ────────
  await enterOvertime(page);
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#status-text")).toContainText(/break/i, {
    timeout: 5_000,
  });
  await expect(page.locator(".overtime-cta.visible")).toBeHidden();

  for (let i = 0; i < 4; i++) {
    const text = (await page.locator("#status-text").textContent()) || "";
    if (/focus/i.test(text)) break;
    await page.locator("#skip-btn").click();
  }
  await expect(page.locator("#status-text")).toContainText(/focus/i);

  // ── Block 6. T031 — Abort keyboard shortcut clears treatment ──────
  // Emit "abort" on the global-shortcut event channel via the mock
  // and assert the engine returns to idle in the current focus mode
  // (NOT advanced to break — abort discards), the overtime treatment
  // is gone, and the CTA is gone (FR-021, SC-005, SC-009).
  await enterOvertime(page);
  await page.evaluate(() => {
    window.__TAURI__.event.emit("global-shortcut", "abort");
  });
  await expect(page.locator(".overtime-cta.visible")).toBeHidden({
    timeout: 5_000,
  });
  await expect(page.locator("#status-text")).toContainText(/focus/i);
  await expect(page.locator("#play-icon")).toBeVisible();
  await expect(page.locator("#stop-btn")).not.toHaveClass(/overtime/);

  // ── Block 7. T032 — Pause during overtime reverts to Paused matrix
  // Re-enter overtime, fire the start-stop shortcut (which mirrors
  // the play-pause click), and assert the matrix flips back to the
  // Paused trio (`Abort | Resume | Complete`) with the CTA hidden
  // and the .overtime button classes gone (FR-022, FR-023).
  await enterOvertime(page);
  await page.evaluate(() => {
    window.__TAURI__.event.emit("global-shortcut", "start-stop");
  });
  await expect(page.locator("#play-icon")).toBeVisible({ timeout: 5_000 });
  await expect(page.locator(".overtime-cta.visible")).toBeHidden();
  await expect(page.locator("#stop-btn")).not.toHaveClass(/overtime/);
  await expect(page.locator("#play-pause-btn")).not.toHaveClass(/overtime/);
  await expect(page.locator("#skip-btn")).not.toHaveClass(/overtime/);
  // Right slot now reads ✓ Complete (Paused matrix).
  await expect(page.locator("#skip-btn")).toHaveAttribute(
    "aria-label",
    /complete/i
  );
  // Left slot reads ✕ Abort (Paused matrix).
  await expect(page.locator("#stop-btn")).toHaveAttribute(
    "aria-label",
    /abort/i
  );

  // Resume — overtime treatment returns.
  await page.evaluate(() => {
    window.__TAURI__.event.emit("global-shortcut", "start-stop");
  });
  await expect(page.locator(".overtime-cta.visible")).toBeVisible({
    timeout: 5_000,
  });
  await expect(page.locator("#stop-btn")).toHaveClass(/overtime/);
});
