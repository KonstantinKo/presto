# Quickstart — Feature 004 (Ambient Background Sounds)

Contributor's path to exercising the feature end-to-end and regenerating the affected baseline. Assumes a clean `004-ambient-sounds` checkout; if `cargo` / `trunk` / `npx` are not on `PATH`, see the root `AGENTS.md` and `CLAUDE.md`.

## Local build

```bash
# Frontend (Leptos + WASM via Trunk)
cd src
trunk build --release   # or: trunk serve  (dev mode, hot reload, http://localhost:1420)

# Backend (Tauri 2.x) — NOT runnable in CI / agentex worktrees (needs GUI deps).
# For local desktop runs only:
cargo tauri dev
```

A `trunk build --release` step is the cold-start sanity check that picks up the new `<link data-trunk rel="copy-dir" href="assets/audio" data-target-path="assets/audio" />` in `src/index.html`. After the build, the `dist/` tree should contain `dist/assets/audio/ambient/*.mp3` mirroring `src/assets/audio/ambient/*.mp3` byte-for-byte.

## Where to drop the vendored MP3 files

Seven files, one per non-`None` `AmbientSoundType` variant, dropped at:

```text
src/assets/audio/ambient/rain.mp3
src/assets/audio/ambient/fire.mp3
src/assets/audio/ambient/library.mp3
src/assets/audio/ambient/fan.mp3
src/assets/audio/ambient/storm.mp3
src/assets/audio/ambient/white-noise.mp3
src/assets/audio/ambient/wind.mp3
```

### File constraints (per SC-008 / A6 / Research Decision 2)

- Format: MP3 only (see [research.md §Decision 2](./research.md#decision-2--audio-format-mp3-only)).
- Size: ≤2 MB per file. Total ≤14 MB.
- Length: 60–120 s per file (recommended; ambient feel requires enough material to avoid perceived repetition).
- Licensing: CC0 / public domain / equivalent royalty-free with no UI-attribution obligation (see [research.md §Decision 3](./research.md#decision-3--asset-licensing-cc0--equivalent-royalty-free-no-ui-attribution-obligation)).
- Mastering: normalised to a consistent perceptual loudness (LUFS-I around -23 to -18) so the volume slider has a comparable meaning across tracks. No clipping at slider value `100` on a system with default audio levels (SC-013).

### Placeholder silent MP3s (acceptable for the test-first GREEN commit)

If real CC0 assets need separate sourcing, the GREEN commit may ship placeholder silent MP3s generated via:

```bash
# 90 s of silent stereo MP3 at 128 kbps — well under 2 MB.
for track in rain fire library fan storm white-noise wind; do
  ffmpeg -f lavfi -i anullsrc=channel_layout=stereo:sample_rate=44100 \
         -t 90 -c:a libmp3lame -b:a 128k "src/assets/audio/ambient/${track}.mp3"
done
```

The real CC0 files MUST be swapped in before merge. Don't ship placeholders to users.

## How to run the new tests

### Three RED-first IPC round-trip tests (`cargo test`)

These are the test-first failing tests that precede the implementation commit (see `plan.md §Phase 0`):

```bash
# From the repository root:
cargo test --workspace --frozen \
    -p presto-ipc \
    -- \
    ambient_sound_legacy_fields_default \
    ambient_sound_round_trip \
    ambient_sound_type_serialises_kebab_case
```

Or run the whole `settings.rs` test module to see them alongside the existing metronome legacy tests:

```bash
cargo test --workspace --frozen -p presto-ipc settings::tests
```

### MANDATORY non-RED-first wasm-bindgen-test for the driver state machine

```bash
# Requires wasm-pack (cargo install wasm-pack).
cd src
wasm-pack test --node -- \
    --test ambient_audio_state_transitions
```

(Exact test target name may vary depending on how Phase 3 task generation lays out the test file — adjust the `--test` flag accordingly.)

### New e2e flow (Settings → Notifications round-trip)

```bash
# From the repository root:
cd tests/e2e
npx playwright test settings-notifications.spec.js --reporter=line
```

The new flow exercises:
- Toggle `#ambient-sound-enabled` on.
- Pick `Rain` from `#ambient-sound-type`.
- Drag `#ambient-sound-volume` to 30.
- Close and reopen Settings — assert all three values persist.

The audio playback itself is NOT exercised in e2e (chromium headless has no audio-output assertion). Audible behaviour is covered by the wasm-bindgen-test for the state machine and by PR-time manual review.

## How to regenerate the affected baseline

Exactly one baseline regenerates: `tests/e2e/__screenshots__/visual-regression/settings-notifications-chromium-linux.png`.

```bash
cd tests/e2e
npx playwright test visual-regression.spec.js \
    --update-snapshots \
    --grep "settings-notifications"
```

After regeneration, review the diff visually against the per-baseline justification (below) and commit the regenerated baseline. The PR description MUST include the per-baseline note verbatim.

### Per-baseline justification (paste verbatim into the PR description)

> `settings-notifications-chromium-linux.png`: ambient-sound checkbox, track dropdown, and volume slider added below the metronome row. No other layout change.

### Sidebar mask still in effect

The feature 003 sidebar-mask posture (`mask: [page.locator(".sidebar")]` on non-sidebar baselines) remains active. This feature does NOT change the sidebar, so the mask is irrelevant to whether other baselines diff. Per SC-012, no baseline outside Settings → Notifications regenerates — any diff on the timer / statistics / daily / tag-manager / update-notification baselines is treated as a regression to fix in code, not absorbed into the baseline.

## Full local gate sweep (pre-PR)

Mirror what CI will run:

```bash
# Lints + formatting
cargo fmt --check
cargo clippy --workspace --all-targets --frozen -- -D warnings -W clippy::pedantic

# Unit + integration tests
cargo test --workspace --frozen

# Frontend build (cold)
cd src && trunk build --release && cd ..

# wasm-bindgen-tests
cd src && wasm-pack test --node && cd ..

# E2E + visual regression
cd tests/e2e && npx playwright test --reporter=line && cd ../..

# Mock-drift gate (no-op for this feature; sanity check)
bash scripts/check-mock-drift.sh

# Engine-purity gate (zero new web_sys references under src/src/engine/)
bash scripts/check-engine-purity.sh

# Lockfile-drift gate (no-op for this feature; sanity check)
bash scripts/check-lockfile-drift.sh   # (or whatever this gate's script is named in the post-003 layout)
```

All gates should exit zero. If any fail, fix forward (do NOT `--no-verify`).

## Smoke-test the audible behaviour (manual)

Headless chromium can't observe audio; for the audible PR-time smoke test, run `cargo tauri dev` on a desktop with audio output and walk:

1. Open Settings → Notifications. Confirm the three new controls render below the metronome row.
2. Tick `Enable ambient background sound`. Pick `Rain`. Drag the volume slider to ~30.
3. Close Settings. Press Start on the timer (focus session).
4. Within ~200 ms, rain should fade in. Confirm it loops continuously for the session duration.
5. Press Pause. Within ~200 ms, rain fades out.
6. Press Resume. Within ~200 ms, rain fades in.
7. Press Skip (or wait for focus to zero-cross). At the focus → break transition, rain fades out within ~200 ms and stays silent through the break.
8. Re-open Settings, change track to `Fire` while focus is running (start a fresh focus first). Within 300 ms, rain fades down and fire fades in simultaneously.
9. Drag the volume slider while focus is running and fire is playing. The fire volume updates live, no restart.
10. Toggle `Enable ambient background sound` off. Within 200 ms, fire fades out.
11. Toggle back on with track `None`. No fade-in (None = no track to play).
12. Pick `White noise`. Within 300 ms, white noise fades in (cross-fade-from-none collapses to a fade-in).

If any step deviates from the spec's Acceptance Scenarios, file a bug — don't ship.
