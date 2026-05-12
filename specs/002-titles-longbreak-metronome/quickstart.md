# Quickstart: Per-Session Titles, Configurable Long-Break Cadence, Opt-In Metronome

**Phase**: 1 (Design & Contracts)
**Feeds**: [plan.md](./plan.md), [data-model.md](./data-model.md), [contracts/wire-shapes.md](./contracts/wire-shapes.md)

A contributor's end-to-end path to exercise each bundle locally. Assumes the contributor has already done the 001-leptos-migration quickstart (rustup, trunk, Playwright). All paths are repo-root-relative.

---

## Prerequisites (one-time)

```bash
rustup toolchain install 1.83.0
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
(cd tests/e2e && npm ci && npx playwright install --with-deps chromium)
```

---

## Run the test suite (Phase 0 + Phase 1 first; everything else after Phase 5)

```bash
# IPC wire-shape round-trips (Bundle A title + B sessions_per_long_break + C metronome defaults).
cargo test --workspace --frozen -p presto-ipc

# Engine state-machine boundary tests (Bundle B).
cargo test --workspace --frozen -p presto-web engine::tests::long_break engine::tests::skip_session engine::tests::mid_session

# Full workspace test pass.
cargo test --workspace --frozen
```

All four sets must pass before merging the implementation phases (Phase 2+).

---

## Bundle A — Per-session titles, end-to-end

**Goal**: set a title, complete a focus session, see the title in the calendar.

```bash
# Start the dev server (Trunk).
(cd src && trunk serve --port 1420)

# Separately, start the Tauri dev shell pointing at the trunk server.
# (Or use the Tauri-mock fast path: open http://127.0.0.1:1420 directly;
#  the bridge degrades gracefully and persistence falls back per
#  AGENTS.md §Bridge availability.)
cargo tauri dev
```

In the app:

1. On the timer screen, locate the `#timer-status` row. To the **left** of the tag picker, a single-line text input is present, placeholder "What is this session for?".
2. Type a title: `Spec 002 review`.
3. Start the focus session.
4. Set the focus duration to 1 minute in Settings → General for a fast end-to-end test (or wait 25 minutes).
5. When the focus session zero-crosses (chime fires, mode transitions to break), open the calendar view.
6. In the per-day sessions table for today's date, the row for the just-completed session shows **"Spec 002 review"** in the new Title column.
7. Add a manual-backfill session via the calendar's add-session form, type a title, submit. The new row shows the typed title.

**Legacy-record check**:

```bash
# Stop the app. Inspect the on-disk file. Confirm the new field exists for the completed session.
cat ~/.local/share/com.presto.app/history.json   # Linux
# OR
cat ~/Library/Application\ Support/com.presto.app/history.json   # macOS
```

Look for a `"title": "Spec 002 review"` field on the most recent session. Older session entries lack the field entirely — that's the `#[serde(default)]` contract working (they load as `title = None`, render as joined tag names in the calendar).

---

## Bundle B — Configurable sessions-per-long-break, end-to-end

**Goal**: change the cadence to `N=3`, run 3 focus cycles, assert the 3rd transitions to long break.

In the app:

1. Open Settings → General. Locate the new "Sessions per long break" numeric input (default `4`, range `1–10`).
2. Set to `3`. Save.
3. Set focus duration to 1 minute and break duration to 1 minute for a fast test.
4. Run three back-to-back focus sessions (let each zero-cross naturally, or use the skip button — both paths are covered).
5. After the 3rd focus completion, the next mode is **`LongBreak`**, not `Break`. The `long_break_duration` (default 20 min) is the time on the clock.
6. Run the 4th focus session. After it completes, the next mode is **`Break`** (short) — the cadence counter reset.

**Engine boundary test**:

```bash
# RED-first per Principle V: the test file lands failing in Phase 1's RED commit,
# passes in the GREEN commit when the engine consumes the field.
cargo test --workspace -p presto-web engine::tests::long_break_after_n_focus_sessions_with_n_eq_1
cargo test --workspace -p presto-web engine::tests::long_break_after_n_focus_sessions_with_n_eq_10
cargo test --workspace -p presto-web engine::tests::skip_session_long_break_with_n_eq_1
cargo test --workspace -p presto-web engine::tests::mid_session_sessions_per_long_break_change_preserves_anchor
```

**Mid-session-change check**:

1. Start a focus session with `sessions_per_long_break = 4`.
2. Open Settings → General, change to `1`, save.
3. The running focus session does **not** truncate. `time_remaining_secs` and `current_mode` are unchanged at the moment of save.
4. Let the session complete. The next mode is `LongBreak` (because `completed_pomodoros.is_multiple_of(1) == true`).

---

## Bundle C — Metronome, end-to-end

**Goal**: enable the metronome, hear it tick during focus, confirm it stops on every gate-violation state.

In the app:

1. Open Settings → Notifications. Locate the new "Enable metronome during focus" checkbox (default off) and "Metronome BPM" numeric input (default `60`, range `30–180`).
2. Enable the checkbox. Leave BPM at `60`.
3. Set focus duration to 1 minute. Save settings.
4. Start a focus session. Listen for the soft sine tick once per second.
5. Pause the timer. **Ticks stop within the next tick-loop iteration (≤1 s).**
6. Resume. Ticks resume.
7. Let the session complete. The chime fires; ticks **stop** at the zero-cross (mode transitions to break). No ticks during break.
8. (If smart-pause is enabled and `smart_pause_timeout` elapses without activity:) Auto-pause fires; ticks stop.
9. (If `allow_continuous_sessions` is enabled:) On the auto-started next focus, ticks **resume automatically** — the gate is "focus is ticking", not "user pressed start" (per spec A8).
10. (Overtime test, requires `allow_continuous_sessions = true`:) Let a focus session run past zero; ticks stop at the zero-cross even though the session is still "open".

**BPM boundary check**:

1. In Settings, set BPM to `180`. Run a focus session. Three ticks per second.
2. In Settings, set BPM to `30`. Run a focus session. One tick every 2 seconds.
3. Try typing `0`, `250`, or a negative number into the BPM input. The HTML `min`/`max` attributes clamp the value at the input boundary; the persisted `metronome_bpm` is always 30–180.

**Audio-stability check** (long-running):

Run a 25-minute focus session with BPM `180` (≈4500 ticks). The browser tab's audio context remains responsive (no `AudioContext` exhaustion, no memory growth — each tick is per-call `AudioContext` with a short envelope, mirroring `play_chime`).

---

## Visual regression — per-baseline justification (Phase 6)

After all bundles land:

```bash
# Run the visual regression suite. Expect 3 baselines to fail (the touched screens).
(cd tests/e2e && npx playwright test visual-regression.spec.js)

# Inspect the diffs visually. Confirm each one matches the per-baseline note below.
(cd tests/e2e && npx playwright show-report)
```

Then update only the three expected baselines:

```bash
(cd tests/e2e && npx playwright test --update-snapshots visual-regression.spec.js)
git status   # should show exactly 3 modified PNGs under tests/e2e/__screenshots__/visual-regression/
```

**Per-baseline justification** (copy into the eventual PR body verbatim — these are the three restatements required by Principle IV):

> - `timer-chromium-linux.png`: title input added to `#timer-status` row, left of the tag picker. No other layout change.
> - `settings-general-chromium-linux.png`: new "Sessions per long break" numeric input added as a form row. No layout change to existing rows.
> - `settings-notifications-chromium-linux.png`: new "Enable metronome during focus" checkbox + "Metronome BPM" numeric input added as form rows. No layout change to existing rows.

(`calendar-chromium-linux.png` is excluded — the per-day sessions table is intentionally rendered off-viewport per `src/src/components/calendar.rs:571-574`; the Title column lands without a baseline diff; e2e coverage is `tests/e2e/sessions-history.spec.js:37-44`.)

**Override the baseline-cap CI gate** (default cap is 2; this PR's count is 3):

```bash
# In CI: set BASELINE_CAP=3 in the workflow env block for this PR's run.
# Locally: BASELINE_CAP=3 scripts/check-baseline-cap.sh
```

If any baseline **outside** the three touched screens flags a diff, **fix the code** — do not absorb the diff into the baseline. Per FR-025 and Principle IV: an untouched-screen diff is a regression.

---

## Lints + gates — final sweep

```bash
cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic
cargo fmt --check
scripts/check-engine-purity.sh    # zero web_sys references under src/src/engine/
scripts/check-mock-drift.sh        # no new commands; gate green by inaction
BASELINE_CAP=3 scripts/check-baseline-cap.sh
scripts/check-lockfile-drift.sh    # no new deps; gate green by inaction
```

All green → ready for PR.
