// There is no in-app "edit tag" affordance today (verified against src/index.html and
// src/managers/tag-manager.js); the plan's edit-step is satisfied by icon selection during
// creation. If an edit affordance is added later, extend this spec.
import { test, expect } from "./fixtures/index.js";
import { gotoTimer, tapTab } from "./fixtures/screens.js";

test("create tag with custom icon, verify persistence, delete tag", async ({ page }) => {
  await gotoTimer(page);

  // Open the tag dropdown
  await page.locator("#timer-status").click();
  await expect(page.locator("#tag-dropdown-menu")).toBeVisible();

  // Open the icon selector and pick an emoji
  await page.locator("#selected-icon-btn").click();
  await expect(page.locator("#icon-selector-dropdown")).toBeVisible();
  await page.locator('.emoji-option[data-icon="🎯"]').click();

  // Type a new tag name and create it
  await page.locator("#new-tag-name").fill("Deep Work");
  await page.locator("#create-tag-btn").click();

  // Assert the new tag appears in #tag-list
  const newTag = page.locator("#tag-list .tag-item").filter({ hasText: "Deep Work" });
  await expect(newTag).toBeVisible();
  await expect(newTag.locator(".tag-item-name")).toHaveText("Deep Work");

  // Close the dropdown by navigating away (Settings), then back (Timer) — verifies in-memory persistence
  await tapTab(page, "Settings");
  await tapTab(page, "Timer");

  // Re-open the dropdown and assert tag is still present
  await page.locator("#timer-status").click();
  await expect(page.locator("#tag-dropdown-menu")).toBeVisible();
  const persistedTag = page.locator("#tag-list .tag-item").filter({ hasText: "Deep Work" });
  await expect(persistedTag).toBeVisible();

  // Delete the tag via its delete icon
  await persistedTag.locator(".tag-item-delete").click();

  // Assert tag is removed from the list
  await expect(persistedTag).toBeHidden();
});
