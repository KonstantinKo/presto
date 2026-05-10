# Implementation Plan for #36

**Issue:** Settings: complete the data-model and the read/write wires across the app
**Type:** bug
**Branch:** agentex/36-settings-data-model-wires

---

I have enough context. Here is the plan.

# Bug: Settings — data-model and read/write wires incomplete across the app

## Bug Description

Multiple post-migration settings inputs do not persist or do not affect runtime behaviour because the `Settings` / `AppSettings` records lack fields, UI handlers don't write the signal, or no consumer reads the field after it is set. Observable symptoms:

- Theme picker (Dark/Light/Auto) — choice is lost on relaunch; app no longer follows OS dark-mode preference on first launch.
- Timer-theme tile picker — choice is lost on relaunch.
- General → "Max Session Time" input — hardcoded `value="120"`; typed values discarded; auto-pause cap not enforceable.
- Automation → "Allow Continuous Sessions" toggle — flips the signal but the data-shape ownership for this bug stops at the field; engine consumption is explicitly out of scope.
- Update banner → "Skip release" — only dismisses for this session; skipped version reappears on next launch.
- Auth modal → "Continue as Guest" — sets the in-memory `AuthState::Guest` directly without persisting `guest_mode = true`; next launch back to Unauthenticated.
- Calendar → "Focus Weekly Summary" tiles — hardcoded `"0m"`/`"0"`; never reflect completed sessions.
- Timer → Progress dots — `total_sessions.max(11)` clamps low values up to 11 instead of using the raw setting (with a 1 floor); doc-comment claims prior default was 11 — actual JS-era default was 10.

Expected: every Settings input round-trips to disk through the existing debounced `save_settings` sink in `src/src/app.rs`, every downstream reader projects off the shared `RwSignal<Settings>`, and cold-start hydration applies the persisted choice.

## Problem Statement

The `Settings` (Leptos) and `AppSettings` (Tauri) records do not carry the necessary fields (`appearance.theme`, `appearance.timer_theme`, `timer.max_session_time`) and the components that do have backing fields (`skipped_versions`, `guest_mode`) bypass them — they mutate local state instead of the shared signal, so the existing debounced persistence sink never sees the change. Two derived projections (Focus Weekly Summary tiles and the progress-dot count) are wrong even though the source signal exists: the tiles render static literals and the dot count clamps with `.max(11)`.

## Solution Statement

Surgically widen the `Settings` / `AppSettings` records in lockstep, wire every offending `on:change` to the shared `RwSignal<Settings>`, fix the two derived projections to read off the signals they already have access to, add the cold-start `apply_theme` effect in `app.rs`, and fold the legacy `theme_preference` / `timer_theme_preference` migration payload fields into the new `appearance` block instead of dropping them. Engine-side enforcement of `max_session_time` and `allow_continuous_sessions` is explicitly deferred — this PR settles the data shape and the read/write wires only.

## Steps to Reproduce

1. `cargo tauri dev`.
2. Open Settings → Theme; choose **Dark**. Close + reopen — theme reverts to default.
3. With OS dark-mode enabled, first-launch shows light theme (system preference ignored).
4. Settings → General; change Max Session Time to e.g. 60; close + reopen — value reverts to 120 (or: the typed value never lands on disk in the first place).
5. Settings → Automation; tick "Allow Continuous Sessions". Run a focus session past zero-cross — engine hard-cuts to break (this PR does NOT fix engine enforcement; the bug here is that the field today lives in `notifications`, not `appearance/general`, and is otherwise plumbed; out of scope).
6. Trigger an update banner; click "Skip release" — banner closes, but on next launch the same banner returns.
7. Auth overlay → "Continue as Guest" — overlay dismisses; next launch shows auth overlay again.
8. Complete a focus session; switch to Calendar — Focus Weekly Summary tiles still show `0m / 0m / 0 / 0m`.
9. Settings → General → set Daily Sessions = 5; switch to Timer — progress dot row still shows 11 dots.

## Root Cause Analysis

### A — Missing fields on `Settings` / `AppSettings`

- `src/src/bridge/types.rs` `Settings` has no `appearance` block (no `theme` / `timer_theme` slot). `src-tauri/src/lib.rs` `AppSettings` mirrors the same gap.
- `TimerSettings` has no `max_session_time` field.

Consequence: even if a UI handler wrote to the signal, there is no field to write to and no field to round-trip through the existing debounced `commands::save_settings` sink in `src/src/app.rs:256-299`.

### B — Handlers that don't mutate the shared signal

- `src/src/components/settings/theme.rs:81-92` (`on_theme`, `on_timer_theme`) only sets the `<html data-theme>` / `<html data-timer-theme>` attributes via `set_html_attr`. The signal is never touched.
- `src/src/components/settings/general.rs:160-166` renders `#max-session-time` with literal `value="120"` and no `on:change`.
- `src/src/components/auth_modal.rs:316-326` `#continue-guest` calls `auth_state.set(AuthState::Guest)` directly. It does not flip `settings.guest_mode = true`, so the debounced sink never persists the choice. The `WebGuestModeStore::set_guest` localStorage fallback is also never called.
- `src/src/components/update_notification.rs:74-76,107-117` "Skip release" / "Update via Homebrew" / close button all just call `dismissed.set(true)` against a local signal. No mutation of `settings.skipped_versions`.

### C — Listeners / projections that don't read the new fields

- `src/src/app.rs:489-506` the `UPDATE_AVAILABLE` listener calls `update_mgr.handle_event(payload)` unconditionally; it does not filter `payload.version` against `settings.skipped_versions`.
- `src/src/app.rs` has no cold-start effect that calls `apply_theme(resolve_color_mode(...))` after settings load resolves.
- `src/src/components/calendar.rs:307-343` hardcodes `"0m"` / `"0"` for the four Focus Weekly Summary tiles instead of deriving against `use_context::<RwSignal<Vec<ManualSession>>>()`.
- `src/src/components/timer.rs:706` `settings.with(|s| s.timer.total_sessions.max(11))` clamps the dot count up to 11 unconditionally. Doc-comment at lines 696-703 also misclaims the JS-era default was 11; `TimerSettings::default()` at `src/src/bridge/types.rs:155` is 10.

### D — Migration import handler drops `theme_preference`

- `src-tauri/src/migration.rs:147-163` `import_settings` deliberately drops `theme_preference` and `timer_theme_preference` from the wire payload because `AppSettings` has nowhere to put them today. Once the `appearance` block lands, the import handler can fold the legacy values into it.

## Relevant Files

Use these files to fix the bug:

- **`src/src/bridge/types.rs`** — Leptos-side `Settings`, `TimerSettings`, plus the `SettingsOnDisk` migration shim. Add `AppearanceSettings { theme, timer_theme }`, add `TimerSettings::max_session_time`, mirror through `SettingsOnDisk` + `From<SettingsOnDisk> for Settings`.
- **`src-tauri/src/lib.rs`** — Tauri-side `AppSettings`, `TimerSettings`, plus the `AppSettingsOnDisk` migration shim. Must move in byte-stable lockstep with the Leptos record (FR-005 / FR-008). `Default` impl + `default_*` helpers.
- **`src/src/components/settings/theme.rs`** — `on_theme` / `on_timer_theme` must mutate `s.appearance.theme` / `s.appearance.timer_theme` in addition to the `set_html_attr` write. `prop:value` on the buttons should reflect the active selection (optional but consistent with general.rs).
- **`src/src/components/settings/general.rs`** — `#max-session-time` input: bind `prop:value` to `s.timer.max_session_time`, add `on:change` parsing via `parse_minutes` with a sane default (120 min).
- **`src/src/components/auth_modal.rs`** — `#continue-guest` click handler: in addition to the in-memory `auth_state.set(AuthState::Guest)`, mutate `settings.guest_mode = true` via the existing `use_context::<RwSignal<Settings>>()` (already present at line 113). This routes through the debounced persistence sink already wired in `app.rs`.
- **`src/src/components/update_notification.rs`** — Add `settings: RwSignal<Settings>` prop (App router already owns it). "Skip release" handler pushes the current update version onto `settings.skipped_versions` (dedupe via `.contains` check) in addition to dismissing.
- **`src/src/app.rs`** — Three additions:
  1. Cold-start effect: after `commands::load_settings()` resolves and lifts the signal, call `theme::loader::apply_theme(resolve_color_mode(settings.appearance.theme, system_prefers_dark()))` so the OS preference is honoured on first launch and the persisted choice is restored on subsequent launches.
  2. `UPDATE_AVAILABLE` listener: filter `payload.version` against `settings.skipped_versions` before passing to `update_mgr.handle_event`.
  3. Pass `settings=settings` prop to `<UpdateNotification/>`.
- **`src/src/components/calendar.rs`** — Replace the four static metric values (`"0m"` × 3, `"0"` × 1) with `Signal::derive` projections off `use_context::<RwSignal<Vec<ManualSession>>>()`. Each tile reads a different aggregate: weekly focus minutes (sum of focus-type rows in the current week), average focus minutes/day, sessions this week, weekly total minutes (all session types).
- **`src/src/components/timer.rs:706`** — Replace `total_sessions.max(11)` with `total_sessions.max(1)` (raw setting value, 1-floor to avoid a zero-dot row). Update the adjacent doc-comment to reflect the JS-era default was 10, not 11.
- **`src-tauri/src/migration.rs`** — `import_settings` (line 147): fold the legacy `theme_preference` (auto / dark / light) and `timer_theme_preference` (espresso / pipboy / …) values into the merged `AppSettings.appearance` before writing.
- **`src/src/managers/auth.rs`** — `WebGuestModeStore` already exists; the auth modal can keep using it for the localStorage fallback. No structural change.
- **`src/src/components/settings/notifications.rs`** — Add `#allow-continuous-sessions` toggle wire-up here if reachable from the Notifications tab per the issue brief. (Issue mentions "Notifications settings"; today the toggle is in `automation.rs`. The current location already mutates `s.notifications.allow_continuous_sessions`; no UI move is required for the data-shape fix. Engine consumption is deferred.)

### New Files

None. Every change is a field addition or an `on:change` / `Effect` attachment in existing modules.

## Step by Step Tasks

IMPORTANT: Execute every step in order, top to bottom. Observe TDD — every data-model widening lands with a serde round-trip test in the same edit; every handler/effect change lands with a unit test where the test target is non-DOM (parse helpers, projections); UI plumbing is exercised via the existing e2e suite as a downstream gate.

### 1 — Widen `AppSettings` (Tauri side) with `appearance` and `timer.max_session_time`

In `src-tauri/src/lib.rs`:

- Add `struct AppearanceSettings { theme: String, timer_theme: String }` with `Default` returning `theme: "auto", timer_theme: "espresso"` (matching the JS-era cold-start at `src/managers/theme-manager.js`). Carry `#[serde(default)]` on each inner field.
- Add `appearance: AppearanceSettings` (carry `#[serde(default)]`) to `AppSettings` and to `AppSettingsOnDisk`.
- Add `max_session_time: u32` to `TimerSettings` with `#[serde(default = "default_max_session_time")]`; helper returns 120.
- Update the `Default` impl on `AppSettings` and the `From<AppSettingsOnDisk> for AppSettings` impl.
- Write a `#[test]` that asserts:
  - `AppSettings::default()` serialises to JSON containing `"appearance":{"theme":"auto","timer_theme":"espresso"}` and `"max_session_time":120`.
  - A legacy 0.4.x JSON without `appearance` or `max_session_time` deserialises to the defaults.
  - A round-trip of the default `AppSettings` yields no field drift.

### 2 — Widen Leptos `Settings` in lockstep

In `src/src/bridge/types.rs`:

- Define `AppearanceSettings` matching the Tauri-side shape byte-for-byte (snake_case via serde default).
- Add `appearance: AppearanceSettings` (`#[serde(default)]`) to `Settings` and `SettingsOnDisk`; update `Default` and `From<SettingsOnDisk> for Settings`.
- Add `max_session_time: u32` with `#[serde(default = "default_max_session_time")]` to `TimerSettings`; helper returns 120. Update the `Default` impl.
- Pin the wire shape via the existing `settings_round_trips_default_shape` test (extend asserts to cover `appearance`, `max_session_time`).
- Add a test confirming pre-existing 0.4.x JSONs (the `settings_deserialises_from_minimal_legacy_json` fixture) still round-trip after the widening — the serde defaults must fire.

### 3 — Settings manager round-trip / writeback flag still idempotent

In `src/src/managers/settings.rs`:

- Update the existing `idempotent_missing_field_migration_writes_back` test fixture to include `appearance` and `max_session_time` in the canonical fixture so the round-trip stays clean.
- Verify `save_writes_full_shape_drops_legacy_field` still passes (no new legacy fields emitted).

### 4 — Wire Theme settings handlers to the shared signal

In `src/src/components/settings/theme.rs`:

- Accept `settings: RwSignal<Settings>` and `toast: SettingsToast` as props (matching the other tabs). Update `mod.rs` to pass them.
- `on_theme(pref)` (where `pref` is "auto" | "light" | "dark"): in addition to the existing `set_html_attr` write, `settings.update(|s| s.appearance.theme = pref.to_string())` and fire `toast.show("Settings saved")`.
- `on_timer_theme(id)`: in addition to the existing `apply_theme` + `set_html_attr` writes, `settings.update(|s| s.appearance.timer_theme = id.to_string())` and fire the toast.
- Add a unit test for a tiny pure helper (e.g. `normalise_theme_pref` that maps unknown input → "auto") so the change is TDD-anchored.

### 5 — Wire `#max-session-time` to the signal

In `src/src/components/settings/general.rs`:

- Derive a `max_session_time` signal off `settings.with(|s| s.timer.max_session_time.to_string())`.
- Replace the static `value="120"` with `prop:value=move || max_session_time.get()`.
- Add `on:change=on_max_session_change` invoking `parse_minutes(&event_target_value(&ev), 120)`; `settings.update(...)` + toast.
- Extend `parse_minutes_falls_back_on_empty_or_garbage` test to cover the new fallback default.

### 6 — Fix the progress-dot clamp and the doc-comment in TimerView

In `src/src/components/timer.rs`:

- Line 706: change `s.timer.total_sessions.max(11)` to `s.timer.total_sessions.max(1)`.
- Lines 696-703 comment: correct the "JS-era default was 11" claim to "10"; trim the no-longer-load-bearing pixel-baseline reference if it now conflicts with the corrected behaviour.
- Add a unit test for a tiny pure projection helper (`fn dot_count(total: u32) -> u32 { total.max(1) }`) so the regression is caught at host-test time.

### 7 — Auth modal: persist guest mode through the shared signal

In `src/src/components/auth_modal.rs`:

- The component already reads `let settings = use_context::<RwSignal<Settings>>()` at line 113. Reuse it.
- Update `#continue-guest`'s `on:click` handler at lines 318-322:
  ```
  settings.update(|s| s.guest_mode = true);
  auth_state.set(AuthState::Guest);
  overlay_open.set(false);
  ```
- The existing debounced persistence sink in `app.rs` carries `guest_mode` to disk via `save_settings`; the cold-start projection in `app.rs:213-219` already lifts `auth_state` into `Guest` when `loaded.guest_mode` is true on next launch.
- Pin via a host-side test on a small projection helper if one falls naturally out; otherwise rely on the cold-start projection test path in `managers::auth::tests::initial_state_guest_when_localstorage_flag_set` (the in-memory store covers the equivalent).

### 8 — Update notification: persist skipped versions and filter the listener

In `src/src/components/update_notification.rs`:

- Add `settings: RwSignal<Settings>` prop. Update App router call site.
- "Skip release" handler (the second button's `on:click`): in addition to `dismissed.set(true)`, read the current available version off `update_info` and push it onto `settings.skipped_versions` only if not already present (`if !s.skipped_versions.contains(&v) { s.skipped_versions.push(v); }`).
- Add a unit test for the dedupe helper:
  ```
  fn push_skipped(list: &mut Vec<String>, version: &str) { … }
  ```
  with cases for empty list, dupe rejection, distinct versions appended.

In `src/src/app.rs:489-506`:

- Before `update_mgr.handle_event(payload)`, read `settings.with_untracked(|s| s.skipped_versions.contains(&payload.version))`; skip the handle call when true.
- Pass `settings=settings` into `<UpdateNotification/>`.

### 9 — Cold-start theme effect in `app.rs`

In `src/src/app.rs`, alongside the existing `commands::load_settings()` resolution path (around lines 202-221):

- After `settings.set(loaded)`, call:
  ```
  let resolved = theme::loader::resolve_color_mode(
      &loaded.appearance.theme,
      theme::loader::system_prefers_dark(),
  );
  theme::loader::apply_theme(resolved);
  // also restore the timer theme:
  theme::loader::apply_theme(&loaded.appearance.timer_theme);
  ```
  (Note: `apply_theme` writes `data-theme`. The timer-theme attribute is `data-timer-theme`; reuse the same `set_html_attr` pattern from `theme.rs`. If two separate writes don't fit, lift `set_html_attr` to a shared spot — minimal scope: a small helper in `theme::loader`.)
- The host-side `theme::loader::apply_theme` is already a no-op stub; the wasm-side body honours the call.

### 10 — Calendar Focus Weekly Summary projections

In `src/src/components/calendar.rs`:

- Compute the current ISO-week (Mon-Sun) bounds off `cursor` (existing signal) via `start_of_week_monday`.
- Use `use_context::<RwSignal<Vec<ManualSession>>>()` (already destructured at line 226) to derive four metrics:
  - `weekly_focus_minutes` = Σ `row.duration` where `row.date` falls in [Mon, Sun] and `row.session_type == Focus`.
  - `avg_focus_per_day_minutes` = `weekly_focus_minutes / 7` (or count distinct days; 7 matches JS-era).
  - `weekly_sessions_count` = count of in-week focus rows.
  - `weekly_total_minutes` = Σ `row.duration` in-week (all session types).
- Render `format!("{}m", n)` and `format!("{}", n)` in the existing four `<div class="metric-value">` slots, replacing the static literals at lines 315, 323, 331, 339.
- Add unit tests for the pure aggregation helpers (single function per metric, taking `&[ManualSession]` + the week bounds → integer). Cases: empty list, sessions in week, sessions outside week, mixed session types. Use `format_session_date` for date matching to stay aligned with the existing engine pin.

### 11 — Fold legacy `theme_preference` into the migration import handler

In `src-tauri/src/migration.rs:147-163`:

- After `helpers::write_settings_to(app_data_dir, legacy_settings)?`, but only when `payload.theme_preference` or `payload.timer_theme_preference` is `Some`, read settings back, fold the values into `appearance.theme` / `appearance.timer_theme`, write back.
- Or (cleaner): merge into `legacy_settings.appearance` BEFORE writing. Either is acceptable.
- Update the `import_settings_folds_theme_preference` test (add a new test if one doesn't exist). Cases:
  - payload carries `theme_preference: Some("dark")` → after import, persisted settings has `appearance.theme == "dark"`.
  - payload carries no theme prefs → defaults applied.
  - payload has both settings AND theme_preference → theme_preference wins (it was a separate localStorage key in JS-era).

### 12 — End-to-end gate

- Run `cargo test --workspace --frozen` — must pass.
- Run `(cd src && wasm-pack test --node)` — must pass.
- Run `(cd tests/e2e && npx playwright test settings-theme.spec.js settings-general.spec.js auth.spec.js update-notification.spec.js calendar-navigation.spec.js)` — must pass.
- `cargo clippy --workspace --all-targets -- -D warnings -W clippy::pedantic` — no new warnings.

## Validation Commands

Execute every command to validate the bug is fixed with zero regressions.

```bash
# 1. Workspace builds clean.
cargo build --workspace --frozen

# 2. Strict lint posture (per constitution).
cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic

# 3. Formatting.
cargo fmt --all --check

# 4. Host-side tests (engine, managers, bridge round-trips, Tauri-side).
cargo test --workspace --frozen

# 5. Wasm-side tests (DOM-bound logic on the Leptos crate).
(cd src && wasm-pack test --node)

# 6. Focused e2e specs for the surfaces touched.
(cd tests/e2e && npx playwright test \
    settings-theme.spec.js \
    settings-general.spec.js \
    settings-automation.spec.js \
    settings-advanced.spec.js \
    auth.spec.js \
    update-notification.spec.js \
    calendar-navigation.spec.js \
    sessions-history.spec.js \
    timer.spec.js \
    _smoke.spec.js)

# 7. Visual-regression — 14 baselines, ≤2% pixel-ratio drift.
(cd tests/e2e && npx playwright test visual-regression.spec.js)

# 8. CI gate scripts.
bash scripts/check-mock-drift.sh
bash scripts/check-baseline-cap.sh
bash scripts/check-engine-purity.sh
bash scripts/check-lockfile-drift.sh

# 9. Manual smoke (cargo tauri dev):
#    - Pick Dark theme → restart → still Dark.
#    - With OS dark mode on, first-launch theme = dark.
#    - General → Max Session Time = 60 → restart → still 60.
#    - Set Daily Sessions = 5 → Timer view shows exactly 5 dots.
#    - Continue as Guest → restart → no auth overlay.
#    - Skip release on update banner → restart → banner stays hidden for that version.
#    - Complete a focus session → Calendar tiles show non-zero values.
```

## Notes

- **Lockstep migration discipline.** The Leptos-side `Settings` and the Tauri-side `AppSettings` are mirror types per FR-005 / FR-008. Every field addition lands in both files in the same commit; the bridge's serde-wasm-bindgen round-trip pins the wire shape via existing tests in `bridge::types::tests` and the Tauri-side test module.
- **Engine consumption explicitly deferred.** The brief says: "The engine-side enforcement of `max_session_time` and `allow_continuous_sessions` is its own work — settle the data shape and reads here first." Do NOT modify `src/src/engine/timer.rs` to act on either field in this PR; the next issue picks that up. The `Allow Continuous Sessions` toggle UI is already wired to the signal in `src/src/components/settings/automation.rs:75-80`; no UI move from Automation → Notifications is required for the data-shape fix.
- **`#[serde(default)]` on every new field.** Critical for FR-005 round-trip: existing on-disk JSONs predating this change must continue to deserialise into the cold-start shape without manual migration.
- **No `--no-verify`.** Lockfile-drift, mock-drift, baseline-cap, engine-purity gates all run pre-commit; if they fail, fix the root cause.
- **Visual-regression baselines.** The progress-dot change (`max(11)` → `max(1)`) may shift the baseline if the default `total_sessions = 10` is what's captured — the dot row goes from 11 to 10 dots. Per AGENTS.md, baseline updates need an explicit one-line PR note and stay within the ≤2 re-captures gate. If the baseline shows 11 dots today and the fix yields 10, that's exactly one baseline update and the count is in-budget.
- **`apply_theme` for timer-theme.** `theme::loader::apply_theme` writes `data-theme`. The timer-theme attribute is `data-timer-theme` and is set by a separate `set_html_attr` call site in `theme.rs`. Cold-start restoration may need a tiny `apply_timer_theme(id)` sibling in `theme::loader` or an inlined `set_html_attr` call in `app.rs`. Keep it minimal — one helper in `theme::loader`.
- **Migration tests.** `import_settings` already has tests at `src-tauri/src/migration.rs:347+`; extend rather than rewrite the suite. The new test should pin both folds (settings present + theme_preference present, and the no-settings + theme_preference-only branch).

---
*Generated by Agentex*
