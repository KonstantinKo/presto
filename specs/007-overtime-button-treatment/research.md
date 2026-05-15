# Research — Overtime Button Treatment

> Phase 0. Only **irreversible** decisions are captured. Trivially reversible ones (catalogue wording, CSS-modifier name, e2e selector cosmetics) are not.

## Decision 1 — `ShortcutSettings` field shape

**Decision**: Add a fourth optional field `abort: Option<String>` to `crates/presto-ipc/src/settings.rs:113-127`. Mirror the wire shape of the existing three (`start_stop`, `reset`, `skip`).

**Why irreversible**: changes the on-disk JSON schema for `settings.json → shortcuts.{}`. Once shipped, a user's settings file may carry the `abort` key; renaming or removing it post-ship breaks the user's binding. Adding a sibling field next to it later would be additive — fine — but renaming is not.

**Alternatives considered**:
- Sub-object `shortcuts.advanced.abort` — rejected; the existing three live at the top level of `shortcuts.{}`; nesting just `abort` would be asymmetric churn.
- A separate top-level `discard_shortcut` field — rejected; `Settings > Shortcuts` panel groups all shortcut bindings, so the data shape should group them too.

**Anchored to**: FR-018, FR-019, FR-020, Principle III (closed type), Principle VI (no new IPC mechanism).

## Decision 2 — No new Tauri command for Abort

**Decision**: Widen the existing `register_global_shortcuts` command (`src-tauri/src/lib.rs:432-473`) to register the Abort binding alongside the existing three. Carry the `"abort"` payload name on the existing `global-shortcut` event channel.

**Why irreversible**: the choice between "extend the existing command" vs. "add a sibling command for Abort registration" pins the contract. The existing command takes the full `ShortcutSettings` struct and re-registers all bindings on each call. A sibling command (e.g., `register_abort_shortcut`) would partial-register, complicating the unregister-then-re-register dance. The full-rebuild semantics are part of the contract — splitting them changes the contract.

**Alternatives considered**:
- New `register_abort_shortcut` command — rejected; doubles the contract surface; complicates the unregister-all-then-re-register-all flow.
- Inline registration on settings save without a command — rejected; the Tauri-side plugin lives behind the `register_global_shortcuts` command for a reason (centralised debounce, error propagation via `BridgeError`).

**Anchored to**: Principle VI (Tauri boundary is stable — no new IPC mechanism), spec § "Out of Scope: A new Tauri command for the Abort shortcut".

## Decision 3 — A11y removal via `tabindex=-1` + `aria-hidden=true`

**Decision**: During overtime, the outer two button slots carry `aria-hidden="true"` and `tabindex="-1"`. The center slot keeps standard tab order and `aria-label`.

**Why irreversible**: alternative a11y patterns (`disabled`, `inert`, `visibility: hidden`) each have different side effects on click handling, focus visibility, and screen-reader announcement. The chosen pattern preserves click handling on the outer slots (mouse + touch still work — the slots remain clickable Complete buttons) while removing them from assistive-tech surfaces. The precedent at `src/src/components/settings/theme.rs:217` confirms the pattern is already in tree.

**Alternatives considered**:
- `disabled` — rejected; disables click handling, which violates FR-007 (clicking any of the three slots completes the session).
- `inert` attribute — rejected; removes click handling AND focus, equivalent to `disabled` for our purposes; also less broadly supported on older Tauri-bundled webview versions.
- `visibility: hidden` — rejected; removes from layout AND visual flow — defeats the "three orange buttons visually present, one accessibility-tree-reachable" UX.

**Anchored to**: FR-014, FR-015, SC-003, SC-004, Principle III (illegal state — "Complete reachable to screen reader but not to mouse" — is impossible by construction).

## Decision 4 — Reuse `--warning-color` (no new CSS variables)

**Decision**: The overtime button tint reuses the existing `--warning-color` CSS variable (`src/style/variables.css:22,48,72` — light `#e67e22`, dark `#f59e0b`). No new variables added.

**Why irreversible**: introducing a new variable (`--overtime-button-color` etc.) and later finding it should have been `--warning-color` is a CSS-cleanup task with cross-baseline consequences. Reusing the existing variable from day one anchors the visual contract to a single token.

**Alternatives considered**:
- New `--overtime-button-bg` and `--overtime-button-fg` variables — rejected; over-tokenisation for one button state.
- Hard-coded color values in `timer.css` — rejected; violates the design-system discipline (and the spec's "All visual treatments described in this spec re-use existing design-system tokens; no new colour values are introduced" assumption).

**Anchored to**: FR-005, FR-006, SC-007, spec Assumptions ("re-use existing design-system tokens").

## Decision 5 — `is_overtime` is the single source of truth for "are we in overtime?"

**Decision**: The existing `is_overtime` derived signal at `src/src/components/timer/mod.rs:1130` is reused unchanged. All overtime-treatment bindings (CTA visibility, button class:overtime, button label/icon flip, a11y attributes) project off this signal.

**Why irreversible**: alternative — introducing a parallel signal (e.g., `is_overtime_ui_treatment_active`) gated on `Running && is_overtime` — splits the source of truth. The Running gate belongs at each consumer site (not at the signal level) because the countdown's orange tint uses the un-gated signal (countdown stays orange in Paused-during-overtime per Edge Cases), while the button matrix uses the Running-gated form. Centralising the Running gate would couple consumers that should be independent.

**Alternatives considered**:
- Introduce `is_overtime_treatment` = `Running && is_overtime` and use it everywhere — rejected; couples the countdown gate to the button gate.
- Push the gate into a new `RunState::Overtime` variant — rejected; explicitly out of scope per spec FR + Principle III ("overtime is a derived predicate, not a new run-state").

**Anchored to**: FR-001, FR-002, FR-022, Principle III, spec Constitutional Anchor III, spec Assumption "the button matrix gains a new dimension … rather than a new run-state variant".

## Out of scope for research

- Engine changes — feature 006 already shipped the branch-B.2 path.
- New CSS animations — explicitly out of scope per spec.
- Default key binding for Abort — explicitly out of scope per spec.
