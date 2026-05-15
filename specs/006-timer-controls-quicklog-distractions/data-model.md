# Phase 1 Data Model

**Feature**: 006-timer-controls-quicklog-distractions
**Date**: 2026-05-15

New typed entities and the closed-sum UI enum that drives the button matrix. Conventions inherited from `crates/presto-ipc/`:

- `#[cfg_attr(feature = "specta", derive(specta::Type))]` for cross-language type generation.
- `#[serde(rename_all = "camelCase")]` for wire shape (matches existing IPC convention).
- `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]` baseline.

All new types live in `crates/presto-ipc/`. The UI-only `RunState` lives in `src/src/components/timer/mod.rs` (it's never serialised).

---

## `QuickLog`

**File**: `crates/presto-ipc/src/quick_log.rs` (new).

**Purpose**: A small ad-hoc task log entry that the user wants to count, but that doesn't justify starting a full pomodoro. Persisted as `Vec<QuickLog>` in `quick_logs.json` in the Tauri app-data directory.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct QuickLog {
    /// UUID v4 (e.g. `"7c5e1f8b-…"`).
    pub id: String,
    /// User-provided. 1..=120 UTF-8 chars. PII — never log in plain.
    pub title: String,
    /// 1..=720 (1 min to 12 h).
    pub elapsed_minutes: u32,
    /// ISO-8601 UTC, e.g. `"2026-05-15T14:32:07.512Z"`.
    pub created_at: String,
    /// chrono `%a %b %d %Y`, e.g. `"Fri May 15 2026"`. Matches `ManualSession.date` precedent.
    pub date: String,
}
```

**Invariants** (enforced at the Tauri boundary per FR-022; also enforced client-side in the modal):

- `1 <= title.chars().count() <= 120`.
- `1 <= elapsed_minutes <= 720`.
- `created_at` parses as RFC3339 / ISO-8601.
- `date` matches the chrono `%a %b %d %Y` format (3-letter weekday, 3-letter month, two-digit day, four-digit year).
- `id` is a UUID v4 string (validation: 36 chars, four hyphens, v4 marker — acceptable to delegate to `uuid::Uuid::parse_str` and tolerate any UUID variant for forward-compat).

**Lifecycle**: created by `QuickLogManager::add` from the Quick Log modal. Mutated by `QuickLogManager::update` from the Inventory row-edit modal. Removed by `QuickLogManager::delete` from the Inventory row-delete affordance. Each mutation triggers a full-vec bulk re-save via `save_quick_logs`.

**Counted by**: a per-period metric distinct from `completed_pomodoros`. Never affects `pomodoros_until_long_break` (FR-027 + SC-005).

---

## `Distraction`

**File**: `crates/presto-ipc/src/distraction.rs` (new).

**Purpose**: A mid-session interruption note. Captured in one of two contexts:

- **In-session** — from the Running right-button modal. `parent_ref` is `Some(_)` with a snapshot of the running session.
- **Retroactive** — from the Inventory (e.g., user back-fills "I got pulled into a meeting at 10:30"). `parent_ref` is `None`.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Distraction {
    pub id: String,
    /// User-provided. 1..=120 UTF-8 chars. PII.
    pub note: String,
    /// ISO-8601 UTC. Modal-submit time for in-session captures; user-set or now() for retroactive.
    pub created_at: String,
    /// chrono `%a %b %d %Y`. Matches `ManualSession.date` precedent.
    pub date: String,
    /// Snapshotted at modal-open time (per spec Clarifications + Edge Cases).
    /// `None` for retroactive entries from Inventory.
    #[serde(default)]
    pub parent_ref: Option<DistractionParentRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DistractionParentRef {
    /// ISO-8601 — when the parent session started.
    pub parent_session_start_ts: String,
    /// The running mode at modal-open.
    pub parent_mode: TimerMode,
    /// The selected tag id at modal-open. `None` if no tag was set.
    pub parent_tag_id: Option<String>,
    /// The session title at modal-open. `None` if not set.
    pub parent_title: Option<String>,
}
```

**Invariants**:

- `1 <= note.chars().count() <= 120`.
- `created_at` parses as RFC3339 / ISO-8601.
- `date` matches `%a %b %d %Y`.
- If `parent_ref.is_some()`, `parent_session_start_ts` parses as ISO-8601 and predates `created_at`.
- `parent_mode` is one of the existing `TimerMode` variants — closed sum from `crates/presto-ipc/src/timer.rs:20-27`.
- `id` is UUID v4 string.

**Lifecycle**: created by `DistractionManager::add` (in-session: from Distraction modal; retroactive: from Inventory header retroactive flow — not in spec scope today but the field allows it). Mutated by `DistractionManager::update` from Inventory row-edit. Removed by `DistractionManager::delete` from Inventory row-delete. Each mutation triggers a full-vec bulk re-save via `save_distractions`.

**Race-free parent ref capture**: `parent_ref` is snapshotted **at modal-open time**, not submit time. If the timer naturally completes while the modal is open, the persisted Distraction still refers to the just-completed session (Edge Cases bullet).

### Render semantics (`DistractionParentRef`)

The two parent-context fields are rendered with different freshness rules in Inventory rows (per FR-024a):

- **Title** (`parent_title`): displayed exactly as snapshotted at capture time. Never re-resolved. If the user later renames the session retroactively (not supported today), the snapshot does not change.
- **Tag** (`parent_tag_id`): looked up against the current tag table at render time. Two cases:
  - Tag still exists ⇒ display the **current** tag name + colour (so renames are reflected).
  - Tag has been deleted ⇒ display the `(deleted tag)` placeholder string, sourced from catalogue key `inventory.deleted_tag_placeholder`.

Rationale: titles are user free-text intent for a specific moment — snapshot semantics protect that intent. Tag identity is structural / categorical — current-name resolution keeps the inventory legible after tag rename.

### Engine field cross-reference: `current_session_elapsed_secs`

(Not new, but its read-time semantics changes for this feature.) Per FR-013a, the existing `pause()` method at `src/src/engine/timer.rs:664-683` is extended in this PR to **settle the wall-clock delta** into `current_session_elapsed_secs` before clearing the start anchor. After the change, the field accurately reflects the user-observed elapsed at pause time (±0 seconds) — `complete` and `abort` invoked from Paused read the precise value.

---

## `RunState` (UI-only, never serialised)

**File**: `src/src/components/timer/mod.rs` (new section).

**Purpose**: The closed-sum that drives the state-aware button matrix per FR-012. Derived from the engine's three orthogonal bools (`is_running`, `is_paused`, `is_auto_paused`) at the UI layer. The engine bools stay as-is — engine-wide refactor is explicitly Out of Scope.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RunState {
    Idle,
    Running,
    Paused, // AutoPaused folds into Paused per FR-012 paragraph 3 + Story 1 AC 3.
}

impl RunState {
    pub(super) fn from_engine(is_running: bool, is_paused: bool, is_auto_paused: bool) -> Self {
        debug_assert!(
            !(is_running && (is_paused || is_auto_paused)),
            "engine illegal state: cannot be both running and paused"
        );
        // Check paused-or-autopaused FIRST — these strictly imply not-running.
        // Earlier mapping ordered `(false, _, _) => Idle` first, which matched
        // engine-Paused state `(false, true, false)` and rendered it as Idle.
        if is_paused || is_auto_paused {
            RunState::Paused
        } else if is_running {
            RunState::Running
        } else {
            RunState::Idle
        }
    }
}
```

**Invariants** (enforced by exhaustive `match` per Principle III):

- Exactly one variant active at a time.
- AutoPaused is never a UI-observable distinct state — it always folds into `Paused`.

**Drives** (via exhaustive `match` in the matrix module):

- Left slot label/icon/handler/class: Idle ⇒ `+ Quick Log`. Running ⇒ `✕ Abort`. Paused ⇒ `✕ Abort`.
- Center slot: Idle ⇒ `▶ Play`. Running ⇒ `⏸ Pause`. Paused ⇒ `▶ Resume`.
- Right slot: Idle ⇒ `→ Skip Mode`. Running ⇒ `! Note Distraction`. Paused ⇒ `✓ Complete`.

---

## New `TimerEvent` variants

**File**: `src/src/engine/timer.rs:24` (the in-process WASM `TimerEvent` enum). The variants land here, NOT in `crates/presto-ipc/src/events.rs` (that file contains only `UpdateAvailablePayload`). These are in-process events consumed by Leptos effects via the existing event-vector pattern, never across the Tauri bridge.

```rust
// Added to the existing `TimerEvent` enum at src/src/engine/timer.rs:24
SessionAborted { aborted_mode: TimerMode, elapsed_secs: u32 },
SessionCompletedEarly { elapsed_secs: u32 },
```

- `SessionAborted` — emitted by `Timer::abort()`. Read from the returned `Vec<TimerEvent>` by the Leptos tick-loop subscriber at `src/src/components/timer/mod.rs` to clear pending auto-restart-countdown UI state. The auto-restart gate at lines 1471-1483 is extended in this PR to additionally require `PomodoroCompleted` in the events vec, so `SessionAborted` (which is not `PomodoroCompleted`) suppresses the auto-restart.
- `SessionCompletedEarly` — emitted by `Timer::complete()` in branch B (count-incrementing path, including the continuous-mode overtime sub-branch). Engine-internal observability for the RED tests. Never serialised to disk; never sent to the Tauri bridge (Principle II).

**Variant placement**: both variants go at the end of the existing `TimerEvent` enum to minimise serialisation reshuffling — though serde with named variants doesn't care about declaration order, downstream readability is the constraint.

---

## Storage schema

**File**: `quick_logs.json` (new) at the Tauri app-data directory. Top-level shape:

```json
[
  {
    "id": "7c5e1f8b-…",
    "title": "Reply to Maria",
    "elapsedMinutes": 5,
    "createdAt": "2026-05-15T14:32:07.512Z",
    "date": "Fri May 15 2026"
  }
]
```

**File**: `distractions.json` (new) at the Tauri app-data directory. Top-level shape:

```json
[
  {
    "id": "…",
    "note": "call the dentist",
    "createdAt": "2026-05-15T08:11:43.001Z",
    "date": "Fri May 15 2026",
    "parentRef": {
      "parentSessionStartTs": "2026-05-15T08:03:00.000Z",
      "parentMode": "focus",
      "parentTagId": "deep-work",
      "parentTitle": "Write the 006 spec"
    }
  }
]
```

Missing files deserialise to `[]` (empty vec) — same as `manual_sessions.json`.

---

## Validation summary

| Field | Range / format | Enforced at |
|---|---|---|
| `QuickLog.title` | `1..=120` chars | Form (client) + Tauri boundary |
| `QuickLog.elapsedMinutes` | `1..=720` | Form + Tauri boundary |
| `Distraction.note` | `1..=120` chars | Form + Tauri boundary |
| `QuickLog.createdAt`, `Distraction.createdAt` | RFC3339 / ISO-8601 | Tauri boundary |
| `QuickLog.date`, `Distraction.date` | `%a %b %d %Y` | Tauri boundary |
| `id` (both types) | UUID v4 string | Tauri boundary |
| `DistractionParentRef.parentMode` | One of `TimerMode` variants | Type system (closed sum) |
| `DistractionParentRef.parentSessionStartTs` | RFC3339 / ISO-8601 | Tauri boundary |

Failures at the Tauri boundary return `BridgeError::InvalidArgument { field, reason }` from `crates/presto-ipc/src/error.rs:29-65`. No new `BridgeError` variants.
