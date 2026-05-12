# Tasks: Per-Session Titles, Configurable Long-Break Cadence, Opt-In Metronome

**Input**: Design docs in `specs/002-titles-longbreak-metronome/`
**Prerequisites**: spec.md, plan.md (POST-fix, 6 phases), data-model.md, contracts/wire-shapes.md, quickstart.md

## Format

`- [ ] [TID] [P?] [Bundle] [Phase] Description with file path` — Bundle ∈ {A,B,C,X}; Phase ∈ {0..6}. `[P]` = parallelizable with other `[P]` tasks in the same phase. Each task lists its **Done-signal** and **Files**. Test-first tasks (Phase 0 + Phase 1) explicitly name **RED** / **GREEN** commit boundaries.

Bundles: **A** = per-session titles · **B** = configurable long-break · **C** = metronome · **X** = cross-cutting.

---

## Phase 0 — IPC wire-shape evolutions (test-first)

**Goal**: widen `Session`, `ManualSession`, `TimerSettings`, `NotificationSettings` per `data-model.md`. Six RED-then-GREEN pairs. RED commits land first and fail `cargo test --workspace --frozen`; GREEN commits land in a separate commit. **The two commits are not collapsed.**

- [ ] **T001** [X] [Phase 0] RED: add failing `presto_ipc::session::tests::title_round_trip_some_none_missing_key` for `Session`
  - **Files**: `crates/presto-ipc/src/session.rs`
  - **Done-signal**: `cargo test --workspace --frozen -p presto-ipc session::tests::title_round_trip_some_none_missing_key` fails with `error[E0560]` or `assertion failed` referencing the missing `title` field on `Session`. Commit the failing test.

- [ ] **T002** [A] [Phase 0] GREEN: add `title: Option<String>` with `#[serde(default)]` to `Session`
  - **Files**: `crates/presto-ipc/src/session.rs`
  - **Done-signal**: `cargo test --workspace --frozen -p presto-ipc session::tests::title_round_trip_some_none_missing_key` passes. Commit separately from T001.
  - **BlockedBy**: T001.

- [ ] **T003** [X] [Phase 0] RED: add failing `presto_ipc::session::tests::manual_session_title_round_trip_some_none_missing_key`
  - **Files**: `crates/presto-ipc/src/session.rs`
  - **Done-signal**: `cargo test --workspace --frozen -p presto-ipc session::tests::manual_session_title_round_trip_some_none_missing_key` fails referencing missing field on `ManualSession`. Separate commit from T002.
  - **BlockedBy**: T002.

- [ ] **T004** [A] [Phase 0] GREEN: add `title: Option<String>` with `#[serde(default)]` to `ManualSession`
  - **Files**: `crates/presto-ipc/src/session.rs`
  - **Done-signal**: `cargo test --workspace --frozen -p presto-ipc` passes. Separate commit from T003.
  - **BlockedBy**: T003.

- [ ] **T005** [X] [Phase 0] RED: add failing `presto_ipc::settings::tests::sessions_per_long_break_default_4` AND `sessions_per_long_break_custom_round_trips`
  - **Files**: `crates/presto-ipc/src/settings.rs`
  - **Done-signal**: both tests compile-fail or assertion-fail (field missing on `TimerSettings`). Separate commit.

- [ ] **T006** [B] [Phase 0] GREEN: add `sessions_per_long_break: u32` to `TimerSettings` with `#[serde(default = "default_sessions_per_long_break")]` + `#[must_use] pub const fn default_sessions_per_long_break() -> u32 { 4 }`
  - **Files**: `crates/presto-ipc/src/settings.rs`
  - **Done-signal**: `cargo test --workspace --frozen -p presto-ipc settings::tests::sessions_per_long_break_default_4 settings::tests::sessions_per_long_break_custom_round_trips` passes. Update `Default for TimerSettings` to call the new const fn. Separate commit from T005.
  - **BlockedBy**: T005.

- [ ] **T007** [X] [Phase 0] RED: add failing `presto_ipc::settings::tests::metronome_default_off_60_bpm` AND `metronome_custom_round_trips`
  - **Files**: `crates/presto-ipc/src/settings.rs`
  - **Done-signal**: both tests fail (fields missing on `NotificationSettings`). Separate commit.
  - **BlockedBy**: T006.

- [ ] **T008** [C] [Phase 0] GREEN: add `metronome: bool` (bare `#[serde(default)]`) + `metronome_bpm: u32` (`#[serde(default = "default_metronome_bpm")]`) to `NotificationSettings`; add `#[must_use] pub const fn default_metronome_bpm() -> u32 { 60 }`
  - **Files**: `crates/presto-ipc/src/settings.rs`
  - **Done-signal**: `cargo test --workspace --frozen -p presto-ipc` passes. Update `Default for NotificationSettings`. Existing `#[allow(clippy::struct_excessive_bools)]` covers the new bool. Separate commit from T007.
  - **BlockedBy**: T007.

- [ ] **T009** [P] [X] [Phase 0] Mock-drift sanity check (proof that no Tauri command was added)
  - **Files**: `tests/e2e/fixtures/tauriMock.js` (read-only verification — no edit expected)
  - **Done-signal**: `bash scripts/check-mock-drift.sh` exits 0. No new `#[tauri::command]` handlers, no new mock `case` branches.

**Phase 0 exit**: `cargo test --workspace --frozen` green; `bash scripts/check-mock-drift.sh` green; engine and UI behaviour unchanged.

---

## Phase 1 — Engine `sessions_per_long_break` (test-first)

**Goal**: parameterise the engine's `is_multiple_of(4)` literals at `:396` (skip) and `:831` (natural). Field default stays `4` so the existing `long_break_after_4_focus_sessions` test (`timer.rs:1267-1289`) keeps passing **unchanged** — that's the regression contract. RED-then-GREEN, separate commits.

- [ ] **T010** [X] [Phase 1] RED: add four failing boundary tests in `src/src/engine/timer.rs::tests`
  - Tests: `long_break_after_n_focus_sessions_with_n_eq_1` (every focus → LongBreak), `long_break_after_n_focus_sessions_with_n_eq_10` (LongBreak only on the 10th), `skip_session_long_break_with_n_eq_1` (skip branch consults the field), `mid_session_sessions_per_long_break_change_preserves_anchor` (setter does not reset `time_remaining_secs` or `current_mode`).
  - **Files**: `src/src/engine/timer.rs` (test module).
  - **Done-signal**: `cargo test --workspace --frozen -p presto-web engine::tests::long_break_after_n_focus_sessions_with_n_eq_1 engine::tests::long_break_after_n_focus_sessions_with_n_eq_10 engine::tests::skip_session_long_break_with_n_eq_1 engine::tests::mid_session_sessions_per_long_break_change_preserves_anchor` fails to compile (calls to missing `set_sessions_per_long_break`) or fails assertions. Separate commit.
  - **BlockedBy**: T008.

- [ ] **T011** [B] [Phase 1] GREEN: add `sessions_per_long_break: u32` field to `TimerState`, defaulted to `4` at the existing `TimerState::new` struct-init (`engine/timer.rs:202`). Constructor signature is **unchanged** (per plan Fix 4).
  - **Files**: `src/src/engine/timer.rs`.
  - **Done-signal**: `cargo build --workspace --frozen` compiles. The 22+ existing `TimerState::new(Durations::default())` call sites compile unchanged. Separate commit from T010.
  - **BlockedBy**: T010.

- [ ] **T012** [B] [Phase 1] GREEN: add `pub const fn set_sessions_per_long_break(&mut self, n: u32)` mirroring `set_durations` (`:435`) — assignment only, no clamp inside the engine.
  - **Files**: `src/src/engine/timer.rs`.
  - **Done-signal**: setter compiles; `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic` green. Same commit as T011 or immediately-following commit.
  - **BlockedBy**: T011.

- [ ] **T013** [B] [Phase 1] GREEN: replace the hard-coded `4` literals at `engine/timer.rs:396` (skip branch) and `:831` (natural zero-cross) with `self.completed_pomodoros.is_multiple_of(self.sessions_per_long_break)`.
  - **Files**: `src/src/engine/timer.rs`.
  - **Done-signal**: `cargo test --workspace --frozen` passes ALL engine tests including the four added in T010 AND the pre-existing `long_break_after_4_focus_sessions`. `grep -n 'is_multiple_of(4)' src/src/engine/timer.rs` returns zero hits. Separate commit ending the RED→GREEN cycle.
  - **BlockedBy**: T012.

- [ ] **T014** [P] [X] [Phase 1] VERIFY pre-existing `engine::tests::long_break_after_4_focus_sessions` (`timer.rs:1267-1289`) still passes with no source-code change to that test.
  - **Done-signal**: `cargo test --workspace --frozen -p presto-web engine::tests::long_break_after_4_focus_sessions` passes; `git diff src/src/engine/timer.rs` shows that the function body of `long_break_after_4_focus_sessions` is byte-stable.
  - **BlockedBy**: T013.

**Phase 1 exit**: full workspace `cargo test --workspace --frozen` green; `scripts/check-engine-purity.sh` green (zero new `web_sys` references under `src/src/engine/`).

---

## Phase 2 — Bundle A: title input UI + persistence

**Goal**: surface a single-line title input in `#timer-status`, capture once at focus zero-cross, thread into both the `Session` persist call AND `synth_completed_session`. UI plumbing — not test-first per Principle V; e2e is the backstop.

- [ ] **T015** [A] [Phase 2] Add a `RwSignal<String>` holding the in-flight title in the timer component and render an `<input type="text" maxlength="120" placeholder="What is this session for?">` to the **left** of the tag picker in the `#timer-status` row.
  - **Files**: `src/src/components/timer/mod.rs`.
  - **Done-signal**: open `http://localhost:1420`, see the empty input left of the tag picker with the placeholder. Typing >120 chars is rejected at the boundary.
  - **BlockedBy**: T004.

- [ ] **T016** [A] [Phase 2] Modify `synth_completed_session` (`timer/mod.rs:213-230`) signature: add `title: Option<String>` parameter, write it into the synthesised `ManualSession.title`.
  - **Files**: `src/src/components/timer/mod.rs`.
  - **Done-signal**: function signature is `fn synth_completed_session(now_ms: i64, focus_duration_secs: u32, title: Option<String>) -> ManualSession`. `cargo build --workspace --frozen` compiles; all in-tree callers updated (only `mod.rs:980`).
  - **BlockedBy**: T015.

- [ ] **T017** [A] [Phase 2] At the focus zero-cross persistence site (`timer/mod.rs:~980`), read the title signal **once**, normalise empty-string to `None`, pass the same value into BOTH the `Session` write AND `synth_completed_session`. Clear the title signal after persistence.
  - **Files**: `src/src/components/timer/mod.rs`.
  - **Done-signal**: type "Spec 002", let focus complete; on-disk `history.json` shows `"title": "Spec 002"` for that record; calendar today's row also shows it (synth path). Empty-input session persists with `title: null` or omitted key.
  - **BlockedBy**: T016.

- [ ] **T018** [A] [Phase 2] Wire the manual-backfill form's `ManualSession` constructor to capture the user-typed title (same empty→None normalisation).
  - **Files**: `src/src/components/calendar.rs` (or wherever the add-session modal lives — Phase 2 generator confirms file).
  - **Done-signal**: open calendar → "Add session", type a title, submit; the new row shows the title and `manual-sessions.json` carries `"title": "..."`.
  - **BlockedBy**: T017.

---

## Phase 3 — Bundle A: calendar Title column

**Goal**: add the Title column to the per-day sessions table with the three-tier fallback chain. Off-viewport per `calendar.rs:571-574` → no visual baseline diff.

- [ ] **T019** [A] [Phase 3] Add a Title column header to the per-day sessions table.
  - **Files**: `src/src/components/calendar.rs` (`#sessions-table-body`).
  - **Done-signal**: scroll the calendar's per-day table into view (per `sessions-history.spec.js:37-44`); the column header reads "Title" and sits between the existing date/time and session-type columns.
  - **BlockedBy**: T018.

- [ ] **T020** [A] [Phase 3] Render the Title column body with the three-tier fallback chain: (1) `Some(t)` → truncate to ~40 visible chars with ellipsis, full text in `title=` attribute for native tooltip; (2) `None` AND `tags.is_some()` and non-empty → joined tag names via `Value::as_str(v.get("name"))`; (3) `None` AND empty/missing tags → `&nbsp;`. No `(untitled)` sentinel.
  - **Files**: `src/src/components/calendar.rs`.
  - **Done-signal**: in the per-day table, a titled row shows the title (truncated + tooltip on overflow); a `None`-row with tags shows joined tag names; a `None`-row without tags renders a non-breaking space (line-height preserved, no collapse). `tests/e2e/sessions-history.spec.js` still passes; `cargo clippy ... -W clippy::pedantic` green.
  - **BlockedBy**: T019.

---

## Phase 4 — Bundle B: Settings General UI + Settings→Engine effect

**Goal**: surface the 1–10 input; mount the Leptos `Effect::new` that mirrors `set_durations` / `set_allow_continuous_sessions` at `timer/mod.rs:463-473`.

- [ ] **T021** [B] [Phase 4] Add a numeric input "Sessions per long break" (`<input type="number" min="1" max="10">`) to the General tab, bound to `settings.timer.sessions_per_long_break`. Save flows through the existing `save_settings`.
  - **Files**: `src/src/components/settings/general.rs`.
  - **Done-signal**: open Settings → General; the input is present with default `4`; typing `0`, `11`, `99` clamps at the UI layer; save persists the in-range value to `settings.json`.
  - **BlockedBy**: T006, T013.

- [ ] **T022** [B] [Phase 4] Mount a Leptos `Effect::new` in the timer component init **immediately adjacent to the existing `set_durations` / `set_allow_continuous_sessions` effects at `timer/mod.rs:463-473`**, reading `settings.timer.sessions_per_long_break` and calling `engine.update(|s| s.set_sessions_per_long_break(...))`. Runs once on init **and** on every settings change.
  - **Files**: `src/src/components/timer/mod.rs`.
  - **Done-signal**: set `sessions_per_long_break = 3` in Settings; run three focus completions (use 1-min focus duration); third completion → `LongBreak`. Change mid-focus to `1` per quickstart "mid-session-change check": running session's `time_remaining_secs` and `current_mode` unchanged at moment of save; next zero-cross → `LongBreak`.
  - **BlockedBy**: T021.

---

## Phase 5 — Bundle C: metronome (settings UI + dedicated scheduler)

**Goal**: Settings → Notifications gets a checkbox + BPM input; a **dedicated** periodic scheduler keyed at `60_000 / bpm` ms runs the audio side effect — **not** polled against the engine's 1-Hz tick loop.

- [ ] **T023** [C] [Phase 5] Add a checkbox "Enable metronome during focus" + numeric input "Metronome BPM" (`min="30" max="180"`) to Settings → Notifications. Both flow through `save_settings`.
  - **Files**: `src/src/components/settings/notifications.rs`.
  - **Done-signal**: open Settings → Notifications; the new rows are present; checkbox default unchecked, BPM default 60; typing `0`, `25`, `200`, `-1` clamps to 30–180 at the UI layer.
  - **BlockedBy**: T008.

- [ ] **T024** [C] [Phase 5] Add a `schedule_metronome_tick(bpm: u32)` helper sitting next to `play_chime` in `timer/mod.rs`. Per call: fresh `AudioContext`, sine oscillator at a frequency distinct from `play_chime`, short gain envelope, no long-lived nodes.
  - **Files**: `src/src/components/timer/mod.rs`.
  - **Done-signal**: manually invoking the helper in dev produces one audible tick; `scripts/check-engine-purity.sh` still green (helper lives in components, not engine); `grep -n 'web_sys' src/src/engine/` returns nothing new.
  - **BlockedBy**: T023.

- [ ] **T025** [C] [Phase 5] Add a component-local `RwSignal<Option<IntervalHandle>>` (or `RefCell<Option<IntervalHandle>>`) to hold the scheduler handle. Drive the lifecycle via a Leptos `Effect::new` watching the **exhaustive gate**: `notifications.metronome && current_mode == Focus && is_running && !is_paused && !is_auto_paused && time_remaining_secs > 0`.
  - **Files**: `src/src/components/timer/mod.rs`.
  - **Done-signal**: code review shows the effect drops the prior handle on falling-edge and creates a new `set_interval_with_handle(move || schedule_metronome_tick(bpm), Duration::from_millis(60_000 / bpm))` on rising-edge; BPM change drops-then-recreates within the same effect run.
  - **BlockedBy**: T024.

- [ ] **T026** [C] [Phase 5] Wire **cancel triggers** explicitly via the effect's gate transitions and an additional watcher on `metronome_bpm`: user-pause, user-resume (recreate), mode change (focus→break/long-break/skip), smart-pause auto-pause, smart-pause auto-resume on activity, overtime entry (`time_remaining_secs` reaches 0), continuous-sessions auto-start of the next focus (recreate, do not rely on prior interval survival), `metronome` toggle off, BPM value change (cancel + recreate at new period), app close (component-unmount RAII drop).
  - **Files**: `src/src/components/timer/mod.rs`.
  - **Done-signal**: walk through each quickstart Bundle C scenario (pause, resume, complete, smart-pause, overtime, continuous-sessions auto-start, settings toggle off, BPM change mid-focus); audibly confirm ticks stop/resume per scenario. SC-010: toggling `metronome` off mid-focus suppresses the next scheduled tick within one reactive flush (≤333 ms at 180 BPM).
  - **BlockedBy**: T025.

- [ ] **T027** [P] [C] [Phase 5] OPTIONAL: wasm-bindgen-test counter-stub for SC-008 / SC-009 (oscillator-creation count under simulated focus / paused / break / long-break / overtime / `metronome=false` states). Plan §Testing strategy permits skipping if the stub adds no logic.
  - **Files**: `src/src/components/timer/mod.rs::tests` (or new test module).
  - **Done-signal**: `wasm-pack test --headless --chrome` (or repo equivalent) passes with the new counter test; OR the task is explicitly skipped and noted in the PR body with "e2e is the backstop per plan §Testing strategy". Either outcome is acceptable.
  - **BlockedBy**: T026.

**Phase 5 exit**: `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic` green; `scripts/check-engine-purity.sh` green; manual Bundle C quickstart pass.

---

## Phase 6 — Baselines: visual review + 3-baseline update

**Goal**: regenerate exactly **three** baselines (timer, settings-general, settings-notifications). Calendar baseline is **NOT** in the regenerate set — the per-day sessions table sits off-viewport per `calendar.rs:571-574`.

- [ ] **T028** [X] [Phase 6] Run the visual regression suite and confirm exactly three baselines flag a diff: `timer-chromium-linux.png`, `settings-general-chromium-linux.png`, `settings-notifications-chromium-linux.png`. Any **other** diff is a regression in code — do not absorb into the baseline.
  - **Done-signal**: `(cd tests/e2e && npx playwright test visual-regression.spec.js)` reports failures only on the three named files; `(cd tests/e2e && npx playwright show-report)` inspected visually.
  - **BlockedBy**: T020, T022, T026.

- [ ] **T029** [X] [Phase 6] Regenerate the three baselines: `(cd tests/e2e && npx playwright test --update-snapshots visual-regression.spec.js)`. Verify `git status` shows **exactly three** PNGs modified under `tests/e2e/__screenshots__/visual-regression/` and that `calendar-chromium-linux.png` is **not** in that list.
  - **Done-signal**: `git status --porcelain tests/e2e/__screenshots__/visual-regression/ | wc -l` returns `3`; `git status` does not list `calendar-chromium-linux.png`.
  - **BlockedBy**: T028.

- [ ] **T030** [X] [Phase 6] One-line justification per baseline, into the PR body (verbatim from plan §IV):
  1. `timer-chromium-linux.png`: title input added to `#timer-status` row, left of the tag picker. No other layout change.
  2. `settings-general-chromium-linux.png`: new "Sessions per long break" numeric input added as a form row. No layout change to existing rows.
  3. `settings-notifications-chromium-linux.png`: new "Enable metronome during focus" checkbox + "Metronome BPM" numeric input added as form rows. No layout change to existing rows.
  - **Done-signal**: the three notes appear verbatim in the PR description body (or in a `BASELINE_NOTES.md` adjacent to `tasks.md` if PR not yet open).
  - **BlockedBy**: T029.

- [ ] **T031** [X] [Phase 6] Document the `BASELINE_CAP=3` override on the cap-script. Either set `BASELINE_CAP=3` in the PR's CI workflow env block, OR run `BASELINE_CAP=3 bash scripts/check-baseline-cap.sh` locally and document in the PR body. Default cap is 2; this PR's count is 3.
  - **Done-signal**: `BASELINE_CAP=3 bash scripts/check-baseline-cap.sh` exits 0.
  - **BlockedBy**: T030.

---

## Final sweep — gates

- [ ] **T032** [X] Final lint + gate sweep before PR.
  - **Done-signal** (each must exit 0):
    - `cargo fmt --check`
    - `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic`
    - `cargo test --workspace --frozen`
    - `bash scripts/check-engine-purity.sh`
    - `bash scripts/check-mock-drift.sh`
    - `BASELINE_CAP=3 bash scripts/check-baseline-cap.sh`
    - `bash scripts/check-lockfile-drift.sh`
  - **BlockedBy**: T031.

---

## Dependencies (compact)

- **Phase 0** (T001–T009): RED→GREEN pairs in strict pairwise order (T001→T002, T003→T004, T005→T006, T007→T008). T009 parallelisable after T008.
- **Phase 1** (T010–T014): T010 (RED) → T011 → T012 → T013 (GREEN) → T014 (verify). T014 parallel with downstream Phase 2 work.
- **Phase 2** (T015–T018): linear within phase; depends on T004.
- **Phase 3** (T019–T020): depends on Phase 2 complete.
- **Phase 4** (T021–T022): depends on T006 (IPC) + T013 (engine). Parallel with Phases 2/3 if staffed.
- **Phase 5** (T023–T027): depends on T008. T027 optional + parallel with T026.
- **Phase 6** (T028–T031): depends on Phases 2/3/4/5 complete.
- **T032**: depends on T031.

## Parallel opportunities

- Phase 2/3 (Bundle A) can run in parallel with Phase 4 (Bundle B UI) and Phase 5 (Bundle C UI) once Phases 0+1 are green — three independent file regions.
- T014 (verify) parallel with start of Phase 2.
- T009 (mock-drift sanity) parallel with anything in Phase 0 after T008.
- T027 (optional wasm-bindgen-test) parallel with T026 or with Phase 6.

## Notes

- **RED/GREEN commits are not collapsed** in Phase 0 (T001/T002, T003/T004, T005/T006, T007/T008) or Phase 1 (T010 then T011→T013). Each RED commit lands first with a failing `cargo test`; each GREEN follows in a separate commit.
- **No new Tauri commands** — T009 is the proof. `tests/e2e/fixtures/tauriMock.js` is untouched in this feature.
- **Calendar baseline (`calendar-chromium-linux.png`) is not regenerated** — the per-day sessions table sits off-viewport per `src/src/components/calendar.rs:571-574`. Visual coverage of the Title column falls back to the existing `tests/e2e/sessions-history.spec.js:37-44` scroll-into-view flow.
- **Engine purity**: Bundle C adds zero `web_sys` references under `src/src/engine/`. The dedicated scheduler lives in `src/src/components/timer/mod.rs` only.
- **Constructor signature unchanged**: `TimerState::new(durations: Durations)` is preserved (plan Fix 4); the 22+ existing call sites compile unchanged.
- **Mid-session settings change**: T022's `Effect::new` mirrors the existing `set_durations` / `set_allow_continuous_sessions` posture at `timer/mod.rs:463-473`; running session's anchor is not reset.
