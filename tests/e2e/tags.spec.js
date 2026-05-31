// There is no in-app "edit tag" affordance today (verified against src/index.html and
// src/managers/tag-manager.js); the plan's edit-step is satisfied by icon selection during
// creation. If an edit affordance is added later, extend this spec.
import { test, expect } from "./fixtures/index.js";
import { gotoTimer, tapTab } from "./fixtures/screens.js";

test("create tag with custom icon, verify persistence, delete tag", async ({ page }) => {
  await gotoTimer(page);

  await page.locator("#timer-status").click();
  await expect(page.locator("#tag-dropdown-menu")).toBeVisible();

  // Open the icon selector and pick a Phosphor glyph.
  // Feature 003 (FR-020): the 5 legacy emoji entries were removed
  // from the picker; the previous selector `.emoji-option[data-icon="🎯"]`
  // is no longer present. `ph-cloud` is one of the 9 new Phosphor
  // entries — `.icon-option[data-icon="ph-cloud"]` is its host.
  await page.locator("#selected-icon-btn").click();
  await expect(page.locator("#icon-selector-dropdown")).toBeVisible();
  await page.locator('.icon-option[data-icon="ph-cloud"]').click();

  await page.locator("#new-tag-name").fill("Deep Work");
  await page.locator("#create-tag-btn").click();

  const newTag = page.locator('#tag-list [role="listitem"]').filter({ hasText: "Deep Work" });
  await expect(newTag).toBeVisible();
  await expect(newTag).toContainText("Deep Work");

  // Close the dropdown by navigating away (Settings), then back (Timer) — verifies in-memory persistence
  await tapTab(page, "Settings");
  await tapTab(page, "Timer");

  await page.locator("#timer-status").click();
  await expect(page.locator("#tag-dropdown-menu")).toBeVisible();
  const persistedTag = page.locator('#tag-list [role="listitem"]').filter({ hasText: "Deep Work" });
  await expect(persistedTag).toBeVisible();

  await persistedTag.getByRole("button", { name: /delete deep work tag/i }).click();

  await expect(persistedTag).toBeHidden();
});
