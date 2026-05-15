# Quickstart — Overtime Button Treatment

> Phase 1. How a developer exercises the feature locally end-to-end.

## Prerequisites

- Workspace bootstrapped (`cargo build --frozen` for backend, Trunk dev server for frontend).
- Feature 006 has merged or is co-resident on the branch (depends on the `engine.complete(clock)` branch-B.2 path at `src/src/engine/timer.rs:998-1040` and the click-dispatch matrix at `src/src/components/timer/mod.rs:2267-2329`).
- Continuous mode is enabled in settings (`continuous_sessions: true`) — without it the engine will not allow the timer to cross zero.

## Manual exercise: see the overtime treatment

1. **Start the app**: `cargo tauri dev` (or `trunk serve` + Tauri shim per local convention).
2. **Enable continuous mode**: Settings > Automation > "Continuous sessions". Save.
3. **Reduce focus duration for impatience**: Settings > General > Focus duration → 1 minute. Save. (The default 25-minute focus would mean a 25-minute wait per test loop.)
4. **Bind the Abort shortcut** (optional but exercises FR-018 → FR-020):
   - Settings > Shortcuts > new "Abort Session" row at the bottom.
   - Click the input, press `Cmd+Alt+W` (or any unused combo). The input populates and the binding persists.
5. **Start a focus session**: click the center `▶ Play` button.
6. **Wait for zero-cross**: 1 minute. The bell sounds, the countdown turns orange, and within the same UI tick:
   - The three control buttons all show `✓ Complete` with orange tint.
   - The center button is filled, the outer two are ghost.
   - A small "Wrap it up!" line appears between the countdown and the buttons.
7. **Test the triple-Complete**: click any of the three buttons. The session ends, the focus time (1 minute + overtime portion) is logged, and the timer advances to the next mode (Break or LongBreak per the configured cadence).

## Manual exercise: pause-during-overtime reverts

1. Repeat steps 1-6 above. You are in overtime.
2. **Pause via global shortcut**: if you bound `Cmd+Alt+Space` to start-stop, press it. (Or click around — smart-pause may auto-pause after the inactivity threshold.)
3. Observe:
   - The button matrix flips back to `✕ Abort | ▶ Resume | ✓ Complete` (the normal Paused trio).
   - The "Wrap it up!" CTA disappears.
   - The button orange tint clears.
   - The countdown's orange tint **remains** (engine still in overtime; only the matrix and CTA are gated on Running).
4. **Resume**: press the start-stop shortcut again, or click the center `▶ Resume` button.
5. Observe: the overtime treatment returns (three orange Complete buttons + CTA).

## Manual exercise: keyboard discard during overtime

1. **Prerequisite**: Abort shortcut bound (step 4 of the first exercise).
2. **Enter overtime**: steps 1-6 of the first exercise.
3. **Press the Abort shortcut** (`Cmd+Alt+W`).
4. Observe:
   - The session is discarded — focus tally does NOT increment.
   - The timer returns to idle in the current focus mode (NOT the next mode — abort does not advance).
   - The overtime treatment is gone; the CTA is gone.

## Verify a11y

With the app in overtime and a screen reader active (VoiceOver on macOS, NVDA on Windows, Orca on Linux):

- **Tab navigation**: pressing Tab on the timer view skips the outer two Complete buttons and lands only on the center filled button.
- **Screen reader announcement**: only the center button is announced ("Complete the current session and advance" — the `timer.ctrl_complete_aria` string in the active locale).
- **Mode pill**: the `(Overtime)` suffix on the mode pill (or its localised equivalent) is announced via the existing `timer.status_overtime` catalogue key.

Selector-based verification (without a screen reader):

```bash
# In the browser devtools / Playwright inspector, with overtime active:
$('#stop-btn').getAttribute('aria-hidden')   // → "true"
$('#stop-btn').getAttribute('tabindex')      // → "-1"
$('#skip-btn').getAttribute('aria-hidden')   // → "true"
$('#skip-btn').getAttribute('tabindex')      // → "-1"
$('#play-pause-btn').getAttribute('aria-hidden')  // → null (or absent)
$('#play-pause-btn').getAttribute('tabindex')     // → "0"
$('#play-pause-btn').getAttribute('aria-label')   // → matches catalogue value
```

## Run the test suite

```bash
# Frontend Rust tests (incl. IPC round-trip + RunState + overtime predicate):
cargo test -p presto-web --target wasm32-unknown-unknown

# Backend Rust tests (incl. settings round-trip):
cargo test -p presto_lib --lib

# IPC crate tests (incl. ShortcutSettings.abort round-trip):
cargo test -p presto-ipc

# Playwright e2e + VR:
npx playwright test

# VR-only:
npx playwright test --grep visual-regression
```

If the VR baseline `timer-focus-continuous-overtime-chromium-linux.png` does not exist yet (first-time run), regenerate:

```bash
npx playwright test --update-snapshots --grep "overtime"
```

Add the one-line PR note documenting the new baseline (see plan.md `Visual regression budget`).

## Verify the i18n hygiene fix

The `mode_label_with_status` `#[cfg(test)]` helper at `src/src/components/timer/mod.rs:154` no longer carries the English literal `"(Overtime)"` — it dispatches to the catalogue. The associated test `mode_label_with_status_overtime_suffix_requires_running` at line 2930 still passes, asserting the suffix appears only when `is_running && is_overtime`. Run:

```bash
cargo test -p presto-web --target wasm32-unknown-unknown mode_label_with_status
```

All `mode_label_with_status_*` tests pass.

## What to look for in the design-QA loop

Before opening the PR, capture screenshots in light + dark mode of the overtime state:

1. Continuous-mode focus past zero, light theme — center button saturated orange, outer two ghost orange, CTA visible.
2. Same, dark theme — colors flip to dark-theme warning palette (`#f59e0b`).
3. Paused-during-overtime, light + dark — matrix reverts to feature-006 Paused trio; countdown stays orange; CTA hidden.

If any of these diverges from the spec's intent (e.g., the ghost slots are too faint, the CTA placement reads as a button instead of a hint, the dark-mode tint disagrees with the countdown's), iterate with the styling sub-agent before opening the PR.
