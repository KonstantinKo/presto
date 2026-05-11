# Implementation Plan for #39

**Issue:** Restore secondary UI affordances, semantic HTML attributes, and the missing display font
**Type:** bug
**Branch:** agentex/39-restore-ui-html-font

---

Investigation done. Outputting plan.

# Bug: Restore secondary UI affordances, semantic HTML, and missing display font (Issue #39)

## Bug Description
The Leptos migration ported the canonical surface for every screen but dropped a long tail of secondary affordances, accessibility metadata, and one external font resource. None of the omissions are catastrophic individually, but together they make the app feel like a draft: missing buttons (Reset to Defaults, Excel export, per-shortcut Clear, edit-session Save/Delete, undo-session-on-break), missing visual cards (Tag Usage pie chart, chart legend), missing nav semantics (`disabled` on Team, `aria-current` on active sidebar), missing tooltips on hover, an incomplete icon picker (5 emojis instead of 3 remixicons + 5 emojis), and the absence of the Roboto Flex display font that timer.css / settings.css still reference.

**Expected:** Every secondary affordance described in `git show a0bb52c:src/index.html` is reachable; sighted hover users see tooltips; screen readers see `aria-current`; Roboto Flex renders the timer digits and mode badge.

**Actual:** Many affordances unreachable; tooltips disappeared; Team route is reachable when it shouldn't be; timer digits and mode badge fall through to Helvetica/Arial.

## Problem Statement
A single mass-restore PR is needed to bring back the long tail of UI elements without reintroducing the pre-migration tooltip-out-of-bounds bug, without breaking existing e2e/visual baselines beyond the constitution's 2-baseline-per-PR tolerance, and without leaving back-end-wired-but-UI-unreachable commands (Excel export). New cards that depend on data signals that don't yet exist must be marked with explicit `TODO` comments at the binding site.

## Solution Statement
Markup-driven restore against `git show a0bb52c:src/index.html` as the reference. Each surface is restored in its owning Leptos component:

- **Restore native `title=` tooltips** (no custom CSS bubble — sidesteps the prior out-of-bounds rendering bug entirely).
- **Restore the missing buttons** with click handlers wired to existing functions (`Settings::default`, `commands::export_sessions_xlsx`, in-place signal mutations).
- **Restore the missing cards** with TODO comments at data-binding sites where signals don't exist yet.
- **Re-apply `disabled` and `title="Team (Coming Soon)"` to `#team-nav`**; the existing `enableTeamButton` fixture already toggles those off for the two specs that exercise Team, so no spec breaks.
- **Re-add Roboto Flex `<link>` to `index.html`** — CSP already allows fonts.googleapis.com / fonts.gstatic.com.
- **Extend the icon picker** to 3 `.icon-option` (ri-brain-line, ri-focus-3-line, ri-lightbulb-line) + 5 `.emoji-option` (existing) — keep the existing `tags.spec.js:17` flow green (still clicks the 🎯 emoji).
- **Add `dialog_save` bridge wrapper + `dialog:allow-save` capability** so the Excel export button can ask the user for a path.
- **Add `aria-current="page"` to the active sidebar nav button** via a derived signal.

TDD is observed: pure projection helpers (mode→stop-icon, mode→skip-icon, duration↔start/end recalculation) get `cfg(test)` host tests before wiring; e2e selectors that move (e.g. `#export-sessions-btn`, `#session-start-time`, `#session-end-time`) get a selector-contract host test alongside.

## Steps to Reproduce
1. `cargo tauri dev` and observe the timer view.
2. **Tooltips:** hover the right-rail icons; no native tooltip appears (only `data-tooltip` and `aria-label`, neither shows on hover).
3. **Font:** open DevTools → Computed → `#timer-minutes`; `font-family` resolves to Helvetica/Arial because Roboto Flex is not loaded — `<link>` removed from `index.html`.
4. **Stop in break:** press Play → Skip → observe stop button still shows × (full reset) instead of back-arrow (undo last completed pomodoro).
5. **Skip icon variants:** with `total_sessions ≥ 4` set, complete 3 focus sessions; the skip button still shows the coffee glyph instead of a moon for the upcoming long break.
6. **Icon picker:** click `#timer-status`, then `#selected-icon-btn`; only 5 emoji options visible — no `ri-brain-line` / `ri-focus-3-line` / `ri-lightbulb-line` rows.
7. **Settings footer:** open Settings → no "Reset to Defaults" button, no "✓ Settings are saved automatically" strip below the active tab.
8. **Shortcuts:** open Settings → Shortcuts → no per-row × Clear button, no description paragraph under each input.
9. **Calendar edit modal:** complete a focus session → switch to Calendar → click "Edit" on the history row → only Duration shown; no Start Time / End Time / Save / Delete.
10. **Tag Usage card:** Calendar view → no "Tag Usage This Week" pie-chart card under the daily-chart card.
11. **Chart legend:** Calendar view → "Today's Development" card has no legend explaining the colored bars.
12. **Excel export:** Calendar view → "Session History" card has no export button despite `commands::export_sessions_xlsx` being wired backend-side.
13. **User dropdown subtitle:** sidebar avatar → dropdown → header has name only, no second-line "Guest Mode" / "Signed In" subtitle.
14. **Team nav:** click `#team-nav`; the Team route is reachable when it should be disabled with a "Coming Soon" tooltip.
15. **Active sidebar nav:** keyboard/screen-reader hits `Tab` to a nav button; no `aria-current="page"` exists on the active button.

## Root Cause Analysis
The migration spec (001-leptos-migration) prioritized first-paint and the visual-regression baseline contract for each screen. The 14 baselines lock the **primary** rendering but do not cover hover states, modal open states (other than Calendar edit modal — which DOES regress because the modal shape is shallower than the JS-era one), or font fallback paths. The migration's component ports therefore satisfied the baseline contract while silently dropping every JS-era affordance that wasn't strictly needed to make a screenshot match.

The Roboto Flex font drop is the cleanest example: `index.html` was rewritten to be a thin Trunk shell, and the pre-migration `<link>` to Google Fonts was not carried forward. CSS still references the font; the browser falls through to the next family in the stack. This is a copy-paste oversight, not a deliberate design call (CSP allows the origin).

The Team nav `disabled` drop is a regression that happens to satisfy `tapTab(page, "Team")` (now no `enableTeamButton` is strictly needed for it to click through), but is wrong product-wise — Team is a demo fixture, not shipping.

The icon-picker `ICON_OPTIONS` constant in `timer.rs:63-69` documents itself as "Mirrors the JS-era set" but actually drops the 3 leading ri-* entries; the inline comment was wrong, not the port. The render branch (`timer.rs:1037-1054`) already detects `ri-` prefix vs emoji and switches `.icon-option` vs `.emoji-option` class — so extending the catalogue is mechanical.

## Relevant Files

### Files to modify

- **`src/src/components/timer.rs`** — Stop button icon (X vs back-arrow gated on mode), skip-button icon variants (add moon + default forward-arrow), `ICON_OPTIONS` extension, right-rail `title=` tooltips. `TimerState::decrement_completed_pomodoros` does NOT exist; we add a public method on the engine that saturates at 0 and rebases the displayed remaining (or we make `on_stop` in break-mode just decrement an outer counter — see "Step by Step Tasks" for the chosen path).
- **`src/src/engine/timer.rs`** — New `pub fn decrement_completed_pomodoros(&mut self)` that saturates at 0. Pure logic, host-testable.
- **`src/src/components/calendar.rs`** — Edit Session modal expansion (start time, end time, duration tri-recalc + Save + Delete), Tag Usage card (markup-only with TODO), chart legend block, Excel export button + click handler. The session-history table row's `on_open_modal` signature widens from `(u32)` to `(ManualSession)` so the modal can edit the full record.
- **`src/src/components/settings/mod.rs`** — Footer strip ("Settings are saved automatically" + "Reset to Defaults" button) rendered inside `#settings-view` below `.settings-content`. The reset button fires `settings.set(Settings::default())`.
- **`src/src/components/settings/shortcuts.rs`** — Per-row Clear (×) button + description paragraph in `shortcut_row`. Clear writes `None` to the matching `ShortcutSettings` field and toasts "Settings saved".
- **`src/src/components/auth_modal.rs`** — Restore `#user-status` span in `#user-dropdown-header`. Drive via a new `user_status_text(&AuthState) -> &'static str` helper (`"Guest Mode"` / `"Signed In"` / `""`).
- **`src/src/app.rs`** — `#team-nav`: `disabled`, `title="Team (Coming Soon)"`, `style="opacity: 0.5; cursor: not-allowed"`. Other nav buttons: keep existing `title=` (already set). Add `aria-current` projection: each button's `aria-current` attribute reads `"page"` when its `is_*` signal is true, otherwise empty string (or omitted via `Option<&str>`).
- **`src/index.html`** — Add `<link rel="preconnect" href="https://fonts.googleapis.com">`, `<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>`, and `<link href="https://fonts.googleapis.com/css2?family=Roboto+Flex:wght@400;800&display=swap" rel="stylesheet">` to `<head>`.
- **`src/src/bridge/commands.rs`** — Add `pub async fn dialog_save(default_path: Option<String>, filters: Vec<(String, Vec<String>)>) -> Result<Option<String>, BridgeError>` wrapping `plugin:dialog|save`. Returns `None` when user cancels.
- **`src-tauri/capabilities/default.json`** — Add `"dialog:allow-save"` to the permissions array.
- **`tests/e2e/fixtures/tauriMock.js`** — Add `case "plugin:dialog|save": return null;` (or a stub path; null mirrors the user-cancel path so any future spec that asserts no export-call-on-cancel passes). Update the case-comment block for `export_sessions_xlsx` only if the existing flow changes.
- **`scripts/check-mock-drift.sh`** — Re-run to verify; if it tracks new wrappers, ensure `dialog_save` is allow-listed (it wraps a plugin call, not a custom command — most likely no list change needed).

### No new files

(Calendar's edit-session-modal handler is extended in-place; the new helper functions live in `calendar.rs` test module + the production view body. The bridge wrapper is added to the existing `commands.rs`.)

## Step by Step Tasks

### 1. Engine: add `decrement_completed_pomodoros` (TDD)
- In `src/src/engine/timer.rs`, write a host test `decrement_completed_pomodoros_saturates_at_zero` that constructs a `TimerState::new`, calls `decrement_completed_pomodoros` repeatedly past 0, and asserts `completed_pomodoros() == 0` (no underflow, no panic).
- Add `pub fn decrement_completed_pomodoros(&mut self)` that does `self.completed_pomodoros = self.completed_pomodoros.saturating_sub(1);`. Method is mode-agnostic (caller in TimerView gates on mode).
- Run `cargo test -p presto-web engine::timer::tests::decrement_completed_pomodoros_saturates_at_zero` — green.

### 2. Timer: restore stop button mode-aware icon swap
- In `src/src/components/timer.rs`, add a host helper `stop_icon_for_mode(mode: TimerMode) -> &'static str` returning `"close"` for Focus, `"undo"` for Break/LongBreak. Add a test enumerating all three modes.
- In the `view!` body inside `#stop-btn`, render BOTH a `<svg id="stop-icon">` (existing X path) and a new `<svg id="undo-icon">` (path `d="M9 12l-2-2m0 0l2-2m-2 2h10.5a4.5 4.5 0 110 9h-4"` — verbatim from `git show a0bb52c:src/index.html` lines 195-208). Toggle each via `style=move || { ... display: none / "" ... }` keyed on the current mode.
- `on_stop` rewrites to:
  - If `current_mode() == Focus`: existing `engine.update(TimerState::reset)`.
  - Else (Break/LongBreak): `engine.update(TimerState::decrement_completed_pomodoros)` (no full reset — the in-flight break continues).
- Add a comment on `on_stop` explaining the why (`undo last session` semantics in break mode).

### 3. Timer: extend skip-icon variants
- Add a host helper `skip_icon_for_mode(mode: TimerMode, next_is_long_break: bool) -> &'static str` returning `"coffee"` for Focus+!long, `"moon"` for Focus+long, `"brain"` for Break/LongBreak. Test exhaustively.
- In the view, the existing `#skip-coffee-icon` and `#skip-brain-icon` stay. Add:
  - `#skip-sleep-icon` (`<i class="ri-moon-line">`), visible when in Focus AND the next mode is LongBreak (compute via `engine.with(|s| s.completed_pomodoros())` modulo `settings.timer.sessions_until_long_break` if such a field exists, otherwise gate on `(completed_pomodoros + 1) % 4 == 0`).
  - `#skip-default-icon` (the SVG forward-arrow from `index.html` 217-225) as the fallback — visible when none of the above match (defensive; covers any future mode addition).
- Wire each icon's `style=` to its visibility predicate; only one icon's `style=""` resolves at a time, the rest are `display: none`.

### 4. Timer: extend `ICON_OPTIONS` to 8 entries + render branch
- Change `ICON_OPTIONS: &[&str]` from 5 emojis to 8 entries: `["ri-brain-line", "ri-focus-3-line", "ri-lightbulb-line", "\u{1f9e0}", "\u{1f4aa}", "\u{1f3af}", "\u{26a1}", "\u{1f525}"]`. Update the comment to drop the "5 emojis" claim and reference issue #39.
- In the `For` loop at the icon picker (around line 1037-1054), replace the hardcoded `class="emoji-option"` with a runtime branch: if the icon starts with `"ri-"`, render `<div class="icon-option" data-icon=icon><i class=icon></i></div>`; else render the existing `<div class="emoji-option" data-icon=icon>{icon}</div>`. The detection mirrors the selected-icon-preview branch already in the view.
- Confirm `tags.spec.js:17` still passes — it clicks `[data-icon="🎯"]` which still resolves to an `.emoji-option`.

### 5. Timer: native tooltips on right-rail indicators + adjust buttons
- Add `title="Smart Pause: Click to toggle automatic pause when inactive"` to `#smart-indicator` alongside the existing `data-tooltip` (keep `data-tooltip` to avoid disturbing any CSS that reads it; the native `title` is the new hover affordance).
- Same for `#auto-start-indicator`, `#continuous-session-indicator`, `#timer-minus-btn` (already has `title`), `#timer-plus-btn` (already has `title`).
- Add `title=` to `#play-pause-btn`, `#stop-btn`, `#skip-btn` using their existing `aria-label` text.
- Rationale: native `title=` sidesteps the prior tooltip-out-of-bounds custom bubble bug per the issue's "fix that when restoring" note.

### 6. Calendar: extend Edit Session modal to start/end/duration + Save/Delete
- Host tests in `calendar.rs::tests`:
  - `duration_from_start_end_minutes` — given `"09:00"`, `"09:25"`, returns 25.
  - `end_time_from_start_duration` — given `"09:00"`, 25, returns `"09:25"`.
  - `start_time_from_end_duration` — given `"09:25"`, 25, returns `"09:00"`.
  - Handle midnight rollover by saturating (don't introduce date math here; the JS-era surface didn't either — clamp to `23:59` on overflow).
- Add the three helpers to `calendar.rs` (pure projections).
- Widen `on_open_modal` from `(duration: u32)` to `(session: ManualSession)`. Add three additional `RwSignal`s in the view: `modal_session_id: RwSignal<Option<String>>`, `modal_start: RwSignal<String>`, `modal_end: RwSignal<String>` (modal_duration already exists).
- Inside the modal body, add three `<input>` rows: `#session-start-time` (`type="time"`), `#session-end-time` (`type="time"`), keep `#session-duration`. Each input's `on:input` writes its signal AND recalculates the other two through the helpers (e.g. editing start time and keeping duration constant → recompute end time). Wrap the cross-recalc in a guard signal to avoid feedback loops (only the actively-edited input drives the recalc).
- Add `<div class="modal-actions">` with Cancel / Delete / Save buttons:
  - Cancel: existing `on_close_modal`.
  - Save: update `sessions` signal — find by id, replace duration / start / end. Then close.
  - Delete: update `sessions` signal — `retain(|s| s.id != id)`. Then close.
- Add `id="session-modal-overlay"`, `#session-form`, `#cancel-session-btn`, `#delete-session-btn`, `#save-session-btn`, `#session-modal-title` to match the pre-migration markup at `index.html:1216-1257`.

### 7. Calendar: Tag Usage card (markup only)
- Below the existing `daily-chart-card` div, add a `<div class="tag-usage-card">` containing `<h3>"Tag Usage This Week"</h3>` and the empty pie-chart placeholder structure from `index.html:386-401`.
- Inline `// TODO(#39): wire pie-chart slices to a tag-frequency projection over sessions filtered by week_dates.` comment at the binding site.

### 8. Calendar: chart legend under Today's Development
- Inside the existing `daily-chart-card`, after `<div class="daily-chart">`, add the `<div class="chart-legend">` block from `index.html:368-379` (two `<span class="legend-item">` rows with focus / break color swatches).

### 9. Calendar: Excel export button on Session History card
- Above `<div class="sessions-table-container">` inside `.sessions-history-card`, insert a `<div class="sessions-header">` (or extend the existing one) with a `<div class="sessions-controls">` wrapper containing `<button id="export-sessions-btn" class="export-btn" title="Export to Excel">` (use the file-with-lines SVG from `index.html:447-491`, or a simpler `<i class="ri-download-line"></i> Export` — pick the simpler glyph since the SVG is a 40-line embed).
- Click handler: `spawn_local` an async block that calls `commands::dialog_save(Some("sessions.xlsx".to_string()), vec![("Excel".to_string(), vec!["xlsx".to_string()])]).await.ok().flatten()`; on `Some(path)`, call `commands::export_sessions_xlsx(path, sessions.get_untracked()).await`. Errors absorbed (toast in a follow-up).

### 10. Settings shell: footer strip + Reset to Defaults button
- In `src/src/components/settings/mod.rs::SettingsView`, after the `<div class="settings-content">` block (and before the toast surface), add:
  ```rust
  <div class="settings-actions setting-item">
      <div class="auto-save-info">
          <span class="auto-save-text">"✓ Settings are saved automatically"</span>
      </div>
      <button
          class="btn-secondary"
          on:click=move |_| {
              settings.set(Settings::default());
              toast.show("Settings reset to defaults");
          }
      >"Reset to Defaults"</button>
  </div>
  ```
- The toast renderer at the shell already handles the "Settings reset to defaults" string the same way as "Settings saved".

### 11. Shortcuts tab: Clear button + description per row
- In `shortcut_row` (`src/src/components/settings/shortcuts.rs`):
  - Add `fn description(self) -> &'static str` returning the per-slot description text from `index.html:677,718,757`.
  - In the view, after the `<input>`, add `<button type="button" class="shortcut-clear" data-shortcut=... aria-label=...>"×"</button>`. Click handler: `settings.update(|s| match slot { ... = None; })` + `toast.show("Settings saved")`.
  - After `</div class="shortcut-input-container">`, add `<p class="setting-description">{slot.description()}</p>`.

### 12. Auth modal: restore `#user-status` subtitle
- Add `fn user_status_text(state: &AuthState) -> &'static str` returning `"Guest Mode"` / `"Signed In"` / `""`. Host test exhaustive over variants.
- Inside `#user-dropdown-header` next to `<span class="user-name" id="user-name">`, add `<span class="user-status" id="user-status">{move || auth_state.with(user_status_text)}</span>`.
- Update the selector-contract host test (`auth_modal_selector_contract_documented`) to include `"user-status"` if it enumerates IDs.

### 13. Sidebar: Team disabled + `aria-current` on active
- In `src/src/app.rs`, on the `#team-nav` button:
  - Add `disabled` attribute (always — no fixture exception needed in our code; the spec fixture already overrides).
  - Change `title="Team"` → `title="Team (Coming Soon)"`.
  - Add `style="opacity: 0.5; cursor: not-allowed"`.
- Remove the `on:click=on_team_nav` handler call (or keep it; with `disabled`, clicks are no-ops at the browser level — keep the handler so the fixture's `disabled = false` override re-enables interactivity without further patching).
- For each of `#timer-nav`, `#calendar-nav`, `#team-nav`, `#settings-nav`, add `attr:aria-current=move || if is_<view>.get() { "page" } else { "" }` (or use the Leptos `class:` analogue for attributes — `attr:aria-current=...` is the Leptos prop).

### 14. index.html: add Roboto Flex link
- Edit `src/index.html` `<head>` to add (before `<link rel="rust" ...>`):
  ```html
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
  <link href="https://fonts.googleapis.com/css2?family=Roboto+Flex:wght@400;800&display=swap" rel="stylesheet" />
  ```
- CSP already permits the origins (`style-src https://fonts.googleapis.com`, `font-src https://fonts.gstatic.com`).

### 15. Bridge: add `dialog_save` wrapper + capability
- In `src/src/bridge/commands.rs`, after `dialog_ask`, add:
  ```rust
  pub async fn dialog_save(
      default_path: Option<String>,
      filters: Vec<(String, Vec<String>)>,
  ) -> Result<Option<String>, BridgeError> {
      #[derive(Serialize)]
      struct FilterArg { name: String, extensions: Vec<String> }
      #[derive(Serialize)]
      struct Args { #[serde(rename = "defaultPath")] default_path: Option<String>, filters: Vec<FilterArg> }
      let filters = filters.into_iter().map(|(name, extensions)| FilterArg { name, extensions }).collect();
      invoke_serde("plugin:dialog|save", &Args { default_path, filters }).await
  }
  ```
- Add `"dialog:allow-save"` to `src-tauri/capabilities/default.json` permissions array.
- Add `case "plugin:dialog|save": return null;` to `tests/e2e/fixtures/tauriMock.js` (around line 309, alongside the existing `plugin:dialog|*` handlers). Cancel-by-default matches the existing `plugin:dialog|ask` pattern.

### 16. Regenerate visual baselines if needed (≤2)
- After all the above land, run `(cd tests/e2e && npx playwright test visual-regression.spec.js)` and inspect any diff PNGs.
- Per the constitution's 2-baseline-per-PR limit, expect at most:
  - `timer-chromium-linux.png` (Roboto Flex now renders, font metrics shift).
  - `auth-modal-chromium-linux.png` (user-status subtitle now appears).
- If MORE than 2 baselines drift, escalate explicitly in the PR description; the most likely culprits are settings-* baselines if the new footer strip pushes content reflow. If so, consider whether the footer's vertical position can be `position: fixed` at the bottom of `.settings-content` to avoid pushing tabs.

### 17. Run the full gate set
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings -W clippy::pedantic`
- `cargo test --workspace --frozen`
- `(cd src && wasm-pack test --node)`
- `(cd tests/e2e && npx playwright test)`
- `bash scripts/check-mock-drift.sh`
- `bash scripts/check-baseline-cap.sh` (if it exists; the gate caps re-captures at 2)
- `bash scripts/check-engine-purity.sh`
- `bash scripts/check-lockfile-drift.sh`

## Validation Commands
Execute every command to validate the bug is fixed with zero regressions.

```bash
# Lint + format
cargo fmt --all --check
cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic

# Host-side tests (engine purity + selector contracts + new helpers)
cargo test --workspace --frozen

# WASM-side tests (bridge wrappers, including the new dialog_save round-trip)
(cd src && wasm-pack test --node)

# E2E + visual regression
(cd tests/e2e && npm ci)
(cd tests/e2e && npx playwright install --with-deps chromium)
(cd tests/e2e && npx playwright test)
(cd tests/e2e && npx playwright test visual-regression.spec.js)

# CI gate scripts
bash scripts/check-mock-drift.sh
bash scripts/check-engine-purity.sh
bash scripts/check-lockfile-drift.sh

# Manual smoke (each item in "Steps to Reproduce" should now resolve):
cargo tauri dev
# In the running app:
#   - Hover any right-rail icon → native tooltip appears
#   - DevTools → Computed → #timer-minutes font-family resolves to "Roboto Flex"
#   - Start → Skip → stop-btn shows the back-arrow (undo) icon; click decrements completed_pomodoros without resetting break time
#   - Complete 3 focus sessions → skip-btn shows moon glyph for the upcoming long break
#   - #timer-status → #selected-icon-btn → 3 ri-* options + 5 emoji options visible
#   - Settings → footer strip + "Reset to Defaults" button visible; click resets every field
#   - Settings → Shortcuts → each row has × button + description paragraph
#   - Calendar → complete a focus session → click "Edit" → Start/End/Duration tri-recalculate; Save / Delete work
#   - Calendar → "Tag Usage This Week" card visible; Today's Development card has legend
#   - Calendar → "Session History" card has Export button; clicking it opens the OS save dialog
#   - Sidebar → user avatar → dropdown header shows "Guest Mode" subtitle
#   - Sidebar → Team button shows greyed-out with "Team (Coming Soon)" tooltip; click does nothing
#   - Keyboard tab through sidebar → active nav button reports aria-current="page" to AT
```

## Notes

- **Why native `title=` over a custom tooltip CSS bubble:** the issue explicitly calls out the prior tooltip-out-of-bounds rendering bug. Native browser tooltips cannot be rendered out of bounds because the browser manages positioning. They are not pixel-controllable (no theming), which is an acceptable trade for parking the rendering bug; the JS-era surface already had `title=` on most icons and the `data-tooltip` attribute was redundant. We leave `data-tooltip` in place to avoid CSS churn.

- **Why the new engine method (`decrement_completed_pomodoros`) instead of mutating the field directly from the view:** Principle I — the engine is the sole owner of `TimerState`'s invariants. The view dispatches via the engine API even for trivial mutations so the host-side test for saturation is co-located with the field.

- **Tag Usage pie-chart + chart legend are intentionally markup-only:** the issue's "Constitution / contract notes" section explicitly permits this with `TODO` comments. The data-binding pass is a follow-up because pie-chart slice generation needs a tag-frequency-over-week projection that isn't currently exposed.

- **The Edit Session modal's `<form>` does NOT submit to a Tauri command directly.** It mutates the in-memory `sessions` signal; the existing `bridge::commands::save_manual_sessions` debounced sink at `app.rs:469-480` picks up the change and persists. Same pattern as everywhere else in the app — no new persistence hop.

- **Team `disabled` interaction with the existing `enableTeamButton` fixture:** the fixture toggles `el.disabled = false`, `el.style.opacity = ''`, `el.style.cursor = ''`, `el.title = 'Team'` — all in a `DOMContentLoaded` `once: true` listener. Our `view!` macro emits inline `style="opacity: 0.5; cursor: not-allowed"` which the fixture's empty-string assignments will clear at runtime. The fixture continues to work; no spec change needed.

- **The settings reset behavior** clobbers in-memory settings to `Settings::default()` and lets the debounced persistence sink at `app.rs:323-361` push the cleared state to disk. The 300ms debounce window means a rapid "reset → manual edit" sequence persists only the edited state, which is correct semantics.

- **The visual-regression suite calls `enableTeamButton()` BEFORE the timer baseline screenshot** (`visual-regression.spec.js:16`), so the timer screenshot will not see a 50%-opacity Team button. The other 13 baselines are taken after `enableTeamButton()` is in effect, so they also will not see the disabled state. The 14 baselines should drift only due to the Roboto Flex font load and the auth-modal subtitle.

- **No new spec files are required** — every restored affordance is either covered by existing specs (Tag picker by `tags.spec.js`; Excel export not currently asserted but no harm in absence) or is markup-only. The selector-contract host tests for `auth_modal`, `calendar`, `settings/mod`, `settings/shortcuts`, `timer` should be extended to include the newly-added IDs to catch drift at `cargo test` time.

---
*Generated by Agentex*
