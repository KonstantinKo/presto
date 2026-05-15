# Feature Specification: Overtime Button Treatment

**Feature Branch**: `007-overtime-button-treatment`
**Created**: 2026-05-15
**Status**: Draft
**Input**: User description: "Overtime Button Treatment - companion to feature 006 (state-aware timer controls). When focus session crosses zero into overtime (continuous mode only), all three buttons converge on Complete: center filled, outer two ghost, orange tint matching pulsating countdown. 'Wrap it up!' CTA visible. A11y exposes only center button. Existing global keyboard shortcut for Abort remains active. Exit overtime → buttons return to normal."

## Constitutional Anchors

This feature is governed by the project constitution at
[`.specify/memory/constitution.md`](../../.specify/memory/constitution.md). The
following principles bind the work:

- **I. Timer Is Sacred.** Every overtime Complete action MUST traverse the
  engine's existing `complete(clock)` path (branch B.2: the
  `session_completed_but_not_saved == true` flow). UI MUST NOT side-channel,
  skip, or simulate that path; the engine remains the single source of truth
  for what counts as a completed focus session.
- **III. Type Safety Over Defensive Code.** Overtime is a derived predicate
  (time remaining below zero in continuous mode), not a new run-state. The
  button matrix gains a new dimension layered onto the existing run-state, not
  a new enum variant. The illegal state — "overtime in non-continuous mode" —
  is impossible by derivation.
- **IV. Visual Regression Is The UI Contract.** A new baseline screenshot
  captures the three-orange-Complete layout with the "Wrap it up!" call-to-
  action. Baseline addition will require an explicit PR note per the standard
  baseline-update discipline.
- **V. Internationalisation Is Non-Negotiable.** The "Wrap it up!" call-to-
  action and the existing "(Overtime)" mode-pill suffix are catalogue keys in
  every supported locale. Hard-coded literals are out.
- **VI. Tauri Boundary Is Stable.** If the discard-safety-valve keyboard
  shortcut is bound globally, it MUST re-use the existing global-shortcut
  routing mechanism; no new Tauri command is introduced.
- **VIII. Spec-Driven Feature Flow.** Multi-file work (timer view, catalogue,
  potentially settings) → spec mandatory.
- **X. Pedantic Linting.** All new Rust code clears
  `cargo clippy --all-targets -- -D warnings -W clippy::pedantic`.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Gentle nudge out of overtime (Priority: P1)

A user runs a focus session in **continuous mode** (the mode where the timer
keeps running past zero instead of auto-stopping). Fourteen minutes after the
bell, they realise they are still working and need to wrap up. The button row
at the bottom of the timer view, which during normal operation offers three
different controls, collapses visually: every slot now says **✓ Complete**, all
tinted the same overtime orange as the pulsating countdown above. The centre
button is filled and prominent; the two outer buttons are ghost-styled and
visually subordinate. A small text line — **"Wrap it up!"** — sits between the
countdown and the buttons. The user clicks any of the three buttons and the
session ends, the focus time (including the overtime minutes) is logged, and
the timer advances to the configured break.

**Why this priority**: This is the entire point of the feature. Overtime is a
moment of low cognitive bandwidth — the user has already drifted past the
intended endpoint. The treatment removes ambiguity ("which button do I want?")
and replaces it with a single obvious action presented three times over. It
turns the moment from a UX choice into a one-click exit.

**Independent Test**: Start a continuous-mode focus session, advance the clock
past zero, and observe that all three button slots show the orange Complete
treatment with the "Wrap it up!" call-to-action visible. Click any slot and
verify the session ends and the break begins.

**Acceptance Scenarios**:

1. **Given** the user is in a running continuous-mode focus session at five
   minutes past zero, **When** they look at the timer view, **Then** all three
   button slots display the **✓ Complete** label, the centre slot is filled
   and saturated orange, the outer two slots are ghost-styled in the same
   orange at standard ghost opacity, and the text "Wrap it up!" is visible
   between the countdown and the button row.
2. **Given** the overtime button treatment is on screen, **When** the user
   clicks the centre slot, **Then** the focus session is recorded with the
   actual elapsed time (planned focus duration plus the overtime minutes), the
   timer advances to the next mode according to the configured long-break
   cadence, and the orange treatment disappears.
3. **Given** the overtime button treatment is on screen, **When** the user
   clicks either of the outer (ghost) slots, **Then** the behaviour is
   identical to clicking the centre slot — same engine transition, same
   logged outcome.
4. **Given** the user is in a running continuous-mode focus session at exactly
   the moment the timer crosses zero, **When** the bell sounds, **Then** the
   button row transitions from the normal Running treatment to the overtime
   treatment within the same UI tick that the countdown turns orange.
5. **Given** the user clicks the centre slot to end the overtime session,
   **When** the next mode (a break) begins, **Then** the "Wrap it up!" text
   is no longer visible and the button row displays the normal Running
   treatment for the new mode.

---

### User Story 2 — No accidental discard during the wrap-up moment (Priority: P2)

The same user, mid-overtime, realises they actually want to discard the
session entirely instead of logging it — for example, they got pulled into an
unrelated conversation and the elapsed time isn't representative of any real
focus work. The on-screen button row offers only Complete in three flavours,
because that's the gentle nudge of P1. The user reaches for the keyboard:
their **Abort** action is bound to a keyboard shortcut, the shortcut still
works during overtime, and pressing it discards the session and returns the
timer to the idle state of the current focus mode. The button row reverts to
its normal idle treatment.

**Why this priority**: Discarding from overtime should be possible — denying
it would be paternalistic — but it should not be the path of least resistance.
A keyboard-only discard path satisfies "reachable" without cluttering the
wrap-up moment with a giant red on-screen button that competes with the
Complete nudge.

**Independent Test**: With the overtime treatment on screen, trigger the
Abort keyboard shortcut and verify the session is discarded, the timer
returns to idle, and the orange overtime treatment is gone.

**Acceptance Scenarios**:

1. **Given** the user is in continuous-mode overtime with the orange Complete
   treatment on screen, **When** they trigger the Abort keyboard shortcut,
   **Then** the focus session is discarded (not counted as completed), the
   timer returns to the idle state of the current focus mode, the orange
   button treatment disappears, and the "Wrap it up!" call-to-action is no
   longer visible.
2. **Given** the user has just installed or upgraded the app and has not
   personally bound the Abort shortcut, **When** they enter overtime, **Then**
   the Settings panel offers Abort as a bindable shortcut so the user can
   assign one if they want a keyboard discard path.
3. **Given** the user has bound the Abort shortcut and quits the app, **When**
   they re-open the app, **Then** the Abort binding persists.

---

### Edge Cases

- **Pause during overtime.** If the user pauses the timer while it is in
  overtime (whether via a keyboard shortcut or by some other means available
  in continuous mode), the button row reverts to the normal Paused treatment
  (Abort | Resume | Complete) and the orange overtime treatment is suspended.
  Rationale: paused timer has no urgency; the wrap-up nudge no longer applies.
  The countdown's orange tint behaviour follows the existing engine signal
  and is not changed by this feature. Resuming returns to the overtime
  treatment if time remaining is still below zero.
- **Smart-pause auto-pause activates during overtime.** Same as above —
  auto-pause folds into Paused, the button matrix follows.
- **Abort via the keyboard during overtime.** The session is discarded, the
  timer returns to idle in the current focus mode, the overtime treatment
  and "Wrap it up!" call-to-action both disappear within the same UI tick.
- **All three Complete buttons produce identical engine state.** The three
  button slots share a single Complete handler; there is exactly one path
  through the engine for the overtime end.
- **The "Wrap it up!" call-to-action is bound strictly to overtime state.**
  It appears the moment the timer crosses zero in continuous-mode focus and
  disappears the moment the session ends (by any means) or the timer pauses.
- **Long-break cadence after overtime Complete.** The engine's existing
  cadence logic determines whether the next break is short or long; this
  feature does not alter that decision.
- **Settings > Shortcuts persistence.** The Abort shortcut binding (if added)
  persists across app restarts using the same storage as the existing
  shortcut bindings.
- **Screen readers during overtime.** Only the centre Complete button is
  announced; the outer two are excluded from the accessibility tree and
  tab order. Pressing Tab on the timer view skips them.

## Requirements *(mandatory)*

### Functional Requirements

**Trigger and scope**

- **FR-001**: The system MUST apply the overtime button treatment when, and
  only when, a focus session is running in continuous mode and the time
  remaining has crossed zero.
- **FR-002**: The system MUST NOT apply the overtime button treatment in any
  other state — not during a short break, not during a long break, not in
  idle, not in any non-continuous focus mode, not while the timer is paused.

**Button layout and labels**

- **FR-003**: During the overtime treatment, all three button slots in the
  bottom row of the timer view MUST display the **✓ Complete** label,
  identical to the standard Complete button label used elsewhere.
- **FR-004**: During the overtime treatment, the centre button slot MUST be
  rendered as the primary (filled) button visual, and the outer two slots
  MUST be rendered as ghost (secondary, lower-emphasis) button visuals.
- **FR-005**: During the overtime treatment, the colour of the centre slot
  MUST match the overtime warning colour (the same colour used by the
  pulsating countdown above). The outer two slots MUST use the same warning
  colour at the standard ghost opacity used elsewhere in the design system.
- **FR-006**: Light-mode and dark-mode overtime button colours MUST match the
  light-mode and dark-mode overtime countdown colours respectively, so that
  the buttons and countdown read as a single visual unit.

**Button behaviour**

- **FR-007**: Clicking any of the three button slots during the overtime
  treatment MUST trigger exactly the same outcome: the focus session is
  recorded as completed with the actual elapsed time including overtime, and
  the timer advances to the next mode per the configured long-break cadence.
- **FR-008**: The overtime Complete action MUST go through the engine's
  existing completion path; the UI MUST NOT bypass the engine to alter
  totals, advance modes, or emit completion events directly. (Constitutional
  Anchor I.)
- **FR-009**: The overtime Complete action MUST NOT double-count the focus
  session in the completed-sessions tally; the engine already increments the
  tally on the zero-crossing in continuous mode, and the wrap-up Complete
  finalises elapsed time without re-incrementing.

**Call-to-action text**

- **FR-010**: During the overtime treatment, the system MUST display a small
  text line reading **"Wrap it up!"** (exactly three words, exactly that
  punctuation) between the countdown and the button row, centred horizontally
  on the timer view, tinted with the same overtime warning colour as the
  countdown and buttons.
- **FR-011**: The "Wrap it up!" call-to-action MUST appear at the same moment
  the overtime button treatment appears and MUST disappear at the same
  moment the overtime button treatment disappears (for any reason: session
  completed, session aborted, timer paused).
- **FR-012**: The "Wrap it up!" call-to-action MUST be translated in every
  supported locale (English default plus all other locales the app currently
  ships) via a new catalogue key. A fresh string in a locale lacking a
  professional translation MAY ship as a good-faith translation following
  the precedent established by feature 005.
- **FR-013**: The system MUST internationalise the existing "(Overtime)"
  suffix on the mode pill at the top of the timer view as part of this
  feature, replacing any hard-coded literal with a catalogue key in every
  supported locale.

**Accessibility**

- **FR-014**: During the overtime treatment, only the centre Complete button
  MUST be reachable by assistive technologies (screen readers, switch
  control, voice control). The outer two button slots MUST be hidden from
  the accessibility tree.
- **FR-015**: During the overtime treatment, only the centre Complete button
  MUST be reachable by keyboard tab navigation; the outer two slots MUST be
  excluded from the tab order.
- **FR-016**: The centre Complete button during the overtime treatment MUST
  carry the same accessible label as the Complete button used elsewhere in
  the app ("Complete the current session and advance" — re-using the
  existing catalogue key).

**Discard safety valve**

- **FR-017**: The user MUST have a keyboard-accessible path to discard the
  focus session during overtime. The discard path MUST be reachable without
  an on-screen button.
- **FR-018**: The system MUST extend the existing global keyboard shortcut
  mechanism to include an **Abort** action alongside the currently-bindable
  start-stop, reset, and skip actions. The Settings > Shortcuts panel MUST
  expose Abort as a fourth bindable row.
- **FR-019**: The Abort shortcut MUST default to unbound, matching the
  existing convention for user-configurable shortcuts. The user MUST be
  able to bind it to a key combination of their choice from the Settings
  panel.
- **FR-020**: The Abort shortcut binding, once set by the user, MUST persist
  across app restarts using the same storage mechanism as the other
  bindable shortcuts.
- **FR-021**: While the overtime treatment is on screen, the Abort
  keyboard shortcut (if bound) MUST remain active and functional. Triggering
  it MUST discard the focus session and return the timer to the idle state
  of the current focus mode.

**Pause interaction**

- **FR-022**: If the user pauses the timer while in overtime, the button row
  MUST revert to the normal Paused treatment (Abort | Resume | Complete) and
  the "Wrap it up!" call-to-action MUST disappear. Resuming with time
  remaining still below zero MUST restore the overtime treatment.
- **FR-023**: If the smart-pause auto-pause feature activates during
  overtime, the system MUST treat the resulting state identically to a
  manual pause for the purposes of the button matrix and call-to-action
  visibility.

**Exit**

- **FR-024**: When the focus session ends (by any of the three Complete
  buttons or by the Abort shortcut), the button row MUST return to its
  normal treatment for the resulting mode (the configured break after
  Complete, or the idle focus mode after Abort) and the "Wrap it up!"
  call-to-action MUST be hidden.

### Key Entities

- **Overtime predicate**: A derived condition (true / false). True when the
  timer is in continuous-mode focus AND time remaining is below zero. This
  predicate drives the button matrix dimension, the call-to-action visibility,
  and the overtime colour treatment. It is not a stored field; it is computed
  from the existing engine state.
- **Button-row state**: The set of (label, visual emphasis, action) triples
  rendered in the three bottom slots. Determined by the combination of the
  existing run-state and the overtime predicate. In overtime, all three
  triples collapse to a single action (Complete).
- **Abort shortcut binding**: A new entry in the user's shortcut preferences.
  Stored alongside the existing three bindings. Carries an optional key
  combination; defaults to none.
- **Wrap-up call-to-action**: A short, locale-aware text line bound to the
  overtime predicate's visibility.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: When a continuous-mode focus session crosses zero, the orange
  three-Complete button treatment and the "Wrap it up!" call-to-action become
  visible within the same UI tick as the countdown's overtime colour change
  — no perceptible delay between the countdown going orange and the buttons
  going orange.
- **SC-002**: 100 % of click interactions on any of the three button slots
  during overtime produce identical engine state transitions (same completed-
  session tally, same total focus time logged, same next-mode advance). This
  is the user-facing expression of the "single Complete path" requirement.
- **SC-003**: With a screen reader active during overtime, the user
  encounters exactly one Complete button when traversing the timer view; the
  outer two slots are silent.
- **SC-004**: With keyboard-only navigation during overtime, the user can
  tab to exactly one Complete button on the timer view; tabbing past it does
  not land on the outer two slots.
- **SC-005**: A user who has bound the Abort shortcut can discard a focus
  session during overtime in a single keystroke (one shortcut press), with
  no on-screen Abort button visible.
- **SC-006**: The "Wrap it up!" text is never visible outside of overtime
  state; it appears and disappears synchronously with the overtime button
  treatment in 100 % of state transitions tested.
- **SC-007**: The full overtime treatment (countdown colour, button row,
  call-to-action) renders identically in light mode and dark mode, with the
  light-mode and dark-mode warning colours matched between countdown,
  buttons, and call-to-action.
- **SC-008**: Every visible string introduced or modified by this feature
  (the "Wrap it up!" call-to-action and the "(Overtime)" mode-pill suffix)
  renders from a catalogue key in every supported locale; no hard-coded
  literal remains in the timer view's overtime rendering path.
- **SC-009**: Exiting overtime by any of the four paths (centre Complete,
  left-ghost Complete, right-ghost Complete, Abort shortcut) returns the
  timer view to its non-overtime treatment within the same UI tick as the
  underlying state change.
- **SC-010**: The new Abort shortcut row appears in Settings > Shortcuts
  in 100 % of locales, with its label and description translated, and the
  bound key persists across at least one app restart.

## Assumptions

- **`[BEST-GUESS PM DECISION]`**: The discard safety valve is satisfied by
  adding **Abort** as a fourth bindable global keyboard shortcut alongside
  the existing three (start-stop, reset, skip). Rationale: the original
  feature brief assumed an "existing global keyboard shortcut for Abort"
  remains active, but no such global shortcut currently exists. Extending
  the existing Settings > Shortcuts panel by one row is the smallest delta
  that satisfies the brief's intent. Default binding is unbound, matching
  the existing pattern.
- **`[BEST-GUESS PM DECISION]`**: The "Wrap it up!" call-to-action is placed
  between the pulsating countdown and the button row, centred horizontally,
  small (treat as a subtle hint, not a banner), in the overtime warning
  colour. The placement matches the visual hierarchy implied by the brief
  (countdown → CTA → button row).
- **`[BEST-GUESS PM DECISION]`**: The existing hard-coded "(Overtime)"
  suffix on the mode pill is internationalised as part of this feature.
  Rationale: the feature already touches the catalogue for the new
  call-to-action, and shipping a partially-internationalised overtime UI
  would violate the project's i18n discipline. This is bundled as a small
  hygiene fix.
- **`[BEST-GUESS PM DECISION]`**: The button matrix gains a new dimension
  (run-state × overtime predicate) rather than a new run-state variant. The
  user-visible behaviour is identical either way; the choice is a structural
  one motivated by the project's "illegal state impossible" discipline. It
  is documented in the spec because it determines how subsequent edits to
  the button matrix will be reasoned about. (Constitutional Anchor III.)
- **`[BEST-GUESS PM DECISION]`**: The outer two button slots during overtime
  use the accessibility-tree-removal pattern of `aria-hidden` plus removal
  from the tab order. The centre slot keeps the standard accessible label
  used elsewhere for Complete actions. This matches the existing tab-order-
  removal pattern used in the Settings panels.
- **`[BEST-GUESS PM DECISION]`**: The "Wrap it up!" call-to-action's
  visibility is bound directly to the overtime predicate; it has no separate
  state. Visible when overtime is true, hidden when overtime is false.
- **`[BEST-GUESS PM DECISION]`**: Overtime treatment does **not** apply when
  the timer is paused, even if the timer is technically still in continuous-
  mode focus and time remaining is still below zero. Rationale: the
  wrap-up nudge is about urgency; a paused timer has no urgency. The button
  row in this state reverts to the normal Paused treatment (Abort | Resume |
  Complete) and the call-to-action is hidden.
- All visual treatments described in this spec (saturated centre, ghost
  outer slots, warning colour) re-use existing design-system tokens; no new
  colour values are introduced.
- The keyboard discard path's binding key is the user's choice; this spec
  does not prescribe a default key combination.
- This feature does not change the engine's behaviour when the timer crosses
  zero in continuous mode; the engine's existing event emission and tally
  increment on zero-cross are unchanged. Only the UI treatment after the
  zero-cross is changed.
- This feature does not introduce animations for the button transition;
  transitions are visual state swaps. Animation work is out of scope.
- This feature does not introduce a confirmation modal on the overtime
  Complete action; clicking any of the three slots ends the session
  directly.

## Out of Scope

- Overtime treatment in any state other than continuous-mode focus past zero.
- Animations for the button-row transition into and out of overtime.
- A confirmation modal on the overtime Complete action.
- Changes to the engine's zero-crossing behaviour (event emission, tally
  increment, mode determination).
- A new Tauri command for the Abort shortcut — the existing global-shortcut
  routing carries it.
- Default key bindings for the Abort shortcut.
- Changes to the long-break cadence logic.
- Visual changes outside the timer view's button row, countdown, and
  mode-pill suffix.
