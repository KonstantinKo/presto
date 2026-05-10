# Contract: Tauri bridge surface

**Phase**: 1 (Design & Contracts)
**Feeds**: [plan.md](../plan.md) §Modules, [data-model.md](../data-model.md)

This file enumerates the Tauri commands surviving in `src-tauri/src/lib.rs`'s `tauri::generate_handler![…]` block post-cutover (25 of today's 36) and the new commands introduced by this migration. For each command we capture: argument shape, return shape (typed `BridgeError` enum), whether it's currently called from JS today, and the post-migration Leptos-side wrapper signature.

The plan's contract is: **post-migration, no Tauri command may be added or modified without updating both sides in the same commit, AND `tests/e2e/fixtures/tauriMock.js` getting the corresponding mock entry in the same commit (or a prior commit, if test-first ordering applies — see §Mock-first rule below).**

The 10 commands deleted in the cutover commit are listed under §Deletions; they do not appear in the surviving table because they have zero JS call sites today (Principle VII — no upstream burden for unused surface).

---

## Surviving commands (25 of today's 36; registered in `src-tauri/src/lib.rs:733`; renumbered post-deletion)

### Persistence — sessions

| # | Command | Args | Returns | Leptos wrapper |
|---|---|---|---|---|
| 1 | `save_session_data` | `session: PomodoroSession` | `Result<(), BridgeError>` | `pub async fn save_session_data(session: Session) -> Result<(), BridgeError>` |
| 2 | `load_session_data` | _(none)_ | `Result<Option<PomodoroSession>, BridgeError>` | `pub async fn load_session_data() -> Result<Option<Session>, BridgeError>` |
| 3 | `get_stats_history` | _(none)_ | `Result<Vec<PomodoroSession>, BridgeError>` | `pub async fn get_stats_history() -> Result<Vec<Session>, BridgeError>` |
| 4 | `save_daily_stats` | `session: PomodoroSession` | `Result<(), BridgeError>` | `pub async fn save_daily_stats(session: Session) -> Result<(), BridgeError>` |

### Persistence — tasks

| # | Command | Args | Returns | Leptos wrapper |
|---|---|---|---|---|
| 5 | `save_tasks` | `tasks: Vec<Task>` | `Result<(), BridgeError>` | `pub async fn save_tasks(tasks: Vec<Task>) -> Result<(), BridgeError>` |
| 6 | `load_tasks` | _(none)_ | `Result<Vec<Task>, BridgeError>` | `pub async fn load_tasks() -> Result<Vec<Task>, BridgeError>` |

### Persistence — manual sessions

| # | Command | Args | Returns | Leptos wrapper |
|---|---|---|---|---|
| 7 | `save_manual_sessions` | `sessions: Vec<ManualSession>` | `Result<(), BridgeError>` | `pub async fn save_manual_sessions(sessions: Vec<ManualSession>) -> Result<(), BridgeError>` |
| 8 | `load_manual_sessions` | _(none)_ | `Result<Vec<ManualSession>, BridgeError>` | `pub async fn load_manual_sessions() -> Result<Vec<ManualSession>, BridgeError>` |

### Persistence — tags

| # | Command | Args | Returns | Leptos wrapper |
|---|---|---|---|---|
| 9 | `load_tags` | _(none)_ | `Result<Vec<Tag>, BridgeError>` | `pub async fn load_tags() -> Result<Vec<Tag>, BridgeError>` |
| 10 | `save_tag` | `tag: Tag` | `Result<(), BridgeError>` | `pub async fn save_tag(tag: Tag) -> Result<(), BridgeError>` |
| 11 | `delete_tag` | `tag_id: String` | `Result<(), BridgeError>` | `pub async fn delete_tag(tag_id: String) -> Result<(), BridgeError>` |
| 12 | `add_session_tag` | `session_tag: SessionTag` | `Result<(), BridgeError>` | `pub async fn add_session_tag(session_tag: SessionTag) -> Result<(), BridgeError>` |

### Settings & data lifecycle

| # | Command | Args | Returns | Leptos wrapper |
|---|---|---|---|---|
| 13 | `save_settings` | `settings: AppSettings` | `Result<(), BridgeError>` | `pub async fn save_settings(settings: Settings) -> Result<(), BridgeError>` |
| 14 | `load_settings` | _(none)_ | `Result<AppSettings, BridgeError>` | `pub async fn load_settings() -> Result<Settings, BridgeError>` |
| 15 | `reset_all_data` | _(none)_ | `Result<(), BridgeError>` | `pub async fn reset_all_data() -> Result<(), BridgeError>` |

### Global shortcuts

| # | Command | Args | Returns | Leptos wrapper |
|---|---|---|---|---|
| 16 | `register_global_shortcuts` | `shortcuts: ShortcutSettings` | `Result<(), BridgeError>` | `pub async fn register_global_shortcuts(shortcuts: ShortcutSettings) -> Result<(), BridgeError>` |

### Activity monitoring

| # | Command | Args | Returns | Leptos wrapper |
|---|---|---|---|---|
| 17 | `start_activity_monitoring` | `timeout_seconds: u64` | `Result<(), BridgeError>` (macOS only — errors on other platforms) | `pub async fn start_activity_monitoring(timeout_seconds: u64) -> Result<(), BridgeError>` |
| 18 | `stop_activity_monitoring` | _(none)_ | `Result<(), BridgeError>` | `pub async fn stop_activity_monitoring() -> Result<(), BridgeError>` |
| 19 | `update_activity_timeout` | `timeout_seconds: u64` | `Result<(), BridgeError>` | `pub async fn update_activity_timeout(timeout_seconds: u64) -> Result<(), BridgeError>` |

### Autostart

| # | Command | Args | Returns | Leptos wrapper |
|---|---|---|---|---|
| 20 | `enable_autostart` | _(none)_ | `Result<(), BridgeError>` | `pub async fn enable_autostart() -> Result<(), BridgeError>` |
| 21 | `disable_autostart` | _(none)_ | `Result<(), BridgeError>` | `pub async fn disable_autostart() -> Result<(), BridgeError>` |
| 22 | `is_autostart_enabled` | _(none)_ | `Result<bool, BridgeError>` | `pub async fn is_autostart_enabled() -> Result<bool, BridgeError>` |

### Window & tray

| # | Command | Args | Returns | Leptos wrapper |
|---|---|---|---|---|
| 23 | `update_tray_icon` | `timer_text: String, is_running: bool, session_mode: TimerMode, current_session: u32, total_sessions: u32, mode_icon: Option<String>` | `Result<(), BridgeError>` | `pub async fn update_tray_icon(args: UpdateTrayIconArgs) -> Result<(), BridgeError>` |
| 24 | `update_tray_menu` | `is_running: bool, is_paused: bool, current_mode: TimerMode` | `Result<(), BridgeError>` | `pub async fn update_tray_menu(is_running: bool, is_paused: bool, current_mode: TimerMode) -> Result<(), BridgeError>` |

### Export

`write_excel_file` has been deprecated and removed (per T235) in favour of `export_sessions_xlsx` (see §New permanent commands). The surviving command count reflects this removal.

### OAuth

| # | Command | Args | Returns | Leptos wrapper |
|---|---|---|---|---|
| 25 | `start_oauth_server` | _(none)_ | `Result<u16, BridgeError>` (returns the port) | `pub async fn start_oauth_server() -> Result<u16, BridgeError>` |

### Deletions (cutover commit)

The following 10 commands have **zero JS call sites** today and are deleted from `src-tauri/src/lib.rs` (and any helpers in `helpers.rs` left dead by their removal). Principle VII rationale: there is no obligation to maintain features the JS layer doesn't exercise; each one would otherwise require a Leptos wrapper, a mock entry, and a RED-first test for surface that nothing uses.

| Command | One-line rationale |
|---|---|
| `save_manual_session` | Never called from JS; today's JS calls `save_manual_sessions` for bulk writes. |
| `delete_manual_session` | Never called from JS; deletion currently goes through bulk re-save with the entry omitted. |
| `get_manual_sessions_for_date` | Never called from JS; today's JS filters in-memory after `load_manual_sessions`. |
| `save_tags` | Never called from JS; today's JS uses `save_tag` per-row. |
| `load_session_tags` | Never called from JS; session-tag joins are derived client-side. |
| `save_session_tags` | Never called from JS; bulk path unused — today's JS uses `add_session_tag` per-row. |
| `unregister_global_shortcuts` | Never called from JS; today's JS overwrites by re-calling `register_global_shortcuts` with empty values. |
| `show_window` | Never called from JS; window-show happens via tray menu / OS focus events. |
| `set_dock_visibility` | Never called from JS; macOS-only and superseded by the unified `status_bar_display` setting (see data-model.md §Settings legacy migration). |
| `set_status_bar_visibility` | Never called from JS; macOS-only and superseded by `status_bar_display`. |

---

## New permanent commands introduced by this migration

These six commands replace JS shims that disappear with the WASM swap. Each follows the **mock-first rule** below.

### `track_event` — replaces `@aptabase/tauri` JS shim

```rust
#[tauri::command]
async fn track_event(
    name: String,
    props: Option<HashMap<String, serde_json::Value>>,
    app: AppHandle,
) -> Result<(), BridgeError> {
    if are_analytics_enabled(&app) {
        let _ = app.track_event(&name, props);
    }
    Ok(())
}
```

**Leptos wrapper**:
```rust
pub async fn track_event(name: &str, props: Option<HashMap<String, serde_json::Value>>) -> Result<(), BridgeError>;
```

**Constitutional anchor**: II — opt-in checked Rust-side at call site; never bypassed.

**Mock entry** (added before implementation):
```js
case "track_event":
  return; // no-op in e2e
```

---

### Supabase auth adapter family — replaces `supabase-js`

Four commands, all under the `supabase_*` prefix:

```rust
#[tauri::command]
async fn supabase_sign_in_with_password(email: String, password: String) -> Result<AuthSession, BridgeError>;

#[tauri::command]
async fn supabase_sign_out(refresh_token: String) -> Result<(), BridgeError>;

#[tauri::command]
async fn supabase_get_session() -> Result<Option<AuthSession>, BridgeError>;

#[tauri::command]
async fn supabase_refresh_session(refresh_token: String) -> Result<AuthSession, BridgeError>;
```

Implementation: a thin Rust module under `src-tauri/src/auth/` (or extending `helpers.rs`) using `tauri::http` (or `reqwest` if needed) to hit the Supabase REST endpoints `/auth/v1/token`, `/auth/v1/logout`, `/auth/v1/user`, `/auth/v1/token?grant_type=refresh_token`. Token storage moves Rust-side (today the JS SDK writes to localStorage; we move to the app-data dir for symmetry with sessions/tasks). The one-shot import path that bridges the JS-era localStorage token to the new Rust-side store is `import_legacy_supabase_session` — see §Transition-only commands below.

**Leptos wrapper**: one fn per command, all returning `Result<…, BridgeError>`.

**Constitutional anchor**: II — guest mode unaffected; auth is opt-in. VI — narrow surface (4 commands).

**Mock entries** (added before implementation):
```js
case "supabase_sign_in_with_password":
  return { access_token: "mock", refresh_token: "mock", user: { id: "mock", email: args.email, user_metadata: {} } };
case "supabase_sign_out":
  return;
case "supabase_get_session":
  return null;
case "supabase_refresh_session":
  return { access_token: "mock-refreshed", refresh_token: "mock-refreshed", user: { id: "mock", email: "test@example.com", user_metadata: {} } };
```

---

### `export_sessions_xlsx` — replaces JS `xlsx` library

```rust
#[tauri::command]
async fn export_sessions_xlsx(path: String, sessions: Vec<ManualSession>) -> Result<(), BridgeError>;
```

Implementation uses `rust_xlsxwriter` (write-only; lean). Builds the workbook server-side from the typed `ManualSession` list and writes to `path`. The legacy `write_excel_file` command was removed (per T235); `export_sessions_xlsx` is the sole export handler.

**Leptos wrapper**:
```rust
pub async fn export_sessions_xlsx(path: String, sessions: Vec<ManualSession>) -> Result<(), BridgeError>;
```

**Constitutional anchor**: VI — single new command. VIII — change documented here.

**Mock entry**:
```js
case "export_sessions_xlsx":
  return; // no-op; the test asserts the call shape, not the file content
```

---

## Transition-only commands (one-shot localStorage migration)

These commands are introduced by the cutover to migrate the JS era's `window.localStorage` payloads into the Rust-side authoritative stores on the first post-cutover launch. They are **slated for removal one minor version after cutover**; once that minor ships, the migration code is dead-on-arrival and gets deleted in a follow-up cleanup. Each command is idempotent: if the corresponding Rust-side store already has data for a given key, the import is skipped (and the localStorage entry is cleared best-effort).

### `import_legacy_supabase_session`

```rust
#[tauri::command]
async fn import_legacy_supabase_session(payload: SupabaseSessionPayload) -> Result<(), BridgeError>;
```

Reads the JS-era token JSON shape (`access_token`, `refresh_token`, `expires_at`, `user`) — see [data-model.md](../data-model.md) `SupabaseSessionPayload`. The adapter validates the payload and persists it to the app-data directory using the same shape that `supabase_get_session` returns. Idempotent: short-circuits with `Ok(())` if a Rust-side session already exists.

### Per-domain `import_legacy_*` commands

One command per preserved localStorage key class (see [data-model.md](../data-model.md) §"Legacy localStorage migration"):

| Command | Source localStorage keys | Rust-side store |
|---|---|---|
| `import_legacy_settings` | `pomodoro-settings`, `theme-preference`, `timer-theme-preference`, `presto_auto_check_updates` | `AppSettings` JSON |
| `import_legacy_history` | `pomodoro-history` | `get_stats_history` store |
| `import_legacy_tasks` | `pomodoro-tasks` | `load_tasks` store |
| `import_legacy_tags` | `presto-tags` | `load_tags` store |
| `import_legacy_manual_sessions` | `presto_manual_sessions` | `load_manual_sessions` store |
| `import_legacy_user_state` | `presto-guest-mode`, `presto-auth-seen`, `presto-skipped-versions`, `pomodoro-session` | `AppSettings` (user-state slice) |

Each takes a domain-specific payload shape (mirrors the legacy JS schema) and returns `Result<(), BridgeError>`. All are idempotent. The single Leptos-side entry point `bridge::storage::migrate_legacy_localstorage()` runs on first post-cutover launch, reads each key via `web-sys::window().local_storage()`, hands the parsed payload to the matching `import_legacy_*` command, and clears the key on success.

Mock entries (Phase 1):
```js
case "import_legacy_supabase_session":
case "import_legacy_settings":
case "import_legacy_history":
case "import_legacy_tasks":
case "import_legacy_tags":
case "import_legacy_manual_sessions":
case "import_legacy_user_state":
  return; // idempotent ack
```

**Constitutional anchor**: VII — these are not "upstream compatibility burden"; they are a one-shot migration with a defined sunset (one minor version), not an indefinite parallel surface.

---

## Tauri events (subscribed via `listen()`)

Events the Leptos crate consumes. Each gets a typed wrapper in `src/src/bridge/events.rs`.

| # | Event name | Payload | Emitted by | Leptos consumer |
|---|---|---|---|---|
| E1 | `user-activity` | `()` | `ActivityMonitor` (macOS) `lib.rs:257` | `engine/activity_signal.rs` |
| E2 | `user-inactivity` | `()` | `ActivityMonitor` (macOS) `lib.rs:269` | `engine/activity_signal.rs` |
| E3 | `global-shortcut` | `&str` (action name: `"start-stop"`, `"reset"`, `"skip"`) | `register_global_shortcuts` `lib.rs:567` | `app.rs` (dispatches into `engine/timer.rs`) |
| E4 | `shortcuts-updated` | `ShortcutSettings` | `register_global_shortcuts` `lib.rs:575` | `managers/settings.rs` |
| E5 | `oauth-callback` | `String` (callback URL) | `start_oauth_server` `lib.rs:1110` | `managers/auth.rs` |
| E6 | `tray-start-session` | `()` | tray menu `lib.rs:817` | `engine/timer.rs` |
| E7 | `tray-pause` | `()` | tray menu `lib.rs:826` | `engine/timer.rs` |
| E8 | `tray-skip` | `()` | tray menu `lib.rs:835` | `engine/timer.rs` |
| E9 | `tray-cancel` | `()` | tray menu `lib.rs:844` | `engine/timer.rs` |
| E10 | (updater plugin events: `tauri://update-available`, `tauri://update-installed`, …) | per `tauri-plugin-updater` schema | the plugin | `managers/update.rs` |

---

## Error handling

Every command in the surviving table and every new command introduced by this feature returns `Result<T, BridgeError>` where `BridgeError` is a serde-tagged Rust enum defined in `src-tauri/src/lib.rs` and mirrored in the Leptos crate. Spec FR-008's compile-time-mismatch promise is the load-bearing rationale: a `String` error channel cannot deliver it. Variants:

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
    SerdeRoundtrip { command: &'static str, error: String },
    /// Catch-all for unexpected Tauri-side failures (filesystem errors, plugin errors, etc.).
    #[error("internal: {msg}")]
    Internal { msg: String },
}
```

**Serde shape**: externally-tagged (`#[serde(tag = "kind")]`) — chosen for the simplest cross-language wire shape: every error JSON object carries a `kind` discriminator and the variant fields alongside (e.g., `{"kind":"invalid_argument","field":"email","reason":"empty"}`). This lets `serde-wasm-bindgen` produce structured error objects the Leptos side can pattern-match on without parsing strings.

**Mapping strategy** (Tauri-side, applied in the cutover commit): every existing `.map_err(|e| e.to_string())` call site in `src-tauri/src/lib.rs` is rewritten to map into a `BridgeError` variant. The default (when the call site has no semantic context) is `BridgeError::Internal { msg: e.to_string() }`. Where the call site already discriminates (e.g., a missing-row branch vs. a write failure), it maps to `NotFound` / `Internal` as appropriate. Where an existing handler already validates an argument and returns a `String` error, the rewrite uses `InvalidArgument`. The migration is mechanical, file-by-file, and covered by a `bridge::error::tests::*` suite in the Leptos crate exercising every variant's serde round-trip.

**Bridge-unavailability** is detected up-front in each wrapper via the `BridgeAvailable` signal (per [data-model.md](../data-model.md) §`BridgeAvailable`); when absent, the wrapper short-circuits to a sentinel value (e.g., empty `Vec` for list-returning commands, `false` for `is_autostart_enabled`, no-op `Ok(())` for save/update commands) — a `BridgeUnavailable` error variant is reserved for the few call sites that must surface the absence explicitly.

**Stringly-typed boundary args, tightened**: `update_tray_menu`'s `current_mode: String` becomes `current_mode: TimerMode` (mirrors data-model.md's `TimerMode` enum). `update_tray_icon`'s `session_mode` likewise tightens to `TimerMode`. No other surviving command has a stringly-typed enum-shaped argument that would otherwise create asymmetric wrapper types.

---

## Mock-first rule (FR-010, Principle VI)

For **every** new Tauri command (every survivor in the table above, plus every new permanent or transition-only command introduced by this feature, plus all future additions):

1. **Mock first**: extend `tests/e2e/fixtures/tauriMock.js` with a `case "<command_name>": …` entry returning a plausible default. **Commit this change first.**
2. **Test next**: add a failing `wasm-bindgen-test` (or Playwright e2e test) that exercises the bridge wrapper. **Commit this RED change.**
3. **Implementation last**: add the typed Leptos wrapper in `src/src/bridge/commands.rs` and the Rust handler in `src-tauri/src/lib.rs`. **Commit the GREEN change.**

The diff history must show the three commits in this order. Squash-merging is allowed for the cutover PR but the per-PR test-first audit (per AGENTS.md §Test-first commit ordering) requires the unsquashed sequence in the PR's commit log.

### Phase 0.5 — mock/handler reconciliation (precedes Phase 1)

The current `default:` clause in `tauriMock.js:285-288` **rejects** unmocked commands — it does not cover them. The mock and the handler set in `src-tauri/src/lib.rs` have drifted: 17 handler-registered commands have no mock entry, and 4 mock entries have no corresponding handler. Of the 17 missing-mock commands, 10 are deleted by this feature (see §Deletions) and don't need mocks. The remaining 8 do, and the 4 stale entries get removed.

Phase 0.5 of the implementation phasing reconciles the mock to today's surviving handler set **before** Phase 1 introduces any new commands. Concretely:

**Add 8 mock entries**: `get_stats_history`, `reset_all_data`, `save_daily_stats`, `start_activity_monitoring`, `stop_activity_monitoring`, `update_activity_timeout`, `update_tray_icon`, `update_tray_menu`.

**Remove 4 stale mock-only entries**: `append_daily_stats`, `delete_all_data`, `load_history`, `open_url` — none correspond to real handlers.

Only after Phase 0.5 lands does Phase 1 add the new commands (`track_event`, `supabase_*`, `export_sessions_xlsx`, the transition-only `import_legacy_*` family) behind the mock-first rule above.
