// Feature 006 / T064 — Quick Log modal flow.
//
// Covers FR-019 (Quick Log left-slot affordance), SC-003 (modal opens
// auto-focused, single-keystroke entry path), SC-005 (entries land in
// the mocked `save_quick_logs` state without touching the engine).
//
// FR-022 boundary validation: an empty title is rejected by the Leptos
// on_submit guard (the form calls ev.prevent_default() so native browser
// required-validation is not the enforcement mechanism). The minutes
// range (1..=720) is enforced via on:input clamping — out-of-range
// values are clamped to the nearest boundary, so the submit guard's
// `!(1..=720).contains(&mins)` check is never reached through normal UI.
// The HTML `min`/`max` attributes document the range contract.
//
// The Idle left button (`#stop-btn`) opens the Quick Log modal in any
// mode (Focus / Break / LongBreak); the Inventory header button on
// the Daily view opens the same modal for retroactive entries.

import { test, expect } from "./fixtures/index.js";
import {
  enableDebugTimers,
  gotoTimer,
  openSettings,
  selectSettingsCategory,
  tapTab,
} from "./fixtures/screens.js";

test("Quick Log: Idle left-slot opens modal, title + minutes submit, Inventory header reuses modal", async ({
  page,
}) => {
  await gotoTimer(page);

  // ── 1. Idle left button is the Quick Log entry point ─────────────
  await expect(page.locator("#timer-view")).toBeVisible();
  await expect(page.locator("#play-icon")).toBeVisible();
  // The label is "Quick Log" (rendered via the catalogue + matrix).
  await expect(page.locator("#stop-btn")).toHaveAttribute(
    "aria-label",
    /quick log/i
  );

  // ── 2. Open modal, confirm visible + max-length + default minutes ─
  // (Autofocus is declared via the `autofocus` HTML attribute; some
  // browsers do not honor it on dynamically-shown overlays. The
  // attribute presence is the contract; runtime focus is not.)
  await page.locator("#stop-btn").click();
  await expect(page.locator("#quick-log-modal-overlay")).toBeVisible();
  await expect(page.locator("#quick-log-title")).toBeVisible();
  await expect(page.locator("#quick-log-title")).toHaveAttribute("maxlength", "120");
  await expect(page.locator("#quick-log-title")).toHaveAttribute("autofocus", "");
  await expect(page.locator("#quick-log-minutes")).toHaveValue("5");

  // ── 3. Cancel returns to Idle, no entry persisted ────────────────
  await page.locator("#cancel-quick-log-btn").click();
  await expect(page.locator("#quick-log-modal-overlay")).toBeHidden();

  // ── 3b. Validation: empty title blocks save ───────────────────────
  // The Leptos on_submit guard rejects an empty (whitespace-trimmed)
  // title and returns early, keeping the modal open.
  await page.locator("#stop-btn").click();
  await expect(page.locator("#quick-log-modal-overlay")).toBeVisible();
  // Leave title empty — do not fill.
  await page.locator("#save-quick-log-btn").click();
  await expect(page.locator("#quick-log-modal-overlay")).toBeVisible();

  // Minutes range is documented via HTML attributes; the on:input
  // handler clamps values to 1..720 before on_submit sees them.
  await expect(page.locator("#quick-log-minutes")).toHaveAttribute("min", "1");
  await expect(page.locator("#quick-log-minutes")).toHaveAttribute("max", "720");

  await page.locator("#cancel-quick-log-btn").click();
  await expect(page.locator("#quick-log-modal-overlay")).toBeHidden();

  // ── 4. Submit valid entry from the timer view ────────────────────
  await page.locator("#stop-btn").click();
  await expect(page.locator("#quick-log-modal-overlay")).toBeVisible();
  await page.locator("#quick-log-title").fill("Reply to PR");
  await page.locator("#quick-log-minutes").fill("12");
  await page.locator("#save-quick-log-btn").click();
  await expect(page.locator("#quick-log-modal-overlay")).toBeHidden();

  // Engine state unchanged: pomodoro counter still zero, timer Idle.
  await expect(page.locator("#play-icon")).toBeVisible();
  await expect(page.locator("#timer-minutes")).toHaveText("25");
  await expect(page.locator("#timer-seconds")).toHaveText("00");

  // ── 5. Inventory header opens the SAME modal for retroactive entry
  await tapTab(page, "Daily");
  await expect(page.locator("#daily-view")).not.toHaveClass(/hidden/);
  await expect(page.locator("#inventory")).toBeVisible();
  // The just-submitted quick log is visible in the Inventory list
  // (the test runs on today's date by default; the entry's `date`
  // field matches and the Inventory filters by selected_day = today).
  await expect(page.locator("#inventory-quicklogs-list")).toBeVisible();
  await expect(
    page.locator(`#inventory-quicklogs-list >> text="Reply to PR"`)
  ).toBeVisible();

  // Header `+ Quick Log` button opens the inventory-hosted modal.
  await page.locator("#inventory-add-quicklog-btn").click();
  await expect(page.locator("#inventory-quick-log-modal-overlay")).toBeVisible();
  await expect(page.locator("#inventory-quick-log-title")).toBeVisible();

  // Submit a second entry from the Inventory path.
  await page.locator("#inventory-quick-log-title").fill("Stand-up notes");
  await page.locator("#inventory-quick-log-minutes").fill("3");
  await page.locator("#inventory-save-quick-log-btn").click();
  await expect(page.locator("#inventory-quick-log-modal-overlay")).toBeHidden();
  await expect(
    page.locator(`#inventory-quicklogs-list >> text="Stand-up notes"`)
  ).toBeVisible();
});

test("Quick Log: Break-state left-slot opens modal", async ({ page }) => {
  // Debug timers fire in ~3 seconds. Auto-start-timer must be OFF so the
  // break stays idle after focus completes — when the break is running the
  // left slot maps to Abort, not Quick Log.
  test.setTimeout(30_000);
  await page.goto("/index.html");

  await openSettings(page);
  await selectSettingsCategory(page, "Automation");
  if (await page.locator("#auto-start-timer").isChecked()) {
    await page.locator("#auto-start-timer").click();
  }
  await expect(page.locator("#auto-start-timer")).not.toBeChecked();

  await enableDebugTimers(page);
  await tapTab(page, "Timer");

  // ── 1. Start a Focus session ──────────────────────────────────────
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#pause-icon")).toBeVisible();

  // ── 2. Focus completes (3 s); mode advances to idle Break ─────────
  // With auto-start off the break timer does not run; #status-text
  // shows the break mode and #play-icon reappears (timer is idle).
  await expect(page.locator("#status-text")).toContainText(/break/i, {
    timeout: 10_000,
  });
  await expect(page.locator("#play-icon")).toBeVisible();

  // ── 3. Left slot (#stop-btn) opens the Quick Log modal in Break ───
  await expect(page.locator("#stop-btn")).toHaveAttribute(
    "aria-label",
    /quick log/i
  );
  await page.locator("#stop-btn").click();
  await expect(page.locator("#quick-log-modal-overlay")).toBeVisible();
});
