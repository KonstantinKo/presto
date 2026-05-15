# Contract: Timer Engine New Entry Points

**Module**: `src/src/engine/timer.rs`
**Adds**: two methods (`abort`, `complete`) and two `TimerEvent` variants (`SessionAborted`, `SessionCompletedEarly`).
**Touches Principle**: I. The Timer Is Sacred (the engine remains a pure state machine; new entry points traverse the existing event surface).

The engine's existing run-state representation (three orthogonal bools `is_running`, `is_paused`, `is_auto_paused` at `src/src/engine/timer.rs:119-173`) is unchanged. Both new methods take `&mut self` and a `&impl Clock` (matching the existing `pause` / `resume` / `skip` signatures at lines 664-722 / 396-445).

---

## `Timer::abort(&mut self, clock: &impl Clock) -> Vec<TimerEvent>`

### Purpose

Discard the in-progress session entirely. Cancel any pending auto-restart. Leave the title in the pill (the UI does that side of the bookkeeping; the engine just goes Idle). Returns to Idle in the **same mode** (does not advance).

### Preconditions

- None. `abort` is idempotent and is a no-op from Idle (returns `[]`).
- Valid (non-trivial) call from: Running, Paused, AutoPaused.

### Postconditions

- `is_running == false`, `is_paused == false`, `is_auto_paused == false`.
- `current_session_elapsed_secs == 0`.
- `current_mode` is **unchanged** (no mode advance, no cadence consultation).
- `completed_pomodoros` is unchanged.
- `total_focus_secs` is unchanged.
- `pomodoros_until_long_break` is unchanged.
- `session_completed_but_not_saved == false` (matches `skip`'s clearing of the flag at `src/src/engine/timer.rs:407-411` — prevents a continuous-mode-overtime-then-abort from leaking the flag into the next session).

### Emitted events

Exactly one of:

- `[]` — if precondition (`is_running || is_paused || is_auto_paused`) was already false (idempotent no-op).
- `[TimerEvent::SessionAborted { aborted_mode: <mode at call time>, elapsed_secs: <elapsed at call time> }]` otherwise.

The `elapsed_secs` field is captured **before** zeroing `current_session_elapsed_secs`, for observability in tests.

### Edge cases

- **Auto-restart suppression** — the auto-start path at `src/src/components/timer/mod.rs:1471-1483` is today gated solely on the running-transition predicate `was_running && !state.is_running()` (no event check). Under that gate, `abort` from Running would flip `is_running` false and unintentionally trigger auto-restart. **The gate MUST be extended in this PR to also require `events.iter().any(|e| matches!(e, TimerEvent::PomodoroCompleted { .. }))`** — matching the existing event-check pattern used at line 1429 (session-save gate). After the change: `abort` emits only `SessionAborted` (no `PomodoroCompleted`) so the gate never fires (RED test `abort_does_not_trigger_auto_restart` covers this at the UI level).
- **Continuous mode** — `abort` semantics are identical. No special handling for the continuous-mode overtime branch (line 810). Continuous-mode-overtime aborts discard the overtime elapsed entirely.
- **Bell / OS notification** — `abort` does NOT fire the bell. Only natural completion and `complete`-with-count do.

### Idempotence

Calling `abort()` again after it has returned to Idle returns `[]`.

---

## `Timer::complete(&mut self, clock: &impl Clock) -> Vec<TimerEvent>`

### Purpose

Honestly end a paused (or auto-paused) Focus session early. Counts as one pomodoro **at the actual elapsed seconds**, not the configured `focus_duration`. Runs the same long-break cadence check as natural completion. Advances mode.

### Preconditions

- `is_paused == true` OR `is_auto_paused == true`.
- From any other state (Idle, Running): `complete` returns `[]` and is a no-op (matches the idempotence rule).
- The existing `pause()` method (lines 664-683) MUST settle the wall-clock delta into `current_session_elapsed_secs` before clearing the start anchor (FR-013a). `complete` reads `current_session_elapsed_secs` after `pause` has settled wall-clock delta — the value is the true elapsed at user-pause time, ±0 seconds. Same applies for `abort` invoked from Paused.

### Branch on elapsed

Read `current_session_elapsed_secs` at call time. Two branches:

#### A. `elapsed < 30` — discard as Abort

Per FR-015 and Story 1 AC 5. Side effects and emitted events are exactly those of `abort(clock)`:

- `[TimerEvent::SessionAborted { aborted_mode, elapsed_secs }]`.
- All postconditions of `abort` apply.

No `PomodoroCompleted`, no count, no advance, no bell, no auto-restart.

#### B. `elapsed >= 30` — count and advance

Branch on `session_completed_but_not_saved` (set by the continuous-mode zero-cross path at `src/src/engine/timer.rs:826`; read by `skip` at lines 407-417 to avoid double-counting):

##### B.1. `session_completed_but_not_saved == false` — normal paused-before-zero path

Traverse the same side-effect sequence as natural completion at `src/src/engine/timer.rs:808-872`:

1. `completed_pomodoros += 1`.
2. `total_focus_secs += current_session_elapsed_secs` (the actual elapsed — FR-014).
3. `current_session_elapsed_secs = 0`.
4. Consult `Settings::timer.sessions_per_long_break` (default 4) to decide next mode.
5. If `completed_pomodoros % sessions_per_long_break == 0`: advance to `TimerMode::LongBreak`. Else: advance to `TimerMode::Break`.
6. Set `is_running = false`, `is_paused = false`, `is_auto_paused = false`.
7. Emit `TimerEvent::PomodoroCompleted { completed_pomodoros }`.
8. Emit `TimerEvent::SessionCompletedEarly { elapsed_secs: <captured before zeroing> }` — engine-internal observability, never serialised, never reaches the Tauri bridge. Fires unconditionally in branch B (count-incrementing path), including the continuous-mode overtime sub-branch B.2.

##### B.2. `session_completed_but_not_saved == true` — continuous-mode overtime path

The zero-cross at `src/src/engine/timer.rs:826` has already incremented `completed_pomodoros` and emitted the canonical `PomodoroCompleted`. The current `current_session_elapsed_secs` accumulator now holds **only** the overtime portion (additive past the zero-cross). `complete` seals + advances without re-counting:

1. Do **NOT** re-increment `completed_pomodoros` (the zero-cross already did it).
2. `total_focus_secs += current_session_elapsed_secs` (the overtime portion — additive on top of what the zero-cross integrated).
3. `current_session_elapsed_secs = 0`.
4. Clear `session_completed_but_not_saved = false`.
5. Mode-advance was already computed at the zero-cross — re-apply the cadence-determined next mode (or, equivalently, leave the mode-advance state set by the zero-cross intact and just transition out of the still-running overtime).
6. Set `is_running = false`, `is_paused = false`, `is_auto_paused = false`.
7. **Suppress** re-emission of `PomodoroCompleted` — the original from the zero-cross is the canonical signal. `complete` in this branch only seals + advances.
8. Emit `TimerEvent::SessionCompletedEarly { elapsed_secs: <overtime portion captured before zeroing> }` per the ratified uniform-emission rule.

The natural-completion-path's bell + OS notification handlers subscribe to `PomodoroCompleted`. They fire identically in B.1 (FR-013, Story 1 AC 2). In B.2 they already fired at the zero-cross — no re-fire.

The auto-restart path at `src/src/components/timer/mod.rs:1471-1483` is gated on `PomodoroCompleted` + `Settings::notifications.auto_start_timer`. After branch B.1 `complete`-with-count, the auto-start fires per the same gate — `complete` is equivalent to natural completion for this purpose. In B.2 the auto-restart already fired at the zero-cross.

### Edge cases

- **AutoPaused** — identical to Paused (FR-013, Story 1 AC 3). The < 30 s rule applies equally — AutoPaused during the first 30 s of Focus still hits branch A and discards.
- **Continuous mode** — `complete` is the **only** path that ends a continuous-mode session with a count (FR-016 + Story 1 AC 4). The engine reads the actual elapsed (which exceeds `focus_duration` in overtime), passes it to branch B verbatim. The cadence check is identical — no carve-out.
- **Long-break advance** — `complete` does NOT bypass the cadence check. If `completed_pomodoros` reaches `sessions_per_long_break` via `complete`, the next mode is `LongBreak` exactly as in natural completion.

### Idempotence

A second call to `complete` from the resulting Idle state (after branch B) returns `[]`. Same for repeated calls after branch A.

---

## New `TimerEvent` variants (appended to the enum at `src/src/engine/timer.rs:24`)

The `TimerEvent` enum lives in-process in `src/src/engine/timer.rs:24` — NOT in `crates/presto-ipc/src/events.rs` (that file contains only `UpdateAvailablePayload`). New variants are consumed by Leptos effects via the existing event-vector return pattern, never via `listen()` across the Tauri bridge.

```rust
// Appended to the existing TimerEvent enum at src/src/engine/timer.rs:24.
SessionAborted { aborted_mode: TimerMode, elapsed_secs: u32 },
SessionCompletedEarly { elapsed_secs: u32 },
```

- `SessionAborted` is an in-process event — the Leptos tick-loop subscriber at `src/src/components/timer/mod.rs` reads it from the returned `Vec<TimerEvent>` and clears any pending auto-restart-countdown UI state. No bridge crossing.
- `SessionCompletedEarly` is engine-internal — emitted into the event vec returned by `complete`, but the UI does not consume it. It exists for RED-test observability. No bridge crossing.

---

## Test obligations (Principle V — RED before GREEN)

Engine RED tests in `src/src/engine/timer.rs`'s `#[cfg(test)] mod tests` (full enumeration in `plan.md`):

- `abort_clears_elapsed_and_returns_to_idle_from_running`
- `abort_clears_elapsed_from_paused_and_autopaused`
- `abort_does_not_touch_completed_pomodoros_or_cadence`
- `abort_does_not_trigger_auto_restart` (event-stream + UI-gate assertion: no `PomodoroCompleted` is emitted, so the running-transition + event-check gate at `src/src/components/timer/mod.rs:1471-1483` never fires)
- `complete_from_paused_with_elapsed_30_increments_count`
- `complete_from_paused_with_elapsed_29_acts_as_abort`
- `complete_from_autopaused_same_as_paused`
- `complete_in_continuous_mode_seals_with_overtime_elapsed` — asserts `SessionCompletedEarly` appears in the returned event vec **in addition to** the seal behaviour (no `PomodoroCompleted` re-emission; canonical one already fired at the zero-cross).
- `complete_in_continuous_overtime_does_not_double_count` — asserts `completed_pomodoros` increments by exactly 1 across the full zero-cross-then-`complete` sequence.
- `complete_advances_mode_via_cadence_check` (parameterised: sessions_per_long_break ∈ {2, 3, 4})
- `complete_idempotent_from_running_is_noop`
- `complete_at_exactly_30s_wall_clock_counts_not_aborts` — pause 30.0 s of wall-clock after start, complete, assert count incremented (asserts `pause()` settles wall-clock delta per FR-013a).
- `complete_from_autopaused_in_continuous_overtime` — intersection test: smart-pause triggering during continuous-mode overtime + then-`complete` seals correctly: count incremented exactly once across the whole sequence, overtime elapsed integrated into `total_focus_secs`, `session_completed_but_not_saved` cleared, mode advanced to break.

Per AGENTS.md test-first commit ordering: the RED commit precedes the GREEN commit. A single combined commit is rejected.
