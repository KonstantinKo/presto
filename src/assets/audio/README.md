# Ambient audio assets (feature 004)

Ten WAV files under `ambient/`, one per non-`None` variant of
`AmbientSoundType` (see `crates/presto-ipc/src/settings.rs`).
All files are PCM 16-bit stereo:

| File | Source | Duration | Channels | Sample rate | Size |
|---|---|---|---|---|---|
| `rain.wav` | BigSoundBank #0820 | 59.9 s | stereo | 22.05 kHz | 5.0 MB |
| `fire.wav` | BigSoundBank #2855 | 59.9 s | stereo | 22.05 kHz | 5.0 MB |
| `library.wav` | BigSoundBank #2561 | 19.9 s | stereo | 22.05 kHz | 1.7 MB |
| `fan.wav` | BigSoundBank #1232 | 12.9 s | stereo | 22.05 kHz | 1.1 MB |
| `storm.wav` | BigSoundBank #2719 | 59.9 s | stereo | 22.05 kHz | 5.0 MB |
| `wind.wav` | BigSoundBank #1450 | 59.9 s | stereo | 22.05 kHz | 5.0 MB |
| `white-noise.wav` | ffmpeg `anoisesrc` | 10 s | 44.1 kHz | 1.7 MB |
| `pink-noise.wav` | ffmpeg `anoisesrc` | 10 s | 44.1 kHz | 1.7 MB |
| `brown-noise.wav` | ffmpeg `anoisesrc` | 10 s | 44.1 kHz | 1.7 MB |
| `binaural.wav` | ffmpeg sines | 5 s | 44.1 kHz | 882 KB |

## Why WAV (not MP3)

The driver feeds each track to a Web Audio `AudioBufferSourceNode`
with `.loop = true`. Source-node loop is sample-accurate by spec.
MP3 carries LAME priming + end-padding samples that
`AudioContext.decodeAudioData` does NOT strip on WebKit, so an
MP3-backed source loops with ~26 ms of silence at every wrap —
audible seam. WAV has no codec padding, so the seam is
sample-tight.

## Why crossfade-swap (loop-prep technique)

Even with sample-accurate playback, a randomly trimmed segment
can still betray its loop point if amplitude or texture at the
splice differs from t=0. The recipe below avoids that:

1. Take a `D`-second window from the source.
2. Split at the midpoint into `[0, D/2]` and `[D/2, D]`.
3. Reorder: second half first, then first half.
4. Apply a short (100 ms) triangular crossfade at the join.

Output start and end are both the original-midpoint sample →
guaranteed identical content at the loop boundary, regardless of
how dynamic the source is. The hidden seam lives in the middle
of the output instead, where it is masked by the crossfade.

## Sourcing (organic tracks)

All six organic tracks are sourced from
[BigSoundBank](https://bigsoundbank.com) by Joseph SARDIN. The
site's license page is unambiguous: every clip there is released
under **CC0 (public domain)**, free for commercial and personal
use without attribution. Direct download URL pattern:
`https://bigsoundbank.com/UPLOAD/mp3/<id>.mp3`.

Track IDs:
- `rain.wav` ← s0820 (rain and thunder in a tent — stationary rain section, no thunder in window)
- `fire.wav` ← s2855 (Fireplace #3, professional Neumann recording). Needs `acompressor` + `alimiter` to flatten the wide dynamic range — sparse loud crackles + quiet baseline. Otherwise the WAV is near-silent.
- `library.wav` ← s2561 (coffee shop at the capucins, "easy to loop")
- `fan.wav` ← s1232 (air/water heat pump, front, "easy to loop")
- `storm.wav` ← **composed** in the style of [mynoise.net's thunder generator](https://mynoise.net/NoiseMachines/thunderNoiseGenerator.php). Eight-layer mix in a single ffmpeg invocation:
  - Rain bed: s0820 stationary rain (70 s window, +1.5 dB)
  - Rumble #1: brown noise, 20–70 Hz band-pass, +12 dB
  - Rumble #2: brown noise, 55–140 Hz band-pass, +8 dB (spectral counterpoint to rumble #1)
  - Low thunder #1: brown noise, 20–180 Hz, 250 ms tri attack + 4.5 s exp decay, at t=7 s, +15 dB
  - Low thunder #2: brown noise, 20–160 Hz, 300 ms tri attack + 3.8 s exp decay, at t=33.5 s, +14 dB
  - Normal thunder #1: brown noise, 35–450 Hz, 40 ms attack + 2.1 s exp decay, at t=14.5 s, +14 dB
  - Normal thunder #2: brown noise, 40–500 Hz, 30 ms attack + 1.9 s exp decay, at t=23.8 s, +15 dB
  - Normal thunder #3: brown noise, 30–550 Hz, 20 ms attack + 2.4 s exp decay, at t=46.5 s, +16 dB

  Mix weights `0.3 0.55 0.45 0.85 0.9 0.9 0.85 0.95` (rain very low so rumbles + thunders dominate). After `alimiter`, crossfade-swap reorders the 60 s window so thunders land at output times 3.5 s / 16.5 s / 37 s / 44.4 s / 53.7 s — irregular intervals, none near the loop boundary. Python wrap+edge fade enforces sample-accurate seam.
- `wind.wav` ← s1450 (strong wind in trees #1)

## Loop length vs. file size

Stationary content (`fan` hum, `library` chatter) loops invisibly
even at ~15–20 s. Event-driven content (rain texture, fire
crackles, wind gusts, storm thunder) needs a longer window so
the pattern doesn't recognisably repeat. With the relaxed asset
budget, organics are encoded **stereo 22.05 kHz @ 60 s** (~5 MB
each).

## Regenerate (organic tracks)

```bash
# Stereo (short, stationary content)
process_loop_stereo() {
  local id=$1 out=$2 start=$3 dur=$4
  local half=$(echo "$dur / 2" | bc -l)
  curl -sL -o "/tmp/${id}.mp3" "https://bigsoundbank.com/UPLOAD/mp3/${id}.mp3"
  ffmpeg -y -i "/tmp/${id}.mp3" -af "
    atrim=${start}:$(echo "$start + $dur" | bc -l),asetpts=PTS-STARTPTS,
    asplit=2[a][b];
    [a]atrim=0:${half},asetpts=PTS-STARTPTS[p1];
    [b]atrim=${half}:${dur},asetpts=PTS-STARTPTS[p2];
    [p2][p1]acrossfade=d=0.1:c1=tri:c2=tri,
    loudnorm=I=-23:TP=-1.5:LRA=11,
    aresample=22050
  " -ar 22050 -c:a pcm_s16le -ac 2 "$out"
}

# Mono (longer, event-driven content)
process_loop_mono() {
  local id=$1 out=$2 start=$3 dur=$4
  local half=$(echo "$dur / 2" | bc -l)
  curl -sL -o "/tmp/${id}.mp3" "https://bigsoundbank.com/UPLOAD/mp3/${id}.mp3"
  ffmpeg -y -i "/tmp/${id}.mp3" -af "
    atrim=${start}:$(echo "$start + $dur" | bc -l),asetpts=PTS-STARTPTS,
    asplit=2[a][b];
    [a]atrim=0:${half},asetpts=PTS-STARTPTS[p1];
    [b]atrim=${half}:${dur},asetpts=PTS-STARTPTS[p2];
    [p2][p1]acrossfade=d=0.1:c1=tri:c2=tri,
    loudnorm=I=-23:TP=-1.5:LRA=11,
    aresample=22050
  " -ar 22050 -c:a pcm_s16le -ac 1 "$out"
}

# Event-driven (mono, 40 s)
process_loop_mono 0820 rain.wav   90 40
process_loop_mono 0740 storm.wav  40 40
process_loop_mono 1451 wind.wav   60 40
process_loop_mono 0989 fire.wav   30 40

# Stationary (stereo, 20 s / 13 s)
process_loop_stereo 2561 library.wav 60 20
process_loop_stereo 1232 fan.wav     0  13
```

## Regenerate (synthesised tracks)

```bash
ffmpeg -y -f lavfi -i "anoisesrc=color=pink:duration=10:amplitude=0.5:sample_rate=44100" \
  -af "volume=-3dB" -c:a pcm_s16le -ac 2 pink-noise.wav

ffmpeg -y -f lavfi -i "anoisesrc=color=brown:duration=10:amplitude=0.6:sample_rate=44100" \
  -af "volume=-2dB" -c:a pcm_s16le -ac 2 brown-noise.wav

ffmpeg -y -f lavfi -i "anoisesrc=color=white:duration=10:amplitude=0.5:sample_rate=44100" \
  -af "volume=-3dB" -c:a pcm_s16le -ac 2 white-noise.wav

ffmpeg -y \
  -f lavfi -i "sine=frequency=200:duration=5:sample_rate=44100" \
  -f lavfi -i "sine=frequency=240:duration=5:sample_rate=44100" \
  -filter_complex "[0:a][1:a]amerge=inputs=2,volume=-3dB" \
  -c:a pcm_s16le -ac 2 binaural.wav
```

## Constraints (per spec)

- CC0 / public domain only. No CC-BY (would force an attribution UI
  surface we don't have).
- ≤ ~6 MB per file (loose; pick smallest size that fits the loop-period
  budget without obvious repetition).
- Max peak ≤ -1 dB for slider clip-safety at volume = 100.
- Loop-friendly (stationary content + crossfade-swap loop-prep).
