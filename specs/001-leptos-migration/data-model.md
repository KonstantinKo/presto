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

**Scope**: shared. The Tauri side currently uses `String` (e.g., `"focus"`, `"break"`, `"longBreak"`) — see `update_tray_icon` in `src-tauri/src/lib.rs:432`. Cutover-period parity: the Leptos side serialises as the same camelCase strings via `#[serde(rename_all = "camelCase")]`. The Tauri side may follow up to use the enum locally, but is out of scope for this feature (per FR-019 / A2).

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
    pub date: String,           // "Mon Jan 01 2024" — current JS Date.toDateString() format
}
```

**Scope**: shared. Mirrors `PomodoroSession` at `src-tauri/src/lib.rs:39-45`. **Do not** redesign the schema (FR-005).

**Wire format**: snake_case to match the existing Rust serde derivation (no `rename_all` on the Tauri side).

---

### `ManualSession`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ManualSession {
    pub id: String,
    pub session_type: String,         // "focus" | "break" | "longBreak" | "custom"
    pub duration: u32,                // minutes
    pub start_time: String,           // "HH:MM"
    pub end_time: String,             // "HH:MM"
    pub notes: Option<String>,
    pub created_at: String,           // ISO string
    pub date: String,
    pub tags: Option<Vec<TagRef>>,    // tag identifiers attached to this session
}
```

**Scope**: shared. Mirrors `src-tauri/src/lib.rs:48-58`. Note `session_type` is a `String` today (open enum); a follow-up may tighten to a sum type, but per A2 this feature does not change the on-disk shape.

**`TagRef`**: a deliberately loose reference type (`{ id: String, name: String }`-ish) because the current JS stores tag objects inline, not ID-only. The Leptos side normalises at consumption time but does not reshape the on-disk record.

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

Mirrors the full nested shape from `src-tauri/src/lib.rs:90-202`. The Leptos side defines the same nested types (`ShortcutSettings`, `TimerSettings`, `NotificationSettings`, `AdvancedSettings`) with the same `#[serde(default = "...")]` markers so settings JSON files written by any released `0.4.x` build deserialise without manual migration (FR-005 idempotent migration path is "fall back to default for missing fields, write back on save").

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
    #[serde(default)]
    pub hide_status_bar: bool,
}
```

**Scope**: shared. Field defaults match `src-tauri/src/lib.rs:122-202`.

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

1. `Settings`: missing `serde(default)`-marked fields fall back to defaults; on next save, the file is rewritten with the full shape.
2. `Session`: shape is closed; no missing-field migration is anticipated.
3. `Task`, `Tag`, `SessionTag`, `ManualSession`: same.

Migration test coverage: the existing `app_settings_missing_serde_default_fields_use_defaults` test at `src-tauri/src/lib.rs:1241-1278` already covers the harshest case (an old settings JSON with several missing fields). The Leptos-side `managers/settings.rs` test mirrors this case for round-trip parity.

---

## Cross-reference

| Type | Defined where | Used by |
|---|---|---|
| `TimerMode` | `src/src/bridge/types.rs` (mirror in `src-tauri/`) | `engine/timer.rs`, `bridge/commands.rs` `update_tray_icon` |
| `Session` | `src/src/bridge/types.rs` | `bridge/commands.rs::save_session_data` etc. |
| `ManualSession` | `src/src/bridge/types.rs` | `managers/session.rs` |
| `Task` | `src/src/bridge/types.rs` | `managers/session.rs` (the JS today couples tasks to session manager) |
| `Tag`, `SessionTag` | `src/src/bridge/types.rs` | `managers/tag.rs`, `managers/session.rs` |
| `Settings` | `src/src/bridge/types.rs` | `managers/settings.rs` |
| `UpdateInfo` | `src/src/bridge/types.rs` | `managers/update.rs` |
| `OAuthCallback` | `src/src/bridge/types.rs` | `managers/auth.rs` (OAuth flow) |
| `AuthSession`, `AuthUser` | `src/src/bridge/types.rs` (auth namespace) | `managers/auth.rs` |
| `AuthState` | `src/src/managers/auth.rs` | `app.rs`, `components/auth_modal.rs` |
| `NavView`, `SettingsTab` | `src/src/managers/navigation.rs` | `app.rs`, `components/*` |
| `ActivitySignal` | `src/src/engine/activity_signal.rs` | `engine/timer.rs` |
| `TimerState` | `src/src/engine/timer.rs` | `app.rs`, `components/timer_view.rs` |
| `BridgeAvailable` | `src/src/bridge/availability.rs` | every `bridge/commands.rs` wrapper |
