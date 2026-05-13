# Ambient audio assets (feature 004)

Seven MP3 files under `ambient/`, one per non-`None` variant of
`AmbientSoundType` (see `crates/presto-ipc/src/settings.rs`):

```
ambient/rain.mp3
ambient/fire.mp3
ambient/library.mp3
ambient/fan.mp3
ambient/storm.mp3
ambient/white-noise.mp3
ambient/wind.mp3
```

## Status: PLACEHOLDERS

The current vendored files are **placeholders**, NOT the final CC0
recordings. They were generated locally via `ffmpeg` for the test-first
GREEN commit so the IPC enum + driver can compile and the e2e selector
contract can be exercised:

- `white-noise.mp3` — real 60 s stereo white noise via `anoisesrc`,
  mastered to ≈ -4.8 dB peak (≥ -1 dB headroom for slider clip-safety
  at volume = 100, per SC-008 / SC-013).
- `rain.mp3`, `fire.mp3`, `library.mp3`, `fan.mp3`, `storm.mp3`,
  `wind.mp3` — 60 s silent placeholders generated via
  `ffmpeg -f lavfi -i anullsrc -t 60 -c:a libmp3lame -b:a 128k`.

## Follow-up: source real CC0 audio

Per [`specs/004-ambient-sounds/research.md`](../../../specs/004-ambient-sounds/research.md)
Decision 3, the seven tracks **MUST** be replaced with real CC0
(Creative Commons Zero / public domain) recordings before public
release. The placeholders are acceptable for the test-first commit
boundary and for headless e2e (which does not assert on audio output),
but shipping silent files to users would be a regression.

Sourcing constraints (per spec):

- CC0 / public domain only. No CC-BY (would force an attribution UI
  surface we don't have).
- ≤ 2 MB per file, 60–120 s loop-friendly material.
- LUFS-I target around −23 to −18; max peak ≤ −1 dB for slider
  clip-safety at volume = 100.
- Suggested sources: filtered freesound.org (CC0), public-domain
  field-recording archives.

Follow-up issue text (suggested):

> Replace placeholder ambient MP3s with real CC0 recordings
>
> The seven ambient tracks under `src/assets/audio/ambient/` are
> currently silent placeholders (white-noise excepted). Source real
> CC0 recordings of rain, fire, library, fan, storm, white noise, and
> wind that meet the spec's loudness + headroom constraints, then
> drop them in via byte-stable filename swap. No IPC or driver code
> needs to change.
