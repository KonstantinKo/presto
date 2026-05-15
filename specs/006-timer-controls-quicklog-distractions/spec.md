# Feature Specification: Timer Control Rework + Quick Log + Distraction Capture

**Feature Branch**: `006-timer-controls-quicklog-distractions`
**Created**: 2026-05-15
**Status**: Draft
**Input**: User description: "Timer Control Rework + Quick Log + Distraction Capture — three converging changes to the timer view: (1) the three controls under the timer become state-aware (different action per slot per run-state: Idle / Running / Paused); (2) the separate session-title input + tag picker merge into one combined pill; (3) two new lightweight entry surfaces (Quick Log, Note Distraction) feed a new Inventory section under Stats / Calendar."

## Clarifications resolved

All entries below carry today's date `2026-05-15` and are flagged **[BEST-GUESS PM DECISION]** — visible to the user, not silently chosen.

- **Quick Log bounds = `[1, 720]` minutes (1 min to 12 h).** Anchored to brief's "sane upper bound"; matches a full workday cap; zero-minute logs are not logs.
- **✓ Complete minimum elapsed = 30 seconds.** Below 30 s, ✓ Complete behaves as Abort (no count, view returns to Idle, title preserved); no toast, no warning. Anchored to acceptance-criteria wording "elapsed > 30 seconds counts"; prevents a single-click 0-s completion exploit.
- **Auto-paused and Paused share an identical control triad** (Abort · Resume · ✓ Complete). Resume from Auto-paused clears auto-pause; ✓ Complete from Auto-paused seals with elapsed-at-pause and runs natural-completion side effects. Anchored to brief's "Auto-paused behaves identically to Paused for control purposes."
- **Distraction parent-session reference is captured by value, not by foreign key.** A still-running session has no persisted Session row, so the ref is a snapshot struct: `parent_session_start_ts`, `parent_mode`, `parent_tag_id`, `parent_title`. Populated only when captured during Running (the only state where `! Note Distraction` is reachable per the matrix).
- **Inventory placement: inside the existing Stats / Calendar area** as a subsection after the existing sessions-history table. Stat-tile widening happens in the existing tiles area; the new lists sit in the new Inventory subsection. Anchored to brief: "inside the existing Stats / Calendar area (not a new sidebar tab)."
- **Stats counter format: append `· N quicklogs · M distractions` to the existing pomodoro-count tile label, only when N or M > 0.** Zero-suffixes hidden. Catalogue strings use the `_one` / `_other` plural-aware split per FR-031. Anchored to brief example "5 pomodoros · 3 quicklogs · 2 distractions".
- **Combined pill DOM structure**: a single `#timer-status-pill` container holds the existing `#timer-status` (tag chip + mode label + chevron) and the existing `#session-title-input` as siblings, left-to-right. Click on chip → tag dropdown; click on title region → focus the input. In Running / Paused / Auto-paused, both children render but the input is `readonly` and the chevron hides. Minimises DOM churn for VR baselines and keeps existing selectors stable.
- **Title visibility on non-Focus modes**: status quo preserved — title region (and combined-pill semantics) apply only to Focus. Break / LongBreak in Idle show only the chip + mode label; the left-button matrix still applies but no title region renders.
- **Abort confirmation modal: none.** Single click discards. Cheat-tax is on ✓ Complete (must pause first), not on Abort. User can always restart by clicking Play (title persists per the matrix).
- **Distraction modal during running**: opens with input focused; Enter submits, Escape cancels; modal closes immediately on submit (no toast); target ≤ 1 s total interaction. Anchored to brief's "<1 second" round-trip target.
- **Turkish (TR) translations for new catalogue strings are deferred to a follow-up i18n update** if the contributor isn't comfortable. EN/DE/IT in scope; TR may fall back to EN as a placeholder. Anchored to feature 005 target set + brief's contributor-comfort caveat.
- **Plural-aware catalogue keys for the stat-tile suffixes**: the new `quicklogs` / `distractions` suffix counts use the `_one` / `_other` split (per FR-031). Single-key flat strings can't express the inflected plural forms in DE / IT / TR — `_one` / `_other` is the i18n standard. EN values: `_one = quicklog/distraction`, `_other = quicklogs/distractions`; DE / IT contribute their own inflected forms; TR follows the existing deferral caveat. [BEST-GUESS PM DECISION] anchored to brief's "5 pomodoros · 3 quicklogs · 2 distractions" example (which already implies plural-aware rendering).

## User Scenarios & Testing *(mandatory)*

> This feature is **one user-facing capability** (state-aware timer controls plus two lightweight capture surfaces feeding a new Inventory section) with three reinforcing user journeys. Constitutional anchors are cited inline by name and number, mirroring the spec 002 / 003 / 004 / 005 precedent.

### User Story 1 — Honest early completion of a focus session (Priority: P1)

A user types `Write the 006 spec` into the combined pill, runs Focus for 18 minutes, finishes early. They press **⏸ Pause**; the right slot reveals **✓ Complete**. Pressing it seals the session as one pomodoro counted with the **actual 18 minutes** (not 25), advances to the configured break, fires the same bell / notification as natural completion, clears the title. They did not have to wait out the remaining minutes nor lie about a clean run.

**Why this priority**: Headline action of the feature — brief goal #2 ("first-class early completion with a built-in cheat-tax"). Tied to **I. The Timer Is Sacred** (✓ Complete MUST traverse the same engine path as natural completion — reads `current_session_elapsed_secs`, increments `completed_pomodoros`, emits `PomodoroCompleted`, runs long-break cadence, advances mode, fires bell/notification) and **V. Test-First For Stateful Engines** (RED-then-GREEN for the new Complete handler — FR-013).

**Independent Test**: Start a Focus session. Wait ≥ 30 s (clock-mocked). Press Pause; confirm the right-slot button reads ✓ Complete. Press ✓ Complete; confirm `completed_pomodoros` incremented by 1, the persisted Session `duration` reflects actual elapsed seconds (not configured 25 min), the view advances to the configured break mode, bell + OS notification fire, title field clears. Repeat from an Auto-paused start — same outcome.

**Acceptance Scenarios**:

1. **Given** a Focus session running ≥ 30 s and then paused, **When** the user looks at the controls, **Then** left = `✕ Abort` (ghost), center = `▶ Resume` (filled), right = `✓ Complete` (filled).
2. **Given** a paused Focus session with `current_session_elapsed_secs >= 30`, **When** the user presses ✓ Complete, **Then** in the same engine tick the session is sealed as one pomodoro counted with actual elapsed (not `focus_duration`), `completed_pomodoros` increments by 1, run-state transitions to Idle in the next-scheduled mode, bell + OS notification fire identically to natural completion, title clears, combined pill becomes interactive. Given `Settings::notifications.auto_start_timer == true`, the post-break auto-restart countdown begins identically to natural completion.
3. **Given** an Auto-paused Focus session with `current_session_elapsed_secs >= 30`, **When** the user presses ✓ Complete, **Then** the same sequence as AC 2 fires — auto-pause does NOT block or alter the Complete path.
4. **Given** a continuous-mode Focus session past `focus_duration` into overtime and now paused, **When** the user presses ✓ Complete, **Then** the session is sealed with actual elapsed (incl. overtime), counted as one pomodoro, advances to the configured break. The only way to end a continuous-mode session with a count.
5. **Given** a paused Focus session with `current_session_elapsed_secs < 30`, **When** the user presses ✓ Complete, **Then** the session is discarded as if Aborted (no count, no advance, returns to Idle in the same mode, title preserved). No toast, no warning.
6. **Given** a Focus session in Running (not Paused), **When** the user looks at the right slot, **Then** ✓ Complete is NOT visible — only `! Note Distraction`. ✓ Complete is reachable exclusively via the pause-first cheat-tax.

---

### User Story 2 — Capturing a mid-session distraction without breaking flow (Priority: P1)

Eight minutes into Focus, the user remembers an unrelated thing ("call the dentist"). They press the right-slot **! Note Distraction**, a single-field modal opens auto-focused, they type and Enter. Modal closes immediately, timer keeps running, the note is in today's Inventory under Distractions. Total interaction: under one second. Session undisturbed — elapsed count, smart-pause activity, title all untouched.

**Why this priority**: Brief's "capture without breaking flow" P1 — the headline justification for the affordance ("classical Pomodoro's Unplanned & Urgent workflow"). Tied to **I. The Timer Is Sacred** (Note Distraction MUST NOT pause, MUST NOT toggle smart-pause, MUST NOT touch `current_session_elapsed_secs` — pure side channel), **II. Local-Only** (free-text user input on disk via Tauri, never in plain logs per PII-scrubbing rule), **VI. The Tauri Boundary Is Stable** (persistence extends the existing command surface; e2e mock gets the new commands first — FR-021).

**Independent Test**: Run a Focus session 8 min in. Press the right-slot button; confirm label `Distraction`, modal title `Note distraction`, input auto-focused, Escape cancels (no write), Enter submits. Submit `call the dentist`. Confirm: modal closes within 1 render tick; `is_running` still `true`; `current_session_elapsed_secs` unchanged (±1 s); today's Inventory Distractions has the entry with the text, ISO-8601 timestamp, parent-session ref (start ts + mode + tag + title).

**Acceptance Scenarios**:

1. **Given** a Focus session in Running, **When** the user looks at the right slot, **Then** it displays `Distraction` (ghost, key `timer.ctrl_note_distraction`).
2. **Given** the user presses `Distraction` while Running, **When** the modal opens, **Then** input is auto-focused, title reads `Note distraction`, input has 120-char `maxlength`, no other field is present.
3. **Given** the modal is open with non-empty text, **When** the user presses Enter, **Then** the note persists to today's Distractions with the text, an auto-generated ISO-8601 `created_at`, and a parent-session ref snapshotting the running session's start ts, mode, tag, and title.
4. **Given** the modal is open, **When** the user presses Escape, **Then** the modal closes without persisting; timer view unchanged.
5. **Given** the modal closes (submit or cancel), **When** the user looks at the timer, **Then** it is still Running, `is_running = true`, `current_session_elapsed_secs` still ticking, title and tag chip unchanged.
6. **Given** the user is in Paused (not Running), **When** the user looks at the right slot, **Then** it displays `✓ Complete`, not `Distraction`. The capture surface is Running-only. Retroactive entry remains available via Inventory.
7. **Given** the user submits two distractions back-to-back (texts `A` then `B`), **When** Inventory is opened, **Then** both are listed under today with distinct `created_at` and identical parent-session refs. No dedup, no batching.

---

### User Story 3 — Logging a small ad-hoc task without starting a 25-minute pomodoro (Priority: P2)

A user just spent 5 minutes replying to a colleague — too small for a 25-min Focus block, too real to leave unaccounted. They press the Idle **+ Quick Log** button (left slot, replacing today's dead `Undo`). A small modal opens: title field (auto-focused, required, 120-char max), elapsed-minutes numeric (default 5, min 1, max 720). They confirm. The entry lands in today's Inventory under Quick logs; the daily Stats tile updates with `· 1 quicklogs` appended. Timer view does not advance, pomodoro counter untouched, long-break cadence untouched — separate accounting channel.

**Why this priority**: Brief's "Quick task without breaking the method" P2 — second pillar alongside Distraction. Tied to **III. Type Safety Over Defensive Code** (QuickLog is a typed struct with strict serde; `title` capped 120, `elapsed_minutes` ranged `[1, 720]` validated at the Tauri boundary per FR-022; no stringly-typed flags), **II. Local-Only** (persisted via new Tauri command pair `load_quick_logs` / `save_quick_logs`, no network), **VIII. Spec-Driven Feature Flow** (multi-file work — managers, persistence, bridge, UI — spec mandatory).

**Independent Test**: From Idle in any mode, confirm left-slot label `+ Quick Log`. Press it; confirm modal title `Log a quick task`, title field auto-focused, minutes pre-filled `5`. Submit title `Reply to Maria` + minutes `5`. Confirm: today's Inventory Quick logs has the entry with title, 5 minutes, ISO-8601 `created_at`; daily Stats tile reads e.g. `0 pomodoros · 1 quicklogs`; `completed_pomodoros` unchanged; current mode and `pomodoros_until_long_break` unchanged.

**Acceptance Scenarios**:

1. **Given** the timer in Idle (any mode — Focus/Break/LongBreak), **When** the user looks at the left slot, **Then** it displays `+ Quick Log` (ghost, key `timer.ctrl_quick_log`) regardless of mode. The legacy `Undo` affordance on Break/LongBreak idle is gone.
2. **Given** the user presses `+ Quick Log`, **When** the modal opens, **Then** title reads `Log a quick task`, title field is auto-focused with 120-char `maxlength`, minutes field is numeric defaulted to `5` with min `1` and max `720`. Submit is enabled only when title is non-empty and minutes are in range.
3. **Given** the modal has title `Reply to Maria` and minutes `5`, **When** the user confirms, **Then** today's Quick logs gains an entry with the title, `elapsed_minutes = 5`, an ISO-8601 `created_at`, and a chrono `%a %b %d %Y` `date`. The daily Stats tile updates with the suffix `· 1 quicklogs`.
4. **Given** the user submits a Quick Log, **When** the user looks at the timer, **Then** `completed_pomodoros`, current mode, `pomodoros_until_long_break`, and run-state are all unchanged.
5. **Given** the user enters minutes outside `[1, 720]`, **When** the user attempts submit, **Then** submission is rejected at the form layer (field highlighted, no Tauri call). The Tauri command also rejects out-of-range per FR-022.
6. **Given** a 120-char title, **When** the user submits, **Then** submission succeeds. **Given** the user tries a 121st char, **Then** the input layer prevents entry (`maxlength`).
7. **Given** Quick Log is opened from the timer view AND from Inventory's `+ Quick Log` header button, **Then** both surfaces present the identical modal; submissions from either surface land in the same list with identical fields.

---

### User Story 4 — Simpler timer view for users who don't title sessions (Priority: P2)

A user who never titles sessions sees only the tag chip + placeholder text (`session title…`) in Idle. Two click zones in one combined pill: chip → tag dropdown, title region → inline title input. They start the timer; placeholder vanishes; pill collapses to a static read-only label (chip + mode + title). No separate dead title-input slot taking adjacent visual space.

**Why this priority**: Brief's "Simpler timer for users who don't title" P2 story. Tied to **IV. Visual Regression Is The UI Contract** (combined-pill restructure changes `timer-*` baselines in Idle/Running/Paused; per-baseline notes per FR-029) and **III. Type Safety Over Defensive Code** (the UI-layer `RunState` enum drives pill interactivity as a closed sum: Idle ⇒ interactive, Running | Paused ⇒ readonly — never flag bools).

**Independent Test**: Open Focus Idle with no title, no tag. Confirm: combined pill renders (chip + mode label + chevron + placeholder `session title…`). Click the chip — tag dropdown opens. Click the placeholder text — title input focused. Type `Spec writing`, start the timer. Confirm: pill collapses to static read-only (chevron gone, chip click is a no-op, title input is `readonly`).

**Acceptance Scenarios**:

1. **Given** Focus Idle, **When** the user looks between the timer and the controls, **Then** a single combined pill is visible (left-to-right: tag chip, mode label, chevron, title input/placeholder); the title region is INSIDE the pill as a sibling of `#timer-status`, not adjacent.
2. **Given** Focus Idle with no title set, **When** the user looks at the title region, **Then** placeholder `session title…` is faintly visible immediately after the chevron.
3. **Given** Focus Idle, **When** the user clicks the tag chip, **Then** the tag-picker dropdown opens (same picker as today).
4. **Given** Focus Idle, **When** the user clicks the title region, **Then** the inline `<input>` gains focus; the user can type up to 120 chars; no Enter-to-commit; value is read off the field at timer-start time.
5. **Given** Running or Paused (including Auto-paused, which renders as Paused), **When** the user looks at the combined pill, **Then** both children are read-only: chevron hidden (chip click is a no-op), title input carries `readonly` (focus denied).
6. **Given** the timer advances Focus → Break or → LongBreak, **When** the user looks at the new mode's view, **Then** the title region is absent — only chip + mode label render.
7. **Given** the view re-enters Focus Idle after a Break/LongBreak ends, **When** the user looks at the title region, **Then** the placeholder is visible (title cleared per FR-011) — the field is ready for the next intention.

---

### User Story 5 — Reviewing what was actually done today (Priority: P3)

End of day: user opens Stats / Calendar, sees the daily tile `5 pomodoros · 3 quicklogs · 2 distractions`. Below the sessions-history table, a new **Inventory** section lists today's Quick logs (title + minutes) and Distractions (note + optional parent-session ref). Per-row Edit + Delete mirror the sessions-history-table pattern. A `+ Quick Log` button at the Inventory header allows retroactive logging.

**Why this priority**: Brief's "Review what I actually did" P3 — consumption side of the new entities. P3 because Stories 2 & 3 deliver value on their own; Inventory is the consumption surface. Tied to **IV. Visual Regression Is The UI Contract** (Inventory adds a new subsection — baselines regenerate per FR-029) and reuses the existing `sessions_history_table.rs` edit/delete-via-modal pattern (recon fact).

**Independent Test**: Seed three Quick logs + two Distractions for today via the underlying Tauri commands. Open daily Stats. Confirm: tile shows `· 3 quicklogs · 2 distractions` appended; new `Inventory` section with `Quick logs` + `Distractions` subsections visible below sessions-history; each row has Edit + Delete. Edit a Quick log title → persists across reload. Delete a Distraction row → stays gone across reload. Press header `+ Quick Log` → identical modal from Story 3 → submit → new row in Quick logs.

**Acceptance Scenarios**:

1. **Given** today has ≥ 1 Quick log AND ≥ 1 Distraction, **When** the user opens daily Stats, **Then** the tile's pomodoro-count label appends ` · N quicklogs · M distractions` with N, M = today's counts; zero-suffixes hidden when either is 0.
2. **Given** daily Stats is open, **When** the user scrolls past the sessions-history table, **Then** an `Inventory` section header is visible, followed by two subsection headers `Quick logs` and `Distractions`.
3. **Given** the Inventory is visible, **When** the user looks at the rows, **Then** Quick log rows show title + elapsed minutes + timestamp; Distraction rows show note + timestamp + (if present) parent-session-ref label; each row has Edit + Delete consistent with the existing sessions-history-table pattern.
4. **Given** the user presses Delete on a Quick log row, **When** the confirmation completes, **Then** the row disappears and stays absent across a reload — the entry is gone from persistence.
5. **Given** the user presses Edit on a Distraction row, **When** the edit modal opens, **Then** the note is pre-filled and editable, timestamp is read-only, and submit persists the change.
6. **Given** the Inventory header carries a `+ Quick Log` button, **When** the user presses it, **Then** the identical modal from Story 3 opens; submission appends to today's Quick logs (retroactive — without leaving Stats / Calendar).
7. **Given** the user navigates back in the calendar to a previous day, **When** the Inventory renders, **Then** it shows that day's entries (filtered by `date`), independently of today.

---

### Edge Cases

- **Start of cycle, ✓ Complete with elapsed < 30 s**: Behaves as Abort (no count, view returns to Idle, title preserved). Blocks the Start → Pause → Complete rapid-fire count-inflation exploit.
- **Pause clicked in the same tick the timer naturally hits zero**: The natural-completion sequence wins (deterministic ordering — `tick()` runs to completion, then state-mutating UI handlers run). User lands in the next-mode Idle with the pomodoro counted. `complete` is unreachable in that flow (already advanced).
- **Auto-pause during the first 30 s of Focus**: Auto-paused renders the Pause triad; ✓ Complete still subject to the < 30 s rule → discarded as Abort.
- **✓ Complete in continuous mode (overtime)**: Pausing past `focus_duration` reveals ✓ Complete; pressing it seals with actual elapsed (incl. overtime) and counts as one pomodoro. The only way to end a continuous-mode session with a count — Abort discards; Skip does not count and isn't reachable from Paused.
- **Abort while the ±5 min adjust modal is open**: Abort closes both the session and the now-meaningless ±5 modal. Inventory / Note / Quick-Log modals stay open — they don't touch session state. **[BEST-GUESS PM DECISION]**
- **Multiple distractions back-to-back in the same session**: All persist as distinct Inventory rows with the same parent-session snapshot but distinct `created_at`. No dedup, no batching.
- **Quick Log submitted from Inventory while a Focus session is running**: Pure side-channel — no pause, no abort, `completed_pomodoros` unchanged, session keeps ticking. `created_at` records submission time.
- **Distraction modal still open when timer naturally hits the bell**: Modal stays open (user typing). Underlying timer transition completes — mode advances to Break, new mode starts (or not, per auto-start). On submit, the Distraction persists with the parent-session ref **snapshotted at modal-open time**, not submit time, to avoid the race. **[BEST-GUESS PM DECISION]**
- **Abort, then Play with title still in the pill**: Title persists; Play resumes intent with the same title and tag. `current_session_elapsed_secs` starts fresh at 0 — Abort discards prior elapsed entirely.
- **Auto-restart pending when Abort is pressed**: Abort cancels the pending auto-restart (engine contract per the brief). User must explicitly press Play next. Title and tag persist; auto-restart countdown UI clears.
- **Inventory subsection on a day with zero entries**: Section header still renders for structural consistency; each subsection shows an empty-state line ("No quick logs today.", "No distractions today."). Daily tile hides both zero-suffixes.
- **Combined pill in Break / LongBreak Idle**: Title region absent (Focus-only). Left = `+ Quick Log`, center = `▶ Play`, right = `→ Skip Mode` all still present.
- **PII in distraction text and quick-log titles**: Free-text input MUST be scrubbed before debug/log emission (Principle II). Persisted JSON on disk is fine (local); stderr / panic messages MUST elide content. Mirrors existing `ManualSession.notes` / `title` handling.

## Requirements *(mandatory)*

### Functional Requirements

**A. Combined tag-and-title pill**

- **FR-001**: The timer view MUST replace the separate session-title input and tag picker with a single `#timer-status-pill` container holding the existing `#timer-status` (chip + mode label + chevron) and the existing `#session-title-input` as siblings, left-to-right.
- **FR-002**: In Focus Idle, the combined pill MUST be fully interactive: clicking the chip area opens the tag dropdown; clicking the title region focuses the title input.
- **FR-003**: In Focus Idle with no title set, the title input MUST display the placeholder `session title…` immediately after the chevron.
- **FR-004**: The title input MUST accept up to 120 characters (`maxlength`). No Enter-to-commit; the value is read off the field when `▶ Play` is pressed.
- **FR-005**: In Running, Paused, and Auto-paused (Focus), the combined pill MUST render but be read-only: chevron hides and chip clicks are no-ops; title input is `readonly`.
- **FR-006**: Combined-pill semantics apply only to Focus mode. Break and LongBreak render only the chip + mode label without a title region.

**B. Title clearing behaviour**

- **FR-007**: Title MUST auto-clear on natural focus completion (status quo).
- **FR-008**: Title MUST auto-clear on `→ Skip Mode` (status quo).
- **FR-009**: Title MUST auto-clear on `✓ Complete` (new behaviour).
- **FR-010**: Title MUST persist on `✕ Abort` to carry user intent into a resumed run.
- **FR-011**: Title is sealed at timer-start; the pill becomes interactive again only when the run returns to Idle.

**C. Three-state button matrix**

- **FR-012**: The three controls MUST display the following triad per run-state:
  - **Idle**: left `+ Quick Log` (ghost), center `▶ Play` (filled), right `→ Skip Mode` (ghost).
  - **Running**: left `✕ Abort` (ghost), center `⏸ Pause` (filled), right `! Note Distraction` (ghost, label `Distraction`).
  - **Paused / Auto-paused**: left `✕ Abort` (ghost), center `▶ Resume` (filled), right `✓ Complete` (filled).
- **FR-013**: ✓ Complete on a Paused or Auto-paused Focus session MUST traverse the same engine path as natural completion: increment `completed_pomodoros`, integrate elapsed into `total_focus_secs`, reset `current_session_elapsed_secs`, emit `PomodoroCompleted`, run long-break cadence check, advance mode, stop running, fire bell + OS notification. MUST NOT bypass the engine. In the continuous-mode overtime sub-path (where the engine's zero-cross has already incremented the count + emitted `PomodoroCompleted`), `complete` MUST NOT re-increment the count and MUST NOT re-emit `PomodoroCompleted` — it seals the overtime portion into `total_focus_secs` and advances mode per the cadence already computed at the zero-cross.
- **FR-013a**: `pause()` MUST settle wall-clock elapsed into `current_session_elapsed_secs` before transitioning to Paused (i.e., before clearing the start anchor). `complete` and `abort` invoked from Paused MUST observe the true elapsed at the moment the user paused — ±0 seconds, not ±1 second. Anchored to Principle I (Timer Is Sacred) and Principle V (Test-First); covered by the RED test `complete_at_exactly_30s_wall_clock_counts_not_aborts`.
- **FR-014**: The persisted Session record from ✓ Complete MUST use the **actual elapsed seconds** at pause time, not the configured `focus_duration`.
- **FR-015**: ✓ Complete with `current_session_elapsed_secs < 30` MUST silently behave as Abort (no count, no advance, return to Idle in the same mode, title preserved). No toast, no warning.
- **FR-016**: ✓ Complete MUST function in continuous mode (overtime), sealing with actual elapsed (incl. overtime). The only way to end a continuous-mode session with a count.
- **FR-017**: `✕ Abort` MUST discard the in-progress session (no count, no advance), cancel any pending auto-restart, leave the title in the pill, and return to Idle in the same mode.
- **FR-018**: `→ Skip Mode` MUST retain today's semantics: advance per configured cadence without starting, without counting; emit `SessionSkipped { skipped_mode, elapsed_secs }`.

**D. Per-button modal interactions**

- **FR-019**: `+ Quick Log` MUST open a modal titled `Log a quick task` with a title field (auto-focused, required, 120-char max) and an elapsed-minutes numeric field (default 5, min 1, max 720). Submission persists a new QuickLog; closes the modal; does NOT touch the engine, pomodoro counter, mode, or long-break cadence.
- **FR-020**: `! Note Distraction` (in Running) MUST open a modal titled `Note distraction` with a single text field (auto-focused, required, 120-char max), Enter submits, Escape cancels, modal closes immediately on submit (no toast). Persists with the text, ISO-8601 `created_at`, and a parent-session ref snapshotting the running session's start timestamp, mode, tag, title.
- **FR-021**: Two new Tauri command pairs MUST extend persistence: `load_quick_logs` / `save_quick_logs` and `load_distractions` / `save_distractions`. Pattern (full-list bulk re-save, ISO-8601 + chrono-formatted-date) mirrors the `load_manual_sessions` / `save_manual_sessions` precedent. The e2e mock at `tests/e2e/fixtures/tauriMock.js` MUST be extended first.
- **FR-022**: Save-side Tauri commands MUST validate at the boundary: QuickLog `title` ≤ 120, `elapsed_minutes` in `[1, 720]`; Distraction `note` ≤ 120. Out-of-range values are rejected, not silently truncated.

**E. Inventory section and Stats widening**

- **FR-023**: A new `Inventory` section MUST be added to the Stats / Calendar area, positioned after the existing sessions-history table, containing two subsections: `Quick logs` and `Distractions`.
- **FR-024**: Each Inventory row MUST display per-row Edit + Delete affordances reusing the existing `sessions_history_table.rs` edit/delete-via-modal pattern.
- **FR-024a**: Inventory rendering MUST resolve `parent_tag_id` against the current tag table; display the current tag name (reflecting renames); fall back to `(deleted tag)` placeholder if the tag has been deleted. `parent_title` is rendered as snapshotted (never re-resolved).
- **FR-025**: The Inventory header MUST carry a `+ Quick Log` button opening the identical modal from FR-019 (retroactive logging).
- **FR-026**: Inventory MUST filter by the selected day's `date` field (chrono-formatted `%a %b %d %Y`, per `ManualSession` precedent).
- **FR-027**: The daily-stats pomodoro-count tile label MUST append `· N quicklogs` when N > 0 and `· M distractions` when M > 0, in that order, after the existing pomodoro count. Zero-suffixes hidden. Plural forms use the `_one` / `_other` key pair (see FR-031). The same widening applies to weekly and monthly tiles using the period-aggregated counts.

**F. Removed surface**

- **FR-028**: The existing `Undo`-last-pomodoro affordance on the left button (Break/LongBreak idle) MUST be removed. Equivalent outcome remains reachable via per-row delete in the sessions-history table.
- **FR-028a**: The `timer.ctrl_undo` and `timer.ctrl_undo_aria` catalogue keys MUST be pruned from `src/locales/{en,de,it,tr}.json` in the same PR. Anchored to the brief's removal of the Undo affordance; dead keys would be silently load-bearing and drift-prone.

**G. Visual regression discipline**

- **FR-029**: Affected baselines MUST regenerate with one-line per-baseline PR notes:
  - `timer-focus-idle-*`: combined pill replaces separate controls; left button now `+ Quick Log`.
  - `timer-focus-running-*`: combined pill collapsed read-only; right button now `Distraction`.
  - `timer-focus-paused-*` (and any auto-paused baseline): collapsed read-only; right button now `✓ Complete`.
  - `timer-break-idle-*`, `timer-longbreak-idle-*`: left button now `+ Quick Log` (was `Undo`).
  - `daily-*`: Inventory subsection visible; pomodoro-count tile may carry quicklog/distraction suffixes.
  - Weekly/monthly tiles affected by FR-027: tile labels widened.
  - No baselines outside this set are expected to regenerate; unrelated drift is a code regression, not a baseline absorption.
- **FR-030**: The PR description MUST list each regenerated baseline with its one-line note. No bare PNG diff lands without prose.

**H. i18n catalogue**

- **FR-031**: All new visible strings MUST extend the typed-key catalogue from feature 005, with EN + DE + IT translations. TR MAY fall back to EN as a placeholder per Clarifications. New keys (non-exhaustive):
  - `timer.ctrl_quick_log` — `Quick Log`
  - `timer.ctrl_skip_mode` — `Skip Mode` (renamed from today's `Skip session`)
  - `timer.ctrl_abort` — `Abort`
  - `timer.ctrl_note_distraction` — `Distraction`
  - `timer.ctrl_complete` — `Complete`
  - `timer.pill_title_placeholder` — `session title…`
  - `inventory.section_header` — `Inventory`
  - `inventory.subsection_quicklogs` — `Quick logs`
  - `inventory.subsection_distractions` — `Distractions`
  - `inventory.empty_quicklogs` — `No quick logs today.`
  - `inventory.empty_distractions` — `No distractions today.`
  - `inventory.deleted_tag_placeholder` — `(deleted tag)` (per FR-024a)
  - `modal.quick_log_title` — `Log a quick task`
  - `modal.note_distraction_title` — `Note distraction`
  - `stats.tile_daily_quicklogs_one` — `quicklog` / `stats.tile_daily_quicklogs_other` — `quicklogs`
  - `stats.tile_daily_distractions_one` — `distraction` / `stats.tile_daily_distractions_other` — `distractions`
  - `stats.tile_weekly_quicklogs_one` / `stats.tile_weekly_quicklogs_other`
  - `stats.tile_weekly_distractions_one` / `stats.tile_weekly_distractions_other`
  - `stats.tile_monthly_quicklogs_one` / `stats.tile_monthly_quicklogs_other`
  - `stats.tile_monthly_distractions_one` / `stats.tile_monthly_distractions_other`
  - The `_one` / `_other` split is **[BEST-GUESS PM DECISION]** — EN morphology is simple (singular vs plural), but DE / IT / TR have inflected plural forms that the existing single-key pattern can't express. The `_one` / `_other` convention is the i18n standard for plural-aware catalogues. DE / IT supply their own inflected forms in scope; TR may fall back to EN per the existing TR-deferral caveat. See FR-027.

**I. Engine + manager invariants**

- **FR-032**: A new `QuickLogManager` MUST own the in-memory QuickLog list and persist via the new Tauri commands; lifecycle mirrors the existing `SessionManager` pattern. RED-then-GREEN test-first per **V**.
- **FR-033**: A new `DistractionManager` MUST own the in-memory Distraction list and persist similarly. RED-then-GREEN test-first per **V**.
- **FR-034**: The timer engine MUST gain two new entry points: `abort()` (cancels any pending auto-restart, clears `current_session_elapsed_secs`, returns to Idle in the same mode without incrementing counters) and `complete()` (the FR-013 path). Both covered by RED-then-GREEN unit tests, including the FR-015 < 30 s edge case and the FR-017 auto-restart-cancellation case.
- **FR-035**: The `! Note Distraction` capture path MUST NOT pause the engine, MUST NOT toggle smart-pause, MUST NOT touch `current_session_elapsed_secs`, MUST NOT alter the running session. Pure side channel.
- **FR-036**: No new lint exceptions. New code clears `cargo clippy --all-targets -- -D warnings -W clippy::pedantic` on both crates (per **X**).

### Key Entities

- **QuickLog**: A small ad-hoc task log entry. Fields: `id: String` (UUID v4), `title: String` (≤ 120), `elapsed_minutes: u32` (in `[1, 720]`), `created_at: String` (ISO-8601), `date: String` (chrono `%a %b %d %Y`, per `ManualSession` precedent). Persisted as `Vec<QuickLog>` via `load_quick_logs` / `save_quick_logs`. Editable and deletable from Inventory. Counted by its own per-period metric — NOT the pomodoro counter; NOT the long-break cadence.
- **Distraction**: A mid-session interruption note. Fields: `id: String` (UUID v4), `note: String` (≤ 120), `created_at: String` (ISO-8601), `date: String`, `parent_session_ref: Option<DistractionParentRef>`. The parent ref is `None` for retroactive entries (from Inventory outside a running session) and `Some(_)` for in-session captures. Persisted as `Vec<Distraction>` via `load_distractions` / `save_distractions`.
- **DistractionParentRef**: A by-value snapshot of the parent session at capture time. Fields: `parent_session_start_ts: String`, `parent_mode: TimerMode`, `parent_tag_id: Option<String>`, `parent_title: Option<String>`. Captured at modal-open, not submit time (race-free per the edge case).
- **RunState** (UI-layer closed sum driving the button matrix): `Idle | Running | Paused`, derived from the engine's existing `is_running` / `is_paused` / `is_auto_paused` bools (AutoPaused folds into Paused per the matrix's parity rule — see Clarifications 2026-05-15). The engine's fields stay as-is — no engine refactor. Per **III**, the matrix is wired off this enum as an exhaustive `match`, never via string comparisons or flag-bool conditions.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user completes a Focus session early via Pause → ✓ Complete in under 5 s from the decision to be done, with the count reflecting actual elapsed.
- **SC-002**: A user captures a mid-session distraction (modal open → text → submit → modal closed → timer running) in under 1 s of interaction time (excluding typing).
- **SC-003**: A user logs a Quick Log (modal open → title + minutes confirmed → entry visible in Inventory) in under 10 s end-to-end on a typical Idle timer view.
- **SC-004**: 100% of ✓ Complete completions count exactly once toward `completed_pomodoros` and advance the long-break cadence by exactly one, identical to natural completion.
- **SC-005**: 100% of Quick Log submissions land in Inventory under Quick logs without touching `completed_pomodoros` or `pomodoros_until_long_break`.
- **SC-006**: 100% of in-session Distraction submissions persist with a non-empty parent-session ref (start ts + mode at minimum); 100% of retroactive entries persist with `None`.
- **SC-007**: `#timer-status-pill` is present in every Focus state; selectors `#timer-status` and `#session-title-input` remain stable (existing e2e tests targeting them continue to resolve).
- **SC-008**: VR baselines outside the FR-029 enumerated set do NOT regenerate. Any outside-set diff is treated as a code regression, not absorbed.
- **SC-009**: The daily-stats tile label exactly matches `K pomodoros[· N quicklogs][· M distractions]` (suffixes hidden when zero) across ≥ 4 representative days: zero-everything; pomodoros-only; pomodoros + quicklogs; pomodoros + quicklogs + distractions.
- **SC-010**: Abort cancels any pending auto-restart within the same engine tick — no observable window in which the auto-restart still fires after Abort returns.
- **SC-011**: `clippy --all-targets -- -D warnings -W clippy::pedantic` passes on both crates with zero new `#[allow]` exceptions for this feature's code.
- **SC-012**: ✓ Complete with `current_session_elapsed_secs < 30` yields zero increment of `completed_pomodoros` and zero advance of `pomodoros_until_long_break` in 100% of cases (anti-cheat invariant).

## Out of Scope (Non-Goals)

- Importing or backfilling pre-existing Quick Log or Distraction entries. Inventory starts empty for everyone at upgrade time.
- Editing pomodoro titles after natural completion — sealed-on-completion status quo preserved.
- Post-bell distraction-review prompts ("here are the 3 distractions you captured…"). Brief explicitly rules this out.
- Settings toggles to hide individual buttons. The button matrix is uniform across installs.
- Mobile / small-screen rework of the timer view. Desktop-first per `VISION.md`.
- Engine-wide refactor of the three orthogonal bools (`is_running`, `is_paused`, `is_auto_paused`) into a single enum. The UI's `RunState` is a UI-layer derivation; engine-wide refactor is a separate concern.
- Hard requirement to localise TR strings in this PR if a TR-fluent contributor isn't available (follow-up i18n update covers TR).
- Reworking the sessions-history-table edit/delete pattern — Inventory reuses as-is.

## Assumptions

- The `ManualSession` persistence pattern (Tauri `load_*` / `save_*` pair, full-list bulk re-save, ISO-8601 `created_at`, chrono `%a %b %d %Y` `date`) is the reference precedent for both new entities. No persistence-layer innovation.
- Existing tag-picker dropdown and inline-title-input components are reused as-is, just repositioned into a single container.
- The `sessions_history_table.rs` edit/delete-via-modal pattern handles both new entities — both have a free-text field plus a numeric/timestamp, well within the existing pattern's range.
- Smart-pause / auto-pause continue to work unchanged; the button matrix reads the resulting state as `Paused` for control purposes.
- Continuous mode, auto-start, and the ±5 min adjust buttons keep working unchanged — orthogonal to the button-matrix rework.
- The i18n typed-key catalogue from feature 005 accepts new keys via the same mechanism — no library swap.
- No new third-party dependencies — both managers reuse existing serde, chrono, uuid. Per **IX. Lock Files Are First-Class**, `Cargo.lock` regenerates in the same commit if any transitive bump is incidentally pulled in.
- The Tauri command boundary-validation pattern (e.g. settings-save bounds) is reused. No new IPC mechanism per **VI. The Tauri Boundary Is Stable**.
- VR baseline regeneration is a one-time hit on this PR. Subsequent UI work doesn't re-regenerate the same baselines unless visible chrome changes.
