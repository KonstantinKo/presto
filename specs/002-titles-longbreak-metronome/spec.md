# Feature Specification: Per-Session Titles, Configurable Long-Break Cadence, Opt-In Metronome

**Feature Branch**: `002-titles-longbreak-metronome`
**Created**: 2026-05-12
**Status**: Draft
**Input**: User description: "Three independent quality-of-life bundles for presto. Inspired functionally (not visually) by flow.app — i.e. the *behaviour* of per-session naming, configurable cycle counts, and an optional metronome. Presto's existing theming is unchanged; visual regression baselines stay locked."

## Clarifications resolved
- 2026-05-12: Metronome stops on overtime entry — aligns with VISION.md's deterministic-external-timer model; overtime is a soft state where the metronome's prescriptive pacing role ends. The audio-playback gate is `Focus mode AND time_remaining > 0`, not merely `current_mode == Focus`.
- 2026-05-12: Metronome auto-resumes on continuous-sessions auto-start — Principle I treats engine state identically regardless of trigger; the gate is "focus mode is active and ticking", not "user pressed start".
- 2026-05-12: Visual regression baseline count narrowed from 4 to 3 — calendar's per-day sessions table is rendered off-viewport (`src/src/components/calendar.rs:571-574`); Title column lands without a baseline diff.

## User Scenarios & Testing *(mandatory)*

> This feature is **three independent quality-of-life bundles** (Bundle A — per-session title + history; Bundle B — configurable sessions-per-long-break; Bundle C — opt-in metronome). Each bundle ships as its own user-facing capability and its own underlying integrity guarantee. Any single bundle, shipped alone, is a meaningful unit of value. Constitutional anchors are cited inline.

### User Story 1 - Naming what this focus session is for (Bundle A, Priority: P1)

Before or during a focus session, a user types a short free-text title ("Spec 002 review", "Refactor session.rs") into a single-line input next to the existing tag picker on the timer screen. The title is committed to the session record on focus completion and appears in a new Title column on the calendar's per-day sessions table. Untitled rows fall back to the joined tag names so the column never collapses.

**Why this priority**: Headline user benefit of the bundle — "what did I spend this pomodoro on?" — and the change most directly aligned with the flow.app-style behaviour brief. Tied to **II. Local-First, Privacy-Default** (titles never leave the app-data directory) and **VI. The Tauri Boundary Is Stable** (`Session` + `ManualSession` wire-shape evolution gated by `#[serde(default)]`).

**Independent Test**: With seeded pre-bundle history (no titles), start a focus session, type "Spec 002 review", complete it, open the calendar day, and verify the new row shows "Spec 002 review" while pre-bundle rows show joined tag names.

**Acceptance Scenarios**:

1. **Given** the timer screen with the tag picker visible in the `#timer-status` row, **When** a user looks at the row before starting a focus session, **Then** a single-line title input is present to the left of the tag picker, empty by default, with a placeholder hinting "What is this session for?".
2. **Given** a user typing a title and then starting a focus session, **When** the session completes (focus zero-cross), **Then** the persisted `Session` record carries the typed title verbatim — the title is captured into the in-flight session and committed on completion, not live-mirrored every tick.
3. **Given** a user opening the manual-backfill form to log a missed session, **When** they enter a title and submit, **Then** the persisted `ManualSession` record carries the typed title.
4. **Given** the calendar view's per-day sessions table after the bundle ships, **When** the user views a day with a mix of titled and untitled sessions, **Then** the Title column shows the typed title for titled rows and the joined tag names for untitled rows — the column is never empty.
5. **Given** a title longer than the cap, **When** the user types past 120 characters, **Then** further input is rejected at the input boundary (the input refuses to accept more characters or visibly truncates incoming paste), so the persisted title is always ≤120 chars.
6. **Given** the calendar Title column with a title longer than the display fit (~40 chars), **When** the row renders, **Then** the title is truncated with an ellipsis and the full title is reachable via a tooltip / hover affordance.

---

### User Story 2 - Legacy sessions load unchanged after the title field is added (Bundle A, Priority: P1)

A user with months of pre-bundle history opens the new build. Every session loads. None are corrupted, none vanish, none re-prompt for migration. The Title column shows joined tag names for those rows; the field is `None` and stays `None` permanently — no silent backfill, no inferred title written to disk.

**Why this priority**: Underlying integrity guarantee that makes Story 1 safe to ship — evolving the `Session` and `ManualSession` shape would otherwise be a data-loss hazard. Tied to **VI. The Tauri Boundary Is Stable** (`#[serde(default)]` so older on-disk records deserialise), **II. Local-First, Privacy-Default** (persistence is authoritative), and **III. Type Safety Over Defensive Code** (`Option<String>` is the type-system encoding of "may be absent" — no string sentinel like `""` or `"untitled"`).

**Independent Test**: Seed `history.json` with sessions whose JSON literally lacks the `title` key. Open the new build. Every seeded session loads, the calendar renders, the Title column shows joined tag names (not "(none)", not `null`, not an empty cell).

**Acceptance Scenarios**:

1. **Given** a pre-bundle `history.json` whose `Session` records lack the `title` key entirely, **When** the new build reads the file, **Then** every record deserialises successfully with `title = None`.
2. **Given** a pre-bundle `history.json` whose `ManualSession` records lack the `title` key entirely, **When** the new build reads the file, **Then** every record deserialises successfully with `title = None`.
3. **Given** a session record persisted by the new build with `title = None`, **When** that record is re-serialised and written back to disk, **Then** the on-disk JSON contains either `"title": null` or omits the `title` key — and a subsequent read by either an older build (best-effort) or the new build round-trips without loss.
4. **Given** the calendar Title column for a `None`-title row, **When** the row renders, **Then** the column shows the joined tag names (e.g., "deep-work, coding") and the row's other columns are unchanged.
5. **Given** an existing pre-bundle session whose record is loaded into the new build, **When** the user does nothing, **Then** the session's title is **not** silently set to a derived string (no auto-backfill from tag names into the persisted record) — the on-disk record still has `title` absent or `null` after the read.

---

### User Story 3 - Choosing how often a long break shows up (Bundle B, Priority: P1)

A user finds the default "long break every 4th focus" cadence doesn't suit them. In Settings → General, a numeric input "Sessions per long break" (default 4, range 1–10) lets them pick a new cadence. After saving, the next `N`-th focus completion enters long break, not short break; the cycle continues. The setting persists across restarts.

**Why this priority**: Headline benefit of Bundle B and a long-standing pomodoro convention. It is also the only bundle that touches the engine's deterministic state machine, so its correctness is constitutionally weighted. Tied to **I. The Timer Is Sacred** (engine input change — `sessions_per_long_break` becomes a `Durations`-like configuration the engine reads, not a literal; engine remains pure and DOM-free), **III. Type Safety Over Defensive Code** (1–10 clamp at the Settings UI boundary; engine accepts a `u32` without a runtime guard), and **V. Test-First For Stateful Engines** (failing test precedes the engine change).

**Independent Test**: Set "Sessions per long break" to 3. Drive the engine through 6 focus completions. Assert long break on completions 3 and 6, short break on 1/2/4/5. Repeat with value 5 and assert long break on 5 and 10.

**Acceptance Scenarios**:

1. **Given** Settings → General with a numeric input "Sessions per long break", **When** the user enters a value in 1–10 and saves, **Then** the setting is persisted and the engine receives the new value before the next focus completion.
2. **Given** "Sessions per long break" set to `N` (any value in 1–10), **When** the user completes the `N`-th focus session, **Then** the engine transitions to `LongBreak` mode and the time remaining is set to `long_break_duration` from `TimerSettings`.
3. **Given** the same configuration, **When** the user completes the `(N+1)`-th focus session (which is the 1st focus after a long break), **Then** the engine transitions to `Break` (short break), not `LongBreak` — the cadence counter resets correctly after each long break.
4. **Given** a user attempting to enter a value outside 1–10 in the Settings input (typing `0`, `11`, `99`, or a negative number), **When** they tab out / save, **Then** the input clamps the value to the nearest in-bounds integer or rejects the change at the input layer — the persisted setting and the engine input are always in 1–10.
5. **Given** a pre-bundle `settings.json` without the `sessions_per_long_break` field, **When** the new build loads it, **Then** the missing field defaults to `4` (preserving pre-bundle behaviour exactly) — no migration prompt, no value loss, no settings-tab error state.
6. **Given** the skip-session action mid-focus, **When** the user skips and the engine projects the new mode, **Then** the skip branch uses the same `sessions_per_long_break` check as the natural zero-cross — both branches consult the configurable value, not a hard-coded `4`.

---

### User Story 4 - Settings change doesn't truncate the running session (Bundle B, Priority: P2)

A user is mid-focus and changes "Sessions per long break" from 4 to 3. The running session is unaffected — no truncation, no reset, no mid-flight cadence-counter reclassification. The new value takes effect on the *next* completion-and-transition boundary, matching the existing `Durations` mid-session-replacement posture.

**Why this priority**: Integrity guarantee that makes Story 3 safe to ship without breaking the user's current pomodoro. P2 because it's a narrow edge case of Story 3, not an independently-demoable capability. Tied to **I. The Timer Is Sacred** and **V. Test-First For Stateful Engines**.

**Independent Test**: Start a focus session, run 5 minutes (simulated), change `sessions_per_long_break` from 4 to 1, observe one more minute. Assert `current_mode == Focus`, `time_remaining_secs` reflects the original clock minus elapsed, and the next-transition projection uses `1` only at the next zero-cross.

**Acceptance Scenarios**:

1. **Given** a focus session running with the prior `sessions_per_long_break` value, **When** the user saves a new value in Settings mid-session, **Then** the engine's current `time_remaining_secs` and `current_mode` are unchanged at the moment of save — no truncation, no reset.
2. **Given** the same in-flight scenario, **When** the running focus session completes its current zero-cross, **Then** the next-mode projection uses the *new* `sessions_per_long_break` value to decide between `LongBreak` and `Break`.
3. **Given** the user activates the skip-session action immediately after saving a new `sessions_per_long_break` value, **When** the engine projects the skip target, **Then** the projection uses the new value (because skip is a normal transition boundary, identical to natural zero-cross).

---

### User Story 5 - Optional audible metronome during focus (Bundle C, Priority: P2)

A user wants a steady audible tick during focus sessions. In Settings → Notifications, "Enable metronome during focus" (default off) plus a BPM input (range 30–180, default 60) opt them in. While focus is running, a short sine tick fires every `60_000 / bpm` ms using the same audio context lifecycle as the existing chime. It stops on: user pause, mode transition out of focus, smart-pause auto-pause, overtime entry, or the setting toggling off. It resumes only when focus resumes from pause. Break, long break, and overtime never tick.

**Why this priority**: Headline benefit of Bundle C. P2 (not P1) because A and B are higher-leverage per the PM brief — A is the most-requested user behaviour, B touches the engine. C is "nice-to-have, opt-in, low engine risk". Tied to **I. The Timer Is Sacred** (metronome is a UI-side side effect in the timer component's tick loop next to `play_chime`; engine has zero awareness, zero `web_sys` imports), **II. Local-First, Privacy-Default** (no network, no telemetry, default-off), and **III. Type Safety Over Defensive Code** (BPM clamped at the Settings UI input boundary, not at the audio call site).

**Independent Test**: Enable metronome, BPM 60. Start a focus session in a host that can observe the AudioContext (Tauri-mock setup or wasm-bindgen-test with a stub). Assert one tick/s while focus is running. Pause → no further ticks. Resume → ticks resume. Let focus complete → no ticks in the subsequent break.

**Acceptance Scenarios**:

1. **Given** Settings → Notifications with "Enable metronome during focus" off (the default), **When** the user starts and runs a focus session, **Then** no metronome ticks fire — only the existing chime on transitions, as today.
2. **Given** metronome enabled with BPM `B` (in 30–180), **When** a focus session is running, **Then** a short sine tick fires every `60_000 / B` ms (within ±10 ms scheduling tolerance) for as long as the session is running, paused-state-aside.
3. **Given** a metronome ticking during focus, **When** the user pauses the session, **Then** the next scheduled tick is suppressed and no further ticks fire until focus resumes.
4. **Given** a metronome ticking during focus, **When** the focus session zero-crosses into break (or long break), **Then** ticks stop at the transition and do not fire during break, long break, or any auto-started follow-up break.
5. **Given** a metronome ticking during focus with smart-pause enabled, **When** smart-pause auto-pauses the session due to inactivity, **Then** ticks stop until activity resumes the session.
6. **Given** a metronome ticking during focus, **When** the focus session enters overtime (time-remaining reaches 0 but allow-continuous-sessions keeps the session "open"), **Then** ticks stop at the zero-cross — overtime is a non-counted continuation, not a focus continuation for the purposes of this audible tick.
7. **Given** a metronome ticking, **When** the user toggles "Enable metronome during focus" off in Settings, **Then** the next scheduled tick is suppressed within the current tick loop iteration and no further ticks fire — toggling on again mid-session resumes ticks only on the next focus start, not retroactively.
8. **Given** the BPM input in Settings, **When** the user enters a value outside 30–180 (e.g., 1, 0, 250), **Then** the input clamps to the nearest in-bounds integer or rejects the change at the input layer — the persisted `metronome_bpm` is always ≥ 30 and ≤ 180.
9. **Given** a pre-bundle `settings.json` without the `metronome` and `metronome_bpm` fields, **When** the new build loads it, **Then** `metronome` defaults to `false` and `metronome_bpm` defaults to `60` — pre-bundle users hear no change unless they opt in.

---

### User Story 6 - Visual regression baselines are updated with explicit per-baseline justification (cross-cutting, Priority: P3)

A PR reviewer opens the visual regression diff. Exactly the baselines that **must** legitimately differ — timer screen (title input added to `#timer-status` row), settings General tab ("Sessions per long break" input added), and settings Notifications tab (metronome checkbox + BPM input added) — are regenerated. Each carries a one-line PR-description note. No baseline outside the touched screens is regenerated. (The calendar view is excluded — its per-day sessions table is intentionally rendered off-viewport per `src/src/components/calendar.rs:571-574`, so the Title column lands without a baseline diff; coverage falls back on `tests/e2e/sessions-history.spec.js`'s scroll-into-view flow.)

**Why this priority**: Integrity guarantee that the UI surface is honestly accounted for, not silenced. P3 because it's PR-time discipline, not runtime behaviour. Tied to **IV. Visual Regression Is The UI Contract**. The PM brief's prior 2-baseline budget is explicitly widened here with per-baseline justification.

**Independent Test**: Run the visual regression suite. Confirm failing baselines map exactly to the three touched screens (plus theme variants). Confirm no baselines for untouched screens (tasks list, tag manager, login overlay) flag a diff.

**Acceptance Scenarios**:

1. **Given** the bundle's PR ready for review, **When** the visual regression suite runs, **Then** the only baselines that flag a diff are those for the timer screen, the settings General tab, and the settings Notifications tab (plus any theme-variant captures of those screens).
2. **Given** each regenerated baseline, **When** the PR description is read, **Then** each baseline has a one-line note explaining the intended visual change (e.g., "timer-focus.png: title input added to `#timer-status` row, left of tag picker"). No bare PNG diff lands without prose.
3. **Given** any baseline outside the touched screens flagging a diff, **When** the reviewer sees the failure, **Then** the diff is treated as a regression (fix the code) — not absorbed by regenerating the baseline.

---

### Edge Cases

- **Title length boundary**: A 5,000-char paste persists only the first 120 chars — no silent tail-drop at write time. **[BEST-GUESS PM DECISION]** 120-char cap matches macOS focus-app conventions per PM brief; not constitutionally anchored.
- **Title whitespace**: **[BEST-GUESS PM DECISION]** Stored verbatim (no auto-trim) — trim is a display choice the calendar can apply at render time.
- **Title complex graphemes (emoji, RTL, etc.)**: 120-char cap is **[BEST-GUESS PM DECISION]** character-count-based, not byte-count — matches user mental model. On-disk JSON is UTF-8 per serde defaults.
- **Title save failure**: Form's typed title survives a rejected Tauri persistence call (disk full, locked file), matching the existing tag-selection retry posture.
- **Sessions-per-long-break `N=1`**: Every focus completion goes to long break; counter resets correctly. No degenerate loop, no silent fallback to `4`.
- **Sessions-per-long-break `N=10`**: Long break every 10th completion; `u32` counter trivially in range.
- **Skip during the `(N-1)`-th focus session**: Skip is a focus completion for cadence purposes (matching `is_multiple_of(4)` in the skip branch at `src/src/engine/timer.rs:396`) — next completion is long break.
- **BPM boundary**: 30 BPM → tick every 2 s; 180 BPM → three ticks/s. Both audible-but-not-jarring (per `play_chime`'s gain envelope). 25 min × 180 BPM ≈ 4500 ticks must not exhaust the AudioContext — per-tick lifecycle, no long-lived oscillator.
- **Metronome with continuous-sessions**: No ticks during the auto-started break. Ticks auto-resume on the next auto-started focus — gate is "focus mode is active and ticking", not "user pressed start"; introducing a `UserInitiatedStart`-vs-`AutoStarted` distinction in the IPC layer would be churn for a behavioural difference the engine doesn't make.
- **Metronome + chime collision**: At a zero-cross, chime fires and the metronome's next scheduled tick is suppressed (Story 5 scenario 4). Shared lifecycle pattern, different instants.
- **Legacy records load without new fields**: `Session`, `ManualSession`, `TimerSettings`, and `NotificationSettings` records persisted by a pre-bundle release load with the new fields defaulted (Stories 2/3/5). This is the Principle VI wire-shape evolution contract.
- **`metronome_bpm` hand-edit outside 30–180**: **[BEST-GUESS PM DECISION]** No defensive clamp at the audio call site (Principle III). UI re-clamps on next Settings open/save.
- **Empty title vs `None`**: An empty string and an absent field both round-trip as `None`. **[BEST-GUESS PM DECISION]** Keeps the calendar fallback consistent; empty-string-as-sentinel is forbidden (Principle III).

## Requirements *(mandatory)*

### Functional Requirements

#### Bundle A — Per-session title + history view (constitutional anchors II, VI, III)

- **FR-001**: `Session` (in `crates/presto-ipc/src/session.rs`) MUST gain a `title: Option<String>` field, `snake_case` wire shape, with `#[serde(default)]` so pre-bundle records deserialise unchanged.
- **FR-002**: `ManualSession` (same file) MUST gain a `title: Option<String>` field with the same `#[serde(default)]` posture. The manual-backfill submit path MUST capture the user-entered title.
- **FR-003**: The timer screen MUST surface a single-line title input in the `#timer-status` row, **left** of the tag picker, empty by default. The input MUST capture into the in-flight `Session` once at focus completion (zero-cross) — NOT live-mirrored on every tick.
- **FR-004**: The title input MUST enforce a maximum of 120 user-perceived characters at the input boundary. Persisted `title` MUST be ≤120 chars under all input paths (typed, pasted, IME).
- **FR-005**: An empty title MUST round-trip as `None` — not as an empty string. The on-disk JSON MUST emit `"title": null` or omit the key. Consumers MUST treat empty string and absent field as equivalent.
- **FR-006**: The calendar's per-day sessions table MUST gain a Title column. `Some` rows show the title (truncated at ~40 visible chars with ellipsis + tooltip on overflow). `None` rows fall back to joined tag names — column is never empty.
- **FR-007**: The new title field MUST NOT trigger a one-shot migration. Existing sessions stay `None` permanently; no inferred / derived value is silently written back.

#### Bundle B — Configurable sessions-per-long-break (constitutional anchors I, III, V, VI)

- **FR-008**: `TimerSettings` (in `crates/presto-ipc/src/settings.rs`) MUST gain `sessions_per_long_break: u32` with `#[serde(default = "default_sessions_per_long_break")]` returning `4`. The default-function pattern MUST mirror existing `default_weekly_goal` / `default_max_session_time` (`#[must_use] pub const fn default_<...>() -> u32 { <literal> }`).
- **FR-009**: The Settings UI's General tab MUST surface a numeric input "Sessions per long break" clamped to 1–10 at the input layer. The persisted value is always in 1–10.
- **FR-010**: The engine MUST consume `sessions_per_long_break` as a configuration input, mirroring `Durations` (constructed from `TimerSettings` at boot, re-applied on settings change). The hard-coded literal `4` MUST be removed from both focus-completion branches (natural zero-cross at `src/src/engine/timer.rs:831` and skip-session at `src/src/engine/timer.rs:396`); both MUST consult the configured value.
- **FR-011**: The engine MUST remain a pure state machine — no `web_sys` imports, no DOM reads, no I/O. (Principle I.)
- **FR-012**: A mid-session change to `sessions_per_long_break` MUST NOT truncate the running session, reset the timer, or mid-flight reclassify the cadence-counter. New value takes effect on the *next* completion-and-transition boundary, matching the existing mid-session settings-replacement posture.
- **FR-013**: At integer boundaries `N=1` and `N=10`, the cadence counter MUST be deterministic: `N=1` enters long break on every focus completion with correct reset; `N=10` enters long break only every 10th completion with no overflow.

#### Bundle C — Metronome (opt-in) (constitutional anchors I, II, III)

- **FR-014**: `NotificationSettings` (in `crates/presto-ipc/src/settings.rs`) MUST gain `metronome: bool` and `metronome_bpm: u32`, each `#[serde(default)]`. Defaults: `metronome = false`, `metronome_bpm = 60`.
- **FR-015**: The Settings UI's Notifications tab MUST surface a checkbox "Enable metronome during focus" and a numeric input "Metronome BPM" clamped to 30–180 at the input layer.
- **FR-016**: The metronome MUST be implemented as a UI-side side effect in the timer component's tick loop alongside `play_chime`. It MUST NOT modify engine state, the engine's event vocabulary, or introduce a `web_sys` import into the engine module.
- **FR-017**: When `metronome = true` and `current_mode == Focus` and the session is running (not paused, not auto-paused, not in overtime), a short sine tick MUST fire every `60_000 / metronome_bpm` ms (±10 ms tolerance) using the same `AudioContext`-per-call lifecycle as `play_chime`.
- **FR-018**: The metronome MUST stop on any of: user-initiated pause, mode transition out of focus, smart-pause auto-pause, overtime entry, or the user toggling `metronome` off. It MUST resume only when a focus session resumes from pause (or starts fresh in focus mode).
- **FR-019**: The metronome MUST never tick during `Break`, `LongBreak`, overtime, or the gap between sessions.
- **FR-020**: The implementation MUST NOT hold a long-lived oscillator or accumulate AudioContext nodes — each tick is a fresh oscillator with a short envelope. 25 minutes × 180 BPM (≈4500 ticks) MUST NOT exhaust browser-tab resources.

#### Cross-cutting (constitutional anchors III, VI, IX)

- **FR-021**: Any new top-level Args struct (for new Tauri commands) MUST live in `crates/presto-ipc/src/args.rs` with `#[serde(rename_all = "camelCase")]`. Hand-rolled `struct Args { ... }` in `src/src/bridge/commands.rs` is forbidden. For single-key argument bags, the existing `invoke_named_arg` helper MUST be reused.
- **FR-022**: The `every_args_struct_top_level_keys_are_camel_case` defence-in-depth test MUST cover any new Args struct.
- **FR-023**: This feature MUST NOT add new runtime dependencies. If unavoidable, lockfiles (`Cargo.lock`, `tests/e2e/package-lock.json`) MUST be updated in the same commit (Principle IX).
- **FR-024**: No new network-egress paths and no telemetry events. (Principle II.)
- **FR-025**: Visual regression baselines for the timer screen, settings General tab, and settings Notifications tab MAY be regenerated. Each MUST carry a one-line PR-description note. Baselines outside these screens MUST NOT be regenerated; an untouched-screen diff is a regression to fix in code. (The calendar baseline is intentionally excluded — the per-day sessions table is rendered off-viewport per `src/src/components/calendar.rs:571-574`; the Title column lands without a baseline diff.)

#### Test-first scope (constitutional anchor V)

- **FR-026**: Failing tests MUST precede implementation for: (a) the engine's zero-cross focus-completion branch reading `sessions_per_long_break` at integer boundaries `N=1, N=4, N=10` — natural completion **and** skip-session paths; (b) mid-session settings-change preserving the in-flight session anchor; (c) `Session` and `ManualSession` round-trip with `title = Some(...)`, `title = None`, and the legacy no-`title`-key shape. UI plumbing (title input, metronome audio call, Settings inputs, calendar column rendering) is e2e-covered and NOT in Principle-V scope.

#### Out-of-scope guards

- **FR-027**: No session-naming UI outside the timer screen and manual-backfill form. The calendar Title column is read-only; edit-title-from-calendar is a deferred follow-up.
- **FR-028**: No separate metronome volume control or sound-pack. Single soft sine tick only.
- **FR-029**: No title search/filter in v1. Title column is display-only.
- **FR-030**: No changes to `focus_duration` / `break_duration` / `long_break_duration` or their UI — Bundle B changes only the cadence *count*, not the durations.

### Key Entities

> Three small wire-shape evolutions on existing entities, plus two pure UI-side concepts. No new on-disk entities are introduced.

- **`Session` (evolved, `crates/presto-ipc/src/session.rs`)**: Gains `title: Option<String>` (≤120 user-perceived chars). Captured into the in-flight session on user input, committed on focus completion. `#[serde(default)]`; legacy records without the key deserialise as `None`.
- **`ManualSession` (evolved, same file)**: Same `title: Option<String>` field, captured at backfill-form submit. Same `#[serde(default)]` posture.
- **`TimerSettings` (evolved, `crates/presto-ipc/src/settings.rs`)**: Gains `sessions_per_long_break: u32` (default 4) with `#[serde(default = "default_sessions_per_long_break")]`. Range 1–10 enforced at the Settings UI input layer. Read by the engine alongside `Durations`.
- **`NotificationSettings` (evolved, same file)**: Gains `metronome: bool` (default `false`) and `metronome_bpm: u32` (default 60), each `#[serde(default)]`. BPM range 30–180 enforced at the Settings UI input layer. Read by the timer component's tick loop; engine never reads either field.
- **Title input (UI-side)**: Single-line text input in the `#timer-status` row, left of the tag picker. State held in the timer component (analogous to current tag selection); harvested into the `Session` record at focus completion.
- **Metronome tick driver (UI-side, no engine state)**: Periodic audio side effect in the timer component's tick loop. Gated by `metronome && current_mode == Focus && session is running && not in overtime`. Each tick is a fresh AudioContext oscillator with a short envelope (mirrors `play_chime`).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can type a session title and see it appear in the calendar Title column for that day's completed focus session, end-to-end, with 0 manual data steps.
- **SC-002**: 100% of pre-bundle session records (no `title` key) deserialise as `title = None` — measured by a round-trip test against a literal pre-bundle JSON fixture.
- **SC-003**: The Title column shows joined tag names for 100% of `None`-title rows; 0 rows render an empty cell or `null` literal.
- **SC-004**: Across 100 paste-stress attempts, the persisted `title` is always ≤120 user-perceived characters.
- **SC-005**: With `sessions_per_long_break = N` (`N ∈ {1, 2, 3, 4, 5, 10}`), the engine transitions to `LongBreak` on exactly the `N`-th, `2N`-th, `3N`-th focus completions and to `Break` on all others — measured by 30 consecutive focus completions in an engine unit test.
- **SC-006**: A pre-bundle `settings.json` lacking `sessions_per_long_break` deserialises to `4`, and engine behaviour on the default-cadence path is bit-for-bit identical to pre-bundle — 0 drift.
- **SC-007**: A mid-focus settings change does not alter the running session's `time_remaining_secs` or `current_mode` at the moment of save. New value applies only on the *next* zero-cross or skip transition.
- **SC-008**: With `metronome = true` and `metronome_bpm = B` (`B ∈ {30, 60, 120, 180}`), a focus session produces `⌊focus_duration_secs × B / 60⌋ ± 1` ticks before zero-cross — measured by counting AudioContext oscillator-creation calls in a simulated 25-minute wasm-bindgen-test.
- **SC-009**: 0 metronome ticks fire during any of: paused, auto-paused (smart-pause), `Break`, `LongBreak`, overtime, or `metronome = false` — measured across the union of those states in a wasm-bindgen-test suite.
- **SC-010**: Toggling `metronome` off mid-focus stops the next scheduled tick within 1 tick-loop iteration (≈1 s at standard cadence).
- **SC-011**: 100% of pre-bundle `notifications` records (lacking `metronome` and `metronome_bpm`) deserialise as `metronome = false` and `metronome_bpm = 60`.
- **SC-012**: Only baselines for the three touched screens (timer / Settings General / Settings Notifications, plus theme variants) are regenerated. 0 baselines outside touched screens regenerated. Each regenerated baseline has a one-line PR note.
- **SC-013**: 0 new `web_sys` imports in the engine module after Bundle C lands. Measured by `grep '^use web_sys' src/src/engine/` returning the same set before and after.
- **SC-014**: 0 new runtime dependencies. Measured by `Cargo.lock` diff showing only version-bump noise.
- **SC-015**: 0 new network-egress code paths. Measured by `grep` for `fetch(` / `reqwest` / `supabase` / `aptabase` call-site additions in the diff — zero hits.

## Assumptions

- **A1 — Three independent bundles, one PR**: Default delivery shape is one PR landing all three, but each bundle is independently testable (Stories 1, 3, 5 are pairwise independent) and could be split at planning time without breaking the spec.
- **A2 — `serde(default)`-gated wire-shape evolution, not one-shot migration**: All five new fields (`Session.title`, `ManualSession.title`, `TimerSettings.sessions_per_long_break`, `NotificationSettings.metronome`, `NotificationSettings.metronome_bpm`) use `#[serde(default)]`. No first-launch migration; Principle VI honoured at the deserialiser.
- **A3 — Title cap 120 chars, user-perceived**: **[BEST-GUESS PM DECISION]** Matches macOS focus-app conventions per PM brief; counted in graphemes, not bytes. Not constitutionally anchored.
- **A4 — Calendar Title column truncates ~40 visible chars**: **[BEST-GUESS PM DECISION]** Display-only truncation with ellipsis + tooltip per PM brief. Not constitutionally anchored; subject to `/speckit-clarify` if calendar layout needs different spacing.
- **A5 — Sessions-per-long-break range 1–10**: **[BEST-GUESS PM DECISION]** Per PM brief. Lower bound is degenerate-but-coherent; upper bound is a deliberate ceiling. UX choice, not constitutionally anchored.
- **A6 — BPM range 30–180**: **[BEST-GUESS PM DECISION]** Per PM brief. 30 BPM is the slowest musically-coherent metronome speed; 180 BPM is under the audio-stability threshold for sustained ticking. UX + audio-stability choice, not constitutionally anchored.
- **A7 — Metronome stops on overtime entry**: Aligns with VISION.md's deterministic-external-timer model — overtime is a soft state where the metronome's prescriptive pacing role ends. The audio-playback gate is `Focus mode AND time_remaining > 0` (not just `current_mode == Focus`).
- **A8 — Metronome auto-resume on continuous-sessions auto-start**: Gate is "focus mode is active and ticking", not "user pressed start" — ticks resume automatically on the next auto-started focus. Principle I treats engine state identically regardless of trigger; a `UserInitiatedStart`-vs-`AutoStarted` distinction in the IPC layer would be churn for a behavioural difference the engine doesn't make.
- **A9 — Empty title and absent title are equivalent**: Per FR-005, both round-trip as `None`. Anchored in Principle III: `Option<String>` is the type-system encoding of "may be absent"; string sentinels forbidden.
- **A10 — No edit-title-from-calendar in v1**: Per FR-027. Existing manual-session edit path covers `ManualSession`; naturally-completed `Session` records have no edit affordance in v1.
- **A11 — Sound-pack and metronome volume out of scope**: Per FR-028. Single soft sine tick only.
- **A12 — Chime and metronome share AudioContext lifecycle pattern, per call**: Each chime call and each metronome tick creates its own AudioContext (per `play_chime` at `src/src/components/timer/mod.rs:244-265`). Steady-state rate (≤3 ticks/s + chime on transitions) is an order of magnitude below browser-tab audio limits.
- **A13 — Visual regression budget widens to cover touched screens**: Per FR-025 and Story 6. The widening is gated by per-baseline justification notes (Principle IV honoured by explicit policy, not by an arbitrary budget number).
- **A14 — No engine `web_sys` imports introduced by Bundle C**: Per FR-016 and SC-013. Metronome lives in the timer component's tick loop, not in the engine module.
- **A15 — Strict static analysis posture continues to apply**: Per Principle III. New code is clippy-pedantic-clean; no `#[allow(...)]` without inline justification. The existing `#[allow(clippy::struct_excessive_bools)]` on `NotificationSettings` already names "every bool is an independent settings toggle" — covers `metronome` being added to the bool count.
