# Feature Specification: Opt-In Ambient Background Sounds During Focus

**Feature Branch**: `004-ambient-sounds`
**Created**: 2026-05-13
**Status**: Draft
**Input**: User description: "Opt-in ambient background sounds during focus sessions. User toggles 'Enable ambient background sound' in Settings → Notifications, picks one of a vendored set (rain, fire, library, fan, storm, white-noise, wind), adjusts volume (0–100, default 50), and the sound loops while a focus session is running. Default off. Mirrors the metronome lifecycle from feature 002 (Bundle C) — UI-side side effect in the timer component's tick loop; engine remains pure. Vendored assets, no CDN. Decoupled from metronome and notification chimes."

## Clarifications resolved

- 2026-05-13: Ambient sound plays only while `time_remaining > 0` (no playback in overtime). Mirrors feature 002 Bundle C's metronome gate at `src/src/components/timer/mod.rs:1358-1366` (`metronome && Focus && running && !paused && !auto_paused && time_remaining > 0`). Overtime is a soft continuation where the prescriptive pacing/atmosphere role of an external audio cue ends — the timer is no longer "on the clock" in the user's mental model.
- 2026-05-13: Ambient sound auto-resumes on continuous-sessions auto-start of a fresh focus session. The gate is "focus mode is active and ticking", not "user pressed start" — Principle I treats engine state identically regardless of trigger (same posture as metronome in feature 002, Assumption A8).
- 2026-05-13: `None` is a first-class enum variant ("no track selected"), not a string sentinel. Selecting `None` is equivalent to disabling the feature for playback purposes but preserves the user's prior volume setting, so toggling the feature off-then-on (or picking `None` then a real track) does not reset the volume slider.
- 2026-05-13: Track changes while playing cross-fade (300 ms fade-out of the old track + 300 ms fade-in of the new track, started simultaneously). Volume changes apply live to the active track without re-starting it.
- 2026-05-13: Pause / smart-pause / mode-transition events fade the active track out over 200 ms (no jarring cut). Resume fades back in over 200 ms. Settings toggle-off and settings track-change-to-`None` use the same 200 ms fade-out.
- 2026-05-13: The three audio surfaces (notification chime, metronome, ambient sound) coexist. They do not share an audio-graph instance, do not compete for a single decoded buffer, and are volume-independent. Volume changes on the ambient slider do not affect chime or metronome amplitude.

## User Scenarios & Testing *(mandatory)*

> This feature is **one user-facing capability** with one underlying integrity guarantee (legacy settings-file compatibility) and one cross-cutting PR-time discipline (visual-regression budget). Constitutional anchors are cited inline by name **and** number, mirroring the spec 002 / spec 003 precedent.

### User Story 1 - Looping a chosen ambient sound during focus (Priority: P1)

A user opens Settings → Notifications, ticks **"Enable ambient background sound"**, picks **Rain** from a track dropdown, and drags the volume slider to a comfortable level (e.g., 30/100). They close Settings and start a focus session on the timer screen. The chosen rain loop fades in and plays continuously for the duration of the focus session. The sound fades out on pause; fades back in on resume. It fades out on focus → break / long break transition and stays silent through the break. When the next focus session begins (manual start or continuous-sessions auto-start), the rain loop fades back in.

**Why this priority**: This is the entire user-facing value of the feature. It is a single capability — opt-in atmospheric audio paired to focus mode. Tied to **I. The Timer Is Sacred** (ambient playback is a UI-side side effect of the timer component's tick loop and state-transition events, mirroring the metronome at `src/src/components/timer/mod.rs:1358-1368`; the engine emits state, the UI consumes; no `web_sys` imports added to `src/src/engine/`); **II. Local-Only** (audio assets are vendored locally — no CDN, no network egress, no telemetry); **III. Type Safety Over Defensive Code** (`AmbientSoundType` is a closed sum-type enum with eight variants — `None`, `Rain`, `Fire`, `Library`, `Fan`, `Storm`, `WhiteNoise`, `Wind` — never a string; volume is `u32` clamped at the Settings UI input boundary).

**Independent Test**: With ambient sound disabled (default), start a focus session and assert no ambient audio playback occurs. Enable the feature, pick `Rain`, set volume to 50, start a fresh focus session, and assert (in a host that can observe audio playback — Tauri-mock setup or wasm-bindgen-test with a stub) that a loop tied to the `rain` asset begins playing within 200 ms of focus start. Pause the session; assert playback fades out within ~200 ms. Resume; assert playback fades back in. Skip to break; assert playback fades out and does not resume during the break. Start the next focus session; assert playback fades back in for the new focus.

**Acceptance Scenarios**:

1. **Given** Settings → Notifications with "Enable ambient background sound" off (the default), **When** the user starts and runs a focus session, **Then** no ambient audio plays — only the existing chime on transitions and (if separately enabled) the metronome tick, as today.
2. **Given** "Enable ambient background sound" toggled on, track set to `Rain`, volume `50`, **When** a focus session is running and not paused / auto-paused / in overtime, **Then** the rain loop plays continuously, looping seamlessly enough for ambient (loop seams are acceptable — see Edge Cases).
3. **Given** an ambient loop playing during focus, **When** the user pauses the session (user-initiated pause OR smart-pause auto-pause), **Then** the loop fades out over ~200 ms and remains silent until the session resumes.
4. **Given** an ambient loop playing during focus, **When** the focus session zero-crosses into `Break` or `LongBreak`, **Then** the loop fades out over ~200 ms at the transition and stays silent during the break, long break, and the gap between sessions.
5. **Given** an ambient loop playing during focus, **When** `time_remaining` reaches 0 and the session enters overtime (allow-continuous-sessions extends the open session past the clock), **Then** the loop fades out at the zero-cross — overtime is a non-counted continuation and is not a focus continuation for ambient-sound purposes.
6. **Given** the user is in Settings with the feature on, **When** they pick a different track (e.g., `Rain` → `Fire`) **while** a focus session is actively playing the old track, **Then** the active track fades out over 300 ms and the new track fades in over 300 ms simultaneously (cross-fade) — no full silent gap, no abrupt cut.
7. **Given** an ambient loop playing during focus, **When** the user drags the volume slider in Settings, **Then** the active track's output amplitude follows the slider live — no re-start, no fade artefact.
8. **Given** an ambient loop playing during focus, **When** the user toggles "Enable ambient background sound" off in Settings, **Then** the loop fades out over ~200 ms and no further playback occurs until the toggle is on AND a real track is selected AND focus is running.
9. **Given** an ambient loop playing during focus, **When** the user changes the track selector to `None`, **Then** the active track fades out over ~200 ms and no other track fades in — `None` is equivalent to "no track to play" while preserving the user's prior volume slider value.
10. **Given** continuous-sessions enabled and an ambient track configured, **When** the engine auto-starts a fresh focus session immediately after a break, **Then** the ambient track fades in for the new focus session without requiring the user to press Start (gate is "focus mode is active and ticking", not "user pressed start" — matches the metronome auto-resume posture from feature 002).

---

### User Story 2 - Legacy settings load unchanged after the new fields are added (Priority: P1)

A user with a pre-feature-004 `settings.json` opens the new build. The settings file deserialises cleanly — no migration prompt, no error state, no data loss. `ambient_sound_enabled` defaults to `false`, `ambient_sound_type` defaults to `None`, `ambient_sound_volume` defaults to `50`. Pre-feature-004 users hear no change unless they opt in.

**Why this priority**: Underlying integrity guarantee that makes Story 1 safe to ship — evolving the `NotificationSettings` shape would otherwise be a data-loss / first-launch-error hazard. Tied to **VI. The Tauri Boundary Is Stable** (each new field carries `#[serde(default)]`, mirroring the metronome field at `crates/presto-ipc/src/settings.rs:185-186` so older on-disk records deserialise) and **III. Type Safety Over Defensive Code** (a closed enum `AmbientSoundType` with a `None` variant is the type-system encoding of "no track selected" — no string sentinel like `""` or `"none"` on the wire other than the enum's own kebab-case serialisation).

**Independent Test**: Seed `settings.json` whose `notifications` block lacks `ambient_sound_enabled`, `ambient_sound_type`, and `ambient_sound_volume` keys entirely. Open the new build. The settings deserialise successfully; the Settings → Notifications tab shows the checkbox unchecked, the track dropdown selected to `None`, the volume slider at `50`. No first-run prompt, no error toast, no settings-tab error state.

**Acceptance Scenarios**:

1. **Given** a pre-feature-004 `settings.json` whose `notifications` block lacks `ambient_sound_enabled`, **When** the new build reads the file, **Then** the field deserialises to `false`.
2. **Given** the same fixture whose `notifications` block lacks `ambient_sound_type`, **When** the new build reads the file, **Then** the field deserialises to `AmbientSoundType::None`.
3. **Given** the same fixture whose `notifications` block lacks `ambient_sound_volume`, **When** the new build reads the file, **Then** the field deserialises to `50`.
4. **Given** a new-build `notifications` record persisted with the defaults (off / `None` / 50), **When** it is re-serialised to disk and re-read, **Then** the round-trip preserves all three values byte-stable.
5. **Given** a new-build `notifications` record with the feature enabled (`true` / `Rain` / `30`), **When** it is re-serialised and re-read, **Then** the round-trip preserves all three values; the on-disk JSON encodes `ambient_sound_type` as a kebab-case string (`"rain"` / `"white-noise"` / etc.) matching the existing `StatusBarDisplay` precedent at `crates/presto-ipc/src/settings.rs:26-35`.
6. **Given** a pre-feature-004 `settings.json` already containing the `metronome` field (feature 002 baseline), **When** the new build reads it and writes it back, **Then** the existing `metronome` field is preserved byte-stable — the new feature's serde-evolution does not corrupt the prior feature's fields.

---

### User Story 3 - Visual regression baselines are updated with explicit per-baseline justification (cross-cutting, Priority: P2)

A PR reviewer opens the visual regression diff. Exactly one baseline — the Settings → Notifications tab — is regenerated, carrying a one-line PR-description note ("settings-notifications: ambient-sound checkbox, track dropdown, and volume slider added below the metronome row"). No baseline outside Settings → Notifications is regenerated. The timer screen baseline does **not** change (the feature has no on-screen affordance on the timer itself — playback is silent-by-default and the visible chrome lives entirely in Settings).

**Why this priority**: Integrity guarantee that the UI surface is honestly accounted for, not silenced. P2 because it is PR-time discipline, not runtime behaviour. Tied to **IV. Visual Regression Is The UI Contract** — baselines are the UI contract, and changes carry per-baseline justification notes. Mirrors feature 002 Story 6 and feature 003 CHK040's posture.

**Independent Test**: Run the visual regression suite. Confirm the failing baseline maps exactly to `settings-notifications-chromium-linux.png` (the Settings → Notifications tab). Confirm no baselines for untouched screens (timer, statistics, daily, tasks list, tag manager) flag a diff.

**Acceptance Scenarios**:

1. **Given** the feature's PR ready for review, **When** the visual regression suite runs, **Then** the only baseline that flags a diff is `settings-notifications-chromium-linux.png`.
2. **Given** the regenerated baseline, **When** the PR description is read, **Then** the baseline has a one-line note explaining the intended visual change. No bare PNG diff lands without prose.
3. **Given** any baseline outside Settings → Notifications flagging a diff, **When** the reviewer sees the failure, **Then** the diff is treated as a regression (fix the code) — not absorbed by regenerating the baseline.

---

### Edge Cases

- **Loop seam silence**: HTML5 `<audio loop>` (and equivalent decoded-buffer loops) introduce a tiny silence gap on most platforms when the audio element restarts at the end of a loop. **[BEST-GUESS PM DECISION]** Acceptable for ambient sound — rain / fire / wind / etc. naturally have silence breaks and the gap (typically tens of milliseconds) reads as natural texture. Not acceptable for the metronome (which uses a different per-tick lifecycle).
- **Asset missing at runtime**: If a vendored asset file is absent from the dist tree (build mis-configuration, partial deploy), the ambient-sound subsystem must silently fail to start that track — no error toast, no exception bubbled to the user, no engine impact. The Settings UI keeps the user's selected track value; on next focus start, the playback path is a no-op for that track until the file is back.
- **Track-change-to-`None` while playing**: Per Story 1 scenario 9, fades out and does not fade in any replacement. Volume slider value is preserved.
- **Track-change-from-`None`-to-real while playing**: The "from `None`" case has nothing to fade out, so this is a simple fade-in over 300 ms (the cross-fade collapses to a fade-in when the prior track was absent).
- **Volume `0` while playing**: The active loop continues to play at zero amplitude. No fade-out, no automatic disable — `0` is a valid amplitude setting, not a disable sentinel. Toggling the slider back up resumes audibility live.
- **Volume `100` while playing**: Maximum amplitude. No clipping safeguards required beyond what the audio element / mixer applies natively (assets are mastered with headroom — see SC-008).
- **Volume hand-edit outside 0–100 in `settings.json`**: No defensive clamp at the audio call site (Principle III — validate at system boundaries only). The Settings UI re-clamps to 0–100 on next open / save (UI boundary clamp). A hand-edited `200` (stored as `u32`, which serde accepts) results in the driver attempting `HtmlAudioElement.volume = 200.0 / 100.0 = 2.0`. Per the HTML5 spec, `HtmlAudioElement.volume` throws `IndexSizeError` for values outside `0.0..=1.0` — it does NOT clamp. The Leptos call site swallows the error; the element retains its prior volume. **The audio side never re-clamps; the throw is the failure mode.** A hand-edited negative value into an unsigned field is rejected by serde at deserialise time (serde cannot represent a negative `u32`); the field falls back to the `#[serde(default)]` value of `50`.
- **Pause-then-track-change**: If the user opens Settings during a paused focus session and switches tracks, no audible cross-fade occurs (nothing is playing). On resume, the **new** track fades in. No memory of the old track is carried across the pause.
- **Continuous-sessions cycling and WKWebView autoplay**: Per Story 1 scenario 10, ambient fades in for each fresh auto-started focus. Auto-start has no fresh user gesture — the original Start click may be 25+ minutes prior. WKWebView's autoplay heuristic does not carry a stale gesture to a newly-created audio element. **Resolution (PM decision)**: a persistent `HtmlAudioElement` is pre-warmed on the user's first Start click (or when ambient sound is enabled) and kept alive across breaks / long-breaks / auto-starts within the same app session. Its lifetime acts as the gesture lease. When ambient sound is OFF or `None`, the element exists with `.src = ""` (silent, no decoding cost). See [research.md](./research.md) Decision 1. There is no continuous mid-break playback bleed-through — the element is silent (`.src = ""`) during breaks.
- **Three audio surfaces coexist**: Chime fires on transitions, metronome ticks once a second during focus (if enabled), ambient sound loops continuously during focus (if enabled). All three may be active simultaneously. They do not share an `AudioContext` / `<audio>` element / decoded buffer instance; volume is independent (the volume slider for ambient does not attenuate chime or metronome).
- **Asset file format**: **[BEST-GUESS PM DECISION]** The spec mandates "vendored MP3 or OGG assets at `src/assets/audio/ambient/<track>.{mp3,ogg}`". Exact choice (one format vs both, what file size budget per track) is a plan-level decision. Constraint surfaced as SC-008.
- **Asset sourcing / licensing**: **[BEST-GUESS PM DECISION]** Audio assets MUST be CC0 (Creative Commons Zero) or equivalent royalty-free, with no attribution requirement that would force runtime UI changes. Sourcing the seven files is a tasks-level concern, not a spec concern. Surfaced as A6.
- **Loop length too short**: A 5-second loop would feel obviously repetitive even for "ambient". **[BEST-GUESS PM DECISION]** Recommended length 60–120 s per track to avoid perceived repetition; encoded as SC-008.

## Requirements *(mandatory)*

### Functional Requirements

#### Behaviour — settings shape and defaults (constitutional anchors III, VI)

- **FR-001**: `NotificationSettings` (in `crates/presto-ipc/src/settings.rs`) MUST gain three new fields: `ambient_sound_enabled: bool`, `ambient_sound_type: AmbientSoundType`, `ambient_sound_volume: u32`. Each MUST carry `#[serde(default)]` so pre-feature-004 settings files deserialise unchanged, mirroring the metronome field at `crates/presto-ipc/src/settings.rs:185-186`.
- **FR-002**: `AmbientSoundType` MUST be a closed sum-type Rust enum with exactly eight variants: `None`, `Rain`, `Fire`, `Library`, `Fan`, `Storm`, `WhiteNoise`, `Wind`. Its wire shape MUST be `kebab-case` (e.g., `"none"`, `"white-noise"`), matching the precedent of `StatusBarDisplay` at `crates/presto-ipc/src/settings.rs:25-35`.
- **FR-003**: Default values MUST be `ambient_sound_enabled = false`, `ambient_sound_type = AmbientSoundType::None`, `ambient_sound_volume = 50`.
- **FR-004**: `ambient_sound_volume` MUST be stored as `u32` and clamped at the Settings UI input boundary to 0–100 inclusive. The engine and audio code MUST NOT contain runtime range guards on this value (Principle III — type-boundary validation at the UI input only).
- **FR-005**: An `AmbientSoundType::None` selection MUST be equivalent to "no track to play" for playback purposes while leaving `ambient_sound_volume` untouched in settings. Toggling `ambient_sound_enabled` off MUST NOT reset the user's `ambient_sound_type` or `ambient_sound_volume` either.

#### Behaviour — playback lifecycle (constitutional anchors I, II, III)

- **FR-006**: The ambient-sound playback MUST be implemented as a UI-side side effect in the timer component's tick loop / state-transition handlers, alongside `play_chime` and `play_metronome_tick` at `src/src/components/timer/mod.rs` (analogous to the metronome gate at lines 1358–1368). It MUST NOT modify engine state, the engine's event vocabulary, or introduce any new `web_sys` import into `src/src/engine/`.
- **FR-007**: Playback MUST occur when **all** of the following are true: `ambient_sound_enabled = true` AND `ambient_sound_type != None` AND `current_mode == Focus` AND the session is running (`is_running() && !is_paused() && !is_auto_paused()`) AND `time_remaining_secs() > 0`. The gate-condition expression MUST mirror the metronome gate's structure for consistency.
- **FR-008**: Playback MUST stop (fading out over ~200 ms) on any of: user-initiated pause, smart-pause auto-pause, mode transition out of focus (focus → break / long break zero-cross), overtime entry (`time_remaining_secs` reaches 0 with allow-continuous-sessions keeping the session open), setting toggle to `ambient_sound_enabled = false`, or setting track change to `ambient_sound_type = None`.
- **FR-009**: Playback MUST resume (fading in over ~200 ms) when a focus session resumes from pause OR when a fresh focus session starts (manual start or continuous-sessions auto-start).
- **FR-010**: A track change from one non-`None` track to a different non-`None` track WHILE playback is active MUST cross-fade: the active track fades out over ~300 ms and the new track fades in over ~300 ms simultaneously. A track change from `None` to a non-`None` track WHILE playback is otherwise gated on (focus running, enabled toggle on) collapses the cross-fade to a fade-in of the new track over ~300 ms.
- **FR-011**: A volume change WHILE playback is active MUST apply live without re-starting the track and without an audible artefact.
- **FR-012**: Ambient sound MUST NEVER play during `Break`, `LongBreak`, overtime, idle, or the gap between sessions. This MUST hold across all transitions including continuous-sessions auto-cycling.

#### Behaviour — Settings UI affordances (constitutional anchor III)

- **FR-013**: The Settings → Notifications tab MUST surface three new controls, placed below the existing metronome row: (a) a checkbox **"Enable ambient background sound"** with selector `#ambient-sound-enabled`; (b) a track dropdown **"Ambient sound"** with selector `#ambient-sound-type` whose options correspond 1:1 with `AmbientSoundType` variants in user-readable labels (e.g., "None", "Rain", "Fire", "Library", "Fan", "Storm", "White noise", "Wind"); (c) a volume slider **"Volume"** with selector `#ambient-sound-volume`, range 0–100, default 50.
- **FR-014**: The track dropdown and the volume slider MUST be visible regardless of the checkbox state. Disabling the checkbox MUST NOT hide or grey out the other two controls — the user should be able to pre-pick a track and volume before opting in, and toggling off should not destructively reset the slider position.
- **FR-015**: The three new selectors MUST be additive to the e2e selector contract: `#ambient-sound-enabled`, `#ambient-sound-type`, `#ambient-sound-volume`. No existing Settings → Notifications selector (`#smart-pause`, `#desktop-notifications`, `#sound-notifications`, the metronome checkbox if any) MAY be renamed or removed.

#### Behaviour — vendored assets (constitutional anchors II, IX)

- **FR-016**: Audio assets MUST be vendored under `src/assets/audio/ambient/` (one file per non-`None` track: `rain`, `fire`, `library`, `fan`, `storm`, `white-noise`, `wind`). Trunk's `copy-dir` directive MUST mirror this directory into the dist tree, following the same pattern as the existing icon and Phosphor font vendoring at `src/index.html:26-35`. A new `<link data-trunk rel="copy-dir" href="assets/audio" data-target-path="assets/audio" />` entry is added to `src/index.html`.
- **FR-017**: No audio asset, no audio metadata, and no playback control MAY be fetched from a network URL at runtime. CSP / `_blockExternal` posture remains unchanged. (Principle II.)
- **FR-018**: This feature MUST NOT add any new runtime Rust or npm dependency. The audio playback path MUST use Web platform APIs already available to the Leptos crate via `web_sys` (selection of HTML5 `<audio>` element vs Web Audio API is a plan-level decision; both are already reachable through existing `web_sys` re-exports). If a new dependency is unavoidable, both Cargo.lock and the e2e package-lock.json MUST be updated in the same commit. (Principle IX.)

#### Cross-cutting (constitutional anchors III, VI, IX)

- **FR-019**: No new Tauri command is expected. The settings IPC surface (`get_settings`, `update_settings`) carries the three new fields transparently as part of `NotificationSettings`. If a plan-level decision adds a new Tauri command (e.g., for preview-on-Settings-screen), it MUST follow the args-struct convention in `crates/presto-ipc/src/args.rs` with `#[serde(rename_all = "camelCase")]` and the `every_args_struct_top_level_keys_are_camel_case` defence-in-depth test MUST cover it.
- **FR-020**: No new network-egress path. No telemetry events. (Principle II.)
- **FR-021**: Visual regression baselines outside Settings → Notifications MUST NOT be regenerated. The single permitted regeneration is `settings-notifications-chromium-linux.png`, which MUST carry a one-line PR-description note. An untouched-screen diff is a regression to fix in code, not absorbed by re-baselining. (Principle IV.)

#### Test-first scope (constitutional anchor V)

- **FR-022**: Failing tests MUST precede implementation for the `NotificationSettings` serde-evolution: (a) round-trip of a pre-feature-004 JSON fixture that omits all three new keys → defaults to `false` / `None` / `50`; (b) round-trip of a JSON fixture with each non-`None` `AmbientSoundType` variant set → kebab-case serialisation round-trips byte-stable; (c) round-trip of a JSON fixture that already contains the `metronome` field (feature 002 baseline) AND the new ambient fields → both feature-002 and feature-004 fields survive the round-trip. UI plumbing (settings dropdown rendering, volume slider, checkbox), audio playback wiring, and fade-curve scheduling are e2e-covered and NOT in Principle V scope (engine has no new state).

#### Out-of-scope guards

- **FR-023**: No user-uploadable or filesystem-picker custom ambient tracks. The seven vendored tracks plus `None` are the entire selection surface in v1.
- **FR-024**: No per-track volume override. Volume is a single slider that applies to whichever track is selected. (A future feature could add per-track gain calibration — out of scope here.)
- **FR-025**: See FR-012 (out-of-scope guard restating the never-play-during-non-focus rule for emphasis).
- **FR-026**: No coupling between ambient sound and metronome or notification chime. They are independent toggles, independent volume settings, independent lifecycles. (See A4.)
- **FR-027**: No preview-on-Settings-screen play affordance in v1. **[BEST-GUESS PM DECISION]** The "tick to preview" pattern would require synchronising the Settings preview lifecycle with the focus-session lifecycle (or accepting a Settings-screen-only preview that doesn't follow the gate), and both have UX-surface implications worth their own spec. Defer.
- **FR-028**: No automatic asset preloading at app start. Tracks load lazily on first use to avoid adding ~14 MB of asset bytes to cold-start time when ambient sound is disabled (default).

### Key Entities

> One small wire-shape evolution on an existing entity (`NotificationSettings`), one new typed enum (`AmbientSoundType`), one vendored asset family, and one UI-side playback driver concept. No new on-disk entities are introduced.

- **`NotificationSettings` (evolved, `crates/presto-ipc/src/settings.rs`)**: Gains three fields — `ambient_sound_enabled: bool` (default `false`), `ambient_sound_type: AmbientSoundType` (default `None`), `ambient_sound_volume: u32` (default `50`). Each `#[serde(default)]` so legacy records deserialise unchanged. Read by the timer component's UI-side ambient-sound driver; the engine never reads any of the three fields.
- **`AmbientSoundType` (new typed enum, `crates/presto-ipc/src/settings.rs` or sibling module)**: Eight-variant closed sum type — `None`, `Rain`, `Fire`, `Library`, `Fan`, `Storm`, `WhiteNoise`, `Wind`. Wire shape kebab-case (matching `StatusBarDisplay` precedent). The `None` variant is a first-class "no track selected" sentinel, not a string sentinel and not an `Option<AmbientSoundType>` (the typed enum already encodes the absence case).
- **Vendored ambient sound asset family (new, `src/assets/audio/ambient/`)**: Seven audio files — `rain.{mp3|ogg}`, `fire.{mp3|ogg}`, `library.{mp3|ogg}`, `fan.{mp3|ogg}`, `storm.{mp3|ogg}`, `white-noise.{mp3|ogg}`, `wind.{mp3|ogg}`. Looping-friendly recordings, CC0 or equivalent royalty-free, ≤2 MB per file (SC-008), 60–120 s recommended length. Vendored via Trunk `copy-dir`; not fetched at runtime.
- **Ambient-sound playback driver (UI-side, no engine state)**: Stateful component sitting in the timer component's effect / tick-loop layer. Holds the active audio playback handle (HTML5 `<audio>` element or Web Audio API node graph — plan-level decision), the current fade state, and the current track identity. Reacts to settings changes (track / volume / enabled toggle) and engine state events (pause, resume, mode transition, overtime entry). Distinct from `play_chime` (per-call AudioContext, transient) and `play_metronome_tick` (cached AudioContext singleton, per-tick).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can enable the feature, pick a track, and hear it loop during a focus session, end-to-end, with 0 manual data steps and ≤4 UI interactions (open Settings → tick checkbox → pick track → close Settings; volume defaults to 50).
- **SC-002**: 100% of pre-feature-004 `notifications` records (lacking all three new keys) deserialise as `ambient_sound_enabled = false`, `ambient_sound_type = AmbientSoundType::None`, `ambient_sound_volume = 50` — measured by a round-trip test against a literal pre-feature-004 JSON fixture, mirroring the metronome legacy fixture at `crates/presto-ipc/src/settings.rs:362-375`.
- **SC-003**: Across all eight `AmbientSoundType` variants, settings round-trip byte-stable through serde with kebab-case wire encoding — `None` ↔ `"none"`, `Rain` ↔ `"rain"`, `Fire` ↔ `"fire"`, `Library` ↔ `"library"`, `Fan` ↔ `"fan"`, `Storm` ↔ `"storm"`, `WhiteNoise` ↔ `"white-noise"`, `Wind` ↔ `"wind"`.
- **SC-004**: 0 ambient-sound playback events fire during any of: idle, paused, smart-pause auto-paused, `Break`, `LongBreak`, overtime, `ambient_sound_enabled = false`, or `ambient_sound_type = None` — measured across the union of those states in an e2e or wasm-bindgen-test suite.
- **SC-005**: When `ambient_sound_enabled = true` and `ambient_sound_type != None`, a focus session produces continuous ambient playback for 100% of its non-overtime duration, with playback fade-out commencing within 50 ms of any stop trigger (pause, mode transition, overtime entry, toggle-off, track-change-to-`None`) and completing within ~250 ms total (50 ms trigger-to-start latency + 200 ms ramp = 250 ms end-to-end). Implementations MUST NOT shorten the 200 ms ramp to compensate for trigger latency.
- **SC-006**: A mid-focus track change (one non-`None` track to a different non-`None` track) executes a cross-fade with the new track audibly present within 300 ms of the change and the old track inaudible within 300 ms — no full silent gap longer than ~50 ms between them.
- **SC-007**: A mid-focus volume slider change reflects in the active track's amplitude within 50 ms with no audible glitch, no track restart, and no fade artefact.
- **SC-008**: Each vendored audio file MUST be ≤2 MB and ≥60 s of usable looping audio. Total vendored audio asset footprint ≤14 MB (7 files × 2 MB).
- **SC-009**: 0 new `web_sys` imports in `src/src/engine/`. Measured by `grep -r 'web_sys' src/src/engine/` (recursive, no anchor — catches re-exports and nested `use` forms) returning the same set before and after. (Principle I.)
- **SC-010**: 0 new runtime dependencies. Measured by `Cargo.lock` and `tests/e2e/package-lock.json` diffs showing only version-bump noise (if any). (Principle IX.)
- **SC-011**: 0 new network-egress code paths. Measured by `grep` for `fetch(` / `reqwest` / `supabase` / `aptabase` / new external URLs in the diff — zero hits. (Principle II.)
- **SC-012**: Only 1 visual regression baseline is regenerated — `settings-notifications-chromium-linux.png`. 0 baselines outside Settings → Notifications are regenerated. The regenerated baseline has a one-line PR note. (Principle IV.)
- **SC-013**: Audio playback for any vendored track must NOT clip at maximum slider value (`100`) on a system with default audio levels — measured by spot-check against the seven mastered assets; a clipping report fails the SC and requires asset re-mastering before merge.

## Assumptions

- **A1 — One PR, one user-facing capability**: The default delivery shape is a single PR landing the settings evolution, the UI controls, the vendored assets, and the playback driver together. The settings legacy-compat round-trip (Story 2) is the integrity guarantee that makes Story 1 safe to ship and is not split off.
- **A2 — `#[serde(default)]`-gated wire-shape evolution, not one-shot migration**: All three new fields (`ambient_sound_enabled`, `ambient_sound_type`, `ambient_sound_volume`) use `#[serde(default)]`. No first-launch migration; **Principle VI** honoured at the deserialiser, mirroring feature 002's metronome pattern exactly.
- **A3 — Audio assets vendored, not generated**: Audio files are pre-mastered and committed to the repo (or to a release-artefacts location with deterministic CI fetch — plan-level decision). No procedural audio generation at runtime.
- **A4 — Three audio surfaces are independent**: Chime (transition fanfare), metronome (focus tick), ambient sound (focus background) coexist without shared state. Each has its own volume / enable lifecycle. Each may be active during the same focus session.
- **A5 — `AmbientSoundType::None` is a first-class variant, not `Option<AmbientSoundType>`**: The closed enum encodes "no track selected" directly. **[BEST-GUESS PM DECISION]** This avoids the `Option<None>` vs `Option<Some(None)>` ambiguity, matches the user mental model (the dropdown's first option is "None"), and lines up with how `AmbientSoundType` serialises to the wire as a kebab-case string with no nullable wrapper. (Principle III — type-system encoding of "absence" via an explicit variant.)
- **A6 — Audio asset licensing**: **[BEST-GUESS PM DECISION]** Vendored audio MUST be CC0 (Creative Commons Zero) or equivalent royalty-free, with no attribution requirement that would surface in the app's UI or About dialog. Sourcing the seven files is a tasks-level concern; licensing is asserted here as a constraint on that work. Without it, the vendoring path silently incurs an attribution-display obligation we have no UI affordance for.
- **A7 — Loop seams are acceptable for ambient**: **[BEST-GUESS PM DECISION]** Per Edge Cases, the tens-of-milliseconds silence gap on loop restart is acceptable for rain / fire / etc. Not acceptable for the metronome (which uses a different per-tick mechanism, not a long-loop). If user feedback after ship objects, a follow-up spec can revisit the audio mechanism (Web Audio API with seamless looping vs `<audio loop>` simple looping).
- **A8 — Cross-fade duration 300 ms, pause-fade 200 ms**: **[BEST-GUESS PM DECISION]** Per Clarifications resolved. 200 ms is the minimum perceptually-smooth fade for "not jarring"; 300 ms for track-change cross-fade keeps the perceived gap minimal while letting both fades complete fully overlapped. Not constitutionally anchored; could be tuned per UX research.
- **A9 — Volume range 0–100, default 50**: **[BEST-GUESS PM DECISION]** Per the brief. Range is the de-facto standard for amplitude sliders in desktop apps; default 50 puts the slider at a "noticeable but not loud" amplitude that the user can adjust either direction. Not constitutionally anchored.
- **A10 — Volume slider lives in Settings, not on the timer screen**: The timer screen baseline does NOT regenerate (SC-012). The brief is unambiguous that affordances live in Settings → Notifications; a timer-screen-side ambient volume widget is a separate UX choice that would require its own visual-regression budget and is out of scope.
- **A11 — `None` is equivalent to "feature off for playback" while preserving volume**: Per FR-005. Toggling the enable checkbox off OR picking `None` from the dropdown both halt playback. The two controls are NOT redundant: the checkbox is the persistent on/off intent; the `None` variant is the "I've enabled the feature but picked no track" intermediate state (e.g., during initial onboarding before the user picks one).
- **A12 — Visual regression budget is one baseline**: Per Story 3 / FR-021 / SC-012. The widening is gated by a per-baseline justification note in the PR description. Principle IV honoured by explicit policy.
- **A13 — No engine `web_sys` imports introduced**: Per FR-006 and SC-009. The ambient-sound driver lives in the timer component, not in `src/src/engine/`. Principle I is preserved by construction.
- **A14 — Strict static analysis posture continues to apply**: Per Principle X. New code is clippy-pedantic-clean; no `#[allow(...)]` without inline justification. The existing `#[allow(clippy::struct_excessive_bools)]` on `NotificationSettings` at `crates/presto-ipc/src/settings.rs:167` continues to cover the count after `ambient_sound_enabled` is added.
- **A15 — Asset loading is lazy, not eager**: Per FR-028. Cold-start performance is not penalised when ambient sound is disabled (the default); assets are loaded on first focus start with the feature enabled. Plan-level decision on whether to pre-decode or stream is deferred to the plan.
