use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::sync::{Arc, LazyLock, Mutex};
#[cfg(target_os = "macos")]
use std::thread;
use std::time::{Duration, Instant};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};
use tauri_plugin_aptabase::EventTracker;
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
use tauri_plugin_oauth::start;

mod auth;
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
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BridgeError {
    /// `window.__TAURI_INTERNALS__` is absent (Leptos-side only).
    #[error("bridge unavailable")]
    BridgeUnavailable,
    /// The caller lacks the required session for this command.
    #[error("not authenticated")]
    NotAuthenticated,
    /// An argument failed validation at the boundary.
    #[error("invalid argument {field}: {reason}")]
    InvalidArgument { field: String, reason: String },
    /// The requested file, key, or row does not exist.
    #[error("not found: {resource}")]
    NotFound { resource: String },
    /// `serde-wasm-bindgen` failed to deserialise the return on the Leptos
    /// side. Tauri-side handlers do not produce this variant; it exists in
    /// the mirror so the type definition stays single-sourced.
    ///
    /// `command: String` (not `&'static str`) so the enum's `Deserialize`
    /// impl works for non-static input. See the matching note in
    /// `presto-web/src/bridge/error.rs` for the full rationale.
    #[error("serde roundtrip failed in {command}: {error}")]
    SerdeRoundtrip { command: String, error: String },
    /// Catch-all for unexpected Tauri-side failures.
    #[error("internal: {msg}")]
    Internal { msg: String },
}

/// `TimerMode` — closed-domain enum for the live-engine session mode.
///
/// Spec 001-leptos-migration §Phase 1A T027; data-model.md §`TimerMode`.
/// On-disk wire form is camelCase strings (`"focus"`, `"break"`,
/// `"longBreak"`). Tray-handler args (`update_tray_menu.current_mode`,
/// `update_tray_icon.session_mode`) tighten from `String` to this enum.
/// The Leptos-side mirror lands in Phase 1C (T076-T079).
///
/// Distinct from `SessionType` (T028-T029): manual sessions can carry the
/// `Custom` variant; the live engine cannot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TimerMode {
    Focus,
    Break,
    LongBreak,
}

/// `SessionType` — closed-domain enum for manual-session entries.
///
/// Spec 001-leptos-migration §Phase 1A T029; data-model.md §`SessionType`.
/// Mirrors `presto-web/src/bridge/session_type.rs`. Wire form: camelCase
/// strings (`"focus"`, `"break"`, `"longBreak"`, `"custom"`).
///
/// Distinct from `TimerMode` because manual entries can carry the
/// `Custom` variant for user-defined session shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionType {
    Focus,
    Break,
    LongBreak,
    Custom,
}

// `String → BridgeError` via Internal. Lets `?` auto-convert legacy
// `Result<_, String>` returns from `helpers.rs` (which keeps the legacy
// error type for now) into `BridgeError` at the handler boundary. The
// conversion is a "no semantic context" mapping. Tighter variants
// (NotFound / InvalidArgument / NotAuthenticated) are still spelled out
// at the call sites that warrant them — `From<String>` is the catch-all
// fallback for plumbing.
//
// Spec 001-leptos-migration §Phase 1A T027.
impl From<String> for BridgeError {
    fn from(msg: String) -> Self {
        Self::Internal { msg }
    }
}

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

#[derive(Debug, Serialize, Deserialize, Clone)]
struct PomodoroSession {
    completed_pomodoros: u32,
    total_focus_time: u32, // in seconds
    current_session: u32,
    date: String,
}

// `session_type: SessionType` (was `String`) per spec 001 T029 —
// closed-domain enum tightening. Wire format unchanged: camelCase
// strings via `#[serde(rename_all = "camelCase")]` on `SessionType`.
// On-disk shape preserved exactly (FR-005 idempotent round-trip).
#[derive(Debug, Serialize, Deserialize, Clone)]
struct ManualSession {
    id: String,
    session_type: SessionType,
    duration: u32,      // in minutes
    start_time: String, // "HH:MM"
    end_time: String,   // "HH:MM"
    notes: Option<String>,
    created_at: String, // ISO string
    date: String,
    tags: Option<Vec<serde_json::Value>>, // Array of tag objects
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Tag {
    id: String,
    name: String,
    icon: String,  // emoji or remix icon class
    color: String, // hex color code
    created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SessionTag {
    session_id: String,
    tag_id: String,
    duration: u32, // time spent on this tag in seconds
    created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Task {
    id: u64,
    text: String,
    completed: bool,
    created_at: String,
    completed_at: Option<String>,
}

/// User-facing settings; the bool fields are independent toggles, splitting
/// them into nested structs would hurt config readability.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Serialize, Deserialize, Clone)]
struct AppSettings {
    shortcuts: ShortcutSettings,
    timer: TimerSettings,
    notifications: NotificationSettings,
    #[serde(default)]
    advanced: AdvancedSettings,
    autostart: bool,
    #[serde(default = "default_analytics_enabled")]
    analytics_enabled: bool,
    #[serde(default)]
    hide_icon_on_close: bool,
    #[serde(default)]
    hide_status_bar: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ShortcutSettings {
    start_stop: Option<String>,
    reset: Option<String>,
    skip: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TimerSettings {
    focus_duration: u32,
    break_duration: u32,
    long_break_duration: u32,
    total_sessions: u32,
    #[serde(default = "default_weekly_goal")]
    weekly_goal_minutes: u32,
}

const fn default_weekly_goal() -> u32 {
    125
}

const fn default_analytics_enabled() -> bool {
    true
}

/// Loads settings synchronously from disk, falling back to defaults on any error.
fn load_settings_sync(app: &AppHandle) -> AppSettings {
    let Ok(app_data_dir) = app.path().app_data_dir() else {
        return AppSettings {
            analytics_enabled: false,
            ..AppSettings::default()
        };
    };
    helpers::read_settings_from(&app_data_dir).unwrap_or_default()
}

fn are_analytics_enabled(app: &AppHandle) -> bool {
    app.state::<SettingsState>()
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .analytics_enabled
}

/// User-facing notification preferences; each bool maps to an independent
/// UI toggle, restructuring would not match the settings UI.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Serialize, Deserialize, Clone)]
struct NotificationSettings {
    desktop_notifications: bool,
    sound_notifications: bool,
    auto_start_timer: bool,
    #[serde(default)]
    auto_start_focus: bool,
    #[serde(default)]
    allow_continuous_sessions: bool,
    smart_pause: bool,
    smart_pause_timeout: u32, // timeout in seconds
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
struct AdvancedSettings {
    #[serde(default)]
    debug_mode: bool, // Debug mode with 3-second timers
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            shortcuts: ShortcutSettings {
                start_stop: Some("CommandOrControl+Alt+Space".to_string()),
                reset: Some("CommandOrControl+Alt+R".to_string()),
                skip: Some("CommandOrControl+Alt+S".to_string()),
            },
            timer: TimerSettings {
                focus_duration: 25,
                break_duration: 5,
                long_break_duration: 20,
                total_sessions: 10,
                weekly_goal_minutes: 125,
            },
            notifications: NotificationSettings {
                desktop_notifications: true,
                sound_notifications: true,
                auto_start_timer: true,
                auto_start_focus: false,
                allow_continuous_sessions: false,
                smart_pause: false,
                smart_pause_timeout: 30, // default 30 seconds
            },
            advanced: AdvancedSettings::default(),
            autostart: false,
            analytics_enabled: true,
            hide_icon_on_close: false,
            hide_status_bar: false,
        }
    }
}

fn should_debounce_shortcut(action: &str) -> bool {
    let mut map = helpers::lock_or_recover(&SHORTCUT_DEBOUNCE);
    helpers::is_debounced(&mut map, action, Instant::now(), Duration::from_millis(500))
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
    fn start_monitoring(&self) -> Result<(), String> {
        let mut is_monitoring = helpers::lock_or_recover(&self.is_monitoring);
        if *is_monitoring {
            return Ok(());
        }
        *is_monitoring = true;

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

                thread::sleep(Duration::from_millis(500)); // Check every 500ms
            }
        });

        Ok(())
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
async fn start_activity_monitoring(app: AppHandle, timeout_seconds: u64) -> Result<(), BridgeError> {
    #[cfg(target_os = "macos")]
    {
        let mut monitor = helpers::lock_or_recover(&ACTIVITY_MONITOR);
        if monitor.is_none() {
            *monitor = Some(ActivityMonitor::new(app, timeout_seconds));
        }
        if let Some(ref m) = *monitor {
            m.start_monitoring()?;
        }
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, timeout_seconds);
        Err(BridgeError::Internal { msg: "Activity monitoring is only supported on macOS".to_string() })
    }
}

#[tauri::command]
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
async fn update_activity_timeout(timeout_seconds: u64) -> Result<(), BridgeError> {
    let monitor = helpers::lock_or_recover(&ACTIVITY_MONITOR);
    monitor.as_ref().map_or_else(
        || Err(BridgeError::Internal { msg: "Activity monitor not initialized".to_string() }),
        |m| {
            m.update_threshold(timeout_seconds);
            Ok(())
        },
    )
}

#[tauri::command]
async fn save_session_data(session: PomodoroSession, app: AppHandle) -> Result<(), BridgeError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to get app data directory: {e}") })?;

    helpers::write_session_to(&app_data_dir, &session)?;

    if are_analytics_enabled(&app) {
        let properties = Some(serde_json::json!({
            "completed_pomodoros": session.completed_pomodoros,
            "total_focus_time": session.total_focus_time,
            "current_session": session.current_session
        }));
        let _ = app.track_event("session_saved", properties);
    }

    Ok(())
}

#[tauri::command]
async fn load_session_data(app: AppHandle) -> Result<Option<PomodoroSession>, BridgeError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to get app data directory: {e}") })?;
    helpers::read_session_from(&app_data_dir).map_err(BridgeError::from)
}

#[tauri::command]
async fn save_tasks(tasks: Vec<Task>, app: AppHandle) -> Result<(), BridgeError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to get app data directory: {e}") })?;

    helpers::write_tasks_to(&app_data_dir, &tasks)?;

    if are_analytics_enabled(&app) {
        let _ = app.track_event("tasks_saved", None);
    }

    Ok(())
}

#[tauri::command]
async fn load_tasks(app: AppHandle) -> Result<Vec<Task>, BridgeError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to get app data directory: {e}") })?;
    helpers::read_tasks_from(&app_data_dir).map_err(BridgeError::from)
}

#[tauri::command]
async fn get_stats_history(app: AppHandle) -> Result<Vec<PomodoroSession>, BridgeError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to get app data directory: {e}") })?;
    helpers::read_history_from(&app_data_dir).map_err(BridgeError::from)
}

#[tauri::command]
async fn save_daily_stats(session: PomodoroSession, app: AppHandle) -> Result<(), BridgeError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to get app data directory: {e}") })?;
    helpers::append_daily_stats_to(&app_data_dir, &session).map_err(BridgeError::from)
}

// `session_mode: TimerMode` (was `String`) per spec 001 T027 — closed-domain
// enum tightening. Wire format unchanged: camelCase ("focus"/"break"/
// "longBreak") via `#[serde(rename_all = "camelCase")]` on `TimerMode`.
#[tauri::command]
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
                    match session_mode {
                        TimerMode::Focus => "🧠",
                        TimerMode::Break => "☕",
                        TimerMode::LongBreak => "🌙",
                    }
                    .to_string()
                });

                let status = if is_running { "Running" } else { "Paused" };
                let title = format!("{icon} {timer_text}");
                tray.set_title(Some(title))
                    .map_err(|e| BridgeError::Internal { msg: format!("Failed to set title: {e}") })?;

                let tooltip = match session_mode {
                    TimerMode::Focus => format!(
                        "Presto - Session {current_session}/{total_sessions} ({status})"
                    ),
                    TimerMode::LongBreak => format!("Presto - Long Break ({status})"),
                    TimerMode::Break => format!("Presto - Short Break ({status})"),
                };

                tray.set_tooltip(Some(tooltip))
                    .map_err(|e| BridgeError::Internal { msg: format!("Failed to set tooltip: {e}") })?;
            }
            Ok(())
        })();
    })
    .map_err(|e| BridgeError::Internal { msg: format!("Failed to run on main thread: {e}") })?;

    // Extract the result from the mutex (named binding required by borrow checker:
    // the temporary MutexGuard must drop before `result` does).
    let final_result = helpers::lock_or_recover(&result).clone();
    final_result
}

#[allow(clippy::unused_async)] // awaits set_dock_visibility on macOS
async fn show_app_window(app: AppHandle) {
    let settings = app
        .state::<SettingsState>()
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    if settings.hide_icon_on_close {
        #[cfg(target_os = "macos")]
        {
            let _ = set_dock_visibility(app.clone(), true).await;
        }
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
async fn show_window(app: AppHandle) -> Result<(), BridgeError> {
    show_app_window(app).await;
    Ok(())
}

#[tauri::command]
async fn save_settings(settings: AppSettings, app: AppHandle) -> Result<(), BridgeError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to get app data directory: {e}") })?;

    helpers::write_settings_to(&app_data_dir, &settings)?;

    *app.state::<SettingsState>()
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = settings;

    Ok(())
}

#[tauri::command]
async fn load_settings(app: AppHandle) -> Result<AppSettings, BridgeError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to get app data directory: {e}") })?;
    helpers::read_settings_from(&app_data_dir)
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to read settings: {e}") })
}

#[tauri::command]
async fn register_global_shortcuts(
    app: AppHandle,
    shortcuts: ShortcutSettings,
) -> Result<(), BridgeError> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to unregister shortcuts: {e}") })?;

    for (action, shortcut_str) in [
        ("start-stop", &shortcuts.start_stop),
        ("reset", &shortcuts.reset),
        ("skip", &shortcuts.skip),
    ] {
        if let Some(ref shortcut_str) = shortcut_str {
            let shortcut: Shortcut = shortcut_str
                .parse()
                .map_err(|e| BridgeError::Internal { msg: format!("Invalid {action} shortcut '{shortcut_str}': {e}") })?;

            let app_handle = app.clone();
            let action_owned = action.to_string();
            app.global_shortcut()
                .on_shortcut(shortcut, move |_app, _shortcut, _event| {
                    if !should_debounce_shortcut(&action_owned) {
                        let _ = app_handle.emit("global-shortcut", action_owned.as_str());
                    }
                })
                .map_err(|e| BridgeError::Internal { msg: format!("Failed to register {action} shortcut: {e}") })?;
        }
    }

    // Emit an event to the frontend to update local shortcuts as well
    app.emit("shortcuts-updated", &shortcuts)
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to emit shortcuts update: {e}") })?;

    Ok(())
}

#[tauri::command]
async fn unregister_global_shortcuts(app: AppHandle) -> Result<(), BridgeError> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to unregister shortcuts: {e}") })?;
    Ok(())
}

#[tauri::command]
async fn reset_all_data(app: AppHandle) -> Result<(), BridgeError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to get app data directory: {e}") })?;

    helpers::delete_all_data_in(&app_data_dir)?;

    *app.state::<SettingsState>()
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = AppSettings::default();

    Ok(())
}

#[tauri::command]
async fn enable_autostart(app: AppHandle) -> Result<(), BridgeError> {
    app.autolaunch()
        .enable()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to enable autostart: {e}") })
}

#[tauri::command]
async fn disable_autostart(app: AppHandle) -> Result<(), BridgeError> {
    app.autolaunch()
        .disable()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to disable autostart: {e}") })
}

#[tauri::command]
async fn is_autostart_enabled(app: AppHandle) -> Result<bool, BridgeError> {
    app.autolaunch()
        .is_enabled()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to check autostart status: {e}") })
}

#[tauri::command]
async fn save_manual_sessions(sessions: Vec<ManualSession>, app: AppHandle) -> Result<(), BridgeError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to get app data directory: {e}") })?;

    helpers::write_manual_sessions_to(&app_data_dir, &sessions)?;

    if are_analytics_enabled(&app) {
        let properties = Some(serde_json::json!({
            "session_count": sessions.len()
        }));
        let _ = app.track_event("manual_sessions_saved", properties);
    }

    Ok(())
}

#[tauri::command]
async fn load_manual_sessions(app: AppHandle) -> Result<Vec<ManualSession>, BridgeError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to get app data directory: {e}") })?;
    helpers::read_manual_sessions_from(&app_data_dir).map_err(BridgeError::from)
}

#[tauri::command]
async fn save_manual_session(session: ManualSession, app: AppHandle) -> Result<(), BridgeError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to get app data directory: {e}") })?;
    helpers::upsert_manual_session_in(&app_data_dir, session).map_err(BridgeError::from)
}

#[tauri::command]
async fn delete_manual_session(session_id: String, app: AppHandle) -> Result<(), BridgeError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to get app data directory: {e}") })?;
    helpers::delete_manual_session_in(&app_data_dir, &session_id).map_err(BridgeError::from)
}

#[tauri::command]
async fn get_manual_sessions_for_date(
    date: String,
    app: AppHandle,
) -> Result<Vec<ManualSession>, BridgeError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to get app data directory: {e}") })?;
    let sessions = helpers::read_manual_sessions_from(&app_data_dir)?;
    Ok(sessions.into_iter().filter(|s| s.date == date).collect())
}

/// Builds and runs the Tauri application.
///
/// # Panics
///
/// Panics if the Tauri runtime fails to initialize or if the GUI cannot be
/// constructed. The native runtime fails fast in this case because there is
/// nothing the rest of the app can do without it.
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
            .plugin(tauri_plugin_oauth::init())
            .plugin(tauri_plugin_aptabase::Builder::new("A-EU-9457123106").build())
            .invoke_handler(tauri::generate_handler![
                save_session_data,
                load_session_data,
                save_tasks,
                load_tasks,
                get_stats_history,
                save_daily_stats,
                update_tray_icon,
                update_tray_menu,
                show_window,
                save_settings,
                load_settings,
                register_global_shortcuts,
                unregister_global_shortcuts,
                reset_all_data,
                start_activity_monitoring,
                stop_activity_monitoring,
                update_activity_timeout,
                enable_autostart,
                disable_autostart,
                is_autostart_enabled,
                save_manual_sessions,
                load_manual_sessions,
                save_manual_session,
                delete_manual_session,
                get_manual_sessions_for_date,
                load_tags,
                save_tags,
                save_tag,
                delete_tag,
                load_session_tags,
                save_session_tags,
                add_session_tag,
                write_excel_file,
                start_oauth_server,
                set_dock_visibility,
                set_status_bar_visibility,
                track_event,
                supabase_sign_in_with_password,
                supabase_sign_out,
                supabase_get_session
            ])
            .setup(|app| {
                let initial_settings = load_settings_sync(app.handle());
                app.manage(SettingsState(Mutex::new(initial_settings)));

                let app_handle_analytics = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if are_analytics_enabled(&app_handle_analytics) {
                        let _ = app_handle_analytics.track_event("app_started", None);
                    }
                });

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

                let _tray = TrayIconBuilder::with_id("main")
                    .menu(&menu)
                    .show_menu_on_left_click(true)
                    .on_menu_event(move |_tray, event| match event.id.as_ref() {
                        "show" => {
                            let app_clone = app_handle.clone();
                            tauri::async_runtime::spawn(async move {
                                show_app_window(app_clone).await;
                            });
                        }
                        "start_session" => {
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.emit("tray-start-session", ());
                            }
                            let app_clone = app_handle.clone();
                            tauri::async_runtime::spawn(async move {
                                show_app_window(app_clone).await;
                            });
                        }
                        "pause" => {
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.emit("tray-pause", ());
                            }
                            let app_clone = app_handle.clone();
                            tauri::async_runtime::spawn(async move {
                                show_app_window(app_clone).await;
                            });
                        }
                        "skip" => {
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.emit("tray-skip", ());
                            }
                            let app_clone = app_handle.clone();
                            tauri::async_runtime::spawn(async move {
                                show_app_window(app_clone).await;
                            });
                        }
                        "cancel" => {
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.emit("tray-cancel", ());
                            }
                            let app_clone = app_handle.clone();
                            tauri::async_runtime::spawn(async move {
                                show_app_window(app_clone).await;
                            });
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
                    })
                    .build(app)?;

                if let Some(window) = app.get_webview_window("main") {
                    let app_handle_for_close = app.handle().clone();
                    window.on_window_event(move |event| {
                        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                            api.prevent_close();

                            let settings = app_handle_for_close
                                .state::<SettingsState>()
                                .0
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner)
                                .clone();
                            let app_handle_clone = app_handle_for_close.clone();
                            tauri::async_runtime::spawn(async move {
                                if let Some(window) = app_handle_clone.get_webview_window("main") {
                                    let _ = window.hide();
                                    if settings.hide_icon_on_close {
                                        #[cfg(target_os = "macos")]
                                        {
                                            let _ = set_dock_visibility(
                                                app_handle_clone.clone(),
                                                false,
                                            )
                                            .await;
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
                            // Try to register default shortcuts
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
                tauri::RunEvent::Exit => {
                    // Always track app exit event regardless of analytics settings
                    // since this is the final event and useful for crash detection
                    let _ = app_handle.track_event("app_exited", None);
                    app_handle.flush_events_blocking();
                }
                #[cfg(target_os = "macos")]
                tauri::RunEvent::Reopen { .. } => {
                    let app_handle_clone = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        show_app_window(app_handle_clone).await;
                    });
                }
                _ => {}
            });
    });
}

#[tauri::command]
async fn load_tags(app: AppHandle) -> Result<Vec<Tag>, BridgeError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to get app data directory: {e}") })?;
    helpers::read_tags_from(&app_data_dir).map_err(BridgeError::from)
}

#[tauri::command]
async fn save_tags(tags: Vec<Tag>, app: AppHandle) -> Result<(), BridgeError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to get app data directory: {e}") })?;
    helpers::write_tags_to(&app_data_dir, &tags).map_err(BridgeError::from)
}

#[tauri::command]
async fn save_tag(tag: Tag, app: AppHandle) -> Result<(), BridgeError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to get app data directory: {e}") })?;
    helpers::upsert_tag_in(&app_data_dir, tag).map_err(BridgeError::from)
}

#[tauri::command]
async fn delete_tag(tag_id: String, app: AppHandle) -> Result<(), BridgeError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to get app data directory: {e}") })?;
    helpers::delete_tag_in(&app_data_dir, &tag_id).map_err(BridgeError::from)
}

#[tauri::command]
async fn load_session_tags(app: AppHandle) -> Result<Vec<SessionTag>, BridgeError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to get app data directory: {e}") })?;
    helpers::read_session_tags_from(&app_data_dir).map_err(BridgeError::from)
}

#[tauri::command]
async fn save_session_tags(session_tags: Vec<SessionTag>, app: AppHandle) -> Result<(), BridgeError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to get app data directory: {e}") })?;
    helpers::write_session_tags_to(&app_data_dir, &session_tags).map_err(BridgeError::from)
}

#[tauri::command]
async fn add_session_tag(session_tag: SessionTag, app: AppHandle) -> Result<(), BridgeError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to get app data directory: {e}") })?;
    helpers::append_session_tag_in(&app_data_dir, session_tag).map_err(BridgeError::from)
}

// `current_mode: TimerMode` (was `String`) per spec 001 T027 — closed-domain
// enum tightening. Wire format unchanged: camelCase strings.
#[tauri::command]
async fn update_tray_menu(
    app: AppHandle,
    is_running: bool,
    is_paused: bool,
    current_mode: TimerMode,
) -> Result<(), BridgeError> {
    let tray = app.tray_by_id("main");

    if let Some(tray) = tray {
        let show_item = MenuItem::with_id(&app, "show", "Show Presto", true, None::<&str>)
            .map_err(|e| BridgeError::Internal { msg: format!("Failed to create show item: {e}") })?;

        // Start Session: enabled only if not running
        let start_session_item = MenuItem::with_id(
            &app,
            "start_session",
            "Start Session",
            !is_running,
            None::<&str>,
        )
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to create start session item: {e}") })?;

        // Pause: enabled only if running and not paused
        let pause_item = MenuItem::with_id(
            &app,
            "pause",
            "Pause",
            is_running && !is_paused,
            None::<&str>,
        )
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to create pause item: {e}") })?;

        // Skip: enabled only if running
        let skip_item = MenuItem::with_id(&app, "skip", "Skip Session", is_running, None::<&str>)
            .map_err(|e| BridgeError::Internal { msg: format!("Failed to create skip item: {e}") })?;

        // Cancel: enabled if in focus mode, disabled in break/longBreak (undo)
        let cancel_text = if matches!(current_mode, TimerMode::Focus) {
            "Cancel"
        } else {
            "Cancel Last"
        };
        let cancel_item = MenuItem::with_id(&app, "cancel", cancel_text, true, None::<&str>)
            .map_err(|e| BridgeError::Internal { msg: format!("Failed to create cancel item: {e}") })?;

        let quit_item = MenuItem::with_id(&app, "quit", "Quit", true, None::<&str>)
            .map_err(|e| BridgeError::Internal { msg: format!("Failed to create quit item: {e}") })?;

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
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to create menu: {e}") })?;

        tray.set_menu(Some(new_menu))
            .map_err(|e| BridgeError::Internal { msg: format!("Failed to set tray menu: {e}") })?;
    }

    Ok(())
}

/// Decodes `data` from standard base64 and writes the result to `path`.
///
/// Exposed as `pub` so `tests/commands.rs` can wire it into a `MockRuntime`
/// app via a locally-defined `#[tauri::command]` wrapper (applying `pub`
/// directly to a `#[tauri::command]` that also appears in `generate_handler!`
/// causes a macro-namespace conflict due to `#[macro_export]`).
///
/// # Errors
///
/// Returns `BridgeError::InvalidArgument` if `data` is not valid base64
/// (caller's input failed validation), or `BridgeError::Internal` if the
/// file cannot be written to `path` (filesystem failure with no
/// caller-actionable detail). Spec 001-leptos-migration §Phase 1A T027.
pub fn decode_and_write_file(path: &str, data: &str) -> Result<(), BridgeError> {
    let decoded_data = general_purpose::STANDARD
        .decode(data)
        .map_err(|e| BridgeError::InvalidArgument {
            field: "data".to_string(),
            reason: format!("invalid base64: {e}"),
        })?;
    fs::write(path, decoded_data).map_err(|e| BridgeError::Internal {
        msg: format!("Failed to write Excel file to {path}: {e}"),
    })?;
    Ok(())
}

#[tauri::command]
async fn write_excel_file(path: String, data: String) -> Result<(), BridgeError> {
    decode_and_write_file(&path, &data)
}

#[tauri::command]
async fn start_oauth_server(window: tauri::Window) -> Result<u16, BridgeError> {
    start(move |url| {
        let _ = window.emit("oauth-callback", url);
    })
    .map_err(|err| BridgeError::Internal { msg: err.to_string() })
}

// `track_event` — Phase 1D T086.
//
// Replaces the JS `@aptabase/tauri` shim that the migration deletes. The
// shim's only behaviour was to forward `track_event(...)` to the Aptabase
// plugin via `invoke()`. This handler does the same with one extra
// guarantee (per spec FR-018 / Principle II): the `analytics_enabled`
// opt-in toggle is checked Rust-side via `are_analytics_enabled` before
// any forwarding happens — a Leptos call site cannot accidentally bypass
// it because the gate lives below the bridge.
//
// `props` is `Option<HashMap<String, Value>>` — `None` matches the
// bare-name path Aptabase's `EventTracker::track_event` accepts directly.
// We construct a `serde_json::Value::Object` from the HashMap and pass it
// to the plugin (the plugin's `track_event` API takes a serializable
// payload). When the toggle is off, the handler returns `Ok(())` without
// forwarding — the disabled path is silent, not an error (matches the
// existing in-handler call sites at `lib.rs:474`, `lib.rs:500`, etc.).
#[tauri::command]
async fn track_event(
    name: String,
    props: Option<HashMap<String, serde_json::Value>>,
    app: AppHandle,
) -> Result<(), BridgeError> {
    if are_analytics_enabled(&app) {
        // The aptabase plugin's `EventTracker::track_event` takes a
        // serde_json::Value bag (not a typed HashMap); we wrap the map
        // into `Value::Object` here so the bridge stays typed at the
        // boundary while the plugin keeps its `Value` shape internally.
        let payload = props.map(|map| serde_json::Value::Object(map.into_iter().collect()));
        let _ = app.track_event(&name, payload);
    }
    Ok(())
}

// `supabase_sign_in_with_password` — Phase 1D T089.
//
// Replaces the JS `supabase-js` `signInWithPassword` call. Authenticates
// against Supabase REST `/auth/v1/token?grant_type=password` and persists
// the resulting session to the app-data directory. The returned
// `auth::AuthSession` mirrors the Leptos-side `bridge::types::AuthSession`
// byte-for-byte on the wire.
//
// Network/HTTP failure → `BridgeError::Internal`. Invalid credentials
// (HTTP 400/401 from Supabase) → `BridgeError::InvalidArgument`.
// Empty email/password → `BridgeError::InvalidArgument` before any HTTP
// roundtrip. Spec FR-018 / Principle II: auth is opt-in; guest mode is
// unaffected because it's a separate localStorage flag, not a Supabase
// concept.
#[tauri::command]
async fn supabase_sign_in_with_password(
    email: String,
    password: String,
    app: AppHandle,
) -> Result<auth::AuthSession, BridgeError> {
    let session = auth::sign_in_with_password(&email, &password).await?;
    let app_data_dir = app.path().app_data_dir().map_err(|e| BridgeError::Internal {
        msg: format!("Failed to get app data directory: {e}"),
    })?;
    auth::persist_session(&app_data_dir, &session)?;
    Ok(session)
}

// `supabase_sign_out` — Phase 1D T091.
//
// Replaces the JS `supabase-js` `signOut` call. POSTs to Supabase REST
// `/auth/v1/logout` to revoke the refresh token server-side, then
// removes the persisted session file from the app-data dir. Network
// failure on the REST call is tolerated (best-effort revocation —
// matches supabase-js's same-named behaviour); the local clear is
// always attempted so the user is signed out client-side regardless
// of network status.
//
// Empty `refresh_token` → `InvalidArgument` before any HTTP roundtrip
// or filesystem touch. Filesystem errors during the local clear (other
// than NotFound, which is absorbed as the idempotent no-op) → `Internal`.
#[tauri::command]
async fn supabase_sign_out(refresh_token: String, app: AppHandle) -> Result<(), BridgeError> {
    auth::sign_out(&refresh_token).await?;
    let app_data_dir = app.path().app_data_dir().map_err(|e| BridgeError::Internal {
        msg: format!("Failed to get app data directory: {e}"),
    })?;
    auth::clear_session(&app_data_dir)
}

// `supabase_get_session` — Phase 1D T093.
//
// Reads the persisted Supabase session from the app-data directory and
// returns `Some(AuthSession)` when a session is present, `None` for the
// cold-start (no-file) case. No-arg by design: the JS-era code path
// invoked `supabase.auth.getSession()` which read from localStorage; we
// move the read Rust-side per research.md §6 Decision §6 (single
// source of truth lives below the bridge).
#[tauri::command]
async fn supabase_get_session(app: AppHandle) -> Result<Option<auth::AuthSession>, BridgeError> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| BridgeError::Internal {
        msg: format!("Failed to get app data directory: {e}"),
    })?;
    auth::read_session(&app_data_dir)
}

#[tauri::command]
async fn set_dock_visibility(_app: AppHandle, _visible: bool) -> Result<(), BridgeError> {
    #[cfg(target_os = "macos")]
    {
        _app.run_on_main_thread(move || {
            set_dock_visibility_native(_visible);
        })
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to run on main thread: {e}") })?;
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err(BridgeError::Internal { msg: "Dock visibility is only supported on macOS".to_string() })
    }
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

#[tauri::command]
async fn set_status_bar_visibility(_app: AppHandle, _visible: bool) -> Result<(), BridgeError> {
    #[cfg(target_os = "macos")]
    {
        use std::sync::{Arc, Mutex};

        let result = Arc::new(Mutex::new(Ok(())));
        let result_clone = Arc::clone(&result);

        _app.run_on_main_thread(move || {
            let mut result_guard = helpers::lock_or_recover(&result_clone);
            *result_guard = match set_system_ui_mode_safe(_visible) {
                Ok(()) => {
                    log::info!(
                        "✅ Status bar visibility successfully set to: {}",
                        if _visible { "visible" } else { "hidden" }
                    );
                    Ok(())
                }
                Err(e) => {
                    log::error!("❌ Failed to set status bar visibility: {e}");
                    Err(BridgeError::Internal { msg: format!("Failed to set status bar visibility: {e}") })
                }
            };
        })
        .map_err(|e| BridgeError::Internal { msg: format!("Failed to run on main thread: {e}") })?;

        // Extract the result from the mutex (named binding required by borrow checker:
        // the temporary MutexGuard must drop before `result` does).
        let final_result = helpers::lock_or_recover(&result).clone();
        final_result
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err(BridgeError::Internal { msg: "Status bar visibility is only supported on macOS".to_string() })
    }
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn set_system_ui_mode_safe(visible: bool) -> Result<(), String> {
    use cocoa::appkit::{NSApp, NSApplication, NSApplicationPresentationOptions};
    use cocoa::base::nil;

    // SAFETY: NSApp() returns a raw pointer that is nil if no shared NSApplication exists.
    // We null-check before calling setPresentationOptions_, and this function is only
    // invoked from the main thread via run_on_main_thread, satisfying AppKit's requirement.
    unsafe {
        let app = NSApp();
        if app == nil {
            return Err(BridgeError::Internal { msg: "NSApplication shared instance is nil".to_string() });
        }

        let options = if visible {
            NSApplicationPresentationOptions::NSApplicationPresentationDefault
        } else {
            NSApplicationPresentationOptions::NSApplicationPresentationHideMenuBar
                | NSApplicationPresentationOptions::NSApplicationPresentationHideDock
        };

        app.setPresentationOptions_(options);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        default_analytics_enabled, default_weekly_goal, AppSettings, ManualSession,
        PomodoroSession, SessionTag, Tag, Task,
    };

    #[test]
    fn weekly_goal_default_is_125() {
        assert_eq!(default_weekly_goal(), 125);
    }

    #[test]
    fn analytics_enabled_default_is_true() {
        assert!(default_analytics_enabled());
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
        assert_eq!(s.analytics_enabled, defaults.analytics_enabled);
        assert_eq!(s.hide_icon_on_close, defaults.hide_icon_on_close);
        assert_eq!(s.hide_status_bar, defaults.hide_status_bar);
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
        assert_eq!(s.advanced.debug_mode, defaults.advanced.debug_mode);
    }

    #[test]
    fn app_settings_default_has_expected_values() {
        let s = AppSettings::default();
        assert_eq!(s.timer.focus_duration, 25);
        assert_eq!(s.timer.break_duration, 5);
        assert_eq!(s.timer.long_break_duration, 20);
        assert_eq!(s.timer.total_sessions, 10);
        assert_eq!(s.timer.weekly_goal_minutes, 125);
        assert!(s.analytics_enabled);
        assert!(!s.autostart);
        assert!(!s.hide_icon_on_close);
        assert!(!s.hide_status_bar);
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
    }

    #[test]
    fn pomodoro_session_serializes_and_deserializes() {
        let session = PomodoroSession {
            completed_pomodoros: 3,
            total_focus_time: 4500,
            current_session: 2,
            date: "Mon Jan 01 2024".to_string(),
        };
        let json = serde_json::to_string(&session).unwrap();
        let parsed: PomodoroSession = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.completed_pomodoros, session.completed_pomodoros);
        assert_eq!(parsed.total_focus_time, session.total_focus_time);
        assert_eq!(parsed.current_session, session.current_session);
        assert_eq!(parsed.date, session.date);
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
        };
        let json = serde_json::to_string(&session_with_tags).unwrap();
        let parsed: ManualSession = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "session-1");
        assert_eq!(parsed.duration, 25);
        assert_eq!(parsed.session_type, super::SessionType::Focus);
        assert!(parsed.notes.is_some());
        assert!(parsed.tags.is_some());

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
        };
        let json = serde_json::to_string(&session_no_extras).unwrap();
        let parsed: ManualSession = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.session_type, super::SessionType::Break);
        assert!(parsed.notes.is_none());
        assert!(parsed.tags.is_none());
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
}
