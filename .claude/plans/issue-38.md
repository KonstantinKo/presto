# Implementation Plan for #38

**Issue:** Visual: mode-aware projections + active-state class bindings never wired
**Type:** bug
**Branch:** agentex/38-mode-aware-active-state

---

Research complete. Drafting plan now.

```md
# Bug: Visual — mode-aware projections + active-state class bindings never wired

## Bug Description

The Leptos rewrite ships the CSS for state-aware styling (`.sidebar.break`, `.theme-option.active`, `.notification-status.status-disabled`, `body.focus`/`body.break`/`body.longBreak` backdrops, `.settings-indicators i.active`) but the components never bind those class names to the underlying state. The stylesheet contract is intact; the JSX-side `class:xxx=` bindings are missing. A secondary regression: sidebar / right-rail icons render ~33% smaller than before because the DOM was swapped from `<svg>` to `<i class="ri-…">` (Remix Icon webfont) while the CSS still targets `svg`.

User-visible symptoms:

- Sidebar stays red (focus palette) forever, even when the main timer flips to green (break) / blue (long-break) — visually jarring split.
- Body backdrop tint per mode (light pink / green / blue) never applies, leaving macOS native window chrome floating against an un-tinted backdrop.
- The mode badge above the timer reads only `"Focus"` / `"Break"` / `"Long Break"`; the prior `" (Paused)"` / `" (Auto-paused)"` / `" (Overtime)"` suffixes are gone.
- Settings → Theme: Auto / Light / Dark buttons and the timer-theme tiles never show the "active" treatment (no opacity bump, no checkmark) — the user has no visual confirmation of which theme is in effect.
- Settings → Notifications: the status panel renders as bare text + a `Test` button with no padding, border-left, or background tint — looks like debug output.
- Right-rail indicators (lightbulb / play-circle / repeat) are pure decoration: clicks do nothing, and they never flip between `-line` and `-fill active`.
- Sidebar nav icons and right-rail icons render ~33% smaller than the captured baselines because `.sidebar-icon svg { width: 24px }` no longer matches the DOM (now `<i>`).

## Problem Statement

Six independent class / attribute projections that the CSS depends on are absent from the Leptos `view!` macros, and one CSS selector group (`.sidebar-icon svg`, `.sidebar-icon-large svg`) targets a DOM node that no longer exists. The fix is mechanical wiring of `class:xxx=` bindings plus a one-line CSS sweep — the underlying state already lives in the right signals (`engine`, `RwSignal<Settings>`).

## Solution Statement

Surgically wire each missing binding without restructuring the views:

1. `<nav class="sidebar focus">` → drive `class:focus` / `class:break` / `class:longBreak` from `engine.current_mode()`.
2. Add a single `Effect` in `app.rs` that mutates `document.body.class_name` to the current mode token so `body.focus { background: var(--focus-bg) }` rules apply.
3. Extend the timer's `mode_text` derived signal to append `" (Paused)"` / `" (Auto-paused)"` / `" (Overtime)"` based on `is_paused()` / `is_auto_paused()` / overtime flag.
4. Wire each Theme button / tile with `class:active=move || current_theme == "<this>"` (theme already lives in `settings.appearance.{theme,timer_theme}`).
5. Add `class:status-ready` / `class:status-disabled` to `#notification-status` and restore the box-shape rules (padding, border-radius, margin, font-size) on `.notification-status`.
6. Wire each right-rail indicator with `on:click` that toggles the corresponding `Settings.notifications` flag, plus a `class=move || …` projection that flips between `ri-X-line` and `ri-X-fill active`.
7. Add `.sidebar-icon i { font-size: 24px }` / `.sidebar-icon-large i { font-size: 26px }` rules (and the same sweep in `responsive.css`) so the icon size matches the captured baselines.

TDD: every new pure projection (mode-label suffix mapper, indicator class projector, status modifier mapper) gets a host-side `cargo test` covering the truth table before the `view!` binding is added.

## Steps to Reproduce

1. `cargo tauri dev`.
2. Open Settings → Theme. Observe: none of the three Auto / Light / Dark buttons appears "selected"; no checkmark on any timer-theme tile.
3. Open Settings → Notifications. Toggle desktop notifications off. Observe: the `#notification-status` panel has no padding / no border-left / no background tint — it's bare text + a `Test` button.
4. Click the Test button — no error, but observe the panel is unstyled.
5. Back to Timer. Open DevTools → Elements → `<nav class="sidebar">`. Manually edit the class attribute to `sidebar break`. Observe: sidebar palette flips to green. Restore. Now `cargo tauri dev` flow — start the focus timer (`-5 -5 -5 -5 -5` to drain it, or set `debug_mode = true` in Settings → Advanced to clamp to 3s). Wait for completion → mode flips to Break. Observe: sidebar stays red while the main `.timer-container` flips to green.
6. Observe the body background: it never picks up `var(--focus-bg)` / `var(--break-bg)` / `var(--long-break-bg)` — body stays at the default backdrop.
7. Observe the `#status-text` badge during pause: it reads `"Focus"` while the timer is paused, not `"Focus (Paused)"`.
8. Click any of the three right-rail icons (`#smart-indicator`, `#auto-start-indicator`, `#continuous-session-indicator`) — nothing happens. Inspect: their class never includes `active`.
9. Compare any sidebar icon against `tests/e2e/__screenshots__/visual-regression/timer-chromium-linux.png` — the rendered icons are ~33% smaller than the baseline glyphs.

## Root Cause Analysis

Cause for each symptom:

| Symptom | Root cause |
|---|---|
| Sidebar stuck red | `src/src/app.rs:640` — `<nav class="sidebar focus">` is hardcoded. The Phase 4d cut explicitly notes "wires only `focus` since the timer engine starts in `Focus` mode … Phase 4e attaches the engine-mode → sidebar-class projection" — Phase 4e never landed. |
| Body backdrop never tints | No effect in `app.rs` mutates `document.body.class_name`. The CSS at `style/layout.css:32-44` matches on `body.focus`/`body.break`/`body.longBreak` but no producer ever sets those classes. |
| Badge missing suffixes | `src/src/components/timer.rs:549` — `mode_text = Signal::derive(move \|\| engine.with(\|s\| mode_label(s.current_mode())))` returns just the mode label. The JS-era surface at `src/managers/navigation-manager.js` appended `" (Paused)"` / `" (Auto-paused)"` / `" (Overtime)"`; that projection was dropped during the port. |
| Theme buttons / tiles never `.active` | `src/src/components/settings/theme.rs:111-138` and `:155-162` — no `class:active=` binding. The shared `RwSignal<Settings>` carries `appearance.theme` and `appearance.timer_theme` (already persisted by the debounced sink), so the projection is one-liner per button / tile. |
| Notification status panel unstyled | `src/src/components/settings/notifications.rs:102-107` renders `<div id="notification-status" class="notification-status">` but never toggles `status-ready`/`status-disabled`. CSS at `style/notifications.css:194-223` defines color rules for those modifiers but the base `.notification-status` ruleset lacks the JS-era inline box-shape rules (padding, border-radius, margin, font-size) — those rules were never ported, so even before the modifier classes the panel renders bare. |
| Right-rail indicators inert | `src/src/components/timer.rs:1113-1136` renders three `<i>` with hardcoded `ri-X-line` classes — no `on:click`, no `class=move \|\| …` projection over the corresponding `Settings.notifications.{smart_pause,auto_start_timer,allow_continuous_sessions}` flag. |
| Icon sizing 33% smaller | DOM swap from `<svg>` to `<i class="ri-…">` left `style/sidebar.css:85-93` (`.sidebar-icon svg { width: 24px }` etc.) targeting nothing. Webfont glyphs inherit the default `font-size` (~15px) and render visibly smaller than the 24px / 26px baseline. Same pattern in `style/responsive.css:118-119` and `:243-244`. The `.settings-indicators i { font-size: 20px }` rule at `style/smart-indicator.css:36-48` is the correct pattern — sidebar CSS needs the same shape. |

## Relevant Files

Use these files to fix the bug:

- **`src/src/app.rs`** — hardcoded `class="sidebar focus"` at line 640; needs `class:focus` / `class:break` / `class:longBreak` bound to `engine.current_mode()`. Also the host for the new `Effect` that mirrors the current mode onto `document.body.class_name` (clearing any prior mode class first so the three classes are mutually exclusive).
- **`src/src/components/timer.rs`** — `mode_text` derived signal at line 549 needs the suffix projection; right-rail indicators at lines 1113–1136 need `class=` + `on:click` wiring against `RwSignal<Settings>`. The view's `app_toast` is already in scope so a "Smart pause enabled / disabled" toast is one line if desired (out of scope — visual fix only).
- **`src/src/components/settings/theme.rs`** — each `<button class="theme-option" …>` (3 buttons) and each `<button class="timer-theme-option" …>` (mapped from `ALL_THEMES`) needs `class:active=move || …` against `settings.appearance.theme` / `settings.appearance.timer_theme`.
- **`src/src/components/settings/notifications.rs`** — `#notification-status` div at line 102 needs `class:status-ready` / `class:status-disabled` bound to `desktop_enabled`.
- **`src/style/notifications.css`** — base `.notification-status` ruleset at line 194 needs `padding: 8px 12px; border-radius: 6px; margin-top: 8px; font-size: 13px;` restored alongside the existing color rules.
- **`src/style/sidebar.css`** — add `.sidebar-icon i { font-size: 24px }` / `.sidebar-icon-large i { font-size: 26px }` (mirror the `font-size: 20px` pattern at `style/smart-indicator.css:37`).
- **`src/style/responsive.css`** — same sweep at lines 118-119 (mobile breakpoint) and 243-244 (smaller-screen breakpoint).
- **`src/src/engine/timer.rs`** — read-only reference. `is_running()`, `is_paused()`, `is_auto_paused()`, and the overtime check (`time_remaining_secs_signed() < 0`) already exist; the suffix projection consumes them.
- **`src/src/bridge/types.rs`** — read-only reference. `Settings.appearance.{theme,timer_theme}` and `Settings.notifications.{smart_pause,auto_start_timer,allow_continuous_sessions}` already exist on the type.
- **`tests/e2e/__screenshots__/visual-regression/timer-chromium-linux.png`** — likely needs re-capture because the body backdrop tint and sidebar icon size land within the captured viewport. Per the constitution's "≤2 baselines per PR without escalation" rule, this is the single allowed update; the bug fix is the explicit one-line PR justification.

### New Files

None.

## Step by Step Tasks

IMPORTANT: Execute every step in order, top to bottom.

### 1. Add pure unit tests for the new mode-label suffix projection (TDD red)

- Add a host-side test module to `src/src/components/timer.rs` (or expose a new pure helper `fn mode_label_with_status(mode: TimerMode, is_running: bool, is_paused: bool, is_auto_paused: bool, is_overtime: bool) -> String`).
- Cases to cover:
  - `(Focus, false, false, false, false)` → `"Focus"` (idle).
  - `(Focus, true, false, false, false)` → `"Focus"` (running, no suffix — matches `_smoke.spec.js` first-paint and `sessions-history.spec.js:28` exact-text contract).
  - `(Focus, false, true, false, false)` → `"Focus (Paused)"`.
  - `(Focus, false, false, true, false)` → `"Focus (Auto-paused)"`.
  - `(Focus, true, false, false, true)` → `"Focus (Overtime)"`.
  - Repeat for `Break` / `LongBreak` (label prefix).
  - Tie-break: when both paused and overtime are true, prefer `(Paused)` — the JS-era surface gated overtime on `is_running` so overtime can only show while running.
- Run `cargo test -p presto-web --lib` — assert the new tests fail (helper doesn't exist).

### 2. Implement the suffix helper and bind it (TDD green)

- Add the `mode_label_with_status` helper alongside the existing `mode_label` in `src/src/components/timer.rs`.
- Update `mode_text` derived signal at line 549:
  ```rust
  let mode_text = Signal::derive(move || {
      engine.with(|s| mode_label_with_status(
          s.current_mode(),
          s.is_running(),
          s.is_paused(),
          s.is_auto_paused(),
          s.time_remaining_secs_signed() < 0,
      ))
  });
  ```
- Run `cargo test -p presto-web --lib` — assert all suffix tests pass.

### 3. Wire the sidebar mode-class projection in `app.rs`

- Replace `<nav class="sidebar focus">` at `src/src/app.rs:640` with:
  ```rust
  <nav
      class="sidebar"
      class:focus=move || engine.with(|s| matches!(s.current_mode(), TimerMode::Focus))
      class:break=move || engine.with(|s| matches!(s.current_mode(), TimerMode::Break))
      class:longBreak=move || engine.with(|s| matches!(s.current_mode(), TimerMode::LongBreak))
  >
  ```
- Import `crate::bridge::timer_mode::TimerMode` at the top of `app.rs`.
- Verify the comment at lines 629–639 still accurately describes the wiring — the "Phase 4e attaches the engine-mode → sidebar-class projection" sentence becomes load-bearing-resolved, so drop the future-tense suffix.

### 4. Wire the body mode-class effect in `app.rs`

- Add (inside the `App` function body, near the other `Effect::new` calls that read `engine`):
  ```rust
  Effect::new(move |_| {
      let mode = engine.with(|s| s.current_mode());
      let token = match mode {
          TimerMode::Focus => "focus",
          TimerMode::Break => "break",
          TimerMode::LongBreak => "longBreak",
      };
      if let Some(body) = web_sys::window()
          .and_then(|w| w.document())
          .and_then(|d| d.body())
      {
          // Replace any prior mode class without clobbering other body
          // classes (none today, but defensive).
          let _ = body.class_list().remove_3("focus", "break", "longBreak");
          let _ = body.class_list().add_1(token);
      }
  });
  ```
- This Effect runs at mount with mode=Focus (sets `body.focus` immediately) and re-fires on every mode transition.

### 5. Add pure unit tests for the right-rail indicator class projection (TDD red)

- Add a `fn indicator_class(base_stem: &str, enabled: bool) -> &'static str` helper or a per-icon enum, expressed as a const map. Truth table:
  - `("lightbulb", true)` → `"ri-lightbulb-fill active"`.
  - `("lightbulb", false)` → `"ri-lightbulb-line"`.
  - `("play-circle", true)` → `"ri-play-circle-fill active"`.
  - `("play-circle", false)` → `"ri-play-circle-line"`.
  - `("repeat", true)` → `"ri-repeat-fill active"`.
  - `("repeat", false)` → `"ri-repeat-line"`.
- Implement with three discrete `match`-guarded `Signal::derive` closures in the view (one per icon) or one generic helper — the choice is local; tests pin the resulting strings.

### 6. Wire the right-rail indicator click handlers + class projections in `timer.rs`

- At `src/src/components/timer.rs:1120-1136`, replace each `<i class="ri-…-line" …>` with a click-capable, class-driven version:
  ```rust
  <i
      id="smart-indicator"
      class=move || if settings.with(|s| s.notifications.smart_pause) {
          "ri-lightbulb-fill active"
      } else {
          "ri-lightbulb-line"
      }
      style="display: block"
      data-tooltip="Smart Pause: Click to toggle automatic pause when inactive"
      on:click=move |_| settings.update(|s| {
          s.notifications.smart_pause = !s.notifications.smart_pause;
      })
  ></i>
  ```
- Repeat for `#auto-start-indicator` (flag: `auto_start_timer`) and `#continuous-session-indicator` (flag: `allow_continuous_sessions`).
- The shared `RwSignal<Settings>` is already in scope as the `settings` local at line 281; mutations propagate through the debounced save sink in `app.rs` (line 322 onwards) so persistence is automatic. The shortcut / activity-monitor / autostart settings-driven side-effect Effects in `app.rs` (lines 381–440) cover the OS-level side effects of these flags — `smart_pause` flips immediately routes through `start/stop_activity_monitoring`.

### 7. Wire the Theme settings active-state bindings in `theme.rs`

- At `src/src/components/settings/theme.rs:99`, derive the current theme preference inside `ThemeSettings`:
  ```rust
  let current_theme = Signal::derive(move || settings.with(|s| s.appearance.theme.clone()));
  let current_timer_theme = Signal::derive(move || settings.with(|s| s.appearance.timer_theme.clone()));
  ```
- For each of the three theme buttons (lines 111–137), add `class:active=move || current_theme.get() == "<this>"` where `<this>` is `"auto"` / `"light"` / `"dark"` respectively.
- For the timer-theme tile mapped at lines 149–164, capture `current_timer_theme` and bind `class:active=move || current_timer_theme.get() == id`. Note `id` is a `&'static str` so the closure must hold an owned `String::from(id)` or rely on `id`'s `'static` lifetime — the closure already captures `id` by copy.

### 8. Wire the Notifications status modifier-class binding in `notifications.rs`

- At `src/src/components/settings/notifications.rs:102`, replace the static `class="notification-status"` with mode-bound bindings:
  ```rust
  <div
      id="notification-status"
      class="notification-status"
      class:status-ready=move || desktop_enabled.get()
      class:status-disabled=move || !desktop_enabled.get()
  >
  ```
- No new derived signal needed — `desktop_enabled` is already in scope at line 38.

### 9. Restore the `.notification-status` box-shape rules in `notifications.css`

- At `src/style/notifications.css:194-199`, extend the base rule:
  ```css
  .notification-status {
      padding: 8px 12px;
      border-radius: 6px;
      margin-top: 8px;
      font-size: 13px;
      border-left: 3px solid #ccc;
      background-color: var(--bg-secondary);
      color: var(--text-secondary);
      transition: all 0.3s ease;
  }
  ```
- All four modifier rules (`status-ready` / `status-warning` / `status-error` / `status-disabled`) inherit the box-shape from the base; only color tokens differ — no other CSS change needed.

### 10. Sweep the `<svg>` → `<i>` icon-sizing CSS

- In `src/style/sidebar.css`, alongside the existing `.sidebar-icon svg { width: 24px; height: 24px; }` and `.sidebar-icon-large svg { width: 26px; height: 26px; }` (lines 85–93), add:
  ```css
  .sidebar-icon i {
      font-size: 24px;
      line-height: 1;
  }

  .sidebar-icon-large i {
      font-size: 26px;
      line-height: 1;
  }
  ```
- Mirror the same addition in `src/style/responsive.css` at the two existing `.sidebar-icon svg` / `.sidebar-icon-large svg` rule groups (around lines 118-119 and 243-244). Keep the `svg` rules in place to avoid scope creep — the DOM no longer carries `<svg>` for these but the rules are inert, not harmful, and removing them is a separate cleanup.

### 11. Run the existing host-side test surface to catch contract drift

- `cargo test -p presto-web --workspace --frozen` — ensures the selector contract pins (`timer.rs::tests::timer_view_selector_contract_documented`, `notifications.rs::tests::notifications_selector_contract_documented`, `theme.rs::tests::theme_settings_selector_contract_documented`) still resolve.

### 12. Run the wasm-bindgen tests

- `(cd src && wasm-pack test --node)` — DOM-bound tests that exercise click handlers and class projections. Confirm no new failures; add a wasm-bindgen-test for the indicator-click + settings-flag round-trip if one isn't already covered (optional — host-side unit tests cover the projection; the wasm test would cover the `view!` wiring end-to-end).

### 13. Run the lint posture

- `cargo clippy --workspace --all-targets -- -D warnings -W clippy::pedantic -W clippy::nursery`.
- Expected friction: the new `class:` bindings produce closures that may trip `clippy::redundant_closure_for_method_calls` or `clippy::needless_pass_by_value` — fix as flagged, do not blanket-allow.

### 14. Run the formatter

- `cargo fmt --all --check`. Fix drift with `cargo fmt --all`.

### 15. Run the Playwright suite

- `(cd tests/e2e && npm ci && npx playwright install chromium && npx playwright test)`.
- Confirm the strict-mode `#status-text` assertions in `sessions-history.spec.js:28` and `settings-automation.spec.js:60` (`toHaveText("Break")`) still resolve — after a focus → break mode transition in non-continuous mode, the engine state is `(is_running=false, is_paused=false, is_auto_paused=false)`, so the suffix helper returns plain `"Break"` (no parentheses). The auto-start path lands at `is_running=true`, also producing plain `"Break"`. If a test does fail here, the suffix mapping needs adjustment — verify the engine's post-completion shape with `cargo test`'s existing assertions first.
- Confirm `settings-notifications.spec.js:26` (`toContainText("Disabled")`) still passes — the modifier class addition is independent of the text projection.
- Confirm `settings-theme.spec.js` still passes — the `class:active` binding does not change the `data-theme` / `data-timer-theme` attribute side effects.

### 16. Run the visual regression suite

- `(cd tests/e2e && npx playwright test visual-regression.spec.js)`.
- Expected diffs:
  - `timer-chromium-linux.png` — sidebar icons render at 24/26px (vs. ~15px previously); body acquires the `focus` backdrop tint. This is the intended visual fix.
  - `settings-notifications-chromium-linux.png` — `#notification-status` gains its box shape; in cold-start `desktop_notifications=true` so the panel renders with the green `status-ready` palette.
  - `settings-theme-chromium-linux.png` — the cold-start `theme=auto`, `timer_theme=espresso` defaults light up Auto button + Espresso tile.
- If three or more baselines change: STOP, escalate per the constitution's "≤2 baselines per PR without escalation". If two or fewer change: regenerate with `npx playwright test visual-regression.spec.js --update-snapshots`, visually review each PNG in an image diff tool, and commit with the one-line PR justification: "Intended re-capture: bug fix #38 (visual: mode-aware projections + active-state bindings)".
- If `timer-chromium-linux.png` is the only one that changes (the settings panels weren't captured in a mode that exercises the new bindings), then only one baseline updates — within budget.

### 17. Smoke test the right-rail indicators in `cargo tauri dev`

- Visit Timer view. Click `#smart-indicator` → CSS should flip the icon to `ri-lightbulb-fill active` and `Settings → Notifications → "Smart Pause"` toggle (if visible there) should be ON. The `activity_monitor` Effect in `app.rs:400-423` should fire `start_activity_monitoring`.
- Click `#auto-start-indicator` → flips to `ri-play-circle-fill active`; toggling off and back on confirms the round-trip.
- Click `#continuous-session-indicator` → flips to `ri-repeat-fill active`.
- Restart the app (`cargo tauri dev` exit + re-launch) and confirm the indicator state persisted (debounced save sink at `app.rs:322` writes after 300ms).

## Validation Commands

Execute every command to validate the bug is fixed with zero regressions.

```bash
# Host-side unit + integration tests (covers the new pure projections).
cargo test --workspace --frozen

# WASM-side DOM-bound tests.
(cd src && wasm-pack test --node)

# Lint posture (strict-deny pedantic + nursery).
cargo clippy --workspace --all-targets -- -D warnings -W clippy::pedantic -W clippy::nursery

# Formatting drift.
cargo fmt --all --check

# E2E behavioural suite (17 specs).
(cd tests/e2e && npm ci && npx playwright install chromium && npx playwright test --reporter=list)

# Visual regression suite (14 baselines, ≤2% pixel-ratio tolerance).
(cd tests/e2e && npx playwright test visual-regression.spec.js)

# Mock-drift gate (Tauri handlers ↔ tauriMock.js).
bash scripts/check-mock-drift.sh

# Lockfile drift gate (manifest ↔ lock pairs).
bash scripts/check-lockfile-drift.sh
```

## Notes

- The right-rail indicator click handlers mutate `Settings.notifications.{smart_pause,auto_start_timer,allow_continuous_sessions}` via the shared `RwSignal<Settings>`. The persistence sink in `app.rs:322-360` debounces writes by 300ms and lands them on disk via `commands::save_settings`. The OS-level side-effect Effects in `app.rs:381-440` (shortcuts, smart-pause activity monitoring, autostart) ride the same signal — toggling `smart_pause` from the indicator immediately triggers `start_activity_monitoring` / `stop_activity_monitoring` without waiting for the 300ms debounce. No new bridge calls are needed.
- The suffix helper's `(Overtime)` branch is gated on `time_remaining_secs_signed() < 0`. This is true after `OvertimeStarted` fires in `allow_continuous_sessions` mode and stays true until the user manually resets or skips. The engine's existing tests at `engine/timer.rs::tests` cover the state machine; the new tests only cover the projection.
- Why the body class is mutated imperatively (effect + DOM call) rather than via a Leptos `class:focus` binding on `<body>`: Leptos owns the `<App>` component's root, not `<body>` — `index.html`'s static `<body>` lives outside the Leptos render tree. The `class_list().add_1` / `remove_3` pattern is the only correct way to project state onto an element Leptos doesn't own.
- The `class_list().remove_3("focus", "break", "longBreak")` call is safe to invoke unconditionally — `DOMTokenList.remove` is a no-op on absent tokens.
- The icon-sizing CSS sweep deliberately keeps the `<svg>`-targeting rules in place. They're inert (no `<svg>` children exist) but removing them is unrelated to the bug and risks accidentally breaking a future component that does use SVG glyphs. Tagging them with a TODO is acceptable; deleting is out of scope.
- Visual regression baseline cap: the constitution allows ≤2 re-captures per PR without escalation. The expected diffs are timer + settings-notifications + settings-theme = 3 baselines. If all three trip the 2% tolerance, escalation is needed. Mitigation: the `settings-theme` and `settings-notifications` baselines may be within tolerance for the Espresso-default + cold-start `desktop_notifications=true` paint, because the active-state and status-ready CSS rules use semi-transparent colors that diff softly. Run the suite once to measure the actual diff ratios before deciding on re-capture vs. escalation. If the timer-only baseline changes, no escalation is needed.
- The badge-suffix change has a strict-mode e2e interaction risk: `sessions-history.spec.js:28` and `settings-automation.spec.js:60` use `toHaveText("Break")` (exact match). The new helper must return plain `"Break"` (no suffix) during both running and post-completion idle states. The unit-test truth table above pins this.
- TDD: the four new projections (mode-label suffix mapper, indicator class projector, current-theme matcher, current-timer-theme matcher) are pure functions and should land with host-side unit tests *first* (failing), then implementation, then `view!` wiring. The `view!` macro itself is not unit-testable without `wasm-bindgen-test`; integration coverage rides on the e2e suite.
```

---
*Generated by Agentex*
