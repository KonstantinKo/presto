# Contract: Tauri bridge surface

**Phase**: 1 (Design & Contracts)
**Feeds**: [plan.md](../plan.md) §Modules, [data-model.md](../data-model.md)

This file enumerates every Tauri command currently registered in `src-tauri/src/lib.rs`'s `tauri::generate_handler![…]` block and the three new commands introduced by this migration. For each command we capture: argument shape, return shape (including error variant), whether it's currently called from JS today, and the post-migration Leptos-side wrapper signature.

The plan's contract is: **post-migration, no Tauri command may be added or modified without updating both sides in the same commit, AND `tests/e2e/fixtures/tauriMock.js` getting the corresponding mock entry in the same commit (or a prior commit, if test-first ordering applies — see §Mock-first rule below).**

---

## Existing commands (registered in `src-tauri/src/lib.rs:733`)

### Persistence — sessions

| # | Command | Args | Returns | Used by JS today? | Leptos wrapper |
|---|---|---|---|---|---|
| 1 | `save_session_data` | `session: PomodoroSession` | `Result<(), String>` | yes (timer engine) | `pub async fn save_session_data(session: Session) -> Result<(), BridgeError>` |
| 2 | `load_session_data` | _(none)_ | `Result<Option<PomodoroSession>, String>` | yes (startup) | `pub async fn load_session_data() -> Result<Option<Session>, BridgeError>` |
| 3 | `get_stats_history` | _(none)_ | `Result<Vec<PomodoroSession>, String>` | yes (history view) | `pub async fn get_stats_history() -> Result<Vec<Session>, BridgeError>` |
| 4 | `save_daily_stats` | `session: PomodoroSession` | `Result<(), String>` | yes (timer engine end-of-day) | `pub async fn save_daily_stats(session: Session) -> Result<(), BridgeError>` |

### Persistence — tasks

| # | Command | Args | Returns | Used by JS today? | Leptos wrapper |
|---|---|---|---|---|---|
| 5 | `save_tasks` | `tasks: Vec<Task>` | `Result<(), String>` | yes | `pub async fn save_tasks(tasks: Vec<Task>) -> Result<(), BridgeError>` |
| 6 | `load_tasks` | _(none)_ | `Result<Vec<Task>, String>` | yes | `pub async fn load_tasks() -> Result<Vec<Task>, BridgeError>` |

### Persistence — manual sessions

| # | Command | Args | Returns | Used by JS today? | Leptos wrapper |
|---|---|---|---|---|---|
| 7 | `save_manual_sessions` | `sessions: Vec<ManualSession>` | `Result<(), String>` | yes (bulk save) | `pub async fn save_manual_sessions(sessions: Vec<ManualSession>) -> Result<(), BridgeError>` |
| 8 | `load_manual_sessions` | _(none)_ | `Result<Vec<ManualSession>, String>` | yes | `pub async fn load_manual_sessions() -> Result<Vec<ManualSession>, BridgeError>` |
| 9 | `save_manual_session` | `session: ManualSession` | `Result<(), String>` | **no** — wired but not currently invoked from JS (today's JS calls `save_manual_sessions` for bulk) | `pub async fn save_manual_session(session: ManualSession) -> Result<(), BridgeError>` |
| 10 | `delete_manual_session` | `session_id: String` | `Result<(), String>` | yes | `pub async fn delete_manual_session(session_id: String) -> Result<(), BridgeError>` |
| 11 | `get_manual_sessions_for_date` | `date: String` | `Result<Vec<ManualSession>, String>` | **no** — wired but not currently invoked from JS (today's JS filters in-memory after `load_manual_sessions`) | `pub async fn get_manual_sessions_for_date(date: String) -> Result<Vec<ManualSession>, BridgeError>` |

### Persistence — tags

| # | Command | Args | Returns | Used by JS today? | Leptos wrapper |
|---|---|---|---|---|---|
| 12 | `load_tags` | _(none)_ | `Result<Vec<Tag>, String>` | yes | `pub async fn load_tags() -> Result<Vec<Tag>, BridgeError>` |
| 13 | `save_tags` | `tags: Vec<Tag>` | `Result<(), String>` | yes | `pub async fn save_tags(tags: Vec<Tag>) -> Result<(), BridgeError>` |
| 14 | `save_tag` | `tag: Tag` | `Result<(), String>` | yes | `pub async fn save_tag(tag: Tag) -> Result<(), BridgeError>` |
| 15 | `delete_tag` | `tag_id: String` | `Result<(), String>` | yes | `pub async fn delete_tag(tag_id: String) -> Result<(), BridgeError>` |
| 16 | `load_session_tags` | _(none)_ | `Result<Vec<SessionTag>, String>` | yes | `pub async fn load_session_tags() -> Result<Vec<SessionTag>, BridgeError>` |
| 17 | `save_session_tags` | `session_tags: Vec<SessionTag>` | `Result<(), String>` | yes | `pub async fn save_session_tags(session_tags: Vec<SessionTag>) -> Result<(), BridgeError>` |
| 18 | `add_session_tag` | `session_tag: SessionTag` | `Result<(), String>` | yes | `pub async fn add_session_tag(session_tag: SessionTag) -> Result<(), BridgeError>` |

### Settings & data lifecycle

| # | Command | Args | Returns | Used by JS today? | Leptos wrapper |
|---|---|---|---|---|---|
| 19 | `save_settings` | `settings: AppSettings` | `Result<(), String>` | yes | `pub async fn save_settings(settings: Settings) -> Result<(), BridgeError>` |
| 20 | `load_settings` | _(none)_ | `Result<AppSettings, String>` | yes | `pub async fn load_settings() -> Result<Settings, BridgeError>` |
| 21 | `reset_all_data` | _(none)_ | `Result<(), String>` | yes (advanced settings) | `pub async fn reset_all_data() -> Result<(), BridgeError>` |

### Global shortcuts

| # | Command | Args | Returns | Used by JS today? | Leptos wrapper |
|---|---|---|---|---|---|
| 22 | `register_global_shortcuts` | `shortcuts: ShortcutSettings` | `Result<(), String>` | yes (settings save) | `pub async fn register_global_shortcuts(shortcuts: ShortcutSettings) -> Result<(), BridgeError>` |
| 23 | `unregister_global_shortcuts` | _(none)_ | `Result<(), String>` | **no** — wired but not currently invoked from JS (today's JS overwrites by re-calling `register_global_shortcuts` with empty values) | `pub async fn unregister_global_shortcuts() -> Result<(), BridgeError>` |

### Activity monitoring

| # | Command | Args | Returns | Used by JS today? | Leptos wrapper |
|---|---|---|---|---|---|
| 24 | `start_activity_monitoring` | `timeout_seconds: u64` | `Result<(), String>` (macOS only — errors on other platforms) | yes (smart-pause init) | `pub async fn start_activity_monitoring(timeout_seconds: u64) -> Result<(), BridgeError>` |
| 25 | `stop_activity_monitoring` | _(none)_ | `Result<(), String>` | yes | `pub async fn stop_activity_monitoring() -> Result<(), BridgeError>` |
| 26 | `update_activity_timeout` | `timeout_seconds: u64` | `Result<(), String>` | yes (settings change) | `pub async fn update_activity_timeout(timeout_seconds: u64) -> Result<(), BridgeError>` |

### Autostart

| # | Command | Args | Returns | Used by JS today? | Leptos wrapper |
|---|---|---|---|---|---|
| 27 | `enable_autostart` | _(none)_ | `Result<(), String>` | yes | `pub async fn enable_autostart() -> Result<(), BridgeError>` |
| 28 | `disable_autostart` | _(none)_ | `Result<(), String>` | yes | `pub async fn disable_autostart() -> Result<(), BridgeError>` |
| 29 | `is_autostart_enabled` | _(none)_ | `Result<bool, String>` | yes | `pub async fn is_autostart_enabled() -> Result<bool, BridgeError>` |

### Window & tray

| # | Command | Args | Returns | Used by JS today? | Leptos wrapper |
|---|---|---|---|---|---|
| 30 | `update_tray_icon` | `timer_text: String, is_running: bool, session_mode: String, current_session: u32, total_sessions: u32, mode_icon: Option<String>` | `Result<(), String>` | yes | `pub async fn update_tray_icon(args: UpdateTrayIconArgs) -> Result<(), BridgeError>` |
| 31 | `update_tray_menu` | `is_running: bool, is_paused: bool, current_mode: String` | `Result<(), String>` | yes | `pub async fn update_tray_menu(is_running: bool, is_paused: bool, current_mode: TimerMode) -> Result<(), BridgeError>` |
| 32 | `show_window` | _(none)_ | `Result<(), String>` | yes | `pub async fn show_window() -> Result<(), BridgeError>` |
| 33 | `set_dock_visibility` | `visible: bool` | `Result<(), String>` (macOS only) | yes (`hide_icon_on_close` setting) | `pub async fn set_dock_visibility(visible: bool) -> Result<(), BridgeError>` |
| 34 | `set_status_bar_visibility` | `visible: bool` | `Result<(), String>` (macOS only) | yes (`hide_status_bar` setting) | `pub async fn set_status_bar_visibility(visible: bool) -> Result<(), BridgeError>` |

### Export

| # | Command | Args | Returns | Used by JS today? | Leptos wrapper |
|---|---|---|---|---|---|
| 35 | `write_excel_file` | `path: String, data: String (base64)` | `Result<(), String>` | yes (history export) — but **deprecated by the new `export_sessions_xlsx`**, see below. Kept during cutover for backward bridge surface; removed in Phase 6 cleanup. | `pub async fn write_excel_file(path: String, data: String) -> Result<(), BridgeError>` |

### OAuth

| # | Command | Args | Returns | Used by JS today? | Leptos wrapper |
|---|---|---|---|---|---|
| 36 | `start_oauth_server` | _(none)_ | `Result<u16, String>` (returns the port) | yes (Supabase OAuth flow) | `pub async fn start_oauth_server() -> Result<u16, BridgeError>` |

---

## New commands introduced by this migration

These three commands replace JS shims that disappear with the WASM swap. Each follows the **mock-first rule** below.

### `track_event` — replaces `@aptabase/tauri` JS shim

```rust
#[tauri::command]
async fn track_event(
    name: String,
    props: Option<HashMap<String, serde_json::Value>>,
    app: AppHandle,
) -> Result<(), String> {
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
async fn supabase_sign_in_with_password(email: String, password: String) -> Result<AuthSession, String>;

#[tauri::command]
async fn supabase_sign_out(refresh_token: String) -> Result<(), String>;

#[tauri::command]
async fn supabase_get_session() -> Result<Option<AuthSession>, String>;

#[tauri::command]
async fn supabase_refresh_session(refresh_token: String) -> Result<AuthSession, String>;
```

Implementation: a thin Rust module under `src-tauri/src/auth/` (or extending `helpers.rs`) using `tauri::http` (or `reqwest` if needed) to hit the Supabase REST endpoints `/auth/v1/token`, `/auth/v1/logout`, `/auth/v1/user`, `/auth/v1/token?grant_type=refresh_token`. Token storage moves Rust-side (today the JS SDK writes to localStorage; we move to the app-data dir for symmetry with sessions/tasks).

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
async fn export_sessions_xlsx(path: String, sessions: Vec<ManualSession>) -> Result<(), String>;
```

Implementation uses `rust_xlsxwriter` (write-only; lean). Builds the workbook server-side from the typed `ManualSession` list and writes to `path`. The pre-existing `write_excel_file` command is kept for cutover-period parity but unused by the post-cutover Leptos crate; it's removed in Phase 6.

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

The current Tauri command shape is `Result<T, String>` — error variant is a free-form string (e.g., `"Failed to write to disk: …"`). Tightening this to a typed `BridgeError` enum is **out of scope** for this feature per FR-019 / A2 (no backend behavioural changes beyond what the migration requires).

The Leptos wrappers define a local `BridgeError` that captures three categories:

```rust
#[derive(Debug, Clone)]
pub enum BridgeError {
    /// `window.__TAURI_INTERNALS__` is absent (per FR-009 / AGENTS.md §Bridge availability).
    BridgeUnavailable,
    /// The command returned a `String` error variant.
    CommandFailed { command: &'static str, message: String },
    /// `serde-wasm-bindgen` failed to deserialise the return.
    SerdeRoundtrip { command: &'static str, error: String },
}
```

Wrappers translate `Result<T, String>` from Tauri into `Result<T, BridgeError::CommandFailed>` on the Leptos side. Bridge-unavailability is detected up-front in each wrapper via the `BridgeAvailable` signal (per [data-model.md](../data-model.md) §`BridgeAvailable`); when absent, the wrapper short-circuits to a sentinel value (e.g., empty `Vec` for list-returning commands, `false` for `is_autostart_enabled`, no-op `Ok(())` for save/update commands).

---

## Mock-first rule (FR-010, Principle VI)

For **every** new Tauri command (current set of 36, plus the three new this feature, plus all future):

1. **Mock first**: extend `tests/e2e/fixtures/tauriMock.js` with a `case "<command_name>": …` entry returning a plausible default. **Commit this change first.**
2. **Test next**: add a failing `wasm-bindgen-test` (or Playwright e2e test) that exercises the bridge wrapper. **Commit this RED change.**
3. **Implementation last**: add the typed Leptos wrapper in `src/src/bridge/commands.rs` and the Rust handler in `src-tauri/src/lib.rs`. **Commit the GREEN change.**

The diff history must show the three commits in this order. Squash-merging is allowed for the cutover PR but the per-PR test-first audit (per AGENTS.md §Test-first commit ordering) requires the unsquashed sequence in the PR's commit log.

For commands already wired but not currently called from JS (rows 9, 11, 23 above marked "no — wired but not currently invoked"): the mock already covers their plausible defaults via the `default:` clause in `tauriMock.js:285-288` rejecting unmocked commands. Phase 1 of the implementation phasing extends the mock to cover them explicitly.
