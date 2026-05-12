# Contract: Wire-shape evolutions

**Phase**: 1 (Design & Contracts)
**Feeds**: [../plan.md](../plan.md) §Modules, [../data-model.md](../data-model.md)

This file enumerates the JSON wire-shape changes for feature 002. Style mirrors `specs/001-leptos-migration/contracts/tauri-bridge.md`. Every modified struct gets a before/after JSON example; consumers (`save_session_data`, `save_manual_sessions`, `save_settings`) are unchanged at the command-signature level.

## No new Tauri commands

**Explicit statement (load-bearing for the mock-drift gate)**: feature 002 introduces **zero** new Tauri commands. All persistence flows through commands that already exist in `src-tauri/src/lib.rs`'s `tauri::generate_handler![…]` block:

- `save_session_data(session: PomodoroSession)` — accepts the evolved `Session` shape via `#[serde(default)]` on the new `title` field. No handler change needed.
- `load_session_data()` → `Option<PomodoroSession>` — returns the evolved shape; legacy on-disk records load with `title = None`.
- `save_manual_sessions(sessions: Vec<ManualSession>)` — accepts the evolved `ManualSession` shape via `#[serde(default)]` on `title`.
- `load_manual_sessions()` → `Vec<ManualSession>` — returns the evolved shape.
- `save_settings(settings: AppSettings)` — accepts the evolved `TimerSettings` and `NotificationSettings` shapes via `#[serde(default = "...")]` on the three new fields.
- `load_settings()` → `AppSettings` — returns the evolved shape; legacy on-disk records load with the configured defaults.

The Tauri-side handler bodies are unchanged. The `presto-ipc` crate's struct definitions are the source of truth for both sides (post-001 single-source posture).

The mock-drift gate at `scripts/check-mock-drift.sh` compares the set of `#[tauri::command]` handlers against the set of `case "<name>":` branches in `tests/e2e/fixtures/tauriMock.js`. **No change to either side in this feature → gate stays green.** If a future bundle splits and adds a command (e.g. a hypothetical `update_session_title(session_id, title)`), that command lands on the mock first per FR-010, then the test, then the handler — outside this feature's scope.

---

## Modified struct 1 — `Session`

**Crate**: `presto-ipc`
**File**: `crates/presto-ipc/src/session.rs`
**Wire shape**: `snake_case` (matches existing serde derivation; no `rename_all` attribute on the struct).

### Before

```json
{
  "completed_pomodoros": 3,
  "total_focus_time": 4500,
  "current_session": 4,
  "date": "Sat May 10 2026"
}
```

### After (Some-title case)

```json
{
  "completed_pomodoros": 3,
  "total_focus_time": 4500,
  "current_session": 4,
  "date": "Sat May 10 2026",
  "title": "Spec 002 review"
}
```

### After (None-title case)

```json
{
  "completed_pomodoros": 3,
  "total_focus_time": 4500,
  "current_session": 4,
  "date": "Sat May 10 2026",
  "title": null
}
```

`null` and omitted key are both valid on the wire and both deserialise to `None` (per `#[serde(default)]` on `Option<String>`).

### Legacy load behaviour

The "Before" example (no `title` key) loads as `Session { …, title: None }` under the new build. No migration write-back; the field stays absent on disk until the user creates a new session with a typed title.

### Round-trip test reference

`crates/presto-ipc/src/session.rs` `tests::title_round_trip_some_none_legacy` — see [data-model.md §Evolution 1](../data-model.md#evolution-1--sessiontitle).

---

## Modified struct 2 — `ManualSession`

**Crate**: `presto-ipc`
**File**: `crates/presto-ipc/src/session.rs`
**Wire shape**: `snake_case`.

### Before

```json
{
  "id": "ms-001",
  "session_type": "focus",
  "duration": 25,
  "start_time": "09:00",
  "end_time": "09:25",
  "notes": null,
  "created_at": "2026-05-10T09:00:00Z",
  "date": "Sat May 10 2026",
  "tags": null
}
```

### After (Some-title case)

```json
{
  "id": "ms-001",
  "session_type": "focus",
  "duration": 25,
  "start_time": "09:00",
  "end_time": "09:25",
  "notes": null,
  "created_at": "2026-05-10T09:00:00Z",
  "date": "Sat May 10 2026",
  "tags": null,
  "title": "Catch-up — Spec 002"
}
```

### Legacy load behaviour

The "Before" example loads as `ManualSession { …, title: None }`. Calendar Title column for this row falls back to joined tag names (or an empty string if `tags` is also `None` — Phase 3 task generation picks the in-row fallback for the doubly-absent case; spec FR-006 says "joined tag names"; the three-tier chain in plan §Phase 3 + tasks T020 resolves the doubly-absent case (`title: None` AND `tags: None`/empty) to a non-breaking space `&nbsp;` so the row keeps its visual line height. No string sentinel like `(untitled)` is rendered).

### Round-trip test reference

`crates/presto-ipc/src/session.rs` `tests::manual_session_title_round_trip_*` — same structure as Evolution 1's test.

---

## Modified struct 3 — `TimerSettings`

**Crate**: `presto-ipc`
**File**: `crates/presto-ipc/src/settings.rs`
**Wire shape**: `snake_case`.

### Before

```json
{
  "focus_duration": 25,
  "break_duration": 5,
  "long_break_duration": 20,
  "total_sessions": 10,
  "weekly_goal_minutes": 125,
  "max_session_time": 120
}
```

### After

```json
{
  "focus_duration": 25,
  "break_duration": 5,
  "long_break_duration": 20,
  "total_sessions": 10,
  "weekly_goal_minutes": 125,
  "max_session_time": 120,
  "sessions_per_long_break": 4
}
```

### Legacy load behaviour

The "Before" example (no `sessions_per_long_break` key) loads as `TimerSettings { …, sessions_per_long_break: 4 }` via the new `default_sessions_per_long_break()` const fn. Engine behaviour on the default cadence is bit-for-bit identical to pre-bundle (SC-006).

### Round-trip test reference

`crates/presto-ipc/src/settings.rs` `tests::timer_settings_default_sessions_per_long_break_is_4` — see [data-model.md §Evolution 3](../data-model.md#evolution-3--timersettingssessions_per_long_break).

---

## Modified struct 4 — `NotificationSettings`

**Crate**: `presto-ipc`
**File**: `crates/presto-ipc/src/settings.rs`
**Wire shape**: `snake_case`.

### Before

```json
{
  "desktop_notifications": true,
  "sound_notifications": true,
  "auto_start_timer": true,
  "auto_start_focus": false,
  "allow_continuous_sessions": false,
  "smart_pause": false,
  "smart_pause_timeout": 30
}
```

### After

```json
{
  "desktop_notifications": true,
  "sound_notifications": true,
  "auto_start_timer": true,
  "auto_start_focus": false,
  "allow_continuous_sessions": false,
  "smart_pause": false,
  "smart_pause_timeout": 30,
  "metronome": false,
  "metronome_bpm": 60
}
```

### Legacy load behaviour

The "Before" example (no `metronome`, no `metronome_bpm`) loads as `NotificationSettings { …, metronome: false, metronome_bpm: 60 }`. Pre-bundle users hear no change unless they explicitly opt in (SC-011).

### Round-trip test reference

`crates/presto-ipc/src/settings.rs` `tests::notification_settings_default_metronome_is_off_at_60_bpm` — see [data-model.md §Evolution 4](../data-model.md#evolution-4--notificationsettingsmetronome--metronome_bpm).

---

## Combined `AppSettings` load example

The full `settings.json` round-trip exercise (Phase 0 test). A pre-bundle file lacks **all three** of `sessions_per_long_break`, `metronome`, `metronome_bpm`. After load, the in-memory `Settings` has those defaults filled. A subsequent `save_settings` re-emits the JSON **with** the new fields (no longer absent). Pre-bundle builds reading the post-save file would themselves apply `#[serde(default)]` for any field they don't recognise — clean forward/backward round-trip per Principle VI.

---

## Mock-first ordering rule — N/A this feature

Per FR-010 and Principle VI: adding any Tauri command means (1) extend the mock first, (2) add the failing test, (3) land the real handler. **Feature 002 introduces no new Tauri commands**, so this ordering rule does not apply. `tests/e2e/fixtures/tauriMock.js` is unchanged.
