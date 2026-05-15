use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
#[cfg(target_os = "macos")]
use std::thread;
use std::time::{Duration, Instant};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

mod exports;
mod helpers;

// `BridgeError` — typed return variant for every Tauri command.
//
// Spec 001-leptos-migration §Phase 1A T025; data-model.md §`BridgeError`.
// Mirrors `presto-web/src/bridge/error.rs` byte-for-byte on the wire so a
// `Result<T, BridgeError>` round-trips through `invoke()` without a
// translation layer.
//
// Wire form: externally-tagged JSON via
// `#[serde(tag = "kind", rename_all = "snake_case")]`. The
// `bridge_error_serde_roundtrip_*` test suite (see `mod tests` below) pins
// each variant's serialised bytes; any divergence between this mirror and
// the Leptos-side definition fails both crates' tests at once.
//
// Mapping strategy (T027 mechanical rewrite of every legacy
// `.map_err(|e| format!(…))` call site): default to `Internal { msg }`
// when the call site has no semantic context; tighten to `NotFound`,
// `InvalidArgument`, or `NotAuthenticated` where it does. Keeps spec
// FR-008's compile-time-mismatch promise load-bearing.
//
// Note: `BridgeUnavailable` is intentionally part of the same enum even
// though the Tauri side never produces it. Both crates share one type
// definition so the wire format cannot drift; the variant is consumed
// solely by the Leptos wrappers when `window.__TAURI_INTERNALS__` is
// absent.
// `BridgeError`, `TimerMode`, and `SessionType` live in the shared
// `presto-ipc` crate so a wire-shape change can't drift between the
// Tauri backend and the Leptos frontend. Re-exported here for
// in-crate path stability.
pub use presto_ipc::{BridgeError, SessionType, TimerMode};

// `From<String> for BridgeError` lives in the shared crate (orphan
// rules — both `String` and `BridgeError` are foreign here).

// Type alias for the app handle to avoid generic complexity
type AppHandle = tauri::AppHandle<tauri::Wry>;

/// Tauri managed state holding the in-memory settings cache.
struct SettingsState(Mutex<AppSettings>);

static ACTIVITY_MONITOR: Mutex<Option<ActivityMonitor>> = Mutex::new(None);

static SHORTCUT_DEBOUNCE: LazyLock<Mutex<HashMap<String, Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

struct ActivityMonitor {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    last_activity: Arc<Mutex<Instant>>,
    is_monitoring: Arc<Mutex<bool>>,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    app_handle: AppHandle,
    inactivity_threshold: Arc<Mutex<Duration>>,
}

// Domain records moved to `presto_ipc` (Phase F). Backend keeps
// `PomodoroSession` as a local alias for the shared `Session` to
// minimise call-site churn (helpers.rs uses `PomodoroSession`
// pervasively).
pub use presto_ipc::{
    Distraction, DistractionParentRef, ManualSession, QuickLog, Session as PomodoroSession,
    SessionTag, Tag, Task,
};

// Settings tree (`AppSettings`, `AppSettingsOnDisk` shim, nested
// settings substructs) moved to `presto_ipc::settings` in Phase F.
// The `From<AppSettingsOnDisk> for AppSettings` impl moved with the
// types so the legacy `hide_status_bar → status_bar_display`
// migration logic stays single-sourced.
pub use presto_ipc::{
    default_max_session_time, default_weekly_goal, AdvancedSettings, AppearanceSettings,
    NotificationSettings, Settings as AppSettings, SettingsOnDisk as AppSettingsOnDisk,
    ShortcutSettings, StatusBarDisplay, TimerSettings,
};

/// Loads settings synchronously from disk, falling back to defaults on any error.
fn load_settings_sync(app: &AppHandle) -> AppSettings {
    let Ok(app_data_dir) = app.path().app_data_dir() else {
        return AppSettings::default();
    };
    helpers::read_settings_from(&app_data_dir).unwrap_or_default()
}

fn should_debounce_shortcut(action: &str) -> bool {
    let mut map = helpers::lock_or_recover(&SHORTCUT_DEBOUNCE);
    helpers::is_debounced(&mut map, action, Instant::now(), Duration::from_millis(500))
}

fn get_app_data_dir(app: &AppHandle) -> Result<std::path::PathBuf, BridgeError> {
    app.path()
        .app_data_dir()
        .map_err(|e| BridgeError::Internal {
            msg: format!("Failed to get app data directory: {e}"),
        })
}

impl ActivityMonitor {
    #[cfg(target_os = "macos")]
    fn new(app_handle: AppHandle, timeout_seconds: u64) -> Self {
        Self {
            last_activity: Arc::new(Mutex::new(Instant::now())),
            is_monitoring: Arc::new(Mutex::new(false)),
            app_handle,
            inactivity_threshold: Arc::new(Mutex::new(Duration::from_secs(timeout_seconds))),
        }
    }

    #[cfg(target_os = "macos")]
    fn start_monitoring(&self) {
        {
            let mut is_monitoring = helpers::lock_or_recover(&self.is_monitoring);
            if *is_monitoring {
                return;
            }
            *is_monitoring = true;
        }

        let last_activity = Arc::clone(&self.last_activity);
        let is_monitoring_clone = Arc::clone(&self.is_monitoring);
        let inactivity_threshold = Arc::clone(&self.inactivity_threshold);
        let app_handle = self.app_handle.clone();

        thread::spawn(move || {
            let mut prev_active = false;
            loop {
                {
                    let monitoring = helpers::lock_or_recover(&is_monitoring_clone);
                    if !*monitoring {
                        break;
                    }
                }

                let threshold = {
                    let threshold_guard = helpers::lock_or_recover(&inactivity_threshold);
                    *threshold_guard
                };

                let has_activity = Self::check_system_activity();

                if has_activity {
                    {
                        let mut last = helpers::lock_or_recover(&last_activity);
                        *last = Instant::now();
                    }
                    // Only emit on idle→active transition to avoid flooding IPC
                    if !prev_active {
                        let _ = app_handle.emit("user-activity", ());
                    }
                    prev_active = true;
                } else {
                    let elapsed = {
                        let last = helpers::lock_or_recover(&last_activity);
                        last.elapsed()
                    };

                    // Only emit on active→idle transition; without resetting the
                    // timer, prev_active gates further emissions until activity resumes.
                    if elapsed >= threshold && prev_active {
                        let _ = app_handle.emit("user-inactivity", ());
                        prev_active = false;
                    }
                }

                thread::sleep(Duration::from_millis(500));
            }
        });
    }

    #[cfg(target_os = "macos")]
    fn check_system_activity() -> bool {
        Self::get_system_idle_time() < 1.0
    }

    #[cfg(target_os = "macos")]
    #[allow(unsafe_code)]
    fn get_system_idle_time() -> f64 {
        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGEventSourceSecondsSinceLastEventType(state_id: u32, event_type: u32) -> f64;
        }

        // SAFETY: CGEventSourceSecondsSinceLastEventType is a pure read-only CoreGraphics
        // function. kCGEventSourceStateCombinedSessionState=0, kCGAnyInputEventType=0xFFFF_FFFF.
        unsafe { CGEventSourceSecondsSinceLastEventType(0, 0xFFFF_FFFF) }
    }

    fn stop_monitoring(&self) {
        let mut is_monitoring = helpers::lock_or_recover(&self.is_monitoring);
        *is_monitoring = false;
    }

    fn update_threshold(&self, timeout_seconds: u64) {
        let mut threshold = helpers::lock_or_recover(&self.inactivity_threshold);
        *threshold = Duration::from_secs(timeout_seconds);
    }
}

#[tauri::command]
#[specta::specta]
async fn start_activity_monitoring(
    app: AppHandle,
    timeout_seconds: u64,
) -> Result<(), BridgeError> {
    #[cfg(target_os = "macos")]
    {
        let mut monitor = helpers::lock_or_recover(&ACTIVITY_MONITOR);
        if monitor.is_none() {
            *monitor = Some(ActivityMonitor::new(app, timeout_seconds));
        }
        if let Some(ref m) = *monitor {
            m.start_monitoring();
        }
        drop(monitor);
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, timeout_seconds);
        Ok(())
    }
}

#[tauri::command]
#[specta::specta]
async fn stop_activity_monitoring() -> Result<(), BridgeError> {
    {
        let monitor = helpers::lock_or_recover(&ACTIVITY_MONITOR);
        if let Some(ref m) = *monitor {
            m.stop_monitoring();
        }
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn update_activity_timeout(timeout_seconds: u64) -> Result<(), BridgeError> {
    let monitor = helpers::lock_or_recover(&ACTIVITY_MONITOR);
    monitor.as_ref().map_or(Ok(()), |m| {
        m.update_threshold(timeout_seconds);
        Ok(())
    })
}

#[tauri::command]
#[specta::specta]
async fn save_session_data(session: PomodoroSession, app: AppHandle) -> Result<(), BridgeError> {
    let app_data_dir = get_app_data_dir(&app)?;

    helpers::write_session_to(&app_data_dir, &session)?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn load_session_data(app: AppHandle) -> Result<Option<PomodoroSession>, BridgeError> {
    let app_data_dir = get_app_data_dir(&app)?;
    helpers::read_session_from(&app_data_dir).map_err(BridgeError::from)
}

#[tauri::command]
#[specta::specta]
async fn save_tasks(tasks: Vec<Task>, app: AppHandle) -> Result<(), BridgeError> {
    let app_data_dir = get_app_data_dir(&app)?;

    helpers::write_tasks_to(&app_data_dir, &tasks)?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn load_tasks(app: AppHandle) -> Result<Vec<Task>, BridgeError> {
    let app_data_dir = get_app_data_dir(&app)?;
    helpers::read_tasks_from(&app_data_dir).map_err(BridgeError::from)
}

#[tauri::command]
#[specta::specta]
async fn get_stats_history(app: AppHandle) -> Result<Vec<PomodoroSession>, BridgeError> {
    let app_data_dir = get_app_data_dir(&app)?;
    helpers::read_history_from(&app_data_dir).map_err(BridgeError::from)
}

#[tauri::command]
#[specta::specta]
async fn save_daily_stats(session: PomodoroSession, app: AppHandle) -> Result<(), BridgeError> {
    let app_data_dir = get_app_data_dir(&app)?;
    helpers::append_daily_stats_to(&app_data_dir, &session).map_err(BridgeError::from)
}

// `session_mode: TimerMode` (was `String`) per spec 001 T027 — closed-domain
// enum tightening. Wire format unchanged: camelCase ("focus"/"break"/
// "longBreak") via `#[serde(rename_all = "camelCase")]` on `TimerMode`.
#[tauri::command]
#[specta::specta]
async fn update_tray_icon(
    app: AppHandle,
    timer_text: String,
    is_running: bool,
    session_mode: TimerMode,
    current_session: u32,
    total_sessions: u32,
    mode_icon: Option<String>,
) -> Result<(), BridgeError> {
    use std::sync::{Arc, Mutex};

    // Use Arc<Mutex<Result<(), BridgeError>>> to capture the result from
    // the main thread.
    let result: Arc<Mutex<Result<(), BridgeError>>> = Arc::new(Mutex::new(Ok(())));
    let result_clone = Arc::clone(&result);

    let app_clone = app.clone();

    // Move the operation to the main thread using Tauri's app handle
    // This ensures macOS tray operations run on the main thread
    app.run_on_main_thread(move || {
        let mut result_guard = helpers::lock_or_recover(&result_clone);
        *result_guard = (|| -> Result<(), BridgeError> {
            if let Some(tray) = app_clone.tray_by_id("main") {
                let icon = mode_icon.unwrap_or_else(|| {
                    // Glyphs lifted from ramazanberkozbek/presto — render
                    // as monospace text in the macOS menu bar.
                    match session_mode {
                        // ◉ filled circle = focus
                        TimerMode::Focus => "\u{25c9}",
                        // ☼ sun = short break
                        TimerMode::Break => "\u{263c}",
                        // ☾ moon = long break
                        TimerMode::LongBreak => "\u{263e}",
                    }
                    .to_string()
                });

                let status = if is_running { "Running" } else { "Paused" };
                let title = format!("{icon} {timer_text}");
                tray.set_title(Some(title))
                    .map_err(|e| BridgeError::Internal {
                        msg: format!("Failed to set title: {e}"),
                    })?;

                let tooltip = match session_mode {
                    TimerMode::Focus => {
                        format!("Presto - Session {current_session}/{total_sessions} ({status})")
                    }
                    TimerMode::LongBreak => format!("Presto - Long Break ({status})"),
                    TimerMode::Break => format!("Presto - Short Break ({status})"),
                };

                tray.set_tooltip(Some(tooltip))
                    .map_err(|e| BridgeError::Internal {
                        msg: format!("Failed to set tooltip: {e}"),
                    })?;
            }
            Ok(())
        })();
    })
    .map_err(|e| BridgeError::Internal {
        msg: format!("Failed to run on main thread: {e}"),
    })?;

    // Extract the result from the mutex (named binding required by borrow checker:
    // the temporary MutexGuard must drop before `result` does).
    let final_result = helpers::lock_or_recover(&result).clone();
    final_result
}

fn emit_tray_and_show(app: &AppHandle, event: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit(event, ());
    }
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        show_app_window(app_clone).await;
    });
}

#[allow(clippy::unused_async)] // awaits run_on_main_thread on macOS
async fn show_app_window(app: AppHandle) {
    let settings = helpers::lock_or_recover(&app.state::<SettingsState>().0).clone();
    if settings.hide_icon_on_close {
        #[cfg(target_os = "macos")]
        {
            let _ = app.run_on_main_thread(move || {
                set_dock_visibility_native(true);
            });
        }
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
#[specta::specta]
async fn save_settings(settings: AppSettings, app: AppHandle) -> Result<(), BridgeError> {
    let app_data_dir = get_app_data_dir(&app)?;

    helpers::write_settings_to(&app_data_dir, &settings)?;

    *helpers::lock_or_recover(&app.state::<SettingsState>().0) = settings;

    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn load_settings(app: AppHandle) -> Result<AppSettings, BridgeError> {
    let app_data_dir = get_app_data_dir(&app)?;
    helpers::read_settings_from(&app_data_dir).map_err(|e| BridgeError::Internal {
        msg: format!("Failed to read settings: {e}"),
    })
}

/// Project a `ShortcutSettings` into the (action, parsed-Shortcut) pairs
/// the registration loop installs. Pure helper extracted so the
/// 2-tuple iteration order + `Option::None` skip + parse-error shape
/// (`BridgeError::Internal { msg }` mentioning the action name) is unit
/// testable without an `AppHandle`. Iteration order is the canonical
/// wire-name order: `start-stop`, `reset`, `skip`, `abort` (feature 007).
fn parse_shortcut_bindings(
    shortcuts: &ShortcutSettings,
) -> Result<Vec<(&'static str, Shortcut)>, BridgeError> {
    let bindings: [(&'static str, &Option<String>); 4] = [
        ("start-stop", &shortcuts.start_stop),
        ("reset", &shortcuts.reset),
        ("skip", &shortcuts.skip),
        // Feature 007 (T023, FR-018): abort joins the registration loop.
        // Wire name is kebab-case to match the existing sibling slots
        // and the frontend listener at `src/src/app.rs:613-624`.
        ("abort", &shortcuts.abort),
    ];
    let mut out: Vec<(&'static str, Shortcut)> = Vec::with_capacity(4);
    for (action, shortcut_str) in bindings {
        if let Some(ref shortcut_str) = *shortcut_str {
            let shortcut: Shortcut = shortcut_str.parse().map_err(|e| BridgeError::Internal {
                msg: format!("Invalid {action} shortcut '{shortcut_str}': {e}"),
            })?;
            out.push((action, shortcut));
        }
    }
    Ok(out)
}

#[tauri::command]
#[specta::specta]
async fn register_global_shortcuts(
    app: AppHandle,
    shortcuts: ShortcutSettings,
) -> Result<(), BridgeError> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| BridgeError::Internal {
            msg: format!("Failed to unregister shortcuts: {e}"),
        })?;

    let bindings = parse_shortcut_bindings(&shortcuts)?;
    for (action, shortcut) in bindings {
        let app_handle = app.clone();
        let action_owned = action.to_string();
        app.global_shortcut()
            .on_shortcut(shortcut, move |_app, _shortcut, _event| {
                if !should_debounce_shortcut(&action_owned) {
                    let _ = app_handle.emit("global-shortcut", action_owned.as_str());
                }
            })
            .map_err(|e| BridgeError::Internal {
                msg: format!("Failed to register {action} shortcut: {e}"),
            })?;
    }

    // Emit an event to the frontend to update local shortcuts as well
    app.emit("shortcuts-updated", &shortcuts)
        .map_err(|e| BridgeError::Internal {
            msg: format!("Failed to emit shortcuts update: {e}"),
        })?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn reset_all_data(app: AppHandle) -> Result<(), BridgeError> {
    let app_data_dir = get_app_data_dir(&app)?;

    helpers::delete_all_data_in(&app_data_dir)?;

    *helpers::lock_or_recover(&app.state::<SettingsState>().0) = AppSettings::default();

    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn enable_autostart(app: AppHandle) -> Result<(), BridgeError> {
    app.autolaunch()
        .enable()
        .map_err(|e| BridgeError::Internal {
            msg: format!("Failed to enable autostart: {e}"),
        })
}

#[tauri::command]
#[specta::specta]
async fn disable_autostart(app: AppHandle) -> Result<(), BridgeError> {
    app.autolaunch()
        .disable()
        .map_err(|e| BridgeError::Internal {
            msg: format!("Failed to disable autostart: {e}"),
        })
}

#[tauri::command]
#[specta::specta]
async fn is_autostart_enabled(app: AppHandle) -> Result<bool, BridgeError> {
    app.autolaunch()
        .is_enabled()
        .map_err(|e| BridgeError::Internal {
            msg: format!("Failed to check autostart status: {e}"),
        })
}

#[tauri::command]
#[specta::specta]
async fn save_manual_sessions(
    sessions: Vec<ManualSession>,
    app: AppHandle,
) -> Result<(), BridgeError> {
    let app_data_dir = get_app_data_dir(&app)?;

    helpers::write_manual_sessions_to(&app_data_dir, &sessions)?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn load_manual_sessions(app: AppHandle) -> Result<Vec<ManualSession>, BridgeError> {
    let app_data_dir = get_app_data_dir(&app)?;
    helpers::read_manual_sessions_from(&app_data_dir).map_err(BridgeError::from)
}

// ── Feature 006: Quick logs + Distractions ──────────────────────────────────
//
// Tauri boundary-validation per FR-022 lives in two pure helpers
// (`validate_quick_logs`, `validate_distractions`) so the contract is
// directly unit-testable without a Tauri runtime. The command bodies
// are thin glue over validator + helpers IO.

/// Validates a `Vec<QuickLog>` at the Tauri boundary per
/// `specs/006-timer-controls-quicklog-distractions/contracts/persistence-commands.md`.
///
/// Title length (chars) ∈ 1..=120; `elapsed_minutes` ∈ 1..=720. First
/// failure short-circuits with `BridgeError::InvalidArgument`. Field
/// names use the camelCase wire shape so the frontend's `match`
/// against the wire bytes lands cleanly.
fn validate_quick_logs(logs: &[QuickLog]) -> Result<(), BridgeError> {
    for log in logs {
        let title_len = log.title.chars().count();
        if !(1..=120).contains(&title_len) {
            return Err(BridgeError::InvalidArgument {
                field: "title".to_string(),
                reason: format!("title length {title_len} not in 1..=120"),
            });
        }
        if !(1..=720).contains(&log.elapsed_minutes) {
            return Err(BridgeError::InvalidArgument {
                field: "elapsedMinutes".to_string(),
                reason: format!("elapsedMinutes {} not in 1..=720", log.elapsed_minutes),
            });
        }
    }
    Ok(())
}

/// Validates a `Vec<Distraction>` at the Tauri boundary per
/// `specs/006-timer-controls-quicklog-distractions/contracts/persistence-commands.md`.
///
/// `note` length ∈ 1..=120; if `parent_ref.parent_title.is_some()`,
/// the title must also fit in 1..=120 chars.
fn validate_distractions(entries: &[Distraction]) -> Result<(), BridgeError> {
    for entry in entries {
        let note_len = entry.note.chars().count();
        if !(1..=120).contains(&note_len) {
            return Err(BridgeError::InvalidArgument {
                field: "note".to_string(),
                reason: format!("note length {note_len} not in 1..=120"),
            });
        }
        if let Some(parent) = entry.parent_ref.as_ref() {
            if let Some(parent_title) = parent.parent_title.as_ref() {
                let len = parent_title.chars().count();
                if !(1..=120).contains(&len) {
                    return Err(BridgeError::InvalidArgument {
                        field: "parentRef.parentTitle".to_string(),
                        reason: format!("parent_title length {len} not in 1..=120"),
                    });
                }
            }
        }
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn save_quick_logs(quick_logs: Vec<QuickLog>, app: AppHandle) -> Result<(), BridgeError> {
    validate_quick_logs(&quick_logs)?;
    let app_data_dir = get_app_data_dir(&app)?;
    helpers::write_quick_logs_to(&app_data_dir, &quick_logs)?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn load_quick_logs(app: AppHandle) -> Result<Vec<QuickLog>, BridgeError> {
    let app_data_dir = get_app_data_dir(&app)?;
    helpers::read_quick_logs_from(&app_data_dir).map_err(BridgeError::from)
}

#[tauri::command]
#[specta::specta]
async fn save_distractions(
    distractions: Vec<Distraction>,
    app: AppHandle,
) -> Result<(), BridgeError> {
    validate_distractions(&distractions)?;
    let app_data_dir = get_app_data_dir(&app)?;
    helpers::write_distractions_to(&app_data_dir, &distractions)?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn load_distractions(app: AppHandle) -> Result<Vec<Distraction>, BridgeError> {
    let app_data_dir = get_app_data_dir(&app)?;
    helpers::read_distractions_from(&app_data_dir).map_err(BridgeError::from)
}

/// Builds and runs the Tauri application.
///
/// # Panics
///
/// Panics if the Tauri runtime fails to initialize or if the GUI cannot be
/// constructed. The native runtime fails fast in this case because there is
/// nothing the rest of the app can do without it.
/// Construct the `tauri-specta` Builder collecting every
/// `#[tauri::command] #[specta::specta]` handler. Shared between the
/// release runtime (`run()`) and the CI bindings-drift test at
/// `tests/bindings_export.rs` so the test exports the same surface
/// the binary registers — no opportunity for the two to drift.
#[must_use]
pub fn build_specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        save_session_data,
        load_session_data,
        save_tasks,
        load_tasks,
        get_stats_history,
        save_daily_stats,
        update_tray_icon,
        update_tray_menu,
        save_settings,
        load_settings,
        register_global_shortcuts,
        reset_all_data,
        start_activity_monitoring,
        stop_activity_monitoring,
        update_activity_timeout,
        enable_autostart,
        disable_autostart,
        is_autostart_enabled,
        save_manual_sessions,
        load_manual_sessions,
        save_quick_logs,
        load_quick_logs,
        save_distractions,
        load_distractions,
        load_tags,
        save_tag,
        delete_tag,
        add_session_tag,
        export_sessions_xlsx,
        export_sessions_csv,
        dialog_save,
        dialog_ask,
    ])
}

/// Tauri runtime entrypoint. Builds the typed command Builder via
/// `tauri-specta`, exports the TS bindings in debug builds, then
/// hands `invoke_handler` to the Tauri `Builder` for the live
/// runtime.
///
/// # Panics
/// Panics if the underlying Tauri `Builder::build` fails (e.g. a
/// missing `tauri.conf.json` entry, an invalid bundle config, or a
/// plugin init error). The native runtime cannot recover from this
/// state — the binary fails fast at startup rather than hand a
/// half-constructed app to the user.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[allow(
    clippy::too_many_lines,
    reason = "Tauri builder uses a method-chain DSL — splitting it loses readability."
)]
#[allow(
    clippy::large_stack_frames,
    reason = "tauri::generate_context!() expands to a large constant; refactoring is not under our control."
)]
pub fn run() {
    let specta_builder = build_specta_builder();
    // Re-emit `src/bindings/tauri.ts` on every dev launch so the
    // generated bindings stay in step with the Rust source while the
    // user is hacking. The release binary skips this hop (no source
    // tree present at install time). The CI gate at
    // `tests/bindings_export.rs` is the authoritative drift check.
    #[cfg(debug_assertions)]
    {
        let exporter = specta_typescript::Typescript::default()
            .bigint(specta_typescript::BigIntExportBehavior::String);
        let _ = specta_builder.export(exporter, "../src/bindings/tauri.ts");
    }

    tauri::async_runtime::block_on(async {
        tauri::Builder::default()
            .plugin(
                tauri_plugin_log::Builder::new()
                    .level(if cfg!(debug_assertions) {
                        log::LevelFilter::Debug
                    } else {
                        log::LevelFilter::Info
                    })
                    .targets([
                        tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                        tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                            file_name: None,
                        }),
                        tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Webview),
                    ])
                    .build(),
            )
            .plugin(tauri_plugin_opener::init())
            .plugin(tauri_plugin_global_shortcut::Builder::new().build())
            .plugin(tauri_plugin_dialog::init())
            .plugin(tauri_plugin_notification::init())
            .plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                None,
            ))
            .plugin(tauri_plugin_updater::Builder::new().build())
            .plugin(tauri_plugin_process::init())
            .invoke_handler(specta_builder.invoke_handler())
            .setup(|app| {
                let initial_settings = load_settings_sync(app.handle());
                app.manage(SettingsState(Mutex::new(initial_settings)));

                let show_item = MenuItem::with_id(app, "show", "Show Presto", true, None::<&str>)?;
                let start_session_item =
                    MenuItem::with_id(app, "start_session", "Start Session", false, None::<&str>)?;
                let pause_item = MenuItem::with_id(app, "pause", "Pause", false, None::<&str>)?;
                let skip_item =
                    MenuItem::with_id(app, "skip", "Skip Session", false, None::<&str>)?;
                let cancel_item = MenuItem::with_id(app, "cancel", "Cancel", false, None::<&str>)?;
                let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
                let menu = Menu::with_items(
                    app,
                    &[
                        &show_item,
                        &start_session_item,
                        &pause_item,
                        &skip_item,
                        &cancel_item,
                        &quit_item,
                    ],
                )?;

                let app_handle = app.handle().clone();
                let app_handle_for_click = app_handle.clone();

                // macOS: TrayIconBuilder does NOT auto-load default_window_icon.
                // Without an explicit icon AND with no title, the NSStatusItem
                // renders at zero width and is invisible (tauri-apps/tauri#11931).
                // We seed an initial title so the countdown text is present from
                // boot — no icon is set so macOS doesn't render a duplicate
                // template silhouette next to the emoji glyph in the title.
                // ◉ glyph matches the focus icon emitted by `tray.rs` so
                // the initial frame doesn't flash a different symbol before
                // the first `update_tray_icon` payload arrives.
                let initial_tray_title = "\u{25c9} 25:00 (0/10)";
                let tray_builder = TrayIconBuilder::with_id("main")
                    .menu(&menu)
                    .show_menu_on_left_click(true)
                    .title(initial_tray_title)
                    .on_menu_event(move |_tray, event| match event.id.as_ref() {
                        "show" => {
                            let app_clone = app_handle.clone();
                            tauri::async_runtime::spawn(async move {
                                show_app_window(app_clone).await;
                            });
                        }
                        "start_session" => {
                            emit_tray_and_show(&app_handle, "tray-start-session");
                        }
                        "pause" => {
                            emit_tray_and_show(&app_handle, "tray-pause");
                        }
                        "skip" => {
                            emit_tray_and_show(&app_handle, "tray-skip");
                        }
                        "cancel" => {
                            emit_tray_and_show(&app_handle, "tray-cancel");
                        }
                        "quit" => {
                            app_handle.exit(0);
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(move |_tray, event| {
                        if let TrayIconEvent::Click { .. } = event {
                            let app_clone = app_handle_for_click.clone();
                            tauri::async_runtime::spawn(async move {
                                show_app_window(app_clone).await;
                            });
                        }
                    });

                let _tray = tray_builder.build(app)?;

                if let Some(window) = app.get_webview_window("main") {
                    let app_handle_for_close = app.handle().clone();
                    window.on_window_event(move |event| {
                        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                            api.prevent_close();

                            let settings = helpers::lock_or_recover(
                                &app_handle_for_close.state::<SettingsState>().0,
                            )
                            .clone();
                            let app_handle_clone = app_handle_for_close.clone();
                            tauri::async_runtime::spawn(async move {
                                if let Some(window) = app_handle_clone.get_webview_window("main") {
                                    let _ = window.hide();
                                    if settings.hide_icon_on_close {
                                        #[cfg(target_os = "macos")]
                                        {
                                            let _ =
                                                app_handle_clone.run_on_main_thread(move || {
                                                    set_dock_visibility_native(false);
                                                });
                                        }
                                    }
                                }
                            });
                        }
                    });
                }

                let app_handle_for_shortcuts = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    match load_settings(app_handle_for_shortcuts.clone()).await {
                        Ok(settings) => {
                            if let Err(e) = register_global_shortcuts(
                                app_handle_for_shortcuts,
                                settings.shortcuts,
                            )
                            .await
                            {
                                log::error!("Failed to register global shortcuts on startup: {e}");
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to load settings on startup: {e}");
                            let default_settings = AppSettings::default();
                            if let Err(e) = register_global_shortcuts(
                                app_handle_for_shortcuts,
                                default_settings.shortcuts,
                            )
                            .await
                            {
                                log::error!("Failed to register default global shortcuts: {e}");
                            }
                        }
                    }
                });

                Ok(())
            })
            .build(tauri::generate_context!())
            .expect("error while running tauri application")
            .run(|app_handle, event| match event {
                #[cfg(target_os = "macos")]
                tauri::RunEvent::Reopen { .. } => {
                    let app_handle_clone = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        show_app_window(app_handle_clone).await;
                    });
                }
                _ => {
                    let _ = app_handle;
                }
            });
    });
}

#[tauri::command]
#[specta::specta]
async fn load_tags(app: AppHandle) -> Result<Vec<Tag>, BridgeError> {
    let app_data_dir = get_app_data_dir(&app)?;
    helpers::read_tags_from(&app_data_dir).map_err(BridgeError::from)
}

#[tauri::command]
#[specta::specta]
async fn save_tag(tag: Tag, app: AppHandle) -> Result<(), BridgeError> {
    let app_data_dir = get_app_data_dir(&app)?;
    helpers::upsert_tag_in(&app_data_dir, tag).map_err(BridgeError::from)
}

#[tauri::command]
#[specta::specta]
async fn delete_tag(tag_id: String, app: AppHandle) -> Result<(), BridgeError> {
    let app_data_dir = get_app_data_dir(&app)?;
    helpers::delete_tag_in(&app_data_dir, &tag_id).map_err(BridgeError::from)
}

#[tauri::command]
#[specta::specta]
async fn add_session_tag(session_tag: SessionTag, app: AppHandle) -> Result<(), BridgeError> {
    let app_data_dir = get_app_data_dir(&app)?;
    helpers::append_session_tag_in(&app_data_dir, session_tag).map_err(BridgeError::from)
}

// `current_mode: TimerMode` (was `String`) per spec 001 T027 — closed-domain
// enum tightening. Wire format unchanged: camelCase strings.
#[tauri::command]
#[specta::specta]
async fn update_tray_menu(
    app: AppHandle,
    is_running: bool,
    is_paused: bool,
    current_mode: TimerMode,
) -> Result<(), BridgeError> {
    let tray = app.tray_by_id("main");

    if let Some(tray) = tray {
        let show_item = MenuItem::with_id(&app, "show", "Show Presto", true, None::<&str>)
            .map_err(|e| BridgeError::Internal {
                msg: format!("Failed to create show item: {e}"),
            })?;

        // Start Session: enabled only if not running
        let start_session_item = MenuItem::with_id(
            &app,
            "start_session",
            "Start Session",
            !is_running,
            None::<&str>,
        )
        .map_err(|e| BridgeError::Internal {
            msg: format!("Failed to create start session item: {e}"),
        })?;

        // Pause: enabled only if running and not paused
        let pause_item = MenuItem::with_id(
            &app,
            "pause",
            "Pause",
            is_running && !is_paused,
            None::<&str>,
        )
        .map_err(|e| BridgeError::Internal {
            msg: format!("Failed to create pause item: {e}"),
        })?;

        // Skip: enabled only if running
        let skip_item = MenuItem::with_id(&app, "skip", "Skip Session", is_running, None::<&str>)
            .map_err(|e| BridgeError::Internal {
            msg: format!("Failed to create skip item: {e}"),
        })?;

        // Cancel: enabled if in focus mode, disabled in break/longBreak (undo)
        let cancel_text = if matches!(current_mode, TimerMode::Focus) {
            "Cancel"
        } else {
            "Cancel Last"
        };
        let cancel_item = MenuItem::with_id(&app, "cancel", cancel_text, true, None::<&str>)
            .map_err(|e| BridgeError::Internal {
                msg: format!("Failed to create cancel item: {e}"),
            })?;

        let quit_item =
            MenuItem::with_id(&app, "quit", "Quit", true, None::<&str>).map_err(|e| {
                BridgeError::Internal {
                    msg: format!("Failed to create quit item: {e}"),
                }
            })?;

        let new_menu = Menu::with_items(
            &app,
            &[
                &show_item,
                &start_session_item,
                &pause_item,
                &skip_item,
                &cancel_item,
                &quit_item,
            ],
        )
        .map_err(|e| BridgeError::Internal {
            msg: format!("Failed to create menu: {e}"),
        })?;

        tray.set_menu(Some(new_menu))
            .map_err(|e| BridgeError::Internal {
                msg: format!("Failed to set tray menu: {e}"),
            })?;
    }

    Ok(())
}

// `export_sessions_xlsx` — write a workbook of manual sessions to
// `path` using `rust_xlsxwriter` (write-only; we never read .xlsx).
#[tauri::command]
#[specta::specta]
async fn export_sessions_xlsx(
    path: String,
    sessions: Vec<ManualSession>,
) -> Result<(), BridgeError> {
    exports::export(std::path::Path::new(&path), &sessions)
}

// `export_sessions_csv` — write the same column schema as the xlsx
// export to `path` as RFC 4180 CSV. Used by the daily view's export
// button (the xlsx wrapper is retained for callers that prefer
// spreadsheets).
#[tauri::command]
#[specta::specta]
async fn export_sessions_csv(
    path: String,
    sessions: Vec<ManualSession>,
) -> Result<(), BridgeError> {
    exports::export_csv(std::path::Path::new(&path), &sessions)
}

// Dialog plugin wrappers.
//
// The frontend used to call `plugin:dialog|save` / `plugin:dialog|ask`
// directly through the raw `invoke` bridge, hand-rolling the JSON
// envelope. That had no compile-time contract against the plugin's
// signature — the dialog plugin expects `{ options: SaveDialogOptions }`
// but our wrapper sent the fields flat, and Serde silently bound
// nothing into the missing field. Result: the save dialog never
// opened.
//
// Wrapping the plugin calls in our own typed commands moves the wire
// contract under the `tauri-specta` bindings-drift test
// (`tests/bindings_export.rs`), so any future signature change between
// the frontend's `commands::dialog_*` wrappers and these handlers
// fails CI loud instead of silently dropping calls.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DialogFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

#[tauri::command]
#[specta::specta]
async fn dialog_save(
    window: tauri::Window,
    default_path: Option<String>,
    filters: Vec<DialogFilter>,
) -> Result<Option<String>, BridgeError> {
    use tauri_plugin_dialog::DialogExt;
    let mut builder = window.dialog().file().set_parent(&window);
    if let Some(p) = default_path {
        builder = builder.set_file_name(p);
    }
    for filter in &filters {
        let exts: Vec<&str> = filter.extensions.iter().map(String::as_str).collect();
        builder = builder.add_filter(&filter.name, &exts);
    }
    // `blocking_save_file` is safe inside a `#[tauri::command]` because
    // Tauri runs commands on a dedicated worker thread — same pattern
    // the dialog plugin's own `save` command uses (commands.rs:231).
    let path = builder.blocking_save_file();
    Ok(path.map(|p| p.to_string()))
}

#[tauri::command]
#[specta::specta]
async fn dialog_ask(
    window: tauri::Window,
    message: String,
    title: String,
) -> Result<bool, BridgeError> {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
    let confirmed = window
        .dialog()
        .message(message)
        .title(title)
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::YesNo)
        .blocking_show();
    Ok(confirmed)
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn set_dock_visibility_native(visible: bool) {
    use cocoa::appkit::{NSApp, NSApplication, NSApplicationActivationPolicy};
    use cocoa::base::nil;

    // SAFETY: NSApp() returns a raw pointer that is nil if no shared NSApplication
    // exists. We null-check against `nil` before invoking setActivationPolicy_, and
    // this entire function is only invoked from the main thread via run_on_main_thread,
    // satisfying AppKit's main-thread requirement for NSApplication mutation.
    unsafe {
        let app = NSApp();
        if app != nil {
            let policy = if visible {
                NSApplicationActivationPolicy::NSApplicationActivationPolicyRegular
            } else {
                NSApplicationActivationPolicy::NSApplicationActivationPolicyAccessory
            };

            app.setActivationPolicy_(policy);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        default_weekly_goal, AppSettings, ManualSession, PomodoroSession, SessionTag,
        StatusBarDisplay, Tag, Task,
    };

    #[test]
    fn bundle_config_has_at_least_one_icon() {
        // Guard against a future cleanup pass deleting bundle.icon, which
        // would make Manager::default_window_icon() return None and silently
        // re-break the macOS tray (see issue #40).
        let conf = include_str!("../tauri.conf.json");
        let parsed: serde_json::Value = serde_json::from_str(conf).expect("valid tauri.conf.json");
        let icons = parsed["bundle"]["icon"]
            .as_array()
            .expect("bundle.icon array");
        assert!(
            !icons.is_empty(),
            "bundle.icon must be non-empty to populate default_window_icon"
        );
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        for icon in icons {
            let icon_path = icon.as_str().expect("icon entry must be a string");
            let full_path = manifest_dir.join(icon_path);
            assert!(
                full_path.exists(),
                "bundle.icon references missing file: {icon_path}"
            );
        }
    }

    #[test]
    fn weekly_goal_default_is_125() {
        assert_eq!(default_weekly_goal(), 125);
    }

    #[test]
    fn app_settings_missing_serde_default_fields_use_defaults() {
        // Simulate an older settings JSON that predates newer #[serde(default)] fields.
        let json = r#"{
            "shortcuts": {},
            "timer": {
                "focus_duration": 25,
                "break_duration": 5,
                "long_break_duration": 20,
                "total_sessions": 10
            },
            "notifications": {
                "desktop_notifications": true,
                "sound_notifications": true,
                "auto_start_timer": true,
                "smart_pause": false,
                "smart_pause_timeout": 30
            },
            "autostart": false
        }"#;
        let s: AppSettings = serde_json::from_str(json).expect("should deserialize");
        let defaults = AppSettings::default();
        assert_eq!(s.hide_icon_on_close, defaults.hide_icon_on_close);
        // Phase 3a T150: when neither legacy `hide_status_bar` nor new
        // `status_bar_display` is present, the field defaults to
        // `StatusBarDisplay::Default`. The legacy fallback path
        // (`hide_status_bar: true → IconOnly`) is exercised by T151's
        // `migrates_hide_status_bar_to_status_bar_display` once T152
        // lands the custom deserializer.
        assert_eq!(s.status_bar_display, defaults.status_bar_display);
        assert_eq!(s.status_bar_display, StatusBarDisplay::Default);
        assert_eq!(
            s.notifications.auto_start_focus,
            defaults.notifications.auto_start_focus
        );
        assert_eq!(
            s.notifications.allow_continuous_sessions,
            defaults.notifications.allow_continuous_sessions
        );
        assert_eq!(
            s.timer.weekly_goal_minutes,
            defaults.timer.weekly_goal_minutes
        );
        assert_eq!(s.timer.max_session_time, defaults.timer.max_session_time);
        assert_eq!(s.advanced.debug_mode, defaults.advanced.debug_mode);
        assert_eq!(s.appearance.theme, defaults.appearance.theme);
        assert_eq!(s.appearance.timer_theme, defaults.appearance.timer_theme);
    }

    #[test]
    fn app_settings_legacy_hide_status_bar_migrates_to_status_bar_display() {
        // F1/M3 lockstep migration mirror — the Tauri-side companion
        // to `presto-web::managers::settings::tests::migrates_hide_status_bar_to_status_bar_display`.
        // Covers the same five cases (per data-model.md §"Settings
        // legacy migration") so a future drift on either side
        // regresses loud:
        //
        //   1. hide_status_bar:true → IconOnly
        //   2. hide_status_bar:false → Default
        //   3. status_bar_display:"icon-only" → IconOnly
        //   4. status_bar_display:"default" → Default
        //   5. neither → Default
        let make_json = |status_bar_fragment: &str| {
            format!(
                r#"{{
                    "shortcuts": {{"start_stop": null, "reset": null, "skip": null, "abort": null}},
                    "timer": {{"focus_duration": 25, "break_duration": 5,
                              "long_break_duration": 20, "total_sessions": 10}},
                    "notifications": {{"desktop_notifications": true,
                                      "sound_notifications": true,
                                      "auto_start_timer": true, "smart_pause": false,
                                      "smart_pause_timeout": 30}},
                    "autostart": false{status_bar_fragment}
                }}"#
            )
        };

        let cases: &[(&str, StatusBarDisplay)] = &[
            (r#", "hide_status_bar": true"#, StatusBarDisplay::IconOnly),
            (r#", "hide_status_bar": false"#, StatusBarDisplay::Default),
            (
                r#", "status_bar_display": "icon-only""#,
                StatusBarDisplay::IconOnly,
            ),
            (
                r#", "status_bar_display": "default""#,
                StatusBarDisplay::Default,
            ),
            ("", StatusBarDisplay::Default),
        ];

        for (fragment, expected) in cases {
            let json = make_json(fragment);
            let s: AppSettings = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("case {fragment:?} must deserialise: {e}"));
            assert_eq!(
                s.status_bar_display, *expected,
                "case {fragment:?} must yield {expected:?}",
            );
        }

        // Re-serialising any AppSettings record drops `hide_status_bar`
        // (no field for it on the struct) — verify with a
        // hide_status_bar:true round-trip.
        let json = make_json(r#", "hide_status_bar": true"#);
        let s: AppSettings = serde_json::from_str(&json).unwrap();
        let resaved = serde_json::to_string(&s).unwrap();
        assert!(
            !resaved.contains("hide_status_bar"),
            "save must drop legacy hide_status_bar field",
        );
        assert!(
            resaved.contains(r#""status_bar_display":"icon-only""#),
            "save must emit kebab-case status_bar_display",
        );
    }

    #[test]
    fn app_settings_default_has_expected_values() {
        let s = AppSettings::default();
        assert_eq!(s.timer.focus_duration, 25);
        assert_eq!(s.timer.break_duration, 5);
        assert_eq!(s.timer.long_break_duration, 20);
        assert_eq!(s.timer.total_sessions, 10);
        assert_eq!(s.timer.weekly_goal_minutes, 125);
        assert_eq!(s.timer.max_session_time, 120);
        assert!(!s.autostart);
        assert!(!s.hide_icon_on_close);
        assert_eq!(s.status_bar_display, StatusBarDisplay::Default);
        assert!(s.notifications.desktop_notifications);
        assert!(s.notifications.sound_notifications);
        assert!(s.notifications.auto_start_timer);
        assert!(!s.notifications.auto_start_focus);
        assert!(!s.notifications.allow_continuous_sessions);
        assert!(!s.notifications.smart_pause);
        assert_eq!(s.notifications.smart_pause_timeout, 30);
        assert!(!s.advanced.debug_mode);
        assert!(s.shortcuts.start_stop.is_some());
        assert!(s.shortcuts.reset.is_some());
        assert!(s.shortcuts.skip.is_some());
        assert!(s.skipped_versions.is_empty());
        // Appearance defaults: auto color-mode, espresso timer theme.
        assert_eq!(s.appearance.theme, "auto");
        assert_eq!(s.appearance.timer_theme, "espresso");
    }

    #[test]
    fn app_settings_appearance_round_trips_and_defaults_for_legacy_json() {
        // A legacy 0.4.x JSON without `appearance` or `max_session_time` must
        // deserialise to the defaults.
        let legacy = r#"{
            "shortcuts": {"start_stop": null, "reset": null, "skip": null, "abort": null},
            "timer": {"focus_duration": 25, "break_duration": 5,
                      "long_break_duration": 20, "total_sessions": 10},
            "notifications": {"desktop_notifications": true,
                              "sound_notifications": true,
                              "auto_start_timer": true, "smart_pause": false,
                              "smart_pause_timeout": 30},
            "autostart": false
        }"#;
        let s: AppSettings = serde_json::from_str(legacy).expect("legacy shape must deserialise");
        assert_eq!(s.appearance.theme, "auto");
        assert_eq!(s.appearance.timer_theme, "espresso");
        assert_eq!(s.timer.max_session_time, 120);

        // A round-trip of the default `AppSettings` must include the new fields.
        let json = serde_json::to_string(&AppSettings::default()).expect("must serialise");
        let v: serde_json::Value = serde_json::from_str(&json).expect("must parse as JSON");
        assert_eq!(v["appearance"]["theme"], "auto");
        assert_eq!(v["appearance"]["timer_theme"], "espresso");
        assert!(v["appearance"]["locale"].is_null());
        assert!(json.contains(r#""max_session_time":120"#));
        let decoded: AppSettings = serde_json::from_str(&json).expect("round-trip must succeed");
        assert_eq!(decoded.appearance.theme, "auto");
        assert_eq!(decoded.appearance.timer_theme, "espresso");
        assert_eq!(decoded.timer.max_session_time, 120);
    }

    #[test]
    fn pomodoro_session_serializes_and_deserializes() {
        let session = PomodoroSession {
            completed_pomodoros: 3,
            total_focus_time: 4500,
            current_session: 2,
            date: "Mon Jan 01 2024".to_string(),
            title: Some("Sprint planning".to_string()),
        };
        let json = serde_json::to_string(&session).unwrap();
        let parsed: PomodoroSession = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.completed_pomodoros, session.completed_pomodoros);
        assert_eq!(parsed.total_focus_time, session.total_focus_time);
        assert_eq!(parsed.current_session, session.current_session);
        assert_eq!(parsed.date, session.date);
        assert_eq!(parsed.title.as_deref(), Some("Sprint planning"));

        let session_no_title = PomodoroSession {
            completed_pomodoros: 1,
            total_focus_time: 1500,
            current_session: 1,
            date: "Tue Jan 02 2024".to_string(),
            title: None,
        };
        let json_no_title = serde_json::to_string(&session_no_title).unwrap();
        let parsed_no_title: PomodoroSession = serde_json::from_str(&json_no_title).unwrap();
        assert!(parsed_no_title.title.is_none());
    }

    #[test]
    fn tag_serializes_and_deserializes() {
        let tag = Tag {
            id: "tag-1".to_string(),
            name: "Work".to_string(),
            icon: "ri-briefcase-line".to_string(),
            color: "#3b82f6".to_string(),
            created_at: "1234567890".to_string(),
        };
        let json = serde_json::to_string(&tag).unwrap();
        let parsed: Tag = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, tag.id);
        assert_eq!(parsed.name, tag.name);
        assert_eq!(parsed.icon, tag.icon);
        assert_eq!(parsed.color, tag.color);
        assert_eq!(parsed.created_at, tag.created_at);
    }

    #[test]
    fn task_serializes_with_optional_completed_at() {
        let task_incomplete = Task {
            id: 1,
            text: "Write tests".to_string(),
            completed: false,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            completed_at: None,
        };
        let json = serde_json::to_string(&task_incomplete).unwrap();
        let parsed: Task = serde_json::from_str(&json).unwrap();
        assert!(!parsed.completed);
        assert!(parsed.completed_at.is_none());

        let task_complete = Task {
            id: 2,
            text: "Deploy app".to_string(),
            completed: true,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            completed_at: Some("2024-01-01T12:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&task_complete).unwrap();
        let parsed: Task = serde_json::from_str(&json).unwrap();
        assert!(parsed.completed);
        assert_eq!(parsed.completed_at.as_deref(), Some("2024-01-01T12:00:00Z"));
    }

    #[test]
    fn manual_session_serializes_with_optional_fields() {
        let session_with_tags = ManualSession {
            id: "session-1".to_string(),
            session_type: super::SessionType::Focus,
            duration: 25,
            start_time: "09:00".to_string(),
            end_time: "09:25".to_string(),
            notes: Some("Deep work".to_string()),
            created_at: "2024-01-01T09:00:00Z".to_string(),
            date: "2024-01-01".to_string(),
            tags: Some(vec![serde_json::json!({"id": "tag-1", "name": "Work"})]),
            title: Some("Deep work".to_string()),
        };
        let json = serde_json::to_string(&session_with_tags).unwrap();
        let parsed: ManualSession = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "session-1");
        assert_eq!(parsed.duration, 25);
        assert_eq!(parsed.session_type, super::SessionType::Focus);
        assert!(parsed.notes.is_some());
        assert!(parsed.tags.is_some());
        assert_eq!(parsed.title.as_deref(), Some("Deep work"));

        let session_no_extras = ManualSession {
            id: "session-2".to_string(),
            session_type: super::SessionType::Break,
            duration: 5,
            start_time: "09:25".to_string(),
            end_time: "09:30".to_string(),
            notes: None,
            created_at: "2024-01-01T09:25:00Z".to_string(),
            date: "2024-01-01".to_string(),
            tags: None,
            title: None,
        };
        let json = serde_json::to_string(&session_no_extras).unwrap();
        let parsed: ManualSession = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.session_type, super::SessionType::Break);
        assert!(parsed.notes.is_none());
        assert!(parsed.tags.is_none());
        assert!(parsed.title.is_none());
    }

    #[test]
    fn session_tag_serializes_and_deserializes() {
        let session_tag = SessionTag {
            session_id: "session-1".to_string(),
            tag_id: "tag-1".to_string(),
            duration: 1500,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&session_tag).unwrap();
        let parsed: SessionTag = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.session_id, session_tag.session_id);
        assert_eq!(parsed.tag_id, session_tag.tag_id);
        assert_eq!(parsed.duration, session_tag.duration);
    }

    #[test]
    fn history_trimming_keeps_last_thirty_entries() {
        let mut history: Vec<PomodoroSession> = (0u32..35u32)
            .map(|i| PomodoroSession {
                completed_pomodoros: i,
                total_focus_time: i * 1500,
                current_session: 1,
                date: format!("2024-01-{i:02}"),
                title: None,
            })
            .collect();

        history.sort_by(|a, b| a.date.cmp(&b.date));
        if history.len() > 30 {
            let start_index = history.len() - 30;
            history.drain(0..start_index);
        }

        assert_eq!(history.len(), 30);
        assert_eq!(history[0].completed_pomodoros, 5);
        assert_eq!(history[29].completed_pomodoros, 34);
    }

    // -- BridgeError mirror tests (spec 001-leptos-migration T023 RED / T025 GREEN).
    //
    // The Tauri-side `BridgeError` enum mirrors the Leptos-side definition in
    // `presto-web/src/bridge/error.rs`. Wire shape: externally-tagged JSON via
    // `#[serde(tag = "kind", rename_all = "snake_case")]`. Both sides assert
    // the same byte-for-byte representation so a serde-incompatible change
    // breaks both crates' tests at once.
    //
    // RED-phase content: these tests reference `BridgeError`, which is not
    // yet declared; the module fails to compile. The implementation lands in
    // T025 GREEN.

    #[test]
    fn bridge_error_serde_roundtrip_bridge_unavailable() {
        let json = serde_json::to_string(&super::BridgeError::BridgeUnavailable).unwrap();
        assert_eq!(json, r#"{"kind":"bridge_unavailable"}"#);
    }

    #[test]
    fn bridge_error_serde_roundtrip_not_authenticated() {
        let json = serde_json::to_string(&super::BridgeError::NotAuthenticated).unwrap();
        assert_eq!(json, r#"{"kind":"not_authenticated"}"#);
    }

    #[test]
    fn bridge_error_serde_roundtrip_invalid_argument() {
        let err = super::BridgeError::InvalidArgument {
            field: "email".to_string(),
            reason: "empty".to_string(),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(
            json,
            r#"{"kind":"invalid_argument","field":"email","reason":"empty"}"#
        );
    }

    #[test]
    fn bridge_error_serde_roundtrip_not_found() {
        let err = super::BridgeError::NotFound {
            resource: "settings.json".to_string(),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, r#"{"kind":"not_found","resource":"settings.json"}"#);
    }

    #[test]
    fn bridge_error_serde_roundtrip_serde_roundtrip_variant() {
        let err = super::BridgeError::SerdeRoundtrip {
            command: "load_settings".to_string(),
            error: "missing field `timer`".to_string(),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(
            json,
            r#"{"kind":"serde_roundtrip","command":"load_settings","error":"missing field `timer`"}"#
        );
    }

    #[test]
    fn bridge_error_serde_roundtrip_internal() {
        let err = super::BridgeError::Internal {
            msg: "boom".to_string(),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, r#"{"kind":"internal","msg":"boom"}"#);
    }

    #[test]
    fn bridge_error_serde_roundtrip_decodes_external_tag() {
        let json = r#"{"kind":"invalid_argument","field":"password","reason":"too short"}"#;
        let decoded: super::BridgeError = serde_json::from_str(json).unwrap();
        match decoded {
            super::BridgeError::InvalidArgument { field, reason } => {
                assert_eq!(field, "password");
                assert_eq!(reason, "too short");
            }
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    // -- SessionType mirror tests (spec 001-leptos-migration T029).
    //
    // Mirrors `presto-web/src/bridge/session_type.rs` — same wire shape,
    // same camelCase strings. The mirror lets a `ManualSession` round-trip
    // across the bridge without translation (FR-013 closed-domain enum).

    #[test]
    fn session_type_serde_roundtrip_focus() {
        assert_eq!(
            serde_json::to_string(&super::SessionType::Focus).unwrap(),
            r#""focus""#
        );
        let decoded: super::SessionType = serde_json::from_str(r#""focus""#).unwrap();
        assert_eq!(decoded, super::SessionType::Focus);
    }

    #[test]
    fn session_type_serde_roundtrip_break() {
        assert_eq!(
            serde_json::to_string(&super::SessionType::Break).unwrap(),
            r#""break""#
        );
        let decoded: super::SessionType = serde_json::from_str(r#""break""#).unwrap();
        assert_eq!(decoded, super::SessionType::Break);
    }

    #[test]
    fn session_type_serde_roundtrip_long_break() {
        assert_eq!(
            serde_json::to_string(&super::SessionType::LongBreak).unwrap(),
            r#""longBreak""#
        );
        let decoded: super::SessionType = serde_json::from_str(r#""longBreak""#).unwrap();
        assert_eq!(decoded, super::SessionType::LongBreak);
    }

    #[test]
    fn session_type_serde_roundtrip_custom() {
        assert_eq!(
            serde_json::to_string(&super::SessionType::Custom).unwrap(),
            r#""custom""#
        );
        let decoded: super::SessionType = serde_json::from_str(r#""custom""#).unwrap();
        assert_eq!(decoded, super::SessionType::Custom);
    }

    // -- BridgeError mapping coverage (spec 001-leptos-migration T026 RED / T027 GREEN).
    //
    // Source-level invariant: post-T026, no `#[tauri::command]` handler in
    // `src-tauri/src/lib.rs` returns `Result<_, String>`. Every handler returns
    // `Result<_, BridgeError>` (or `Result<_, BridgeError<…>>` if generic in
    // future). The test reads its own crate source via `include_str!` and
    // greps for the legacy pattern.
    //
    // Why source-level rather than runtime: the rewrite is mechanical and
    // exhaustive — the only durable invariant is "no Result<_, String> on a
    // command signature". Exercising every handler at runtime would duplicate
    // the per-wrapper tests that land in Phase 1C (T032+).
    //
    // RED-phase content: the assertion fails because today's handlers all
    // return Result<_, String>. T027 GREEN flips this to zero by mechanically
    // rewriting every map_err call site.
    #[test]
    fn bridge_error_mapping_coverage_no_string_result_in_handlers() {
        let src = include_str!("lib.rs");
        let lines: Vec<&str> = src.lines().collect();
        let mut offenders: Vec<(usize, &str)> = Vec::new();
        let mut prev_was_command_attr = false;
        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim_start();
            // Look for the function-line that immediately follows
            // `#[tauri::command]`. We accept multi-line attribute blocks too
            // (the attribute may sit a few lines above with cfg-gates) but in
            // this codebase #[tauri::command] is always on the line directly
            // before the fn declaration.
            let is_fn_line = trimmed.starts_with("async fn ") || trimmed.starts_with("fn ");
            if prev_was_command_attr && is_fn_line {
                // The signature may span multiple lines; concatenate until we
                // hit `{`.
                let mut sig = String::new();
                for line2 in &lines[idx..] {
                    sig.push_str(line2);
                    sig.push(' ');
                    if line2.contains('{') {
                        break;
                    }
                }
                if sig.contains("Result<") && sig.contains(", String>") {
                    offenders.push((idx + 1, line.trim()));
                }
            }
            prev_was_command_attr = trimmed == "#[tauri::command]";
        }
        assert!(
            offenders.is_empty(),
            "Found {} #[tauri::command] handler(s) still returning Result<_, String>; \
             expected zero post-T027. Offenders (line: signature head):\n{}",
            offenders.len(),
            offenders
                .iter()
                .map(|(ln, s)| format!("  L{ln}: {s}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    // -- Feature 006: Quick-log + Distraction persistence (T030–T038).
    //
    // Tests exercise (1) the helpers in `helpers.rs` for read/write IO
    // round-trips + missing-file + corrupt-file behaviour, and (2) the
    // boundary-validation functions `validate_quick_logs` /
    // `validate_distractions` (extracted so the contract is testable
    // without spinning up a tauri::test runtime). The Tauri command
    // bodies are thin glue over these two layers.

    use super::{Distraction, DistractionParentRef, QuickLog};

    fn sample_quick_log() -> QuickLog {
        QuickLog {
            id: "11111111-1111-1111-1111-111111111111".to_string(),
            title: "Reply to email".to_string(),
            elapsed_minutes: 5,
            created_at: "2026-05-15T12:00:00Z".to_string(),
            date: "Fri May 15 2026".to_string(),
        }
    }

    fn sample_distraction(note: &str) -> Distraction {
        Distraction {
            id: "22222222-2222-2222-2222-222222222222".to_string(),
            note: note.to_string(),
            created_at: "2026-05-15T12:30:00Z".to_string(),
            date: "Fri May 15 2026".to_string(),
            parent_ref: Some(DistractionParentRef {
                parent_session_start_ts: "2026-05-15T12:25:00Z".to_string(),
                parent_mode: super::TimerMode::Focus,
                parent_tag_id: Some("default-focus".to_string()),
                parent_title: Some("Deep work".to_string()),
            }),
        }
    }

    /// T030: save → load round-trip preserves the vec verbatim.
    #[test]
    fn save_quick_logs_round_trip() {
        let dir = std::env::temp_dir().join(format!(
            "presto_test_quick_logs_round_trip_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let logs = vec![sample_quick_log()];
        super::helpers::write_quick_logs_to(&dir, &logs).expect("write");
        let loaded = super::helpers::read_quick_logs_from(&dir).expect("read");
        assert_eq!(loaded, logs);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// T031: `elapsed_minutes` range 1..=720, both boundaries rejected.
    /// Field name is camelCase per the wire shape.
    #[test]
    fn save_quick_logs_rejects_out_of_range_minutes() {
        for &m in &[0u32, 721u32] {
            let mut log = sample_quick_log();
            log.elapsed_minutes = m;
            let err =
                super::validate_quick_logs(&[log]).expect_err("must reject out-of-range minutes");
            match err {
                super::BridgeError::InvalidArgument { field, .. } => {
                    assert_eq!(field, "elapsedMinutes", "field name must be camelCase");
                }
                other => panic!("expected InvalidArgument, got {other:?}"),
            }
        }
    }

    /// T032: 121-char title rejected.
    #[test]
    fn save_quick_logs_rejects_overlong_title() {
        let mut log = sample_quick_log();
        log.title = "a".repeat(121);
        let err = super::validate_quick_logs(&[log]).expect_err("must reject overlong title");
        match err {
            super::BridgeError::InvalidArgument { field, .. } => {
                assert_eq!(field, "title");
            }
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    /// T033: empty title rejected.
    #[test]
    fn save_quick_logs_rejects_empty_title() {
        let mut log = sample_quick_log();
        log.title = String::new();
        let err = super::validate_quick_logs(&[log]).expect_err("must reject empty title");
        match err {
            super::BridgeError::InvalidArgument { field, .. } => {
                assert_eq!(field, "title");
            }
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    /// T034: save → load round-trip preserves the vec verbatim, including
    /// the `parent_ref` payload.
    #[test]
    fn save_distractions_round_trip() {
        let dir = std::env::temp_dir().join(format!(
            "presto_test_distractions_round_trip_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let entries = vec![sample_distraction("Slack ping")];
        super::helpers::write_distractions_to(&dir, &entries).expect("write");
        let loaded = super::helpers::read_distractions_from(&dir).expect("read");
        assert_eq!(loaded, entries);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// T035: 121-char note rejected.
    #[test]
    fn save_distractions_rejects_overlong_note() {
        let entry = sample_distraction(&"a".repeat(121));
        let err = super::validate_distractions(&[entry]).expect_err("must reject overlong note");
        match err {
            super::BridgeError::InvalidArgument { field, .. } => {
                assert_eq!(field, "note");
            }
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    /// T036: `parent_ref.parent_title` overlength rejected with field
    /// `parentRef.parentTitle`.
    #[test]
    fn save_distractions_rejects_overlong_parent_title() {
        let mut entry = sample_distraction("Quick");
        if let Some(parent) = entry.parent_ref.as_mut() {
            parent.parent_title = Some("a".repeat(121));
        }
        let err =
            super::validate_distractions(&[entry]).expect_err("must reject overlong parent title");
        match err {
            super::BridgeError::InvalidArgument { field, .. } => {
                assert_eq!(field, "parentRef.parentTitle");
            }
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    /// T037: missing files yield Ok(empty vec) for both load paths.
    #[test]
    fn load_returns_empty_when_file_missing() {
        let dir =
            std::env::temp_dir().join(format!("presto_test_missing_files_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // Note: directory itself is absent — read helpers should still
        // return Ok([]) (mirrors `read_manual_sessions_from` semantics).
        let ql = super::helpers::read_quick_logs_from(&dir).expect("read missing quick logs");
        let dr = super::helpers::read_distractions_from(&dir).expect("read missing distractions");
        assert!(ql.is_empty());
        assert!(dr.is_empty());
    }

    /// T038: corrupt JSON yields an error string whose message is
    /// PII-scrubbed (no payload bytes from the corrupt file appear in
    /// the human-readable reason). Mirrors AG-10 finding.
    #[test]
    fn load_handles_corrupt_file_with_bridge_error_internal() {
        let dir =
            std::env::temp_dir().join(format!("presto_test_corrupt_files_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        // Distinctive PII-style bytes the test asserts are NOT echoed.
        let sentinel = "USER_SECRET_KAYAK_BANANA";
        std::fs::write(
            dir.join("quick_logs.json"),
            format!("{{ corrupted {sentinel} }}"),
        )
        .expect("seed corrupt file");
        std::fs::write(
            dir.join("distractions.json"),
            format!("[ corrupted {sentinel} ]"),
        )
        .expect("seed corrupt file");

        let ql_err = super::helpers::read_quick_logs_from(&dir).expect_err("must fail on corrupt");
        let dr_err =
            super::helpers::read_distractions_from(&dir).expect_err("must fail on corrupt");
        assert!(
            !ql_err.contains(sentinel),
            "PII payload leaked into quick_logs error: {ql_err}"
        );
        assert!(
            !dr_err.contains(sentinel),
            "PII payload leaked into distractions error: {dr_err}"
        );
        // BridgeError::from(String) maps to Internal { msg } per contract.
        let lifted: super::BridgeError = ql_err.into();
        assert!(matches!(lifted, super::BridgeError::Internal { .. }));

        let _ = std::fs::remove_dir_all(&dir);
    }

    // Feature 007 T013b: Tauri-bridge contract for the widened
    // `register_global_shortcuts` argument. Tests exercise the pure
    // `parse_shortcut_bindings` helper which mirrors the loop in
    // `register_global_shortcuts` 1:1 — same iteration order, same
    // parsing call, same error shape. The helper is the testable
    // surface; the live registration loop wraps it with the
    // `AppHandle`-bound side effects (unregister_all, on_shortcut
    // closure install, shortcuts-updated emit).

    /// Feature 007 T013b (RED → T023 GREEN): a fully bound
    /// `ShortcutSettings` with `abort = Some(_)` parses successfully
    /// and the resulting bindings slice contains exactly four entries
    /// in the canonical order: start-stop, reset, skip, abort.
    #[test]
    fn register_global_shortcuts_widened_arg_accepts_abort() {
        let shortcuts = super::ShortcutSettings {
            start_stop: Some("CommandOrControl+Alt+Space".to_string()),
            reset: Some("CommandOrControl+Alt+R".to_string()),
            skip: Some("CommandOrControl+Alt+S".to_string()),
            abort: Some("CommandOrControl+Alt+W".to_string()),
        };
        let parsed =
            super::parse_shortcut_bindings(&shortcuts).expect("all four bindings must parse");
        let actions: Vec<&str> = parsed.iter().map(|(a, _)| *a).collect();
        assert_eq!(actions, vec!["start-stop", "reset", "skip", "abort"]);
    }

    /// Feature 007 T013b (RED → T023 GREEN): `abort = None` is skipped
    /// by the iteration — no binding is emitted for the `"abort"`
    /// action. Mirrors the existing `if let Some(ref s) = …` gate the
    /// three sibling fields already enjoy.
    #[test]
    fn register_global_shortcuts_widened_arg_skips_unbound_abort() {
        let shortcuts = super::ShortcutSettings {
            start_stop: Some("CommandOrControl+Alt+Space".to_string()),
            reset: None,
            skip: None,
            abort: None,
        };
        let parsed = super::parse_shortcut_bindings(&shortcuts).expect("must parse");
        let actions: Vec<&str> = parsed.iter().map(|(a, _)| *a).collect();
        assert_eq!(actions, vec!["start-stop"]);
        assert!(
            !actions.contains(&"abort"),
            "abort: None must not yield a binding entry"
        );
    }

    /// Feature 007 T013b (RED → T023 GREEN): an unparseable abort
    /// shortcut spec returns `BridgeError::Internal { msg }` carrying
    /// the action name `"abort"` for diagnosability. The action name
    /// appears in the error message so the user can identify which
    /// binding failed.
    #[test]
    fn register_global_shortcuts_widened_arg_invalid_abort_returns_internal_error() {
        let shortcuts = super::ShortcutSettings {
            start_stop: None,
            reset: None,
            skip: None,
            abort: Some("not-a-shortcut".to_string()),
        };
        match super::parse_shortcut_bindings(&shortcuts) {
            Err(super::BridgeError::Internal { msg }) => {
                assert!(
                    msg.contains("abort"),
                    "error msg must mention the action name 'abort': got {msg}"
                );
            }
            Err(other) => panic!("expected BridgeError::Internal, got {other:?}"),
            Ok(_) => panic!("invalid shortcut spec must not parse"),
        }
    }
}
