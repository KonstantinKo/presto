// Feature 006 / T067 — ✕ Abort button + AG-2 regression
// (no auto-restart after Abort).
//
// Covers FR-017 (Abort returns to Idle, title persists, no count, no
// advance), SC-010 (Abort suppresses the auto-restart UI gate even
// when `notifications.auto_start_timer = true`).

import { test, expect } from "./fixtures/index.js";
import { gotoTimer, openSettings, selectSettingsCategory, tapTab } from "./fixtures/screens.js";

test("Abort: Running + Paused both clear to Idle preserving title, no auto-restart fires", async ({
  page,
}) => {
  await page.goto("/index.html");

  // ── 0. Confirm auto-start defaults ON (matches the JS-era default).
  // The AG-2 regression scenario requires `auto_start_timer = true` —
  // we only need to assert the default, not toggle it. If the default
  // ever flips, the assertion guards us against a silent regression.
  await openSettings(page);
  await selectSettingsCategory(page, "Automation");
  await expect(page.locator("#auto-start-timer")).toBeChecked();

  await tapTab(page, "Timer");

  // ── 1. Set a title for the upcoming session ──────────────────────
  await page.locator("#session-title-input").fill("Resume me later");
  await expect(page.locator("#session-title-input")).toHaveValue("Resume me later");

  // ── 2. Start → wait a tick → ✕ Abort from Running ────────────────
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#pause-icon")).toBeVisible();
  await expect(page.locator("#timer-seconds")).not.toHaveText("00", { timeout: 5000 });

  // Left slot is now ✕ Abort.
  await expect(page.locator("#stop-btn")).toHaveAttribute("aria-label", /abort/i);
  await page.locator("#stop-btn").click();

  // Returns to Idle in the same mode (Focus).
  await expect(page.locator("#play-icon")).toBeVisible();
  await expect(page.locator("#status-text")).toContainText(/focus/i);
  // Title preserved per FR-017.
  await expect(page.locator("#session-title-input")).toHaveValue("Resume me later");

  // AG-2: NO auto-restart countdown should appear.
  // The autostart UI overlay is gated on a PomodoroCompleted event;
  // Abort emits only SessionAborted so the overlay must NOT fire.
  // The assertion settles within ~1500ms — long enough to catch the
  // countdown if it would have surfaced on the next tick.
  await expect(page.locator(".autostart-overlay")).toBeHidden({ timeout: 1500 });

  // ── 3. Repeat from Paused ────────────────────────────────────────
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#pause-icon")).toBeVisible();
  await expect(page.locator("#timer-seconds")).not.toHaveText("00", { timeout: 5000 });

  // Pause.
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#play-icon")).toBeVisible();

  // Left slot still ✕ Abort in Paused.
  await expect(page.locator("#stop-btn")).toHaveAttribute("aria-label", /abort/i);
  await page.locator("#stop-btn").click();

  await expect(page.locator("#play-icon")).toBeVisible();
  // Title still preserved.
  await expect(page.locator("#session-title-input")).toHaveValue("Resume me later");
  // No auto-restart countdown.
  await expect(page.locator(".autostart-overlay")).toBeHidden({ timeout: 1500 });
});
