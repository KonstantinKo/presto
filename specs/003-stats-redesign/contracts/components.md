# Component Contracts: Feature 003

**Branch**: `003-stats-redesign` | **Date**: 2026-05-12

This feature adds **no new Tauri commands** (FR-040, SC-016). The mock-drift gate (`scripts/check-mock-drift.sh`) sees no new `#[tauri::command]` handlers and stays green without mock changes. The component contracts that this feature does add are two — both UI-side, both wasm-bindgen-test covered.

## Contract 1: `IconClass` enum + `IconClass::from_icon_name` parser

**Location**: `src/src/components/icon.rs` (or inlined into `timer/mod.rs` if the call-site count keeps it small — decided at Phase 1 task generation).
**Anchors**: FR-023, FR-024, FR-025, SC-010, SC-011, A9, A20 (Edge Cases "Tag with `icon = \"\"`"), Principle III.

### Public API

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IconClass {
    Remix(String),    // payload: glyph suffix (e.g. "brain-line")
    Phosphor(String), // payload: glyph suffix (e.g. "cloud")
    Glyph(String),    // payload: raw grapheme to emit as text
}

impl IconClass {
    pub fn from_icon_name(name: &str) -> Self;
}

pub fn render(class: &IconClass) -> impl IntoView;
```

### Parser branches

The `from_icon_name` constructor implements typed-prefix dispatch with three branches:

1. **`name.starts_with("ri-")`** → `IconClass::Remix(suffix)` where `suffix` is `name.strip_prefix("ri-").unwrap()`. Example: `"ri-brain-line"` → `Remix("brain-line".to_string())`.
2. **`name.starts_with("ph-")`** → `IconClass::Phosphor(suffix)`. Example: `"ph-cloud"` → `Phosphor("cloud".to_string())`.
3. **Else** (no recognised prefix, including the empty string) → `IconClass::Glyph(name.to_string())`. Example: `"\u{1f9e0}"` → `Glyph("\u{1f9e0}".to_string())`; `""` → `Glyph("".to_string())`.

The Glyph branch covers the legacy emoji-icon path (FR-024) and the `icon = ""` edge case per A20 — an empty payload propagates through `render` as an empty `<i></i>` (no glyph, no fallback to `ri-brain-line`). The constructor never panics; the parser is total over `&str`.

### Render rules

The `render` function exhaustively matches on the enum:

- `IconClass::Remix(suffix)` → `<i class={format!("ri-{suffix}")}></i>`
- `IconClass::Phosphor(suffix)` → `<i class={format!("ph ph-{suffix}")}></i>` (the outer `ph` wrapper class is **required** for the Phosphor `@font-face` to bind; the inner `ph-{suffix}` selects the glyph via `::before { content: "\xxxx"; }`)
- `IconClass::Glyph(grapheme)` → `<i>{grapheme}</i>` (text content; renders as the raw character via the system / app default font)

### Edge-case dispatch table

The following inputs require explicit handling that the three-branch summary above does not spell out:

| Input | Result | Rationale |
|---|---|---|
| `""` (empty string) | `IconClass::Glyph("")` | Falls to the Else branch; an empty input has no recognised prefix. Per A20 / Edge Cases "Tag with `icon = \"\"`", empty-icon-as-no-icon is the documented behaviour — the empty payload propagates as an empty `<i></i>`. |
| `"ri-"` (prefix-only, no suffix) | `IconClass::Glyph("ri-")` | The parser uses `strip_prefix("ri-")` and would yield a zero-length suffix `""`, producing `Remix("")` which renders `<i class="ri-"></i>` — a silent no-glyph. **The implementer MUST require a non-empty suffix**: if `strip_prefix("ri-")` returns `Some("")`, treat the input as unrecognised and fall to `Glyph("ri-")`. A prefix-only icon name is data corruption; making it visibly render as the literal string "ri-" is preferable to the silent empty-glyph masking the corruption. Same rule applies to `"ph-"` → `Glyph("ph-")`. |
| `"phone"` (un-dashed ph-prefix) | `IconClass::Glyph("phone")` | `starts_with("ph-")` requires the dash separator; `"phone"` does not match, so it falls to `Glyph("phone")`. Implementors MUST NOT widen the match to `starts_with("ph")` — that would incorrectly capture legitimate words. |
| `" ri-foo"` (leading whitespace) | `IconClass::Glyph(" ri-foo")` | The parser does NOT trim; leading/trailing whitespace prevents prefix matching and falls to the Glyph branch. The input layer (FR-021's new-tag icon picker) is responsible for delivering clean strings; the parser is not a sanitiser. |

### Assumptions cited

- **A20 / Edge Cases "Tag with `icon = \"\"`"**: empty-icon-as-no-icon is the documented behaviour. A pre-rework tag with no icon (corrupt or hand-edited) renders as an empty `<i>` — the parser's "Else" branch is the path; there is no sentinel-fallback to `ri-brain-line`. Masking the data-integrity issue would violate Principle III.
- **A9 / FR-024**: legacy emoji-icon tags continue rendering as raw graphemes via the `Glyph` branch. No data migration; no on-disk write-back. The constructor is the single point at which the typed dispatch happens; downstream code is exhaustive over the enum.

### Tests (RED-first per FR-025)

`src/src/components/icon.rs::tests` (wasm-bindgen-test):

1. `parser_remix_branch`: `IconClass::from_icon_name("ri-brain-line") == IconClass::Remix("brain-line".to_string())`.
2. `parser_phosphor_branch`: `IconClass::from_icon_name("ph-cloud") == IconClass::Phosphor("cloud".to_string())`.
3. `parser_glyph_branch_emoji`: `IconClass::from_icon_name("\u{1f9e0}") == IconClass::Glyph("\u{1f9e0}".to_string())`.
4. `parser_glyph_branch_empty`: `IconClass::from_icon_name("") == IconClass::Glyph(String::new())`.
4a. `parser_remix_prefix_only`: `IconClass::from_icon_name("ri-") == IconClass::Glyph("ri-".to_string())`. The parser rejects zero-length suffix and falls to Glyph.
4b. `parser_phosphor_prefix_only`: `IconClass::from_icon_name("ph-") == IconClass::Glyph("ph-".to_string())`. Same rule.
4c. `parser_undashed_ph_prefix`: `IconClass::from_icon_name("phone") == IconClass::Glyph("phone".to_string())`. `starts_with("ph-")` requires the dash; un-dashed strings beginning with `ph` are plain glyphs.
4d. `parser_leading_whitespace`: `IconClass::from_icon_name(" ri-foo") == IconClass::Glyph(" ri-foo".to_string())`. The parser does NOT trim.
5. `render_remix_emits_i_with_ri_class`: render `IconClass::Remix("brain-line".to_string())`; assert DOM contains `<i class="ri-brain-line">`.
6. `render_phosphor_emits_i_with_ph_wrapper_and_glyph`: render `IconClass::Phosphor("cloud".to_string())`; assert DOM contains `<i class="ph ph-cloud">`. **Both** the `ph` wrapper and the `ph-cloud` glyph class are required (SC-010).
7. `render_glyph_emits_text_content`: render `IconClass::Glyph("\u{1f9e0}".to_string())`; assert DOM is `<i>🧠</i>` (the raw grapheme). Covers SC-011 (pre-rework emoji tag round-trip).
8. `render_glyph_empty_emits_empty_i`: render `IconClass::Glyph(String::new())`; assert DOM is `<i></i>` (no children).

The RED commit lands first (test asserts fail because the module / parser doesn't exist); the GREEN commit follows (parser + render implemented; tests pass). Per AGENTS.md §Test-first commit ordering, the two commits are NOT collapsed.

## Contract 2: `BarChartProps` + the `BarChart` component

**Location**: `src/src/components/stats/bar_chart.rs`.
**Anchors**: FR-004, FR-005, FR-006, SC-002, SC-003, SC-004, Principle III.

### Public API

```rust
#[derive(Clone, Debug)]
pub struct BarChartProps {
    pub max_scale: u32,
    pub x_axis_labels: Vec<String>,
    pub bar_values: Vec<u32>,
    pub min_bar_height_px: u32,
}

#[component]
pub fn BarChart(props: BarChartProps) -> impl IntoView;
```

### Field semantics

- **`max_scale: u32`** — y-axis ceiling in focus-minutes. The bar at index `i` renders at height-fraction `(bar_values[i] as f64 / max_scale as f64).clamp(0.0, 1.0)` of the chart's pixel height, with the minimum-visible-height floor applied (see `min_bar_height_px`). Per-period floor policy (caller-applied; FR-005):
  - Daily: `max_scale == 60` (fixed; the 60-min/hour ceiling)
  - Weekly: `max_scale = max(20, bar_values.iter().max().copied().unwrap_or(0))`
  - Monthly: `max_scale = max(50, ...)`
  - Yearly: `max_scale = max(100, ...)`
- **`x_axis_labels: Vec<String>`** — one label per bar. Caller-constructed in the period-specific format (`"00:00"`, `"02:00"`, ... for Daily; `"Mon"`, `"Tue"`, ... for Weekly; `"1"`, `"2"`, ..., `"31"` for Monthly; `"Jan"`, `"Feb"`, ..., `"Dec"` for Yearly).
- **`bar_values: Vec<u32>`** — focus-minute total per bar; parallel to `x_axis_labels` (the BarChart never zips against a mis-sized values vec — it's the caller's responsibility to construct parallel slices).
- **`min_bar_height_px: u32`** — visual floor. Bars with value 0 render at this height so the chart never has zero-height bars (FR-006). Matches the existing 8 px floor pattern at the about-to-be-removed `calendar.rs:495-501`; configurable so the CSS can tune the floor without a Rust-side change. Minimum is 4 px per spec edge-case "Empty period bar chart".

### Invariants

1. **`x_axis_labels.len() == bar_values.len()`**: parallel slices. Asserted at construction or by SC-003's wasm-bindgen-test that counts `.bar` DOM nodes per period.
2. **`bar_values.iter().max().copied().unwrap_or(0) <= max_scale`**: caller-enforced via the per-period floor computation. The BarChart does NOT re-scale internally; it trusts the caller's `max_scale`.
3. **`min_bar_height_px >= 4`**: caller-enforced. The component does not silently coerce; an out-of-range value is the caller's bug.
4. **Empty-period rendering**: when `bar_values.iter().all(|v| *v == 0)`, every bar renders at `min_bar_height_px` (FR-006 + SC-004's component-level test). The chart still has labels; only the bar heights are uniform-floor.

### Per-period bar counts (SC-003)

- Daily: `x_axis_labels.len() == 24` (one per hour, 00:00–23:00)
- Weekly: `x_axis_labels.len() == 7`
- Monthly: `x_axis_labels.len() ∈ {28, 29, 30, 31}` (depends on the month's actual length)
- Yearly: `x_axis_labels.len() == 12`

### Tests (NO RED-first; e2e + wasm-bindgen-test default)

`src/src/components/stats/bar_chart.rs::tests`:

1. (SC-002) Grep / file-structure check: `grep -c "pub fn BarChart" src/src/components/stats/bar_chart.rs` returns 1. Asserted in CI, not at runtime.
2. (SC-003) For each `Period`, construct `BarChartProps` with the period's expected bar count, render via wasm-bindgen-test, count `.bar` DOM nodes; assert equality with the expected count.
3. (SC-004) Construct `BarChartProps { max_scale: 0, bar_values: vec![0; 7], x_axis_labels: vec!["Mon".into(); 7], min_bar_height_px: 4 }`; render; assert every `.bar` element's computed style `height >= 4 px` (FR-006).

These tests are recommended but not RED-first — the `BarChart` is UI-rendering code per Principle V's documented carve-out. The two RED-first tests in this feature are `daily::day_clamp::tests` (Contract: A1's exception, see data-model.md) and `icon::tests` (Contract 1 above).

## Tauri command contract: none

This feature introduces **zero** new Tauri commands (FR-040, SC-016). The Statistics view, Daily view, Phosphor renderer, and control-button tooltips all source their data from existing context signals seeded by the existing `load_sessions` / `load_tags` / `load_settings` commands in `crates/presto-ipc/`. The Tauri bridge mock at `tests/e2e/fixtures/tauriMock.js` is unmodified by this feature (Principle VI / FR-040). The mock-drift gate stays green.
