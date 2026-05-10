# Data Model: Leptos Frontend Migration

**Phase**: 1 (Design & Contracts)
**Feeds**: [plan.md](./plan.md) §Modules, [contracts/tauri-bridge.md](./contracts/tauri-bridge.md)

This document enumerates the sum types and structs that travel across the Tauri bridge, plus the Leptos-only signals that compose them. Schemas mirror what's already on disk and on the wire today (per FR-005, the on-disk format does not change). The migration's job is to give them typed Rust homes.

For each entity we state: shape, scope (shared between Leptos & Tauri / Leptos-only / Tauri-only), and current JS representation (where the field set comes from).

---

## Shared types — bridge boundary (Leptos & `src-tauri/`)

These types must round-trip via `invoke()` payloads and event payloads. The Leptos-side definition lives in `src/src/bridge/commands.rs` (or a sibling `bridge/types.rs` — Phase 1 chooses); the Tauri-side definition stays in `src-tauri/src/lib.rs` where it already exists. Field names match exactly so `serde-wasm-bindgen` produces a compatible JSON shape.

### `TimerMode`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TimerMode {
    Focus,
    Break,
    LongBreak,
}
```

**Scope**: shared. The Tauri side currently uses `String` (e.g., `"focus"`, `"break"`, `"longBreak"`) — see `update_tray_icon` in `src-tauri/src/lib.rs §update_tray_icon` (section reference, not line number; future-proofs against drift). The cutover commit tightens both sides: `update_tray_menu`'s `current_mode` arg and `update_tray_icon`'s `session_mode` arg both become `TimerMode` (see [contracts/tauri-bridge.md](./contracts/tauri-bridge.md) §Error handling, "stringly-typed boundary args, tightened"). The wire format remains the same camelCase strings via `#[serde(rename_all = "camelCase")]`, so no on-disk or in-flight payload shapes change.

**Current JS**: `src/core/pomodoro-timer.js` uses string constants `"focus" | "break" | "longBreak"`.

---

### `Session` (per the existing `PomodoroSession` shape)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Session {
    pub completed_pomodoros: u32,
    pub total_focus_time: u32,  // seconds
    pub current_session: u32,
    pub date: String,           // chrono format `%a %b %d %Y` — exact-byte match for JS Date.toDateString()
}
```

**Scope**: shared. Mirrors `PomodoroSession` at `src-tauri/src/lib.rs:39-45`. **Do not** redesign the schema (FR-005).

**Wire format**: snake_case to match the existing Rust serde derivation (no `rename_all` on the Tauri side).

**`date` format pinning**: the chrono format string is `"%a %b %d %Y"`. JS `Date.prototype.toDateString()` per ECMA-262 produces a zero-padded day; chrono's `%d` is also zero-padded, so the formats currently match byte-for-byte. The Leptos crate ships a Rust unit test (`engine::date_format::tests::matches_js_to_date_string`) that iterates 366 dates and asserts `chrono_format(date) == known_js_format(date)` for a representative sample, pinning the format so a future chrono change that breaks parity fails loud at CI time rather than silently corrupting on-disk dates.

---

### `ManualSession`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ManualSession {
    pub id: String,
    pub session_type: SessionType,    // closed-domain sum type per FR-013
    pub duration: u32,                // minutes
    pub start_time: String,           // "HH:MM"
    pub end_time: String,             // "HH:MM"
    pub notes: Option<String>,
    pub created_at: String,           // ISO string
    pub date: String,
    pub tags: Option<Vec<TagRef>>,    // tag identifiers attached to this session
}
```

**Scope**: shared. Mirrors `src-tauri/src/lib.rs:48-58`. The Tauri-side `session_type: String` is tightened to `SessionType` in the cutover commit; the on-disk wire form is preserved exactly (camelCase strings) via `#[serde(rename_all = "camelCase")]` on the enum, satisfying both FR-013 (closed-domain sum types) and A2 (no on-disk shape change).

**`TagRef`**: a deliberately loose reference type (`{ id: String, name: String }`-ish) because the current JS stores tag objects inline, not ID-only. The Leptos side normalises at consumption time but does not reshape the on-disk record.

---

### `SessionType`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionType {
    Focus,
    Break,
    LongBreak,
    Custom,
}
```

**Scope**: shared. Closed-domain replacement for the `session_type: String` field on `ManualSession` (and any other manual-session record). Distinct from `TimerMode` because manual entries can carry the `"custom"` variant for user-defined session shapes (see the Tauri-side comment at `src-tauri/src/lib.rs:50` documenting `"focus" | "break" | "longBreak" | "custom"`); `TimerMode` is the live-engine domain and has only the three intrinsic modes.

**Wire format**: camelCase strings (`"focus"`, `"break"`, `"longBreak"`, `"custom"`) — the exact on-disk shape the JS era already writes. Pinned by `#[serde(rename_all = "camelCase")]` so the round-trip is byte-stable across the cutover.

**Current JS**: `src/managers/session-manager.js:277` and `src/core/pomodoro-timer.js:2345` both write `session_type: "focus"`; `src/managers/navigation-manager.js:314,958` reads `session.session_type || session.type`.

**Constitutional anchor**: III (Type Safety Over Defensive Code) — closed sum type; FR-013 closed-domain promise is now backed for manual sessions.

---

### `Task`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Task {
    pub id: u64,
    pub text: String,
    pub completed: bool,
    pub created_at: String,
    pub completed_at: Option<String>,
}
```

**Scope**: shared. Mirrors `src-tauri/src/lib.rs:78-84`.

---

### `Tag`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub icon: String,    // emoji or remix icon class (e.g., "ri-briefcase-line")
    pub color: String,   // hex color code (e.g., "#3b82f6")
    pub created_at: String,
}
```

**Scope**: shared. Mirrors `src-tauri/src/lib.rs:61-67`.

---

### `SessionTag`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SessionTag {
    pub session_id: String,
    pub tag_id: String,
    pub duration: u32,   // seconds spent on this tag in this session
    pub created_at: String,
}
```

**Scope**: shared. Mirrors `src-tauri/src/lib.rs:70-75`.

---

### `Settings` / `AppSettings`

Mirrors the full nested shape from `src-tauri/src/lib.rs:90-202`, with one shape change in this feature: the legacy `hide_status_bar: bool` field is replaced by a typed `status_bar_display: StatusBarDisplay` enum. The Leptos side defines the same nested types (`ShortcutSettings`, `TimerSettings`, `NotificationSettings`, `AdvancedSettings`) with the same `#[serde(default = "...")]` markers so settings JSON files written by any released `0.4.x` build deserialise without manual migration (FR-005 idempotent migration path: fall back to default for missing fields, write back on save).

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct Settings {
    pub shortcuts: ShortcutSettings,
    pub timer: TimerSettings,
    pub notifications: NotificationSettings,
    #[serde(default)]
    pub advanced: AdvancedSettings,
    pub autostart: bool,
    #[serde(default = "default_analytics_enabled")]
    pub analytics_enabled: bool,
    #[serde(default)]
    pub hide_icon_on_close: bool,
    #[serde(default, deserialize_with = "deserialize_status_bar_display_with_legacy_fallback")]
    pub status_bar_display: StatusBarDisplay,
    // NOTE: `hide_status_bar` is intentionally absent from this struct. It is consumed only by the
    // legacy fallback deserializer above; once a settings file is read with this struct and re-saved,
    // the on-disk shape carries `status_bar_display` and the legacy field is gone.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum StatusBarDisplay {
    #[default]
    Default,
    IconOnly,
}
```

**Scope**: shared. Field defaults match `src-tauri/src/lib.rs:122-202`.

#### Settings legacy migration

Existing on-disk JSON written by any released `0.4.x` build carries `hide_status_bar: bool` (and, in builds where the JS-side `status_bar_display` field was already written, the kebab-case string `"default"` or `"icon-only"`). The custom deserializer `deserialize_status_bar_display_with_legacy_fallback` reads the JSON object once and resolves `status_bar_display`:

1. If `status_bar_display` is present, use it. The on-disk wire form is kebab-case (`"default"` / `"icon-only"`), matched by the enum's `#[serde(rename_all = "kebab-case")]`.
2. Else if `hide_status_bar: true` is present, use `StatusBarDisplay::IconOnly` (emits `"icon-only"` on next save).
3. Else if `hide_status_bar: false` is present, use `StatusBarDisplay::Default` (emits `"default"` on next save).
4. Else, use `StatusBarDisplay::default()` (i.e., `Default`).

On the next save, the file is rewritten with `status_bar_display` only — `hide_status_bar` is not emitted (it has no field in the struct). This mirrors the JS-side migration logic at `src/managers/settings-manager.js:109-119` ported to Rust, and is exercised by `managers/settings::tests::migrates_hide_status_bar_to_status_bar_display`. The named test fixture literally contains `"status_bar_display": "icon-only"` from a pre-cutover JS-era settings JSON to assert the kebab-case wire form round-trips correctly.

---

### `UpdateInfo` (event payload from `tauri-plugin-updater`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateInfo {
    NoUpdate,
    Available { version: String, notes: Option<String> },
}
```

**Scope**: bridge boundary. Mirrors what the existing `plugin:updater|check` returns today (see `tauriMock.js:244-258` for the e2e shape).

---

### `OAuthCallback`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthCallback {
    pub url: String,
}
```

**Scope**: bridge event payload. Mirrors `src-tauri/src/lib.rs:1108-1113`.

---

### `Session` (Supabase auth session — distinct from pomodoro `Session`)

To avoid collision, this lives in the `bridge::auth` namespace as `AuthSession`.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSession {
    pub access_token: String,
    pub refresh_token: String,
    pub user: AuthUser,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub id: String,
    pub email: String,
    pub user_metadata: serde_json::Value,
}
```

**Scope**: shared. Replaces the JS Supabase SDK's session/user types. Persisted Rust-side per Decision §6 (research.md).

---

### `SupabaseSessionPayload` (transition-only — Supabase localStorage import)

The on-the-wire shape of the JS-era Supabase auth token persisted at `window.localStorage.getItem("sb-<project-ref>-auth-token")`. Deserialised by the Leptos crate on first post-cutover launch and handed to the Tauri-side adapter via `import_legacy_supabase_session(payload)` for re-persistence in the Rust-managed app-data store.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupabaseSessionPayload {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64,            // Unix epoch seconds, per supabase-js
    pub user: AuthUser,
}
```

**Scope**: bridge boundary, transition-only. Slated for removal one minor version after cutover, alongside the `import_legacy_supabase_session` command. Entry point: `bridge::storage::migrate_legacy_localstorage()` calls into `managers/auth.rs` for the Supabase slice.

---

### `BridgeError` (Tauri command error variant)

Every Tauri command returns `Result<T, BridgeError>`. This replaces today's untyped `Result<T, String>` and makes spec FR-008's compile-time-mismatch promise load-bearing — a `String` error channel cannot deliver structured error discrimination across the bridge.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BridgeError {
    /// `window.__TAURI_INTERNALS__` is absent (per FR-009 / AGENTS.md §Bridge availability). Leptos-side only — never produced by the Tauri handler.
    #[error("bridge unavailable")]
    BridgeUnavailable,
    /// The caller is in a state where this command is invalid (e.g., a Supabase command without an active session).
    #[error("not authenticated")]
    NotAuthenticated,
    /// An argument failed validation at the boundary.
    #[error("invalid argument {field}: {reason}")]
    InvalidArgument { field: String, reason: String },
    /// The requested resource (file, key, row) does not exist.
    #[error("not found: {resource}")]
    NotFound { resource: String },
    /// `serde-wasm-bindgen` failed to deserialise the return on the Leptos side.
    #[error("serde roundtrip failed in {command}: {error}")]
    SerdeRoundtrip { command: String, error: String },
    /// Catch-all for unexpected Tauri-side failures (filesystem errors, plugin errors, etc.).
    #[error("internal: {msg}")]
    Internal { msg: String },
}
```

**Scope**: shared. The Leptos crate mirrors the same enum (or imports a shared crate-level type if a cross-workspace `bridge-types` crate is added; Phase 1 chooses). Wire shape is externally-tagged JSON: `{"kind":"invalid_argument","field":"email","reason":"empty"}`.

**Mapping strategy** (Tauri-side, applied in the cutover commit): every existing `.map_err(|e| e.to_string())` call site in `src-tauri/src/lib.rs` is rewritten to map into a `BridgeError` variant — defaulting to `BridgeError::Internal { msg: e.to_string() }` where the call site has no semantic context, and to `NotFound` / `InvalidArgument` / `NotAuthenticated` where it does. Coverage: a `bridge::error::tests::*` suite exercises every variant's serde round-trip.

**Constitutional anchor**: III (Type Safety Over Defensive Code) — closed sum type; FR-008 compile-time-mismatch promise is now backed.

---

## Leptos-only types — frontend signals

These don't cross the bridge. They live in `src/src/managers/*` and `src/src/engine/`.

### `AuthState` — in `managers/auth.rs`

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum AuthState {
    Unauthenticated,
    Guest,
    SignedIn { user: AuthUser },
}
```

**Initial state**: read on app start; `Guest` if `presto-guest-mode == "true"` in localStorage; `SignedIn` if `bridge::commands::supabase_get_session` returns a session; `Unauthenticated` otherwise.

**Transitions**:
- `Unauthenticated → SignedIn` on `sign_in_with_password` success.
- `Unauthenticated → Guest` on user "continue as guest" action (writes `presto-guest-mode = "true"`).
- `SignedIn → Unauthenticated` on `sign_out`.
- `Guest → SignedIn` on `sign_in_with_password` success (clears `presto-guest-mode`).

**Constitutional anchor**: II — guest mode is first-class.

---

### `NavView` — in `managers/navigation.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavView {
    Timer,
    Tasks,
    History,
    Calendar,
    Tags,
    Team,
    Settings(SettingsTab),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    Shortcuts,
    Notifications,
    Automation,
    Advanced,
    Goals,
    Theme,
    Updates,
}
```

**Initial state**: `NavView::Timer`. **Transitions**: any `NavView::X → NavView::Y` is allowed (router-style; no transition gating).

---

### `ActivitySignal` — in `engine/activity_signal.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivitySignal {
    Idle { since_ms: u64 },
    Active { at_ms: u64 },
}
```

**Source**: a fold over (a) Tauri events `user-activity` / `user-inactivity` (Rust-side detected on macOS), and (b) DOM events `mousemove`, `keydown`, `visibilitychange` (web-sys-listened in the bridge layer). Engine consumes `ActivitySignal` only — never the raw events. This is what FR-002 mandates.

**Reduction rules**:
- Any raw activity event → emit `Active { at_ms: now }`.
- N seconds of no activity (where N = `settings.notifications.smart_pause_timeout`) → emit `Idle { since_ms: now - N*1000 }`.
- Edge-detected: idle→active and active→idle transitions only; mid-state events are folded.

**Constitutional anchor**: I — engine is pure; signal is normalised at the boundary.

---

### `TimerState` — in `engine/timer.rs`

```rust
#[derive(Debug, Clone)]
pub struct TimerState {
    pub mode: TimerMode,
    pub current_session: u32,
    pub total_sessions: u32,
    pub elapsed_ms: u64,
    pub paused: bool,
    pub started_at_ms: Option<u64>,   // wall-clock anchor for drift comp
    pub completed_pomodoros: u32,
    pub total_focus_time_s: u32,
}
```

**Transitions**: full set lifted from `src/core/pomodoro-timer.js`. RED-first tests per behaviour (Phase 2).

**Constitutional anchor**: I — pure state machine.

---

### `BridgeAvailable` — in `bridge/availability.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeAvailable {
    Available,
    Absent,
}
```

**Source**: one-time read of `window.__TAURI_INTERNALS__` at app start. **Consumers**: every `invoke()` wrapper checks this signal; when `Absent`, it short-circuits to a sentinel return value (e.g., `load_tags` returns the default tag list; `save_*` no-ops; `is_autostart_enabled` returns `false`). This preserves AGENTS.md §Bridge availability and FR-009.

---

## On-disk format invariants

Per FR-005, the on-disk shape of every persisted record is unchanged. The Tauri-side helpers in `src-tauri/src/helpers.rs` already produce and consume the JSON shapes shown above. The Leptos side serialises with the same field names + serde defaults, so a settings JSON written by any `0.4.x` build round-trips cleanly:

1. `Settings`: missing `serde(default)`-marked fields fall back to defaults; on next save, the file is rewritten with the full shape. The `hide_status_bar` legacy field migrates to `status_bar_display` per §"Settings legacy migration" above.
2. `Session`: shape is closed; no missing-field migration is anticipated.
3. `Task`, `Tag`, `SessionTag`, `ManualSession`: same.

Migration test coverage: the existing `app_settings_missing_serde_default_fields_use_defaults` test at `src-tauri/src/lib.rs:1241-1278` already covers the harshest case (an old settings JSON with several missing fields). The Leptos-side `managers/settings.rs` test mirrors this case for round-trip parity, plus the named test `migrates_hide_status_bar_to_status_bar_display`.

---

## Legacy localStorage migration

The JS era persists 14 keys in `window.localStorage` (enumerated at `src/config/storage-keys.js:1-19`). The cutover migrates the preserved subset to the Rust-side authoritative stores via a single one-shot entry point `bridge::storage::migrate_legacy_localstorage()`, called once on first post-cutover launch from `app.rs` startup. The entry point reads each preserved key via `web-sys::window().local_storage()`, parses the legacy shape, hands the payload to the matching `import_legacy_*` Tauri command (see [contracts/tauri-bridge.md](./contracts/tauri-bridge.md) §"Transition-only commands"), and clears the localStorage key on successful import. Idempotent: if the corresponding Rust-side store already has data, the import is skipped (and the localStorage key is still cleared best-effort).

Per-key disposition:

| Key | Class | Disposition |
|---|---|---|
| `presto-guest-mode` | user-state flag | Preserve. Migrated via `import_legacy_user_state` into the Rust-side user-state slice of `AppSettings`. |
| `presto-auth-seen` | user-state flag | Preserve. Same. |
| `theme-preference` | user preference | Preserve. Migrated via `import_legacy_settings` into `AppSettings`. |
| `timer-theme-preference` | user preference | Preserve. Same. |
| `presto-skipped-versions` | user-state | Preserve (used by the updater logic). Migrated via `import_legacy_user_state`. |
| `presto_auto_check_updates` | user preference | Preserve. Migrated via `import_legacy_settings`. |
| `pomodoro-session` | active session snapshot | Preserve (cross-launch resume). Migrated via `import_legacy_user_state`. |
| `pomodoro-tasks` | active task list | Preserve. Migrated via `import_legacy_tasks`. |
| `pomodoro-settings` | full settings JSON | Preserve. Merged with the Rust-side `AppSettings` via `import_legacy_settings`; the `hide_status_bar` → `status_bar_display` resolution from §"Settings legacy migration" carries over here. |
| `pomodoro-history` | session history | Preserve. Migrated via `import_legacy_history`. |
| `presto-tags` | tags | Preserve. Migrated via `import_legacy_tags`. |
| `presto_manual_sessions` | manual session entries | Preserve. Migrated via `import_legacy_manual_sessions`. |
| `pomodoro-stats` | vestigial accumulator | Drop. Only ever cleared on reset by `src/main.js:247`; never read or written elsewhere. |
| `presto_force_update_test` | test-only flag | Drop. No production code path. |

Sunset: the migration entry point and all `import_legacy_*` commands are slated for removal one minor version after cutover. Coverage: per-import wasm-bindgen-test exercises the migration with a mocked localStorage and asserts the matching Tauri command receives the expected payload (see plan.md §Testing strategy and test-first markers).

---

## Cross-reference

| Type | Defined where | Used by |
|---|---|---|
| `TimerMode` | `src/src/bridge/types.rs` (mirror in `src-tauri/`) | `engine/timer.rs`, `bridge/commands.rs` `update_tray_icon`, `update_tray_menu` |
| `SessionType` | `src/src/bridge/types.rs` (mirror in `src-tauri/`) | `bridge/types.rs` `ManualSession.session_type`; `managers/session.rs` |
| `Session` | `src/src/bridge/types.rs` | `bridge/commands.rs::save_session_data` etc. |
| `ManualSession` | `src/src/bridge/types.rs` | `managers/session.rs` |
| `Task` | `src/src/bridge/types.rs` | `managers/session.rs` (the JS today couples tasks to session manager) |
| `Tag`, `SessionTag` | `src/src/bridge/types.rs` | `managers/tag.rs`, `managers/session.rs` |
| `Settings`, `StatusBarDisplay` | `src/src/bridge/types.rs` | `managers/settings.rs` |
| `UpdateInfo` | `src/src/bridge/types.rs` | `managers/update.rs` |
| `OAuthCallback` | `src/src/bridge/types.rs` | `managers/auth.rs` (OAuth flow) |
| `AuthSession`, `AuthUser` | `src/src/bridge/types.rs` (auth namespace) | `managers/auth.rs` |
| `SupabaseSessionPayload` | `src/src/bridge/types.rs` (auth namespace; transition-only) | `managers/auth.rs` (one-shot localStorage migration entry point) |
| `BridgeError` | `src/src/bridge/types.rs` (mirror in `src-tauri/`) | every `bridge/commands.rs` wrapper, every `#[tauri::command]` handler |
| `AuthState` | `src/src/managers/auth.rs` | `app.rs`, `components/auth_modal.rs` |
| `NavView`, `SettingsTab` | `src/src/managers/navigation.rs` | `app.rs`, `components/*` |
| `ActivitySignal` | `src/src/engine/activity_signal.rs` | `engine/timer.rs` |
| `TimerState` | `src/src/engine/timer.rs` | `app.rs`, `components/timer_view.rs` |
| `BridgeAvailable` | `src/src/bridge/availability.rs` | every `bridge/commands.rs` wrapper |
