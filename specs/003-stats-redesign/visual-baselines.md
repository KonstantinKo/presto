# Visual-regression baseline justifications — feature 003

Transient gate artefact for Phase 7 (T027). Removed at PR-cleanup time;
the bullets land verbatim in the PR description.

## Touched surfaces (intentional baseline change)

- `statistics-daily-chromium-linux.png` (NEW): Daily period tab of the
  new Statistics view — 24-hour focus distribution bar chart with 24
  hourly bars; new layout, no prior baseline.
- `statistics-weekly-chromium-linux.png` (NEW, supersedes
  `calendar-chromium-linux.png`): Weekly period tab of the new
  Statistics view — 7-bar focus distribution chart + focus summary
  tiles + tag-usage pie. Supersedes the deleted
  `calendar-chromium-linux.png`.
- `statistics-monthly-chromium-linux.png` (NEW): Monthly period tab of
  the new Statistics view — 28–31-day focus distribution bar chart.
- `statistics-yearly-chromium-linux.png` (NEW): Yearly period tab of
  the new Statistics view — 12-month focus distribution bar chart.
- `daily-chromium-linux.png` (NEW): New Daily drill-down view —
  month-grid + sessions-timeline + off-viewport sessions-history table.
- `tag-manager-chromium-linux.png` (REGENERATED): Tag-picker dropdown
  now shows 12 icon options (3 remixicon + 9 Phosphor; 5 emoji entries
  removed per FR-020 / FR-021).
- `calendar-chromium-linux.png` (DELETED): Calendar view renamed to
  Statistics view with period tabs; the cold-load Weekly frame
  supersedes it.

## Drift-cleanup regen (Principle X — buck stops here)

The non-touched baselines are sidebar-masked (FR-037) so the
four-icons-vs-three-icons swap does NOT cascade. After the mask
landed, however, 10 non-feature-003 baselines still failed visual
comparison against the current render — pre-existing drift from
prior features that prior PRs did not clean up. Per Constitution
Principle X ("no pre-existing errors are tolerated or ignored"),
this PR regenerates them as a one-shot cleanup:

- `timer-chromium-linux.png` (DRIFT CLEANUP): Pre-existing baseline
  drift from features 001 (Leptos migration), 002 (per-session title
  input "What is this session for?"), and #50 (local-only pivot —
  Teams icon removed). Last regenerated 9+ months ago at commit
  `3f1119e`. Sidebar is masked so feature 003's 4-icon layout does
  not show here.
- `update-notification-chromium-linux.png` (DRIFT CLEANUP): Same
  rationale as `timer-*` — same screen with the dark Update banner
  overlay at top.
- `settings-advanced-chromium-linux.png` (DRIFT CLEANUP): Pre-existing
  baseline drift from feature 001 (Leptos migration ~12px layout
  shift). Sidebar masked.
- `settings-automation-chromium-linux.png` (DRIFT CLEANUP): same.
- `settings-general-chromium-linux.png` (DRIFT CLEANUP): same.
- `settings-goals-chromium-linux.png` (DRIFT CLEANUP): same.
- `settings-notifications-chromium-linux.png` (DRIFT CLEANUP): same.
- `settings-shortcuts-chromium-linux.png` (DRIFT CLEANUP): same.
- `settings-theme-chromium-linux.png` (DRIFT CLEANUP): same.
- `settings-updates-chromium-linux.png` (DRIFT CLEANUP): same.
