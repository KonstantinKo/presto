import { test, expect } from "./fixtures/index.js";

// Feature 003 Bundle D (SC-013): the control-button tooltip's CSS rule
// triggers on both `:hover` AND `:focus-visible`, so keyboard users see
// the tooltip too. The tooltip text source-of-truth is the
// `data-tooltip` attribute on each control button.

test("control-button tooltips expose data-tooltip + react to keyboard focus", async ({ page }) => {
  await page.goto("/index.html");

  // Wait for the timer view to settle on cold-load.
  await page.waitForSelector("#timer-minutes", { state: "visible", timeout: 15000 });

  // FR-027 — Focus mode: stop-btn shows "Reset" terse + "Reset timer"
  // verbose; aria-label == title.
  const stopBtn = page.locator("#stop-btn");
  await expect(stopBtn).toHaveAttribute("data-tooltip", "Reset");
  await expect(stopBtn).toHaveAttribute("aria-label", "Reset timer");
  await expect(stopBtn).toHaveAttribute("title", "Reset timer");

  // FR-028 — idle play/pause: terse "Start", verbose stable.
  const playPauseBtn = page.locator("#play-pause-btn");
  await expect(playPauseBtn).toHaveAttribute("data-tooltip", "Start");
  await expect(playPauseBtn).toHaveAttribute("aria-label", "Start or pause timer");
  await expect(playPauseBtn).toHaveAttribute("title", "Start or pause timer");

  // FR-029 — skip: terse "Skip session", verbose "Skip current
  // session and advance to the next phase". Spec A11 + SC-004 +
  // contracts §3 require these to be DISTINCT catalogue keys
  // (`timer.ctrl_skip_session` for the terse tooltip, and
  // `timer.ctrl_skip_session_aria` for the verbose aria-label / title)
  // so a copy edit on one cannot silently collapse the pair (CHK041
  // drift-impossibility).
  const skipBtn = page.locator("#skip-btn");
  await expect(skipBtn).toHaveAttribute("data-tooltip", "Skip session");
  await expect(skipBtn).toHaveAttribute(
    "aria-label",
    "Skip current session and advance to the next phase",
  );
  await expect(skipBtn).toHaveAttribute(
    "title",
    "Skip current session and advance to the next phase",
  );

  // SC-013 — tooltip shows on both `:hover` AND `:focus-visible` per
  // FR-030. We probe the `::after` pseudo-element's `opacity` via
  // `getComputedStyle(el, '::after')`.
  //
  // (a) Hover path: `Locator.hover()` reliably triggers `:hover` and
  // is the canonical Playwright affordance for asserting hover
  // styles. The CSS rule fires both hover and focus-visible from the
  // same declaration, so verifying one branch + greping for the
  // selector pinning the other is the practical Playwright contract
  // here (the focus-visible heuristic is browser-internal and not
  // reliably triggerable from automation).
  await stopBtn.hover();
  // The 150 ms CSS transition completes before we read; Playwright
  // serialises evaluate() after the prior action so by the time the
  // getComputedStyle runs the transition is settled.
  await expect
    .poll(
      async () =>
        parseFloat(
          await stopBtn.evaluate((el) => getComputedStyle(el, "::after").opacity)
        ),
      { timeout: 2000 }
    )
    .toBeGreaterThan(0.9);

  // (b) Focus path: walk Tab keys until `#stop-btn` is the active
  // element so `:focus-visible` heuristic accepts the activation.
  await page.keyboard.press("Tab");
  for (let i = 0; i < 15; i++) {
    const focusedId = await page.evaluate(() => document.activeElement?.id ?? "");
    if (focusedId === "stop-btn") break;
    await page.keyboard.press("Tab");
  }
  await expect(stopBtn).toBeFocused();

  // FR-028 — pressing the play button updates the terse tooltip to
  // "Pause" while the verbose label stays "Start or pause timer"
  // (CHK041 invariant: aria-label decoupled from data-tooltip).
  await playPauseBtn.click();
  await expect(playPauseBtn).toHaveAttribute("data-tooltip", "Pause");
  await expect(playPauseBtn).toHaveAttribute("aria-label", "Start or pause timer");

  // Pause it again — terse becomes "Resume".
  await playPauseBtn.click();
  await expect(playPauseBtn).toHaveAttribute("data-tooltip", "Resume");
  await expect(playPauseBtn).toHaveAttribute("aria-label", "Start or pause timer");
});
