# Data Model: Per-Session Titles, Configurable Long-Break Cadence, Opt-In Metronome

**Phase**: 1 (Design & Contracts)
**Feeds**: [plan.md](./plan.md) §Modules, [contracts/wire-shapes.md](./contracts/wire-shapes.md)

This document captures the **four wire-shape evolutions** for feature 002. Each is a single field added to an existing IPC struct in `crates/presto-ipc/`, gated by `#[serde(default)]` so pre-bundle records deserialise unchanged (Principle VI). No new entities, no new Tauri commands, no on-disk migration.

For each evolution we state: field type, default, `#[serde(default)]` policy, legacy-record load behaviour, and a 1-test round-trip sketch.

The defaults follow the canonical pattern already in `crates/presto-ipc/src/settings.rs`:

```rust
#[serde(default = "default_weekly_goal")]
pub weekly_goal_minutes: u32,
...
/// Default weekly focus goal — 125 minutes per week.
#[must_use]
pub const fn default_weekly_goal() -> u32 {
    125
}
```

This is the precedent the four new fields mirror verbatim.

---

## Evolution 1 — `Session::title`

**File**: `crates/presto-ipc/src/session.rs`
**Bundle**: A
**Constitutional anchors**: II (titles never leave app-data), III (`Option<String>` over string sentinels), VI (wire-shape evolution gated by `#[serde(default)]`).

### Field

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Session {
    pub completed_pomodoros: u32,
    /// Seconds.
    pub total_focus_time: u32,
    pub current_session: u32,
    /// `%a %b %d %Y` (e.g., "Sat May 10 2026").
    pub date: String,
    /// User-typed session title (≤120 user-perceived chars).
    /// `None` for sessions created before this field existed (Bundle A,
    /// post-002), and for in-flight sessions that completed without a
    /// typed title. Empty-string is forbidden — normalised to `None` at
    /// the capture boundary per Principle III.
    #[serde(default)]
    pub title: Option<String>,
}
```

### Default

`None` (via `Option<String>`'s `Default` impl).

### `#[serde(default)]` policy

Bare `#[serde(default)]` — no custom default function needed; `Option<String>::default()` is `None`.

### Legacy-record load behaviour

Pre-bundle `history.json` records lack the `title` key. Serde sees the missing field, applies `Default`, fills `None`. The field is **not** silently written back on save — re-serialising a `None`-title record emits either `"title": null` or omits the key (depending on `serde_json` settings; both round-trip). Bundle A spec FR-007 forbids any one-shot migration that backfills `Some(...)` from inferred data; this is honoured by **not** adding any post-deserialise hook.

### Round-trip test sketch (RED-first)

```rust
#[test]
fn title_round_trip_some_none_legacy() {
    // Some — typed title round-trips byte-stable.
    let s1 = Session { completed_pomodoros: 3, total_focus_time: 4500, current_session: 4, date: "Sat May 10 2026".into(), title: Some("Spec 002 review".into()) };
    let json1 = serde_json::to_string(&s1).unwrap();
    let s1_back: Session = serde_json::from_str(&json1).unwrap();
    assert_eq!(s1_back.title.as_deref(), Some("Spec 002 review"));

    // None — round-trips as None (`"title": null` or omitted).
    let s2 = Session { title: None, ..s1.clone() };
    let s2_back: Session = serde_json::from_str(&serde_json::to_string(&s2).unwrap()).unwrap();
    assert!(s2_back.title.is_none());

    // Legacy — pre-bundle JSON without the key deserialises as None.
    let legacy = r#"{"completed_pomodoros":3,"total_focus_time":4500,"current_session":4,"date":"Sat May 10 2026"}"#;
    let s3: Session = serde_json::from_str(legacy).unwrap();
    assert!(s3.title.is_none());
}
```

---

## Evolution 2 — `ManualSession::title`

**File**: `crates/presto-ipc/src/session.rs`
**Bundle**: A
**Constitutional anchors**: II, III, VI (same as Evolution 1).

### Field

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ManualSession {
    pub id: String,
    pub session_type: SessionType,
    /// Minutes.
    pub duration: u32,
    /// `HH:MM`.
    pub start_time: String,
    /// `HH:MM`.
    pub end_time: String,
    pub notes: Option<String>,
    /// ISO-8601.
    pub created_at: String,
    /// `%a %b %d %Y`.
    pub date: String,
    pub tags: Option<Vec<serde_json::Value>>,
    /// User-typed session title (≤120 user-perceived chars). Captured at
    /// manual-backfill submit time. Same `None`-normalisation contract as
    /// `Session::title`.
    #[serde(default)]
    pub title: Option<String>,
}
```

### Default

`None`.

### `#[serde(default)]` policy

Bare `#[serde(default)]`. Distinct from `notes: Option<String>` (which has no `#[serde(default)]` because it predates the post-001 wire-shape posture) — kept aligned with the `Session::title` pattern.

### Legacy-record load behaviour

Pre-bundle `manual-sessions.json` records lack the `title` key. Serde applies `Default` → `None`. No backfill, no silent write-back of an inferred value (FR-007).

### Natural-completion population path

`ManualSession.title` is populated on **two** code paths, not just the manual-backfill submit:

1. **Manual-backfill submit** (calendar "Add session" modal) — the user explicitly types a title; the form passes it into the `ManualSession` constructor.
2. **Natural focus completion** — at focus zero-cross the `synth_completed_session` helper at `src/src/components/timer/mod.rs:213-230` (called from the engine-completion path around `mod.rs:980`) synthesises an in-memory `ManualSession` row so the calendar's `#sessions-table-body` reflects today's auto-saved sessions. Its signature gains `title: Option<String>`, sourced from the same Leptos title-input signal that populates `Session.title` at the same moment. Without this, naturally-completed focus sessions would render with an empty Title column even when the user did type a title.

Both paths normalise empty-string to `None` at the capture boundary (Principle III).

### Round-trip test sketch (RED-first)

Identical structure to Evolution 1's test, against `ManualSession` instead of `Session`. The legacy-shape fixture is a real-looking record from the JS era. Pair the two tests in a single `#[test] fn manual_session_title_round_trip_*` for brevity.

---

## Evolution 3 — `TimerSettings::sessions_per_long_break`

**File**: `crates/presto-ipc/src/settings.rs`
**Bundle**: B
**Constitutional anchors**: I (engine input), III (clamp at UI boundary, not engine), V (test-first for engine consumption), VI (wire-shape evolution).

### Field

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct TimerSettings {
    pub focus_duration: u32,
    pub break_duration: u32,
    pub long_break_duration: u32,
    pub total_sessions: u32,
    #[serde(default = "default_weekly_goal")]
    pub weekly_goal_minutes: u32,
    #[serde(default = "default_max_session_time")]
    pub max_session_time: u32,
    /// Number of focus completions per long-break cycle (1–10 enforced
    /// at the Settings UI input boundary). The engine reads this field
    /// as a configuration input alongside `Durations`; pre-bundle
    /// settings JSONs lacking the field default to `4` (the value
    /// previously hard-coded at `engine/timer.rs:396` and `:831`).
    #[serde(default = "default_sessions_per_long_break")]
    pub sessions_per_long_break: u32,
}
```

Add to the same file, next to `default_weekly_goal` / `default_max_session_time`:

```rust
/// Default sessions-per-long-break cadence — every 4th focus
/// completion enters long break (matches the pre-002 hard-coded
/// literal in `src/src/engine/timer.rs:396` and `:831`).
#[must_use]
pub const fn default_sessions_per_long_break() -> u32 {
    4
}
```

`Default for TimerSettings` adds the corresponding field initialiser: `sessions_per_long_break: default_sessions_per_long_break()`.

### Default

`4` (matches the previously hard-coded literal in the engine).

### `#[serde(default = "...")]` policy

Function-based, matching the canonical `default_weekly_goal` / `default_max_session_time` pattern. `#[must_use] pub const fn` is the literal style precedent.

### Legacy-record load behaviour

Pre-bundle `settings.json` records lack the `sessions_per_long_break` field in the `timer` object. Serde applies `default_sessions_per_long_break()` → `4`. Engine behaviour on the default-cadence path is bit-for-bit identical to pre-bundle (SC-006).

### Round-trip test sketch (RED-first)

```rust
#[test]
fn timer_settings_default_sessions_per_long_break_is_4() {
    let legacy = r#"{
        "focus_duration": 25,
        "break_duration": 5,
        "long_break_duration": 20,
        "total_sessions": 10
    }"#;
    let s: TimerSettings = serde_json::from_str(legacy).unwrap();
    assert_eq!(s.sessions_per_long_break, 4);
    assert_eq!(s.weekly_goal_minutes, 125);   // existing default unchanged
}
```

---

## Evolution 4 — `NotificationSettings::metronome` + `metronome_bpm`

**File**: `crates/presto-ipc/src/settings.rs`
**Bundle**: C
**Constitutional anchors**: I (UI-side, engine pure), II (no network), III (clamp at UI boundary), VI (wire-shape evolution).

### Fields

```rust
#[allow(clippy::struct_excessive_bools)]   // existing allowance covers the added bool
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
    /// When true, fire a metronome tick during focus sessions at the
    /// configured `metronome_bpm`. Default `false` (opt-in per
    /// Principle II). UI-side side effect only — engine is unaware.
    #[serde(default)]
    pub metronome: bool,
    /// Metronome tempo in beats per minute (30–180 enforced at the
    /// Settings UI input boundary). Read by `components/timer/mod.rs`
    /// only; the audio call site does not re-validate the range.
    #[serde(default = "default_metronome_bpm")]
    pub metronome_bpm: u32,
}
```

Add to the same file, next to the other default-function patterns:

```rust
/// Default metronome BPM — 60 (one tick per second, a comfortable
/// pacing default for focus work).
#[must_use]
pub const fn default_metronome_bpm() -> u32 {
    60
}
```

`Default for NotificationSettings` adds: `metronome: false, metronome_bpm: default_metronome_bpm()`.

### Defaults

`metronome = false`, `metronome_bpm = 60`.

### `#[serde(default)]` policy

`metronome` uses bare `#[serde(default)]` (a `bool` defaulting to `false` does not need a function). `metronome_bpm` uses the function-based `#[serde(default = "default_metronome_bpm")]` to make the default value declarative + greppable, matching the rest of the file.

### Legacy-record load behaviour

Pre-bundle `settings.json` records lack both fields. Serde applies `bool::default()` → `false` for `metronome`; `default_metronome_bpm()` → `60` for `metronome_bpm`. Pre-bundle users hear no change unless they opt in (SC-011).

### Round-trip test sketch (RED-first)

The fixture below includes every `NotificationSettings` field that lacks `#[serde(default)]` (`desktop_notifications`, `sound_notifications`, `auto_start_timer`, `smart_pause`, `smart_pause_timeout`). Fields that already have `#[serde(default)]` in the pre-002 shape (`auto_start_focus`, `allow_continuous_sessions`) are intentionally omitted so the assertions on the bottom prove their default behaviour is unaffected by the new field additions. The new `metronome` / `metronome_bpm` fields are also omitted so the fixture exercises the legacy-load path that this evolution introduces.

```rust
#[test]
fn notification_settings_default_metronome_is_off_at_60_bpm() {
    // Includes all fields lacking `#[serde(default)]`. Fields with
    // `#[serde(default)]` (auto_start_focus, allow_continuous_sessions,
    // metronome, metronome_bpm) are omitted on purpose to exercise the
    // legacy-load path.
    let legacy = r#"{
        "desktop_notifications": true,
        "sound_notifications": true,
        "auto_start_timer": true,
        "smart_pause": false,
        "smart_pause_timeout": 30
    }"#;
    let n: NotificationSettings = serde_json::from_str(legacy).unwrap();
    assert!(!n.metronome);
    assert_eq!(n.metronome_bpm, 60);
    // Existing post-001 defaults still apply.
    assert!(!n.auto_start_focus);
    assert!(!n.allow_continuous_sessions);
}
```

---

## Engine-side field (Bundle B, not an IPC type)

`TimerState` in `src/src/engine/timer.rs` gains a `sessions_per_long_break: u32` field. **The constructor signature is unchanged** — `pub fn new(durations: Durations) -> Self` at `engine/timer.rs:202` stays as-is; the new field is initialised to `4` in the existing struct-initialisation expression alongside the other defaulted fields (`total_sessions: 10`, `allow_continuous_sessions: false`, etc.). A new setter `pub const fn set_sessions_per_long_break(&mut self, n: u32)` mirrors the posture of `set_durations` at `engine/timer.rs:435` — assignment, no clamp inside the engine (the 1–10 clamp lives at the Settings UI input boundary, per Principle III).

The 22+ existing `TimerState::new(Durations::default())` call sites (app.rs, tray.rs ×4, session.rs ×4, `timer/mod.rs:453`, engine tests ×14) compile unchanged because the constructor arity does not change. The mechanical churn warned about in earlier draft revisions of this document no longer applies.

The default-4 at the field declaration is what keeps the existing `long_break_after_4_focus_sessions` test at `timer.rs:1267-1289` passing without modification: in any `TimerState` constructed via `TimerState::new(...)`, `sessions_per_long_break` starts at 4 just as the previous hard-coded literal at `:396` and `:831` produced.

The setter is invoked from the production boot path and from the Phase 4 Leptos `Effect::new` that mirrors the existing `set_durations` and `set_allow_continuous_sessions` effects (`src/src/components/timer/mod.rs:463-473`); the effect reads `settings.timer.sessions_per_long_break` and propagates it to the engine on settings change.

This is **not** an IPC type — it's an engine-state field whose value comes from `TimerSettings::sessions_per_long_break` at boot and on every settings save. The engine never deserialises settings JSON directly.

---

## Engine-side state for metronome (Bundle C) — explicitly NOT added

Per Principle I and FR-016: the engine has no metronome state. The metronome's scheduling lives in `src/src/components/timer/mod.rs` as a dedicated periodic timer created via `leptos::prelude::set_interval_with_handle` keyed at `60_000 / bpm` ms. The returned `IntervalHandle` is stored in a component-local `RefCell<Option<IntervalHandle>>`; `.clear()` cancels the interval. Lifecycle is driven by a Leptos `Effect::new` watching the gate signal (`metronome && current_mode == Focus && is_running && !is_paused && !is_auto_paused && time_remaining_secs > 0`): rising-edge creates the interval, falling-edge clears it, BPM change clears+recreates so the period reflects the new value. The engine does not emit a `MetronomeShouldTick` event; the timer component reads the engine's existing `current_mode` / `is_running` / `is_paused` / `is_auto_paused` / `time_remaining_secs` fields and gates the tick itself. **No engine evolution for Bundle C.**

---

## No-migration posture

Per CLAUDE.md "No upstream compatibility burden" + Principle VII: no `From<OldShape> for NewShape` projection is added. The `#[serde(default)]` mechanism is sufficient. Existing pre-bundle sessions stay `title = None` permanently; existing pre-bundle settings stay `sessions_per_long_break = 4` and `metronome = false` until the user changes them in Settings.

Contrast with the post-001 `SettingsOnDisk` → `Settings` shim for `hide_status_bar → status_bar_display`: that shim exists because the JS-era on-disk shape was different in kind (bool vs typed enum). Here the on-disk shape **adds** fields without changing existing ones, so the standard `#[serde(default)]` mechanism handles the legacy load with no projection needed.
