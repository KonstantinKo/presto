# Implementation Plan: Per-Session Titles, Configurable Long-Break Cadence, Opt-In Metronome

**Branch**: `002-titles-longbreak-metronome` | **Date**: 2026-05-12 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification at `specs/002-titles-longbreak-metronome/spec.md`

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

Three independent quality-of-life bundles. **Bundle A (titles)** widens `Session` and `ManualSession` in `crates/presto-ipc/src/session.rs` with `title: Option<String>` (`#[serde(default)]`), surfaces a single-line input in the `#timer-status` row to the left of the tag picker, captures into the in-flight session at focus completion, and adds a Title column to the calendar's per-day table that falls back to joined tag names for `None` rows. **Bundle B (long-break cadence)** parameterises the previously hard-coded `4` in the engine: `TimerSettings` gains `sessions_per_long_break: u32` (default `4` via `default_sessions_per_long_break()`), the Settings General tab gains a 1–10 numeric input clamped at the UI boundary, and the engine's natural zero-cross (`src/src/engine/timer.rs:831`) and skip-session (`:396`) branches consult the configured value test-first. **Bundle C (metronome)** widens `NotificationSettings` with `metronome: bool` and `metronome_bpm: u32` (defaults `false` / `60`), surfaces a checkbox + BPM input in Settings Notifications, and adds a per-tick `AudioContext`-per-call side effect to `src/src/components/timer/mod.rs`'s tick loop — engine remains DOM-free. No new Tauri commands, no new dependencies, no on-disk migration. Wire-shape evolution per Principle VI via `#[serde(default)]`. Detail in [research.md](./research.md), [data-model.md](./data-model.md), [contracts/wire-shapes.md](./contracts/wire-shapes.md), [quickstart.md](./quickstart.md).

## Technical Context

**Language/Version**: Rust 1.83+; `wasm32-unknown-unknown` target for the Leptos crate; backend Rust unchanged. No version bump from the 001-leptos-migration baseline.
**Primary Dependencies**: Unchanged. `leptos = "0.7"`, `serde`, `serde-wasm-bindgen`, `web-sys` (already imported in `components/timer/mod.rs` for `play_chime`; the metronome reuses the same surface), `chrono` (unrelated to this feature). Backend deps unchanged.
**Storage**: Tauri app-data directory (authoritative; unchanged). `history.json`, `manual-sessions.json`, `settings.json` evolve at the field level only — file paths and the surrounding shape are stable. Legacy records without the new fields deserialise via `#[serde(default)]`.
**Testing**: `cargo test --workspace --frozen` for IPC round-trip + engine; `wasm-bindgen-test` for the metronome tick-counter (mocked `AudioContext`); Playwright e2e + visual regression for UI plumbing.
**Target Platform**: macOS, Linux, Windows desktops (CSR-only single-window Tauri 2.x).
**Performance Goals**: No regression. Metronome at the upper bound (180 BPM × 25 min ≈ 4500 ticks) must not exhaust browser-tab audio resources — per-tick oscillator lifecycle, no long-lived nodes (mirrors `play_chime`).
**Constraints**: Strict static analysis stays green (Principle III). `#[allow(clippy::struct_excessive_bools)]` already covers `NotificationSettings` adding one more bool (`metronome`). Engine purity grep gate (`scripts/check-engine-purity.sh`) must remain at zero `web_sys` references under `src/src/engine/`. The baseline-cap gate (`scripts/check-baseline-cap.sh`) needs an explicit override for this PR — see §[Constitution Check](#constitution-check) IV.
**Scale/Scope**: Three wire-shape additions, one engine input parameterisation, one UI side-effect. ~10 files touched, no new modules, no new Tauri commands.

## Constitution Check

*GATE: must pass before Phase 0. Re-checked after Phase 1.*

Only principles with material content are listed below per repo artefact discipline.

### I. The Timer Is Sacred — Bundle B

The engine's natural zero-cross at `src/src/engine/timer.rs:831` and skip-session branch at `:396` both read the hard-coded literal `4`. Bundle B replaces that literal with `self.sessions_per_long_break`, a `u32` field on `TimerState` set at construction (mirrors how `durations: Durations` is held). The engine remains a pure state machine: it accepts the new field as a configuration input, no DOM read, no `web_sys` import. Mid-session settings change uses the existing `set_durations`-style replacement posture — the running session's anchor is unchanged; the new value applies at the next transition boundary. **PASS.**

### I. The Timer Is Sacred — Bundle C

Metronome is a UI-side side effect in `src/src/components/timer/mod.rs`'s `handle_events` / tick loop, sitting next to `play_chime`. The engine has zero awareness of the metronome — no event, no state field, no `web_sys` import. The engine-purity grep gate enforces this. **PASS.**

### II. Local-First, Privacy-Default — Bundle C

No network egress. No telemetry. Default-off (`metronome: bool = false`). Audio playback is local browser-tab `AudioContext`. **PASS.**

### III. Type Safety Over Defensive Code — all three bundles

- **Bundle A**: `title: Option<String>` is the type-system encoding of "may be absent" — no `""`-as-sentinel branch at any consumer. The calendar's `None`-row fallback (joined tag names) is a presentational decision, not a defensive guard.
- **Bundle B**: 1–10 clamp lives at the Settings UI input layer (`<input type="number" min=1 max=10>`); engine accepts a `u32` without a runtime guard.
- **Bundle C**: 30–180 clamp lives at the Settings UI BPM input layer; the audio call site reads the stored `u32` and divides into `60_000` without re-clamping. A hand-edited out-of-range value re-clamps on next Settings open/save — not at the audio call site.
- `#[allow(clippy::struct_excessive_bools)]` already on `NotificationSettings` covers the added `metronome` bool with the existing inline justification ("every bool is an independent UI toggle").

**PASS.**

### IV. Visual Regression Is The UI Contract — cross-cutting

This feature touches **three** visual baselines (plus any theme variants on those screens): `timer-chromium-linux.png` (title input added to `#timer-status` row), `settings-general-chromium-linux.png` ("Sessions per long break" input added), `settings-notifications-chromium-linux.png` (metronome checkbox + BPM input added). The calendar's per-day sessions table is **intentionally rendered off the visible viewport** — see `src/src/components/calendar.rs:571-574` verbatim: `// Sessions table — kept off the visible viewport so the // visual-regression baseline doesn't include it; the // sessions-history.spec.js:37-44 flow scrolls into it // to find #sessions-table-body rows + the edit modal.` Adding a Title column to that off-viewport table therefore does **not** affect `calendar-chromium-linux.png`. The baseline-cap gate at `scripts/check-baseline-cap.sh` defaults to **2**; this PR's count is **3**.

**Decision: (a) — accept the widened update with per-baseline justification in this PR.** Rationale: flow.app's influence on presto is *functional*, not visual; existing presto theming is unchanged. Each of the three baselines reflects exactly one new affordance (a single text input, two new form rows). Splitting into two PRs (option b) would force one bundle to ship without its UI surface, which is worse for review coherence than three narrowly-justified baseline updates.

**Per-baseline justification** (pre-anchored here; restated in the eventual PR description):
- `timer-chromium-linux.png`: title input added to `#timer-status` row, left of the tag picker. No other layout change.
- `settings-general-chromium-linux.png`: new "Sessions per long break" numeric input added as a form row. No layout change to existing rows.
- `settings-notifications-chromium-linux.png`: new "Enable metronome during focus" checkbox + "Metronome BPM" numeric input added as form rows. No layout change to existing rows.

The Title column added to the calendar's per-day sessions table lands without a baseline diff because the table sits off-viewport (see `calendar.rs:571-574` quoted above; visible-viewport timeline is at `calendar.rs:535-566`); visual coverage of the Title column relies on the existing `sessions-history.spec.js:37-44` scroll-into-view flow, which already exercises `#sessions-table-body` rows.

The CI gate accepts an override via `BASELINE_CAP=3`; the PR description documents the override and the three per-baseline notes above. **PASS** with documented widening (not a violation — the gate's "configuration override + explicit justification" path is the documented mechanism, per Principle IV's "changes to baseline count are configuration details").

### V. Test-First For Stateful Engines — Bundle B

Failing tests precede implementation for:
- `engine::tests::long_break_after_n_focus_sessions_with_n_eq_1` / `_eq_4` / `_eq_10` — natural zero-cross consults `sessions_per_long_break` at `timer.rs:831`. Existing test `long_break_after_4_focus_sessions` (`timer.rs:1267-1289`) covers the legacy `N=4` shape; new boundary tests join it test-first.
- `engine::tests::skip_session_long_break_with_n_eq_1` / `_eq_4` — skip branch at `timer.rs:396` consults the same field.
- `engine::tests::mid_session_sessions_per_long_break_change_preserves_anchor` — saving a new value mid-focus does not truncate `time_remaining_secs` or change `current_mode` at the moment of save.
- `presto_ipc::session::tests::title_round_trip_some_none_missing_key` — `Session` + `ManualSession` deserialise the three legacy / new shapes (Bundle A; in scope for V because it's the IPC wire-shape contract, not UI plumbing).

UI plumbing (title input rendering, settings inputs, calendar column, metronome audio side effect) is e2e + visual-regression covered per Principle V's exemption for "UI rendering, view wiring, trivial CRUD".

**PASS.**

### VI. The Tauri Boundary Is Stable — Bundles A, B, C

No new Tauri commands. All three bundles persist via existing commands: `save_session_data` (Session.title), `save_manual_sessions` (ManualSession.title), `save_settings` (TimerSettings.sessions_per_long_break + NotificationSettings.metronome + NotificationSettings.metronome_bpm). Wire-shape evolution is per the existing `#[serde(default)]` pattern (mirrors `TimerSettings::weekly_goal_minutes` widening in `crates/presto-ipc/src/settings.rs:108-114`; the post-cutover precedent). The mock-drift gate (`scripts/check-mock-drift.sh`) sees no new commands and stays green without mock changes. **PASS.**

### IX. Lock Files Are First-Class — N/A

No new dependencies. `Cargo.lock` and `tests/e2e/package-lock.json` are unchanged in this feature. The lockfile-drift gate stays green by inaction. If a dep slips in, lockstep is mandatory.

### Verdict

No principle is **VIOLATION**. The IV widening is a documented gate-override, not a constitution amendment. Proceed to Phase 0.

## Project Structure

### Documentation (this feature)

```text
specs/002-titles-longbreak-metronome/
├── plan.md                       # This file
├── research.md                   # Phase 0 — 3 decisions w/ rationale (audio gating, BPM clamp location, char-cap normalisation, calendar truncation)
├── data-model.md                 # Phase 1 — 4 wire-shape evolutions: Session.title, ManualSession.title, TimerSettings.sessions_per_long_break, NotificationSettings.{metronome,metronome_bpm}
├── contracts/
│   └── wire-shapes.md            # Phase 1 — every modified struct + before/after JSON example; "no new Tauri commands" called out explicitly
├── checklists/                   # Authored at /speckit-specify
├── quickstart.md                 # Phase 1 — contributor's path to exercising each bundle end-to-end
└── tasks.md                      # Phase 2 — generated by /speckit-tasks (NOT this command)
```

### Source Code (paths touched; no new modules)

```text
crates/presto-ipc/src/
├── session.rs                    # +title: Option<String> on Session and ManualSession; +tests::title_round_trip_*
└── settings.rs                   # +sessions_per_long_break on TimerSettings (+ default_sessions_per_long_break const fn);
                                  # +metronome + metronome_bpm on NotificationSettings (+ default_metronome_bpm const fn)

src/src/
├── engine/
│   ├── timer.rs                  # TimerState gains sessions_per_long_break: u32 (field init defaults to 4 inside the existing
│   │                             # `pub fn new(durations: Durations) -> Self` — constructor signature is unchanged);
│   │                             # literals at :396 and :831 replaced;
│   │                             # set_sessions_per_long_break() mirrors set_durations() posture; +tests for N=1/4/10 boundaries
│   └── (no new files)
└── components/
    ├── timer/
    │   └── mod.rs                # Bundle A: title input in #timer-status row, captured into in-flight Session at focus completion;
    │                             # Bundle C: metronome tick driver next to play_chime — schedule_metronome_tick(bpm) per tick loop iteration
    ├── calendar.rs               # Bundle A: Title column in per-day sessions table; None → joined tag names; truncate at ~40 chars + tooltip
    └── settings/
        ├── general.rs            # Bundle B: numeric input "Sessions per long break" (min=1 max=10)
        └── notifications.rs      # Bundle C: checkbox "Enable metronome during focus" + numeric input "Metronome BPM" (min=30 max=180)

tests/e2e/__screenshots__/visual-regression/
├── timer-chromium-linux.png             # regenerate (per-baseline justification above)
├── settings-general-chromium-linux.png  # regenerate
└── settings-notifications-chromium-linux.png  # regenerate
# calendar-chromium-linux.png is NOT regenerated — the per-day sessions
# table is rendered off-viewport (calendar.rs:571-574); Title column lands
# without a visual diff, covered by sessions-history.spec.js:37-44.
```

**Structure Decision**: No new modules. All four wire-shape evolutions live in existing IPC files; the engine input is one new field on the existing `TimerState`; the metronome lives in the existing timer-component tick loop. This minimal-surface posture is what makes the three bundles independent at the file level — A and C don't touch the engine; B doesn't touch the audio side effect; A's title plumbing doesn't touch settings.

## Modules

Terse change table. Bundle column: A=titles, B=long-break, C=metronome, X=cross-cutting (tests).

| Path | Change | Bundle |
|---|---|---|
| `crates/presto-ipc/src/session.rs` | `+ title: Option<String>` on `Session` and `ManualSession` (`#[serde(default)]`) | A |
| `crates/presto-ipc/src/session.rs::tests` | `+ title_round_trip_some_none_missing_key` covering Some/None/legacy-no-key for both records | A,X |
| `crates/presto-ipc/src/settings.rs` | `+ sessions_per_long_break: u32` on `TimerSettings` (`#[serde(default = "default_sessions_per_long_break")]`); `+ pub const fn default_sessions_per_long_break() -> u32 { 4 }` | B |
| `crates/presto-ipc/src/settings.rs` | `+ metronome: bool` (`#[serde(default)]`, default `false`) and `+ metronome_bpm: u32` (`#[serde(default = "default_metronome_bpm")]`, default `60`) on `NotificationSettings`; `+ pub const fn default_metronome_bpm() -> u32 { 60 }` | C |
| `src/src/engine/timer.rs` | `TimerState` gains `sessions_per_long_break: u32`; **constructor signature is unchanged** — the new field is initialised to `4` in the existing struct-init expression inside `TimerState::new` at `engine/timer.rs:202`. Literals at `:396` and `:831` consult `self.sessions_per_long_break`. `+ pub const fn set_sessions_per_long_break(&mut self, n: u32)` mirrors `set_durations`'s posture (assignment, no clamp inside the engine). The 22+ existing `TimerState::new(Durations::default())` call sites (app.rs, tray.rs ×4, session.rs ×4, `timer/mod.rs:453`, engine tests ×14) compile unchanged. | B |
| `src/src/engine/timer.rs::tests` | `+ long_break_after_n_focus_sessions_with_n_eq_1`, `..._eq_10`; `+ skip_session_long_break_with_n_eq_1`; `+ mid_session_sessions_per_long_break_change_preserves_anchor` | B,X |
| `src/src/components/timer/mod.rs` | `+` title input element in `#timer-status` row, left of tag picker; in-flight `title` signal harvested **once** at focus zero-cross and passed to BOTH the `Session` persist call AND the existing `synth_completed_session` helper (at `mod.rs:213-230`). `synth_completed_session`'s signature gains `title: Option<String>` so the synthesised `ManualSession` row (today's hard-coded `notes: None, tags: None, …` at `:217-229`) receives the user-typed title and the calendar's per-day table renders it without a second IPC round-trip. Empty-string is normalised to `None` at the capture boundary. | A |
| `src/src/components/timer/mod.rs` | `+ schedule_metronome_tick(bpm: u32)` helper next to `play_chime`; gate `metronome && current_mode == Focus && is_running && time_remaining_secs > 0`; per-tick `AudioContext`-per-call (mirrors `play_chime`); also responds to settings-change signal to stop the next scheduled tick when `metronome` toggles off mid-focus | C |
| `src/src/components/calendar.rs` | `+` Title column in the per-day sessions table with a three-tier fallback chain: (1) `Some(title)` → truncated title (~40 chars with ellipsis + full text in the `title=` attribute as native tooltip); (2) `None` AND `tags.is_some()` and non-empty → joined tag names from `tags: Option<Vec<serde_json::Value>>` via `.get("name").and_then(Value::as_str)` (matches the existing tag-display convention used elsewhere in calendar.rs); (3) `None` AND (`tags.is_none() || tags.as_ref().map_or(true, Vec::is_empty)`) → non-breaking space `&nbsp;` so the row keeps its visual line height. No `(untitled)` string sentinel — keeps the visual minimal per the flow.app-functional reference. | A |
| `src/src/components/settings/general.rs` | `+` numeric input "Sessions per long break" (`min=1 max=10`); writes through existing `save_settings` | B |
| `src/src/components/settings/notifications.rs` | `+` checkbox "Enable metronome during focus" and `+` numeric input "Metronome BPM" (`min=30 max=180`); writes through existing `save_settings` | C |
| `tests/e2e/__screenshots__/visual-regression/*.png` | 3 baselines regenerated with per-baseline justification (timer, settings-general, settings-notifications); calendar baseline is **not** touched because the per-day sessions table is rendered off-viewport (`calendar.rs:571-574`) — Title column lands without a visual diff and is covered by the existing `sessions-history.spec.js:37-44` scroll-into-view flow | A,B,C |

## Testing strategy and test-first markers

Per Principle V: failing-test commits precede implementation commits for **the engine's `sessions_per_long_break` consumption** and **the IPC wire-shape round-trip for `Session.title` / `ManualSession.title`**. UI plumbing is e2e-covered.

| Module | Test runner | Test-first? | Notes |
|---|---|---|---|
| `presto_ipc::session::tests` | `cargo test` (workspace) | **YES (RED-first)** | Bundle A. Three asserts per record: `Some("foo")` round-trips byte-stable; `None` emits `"title":null` or omits the key and round-trips; literal pre-bundle JSON without the `title` key deserialises as `title = None`. |
| `engine::tests` (cadence boundaries) | `cargo test` | **YES (RED-first)** | Bundle B. Boundary tests `N=1` (long break every focus completion), `N=10` (long break only every 10th), and skip-branch `N=1`. Existing `long_break_after_4_focus_sessions` continues to pass once the field defaults to 4; the new tests fail before the engine field exists. |
| `engine::tests::mid_session_sessions_per_long_break_change_preserves_anchor` | `cargo test` | **YES (RED-first)** | Bundle B. Asserts `time_remaining_secs` and `current_mode` are unchanged at the moment a new `sessions_per_long_break` is applied to a running focus session. Next zero-cross uses the new value. |
| `presto_ipc::settings::tests` (sessions_per_long_break default) | `cargo test` | YES (RED-first; trivial) | Bundle B. Asserts `serde_json::from_str("{...legacy shape without the field...}")` resolves to `4`. |
| `presto_ipc::settings::tests` (metronome defaults) | `cargo test` | YES (RED-first; trivial) | Bundle C. Asserts pre-bundle `NotificationSettings` JSON without `metronome` and `metronome_bpm` deserialises to `false` / `60`. |
| `components/timer/mod.rs` (title input wiring) | Playwright e2e | NO | Bundle A UI plumbing; e2e + visual regression covers it. |
| `components/timer/mod.rs` (metronome tick scheduling) | `wasm-bindgen-test` (counter-stub) | OPTIONAL test-first | Bundle C. SC-008 / SC-009 are wasm-bindgen-test assertions counting oscillator-creation calls under simulated focus / paused / break states. The dedicated-scheduler design (`leptos::prelude::set_interval_with_handle` at `60_000 / bpm` ms, lifecycle driven by a Leptos `Effect::new` watching the gate signal — see Phase 5 step 3) is what the counter-stub exercises; the engine's 1-Hz tick loop is **not** involved. If the counter-stub introduces meaningful logic, RED-first applies; if it's pure timing instrumentation, e2e is sufficient. Decision deferred to the Phase 5 task generation pass. |
| `components/calendar.rs` (Title column rendering) | Playwright e2e + visual regression | NO | Bundle A UI plumbing. |
| `components/settings/{general,notifications}.rs` | Playwright e2e + visual regression | NO | Bundle B + C UI plumbing. |

**Mock-first ordering rule** (per FR-010 and Principle VI): **N/A this feature.** No new Tauri commands; the mock-drift gate stays green without modifications.

## CI gates

Reference `.agentex.yml` (post-001 stage definitions). All gates already exist; this feature interacts with four of them:

### Mock-drift gate — `scripts/check-mock-drift.sh`

**No action needed.** No new `#[tauri::command]` handlers, no new mock cases. Run as a sanity check; expect green.

### Engine-purity gate — `scripts/check-engine-purity.sh`

**Load-bearing.** Bundle C must not slip `web_sys` imports into `src/src/engine/`. The metronome state and the `AudioContext` calls live in `src/src/components/timer/mod.rs` only. Gate is zero-tolerance; CI fails the build on the first `web_sys|web-sys` reference under `src/src/engine/`.

### Baseline-cap gate — `scripts/check-baseline-cap.sh`

**Override required.** Default cap is 2; this feature regenerates 3 baselines (see [Constitution Check IV](#iv-visual-regression-is-the-ui-contract--cross-cutting)). Use the script's documented `BASELINE_CAP=3` env override in the PR's CI run (or as a one-PR config commit that the script reads — script supports either). PR description must include the three per-baseline justifications restated verbatim from §IV above. Theme variants of the three touched screens count separately if present. The calendar baseline is intentionally **not** included — the Title column lands on an off-viewport table (`calendar.rs:571-574`).

### Lockfile-drift gate

**No action needed.** No new deps. Drift gate stays green by inaction. If any dep slips in, both `Cargo.lock` and `tests/e2e/package-lock.json` must be staged in the same commit (Principle IX).

### Strict static analysis

`cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic` stays green. `#[allow(clippy::struct_excessive_bools)]` on `NotificationSettings` already names "every bool is an independent settings toggle" — covers adding `metronome` to the bool count. No other allows needed.

## Implementation phasing

Six phases. Bundles are independently testable; the phase order keeps wire-shape evolution at the bottom of the dependency stack (Phase 0), the engine state-machine work next (Phase 1, test-first), then UI by bundle.

### Phase 0 — IPC widenings + test-first scaffolding

**Entry**: clean branch `002-titles-longbreak-metronome` post-spec.
**Exit**: `crates/presto-ipc/src/session.rs` gains `title: Option<String>` on `Session` and `ManualSession` (both with `#[serde(default)]`); `crates/presto-ipc/src/settings.rs` gains `sessions_per_long_break` on `TimerSettings` and `metronome` + `metronome_bpm` on `NotificationSettings` (all `#[serde(default = "...")]` with the const-fn defaults). Round-trip tests in `presto_ipc::session::tests` + `presto_ipc::settings::tests` cover Some/None/legacy-no-key shapes — RED commit precedes GREEN commit (the test asserts presence of the field; the field's addition makes it pass). No behaviour change yet — the engine and UI don't read the new fields.
**Test-first**: YES per Principle V (wire-shape contract; in scope for V because IPC round-trip is the persistence boundary).
- **Test-first commit ordering** (AGENTS.md §Test-first commit ordering, Principle V): the RED commit lands first (failing tests; `cargo test --workspace --frozen` exits non-zero on the new asserts). The GREEN commit follows in a separate commit (implementation lands; `cargo test --workspace --frozen` exits zero). The two commits are NOT collapsed.

### Phase 1 — Engine `sessions_per_long_break` (Bundle B, test-first)

**Entry**: Phase 0 complete.
**Exit**: `TimerState` gains a `sessions_per_long_break: u32` field. **Constructor signature is unchanged** (`pub fn new(durations: Durations) -> Self` stays as-is at `engine/timer.rs:202`); the new field is initialised to `4` in the existing struct-initialisation expression alongside the other defaulted fields (`total_sessions: 10`, `allow_continuous_sessions: false`, etc.). A new setter `pub const fn set_sessions_per_long_break(&mut self, n: u32)` mirrors `set_durations`'s posture at `engine/timer.rs:435` (assignment, no clamp inside the engine — the 1–10 clamp lives at the Settings UI input boundary). The 22+ existing call sites of `TimerState::new(Durations::default())` (app.rs, tray.rs ×4, session.rs ×4, `timer/mod.rs:453`, engine tests ×14) compile unchanged because the constructor arity does not change; only the production boot path (`src/src/app.rs` where settings are loaded and applied to the engine, mirroring the existing `set_durations` + `set_allow_continuous_sessions` effects in `timer/mod.rs:463-473`) gains a follow-up `set_sessions_per_long_break(settings.timer.sessions_per_long_break)` call after construction (or, equivalently, the Phase 4 Leptos `Effect::new` documented in Phase 4's exit). The default-4 at the field declaration is what keeps the existing `long_break_after_4_focus_sessions` test (`timer.rs:1267-1289`) passing without modification. Literals at `src/src/engine/timer.rs:396` (skip branch) and `:831` (natural zero-cross) are replaced with `self.completed_pomodoros.is_multiple_of(self.sessions_per_long_break)`. RED-first: the boundary tests `_eq_1`, `_eq_10`, `skip_..._eq_1`, and `mid_session_..._preserves_anchor` land failing, then the engine field + branch update makes them pass.
**Test-first**: YES per Principle V (engine state machine).
- **Test-first commit ordering** (AGENTS.md §Test-first commit ordering, Principle V): the RED commit lands first (failing tests; `cargo test --workspace --frozen` exits non-zero on the new asserts). The GREEN commit follows in a separate commit (implementation lands; `cargo test --workspace --frozen` exits zero). The two commits are NOT collapsed.

### Phase 2 — Bundle A: title input UI + persistence

**Entry**: Phase 0 complete (engine work in Phase 1 is independent and may run in parallel).
**Exit**: `src/src/components/timer/mod.rs` gains a single-line title input in the `#timer-status` row, left of the tag picker. The input's value is held in a Leptos signal local to the timer component. At focus zero-cross (where the existing `Session` write happens around `mod.rs:980`), the signal's current value is **read once** and passed into both the `Session` persist call AND the `synth_completed_session` helper. `synth_completed_session`'s signature gains `title: Option<String>` so the synthesised `ManualSession` row that today is constructed with `notes: None, tags: None, …` at `timer/mod.rs:213-230` receives the user-typed title — the calendar's per-day `ManualSession` row therefore sees the typed title, not just sessions reloaded from disk. Empty string is normalised to `None` at the boundary, never persisted as `""`. Manual-backfill form (also in the timer / calendar surface) gains a matching input for `ManualSession.title`. The 120-char cap is enforced via `maxlength=120` on the input + a length-check at the capture point. Paste handling: the browser's native `maxlength` truncates pasted strings.
**Test-first**: NO (UI plumbing; e2e + visual regression covers it).

### Phase 3 — Bundle A: calendar Title column

**Entry**: Phase 2 complete.
**Exit**: `src/src/components/calendar.rs`'s per-day sessions table gains a Title column. The rendering function inspects `session.title` and `session.tags` and emits via the following explicit three-tier fallback chain:
1. `Some(title)` → render the title truncated at ~40 visible chars with ellipsis, plus the full title in the `title=` attribute as the native tooltip;
2. `None` AND `tags.is_some()` and the inner `Vec` is non-empty → render the joined tag names by reading `Value::as_str(v.get("name"))` from each `serde_json::Value` in `tags: Option<Vec<serde_json::Value>>` (matches the existing tag-display convention) and joining on `", "`;
3. `None` AND (`tags.is_none() || tags.as_ref().map_or(true, Vec::is_empty)`) → render a non-breaking space `&nbsp;` so the row keeps its visual line height.

No `(untitled)` string sentinel — keeps the visual minimal per the flow.app-functional reference. This fallback chain matters because `synth_completed_session` produces `tags: None` today (even after Fix 3 threads the title through), so a focus completion with neither a typed title nor tags would otherwise collapse the cell to empty.

Column position: between the existing date / time column and the session-type column (or wherever the layout review settles — Phase 3's task generation will fix it).
**Test-first**: NO (UI plumbing).

### Phase 4 — Bundle B: settings UI (General tab)

**Entry**: Phase 1 complete (engine consumes `sessions_per_long_break`); Phase 0 complete (IPC field exists).
**Exit**: `src/src/components/settings/general.rs` gains a numeric input "Sessions per long break" (`<input type="number" min=1 max=10>`). The value is read from / written to the existing `Settings` signal; persistence uses the existing `save_settings` Tauri command.

The Settings → Engine wiring is added **in this phase**, not in Phase 1 — Phase 1 only adds the engine field + setter. A Leptos `Effect::new` is mounted in the timer-component init, mirroring the existing `set_allow_continuous_sessions` effect at `src/src/components/timer/mod.rs:468-473` (and the `set_durations` effect immediately above it at `:463-466`). The effect reads `settings.timer.sessions_per_long_break` and calls `engine.update(|s| s.set_sessions_per_long_break(...))`. It runs once on init (so the engine picks up the persisted value on boot) and re-runs whenever the settings signal changes (so a mid-session settings save propagates without a process restart, per the `set_durations`-mirror posture established in Phase 1).
**Test-first**: NO (UI plumbing).

### Phase 5 — Bundle C: metronome (settings UI + audio plumbing)

**Entry**: Phase 0 complete (IPC fields exist).
**Exit**:
1. `src/src/components/settings/notifications.rs` gains a checkbox "Enable metronome during focus" (writes `metronome: bool`) and a numeric input "Metronome BPM" (`min=30 max=180`, writes `metronome_bpm: u32`). Both flow through the existing `save_settings`.
2. `src/src/components/timer/mod.rs` gains a `schedule_metronome_tick(bpm: u32)` helper next to `play_chime`. The function creates a per-call `AudioContext`, an oscillator (sine, slightly different frequency to `play_chime` to avoid confusion — Phase 5 task generation picks the exact Hz), a short gain envelope, plays once, and returns — no long-lived nodes.
3. **Dedicated periodic scheduler — NOT the engine's 1-Hz tick loop.** The metronome runs on its own timer keyed at exactly `60_000 / bpm` ms via `leptos::prelude::set_interval_with_handle` (the same function the existing engine tick-loop at `src/src/components/timer/mod.rs:964` uses — no new dep). It returns `Result<IntervalHandle, JsValue>`; the handle is stored in a component-local `RwSignal<Option<IntervalHandle>>` (or `RefCell<Option<IntervalHandle>>`) and `.clear()` is called on the handle to cancel. (Alternative: `gloo-timers::callback::Interval` if a refactor adds the dep elsewhere, but **no new dep is added in this feature** (Principle IX / Technical Context).) The "ms-since-last-tick against `60_000 / bpm` per loop iteration" path is rejected (drift at high BPM; tick resolution mismatch).

   **Gate (exhaustive, all from spec):** `notifications.metronome && current_mode == Focus && is_running && !is_paused && !is_auto_paused && time_remaining_secs > 0`.

   **Lifecycle:**
   - **Created** when the gate transitions to enabled (rising edge).
   - **Cancelled and recreated** on BPM change (period changes → new `Interval` at the new period).
   - **Cancelled** when the gate transitions to disabled (`Drop` of the `Interval` handle suffices — RAII).

   **Cancel/recreate triggers (enumerate explicitly):** user pauses, user resumes, mode change (focus → break / long-break / skip-session), smart-pause auto-pause, smart-pause auto-resume on activity, overtime entry (`time_remaining_secs` reaches 0), continuous-sessions auto-start of the next focus (must re-create, not assume the prior `Interval` survives), metronome toggled off in settings, BPM value changed in settings, app close (component unmount drops the `RefCell`, dropping the `Interval` — RAII cleanup).

   **Resume signal:** scheduler restarts on the rising edge of `is_running && current_mode == Focus && time_remaining_secs > 0 && notifications.metronome` — implemented as a Leptos `Effect::new` that watches the gate signal and reconciles the `RefCell<Option<Interval>>` accordingly (drop on falling edge; construct on rising edge; drop-then-construct on BPM change).
4. Toggling `metronome` off mid-focus drops the `Interval` immediately on the next reactive flush; no further audio tick fires. At 180 BPM the next tick boundary is at most ~333 ms away, well under SC-008's 10 ms cadence-tolerance for actual ticks and satisfying SC-010's "next tick suppressed" requirement.

**Test-first**: PARTIAL. SC-008 / SC-009 wasm-bindgen-test for the tick counter is **optional test-first** depending on whether the counter-stub introduces meaningful logic; e2e is the default backstop. Decision in §[Testing strategy](#testing-strategy-and-test-first-markers).

### Phase 6 — Baseline cap: visual review + three-baseline update

**Entry**: Phases 2 / 3 / 4 / 5 complete; e2e suite passes except for the three expected visual-regression diffs.
**Exit**: The three impacted baselines (`timer`, `settings-general`, `settings-notifications`) are regenerated locally via `npx playwright test --update-snapshots visual-regression.spec.js` (or equivalent), reviewed visually one-by-one against the per-baseline justifications in §IV, committed in a single commit titled `chore(visual): update 3 baselines for feature 002 (titles + long-break + metronome)`. The PR description restates the three per-baseline notes. The `BASELINE_CAP=3` override is documented in the PR or the CI workflow's env block. The calendar Title column lands without a baseline diff because the per-day sessions table is rendered off-viewport (`src/src/components/calendar.rs:571-574`); visual coverage of the new column relies on the existing `sessions-history.spec.js:37-44` scroll-into-view flow. The CI baseline-cap gate fails as expected for any unjustified fourth diff (untouched screens flagging a diff = regression in code; fix the code, do not absorb into the baseline).
**Test-first**: N/A (visual gate is itself the test).

## Post-design Constitution Check

Re-checked after Phase 1 design (research.md, data-model.md, contracts/wire-shapes.md, quickstart.md). Verdicts unchanged from §[Constitution Check](#constitution-check). Material principles re-affirmed:

- **I (Bundle B)**: data-model.md and contracts/wire-shapes.md confirm `sessions_per_long_break` is a `u32` configuration input, mirrored on `TimerSettings` (IPC) and on `TimerState` (engine field). Engine remains DOM-free; the engine-purity grep gate enforces.
- **I (Bundle C)**: contracts/wire-shapes.md notes that no IPC field maps the metronome into the engine. The metronome is entirely UI-side. `src/src/engine/` stays at zero `web_sys` references.
- **III**: data-model.md restates `Option<String>` as the type-system encoding for the title field; no string-sentinel fallback at any IPC consumer.
- **IV**: §[Constitution Check IV](#iv-visual-regression-is-the-ui-contract--cross-cutting) pre-anchors the per-baseline justifications. The PR description must restate these verbatim — `quickstart.md` lists the verbatim text for copy-paste.
- **V**: §[Testing strategy](#testing-strategy-and-test-first-markers) enumerates the RED-first tests in `presto_ipc::session::tests` (Bundle A) and `engine::tests` (Bundle B). UI plumbing exempt per Principle V's documented carve-out.
- **VI**: contracts/wire-shapes.md explicitly states "no new Tauri commands are introduced"; the mock-drift gate stays green without changes.

## Complexity Tracking

> No Constitution Check violations require justification. The IV widening (cap 2 → 3) is a documented gate-override, not a violation.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|---|---|---|
| (none) | — | — |
