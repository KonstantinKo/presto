# Component & Side-Effect Contracts — Feature 004

Three contracts.

1. `AmbientSoundType` — wire-shape enum (closed sum type, kebab-case strings).
2. `NotificationSettings` — three-field evolution; each `#[serde(default)]` following the metronome precedent.
3. `AmbientAudio` — UI-side side-effect driver with a five-state machine and explicit fade-duration table.

## 1. `AmbientSoundType` — wire-shape contract

### Closed sum type (eight variants)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "kebab-case")]
pub enum AmbientSoundType {
    #[default]
    None,
    Rain,
    Fire,
    Library,
    Fan,
    Storm,
    WhiteNoise,
    Wind,
}
```

The `#[serde(rename_all = "kebab-case")]` attribute is the same one used by `StatusBarDisplay` at `crates/presto-ipc/src/settings.rs:27` (line `#[serde(rename_all = "kebab-case")]`); serde handles the variant ↔ string mapping in both directions exhaustively. No custom `Serialize` / `Deserialize` impls required. No `from_str` parser required (serde IS the parser).

### Wire-shape assertion table

| Variant | Serialised JSON value (in `notifications.ambient_sound_type`) |
|---|---|
| `AmbientSoundType::None` | `"none"` |
| `AmbientSoundType::Rain` | `"rain"` |
| `AmbientSoundType::Fire` | `"fire"` |
| `AmbientSoundType::Library` | `"library"` |
| `AmbientSoundType::Fan` | `"fan"` |
| `AmbientSoundType::Storm` | `"storm"` |
| `AmbientSoundType::WhiteNoise` | `"white-noise"` |
| `AmbientSoundType::Wind` | `"wind"` |

The mapping is asserted byte-stable by `presto_ipc::settings::tests::ambient_sound_type_serialises_kebab_case` (one of the three RED-first tests).

### `Default` impl

Returns `AmbientSoundType::None` via `#[default]` on the variant — derived automatically by `#[derive(Default)]`. This is the default value used by `#[serde(default)]` on the `ambient_sound_type` field of `NotificationSettings`.

### `None` variant invariants

- `None` is NOT the empty string on the wire — it serialises as the kebab-case literal `"none"`.
- `None` is NOT `Option::None` — the closed enum encodes the absence case directly.
- `None` is a valid persisted value: a user who picks the "None" option from the dropdown writes `"none"` to the wire. This is distinct from never having opted into the feature (where the field would have been absent from the legacy JSON entirely, and `#[serde(default)]` would have produced `AmbientSoundType::None`).
- The lookup `asset_path(AmbientSoundType::None)` returns `None` — there is no asset file for this variant. The UI-side driver gates its playback path on `asset_path` being `Some(...)`.

## 2. `NotificationSettings` evolution — wire-shape contract

Three new fields appended to the existing struct. The metronome field at `crates/presto-ipc/src/settings.rs:185-186` is the exact serde-evolution precedent — verbatim:

```rust
    /// When true, fire a soft tick once per second during focus
    /// sessions, in sync with the 1 Hz countdown. Default `false`
    /// (opt-in per Principle II). UI-side side effect only — engine
    /// is unaware. Locked to the second; not user-configurable.
    #[serde(default)]
    pub metronome: bool,
```

This feature's evolution mirrors the `#[serde(default)]` attribute placement byte-for-byte.

### Field additions

```rust
#[serde(default)]
pub ambient_sound_enabled: bool,

#[serde(default)]
pub ambient_sound_type: AmbientSoundType,

#[serde(default = "default_ambient_sound_volume")]
pub ambient_sound_volume: u32,
```

### `Default` impl additions

```rust
ambient_sound_enabled: false,
ambient_sound_type: AmbientSoundType::None,
ambient_sound_volume: 50,
```

### `const fn` default helper

```rust
#[must_use]
pub const fn default_ambient_sound_volume() -> u32 {
    50
}
```

Placed alongside the existing `default_metronome_bpm` precedent (the metronome BPM helper was removed in feature 002's refinement to a tick-per-second posture — see the `metronome_bpm_legacy_field_ignored` test at `crates/presto-ipc/src/settings.rs:382-395` — but the const-fn pattern is the canonical default-value emitter for `#[serde(default = "...")]`). The `#[must_use]` annotation is consistent with the post-003 strict clippy posture.

### Legacy fixture round-trip

A pre-feature-004 `notifications` JSON block (feature 002 baseline) lacking all three new keys:

```json
{
  "desktop_notifications": true,
  "sound_notifications": true,
  "auto_start_timer": true,
  "smart_pause": false,
  "smart_pause_timeout": 30,
  "metronome": false
}
```

Deserialises to:

| Field | Value | Source |
|---|---|---|
| `desktop_notifications` | `true` | Wire |
| `sound_notifications` | `true` | Wire |
| `auto_start_timer` | `true` | Wire |
| `auto_start_focus` | `false` | `#[serde(default)]` (feature 001 precedent) |
| `allow_continuous_sessions` | `false` | `#[serde(default)]` (feature 001 precedent) |
| `smart_pause` | `false` | Wire |
| `smart_pause_timeout` | `30` | Wire |
| `metronome` | `false` | Wire (feature 002 baseline) |
| `ambient_sound_enabled` | `false` | `#[serde(default)]` (feature 004 NEW) |
| `ambient_sound_type` | `AmbientSoundType::None` | `#[serde(default)]` (feature 004 NEW) |
| `ambient_sound_volume` | `50` | `#[serde(default = "default_ambient_sound_volume")]` (feature 004 NEW) |

This round-trip is asserted by `ambient_sound_legacy_fields_default` — the first of the three RED-first tests.

### Non-default round-trip

A new-build `notifications` JSON block:

```json
{
  "desktop_notifications": true,
  "sound_notifications": true,
  "auto_start_timer": true,
  "auto_start_focus": false,
  "allow_continuous_sessions": false,
  "smart_pause": false,
  "smart_pause_timeout": 30,
  "metronome": true,
  "ambient_sound_enabled": true,
  "ambient_sound_type": "rain",
  "ambient_sound_volume": 30
}
```

Serialises and deserialises byte-stable; the `metronome: true` value from feature 002 survives the round-trip alongside the new fields. Asserted by `ambient_sound_round_trip` — the second of the three RED-first tests.

### Wire-shape constraint summary

- No new struct types on the wire (the new fields live inside the existing `NotificationSettings`).
- No new `#[allow(...)]` annotations — the existing `#[allow(clippy::struct_excessive_bools)]` at `crates/presto-ipc/src/settings.rs:167` continues to cover the bool count (its inline justification — "every bool maps to an independent UI toggle" — applies to `ambient_sound_enabled` identically).
- No new Tauri commands. `save_settings` / `load_settings` transparently round-trip the new fields as part of the existing `Settings` payload.

## 3. `AmbientAudio` — UI-side side-effect driver

The driver lives in `src/src/components/ambient_audio.rs`. It is NOT a Leptos `#[component]` in the visual sense — it emits no DOM. It is a state-machine + side-effect surface invoked by the timer component's gate-watching `Effect`.

### Public surface (functions / signals invoked from `timer/mod.rs`)

| Operation | Trigger source | Effect |
|---|---|---|
| `start(track, volume)` | Gate rising edge | Creates an `HtmlAudioElement` for `track`'s asset path with `.loop = true`, `.volume = 0.0`, calls `.play()`, schedules a 200 ms fade-in to `volume / 100.0`. Driver state → `Playing { track }` |
| `pause()` | Pause / smart-pause / overtime entry | 200 ms fade-out (ramp `.volume` to 0.0 over 200 ms), then `.pause()` on the element. Driver state → `Paused { track }` (the element remains resident) |
| `resume(volume)` | Resume from pause | `.play()` on the resident element, 200 ms fade-in from 0.0 to `volume / 100.0`. Driver state → `Playing { track }` |
| `cross_fade(new_track, volume)` | Settings track change while gate is high | Spawns a second `HtmlAudioElement` for `new_track`, `.loop = true`, `.volume = 0.0`, calls `.play()`. The outgoing element ramps from `volume / 100.0` down to 0.0 over 300 ms; the incoming element ramps from 0.0 up to `volume / 100.0` over 300 ms. At ramp completion, the outgoing element is `.pause()`d and dropped. Driver state during ramp: `CrossFading { outgoing, incoming }`; after: `Playing { track: new_track }` |
| `set_volume(volume)` | Slider drag while gate is high | Updates the resident element's `.volume` slot to `volume / 100.0` immediately (no fade, no restart). State unchanged |
| `fade_out()` | Gate falling edge (disable / `None` / mode out of focus / overtime / session end) | 200 ms fade-out, `.pause()`, drop the element. Driver state → `Idle` |
| `is_playing() -> bool` | Test / debug introspection | Returns whether the current state is one of `Playing` / `Paused` / `CrossFading` / `FadingOut` |

### State machine (full transition diagram)

```
                                                                    ┌──────────────────────────────┐
                                                                    │                              │
                                                                    │ Track change in Settings →   │
                                                                    │ Non-None (different track)   │
                                                                    │                              │
                                                                    ▼                              │
   ┌───────────┐    Gate rising edge       ┌──────────────────┐  Cross-fade 300 ms    ┌────────────────────────────┐
   │           │    (Focus mode AND        │                  │ ────────────────────► │                            │
   │   Idle    │ ─────────────────────────►│ Playing(track)   │                       │ CrossFading(old, new)      │
   │           │    enabled AND            │                  │                       │                            │
   └───────────┘    type != None)          └──────────────────┘                       └────────────────────────────┘
        ▲                                       │      ▲                                         │
        │                                       │      │                                         │ Ramp complete
        │ Fade-out                              │      │                                         │ (300 ms)
        │ complete                              │      │ Resume                                  │
        │ (200 ms)                              │      │ (fade-in 200 ms)                        ▼
        │                                       │      │                                ┌──────────────────┐
        │                                       │      │                                │ Playing(new)     │
        │              Pause / smart-pause /    │      │                                └──────────────────┘
        │              overtime entry           ▼      │
        │              (fade-out 200 ms)   ┌──────────────────┐
        │                                  │                  │
   ┌──────────────┐ ◄────────────────────── │  Paused(track)   │
   │ FadingOut    │       Mode out of      │                  │
   │ (track)      │       focus /          └──────────────────┘
   └──────────────┘       disable toggle /
                          track → None
                          (fade-out 200 ms)
```

Listed exhaustively as transition arcs (for code-review parity with the metronome's lifecycle table in feature 002 plan):

1. `Idle → Playing(t)` on gate rising edge. Trigger: `(notifications.ambient_sound_enabled && ambient_sound_type != AmbientSoundType::None && current_mode == Focus && is_running && !is_paused && !is_auto_paused && time_remaining_secs > 0)` transitioning from false to true, where `t` is the current `ambient_sound_type` value. Effect: 200 ms fade-in from volume 0 to `slider / 100.0`.
2. `Playing(t) → Paused(t)` on pause / smart-pause / overtime entry. Trigger: `is_paused || is_auto_paused || time_remaining_secs <= 0` rising. Effect: 200 ms fade-out, then `.pause()` on the element (element remains resident).
3. `Paused(t) → Playing(t)` on resume. Trigger: `is_paused || is_auto_paused` falling AND `time_remaining_secs > 0` AND `current_mode == Focus`. Effect: `.play()` on the resident element, 200 ms fade-in from 0 to `slider / 100.0`.
4. `Playing(old) → CrossFading(old, new)` on track change in settings. Trigger: `ambient_sound_type` reactive value changes from `old` to `new` where `old != new` and `new != AmbientSoundType::None`. Effect: spawn second `<audio>` element, simultaneous 300 ms ramps (outgoing → 0, incoming 0 → `slider / 100.0`).
5. `CrossFading(_, new) → Playing(new)` on ramp completion (300 ms). Effect: drop the outgoing element handle; the resident element is now `new`. **IMPORTANT**: the completion callback MUST re-check the gate before transitioning. If `gate_high == false` at the moment of completion (e.g., overtime entered during the 300 ms ramp), transition to `FadingOut(new)` instead. This makes the cross-fade idempotent against late gate-flips — see arc (10) below.
6. `CrossFading(old, new) → FadingOut(both)` when `gate_high` becomes false (user disabled, picked `None`, or session ended) while cross-fade is in progress. Cancels both 300 ms ramps; fades both elements out from their CURRENT `.volume` over 200 ms. Both elements terminated when ramps complete. After ramp completion, transitions to `Idle`.
7. `Playing(t) → FadingOut(t)` on session-end / disabled / type → None. Trigger: any of `current_mode != Focus`, `ambient_sound_enabled == false`, `ambient_sound_type == None`. Effect: 200 ms fade-out.
8. `Paused(t) → FadingOut(t)` on the same triggers as (7) — note the audible behaviour is no different because volume is already 0 in `Paused`, but the state-machine arc exists so the resident element is properly dropped.
9. `FadingOut(t) → Idle` on ramp completion (200 ms). Effect: `.pause()` + drop element handle.
10. `Playing(t) → Playing(t)` (self-arc) on slider volume change. Effect: immediate `.volume` update on the resident element. NOT a state-machine arc per se; the state stays `Playing` and the volume slot updates.

### Fade-duration table

| Transition | Fade duration | Implementation |
|---|---|---|
| `Idle → Playing` (gate rising edge) | 200 ms fade-in | `.volume` linearly 0 → `slider / 100.0` over 200 ms |
| `Playing → Paused` | 200 ms fade-out | `.volume` linearly `slider / 100.0` → 0 over 200 ms, then `.pause()` |
| `Paused → Playing` (resume) | 200 ms fade-in | `.play()`, `.volume` linearly 0 → `slider / 100.0` over 200 ms |
| `Playing → CrossFading → Playing(new)` (track change) | 300 ms (overlapped) | Outgoing `.volume` `slider / 100.0` → 0 over 300 ms; incoming `.volume` 0 → `slider / 100.0` over 300 ms simultaneously |
| `Playing/Paused → FadingOut → Idle` (disable / None / mode-out / session-end) | 200 ms fade-out | `.volume` linearly current → 0 over 200 ms, then `.pause()` + drop |
| `Playing(t) → Playing(t)` (slider drag) | 0 ms (live update) | `.volume = new / 100.0` immediately |

The 200 ms / 300 ms numbers are PM decisions per Spec A8. The ramp is linear in `.volume` (the underlying `HtmlAudioElement.volume` property is a multiplier on the output; linear in volume is perceptually slightly faster than linear in dB, which is acceptable for the 200/300 ms budget).

### Pre-emption rules

- A new transition starting while a ramp is in flight cancels the in-flight ramp (drops its `IntervalHandle`) and starts a fresh ramp. Audio elements are re-used where the from-state and to-state share the same logical element (e.g., a fade-out cancelled by a resume keeps the same `<audio>` element and just reverses the ramp direction).
- `CrossFading` cannot be pre-empted by a second track change mid-ramp; the second change is queued by Leptos's reactive flush, and the driver observes only the final settled `ambient_sound_type` value at the next reactive tick. (Edge case: two rapid track changes within the 300 ms ramp window — the second value wins; the in-flight cross-fade completes to the first new value, then a second cross-fade fires to the final value. This is acceptable per the spec — no requirement on rapid-fire track changes.)
- **Track-change-while-FadingOut rule**: while in `FadingOut`, settings mutations (track change, enable toggle, volume change) update the persisted settings only — no transition fires from the driver. Next entry to `Idle` re-evaluates the gate against the latest settings and may immediately transition to `Playing(latest_track)` if the gate is now high again.
- **Volume-change-while-Paused rule**: while in `Paused(track)`, a volume change updates the driver's stored target volume. The change does NOT modify the element (which is at `.volume = 0` during pause). On resume (`Paused → Playing`), the fade-in ramp targets the NEW stored volume.
- **Volume-change-while-Idle rule**: while in `Idle`, volume changes update the persisted settings only. The next `Idle → Playing` transition reads the latest volume value for the fade-in ramp's target.
- **Cross-fade completion vs gate-flip race**: the cross-fade's completion callback MUST re-check the gate before transitioning `CrossFading(old, new) → Playing(new)`. If `gate_high == false` at the moment of completion (e.g., overtime entered during the 300 ms ramp), transition to `FadingOut(new)` instead. This makes the cross-fade idempotent against late gate-flips (arc 5 above).

### Host-testable projection pattern (`AudioElementHandle` trait)

`wasm-pack test --node` provides no DOM; `web_sys::HtmlAudioElement` is therefore unusable in that environment. The driver uses a trait abstraction to keep the state-machine logic host-testable.

Define in `src/src/components/ambient_audio.rs`:

```rust
pub trait AudioElementHandle {
    fn set_src(&self, src: &str);
    fn set_volume(&self, vol: f64);
    fn play(&self) -> Result<(), JsValue>;
    fn pause(&self);
    fn current_time(&self) -> f64;
}
```

Real implementation (browser target):

```rust
pub struct HtmlAudioWrapper(pub HtmlAudioElement);

impl AudioElementHandle for HtmlAudioWrapper { /* delegates to HtmlAudioElement */ }
```

Test implementation (injected by `wasm-bindgen-test` in `--node` mode):

```rust
pub struct MockAudioHandle { pub calls: RefCell<Vec<String>> }

impl AudioElementHandle for MockAudioHandle {
    fn set_src(&self, src: &str) { self.calls.borrow_mut().push(format!("set_src:{src}")); }
    fn set_volume(&self, vol: f64) { self.calls.borrow_mut().push(format!("set_volume:{vol}")); }
    fn play(&self) -> Result<(), JsValue> { self.calls.borrow_mut().push("play".into()); Ok(()) }
    fn pause(&self) { self.calls.borrow_mut().push("pause".into()); }
    fn current_time(&self) -> f64 { 0.0 }
}
```

The `AmbientAudio` state machine takes a generic `H: AudioElementHandle`. The `wasm-bindgen-test` injects `MockAudioHandle` and asserts on `calls` after each scenario step. This pattern mirrors the `IconClass::render_spec` host-testable projection in feature 003.

### Non-Tauri / non-IPC scope

The `AmbientAudio` module is entirely UI-side. It:

- Does NOT define any `#[tauri::command]` handler.
- Does NOT call `tauri::invoke(...)` or `tauri::event::listen(...)`.
- Does NOT add anything to `tests/e2e/fixtures/tauriMock.js`.
- Does NOT import anything from `src/src/engine/`.
- DOES import `web_sys::HtmlAudioElement` and `web_sys::HtmlMediaElement` (both reachable through the existing `web-sys` crate by widening the feature list — see [plan.md §Phase 1](../plan.md#phase-1--web-sys-feature-widening)).
- DOES read from the timer component's `RwSignal<Settings>` and the timer component's engine-state-derived signals (`current_mode`, `is_running`, `is_paused`, `is_auto_paused`, `time_remaining_secs`) — both via reactive sources passed in as `Signal`-typed parameters when the driver is wired from `timer/mod.rs`.

### Mock-drift gate impact

The mock-drift gate (`scripts/check-mock-drift.sh` or equivalent post-002 / 003 gate) sees zero new `#[tauri::command]` handlers and zero new mock entries. Verified by spot-check against `tests/e2e/fixtures/tauriMock.js` — no new commands are required to ship this feature.

### Testing surface

Per [plan.md §Testing strategy](../plan.md#testing-strategy-and-test-first-markers):

- The IPC wire-shape evolution carries three RED-first `cargo test` cases in `presto_ipc::settings::tests` (see [plan.md §Testing strategy](../plan.md#testing-strategy-and-test-first-markers) and the three test names enumerated in [plan.md §Constitution Check V](../plan.md#v-test-first-for-stateful-engines--ipc-round-trip-scope-only)).
- The driver state machine carries one MANDATORY non-RED-first `wasm-bindgen-test` (`ambient_audio::tests::state_transitions`) that lands alongside the Phase 3 implementation. It exercises the full transition matrix below with a `MockAudioHandle` stub and asserts the resulting call log at each step (see host-testable projection pattern below).
- UI plumbing (the three new selectors `#ambient-sound-enabled`, `#ambient-sound-type`, `#ambient-sound-volume`) is e2e-covered via the new flow in `tests/e2e/settings-notifications.spec.js`.
- The audio playback itself is NOT exercised in e2e (headless chromium has no audio output assertion path); coverage of the audible behaviour is the state-machine wasm-bindgen-test plus PR-time manual review.

#### `ambient_sound_type_serialises_kebab_case` test — all 8 variants (RED-first)

The test MUST enumerate all eight `AmbientSoundType` variants explicitly:

| Variant | Wire string | Direction |
|---|---|---|
| `None` | `"none"` | both (serialize + deserialize) |
| `Rain` | `"rain"` | both |
| `Fire` | `"fire"` | both |
| `Library` | `"library"` | both |
| `Fan` | `"fan"` | both |
| `Storm` | `"storm"` | both |
| `WhiteNoise` | `"white-noise"` | both — critical: the only multi-word variant |
| `Wind` | `"wind"` | both |

`WhiteNoise → "white-noise"` MUST be explicitly asserted (not covered by a generic round-trip). If `#[serde(rename_all = "kebab-case")]` is misconfigured, `WhiteNoise` might silently serialize as `"whitenoise"` or `"white_noise"`.

#### `state_transitions` wasm-bindgen-test — full pre-emption matrix (non-RED-first, MANDATORY)

The test MUST cover these nine scenarios, not just the happy path:

1. **Happy path** — `Idle → Playing → Paused → Playing → CrossFading → Playing → FadingOut → Idle`: full state machine walk through all five states; asserts call log entries (`play`, `pause`, `set_volume`, etc.) at each transition via `MockAudioHandle`.
2. **Disable-during-cross-fade** — `Playing → CrossFading → FadingOut → Idle`: gate_high goes false during the 300 ms cross-fade ramp; asserts arc (6) fires (both elements fade out from their CURRENT volume over 200 ms), not arc (5).
3. **Track-change-ignored-during-FadingOut** — `Playing → FadingOut + track-change-while-fading → Idle`: track changes while in FadingOut update persisted settings only; no new transition fires; on exit to Idle, re-evaluates gate and may start Playing with the latest track.
4. **Cross-fade completion race** — `Playing → CrossFading + gate-flip-during-fade → FadingOut`: gate_high flips false between the cross-fade start and the completion callback; asserts the completion callback detects `gate_high == false` and transitions to `FadingOut(new)` not `Playing(new)`.
5. **Volume-change-while-Paused** — `Paused → volume-change → Paused → resume-with-new-target`: asserts the element stays at `set_volume:0.0` during the change; on resume, the fade-in ramp targets the new stored volume, not the old one.
6. **Volume-change-while-Idle** — `Idle → volume-change → Idle → Playing-with-new-target`: asserts no transition fires during the idle volume change; on first Playing entry, the ramp targets the updated volume.
7. **Rapid-fire track changes** — `Playing(a) → track-change-to-b → track-change-to-c` within 100 ms: each new change cancels the in-flight cross-fade and starts a fresh one; the final settled value wins; no phantom audio from the abandoned ramp.
8. **None → real → None cycle** — `Idle → Playing → FadingOut → Idle`: full cycle through the `None`-less path without an intermediate cross-fade; confirms the collapse from cross-fade to simple fade-in works when starting from no prior track.
9. **Disable-while-Paused** — state in `Paused(track)`, user toggles `ambient_sound_enabled = false`. Expected: transitions to `FadingOut`. The resident element fades from its current volume = 0 (no audible effect) but the state machine MUST cleanly tear down both the resident element and any pre-warmed slots, then transition to `Idle` when the ramp completes.
