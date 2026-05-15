// Ambient background sound driver — feature 004.
//
// UI-side side-effect manager. Owns a state machine that controls
// looping playback through one or two audio handles abstracted
// behind the `AudioElementHandle` trait so the state machine
// itself is host-testable (`cargo test` on the host has no DOM /
// Web Audio runtime).
//
// Browser-side implementation: `WebAudioWrapper` — a Web Audio
// API path using `AudioBufferSourceNode.loop=true` for
// sample-accurate gapless looping. (An earlier `HtmlAudioElement`
// implementation was replaced because HTML5 `<audio loop>` on
// WebKit / WKWebView is not sample-accurate: MP3 carries LAME
// priming + end-padding, and even WAV has a ~10–50 ms perceptible
// seam every wrap.)
//
// Mirrors the host-testable projection pattern from
// `crate::components::icon::IconClass` (feature 003) — the trait is
// the seam between the pure state machine (testable with
// `MockAudioHandle`) and the browser-side `WebAudioWrapper`.
//
// State machine + transition arcs documented in
// `specs/004-ambient-sounds/contracts/components.md` §3.
//
// Engine impact: zero. The driver reads timer state via the gate
// signal computed by the timer component (`current_mode`,
// `is_running`, `is_paused`, `is_auto_paused`, `time_remaining_secs`)
// and reads ambient settings via `Settings::notifications`. It does
// not import anything from `crate::engine`, does not call any Tauri
// command, and does not emit any new engine event. Principle I
// preserved by construction.
//
// Lint allowance: the driver's body has many short helpers
// (`apply_volume`, `cancel_ramp`, etc.) and a deep match on
// `AmbientAudioState`; `must_use_candidate` and `too_many_lines`
// would fire on the public API without buying us anything (the
// public functions are invoked from `timer/mod.rs` for side effects
// and don't bind a return value).
#![allow(clippy::must_use_candidate, clippy::too_many_lines)]

use std::rc::Rc;

use presto_ipc::AmbientSoundType;

/// Asset paths for each non-`None` `AmbientSoundType`. Returns
/// `None` for `AmbientSoundType::None` — that variant has no asset
/// and no playback.
///
/// Match-exhaustive on the closed enum so a new variant fails the
/// build here, not silently at runtime with a missing file.
#[must_use]
pub const fn asset_path(t: AmbientSoundType) -> Option<&'static str> {
    // All tracks vendored as WAV (PCM 16-bit). MP3 carries LAME
    // priming + end-padding bytes that the Web Audio
    // `AudioBufferSourceNode` decoded-buffer playback path emits
    // as ~26 ms of silence at every loop wrap — a perceptible
    // seam. WAV has no codec padding, so the seam is
    // sample-tight on WebKit. Organic tracks are downsampled to
    // 22.05 kHz stereo + crossfade-swap loop-prepped (recipe in
    // `src/assets/audio/README.md`) to keep each file under the
    // 2 MB asset cap (~1.7 MB each).
    match t {
        AmbientSoundType::None => None,
        AmbientSoundType::Rain => Some("/assets/audio/ambient/rain.wav"),
        AmbientSoundType::Fire => Some("/assets/audio/ambient/fire.wav"),
        AmbientSoundType::Library => Some("/assets/audio/ambient/library.wav"),
        AmbientSoundType::Fan => Some("/assets/audio/ambient/fan.wav"),
        AmbientSoundType::Storm => Some("/assets/audio/ambient/storm.wav"),
        AmbientSoundType::WhiteNoise => Some("/assets/audio/ambient/white-noise.wav"),
        AmbientSoundType::Wind => Some("/assets/audio/ambient/wind.wav"),
        AmbientSoundType::PinkNoise => Some("/assets/audio/ambient/pink-noise.wav"),
        AmbientSoundType::BrownNoise => Some("/assets/audio/ambient/brown-noise.wav"),
        AmbientSoundType::Binaural => Some("/assets/audio/ambient/binaural.wav"),
    }
}

/// Opaque error returned by `AudioElementHandle::play`.
///
/// Signals a browser-side `.play()` rejection (typically an
/// autoplay-policy block). The driver swallows the error — the user
/// can re-press Start to retry — so the payload is intentionally
/// empty.
#[derive(Debug, Clone, Copy)]
pub struct AudioPlayError;

/// Host-testable abstraction over a browser-side audio handle.
///
/// Five methods cover the surface the state machine needs:
/// switching the source URL, setting volume, starting playback,
/// stopping playback, and reading current time. Host tests inject
/// `MockAudioHandle`; the wasm target injects `WebAudioWrapper`,
/// which routes through `AudioContext.decodeAudioData` and
/// `AudioBufferSourceNode.loop=true` for sample-accurate gapless
/// looping.
pub trait AudioElementHandle {
    /// Set the source URL to fetch + decode. Cache-friendly: a
    /// second call with the same URL is a no-op (the Web Audio
    /// implementation keeps a process-wide `AudioBuffer` cache).
    /// Stops any currently-playing source — switching tracks
    /// implies the previous source is no longer wanted.
    fn set_src(&self, src: &str);
    /// Set the playback gain — `0.0..=1.0`. The Web Audio
    /// implementation maps this onto a `GainNode` `AudioParam`;
    /// out-of-range values clamp silently at the node.
    fn set_volume(&self, vol: f64);
    /// Start playback. Each call creates a fresh source node from
    /// the cached buffer (Web Audio rule: `AudioBufferSourceNode`
    /// is single-use). If decode is still in flight, the start is
    /// queued and fires once the buffer lands.
    ///
    /// # Errors
    /// Returns `AudioPlayError` if the browser audio engine
    /// rejects the start. The driver swallows the error; the user
    /// can re-press Start to retry.
    fn play(&self) -> Result<(), AudioPlayError>;
    /// Stop the current source and drop it. Web Audio sources are
    /// single-use, so the next `play()` will create a new source
    /// from the cached buffer (loop restarts from sample 0 rather
    /// than resuming from the pause instant — acceptable because
    /// the driver's fade ramps mask the discontinuity).
    fn pause(&self);
    /// Current playback position (seconds) for diagnostics; the
    /// driver itself does not branch on this value. Returns `0.0`
    /// when no source is live.
    fn current_time(&self) -> f64;
}

/// UI-side runtime state for the ambient-audio driver.
///
/// NOT serialised. NOT on the IPC wire. NOT persisted across
/// restarts. Five variants — see
/// `specs/004-ambient-sounds/data-model.md` §3 for the full
/// transition diagram.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AmbientAudioState {
    /// Nothing playing.
    #[default]
    Idle,
    /// One element playing at the configured slider volume.
    Playing { track: AmbientSoundType },
    /// One element resident at volume 0, paused. Re-enters
    /// `Playing` on resume via a 200 ms fade-in.
    Paused { track: AmbientSoundType },
    /// Two elements alive: outgoing track ramping down, incoming
    /// ramping up. 300 ms simultaneous ramps.
    CrossFading {
        outgoing: AmbientSoundType,
        incoming: AmbientSoundType,
    },
    /// One element ramping volume → 0 over 200 ms; will be paused +
    /// dropped at ramp end. Transitions to `Idle`.
    FadingOut { track: AmbientSoundType },
}

/// Companion slots holding the actual `AudioElementHandle` instances.
///
/// `current` is the handle associated with the latest entered
/// `Playing` / `Paused` / `FadingOut` / `CrossFading.incoming` state.
/// `previous` is occupied only during a cross-fade — it holds the
/// outgoing handle. Both are `None` in `Idle`.
struct Slots<H: AudioElementHandle> {
    current: Option<Rc<H>>,
    previous: Option<Rc<H>>,
}

impl<H: AudioElementHandle> Default for Slots<H> {
    fn default() -> Self {
        Self {
            current: None,
            previous: None,
        }
    }
}

/// Active ramp tracker. The driver schedules ramps as discrete
/// updates: every `tick()` call advances `elapsed_ms` by `step_ms`
/// and recomputes the volumes from the start/target endpoints. When
/// `elapsed_ms >= total_ms` the ramp is complete and the driver
/// fires the post-ramp transition.
///
/// `step_ms` is informational only — the host-test pattern lets the
/// test drive `tick()` in arbitrary chunks; the production wiring
/// in `timer/mod.rs` uses a `set_interval` of ~16 ms (60 Hz).
#[derive(Debug, Clone)]
struct Ramp {
    total_ms: u32,
    elapsed_ms: u32,
    /// Outgoing element ramp endpoints — `None` for fade-in only
    /// arcs (Idle → Playing, Paused → Playing).
    out_from: Option<f64>,
    out_to: Option<f64>,
    /// Incoming element ramp endpoints — always Some during a ramp.
    in_from: f64,
    in_to: f64,
}

impl Ramp {
    fn progress(&self) -> f64 {
        if self.total_ms == 0 {
            return 1.0;
        }
        f64::from(self.elapsed_ms.min(self.total_ms)) / f64::from(self.total_ms)
    }

    const fn done(&self) -> bool {
        self.elapsed_ms >= self.total_ms
    }
}

/// The ambient-sound driver. Generic over `AudioElementHandle` for
/// host-testability; the production wiring instantiates it as
/// `AmbientAudio<HtmlAudioWrapper>`.
///
/// Factory is supplied at construction so the driver creates new
/// handles on demand (cross-fade spawns a second element); the
/// production wiring passes a closure that calls
/// `HtmlAudioElement::new()`.
pub struct AmbientAudio<H: AudioElementHandle> {
    state: AmbientAudioState,
    slots: Slots<H>,
    ramp: Option<Ramp>,
    /// Cached current target volume (0..=100). Stored separately so
    /// volume-while-Paused updates land on resume without the
    /// element waking up.
    target_volume: u32,
    /// Factory that produces a fresh element handle. Called once per
    /// non-`None` track introduction.
    factory: Rc<dyn Fn() -> Rc<H>>,
}

/// Public API surface invoked from the timer-component gate
/// effect. Each operation is at most one state transition. Ramps
/// are advanced separately via `tick()`.
impl<H: AudioElementHandle + 'static> AmbientAudio<H> {
    /// Construct a fresh driver. `factory` produces a new element
    /// handle on demand. The driver starts in `Idle` with no
    /// resident handles.
    pub fn new(factory: Rc<dyn Fn() -> Rc<H>>) -> Self {
        Self {
            state: AmbientAudioState::Idle,
            slots: Slots::default(),
            ramp: None,
            target_volume: 50,
            factory,
        }
    }

    /// Read-only view of the current state — exposed for the
    /// host-side state-machine tests.
    #[must_use]
    pub const fn state(&self) -> &AmbientAudioState {
        &self.state
    }

    /// Arc 1: Idle → Playing(t). Spawn a fresh handle, point it at
    /// the asset, start a 200 ms fade-in from 0.0 to
    /// `volume / 100.0`.
    ///
    /// No-op if `track == None` (no asset to play). No-op if not in
    /// Idle (the caller is the timer gate effect, which only invokes
    /// `start` on rising edge from `Idle`).
    pub fn start(&mut self, track: AmbientSoundType, volume: u32) {
        let Some(path) = asset_path(track) else {
            // Defensive: caller should not invoke start with None.
            // Drop silently — preserves engine purity (no toast, no
            // panic).
            return;
        };
        if !matches!(self.state, AmbientAudioState::Idle) {
            return;
        }
        // Principle III: no defensive state mutation outside the
        // success path. Set the cached target only after the guards
        // pass so a rejected `start` does not leave a stale ceiling
        // around for the next ramp.
        self.target_volume = volume;
        let handle = (self.factory)();
        handle.set_src(path);
        handle.set_volume(0.0);
        let _ = handle.play();
        self.slots.current = Some(handle);
        self.state = AmbientAudioState::Playing { track };
        self.ramp = Some(Ramp {
            total_ms: 200,
            elapsed_ms: 0,
            out_from: None,
            out_to: None,
            in_from: 0.0,
            in_to: f64::from(volume) / 100.0,
        });
    }

    /// Arc 2: `Playing(t)` → `Paused(t)` OR `CrossFading(_, incoming)`
    /// → `Paused(incoming)`. 200 ms fade-out then pause on the resident
    /// element(s). Cancels any in-flight ramp (the new fade-out
    /// starts from whatever volume each element currently has, per
    /// the pre-emption rule).
    ///
    /// When pausing mid-cross-fade we retarget BOTH the incoming
    /// (`current` slot) and outgoing (`previous` slot) elements'
    /// ramps to 0.0 over 200 ms so the user doesn't hear the
    /// cross-fade play through. The state lands on the incoming
    /// track (that's what the next resume will fade back in). The
    /// outgoing element gets paused + released when the ramp
    /// completes (see the `Paused` arm of `tick()`'s post-ramp
    /// dispatch).
    pub fn pause(&mut self) {
        match self.state.clone() {
            AmbientAudioState::Playing { track } => {
                let current_volume =
                    self.in_flight_target_or(f64::from(self.target_volume) / 100.0);
                self.state = AmbientAudioState::Paused { track };
                self.ramp = Some(Ramp {
                    total_ms: 200,
                    elapsed_ms: 0,
                    out_from: None,
                    out_to: None,
                    in_from: current_volume,
                    in_to: 0.0,
                });
            }
            AmbientAudioState::CrossFading { incoming, .. } => {
                // FR-008: pausing mid-cross-fade must not leak audio.
                // Retarget both elements' ramps to 0 over 200 ms; the
                // post-ramp `Paused` dispatch releases the outgoing
                // slot.
                let (out_v, in_v) = self.cross_fade_current_volumes();
                self.state = AmbientAudioState::Paused { track: incoming };
                self.ramp = Some(Ramp {
                    total_ms: 200,
                    elapsed_ms: 0,
                    out_from: Some(out_v),
                    out_to: Some(0.0),
                    in_from: in_v,
                    in_to: 0.0,
                });
            }
            AmbientAudioState::Idle
            | AmbientAudioState::Paused { .. }
            | AmbientAudioState::FadingOut { .. } => {
                // No-op: nothing playing, already paused, or already
                // tearing down — caller's gate Effect should not
                // dispatch pause from these states, but be lenient.
            }
        }
    }

    /// Arc 3: Paused(t) → Playing(t). `.play()` + 200 ms fade-in
    /// from 0.0 to `volume / 100.0`. Updates the stored target so
    /// future ramps use the latest value.
    pub fn resume(&mut self, volume: u32) {
        self.target_volume = volume;
        let AmbientAudioState::Paused { track } = self.state.clone() else {
            return;
        };
        if let Some(h) = self.slots.current.as_ref() {
            h.set_volume(0.0);
            let _ = h.play();
        }
        self.state = AmbientAudioState::Playing { track };
        self.ramp = Some(Ramp {
            total_ms: 200,
            elapsed_ms: 0,
            out_from: None,
            out_to: None,
            in_from: 0.0,
            in_to: f64::from(volume) / 100.0,
        });
    }

    /// Arc 4: Playing(old) → CrossFading(old, new). Spawns a second
    /// element for the new track; 300 ms simultaneous ramps (outgoing
    /// → 0.0, incoming 0.0 → `volume / 100.0`).
    ///
    /// Idempotent against repeated calls with the same new track:
    /// the second call discards the in-flight ramp and starts a
    /// fresh one (matches the spec's pre-emption rule for rapid-fire
    /// track changes).
    pub fn cross_fade(&mut self, new_track: AmbientSoundType, volume: u32) {
        self.target_volume = volume;
        let Some(new_path) = asset_path(new_track) else {
            // Track-to-None while playing collapses to a fade-out
            // (arc 7). Delegate to the dedicated entry point.
            self.fade_out();
            return;
        };
        let (outgoing_track, current_volume) = match &self.state {
            AmbientAudioState::Playing { track } => {
                let v = self.in_flight_target_or(f64::from(self.target_volume) / 100.0);
                (*track, v)
            }
            AmbientAudioState::CrossFading { incoming, .. } => {
                // Rapid-fire track change: keep the incoming-side
                // element (its asset is already partially loaded);
                // restart the ramp.
                let v = self.in_flight_target_or(0.0);
                (*incoming, v)
            }
            AmbientAudioState::Idle
            | AmbientAudioState::Paused { .. }
            | AmbientAudioState::FadingOut { .. } => {
                // Track change while not actively playing
                // (Idle/Paused) OR while already tearing down
                // (FadingOut) is a settings-only update — no
                // cross-fade. Driver re-evaluates the gate on the
                // next entry to `Idle` and may transition to
                // `Playing(latest_track)` directly.
                return;
            }
        };
        // Move the prior current handle to previous (for outgoing
        // ramp) and spawn a fresh handle for the incoming track.
        if let Some(prev) = self.slots.current.take() {
            self.slots.previous = Some(prev);
        }
        let new_handle = (self.factory)();
        new_handle.set_src(new_path);
        new_handle.set_volume(0.0);
        let _ = new_handle.play();
        self.slots.current = Some(new_handle);
        self.state = AmbientAudioState::CrossFading {
            outgoing: outgoing_track,
            incoming: new_track,
        };
        self.ramp = Some(Ramp {
            total_ms: 300,
            elapsed_ms: 0,
            out_from: Some(current_volume),
            out_to: Some(0.0),
            in_from: 0.0,
            in_to: f64::from(volume) / 100.0,
        });
    }

    /// Arc 10: `Playing(t)` → `Playing(t)` self-arc. Immediate volume
    /// update on the resident element; no fade, no restart. While
    /// in `Paused` / `Idle` / `FadingOut`, updates the stored target
    /// only (no element change). During `CrossFading`, retargets the
    /// incoming ramp's endpoint so a slider drag mid-fade is honoured.
    pub fn set_volume(&mut self, volume: u32) {
        self.target_volume = volume;
        match &self.state {
            AmbientAudioState::Playing { .. } => {
                if let Some(h) = self.slots.current.as_ref() {
                    h.set_volume(f64::from(volume) / 100.0);
                }
                // Re-target any in-flight fade-in ramp so the user
                // sees the new ceiling immediately.
                if let Some(ramp) = self.ramp.as_mut() {
                    ramp.in_to = f64::from(volume) / 100.0;
                }
            }
            AmbientAudioState::CrossFading { .. } => {
                // V2 fix: retarget the incoming ramp endpoint so a
                // slider drag during the 300 ms cross-fade isn't
                // ignored. The outgoing ramp's endpoint stays at 0
                // (the outgoing track is fading to silence regardless
                // of what the user wants the incoming track to settle
                // at).
                if let Some(ramp) = self.ramp.as_mut() {
                    ramp.in_to = f64::from(volume) / 100.0;
                }
            }
            AmbientAudioState::Paused { .. }
            | AmbientAudioState::Idle
            | AmbientAudioState::FadingOut { .. } => {
                // Per pre-emption rules: volume change while
                // not-actively-playing updates the stored target only.
                // On next resume / fade-out-complete / Idle→Playing
                // entry, the new value drives the fade-in.
            }
        }
    }

    /// Arcs 7/8: `Playing` / `Paused` → `FadingOut`. 200 ms fade-out
    /// from the current volume to 0.0 then pause + drop element.
    ///
    /// Also handles arc 6: `CrossFading` → `FadingOut`. Cancels both
    /// in-flight ramps; both elements fade from their current
    /// `.volume` to 0 over 200 ms and are dropped at ramp completion.
    pub fn fade_out(&mut self) {
        match self.state.clone() {
            AmbientAudioState::Playing { track } | AmbientAudioState::Paused { track } => {
                let current_volume =
                    self.in_flight_target_or(f64::from(self.target_volume) / 100.0);
                self.state = AmbientAudioState::FadingOut { track };
                self.ramp = Some(Ramp {
                    total_ms: 200,
                    elapsed_ms: 0,
                    out_from: None,
                    out_to: None,
                    in_from: current_volume,
                    in_to: 0.0,
                });
            }
            AmbientAudioState::CrossFading { incoming, .. } => {
                // Compute both elements' current volumes from the
                // in-flight cross-fade ramp before mutating it. Both
                // elements fade to 0 over 200 ms from their current
                // value.
                let (out_v, in_v) = self.cross_fade_current_volumes();
                self.state = AmbientAudioState::FadingOut { track: incoming };
                self.ramp = Some(Ramp {
                    total_ms: 200,
                    elapsed_ms: 0,
                    out_from: Some(out_v),
                    out_to: Some(0.0),
                    in_from: in_v,
                    in_to: 0.0,
                });
            }
            AmbientAudioState::Idle | AmbientAudioState::FadingOut { .. } => {
                // No-op: already silent or already fading.
            }
        }
    }

    /// Advance any in-flight ramp by `step_ms` milliseconds. Updates
    /// element volumes and, on completion, fires the post-ramp state
    /// transition (`CrossFading` → `Playing(new)`, `FadingOut` →
    /// `Idle`, etc.).
    ///
    /// The production wiring calls this from a `set_interval` at
    /// ~16 ms cadence. Tests call it in arbitrary steps to drive the
    /// state machine deterministically.
    pub fn tick(&mut self, step_ms: u32) {
        let Some(ramp) = self.ramp.as_mut() else {
            return;
        };
        ramp.elapsed_ms = ramp.elapsed_ms.saturating_add(step_ms);
        let progress = ramp.progress();
        let in_v = (ramp.in_to - ramp.in_from).mul_add(progress, ramp.in_from);
        if let Some(h) = self.slots.current.as_ref() {
            h.set_volume(in_v.clamp(0.0, 1.0));
        }
        if let (Some(from), Some(to)) = (ramp.out_from, ramp.out_to) {
            let out_v = (to - from).mul_add(progress, from);
            if let Some(h) = self.slots.previous.as_ref() {
                h.set_volume(out_v.clamp(0.0, 1.0));
            }
        }
        if !ramp.done() {
            return;
        }
        // Ramp complete — fire the post-ramp transition.
        self.ramp = None;
        match self.state.clone() {
            AmbientAudioState::CrossFading { incoming, .. } => {
                // Drop the outgoing element + transition to Playing(incoming).
                if let Some(prev) = self.slots.previous.take() {
                    prev.pause();
                }
                self.state = AmbientAudioState::Playing { track: incoming };
            }
            AmbientAudioState::FadingOut { .. } => {
                // Drop both elements (any cross-fade refugees in
                // `previous` need pausing too) and return to Idle.
                if let Some(prev) = self.slots.previous.take() {
                    prev.pause();
                }
                if let Some(cur) = self.slots.current.take() {
                    cur.pause();
                }
                self.state = AmbientAudioState::Idle;
            }
            AmbientAudioState::Paused { track } => {
                // Pause fade-out completed — actually pause the
                // element(s) now that volume has reached 0. If we
                // entered Paused mid-cross-fade the outgoing handle
                // is still in `previous`; release it here so the
                // next resume only fades the incoming element back
                // in.
                if let Some(prev) = self.slots.previous.take() {
                    prev.pause();
                }
                if let Some(h) = self.slots.current.as_ref() {
                    h.pause();
                }
                // State already updated when pause() was called.
                self.state = AmbientAudioState::Paused { track };
            }
            AmbientAudioState::Playing { .. } | AmbientAudioState::Idle => {
                // Fade-in ramp completed (Playing) — element already
                // at target volume from the last `tick`. Or no ramp
                // should have been in flight (Idle, defensive).
                // Nothing further to do in either case.
            }
        }
    }

    /// Read the in-flight ramp's `in_to` (i.e. the target volume the
    /// driver is fading towards). Falls back to `default` when no
    /// ramp is active.
    fn in_flight_target_or(&self, default: f64) -> f64 {
        // Use the ramp's CURRENT computed value (progress through
        // in_from → in_to) so a cross-fade that gets pre-empted
        // starts its successor ramp from the correct volume, not
        // from the ramp's target.
        self.ramp.as_ref().map_or(default, |ramp| {
            let p = ramp.progress();
            (ramp.in_to - ramp.in_from).mul_add(p, ramp.in_from)
        })
    }

    /// Compute the current volumes of both elements during a
    /// `CrossFading` ramp. Returns `(outgoing_v, incoming_v)`.
    fn cross_fade_current_volumes(&self) -> (f64, f64) {
        self.ramp.as_ref().map_or((0.0, 0.0), |ramp| {
            let p = ramp.progress();
            let out_v = match (ramp.out_from, ramp.out_to) {
                (Some(from), Some(to)) => (to - from).mul_add(p, from),
                _ => 0.0,
            };
            let in_v = (ramp.in_to - ramp.in_from).mul_add(p, ramp.in_from);
            (out_v, in_v)
        })
    }
}

// ---------------------------------------------------------------------
// Browser-side implementation (Web Audio API)
// ---------------------------------------------------------------------
//
// Earlier wiring used `HtmlAudioElement` with `.loop = true` and the
// browser's native looper. That path turned out to be unfixably
// gappy on WebKit / WKWebView: MP3 carries LAME priming + end
// padding the native looper cannot skip (audible click on every
// loop), and even WAV has a ~10-50 ms perceptible seam because the
// HTML5 media element does not loop sample-accurately.
//
// `AudioBufferSourceNode` with `loop = true`, by contrast, is
// defined by the Web Audio spec as bit-perfect sample-accurate
// gapless. The price is async decode: a `decodeAudioData` call has
// to resolve before playback can start. We pay that once per track
// per session and cache the decoded buffer in a process-wide
// `BUFFER_CACHE`, so subsequent plays / cross-fades / pause-resume
// cycles are synchronous.

/// Web Audio-backed `AudioElementHandle` implementation.
///
/// Each instance owns one `GainNode` connected to the shared
/// `AudioContext`'s destination. The actual playing node is an
/// `AudioBufferSourceNode` that is recreated on every play /
/// resume (per Web Audio rules — sources are single-use).
#[cfg(target_arch = "wasm32")]
pub struct WebAudioWrapper {
    /// `Some` once `WebAudioWrapper::new()` succeeded in acquiring
    /// the shared `AudioContext` + a fresh `GainNode`. `None` if
    /// audio capability is unavailable (no Web Audio in the host).
    /// Every method below short-circuits on `None`, never panics —
    /// so the driver remains functional (state machine still
    /// advances, ramps still tick) even on hostless / mocked
    /// environments.
    ctx_and_gain: Option<(web_sys::AudioContext, web_sys::GainNode)>,
    /// Currently-playing source node. `None` between
    /// `pause()` (or before the first decode lands) and the next
    /// `play()`. Single-use per the Web Audio API.
    source: std::rc::Rc<std::cell::RefCell<Option<web_sys::AudioBufferSourceNode>>>,
    /// Track URL the caller last requested via `set_src`. Used to
    /// detect "decode finished but caller has since switched
    /// tracks" so the stale buffer doesn't pop in.
    current_src: std::rc::Rc<std::cell::RefCell<String>>,
    /// True between a `play()` call that found no cached buffer
    /// and the moment the async decode resolves. The async resolver
    /// checks this flag and starts playback once the buffer lands.
    play_pending: std::rc::Rc<std::cell::Cell<bool>>,
    /// `AudioContext.currentTime` at the most recent `start()` —
    /// surfaced via `current_time()` for the diagnostics path.
    play_start: std::rc::Rc<std::cell::Cell<f64>>,
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    /// Process-wide singleton `AudioContext`. WebKit caps each page
    /// at ~4-6 contexts before `AudioContext::new()` silently fails;
    /// sharing one across every wrapper (and across cross-fades that
    /// transiently spawn two wrappers) keeps us well under the cap.
    static SHARED_CTX: std::cell::RefCell<Option<web_sys::AudioContext>> =
        const { std::cell::RefCell::new(None) };
    /// Decoded `AudioBuffer` cache keyed by asset URL. Decode is
    /// idempotent so concurrent decodes of the same URL just race
    /// to write the same buffer — last writer wins, both readers
    /// observe a usable buffer. Held across cross-fades so a Rain
    /// → Fire → Rain bounce decodes once, plays twice.
    static BUFFER_CACHE: std::cell::RefCell<
        std::collections::HashMap<String, web_sys::AudioBuffer>,
    > = std::cell::RefCell::new(std::collections::HashMap::new());
}

#[cfg(target_arch = "wasm32")]
fn shared_audio_ctx() -> Option<web_sys::AudioContext> {
    SHARED_CTX.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = web_sys::AudioContext::new().ok();
        }
        // Idempotent on a running context. Necessary on macOS
        // WKWebView when the context was created outside a gesture
        // and is sitting in `suspended` state.
        if let Some(ctx) = slot.as_ref() {
            let _ = ctx.resume();
        }
        slot.clone()
    })
}

/// Halt an `AudioBufferSourceNode`. The `stop` family of methods
/// is currently deprecation-warning-tagged in `web_sys` 0.3
/// pending an API refresh, but it remains the correct call to
/// halt a buffer source per the Web Audio spec — there is no
/// non-deprecated alternative. Localised allow keeps the warning
/// out of the broader build.
#[cfg(target_arch = "wasm32")]
#[allow(deprecated)]
fn stop_source(node: &web_sys::AudioBufferSourceNode) {
    let _ = node.stop_with_when(0.0);
}

#[cfg(target_arch = "wasm32")]
impl WebAudioWrapper {
    /// Build a fresh wrapper. Infallible: if the shared
    /// `AudioContext` cannot be constructed (no audio hardware,
    /// headless host, autoplay policy refusal) the wrapper stores
    /// `None` and every method silently no-ops. The driver's
    /// state machine still advances normally; just no sound.
    /// This contract is what lets the `with_driver` factory drop
    /// its `.expect(...)` — `new()` is never a panic risk.
    #[must_use]
    pub fn new() -> Self {
        let ctx_and_gain = (|| -> Option<(web_sys::AudioContext, web_sys::GainNode)> {
            let ctx = shared_audio_ctx()?;
            let gain = ctx.create_gain().ok()?;
            gain.gain().set_value(0.0);
            gain.connect_with_audio_node(&ctx.destination()).ok()?;
            Some((ctx, gain))
        })();
        Self {
            ctx_and_gain,
            source: std::rc::Rc::new(std::cell::RefCell::new(None)),
            current_src: std::rc::Rc::new(std::cell::RefCell::new(String::new())),
            play_pending: std::rc::Rc::new(std::cell::Cell::new(false)),
            play_start: std::rc::Rc::new(std::cell::Cell::new(0.0)),
        }
    }

    /// Start a fresh `AudioBufferSourceNode` from `buffer`, connect
    /// it to the wrapper's `GainNode`, kick it off looping. Stops
    /// any prior source first — defensive; the call sites already
    /// clear the slot but a leaked source would double-play.
    fn start_source(&self, buffer: &web_sys::AudioBuffer) {
        let Some((ctx, gain)) = self.ctx_and_gain.as_ref() else {
            return;
        };
        if let Some(prev) = self.source.borrow_mut().take() {
            stop_source(&prev);
        }
        let Ok(source) = ctx.create_buffer_source() else {
            return;
        };
        source.set_buffer(Some(buffer));
        source.set_loop(true);
        if source.connect_with_audio_node(gain).is_err() {
            return;
        }
        if source.start().is_err() {
            return;
        }
        self.play_start.set(ctx.current_time());
        *self.source.borrow_mut() = Some(source);
    }
}

/// Create + resume the shared ambient `AudioContext`.
///
/// **Must be called from inside a user-gesture handler**
/// (start-button click, keyboard shortcut) so macOS `WKWebView`
/// unlocks the autoplay policy for the context. Subsequent
/// `WebAudioWrapper::new()` calls (which happen from inside the
/// gate Effect, outside the gesture) reuse the already-resumed
/// singleton. Idempotent; cheap once primed.
///
/// Mirrors the `prime_audio_context()` pattern at
/// `crate::components::timer::prime_audio_context` for the chime
/// `AudioContext`. The two functions independently prime two
/// separate `AudioContext`s — both share a single gesture
/// unlock from the user's Start click.
#[cfg(target_arch = "wasm32")]
pub fn prime_ambient_audio() {
    let _ = shared_audio_ctx();
}

#[cfg(not(target_arch = "wasm32"))]
pub const fn prime_ambient_audio() {}

#[cfg(target_arch = "wasm32")]
impl Default for WebAudioWrapper {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for WebAudioWrapper {
    /// Disconnect the wrapper's `GainNode` from the shared
    /// `AudioContext.destination` when the wrapper is dropped.
    ///
    /// Web Audio nodes are kept alive by the audio graph as long
    /// as they remain connected, regardless of JS-side handle
    /// references. Without this drop the `GainNode` (and any
    /// `AudioBufferSourceNode` still hanging off it) would leak
    /// every time the driver drops a wrapper — most visibly on
    /// every cross-fade completion, which calls `slots.previous.take()`
    /// to drop the outgoing wrapper.
    ///
    /// Disconnecting also implicitly stops any source still routed
    /// through this gain; we additionally `stop_source()` first as
    /// belt-and-braces for engines that don't synchronously halt
    /// disconnected sources.
    fn drop(&mut self) {
        if let Some(prev) = self.source.borrow_mut().take() {
            stop_source(&prev);
        }
        if let Some((_, gain)) = self.ctx_and_gain.as_ref() {
            let _ = gain.disconnect();
        }
    }
}

/// Callback invoked once a decoded buffer lands in
/// `BUFFER_CACHE`. Boxed because each `set_src` call site
/// captures a different set of `Rc`'d wrapper fields.
#[cfg(target_arch = "wasm32")]
type DecodeNotify = Box<dyn FnOnce(&web_sys::AudioBuffer)>;

/// Shared fetch + `decodeAudioData` pipeline. `notify` is invoked
/// after the buffer lands in `BUFFER_CACHE`; the wrapper-side
/// caller uses it to trigger pending playback. `None` = pure
/// pre-warm.
#[cfg(target_arch = "wasm32")]
fn spawn_decode(ctx: web_sys::AudioContext, url: String, notify: Option<DecodeNotify>) {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::{spawn_local, JsFuture};
    spawn_local(async move {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Ok(resp_val) = JsFuture::from(window.fetch_with_str(&url)).await else {
            return;
        };
        let Ok(resp) = resp_val.dyn_into::<web_sys::Response>() else {
            return;
        };
        let Ok(ab_promise) = resp.array_buffer() else {
            return;
        };
        let Ok(ab_val) = JsFuture::from(ab_promise).await else {
            return;
        };
        let Ok(array_buffer) = ab_val.dyn_into::<js_sys::ArrayBuffer>() else {
            return;
        };
        let Ok(decode_promise) = ctx.decode_audio_data(&array_buffer) else {
            return;
        };
        let Ok(buf_val) = JsFuture::from(decode_promise).await else {
            return;
        };
        let Ok(buffer) = buf_val.dyn_into::<web_sys::AudioBuffer>() else {
            return;
        };
        BUFFER_CACHE.with(|c| {
            c.borrow_mut().insert(url, buffer.clone());
        });
        if let Some(notify) = notify {
            notify(&buffer);
        }
    });
}

#[cfg(target_arch = "wasm32")]
impl AudioElementHandle for WebAudioWrapper {
    fn set_src(&self, src: &str) {
        if *self.current_src.borrow() == src {
            return;
        }
        *self.current_src.borrow_mut() = src.to_string();
        if let Some(prev) = self.source.borrow_mut().take() {
            stop_source(&prev);
        }
        let Some((ctx, gain)) = self.ctx_and_gain.as_ref() else {
            return;
        };
        // Cache hit: nothing to do until `play()` fires.
        let cached = BUFFER_CACHE.with(|c| c.borrow().get(src).cloned());
        if cached.is_some() {
            return;
        }
        // Cache miss: kick off decode. If a `play()` is pending by
        // the time decode resolves, the closure starts the source.
        let source_slot = self.source.clone();
        let current_src = self.current_src.clone();
        let play_pending = self.play_pending.clone();
        let play_start = self.play_start.clone();
        let ctx_for_start = ctx.clone();
        let gain_for_start = gain.clone();
        let url = src.to_string();
        let url_for_check = url.clone();
        let notify = Box::new(move |buffer: &web_sys::AudioBuffer| {
            // Stale: the caller has since switched tracks. Decoded
            // buffer is still in the cache for next time; just don't
            // play this one.
            if *current_src.borrow() != url_for_check {
                return;
            }
            if !play_pending.get() {
                return;
            }
            play_pending.set(false);
            if let Some(prev) = source_slot.borrow_mut().take() {
                stop_source(&prev);
            }
            let Ok(source) = ctx_for_start.create_buffer_source() else {
                return;
            };
            source.set_buffer(Some(buffer));
            source.set_loop(true);
            if source.connect_with_audio_node(&gain_for_start).is_err() {
                return;
            }
            if source.start().is_err() {
                return;
            }
            play_start.set(ctx_for_start.current_time());
            *source_slot.borrow_mut() = Some(source);
        });
        spawn_decode(ctx.clone(), url, Some(notify));
    }

    fn set_volume(&self, vol: f64) {
        let Some((_, gain)) = self.ctx_and_gain.as_ref() else {
            return;
        };
        // GainNode AudioParam takes `f32`. The driver only ever
        // passes 0..=1; out-of-range values would silently clamp at
        // the node, not throw.
        #[allow(clippy::cast_possible_truncation)]
        gain.gain().set_value(vol as f32);
    }

    fn play(&self) -> Result<(), AudioPlayError> {
        // No source URL set → nothing to play. Returning without
        // touching `play_pending` avoids a footgun where a `play()`
        // called before `set_src` would arm `play_pending=true`,
        // then the next `set_src` would auto-start playback even
        // though the caller never explicitly requested it.
        if self.current_src.borrow().is_empty() {
            return Ok(());
        }
        let cached = BUFFER_CACHE.with(|c| c.borrow().get(&*self.current_src.borrow()).cloned());
        if let Some(buffer) = cached {
            self.start_source(&buffer);
            self.play_pending.set(false);
        } else {
            // Decode still in flight. Mark pending; the
            // `spawn_decode` notify closure starts the source when
            // the buffer lands.
            self.play_pending.set(true);
        }
        Ok(())
    }

    fn pause(&self) {
        self.play_pending.set(false);
        if let Some(prev) = self.source.borrow_mut().take() {
            stop_source(&prev);
        }
    }

    fn current_time(&self) -> f64 {
        // Only meaningful when a source is actively running. Between
        // `pause()` and the next `play()` the `play_start` anchor is
        // stale, so reporting `ctx.current_time() - play_start` would
        // grow as wall-clock — misleading vs. the
        // `HtmlAudioElement.currentTime` it replaces (frozen at the
        // pause instant). Driver does not branch on this value; it is
        // surfaced only for diagnostics. Returning 0 when no source is
        // live is the safe answer.
        if self.source.borrow().is_none() {
            return 0.0;
        }
        let Some((ctx, _)) = self.ctx_and_gain.as_ref() else {
            return 0.0;
        };
        ctx.current_time() - self.play_start.get()
    }
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    /// Process-wide singleton driver. Materialised on the first
    /// `with_driver` call inside the timer component's gate effect.
    /// `RefCell` is fine because Leptos's CSR runtime is single-
    /// threaded (one WebAssembly main thread per app instance).
    static DRIVER: std::cell::RefCell<Option<AmbientAudio<WebAudioWrapper>>> =
        const { std::cell::RefCell::new(None) };
}

/// Run a closure against the process-wide driver.
///
/// Wasm-only. Driver is materialised on the first call. Factory
/// is infallible — `WebAudioWrapper::new()` stores `None`
/// internally if audio capability is unavailable and the wrapper
/// silently no-ops.
#[cfg(target_arch = "wasm32")]
pub fn with_driver<R>(f: impl FnOnce(&mut AmbientAudio<WebAudioWrapper>) -> R) -> Option<R> {
    DRIVER.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            let factory: Rc<dyn Fn() -> Rc<WebAudioWrapper>> =
                Rc::new(|| Rc::new(WebAudioWrapper::new()));
            *slot = Some(AmbientAudio::new(factory));
        }
        slot.as_mut().map(f)
    })
}

/// Host-target stub so non-wasm test runs don't trip on the
/// `with_driver` import. Returns `None` because there's no driver
/// to drive without a DOM.
#[cfg(not(target_arch = "wasm32"))]
pub fn with_driver<R>(_f: impl FnOnce(&mut AmbientAudio<WebAudioWrapper>) -> R) -> Option<R> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub struct WebAudioWrapper;

#[cfg(not(target_arch = "wasm32"))]
impl WebAudioWrapper {
    /// Host-side stub matching the wasm-side `new()` surface so
    /// the factory compiles unconditionally.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for WebAudioWrapper {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl AudioElementHandle for WebAudioWrapper {
    fn set_src(&self, _src: &str) {}
    fn set_volume(&self, _vol: f64) {}
    fn play(&self) -> Result<(), AudioPlayError> {
        Ok(())
    }
    fn pause(&self) {}
    fn current_time(&self) -> f64 {
        0.0
    }
}

// ---------------------------------------------------------------------
// Tests — host-testable via MockAudioHandle (no DOM dependency).
// Coverage: nine pre-emption scenarios per
// `specs/004-ambient-sounds/contracts/components.md` §3.
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{
        asset_path, AmbientAudio, AmbientAudioState, AmbientSoundType, AudioElementHandle,
    };
    use std::cell::RefCell;
    use std::collections::HashSet;
    use std::rc::Rc;

    /// Every non-`None` variant must resolve to a `.wav` path under
    /// `/assets/audio/ambient/`. The path strings are the only thing
    /// `WebAudioWrapper.set_src` is given — a typo here = a silent
    /// 404 in production with no compile-time tripwire.
    #[test]
    fn asset_path_returns_wav_for_every_non_none_variant() {
        const VARIANTS: &[AmbientSoundType] = &[
            AmbientSoundType::Rain,
            AmbientSoundType::Fire,
            AmbientSoundType::Library,
            AmbientSoundType::Fan,
            AmbientSoundType::Storm,
            AmbientSoundType::WhiteNoise,
            AmbientSoundType::Wind,
            AmbientSoundType::PinkNoise,
            AmbientSoundType::BrownNoise,
            AmbientSoundType::Binaural,
        ];
        for v in VARIANTS {
            let p = asset_path(*v).unwrap_or_else(|| panic!("no asset path for {v:?}"));
            assert!(
                p.starts_with("/assets/audio/ambient/"),
                "{v:?} path not under /assets/audio/ambient/: {p}",
            );
            assert!(
                std::path::Path::new(p)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("wav")),
                "{v:?} path not a .wav (Web Audio loop seam relies on no MP3 priming): {p}",
            );
        }
        assert_eq!(asset_path(AmbientSoundType::None), None);
    }

    /// Path uniqueness: two enum variants pointing at the same file
    /// would silently collapse the cache key in `BUFFER_CACHE`, so a
    /// duplicate path is a real bug, not a stylistic nit.
    #[test]
    fn asset_paths_are_unique() {
        const VARIANTS: &[AmbientSoundType] = &[
            AmbientSoundType::Rain,
            AmbientSoundType::Fire,
            AmbientSoundType::Library,
            AmbientSoundType::Fan,
            AmbientSoundType::Storm,
            AmbientSoundType::WhiteNoise,
            AmbientSoundType::Wind,
            AmbientSoundType::PinkNoise,
            AmbientSoundType::BrownNoise,
            AmbientSoundType::Binaural,
        ];
        let mut seen: HashSet<&'static str> = HashSet::new();
        for v in VARIANTS {
            let p = asset_path(*v).expect("path");
            assert!(seen.insert(p), "duplicate asset path: {p}");
        }
    }

    #[derive(Default)]
    struct MockAudioHandle {
        calls: RefCell<Vec<String>>,
    }

    impl AudioElementHandle for MockAudioHandle {
        fn set_src(&self, src: &str) {
            self.calls.borrow_mut().push(format!("set_src:{src}"));
        }
        fn set_volume(&self, vol: f64) {
            // Round to 3 decimal places for stable assertion text.
            self.calls.borrow_mut().push(format!("set_volume:{vol:.3}"));
        }
        fn play(&self) -> Result<(), super::AudioPlayError> {
            self.calls.borrow_mut().push("play".into());
            Ok(())
        }
        fn pause(&self) {
            self.calls.borrow_mut().push("pause".into());
        }
        fn current_time(&self) -> f64 {
            0.0
        }
    }

    /// Factory state: each call to the factory closure pushes a
    /// fresh `MockAudioHandle` onto the produced vector AND returns
    /// it. Tests can inspect the vector to see every handle that was
    /// materialised over the run.
    struct Factory {
        produced: Rc<RefCell<Vec<Rc<MockAudioHandle>>>>,
    }

    impl Factory {
        fn new() -> Self {
            Self {
                produced: Rc::new(RefCell::new(Vec::new())),
            }
        }
        fn closure(&self) -> Rc<dyn Fn() -> Rc<MockAudioHandle>> {
            let produced = self.produced.clone();
            Rc::new(move || {
                let h = Rc::new(MockAudioHandle::default());
                produced.borrow_mut().push(h.clone());
                h
            })
        }
        fn count(&self) -> usize {
            self.produced.borrow().len()
        }
        fn handle(&self, idx: usize) -> Rc<MockAudioHandle> {
            self.produced.borrow()[idx].clone()
        }
    }

    fn calls_contains(h: &Rc<MockAudioHandle>, pat: &str) -> bool {
        h.calls.borrow().iter().any(|c| c.contains(pat))
    }

    /// Scenario 1: happy path through `Idle` → `Playing` → `Paused` →
    /// `Playing` → `CrossFading` → `Playing` → `FadingOut` → `Idle`.
    #[test]
    fn state_idle_to_playing_happy_path() {
        let factory = Factory::new();
        let mut driver = AmbientAudio::new(factory.closure());

        assert_eq!(driver.state(), &AmbientAudioState::Idle);

        driver.start(AmbientSoundType::Rain, 50);
        assert_eq!(
            driver.state(),
            &AmbientAudioState::Playing {
                track: AmbientSoundType::Rain
            }
        );
        assert_eq!(factory.count(), 1);
        let rain_handle = factory.handle(0);
        assert!(calls_contains(
            &rain_handle,
            "set_src:/assets/audio/ambient/rain.wav"
        ));
        assert!(calls_contains(&rain_handle, "play"));

        // Drive the 200 ms fade-in to completion.
        driver.tick(200);
        assert_eq!(
            driver.state(),
            &AmbientAudioState::Playing {
                track: AmbientSoundType::Rain
            }
        );
    }

    /// Scenario 2: pause during Playing → Paused. Driver fades from
    /// current volume to 0 over 200 ms then pauses the element.
    #[test]
    fn pause_during_playing() {
        let factory = Factory::new();
        let mut driver = AmbientAudio::new(factory.closure());
        driver.start(AmbientSoundType::Rain, 60);
        driver.tick(200); // settle to Playing at target

        driver.pause();
        assert_eq!(
            driver.state(),
            &AmbientAudioState::Paused {
                track: AmbientSoundType::Rain
            }
        );
        // Drive the fade-out to completion; element must receive a
        // pause call.
        driver.tick(200);
        let h = factory.handle(0);
        assert!(calls_contains(&h, "pause"));
    }

    /// Scenario 3: resume from Paused → Playing. Element receives
    /// a new play call and fades back up.
    #[test]
    fn resume_from_paused() {
        let factory = Factory::new();
        let mut driver = AmbientAudio::new(factory.closure());
        driver.start(AmbientSoundType::Fire, 40);
        driver.tick(200);
        driver.pause();
        driver.tick(200);

        // Clear the call history for the existing element to
        // isolate the resume-side calls.
        let h = factory.handle(0);
        h.calls.borrow_mut().clear();

        driver.resume(40);
        assert_eq!(
            driver.state(),
            &AmbientAudioState::Playing {
                track: AmbientSoundType::Fire
            }
        );
        assert!(calls_contains(&h, "play"));

        driver.tick(200);
        // Should be at target volume after the 200 ms ramp.
        let last = h.calls.borrow().last().cloned().unwrap_or_default();
        assert!(last.starts_with("set_volume:0.4"), "got {last}");
    }

    /// Scenario 4: cross-fade on track change. Two elements alive
    /// during the 300 ms cross-fade; completion drops the outgoing.
    #[test]
    fn cross_fade_on_track_change() {
        let factory = Factory::new();
        let mut driver = AmbientAudio::new(factory.closure());
        driver.start(AmbientSoundType::Rain, 50);
        driver.tick(200); // settle

        driver.cross_fade(AmbientSoundType::Fire, 50);
        assert_eq!(
            driver.state(),
            &AmbientAudioState::CrossFading {
                outgoing: AmbientSoundType::Rain,
                incoming: AmbientSoundType::Fire,
            }
        );
        assert_eq!(factory.count(), 2);
        let fire_handle = factory.handle(1);
        assert!(calls_contains(
            &fire_handle,
            "set_src:/assets/audio/ambient/fire.wav"
        ));
        assert!(calls_contains(&fire_handle, "play"));

        // Drive the 300 ms cross-fade to completion.
        driver.tick(300);
        assert_eq!(
            driver.state(),
            &AmbientAudioState::Playing {
                track: AmbientSoundType::Fire
            }
        );
        // The outgoing rain handle should have been paused at completion.
        let rain_handle = factory.handle(0);
        assert!(calls_contains(&rain_handle, "pause"));
    }

    /// Scenario 5: fade-out on session end (`Playing` → `FadingOut`
    /// → `Idle`).
    #[test]
    fn fade_out_on_session_end() {
        let factory = Factory::new();
        let mut driver = AmbientAudio::new(factory.closure());
        driver.start(AmbientSoundType::Wind, 50);
        driver.tick(200);

        driver.fade_out();
        assert_eq!(
            driver.state(),
            &AmbientAudioState::FadingOut {
                track: AmbientSoundType::Wind
            }
        );
        driver.tick(200);
        assert_eq!(driver.state(), &AmbientAudioState::Idle);
    }

    /// Scenario 6: disable during cross-fade. Both elements fade
    /// out from their CURRENT volume over 200 ms (arc 6).
    #[test]
    fn disable_during_cross_fade() {
        let factory = Factory::new();
        let mut driver = AmbientAudio::new(factory.closure());
        driver.start(AmbientSoundType::Rain, 50);
        driver.tick(200); // settle to Playing at 0.5

        driver.cross_fade(AmbientSoundType::Fire, 50);
        driver.tick(150); // half-way through cross-fade

        // Disable mid-cross-fade.
        driver.fade_out();
        assert!(matches!(
            driver.state(),
            &AmbientAudioState::FadingOut { .. }
        ));
        // Drive the 200 ms fade-out.
        driver.tick(200);
        assert_eq!(driver.state(), &AmbientAudioState::Idle);
        // Both elements should have been paused.
        let rain_handle = factory.handle(0);
        let fire_handle = factory.handle(1);
        assert!(calls_contains(&rain_handle, "pause"));
        assert!(calls_contains(&fire_handle, "pause"));
    }

    /// Scenario 7: track change while in `FadingOut` updates
    /// settings only — no driver transition fires.
    #[test]
    fn track_change_during_fade_out_ignored() {
        let factory = Factory::new();
        let mut driver = AmbientAudio::new(factory.closure());
        driver.start(AmbientSoundType::Rain, 50);
        driver.tick(200);
        driver.fade_out();
        let factory_count_before = factory.count();

        // Track change while in FadingOut: cross_fade is a no-op.
        driver.cross_fade(AmbientSoundType::Fire, 50);
        assert!(matches!(
            driver.state(),
            &AmbientAudioState::FadingOut { .. }
        ));
        assert_eq!(
            factory.count(),
            factory_count_before,
            "no new element should have been materialised while FadingOut",
        );
    }

    /// Scenario 8: volume change while Paused — target stored, used
    /// on resume.
    #[test]
    fn volume_change_while_paused() {
        let factory = Factory::new();
        let mut driver = AmbientAudio::new(factory.closure());
        driver.start(AmbientSoundType::Library, 50);
        driver.tick(200);
        driver.pause();
        driver.tick(200);

        // Clear calls; now change volume while paused.
        let h = factory.handle(0);
        h.calls.borrow_mut().clear();
        driver.set_volume(80);
        // No set_volume call should fire on the element while paused.
        assert!(
            !h.calls
                .borrow()
                .iter()
                .any(|c| c.starts_with("set_volume:")),
            "set_volume while paused must not write to the element: {:?}",
            h.calls.borrow(),
        );

        // Resume with the new target.
        driver.resume(80);
        driver.tick(200);
        // The last set_volume on the element should target ~0.8.
        let last = h.calls.borrow().last().cloned().unwrap_or_default();
        assert!(
            last.starts_with("set_volume:0.8"),
            "expected fade-in to settle near 0.8; got {last}",
        );
    }

    /// Scenario 9: disable while `Paused` — driver transitions to
    /// `FadingOut` then `Idle`; both slots clean.
    #[test]
    fn disable_while_paused() {
        let factory = Factory::new();
        let mut driver = AmbientAudio::new(factory.closure());
        driver.start(AmbientSoundType::Storm, 50);
        driver.tick(200);
        driver.pause();
        driver.tick(200);

        driver.fade_out();
        assert!(matches!(
            driver.state(),
            &AmbientAudioState::FadingOut { .. }
        ));
        driver.tick(200);
        assert_eq!(driver.state(), &AmbientAudioState::Idle);
        // The single resident element should have been paused
        // (could be called twice — once on pause, once on fade-out
        // completion — both are valid).
        let h = factory.handle(0);
        assert!(calls_contains(&h, "pause"));
    }

    /// R-004 regression pin: disable-while-paused fires `fade_out`
    /// from `Paused` and lands cleanly in `Idle` with the resident
    /// slot vacated.
    ///
    /// Before this fix the timer-side gate Effect skipped the
    /// fade-out because its `gate_high` boolean was already false
    /// when pause flipped `active_focus` off, so disabling the feature
    /// while paused was a non-event and the driver was stranded in
    /// `Paused`. The driver-side state machine ALWAYS supported the
    /// `Paused` → `FadingOut` → `Idle` arc (Scenario 9 above); this
    /// test pins that contract so a future refactor at the driver
    /// layer can't silently regress the bug that the timer fix
    /// depends on.
    #[test]
    fn disable_while_paused_tears_down_cleanly() {
        let factory = Factory::new();
        let mut driver = AmbientAudio::new(factory.closure());
        driver.start(AmbientSoundType::Fan, 60);
        driver.tick(200);

        // Pause: settle fully so the driver is in `Paused` with no
        // ramp in flight.
        driver.pause();
        driver.tick(200);
        assert!(matches!(driver.state(), &AmbientAudioState::Paused { .. }));

        // The R-004 fix has the timer-side Effect call `fade_out`
        // when the user disables ambient while paused. Replay that
        // dispatch here against the driver directly.
        driver.fade_out();
        assert!(matches!(
            driver.state(),
            &AmbientAudioState::FadingOut { .. }
        ));

        driver.tick(200);
        assert_eq!(driver.state(), &AmbientAudioState::Idle);
        // After landing in Idle a fresh focus session must be able
        // to spawn a NEW element via `start()` — confirm the slot
        // was vacated. We assert this indirectly by starting a new
        // track and verifying the factory materialises a SECOND
        // handle.
        let count_before = factory.count();
        driver.start(AmbientSoundType::Wind, 60);
        assert_eq!(
            factory.count(),
            count_before + 1,
            "post-fade-out start must spawn a fresh element"
        );
    }

    /// V2 regression pin: volume slider drag mid-cross-fade retargets
    /// the incoming ramp endpoint. Outgoing ramp stays anchored at 0
    /// (the outgoing track is fading to silence regardless of slider).
    ///
    /// Before this fix the `CrossFading` arm fell into the generic
    /// "stored target only" branch shared with `Paused` / `Idle` /
    /// `FadingOut`, so a slider drag during the 300 ms cross-fade was
    /// silently dropped and the incoming track settled at whatever
    /// the previous target had been.
    #[test]
    fn volume_change_during_cross_fade() {
        let factory = Factory::new();
        let mut driver = AmbientAudio::new(factory.closure());
        driver.start(AmbientSoundType::Rain, 50);
        driver.tick(200); // settle at 0.5

        driver.cross_fade(AmbientSoundType::Fire, 50);
        assert!(matches!(
            driver.state(),
            &AmbientAudioState::CrossFading { .. }
        ));

        // Drag the slider to 75 in the middle of the cross-fade.
        driver.set_volume(75);

        // The incoming ramp endpoint must reflect the new target.
        let ramp = driver
            .ramp
            .as_ref()
            .expect("cross_fade installs a ramp; set_volume preserves it");
        assert!(
            (ramp.in_to - 0.75).abs() < 1e-9,
            "incoming ramp endpoint should retarget to 0.75; got {}",
            ramp.in_to,
        );
        // The outgoing ramp endpoint is anchored at 0 regardless of
        // the slider (fading to silence).
        assert_eq!(
            ramp.out_to,
            Some(0.0),
            "outgoing ramp endpoint must stay at 0.0",
        );

        // Drive the cross-fade to completion and confirm the
        // incoming element settles at the NEW target (0.75), not the
        // original 0.50.
        driver.tick(300);
        assert_eq!(
            driver.state(),
            &AmbientAudioState::Playing {
                track: AmbientSoundType::Fire
            }
        );
        let fire_handle = factory.handle(1);
        let last = fire_handle
            .calls
            .borrow()
            .iter()
            .rev()
            .find(|c| c.starts_with("set_volume:"))
            .cloned()
            .unwrap_or_default();
        assert!(
            last.starts_with("set_volume:0.75"),
            "incoming track must settle at retargeted volume; last set_volume was {last}",
        );
    }

    /// FR-008 regression pin: pausing mid-cross-fade must fade BOTH
    /// elements to 0 within 200 ms, land in `Paused(incoming)`, and
    /// release the outgoing handle.
    ///
    /// Before this fix `pause()` only matched `Playing`, so a pause
    /// invoked during the 300 ms cross-fade window was silently
    /// no-opped and both elements continued ramping to their
    /// cross-fade targets — audio played straight through pause.
    #[test]
    fn pause_during_cross_fade_fades_both_to_zero() {
        let factory = Factory::new();
        let mut driver = AmbientAudio::new(factory.closure());
        driver.start(AmbientSoundType::Rain, 50);
        driver.tick(200); // settle at 0.5

        driver.cross_fade(AmbientSoundType::Fire, 50);
        assert!(matches!(
            driver.state(),
            &AmbientAudioState::CrossFading { .. }
        ));

        // Advance partway through the 300 ms cross-fade.
        driver.tick(100);

        // Pause mid-cross-fade. State must land on the INCOMING track
        // (that's what resume will fade back in).
        driver.pause();
        assert_eq!(
            driver.state(),
            &AmbientAudioState::Paused {
                track: AmbientSoundType::Fire
            }
        );

        let rain_handle = factory.handle(0);
        let fire_handle = factory.handle(1);

        // Drive the 200 ms fade-out to completion. Both handles must
        // receive a final `set_volume:0.000` — neither one may be left
        // sounding after pause.
        driver.tick(200);
        let rain_last_vol = rain_handle
            .calls
            .borrow()
            .iter()
            .rev()
            .find(|c| c.starts_with("set_volume:"))
            .cloned()
            .unwrap_or_default();
        let fire_last_vol = fire_handle
            .calls
            .borrow()
            .iter()
            .rev()
            .find(|c| c.starts_with("set_volume:"))
            .cloned()
            .unwrap_or_default();
        assert_eq!(
            rain_last_vol, "set_volume:0.000",
            "outgoing rain element must end at 0 volume; got {rain_last_vol}",
        );
        assert_eq!(
            fire_last_vol, "set_volume:0.000",
            "incoming fire element must end at 0 volume; got {fire_last_vol}",
        );
        // The outgoing rain handle must have been released via pause()
        // when the ramp completed.
        assert!(calls_contains(&rain_handle, "pause"));

        // Resume from this Paused state must fade ONLY the incoming
        // (fire) element from 0 back to target_volume / 100.
        fire_handle.calls.borrow_mut().clear();
        driver.resume(50);
        assert_eq!(
            driver.state(),
            &AmbientAudioState::Playing {
                track: AmbientSoundType::Fire
            }
        );
        assert!(calls_contains(&fire_handle, "play"));

        driver.tick(200);
        let fire_settled = fire_handle
            .calls
            .borrow()
            .iter()
            .rev()
            .find(|c| c.starts_with("set_volume:"))
            .cloned()
            .unwrap_or_default();
        assert!(
            fire_settled.starts_with("set_volume:0.5"),
            "incoming track must ramp back to target after resume; got {fire_settled}",
        );
    }
}
