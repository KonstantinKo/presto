# Tasks: Opt-In Ambient Background Sounds During Focus

**Input**: Design docs in `specs/004-ambient-sounds/`
**Prerequisites**: spec.md (28 FRs, 13 SCs, 15 Assumptions), plan.md (6 phases), data-model.md, contracts/components.md, research.md, quickstart.md

## Format

`- [ ] [TID] [P?] [US?] Description with file path` — User stories: **US1** = looping focus ambient (FR-006–012, SC-001, SC-004–007), **US2** = legacy settings compatibility (FR-001–005, SC-002–003), **US3** = visual regression baseline (FR-021, SC-012). `[P]` = parallelisable with other `[P]` tasks in the same phase. Each task lists its **Done-signal** and **Files**. Test-first tasks carry explicit **RED** / **GREEN** commit-boundary labels (NOT collapsed — separate commits mandatory).

---

## Phase 0 — Pre-flight: asset vendoring + web-sys feature widening

**Goal**: vendor the seven CC0 MP3 files, wire the Trunk `copy-dir` directive, and widen the `web-sys` feature list so `HtmlAudioElement` is reachable in the Leptos crate. No playback logic yet; this unblocks all downstream phases.

**Exit**: `trunk build --release` (from `src/`) succeeds; `dist/assets/audio/ambient/` contains the seven MP3s (or CC0-sourced placeholders); `HtmlAudioElement` feature entry present in `src/Cargo.toml`; `cargo build --workspace --frozen` compiles. `cargo fmt --check && cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic` green.

- [ ] **T001** [P] [Phase 0] Source and vendor seven CC0 MP3 ambient tracks into `src/assets/audio/ambient/` — `rain.mp3`, `fire.mp3`, `library.mp3`, `fan.mp3`, `storm.mp3`, `white-noise.mp3`, `wind.mp3`
  - **Files**: `src/assets/audio/ambient/rain.mp3`, `fire.mp3`, `library.mp3`, `fan.mp3`, `storm.mp3`, `white-noise.mp3`, `wind.mp3` (new directory tree)
  - **Procedure**: source CC0 recordings from freesound.org (filtered to CC0 license) or equivalent CC0 archives. Each file must be ≤2 MB, 60–120 s, normalised to LUFS-I ≈ -23 to -18 (no clipping at slider 100). If real CC0 assets need separate sourcing, generate placeholder silent MP3s via `for track in rain fire library fan storm white-noise wind; do ffmpeg -f lavfi -i anullsrc=channel_layout=stereo:sample_rate=44100 -t 90 -c:a libmp3lame -b:a 128k "src/assets/audio/ambient/${track}.mp3"; done` — placeholders MUST be swapped for real CC0 files before merge per quickstart.md §Placeholder fallback (research.md Decision 3).
  - **Done-signal**: `ls src/assets/audio/ambient/*.mp3 | wc -l` returns 7. `du -sh src/assets/audio/ambient/` is ≤14 MB total. `file src/assets/audio/ambient/rain.mp3` reports MPEG audio. No file exceeds `du -h src/assets/audio/ambient/*.mp3 | awk '$1 > "2.0M"'` threshold. Each of the 7 MP3 files MUST pass a loudness/clip-headroom check. Run `ffmpeg -i <file> -af volumedetect -f null /dev/null 2>&1 | grep max_volume` for each; verify `max_volume` is ≤ -1.0 dB (headroom for slider clip-safety at volume=100). Files failing this gate must be re-mastered or replaced.

- [ ] **T002** [P] [Phase 0] Wire Trunk `copy-dir` directive for the audio asset tree in `src/index.html`
  - **Files**: `src/index.html`
  - **Change**: add `<link data-trunk rel="copy-dir" href="assets/audio" data-target-path="assets/audio" />` between the existing icon and Phosphor copy-dir lines at `:26-35` (mirrors the pattern exactly — see plan.md §Modules table for `src/index.html`). No other change to `index.html`.
  - **Done-signal**: `trunk build --release` (from `src/`) exits 0; `ls dist/assets/audio/ambient/*.mp3 | wc -l` returns 7. No CDN URL is present in the added line. `grep 'assets/audio' src/index.html` returns the new copy-dir line.
  - **BlockedBy**: T001.

- [ ] **T003** [P] [Phase 0] Widen `web-sys` feature list in `src/Cargo.toml` to include `HtmlAudioElement` and `HtmlMediaElement`
  - **Files**: `src/Cargo.toml`
  - **Change**: locate the existing `[dependencies.web-sys]` block (currently contains `AudioContext`, `OscillatorNode`, etc. for `play_chime` and `play_metronome_tick`); append `"HtmlAudioElement"` and `"HtmlMediaElement"` to the `features = [...]` list. No version bump; no new dependency line. `Cargo.lock` is unchanged (no new crate; no version change to `web-sys` — only the in-place feature list grows by 2 entries, per plan.md §IX and §Phase 1).
  - **Done-signal**: `cargo build --workspace --frozen` exits 0. `grep 'HtmlAudioElement' src/Cargo.toml` returns a hit. `git diff Cargo.lock` shows zero changes (no lockfile mutation). `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic` green.

**Phase 0 exit**: `trunk build --release` succeeds. `dist/assets/audio/ambient/` has 7 MP3 files. `cargo clippy` + `cargo fmt --check` + `cargo build --workspace --frozen` all green. `bash scripts/check-mock-drift.sh` exits 0 (no new Tauri commands — no-op at this phase).

---

## Phase 1 — IPC widening: `AmbientSoundType` + `NotificationSettings` evolution [test-first]

**Goal**: Add `AmbientSoundType` enum and three new `#[serde(default)]` fields to `NotificationSettings` in `crates/presto-ipc/src/settings.rs`. Test-first per Principle V (wire-shape contract is the persistence boundary). THREE RED commits precede THREE GREEN commits; the pairs are NOT collapsed.

**Exit**: `cargo test --workspace --frozen -p presto-ipc settings::tests` passes all three new tests alongside the pre-existing metronome legacy tests. `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic` green. Engine and UI do not yet read the new fields — that's Phase 3.

### Test-first triplet (US2 — legacy compat + wire shape)

- [ ] **T004** [US2] [Phase 1] **[test-first RED]** Write failing `presto_ipc::settings::tests::ambient_sound_legacy_fields_default` in `crates/presto-ipc/src/settings.rs`
  - **Files**: `crates/presto-ipc/src/settings.rs` (test module)
  - **Test body**: deserialise the pre-feature-004 `NotificationSettings` JSON fixture (contains only `desktop_notifications`, `sound_notifications`, `auto_start_timer`, `smart_pause`, `smart_pause_timeout`, `metronome` — no ambient fields). Assert `ambient_sound_enabled == false`, `ambient_sound_type == AmbientSoundType::None`, `ambient_sound_volume == 50`. Mirrors `metronome_default_off` at `:362-375` verbatim. Full fixture from contracts/components.md §2 "Legacy fixture round-trip". Commit the failing test separately from implementation.
  - **Done-signal**: `cargo test --workspace --frozen -p presto-ipc settings::tests::ambient_sound_legacy_fields_default` exits **non-zero** (compile-fail or assertion-fail referencing missing `AmbientSoundType` or missing fields on `NotificationSettings`). The test body exists in the source file; no implementation exists yet. The exit non-zero MUST reference a compile error or type-resolution failure on `AmbientSoundType` or the missing `notifications.ambient_*` fields — NOT a `todo!()` body or `assert!(false)` placeholder. The test body MUST reference the types/fields directly so the build fails on missing symbols.

- [ ] **T005** [US2] [Phase 1] **[test-first RED]** Write failing `presto_ipc::settings::tests::ambient_sound_round_trip` in `crates/presto-ipc/src/settings.rs`
  - **Files**: `crates/presto-ipc/src/settings.rs` (test module)
  - **Test body**: deserialise the non-default new-build JSON fixture (`ambient_sound_enabled: true`, `ambient_sound_type: "rain"`, `ambient_sound_volume: 30`, plus `metronome: true` from feature 002). Assert round-trip byte-stable. Assert the feature-002 `metronome: true` field survives the round-trip alongside the new fields (covers Acceptance Scenario 2.6). Full fixture from contracts/components.md §2 "Non-default round-trip". Separate commit from T004.
  - **Done-signal**: `cargo test --workspace --frozen -p presto-ipc settings::tests::ambient_sound_round_trip` exits **non-zero**. Separate commit from T004. The exit non-zero MUST reference a compile error or type-resolution failure on `AmbientSoundType` or the missing `notifications.ambient_*` fields — NOT a `todo!()` body or `assert!(false)` placeholder. The test body MUST reference the types/fields directly so the build fails on missing symbols.
  - **BlockedBy**: T004.

- [ ] **T006** [US2] [Phase 1] **[test-first RED]** Write failing `presto_ipc::settings::tests::ambient_sound_type_serialises_kebab_case` in `crates/presto-ipc/src/settings.rs`
  - **Files**: `crates/presto-ipc/src/settings.rs` (test module)
  - **Test body**: enumerate all eight `AmbientSoundType` variants explicitly with both serialise and deserialise directions: `None ↔ "none"`, `Rain ↔ "rain"`, `Fire ↔ "fire"`, `Library ↔ "library"`, `Fan ↔ "fan"`, `Storm ↔ "storm"`, `WhiteNoise ↔ "white-noise"` (CRITICAL — the multi-word variant), `Wind ↔ "wind"`. Per contracts/components.md §1 "Wire-shape assertion table". Separate commit from T005.
  - **Done-signal**: `cargo test --workspace --frozen -p presto-ipc settings::tests::ambient_sound_type_serialises_kebab_case` exits **non-zero**. Separate commit from T005. The exit non-zero MUST reference a compile error or type-resolution failure on `AmbientSoundType` or the missing `notifications.ambient_*` fields — NOT a `todo!()` body or `assert!(false)` placeholder. The test body MUST reference the types/fields directly so the build fails on missing symbols.
  - **BlockedBy**: T005.

### Implementation GREEN: `AmbientSoundType` enum + `NotificationSettings` fields

- [ ] **T007** [US2] [Phase 1] **[test-first GREEN]** Implement `AmbientSoundType` enum and add three fields to `NotificationSettings` in `crates/presto-ipc/src/settings.rs`
  - **Files**: `crates/presto-ipc/src/settings.rs`
  - **Changes** (per data-model.md §1 and §2):
    1. Add `pub enum AmbientSoundType` with `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]`, `#[cfg_attr(feature = "specta", derive(specta::Type))]`, `#[serde(rename_all = "kebab-case")]`; variants: `#[default] None`, `Rain`, `Fire`, `Library`, `Fan`, `Storm`, `WhiteNoise`, `Wind`. Placed alongside `StatusBarDisplay` (mirrors its pattern at `:25-35`).
    2. Add to `NotificationSettings` struct (after `metronome` field at `:185-186`): `#[serde(default)] pub ambient_sound_enabled: bool`, `#[serde(default)] pub ambient_sound_type: AmbientSoundType`, `#[serde(default = "default_ambient_sound_volume")] pub ambient_sound_volume: u32`.
    3. Add `#[must_use] pub const fn default_ambient_sound_volume() -> u32 { 50 }`.
    4. Update `Default for NotificationSettings` impl to include `ambient_sound_enabled: false, ambient_sound_type: AmbientSoundType::None, ambient_sound_volume: 50`.
    5. The existing `#[allow(clippy::struct_excessive_bools)]` at `:167` continues to cover the bool count — no new `#[allow]` line needed.
  - **Done-signal**: `cargo test --workspace --frozen -p presto-ipc settings::tests::ambient_sound_legacy_fields_default` AND `ambient_sound_round_trip` AND `ambient_sound_type_serialises_kebab_case` all pass. `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic` green. Commit **separately** from T004/T005/T006.
  - **BlockedBy**: T006.

**Phase 1 exit**: `cargo test --workspace --frozen` green (3 new tests + all pre-existing). `cargo clippy` + `cargo fmt --check` green. No engine or UI code reads the new fields yet.

---

## Phase 2 — Settings UI: three new controls in Settings → Notifications

**Goal**: Surface `#ambient-sound-enabled`, `#ambient-sound-type`, `#ambient-sound-volume` in `src/src/components/settings/notifications.rs`, below the existing metronome row. UI plumbing — not test-first per Principle V; e2e is the backstop. IPC fields (T007) must exist first.

**Exit**: `cargo clippy` + `cargo fmt --check` green. All three controls render in Settings → Notifications below the metronome row. Each writes through the existing `save_settings` Tauri command. Controls are visible regardless of checkbox state. No audio playback yet.

- [ ] **T008** [US1] [Phase 2] Add the three ambient-sound controls to `src/src/components/settings/notifications.rs` below the existing metronome row
  - **Files**: `src/src/components/settings/notifications.rs`
  - **Changes** (per plan.md §Modules table and FR-013 / FR-014 / FR-015):
    1. **Checkbox**: `<input id="ambient-sound-enabled" type="checkbox">` with label `"Enable ambient background sound"`. Writes through `settings.notifications.ambient_sound_enabled` via `save_settings` on change.
    2. **Dropdown**: `<select id="ambient-sound-type">` with eight `<option>` entries mapping 1:1 to `AmbientSoundType` variants using user-readable labels: "None" / "Rain" / "Fire" / "Library" / "Fan" / "Storm" / "White noise" / "Wind". Values are the kebab-case wire strings (`"none"`, `"rain"`, etc.). Writes through `settings.notifications.ambient_sound_type`.
    3. **Volume slider**: `<input id="ambient-sound-volume" type="range" min="0" max="100" step="1">` with label `"Volume"`. Writes through `settings.notifications.ambient_sound_volume`. UI-boundary clamp is handled by `min=0 max=100` (FR-004).
    4. All three controls are **visible regardless of checkbox state** (FR-014). Do NOT conditionally hide or disable on `ambient_sound_enabled`.
    5. Selectors are **additive** — no existing selector (`#smart-pause`, `#desktop-notifications`, `#sound-notifications`, metronome checkbox) is renamed or removed (FR-015).
    6. Optional: add a small CSS block for the new range slider in `src/style/settings.css` (or wherever existing settings row styles live) if the project's existing slider style does not already cover it. The CSS addition must NOT affect any baseline outside `settings-notifications-chromium-linux.png`.
  - **Done-signal**: `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic && cargo fmt --check` green. `cargo build --workspace --frozen` compiles. In browser (or `trunk serve`): Settings → Notifications shows the three new controls below the metronome row. Changing each control and saving persists the new fields to `settings.json`. `grep 'ambient-sound-enabled\|ambient-sound-type\|ambient-sound-volume' src/src/components/settings/notifications.rs` returns 3 hits.
  - **BlockedBy**: T007.

**Phase 2 exit**: `cargo clippy` + `cargo fmt --check` green. Settings UI controls render and persist. e2e flow (written in Phase 4) will cover them.

---

## Phase 3 — `AmbientAudio` driver + timer-component gate effect

**Goal**: Implement the `AmbientAudio` state-machine driver (`src/src/components/ambient_audio.rs`) and wire the composite gate `Effect` into the timer component. The MANDATORY non-RED-first wasm-bindgen-test for the state machine ships in the same phase alongside the implementation. Phase 0 (`HtmlAudioElement` reachable) and Phase 2 (UI writes the fields) must both be complete.

**Exit**: `ambient_audio.rs` implements the full 5-state machine (`Idle / Playing / Paused / CrossFading / FadingOut`) with all 10 transition arcs from contracts/components.md §3. `wasm-pack test --node src/ -- --filter ambient_audio::tests` green (8 scenarios). Timer component gate effect drives the driver. `cargo clippy` + `cargo fmt --check` green. `bash scripts/check-engine-purity.sh` exits 0 (zero new `web_sys` references under `src/src/engine/`).

- [ ] **T009** [US1] [Phase 3] Create `src/src/components/ambient_audio.rs` — the state-machine driver
  - **Files**: `src/src/components/ambient_audio.rs` (new), `src/src/components/mod.rs`
  - **Implementation** (per data-model.md §3, contracts/components.md §3, plan.md §Phase 3):
    1. Define internal `enum AmbientAudioState { Idle, Playing { track }, Paused { track }, CrossFading { outgoing, incoming }, FadingOut { track } }`.
    2. Define `const fn asset_path(t: AmbientSoundType) -> Option<&'static str>` returning `Some("/assets/audio/ambient/rain.mp3")` etc. for each non-`None` variant (see data-model.md §1 "Asset-path mapping"). Match-exhaustiveness ensures sync.
    3. Define the `AudioElementHandle` trait (see contracts/components.md §Host-testable projection pattern) with `set_src`, `set_volume`, `play`, `pause`, `current_time`. Provide `HtmlAudioWrapper(HtmlAudioElement)` as the real implementation. The state machine takes a generic `H: AudioElementHandle`. Hold two `RwSignal<Option<Box<dyn AudioElementHandle>>>` slots (`current_audio`, `previous_audio`) and a `RwSignal<Option<gloo::timers::callback::Interval>>` (or `leptos::set_interval` handle) for the active fade ramp. Companion slot occupancy per data-model.md §3 "Companion runtime slots" table.
    4. Expose public functions called from the timer-component gate effect:
       - `start(track: AmbientSoundType, volume: u32)` → arc 1 (`Idle → Playing`), 200 ms fade-in.
       - `pause()` → arc 2 (`Playing → Paused`), 200 ms fade-out then `.pause()`.
       - `resume(volume: u32)` → arc 3 (`Paused → Playing`), 200 ms fade-in.
       - `cross_fade(new_track: AmbientSoundType, volume: u32)` → arc 4 (`Playing → CrossFading`), 300 ms overlapped ramps; completion callback re-checks gate (arc 5 / arc 10 race — contracts/components.md §3 "Cross-fade completion vs gate-flip race").
       - `set_volume(volume: u32)` → arc 10 self-arc on `Playing`, immediate `.volume` update.
       - `fade_out()` → arcs 7/8 (`Playing/Paused → FadingOut`), 200 ms fade-out.
    5. Pre-emption rules from contracts/components.md §3 "Pre-emption rules": new transition cancels in-flight ramp (`IntervalHandle` drop); track-change-while-FadingOut is settings-only update; volume-change-while-Paused updates stored target; arc 6 (`CrossFading → FadingOut` on gate-flip) fades both elements from current `.volume`.
    6. Persistent two-element pre-warm: elements created at `start()` time and kept alive (`.src = ""` when not playing) to maintain WKWebView gesture lease for continuous-sessions auto-start (research.md Decision 1).
    7. Fade ramp implementation: JS-side `set_interval` over 200/300 ms updating `.volume` linearly. Linear in `.volume` is acceptable per plan.md §Technical Context.
    8. RAII cleanup: dropping the component drops element handles.
    9. Module-wide `#![allow(clippy::must_use_candidate)]` with Leptos-`#[component]` justification only if triggered by clippy.
    10. Register `pub mod ambient_audio;` in `src/src/components/mod.rs`.
  - **Done-signal**: `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic && cargo fmt --check` green. `cargo build --workspace --frozen` compiles. `grep -r 'web_sys' src/src/engine/` returns same set as before this task (engine purity preserved). `grep 'pub mod ambient_audio' src/src/components/mod.rs` returns a hit.

- [ ] **T010** [US1] [Phase 3] **[MANDATORY non-RED-first]** Add `ambient_audio::tests::state_transitions` wasm-bindgen-test covering all 9 pre-emption scenarios alongside the implementation
  - **Files**: `src/src/components/ambient_audio.rs` (test module within the file, or `src/src/components/ambient_audio/tests.rs`)
  - **Test coverage** (per contracts/components.md §3 "state_transitions wasm-bindgen-test — full pre-emption matrix"; ALL 9 scenarios required):
    1. **Happy path** — `Idle → Playing → Paused → Playing → CrossFading → Playing → FadingOut → Idle`: full state-machine walk; inject `MockAudioHandle`, assert `calls` log entries (`play`, `pause`, `set_volume:X`, `set_src:Y`) at each transition.
    2. **Disable-during-cross-fade** — `Playing → CrossFading → FadingOut → Idle`: gate_high goes false during 300 ms ramp; arc 6 fires (both elements fade from current volume over 200 ms), NOT arc 5.
    3. **Track-change-ignored-during-FadingOut** — track change while in `FadingOut` updates settings only; no new transition fires; on exit to `Idle`, re-evaluates gate and may start `Playing` with the latest track.
    4. **Cross-fade completion race** — `Playing → CrossFading + gate-flip → FadingOut`: completion callback detects `gate_high == false`; transitions to `FadingOut(new)`, not `Playing(new)`.
    5. **Volume-change-while-Paused** — element stays at `set_volume:0.0` during the change; on resume, fade-in targets the new stored volume.
    6. **Volume-change-while-Idle** — no transition fires; on first `Playing` entry, ramp targets the updated volume.
    7. **Rapid-fire track changes** — `Playing(a) → track-change-to-b → track-change-to-c` within 100 ms: each new change cancels in-flight cross-fade; final settled value wins.
    8. **None → real → None cycle** — `Idle → Playing → FadingOut → Idle`: full cycle; confirms cross-fade collapses to fade-in when starting from no prior track.
    9. **Disable-while-Paused** — state in `Paused(track)`, user toggles `ambient_sound_enabled = false`. Expected: transitions to `FadingOut`; `MockAudioHandle.calls` MUST contain a `pause` entry and no `play` entry after the fade completes; final state is `Idle`.
    Inject `MockAudioHandle` (defined in `ambient_audio.rs`, implements `AudioElementHandle`) per contracts/components.md §Host-testable projection pattern. `wasm-pack test --node` has no DOM — `HtmlAudioElement` is unavailable; `MockAudioHandle` is the test-environment stand-in.
  - **Done-signal**: `wasm-pack test --node src/ -- --filter ambient_audio::tests` exits 0 (all 9 scenarios pass). Lands in the **same commit** as T009's implementation (not before — coverage gate, not RED-first, per plan.md §V and feature 003 precedent for `tooltip_text_matrix`).
  - **BlockedBy**: T009.

- [ ] **T011** [US1] [Phase 3] Wire the `AmbientAudio` gate effect into `src/src/components/timer/mod.rs`
  - **Files**: `src/src/components/timer/mod.rs`
  - **Changes** (per plan.md §Modules table for `timer/mod.rs` and contracts/components.md §3 "Public surface"):
    1. Add a `leptos::Effect::new` in the timer-component init body, immediately adjacent to the existing metronome gate at `:1358-1368`. The effect watches the composite gate signal: `notifications.ambient_sound_enabled && ambient_sound_type != AmbientSoundType::None && current_mode == Focus && is_running() && !is_paused() && !is_auto_paused() && time_remaining_secs() > 0` (mirrors metronome gate structure; FR-007).
    2. **Rising edge** (gate false → true): call `ambient_audio::start(track, volume)`.
    3. **Falling edge** (gate true → false): call `ambient_audio::fade_out()`.
    4. **Track change while gate is high** (`ambient_sound_type` reactive value changes, old != new, new != None): call `ambient_audio::cross_fade(new_track, volume)`.
    5. **Volume change while gate is high** (`ambient_sound_volume` changes): call `ambient_audio::set_volume(new_volume)`.
    6. **Pause / smart-pause / overtime entry** (gate sub-condition `is_paused || is_auto_paused || time_remaining_secs <= 0` rising while mode is still Focus): call `ambient_audio::pause()`.
    7. **Resume from pause** (gate sub-condition falling while mode is Focus and time > 0): call `ambient_audio::resume(volume)`.
    8. The metronome gate at `:1358-1368` is **untouched** (FR-006 / Principle I).
    9. Import `use crate::components::ambient_audio;` at the top of the file.
  - **Done-signal**: `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic && cargo fmt --check` green. `bash scripts/check-engine-purity.sh` exits 0 (zero new `web_sys` references under `src/src/engine/`). `grep -n 'web_sys' src/src/engine/` returns same set as before. The metronome gate at `:1358-1368` is byte-stable (`git diff` on that range shows no change). Manual smoke-test (if desktop available): focus session with Rain selected and feature enabled → rain fades in; pause → fades out; resume → fades in.
  - **BlockedBy**: T008, T010.

**Phase 3 exit**: `wasm-pack test --node src/` green (8 state-machine scenarios). `cargo clippy` + `cargo fmt --check` green. `bash scripts/check-engine-purity.sh` exits 0. Full ambient playback lifecycle works end-to-end (driver + gate effect + settings fields + vendored assets).

---

## Phase 4 — E2E: settings-notifications flow for the three new controls

**Goal**: Add a Playwright e2e flow to `tests/e2e/settings-notifications.spec.js` exercising `#ambient-sound-enabled`, `#ambient-sound-type`, `#ambient-sound-volume` (toggle, pick, drag, persist). Audio playback itself is NOT asserted (headless chromium has no audio output path); the wasm-bindgen-test in Phase 3 covers state-machine correctness.

**Exit**: `npx playwright test settings-notifications.spec.js --reporter=line` passes (all flows including the new ambient flow). No existing selectors are modified or removed.

- [ ] **T012** [US1] [Phase 4] Extend `tests/e2e/settings-notifications.spec.js` with an ambient-sound e2e flow
  - **Files**: `tests/e2e/settings-notifications.spec.js`
  - **Test flow** (per plan.md §Modules table, quickstart.md §New e2e flow, FR-013 / FR-015):
    1. Navigate to Settings → Notifications.
    2. Toggle `#ambient-sound-enabled` on (assert it becomes checked).
    3. Pick `Rain` (`"rain"`) from `#ambient-sound-type` dropdown (assert option selected).
    4. Drag (or fill) `#ambient-sound-volume` slider to 30 (assert `.value == "30"`).
    5. Close and reopen Settings → Notifications.
    6. Assert `#ambient-sound-enabled` is checked, `#ambient-sound-type` shows `"rain"`, `#ambient-sound-volume` value is `"30"` — values persist across re-open.
    7. Toggle `#ambient-sound-enabled` off (assert unchecked); assert the dropdown and slider are still **visible** (FR-014 — not hidden on disable).
    8. Pick `"none"` from `#ambient-sound-type` while feature is off; assert slider value is still `"30"` (volume preserved on None selection, FR-005).
    - No audio playback assertions — covered by wasm-bindgen-test.
    - Do NOT rename or remove any existing selector (`#smart-pause`, `#desktop-notifications`, `#sound-notifications`, metronome selectors if any) (FR-015).
  - **Done-signal**: `npx playwright test settings-notifications.spec.js --reporter=line` exits 0 (all tests including the new ambient flow). `grep 'ambient-sound-enabled\|ambient-sound-type\|ambient-sound-volume' tests/e2e/settings-notifications.spec.js` returns ≥3 hits. No existing test in that file fails.
  - **BlockedBy**: T008.

**Phase 4 exit**: `npx playwright test settings-notifications.spec.js --reporter=line` green. e2e selector contract satisfied (FR-015).

---

## Phase 5 — Visual-regression baseline regen + final gate sweep

**Goal**: Regenerate exactly one visual regression baseline (`settings-notifications-chromium-linux.png`) — the only screen that changed. Confirm no other baselines flag a diff. Run the full gate sweep.

**Exit**: CI visual-regression run sees only one failing baseline diff (`settings-notifications-chromium-linux.png`). After regen, all baselines pass. Full gate sweep exits 0. SC-009, SC-010, SC-011, SC-012 all satisfied.

### User Story 3 — Visual regression baseline (FR-021 / SC-012)

- [ ] **T013** [US3] [Phase 5] Confirm only `settings-notifications-chromium-linux.png` diffs, then regenerate it
  - **Files**: `tests/e2e/__screenshots__/visual-regression/settings-notifications-chromium-linux.png`
  - **Procedure** (per quickstart.md §Regenerate affected baseline and plan.md §Phase 5):
    1. Run `cd tests/e2e && npx playwright test visual-regression.spec.js --reporter=line` — confirm the ONLY failing baseline is `settings-notifications-chromium-linux.png`. Any diff on timer, statistics, daily, tag-manager, or other settings tabs is a regression to fix in code — do NOT absorb by re-baselining (FR-021 / SC-012).
    2. Regenerate: `npx playwright test visual-regression.spec.js --update-snapshots --grep "settings-notifications"`.
    3. Review the regenerated PNG visually: three new affordances should appear below the metronome row — checkbox ("Enable ambient background sound"), track dropdown ("Ambient sound"), volume slider ("Volume"). No other layout change.
    4. Update `README.md` to document the Linux runtime dependency on `gstreamer1-plugins-bad-free` and `gstreamer1-libav` for MP3 audio decoding via WebKitGTK. Done-signal: `grep -i 'gstreamer\|libav' README.md` returns at least one hit.
    5. Stage and commit the single PNG: `git add __screenshots__/visual-regression/settings-notifications-chromium-linux.png`.
  - **Done-signal**: `git status tests/e2e/__screenshots__/visual-regression/ | grep -v '^?' | wc -l` returns 1 (exactly one PNG modified). `npx playwright test visual-regression.spec.js --reporter=line` exits 0 (all baselines pass after regen). The PR description includes the per-baseline note verbatim: `settings-notifications-chromium-linux.png: ambient-sound checkbox, track dropdown, and volume slider added below the metronome row. No other layout change.` (per plan.md §IV and quickstart.md §Per-baseline justification).
  - **BlockedBy**: T012.

### Final gate sweep

- [ ] **T014** [Phase 5] Full final gate sweep before opening the PR
  - **Files**: (read-only verification; no source edits)
  - **Done-signal** (ALL must exit 0 or return expected values):
    - `cargo fmt --check`
    - `cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic`
    - `cargo test --workspace --frozen` (includes the 3 new IPC round-trip tests + all pre-existing)
    - `cd src && wasm-pack test --node` (includes the 9 state-machine scenarios in `ambient_audio::tests`)
    - `cd src && trunk build --release` (cold build; confirms copy-dir wires the MP3 assets)
    - `cd tests/e2e && npx playwright test --reporter=line` (full e2e including new ambient flow + visual regression)
    - `bash scripts/check-engine-purity.sh` (SC-009: zero new `web_sys` under `src/src/engine/`)
    - `bash scripts/check-mock-drift.sh` (SC-010: 0 new Tauri commands; no new mock entries)
    - `git diff Cargo.lock` returns zero lines of added dependencies (SC-010: no new runtime deps)
    - `grep -r 'web_sys' src/src/engine/` — returns same set as before the feature (SC-009)
    - `grep -rn 'fetch(\|reqwest\|supabase\|aptabase' src/ tests/` — returns 0 hits (SC-011: no new network egress)
    - `grep -i 'ramazan\|murdercode' specs/004-ambient-sounds/tasks.md` — returns 0 hits
  - **BlockedBy**: T013.

**Phase 5 exit**: All gates exit 0. PR ready to open with the per-baseline note in the description.

---

## Dependencies (compact)

- **Phase 0** (T001–T003): T001 → T002 (assets must exist before `index.html` is wired). T003 is independent of T001/T002 and can run in parallel with both.
- **Phase 1** (T004–T007): T004 (RED) → T005 (RED) → T006 (RED) → T007 (GREEN). All four are sequential. RED commits land first, separately from T007. **The three RED commits and the single GREEN commit are NOT collapsed.**
- **Phase 2** (T008): Blocked by T007 (IPC fields must exist). Can run in parallel with Phase 0 completion if T007 is done.
- **Phase 3** (T009–T011): T009 → T010 (in same commit) → T011. T009+T010 blocked by T003 (HtmlAudioElement reachable) AND T007 (AmbientSoundType importable). T011 blocked by T010 AND T008 (timer reads settings fields).
- **Phase 4** (T012): Blocked by T008 (controls must exist in the DOM for e2e selectors to resolve).
- **Phase 5** (T013–T014): T013 blocked by T012 (e2e flow must pass before baseline regen). T014 blocked by T013.

## Parallel opportunities

- T001 (asset sourcing) + T003 (web-sys widening) + T004 (first RED test) can all start in parallel immediately (they touch different files: `src/assets/`, `src/Cargo.toml`, `crates/presto-ipc/src/settings.rs`).
- T002 (index.html copy-dir) can start as soon as T001 completes.
- Phase 2 (T008, settings UI) and Phase 0 are fully independent once T007 completes and can run in parallel with Phase 0 asset sourcing if the IPC layer is ready.
- T010 (wasm-bindgen-test) ships in the same commit as T009 (driver implementation) — they are not separate tasks in terms of commit sequencing, but are listed separately for traceability.
- T012 (e2e flow) and T009/T010/T011 (driver + wire) can run in parallel after T008, since they touch different files (`tests/e2e/` vs `src/src/components/`).

---

## Notes

- **RED/GREEN commits are NOT collapsed** for T004, T005, T006 (each RED lands separately), then T007 (GREEN). Four separate commits in Phase 1. Per AGENTS.md §Test-first commit ordering and plan.md §Phase 0.
- **T010 (wasm-bindgen-test) is non-RED-first** — it lands alongside T009 in Phase 3, NOT before it. It is a MANDATORY coverage gate per plan.md §Testing strategy, not a Principle V RED-first pair. Mirroring feature 003's `tooltip_text_matrix` posture (T019 in `specs/003-stats-redesign/tasks.md`).
- **No new Tauri commands** — `bash scripts/check-mock-drift.sh` stays green throughout. `tests/e2e/fixtures/tauriMock.js` is untouched. The three new `NotificationSettings` fields flow transparently through the existing `save_settings` / `load_settings` round-trip (FR-019 / plan.md §VI).
- **No new runtime deps** — `src/Cargo.toml` gains two feature-list entries on the existing `web-sys` dependency block; `Cargo.lock` is unchanged (no new crate, no version bump). `tests/e2e/package-lock.json` is unchanged (no new npm dep). (FR-018 / SC-010 / plan.md §IX.)
- **Engine purity**: all new code lives under `src/src/components/` and `crates/presto-ipc/`; `src/src/engine/` is byte-stable. Principle I enforced by construction; verified by `bash scripts/check-engine-purity.sh` in T011 and T014 (FR-006 / SC-009 / plan.md §I).
- **Asset format**: MP3 only — no OGG fallback (research.md Decision 2). Linux deployment caveat: `gstreamer1-libav` must be installed for WebKitGTK MP3 decoding; document in release notes before merge.
- **Placeholder MP3s are acceptable for T001** if real CC0 assets need separate sourcing time. They MUST be swapped for real CC0 files before merge (quickstart.md §Placeholder silent MPs / research.md Decision 3 / A6). The GREEN IPC commit (T007) may land with placeholders.
- **Volume `0` is not a disable sentinel** (FR-005 / A11): the active loop continues at zero amplitude. The slider's left end is valid, not "off". This is enforced at the UI boundary by `min=0 max=100` but NOT by any guard in the driver.
- **Only one visual regression baseline regenerates** — `settings-notifications-chromium-linux.png`. The timer screen baseline does NOT change (no timer-screen chrome added). Any diff on untouched screens (timer, statistics, daily, tag-manager, other settings tabs) is a regression to fix in code, not absorbed by re-baselining (FR-021 / SC-012 / Principle IV).
- **`AmbientSoundType::None` is a first-class variant** (not `Option<AmbientSoundType>`, not `""`). Wire string `"none"`. Deserialises from absent field via `#[serde(default)]` which calls `AmbientSoundType::default()` = `None` variant (A5 / FR-002 / Principle III).
- **Continuous-sessions auto-start gate**: the ambient gate is "focus mode active and ticking", NOT "user pressed Start". Continuous-sessions auto-start triggers the gate rising-edge exactly as a manual Start click does — the persistent `HtmlAudioElement` pre-warm maintains the WKWebView gesture lease across auto-start boundaries (research.md Decision 1 / FR-009 / Acceptance Scenario 1.10).
- **No spec artefact references fork attribution**: `grep -i 'ramazan\|murdercode' specs/004-ambient-sounds/tasks.md` returns 0 hits (verified in T014).
