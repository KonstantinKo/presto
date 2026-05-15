# Implementation Plan: Overtime Button Treatment

**Branch**: `007-overtime-button-treatment` | **Date**: 2026-05-15 | **Spec**: [`spec.md`](spec.md)
**Input**: Feature specification from `/specs/007-overtime-button-treatment/spec.md`

> Thin, living plan. Cites spec FRs and code paths. Does not re-state them.
> Spec FR-NNN labels are normative; this plan binds them to file paths.

## Summary

A UI-layer treatment for one moment in the engine's lifecycle that already has full engine coverage: continuous-mode focus past zero. No engine change. The plan threads four narrow seams together:

1. **2D button matrix at the UI layer** (FR-001 → FR-009). The existing `RunState` closed sum (`src/src/components/timer/mod.rs:216-243`) stays as-is. A derived signal `(RunState, is_overtime)` selects the button trio in the click-dispatch `match` at `src/src/components/timer/mod.rs:2273-2277` and `2323-2328`. In overtime, all three slots dispatch to the existing `on_complete` handler — same engine path as feature 006's Paused-Complete.
2. **`"Wrap it up!"` CTA** (FR-010 → FR-012). New `<p class="overtime-cta">` element between the countdown and the button row, visibility bound to `is_overtime`, text from new catalogue key `timer.overtime_cta`.
3. **CSS overtime tint on buttons** (FR-005, FR-006). New `.control-btn.overtime` modifier class re-using `var(--warning-color)` (already light `#e67e22` / dark `#f59e0b` — `src/style/variables.css:22,48,72`). Filled-vs-ghost driven by the existing `.primary` semantics. No new CSS variables.
4. **Abort global shortcut** (FR-017 → FR-021). Extend `ShortcutSettings` (`crates/presto-ipc/src/settings.rs:113-127`) with an `abort: Option<String>` field, extend the Tauri-side registration loop at `src-tauri/src/lib.rs:432-473` to also register `abort`, extend the frontend listener at `src/src/app.rs:613-624` to route `"abort"` to `engine.abort(clock)`, extend the Settings > Shortcuts panel (`src/src/components/settings/shortcuts.rs`) with a fourth row.

A11y treatment of the outer slots reuses the `tabindex=-1` + `aria-hidden` precedent from `src/src/components/settings/theme.rs:217`. Visual regression budget: one new baseline.

## Technical Context

**Language/Version**: Rust 1.75+ (frontend WASM + backend native), JS in the Playwright e2e suite.
**Primary Dependencies**: Leptos (CSR + WASM), Trunk, Tauri 2.x, `tauri-plugin-global-shortcut`, `presto-ipc` (workspace), `serde`. All already in tree.
**Storage**: `ShortcutSettings.abort` joins the existing `settings.json` payload via `AppSettings::shortcuts`. No new file.
**Testing**: `wasm-bindgen-test` for frontend (the matrix derivation), `cargo test` for `crates/presto-ipc` round-trips, Playwright (chromium) for e2e + visual regression.
**Target Platform**: Tauri desktop. CI VR reference is `chromium-linux`.
**Project Type**: Single-user desktop app.
**Performance Goals**: Overtime treatment + countdown colour flip within the same UI tick as the zero-cross (SC-001). Exit-to-non-overtime within the same UI tick as the underlying state change (SC-009).
**Constraints**: Engine remains a pure state machine (Principle I) — this feature does NOT touch the engine. Local-only (Principle II). Strict clippy + fmt (Principle X). No new `#[allow]` carve-outs.
**Scale/Scope**: One new IPC field (`ShortcutSettings.abort`), zero new Tauri commands (registration handler `register_global_shortcuts` widened — same surface), one new global-shortcut event name (`"abort"` on the existing `global-shortcut` channel), one new settings-panel row, one new CSS modifier class, one new CTA element, one new e2e spec, one extension to the existing `settings-shortcuts.spec.js`, one new VR baseline. Three new catalogue keys (`timer.overtime_cta`, `settings.shortcuts.label_abort`, `settings.shortcuts.desc_abort`) across EN/DE/IT/TR.

## Constitution Check

Only violations and justified deviations are listed. Pass lines are omitted.

No principle violations. Notes on principle-brushed surfaces:

- **I. The Timer Is Sacred** — zero engine changes. All three overtime Complete clicks dispatch to the existing feature-006 `on_complete` handler, which calls `engine.complete(clock)`. The branch-B.2 path at `src/src/engine/timer.rs:998-1040` (already shipped via feature 006) handles continuous-mode overtime seal + cadence advance. No new entry points, no new variants, no helper extraction.
- **III. Type Safety Over Defensive Code** — overtime is a **derived predicate** (the existing `is_overtime` signal at `src/src/components/timer/mod.rs:1130`), not a new `RunState` variant. The 2D matrix is an exhaustive 2-tuple `match`, never a flag-bool conditional. The illegal state "overtime in non-continuous mode" is impossible by derivation: `is_overtime` reads `engine.time_remaining_secs_signed() < 0`, and the engine only allows time-remaining to go negative in continuous-mode focus. `ShortcutSettings.abort: Option<String>` is symmetric with the existing three fields — no defensive new variant.
- **IV. Visual Regression Is The UI Contract** — one new baseline (`timer-focus-continuous-overtime-chromium-linux.png`). Existing baselines unaffected: overtime is gated on continuous-mode + focus-past-zero, never reached by the canonical idle / running / paused baselines. Per-baseline PR note included.
- **V. Test-First For Stateful Engines** — **NOT in scope** for this feature. The engine paths exercised (`engine.complete(clock)` branch B.2 and the existing `engine.abort(clock)`) are fully covered by feature 006's RED tests at `src/src/engine/timer.rs` (`#[cfg(test)] mod tests`). No new engine behaviour. UI plumbing and a11y are covered by the e2e suite, which is not Principle V scope.
- **VI. The Tauri Boundary Is Stable** — **no new Tauri command**. The existing `register_global_shortcuts` command (`src-tauri/src/lib.rs:432-473`) handles the abort binding via the same loop that handles start-stop / reset / skip. The existing `global-shortcut` event channel (`src/src/bridge/events.rs:55`) carries the new `"abort"` name as a primitive `String` payload — same shape as today's three names. The mock at `tests/e2e/fixtures/tauriMock.js:127` already accepts `register_global_shortcuts` and does not validate the payload shape; it absorbs the new `abort` field transparently. **No new mock command required** — mock-first rule is honoured by inspection.
- **VIII. Spec-Driven Feature Flow** — multi-file work (timer view, settings panel, IPC type, Tauri registration, catalogue, e2e). Spec exists at `specs/007-overtime-button-treatment/spec.md` with 24 FRs, 10 SCs, 7 anchored PM markers.

## Project Structure

### Documentation (this feature)

```text
specs/007-overtime-button-treatment/
├── plan.md                              # This file
├── spec.md                              # Already written
├── research.md                          # Phase 0 — only irreversible decisions
├── data-model.md                        # Phase 1 — derived types + invariants
├── contracts/
│   └── shortcut-registration.md         # Widened register_global_shortcuts + abort event
└── quickstart.md                        # Phase 1 — dev exercise of the feature
```

### Source Code Touched (repository root)

```text
crates/presto-ipc/src/
└── settings.rs                # +abort: Option<String> on ShortcutSettings (line 113-127),
                               #   Default::default() leaves it None (FR-019)

src-tauri/src/
└── lib.rs                     # Widen the registration loop at lines 442-446 to include
                               #   ("abort", &shortcuts.abort). Same on_shortcut closure
                               #   pattern, same debounce, same global-shortcut emit.

src/src/
├── app.rs                     # Extend the listener at lines 613-624 to route "abort"
│                              #   into engine.abort(clock). Routing uses the same
│                              #   mechanism feature 006 uses for the abort handler.
├── components/
│   ├── timer/
│   │   └── mod.rs             # (a) Click-dispatch match extended to 2-tuple
│   │                          #     (RunState, is_overtime) at lines 2273-2277, 2323-2328.
│   │                          #     Overtime ⇒ on_complete for all three slots.
│   │                          # (b) New <p class="overtime-cta"> element between the
│   │                          #     countdown div and the .controls div (around line 2241).
│   │                          # (c) aria-hidden + tabindex=-1 on the outer two
│   │                          #     <button>s when (Running, true). Center keeps the
│   │                          #     existing aria-label.
│   │                          # (d) Label + icon dispatch extended to show "✓ Complete"
│   │                          #     on all three slots when is_overtime is true.
│   │                          # (e) class:overtime on the three control-btn elements
│   │                          #     bound to (run_state == Running && is_overtime).
│   │                          # (f) Remove the now-obsolete #[cfg(test)]
│   │                          #     mode_label_with_status hard-coded "(Overtime)" at
│   │                          #     line 154 — replace with the catalogue key. See
│   │                          #     "i18n hygiene note" below for why this is small.
│   └── settings/
│       └── shortcuts.rs       # +ShortcutSlot::Abort variant, +abort field accessor in
│                              #   the slot trio, +shortcut_row call for Abort at the
│                              #   bottom of the section, +selector-contract test for
│                              #   #abort-shortcut.
├── i18n/                      # +3 catalogue keys: timer.overtime_cta,
│                              #   settings.shortcuts.label_abort,
│                              #   settings.shortcuts.desc_abort.
└── locales/
    ├── en.json                # New keys (EN source-of-truth)
    ├── de.json                # New keys (good-faith DE translation)
    ├── it.json                # New keys (good-faith IT translation)
    └── tr.json                # New keys (EN fallback acceptable per feature 005 hedge)

src/style/
└── timer.css                  # +.control-btn.overtime modifier:
                               #     border-color + color = var(--warning-color)
                               #   +.control-btn.overtime.primary (filled center):
                               #     background = var(--warning-color), inverted text
                               #   +.overtime-cta element styling (small, centered,
                               #     same warning-color tint).

tests/e2e/
├── timer-overtime.spec.js                  # NEW — see Test plan section.
└── settings-shortcuts.spec.js              # Extend with the fourth-row Abort case.

tests/e2e/__screenshots__/visual-regression/
└── timer-focus-continuous-overtime-chromium-linux.png    # NEW baseline.
```

**Structure Decision**: Reuse the existing Tauri + Leptos layout. No new module. No new crate. The change is a coordinated set of edits across one IPC type, one Tauri handler, one frontend listener, one timer view, one settings panel, one CSS file, four catalogue files, two e2e specs.

## Architecture overview

Two concentric rings, no engine ring.

**Ring 1 — UI matrix and presentation (Principles III + IV).** The derived signal `is_overtime` already exists at `src/src/components/timer/mod.rs:1130`:

```rust
let is_overtime = Signal::derive(move || engine.with(|s| s.time_remaining_secs_signed() < 0));
```

It is already wired to `class:overtime` on `#timer-view.container` (line 1885) and `.timer-container` (line 2237), and the pulsating-orange `--warning-color` CSS rule fires off it at `src/style/timer.css:644-649`. Reuse this signal — it is the single source of truth for "are we in overtime?" across the timer view.

The button matrix today is a `(RunState, …)`-driven exhaustive `match` at two call sites — the left slot's click dispatch at `src/src/components/timer/mod.rs:2273-2277` and the right slot's at `2323-2328`. This feature lifts the dispatch to `(RunState, is_overtime)`:

```rust
// Left slot (id=stop-btn) click handler (overtime extension):
on:click=move |ev| {
    let ot = is_overtime.get_untracked();
    match (run_state.get(), ot) {
        (RunState::Running, true)  => on_complete(ev),  // overtime collapses to Complete
        (RunState::Idle, _)        => on_open_quick_log(ev),
        (RunState::Running, false) => on_abort(ev),
        (RunState::Paused, _)      => on_abort(ev),     // FR-022: paused-during-overtime falls back to normal Paused
    }
}
```

The center slot (`#play-pause-btn`) already calls `on_complete` from Paused (per feature 006's matrix) and `on_play_pause` from Running. In overtime-Running, the center slot dispatches via a single named closure `on_center_click` (see PM-decision #3) — the 2D `(RunState, is_overtime)` matrix lives in one place, not spread across JSX. The right slot (`#skip-btn`) gets the same overtime-collapse treatment as the left slot.

**Critical invariant**: `RunState::Paused` short-circuits the overtime treatment. Per FR-022 and FR-023 + Edge Cases: a paused overtime session reverts to the normal Paused matrix (`Abort | Resume | Complete`). The CTA hides, the button orange tint clears. The countdown's orange tint continues because the engine's `is_overtime` predicate remains true — that's an engine fact, not a presentation fact. This is the `[BEST-GUESS PM DECISION]` item 7 from the spec, formalised: **`is_overtime`'s effect on the button matrix and CTA is gated on `RunState::Running`**, while its effect on the countdown is gated on the engine's predicate alone.

The CTA element is a sibling of `.controls` and `.timer-container`, inserted between them so the visual hierarchy reads countdown → CTA → button row (per spec Assumption "between the pulsating countdown and the button row"). Visibility is bound to the same Running × overtime predicate as the buttons so the CTA and the button orange treatment appear / disappear synchronously (SC-001, SC-006, SC-009).

**A11y (FR-014, FR-015, SC-003, SC-004)**: when `(Running, true)`, the left and right `<button>` elements carry `aria-hidden="true"` and `tabindex="-1"`. The center button keeps its standard `aria-label` (the existing `timer.ctrl_complete_aria` key, already used by feature 006 for Paused-Complete — `FR-016` requirement). The Settings > Theme panel's tab-order-removal precedent at `src/src/components/settings/theme.rs:217` is the pattern. Click still works on the outer slots (mouse + touch are not in the accessibility tree; `aria-hidden` removes from screen readers; `tabindex=-1` removes from tab order — pointer events untouched).

**Ring 2 — Settings + global-shortcut wiring (Principles III + VI).** `ShortcutSettings` at `crates/presto-ipc/src/settings.rs:113-127` today carries three `Option<String>` fields. Add a fourth, `abort: Option<String>`, defaulting to `None` (FR-019). The settings persistence is bulk-serialised via the existing `AppSettings` round-trip — no helper change.

The Tauri-side handler `register_global_shortcuts` at `src-tauri/src/lib.rs:432-473` iterates a static slice of `(action, shortcut_str)` pairs. Add `("abort", &shortcuts.abort)` to that slice. Same `on_shortcut` closure, same `should_debounce_shortcut` gate, same `app_handle.emit("global-shortcut", action_owned.as_str())` line. The closure code is variant-free; no new branch needed.

The frontend listener at `src/src/app.rs:613-624` is currently a no-op stub (Phase 4c never landed; feature 006 did NOT wire the existing three names). Feature 007 implements the full four-arm dispatch. Wire names are kebab-case per `src-tauri/src/lib.rs:442-446`; matches Tauri emitter.

**Default binding** (FR-019): `abort: None`. Matches the `[BEST-GUESS PM DECISION]` in the spec — the user opts in via the Settings panel.

**Side-effect parity for overtime Complete**: feature 006's `on_complete` handler (at `src/src/components/timer/mod.rs:1444-1464`) already runs the same downstream pipeline as `on_play_pause` — `handle_events` (drives bell/notification + UI toasts), `apply_tag_tracking_events`, `dispatch_tray_update`. Plus `persist_focus_completion` on count-with-advance (feature 006 R-001). Skips `prime_audio_context` + `prime_ambient_audio` because those are user-gesture-time primers for the audio API — needed on Start/Resume/Pause clicks, NOT on Complete (the bell sound is fired downstream by the engine event handlers, which use already-primed audio contexts from the original Start click). Feature 007's three overtime buttons all dispatch via `on_complete`, inheriting this parity.

**Settings UI** (`src/src/components/settings/shortcuts.rs`): the existing `ShortcutSlot` enum has three variants. Add `Abort` as a fourth. The slot's `input_id` is `"abort-shortcut"` (kebab-case to match the existing `"start-stop-shortcut"`, `"reset-shortcut"`, `"skip-shortcut"` selector convention — confirmed by the `shortcuts_selector_contract_documented` test at line 259). Render a fourth `shortcut_row(ShortcutSlot::Abort, …)` at the end of the section.

**Race / refresh consideration**: when the user changes the Abort shortcut binding mid-session, the existing `register_global_shortcuts` re-registration call unregisters all four bindings and re-registers from scratch. The existing `start_stop` / `reset` / `skip` bindings already trigger a full re-registration on change; the new `abort` binding inherits that behaviour for free.

## Module breakdown

| Module | Path | Why |
|---|---|---|
| `presto_ipc::settings::ShortcutSettings` | `crates/presto-ipc/src/settings.rs:113-127` (edit) | +`pub abort: Option<String>` field. `Default::default()` leaves it `None`. Serde wire convention follows the three existing fields — `#[serde(default)]` is implicit via `Option`. No new `serde` attributes needed. When extending the `Default` impl, add a `//` doc-comment line above the `abort: None` initialiser explaining the intentional asymmetry. The comment is normative — future implementations must not "fix" the asymmetry without spec revision. |
| `src-tauri/src/lib.rs` | edit (lines 432-473) | Add `("abort", &shortcuts.abort)` to the registration loop's iterator. Same closure, same emit. |
| `src/src/app.rs` | **rewrite** (lines 613-624) | The existing listener body is a no-op stub (Phase 4c never landed). Feature 007 implements the full four-arm dispatch — `"start-stop"` → `engine.try_update(state.start_stop)`, `"reset"` → `engine.try_update(state.reset)`, `"skip"` → `engine.try_update(state.skip)`, `"abort"` → `engine.try_update(state.abort)`. Each branch follows the same side-effect pipeline that the corresponding UI button uses (`handle_events`, `apply_tag_tracking_events`, `dispatch_tray_update`). The four engine entry points are the canonical truth — global shortcuts MUST funnel through them, not parallel-dispatch. Wire names are kebab-case per `src-tauri/src/lib.rs:442-446`; matches Tauri emitter. |
| `src/src/components/timer/mod.rs` | edit (lines 2267-2329, plus the CTA insertion around line 2241, plus the `mode_label_with_status` test helper update at line 154) | (a) Click-dispatch lifted to `(RunState, is_overtime)` 2-tuple match — see Architecture overview. (b) New `<p class="overtime-cta">` element between `.timer-container` and `.controls`. (c) `aria-hidden` + `tabindex` bindings on the outer slots. (d) Labels + icons (`✓ Complete` ✕3) dispatch on overtime predicate. (e) `class:overtime` on each `.control-btn`. (f) Test helper's hard-coded `"(Overtime)"` (line 154) replaced with the catalogue key. |
| `src/src/components/settings/shortcuts.rs` | edit | +`ShortcutSlot::Abort` variant, +`input_id`/`placeholder`/`label`/`description` arms, +`shortcut_row(ShortcutSlot::Abort, …)` call in the view, +selector-contract test extension. |
| `src/locales/{en,de,it,tr}.json` | edit | +3 new keys: `timer.overtime_cta`, `settings.shortcuts.label_abort`, `settings.shortcuts.desc_abort`. EN source-of-truth, DE/IT good-faith translations, TR may EN-fallback per feature 005 hedge. |
| `src/style/timer.css` | edit | +`.control-btn.overtime` (border + color = `var(--warning-color)`), +`.control-btn.overtime.primary` (filled center; background = warning, foreground = inverted), +`.overtime-cta` (small centered text, warning tint, margin tuned to sit between countdown and button row). |
| `tests/e2e/timer-overtime.spec.js` | new | Triple-Complete dispatch, orange tint, CTA visibility, a11y removal, exit-via-Complete, exit-via-Abort-keyboard. |
| `tests/e2e/settings-shortcuts.spec.js` | edit | Fourth-row Abort recording + persistence across reload. |
| `tests/e2e/__screenshots__/visual-regression/timer-focus-continuous-overtime-chromium-linux.png` | new | The orange three-Complete + CTA + pulsating-orange-countdown frame. |

## IPC / Tauri command surface changes

**No new Tauri command.** The existing `register_global_shortcuts` (`src-tauri/src/lib.rs:432-473`) is the carrier. Its argument shape today is `ShortcutSettings`; widening that struct with a new `Option<String>` field is a backwards-compatible serde change (old JSON deserialises with `abort = None` via `Option`'s default behaviour). The mock at `tests/e2e/fixtures/tauriMock.js:127` accepts the command without payload validation, so the new field is absorbed transparently — **no mock change required** (verified by inspection per the mock-first principle).

**No new event channel.** The existing `global-shortcut` event channel (`src/src/bridge/events.rs:55`, declared as `pub const GLOBAL_SHORTCUT: &str = "global-shortcut";`) carries the new `"abort"` name as a primitive `String` payload — identical shape to the existing three names. Listener-side, `src/src/app.rs:614` already deserialises the payload as `String`.

**Settings payload** (load_settings / save_settings): the existing `AppSettings` JSON wire shape is widened by one optional field inside `shortcuts`. The roundtrip tests at `src-tauri/src/lib.rs:1151,1237` already exercise null bindings (`"shortcuts": {"start_stop": null, "reset": null, "skip": null}`); a single line in those test fixtures gains `"abort": null`. The pre-feature settings.json on a user's disk (which lacks the `abort` key entirely) deserialises with `abort = None` — `serde`'s `Option` already handles missing keys without a `#[serde(default)]` attribute on the field, per the precedent of the existing three nullable fields.

Full text contracts (widened registration handler + abort event semantics) live in `contracts/shortcut-registration.md`.

## UI surface changes

The three timer-view edits, sequenced in dependency order:

**1. CTA element** (FR-010, FR-011, FR-012). Insert between the `.timer-container` (line 2241 closing tag) and the `.controls` (line 2267 opening tag) a new element:

```rust
<p
    class="overtime-cta"
    class:visible=move || matches!(run_state.get(), RunState::Running) && is_overtime.get()
>
    {move || t_string!(i18n, timer.overtime_cta)}
</p>
```

The `.overtime-cta` CSS rule defaults to `display: none`; `.overtime-cta.visible` sets `display: block` (or `visible`). Visibility predicate is `(Running, is_overtime == true)` — `Paused` short-circuits per FR-022 + the architecture-overview invariant.

**2. Button matrix extension** (FR-001 → FR-005). The two click-dispatch matches at lines 2273-2277 and 2323-2328 widen to 2-tuple. The center button's `on_play_pause` handler at line 2308 is replaced by the named `on_center_click` closure (see PM-decision #3), which dispatches to `on_complete` when `(Running, is_overtime) == (Running, true)`. The slot **labels** flip to `✓ Complete` for all three when overtime is active:

| Slot | (Running, false) | (Running, true) | (Paused, *) |
|---|---|---|---|
| Left | `✕ Abort` (ghost) | `✓ Complete` (ghost, overtime tint) | `✕ Abort` (ghost) |
| Center | `⏸ Pause` (filled) | `✓ Complete` (filled, overtime tint) | `▶ Resume` (filled) |
| Right | `! Note Distraction` (ghost) | `✓ Complete` (ghost, overtime tint) | `✓ Complete` (filled) |

The `(Paused, *)` column is the existing feature-006 matrix — overtime is gated off Running so Paused-during-overtime is the same as normal Paused (per FR-022). The icon visibility logic mirrors the label dispatch: the existing per-icon `display: none / inherit` toggles at lines 2286, 2299, 2334+ get a new branch for `(Running, true)` that shows the `✓` glyph (the same one feature 006 uses for the Paused Complete button, already in tree).

**3. A11y attributes** (FR-014, FR-015). On `#stop-btn` and `#skip-btn` only:

```rust
aria-hidden=move || matches!(run_state.get(), RunState::Running) && is_overtime.get()
tabindex=move || if matches!(run_state.get(), RunState::Running) && is_overtime.get() { -1 } else { 0 }
```

Pattern matches `src/src/components/settings/theme.rs:217`. The center `#play-pause-btn` keeps the standard tab order and `aria-label`. The center's `aria-label` reactive binding already reads from `verbose_label_play` (line 2305), which is a feature-006 signal derived from `(RunState, TimerMode)`. Extend `verbose_label_play` (and its terse + tooltip siblings) to additionally project on overtime — in `(Running, true)`, return the existing `timer.ctrl_complete_aria` (and `timer.ctrl_complete` / its terse variant) so screen readers announce `"Complete the current session and advance"` (FR-016).

**4. `class:overtime` on the three `<button>`s.** Bound to `(Running, is_overtime == true)`. The new CSS modifier replaces the per-mode color tokens with `var(--warning-color)`. The Paused-state right-slot's existing `class:primary` binding at line 2319 is untouched; in overtime the center is the only filled slot (per the matrix above).

**5. i18n hygiene note (FR-013).** Production code at `src/src/components/timer/mod.rs:1179` already uses `t_string!(i18n, timer.status_overtime)` — the catalogue key exists at `src/locales/en.json:172`. The only remaining literal is in the `#[cfg(test)]` helper at `src/src/components/timer/mod.rs:154`. Feature 007 ships the test-helper update as a one-line hygiene fix (replace the literal with the catalogue key), not a production change. **Production code is already internationalised.** FR-013's intent is satisfied; the only delta this PR ships is removing the test-helper literal in favour of the catalogue key (so a future re-word of the catalogue value does not silently desync the test helper's assertion expectations). See `[BEST-GUESS PM DECISION]` item 1 below.

## Visual regression budget

Per Principle IV.

| Baseline | Change | One-line PR note (draft) |
|---|---|---|
| `timer-focus-continuous-overtime-chromium-linux.png` (NEW) | The overtime button-row treatment with three orange ✓ Complete slots (center filled, outer two ghost), the "Wrap it up!" CTA between the countdown and the buttons, the pulsating-orange countdown above. Captured in continuous mode at 14 minutes past zero (matches Story 1 framing). | "Timer Focus Continuous Overtime (new baseline): three-orange ✓ Complete button row + 'Wrap it up!' CTA + pulsating-orange countdown." |

**No other baselines are expected to regenerate.** Overtime is gated on continuous-mode focus past zero, a state not visited by the canonical idle/running/paused baselines. Any unrelated diff is a code regression (SC-007 / SC-008 — light + dark + the localised strings render unchanged outside this one new frame).

**Baseline cap.** `.agentex.yml` carries a default cap; feature 006 widened it to 3 via a per-feature carve-out (line 67). This feature **adds 1 new** baseline, regenerates 0 — well under the default cap. No carve-out needed. The PR description includes the one-line note as standard.

**Dark-mode coverage.** Per SC-007 the overtime treatment renders identically in light and dark — but the VR baseline is chromium-linux light only (CI reference platform). Dark-mode parity is verified via the pre-PR design QA loop (user-memory `feedback_design_qa_loop`): inspect light + dark screenshots before opening the PR. Iterate with fix agents until both render clean.

## Test plan

Test-first commit ordering per **V. Test-First For Stateful Engines** — **not in scope** for this feature. The engine is not touched. RED-then-GREEN ordering applies only to engine + manager + Tauri-bridge contract tests. UI plumbing, a11y wiring, and shortcut listener routing are covered by the e2e suite (which is not Principle V scope — `AGENTS.md` rule).

**No new engine tests.** The branch-B.2 continuous-mode-overtime path at `src/src/engine/timer.rs:998-1040` is fully covered by feature 006's RED tests (`complete_in_continuous_mode_seals_with_overtime_elapsed`, `complete_in_continuous_overtime_does_not_double_count`).

**`crates/presto-ipc` round-trip tests** (regression — small):

| Test | Asserts |
|---|---|
| `shortcut_settings_with_abort_roundtrips` | A `ShortcutSettings { start_stop: Some(_), reset: Some(_), skip: Some(_), abort: Some("CommandOrControl+Alt+W") }` serialises + deserialises identically. |
| `shortcut_settings_with_unbound_abort_roundtrips` | `abort: None` serialises to `null` and deserialises back to `None`. |
| `shortcut_settings_missing_abort_field_defaults_to_none` | A pre-feature settings.json (no `abort` key) deserialises with `abort: None`. Mirrors the existing nullable-field precedent. |

These are RED-before-GREEN per the **wire-format-as-engine** rule — the IPC type IS a stateful contract.

**E2E specs** (mock-first; the mock already absorbs `register_global_shortcuts` payload-agnostically):

| Spec | Asserts |
|---|---|
| `tests/e2e/timer-overtime.spec.js` (NEW) — *Triple-Complete dispatch* | Continuous mode, focus, advance past zero (mock the engine to expose overtime). Click the left ghost slot → engine `complete` fired, session ends. Re-enter overtime, click the right ghost slot → same outcome. Re-enter, click the center filled slot → same outcome. (Maps to FR-007, FR-008, SC-002.) |
| `timer-overtime.spec.js` — *Orange tint visible* | In overtime, assert the three `.control-btn` elements carry `class*=overtime`, and the `.overtime-cta` element is visible with text "Wrap it up!" (or the localised string per the active locale). (FR-005, FR-010.) |
| `timer-overtime.spec.js` — *A11y removal of outer slots* | In overtime, assert `#stop-btn` has `aria-hidden="true"` and `tabindex="-1"`; same for `#skip-btn`. `#play-pause-btn` has `aria-label` containing the localised `timer.ctrl_complete_aria` text and `tabindex="0"`. Selector-based assertions (not role-based) — see risk register item 2. (FR-014, FR-015, FR-016, SC-003, SC-004.) |
| `timer-overtime.spec.js` — *Exit via Complete clears the treatment* | After any of the three Complete clicks, assert `.control-btn` elements no longer carry `class*=overtime`, `.overtime-cta` is not visible, and the timer view shows the next mode's normal treatment (post-cadence Break or LongBreak). (FR-024, SC-009.) |
| `timer-overtime.spec.js` — *Exit via Abort keyboard clears the treatment* | Bind a test shortcut for Abort (mock `register_global_shortcuts`), enter overtime, emit the `global-shortcut` event with payload `"abort"` (the listener receives), assert the engine returns to idle in the current focus mode, the overtime treatment is gone, the CTA is gone. (FR-021, SC-005, SC-009.) |
| `timer-overtime.spec.js` — *Pause during overtime reverts to Paused matrix* | In overtime, click the center filled button … wait — the center IS Complete in overtime. The way to reach Paused-during-overtime is: pause via global shortcut (start_stop) DURING overtime. After the pause, assert the matrix shows `✕ Abort \| ▶ Resume \| ✓ Complete` (the feature-006 Paused trio), the CTA is hidden, the `.control-btn.overtime` classes are gone. Resume → overtime treatment returns. (FR-022, FR-023.) |
| `tests/e2e/settings-shortcuts.spec.js` (EDIT) — *Fourth-row Abort* | Recording widget at `#abort-shortcut` accepts a binding, the binding persists across a settings reload, the binding is included in the `register_global_shortcuts` payload on save. (FR-018, FR-019, FR-020, SC-010.) |

**Mock-first sequencing.** The mock at `tests/e2e/fixtures/tauriMock.js` is already payload-agnostic for `register_global_shortcuts` (line 127). The `global-shortcut` event channel needs a per-spec emit helper so the e2e test can simulate the Abort shortcut firing. If the helper does not exist (check `tauriMock.js` for `emit("global-shortcut", "start_stop")` precedent — likely already in place from feature 006), no change. If absent, extend the mock first per Principle V's mock-first rule before writing the test.

## Constitution mapping

Only deviations + justifications. (Pass-affirmations omitted per standing constraint.)

No principle violations. No `Complexity Tracking` entries.

Notes:

- **Principle V scope is empty** because the engine is untouched. The three IPC round-trip tests above are wire-format contracts, not engine tests — they live in `crates/presto-ipc/src/settings.rs` `#[cfg(test)] mod tests`. RED-before-GREEN still applies to those three.
- **Principle VI** is honoured by reuse: no new Tauri command, no new event channel, no new IPC mechanism. The existing `register_global_shortcuts` widens its `ShortcutSettings` argument shape (additive, backwards-compatible at the serde layer). The mock absorbs the widened payload transparently — verified by reading `tauriMock.js:127` (the mock returns `Ok(())` without inspecting the payload).
- **Principle IV** is honoured with one new baseline + a one-line PR note. The dark-mode parity (SC-007) is covered by the pre-PR design QA loop, not by a second baseline — the CI reference is chromium-linux light only.

## CI / quality gates touched

- **Backend clippy + fmt** (per Principle X / `AGENTS.md`): new IPC field, settings panel arm, Tauri registration arm. All clear `cargo clippy --all-targets -- -D warnings -W clippy::pedantic` with zero new `#[allow]`.
- **Frontend wasm-bindgen-test**: the three IPC round-trip tests in `crates/presto-ipc/src/settings.rs` run under the existing test runner.
- **Playwright + VR**: new `timer-overtime.spec.js`, edited `settings-shortcuts.spec.js`, new baseline `timer-focus-continuous-overtime-chromium-linux.png`. Baseline cap unchanged (default cap suffices).
- **Pre-commit hook**: lockfile-drift check unchanged. **No new Cargo dependencies expected.** If a transitive bump is incidentally pulled in (unlikely — the changes are pure code), `Cargo.lock` lands in the same commit per Principle IX.
- **`.agentex.yml` pipeline**: no changes. The existing pipeline runs the new tests + VR suite.

## Migration / lockfile notes

- **No data migration.** The new `ShortcutSettings.abort` field defaults to `None` on first read of a pre-feature settings.json. The `Option` discriminant on a missing JSON key deserialises to `None` via `serde`'s default behaviour for `Option<T>`. No `#[serde(default)]` attribute needed (mirroring the existing three nullable fields).
- **No new Cargo dependencies.** Everything in tree.
- **i18n catalogue files** gain three new keys (`timer.overtime_cta`, `settings.shortcuts.label_abort`, `settings.shortcuts.desc_abort`). The typed-key compile-time check (feature 005) catches missing keys at build time.
- **Updater path** (Principle VII): existing presto users on the current release get the new `abort: None` value on first read; their existing three shortcut bindings round-trip unchanged. No back-compat work.

## Risk register

| Rank | Risk | Mitigation |
|---|---|---|
| 1 | **Listener was no-op; feature 007 implements the full four-arm dispatch.** The listener at `src/src/app.rs:613-624` was a no-op stub — feature 006 did not wire the existing three names. Feature 007 implements all four arms (`"start-stop"`, `"reset"`, `"skip"`, `"abort"`). Tests must verify each of the four arms triggers the right engine method via the actual `tauri:Manager.emit` path, not by direct engine call. | Four-arm dispatch in one atomic rewrite; e2e test exercises each arm via `mock.emit("global-shortcut", "<name>")`. |
| 2 | Shortcut registration churn — `register_global_shortcuts` unregisters all bindings before re-registering on every settings change. With four bindings, the unbind window widens slightly. | Add an e2e QA assertion: rapid Settings > Shortcuts edits do not observably degrade the other three bindings (test by triggering each binding immediately after an Abort-binding rebind). The existing three shortcuts already have this property; the risk is pre-existing and widened by one. |
| 3 | Playwright a11y assertion fragility — `aria-hidden="true"` removes elements from the accessibility tree. Playwright's `getByRole` and `locator.first()` semantics on role-based queries will not find the outer two `<button>` elements during overtime. Tests that assert "exactly one Complete button is in the accessibility tree" via role queries are correct; tests that assert specific DOM attributes must use DOM selectors. | The e2e spec uses **selector-based** assertions for the a11y test (`#stop-btn[aria-hidden=true]`, `#skip-btn[tabindex=-1]`) and **role-based** assertions for the SC-003 "user encounters exactly one Complete button" check (`page.getByRole('button', { name: <complete-aria-text> })`). The two assertion shapes are documented inline in `timer-overtime.spec.js`. |
| 4 | Smart-pause / auto-pause during overtime — `is_auto_paused` flips `RunState` to `Paused` per the existing `RunState::from_engine` mapping at `src/src/components/timer/mod.rs:230-242`. Overtime treatment then turns off (per FR-022, FR-023, and `[BEST-GUESS PM DECISION]` #7 in the spec). Risk: the user's intent during smart-pause-overtime is ambiguous — they were in overtime, then went idle (e.g., walked away), and smart-pause kicked in. The CTA disappears and the button row reverts to `Abort \| Resume \| Complete`. The user resumes and the overtime treatment returns. | Behaviour matches the spec (FR-023 + Edge Cases). RED test in `timer-overtime.spec.js` exercises this transition. The intent ambiguity is a UX truth, not a code bug. If post-feedback shows user confusion, a follow-up could keep the CTA visible during auto-pause-overtime — out of scope for this PR. |

## Best-guess decisions made / `[BEST-GUESS PM DECISION]` markers

1. **`[BEST-GUESS PM DECISION]`** FR-013's "internationalise the existing `(Overtime)` mode-pill suffix" is already 95% done in production code — `src/src/components/timer/mod.rs:1179` already uses `t_string!(i18n, timer.status_overtime)`, and the key already exists at `src/locales/en.json:172`. The remaining hard-coded literal at line 154 is in a `#[cfg(test)]` helper. This PR removes that test-helper literal (replacing it with the catalogue key) so the test helper does not silently desync from the catalogue if the localised string is later re-worded. — Rationale: the FR is satisfied by treating the test-helper update as a hygiene fix rather than a production-code change. If the PM intended the FR to call out an actual production-code defect, this is the place to flag it; we did not find one.
2. **`[BEST-GUESS PM DECISION]`** Paused-during-overtime falls back to the feature-006 Paused matrix (`Abort \| Resume \| Complete`) with the CTA hidden and the button orange tint cleared, even though the engine's `is_overtime` predicate remains true (the countdown's orange tint stays via the engine-level signal). — Rationale: spec FR-022 + the `[BEST-GUESS PM DECISION]` #7 in the spec already pin this. The plan formalises the matrix-and-CTA gate as `Running && is_overtime` (rather than `is_overtime` alone) — the countdown gate remains `is_overtime` alone.
3. **Center-button overtime gate is a single named closure.** Create `on_center_click` co-located with `on_play_pause` at `src/src/components/timer/mod.rs:1327-1349`. Body:
   ```rust
   let on_center_click = move |ev| {
       if is_overtime.get_untracked() && matches!(run_state.get_untracked(), RunState::Running) {
           on_complete(ev);
       } else {
           on_play_pause(ev);
       }
   };
   ```
   JSX binds `on:click=on_center_click` (line 2308). The 2D `(RunState, is_overtime)` matrix lives in ONE place — no JSX-level conditional wrapping. Engine dispatch is single-sourced. Anchored to **III. Type Safety Over Defensive Code**.
4. **`[BEST-GUESS PM DECISION]`** No carve-out is added to `.agentex.yml`'s baseline-cap. The default cap (3, from the feature 006 carve-out which expires when 006 merges) suffices: this feature adds 1 new baseline. — Rationale: read `.agentex.yml:67-79` — feature 006 widened the cap to 3 explicitly; under default conditions the cap returns to whatever was pre-006. If feature 006 has merged and the cap is back to the lower default, the 1-new-baseline addition is well under any reasonable cap. If the cap math doesn't work out at PR time, the carve-out is a one-line yaml edit.
5. **`[CONFIRMED]`** The listener at `src/src/app.rs:613-624` is a no-op stub — feature 006 did NOT wire the existing three names (`"start-stop"`, `"reset"`, `"skip"`). Feature 007 implements the full four-arm dispatch for all four names. The listener becomes a complete 4-arm `match` (not a 1-arm extension). Wire names are kebab-case throughout.

## Unresolved questions for the PM

(Soft — won't block tasks/implement.)

- Should the Settings > Shortcuts panel's Abort description include a hint that this is the "discard safety valve" during overtime? Currently the plan ships a neutral description (e.g., `"Discard the current focus session without logging it."`) without overtime framing — adding the overtime context to the description could help discoverability but might confuse users who set the binding for non-overtime discards.
- Should the overtime CTA element carry an `aria-live="polite"` attribute so screen readers announce "Wrap it up!" when overtime begins, or is the mode-pill suffix announcement (already i18n'd as `timer.status_overtime`) sufficient? Currently the plan does NOT add `aria-live` — the mode-pill change covers the announcement cleanly.
