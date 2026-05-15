# Quickstart: Timer Control Rework + Quick Log + Distraction Capture

**Feature**: 006-timer-controls-quicklog-distractions
**Audience**: developer exercising the implemented feature end-to-end.
**Pre-req**: feature branch checked out, frontend + backend built, `npm ci` + `cargo build --frozen` clean.

This is a dev-exercise script, not a test. The full RED/GREEN test enumeration lives in `plan.md`.

---

## Setup

```bash
# From repo root
npm ci
cargo build --frozen --manifest-path src-tauri/Cargo.toml
cargo build --frozen --manifest-path src/Cargo.toml

# Run the trunk dev server (port 1420) — for the e2e mock context
cd src && trunk serve
# In a second shell, run the Tauri dev shell (requires GUI deps — do NOT run in CI/agentex worktree)
cargo tauri dev
```

---

## Exercise 1 — Honest early completion (Story 1, P1)

1. Open the timer view. Confirm Idle Focus.
2. Click the chip in `#timer-status-pill`, pick a tag (or none).
3. Click in the title region (placeholder `session title…`), type `Write the 006 spec`.
4. Press `▶ Play`. Confirm: pill collapses to read-only (chevron gone, title `readonly`). Right-slot button is now `Distraction`. Left-slot button is now `Abort`.
5. Wait ≥ 30 seconds.
6. Press `⏸ Pause`. Confirm: right-slot button is now `✓ Complete` (filled). Left-slot still `Abort`.
7. Press `✓ Complete`. Confirm:
   - `completed_pomodoros` increments by 1 (visible in stats / tray).
   - Mode advances to Break (or LongBreak per cadence).
   - Bell + OS notification fire.
   - Title clears.
   - Pill becomes interactive again on the new mode's Idle (no title region in Break/LongBreak — chip + mode label only).

### Variant 1a — Below-threshold complete acts as Abort

1. Open Idle Focus. Type a title.
2. Press `▶ Play`. Wait 10 seconds.
3. Press `⏸ Pause`. Press `✓ Complete`.
4. Confirm: no count, no advance, returns to Idle Focus with the title still in the pill. No toast, no warning.

### Variant 1b — Continuous-mode overtime

1. In Settings, enable continuous mode (`notifications.allow_continuous_sessions = true`).
2. Open Idle Focus with the focus duration set short for testing (e.g., 1 minute).
3. Press `▶ Play`. Let it tick past the 1-minute mark into overtime.
4. Press `⏸ Pause` at, say, 1m 20s elapsed.
5. Press `✓ Complete`. Confirm: counts as one pomodoro at the actual elapsed (~80 seconds toward `total_focus_secs`). Mode advances.

---

## Exercise 2 — Distraction capture without breaking flow (Story 2, P1)

1. Open Idle Focus. Press `▶ Play`. Wait 8 seconds.
2. Press `! Note Distraction` (right slot). Confirm: modal opens auto-focused, title reads `Note distraction`, input has `maxlength=120`.
3. Type `call the dentist`. Press Enter.
4. Confirm:
   - Modal closes within one render tick.
   - Timer keeps ticking (`is_running` still `true`; `current_session_elapsed_secs` unchanged ±1 s).
   - Open today's Inventory (navigate to daily Stats / Calendar). Confirm: today's Distractions row shows `call the dentist`, an ISO-8601 timestamp, and a parent-session-ref label (start ts + mode + tag + title).

### Variant 2a — Escape cancels

1. Press `! Note Distraction`. Type partial text. Press Escape.
2. Confirm: modal closes, no write, timer untouched.

### Variant 2b — Multiple distractions back-to-back

1. While Running, press `! Note Distraction` twice in quick succession, submitting `A` then `B`.
2. Confirm: today's Distractions has both rows with distinct `createdAt` and identical `parentRef`.

---

## Exercise 3 — Quick Log without starting a pomodoro (Story 3, P2)

1. Open the timer in Idle (any mode — Focus / Break / LongBreak).
2. Press `+ Quick Log` (left slot). Confirm: modal opens, title `Log a quick task`, title field auto-focused with `maxlength=120`, minutes field defaulted to `5` (min `1`, max `720`).
3. Type `Reply to Maria`. Leave minutes at `5`. Press Submit.
4. Confirm:
   - Modal closes.
   - Open today's Inventory. Quick logs has a row with `Reply to Maria`, `5 minutes`, an ISO-8601 timestamp.
   - Open daily Stats tile. Label reads `0 pomodoros · 1 quicklogs` (or `K pomodoros · 1 quicklogs` if K > 0).
   - Pomodoro counter is unchanged. Mode is unchanged. `pomodoros_until_long_break` is unchanged.

### Variant 3a — Out-of-range minutes rejected

1. Open the Quick Log modal. Type a title. Enter `0` in minutes. Try Submit.
2. Confirm: Submit is disabled (or rejected at the form layer). The Tauri command would also reject (`BridgeError::InvalidArgument { field: "elapsedMinutes" }`) if a malicious client bypassed the form.
3. Repeat with `721`. Same outcome.

### Variant 3b — Quick Log from Inventory header

1. Navigate to daily Stats / Calendar. In the Inventory header, press `+ Quick Log`.
2. Confirm: same modal as Story 3. Submit lands in today's Quick logs.

---

## Exercise 4 — Combined-pill semantics (Story 4, P2)

1. Open Idle Focus with no title set, no tag selected.
2. Confirm: combined pill renders (chip + mode label + chevron + placeholder `session title…`).
3. Click the chip — tag dropdown opens.
4. Click the placeholder — title input gains focus.
5. Type `Spec writing`. Press `▶ Play`.
6. Confirm: pill collapses to static read-only. Chevron gone. Chip click is a no-op. Title input has `readonly`. Pause; same.
7. Skip to Break (`→ Skip Mode` from Idle, or press `⏸ Pause` then `✓ Complete`). Confirm: title region absent in Break/LongBreak Idle — only chip + mode label render.

---

## Exercise 5 — Inventory review and editing (Story 5, P3)

1. Seed three Quick logs and two Distractions for today (via the manual exercises above, or via direct mock-store seeding in dev tools).
2. Open daily Stats. Confirm: tile label reads e.g. `5 pomodoros · 3 quicklogs · 2 distractions` (with whatever pomodoro count is in play).
3. Scroll past the sessions-history table. Confirm: `Inventory` section visible with two subsections: `Quick logs`, `Distractions`.
4. Each Quick log row shows title + minutes + timestamp. Each Distraction row shows note + timestamp + (if present) parent-session-ref label.
5. Press Edit on a Quick log row. Modal opens with values pre-filled. Change the title. Submit. Confirm: row updates. Reload the page. Confirm: change persists.
6. Press Delete on a Distraction row. Confirm: row disappears. Reload. Confirm: still gone.
7. Navigate the calendar to a previous day. Confirm: Inventory shows that day's entries (filtered by `date`), independently of today.

---

## Exercise 6 — Abort behaviour (FR-017, Story 1 implicitly)

1. Open Idle Focus. Type a title. Press `▶ Play`. Let it run for 60 seconds.
2. Press `✕ Abort` (left slot).
3. Confirm: returns to Idle Focus. Title persists in the pill. Pomodoro counter unchanged. `current_session_elapsed_secs` is 0 (next Play starts fresh).

### Variant 6a — Abort suppresses pending auto-restart

1. In Settings, enable `notifications.auto_start_timer`.
2. Run a Focus session to natural completion. Confirm: auto-restart countdown appears for the next session.
3. Repeat. This time, instead of letting natural completion fire, press `✕ Abort` mid-run.
4. Confirm: no auto-restart countdown appears. The user must press `▶ Play` to start the next session.

---

## VR baseline check

After exercising the above, run the visual regression suite:

```bash
npx playwright test --update-snapshots=missing
```

Expected regenerated baselines (per `plan.md` Visual regression budget):

- `timer-chromium-linux.png` (Idle pill + new left label).
- `daily-chromium-linux.png` (Inventory subsection).
- `statistics-daily-chromium-linux.png` (and weekly/monthly) — only if the scenario has non-zero quick logs/distractions for the seeded day. The canonical baseline keeps the zero-suffix scenario so the visual remains unchanged.

No baselines outside this set should regenerate (SC-008). Any unrelated diff is a code regression.

---

## Clippy + fmt check

```bash
cargo clippy --all-targets -- -D warnings -W clippy::pedantic --manifest-path src-tauri/Cargo.toml
cargo clippy --all-targets -- -D warnings -W clippy::pedantic --manifest-path src/Cargo.toml
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo fmt --check --manifest-path src/Cargo.toml
```

Both must pass with zero new `#[allow]` exceptions (SC-011).
