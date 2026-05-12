# Quickstart: Feature 003 — Statistics Redesign + Daily View + Phosphor Icons + Tooltips

**Branch**: `003-stats-redesign` | **Date**: 2026-05-12

Contributor's path to building, navigating, and exercising the feature end-to-end.

## 1. Where the new surfaces live

| Surface | Module | Anchor |
|---|---|---|
| Renamed Statistics view (period tabs + reusable bar chart + tag-usage pie) | `src/src/components/stats/{mod,bar_chart,period_selector,period_nav,tag_usage_pie}.rs` | Bundle A |
| New Daily drill-down view | `src/src/components/daily/{mod,month_grid,sessions_timeline,sessions_history_table,day_clamp}.rs` | Bundle B |
| Icon-renderer typed dispatch | `src/src/components/icon.rs` (or inlined into `timer/mod.rs`) | Bundle C |
| Phosphor webfont + CSS | `src/assets/icons/phosphor/` (vendored copy-dir target) | Bundle C |
| Control-button tooltips | `src/src/components/timer/mod.rs:1556`, `:1583`, `:1600` + CSS rule in `src/style/timer.css` | Bundle D |
| Sidebar fourth nav + route enum | `src/src/app.rs:580-616` | Bundles A + B |
| Optional Peak Focus Hour line chart | `src/src/components/stats/peak_focus_time.rs` (build-config gated) | Bundle E |

## 2. Local build

The standard contributor commands are unchanged from the rest of the project:

```bash
# Frontend (Leptos / Trunk)
cd src/
trunk serve              # dev server at localhost:1420
trunk build --release    # production WASM bundle

# Backend lint + test
cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic
cargo fmt --check
cargo test --workspace --frozen

# Tauri dev — DO NOT run in CI / agentex worktrees (GUI deps)
cargo tauri dev
```

The lockfile-drift gate uses `cargo build --frozen` and `npm ci`; never `cargo build` plain or `npm install` — both would mutate the lockfile and fail CI (Principle IX).

## 3. Vendoring Phosphor (one-time, before first build)

Bundle C requires the Phosphor regular-weight webfont vendored under `src/assets/icons/phosphor/`. Procedure:

```bash
# 1. Add the npm devDependency (lockstep — lockfile is regenerated)
cd tests/e2e/
npm install --save-dev @phosphor-icons/web

# 2. Copy the regular-weight font files + CSS into src/assets/icons/phosphor/
mkdir -p ../../src/assets/icons/phosphor
cp node_modules/@phosphor-icons/web/src/regular/Phosphor.{eot,svg,ttf,woff,woff2} \
   ../../src/assets/icons/phosphor/
cp node_modules/@phosphor-icons/web/src/regular/style.css \
   ../../src/assets/icons/phosphor/phosphor.css

# 3. Commit the vendored assets + the regenerated lockfile in the SAME commit (Principle IX).
git add ../../src/assets/icons/phosphor/ tests/e2e/package.json tests/e2e/package-lock.json
git commit -m "vendor: Phosphor regular-weight webfont for feature 003"
```

The committed assets are the canonical artefacts; the `@phosphor-icons/web` `devDependency` exists only so the next re-vendoring (e.g. on a Phosphor major bump) is reproducible. The WASM build does **not** link against the npm package; only the committed `assets/icons/phosphor/*` files end up in the dist tree, via `src/index.html`'s `<link data-trunk rel="copy-dir" ...>` directive.

## 4. Running the relevant e2e specs

```bash
cd tests/e2e/

# All e2e
npx playwright test

# The specs this feature touches
npx playwright test calendar-navigation.spec.js          # migrated to tapTab("Daily")
npx playwright test sessions-history.spec.js              # migrated to tapTab("Daily")
npx playwright test visual-regression.spec.js             # 4 new statistics frames + 1 daily frame + regenerated tag-manager
npx playwright test daily.spec.js                         # NEW — SC-006, SC-007

# Headed mode (manual visual review)
npx playwright test --headed --debug daily.spec.js
```

The test environment uses Vite (not `tauri dev`) per `tests/e2e/CLAUDE.md`; the Tauri bridge is mocked at `tests/e2e/fixtures/tauriMock.js`. No new mock entries are needed for this feature (FR-040 / Principle VI).

## 5. Regenerating visual-regression baselines (FR-043 / CHK040)

This feature regenerates six baselines. Run the update against just the visual-regression spec, then **visually review each PNG one-by-one** before committing.

```bash
cd tests/e2e/
npx playwright test visual-regression.spec.js --update-snapshots

# Review each regenerated PNG against the per-baseline justifications below
ls -la __screenshots__/visual-regression/

# Commit in a single visual-only commit
git add __screenshots__/visual-regression/{statistics-*,daily-,tag-manager-}chromium-linux.png
git rm __screenshots__/visual-regression/calendar-chromium-linux.png
git commit -m "chore(visual): replace calendar.png with 4 per-period statistics + 1 daily + regenerated tag-manager (feature 003)"
```

### Per-baseline justifications (copy-paste into the PR description verbatim per FR-043)

- `statistics-daily-chromium-linux.png`: new Daily period variant of the renamed Statistics view; 24 hourly bars with the fixed 60-min/hour ceiling; `#prev-day` / `#next-day` / `#day-range` navigator widget.
- `statistics-weekly-chromium-linux.png`: cold-load default frame of the renamed Statistics view; supersedes `calendar-chromium-linux.png`. Weekly bar chart preserved; right-column mini-calendar + Today's Sessions panel removed (moved to the new Daily view per FR-019).
- `statistics-monthly-chromium-linux.png`: new Monthly period variant; 28–31 day-bars with a ≥ 50 min floor; `#prev-month-period` / `#next-month-period` / `#month-range` navigator.
- `statistics-yearly-chromium-linux.png`: new Yearly period variant; 12 month-bars labelled Jan–Dec with a ≥ 100 min floor; `#prev-year` / `#next-year` / `#year-range` navigator.
- `daily-chromium-linux.png`: new Daily drill-down view; two-column layout with the migrated month-grid on the left and the migrated sessions timeline + off-viewport sessions-history table on the right.
- `tag-manager-chromium-linux.png`: tag-picker dropdown now shows 12 icon options (3 remixicon + 9 Phosphor); the 5 emoji entries removed. No other layout change.

If CI shows a diff on any **other** baseline (`timer`, `settings-*`, `update-notification`), treat it as a regression in code and investigate — do NOT absorb into the baseline (SC-014). The sidebar mask in non-sidebar baselines (per FR-037) is what prevents the four-icons-vs-three-icons sidebar change from cascade-regenerating every baseline.

## 6. Running the test-first units

Two RED-first wasm-bindgen-tests (per A1's Principle V exception and per FR-025):

```bash
# Day-clamp helper (A1's exception — pure time-keeping math)
cargo test -p presto-leptos --frozen daily::day_clamp::tests

# Icon-renderer dispatch (FR-025 — boundary parser + closed-enum dispatch)
wasm-pack test --node src/  -- --filter icon::tests
```

The RED commit lands first (tests fail because the module doesn't exist); the GREEN commit follows (implementation lands; tests pass). Per AGENTS.md §Test-first commit ordering, the two commits are NOT collapsed.

## 7. Verifying the constitution check at PR time

Manual checks that run as part of code review (not CI):

```bash
# SC-015: no border-right on sidebar
grep -rE "border-right" src/style/sidebar.css src/style/layout.css
# expected: zero hits, before and after the PR

# SC-016: no new Tauri commands / events / network egress
git diff main -- src-tauri/ | grep -E "#\[tauri::command\]|events::emit|reqwest|fetch\("
# expected: zero diff additions

# SC-017: no new runtime lockfile entries (devDependencies are OK)
git diff main -- Cargo.lock
# expected: zero diff (Cargo.lock unchanged this feature)

# SC-018: no fork attribution in any spec-kit artefact
grep -rE 'ramazan|murdercode|github\.com/' specs/003-stats-redesign/
# expected: zero hits

# SC-019: English-only UI strings (sample check on new modules)
grep -rE '[^\x00-\x7F]' src/src/components/stats/ src/src/components/daily/ src/src/components/icon.rs
# expected: zero hits except for intentional glyphs (e.g. \u{2026} ellipsis)
```

## 8. If Bundle E is cut

Bundle E (PeakFocusTime line chart) is the explicit cut-line per A15 / FR-035. If the cycle's timeline tightens after Phase 5, defer it to a follow-up issue. The Weekly tab simply renders without the Peak Focus Hour panel; Bundles A–D are unaffected (Story 6 Acceptance 3). To cut: omit `src/src/components/stats/peak_focus_time.rs`, remove the `pub mod peak_focus_time;` line from `src/src/components/stats/mod.rs`, and remove the `<PeakFocusTime />` instantiation from the Weekly variant's render. No e2e or visual-regression changes are needed (the line chart had no dedicated baseline).
