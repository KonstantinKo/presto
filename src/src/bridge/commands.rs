// Typed wrappers for every surviving Tauri command.
//
// Spec 001-leptos-migration §Phase 1C T032-T083; contracts/tauri-bridge.md
// §"Surviving commands". One wrapper per command; the wrapper enforces
// FR-008's compile-time-mismatch promise (a Leptos call site whose
// argument or return type drifts from the Rust handler IS a compile
// error) and the FR-009 short-circuit: every wrapper checks
// `bridge_available()` and returns `BridgeError::BridgeUnavailable` when
// the Tauri JS bridge is not present.
//
// Commands are grouped by domain (sessions, tasks, manual sessions, tags,
// settings, …) in the order of contracts/tauri-bridge.md. Tests sit in
// the `tests` submodule below; each command has at least one
// `wasm-bindgen-test` covering the bridge-absent short-circuit, and a
// signature-pinning compile-time assertion.
//
// Lint allowance: `clippy::future_not_send` is allowed at the module level
// because the bridge runs exclusively on `wasm32-unknown-unknown`, where
// the runtime is single-threaded and `JsValue` (plus everything
// transitively built on it: `JsFuture`, `Promise`, `serde-wasm-bindgen`
// values) is `!Send` by construction. Demanding `Send` here would force
// every wrapper to invent a Send-erasure shim that does nothing on the
// WASM target. Spec 001 plan.md §Modules makes the same call; no
// non-WASM consumer of this module exists.
#![allow(
    clippy::future_not_send,
    reason = "Tauri JS bridge futures carry JsValue and run only on single-threaded wasm32."
)]

use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use super::availability::bridge_available;
use super::types::BridgeError;
use super::types::TimerMode;
use super::types::{
    AddSessionTagArgs, DeleteTagArgs, Distraction, ManualSession, QuickLog, Session, SessionTag,
    Settings, ShortcutSettings, StartActivityMonitoringArgs, Tag, Task, UpdateActivityTimeoutArgs,
    UpdateTrayIconArgs, UpdateTrayMenuArgs,
};

#[wasm_bindgen]
extern "C" {
    /// Tauri 2.x JS bridge entry point. Bound to
    /// `window.__TAURI_INTERNALS__.invoke(cmd, args)`. Callers MUST
    /// short-circuit on `bridge_available().is_absent()` before invoking
    /// — the binding panics in environments where the global is missing
    /// (the `__TAURI_INTERNALS__` object is created by the Tauri webview
    /// bootstrap; it does not exist in node, in the Trunk dev server, or
    /// in the e2e mock harness).
    #[wasm_bindgen(
        js_namespace = ["__TAURI_INTERNALS__"],
        js_name = invoke,
        catch
    )]
    fn tauri_invoke(cmd: &str, args: JsValue) -> Result<js_sys::Promise, JsValue>;
}

/// Generic invoke helper. Performs the FR-009 bridge-availability
/// short-circuit, then serialises the typed argument bag, calls
/// `window.__TAURI_INTERNALS__.invoke`, awaits the resulting `Promise`,
/// and deserialises the typed return.
///
/// The helper is intentionally `async fn` rather than a hand-written
/// `impl Future` so call sites compose with the rest of the Leptos async
/// surface (every wrapper is `async fn ... -> Result<R, BridgeError>`).
async fn invoke_serde<A, R>(cmd: &'static str, args: &A) -> Result<R, BridgeError>
where
    A: Serialize + ?Sized,
    R: DeserializeOwned,
{
    if bridge_available().is_absent() {
        return Err(BridgeError::BridgeUnavailable);
    }
    let js_args = serde_wasm_bindgen::to_value(args).map_err(|e| BridgeError::SerdeRoundtrip {
        command: cmd.to_string(),
        error: format!("serialise args: {e}"),
    })?;
    let promise = tauri_invoke(cmd, js_args).map_err(|e| BridgeError::Internal {
        msg: format!("invoke('{cmd}') failed at the bridge boundary: {e:?}"),
    })?;
    let resolved = JsFuture::from(promise)
        .await
        .map_err(|e| map_promise_rejection(cmd, &e))?;
    serde_wasm_bindgen::from_value(resolved).map_err(|e| BridgeError::SerdeRoundtrip {
        command: cmd.to_string(),
        error: format!("deserialise return: {e}"),
    })
}

/// Invoke a command whose argument bag carries a single named key.
///
/// Collapses the boilerplate `Args` struct that ~15 wrappers shared
/// (`#[derive(Serialize)] struct Args { x }` then
/// `invoke_serde(cmd, &Args { x })`). The `key` is the Tauri-side
/// handler's parameter name.
///
/// Single-word keys (e.g. `"session"`, `"tag"`, `"settings"`) are
/// camelCase-stable, so this helper does NOT apply `rename_all`. For
/// multi-word keys (`tag_id`, `refresh_token`, etc.) use a dedicated
/// `Args` struct from `presto-ipc::args` instead — those need
/// camelCase wire renaming and benefit from being part of the shared
/// IPC contract.
async fn invoke_named_arg<V, R>(
    cmd: &'static str,
    key: &'static str,
    value: V,
) -> Result<R, BridgeError>
where
    V: Serialize,
    R: DeserializeOwned,
{
    let map = std::collections::BTreeMap::from([(key, value)]);
    invoke_serde(cmd, &map).await
}

/// Translate a rejected Tauri-side `Promise` into a `BridgeError`. The
/// Tauri runtime wraps Rust-side `Err(BridgeError)` returns as the
/// rejected value; if it deserialises cleanly we keep the structured
/// variant, otherwise we fall back to `Internal` with the raw string.
fn map_promise_rejection(cmd: &'static str, raw: &JsValue) -> BridgeError {
    if let Ok(typed) = serde_wasm_bindgen::from_value::<BridgeError>(raw.clone()) {
        return typed;
    }
    if let Some(s) = raw.as_string() {
        return BridgeError::Internal {
            msg: format!("invoke('{cmd}') rejected: {s}"),
        };
    }
    BridgeError::Internal {
        msg: format!("invoke('{cmd}') rejected with non-string value"),
    }
}

// ---------------------------------------------------------------------------
// Persistence — sessions
// ---------------------------------------------------------------------------

/// Persist the live pomodoro session to disk. Tauri-side handler:
/// `save_session_data(session: PomodoroSession) -> Result<(), BridgeError>`
/// at `src-tauri/src/lib.rs:462`.
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present (Trunk dev server, e2e mock, node tests). Otherwise returns
/// whatever variant the Tauri-side handler maps its filesystem failure to
/// (typically `BridgeError::Internal`).
pub async fn save_session_data(session: Session) -> Result<(), BridgeError> {
    invoke_named_arg("save_session_data", "session", session).await
}

/// Read the persisted live session from disk. Tauri-side handler:
/// `load_session_data() -> Result<Option<PomodoroSession>, BridgeError>`
/// at `src-tauri/src/lib.rs:483`.
///
/// `Option<Session>` is the load-bearing shape — the handler returns
/// `None` for the cold-start "no session yet" case rather than surfacing
/// it as `BridgeError::NotFound`.
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever variant the Tauri-side handler
/// maps its filesystem failure to (typically `BridgeError::Internal`).
pub async fn load_session_data() -> Result<Option<Session>, BridgeError> {
    invoke_serde("load_session_data", &serde_json::Value::Null).await
}

/// Read the persisted full session history. Tauri-side handler:
/// `get_stats_history() -> Result<Vec<PomodoroSession>, BridgeError>`
/// at `src-tauri/src/lib.rs:517`.
///
/// Returns an empty `Vec` if no history file exists yet (the Tauri-side
/// helper at `helpers::read_history_from` treats `NotFound` as empty —
/// a cold-start convention, not an error).
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever variant the Tauri-side handler
/// maps its filesystem failure to (typically `BridgeError::Internal`).
pub async fn get_stats_history() -> Result<Vec<Session>, BridgeError> {
    invoke_serde("get_stats_history", &serde_json::Value::Null).await
}

/// Append a completed session to the on-disk daily-stats file. Tauri-side
/// handler: `save_daily_stats(session: PomodoroSession) -> Result<(), BridgeError>`
/// at `src-tauri/src/lib.rs:526`.
///
/// Distinct from `save_session_data`, which overwrites the *live* session
/// file (a single-record snapshot of the in-progress timer). This command
/// appends to the daily-stats file (a session-by-session log used by the
/// stats / history view).
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever variant the Tauri-side handler
/// maps its filesystem failure to (typically `BridgeError::Internal`).
pub async fn save_daily_stats(session: Session) -> Result<(), BridgeError> {
    invoke_named_arg("save_daily_stats", "session", session).await
}

// ---------------------------------------------------------------------------
// Persistence — tasks
// ---------------------------------------------------------------------------

/// Persist the user's task list to disk. Tauri-side handler:
/// `save_tasks(tasks: Vec<Task>) -> Result<(), BridgeError>`
/// at `src-tauri/src/lib.rs:492`.
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever variant the Tauri-side handler
/// maps its filesystem failure to (typically `BridgeError::Internal`).
pub async fn save_tasks(tasks: Vec<Task>) -> Result<(), BridgeError> {
    invoke_named_arg("save_tasks", "tasks", tasks).await
}

/// Read the persisted task list. Tauri-side handler:
/// `load_tasks() -> Result<Vec<Task>, BridgeError>`
/// at `src-tauri/src/lib.rs:508`.
///
/// Returns an empty `Vec` if no tasks file exists yet (the Tauri-side
/// helper at `helpers::read_tasks_from` treats `NotFound` as empty —
/// a cold-start convention, not an error).
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever variant the Tauri-side handler
/// maps its filesystem failure to (typically `BridgeError::Internal`).
pub async fn load_tasks() -> Result<Vec<Task>, BridgeError> {
    invoke_serde("load_tasks", &serde_json::Value::Null).await
}

// ---------------------------------------------------------------------------
// Persistence — manual sessions
// ---------------------------------------------------------------------------

/// Persist the user's manual-session entries to disk. Tauri-side handler:
/// `save_manual_sessions(sessions: Vec<ManualSession>) -> Result<(), BridgeError>`
/// at `src-tauri/src/lib.rs:736`.
///
/// The closed-domain `SessionType` enum is enforced at the wrapper
/// boundary (Phase 1A T029) — a stringly-typed `session_type` value
/// would not compile here. Wire form is preserved exactly per FR-005:
/// `SessionType` serialises as the existing camelCase strings
/// (`"focus"` / `"break"` / `"longBreak"` / `"custom"`).
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever variant the Tauri-side handler
/// maps its filesystem failure to (typically `BridgeError::Internal`).
pub async fn save_manual_sessions(sessions: Vec<ManualSession>) -> Result<(), BridgeError> {
    invoke_named_arg("save_manual_sessions", "sessions", sessions).await
}

/// Append a single completed session. Tauri-side handler:
/// `append_manual_session(session: ManualSession) -> Result<(), BridgeError>`.
///
/// More efficient than `save_manual_sessions` for the timer-completion path —
/// reads the current list, pushes one entry, and writes back, rather than
/// bulk-rewriting a list that grows by one per pomodoro. The Tauri side caps
/// the list at 1,000 entries to bound file growth.
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever variant the Tauri-side handler
/// maps its filesystem failure to (typically `BridgeError::Internal`).
pub async fn append_manual_session(session: ManualSession) -> Result<(), BridgeError> {
    invoke_named_arg("append_manual_session", "session", session).await
}

/// Read the persisted manual-session entries. Tauri-side handler:
/// `load_manual_sessions() -> Result<Vec<ManualSession>, BridgeError>`
/// at `src-tauri/src/lib.rs:755`.
///
/// Returns an empty `Vec` if no manual-sessions file exists yet (the
/// Tauri-side helper at `helpers::read_manual_sessions_from` treats
/// `NotFound` as empty — a cold-start convention, not an error).
///
/// The closed-domain `SessionType` enum is enforced on the deserialise
/// side: a wire shape carrying `session_type: "<unknown variant>"`
/// surfaces as `BridgeError::SerdeRoundtrip` rather than being silently
/// dropped (FR-013 closed-domain promise).
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever variant the Tauri-side handler
/// maps its filesystem failure to (typically `BridgeError::Internal`),
/// or `BridgeError::SerdeRoundtrip` if a stored record carries an
/// unknown `session_type` variant.
pub async fn load_manual_sessions() -> Result<Vec<ManualSession>, BridgeError> {
    invoke_serde("load_manual_sessions", &serde_json::Value::Null).await
}

// ---------------------------------------------------------------------------
// Persistence — quick logs & distractions (feature 006)
//
// Wire-shape stubs land here in Phase 1. The Tauri-side handlers,
// validation, and JSON IO helpers arrive in Phase 4 (T039-T040). The
// shape mirrors `save_manual_sessions` / `load_manual_sessions`
// exactly — bulk re-save per mutation, empty-vec on missing file.
// ---------------------------------------------------------------------------

/// Persist the user's quick-log entries to disk. Tauri-side handler:
/// `save_quick_logs(quickLogs: Vec<QuickLog>) -> Result<(), BridgeError>`
/// (Phase 4, T040).
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever variant the Tauri-side
/// handler maps boundary-validation or IO failure to (typically
/// `BridgeError::InvalidArgument` or `BridgeError::Internal`).
pub async fn save_quick_logs(quick_logs: Vec<QuickLog>) -> Result<(), BridgeError> {
    invoke_named_arg("save_quick_logs", "quickLogs", quick_logs).await
}

/// Read the persisted quick-log entries. Tauri-side handler:
/// `load_quick_logs() -> Result<Vec<QuickLog>, BridgeError>` (Phase 4,
/// T040). Missing file ⇒ empty vec (cold-start convention).
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever variant the Tauri-side
/// handler maps its filesystem failure to (typically
/// `BridgeError::Internal`).
pub async fn load_quick_logs() -> Result<Vec<QuickLog>, BridgeError> {
    invoke_serde("load_quick_logs", &serde_json::Value::Null).await
}

/// Persist the user's distraction entries to disk. Tauri-side handler:
/// `save_distractions(distractions: Vec<Distraction>) -> Result<(), BridgeError>`
/// (Phase 4, T040).
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever variant the Tauri-side
/// handler maps boundary-validation or IO failure to (typically
/// `BridgeError::InvalidArgument` or `BridgeError::Internal`).
pub async fn save_distractions(distractions: Vec<Distraction>) -> Result<(), BridgeError> {
    invoke_named_arg("save_distractions", "distractions", distractions).await
}

/// Read the persisted distraction entries. Tauri-side handler:
/// `load_distractions() -> Result<Vec<Distraction>, BridgeError>`
/// (Phase 4, T040). Missing file ⇒ empty vec (cold-start convention).
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever variant the Tauri-side
/// handler maps its filesystem failure to (typically
/// `BridgeError::Internal`).
pub async fn load_distractions() -> Result<Vec<Distraction>, BridgeError> {
    invoke_serde("load_distractions", &serde_json::Value::Null).await
}

// ---------------------------------------------------------------------------
// Persistence — tags
// ---------------------------------------------------------------------------

/// Read the persisted tag list. Tauri-side handler:
/// `load_tags() -> Result<Vec<Tag>, BridgeError>`
/// at `src-tauri/src/lib.rs:1059`.
///
/// Returns an empty `Vec` if no tags file exists yet (the Tauri-side
/// helper at `helpers::read_tags_from` treats `NotFound` as empty —
/// same cold-start convention as `load_tasks` / `load_manual_sessions`).
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever variant the Tauri-side handler
/// maps its filesystem failure to (typically `BridgeError::Internal`).
pub async fn load_tags() -> Result<Vec<Tag>, BridgeError> {
    invoke_serde("load_tags", &serde_json::Value::Null).await
}

/// Persist the full tag list in a single atomic write. Tauri-side handler:
/// `save_tags_bulk(tags: Vec<Tag>) -> Result<(), BridgeError>`.
///
/// Replaces the per-tag `save_tag` loop in the tags Effect with a single
/// IPC call — N calls → 1 call per mutation.
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever variant the Tauri-side handler
/// maps its filesystem failure to (typically `BridgeError::Internal`).
pub async fn save_tags_bulk(tags: Vec<Tag>) -> Result<(), BridgeError> {
    invoke_named_arg("save_tags_bulk", "tags", tags).await
}

/// Persist (insert or update) a single tag. Tauri-side handler:
/// `save_tag(tag: Tag) -> Result<(), BridgeError>`
/// at `src-tauri/src/lib.rs:1077`.
///
/// Distinct from the deleted bulk `save_tags(Vec<Tag>)` command — the JS
/// era writes tags one at a time via this upsert path (see
/// contracts/tauri-bridge.md §Deletions for the rationale on dropping
/// the bulk variant).
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever variant the Tauri-side handler
/// maps its filesystem failure to (typically `BridgeError::Internal`).
pub async fn save_tag(tag: Tag) -> Result<(), BridgeError> {
    invoke_named_arg("save_tag", "tag", tag).await
}

/// Delete a tag by its id. Tauri-side handler:
/// `delete_tag(tag_id: String) -> Result<(), BridgeError>`
/// at `src-tauri/src/lib.rs:1086`.
///
/// The argument is a bare `String` (not a `Tag` reference) because the
/// Tauri-side handler matches by `id` only — a wire shape that carries
/// the full record would force a redundant round-trip.
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever variant the Tauri-side handler
/// maps its filesystem failure to (typically `BridgeError::Internal`).
pub async fn delete_tag(tag_id: String) -> Result<(), BridgeError> {
    invoke_serde("delete_tag", &DeleteTagArgs { tag_id }).await
}

/// Append a session-tag join row recording time spent on `tag_id` during
/// `session_id`.
///
/// Tauri-side handler:
/// `add_session_tag(session_tag: SessionTag) -> Result<(), BridgeError>`
/// at `src-tauri/src/lib.rs:1113`.
///
/// One row per tag per session — the JS era appends them one at a time
/// via this command. The deleted bulk `save_session_tags(Vec<SessionTag>)`
/// command had no JS callers and was dropped per Principle VII (see
/// contracts/tauri-bridge.md §Deletions).
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever variant the Tauri-side handler
/// maps its filesystem failure to (typically `BridgeError::Internal`).
pub async fn add_session_tag(session_tag: SessionTag) -> Result<(), BridgeError> {
    invoke_serde("add_session_tag", &AddSessionTagArgs { session_tag }).await
}

// ---------------------------------------------------------------------------
// Settings & data lifecycle
// ---------------------------------------------------------------------------

/// Persist the user's full settings record. Tauri-side handler:
/// `save_settings(settings: AppSettings) -> Result<(), BridgeError>`
/// at `src-tauri/src/lib.rs:625`.
///
/// The handler also updates the in-process `SettingsState` mutex so
/// other Rust-side reads see the fresh value without an extra disk
/// round-trip. The wrapper does not observe that side-effect; it
/// surfaces only the IO outcome.
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever variant the Tauri-side handler
/// maps its filesystem failure to (typically `BridgeError::Internal`).
pub async fn save_settings(settings: Settings) -> Result<(), BridgeError> {
    invoke_named_arg("save_settings", "settings", settings).await
}

/// Read the persisted settings record. Tauri-side handler:
/// `load_settings() -> Result<AppSettings, BridgeError>`
/// at `src-tauri/src/lib.rs:642`.
///
/// Returns `Settings` (not `Option<Settings>`) — the Tauri-side
/// `helpers::read_settings_from` falls back to `AppSettings::default()`
/// when no settings file exists yet, so the cold-start case yields the
/// default record rather than a `None` discriminator. Missing nested
/// fields in older `0.4.x` settings JSONs are filled in by
/// `#[serde(default)]` on each field (per FR-005 — round-trip every
/// released JSON without manual migration).
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever variant the Tauri-side handler
/// maps its filesystem failure to (typically `BridgeError::Internal`),
/// or `BridgeError::SerdeRoundtrip` if the on-disk JSON cannot be
/// deserialised.
pub async fn load_settings() -> Result<Settings, BridgeError> {
    invoke_serde("load_settings", &serde_json::Value::Null).await
}

/// Wipe every app-data file and reset the in-process settings state.
///
/// Removes all sessions, history, tasks, manual sessions, tags, and
/// session-tag join files, then resets the in-process `SettingsState`
/// to `AppSettings::default()`. Tauri-side handler:
/// `reset_all_data() -> Result<(), BridgeError>` at
/// `src-tauri/src/lib.rs:698`.
///
/// No-arg destructive command. The JS-side caller is expected to gate
/// on user intent (a confirm dialog) before invoking; the wrapper does
/// not interpose its own confirmation step.
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever variant the Tauri-side handler
/// maps its filesystem failure to (typically `BridgeError::Internal`).
pub async fn reset_all_data() -> Result<(), BridgeError> {
    invoke_serde("reset_all_data", &serde_json::Value::Null).await
}

// ---------------------------------------------------------------------------
// Global shortcuts
// ---------------------------------------------------------------------------

/// Replace every global keyboard shortcut binding with the supplied
/// `ShortcutSettings`.
///
/// Tauri-side handler:
/// `register_global_shortcuts(shortcuts: ShortcutSettings) -> Result<(), BridgeError>`
/// at `src-tauri/src/lib.rs:652`.
///
/// The handler unregisters all existing shortcuts before installing the
/// new bindings, then emits a `shortcuts-updated` event so the
/// Leptos-side `managers::settings` slice can refresh its local state.
/// The deleted bulk `unregister_global_shortcuts` command (Principle
/// VII) is replaced JS-side by re-calling this command with all-`None`
/// bindings, so no separate wrapper exists.
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever variant the Tauri-side handler
/// maps its plugin failure to (typically `BridgeError::Internal` for an
/// invalid shortcut spec or for the global-shortcut plugin's own errors).
pub async fn register_global_shortcuts(shortcuts: ShortcutSettings) -> Result<(), BridgeError> {
    invoke_named_arg("register_global_shortcuts", "shortcuts", shortcuts).await
}

// ---------------------------------------------------------------------------
// Activity monitoring
// ---------------------------------------------------------------------------

/// Start the macOS-side `ActivityMonitor` with the supplied idle
/// timeout. Tauri-side handler:
/// `start_activity_monitoring(timeout_seconds: u64) -> Result<(), BridgeError>`
/// at `src-tauri/src/lib.rs:418`.
///
/// macOS-only on the Rust side (the handler is `cfg(target_os = "macos")`
/// gated; on other hosts it returns an error). The Leptos wrapper is
/// platform-agnostic — the consumer (`engine::activity_signal`) is
/// responsible for ignoring the error on non-macOS hosts (it falls
/// back to DOM-event-driven activity detection there).
///
/// `timeout_seconds: u64` matches the Tauri-side handler exactly.
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns the Tauri-side handler's error
/// variant — `BridgeError::Internal` on macOS for monitor-thread
/// failures, or whatever the non-macOS stub returns (typically also
/// `BridgeError::Internal`).
pub async fn start_activity_monitoring(timeout_seconds: u64) -> Result<(), BridgeError> {
    invoke_serde(
        "start_activity_monitoring",
        &StartActivityMonitoringArgs { timeout_seconds },
    )
    .await
}

/// Stop the macOS-side `ActivityMonitor`. Tauri-side handler:
/// `stop_activity_monitoring() -> Result<(), BridgeError>`
/// at `src-tauri/src/lib.rs:439`.
///
/// No-arg counterpart to `start_activity_monitoring`. The Tauri-side
/// handler reaches for the `ACTIVITY_MONITOR` static mutex and calls
/// `stop_monitoring()` on the inner monitor if one was previously
/// installed; if no monitor was ever started, the call is silently
/// absorbed and `Ok(())` is returned. The Leptos wrapper does not
/// distinguish those cases.
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. The Tauri-side handler does not currently produce a
/// failure case (see lib.rs `stop_activity_monitoring`); any non-success
/// would surface as `BridgeError::Internal`.
pub async fn stop_activity_monitoring() -> Result<(), BridgeError> {
    invoke_serde("stop_activity_monitoring", &serde_json::Value::Null).await
}

/// Reconfigure the running `ActivityMonitor`'s idle threshold without
/// tearing it down.
///
/// Tauri-side handler:
/// `update_activity_timeout(timeout_seconds: u64) -> Result<(), BridgeError>`
/// at `src-tauri/src/lib.rs:450`.
///
/// `timeout_seconds: u64` matches the Tauri-side handler exactly.
/// Distinct from `stop_activity_monitoring` in that the Tauri-side
/// handler returns `BridgeError::Internal { msg: "Activity monitor not
/// initialized" }` if no monitor is currently installed — callers must
/// install one via `start_activity_monitoring` first, or accept that
/// variant.
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Returns `BridgeError::Internal` if no monitor is
/// installed (Tauri-side handler condition).
pub async fn update_activity_timeout(timeout_seconds: u64) -> Result<(), BridgeError> {
    invoke_serde(
        "update_activity_timeout",
        &UpdateActivityTimeoutArgs { timeout_seconds },
    )
    .await
}

// ---------------------------------------------------------------------------
// Autostart
// ---------------------------------------------------------------------------

/// Enable launch-on-login for the app via the OS-native autolaunch
/// mechanism. Tauri-side handler:
/// `enable_autostart() -> Result<(), BridgeError>`
/// at `src-tauri/src/lib.rs:715`.
///
/// No-arg setter that delegates to the `tauri-plugin-autostart` crate's
/// `AutoLaunchManager::enable()` (which writes a Login Items entry on
/// macOS, a Run-key registry entry on Windows, and a `.desktop`
/// autostart file on XDG-conformant Linux desktops).
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Plugin failures (e.g., user has revoked the OS
/// permission to manage Login Items) surface as `BridgeError::Internal`.
pub async fn enable_autostart() -> Result<(), BridgeError> {
    invoke_serde("enable_autostart", &serde_json::Value::Null).await
}

/// Disable launch-on-login. Tauri-side handler:
/// `disable_autostart() -> Result<(), BridgeError>`
/// at `src-tauri/src/lib.rs:722`.
///
/// Counterpart to `enable_autostart`; delegates to
/// `AutoLaunchManager::disable()`. Idempotent at the plugin layer —
/// calling `disable` when autostart is already off is a successful
/// no-op.
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Plugin failures surface as `BridgeError::Internal`.
pub async fn disable_autostart() -> Result<(), BridgeError> {
    invoke_serde("disable_autostart", &serde_json::Value::Null).await
}

/// Read whether launch-on-login is currently enabled. Tauri-side handler:
/// `is_autostart_enabled() -> Result<bool, BridgeError>`
/// at `src-tauri/src/lib.rs:729`.
///
/// Read-only counterpart to `enable_autostart` / `disable_autostart`;
/// delegates to `AutoLaunchManager::is_enabled()`.
///
/// **Short-circuit policy (Phase 1G refinement, the `is_read_only_query`
/// shape)**: when the Tauri JS bridge is absent the wrapper returns
/// `Ok(false)` rather than the uniform `Err(BridgeError::BridgeUnavailable)`
/// the other 25 wrappers use. Rationale: this is the only "is X
/// enabled?" predicate in the surviving command surface. Consumers
/// (`managers/settings.rs` and the autostart UI toggle) want the
/// `Result<bool, …>` to collapse to a single boolean arm when the
/// bridge is absent (Trunk dev server, e2e mock, node tests) — they
/// would otherwise have to write
/// `match is_autostart_enabled().await { Ok(true) => …, Ok(false) | Err(BridgeUnavailable) => …, Err(other) => … }`,
/// which expands to a three-arm match where the middle arm is
/// always-the-same logic. Returning `Ok(false)` collapses that to
/// `Ok(true) => …, Ok(false) => …, Err(other) => …`. The
/// `Err(BridgeUnavailable)` arm becomes unreachable at the type
/// level for this wrapper.
///
/// **Why scoped to this one wrapper**: save/update commands have
/// real side effects to perform; list-returning reads need to
/// distinguish "bridge absent" from "bridge present, but no rows yet"
/// (a sentinel `Ok(vec![])` would conflate the two). For
/// `is_autostart_enabled`, "bridge absent" and "definitely not
/// enabled" are operationally identical — there's no way to enable
/// autostart without a Tauri bridge, so reporting `false` is honest.
/// See contracts/tauri-bridge.md §"Error handling".
///
/// # Errors
/// Bridge-absent does NOT produce an error — see the short-circuit
/// policy above. Returns `BridgeError::Internal` for plugin failures
/// under a live bridge (rare; e.g., the underlying `auto-launch`
/// crate's macOS-LSSharedFileList API surfacing). Returns
/// `BridgeError::SerdeRoundtrip` if the Tauri-side return shape ever
/// drifts from `bool` (would be a contract regression — not expected
/// in normal operation).
pub async fn is_autostart_enabled() -> Result<bool, BridgeError> {
    // Sentinel short-circuit: read-only "is X enabled?" predicates
    // collapse `BridgeAvailable::Absent` to `Ok(false)` per the
    // doc-comment above. Every other wrapper in this module uses
    // `invoke_serde`'s built-in `Err(BridgeUnavailable)` short-circuit;
    // intercepting here before the helper runs preserves the
    // uniformity of `invoke_serde` while letting this single wrapper
    // diverge in its sentinel shape.
    if bridge_available().is_absent() {
        return Ok(false);
    }
    invoke_serde("is_autostart_enabled", &serde_json::Value::Null).await
}

// ---------------------------------------------------------------------------
// Window & tray
// ---------------------------------------------------------------------------

/// Update the system-tray icon's title, tooltip, and mode glyph.
///
/// Tauri-side handler:
/// `update_tray_icon(timer_text, is_running, session_mode, current_session,
///                   total_sessions, mode_icon) -> Result<(), BridgeError>`
/// at `src-tauri/src/lib.rs:538`.
///
/// The contract collapses six positional Tauri-side args into a single
/// `UpdateTrayIconArgs` struct (data-model.md §`UpdateTrayIconArgs`).
/// `serde-wasm-bindgen` serialises the struct's fields to top-level keys
/// in the Tauri args bag — byte-identical wire shape to the
/// pre-collapse JS call site.
///
/// `session_mode: TimerMode` is the closed-domain enum tightening from
/// Phase 1A T027 (was `String` pre-cutover); the camelCase wire form
/// (`"focus"` / `"break"` / `"longBreak"`) is preserved exactly via
/// `TimerMode`'s `#[serde(rename_all = "camelCase")]`.
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. The Tauri-side handler runs the title/tooltip update on
/// the macOS main thread; failures (e.g., the `tray_by_id("main")`
/// lookup misses, or the macOS thread dispatch fails) surface as
/// `BridgeError::Internal`.
pub async fn update_tray_icon(args: UpdateTrayIconArgs) -> Result<(), BridgeError> {
    invoke_serde("update_tray_icon", &args).await
}

/// Rebuild the system-tray context menu so item enable/disable state
/// reflects the live timer's status.
///
/// Tauri-side handler:
/// `update_tray_menu(is_running: bool, is_paused: bool,
///                   is_overtime: bool, current_mode: TimerMode)
///                   -> Result<(), BridgeError>`
/// at `src-tauri/src/lib.rs:1124`.
///
/// Distinct from `update_tray_icon` in that the contract row 24 keeps
/// the three args as separate parameters (no struct collapse) — the
/// argument count is small and the call site reads naturally.
///
/// `current_mode: TimerMode` is the closed-domain enum tightening from
/// Phase 1A T027 (was `String` pre-cutover); the camelCase wire form is
/// preserved exactly via `TimerMode`'s `#[serde(rename_all =
/// "camelCase")]`. The Tauri-side handler uses the variant to choose
/// the cancel-item label (`"Cancel"` in Focus mode vs. `"Cancel Last"`
/// during a break).
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Tauri-side menu-construction failures (rare; e.g.,
/// `MenuItem::with_id` returns an error) surface as `BridgeError::Internal`.
pub async fn update_tray_menu(
    is_running: bool,
    is_paused: bool,
    is_overtime: bool,
    current_mode: TimerMode,
) -> Result<(), BridgeError> {
    invoke_serde(
        "update_tray_menu",
        &UpdateTrayMenuArgs {
            is_running,
            is_paused,
            is_overtime,
            current_mode,
        },
    )
    .await
}

/// Build an XLSX workbook from `sessions` and write it to `path`.
///
/// Tauri-side handler: `export_sessions_xlsx(path: String, sessions:
/// Vec<ManualSession>) -> Result<(), BridgeError>` at `src-tauri/src/lib.rs`.
/// The handler delegates to `rust_xlsxwriter` (write-only; sufficient
/// because we never read .xlsx files), constructing the workbook
/// server-side rather than over the bridge — less data crossing IPC,
/// no JS xlsx library in the WASM bundle.
///
/// Replaces the JS `xlsx` library's `writeFile()` path; the legacy
/// `write_excel_file` cutover-parity wrapper was removed in Phase 6.
///
/// `path` is the user-chosen save location (typically obtained via
/// the Tauri dialog plugin's `save()` call site-side; the wrapper
/// does not pre-validate the extension or directory existence — the
/// handler's `rust_xlsxwriter::Workbook::save` surfaces filesystem
/// errors via `BridgeError::Internal`).
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Returns `BridgeError::Internal` for filesystem errors,
/// `rust_xlsxwriter` errors, or row/column index overflows on
/// pathologically large session lists.
pub async fn export_sessions_xlsx(
    path: String,
    sessions: Vec<ManualSession>,
) -> Result<(), BridgeError> {
    #[derive(Serialize)]
    struct Args {
        path: String,
        sessions: Vec<ManualSession>,
    }
    invoke_serde("export_sessions_xlsx", &Args { path, sessions }).await
}

/// Write `sessions` to `path` as RFC 4180 CSV. Same column schema as
/// the xlsx export.
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Returns `BridgeError::Internal` for filesystem errors.
pub async fn export_sessions_csv(
    path: String,
    sessions: Vec<ManualSession>,
) -> Result<(), BridgeError> {
    #[derive(Serialize)]
    struct Args {
        path: String,
        sessions: Vec<ManualSession>,
    }
    invoke_serde("export_sessions_csv", &Args { path, sessions }).await
}

// ---------------------------------------------------------------------------
// UI helpers — dialog
// ---------------------------------------------------------------------------

/// Show a native Tauri confirmation dialog. Returns `Ok(true)` when the user
/// confirms, `Ok(false)` when they cancel.
///
/// The bridge mock maps `plugin:dialog|ask` to `false` by default (the e2e
/// test `settings-advanced.spec.js:44` exercises the cancel path). When the
/// bridge is absent, the call short-circuits; callers should use
/// `unwrap_or(false)` to treat the error as "not confirmed".
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever the plugin returns.
pub async fn dialog_ask(message: &str, title: &str) -> Result<bool, BridgeError> {
    #[derive(Serialize)]
    struct Args<'a> {
        message: &'a str,
        title: &'a str,
    }
    // Routed through our own `dialog_ask` Tauri command (defined in
    // `src-tauri/src/lib.rs`) instead of `plugin:dialog|ask` directly.
    // The wire shape is pinned by `tests/bindings_export.rs`, so any
    // drift between the wrapper and the handler signature fails CI
    // — eliminating the silent-bind-nothing class of bug that
    // previously broke the dialog_save call.
    invoke_serde("dialog_ask", &Args { message, title }).await
}

/// Ask the user for a save path via the native file-save dialog.
///
/// Returns `Ok(Some(path))` when the user selects a path, `Ok(None)` when
/// they cancel. The bridge mock maps `plugin:dialog|save` to `null` by
/// default so any spec that asserts no-export-on-cancel passes automatically.
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever the plugin returns.
pub async fn dialog_save(
    default_path: Option<String>,
    filters: Vec<(String, Vec<String>)>,
) -> Result<Option<String>, BridgeError> {
    #[derive(Serialize)]
    struct FilterArg {
        name: String,
        extensions: Vec<String>,
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        default_path: Option<String>,
        filters: Vec<FilterArg>,
    }
    // Routed through our own `dialog_save` Tauri command instead of
    // `plugin:dialog|save` directly. The plugin's command expects
    // `{ options: SaveDialogOptions }` — sending flat args at this
    // boundary silently bound nothing and the file picker never
    // appeared. The wrapper command owns the envelope shape and
    // pins it via `tests/bindings_export.rs`.
    let filters = filters
        .into_iter()
        .map(|(name, extensions)| FilterArg { name, extensions })
        .collect();
    invoke_serde(
        "dialog_save",
        &Args {
            default_path,
            filters,
        },
    )
    .await
}

// Tests gated on `wasm32` because every wrapper-test is a
// `#[wasm_bindgen_test]` — running them via `cargo test` on the host
// target would produce dead-code lint failures (the host-side
// `cfg(target_arch = "wasm32")` removal silently drops the test bodies).
// `wasm-pack test --node` is the canonical test driver per
// `quickstart.md` line 105 and tasks.md T030/T032 done-signals.
#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::{
        add_session_tag, append_manual_session, delete_tag, dialog_save, disable_autostart,
        enable_autostart, export_sessions_xlsx, get_stats_history, is_autostart_enabled,
        load_distractions, load_manual_sessions, load_quick_logs, load_session_data, load_settings,
        load_tags, load_tasks, register_global_shortcuts, reset_all_data, save_daily_stats,
        save_distractions, save_manual_sessions, save_quick_logs, save_session_data, save_settings,
        save_tag, save_tags_bulk, save_tasks, start_activity_monitoring, stop_activity_monitoring,
        update_activity_timeout, update_tray_icon, update_tray_menu,
    };
    use crate::bridge::types::BridgeError;
    use crate::bridge::types::SessionType;
    use crate::bridge::types::TimerMode;
    use crate::bridge::types::{
        Distraction, ManualSession, QuickLog, Session, SessionTag, Settings, ShortcutSettings, Tag,
        Task, UpdateTrayIconArgs,
    };
    use wasm_bindgen_test::wasm_bindgen_test;

    fn sample_session() -> Session {
        Session {
            completed_pomodoros: 3,
            total_focus_time: 4_500,
            current_session: 4,
            date: "Sat May 10 2026".to_string(),
            title: None,
        }
    }

    fn sample_tasks() -> Vec<Task> {
        vec![
            Task {
                id: 1,
                text: "ship the wrapper".to_string(),
                completed: false,
                created_at: "2026-05-10T08:00:00Z".to_string(),
                completed_at: None,
            },
            Task {
                id: 2,
                text: "write the test".to_string(),
                completed: true,
                created_at: "2026-05-10T07:30:00Z".to_string(),
                completed_at: Some("2026-05-10T08:30:00Z".to_string()),
            },
        ]
    }

    fn sample_manual_sessions() -> Vec<ManualSession> {
        vec![ManualSession {
            id: "ms-1".to_string(),
            session_type: SessionType::LongBreak,
            duration: 15,
            start_time: "10:00".to_string(),
            end_time: "10:15".to_string(),
            notes: Some("walk".to_string()),
            created_at: "2026-05-10T10:15:00Z".to_string(),
            date: "Sat May 10 2026".to_string(),
            tags: None,
            // Feature 002 Bundle A: per-session title. Wasm-test
            // sample defaults to `None` (LongBreak; breaks don't
            // carry titles).
            title: None,
        }]
    }

    /// Under `wasm-pack test --node`, no `__TAURI_INTERNALS__` is installed,
    /// so the wrapper MUST short-circuit with `BridgeError::BridgeUnavailable`
    /// rather than calling into a missing global. Pins FR-009.
    #[wasm_bindgen_test]
    async fn save_session_data_round_trip_short_circuits_when_bridge_absent() {
        let result = save_session_data(sample_session()).await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin: the wrapper must accept `Session` by value
    /// and return `Result<(), BridgeError>` per contracts/tauri-bridge.md row 1.
    /// Bind to a typed `async fn` pointer; if the signature drifts, this
    /// stops compiling — that's exactly the FR-008 promise.
    #[wasm_bindgen_test]
    async fn save_session_data_round_trip_signature_pinned() {
        async fn assert_signature(s: Session) -> Result<(), BridgeError> {
            save_session_data(s).await
        }
        // Drive the future once so the binding isn't elided. Under node the
        // bridge is absent, so the call resolves immediately to
        // BridgeUnavailable — but the load-bearing assertion is the typed
        // `async fn` shape captured by `assert_signature`'s declaration.
        let _ = assert_signature(sample_session()).await;
    }

    #[wasm_bindgen_test]
    async fn load_session_data_round_trip_short_circuits_when_bridge_absent() {
        let result = load_session_data().await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 2:
    /// `load_session_data() -> Result<Option<Session>, BridgeError>`.
    /// `Option<Session>` is the load-bearing shape — the Tauri handler
    /// returns `None` for the cold-start "no session yet" case rather than
    /// surfacing it as a `NotFound` error.
    #[wasm_bindgen_test]
    async fn load_session_data_round_trip_signature_pinned() {
        async fn assert_signature() -> Result<Option<Session>, BridgeError> {
            load_session_data().await
        }
        let _ = assert_signature().await;
    }

    #[wasm_bindgen_test]
    async fn get_stats_history_round_trip_short_circuits_when_bridge_absent() {
        let result = get_stats_history().await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 3:
    /// `get_stats_history() -> Result<Vec<Session>, BridgeError>`.
    #[wasm_bindgen_test]
    async fn get_stats_history_round_trip_signature_pinned() {
        async fn assert_signature() -> Result<Vec<Session>, BridgeError> {
            get_stats_history().await
        }
        let _ = assert_signature().await;
    }

    #[wasm_bindgen_test]
    async fn save_daily_stats_round_trip_short_circuits_when_bridge_absent() {
        let result = save_daily_stats(sample_session()).await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 4:
    /// `save_daily_stats(session: Session) -> Result<(), BridgeError>`.
    /// Same shape as `save_session_data` (a one-arg `Session` write) but a
    /// distinct command — the handler appends to a daily-stats file on
    /// disk rather than overwriting the live-session file.
    #[wasm_bindgen_test]
    async fn save_daily_stats_round_trip_signature_pinned() {
        async fn assert_signature(s: Session) -> Result<(), BridgeError> {
            save_daily_stats(s).await
        }
        let _ = assert_signature(sample_session()).await;
    }

    #[wasm_bindgen_test]
    async fn save_tasks_round_trip_short_circuits_when_bridge_absent() {
        let result = save_tasks(sample_tasks()).await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 5:
    /// `save_tasks(tasks: Vec<Task>) -> Result<(), BridgeError>`.
    #[wasm_bindgen_test]
    async fn save_tasks_round_trip_signature_pinned() {
        async fn assert_signature(t: Vec<Task>) -> Result<(), BridgeError> {
            save_tasks(t).await
        }
        let _ = assert_signature(sample_tasks()).await;
    }

    #[wasm_bindgen_test]
    async fn load_tasks_round_trip_short_circuits_when_bridge_absent() {
        let result = load_tasks().await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 6:
    /// `load_tasks() -> Result<Vec<Task>, BridgeError>`.
    /// Returns an empty `Vec` for the no-tasks-file cold-start case.
    #[wasm_bindgen_test]
    async fn load_tasks_round_trip_signature_pinned() {
        async fn assert_signature() -> Result<Vec<Task>, BridgeError> {
            load_tasks().await
        }
        let _ = assert_signature().await;
    }

    #[wasm_bindgen_test]
    async fn save_manual_sessions_round_trip_short_circuits_when_bridge_absent() {
        let result = save_manual_sessions(sample_manual_sessions()).await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 7:
    /// `save_manual_sessions(sessions: Vec<ManualSession>) -> Result<(), BridgeError>`.
    /// Pins that `ManualSession.session_type` is the closed-domain
    /// `SessionType` enum (Phase 1A T029) — a string drift here would
    /// stop compiling.
    #[wasm_bindgen_test]
    async fn save_manual_sessions_round_trip_signature_pinned() {
        async fn assert_signature(s: Vec<ManualSession>) -> Result<(), BridgeError> {
            save_manual_sessions(s).await
        }
        let _ = assert_signature(sample_manual_sessions()).await;
    }

    #[wasm_bindgen_test]
    async fn load_manual_sessions_round_trip_short_circuits_when_bridge_absent() {
        let result = load_manual_sessions().await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 8:
    /// `load_manual_sessions() -> Result<Vec<ManualSession>, BridgeError>`.
    /// Pins both the empty-Vec cold-start convention AND the closed-domain
    /// `SessionType` enum on the deserialise side — a wire shape that
    /// arrived with `session_type: "<unknown variant>"` would surface as
    /// `BridgeError::SerdeRoundtrip` instead of a silently-ignored field.
    #[wasm_bindgen_test]
    async fn load_manual_sessions_round_trip_signature_pinned() {
        async fn assert_signature() -> Result<Vec<ManualSession>, BridgeError> {
            load_manual_sessions().await
        }
        let _ = assert_signature().await;
    }

    #[wasm_bindgen_test]
    async fn load_tags_round_trip_short_circuits_when_bridge_absent() {
        let result = load_tags().await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 9:
    /// `load_tags() -> Result<Vec<Tag>, BridgeError>`.
    /// Returns an empty `Vec` for the no-tags-file cold-start case (the
    /// Tauri-side helper at `helpers::read_tags_from` treats `NotFound`
    /// as empty — same cold-start convention as `load_tasks` /
    /// `load_manual_sessions`).
    #[wasm_bindgen_test]
    async fn load_tags_round_trip_signature_pinned() {
        async fn assert_signature() -> Result<Vec<Tag>, BridgeError> {
            load_tags().await
        }
        let _ = assert_signature().await;
    }

    fn sample_tag() -> Tag {
        Tag {
            id: "tag-focus".to_string(),
            name: "Deep Work".to_string(),
            icon: "ri-brain-line".to_string(),
            color: "#4CAF50".to_string(),
            created_at: "2026-05-10T08:00:00Z".to_string(),
        }
    }

    #[wasm_bindgen_test]
    async fn save_tag_round_trip_short_circuits_when_bridge_absent() {
        let result = save_tag(sample_tag()).await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 10:
    /// `save_tag(tag: Tag) -> Result<(), BridgeError>`.
    /// Distinct from the deleted `save_tags` (Vec) bulk command — the JS
    /// era saves tags one at a time via this upsert path.
    #[wasm_bindgen_test]
    async fn save_tag_round_trip_signature_pinned() {
        async fn assert_signature(t: Tag) -> Result<(), BridgeError> {
            save_tag(t).await
        }
        let _ = assert_signature(sample_tag()).await;
    }

    #[wasm_bindgen_test]
    async fn delete_tag_round_trip_short_circuits_when_bridge_absent() {
        let result = delete_tag("tag-focus".to_string()).await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 11:
    /// `delete_tag(tag_id: String) -> Result<(), BridgeError>`.
    /// The arg is a bare `String` (not a `Tag`) — deletion lookup is by
    /// id, and the Tauri-side handler does an in-place filter rather
    /// than requiring the full record.
    #[wasm_bindgen_test]
    async fn delete_tag_round_trip_signature_pinned() {
        async fn assert_signature(id: String) -> Result<(), BridgeError> {
            delete_tag(id).await
        }
        let _ = assert_signature("tag-focus".to_string()).await;
    }

    fn sample_session_tag() -> SessionTag {
        SessionTag {
            session_id: "session-2026-05-10-04".to_string(),
            tag_id: "tag-focus".to_string(),
            duration: 1500,
            created_at: "2026-05-10T08:25:00Z".to_string(),
        }
    }

    #[wasm_bindgen_test]
    async fn add_session_tag_round_trip_short_circuits_when_bridge_absent() {
        let result = add_session_tag(sample_session_tag()).await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 12:
    /// `add_session_tag(session_tag: SessionTag) -> Result<(), BridgeError>`.
    /// The JS era appends one session-tag join row at a time via this
    /// command; the deleted bulk `save_session_tags` (Vec) had no JS
    /// callers and was dropped per Principle VII.
    #[wasm_bindgen_test]
    async fn add_session_tag_round_trip_signature_pinned() {
        async fn assert_signature(st: SessionTag) -> Result<(), BridgeError> {
            add_session_tag(st).await
        }
        let _ = assert_signature(sample_session_tag()).await;
    }

    fn sample_settings() -> Settings {
        Settings::default()
    }

    #[wasm_bindgen_test]
    async fn save_settings_round_trip_short_circuits_when_bridge_absent() {
        let result = save_settings(sample_settings()).await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 13:
    /// `save_settings(settings: Settings) -> Result<(), BridgeError>`.
    /// Pins the nested `Settings` shape (`ShortcutSettings`,
    /// `TimerSettings`, `NotificationSettings`, `AdvancedSettings`) at
    /// the bridge boundary — a missing or renamed nested field would
    /// stop compiling here per FR-008.
    #[wasm_bindgen_test]
    async fn save_settings_round_trip_signature_pinned() {
        async fn assert_signature(s: Settings) -> Result<(), BridgeError> {
            save_settings(s).await
        }
        let _ = assert_signature(sample_settings()).await;
    }

    #[wasm_bindgen_test]
    async fn load_settings_round_trip_short_circuits_when_bridge_absent() {
        let result = load_settings().await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 14:
    /// `load_settings() -> Result<Settings, BridgeError>`.
    /// Distinct from `load_session_data` / `load_tasks` etc. in that the
    /// return is a bare `Settings` (not `Option<Settings>` or `Vec<…>`):
    /// the Tauri-side handler falls back to `Settings::default()` when
    /// no settings file exists yet, so the cold-start case yields the
    /// default record rather than `None`.
    #[wasm_bindgen_test]
    async fn load_settings_round_trip_signature_pinned() {
        async fn assert_signature() -> Result<Settings, BridgeError> {
            load_settings().await
        }
        let _ = assert_signature().await;
    }

    #[wasm_bindgen_test]
    async fn reset_all_data_round_trip_short_circuits_when_bridge_absent() {
        let result = reset_all_data().await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 15:
    /// `reset_all_data() -> Result<(), BridgeError>`.
    /// No-arg destructive command — the Tauri-side handler clears every
    /// app-data file and resets `SettingsState` to `AppSettings::default()`
    /// in one shot. There is no confirmation arg because the JS-side
    /// caller is expected to gate on user intent before invoking.
    #[wasm_bindgen_test]
    async fn reset_all_data_round_trip_signature_pinned() {
        async fn assert_signature() -> Result<(), BridgeError> {
            reset_all_data().await
        }
        let _ = assert_signature().await;
    }

    fn sample_shortcuts() -> ShortcutSettings {
        ShortcutSettings::default()
    }

    #[wasm_bindgen_test]
    async fn register_global_shortcuts_round_trip_short_circuits_when_bridge_absent() {
        let result = register_global_shortcuts(sample_shortcuts()).await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 16:
    /// `register_global_shortcuts(shortcuts: ShortcutSettings) -> Result<(), BridgeError>`.
    /// `ShortcutSettings` is the same struct embedded in `Settings`
    /// (defined in `types.rs`) — the wrapper does not introduce a
    /// shadow type. The deleted bulk `unregister_global_shortcuts`
    /// command (Principle VII) is replaced JS-side by re-calling this
    /// command with `None` bindings, hence no separate wrapper.
    #[wasm_bindgen_test]
    async fn register_global_shortcuts_round_trip_signature_pinned() {
        async fn assert_signature(s: ShortcutSettings) -> Result<(), BridgeError> {
            register_global_shortcuts(s).await
        }
        let _ = assert_signature(sample_shortcuts()).await;
    }

    #[wasm_bindgen_test]
    async fn start_activity_monitoring_round_trip_short_circuits_when_bridge_absent() {
        let result = start_activity_monitoring(30).await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 17:
    /// `start_activity_monitoring(timeout_seconds: u64) -> Result<(), BridgeError>`.
    /// macOS-only Rust-side; on other platforms the handler returns an
    /// error, but the Leptos wrapper signature is platform-agnostic
    /// because the bridge never branches on the host platform — that
    /// kind of conditionality belongs to the consumer (`engine::activity_signal`).
    /// `timeout_seconds` is `u64` to match the Tauri-side handler exactly.
    #[wasm_bindgen_test]
    async fn start_activity_monitoring_round_trip_signature_pinned() {
        async fn assert_signature(t: u64) -> Result<(), BridgeError> {
            start_activity_monitoring(t).await
        }
        let _ = assert_signature(30).await;
    }

    #[wasm_bindgen_test]
    async fn stop_activity_monitoring_round_trip_short_circuits_when_bridge_absent() {
        let result = stop_activity_monitoring().await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 18:
    /// `stop_activity_monitoring() -> Result<(), BridgeError>`.
    /// No-arg counterpart to `start_activity_monitoring` — the Tauri-side
    /// handler reaches for the `ACTIVITY_MONITOR` mutex and calls
    /// `stop_monitoring()` if a monitor was previously installed; the
    /// no-op case (monitor never started) is silently absorbed at the
    /// handler. The Leptos wrapper does not differentiate.
    #[wasm_bindgen_test]
    async fn stop_activity_monitoring_round_trip_signature_pinned() {
        async fn assert_signature() -> Result<(), BridgeError> {
            stop_activity_monitoring().await
        }
        let _ = assert_signature().await;
    }

    #[wasm_bindgen_test]
    async fn update_activity_timeout_round_trip_short_circuits_when_bridge_absent() {
        let result = update_activity_timeout(45).await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 19:
    /// `update_activity_timeout(timeout_seconds: u64) -> Result<(), BridgeError>`.
    /// `u64` matches the Tauri-side handler exactly. Distinct from
    /// `start_activity_monitoring` in that the Tauri-side handler returns
    /// `BridgeError::Internal { msg: "Activity monitor not initialized" }`
    /// when no monitor is currently installed (rather than absorbing the
    /// call); the wrapper surfaces that variant unchanged.
    #[wasm_bindgen_test]
    async fn update_activity_timeout_round_trip_signature_pinned() {
        async fn assert_signature(t: u64) -> Result<(), BridgeError> {
            update_activity_timeout(t).await
        }
        let _ = assert_signature(45).await;
    }

    #[wasm_bindgen_test]
    async fn enable_autostart_round_trip_short_circuits_when_bridge_absent() {
        let result = enable_autostart().await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 20:
    /// `enable_autostart() -> Result<(), BridgeError>`.
    /// No-arg setter — the Tauri-side handler reaches into the autolaunch
    /// plugin's `AutoLaunchManager::enable()`. Failure (e.g., the user has
    /// revoked the necessary OS permission) maps to `BridgeError::Internal`
    /// at the handler boundary; the wrapper surfaces it unchanged.
    #[wasm_bindgen_test]
    async fn enable_autostart_round_trip_signature_pinned() {
        async fn assert_signature() -> Result<(), BridgeError> {
            enable_autostart().await
        }
        let _ = assert_signature().await;
    }

    #[wasm_bindgen_test]
    async fn disable_autostart_round_trip_short_circuits_when_bridge_absent() {
        let result = disable_autostart().await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 21:
    /// `disable_autostart() -> Result<(), BridgeError>`.
    /// No-arg setter; the Tauri-side handler delegates to the autolaunch
    /// plugin's `AutoLaunchManager::disable()`. Idempotent — calling
    /// `disable` when autostart is already off is a successful no-op.
    #[wasm_bindgen_test]
    async fn disable_autostart_round_trip_signature_pinned() {
        async fn assert_signature() -> Result<(), BridgeError> {
            disable_autostart().await
        }
        let _ = assert_signature().await;
    }

    /// Phase 1G refinement (T118-T119) per contracts/tauri-bridge.md
    /// §"Error handling": `is_autostart_enabled` is a read-only
    /// "is X on?" query, and consumers want the `Result<bool, …>`
    /// shape to collapse to a single `bool` arm at the call site
    /// when the bridge is absent (rather than handling both `Err`
    /// AND `Ok(false)`/`Ok(true)`). The bridge-absent short-circuit
    /// returns `Ok(false)` — the documented sentinel for "treat as
    /// not-enabled" — instead of the uniform `Err(BridgeUnavailable)`
    /// the other 25 wrappers use.
    ///
    /// This is the only wrapper that gets sentinel semantics in
    /// Phase 1G. Save/update commands keep `Err` because their
    /// caller has a real side-effect to perform; list-returning
    /// reads keep `Err` for now (broader sentinel adoption is a
    /// follow-up). See the wrapper doc-comment at the top of this
    /// file for the rationale.
    #[wasm_bindgen_test]
    async fn is_autostart_enabled_short_circuits_to_false_when_bridge_absent() {
        let result = is_autostart_enabled().await;
        match result {
            Ok(false) => {}
            other => panic!("expected Ok(false) sentinel, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 22:
    /// `is_autostart_enabled() -> Result<bool, BridgeError>`. The
    /// outer `Result` is preserved post-1G so callers that want the
    /// fail-loud shape can still bind a non-`BridgeUnavailable` error
    /// (e.g., `Internal` from a plugin failure under the live bridge);
    /// only the `BridgeUnavailable` short-circuit collapses to a
    /// sentinel `Ok(false)`. Pins that the wrapper returns the bare
    /// `bool` rather than an `Option<bool>` or wrapped record.
    #[wasm_bindgen_test]
    async fn is_autostart_enabled_round_trip_signature_pinned() {
        async fn assert_signature() -> Result<bool, BridgeError> {
            is_autostart_enabled().await
        }
        let _ = assert_signature().await;
    }

    fn sample_tray_icon_args() -> UpdateTrayIconArgs {
        UpdateTrayIconArgs {
            timer_text: "24:36".to_string(),
            is_running: true,
            session_mode: TimerMode::Focus,
            current_session: 2,
            total_sessions: 4,
            mode_icon: Some("\u{25c9}".to_string()),
        }
    }

    #[wasm_bindgen_test]
    async fn update_tray_icon_round_trip_short_circuits_when_bridge_absent() {
        let result = update_tray_icon(sample_tray_icon_args()).await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 23:
    /// `update_tray_icon(args: UpdateTrayIconArgs) -> Result<(), BridgeError>`.
    /// The contract collapses the Tauri-side handler's six positional args
    /// (`timer_text`, `is_running`, `session_mode`, `current_session`,
    /// `total_sessions`, `mode_icon`) into a single typed wrapper struct
    /// per data-model.md §`UpdateTrayIconArgs`; the on-the-wire shape is
    /// preserved because `serde-wasm-bindgen` flattens the struct fields
    /// to top-level keys in the Tauri args bag. Pins both the typed
    /// `TimerMode` enum (Phase 1A T027 — was `String`) AND the camelCase
    /// `session_mode` wire form via `TimerMode`'s `#[serde(rename_all =
    /// "camelCase")]`.
    #[wasm_bindgen_test]
    async fn update_tray_icon_round_trip_signature_pinned() {
        async fn assert_signature(a: UpdateTrayIconArgs) -> Result<(), BridgeError> {
            update_tray_icon(a).await
        }
        let _ = assert_signature(sample_tray_icon_args()).await;
    }

    #[wasm_bindgen_test]
    async fn update_tray_menu_round_trip_short_circuits_when_bridge_absent() {
        let result = update_tray_menu(true, false, false, TimerMode::Focus).await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 24:
    /// `update_tray_menu(is_running: bool, is_paused: bool,
    ///                   is_overtime: bool, current_mode: TimerMode)
    ///                   -> Result<(), BridgeError>`.
    /// Distinct from `update_tray_icon` in that the contract keeps the
    /// three Tauri-side args as separate parameters (no struct collapse)
    /// — the call sites are infrequent enough that a wrapper struct
    /// would add noise without ergonomic gain. Pins the typed `TimerMode`
    /// enum (Phase 1A T027 — was `String`) at the bridge boundary; a
    /// string drift on `current_mode` would not compile (FR-008).
    #[wasm_bindgen_test]
    async fn update_tray_menu_round_trip_signature_pinned() {
        async fn assert_signature(
            is_running: bool,
            is_paused: bool,
            is_overtime: bool,
            current_mode: TimerMode,
        ) -> Result<(), BridgeError> {
            update_tray_menu(is_running, is_paused, is_overtime, current_mode).await
        }
        let _ = assert_signature(true, false, false, TimerMode::LongBreak).await;
    }

    // -----------------------------------------------------------------------
    // export_sessions_xlsx — Tauri-side workbook write from
    // `Vec<ManualSession>` (no JS xlsx dep in the bundle).
    // -----------------------------------------------------------------------

    #[wasm_bindgen_test]
    async fn export_sessions_xlsx_round_trip_short_circuits_when_bridge_absent() {
        let result =
            export_sessions_xlsx("/tmp/export.xlsx".to_string(), sample_manual_sessions()).await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md
    /// §"`export_sessions_xlsx` — replaces JS `xlsx` library":
    /// `export_sessions_xlsx(path: String, sessions: Vec<ManualSession>)
    ///     -> Result<(), BridgeError>`.
    /// Replaces the legacy `write_excel_file(path, data)` JS-era path
    /// (removed in Phase 6 per contracts/tauri-bridge.md §Deletions): the
    /// Tauri-side handler builds the workbook itself from typed
    /// `ManualSession` records rather than accepting a base64-encoded
    /// blob the JS `xlsx` library produced. Same on-disk file shape;
    /// less data crossing the bridge.
    #[wasm_bindgen_test]
    async fn export_sessions_xlsx_round_trip_signature_pinned() {
        async fn assert_signature(
            path: String,
            sessions: Vec<ManualSession>,
        ) -> Result<(), BridgeError> {
            export_sessions_xlsx(path, sessions).await
        }
        let _ = assert_signature("/tmp/export.xlsx".to_string(), sample_manual_sessions()).await;
    }

    // -----------------------------------------------------------------------
    // dialog_save — native file-save dialog for the
    // CSV/XLSX export path; returns Ok(Some(path)) on selection, Ok(None)
    // on cancel.
    // -----------------------------------------------------------------------

    #[wasm_bindgen_test]
    async fn dialog_save_short_circuits_when_bridge_absent() {
        let result = dialog_save(None, vec![]).await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin:
    /// `dialog_save(default_path: Option<String>, filters: Vec<(String, Vec<String>)>)
    ///     -> Result<Option<String>, BridgeError>`.
    /// Exercises `Args` and `FilterArg` serialisation inside `dialog_save`
    /// — if either struct field is renamed or the `invoke_serde` call
    /// signature drifts, this stops compiling (FR-008).
    #[wasm_bindgen_test]
    async fn dialog_save_signature_pinned() {
        async fn assert_signature(
            default_path: Option<String>,
            filters: Vec<(String, Vec<String>)>,
        ) -> Result<Option<String>, BridgeError> {
            dialog_save(default_path, filters).await
        }
        let _ = assert_signature(
            Some("/tmp/export.xlsx".to_string()),
            vec![("Excel".to_string(), vec!["xlsx".to_string()])],
        )
        .await;
    }

    #[wasm_bindgen_test]
    async fn save_quick_logs_short_circuits_when_bridge_absent() {
        let result = save_quick_logs(vec![]).await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    #[wasm_bindgen_test]
    async fn save_quick_logs_signature_pinned() {
        async fn assert_signature(ql: Vec<QuickLog>) -> Result<(), BridgeError> {
            save_quick_logs(ql).await
        }
        let _ = assert_signature(vec![]).await;
    }

    #[wasm_bindgen_test]
    async fn load_quick_logs_short_circuits_when_bridge_absent() {
        let result = load_quick_logs().await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    #[wasm_bindgen_test]
    async fn load_quick_logs_signature_pinned() {
        async fn assert_signature() -> Result<Vec<QuickLog>, BridgeError> {
            load_quick_logs().await
        }
        let _ = assert_signature().await;
    }

    #[wasm_bindgen_test]
    async fn save_distractions_short_circuits_when_bridge_absent() {
        let result = save_distractions(vec![]).await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    #[wasm_bindgen_test]
    async fn save_distractions_signature_pinned() {
        async fn assert_signature(d: Vec<Distraction>) -> Result<(), BridgeError> {
            save_distractions(d).await
        }
        let _ = assert_signature(vec![]).await;
    }

    #[wasm_bindgen_test]
    async fn load_distractions_short_circuits_when_bridge_absent() {
        let result = load_distractions().await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    #[wasm_bindgen_test]
    async fn load_distractions_signature_pinned() {
        async fn assert_signature() -> Result<Vec<Distraction>, BridgeError> {
            load_distractions().await
        }
        let _ = assert_signature().await;
    }
}
