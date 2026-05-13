# Data Model — Feature 004 (Ambient Background Sounds)

Three shapes evolve / are introduced.

1. `AmbientSoundType` — new closed sum-type enum on the IPC wire.
2. `NotificationSettings` — three new fields appended to an existing struct.
3. `AmbientAudioState` — UI-side runtime state (NOT serialised, NOT on the IPC wire).

The on-disk schema is unchanged structurally — only field-level additions inside the existing `notifications` block. No migration is required because all three new fields carry `#[serde(default)]` (or `#[serde(default = "...")]` for the `u32` default).

## 1. `AmbientSoundType` (new — `crates/presto-ipc/src/settings.rs`)

Closed eight-variant Rust enum. The `None` variant is a first-class "no track selected" sentinel, not `Option<AmbientSoundType>` and not a string sentinel.

```rust
/// Ambient-sound track selection.
///
/// `None` is a first-class variant ("no track selected"), not the
/// empty string and not `Option<AmbientSoundType>` — the closed enum
/// already encodes the absence case (FR-002, A5; Principle III's
/// "type-system encoding of absence via an explicit variant" rule).
///
/// Wire shape: kebab-case strings (`"none"`, `"rain"`, `"fire"`,
/// `"library"`, `"fan"`, `"storm"`, `"white-noise"`, `"wind"`),
/// matching the `StatusBarDisplay` precedent at `:25-35`.
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

### Wire-shape ↔ variant mapping

| Variant | Wire string | User-readable dropdown label |
|---|---|---|
| `None` | `"none"` | `None` |
| `Rain` | `"rain"` | `Rain` |
| `Fire` | `"fire"` | `Fire` |
| `Library` | `"library"` | `Library` |
| `Fan` | `"fan"` | `Fan` |
| `Storm` | `"storm"` | `Storm` |
| `WhiteNoise` | `"white-noise"` | `White noise` |
| `Wind` | `"wind"` | `Wind` |

`#[serde(rename_all = "kebab-case")]` performs the variant ↔ string mapping for both directions. The dropdown labels are a separate concern (UI-side `match` expression in `settings/notifications.rs`); they are NEVER persisted on the wire and NEVER read from the wire — only the kebab-case strings cross the IPC boundary.

### Asset-path mapping

The non-`None` variants map 1:1 to vendored MP3 filenames in `src/assets/audio/ambient/`:

| Variant | Asset path |
|---|---|
| `None` | (no asset — playback is a no-op) |
| `Rain` | `/assets/audio/ambient/rain.mp3` |
| `Fire` | `/assets/audio/ambient/fire.mp3` |
| `Library` | `/assets/audio/ambient/library.mp3` |
| `Fan` | `/assets/audio/ambient/fan.mp3` |
| `Storm` | `/assets/audio/ambient/storm.mp3` |
| `WhiteNoise` | `/assets/audio/ambient/white-noise.mp3` |
| `Wind` | `/assets/audio/ambient/wind.mp3` |

The asset-path lookup lives in `src/src/components/ambient_audio.rs` as a small `const fn asset_path(t: AmbientSoundType) -> Option<&'static str>` that returns `Some("/assets/audio/ambient/rain.mp3")` for `Rain` and `None` for `AmbientSoundType::None`. Match-exhaustiveness on the closed enum ensures the table stays in sync with the enum variant list.

## 2. `NotificationSettings` evolution (`crates/presto-ipc/src/settings.rs`)

Three new fields appended to the existing struct. The metronome field at `:185-186` is the exact serde-evolution precedent.

### Before (feature 002 baseline at `:167-202`)

```rust
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct NotificationSettings {
    pub desktop_notifications: bool,
    pub sound_notifications: bool,
    pub auto_start_timer: bool,
    #[serde(default)]
    pub auto_start_focus: bool,
    #[serde(default)]
    pub allow_continuous_sessions: bool,
    pub smart_pause: bool,
    /// Seconds.
    pub smart_pause_timeout: u32,
    /// (feature 002, Bundle C — metronome opt-in)
    #[serde(default)]
    pub metronome: bool,
}
```

### After (feature 004)

```rust
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct NotificationSettings {
    pub desktop_notifications: bool,
    pub sound_notifications: bool,
    pub auto_start_timer: bool,
    #[serde(default)]
    pub auto_start_focus: bool,
    #[serde(default)]
    pub allow_continuous_sessions: bool,
    pub smart_pause: bool,
    /// Seconds.
    pub smart_pause_timeout: u32,
    /// (feature 002, Bundle C — metronome opt-in)
    #[serde(default)]
    pub metronome: bool,
    /// (feature 004 — ambient background sound opt-in)
    ///
    /// When true AND `ambient_sound_type != None` AND the timer is in
    /// the focus running state, the selected ambient track loops at
    /// the configured volume. Default `false` (opt-in per Principle II).
    /// UI-side side effect only — engine is unaware.
    #[serde(default)]
    pub ambient_sound_enabled: bool,
    /// (feature 004) Currently-selected ambient track.
    ///
    /// `None` is a first-class "no track selected" sentinel (FR-002,
    /// A5; Principle III). Toggling `ambient_sound_enabled` off OR
    /// picking `None` from the dropdown both halt playback while
    /// preserving the other field's value (FR-005).
    #[serde(default)]
    pub ambient_sound_type: AmbientSoundType,
    /// (feature 004) Output amplitude, 0..=100 inclusive.
    ///
    /// Clamped at the Settings UI input boundary (`<input type="range"
    /// min="0" max="100">`); the audio call site reads the stored
    /// value and passes it through to `HtmlAudioElement::set_volume`
    /// without re-clamping (Principle III — validate at boundaries
    /// only). Default `50` per FR-003 / A9.
    #[serde(default = "default_ambient_sound_volume")]
    pub ambient_sound_volume: u32,
}

#[must_use]
pub const fn default_ambient_sound_volume() -> u32 {
    50
}
```

### `Default` impl evolution

```rust
impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            desktop_notifications: true,
            sound_notifications: true,
            auto_start_timer: true,
            auto_start_focus: false,
            allow_continuous_sessions: false,
            smart_pause: false,
            smart_pause_timeout: 30,
            metronome: false,
            // (feature 004 additions)
            ambient_sound_enabled: false,
            ambient_sound_type: AmbientSoundType::None,
            ambient_sound_volume: 50,
        }
    }
}
```

### Field-level invariants

| Field | Type | Default | Boundary | Wire encoding |
|---|---|---|---|---|
| `ambient_sound_enabled` | `bool` | `false` | UI checkbox; no clamp | JSON bool |
| `ambient_sound_type` | `AmbientSoundType` | `None` | UI dropdown; closed enum | JSON kebab-case string |
| `ambient_sound_volume` | `u32` | `50` | UI slider `min=0 max=100`; serde rejects negatives at deserialise | JSON unsigned integer |

`ambient_sound_volume = 0` is a valid amplitude setting (the slider's left end), not a feature-disable sentinel. Edge Cases / FR-005 / A11 explicitly forbid reading 0 as "off".

### Legacy compatibility

Pre-feature-004 settings JSON missing all three keys deserialises to the defaults above (test `ambient_sound_legacy_fields_default`). A settings JSON containing `metronome: true` (feature 002) and the three new keys round-trips byte-stable in both directions (test `ambient_sound_round_trip`). No migration is required because every new field carries `#[serde(default)]` (or `#[serde(default = "default_ambient_sound_volume")]` for the `u32`).

## 3. `AmbientAudioState` — UI-side runtime state (NOT serialised)

Closed sum type internal to `src/src/components/ambient_audio.rs`. NEVER crosses the IPC boundary. NEVER persisted. Lives in a component-local `RwSignal<AmbientAudioState>` for the duration of the timer component's lifetime.

```rust
/// UI-side runtime state for the ambient-audio driver.
///
/// NOT serialised. NOT on the IPC wire. NOT persisted across restarts.
/// Owned by `src/src/components/ambient_audio.rs`; read by the timer
/// component's gate-watching `Effect` only to know whether to drive
/// the next transition.
///
/// The driver tracks at most two simultaneous `<audio>` elements
/// during a cross-fade. In every other state, at most one element is
/// alive.
#[derive(Debug, Clone)]
pub(crate) enum AmbientAudioState {
    /// Nothing playing; no `<audio>` element resident.
    Idle,
    /// One element playing at the configured slider volume.
    Playing { track: AmbientSoundType },
    /// One element resident at volume 0, `.pause()`d. Re-enters
    /// `Playing` on resume via a 200 ms volume ramp.
    Paused { track: AmbientSoundType },
    /// Two elements alive: the outgoing track ramping volume → 0 over
    /// 300 ms (will be `.pause()`d + dropped at the ramp end) and the
    /// incoming track ramping volume 0 → slider/100 over 300 ms.
    /// Transitions to `Playing { track: new }` at ramp completion.
    CrossFading {
        outgoing: AmbientSoundType,
        incoming: AmbientSoundType,
    },
    /// One element ramping volume → 0 over 200 ms; will be
    /// `.pause()`d + dropped at the ramp end. Transitions to `Idle`.
    FadingOut { track: AmbientSoundType },
}
```

### State-transition surface

The full transition diagram lives in [contracts/components.md](./contracts/components.md). Summary table:

| From | Trigger | To |
|---|---|---|
| `Idle` | Gate signal rising edge (focus mode entered AND enabled AND type != None) | `Playing { track }` (fade-in 200 ms from volume 0) |
| `Playing { t }` | Pause / smart-pause / overtime entry | `Paused { t }` (200 ms fade-out then `.pause()`) |
| `Paused { t }` | Resume | `Playing { t }` (200 ms fade-in) |
| `Playing { old }` | Settings track change to non-`None` `new` | `CrossFading { old, new }` (300 ms) |
| `Playing { _ }` OR `Paused { _ }` | Mode transition out of focus / disable toggle / track → `None` | `FadingOut { _ }` (200 ms) → `Idle` |
| `Playing { t }` | Slider volume change | `Playing { t }` with live `.volume` update (no fade, no restart) |
| `CrossFading { _, new }` | Ramp complete | `Playing { new }` |
| `FadingOut { _ }` | Ramp complete | `Idle` |

The `Playing { t } → Playing { t }` self-transition for volume change is NOT a state machine arc per se — it's a live property update on the resident `<audio>` element. The state machine remains in `Playing` and the `.volume` slot updates atomically.

### Host-testable abstraction

The driver takes a generic `H: AudioElementHandle` rather than using `web_sys::HtmlAudioElement` directly. `wasm-pack test --node` has no DOM so `HtmlAudioElement` is unavailable in that environment. The trait is defined in `ambient_audio.rs`; `HtmlAudioWrapper(HtmlAudioElement)` is the real browser implementation; `MockAudioHandle { calls: RefCell<Vec<String>> }` is the test implementation injected by the `wasm-bindgen-test`. See contracts/components.md §Host-testable projection pattern for the full trait signature and mock body.

### Companion runtime slots (not part of the state enum)

Two `RwSignal<Option<Box<dyn AudioElementHandle>>>` slots — `current_audio` and `previous_audio` — hold the actual element handles. Their occupancy mirrors the state:

| State | `current_audio` | `previous_audio` |
|---|---|---|
| `Idle` | `None` | `None` |
| `Playing { _ }` | `Some(el)` | `None` |
| `Paused { _ }` | `Some(el)` (paused) | `None` |
| `CrossFading { _, _ }` | `Some(incoming)` | `Some(outgoing)` |
| `FadingOut { _ }` | `Some(el)` (ramping down) | `None` |

A fade timer (also a `RwSignal<Option<IntervalHandle>>`) drives the 200 ms / 300 ms ramps. Dropping the `IntervalHandle` cancels the ramp (used when a new transition pre-empts an in-flight ramp — e.g., resume cancels a fade-out).

### Pre-emption rules

- A new transition starting while a ramp is in flight cancels the in-flight ramp and starts a fresh ramp. The audio elements are re-used where possible (e.g., a fade-out cancelled by a resume keeps the same `<audio>` element and just reverses the ramp direction).
- A track change while in `Paused` does NOT cross-fade; nothing is audible to fade. The state moves to `Paused { track: new }` and the previous element is dropped. On resume, the new track fades in.
- A track change while in `Idle` is a no-op (no audio is playing; the new track is recorded in `ambient_sound_type` and will start fading in if/when the gate transitions on).
- A `track → None` transition from `Playing` enters `FadingOut`. A `None → non-None` transition from `Playing` is rare (the state would be `Idle` if `None` is selected); it can occur when the user enables the checkbox first (state stays `Idle` because `type == None`), then picks a track (rising-edge fade-in to `Playing`).

### Non-persistence rationale

The driver state is intentionally not serialised. On app restart with `ambient_sound_enabled = true` and `ambient_sound_type = Rain` already persisted, the driver starts in `Idle`. When the user starts a focus session, the gate goes high and the driver transitions `Idle → Playing { Rain }` with a 200 ms fade-in. Cold-start audio playback is gated on a user action (Start click) regardless of persisted state — this aligns with the macOS WKWebView autoplay restriction (see [research.md](./research.md) §Decision 1) and with the user's expectation that ambient sound starts when the timer starts, not before.
