# Tasks: Statistics Period Tabs, Daily Drill-Down, Phosphor Tag Icons, Control-Button Tooltips

**Input**: Design docs in `specs/003-stats-redesign/`
**Prerequisites**: spec.md (49 FRs, 18 SCs, 20 Assumptions, CHK040–043), plan.md (Phase 0..8), data-model.md, contracts/components.md, quickstart.md

## Format

`- [ ] [TID] [P?] [Bundle] [Phase] Description with file path` — Bundle ∈ {A,B,C,D,E,X}; Phase ∈ {0..8}. `[P]` = parallelisable with other `[P]` tasks in the same phase. Each task lists its **Done-signal** and **Files**. Test-first tasks carry explicit **RED** / **GREEN** commit-boundary labels.

Bundles: **A** = Statistics period tabs + reusable bar chart · **B** = Daily drill-down · **C** = Phosphor tag icons · **D** = control-button tooltips · **E** = PeakFocusTime line chart (optional, cut-line) · **X** = cross-cutting (tests, baselines, e2e migration)

---

## Phase 0 — Pre-flight: Phosphor webfont vendoring + shared `datetime_from_ms` helper

**Goal**: vendor the Phosphor regular-weight webfont as a committed `copy-dir` asset, wire it into `src/index.html`, and promote the `datetime_from_ms` helper to a shared location. No code compiles against the font yet; this unblocks Phase 1 (Bundle C) and Phase 3 (Bundle A). The shared helper unblocks Daily view (Phase 1) and Statistics view (Phase 2) which both need it.

**Exit**: `trunk build --release` (from `src/`) succeeds; `src/assets/icons/phosphor/phosphor.css` exists; `tests/e2e/package.json` carries `@phosphor-icons/web` as a `devDependency`; `tests/e2e/package-lock.json` is regenerated in the same commit (Principle IX / FR-039). `cargo fmt --check && cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic` green.

- [ ] **T001** [P] [X] [Phase 0] Install `@phosphor-icons/web` as a `devDependency` in `tests/e2e/package.json` and copy the regular-weight font files + CSS to `src/assets/icons/phosphor/`.
  - **Files**: `tests/e2e/package.json`, `tests/e2e/package-lock.json`, `src/assets/icons/phosphor/` (new directory)
  - **Procedure**: `cd tests/e2e && npm install --save-dev @phosphor-icons/web` → then copy `node_modules/@phosphor-icons/web/src/regular/Phosphor.{eot,svg,ttf,woff,woff2}` and `node_modules/@phosphor-icons/web/src/regular/style.css` (renamed to `phosphor.css`) into `../../src/assets/icons/phosphor/`. Stage the vendored assets **and** the regenerated `package-lock.json` in the same commit (Principle IX / FR-039 / SC-017). See `specs/003-stats-redesign/quickstart.md §3` for the exact command sequence.
  - **Done-signal**: `ls src/assets/icons/phosphor/` shows `Phosphor.woff2` (and other font files) + `phosphor.css`. `git status tests/e2e/package.json tests/e2e/package-lock.json` shows both files modified. `npm ci` inside `tests/e2e/` exits 0.

- [ ] **T002** [P] [X] [Phase 0] Wire the Phosphor webfont into `src/index.html` via Trunk's `copy-dir` directive and a same-origin stylesheet link, mirroring the existing remixicon block at lines 19–27.
  - **Files**: `src/index.html`
  - **Changes**: (1) add `<link data-trunk rel="copy-dir" href="assets/icons/phosphor" data-target-path="assets/icons/phosphor" />` immediately after the remixicon copy-dir line; (2) add `<link rel="stylesheet" href="/assets/icons/phosphor/phosphor.css" />` immediately after the remixicon stylesheet link. No other change to `index.html`.
  - **Done-signal**: `trunk build --release` (from `src/`) exits 0; `ls dist/assets/icons/phosphor/` shows the font files in the distribution tree. No CDN URL is present in the added lines.
  - **BlockedBy**: T001.

- [ ] **T003** [P] [X] [Phase 0] Promote the `datetime_from_ms` helper from `src/src/components/calendar.rs:46-49` to a shared module `src/src/components/utils/datetime.rs` (or `src/src/time_utils.rs`) so `stats/` and `daily/` can import it without duplication.
  - **Files**: `src/src/components/calendar.rs` (read-only reference), new `src/src/components/utils/datetime.rs` (or `src/src/time_utils.rs`), `src/src/components/mod.rs` or `src/src/lib.rs` (registration)
  - **Changes**: create the new module with `pub fn datetime_from_ms(ms: i64) -> chrono::DateTime<chrono::Utc>` containing the same epoch-fallback body from `calendar.rs:46-49`. Do NOT delete the original from `calendar.rs` yet — `calendar.rs` stays in place through Phase 4. Register the new module (`pub mod utils;` or `pub mod time_utils;`). Existing `calendar.rs` may re-export from the new location or keep its local copy for now.
  - **Done-signal**: `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic && cargo fmt --check` green. New module path compiles.

**Phase 0 exit**: `trunk build --release` succeeds. `cargo clippy` + `cargo fmt --check` green. `bash scripts/check-mock-drift.sh` exits 0 (no new Tauri commands). `grep -rE "border-right" src/style/` returns the same 3 pre-existing pipboy hits.

---

## Phase 1 — Bundle C: Phosphor tag icons [test-first]

**Goal**: add the `IconClass` enum + parser + renderer, expand `ICON_OPTIONS` in `timer/mod.rs`, and wire the typed dispatch into all tag-rendering callsites. RED commit lands first with 8 failing wasm-bindgen-test cases; GREEN commit follows with the implementation.

**Exit**: `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic && cargo fmt --check && wasm-pack test --node src/ -- --filter icon::tests` all green. `ICON_OPTIONS.len() == 12`; zero emoji in the picker.

### Test-first pair — icon parser + renderer (RED → GREEN, two separate commits)

- [ ] **T004** [C] [Phase 1] **[test-first RED]** Write the failing `icon::tests` wasm-bindgen-test module covering all 12 cases (8 parser + 4 render) before any implementation exists.
  - **Files**: `src/src/components/icon.rs` (new; test module only — no implementation yet) and/or `src/src/components/mod.rs` (stub `pub mod icon;`)
  - **Test cases to write** (per `contracts/components.md` Contract 1 Tests section):
    1. `parser_remix_branch`: `from_icon_name("ri-brain-line") == Remix("brain-line")`
    2. `parser_phosphor_branch`: `from_icon_name("ph-cloud") == Phosphor("cloud")`
    3. `parser_glyph_branch_emoji`: `from_icon_name("\u{1f9e0}") == Glyph("\u{1f9e0}")`
    4. `parser_glyph_branch_empty`: `from_icon_name("") == Glyph("")`
    4a. `parser_remix_prefix_only`: `from_icon_name("ri-") == Glyph("ri-")` (zero-length suffix → Glyph)
    4b. `parser_phosphor_prefix_only`: `from_icon_name("ph-") == Glyph("ph-")` (same rule)
    4c. `parser_undashed_ph_prefix`: `from_icon_name("phone") == Glyph("phone")`
    4d. `parser_leading_whitespace`: `from_icon_name(" ri-foo") == Glyph(" ri-foo")`
    5. `render_remix_emits_i_with_ri_class`: rendered DOM contains `<i class="ri-brain-line">`
    6. `render_phosphor_emits_i_with_ph_wrapper_and_glyph`: rendered DOM contains `<i class="ph ph-cloud">` (both classes required)
    7. `render_glyph_emits_text_content`: rendered DOM is `<i>🧠</i>`
    8. `render_glyph_empty_emits_empty_i`: rendered DOM is `<i></i>`
  - **Done-signal**: `wasm-pack test --node src/ -- --filter icon::tests` **exits non-zero** (tests compile but fail because `IconClass`, `from_icon_name`, and `render` are not yet implemented). Commit the failing test **separately** from the implementation.
  - **BlockedBy**: T002 (font wired so the build compiles), T003 (shared helper in place).

- [ ] **T005** [C] [Phase 1] **[test-first GREEN]** Implement `IconClass` enum, `from_icon_name(&str) -> Self` parser, and `render(class: &IconClass) -> impl IntoView` in `src/src/components/icon.rs`. Expand `ICON_OPTIONS` and wire the typed dispatch into all tag-rendering callsites.
  - **Files**: `src/src/components/icon.rs`, `src/src/components/timer/mod.rs`, `src/src/components/mod.rs`
  - **Implementation details**:
    - `IconClass` enum: `Remix(String)`, `Phosphor(String)`, `Glyph(String)` per `data-model.md §IconClass`.
    - `from_icon_name` parser: `strip_prefix("ri-")` with non-empty-suffix check → `Remix`; `strip_prefix("ph-")` with non-empty-suffix check → `Phosphor`; else → `Glyph`. See edge-case dispatch table in `contracts/components.md`.
    - `render` exhaustive match: `Remix(s)` → `<i class="ri-{s}">`, `Phosphor(s)` → `<i class="ph ph-{s}">`, `Glyph(g)` → `<i>{g}</i>`.
    - `ICON_OPTIONS` at `timer/mod.rs:75-84`: expand from 3 remix + 5 emoji → 3 remix (`ri-brain-line`, `ri-focus-3-line`, `ri-lightbulb-line`) + 9 Phosphor (`ph-butterfly`, `ph-cloud`, `ph-code-simple`, `ph-github-logo`, `ph-apple-logo`, `ph-crown-simple`, `ph-atom`, `ph-student`, `ph-cpu`). Remove the 5 emoji entries (`\u{1f9e0}`, `\u{1f4aa}`, `\u{1f3af}`, `\u{26a1}`, `\u{1f525}`). `DEFAULT_NEW_TAG_ICON` at `:92` stays `"ri-brain-line"`.
    - Replace all `if name.starts_with("ri-") { ... }` chains at tag-rendering callsites (`mod.rs:883-905` and the tag list/picker preview at `#selected-icon-btn`) with `IconClass::from_icon_name(&tag.icon)` → exhaustive `match`. Legacy emoji-icon tags continue rendering via `IconClass::Glyph` (FR-024, SC-011).
    - Add `#![allow(clippy::must_use_candidate)]` with Leptos-`#[component]` justification if triggered.
  - **Done-signal**: `wasm-pack test --node src/ -- --filter icon::tests` **exits zero** (all 12 cases pass). `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic && cargo fmt --check` green. `grep -n "pub enum IconClass\|pub fn from_icon_name" src/src/components/icon.rs` returns at least 2 hits (the enum definition and the parser function). Commit **separately** from T004.
  - **BlockedBy**: T004.

**Phase 1 exit**: `cargo clippy` + `cargo fmt --check` + `wasm-pack test --node src/ -- --filter icon::tests` all green. `ICON_OPTIONS.len() == 12`; zero emoji. SC-009 / SC-010 / SC-011 / FR-020 / FR-021 / FR-023 / FR-024 / FR-025 satisfied.

---

## Phase 2 — Bundle B: Daily view — `day_clamp` helper [test-first] + view scaffold

**Goal**: extract `clamp_day_to_month` into its own module with RED-first unit tests; build the `daily/` module tree; add the `#daily-nav` sidebar button; migrate the sessions-history e2e tap. The month-grid, sessions-timeline, and sessions-history-table blocks are extracted from `calendar.rs` into `daily/`; `calendar.rs` is **not** yet deleted (it still backs the Statistics route through Phase 3).

**Exit**: `cargo test --workspace --frozen -p presto-leptos daily::day_clamp::tests` green. The Daily view is reachable via `#daily-nav`; the month-grid, timeline, and sessions-history-table all render inside `daily-view`. `tapTab(page, "Daily")` resolves in e2e.

### Test-first pair — day_clamp helper (RED → GREEN, two separate commits)

- [ ] **T006** [B] [Phase 2] **[test-first RED]** Write the failing `daily::day_clamp::tests` unit tests in `src/src/components/daily/day_clamp.rs` before any implementation exists.
  - **Files**: `src/src/components/daily/day_clamp.rs` (new; test module only), `src/src/components/daily/mod.rs` (stub with `pub mod day_clamp;`), `src/src/components/mod.rs` (add `pub mod daily;`)
  - **Test cases to write** (per `data-model.md §day_clamp::clamp_day_to_month`):
    1. `clamp_no_clamp_may_31`: `clamp_day_to_month(31, May 2026)` → `May 31 2026` (31 days; no clamp)
    2. `clamp_june_31_to_june_30`: `clamp_day_to_month(31, June 2026)` → `June 30 2026` (30-day month)
    3. `clamp_feb_31_leap_year`: `clamp_day_to_month(31, Feb 2024)` → `Feb 29 2024` (leap year)
    4. `clamp_feb_31_non_leap`: `clamp_day_to_month(31, Feb 2025)` → `Feb 28 2025` (non-leap)
    5. `clamp_low_boundary`: `clamp_day_to_month(1, Feb 2026)` → `Feb 1 2026` (no clamp; low boundary)
    6. `clamp_backward_nav_july31_to_june`: `clamp_day_to_month(31, June 2026)` via prev-month from July 31 → `June 30 2026` (direction-agnostic; same result as case 2)
  - **Done-signal**: `cargo test --workspace --frozen -p presto-leptos daily::day_clamp::tests` **exits non-zero** (tests compile, `clamp_day_to_month` function not yet present). Commit the failing tests **separately** from the implementation.
  - **BlockedBy**: T003 (shared utils module in place; `daily/mod.rs` can import from it).

- [ ] **T007** [B] [Phase 2] **[test-first GREEN]** Implement `pub fn clamp_day_to_month(day_of_month: u32, target_month: DateTime<Utc>) -> DateTime<Utc>` in `src/src/components/daily/day_clamp.rs`.
  - **Files**: `src/src/components/daily/day_clamp.rs`
  - **Implementation details**: use `checked_add_months` / chrono's `NaiveDate::from_ymd_opt` to construct the target day within `target_month`'s year+month; if `day_of_month` exceeds the month's length, fall back to the last day. Reference behaviour: `CalendarView::on_next_month` at `src/src/components/calendar.rs:495-509`. The helper must be direction-agnostic (same path for prev and next).
  - **Done-signal**: `cargo test --workspace --frozen -p presto-leptos daily::day_clamp::tests` **exits zero** (all 6 cases pass). `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic && cargo fmt --check` green. Commit **separately** from T006.
  - **BlockedBy**: T006.

### Daily view scaffold and route wiring

- [ ] **T008** [B] [Phase 2] Create `src/src/components/daily/month_grid.rs`, `sessions_timeline.rs`, `sessions_history_table.rs` by extracting the corresponding blocks from `src/src/components/calendar.rs` (lines 542–602, 604–635, 644+). Preserve all e2e selector IDs at the string level (`#calendar-grid`, `#current-month`, `#prev-month`, `#next-month`, `#sessions-timeline`, `#timeline-hours`, `#selected-day-title`, `#sessions-table-body`).
  - **Files**: `src/src/components/daily/month_grid.rs` (new), `src/src/components/daily/sessions_timeline.rs` (new), `src/src/components/daily/sessions_history_table.rs` (new), `src/src/components/daily/mod.rs` (register the three submodules), `src/src/components/calendar.rs` (import the new submodules; keep the existing `CalendarView` route functional)
  - **Implementation details**: each extracted file becomes a standalone `pub fn` component. `month_grid.rs` preserves Sunday-first day-of-week header (FR-018) and `aria-current="date"` on today's cell (FR-018). `sessions_timeline.rs` preserves the empty-state "No sessions completed" label. `sessions_history_table.rs` keeps `#sessions-table-body` off-viewport (A20; do not add `display: block` — it remains hidden in the extraction). `calendar.rs` imports these via `use crate::components::daily::{month_grid::MonthGrid, ...}` and re-uses them in the existing `CalendarView` so nothing breaks.
  - **Done-signal**: `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic && cargo fmt --check` green. `cargo build --workspace --frozen` succeeds. The existing Statistics (Calendar) tab still renders in the browser without regression.
  - **BlockedBy**: T007.

- [ ] **T009** [B] [Phase 2] Build `src/src/components/daily/mod.rs` `DailyView` component: two-column layout (`id="daily-view"`), `month_cursor` + `selected_day` `RwSignal<DateTime<Utc>>` both seeded via `datetime_from_ms(BrowserClock.now_ms())`, day-cell click wires `selected_day` + `.selected` class, prev/next-month header buttons update `month_cursor` + call `clamp_day_to_month` to roll `selected_day`.
  - **Files**: `src/src/components/daily/mod.rs`
  - **Implementation details**: follows `data-model.md §Daily view state`. Two-column layout: left = `<MonthGrid>`, right = `<SessionsTimeline>` + `<SessionsHistoryTable>` (off-viewport). Clicking a day cell sets `selected_day` to that cell's date and adds the `.selected` CSS modifier (FR-016). `← Previous month / Next month →` buttons update `month_cursor`; `selected_day` rolls via `clamp_day_to_month(selected_day.get().day(), new_month_cursor)` (FR-017). Module-wide `#![allow(clippy::must_use_candidate, clippy::too_many_lines)]` with Leptos-`#[component]` justification.
  - **Done-signal**: `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic && cargo fmt --check` green. Component compiles.
  - **BlockedBy**: T008.

- [ ] **T010** [B] [Phase 2] Add the `Daily` variant to the route enum in `src/src/app.rs`, insert the `#daily-nav` sidebar button (between Calendar and Settings), and wire `id="daily-view"` to the route resolver; update `src/src/components/mod.rs` to register `pub mod daily;`.
  - **Files**: `src/src/app.rs` (sidebar at lines 580–616, route enum, route resolver), `src/src/components/mod.rs`
  - **Implementation details**: sidebar order after this task — Timer (`#timer-nav`) → Statistics/Calendar (`#calendar-nav`) → Daily (`#daily-nav`, `data-view="daily"`, `title="Daily"`, inner glyph `<i class="ph ph-calendar-check">`) → Settings (`#settings-nav-large`). The Calendar nav button's inner glyph swaps from `ri-calendar-line` to `<i class="ph ph-chart-line-up">` (A6). The existing `id="calendar-nav"` / `data-view="calendar"` / `id="calendar-view"` selectors are **preserved** (FR-001, FR-012, A6). Route enum gets `Daily` variant; route resolver adds `"daily" => view! { <DailyView /> }` mirroring the existing `"calendar"` branch. (FR-012, FR-013.)
  - **Done-signal**: `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic && cargo fmt --check` green. The `#daily-nav` button appears in the sidebar between Calendar and Settings. Clicking it shows `#daily-view`; other views gain `.hidden`.
  - **BlockedBy**: T009.

- [ ] **T011** [P] [B,X] [Phase 2] Update `tests/e2e/fixtures/screens.js` to extend `tapTab` JSDoc + body for the `'Daily'` tab; update `tests/e2e/sessions-history.spec.js:31` from `tapTab(page, "Calendar")` → `tapTab(page, "Daily")`.
  - **Files**: `tests/e2e/fixtures/screens.js` (lines 21, 23–34), `tests/e2e/sessions-history.spec.js` (line 31)
  - **Implementation details**: JSDoc union extends from `'Timer'|'Calendar'|'Settings'` to `'Timer'|'Calendar'|'Daily'|'Settings'` (FR-019). Add `'Daily'` branch in `tapTab` body pointing to `#daily-nav`. In `sessions-history.spec.js`, `tapTab(page, "Calendar")` → `tapTab(page, "Daily")` at line 31; everything downstream (`page.locator("#sessions-table-body")`, the `getByRole("row")` flow, the edit-modal assertions) is **unchanged** (CHK043, FR-019).
  - **Done-signal**: `npx playwright test sessions-history.spec.js --reporter=line` passes. No assertion changes below line 31.
  - **BlockedBy**: T010.

- [ ] **T012** [P] [B,X] [Phase 2] Write `tests/e2e/daily.spec.js` — new e2e spec covering Daily view routing (SC-006) and day-cell selection (SC-007).
  - **Files**: `tests/e2e/daily.spec.js` (new)
  - **Test cases**:
    - SC-006: click `#daily-nav`; assert `#daily-view` not hidden; assert `#timer-view`, `#calendar-view`, `#settings-view` hidden; assert two-column layout exists.
    - SC-007: with seeded session fixtures, click `#daily-nav`, assert timeline shows today's sessions; click a different day cell; assert `#selected-day-title` text updates to the clicked day's label; assert the clicked cell gains `.selected` class; assert the timeline re-binds.
    - Empty-state: click a day with zero sessions; assert "No sessions completed" label is present.
  - **Done-signal**: `npx playwright test daily.spec.js --reporter=line` passes. Two test blocks green.
  - **BlockedBy**: T011.

**Phase 2 exit**: `cargo test --workspace --frozen` green (day_clamp 6 cases). `cargo clippy` + `cargo fmt --check` green. `npx playwright test daily.spec.js sessions-history.spec.js --reporter=line` passes. FR-012 / FR-013 / FR-014 / FR-015 / FR-016 / FR-017 / FR-018 / FR-019 / SC-006 / SC-007 / SC-008 satisfied.

---

## Phase 3 — Bundle A: Statistics view rework + reusable `BarChart`

**Goal**: rename / restructure `CalendarView` into `StatisticsView` with four period tabs (Daily / Weekly / Monthly / Yearly), a single reusable `BarChart` component, a per-period navigator, and a tag-usage pie. Remove the right-column mini-calendar + Today's Sessions block from Statistics (those now live in the Daily view). Delete `calendar.rs` at end of phase.

**Exit**: `cargo clippy` + `cargo fmt --check` green; `calendar.rs` deleted; `pub mod calendar;` removed from `mod.rs`; Statistics view renders four period tabs with the bar chart, navigator, and tag-usage pie for each. FR-001 / FR-002 / FR-003 / FR-004 / FR-005 / FR-006 / FR-007 / FR-008 / FR-009 / FR-010 / FR-011 / FR-019 / SC-001 / SC-002 / SC-003 / SC-004 / SC-005 satisfied.

- [ ] **T013** [A] [Phase 3] Create `src/src/components/stats/mod.rs`, `bar_chart.rs`, `period_selector.rs`, `period_nav.rs`, `tag_usage_pie.rs`; register `pub mod stats;` in `src/src/components/mod.rs`.
  - **Files**: `src/src/components/stats/mod.rs` (new), `src/src/components/stats/bar_chart.rs` (new), `src/src/components/stats/period_selector.rs` (new), `src/src/components/stats/period_nav.rs` (new), `src/src/components/stats/tag_usage_pie.rs` (new), `src/src/components/mod.rs`
  - **Implementation details**:
    - `stats/mod.rs`: `StatisticsView` component with `id="calendar-view"` (preserved — e2e contract), `data-view="calendar"` preserved (A6 / FR-001). Holds `RwSignal<Period>` seeded to `Period::Weekly` (FR-003 / SC-001) and `RwSignal<Cursor>` seeded to the Weekly anchor via `datetime_from_ms`. Imports the four submodules. Removes the right-column mini-calendar + Today's Sessions block (FR-019 — those now live in Daily view). Module-wide `#![allow(clippy::must_use_candidate, clippy::too_many_lines)]` with Leptos-`#[component]` justification.
    - `stats/bar_chart.rs`: single `pub fn BarChart(props: BarChartProps) -> impl IntoView` — exactly one definition (SC-002). Renders `bar_values.len()` bars; bar height fraction = `(value / max_scale).clamp(0.0, 1.0)` with `min_bar_height_px` floor applied when value == 0 (FR-006 / SC-004). Each bar gets `.bar` CSS class (for SC-003 DOM-node counting). Accepts `BarChartProps { max_scale: u32, x_axis_labels: Vec<String>, bar_values: Vec<u32>, min_bar_height_px: u32 }` per `data-model.md §BarChartProps`.
    - `stats/period_selector.rs`: four-tab selector emitting `Period` on click (FR-002). Tab swap resets `Cursor` to the new period's "current" anchor (FR-008 / SC-005).
    - `stats/period_nav.rs`: branches on `Period` for prev/next labels and range-label format. Weekly variant preserves `#prev-week`, `#next-week`, `#week-range` (FR-009 / A13). New selectors: `#prev-day`/`#next-day`/`#day-range` (Daily), `#prev-month-period`/`#next-month-period`/`#month-range` (Monthly), `#prev-year`/`#next-year`/`#year-range` (Yearly) — FR-007.
    - `stats/tag_usage_pie.rs`: per-period tag-usage pie + legend (FR-010). Static-only v1 (FR-050 / CHK042). Empty-state: "No tagged sessions in this period".
    - `BarChart` instantiated four times from `stats/mod.rs` with per-period `BarChartProps`: Daily (24 bars, `max_scale=60`, labels `"00:00"`.."23:00"`); Weekly (7 bars, `max_scale=max(20, observed_max)`, labels `"Mon"`.."Sun"`); Monthly (28–31 bars, `max_scale=max(50, observed_max)`, labels `"1"`.."31"`); Yearly (12 bars, `max_scale=max(100, observed_max)`, labels `"Jan"`.."Dec"`). Per-period floor rounding per `data-model.md §BarChartProps` (Weekly/Monthly nearest-10, Yearly nearest-50).
  - **Done-signal**: `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic && cargo fmt --check` green. `cargo build --workspace --frozen` succeeds. Four period tabs visible in browser; Weekly is selected on cold-load.
  - **BlockedBy**: T010 (sidebar + route wiring in place; Daily view extraction complete).

- [ ] **T014** [A] [Phase 3] Delete `src/src/components/calendar.rs` and remove `pub mod calendar;` from `src/src/components/mod.rs`; remove the `use crate::components::calendar::CalendarView;` import and Calendar-view host from `src/src/app.rs`.
  - **Files**: `src/src/components/calendar.rs` (deleted), `src/src/components/mod.rs`, `src/src/app.rs`
  - **Implementation details**: by this task, all content from `calendar.rs` has been migrated to `stats/` (Statistics-view content) and `daily/` (month-grid, sessions-timeline, sessions-history-table). `app.rs` now imports `StatisticsView` from `crate::components::stats::mod::StatisticsView`. Confirm the existing `id="calendar-view"` is on `StatisticsView`, not on a deleted element.
  - **Done-signal**: `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic && cargo fmt --check` green. `ls src/src/components/calendar.rs` returns "No such file". `grep -r 'calendar' src/src/components/mod.rs` returns zero hits.
  - **BlockedBy**: T013.

- [ ] **T015** [P] [A,X] [Phase 3] Write optional wasm-bindgen-tests for `bar_chart.rs` covering SC-002, SC-003, SC-004 (bar count per period; all-zero bars render at min-height floor).
  - **Files**: `src/src/components/stats/bar_chart.rs` (test module)
  - **Test cases** (recommended, not RED-first — see `contracts/components.md §Contract 2`):
    - SC-002: `grep -c "pub fn BarChart" src/src/components/stats/bar_chart.rs` returns 1.
    - SC-003: for each `Period`, construct `BarChartProps` with period's bar count (24 / 7 / 28..31 / 12); render; count `.bar` DOM nodes; assert equality.
    - SC-004: `BarChartProps { max_scale: 0, bar_values: vec![0; 7], x_axis_labels: vec!["Mon".into(); 7], min_bar_height_px: 4 }` → every `.bar` height ≥ 4 px.
  - **Done-signal**: `wasm-pack test --node src/ -- --filter bar_chart::tests` exits 0. Or grep check returns 1. `cargo clippy` green.
  - **BlockedBy**: T013.

- [ ] **T016** [P] [A,X] [Phase 3] Update `tests/e2e/calendar-navigation.spec.js` to split the existing single-test into two separate `test(...)` blocks: week-navigation under Statistics view (`tapTab(page, "Calendar")`) and month-navigation under Daily view (`tapTab(page, "Daily")`).
  - **Files**: `tests/e2e/calendar-navigation.spec.js`
  - **Implementation details** (FR-019): the current file has one `tapTab(page, "Calendar")` at line 6 covering both week-nav (lines 9–30, `#prev-week`/`#next-week`/`#week-range`) and month-nav (lines 33–46, `#prev-month`/`#next-month`/`#current-month`). Split into: `test("week navigation under Statistics view", ...)` using `tapTab(page, "Calendar")` covering `#prev-week`/`#next-week`/`#week-range`; and `test("month navigation under Daily view", ...)` using `tapTab(page, "Daily")` covering `#prev-month`/`#next-month`/`#current-month`. Both tests stay in the same file. No selector string changes.
  - **Done-signal**: `npx playwright test calendar-navigation.spec.js --reporter=line` shows two passing test blocks.
  - **BlockedBy**: T014.

**Phase 3 exit**: `cargo clippy` + `cargo fmt --check` green. `calendar.rs` gone. `npx playwright test calendar-navigation.spec.js --reporter=line` passes (2 tests). Statistics view renders four tabs; `Period::Weekly` is default. SC-001 / SC-002 / SC-003 / SC-004 / SC-005 / FR-001–011 / FR-019 covered.

---

## Phase 4 — Bundle D: Control-button tooltips

**Goal**: add `data-tooltip=` attributes to `#stop-btn`, `#play-pause-btn`, `#skip-btn` in `timer/mod.rs`, wired to terse `Signal<String>`s derived from the same engine-state source as the existing verbose `aria-label=`/`title=`. Add the CSS rule (`:hover, :focus-visible`) in `src/style/timer.css`.

**Exit**: `cargo clippy` + `cargo fmt --check` green. Hovering or keyboard-focusing each button shows the correct terse tooltip within ≤150 ms. Verbose `aria-label=` and `title=` strings are preserved verbatim. FR-026 / FR-027 / FR-028 / FR-029 / FR-030 / FR-031 / SC-012 / SC-013 satisfied.

- [ ] **T017** [D] [Phase 4] Add the two derived `Signal<String>` pairs per control button in `src/src/components/timer/mod.rs` and wire `data-tooltip=move || terse_tooltip_*.get()` on the three buttons.
  - **Files**: `src/src/components/timer/mod.rs`
  - **Implementation details** (per `data-model.md §Tooltip-text signals`):
    - For `#stop-btn`: upstream `Signal<StopState>` derived from `engine.current_mode()` → `StopState::Reset` (Focus) or `StopState::Undo` (Break/LongBreak); downstream `verbose_label_stop` (`"Reset timer"` / `"Undo last session"`, preserving `mod.rs:1557` values verbatim) + `terse_tooltip_stop` (`"Reset"` / `"Undo"`).
    - For `#play-pause-btn`: upstream `Signal<PlayPauseState>` from `engine.is_running()` / `engine.is_paused()` / `engine.is_auto_paused()`; downstream `verbose_label_play` (always `"Start or pause timer"`, preserving `mod.rs:1583`) + `terse_tooltip_play` (`"Start"` idle / `"Pause"` running / `"Resume"` paused).
    - For `#skip-btn`: upstream single-state; downstream `verbose_label_skip` (always `"Skip session"`, preserving `mod.rs:1600`) + `terse_tooltip_skip` (always `"Skip session"`).
    - Add `data-tooltip=move || terse_tooltip_stop.get()` to `#stop-btn` at `:1556`; `data-tooltip=move || terse_tooltip_play.get()` to `#play-pause-btn` at `:1583`; `data-tooltip=move || terse_tooltip_skip.get()` to `#skip-btn` at `:1600`. Keep existing `aria-label=move || verbose_label_*.get()` and `title=move || verbose_label_*.get()` bindings (FR-026, CHK041).
  - **Done-signal**: `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic && cargo fmt --check` green. `cargo build --workspace --frozen` compiles. `grep "data-tooltip" src/src/components/timer/mod.rs | wc -l` returns 3.
  - **BlockedBy**: T005 (Phosphor font wired; icon refactor done — same file).

- [ ] **T018** [D] [Phase 4] Add the CSS tooltip rule to `src/style/timer.css` (or the file where the existing `#stop-btn` / `#play-pause-btn` / `#skip-btn` button styles live): `[data-tooltip]:hover::after, [data-tooltip]:focus-visible::after { ... }` with transition ≤150 ms (FR-030, SC-013).
  - **Files**: `src/style/timer.css` (or equivalent — confirm with `grep -rl "stop-btn" src/style/`)
  - **Implementation details**: the `[data-tooltip]` attribute selector drives a CSS pseudo-element tooltip (`:before` or `:after`) that appears on `:hover, :focus-visible`. Transition must be ≤150 ms (no native ~1 s delay). The rule MUST NOT introduce a `border-right` on `.sidebar` or any separator element between sidebar and `.main-content` (FR-036, SC-015). Do not alter any existing rule.
  - **Done-signal**: `grep -rE ":hover,\s*:focus-visible" src/style/` returns at least one hit on the new rule. `grep -rE "border-right" src/style/` returns the same 3 pre-existing pipboy hits and no new hits (SC-015). Hovering `#stop-btn` in the browser shows the tooltip within ≤150 ms.
  - **BlockedBy**: T017.

- [ ] **T019** [P] [D,X] [Phase 4] Write the optional wasm-bindgen-test tooltip-text matrix for SC-012 / FR-031.
  - **Files**: `src/src/components/timer/mod.rs` (test module, or a new `src/src/components/timer/tests.rs`)
  - **Test cases** (recommended, not RED-first): enumerate Focus × Break × LongBreak × running × idle × paused × auto-paused states. Assert: (a) `aria-label == title` per button per state (verbose pair stays paired); (b) `data-tooltip` matches the FR-027/028/029 terse mapping; (c) test MUST NOT assert `aria-label == data-tooltip` (CHK041).
  - **Done-signal**: `wasm-pack test --node src/ -- --filter timer::tests::tooltip` exits 0. The test is REQUIRED (FR-031); skipping is not acceptable. The test is not RED-first; it lands alongside the Bundle D implementation in T017/T018.
  - **BlockedBy**: T018.

- [ ] **T020** [P] [D,X] [Phase 4] Playwright keyboard-focus assertion for SC-013: tab-focus `#stop-btn`; assert tooltip becomes visible.
  - **Files**: `tests/e2e/daily.spec.js` or a new `tests/e2e/tooltips.spec.js`
  - **Test**: navigate to Timer view; `page.keyboard.press("Tab")` until `#stop-btn` is focused; assert the tooltip `::after` content or a visible tooltip element is present on the focused button.
  - **Done-signal**: `npx playwright test tooltips.spec.js --reporter=line` (or the amended `daily.spec.js`) passes.
  - **BlockedBy**: T018.

**Phase 4 exit**: `cargo clippy` + `cargo fmt --check` green. Three buttons carry `data-tooltip`. Verbose pair preserved. CSS rule triggers on `:hover, :focus-visible` within ≤150 ms. SC-012 / SC-013 / FR-026–031 satisfied.

---

## Phase 5 — `calendar.rs` cleanup verification + full e2e run

**Goal**: confirm `calendar.rs` is fully gone, lint is clean, and the full Playwright e2e suite passes (except visual-regression diffs which are expected and handled in Phase 6).

- [ ] **T021** [X] [Phase 5] Final lint + `cargo test` sweep after Phases 0–4 complete.
  - **Done-signal** (each must exit 0):
    - `cargo fmt --check`
    - `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic`
    - `cargo test --workspace --frozen`
    - `bash scripts/check-mock-drift.sh` (0 new Tauri commands — SC-016)
    - `grep -rE "border-right" src/style/` returns exactly 3 hits at `themes/pipboy.css:428,437,446` (SC-015)
    - `grep -rE 'ramazan|murdercode' specs/003-stats-redesign/` returns 0 hits (SC-018)
    - `grep -rE '[^\x00-\x7F]' src/src/components/stats/ src/src/components/daily/ src/src/components/icon.rs` returns 0 hits except intentional glyphs (SC-019)
    - `git diff main -- Cargo.lock` returns 0 new entries (SC-017)
  - **BlockedBy**: T016, T020.

- [ ] **T022** [P] [X] [Phase 5] Run full Playwright e2e suite (excluding visual-regression diffs); confirm all non-visual specs pass.
  - **Done-signal**: `npx playwright test --ignore-snapshots --reporter=line` exits 0. `npx playwright test calendar-navigation.spec.js sessions-history.spec.js daily.spec.js --reporter=line` all green.
  - **BlockedBy**: T021.

**Phase 5 exit**: Cargo build + lint + test clean. e2e suite (non-visual) green.

---

## Phase 6 — Bundle E (OPTIONAL, cut-line): PeakFocusTime line chart

**Goal**: add the 24-point SVG line chart below the Weekly tab's bar chart. If cycle timeline tightens, SKIP this phase entirely — delete `peak_focus_time.rs` and remove the `pub mod peak_focus_time;` line from `stats/mod.rs`. Bundles A–D are unaffected (FR-035, A15).

- [ ] **T023** [E] [Phase 6] **[OPTIONAL]** Create `src/src/components/stats/peak_focus_time.rs` with the 24-point SVG line chart and wire it into the Weekly variant of `StatisticsView`.
  - **Files**: `src/src/components/stats/peak_focus_time.rs` (new), `src/src/components/stats/mod.rs`
  - **Implementation details** (FR-032–034): 24 x-axis points (00:00–23:00); y = average minutes focused per hour over the last 7 days; peak-hour dot + label `"HH:00 — N min/day average"` (FR-033). When < 3 days of data: "Insufficient data — keep tracking to see your peak hour" label (FR-034). No division-by-zero / NaN guard. Add `pub mod peak_focus_time;` to `stats/mod.rs` and `<PeakFocusTime />` to the Weekly variant only (FR-048). No cargo feature flag; no runtime const-bool; source presence is the only toggle.
  - **Done-signal**: `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic && cargo fmt --check` green. PeakFocusTime chart renders below the Weekly bar chart in the browser. With < 3 days of data, "Insufficient data" label is shown.
  - **BlockedBy**: T013.

- [ ] **T024** [P] [E,X] [Phase 6] **[OPTIONAL]** Write optional wasm-bindgen-test for SC-020: 24 x-points + peak-hour dot positioning + insufficient-data fallback.
  - **Files**: `src/src/components/stats/peak_focus_time.rs` (test module)
  - **Done-signal**: `wasm-pack test --node src/ -- --filter peak_focus_time::tests` exits 0. Or test is explicitly deferred with a note.
  - **BlockedBy**: T023.

---

## Phase 7 — Visual-regression baselines (required)

**Goal**: update `tests/e2e/visual-regression.spec.js` to capture four per-period Statistics frames + one Daily frame (replacing the old `calendar-chromium-linux.png`); regenerate the baselines; verify no unintended diffs on untouched screens. FR-043 / FR-044 / CHK040 / SC-014 satisfied.

- [ ] **T025** [X] [Phase 7] Update `tests/e2e/visual-regression.spec.js` to replace the single Calendar snapshot with four per-period Statistics snapshots + one Daily snapshot; add sidebar masking for non-sidebar baselines.
  - **Files**: `tests/e2e/visual-regression.spec.js` (lines 47–51)
  - **Implementation details** (FR-043, FR-037):
    - Replace `tapTab(page, "Calendar")` + `toHaveScreenshot(["visual-regression", "calendar.png"])` with four sequential blocks — for each period (Daily, Weekly, Monthly, Yearly): `tapTab(page, "Calendar")` → click the period tab → `await expect(page).toHaveScreenshot(["visual-regression", "statistics-{daily,weekly,monthly,yearly}-chromium-linux.png"], { mask: [page.locator(".sidebar")] })`.
    - After the four statistics frames: `await tapTab(page, "Daily")` → `await expect(page).toHaveScreenshot(["visual-regression", "daily-chromium-linux.png"], { mask: [page.locator(".sidebar")] })`.
    - Add `mask: [page.locator(".sidebar")]` to **all** non-sidebar screenshots (FR-037) so the four-icons-vs-three-icons sidebar change does NOT cascade-regenerate every baseline.
    - Also regenerate `tag-manager-chromium-linux.png` (it legitimately differs because the dropdown now shows 9 Phosphor icons; update the existing snapshot call to include sidebar mask).
  - **Done-signal**: `npx playwright test visual-regression.spec.js --reporter=line` fails **only** on the expected baselines (statistics-{daily,weekly,monthly,yearly}, daily, tag-manager); no other baselines diff. `grep -c "statistics-.*chromium" tests/e2e/visual-regression.spec.js` returns 4.
  - **BlockedBy**: T022 (full e2e green first).

- [ ] **T026** [X] [Phase 7] Regenerate the six visual-regression baselines locally and commit them in a single visual-only commit.
  - **Files**: `tests/e2e/__screenshots__/visual-regression/` (4 new statistics PNGs, 1 new daily PNG, 1 regenerated tag-manager PNG, 1 deleted calendar PNG)
  - **Procedure**: `cd tests/e2e && npx playwright test visual-regression.spec.js --update-snapshots`. Visually review each new/regenerated PNG against the per-baseline justifications in `specs/003-stats-redesign/quickstart.md §5`. Stage: `git add __screenshots__/visual-regression/{statistics-daily,statistics-weekly,statistics-monthly,statistics-yearly,daily,tag-manager}-chromium-linux.png && git rm __screenshots__/visual-regression/calendar-chromium-linux.png`. Commit message: `chore(visual): replace calendar.png with 4 per-period statistics + 1 daily + regenerated tag-manager (feature 003)`.
  - **Done-signal**: `git status tests/e2e/__screenshots__/visual-regression/` shows exactly 6 PNGs added/modified + 1 PNG deleted. `npx playwright test visual-regression.spec.js --reporter=line` exits 0 (all snapshots match).
  - **BlockedBy**: T025.

- [ ] **T027** [X] [Phase 7] Confirm no unintended baseline diffs on untouched screens; document per-baseline justifications for the PR description.
  - **Files**: PR description (or `specs/003-stats-redesign/BASELINE_NOTES.md` if PR not yet open)
  - **Checks**: (1) `npx playwright test visual-regression.spec.js --reporter=line` exits 0. (2) `git diff tests/e2e/__screenshots__/visual-regression/ | grep "^Binary" | grep -v -E "statistics-|daily-|tag-manager-"` returns 0 lines (no unintended diffs — SC-014). (3) Include the six per-baseline justifications from `quickstart.md §5` verbatim in the PR description.
  - **Done-signal**: CI visual-regression run on the PR sees zero unexpected diffs. PR description carries the six per-baseline notes.
  - **BlockedBy**: T026.

---

## Phase 8 — Final gates

- [ ] **T028** [X] [Phase 8] Full final gate sweep before opening the PR.
  - **Done-signal** (each must exit 0):
    - `cargo fmt --check`
    - `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic`
    - `cargo test --workspace --frozen`
    - `bash scripts/check-mock-drift.sh` (SC-016: 0 new Tauri commands)
    - `bash scripts/check-lockfile-drift.sh` (SC-017: lockfile drift = 0 for runtime deps; devDep change already committed in T001)
    - `grep -rE "border-right" src/style/` — exactly 3 hits at `themes/pipboy.css:428,437,446` (SC-015)
    - `grep -rE 'ramazan|murdercode' specs/003-stats-redesign/` — 0 hits (SC-018)
    - `grep -c "pub fn BarChart" src/src/components/stats/bar_chart.rs` — returns 1 (SC-002)
    - `npx playwright test --reporter=line` — all specs pass (visual-regression exits 0 after T026)
  - **BlockedBy**: T027.

---

## Dependencies (compact)

- **Phase 0** (T001–T003): T001 and T002 sequential (font must exist before `index.html` is wired; T003 is independent). T001→T002; T003 independent.
- **Phase 1** (T004–T005): T004 (RED) → T005 (GREEN). Blocked by T002 + T003.
- **Phase 2** (T006–T012): T006 (RED) → T007 (GREEN) → T008 → T009 → T010 → T011, T012 (T011 and T012 parallel with each other after T010). Blocked by T003 (T006), T005 (no direct block, but Phase 1 should precede for font/icon integration).
- **Phase 3** (T013–T016): T013 → T014 → T016; T015 parallel with T014. Blocked by T010 (sidebar + route in place).
- **Phase 4** (T017–T020): T017 → T018 → T019, T020 (parallel with each other). Blocked by T005 (same file — timer/mod.rs).
- **Phase 5** (T021–T022): T021 → T022. Blocked by T016 + T020.
- **Phase 6** (T023–T024): T023 → T024. Blocked by T013. OPTIONAL — may be skipped.
- **Phase 7** (T025–T027): T025 → T026 → T027. Blocked by T022 (+ T024 if Phase 6 ships).
- **Phase 8** (T028): Blocked by T027.

### Parallel opportunities

- T001 + T003 are fully independent and can start in parallel (Phase 0).
- T002 can start as soon as T001 completes.
- Phase 1 (Bundle C) and Phase 2 (Bundle B) can run in parallel after Phase 0, since they touch different files (`icon.rs` vs `daily/`).
- Phase 4 (Bundle D) can run in parallel with Phase 3 (Bundle A) after Phase 1, since `timer/mod.rs` (Phase 4) and `stats/` (Phase 3) are different file regions.
- T011 + T012 are parallel after T010.
- T015 is parallel with T014 (Phase 3).
- T019 + T020 are parallel after T018 (Phase 4).
- T023 + T024 (Phase 6 optional) are parallel with Phase 5 after T013.

---

## Notes

- **RED/GREEN commits are NOT collapsed** for T004→T005 (icon parser) and T006→T007 (day-clamp). Each RED commit lands first with a failing test; each GREEN follows in a separate commit. Per AGENTS.md §Test-first commit ordering.
- **`calendar.rs` is not deleted until T014**. Through Phases 0–2 it remains in place so the existing Statistics (Calendar) route stays functional while the new modules are built.
- **`id="calendar-view"` and `id="calendar-nav"` are preserved** throughout — the Statistics view keeps these IDs for e2e compatibility (FR-001, A6, CHK043). Only the view title text changes from "Calendar & Statistics" to "Statistics".
- **Sidebar order** (Timer first per FR-012): `#timer-nav` → `#calendar-nav` (Statistics) → `#daily-nav` (Daily) → `#settings-nav-large`. Matches spec Acceptance Scenario 2.1.
- **No new Tauri commands** — `bash scripts/check-mock-drift.sh` must stay green throughout (FR-040 / SC-016).
- **Phosphor font is a vendored asset** (FR-022), not a runtime npm package. `Cargo.lock` is unchanged; only `tests/e2e/package-lock.json` carries the `@phosphor-icons/web` devDependency addition (SC-017 / FR-039).
- **No new on-disk schema changes** (FR-046). All new state is UI-side `RwSignal`; `sessions.json`, `manual-sessions.json`, `tags.json`, `settings.json` shapes are unchanged.
- **No Phosphor migration of existing emoji-icon tags** (FR-047). Legacy tags render via `IconClass::Glyph` fallback (T005 / SC-011). No batch-replace UI; no data migration.
- **Tooltip variants are limited to the three control buttons** (FR-049). Sidebar nav icons keep existing native `title=` only; no styled tooltips for sidebar icons in this feature.
- **No engine or persistence changes** (FR-045). Timer engine, manager state machines, and persistence helpers are untouched.
- **All new UI strings are English** (FR-038 / SC-019). Verified by `grep -rE '[^\x00-\x7F]' src/src/components/stats/ src/src/components/daily/ src/src/components/icon.rs` in T021.
- **No `#[allow(clippy::...)]` without inline principle-anchored justification** (FR-041). Module-wide `#![allow(clippy::must_use_candidate, clippy::too_many_lines)]` is permitted only with the Leptos `#[component]` + single-`view!`-macro-body rationale (carried from `calendar.rs` precedent into `stats/` and `daily/`).
- **No spec artefact references any fork or upstream repository** (FR-042 / SC-018). Verified by `grep -rE 'ramazan|murdercode' specs/003-stats-redesign/` in T021 and T028.
- **Bundle E (Phase 6) is the explicit cut-line** (FR-035 / A15). If skipped: omit `peak_focus_time.rs`; remove `pub mod peak_focus_time;` from `stats/mod.rs`. No other change needed; no baseline impact.
- **`#sessions-table-body` moves to Daily view** (CHK043 / FR-019). It stays off-viewport in its new host (`daily/sessions_history_table.rs`) so feature 002's Title column work continues without a visual-regression diff.
