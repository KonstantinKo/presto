// Feature 006 / T065 — Distraction modal flow.
//
// Covers FR-020 (Running right-slot opens Distraction modal),
// FR-035 (modal is a pure side channel — never touches the engine),
// SC-002 (single-keystroke capture), SC-006 (parent-session ref
// snapshotted at modal-open time).
//
// The modal opens auto-focused; Enter submits; Escape cancels without
// writing. The timer keeps ticking through both flows.
//
import { test, expect } from "./fixtures/index.js";
import { gotoTimer, tapTab } from "./fixtures/screens.js";

test("Distraction: Running right-slot opens modal, Enter submits, Escape cancels, engine untouched", async ({
  page,
}) => {
  await gotoTimer(page);

  // ── 1. Start a Focus session ─────────────────────────────────────
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#pause-icon")).toBeVisible();

  // Right slot is now "Note Distraction" (the Running-state label).
  await expect(page.locator("#skip-btn")).toHaveAttribute(
    "aria-label",
    /distraction/i
  );

  // ── 2. Wait until the timer has accumulated at least 1s. ─────────
  // 5 s budget absorbs parallel-runner load (timer-complete.spec.js
  // runs 44 s concurrently and can starve the 1 Hz WASM interval).
  await expect(page.locator("#timer-seconds")).not.toHaveText("00", {
    timeout: 5000,
  });

  // Capture elapsed time before opening the modal.
  const secondsBefore = await page.locator("#timer-seconds").textContent();

  // ── 3. Escape cancels without writing ────────────────────────────
  // (Autofocus is declared via the `autofocus` HTML attribute; the
  // attribute presence is the contract — runtime focus is browser-
  // dependent for dynamically-shown overlays.)
  await page.locator("#skip-btn").click();
  await expect(page.locator("#distraction-modal-overlay")).toBeVisible();
  await expect(page.locator("#distraction-note")).toBeVisible();
  await expect(page.locator("#distraction-note")).toHaveAttribute("autofocus", "");
  await page.locator("#distraction-note").fill("ignored typing");
  await page.keyboard.press("Escape");
  await expect(page.locator("#distraction-modal-overlay")).toBeHidden();

  // Engine still running.
  await expect(page.locator("#pause-icon")).toBeVisible();

  // ── 4. Open modal again and submit via Enter ─────────────────────
  await page.locator("#skip-btn").click();
  await expect(page.locator("#distraction-modal-overlay")).toBeVisible();
  await page.locator("#distraction-note").fill("phone buzz");
  await page.keyboard.press("Enter");
  await expect(page.locator("#distraction-modal-overlay")).toBeHidden();

  // ── 5. Timer still Running — engine never touched (FR-035) ───────
  await expect(page.locator("#pause-icon")).toBeVisible();
  // The elapsed seconds counter has either advanced or stayed pinned
  // on its prior value within the same tick — what must NOT happen is
  // a reset to 00 (which would mean the modal touched the engine).
  await expect(page.locator("#timer-seconds")).not.toHaveText("00");

  // ── 6. Navigate to Daily and confirm the entry persisted ─────────
  // Pause first so the Daily nav doesn't drag the timer's wall-clock
  // settle into the assertions below.
  await page.locator("#play-pause-btn").click();
  await expect(page.locator("#play-icon")).toBeVisible();

  await tapTab(page, "Daily");
  await expect(page.locator("#daily-view")).not.toHaveClass(/hidden/);
  await expect(page.locator("#inventory-distractions-list")).toBeVisible();
  await expect(
    page.locator(`#inventory-distractions-list >> text="phone buzz"`)
  ).toBeVisible();
  // The "ignored typing" entry must NOT be persisted (Escape cancels).
  await expect(
    page.locator(`#inventory-distractions-list >> text="ignored typing"`)
  ).toBeHidden();
});

