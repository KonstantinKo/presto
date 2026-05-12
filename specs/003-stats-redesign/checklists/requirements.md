# Spec Quality Checklist: Statistics Period Tabs, Daily Drill-Down, Phosphor Tag Icons, Control-Button Tooltips

**Purpose**: Validate that `spec.md` for feature `003-stats-redesign` is complete, internally consistent, and ready for `/speckit-plan`.
**Created**: 2026-05-12
**Feature**: [spec.md](../spec.md)

## Coverage

- [x] CHK001 Every bundle (A=Statistics period tabs, B=Daily view, C=Phosphor icons, D=tooltips, E=PeakFocusTime cut-line) has at least one User Story and a Functional Requirements subsection.
- [x] CHK002 Bundle A has FRs covering: rename of CalendarView, closed Period sum type, default Weekly tab, reusable BarChart component, per-period max_scale + floor, minimum-bar-height floor, per-period navigator selectors, cursor reset on swap, preserved Weekly selectors, tag-usage pie scope, no-new-Tauri-command constraint.
- [x] CHK003 Bundle B has FRs covering: new module + sidebar entry, view container ID, two-column layout, dual cursor state seeding, click-to-select with highlight, month-roll day-clamp, Sundays-first header + today's aria-current, removal of the Statistics view's right-column block.
- [x] CHK004 Bundle C has FRs covering: ICON_OPTIONS replacement (3 remix + 9 Phosphor, no emoji), Phosphor glyph list, vendored-font policy, renderer wrapper class, legacy emoji renderer fallback, typed-prefix dispatch enum, wasm-bindgen-test coverage for the three dispatch branches.
- [x] CHK005 Bundle D has FRs covering: data-tooltip attribute on three control buttons, single signal source for all three attributes, per-state tooltip strings (Reset/Undo, Start/Pause/Resume, Skip session), :hover + :focus-visible CSS rule, attribute-equality test.
- [x] CHK006 Bundle E (optional) has FRs that are explicitly marked OPTIONAL and a defer-to-follow-up clause; Bundles A–D are not blocked by Bundle E's absence.
- [x] CHK007 Cross-cutting FRs cover: no-border-right / no-separator sidebar constraint, visual-regression baseline scope, English-only strings, no new dependencies, no new Tauri commands, no fork attribution, clippy-pedantic posture.

## Constitutional anchors

- [x] CHK008 Principle I (The Timer Is Sacred) explicitly cited and out-of-scope-marked (FR-045): no engine changes.
- [x] CHK009 Principle II (Local-Only) cited on Bundle A (no new network egress) and Bundle B (reads existing commands).
- [x] CHK010 Principle III (Type Safety Over Defensive Code) cited on: Period enum (Bundle A), IconClass enum (Bundle C), and the derived tooltip signal (Bundle D).
- [x] CHK011 Principle IV (Visual Regression Is The UI Contract) cited on Story 5 and the FR-043 / FR-044 baseline-scope block.
- [x] CHK012 Principle V (Test-First For Stateful Engines) explicitly stated as NOT applicable in Assumption A1 — this is a UI-only rework.
- [x] CHK013 Principle VI (The Tauri Boundary Is Stable) cited on FR-011 and FR-040: no new commands, no new IPC.
- [x] CHK014 Principle VIII (Spec-Driven Feature Flow): multi-file UI work; spec is the artefact this checklist validates.
- [x] CHK015 Principle IX (Lock Files Are First-Class) cited on FR-022 (Phosphor vendored, not packaged) and FR-039.
- [x] CHK016 Principle X (Pedantic Linting & Formatting) cited on FR-041: no blanket `#[allow]`s without justification.

## E2E selector contract

- [x] CHK017 Spec enumerates which existing selectors are preserved on the Weekly variant of the new period UI: `#prev-week`, `#next-week`, `#week-range`, `#focus-summary-card`, `#total-focus-week`, `#avg-focus-day`, `#weekly-sessions`, `#weekly-focus-time`, `#sessions-table-body`. (FR-009 + A13.)
- [x] CHK018 Spec enumerates which selectors move host (Statistics → Daily) without string change: `#prev-month`, `#next-month`, `#current-month`, `#calendar-grid`. (A14.)
- [x] CHK019 Spec enumerates new selectors added by this feature: `#daily-nav`, `#daily-view`, `#prev-day`, `#next-day`, `#day-range`, `#prev-month-period`, `#next-month-period`, `#month-range`, `#prev-year`, `#next-year`, `#year-range`, `#hourly-chart`, `#monthly-chart`, `#yearly-chart`. (FR-007, FR-012–013.)
- [x] CHK020 Spec corrects the brief's mention of `#today-focus-time` as a non-existent selector (verified by grep over `tests/e2e/`). (A13.)

## Visual regression baseline scope

- [x] CHK021 Spec lists baselines that MAY regenerate with per-baseline justification required (FR-043).
- [x] CHK022 Spec lists baselines that MUST NOT regenerate (FR-044): `timer-chromium-linux.png` closed-dropdown frame, all `settings-*-chromium-linux.png`, `tag-manager-chromium-linux.png`, `update-notification-chromium-linux.png`.
- [x] CHK023 Spec accounts for the sidebar-icon-strip change (Calendar swap + new Daily entry) with a mask-or-justify policy (FR-037), so unrelated baselines don't cascade-regenerate.
- [x] CHK024 Spec notes the off-viewport `#sessions-table-body` stays off-viewport (A20) so feature 002's Title column work doesn't get cascade-regressed.

## BEST-GUESS PM DECISION markers

- [x] CHK025 Edge Cases section flags `[BEST-GUESS PM DECISION]` markers for: cursor-non-transfer across period swaps; Phosphor font-display posture; tag with `icon = ""`.
- [x] CHK026 Assumptions section flags `[BEST-GUESS PM DECISION]` markers for: A2 (period state in view, not Settings); A3 (BarChart axis policy floors); A4 (cursor non-transfer rationale); A6 (Phosphor icon choices for sidebar swap); A8 (Phosphor regular weight only); A20 (off-viewport sessions-table-body unchanged).
- [x] CHK027 Every BEST-GUESS marker is followed by reasoning (why the chosen alternative was picked over a rejected one).

## Brief deviations and verified-fact corrections

- [x] CHK028 Spec captures the Phosphor-CDN-vs-vendor correction (A7) — the brief said "CDN link in index.html" which would break `_blockExternal`.
- [x] CHK029 Spec captures the sidebar-gradient correction (A12) — there is no CSS `linear-gradient` between sidebar and main; the visual separation is the per-mode `box-shadow` glow.
- [x] CHK030 Spec captures the existing-tooltip correction — the brief said "Stop/Reset/Undo button gains `data-tooltip` and `aria-label`", but verified `mod.rs:1556-1605` shows both `aria-label=` and `title=` are already present. Only `data-tooltip=` is genuinely new (and the strings are shortened for tooltip-fit).
- [x] CHK031 Spec captures the spurious-selector correction (A13) — `#today-focus-time` is not in any current e2e spec.
- [x] CHK032 Spec calls out FR-019 (remove Statistics view's right-column block) as the rework's primary cleanup, not a side effect.

## Independence and test surfaces

- [x] CHK033 Each User Story has an "Independent Test" paragraph.
- [x] CHK034 Bundles A and B are pairwise dependent (B inherits the mini-calendar that A removes from the Statistics view) — flagged in A5; spec is internally consistent about who owns the mini-calendar after the rework.
- [x] CHK035 Bundles C and D are independent of A and B (no shared dependency); each is independently demoable.
- [x] CHK036 Bundle E (optional) has no shared dependency on A–D; cut-line is honoured.

## Success criteria measurability

- [x] CHK037 SC-001 through SC-020 are measurable via grep, wasm-bindgen-test, or e2e Playwright assertion — no purely subjective criteria.
- [x] CHK038 SC-015 ("no new border-right rule on sidebar") is grep-verifiable.
- [x] CHK039 SC-016, SC-017, SC-018 are diff-verifiable (zero new Tauri commands, zero new dependencies, zero fork references).

## Open clarifications to feed `/speckit-clarify`

- [x] CHK040 RESOLVED 2026-05-12: Four per-period baselines required (statistics-daily/weekly/monthly/yearly), not collapsed to one Weekly frame. Anchored in Principle IV. See spec.md Clarifications-resolved log + FR-043 amendment.
- [x] CHK041 RESOLVED 2026-05-12: Verbose `aria-label`/`title` preserved ("Reset timer" / "Undo last session" / "Start or pause timer" / "Skip session"); only `data-tooltip` shortened ("Reset" / "Undo" / "Start" / "Pause" / "Resume" / "Skip session"). Decoupled accessible name from visible tooltip per WCAG 4.1.2. See spec.md FR-026 / FR-027 / FR-028 / FR-029 / FR-031.
- [x] CHK042 RESOLVED 2026-05-12: Pie is static-only in v1; cross-filter deferred. Anchored in Principle VIII. See spec.md FR-050.
- [x] CHK043 RESOLVED 2026-05-12: `#sessions-table-body` moves to Daily view's right column; Statistics becomes trend-only. Anchored in Principle III + IV. `sessions-history.spec.js:31` migrates from `tapTab("Calendar")` to `tapTab("Daily")`; selector unchanged. See spec.md FR-019 amendment + A20 override.

## Notes

- This checklist was generated alongside `spec.md` as the speckit-specify deliverable.
- Open clarifications (CHK040–043) are surfaced as candidates for `/speckit-clarify`.
- Items are numbered sequentially; uncheck and add comments inline as the spec evolves.
