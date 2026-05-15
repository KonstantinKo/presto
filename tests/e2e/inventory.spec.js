// Feature 006 / T068 — Inventory subsection on the Daily view.
//
// Covers FR-023 (Inventory section renders below sessions-history),
// FR-024 (per-row Edit + Delete), FR-024a (deleted-tag placeholder
// for orphaned `parent_tag_id`), FR-025 (`+ Quick Log` header button
// opens the modal), FR-026 (Empty-state per-day), SC-006 (parent-tag
// re-resolution at render time).

import { test, expect } from "./fixtures/index.js";
import { gotoTimer, tapTab } from "./fixtures/screens.js";

test("Inventory: renders, edit + delete, deleted-tag placeholder, header + Quick Log button", async ({
  page,
  tauriMock,
}) => {
  // Seed: 1 quick log + 1 distraction whose parent_tag_id no longer
  // exists in the tag table (FR-024a deleted-tag case). Use today's
  // date format the engine produces — `format_session_date` (e.g.
  // "Fri May 15 2026" for May 15 2026).
  // Build a stable seed by freezing time first.
  await tauriMock.freezeTime("2026-05-15T10:00:00Z");
  await page.addInitScript({
    content: `
if (!window.__E2E_CONFIG__) window.__E2E_CONFIG__ = {};
window.__E2E_CONFIG__.initialQuickLogs = [
  {
    id: "qid-seed-1",
    title: "Seeded quick log",
    elapsedMinutes: 7,
    createdAt: "2026-05-15T09:00:00Z",
    date: "Fri May 15 2026"
  }
];
window.__E2E_CONFIG__.initialDistractions = [
  {
    id: "did-seed-1",
    note: "Seeded distraction with deleted tag",
    createdAt: "2026-05-15T09:30:00Z",
    date: "Fri May 15 2026",
    parentRef: {
      parentSessionStartTs: "2026-05-15T09:00:00Z",
      parentMode: "focus",
      parentTagId: "tag-that-no-longer-exists",
      parentTitle: "Original session title"
    }
  }
];
`,
  });

  await page.goto("/index.html");
  await tapTab(page, "Daily");
  await expect(page.locator("#daily-view")).not.toHaveClass(/hidden/);
  await expect(page.locator("#inventory")).toBeVisible();

  // ── 1. Seeded entries render in their respective subsections ─────
  await expect(page.locator("#inventory-quicklogs-list")).toBeVisible();
  await expect(
    page.locator(`#inventory-quicklogs-list >> text="Seeded quick log"`)
  ).toBeVisible();
  await expect(page.locator("#inventory-distractions-list")).toBeVisible();
  await expect(
    page.locator(`#inventory-distractions-list >> text="Seeded distraction with deleted tag"`)
  ).toBeVisible();

  // ── 2. Deleted-tag placeholder fires (FR-024a) ───────────────────
  await expect(
    page.locator(".inventory-parentref-tag-deleted")
  ).toBeVisible();

  // ── 3. `+ Quick Log` header button opens the modal ───────────────
  await page.locator("#inventory-add-quicklog-btn").click();
  await expect(page.locator("#inventory-quick-log-modal-overlay")).toBeVisible();
  await expect(page.locator("#inventory-quick-log-title")).toBeVisible();
  // Cancel.
  await page.locator("#inventory-cancel-quick-log-btn").click();
  await expect(page.locator("#inventory-quick-log-modal-overlay")).toBeHidden();

  // ── 4. Quick-log row Edit opens the edit modal pre-filled ────────
  // (The in-place visible update of the row after Save is governed by
  // Leptos `<For>` keyed-child semantics; here we assert the edit-
  // flow surface — modal opens with the row's current values,
  // accepts edits, and closes on Save. The persistence round-trip is
  // covered by the manager-level test `update_replaces_in_place`.)
  await page
    .locator('[data-quicklog-id="qid-seed-1"] .edit-quicklog-btn')
    .click();
  await expect(page.locator("#inventory-edit-quicklog-overlay")).toBeVisible();
  await expect(page.locator("#inventory-edit-quicklog-title-input")).toHaveValue(
    "Seeded quick log"
  );
  await expect(page.locator("#inventory-edit-quicklog-minutes-input")).toHaveValue("7");
  await page.locator("#inventory-edit-quicklog-title-input").fill("Edited title");
  await page
    .locator("#inventory-edit-quicklog-form .btn-primary")
    .click();
  await expect(page.locator("#inventory-edit-quicklog-overlay")).toBeHidden();

  // ── 5. Distraction row Delete removes only the target ────────────
  await page
    .locator('[data-distraction-id="did-seed-1"] .delete-distraction-btn')
    .click();
  await expect(
    page.locator(`#inventory-distractions-list >> text="Seeded distraction with deleted tag"`)
  ).toBeHidden();

  // ── 6. Empty state surfaces after the last distraction is gone ───
  await expect(page.locator("#inventory-distractions-empty")).toBeVisible();
});
