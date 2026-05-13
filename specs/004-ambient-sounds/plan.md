# Implementation Plan: Opt-In Ambient Background Sounds During Focus

**Branch**: `004-ambient-sounds` | **Date**: 2026-05-13 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification at `specs/004-ambient-sounds/spec.md`

## Table of Contents

1. [Summary](#summary)
2. [Technical Context](#technical-context)
3. [Constitution Check](#constitution-check)
4. [Project Structure](#project-structure)
5. [Modules](#modules)
6. [Testing strategy and test-first markers](#testing-strategy-and-test-first-markers)
7. [CI gates](#ci-gates)
8. [Implementation phasing](#implementation-phasing)
9. [Post-design Constitution Check](#post-design-constitution-check)
10. [Complexity Tracking](#complexity-tracking)

## Summary

A single user-facing capability: opt-in ambient background sounds (rain / fire / library / fan / storm / white-noise / wind) that loop during focus sessions and stay silent everywhere else. Settings → Notifications gains three controls — checkbox, track dropdown, volume slider (0–100, default 50) — placed below the existing metronome row. `NotificationSettings` (in `crates/presto-ipc/src/settings.rs`) evolves with three `#[serde(default)]` fields (`ambient_sound_enabled: bool`, `ambient_sound_type: AmbientSoundType`, `ambient_sound_volume: u32`) and one new kebab-case enum `AmbientSoundType` (eight variants including `None`), mirroring the feature 002 metronome serde-evolution pattern at `crates/presto-ipc/src/settings.rs:185-186` and the `StatusBarDisplay` kebab-case wire pattern at `crates/presto-ipc/src/settings.rs:25-35`. Playback lives entirely UI-side in a new module `src/src/components/ambient_audio.rs`, driven from the same timer-component tick-loop hook point that already runs the metronome gate at `src/src/components/timer/mod.rs:1358-1368` — engine remains DOM-free. Audio mechanism is the HTML5 `<audio loop>` element via `web_sys::HtmlAudioElement` (already reachable through the existing `web-sys` dependency by adding two feature-list entries; not a new crate). Cross-fade is two simultaneous `<audio>` elements with overlapping `.volume` ramps; pause/resume/disable transitions use 200 ms fades, track changes use 300 ms cross-fades. Vendored MP3 assets live under `src/assets/audio/ambient/` (one file per non-`None` track) and ship via a new Trunk `copy-dir` line in `src/index.html` mirroring the existing icons / phosphor / style block at `src/index.html:26-47`. No new Tauri commands, no new runtime dependencies, no on-disk migration — wire-shape evolution per Principle VI via `#[serde(default)]`. Detail in [research.md](./research.md), [data-model.md](./data-model.md), [contracts/components.md](./contracts/components.md), [quickstart.md](./quickstart.md).

## Technical Context

**Language/Version**: Rust 1.83+; `wasm32-unknown-unknown` target for the Leptos crate; backend Rust unchanged. No version bump from feature 003's baseline.
**Primary Dependencies**: Unchanged at the crate level. `leptos = "0.7"`, `serde`, `serde-wasm-bindgen`, `chrono`, `web-sys` (already imported in `components/timer/mod.rs` for `play_chime` + `play_metronome_tick`; this feature reuses the same crate and only widens its feature list to add `HtmlAudioElement` + the two `HtmlMediaElement` parent traits, both already reachable through the existing `web-sys` dependency). Backend deps unchanged. **No new Cargo or npm runtime dependency.**
**Storage**: Tauri app-data directory; unchanged. `settings.json` evolves at the field level only inside the `notifications` block (three new fields, each `#[serde(default)]`). Legacy records without the new fields deserialise to `false` / `None` / `50`. No new on-disk files; ambient audio assets are not user data — they are vendored read-only resources copied into the Trunk dist tree.
**Testing**: `cargo test --workspace --frozen` for the three new IPC round-trip tests (`ambient_sound_legacy_fields_default`, `ambient_sound_round_trip`, `ambient_sound_type_serialises_kebab_case`) in `crates/presto-ipc/src/settings.rs::tests`; `wasm-bindgen-test` for the MANDATORY `AmbientAudio` state-transition unit (MANDATORY non-RED-first per Phase 3 below; covers the `Idle ↔ Playing ↔ Paused ↔ CrossFading ↔ FadingOut` transitions independently of real audio playback via a `<audio>`-element stub); Playwright e2e for Settings UI plumbing (new selectors `#ambient-sound-enabled`, `#ambient-sound-type`, `#ambient-sound-volume`); visual regression for the one affected baseline (`settings-notifications-chromium-linux.png`).
**Target Platform**: macOS, Linux, Windows desktops (CSR-only single-window Tauri 2.x). The Tauri WebView varies per platform — WKWebView on macOS, WebView2 (Chromium / Edge) on Windows, WebKitGTK on Linux. MP3 is decoded by all three WebView audio stacks without extra codec installation; OGG support is patchy on older WKWebView versions. Plan-level format choice resolved to **MP3 only** per [research.md](./research.md).
**Performance Goals**: No regression. Each ambient track ≤2 MB on disk (per SC-008); HTML5 `<audio>` streams progressively so the cold-start cost of enabling ambient sound is small (the first ~100 KB streams in before playback begins; the rest streams as the loop plays). Total vendored asset footprint ≤14 MB (7 files × 2 MB) gates SC-008. **Architecture decision (continuous-sessions autoplay)**: two persistent `HtmlAudioElement` instances are pre-warmed on the user's first Start click (or when ambient sound is enabled) and kept alive across breaks / long-breaks / auto-starts within the same app session. Their lifetime acts as the WKWebView gesture lease required for continuous-sessions auto-resume (FR-009 + Acceptance Scenario 10). When ambient sound is OFF or `None`, each element exists with `.src = ""` (no decoding cost). Peak transient memory: ~10–30 MB while ambient sound is enabled; zero when disabled. Alternative rejected: require a fresh user gesture per focus session (breaks continuous-sessions UX). See [research.md Decision 1](./research.md#decision-1--playback-mechanism-html5-audio-loop-element) for full rationale. No long-lived Web Audio API graphs; no `AudioContext.decodeAudioData` round-trip; no full decoded `AudioBuffer` in memory.
**Constraints**: Strict static analysis stays green (Principles III + X). The existing `#[allow(clippy::struct_excessive_bools)]` on `NotificationSettings` at `crates/presto-ipc/src/settings.rs:167` continues to cover the bool count after `ambient_sound_enabled` is added (the existing inline justification — "every bool maps to an independent UI toggle" — applies). The engine-purity grep gate (`scripts/check-engine-purity.sh`) must stay at zero `web_sys` references under `src/src/engine/`; this feature only touches UI-side code (`src/src/components/`) and IPC types (`crates/presto-ipc/`), so the gate stays green by construction. The baseline-cap gate stays at the post-003 default — one baseline regenerates (`settings-notifications-chromium-linux.png`).
**Scale/Scope**: Three new wire-shape fields, one new kebab-case enum, one new UI-side module, seven vendored audio files, one Trunk `copy-dir` line, one Settings UI tab evolution, one e2e spec evolution, one baseline regeneration. ~6 files touched + 1 new module + 7 vendored assets; no new Tauri commands; no new IPC commands; no new runtime deps.

## Constitution Check

*GATE: must pass before Phase 0. Re-checked after Phase 1.*

Only principles with material content are listed below per repo artefact discipline.

### I. The Timer Is Sacred — UI-side side effect

The ambient-audio driver lives in `src/src/components/ambient_audio.rs` (new module under `components/`, not `engine/`) and is wired into the same timer-component tick-loop hook point that already gates the metronome at `src/src/components/timer/mod.rs:1358-1368`. The engine emits no new event, gains no new state field, learns nothing about ambient audio. The gate predicate is built entirely from existing `TimerState` reads (`current_mode()`, `is_running()`, `is_paused()`, `is_auto_paused()`, `time_remaining_secs()`) and from `settings.notifications.ambient_sound_*` reads — same shape as the metronome gate. The engine-purity grep gate enforces zero `web_sys` imports under `src/src/engine/`. **PASS.**

### II. Local-Only — vendored assets, no CDN

Seven MP3 files are committed under `src/assets/audio/ambient/` and copied into the Trunk dist tree via a new `copy-dir` directive in `src/index.html` mirroring the existing icons/phosphor/style block at lines 26–47. The `<audio>` element's `src` attribute resolves same-origin (`/assets/audio/ambient/<track>.mp3`). No CDN, no fetch, no network egress. The `_blockExternal` e2e fixture remains effective. Auto-updater traffic is unchanged. **PASS.**

### III. Type Safety Over Defensive Code — closed enum, UI-boundary clamp

- `AmbientSoundType` is a closed eight-variant sum type (`None`, `Rain`, `Fire`, `Library`, `Fan`, `Storm`, `WhiteNoise`, `Wind`) with `#[serde(rename_all = "kebab-case")]`, mirroring `StatusBarDisplay` at `crates/presto-ipc/src/settings.rs:25-35`. The `None` variant is a first-class "no track selected" sentinel — not `Option<AmbientSoundType>`, not the empty string, not a magic value. Downstream consumers branch exhaustively (Rust's match-exhaustiveness check fails compilation if a variant is missed).
- `ambient_sound_volume` is `u32` clamped at the Settings UI input boundary (`<input type="range" min="0" max="100">`). The audio call site reads the stored value and passes it through to `HtmlAudioElement::set_volume(volume_u32 as f64 / 100.0)` without re-clamping; a hand-edited out-of-range value re-clamps on the next Settings open/save. Principle III's "validate at system boundaries only" applies — serde rejects negatives at deserialise time (the field is `u32`); the Settings UI `<input type="range">` rejects out-of-range values at the input layer; the audio site sees an unguarded value and uses it raw.
- The existing `#[allow(clippy::struct_excessive_bools)]` on `NotificationSettings` at `crates/presto-ipc/src/settings.rs:167` covers the bool count after `ambient_sound_enabled` is added — the inline justification ("every bool maps to an independent UI toggle") continues to apply.

**PASS.**

### IV. Visual Regression Is The UI Contract — one baseline

One baseline regenerates: `settings-notifications-chromium-linux.png` (three new affordances added below the metronome row — checkbox, track dropdown, volume slider). Per FR-021 / SC-012, no baseline outside Settings → Notifications regenerates. The timer screen does NOT change visually — playback is silent-by-default and the visible chrome lives entirely in Settings (per A10). The baseline-cap gate stays at its post-003 default with this one baseline carrying a one-line PR-description note.

**Per-baseline justification (pre-anchored here; restated verbatim in the PR description)**:
- `settings-notifications-chromium-linux.png`: ambient-sound checkbox, track dropdown, and volume slider added below the metronome row. No other layout change.

The feature 003 sidebar-mask posture (`mask: [page.locator(".sidebar")]` on non-sidebar baselines) remains in effect — no sidebar change in this feature, so the mask is irrelevant to whether other baselines diff.

**PASS** with one documented baseline regeneration (Principle IV's documented "intended change + one-line note" mechanism, not a widening).

### V. Test-First For Stateful Engines — IPC round-trip scope only

The engine has no new state; the manager state machines are untouched; persistence helpers are untouched. The IPC `NotificationSettings` wire-shape evolution IS in Principle V scope (the round-trip is the persistence boundary). Three RED-first tests precede implementation:

- `crates/presto-ipc/src/settings.rs::tests::ambient_sound_legacy_fields_default` — asserts a pre-feature-004 `NotificationSettings` JSON (no ambient fields, only the feature 002 baseline shape) deserialises to `ambient_sound_enabled = false`, `ambient_sound_type = AmbientSoundType::None`, `ambient_sound_volume = 50`. Mirrors `metronome_default_off` at `crates/presto-ipc/src/settings.rs:362-375` verbatim.
- `crates/presto-ipc/src/settings.rs::tests::ambient_sound_round_trip` — asserts a non-default new-build `NotificationSettings` (e.g., `true` / `Rain` / `30`) round-trips byte-stable through serde, AND the feature 002 `metronome` field is preserved across the same round-trip (Acceptance Scenario 2.6).
- `crates/presto-ipc/src/settings.rs::tests::ambient_sound_type_serialises_kebab_case` — asserts each of the eight `AmbientSoundType` variants serialises to its kebab-case wire string (`"none"`, `"rain"`, `"fire"`, `"library"`, `"fan"`, `"storm"`, `"white-noise"`, `"wind"`) and round-trips byte-stable in both directions.

UI plumbing (checkbox / dropdown / slider rendering) and audio playback wiring are e2e-covered and outside Principle V scope per the documented "UI rendering, view wiring, trivial CRUD plumbing" carve-out.

One MANDATORY non-RED-first wasm-bindgen-test covers the `AmbientAudio` driver's state transitions independently of real audio playback — it lands alongside the implementation (Phase 3), not before it, because the unit is presentational state-machine logic over a Leptos signal graph, not engine math. It is a coverage gate, not a RED-first pair, per the same posture feature 003 took for the tooltip-text-matrix test (FR-031). The wasm-bindgen-test instantiates the driver with a stub `<audio>` element, drives the state-transition surface through `Idle → Playing → Paused → Playing → CrossFading → Playing → FadingOut → Idle`, and asserts the resulting `volume` / `paused` / `currentTime` reads at each step.

**PASS.**

### VI. The Tauri Boundary Is Stable — no new commands, no new IPC

No new Tauri commands. The three new `NotificationSettings` fields flow through the existing `save_settings` / `load_settings` round trip. Wire-shape evolution is per the existing `#[serde(default)]` pattern (mirrors the metronome field at `crates/presto-ipc/src/settings.rs:185-186` exactly). The mock-drift gate (`scripts/check-mock-drift.sh`, if present from the feature 002 / 003 precedent) sees no new commands and stays green without mock changes — verified against `tests/e2e/fixtures/tauriMock.js`.

**PASS.**

### IX. Lock Files Are First-Class — no new deps

No new runtime dependencies. `web_sys::HtmlAudioElement` is reachable through the existing `web-sys` crate by widening the feature list in `src/Cargo.toml` (`HtmlAudioElement`, `HtmlMediaElement` — both `web-sys` features, not new crate additions). The Cargo dependency line is unchanged at the version / source level; only the in-place `features = [...]` list grows by ~2 entries. The lockfile-drift gate stays green by inaction. **PASS.**

### Verdict

No principle is **VIOLATION**. The one IV baseline regeneration is a routine intended change with a per-baseline note, not a widening. Proceed to Phase 0.

## Project Structure

### Documentation (this feature)

```text
specs/004-ambient-sounds/
├── plan.md                  # This file
├── research.md              # Phase 0 — two resolved external decisions: (1) HTML5 <audio loop> vs Web Audio API decoded buffer; (2) MP3 vs OGG / both formats; (3) CC0 asset sourcing posture (deferred to tasks but constrained here)
├── data-model.md            # Phase 1 — AmbientSoundType enum, NotificationSettings evolution, UI-side AmbientAudioState (current track, fade state)
├── contracts/
│   └── components.md        # Phase 1 — three contracts: AmbientSoundType wire shape; NotificationSettings field evolution; AmbientAudio side-effect lifecycle (state-transition diagram + fade-duration table)
├── checklists/              # Authored at /speckit-specify (already present)
├── quickstart.md            # Phase 1 — contributor's path: local build, where to drop the vendored MP3 files, how to run the new tests, how to regenerate the affected baseline
└── tasks.md                 # Phase 2 — generated by /speckit-tasks (NOT this command)
```

### Source Code (new and touched paths)

```text
crates/presto-ipc/src/
└── settings.rs                                    # +AmbientSoundType enum (kebab-case); +3 fields on NotificationSettings
                                                   # (ambient_sound_enabled / ambient_sound_type / ambient_sound_volume, each
                                                   # #[serde(default)]); +3 tests (ambient_sound_legacy_fields_default,
                                                   # ambient_sound_round_trip, ambient_sound_type_serialises_kebab_case)

src/Cargo.toml                                     # web-sys feature-list widening: +"HtmlAudioElement", +"HtmlMediaElement"
                                                   # (no new crate; no version bump; existing `web-sys = "0.3"` line unchanged)

src/src/components/
├── ambient_audio.rs                               # NEW. The side-effect driver. Owns the current/previous <audio> element
│                                                   # handles in component-local RwSignal<Option<HtmlAudioElement>> slots,
│                                                   # the AmbientAudioState enum (Idle / Playing / Paused / CrossFading /
│                                                   # FadingOut), and the start / stop / pause / resume / cross-fade /
│                                                   # volume-change / fade-out helpers. No engine state, no Tauri reads.
├── mod.rs                                         # +pub mod ambient_audio;
├── timer/mod.rs                                   # Wire the AmbientAudio side-effect from the same tick-loop / state-
│                                                   # transition handler that gates the metronome at :1358-1368. A
│                                                   # leptos::Effect::new watches the composite gate signal
│                                                   # (notifications.ambient_sound_enabled && ambient_sound_type != None
│                                                   # && Focus mode && running && !paused && !auto_paused && time_remaining_secs > 0)
│                                                   # and drives the AmbientAudio driver's start / stop / cross-fade entry
│                                                   # points. The metronome gate at :1358-1368 stays untouched.
└── settings/notifications.rs                      # +checkbox #ambient-sound-enabled below the metronome row; +dropdown
                                                   # #ambient-sound-type listing the eight AmbientSoundType variants in
                                                   # user-readable labels; +range slider #ambient-sound-volume (min=0
                                                   # max=100 default=50). All three controls visible regardless of
                                                   # checkbox state (FR-014). Selectors additive (FR-015).

src/style/                                         # The existing settings-notifications stylesheet (whichever file
                                                   # covers it — settings.css or similar; Phase 4 task generation
                                                   # picks the exact file) gains a small block for the new range
                                                   # slider if the project's slider style is not already universal.

src/index.html                                     # +<link data-trunk rel="copy-dir" href="assets/audio"
                                                   # data-target-path="assets/audio" /> — mirrors the existing
                                                   # icons / phosphor / style copy-dir block at lines 26-47.

src/assets/audio/ambient/                          # NEW vendor tree. 7 MP3 files: rain.mp3, fire.mp3, library.mp3,
                                                   # fan.mp3, storm.mp3, white-noise.mp3, wind.mp3. CC0 / royalty-
                                                   # free (A6). ≤2 MB / file, 60-120 s / file (SC-008). Sourcing is
                                                   # a tasks-phase concern; placeholder silent MP3s are acceptable
                                                   # for the GREEN test commit if real CC0 assets need separate
                                                   # sourcing (and are swapped in before merge).

tests/e2e/
├── settings-notifications.spec.js                 # +e2e flow: toggle #ambient-sound-enabled, pick a track from
│                                                   # #ambient-sound-type, drag #ambient-sound-volume, assert the
│                                                   # settings round-trip writes the new fields through.
└── __screenshots__/visual-regression/
    └── settings-notifications-chromium-linux.png  # REGENERATED. Three new affordances below the metronome row.
```

**Structure Decision**: One new UI-side module (`src/src/components/ambient_audio.rs`) is the right place for the side-effect manager because it cleanly separates the audio lifecycle from the timer component's already-dense rendering body. The timer component still owns the gate-signal-watching `Effect`, but it delegates the actual playback ops to the new module — same pattern feature 002's metronome could have used had its surface been larger (the metronome is small enough to live inline at `timer/mod.rs:412-443`; this feature's surface — start / stop / pause / resume / cross-fade / volume-change / fade-out, five state-transition arms, two simultaneous element handles during cross-fade — justifies the extraction). The IPC field evolution lives entirely in the existing `crates/presto-ipc/src/settings.rs` file; no new crate file is needed.

## Modules

Terse change table.

| Path | Change |
|---|---|
| `crates/presto-ipc/src/settings.rs` | `+ pub enum AmbientSoundType { None, Rain, Fire, Library, Fan, Storm, WhiteNoise, Wind }` with `#[serde(rename_all = "kebab-case")]` (mirrors `StatusBarDisplay` at :25-35). `+ ambient_sound_enabled: bool` (`#[serde(default)]`), `+ ambient_sound_type: AmbientSoundType` (`#[serde(default)]`; the enum's `Default` impl returns `None`), `+ ambient_sound_volume: u32` (`#[serde(default = "default_ambient_sound_volume")]`, default `50`) on `NotificationSettings`. `+ pub const fn default_ambient_sound_volume() -> u32 { 50 }`. The `#[allow(clippy::struct_excessive_bools)]` at :167 continues to cover the bool count. |
| `crates/presto-ipc/src/settings.rs::tests` | `+ ambient_sound_legacy_fields_default` (legacy JSON without ambient fields → defaults; mirrors `metronome_default_off` at :362-375); `+ ambient_sound_round_trip` (non-default values round-trip byte-stable; feature 002 `metronome` field preserved across the same round-trip); `+ ambient_sound_type_serialises_kebab_case` (eight-variant kebab-case wire-shape assertion). |
| `src/Cargo.toml` | `web-sys` `features = [...]` list gains `"HtmlAudioElement"` and `"HtmlMediaElement"`. No version bump; no new dependency line. The existing `[dependencies.web-sys]` block at :32-74 already contains `"AudioContext"` / `"OscillatorType"` etc. for `play_chime` and `play_metronome_tick`; the two additions are alongside those entries. |
| `src/src/components/ambient_audio.rs` | NEW. Module exposes `pub fn AmbientAudio(...) -> impl IntoView` (a unit component returning `()` since there is no DOM output — the module's actual surface is its state machine and the side-effect closures the timer component invokes). State machine documented in [contracts/components.md](./contracts/components.md). Uses `web_sys::HtmlAudioElement::new_with_src(...)`, `.set_loop(true)`, `.set_volume(...)`, `.play()`, `.pause()`. Fade ramps implemented as a JS-side `setInterval` over 200/300 ms updating `.volume` linearly to/from the configured slider value (no Web Audio `GainNode` — simple element-level volume is sufficient because the absolute volume range is small and the perceptual smoothness is dominated by the fade duration, not the curve shape). RAII-cleanup: dropping the component drops the `HtmlAudioElement` handles, which releases their decoder resources. |
| `src/src/components/mod.rs` | `+ pub mod ambient_audio;`. |
| `src/src/components/timer/mod.rs` | `+` a `leptos::Effect::new` (mounted in the timer-component init body, alongside the existing tick-loop effect at :1340-1380) that watches the composite gate signal `(notifications.ambient_sound_enabled && ambient_sound_type != None && current_mode == Focus && is_running && !is_paused && !is_auto_paused && time_remaining_secs > 0)`. On rising edge → invoke `ambient_audio::start(track, volume)`. On falling edge → invoke `ambient_audio::fade_out()`. On track change while gate is high → invoke `ambient_audio::cross_fade(new_track, volume)`. On volume change while gate is high → invoke `ambient_audio::set_volume(new_volume)`. The metronome gate at :1358-1368 is untouched. |
| `src/src/components/settings/notifications.rs` | Three new controls placed below the existing metronome row at :42-45 (the metronome derived signal block). (a) `<input id="ambient-sound-enabled" type="checkbox">` writing through `settings.notifications.ambient_sound_enabled`; (b) `<select id="ambient-sound-type">` with eight `<option>` entries mapping 1:1 to the `AmbientSoundType` variants in user-readable labels ("None", "Rain", "Fire", "Library", "Fan", "Storm", "White noise", "Wind"); (c) `<input id="ambient-sound-volume" type="range" min="0" max="100" step="1">` writing through `settings.notifications.ambient_sound_volume`. All three controls visible regardless of checkbox state (FR-014). Each writes through the existing `save_settings` hop on change. |
| `src/style/settings.css` (or wherever the settings row styles live; Phase 4 task generation picks the file) | Optional small block to style the new range slider if the project's existing slider style does not already cover it. The block must not regenerate any baseline outside `settings-notifications-chromium-linux.png` (Principle IV / FR-021). |
| `src/index.html` | `+ <link data-trunk rel="copy-dir" href="assets/audio" data-target-path="assets/audio" />` between the existing copy-dir lines at :26 (icons) and :47 (style), mirroring their pattern verbatim. |
| `src/assets/audio/ambient/{rain,fire,library,fan,storm,white-noise,wind}.mp3` | NEW. Seven vendored MP3 files. CC0 / royalty-free (A6). ≤2 MB / file, 60–120 s / file recommended (SC-008). Sourcing is a tasks-phase concern; placeholder silent MP3s are acceptable for the test-first GREEN commit if real CC0 assets need separate sourcing — they MUST be swapped in before merge. |
| `tests/e2e/settings-notifications.spec.js` | `+` e2e flow that toggles `#ambient-sound-enabled`, picks `Rain` from `#ambient-sound-type`, drags `#ambient-sound-volume` to 30, and asserts the values persist across a settings re-open. The audio playback itself is NOT exercised in e2e (no audio assertion in headless chromium); the wasm-bindgen-test in Phase 3 covers the state-machine transitions. |
| `tests/e2e/__screenshots__/visual-regression/settings-notifications-chromium-linux.png` | REGENERATED with one-line PR note: "settings-notifications: ambient-sound checkbox, track dropdown, and volume slider added below the metronome row". |

## Testing strategy and test-first markers

Per Principle V scope (IPC wire-shape evolution is the persistence boundary), three failing tests precede implementation:

| Module | Test runner | Test-first? | Notes |
|---|---|---|---|
| `presto_ipc::settings::tests::ambient_sound_legacy_fields_default` | `cargo test` (workspace) | **YES (RED-first)** `[test-first]` | Mirrors `metronome_default_off` at `crates/presto-ipc/src/settings.rs:362-375` verbatim. Legacy `NotificationSettings` JSON lacking all three new keys deserialises to `ambient_sound_enabled = false`, `ambient_sound_type = AmbientSoundType::None`, `ambient_sound_volume = 50`. SC-002. |
| `presto_ipc::settings::tests::ambient_sound_round_trip` | `cargo test` | **YES (RED-first)** `[test-first]` | Non-default new-build values (e.g., `true` / `Rain` / `30`) round-trip byte-stable. The same fixture also carries a `metronome: true` value from feature 002 and asserts the metronome field survives the round-trip — covers Acceptance Scenario 2.6. |
| `presto_ipc::settings::tests::ambient_sound_type_serialises_kebab_case` | `cargo test` | **YES (RED-first)** `[test-first]` | Eight-variant wire-shape assertion: `None` ↔ `"none"`, `Rain` ↔ `"rain"`, `Fire` ↔ `"fire"`, `Library` ↔ `"library"`, `Fan` ↔ `"fan"`, `Storm` ↔ `"storm"`, `WhiteNoise` ↔ `"white-noise"`, `Wind` ↔ `"wind"`. SC-003. |
| `ambient_audio::tests::state_transitions` (or equivalent) | `wasm-bindgen-test` | MANDATORY non-RED-first | Driver state-machine coverage: instantiates the driver with a stub `<audio>` element handle and drives the full pre-emption matrix (8 scenarios) documented in [contracts/components.md §state_transitions test matrix](./contracts/components.md#state_transitions-wasm-bindgen-test--full-pre-emption-matrix-non-red-first-mandatory). Minimum scenarios: (1) happy path `Idle → Playing → Paused → Playing → CrossFading → Playing → FadingOut → Idle`; (2) `Playing → CrossFading → FadingOut → Idle` (disable during cross-fade); (3) `Playing → FadingOut + track-change-while-fading → Idle` (track change ignored during fade-out); (4) `Playing → CrossFading + gate-flip → FadingOut` (cross-fade completion race); (5) `Paused → volume-change → Paused → resume-with-new-target`; (6) `Idle → volume-change → Idle → Playing-with-new-target`; (7) rapid-fire track changes (a→b→c within 100 ms — each cancels in-flight cross-fade); (8) `None → real → None` full cycle. Lands alongside the implementation (Phase 3), not before it. Per the same UI-rendering carve-out feature 003 used for FR-031 (tooltip text matrix). Coverage gate, not RED-first. |
| Settings UI (`#ambient-sound-enabled` / `#ambient-sound-type` / `#ambient-sound-volume` round-trip) | Playwright e2e | NO | UI plumbing — e2e + visual regression covers it. SC-001. |
| Visual-regression baseline regeneration | Playwright `toHaveScreenshot` | NO | One baseline (`settings-notifications-chromium-linux.png`); PR-time visual review against the per-baseline justification in §IV. SC-012. |

**Mock-first ordering rule** (per Principle VI): **N/A this feature.** No new Tauri commands; the mock-drift gate stays green without modifications — verified against `tests/e2e/fixtures/tauriMock.js`.

## CI gates

Reference `.agentex.yml` (post-003 stage definitions). All gates already exist; this feature interacts with five of them.

### Mock-drift gate — `scripts/check-mock-drift.sh`

**No action needed.** No new `#[tauri::command]` handlers, no new mock cases. Run as a sanity check; expect green.

### Engine-purity gate — `scripts/check-engine-purity.sh`

**Stays green by construction.** All new code lives under `src/src/components/` and `crates/presto-ipc/`; nothing touches `src/src/engine/`. Zero new `web_sys` references under the engine path.

### Strict static analysis — `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic` + `cargo fmt --check`

**Load-bearing.** New code in `src/src/components/ambient_audio.rs` and the additions in `notifications.rs` and `timer/mod.rs` and `settings.rs` land clippy-pedantic-clean. The existing `#[allow(clippy::struct_excessive_bools)]` on `NotificationSettings` at `crates/presto-ipc/src/settings.rs:167` covers the bool count after `ambient_sound_enabled` is added (the existing inline justification — "every bool maps to an independent UI toggle" — continues to apply; no new `#[allow]` line is added). The Leptos `#[component]` module-wide allowance for `must_use_candidate` / `too_many_lines` carries into the new module per the feature 003 precedent (same single-`view!`-macro-body justification, though `ambient_audio.rs` returns `()` and has no `view!` body — `must_use_candidate` may not fire here, in which case no allowance is needed).

### `wasm-bindgen-test` + `wasm-pack test --node`

**Load-bearing.** Three new RED-first `cargo test` cases in `presto_ipc::settings::tests` land in a RED commit before the GREEN implementation commit (AGENTS.md §Test-first commit ordering). One MANDATORY non-RED-first `wasm-bindgen-test` for the `AmbientAudio` driver state machine lands alongside the implementation in Phase 3.

### Playwright e2e + visual regression

**One baseline regenerates.** `tests/e2e/settings-notifications.spec.js` gains an e2e flow exercising the three new selectors. `tests/e2e/__screenshots__/visual-regression/settings-notifications-chromium-linux.png` regenerates with the per-baseline note from §IV. Other baselines (timer, statistics-*, daily, tag-manager, update-notification, settings-* for the other tabs) stay byte-stable — any diff on those is a regression to fix in code, not absorbed into the baseline.

### Lockfile-drift gate

**No action needed.** No new runtime deps. `Cargo.lock` and `tests/e2e/package-lock.json` are unchanged. The lockfile-drift gate stays green by inaction. The `web-sys` feature-list widening is a `Cargo.toml` change but not a manifest dependency change — `Cargo.lock` is not regenerated because the version / source of `web-sys` is unchanged.

## Implementation phasing

Six phases. Phase 0 vendors the assets and widens the `web-sys` feature list (pre-flight); Phase 1 adds the `AmbientSoundType` enum and `NotificationSettings` fields test-first (IPC); Phase 2 wires the Settings UI; Phase 3 implements the `AmbientAudio` driver + the timer-component gate effect; Phase 4 adds the Playwright e2e flow; Phase 5 regenerates the baseline and runs the final gate sweep.

### Phase 0 — Pre-flight: asset vendoring + `web-sys` feature widening

**Entry**: clean branch `004-ambient-sounds` post-spec.
**Exit**: `crates/presto-ipc/src/settings.rs` defines `pub enum AmbientSoundType` with eight variants and `#[serde(rename_all = "kebab-case")]`; `NotificationSettings` gains the three new fields each with `#[serde(default)]` (or `#[serde(default = "default_ambient_sound_volume")]` for the `u32`); the `Default` impl returns `ambient_sound_enabled: false`, `ambient_sound_type: AmbientSoundType::None`, `ambient_sound_volume: 50`. Three new test cases in `crates/presto-ipc/src/settings.rs::tests` pass. The engine and UI don't read the new fields yet — Phase 2 / 3 do that.
**Test-first**: YES per Principle V (wire-shape contract).
- **Test-first commit ordering** (AGENTS.md §Test-first commit ordering, Principle V): the RED commit lands first (three failing tests; `cargo test --workspace --frozen` exits non-zero on the new asserts). The GREEN commit follows in a separate commit (enum + fields + Default impl land; `cargo test --workspace --frozen` exits zero). The two commits are NOT collapsed.

### Phase 1 — IPC widening: `AmbientSoundType` + `NotificationSettings` evolution (test-first)

**Entry**: Phase 0 complete.
**Exit**: `crates/presto-ipc/src/settings.rs` defines `pub enum AmbientSoundType` with eight variants and `#[serde(rename_all = "kebab-case")]`; `NotificationSettings` gains the three new fields each with `#[serde(default)]` (or `#[serde(default = "default_ambient_sound_volume")]` for the `u32`); the `Default` impl returns `ambient_sound_enabled: false`, `ambient_sound_type: AmbientSoundType::None`, `ambient_sound_volume: 50`. Three new test cases in `crates/presto-ipc/src/settings.rs::tests` pass. The engine and UI don't read the new fields yet — Phase 2 / 3 do that.
**Test-first**: YES per Principle V (wire-shape contract). THREE RED commits precede the GREEN commit; the pairs are NOT collapsed.

### Phase 2 — Settings UI (notifications tab)

**Entry**: Phase 0 complete (IPC field exists). Phase 1 not strictly required for this phase — the UI writes the field through the existing `Settings` signal without touching audio playback yet.
**Exit**: `src/src/components/settings/notifications.rs` gains three controls below the existing metronome row: `#ambient-sound-enabled` checkbox, `#ambient-sound-type` dropdown, `#ambient-sound-volume` range slider. All three visible regardless of checkbox state. Each writes through the existing `save_settings` Tauri command. The `tests/e2e/settings-notifications.spec.js` evolves with an e2e flow exercising the three new selectors. No audio playback yet — that's Phase 3.
**Test-first**: NO (UI plumbing).

### Phase 3 — `AmbientAudio` driver + timer-component gate effect

**Entry**: Phase 0 complete (`HtmlAudioElement` reachable) and Phase 2 complete (UI writes the fields).
**Exit**: `src/src/components/ambient_audio.rs` defines the side-effect driver with the state machine documented in [contracts/components.md](./contracts/components.md), including the pre-emption arcs (CrossFading→FadingOut on gate-flip, FadingOut settings-ignore rule, volume-while-Paused/Idle rules) and the cross-fade completion gate-recheck. `src/src/components/mod.rs` registers the module. `src/src/components/timer/mod.rs` gains a `leptos::Effect::new` that watches the composite gate signal and drives the driver. The MANDATORY non-RED-first wasm-bindgen-test covers all 8 scenarios in the state-machine test matrix (see contracts/components.md and §Testing strategy). The metronome gate at `timer/mod.rs:1358-1368` is untouched.
**Test-first**: PARTIAL — the wasm-bindgen-test is MANDATORY non-RED-first per the §Testing strategy table.

### Phase 4 — E2E: settings-notifications flow for the three new controls

**Entry**: Phases 0–3 complete.
**Exit**: `tests/e2e/settings-notifications.spec.js` gains an e2e flow exercising `#ambient-sound-enabled`, `#ambient-sound-type`, `#ambient-sound-volume` (toggle, pick, drag, persist). `npx playwright test settings-notifications.spec.js --reporter=line` passes.
**Test-first**: NO (UI plumbing — e2e is the backstop).

### Phase 5 — Visual-regression baseline regen + final gate sweep

**Entry**: Phases 0–4 complete.
**Exit**: `tests/e2e/__screenshots__/visual-regression/settings-notifications-chromium-linux.png` is regenerated locally via `npx playwright test tests/e2e/visual-regression.spec.js --update-snapshots`, reviewed visually against the §IV per-baseline justification, and committed in a single commit. Full gate sweep exits 0. The PR description restates the per-baseline note verbatim.
**Test-first**: N/A (visual gate is itself the test).

## Post-design Constitution Check

Re-checked after Phase 1 design (research.md, data-model.md, contracts/components.md, quickstart.md). Verdicts unchanged from §[Constitution Check](#constitution-check). Material principles re-affirmed:

- **I**: contracts/components.md confirms the driver is UI-side only; the engine gains no new state field, no new event, no new `web_sys` import.
- **II**: research.md confirms the vendored-MP3 path is same-origin via Trunk's `copy-dir`; no CDN; no network egress.
- **III**: data-model.md restates `AmbientSoundType` as a closed eight-variant sum type with kebab-case wire shape; `ambient_sound_volume` clamped at the Settings UI input boundary; no defensive guards at the audio call site.
- **IV**: §[Constitution Check IV](#iv-visual-regression-is-the-ui-contract--one-baseline) pre-anchors the one per-baseline justification. quickstart.md lists the verbatim text for copy-paste into the PR description.
- **VI**: contracts/components.md explicitly states "no new Tauri commands"; the mock-drift gate stays green without changes.
- **IX**: research.md confirms the `web-sys` feature-list widening is not a new dependency; `Cargo.lock` is unchanged.

## Complexity Tracking

> No Constitution Check violations require justification. The one IV baseline regeneration is a routine intended change (Principle IV's documented "intended change + one-line note" mechanism), not a widening.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|---|---|---|
| (none) | — | — |
