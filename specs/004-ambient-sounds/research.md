# Research Decisions — Feature 004 (Ambient Background Sounds)

Three external decisions are pinned here because they are irreversible-ish (asset format choice would bind the vendor tree byte-stable; playback mechanism choice would bind the driver's `web_sys` surface; asset licensing posture would bind the on-disk artefacts to a specific provenance trail). The fourth concern — exact CC0 source files for the seven tracks — is a tasks-phase concern, not a plan-phase concern, but the licensing constraint is asserted here so the tasks-phase work doesn't accidentally vendor a non-CC0 file.

## Decision 1 — Playback mechanism: HTML5 `<audio loop>` element

**Chosen**: HTML5 `<audio>` element via `web_sys::HtmlAudioElement`, with `.loop = true` and `.src = "/assets/audio/ambient/<track>.mp3"`. Two persistent `HtmlAudioElement` instances are pre-warmed on the user's first Start click (or when ambient sound is enabled). These elements stay alive across breaks, long-breaks, and auto-starts within the same app session — their lifetime acts as the "gesture lease" for WKWebView's autoplay heuristic. When ambient sound is OFF or `None`, each element exists with `.src = ""` (no decoding cost). When ON, `.src` is set to the picked track. Cross-fade uses these two persistent elements swapping roles. Pause / resume / disable transitions use a single element with a 200 ms linear `.volume` ramp.

**Rejected alternative**: Web Audio API with `AudioContext.decodeAudioData` decoding the MP3 into an `AudioBuffer`, then looping via `AudioBufferSourceNode` with `loop = true`, with a per-source `GainNode` for amplitude control.

**Reasons**:

1. **Long-loop streaming is what `<audio loop>` is designed for.** The browser fetches the MP3 progressively; the first ~100 KB is enough to start playback; the rest streams as the loop plays. With Web Audio API + decoded buffer, the entire MP3 must be downloaded AND decoded into an `AudioBuffer` before any sound emerges — for a ~2 MB / 90 s track at 192 kbps, that's ~2 MB of decode work that produces an ~8 MB `Float32Array` at 44.1 kHz stereo. We'd hold all seven tracks decoded simultaneously if we wanted gap-free track-switching (56 MB resident); we'd hold one decoded track at minimum (8 MB resident). The `<audio>` element holds nothing decoded — it streams.

2. **macOS WKWebView autoplay restriction and continuous-sessions auto-start.** GitHub issue #56 (and the broader macOS WKWebView autoplay behaviour documented across Web compatibility tables) flags that `AudioContext` instances created outside a user-gesture call stack are sometimes left in the `suspended` state on macOS, requiring an explicit `.resume()` after a user gesture. `<audio>.play()` called from within a Leptos `Effect` whose firing chain starts at the user's Start click is treated as user-gesture-rooted by WebKit's autoplay heuristic. However, FR-009 + Acceptance Scenario 10 require ambient sound to auto-resume on continuous-sessions auto-start. Auto-start has NO fresh user gesture — the original Start click was 25+ minutes prior. WKWebView's autoplay heuristic does NOT carry a 25-min-old gesture to a newly-created `HtmlAudioElement`. **Resolution (PM decision applied)**: pre-warm ONE persistent `HtmlAudioElement` on the user's first Start click (or when ambient sound is enabled). Keep this element alive across breaks / long-breaks / auto-starts within the same app session. The element's lifetime acts as the gesture lease. When ambient sound is OFF or `None`, the element exists with `.src = ""` (loaded but silent — no decoding cost). When ON, swap `.src` to the picked track. Cross-fade uses TWO such persistent elements that swap roles. **Alternative considered and rejected**: require a fresh user gesture per focus session — this would break continuous-sessions UX where the user expects ambient to resume automatically. The metronome at `src/src/components/timer/mod.rs:412-443` uses a cached `AudioContext` because the metronome's per-tick latency budget is tight (~10 ms — see feature 002 Bundle C); the ambient sound's latency budget is loose (200 ms fade-in is the user-visible budget), so the pre-warm approach is the right trade.

3. **No `AudioContext.decodeAudioData` round-trip; no large decoded `AudioBuffer` in memory.** Per (1) above — confirmed by spot-checks against the existing `play_chime` / `play_metronome_tick` paths which do NOT use `decodeAudioData` (they synthesise via `OscillatorNode`, not pre-recorded samples). Introducing `decodeAudioData` for the first time would add a non-trivial code path and a measurable memory cost.

4. **`.volume` property is straightforward; cross-fade is two simultaneous elements with overlapping `.volume` ramps.** The cross-fade implementation is a JS `setInterval` over 300 ms updating two `HtmlAudioElement.volume` slots linearly (one ramping from `slider/100.0` down to `0.0`, the other ramping from `0.0` up to `slider/100.0`). With Web Audio API, the equivalent would use `GainNode.gain.linearRampToValueAtTime(...)`, which is more precise (sample-accurate scheduling) but more code and more setup — disproportionate for ambient cross-fades where 300 ms is the perceptual budget and a few millisecond's drift in the ramp endpoints is inaudible.

**Cost of choosing `<audio>`**: loop-seam silence (tens of ms on `<audio loop>` restart on most platforms) is real but acceptable for ambient (rain / fire / wind / etc. naturally have silence breaks; the gap reads as natural texture). Edge Cases entry "Loop seam silence" accepts this explicitly. The metronome at `timer/mod.rs:412-443` uses a different mechanism (one-shot oscillator per tick, no looping) because the metronome's regular click pattern would expose loop seams perceptually.

**Memory overhead (persistent two-element pre-warm approach)**: with the continuous-sessions resolution above, two `HtmlAudioElement` decoder states are alive at all times when ambient sound is enabled. Each decoded MP3 stream consumes approximately 5–10 MB on WebKitGTK, 8–15 MB on WKWebView, and 5–10 MB on Edge WebView2. Peak transient memory while ambient sound is enabled: approximately 10–30 MB. Baseline when disabled: zero — elements with `.src = ""` skip decoding entirely. This is acceptable per the spec's "no battery cost while off" posture (Principle II): the idle elements incur no CPU decode work. The two-element overhead is the trade-off for maintaining the WKWebView gesture lease across auto-start boundaries.

**Cost of NOT choosing Web Audio API**: we lose sample-accurate ramp scheduling and the ability to chain `GainNode` / `AudioWorkletNode` / `DynamicsCompressorNode` for headroom management. This feature does not need any of those — the assets are mastered with headroom (SC-013) and the `.volume` property is enough.

## Decision 2 — Audio format: MP3 only

**Chosen**: MP3 only. One `.mp3` file per non-`None` track at `src/assets/audio/ambient/<track>.mp3`. No OGG, no AAC, no FLAC.

**Rejected alternatives**:

- **Both MP3 and OGG (HTML5 `<source>` selection with format fallback)**: doubles the vendored asset footprint (~14 MB → ~28 MB) for zero user-visible benefit on the three Tauri WebView backends.
- **OGG only**: WKWebView (macOS) support for OGG Vorbis is patchy on older OS versions. Some macOS releases route OGG through the system's audio codec stack, which historically has not shipped a Vorbis decoder by default. MP3 is part of every macOS / Windows / Linux audio stack the project's three Tauri WebViews (WKWebView / WebView2 / WebKitGTK) sit on top of.
- **AAC**: technically universal across the three WebViews, but the licensing posture is murkier than MP3 (which became patent-free in 2017). MP3 is the clean default.
- **FLAC**: gratuitous for ambient — lossless encoding of rain / fire / etc. is wasted bits; MP3 at 128–192 kbps is perceptually transparent for these source materials, and we're held to ≤2 MB / file (SC-008) which a 60–120 s FLAC would blow through.

**Reasons**:

1. **MP3 is universal across all three Tauri WebView backends** — WKWebView (macOS), WebView2 (Windows, Edge / Chromium), WebKitGTK (Linux). Per the HTML5 media compatibility table (verified against caniuse.com's `audio-mp3` entry — MP3 has been universally supported since the post-2017 patent expiry), no platform requires a fallback codec.
2. **OGG support is patchy on older WKWebView versions.** WebKit added Ogg Vorbis support but not on all macOS versions. The project supports macOS as a release target (per the auto-updater path in `VISION.md`); we can't assume the user's macOS version is recent enough.
3. **Vendor footprint stays at ≤14 MB** (7 files × 2 MB) per SC-008. Doubling the footprint for OGG would push the binary install size noticeably for zero compatibility benefit.

**Linux deployment caveat**: MP3 decoding on WebKitGTK depends on `gstreamer1-plugins-bad-free` + `gstreamer1-libav` being installed. Fedora and Debian historically excluded MP3 decoders before 2017 due to patent concerns; some minimal distros still ship without `gst-libav`. The Tauri build's runtime dependency on these plugins MUST be documented in the README or release notes for Linux targets. macOS (WKWebView) and Windows (WebView2) ship with MP3 codecs by default — no additional runtime dependency required on those platforms.

**Cost of choosing MP3-only**: in the unlikely event that a future platform deprecates MP3 in its WebView (no such deprecation is on any vendor's roadmap as of 2026), we'd add a fallback format in a follow-up spec. The wire shape (`AmbientSoundType` enum names) is independent of the file format, so the migration would be a vendor-tree swap plus a `<source>`-tag rewrite — not a wire-shape change.

## Decision 3 — Asset licensing: CC0 / equivalent royalty-free, no UI-attribution obligation

**Chosen constraint** (asserted at plan-phase, sourcing deferred to tasks-phase): every vendored MP3 MUST be CC0 (Creative Commons Zero / public domain) or equivalent royalty-free, with no attribution requirement that would force a runtime UI change.

**Rejected alternatives**:

- **CC-BY (attribution required)**: would force an in-app attribution surface (About dialog entries, or per-track tooltip strings, or a credits modal). The presto About-dialog surface does not currently exist; introducing one for CC-BY compliance is a UX change that belongs in its own spec. We avoid this by binding the constraint to CC0 upfront.
- **Commercial royalty-free libraries** (e.g., freesound.org's premium tier, AudioJungle): commercial licensing is incompatible with the open-source posture of the codebase. The project's existing `LICENSE` file is the canonical reference; commercial royalty-free clips would require either license-file changes or a separate `LICENSES-AUDIO.md` carve-out — disproportionate complexity for what's a small set of ambient tracks.
- **Procedurally-generated audio at runtime** (Web Audio API `OscillatorNode` + filters): physically possible (the metronome at `timer/mod.rs:412-443` is this pattern), but the production work to generate convincing rain / fire / library / etc. via filter chains is large, and the resulting audio is harder to ship "feels right" updates for (a re-mastered MP3 is a file swap; a procedurally-generated track is a code change). Out of scope per A3 (assets vendored, not generated).

**Reasons**:

1. **No attribution surface in v1.** The Settings → Notifications tab does not have room for per-track attribution; the About dialog does not exist. CC0 sidesteps the requirement entirely.
2. **Source candidates exist**: freesound.org filtered to CC0, BBC Sound Effects archive (CC-BY-NC for non-commercial use — not suitable for the project's open posture), and several public-domain field-recording archives. Sourcing is a tasks-phase research / sourcing pass, not a plan-phase decision — but the licensing constraint must be asserted here so the sourcing pass doesn't accidentally pull a CC-BY-SA file.
3. **No attribution-display obligation that would surface in the app's UI or About dialog** (per A6 verbatim).

**Cost of choosing CC0-only**: smaller candidate pool than CC-BY would offer. Mitigated by the fact that ambient field recordings (rain, fire, wind, library noise, fan hum, storm, white noise) are some of the most-recorded sound categories on every public-domain audio platform.

**Placeholder fallback**: per the plan's Phase 4 exit, placeholder silent MP3s (e.g., `ffmpeg -f lavfi -i anullsrc=r=44100:cl=stereo -t 90 -c:a libmp3lame rain.mp3`) are acceptable for the GREEN test commit if the real CC0 assets need separate sourcing. They MUST be swapped in before merge. The tasks-phase pass will source the real files; this plan does not block on that work landing pre-implementation.

## Non-decisions (intentionally deferred to tasks-phase)

- **Exact source URLs / filenames / re-mastered loop-points for the seven tracks.** Tasks-phase sourcing pass.
- **Exact bitrate / sample rate for the seven re-mastered MP3s.** Within the SC-008 envelope (≤2 MB / file, 60–120 s) any bitrate that keeps the file ≤2 MB is acceptable; 128–192 kbps stereo is typical for ambient.
- **Exact volume-default calibration** so a slider at 50 / 100 is "noticeable but not loud" across the seven tracks. This is a mastering-pass concern: each track is normalised to a consistent perceptual loudness (LUFS-I target around -23 to -18) so the slider position has a comparable meaning across tracks. Out of plan-phase scope per A9.
