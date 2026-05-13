// Ambient background sound driver — feature 004.
//
// UI-side side-effect manager. Owns a state machine that controls
// looping playback through one or two `HtmlAudioElement` handles
// abstracted behind the `AudioElementHandle` trait so the state
// machine itself is host-testable (`wasm-pack test --node` has no
// DOM, so the real `HtmlAudioElement` is unavailable there).
//
// Mirrors the host-testable projection pattern from
// `crate::components::icon::IconClass` (feature 003) — the trait is
// the seam between the pure state machine (testable with
// `MockAudioHandle`) and the browser implementation
// (`HtmlAudioWrapper`).
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
    match t {
        AmbientSoundType::None => None,
        AmbientSoundType::Rain => Some("/assets/audio/ambient/rain.mp3"),
        AmbientSoundType::Fire => Some("/assets/audio/ambient/fire.mp3"),
        AmbientSoundType::Library => Some("/assets/audio/ambient/library.mp3"),
        AmbientSoundType::Fan => Some("/assets/audio/ambient/fan.mp3"),
        AmbientSoundType::Storm => Some("/assets/audio/ambient/storm.mp3"),
        AmbientSoundType::WhiteNoise => Some("/assets/audio/ambient/white-noise.mp3"),
        AmbientSoundType::Wind => Some("/assets/audio/ambient/wind.mp3"),
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

/// Host-testable abstraction over the browser-side `HtmlAudioElement`.
///
/// The five methods cover the surface the state machine needs:
/// switching the looping source, ramping volume, starting and
/// stopping playback, and reading the element's current time for
/// loop-seam diagnostics. `wasm-pack test --node` injects
/// `MockAudioHandle`; the wasm target injects `HtmlAudioWrapper`.
pub trait AudioElementHandle {
    /// Set the element's `src` attribute. Empty string means "no
    /// source, do not decode" — used during the pre-warm pattern to
    /// keep the gesture lease without paying decode cost.
    fn set_src(&self, src: &str);
    /// Set the element's `.volume` slot — `0.0..=1.0`. Out-of-range
    /// values may throw at the browser layer (`IndexSizeError`);
    /// callers are expected to feed valid values. The driver only
    /// passes values in `0.0..=1.0`.
    fn set_volume(&self, vol: f64);
    /// Start playback.
    ///
    /// # Errors
    /// Returns `AudioPlayError` if the browser autoplay policy
    /// blocks the call. The driver swallows the error; the user
    /// can re-press Start to retry.
    fn play(&self) -> Result<(), AudioPlayError>;
    /// Pause playback. The element stays decoded; subsequent
    /// `.play()` resumes from the same position.
    fn pause(&self);
    /// Current playback position (seconds). Used by diagnostics; the
    /// driver itself does not branch on this value.
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
/// `current` is the element associated with the latest entered
/// `Playing` / `Paused` / `FadingOut` / `CrossFading.incoming` state.
/// `previous` is occupied only during a cross-fade — it holds the
/// outgoing element. Both are `None` in `Idle`.
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
        self.target_volume = volume;
        let Some(path) = asset_path(track) else {
            // Defensive: caller should not invoke start with None.
            // Drop silently — preserves engine purity (no toast, no
            // panic).
            return;
        };
        if !matches!(self.state, AmbientAudioState::Idle) {
            return;
        }
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

    /// Arc 2: Playing(t) → Paused(t). 200 ms fade-out then pause on
    /// the resident element. Cancels any in-flight ramp (the new
    /// fade-out starts from whatever volume the element currently
    /// has, per the pre-emption rule).
    pub fn pause(&mut self) {
        let AmbientAudioState::Playing { track } = self.state.clone() else {
            return;
        };
        let current_volume = self.in_flight_target_or(f64::from(self.target_volume) / 100.0);
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
    /// only (no element change).
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
            AmbientAudioState::Paused { .. }
            | AmbientAudioState::Idle
            | AmbientAudioState::FadingOut { .. }
            | AmbientAudioState::CrossFading { .. } => {
                // Per pre-emption rules: volume change while
                // not-actively-playing updates the stored target only.
                // On next resume / cross-fade-complete / Idle→Playing
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
                // element now that volume has reached 0.
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
// Browser-side implementation
// ---------------------------------------------------------------------

/// Real `HtmlAudioElement` wrapper used in the wasm target.
#[cfg(target_arch = "wasm32")]
pub struct HtmlAudioWrapper(pub web_sys::HtmlAudioElement);

#[cfg(target_arch = "wasm32")]
impl AudioElementHandle for HtmlAudioWrapper {
    fn set_src(&self, src: &str) {
        self.0.set_src(src);
    }
    fn set_volume(&self, vol: f64) {
        self.0.set_volume(vol);
    }
    fn play(&self) -> Result<(), AudioPlayError> {
        self.0.play().map(|_| ()).map_err(|_| AudioPlayError)
    }
    fn pause(&self) {
        let _ = self.0.pause();
    }
    fn current_time(&self) -> f64 {
        self.0.current_time()
    }
}

#[cfg(target_arch = "wasm32")]
impl HtmlAudioWrapper {
    /// Construct a fresh `HtmlAudioElement`, set `.loop = true`, and
    /// wrap it. Returns `None` if the element cannot be constructed
    /// (e.g. headless host without a DOM).
    #[must_use]
    pub fn new_looping() -> Option<Self> {
        let el = web_sys::HtmlAudioElement::new().ok()?;
        el.set_loop(true);
        Some(Self(el))
    }
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    /// Process-wide singleton driver. Materialised on the first
    /// `with_driver` call inside the timer component's gate effect.
    /// `RefCell` is fine because Leptos's CSR runtime is single-
    /// threaded (one WebAssembly main thread per app instance).
    static DRIVER: std::cell::RefCell<Option<AmbientAudio<HtmlAudioWrapper>>> =
        const { std::cell::RefCell::new(None) };
}

/// Run a closure against the process-wide driver, materialising it
/// on first call. Wasm-only; the wasm target has a DOM so the
/// factory always succeeds.
#[cfg(target_arch = "wasm32")]
pub fn with_driver<R>(f: impl FnOnce(&mut AmbientAudio<HtmlAudioWrapper>) -> R) -> Option<R> {
    DRIVER.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            let factory: Rc<dyn Fn() -> Rc<HtmlAudioWrapper>> = Rc::new(|| {
                Rc::new(HtmlAudioWrapper::new_looping().expect("HtmlAudioElement::new"))
            });
            *slot = Some(AmbientAudio::new(factory));
        }
        slot.as_mut().map(f)
    })
}

/// Host-target stub so non-wasm test runs don't trip on the
/// `with_driver` import. Returns `None` because there's no driver
/// to drive without a DOM.
#[cfg(not(target_arch = "wasm32"))]
pub fn with_driver<R>(_f: impl FnOnce(&mut AmbientAudio<HtmlAudioWrapper>) -> R) -> Option<R> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub struct HtmlAudioWrapper;

#[cfg(not(target_arch = "wasm32"))]
impl AudioElementHandle for HtmlAudioWrapper {
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
    use super::{AmbientAudio, AmbientAudioState, AmbientSoundType, AudioElementHandle};
    use std::cell::RefCell;
    use std::rc::Rc;

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
            "set_src:/assets/audio/ambient/rain.mp3"
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
            "set_src:/assets/audio/ambient/fire.mp3"
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
}
