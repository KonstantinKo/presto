# Implementation Plan: Statistics Period Tabs, Daily Drill-Down, Phosphor Tag Icons, Control-Button Tooltips

**Branch**: `003-stats-redesign` | **Date**: 2026-05-12 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification at `specs/003-stats-redesign/spec.md`

## Table of Contents

1. [Summary](#summary)
2. [Technical Context](#technical-context)
3. [Constitution Check](#constitution-check)
4. [Project Structure](#project-structure)
5. [Modules](#modules)
6. [Testing strategy and test-first markers](#testing-strategy-and-test-first-markers)
7. [CI gates](#ci-gates)
8. [Implementation phasing](#implementation-phasing)
9. [Post-design Constitution Check](#post-design-constitution-check)
10. [Complexity Tracking](#complexity-tracking)

## Summary

Five UI-only bundles that rework the statistics surface, add a Daily drill-down view, swap the tag-picker emoji entries for Phosphor glyphs, and give the timer's control buttons CSS-driven tooltips. **Bundle A** renames `CalendarView` to `StatisticsView`, replaces its single-week summary with a four-tab period selector (`Daily / Weekly / Monthly / Yearly`), and consolidates the four chart layouts behind a single reusable `BarChart` component parametrised by `BarChartProps`. **Bundle B** introduces a new top-level `DailyView` reachable via a fourth sidebar button (`#daily-nav`); it owns the month-grid + sessions-timeline + off-viewport sessions-history table that previously lived on the Calendar view. **Bundle C** expands the tag-picker from 3 remixicon + 5 emoji entries to 3 remixicon + 9 vendored Phosphor entries, with a typed-prefix dispatch (`enum IconClass { Remix | Phosphor | Glyph }`) in the renderer. **Bundle D** adds a `data-tooltip=` attribute to `#stop-btn`, `#play-pause-btn`, `#skip-btn` with terse state-aware strings, paired with the existing verbose `aria-label`/`title` pair via a single engine-state-derived signal. **Bundle E** (optional, cut-line) adds a small Peak Focus Hour line chart to the Weekly variant. No engine changes; no Tauri bridge changes; no on-disk schema changes; no runtime dep additions (Phosphor is a vendored copy-dir asset mirroring the existing remixicon pattern at `src/index.html:19-27`). Detail in [research.md](./research.md), [data-model.md](./data-model.md), [contracts/components.md](./contracts/components.md), [quickstart.md](./quickstart.md).

## Technical Context

**Language/Version**: Rust 1.83+; `wasm32-unknown-unknown` target for the Leptos crate; backend Rust unchanged. No version bump from feature 002's baseline.
**Primary Dependencies**: Unchanged. `leptos = "0.7"`, `serde`, `serde-wasm-bindgen`, `web-sys` (for the existing `BrowserClock`), `chrono` (already used by `CalendarView` for the month-grid math). No new Cargo or npm runtime dependency. Phosphor regular-weight webfont is a vendored asset under `src/assets/icons/phosphor/`, copied into `dist/` by Trunk's `copy-dir` directive — not a package.
**Storage**: Tauri app-data directory; unchanged. `sessions.json`, `manual-sessions.json`, `tags.json`, `settings.json` shapes are unchanged. The new view-component UI state (Period enum, period cursors, Daily-view `(month_cursor, selected_day)`) is session-local Leptos `RwSignal` state — never persisted, never serialised across the Tauri bridge.
**Testing**: `cargo test --workspace --frozen` for the day-clamp helper; `wasm-bindgen-test` for the icon-renderer typed dispatch and the control-button tooltip-text matrix; Playwright e2e for view routing, period tab swap, day-cell selection, sessions-history table migration; visual regression for the four per-period Statistics baselines + the new Daily baseline (per FR-043).
**Target Platform**: macOS, Linux, Windows desktops (CSR-only single-window Tauri 2.x).
**Performance Goals**: No regression. The bar chart re-renders are O(period bar count) — at most 31 DOM nodes per swap (Monthly's max-day month). Phosphor regular-weight webfont is ~50 KB; loads once from same-origin and caches for the install lifetime.
**Constraints**: Strict static analysis stays green (Principles III + X). The existing module-wide `#![allow(clippy::must_use_candidate, clippy::too_many_lines)]` on `calendar.rs` and `timer/mod.rs` carries into the new `stats/` and `daily/` modules with the same Leptos-`#[component]` + single-`view!`-macro-body justification (FR-041). The baseline-cap gate widens for this feature: four per-period Statistics baselines plus one Daily baseline replaces the single existing `calendar-chromium-linux.png` — net +4 baselines, each carrying per-baseline justification in the PR description (FR-043, CHK040).
**Scale/Scope**: Two new module trees (`stats/` and `daily/`), a shared month-grid helper extracted from the soon-to-be-deleted `calendar.rs`, one typed-dispatch enum in the icon renderer, two derived `Signal<String>` per control button. ~12 files touched + 4 new files; no new Tauri commands; no IPC additions.

## Constitution Check

*GATE: must pass before Phase 0. Re-checked after Phase 1.*

Only principles with material content are listed below per repo artefact discipline (Principles II / VII are informational-only here: no network egress is added; no upstream-fork compatibility is in play).

### III. Type Safety Over Defensive Code — Bundles A, C, D

- **Bundle A**: `Period` is a closed sum type `enum Period { Daily, Weekly, Monthly, Yearly }` held in an `RwSignal<Period>` seeded to `Period::Weekly`. The period cursor is a `Cursor` enum or four separate `RwSignal<DateTime<Utc>>`s (one per variant) — never a string, never a free `i32`. The `BarChart` component's input contract is a typed `BarChartProps` struct (`max_scale: u32`, `x_axis_labels: Vec<String>`, `bar_values: Vec<u32>`, `min_bar_height_px: u32`) — no `serde_json::Value` slop at the component boundary.
- **Bundle C**: The icon-renderer dispatch is on `enum IconClass { Remix(String), Phosphor(String), Glyph(String) }`, constructed by a `from_icon_name(&str) -> Self` parser at the input boundary (icon-name string read off `Tag.icon`). Downstream rendering branches exhaustively on the enum — no `if name.starts_with("ri-") { ... } else if name.starts_with("ph-") { ... }` chain at the call site (FR-023, FR-025). The empty-icon-as-no-icon convention (A20 / FR-024) collapses `""` into a no-op render at the parser boundary, not as a defensive guard in the call site.
- **Bundle D**: Two derived `Signal<String>`s per button (`verbose_label`, `terse_tooltip`) both close over the same `engine` reactive source — there is no second copy of the engine state to keep in sync (FR-026, CHK041). The verbose-vs-terse decoupling per CHK041 is a content decision, not a divergent source-of-truth.

**PASS.**

### IV. Visual Regression Is The UI Contract — Bundle A + B (CHK040)

This feature regenerates baselines as follows:
- **Replaced**: `calendar-chromium-linux.png` is **deleted** in this PR (the Calendar tab is renamed to Statistics; the cold-load Weekly frame is its visual successor).
- **Added (4 per-period Statistics baselines)**: `statistics-daily-chromium-linux.png`, `statistics-weekly-chromium-linux.png`, `statistics-monthly-chromium-linux.png`, `statistics-yearly-chromium-linux.png`. Per CHK040 / FR-043, the four per-period frames are **required** — collapsing them to a single Weekly frame would let regressions in the non-Weekly variants ship undetected (24-hour bars vs 7 day-bars vs 28–31 day-bars vs 12 month-bars are materially different layouts).
- **Added (1 new view baseline)**: `daily-chromium-linux.png` for the new Daily drill-down view.
- **Unchanged**: `timer-chromium-linux.png` (tooltips are a hover/focus state not captured by the default screenshot; the tag-picker dropdown is opened only on `tag-manager-chromium-linux.png`), `settings-*-chromium-linux.png` and `update-notification-chromium-linux.png` (all untouched).
- **Regenerated**: `tag-manager-chromium-linux.png` legitimately differs because the dropdown now shows 9 Phosphor icons — flagged for regeneration with per-baseline justification.
- **Sidebar mask required (FR-037)**: the sidebar grows from three to four nav icons and the Calendar icon swaps to `ph-chart-line-up`; without a mask, every cross-sidebar baseline (`timer`, `settings-*`, `tag-manager`, `update-notification`) would diff. The Playwright `mask` option on each `toHaveScreenshot` call excludes the `.sidebar` element from the captured region in non-sidebar scenarios. Decision (a) from Story 5 Acceptance 4 is taken: mask the sidebar in non-sidebar baselines so they stay locked.

**Per-baseline justification (pre-anchored here; restated verbatim in the PR description)**:
- `statistics-daily-chromium-linux.png`: new Daily period variant of the renamed Statistics view; 24 hourly bars with the fixed 60-min/hour ceiling; `#prev-day` / `#next-day` / `#day-range` navigator widget.
- `statistics-weekly-chromium-linux.png`: cold-load default frame of the renamed Statistics view; supersedes `calendar-chromium-linux.png`. Weekly bar chart preserved; right-column mini-calendar + Today's Sessions panel removed (moved to the new Daily view per FR-019).
- `statistics-monthly-chromium-linux.png`: new Monthly period variant; 28–31 day-bars with a ≥ 50 min floor; `#prev-month-period` / `#next-month-period` / `#month-range` navigator.
- `statistics-yearly-chromium-linux.png`: new Yearly period variant; 12 month-bars labelled Jan–Dec with a ≥ 100 min floor; `#prev-year` / `#next-year` / `#year-range` navigator.
- `daily-chromium-linux.png`: new Daily drill-down view; two-column layout with the migrated month-grid on the left and the migrated sessions timeline + off-viewport sessions-history table on the right.
- `tag-manager-chromium-linux.png`: tag-picker dropdown now shows 12 icon options (3 remixicon + 9 Phosphor); the 5 emoji entries removed. No other layout change.

**`calendar-chromium-linux.png` deletion**: this PR removes the existing baseline file in the same commit that adds `statistics-weekly-chromium-linux.png`. The PR description notes the rename and the supersession.

**PASS** with documented widening — the per-baseline justification path is Principle IV's documented mechanism, not a violation. The four-Statistics-baselines decision (CHK040) is itself a Principle IV defence: collapsing them would silence per-variant regressions.

### V. Test-First For Stateful Engines — A1's day-clamp exception + FR-025's icon-dispatch exception

This feature is UI-only and FR-045 explicitly forbids touching the timer engine, manager state machines, persistence helpers, and time-keeping math. Principle V's default carve-out for "UI rendering, view wiring, theme loading, trivial CRUD plumbing" therefore applies to the majority of the work — covered by e2e + wasm-bindgen-test.

**Exception 1 — A1's day-clamp helper**: the pure `clamp_day_to_month` function covered by SC-008 (the `checked_add_months` end-of-month clamping on the Daily view's month-roll) is pure time-keeping math. The spec's A1 entry explicitly carves it into Principle V scope: "If implemented as a standalone function (rather than inlined into a Leptos signal handler), the test-first commit ordering of Principle V applies to it — RED commit precedes the GREEN implementation." The existing `CalendarView::on_next_month` precedent at `src/src/components/calendar.rs:495-509` is the reference behaviour; the extracted helper preserves it byte-stable.

**Exception 2 — FR-025's icon-renderer dispatch**: the `IconClass::from_icon_name(&str) -> IconClass` parser is a pure boundary function and FR-025 mandates a wasm-bindgen-test for the three branches. The function lives at the input boundary (icon-name string → typed enum), which is exactly the boundary-validation case Principle III calls out and Principle V's "manager state machines + time-keeping math" rule transitively covers (the parser is the closed-sum-type constructor for an enum that drives a sum-type dispatch). RED-first applies — the test asserts behaviour (which class the parser emits for each prefix), not internal structure.

**PASS** — both exceptions are inside Principle V's pure-function scope, carry explicit RED-first markers, and have unit-test coverage paths.

### VI. The Tauri Boundary Is Stable — all bundles

No new Tauri commands. No new `events::emit` callsites. The view reads `sessions: RwSignal<Vec<Session>>` and `tags: RwSignal<Vec<Tag>>` from the same context that `CalendarView` already reads (the `load_sessions` / `load_tags` / `load_settings` outputs in `crates/presto-ipc/`). The mock-drift gate (`scripts/check-mock-drift.sh`) sees no new commands and stays green without mock changes. FR-040 is the explicit anchor; SC-016 is the verification.

**PASS.**

### IX. Lock Files Are First-Class — N/A (with a one-commit asterisk)

No new runtime dependencies. Phosphor is a vendored asset (FR-022, A7) — no `phosphor-react` or `@phosphor-icons/web` runtime npm package, no Phosphor Cargo crate. `Cargo.lock` is unchanged in this feature.

**One-commit asterisk**: `tests/e2e/package.json` gains `@phosphor-icons/web` as a `devDependency` solely as a vendoring source — the runtime assets are committed to `src/assets/icons/phosphor/`; the npm package is not loaded at runtime, not bundled by Trunk, and not referenced by the WASM build. Principle IX's lockstep rule applies: the regenerated `tests/e2e/package-lock.json` is staged in the same commit as the `package.json` change. The pre-commit hook enforces this. SC-017 verifies that **runtime** lockfile entries are unchanged; the `devDependency` line is the documented vendoring path.

**Alternative considered**: download the Phosphor regular-weight font files directly from the Phosphor release tarball (committed without an npm dep). Rejected because the npm package is the canonical distribution and pinning to a release version via `package-lock.json` is what makes the vendored assets reproducible across contributor machines.

**N/A for runtime deps; lockstep enforced for the dev-dep vendoring source.**

### X. Pedantic Linting & Formatting — all bundles

New code under `src/src/components/stats/` and `src/src/components/daily/` lands clippy-pedantic-clean. The existing module-wide `#![allow(clippy::must_use_candidate, clippy::too_many_lines)]` on `src/src/components/calendar.rs` and `src/src/components/timer/mod.rs` is the precedent — the same inline justification (Leptos `#[component]` + single-`view!`-macro-body pattern means `must_use_candidate` flags every component function, and the `view!` macro body trivially crosses the 100-line `too_many_lines` threshold for any non-trivial layout) carries into the new modules. FR-041 forbids blanket `#[allow]` for any other lint without an inline principle-anchored justification — e.g., `clippy::too_many_arguments` on the `BarChart` component is rejected; the `BarChartProps` struct already collapses the arguments into a single typed bundle, which is the right Principle-III move.

**PASS.**

### Sidebar gradient / box-shadow constraint (Principle IV companion, FR-036)

The "sidebar gradient" referenced informally is the per-mode `box-shadow: 2px 0 20px color-mix(...)` glow at `src/style/sidebar.css:18, 25, 30, 35` (verified — there is no literal `linear-gradient()` between sidebar and `.main-content`). The constraint is normative: this feature MUST NOT introduce a **new** `border-right` on `.sidebar`, a 1px separator element between sidebar and `.main-content`, or any rule that flattens the existing per-mode shadow into a hard edge. Three pre-existing `border-right` rules at `src/style/themes/pipboy.css:428,437,446` are grandfather exceptions (pipboy's retro-CRT aesthetic intentionally replaces the box-shadow glow with a hard 1-px accent border on `.sidebar.focus/break/longBreak`); the default/light/dark themes carry zero such rules. SC-015 verifies via `grep -rE "border-right" src/style/` (the full `src/style/` tree including `themes/`) — the PR's delta must be exactly zero new hits; the three pre-existing pipboy hits appear identically before and after. New theme variants (none planned here) MUST preserve the box-shadow glow per FR-036.

**PASS.**

### Verdict

No principle is **VIOLATION**. The IV widening (replace 1 baseline + add 4 baselines = net +4) is a documented gate-override anchored in CHK040, not a constitution amendment. The two Principle V exceptions (day-clamp helper, icon-renderer dispatch) are inside Principle V's pure-function scope and carry explicit RED-first markers — they pass the gate as designed. Proceed to Phase 0.

## Project Structure

### Documentation (this feature)

```text
specs/003-stats-redesign/
├── plan.md                       # This file
├── research.md                   # Phase 0 — one resolved external decision (vendored Phosphor font, regular weight only); refer to spec.md for everything else
├── data-model.md                 # Phase 1 — UI-side entity shapes: Period, Cursor, BarChartProps, DailyViewState, IconClass, TooltipSignals
├── contracts/
│   └── components.md             # Phase 1 — two component contracts: IconClass + from_icon_name parser; BarChartProps
├── checklists/                   # Authored at /speckit-specify (already present)
├── quickstart.md                 # Phase 1 — contributor's path: local build, where new surfaces live, how to run e2e specs, how to regenerate baselines
└── tasks.md                      # Phase 2 — generated by /speckit-tasks (NOT this command)
```

### Source Code (new and touched paths)

```text
src/src/components/
├── calendar.rs                   # REMOVED. Contents split into stats/ and daily/ module trees (per-line provenance in the Modules table below).
├── stats/                        # NEW. {mod, bar_chart, period_selector, period_nav, tag_usage_pie, peak_focus_time(OPTIONAL)}.rs
├── daily/                        # NEW. {mod, month_grid, sessions_timeline, sessions_history_table, day_clamp}.rs
├── icon.rs                       # NEW (or inlined into timer/mod.rs if call-site count is small — Phase 1 decision).
├── timer/mod.rs                  # TOUCHED. ICON_OPTIONS expansion; data-tooltip on three control buttons; renderer uses IconClass.
└── mod.rs                        # TOUCHED. `pub mod calendar;` removed; `pub mod stats;` + `pub mod daily;` + optional `pub mod icon;` added.

src/src/app.rs                    # TOUCHED at :580-616. Fourth nav button; route enum +Daily; Calendar icon swap.
src/index.html                    # TOUCHED. Phosphor copy-dir + stylesheet link (mirrors remixicon block at :19-27).
src/assets/icons/phosphor/        # NEW. Vendored Phosphor regular-weight webfont + CSS (~50 KB).

tests/e2e/
├── fixtures/screens.js           # TOUCHED at :21, :23-35. tapTab extended with 'Daily'.
├── visual-regression.spec.js     # TOUCHED at :47-51. 1 calendar frame → 4 statistics frames + 1 daily frame.
├── calendar-navigation.spec.js   # TOUCHED at :6. tapTab "Calendar" → "Daily".
└── sessions-history.spec.js      # TOUCHED at :31. tapTab "Calendar" → "Daily".

tests/e2e/__screenshots__/visual-regression/
├── calendar-chromium-linux.png             # REMOVED
├── statistics-{daily,weekly,monthly,yearly}-chromium-linux.png  # NEW (4)
├── daily-chromium-linux.png                # NEW
└── tag-manager-chromium-linux.png          # REGENERATED (9 Phosphor icons in dropdown)
```

**Structure Decision**: `calendar.rs` does not survive as a single file. The Statistics surface (period tabs + bar chart + tag-usage pie) and the Daily surface (month grid + timeline + history table) are two coherent views that today coexist inside one ~1150-line component; splitting on the bundle boundary is what makes the FR-001 / FR-012 / FR-013 / FR-014 set implementable without a multi-PR migration. The month-grid + sessions-timeline + sessions-history-table extracts (former `calendar.rs:542-635` + `:644+`) live in `daily/`; Statistics-specific pieces in `stats/`. All existing e2e-pinned selector IDs are preserved at the string level — host file changes, IDs do not.

## Modules

Terse change table. Bundle column: A=Statistics period tabs + bar chart, B=Daily drill-down, C=Phosphor tag icons, D=control-button tooltips, E=PeakFocusTime line chart (optional), X=cross-cutting (tests, baselines).

| Path | Change | Bundle |
|---|---|---|
| `src/src/components/calendar.rs` | REMOVED. Contents split into `stats/` and `daily/`. | A,B |
| `src/src/components/stats/mod.rs` | NEW. `StatisticsView`: holds `Period` + per-period `Cursor`, computes period-scoped session set, instantiates `<BarChart {props}/>`. Preserves `id="calendar-view"` + `data-view="calendar"`. | A |
| `src/src/components/stats/bar_chart.rs` | NEW. Single `pub fn BarChart(props: BarChartProps) -> impl IntoView` (SC-002). Renders min-height floor when all-zero (FR-006). | A |
| `src/src/components/stats/period_selector.rs` | NEW. Four-tab selector; cold-load default `Period::Weekly` (FR-003). | A |
| `src/src/components/stats/period_nav.rs` | NEW. Per-period navigator. Branches on `Period` for prev/next labels + range-label format. Preserves `#prev-week` / `#next-week` / `#week-range` (FR-009, A13); adds `#prev-day` / `#next-day` / `#day-range`, `#prev-month-period` / `#next-month-period` / `#month-range`, `#prev-year` / `#next-year` / `#year-range` (FR-007). | A |
| `src/src/components/stats/tag_usage_pie.rs` | NEW. Per-period tag-usage pie + legend. Static-only v1 (FR-010, FR-050 / CHK042). | A |
| `src/src/components/stats/peak_focus_time.rs` | NEW, OPTIONAL. 24-point SVG line chart on Weekly (FR-032–035). Build-config gate so Bundles A–D ship without it (A15). | E |
| `src/src/components/daily/mod.rs` | NEW. `DailyView`: two-column layout, `id="daily-view"`. `(month_cursor, selected_day)` seeded from `BrowserClock.now_ms()` (FR-013, FR-015). | B |
| `src/src/components/daily/month_grid.rs` | NEW. Extracted from `calendar.rs:542-602`. Sun-first day-of-week header (FR-018). Today's cell `aria-current="date"`. Cell click → `selected_day` + `.selected` class (FR-016). Preserves `#calendar-grid`, `#current-month`, `#prev-month`, `#next-month` (A13/A14). | B |
| `src/src/components/daily/sessions_timeline.rs` | NEW. Extracted from `calendar.rs:604-635`. Owns `#sessions-timeline`, `#timeline-hours`, `#selected-day-title`. Empty-state preserved. | B |
| `src/src/components/daily/sessions_history_table.rs` | NEW. Extracted from `calendar.rs:644+` (the off-viewport block). Owns `#sessions-table-body` (CHK043). | B |
| `src/src/components/daily/day_clamp.rs` | NEW. Pure `clamp_day_to_month(day_of_month: u32, target_month: DateTime<Utc>) -> DateTime<Utc>`. **`[test-first]`** per A1's Principle V exception (FR-017, SC-008). | B,X |
| `src/src/components/icon.rs` | NEW (or inlined into `timer/mod.rs` if call-site count keeps it small). `enum IconClass { Remix(String), Phosphor(String), Glyph(String) }`, `from_icon_name(&str) -> Self` parser, `render(class: &IconClass) -> impl IntoView`. **`[test-first]`** per FR-025. | C,X |
| `src/src/components/timer/mod.rs` | TOUCHED. (1) `ICON_OPTIONS` at :75-84 → 3 remix + 9 Phosphor (FR-020/021); `IconClass::Glyph` preserves legacy emoji-icon tags (FR-024, SC-011). (2) Existing verbose `aria-label` / `title` at :1557/:1583/:1600 preserved verbatim (CHK041). Three buttons gain `data-tooltip=move \|\| terse_tooltip.get()` bound to a derived `Signal<String>` keyed off `engine.current_mode()` + run-state predicates (FR-026–029). | C,D |
| `src/src/components/mod.rs` | TOUCHED. `pub mod calendar;` removed; `pub mod stats;`, `pub mod daily;` added; `pub mod icon;` added if extracted. | A,B,C |
| `src/src/app.rs` | TOUCHED at :580-616. Fourth `<button id="daily-nav" data-view="daily" title="Daily">` between Calendar and Settings; route enum `+Daily`; Calendar `ri-calendar-line` → `ph-chart-line-up`; Daily `ph-calendar-check`. Existing `id="calendar-nav"` / `data-view="calendar"` / `id="calendar-view"` preserved (A6, FR-001, FR-012). | A,B |
| `src/index.html` | TOUCHED. One Trunk `copy-dir` + one stylesheet link for Phosphor; mirrors remixicon block at :19-27 (FR-022, A7). | C |
| `src/assets/icons/phosphor/` | NEW. Vendored regular-weight webfont + CSS (A8; ~50 KB). | C |
| `src/style/sidebar.css` | UNCHANGED at :3-35. FR-036 normative: no `border-right`, no 1px separator (SC-015). | — |
| `tests/e2e/fixtures/screens.js` | TOUCHED at :21, :23-35. `tapTab` extended with `'Daily'` branch. | A,B,X |
| `tests/e2e/visual-regression.spec.js` | TOUCHED at :47-51. Calendar screenshot → 4 per-period Statistics + 1 Daily screenshots (CHK040 / FR-043). Sidebar masked on non-sidebar baselines (FR-037). | A,B,X |
| `tests/e2e/calendar-navigation.spec.js` | TOUCHED at :6. `tapTab("Calendar")` → `tapTab("Daily")` (A14). | B,X |
| `tests/e2e/sessions-history.spec.js` | TOUCHED at :31. `tapTab("Calendar")` → `tapTab("Daily")` (CHK043). | B,X |
| `tests/e2e/__screenshots__/visual-regression/` | REMOVED `calendar.png`; NEW `statistics-{daily,weekly,monthly,yearly}.png` + `daily.png`; REGENERATED `tag-manager.png`. | A,B,C,X |

## Testing strategy and test-first markers

Per Principle V's UI-rendering carve-out, A1's day-clamp exception, and FR-025's icon-dispatch exception, this feature has **two explicit `[test-first]` markers**; the rest is UI plumbing covered by e2e + visual regression. One additional MANDATORY non-RED-first wasm-bindgen-test is required for the tooltip text matrix (FR-031) — it lands alongside the Bundle D implementation, not before it.

| Module | Test runner | Test-first? | Notes |
|---|---|---|---|
| `daily::day_clamp::tests` | `cargo test` (workspace) — or `wasm-bindgen-test` if the helper lives behind the wasm cfg gate | **YES (RED-first)** | A1's Principle V exception + SC-008. Boundary cases: `clamp_day_to_month(31, May)` → `May 31` (no clamp); `clamp_day_to_month(31, June)` → `June 30` (clamps down); `clamp_day_to_month(31, Feb-leap)` → `Feb 29`; `clamp_day_to_month(31, Feb-non-leap)` → `Feb 28`; `clamp_day_to_month(1, Feb)` → `Feb 1`. Existing `CalendarView::on_next_month` at `src/src/components/calendar.rs:495-509` is the reference behaviour; the extracted helper preserves it byte-stable. |
| `icon::tests` | `wasm-bindgen-test` | **YES (RED-first)** | FR-025 + SC-010 + SC-011. Asserts: (a) `IconClass::from_icon_name("ri-brain-line")` → `Remix("brain-line")`, renders as `<i class="ri-brain-line"></i>`; (b) `IconClass::from_icon_name("ph-cloud")` → `Phosphor("cloud")`, renders as `<i class="ph ph-cloud"></i>` (the outer `ph` wrapper class is required for the Phosphor font face to bind); (c) `IconClass::from_icon_name("\u{1f9e0}")` → `Glyph("\u{1f9e0}")`, renders as the raw grapheme text content; (d) `IconClass::from_icon_name("")` → no-icon (empty `<i>` per A20 / Edge Cases entry "Tag with `icon = \"\"`"). The parser is a pure function; the dispatch is a closed enum match — RED-first ordering applies because the test asserts boundary-parsing behaviour, not internal structure (Principle V). |
| `tooltip text matrix` (in `timer/mod.rs::tests`) | `wasm-bindgen-test` | MANDATORY non-RED-first | FR-031 + SC-012. Enumerates the state matrix per FR-027 / FR-028 / FR-029 across `Focus` × `Break` × `LongBreak` × `running` × `idle` × `paused` × `auto-paused`. Asserts (a) `aria-label == title` per button per state; (b) `data-tooltip` matches the FR-027/028/029 terse mapping; (c) `aria-label != data-tooltip` per CHK041 (the test MUST NOT assert equality). The wasm-bindgen-test mounts the component and reads the rendered attribute values from the DOM. Per Principle V's UI-rendering carve-out, this test is NOT RED-first — it lands alongside the Bundle D implementation (T017/T018) as a coverage gate. |
| `bar_chart::tests` (period bar counts + floor) | `wasm-bindgen-test` | NO (e2e + visual regression default) | SC-002 + SC-003 + SC-004. SC-002's "only-one `pub fn BarChart` definition" is a grep / file-structure assertion, not a runtime test — verifiable via `grep -c "pub fn BarChart" src/src/components/stats/bar_chart.rs` returning 1. SC-003's "Daily=24, Weekly=7, Monthly={28..31}, Yearly=12 bars" is exercised by e2e (the per-period visual-regression frames count the bars) and by an optional wasm-bindgen-test if the component is shallow enough to mount in isolation. SC-004's per-period floor (Daily=60, Weekly≥20, Monthly≥50, Yearly≥100) is a component-level test that constructs `BarChartProps { max_scale: 0, bar_values: vec![0; 7], ... }` and asserts the rendered chart still has bars at the minimum-visible-height floor (FR-006). |
| Period swap cursor reset (`stats::tests`) | Playwright e2e | NO | SC-005. e2e test swaps tabs and asserts the range label matches the current period anchor (today / this Monday / first-of-this-month / first-of-this-year) each time. |
| Daily view routing + day-cell selection | Playwright e2e (new `daily.spec.js`) | NO | SC-006, SC-007. Test clicks `#daily-nav`, asserts `#daily-view` not hidden + `#timer-view` + `#calendar-view` hidden, clicks a seeded day cell, asserts `#selected-day-title` text updates and the timeline re-binds. |
| Tag-picker icon count + Phosphor render | Playwright e2e + visual regression (`tag-manager-chromium-linux.png`) | NO | SC-009 + SC-010. Visual regression captures the dropdown with 12 icons (3 remix + 9 Phosphor; zero emoji); a wasm-bindgen-test asserts `ICON_OPTIONS.len() == 12` and the prefix distribution. |
| Pre-rework emoji-tag round-trip | `wasm-bindgen-test` (renderer fallback) | NO (covered by `icon::tests`) | SC-011. `icon::tests::glyph_branch_renders_text_content` already covers this — feeds a literal `"\u{1f9e0}"` through `from_icon_name` and asserts the rendered output is the raw grapheme. |
| Tooltip CSS hover + focus | `grep` over the new tooltip CSS rule + Playwright `Tab`-focus assertion | NO | SC-013. Grep verifies the CSS rule contains `:hover, :focus-visible`; Playwright tabs into `#stop-btn`, asserts the tooltip element becomes visible. |
| Visual-regression baseline diff scope | Reading the PR's `__screenshots__/` diff | NO | SC-014. PR-time check, not a runtime test. |
| Sidebar `border-right` absence | `grep -rE "border-right" src/style/` (full tree including `themes/`) | NO | SC-015. Grep returns the same set of hits before and after the PR — three pre-existing pipboy grandfather hits at `themes/pipboy.css:428,437,446` are unchanged; zero new hits on `.sidebar` selectors in any file. |
| 0 new Tauri commands / IPC / network egress | `grep` for `#[tauri::command]` / `events::emit` / `fetch` / `reqwest` in the diff | NO | SC-016. PR-time check. |
| 0 new runtime lockfile entries | `Cargo.lock` + `tests/e2e/package-lock.json` `dependencies` diff inspection | NO | SC-017. PR-time check. `@phosphor-icons/web` is a `devDependency`; the runtime bundle does not link against it. |
| 0 fork attributions | `grep -rE 'ramazan\|murdercode\|github.com/' specs/003-stats-redesign/` | NO | SC-018. PR-time check. |
| English-only UI strings | grep over new components' string literals | NO | SC-019. PR-time check. |
| Bundle E Peak Focus Hour | `wasm-bindgen-test` if Bundle E ships | NO | SC-020 OPTIONAL. Deferred to Phase 6 task generation if Bundle E is included; skipped entirely if cut. |

**Mock-first ordering rule** (per Principle VI / FR-040): **N/A this feature.** No new Tauri commands; the mock-drift gate stays green without modifications.

## CI gates

Reference `.agentex.yml` stage definitions. All gates already exist; this feature interacts with five of them.

### Mock-drift gate — `scripts/check-mock-drift.sh`

**No action needed.** No new `#[tauri::command]` handlers, no new mock cases. Run as a sanity check; expect green. SC-016 is the verification.

### Strict static analysis — `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic` + `cargo fmt --check`

**Load-bearing.** New code under `src/src/components/stats/` and `src/src/components/daily/` lands clippy-pedantic-clean. The existing module-wide `#![allow(clippy::must_use_candidate, clippy::too_many_lines)]` justification (Leptos `#[component]` + single-`view!`-macro-body pattern) carries into the new modules — no other blanket `#[allow]` is permitted without an inline principle-anchored justification (FR-041). Principle X is the anchor.

### `cargo build --frozen` + `trunk build --release`

**No action needed.** No new Cargo or npm runtime dependency (Phosphor is a vendored asset, not a runtime package). Both gates stay green by inaction. SC-017 is the verification.

### `wasm-bindgen-test`

**Load-bearing.** Two new test-first wasm-bindgen-tests land before their implementations: `daily::day_clamp::tests` (6 boundary cases) and `icon::tests` (12 test cases (8 parser + 4 render)). One MANDATORY non-RED-first wasm-bindgen-test for the tooltip text matrix (FR-031 / SC-012); one for the `BarChart` per-period floor (SC-004); one for the tag-picker count (SC-009). Reference: AGENTS.md §Test-first commit ordering — RED commits land before GREEN commits for the two test-first markers.

### Playwright e2e + visual regression

**Load-bearing widening.** Four spec files are touched:
1. `visual-regression.spec.js`: net +4 baseline frames (replace `calendar.png` with `statistics-weekly.png`; add `statistics-daily.png`, `statistics-monthly.png`, `statistics-yearly.png`, `daily.png`; regenerate `tag-manager.png`). The baseline-cap gate (`scripts/check-baseline-cap.sh`, if present from feature 002's precedent) widens for this PR with per-baseline justification — six per-baseline notes pre-anchored in §[Constitution Check IV](#iv-visual-regression-is-the-ui-contract--bundle-a--b-chk040).
2. `calendar-navigation.spec.js`: `tapTab` migration (1-line change).
3. `sessions-history.spec.js`: `tapTab` migration (1-line change).
4. New `daily.spec.js`: routing + day-cell selection assertions (SC-006, SC-007).

### Lockfile-drift gate

**Single-commit lockstep update.** `tests/e2e/package.json` gains `@phosphor-icons/web` as a `devDependency` (vendoring source only — not a runtime dep); the regenerated `tests/e2e/package-lock.json` is staged in the same commit. `Cargo.lock` is unchanged. The pre-commit hook enforces lockstep. SC-017 verifies that the runtime bundle does not link against the package.

## Implementation phasing

Nine phases. Bundles A–E are independently testable; the phase order matches tasks.md: Phase 0 pre-flights the Phosphor asset vendoring and the `datetime_from_ms` helper; Phase 1 (Bundle C) builds the icon typed-dispatch with RED-first tests; Phase 2 (Bundle B) builds the Daily view + `day_clamp` helper RED-first; Phase 3 (Bundle A) reworks the Statistics view; Phase 4 (Bundle D) adds control-button tooltips; Phase 5 is the `calendar.rs` cleanup verification + full e2e run; Phase 6 is the optional Bundle E; Phase 7 regenerates baselines; Phase 8 is the final gate sweep.

### Phase 0 — Pre-flight (Phosphor webfont vendoring + `datetime_from_ms` helper extraction)

**Entry**: clean branch `003-stats-redesign` post-spec.
**Exit**: `src/assets/icons/phosphor/phosphor.css` exists; `src/index.html` wires the Phosphor `copy-dir` + stylesheet link (mirrors the remixicon block at :19-27); `tests/e2e/package.json` carries `@phosphor-icons/web` as a `devDependency`; `tests/e2e/package-lock.json` is regenerated in the same commit (Principle IX / FR-039). The `datetime_from_ms` helper is promoted to a shared module (`src/src/components/utils/datetime.rs` or equivalent). `trunk build --release` (from `src/`) succeeds. `cargo fmt --check && cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic` green.
**Test-first**: NO (vendoring + helper extraction is build-system plumbing; no stateful engine logic).

### Phase 1 — Bundle C: Phosphor tag icons [test-first]

**Entry**: Phase 0 complete.
**Exit**: `src/src/components/icon.rs` defines `enum IconClass`, `from_icon_name(&str) -> Self`, and `render(class: &IconClass) -> impl IntoView`. `ICON_OPTIONS` at `timer/mod.rs:75-84` expands from 3 remix + 5 emoji → 3 remix + 9 Phosphor. Legacy emoji-icon tags render via `IconClass::Glyph` fallback (FR-024, SC-011). All `icon::tests` (12 cases) pass.
**Test-first**: YES for `icon::tests` (FR-025 — boundary parser). UI plumbing is e2e + visual regression covered.
- **Test-first commit ordering** (AGENTS.md §Test-first commit ordering, Principle V): the RED commit for `icon::tests` lands first (12 failing wasm-bindgen-test cases: 8 parser + 4 render branches; `wasm-pack test --node` exits non-zero). The GREEN commit follows (parser + render fn implemented; tests pass). The two commits are NOT collapsed.

### Phase 2 — Bundle B: Daily view + `day_clamp` helper [test-first]

**Entry**: Phase 0 complete (independent of Phase 1 at the file level).
**Exit**: `src/src/components/daily/mod.rs` exists with `DailyView`'s two-column layout (`id="daily-view"`); it imports the extracted month-grid, sessions-timeline, sessions-history-table modules plus `day_clamp`. The sidebar in `src/src/app.rs:580-616` gains a fourth nav button (`id="daily-nav"`, `data-view="daily"`, title `"Daily"`). The route enum gains a `Daily` variant. `tests/e2e/sessions-history.spec.js:31` migrates from `tapTab(page, "Calendar")` to `tapTab(page, "Daily")`. `daily.spec.js` (SC-006, SC-007) passes. **`calendar.rs` is not yet deleted** — Phase 2 leaves it referencing the new modules so the existing Statistics route still works end-to-end.
**Test-first**: YES for `day_clamp` (A1's Principle V exception). UI plumbing is e2e + visual regression covered.
- **Test-first commit ordering** (AGENTS.md §Test-first commit ordering, Principle V): the RED commit for `day_clamp::tests` lands first (6 failing boundary cases; `cargo test --workspace --frozen` exits non-zero). The GREEN commit follows (extracted helper passes the tests; `cargo test` exits zero). The two commits are NOT collapsed.

### Phase 3 — Bundle A: Statistics view (rename + period tabs + reusable bar chart)

**Entry**: Phase 2 complete (Daily view + sidebar route in place).
**Exit**: `src/src/components/stats/mod.rs` exists with `StatisticsView`'s period-tab layout. The new modules `stats/{bar_chart,period_selector,period_nav,tag_usage_pie}.rs` are populated. `BarChart` is exactly one `pub fn BarChart` definition (SC-002). The cold-load default is `Period::Weekly`. The four per-period navigators emit the correct selector IDs (preserving `#prev-week` / `#next-week` / `#week-range` for the Weekly variant per FR-009 / A13; adding `#prev-day` / `#next-day` / `#day-range`, `#prev-month-period` / `#next-month-period` / `#month-range`, `#prev-year` / `#next-year` / `#year-range` for the new variants per FR-007). The right-column mini-calendar + Today's Sessions + sessions-history table block (former `calendar.rs:542-635` and `:644+`) is **removed from the Statistics view** — Daily (Phase 2) is now the single source of truth (FR-019, A5, CHK043). `src/src/components/calendar.rs` is **deleted**; `src/src/components/mod.rs:23` removes the `pub mod calendar;` line and adds `pub mod stats;` + `pub mod daily;`. The view title text in the Statistics container changes from "Calendar & Statistics" to "Statistics" (FR-001).
**Test-first**: NO (UI plumbing; e2e + visual regression covers SC-005). The `BarChart` per-period floor test (SC-004) is an optional wasm-bindgen-test, not RED-first.

### Phase 4 — Bundle D: Control-button tooltips

**Entry**: Phase 1 complete (same file — `timer/mod.rs` — as the icon refactor; coordinate merge order).
**Exit**: `src/src/components/timer/mod.rs` at :1556, :1583, :1600 each gains a `data-tooltip=move \|\| terse_tooltip.get()` attribute. Two derived `Signal<String>`s per button (`verbose_label` and `terse_tooltip`) close over the same engine reactive source (`engine.current_mode()` + `engine.is_running()` + `engine.is_paused()` + `engine.is_auto_paused()`) so per-state drift is impossible (FR-026, CHK041). The verbose `aria-label` / `title` strings at :1557-1558 / :1583 / :1600 are **preserved verbatim** ("Reset timer" / "Undo last session" / "Start or pause timer" / "Skip session"); only the new `data-tooltip` carries the terse counterparts (FR-027, FR-028, FR-029). The CSS rule driving the visible tooltip triggers on `:hover, :focus-visible` (FR-030, SC-013) — added to `src/style/timer.css` (or wherever the existing button styles live; Phase 4 task generation picks the file). Transition ≤ 150 ms (no native ~1 s delay).
**Test-first**: The tooltip text matrix wasm-bindgen-test (FR-031 / SC-012) is **MANDATORY non-RED-first** — it lands alongside the Bundle D implementation (T017/T018) per Principle V's UI-rendering carve-out; it is a coverage gate, not a RED-first pair. The CSS-rule grep (SC-013) is a PR-time check, not a runtime test.

### Phase 5 — `calendar.rs` cleanup verification + full e2e run

**Entry**: Phases 1 / 2 / 3 / 4 complete.
**Exit**: `calendar.rs` is confirmed fully gone; lint is clean; the full Playwright e2e suite passes (except visual-regression diffs which are expected and handled in Phase 7). `cargo test --workspace --frozen` green. `bash scripts/check-mock-drift.sh` exits 0.
**Test-first**: N/A.

### Phase 6 — OPTIONAL Bundle E: PeakFocusTime line chart

**Entry**: Phases 1–5 complete; cycle's timeline still has budget.
**Exit**: `src/src/components/stats/peak_focus_time.rs` exists with the 24-point SVG line chart, mounted at the bottom of the Weekly variant of `StatisticsView`. The "Insufficient data — keep tracking to see your peak hour" label appears when fewer than 3 days of data are present (FR-034). A wasm-bindgen-test asserts the 24 x-points + the peak-hour dot positioning + the insufficient-data fallback (SC-020).
**Test-first**: OPTIONAL. The line-chart component is pure presentational logic over the period's session set; a wasm-bindgen-test is reasonable but not Principle V-mandated. Decision in Phase 6 task generation.
**Cut-line**: If the cycle's timeline tightens after Phase 5, Bundle E is **deferred to a follow-up issue** (A15). The Weekly tab simply renders without the Peak Focus Hour panel; Bundles A–D are unaffected (FR-035, Story 6 Acceptance 3). Bundle E is cut by **source-omission only**: delete `src/src/components/stats/peak_focus_time.rs` and remove the `pub mod peak_focus_time;` line from `src/src/components/stats/mod.rs`. There is no cargo feature flag, no runtime `const bool`, and no `cfg(feature = "…")` gate — source presence is the only toggle.

### Phase 7 — Visual-regression baselines (required)

**Entry**: Phases 1 / 2 / 3 / 4 / 5 complete (+ Phase 6 if shipping); e2e suite passes except for the expected visual-regression diffs.
**Exit**:
1. `tests/e2e/visual-regression.spec.js` at :47-51 is restructured: the single `tapTab(page, "Calendar")` + `await expect(page).toHaveScreenshot(["visual-regression", "calendar.png"])` block is replaced with four sequential per-period frames. After it, a new section `await tapTab(page, "Daily")` + screenshot for the Daily view. Sidebar masking via `mask: [page.locator(".sidebar")]` is applied to all non-sidebar screenshots (FR-037).
2. Baselines are regenerated locally via `npx playwright test tests/e2e/visual-regression.spec.js --update-snapshots`, reviewed visually one-by-one against the per-baseline justifications in §[Constitution Check IV](#iv-visual-regression-is-the-ui-contract--bundle-a--b-chk040), and committed in a single commit titled `chore(visual): replace calendar.png with 4 per-period statistics + 1 daily + regenerated tag-manager (feature 003)`.
3. The PR description restates the six per-baseline notes verbatim. The `calendar-chromium-linux.png` deletion is part of the same commit.
4. CI's visual-regression run on the PR sees zero unexpected diffs; any unjustified diff on an untouched baseline (`timer`, `settings-*`, `update-notification`) is treated as a regression in code, not absorbed into the baseline (SC-014).

**Test-first**: N/A (visual gate is itself the test).

### Phase 8 — Final gates

**Entry**: Phase 7 complete.
**Exit**: Full final gate sweep passes — `cargo fmt --check`, `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic`, `cargo test --workspace --frozen`, `bash scripts/check-mock-drift.sh` (SC-016), lockfile-drift check (SC-017), `grep -rE "border-right" src/style/` (exactly 3 pre-existing pipboy hits — SC-015), `grep -c "pub fn BarChart" src/src/components/stats/bar_chart.rs` returns 1 (SC-002), `npx playwright test --reporter=line` all green (visual-regression exits 0 after Phase 7). PR is opened.
**Test-first**: N/A.

## Post-design Constitution Check

Re-checked after Phase 1 design (research.md, data-model.md, contracts/components.md, quickstart.md). Verdicts unchanged from §[Constitution Check](#constitution-check). Material principles re-affirmed:

- **III**: data-model.md restates `Period`, `Cursor`, `IconClass` as closed sum types; `BarChartProps` as a typed struct; the verbose/terse tooltip signals as two `Signal<String>`s closed over the same engine reactive source. contracts/components.md anchors the `IconClass::from_icon_name` parser contract and the `BarChartProps` shape.
- **IV**: §[Constitution Check IV](#iv-visual-regression-is-the-ui-contract--bundle-a--b-chk040) pre-anchors the six per-baseline justifications. quickstart.md lists the verbatim text for copy-paste into the PR description.
- **V**: §[Testing strategy](#testing-strategy-and-test-first-markers) enumerates the two RED-first tests (`day_clamp::tests`, `icon::tests`). UI plumbing exempt per Principle V's documented carve-out.
- **VI**: contracts/components.md explicitly states "no new Tauri commands"; the mock-drift gate stays green without changes. SC-016 verifies.

## Complexity Tracking

> No Constitution Check violations require justification. The IV widening (replace 1 + add 4 = net +4 baselines) is a documented gate-override anchored in CHK040, not a violation. The two Principle V exceptions (day-clamp helper, icon-renderer dispatch) are inside Principle V's pure-function scope and carry explicit `[test-first]` markers — they pass the gate as designed, not as accommodations.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|---|---|---|
| (none) | — | — |
