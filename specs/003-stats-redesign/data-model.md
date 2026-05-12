# Data Model: Feature 003 — UI-side entity shapes

**Branch**: `003-stats-redesign` | **Date**: 2026-05-12

> **No new on-disk entities.** All new state in this feature is UI-side, held in Leptos `RwSignal`s scoped to a single view component or to the timer component's reactive context. Nothing is persisted to `settings.json`, `sessions.json`, `manual-sessions.json`, or `tags.json`. Nothing serialises across the Tauri bridge (FR-002, FR-040). The "entities" below are the typed UI-state shapes the new modules consume.

## `Period` (enum, session-local UI state)

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Period {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}
```

- **Where held**: `RwSignal<Period>` inside `src/src/components/stats/mod.rs`'s `StatisticsView` component.
- **Seed**: `Period::Weekly` on cold-load (FR-003, SC-001 — Weekly matches the pre-rework default).
- **Transitions**: changes only via `period_selector.rs`'s tab click. No serialisation; no persistence (FR-002, A2).
- **Drives**: the period cursor (below), the `BarChartProps` that get instantiated, the per-period navigator widget's prev/next labels and range-label format, the period-scoped session filter for the tag-usage pie.
- **Principle anchor**: Closed sum type per Principle III.

## `Cursor` (enum, one variant per `Period`)

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cursor {
    Daily(DateTime<Utc>),    // anchored to a specific day
    Weekly(DateTime<Utc>),   // anchored to Monday of the week
    Monthly(DateTime<Utc>),  // anchored to first-of-month
    Yearly(DateTime<Utc>),   // anchored to first-of-year (Jan 1)
}
```

- **Where held**: `RwSignal<Cursor>` inside `StatisticsView`. Equivalent alternative: four separate `RwSignal<DateTime<Utc>>`s (one per variant). The single-enum form is preferred for the closed-sum-type guarantee.
- **Seed**: matches the current `Period`'s anchor (today / this week's Monday / first-of-this-month / first-of-this-year) at component mount. Time source: `BrowserClock.now_ms()` (the existing `src/src/components/browser_clock.rs` helper; identical to `CalendarView`'s current seed at the equivalent line).
- **Transitions on Period swap**: the cursor **resets** to the new period's "current" anchor (FR-008, A4, SC-005). Cursor state does NOT carry across period swaps. Rationale: swapping Yearly→Daily produces "today", not "January 1 of the previously-viewed year" — anchored in user intuition per A4.
- **Transitions on prev/next click**: decrement / increment by one period unit (one day / one week / one month / one year). End-of-month roll uses `checked_add_months` + the `day_clamp::clamp_day_to_month` helper (see below).
- **Principle anchor**: Closed sum type + `DateTime<Utc>` payload per Principle III (no string-typed dates; no free `i32` epochs).

## `BarChartProps` (component param struct, new)

```rust
#[derive(Clone, Debug)]
pub struct BarChartProps {
    pub max_scale: u32,            // y-axis ceiling in focus-minutes (period-specific floor applied)
    pub x_axis_labels: Vec<String>, // bar labels — one per bar
    pub bar_values: Vec<u32>,      // focus-minute total per bar — must match x_axis_labels.len()
    pub min_bar_height_px: u32,    // visual-floor: bars render at this height when value is 0
}
```

- **Where used**: input to the `pub fn BarChart(props: BarChartProps) -> impl IntoView` component in `src/src/components/stats/bar_chart.rs`. Constructed by `StatisticsView::mod.rs` once per period; passed into `<BarChart {props}/>`.
- **Invariants** (asserted at construction or by wasm-bindgen-test SC-003 / SC-004):
  - `x_axis_labels.len() == bar_values.len()` (parallel slices)
  - For Daily: `max_scale == 60` (fixed 60-min/hour ceiling — a focus session can't exceed 60 minutes within one wall-clock hour; A3)
  - For Weekly: `max_scale == max(20, round_up_to_nearest_10(bar_values.iter().max()))`
  - For Monthly: `max_scale == max(50, round_up_to_nearest_10(bar_values.iter().max()))`
  - For Yearly: `max_scale == max(100, round_up_to_nearest_50(bar_values.iter().max()))`
  - **Rounding policy** (for readable tick labels): non-Daily periods round `observed_max` up to the nearest multiple before comparing with the floor. Weekly and Monthly use nearest 10 (e.g., 87 → 90, 91 → 100, 90 → 90). Yearly uses nearest 50 (e.g., 91 → 100, 151 → 200) because hour-scale magnitudes make 10-minute ticks too dense. The rounding is applied to `observed_max` before the `max(floor, ...)` comparison so the floor itself is never rounded down.
  - `min_bar_height_px ≥ 4` (matches the existing 8 px floor pattern at `src/src/components/calendar.rs:495-501`; configurable so the CSS can tune the floor without a Rust-side change)
- **No serialisation**. The struct is constructed and consumed within the same render tick.
- **Principle anchor**: typed-struct boundary per Principle III; no `serde_json::Value` slop at the component edge.

## Daily view state (component-local)

```rust
// inside `src/src/components/daily/mod.rs`
// Real pattern: BrowserClock exposes only `fn now_ms(&self) -> i64`
// (verified at src/src/components/browser_clock.rs:15,20).
// The `datetime_from_ms` helper (see src/src/components/calendar.rs:46-49,
// also used at calendar.rs:318) wraps DateTime::<Utc>::from_timestamp_millis(now_ms)
// with a defensive epoch fallback:
//   fn datetime_from_ms(ms: i64) -> DateTime<Utc> {
//       DateTime::<Utc>::from_timestamp_millis(ms).unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
//   }
// When extracted to `daily/`, this helper MUST be promoted to a shared module
// (e.g., `src/src/components/utils/datetime.rs` or `src/src/time_utils.rs`) so
// that both `stats/` and `daily/` import the same definition without duplication.
let now_utc = datetime_from_ms(BrowserClock.now_ms());
let month_cursor: RwSignal<DateTime<Utc>> = RwSignal::new(now_utc);
let selected_day: RwSignal<DateTime<Utc>> = RwSignal::new(now_utc);
```

- **Where held**: two `RwSignal<DateTime<Utc>>`s in `DailyView`.
- **Seed**: both seed via `datetime_from_ms(BrowserClock.now_ms())` (FR-015; `BrowserClock::now_ms` is the only method — verified at `src/src/components/browser_clock.rs:15,20`; pattern from `src/src/components/calendar.rs:46-49,318`). Host-side test builds resolve `BrowserClock` to the unix epoch; e2e runs use the real wall clock (Edge Cases entry "Statistics → Weekly cursor seeded from `BrowserClock`").
- **Transitions**:
  - `month_cursor` changes on prev/next-month click in the month-grid header (FR-017).
  - `selected_day` changes on day-cell click (FR-016) AND on `month_cursor` change via the day-clamp helper: the day-of-month rolls forward to the same DoM in the new month if it exists, otherwise clamps to the last day of the new month (FR-017, SC-008, A1's Principle V exception).
- **Drives**: the month-grid render (highlights today's cell with `aria-current="date"`; highlights the selected day with `.selected` class modifier); the sessions-timeline panel (binds to `selected_day`'s session set); the sessions-history table at the bottom (also binds to `selected_day`).
- **Independence from Statistics's `Cursor::Daily`**: the Daily view's `(month_cursor, selected_day)` is an independent state pair held by `DailyView`, with no shared signal with Statistics's `Cursor::Daily`. Navigating Daily-view's month-grid does NOT update the Statistics view's daily cursor, and vice versa. Each view manages its own state.
- **Principle anchor**: `DateTime<Utc>` per Principle III (not strings; not three-int tuples).

### `day_clamp::clamp_day_to_month` helper (extracted, `[test-first]`)

```rust
// src/src/components/daily/day_clamp.rs
pub fn clamp_day_to_month(day_of_month: u32, target_month: DateTime<Utc>) -> DateTime<Utc> {
    // returns a DateTime anchored at:
    //   - day_of_month within target_month's year/month if it exists, OR
    //   - the last day of target_month if day_of_month exceeds that month's length.
    // Existing CalendarView::on_next_month at src/src/components/calendar.rs:495-509 is the reference behaviour.
}
```

- **Test cases** (RED-first per A1):
  - `clamp_day_to_month(31, May 2026)` → `May 31 2026` (no clamp; May has 31 days)
  - `clamp_day_to_month(31, June 2026)` → `June 30 2026` (clamps; June has 30 days)
  - `clamp_day_to_month(31, Feb 2024)` → `Feb 29 2024` (leap year)
  - `clamp_day_to_month(31, Feb 2025)` → `Feb 28 2025` (non-leap)
  - `clamp_day_to_month(1, Feb 2026)` → `Feb 1 2026` (low boundary; no clamp)
  - `clamp_day_to_month(31, June 2026)` reached **via prev-month from July 31** → `June 30 2026` (clamps; same result as the forward case — the helper is direction-agnostic; backward navigation through `← Previous month` hits the same clamp path)

## `IconClass` (enum, typed-prefix dispatch)

```rust
// src/src/components/icon.rs (or inlined into timer/mod.rs)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IconClass {
    Remix(String),    // payload: the glyph suffix, e.g. "brain-line" from "ri-brain-line"
    Phosphor(String), // payload: the glyph suffix, e.g. "cloud" from "ph-cloud"
    Glyph(String),    // payload: a raw grapheme to emit as text, e.g. "\u{1f9e0}"
}

impl IconClass {
    pub fn from_icon_name(name: &str) -> Self {
        if let Some(suffix) = name.strip_prefix("ri-") {
            if !suffix.is_empty() {
                return IconClass::Remix(suffix.to_string());
            }
        } else if let Some(suffix) = name.strip_prefix("ph-") {
            if !suffix.is_empty() {
                return IconClass::Phosphor(suffix.to_string());
            }
        }
        IconClass::Glyph(name.to_string())
    }
}
```

- **Where constructed**: at the input boundary — wherever `Tag.icon: String` is read into the renderer (e.g., `src/src/components/timer/mod.rs`'s tag list rendering callsites, the `#status-icon` derivation, the icon-picker preview at `#selected-icon-btn`).
- **Where consumed**: the `render(class: &IconClass) -> impl IntoView` function. Exhaustive `match` on the enum — no `if name.starts_with(...)` chain at the call site (FR-023).
- **Parser edge cases** (see `contracts/components.md` Contract 1 edge-case table for the full rationale):
  - `""` → `Glyph("")` (empty-icon-as-no-icon per A20; renders as empty `<i></i>`)
  - `"ri-"` → `Glyph("ri-")` (prefix-only with no suffix is data corruption; parser MUST require non-empty suffix after `strip_prefix`; same rule for `"ph-"`)
  - `"phone"` → `Glyph("phone")` (`starts_with("ph-")` requires the dash; un-dashed inputs are plain glyphs)
  - `" ri-foo"` → `Glyph(" ri-foo")` (no trimming; input layer is responsible for clean strings)
- **Render rules** (asserted by `icon::tests` per FR-025):
  - `IconClass::Remix(suffix)` → `<i class="ri-{suffix}"></i>` (no wrapper class)
  - `IconClass::Phosphor(suffix)` → `<i class="ph ph-{suffix}"></i>` (the outer `ph` is the Phosphor stylesheet's required wrapper class for the font face to bind)
  - `IconClass::Glyph(grapheme)` → `<i>{grapheme}</i>` (text content; `grapheme.is_empty()` → empty `<i>`, per A20 / Edge Cases "Tag with `icon = \"\"`")
- **Backward compatibility** (FR-024, SC-011): pre-rework tags persisted with `icon = "\u{1f9e0}"` etc. continue rendering as raw graphemes through the `Glyph` branch. No data migration; no on-disk write-back.
- **Principle anchor**: Closed sum type + boundary parsing per Principle III; covered by RED-first wasm-bindgen-test per FR-025 + Principle V.

## Tooltip-text signals (per control button)

Per button, the architecture is a **one-upstream, two-downstream** pattern:

1. **ONE upstream `Signal<ButtonState>`** — a closed-sum enum capturing the button's current logical state — derived once from `engine.current_mode()` + run-state predicates. The three upstream types are:
   - `ResetVsUndo` for `#stop-btn` (Focus → Reset; Break/LongBreak → Undo)
   - `StartVsPauseVsResume` for `#play-pause-btn` (idle → Start; running → Pause; paused/auto-paused → Resume)
   - `Skip` for `#skip-btn` (single variant; no state changes)
2. **TWO downstream `Signal<String>`s** — `verbose_label` and `terse_tooltip` — that both `.with()` the upstream signal and project to a String.

This guarantees: (a) one engine-state read per re-render (the upstream derivation fires once; both downstream strings consume it); (b) the verbose and terse strings never drift because they share the same `ButtonState` input.

```rust
// inside `src/src/components/timer/mod.rs`'s control-buttons region
//
// Step 1: one upstream signal per button (closed-sum ButtonState)
let stop_btn_state = Signal::derive(move || {
    if engine.with(|s| s.current_mode() == TimerMode::Focus) { StopState::Reset } else { StopState::Undo }
});

// Step 2: two downstream strings per button, both reading stop_btn_state
let verbose_label_stop = Signal::derive(move || match stop_btn_state.get() {
    StopState::Reset => "Reset timer".to_string(),
    StopState::Undo  => "Undo last session".to_string(),
});
let terse_tooltip_stop = Signal::derive(move || match stop_btn_state.get() {
    StopState::Reset => "Reset".to_string(),
    StopState::Undo  => "Undo".to_string(),
});
// analogous upstream + downstream pairs for play_pause and skip
```

- **Where held**: per button: one upstream `Signal<ButtonState>` + two downstream `Signal<String>`s (`verbose_label_*` and `terse_tooltip_*`), all reactive over `engine.current_mode()` + run-state predicates (`is_running`, `is_paused`, `is_auto_paused`).
- **Bindings**:
  - `aria-label=move \|\| verbose_label_*.get()`
  - `title=move \|\| verbose_label_*.get()`
  - `data-tooltip=move \|\| terse_tooltip_*.get()`
- **CHK041 invariant**: the verbose pair (`aria-label` == `title`) is the WCAG 4.1.2 accessible name; the terse `data-tooltip` is the visible UI tooltip. They are intentionally decoupled — the test (SC-012 / FR-031) asserts `aria-label == title` per button per state but MUST NOT assert `aria-label == data-tooltip`.
- **Per-state mapping**:
  - **`#stop-btn`** (FR-027):
    - Focus: verbose `"Reset timer"` / terse `"Reset"`
    - Break / LongBreak: verbose `"Undo last session"` / terse `"Undo"`
  - **`#play-pause-btn`** (FR-028): verbose `"Start or pause timer"` always; terse `"Start"` (idle) / `"Pause"` (running) / `"Resume"` (paused or auto-paused)
  - **`#skip-btn`** (FR-029): verbose `"Skip session"` / terse `"Skip session"` — single string, no state variants
- **Drift-impossibility**: both signals close over the same `engine` reactive source. A mode change fires both atomically; there is no "second copy of the engine state" to keep in sync.
- **Principle anchor**: Derived-from-single-source per Principle III + Principle VI (engine state is the truth; UI surfaces are projections).
