# Data Model — Overtime Button Treatment

> Phase 1. Types, signals, and invariants introduced or extended by this feature. The engine layer is **untouched** (feature 006 already shipped the branch-B.2 path); this document covers only the IPC widening and UI-layer derived types.

## Persisted types

### `ShortcutSettings` — extended

**File**: `crates/presto-ipc/src/settings.rs:113-127`

**Today** (pre-feature):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ShortcutSettings {
    pub start_stop: Option<String>,
    pub reset: Option<String>,
    pub skip: Option<String>,
}
```

**Feature 007**:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ShortcutSettings {
    pub start_stop: Option<String>,
    pub reset: Option<String>,
    pub skip: Option<String>,
    /// Feature 007. Optional binding for the Abort action (used as a
    /// keyboard-accessible discard during overtime; usable from any
    /// running state). `None` = unbound. Default is `None`.
    pub abort: Option<String>,
}
```

**`Default` impl**:

```rust
impl Default for ShortcutSettings {
    fn default() -> Self {
        Self {
            start_stop: Some("CommandOrControl+Alt+Space".to_string()),
            reset: Some("CommandOrControl+Alt+R".to_string()),
            skip: Some("CommandOrControl+Alt+S".to_string()),
            // Intentional asymmetry: abort defaults to None (unbound) per FR-019.
            // The three sibling fields above are pre-bound as a convenience for
            // users who never open Settings. Abort is opt-in — it is primarily
            // a power-user escape hatch during overtime. Do not "fix" this
            // asymmetry without a spec revision.
            abort: None,
        }
    }
}
```

**Invariants**:

- `abort` is `Option<String>`. `None` means "user has not bound the shortcut" — the registration loop in `register_global_shortcuts` skips the binding entirely (existing branch at `src-tauri/src/lib.rs:447`: `if let Some(ref shortcut_str) = shortcut_str { … }`). **Default**: `None` (unbound). Intentional asymmetry vs the existing three pre-bound shortcuts — FR-019 mandates Abort defaults to unbound so users opt-in. Sibling fields' pre-bound defaults are unchanged.
- When `Some(s)`, `s` is a Tauri shortcut spec (e.g., `"CommandOrControl+Alt+W"`). Parsing happens Rust-side via `s.parse::<Shortcut>()` (line 448); invalid specs return `BridgeError::Internal { msg: … }`.
- Wire shape: pre-feature `settings.json` (missing the `abort` key) deserialises to `abort: None` via `serde`'s default behaviour for `Option<T>` on a missing key — no `#[serde(default)]` attribute required (mirrors the existing three nullable fields' precedent).

**Migration**: none. The widening is forward-compatible at the JSON level.

**Round-trip tests** (in `#[cfg(test)] mod tests` of `crates/presto-ipc/src/settings.rs`):

| Test | Asserts |
|---|---|
| `shortcut_settings_with_abort_roundtrips` | `Some("CommandOrControl+Alt+W")` serialises + deserialises identically. |
| `shortcut_settings_with_unbound_abort_roundtrips` | `None` serialises to `null` and back to `None`. |
| `shortcut_settings_missing_abort_field_defaults_to_none` | Pre-feature settings JSON (no `abort` key) deserialises to `abort: None`. |

## Derived UI types — unchanged

### `RunState` enum — NOT modified

**File**: `src/src/components/timer/mod.rs:216-243`

**Decision**: keep `RunState` exactly as feature 006 shipped it (`Idle | Running | Paused`). Overtime is layered on top as a separate boolean dimension, not a fourth variant.

**Why**: per spec Constitutional Anchor III — "overtime is a derived predicate, not a new run-state." A fourth variant would couple the engine's run-state bools to the UI-presentation overtime predicate; the engine has no concept of "overtime as a state" (the engine's `time_remaining_secs_signed()` going negative is the only signal, and it can be true under both Running and Paused depending on smart-pause behaviour).

**Invariant preserved**: `RunState::from_engine` is exhaustive over `(is_running, is_paused, is_auto_paused)` and the `debug_assert!` at line 231 still rejects illegal states.

### `is_overtime` signal — reused, NOT redefined

**File**: `src/src/components/timer/mod.rs:1130`

```rust
let is_overtime = Signal::derive(move || engine.with(|s| s.time_remaining_secs_signed() < 0));
```

**Invariants** (already enforced by the engine):

- True only when the engine is in continuous-mode focus past zero. The engine sets `time_remaining_secs` to a non-negative value for all non-continuous-mode states; the signed wrapper at the consumer side reads the raw signed value.
- Can be true while `RunState::Paused` (smart-pause kicks in during overtime → engine's `is_paused` or `is_auto_paused` is true, `time_remaining_secs_signed()` is still negative).
- Becomes false the moment the focus session ends (Complete or Abort) — `engine.complete(clock)` sets `time_remaining_secs` to the next mode's positive duration; `engine.abort(clock)` resets to the current mode's positive duration.

### Overtime treatment gate — new, but expressed inline

There is **no new named signal** for "is the overtime treatment active". The gate is expressed inline at each consumer:

```rust
// CTA visibility, button class:overtime, button label flip, a11y removal:
matches!(run_state.get(), RunState::Running) && is_overtime.get()

// Countdown class:overtime (unchanged from today):
is_overtime.get()
```

**Why inline rather than a named signal**: the countdown gate and the matrix gate are **deliberately different** (the countdown stays orange during paused-overtime; the matrix does not). A single named signal would merge them, splitting later when re-divergence is needed. The inline form makes the two gates' independence visible at the call site. See `research.md` Decision 5 for the irreversibility rationale.

**Invariants enforced at consumer sites**:

- The CTA `class:visible` and the buttons' `class:overtime` evaluate the same predicate (`Running && is_overtime`) — they appear and disappear synchronously (SC-001, SC-006, SC-009).
- The a11y attributes (`aria-hidden`, `tabindex`) on `#stop-btn` and `#skip-btn` evaluate the same predicate — they apply only when the visual overtime treatment applies.

## Button-trio outcome by `(RunState, is_overtime)`

| `RunState` | `is_overtime` | Left slot | Center slot | Right slot |
|---|---|---|---|---|
| `Idle` | * (always false; engine cannot be in Idle and overtime simultaneously) | `+ Quick Log` (ghost) | `▶ Play` (filled) | `→ Skip Mode` (ghost) |
| `Running` | `false` | `✕ Abort` (ghost) | `⏸ Pause` (filled) | `! Note Distraction` (ghost) |
| `Running` | `true` | `✓ Complete` (ghost, overtime tint) | `✓ Complete` (filled, overtime tint) | `✓ Complete` (ghost, overtime tint) |
| `Paused` | `false` | `✕ Abort` (ghost) | `▶ Resume` (filled) | `✓ Complete` (filled) |
| `Paused` | `true` | `✕ Abort` (ghost) | `▶ Resume` (filled) | `✓ Complete` (filled) |

**Note the `Paused, true` row**: identical to `Paused, false`. Per FR-022 + spec `[BEST-GUESS PM DECISION]` #7, overtime treatment turns off when paused. The countdown stays orange (engine-level signal); the button matrix and CTA do not. This is the gate-divergence formalised above.

**Illegal-state guard**: `(Idle, true)` is impossible — the engine cannot present `is_running == false` and `time_remaining_secs_signed() < 0` simultaneously, because Idle implies `time_remaining_secs` equals the current mode's duration (positive). No `debug_assert!` needed; the engine state machine enforces it.

## Outcome of clicks per `(RunState, is_overtime, slot)`

| `RunState` | `is_overtime` | Slot | Click dispatches to |
|---|---|---|---|
| `Running` | `false` | Left | `on_abort` (engine `abort(clock)`) |
| `Running` | `false` | Center | `on_play_pause` (engine `pause(clock)`) |
| `Running` | `false` | Right | `on_open_distraction` (modal — no engine call) |
| `Running` | `true` | Left | `on_complete` (engine `complete(clock)` → branch B.2) |
| `Running` | `true` | Center | `on_complete` (engine `complete(clock)` → branch B.2) |
| `Running` | `true` | Right | `on_complete` (engine `complete(clock)` → branch B.2) |
| `Paused` | * | Left | `on_abort` |
| `Paused` | * | Center | `on_play_pause` (engine `resume(clock)`) |
| `Paused` | * | Right | `on_complete` (engine `complete(clock)` → branch B.1 or B.2) |

**Single Complete path** (FR-008, SC-002): the three `Running, true` rows all dispatch to the same `on_complete` handler — verified by inspection of the click-match arms. The engine's branch-B.2 path is the only side-effect path reached.

## New catalogue keys

| Key | EN source-of-truth | Use site |
|---|---|---|
| `timer.overtime_cta` | `"Wrap it up!"` | New `<p class="overtime-cta">` element in the timer view. |
| `settings.shortcuts.label_abort` | `"Abort Session:"` (matching the existing `label_start_stop` / `label_reset` / `label_skip` shape) | Settings > Shortcuts > Abort row label. |
| `settings.shortcuts.desc_abort` | `"Discard the current focus session without logging it."` (or `"Discard the current focus session; useful as a keyboard discard path during overtime."` per the unresolved PM question) | Settings > Shortcuts > Abort row description. |

Three new catalogue keys per locale × four locales (EN, DE, IT, TR) = 12 new strings. DE / IT are good-faith translations. TR may EN-fallback per the feature 005 hedge.

## Removed strings — none

No catalogue keys are removed. The hard-coded `"(Overtime)"` literal at `src/src/components/timer/mod.rs:154` lives inside a `#[cfg(test)]` helper function (`mode_label_with_status`). It is replaced with `t_string!(i18n, timer.status_overtime)` — the catalogue key already exists at `src/locales/en.json:172`. No new keys added; no keys removed; the test helper just stops carrying its own English copy.

## What does NOT change

- `Settings`, `AppSettings`, `AppearanceSettings`, `TimerSettings`, all other settings structs — untouched.
- `TimerEvent` enum — untouched.
- `RunState` enum — untouched.
- Engine state — untouched.
- All other catalogue keys — untouched.
- The `register_global_shortcuts` command signature — argument shape widens via `ShortcutSettings` becoming wider, but the command's parameter type name is unchanged.
- The `global-shortcut` event channel name (`"global-shortcut"`) and payload shape (primitive `String`) — unchanged.
