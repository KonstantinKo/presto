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

mod helpers;

// Type alias for the app handle to avoid generic complexity
type AppHandle = tauri::AppHandle<tauri::Wry>;

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

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ManualSession {
    id: String,
    session_type: String, // "focus", "break", "longBreak", "custom"
    duration: u32,        // in minutes
    start_time: String,   // "HH:MM"
    end_time: String,     // "HH:MM"
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

async fn are_analytics_enabled(app: &AppHandle) -> bool {
    match load_settings(app.clone()).await {
        Ok(settings) => settings.analytics_enabled,
        Err(_) => true, // Default to enabled if we can't load settings
    }
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

                    let _ = app_handle.emit("user-activity", ());
                } else {
                    let elapsed = {
                        let last = helpers::lock_or_recover(&last_activity);
                        last.elapsed()
                    };

                    if elapsed >= threshold {
                        let _ = app_handle.emit("user-inactivity", ());

                        // Reset the timer to avoid spam
                        {
                            let mut last = helpers::lock_or_recover(&last_activity);
                            *last = Instant::now();
                        }
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
async fn start_activity_monitoring(app: AppHandle, timeout_seconds: u64) -> Result<(), String> {
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
        Err("Activity monitoring is only supported on macOS".to_string())
    }
}

#[tauri::command]
async fn stop_activity_monitoring() -> Result<(), String> {
    {
        let monitor = helpers::lock_or_recover(&ACTIVITY_MONITOR);
        if let Some(ref m) = *monitor {
            m.stop_monitoring();
        }
    }
    Ok(())
}

#[tauri::command]
async fn update_activity_timeout(timeout_seconds: u64) -> Result<(), String> {
    let monitor = helpers::lock_or_recover(&ACTIVITY_MONITOR);
    monitor.as_ref().map_or_else(
        || Err("Activity monitor not initialized".to_string()),
        |m| {
            m.update_threshold(timeout_seconds);
            Ok(())
        },
    )
}

#[tauri::command]
async fn save_session_data(session: PomodoroSession, app: AppHandle) -> Result<(), String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {e}"))?;

    fs::create_dir_all(&app_data_dir).map_err(|e| format!("Failed to create directory: {e}"))?;

    let file_path = app_data_dir.join("session.json");
    helpers::write_json_atomic(&file_path, &session)?;

    if are_analytics_enabled(&app).await {
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
async fn load_session_data(app: AppHandle) -> Result<Option<PomodoroSession>, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {e}"))?;
    let file_path = app_data_dir.join("session.json");

    if !file_path.exists() {
        return Ok(None);
    }

    let content =
        fs::read_to_string(&file_path).map_err(|e| format!("Failed to read session file: {e}"))?;
    let mut session: PomodoroSession =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse session: {e}"))?;

    let today_legacy = chrono::Local::now().format("%a %b %d %Y").to_string();
    let today_iso = chrono::Local::now().format("%Y-%m-%d").to_string();

    let is_same_day = session.date == today_legacy
        || session.date == today_iso
        || chrono::NaiveDate::parse_from_str(&session.date, "%a %b %d %Y")
            .is_ok_and(|d| d.format("%Y-%m-%d").to_string() == today_iso);

    if is_same_day && session.date != today_legacy {
        session.date.clone_from(&today_legacy);
        helpers::write_json_atomic(&file_path, &session)?;
    } else if !is_same_day {
        session.completed_pomodoros = 0;
        session.total_focus_time = 0;
        session.current_session = 1;
        session.date.clone_from(&today_legacy);
        helpers::write_json_atomic(&file_path, &session)?;
    }

    Ok(Some(session))
}

#[tauri::command]
async fn save_tasks(tasks: Vec<Task>, app: AppHandle) -> Result<(), String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {e}"))?;

    fs::create_dir_all(&app_data_dir).map_err(|e| format!("Failed to create directory: {e}"))?;

    let file_path = app_data_dir.join("tasks.json");
    helpers::write_json_atomic(&file_path, &tasks)?;

    if are_analytics_enabled(&app).await {
        let _ = app.track_event("tasks_saved", None);
    }

    Ok(())
}

#[tauri::command]
async fn load_tasks(app: AppHandle) -> Result<Vec<Task>, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {e}"))?;
    let file_path = app_data_dir.join("tasks.json");

    if !file_path.exists() {
        return Ok(Vec::new());
    }

    let content =
        fs::read_to_string(file_path).map_err(|e| format!("Failed to read tasks file: {e}"))?;
    let tasks: Vec<Task> =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse tasks: {e}"))?;

    Ok(tasks)
}

#[tauri::command]
async fn get_stats_history(app: AppHandle) -> Result<Vec<PomodoroSession>, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {e}"))?;
    let history_path = app_data_dir.join("history.json");

    if !history_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(history_path)
        .map_err(|e| format!("Failed to read history file: {e}"))?;
    let history: Vec<PomodoroSession> =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse history: {e}"))?;

    Ok(history)
}

#[tauri::command]
async fn save_daily_stats(session: PomodoroSession, app: AppHandle) -> Result<(), String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {e}"))?;

    fs::create_dir_all(&app_data_dir).map_err(|e| format!("Failed to create directory: {e}"))?;

    let history_path = app_data_dir.join("history.json");

    let mut history: Vec<PomodoroSession> = if history_path.exists() {
        let content = fs::read_to_string(&history_path)
            .map_err(|e| format!("Failed to read history: {e}"))?;
        serde_json::from_str(&content).unwrap_or_else(|e| {
            log::warn!("Failed to parse history.json, starting fresh: {e}");
            Vec::new()
        })
    } else {
        Vec::new()
    };

    history.retain(|s| s.date != session.date);
    history.push(session);

    // Keep only last 30 days
    history.sort_by(|a, b| a.date.cmp(&b.date));
    if history.len() > 30 {
        let start_index = history.len() - 30;
        history.drain(0..start_index);
    }

    helpers::write_json_atomic(&history_path, &history)?;

    Ok(())
}

#[tauri::command]
async fn update_tray_icon(
    app: AppHandle,
    timer_text: String,
    is_running: bool,
    session_mode: String,
    current_session: u32,
    total_sessions: u32,
    mode_icon: Option<String>,
) -> Result<(), String> {
    use std::sync::{Arc, Mutex};

    // Use Arc<Mutex<Result<(), String>>> to capture the result from the main thread
    let result = Arc::new(Mutex::new(Ok(())));
    let result_clone = Arc::clone(&result);

    let app_clone = app.clone();

    // Move the operation to the main thread using Tauri's app handle
    // This ensures macOS tray operations run on the main thread
    app.run_on_main_thread(move || {
        let mut result_guard = helpers::lock_or_recover(&result_clone);
        *result_guard = (|| -> Result<(), String> {
            if let Some(tray) = app_clone.tray_by_id("main") {
                let icon = mode_icon.unwrap_or_else(|| match session_mode.as_str() {
                    "focus" => "🧠".to_string(),
                    "break" => "☕".to_string(),
                    "longBreak" => "🌙".to_string(),
                    _ => "⏱️".to_string(),
                });

                let status = if is_running { "Running" } else { "Paused" };
                let title = format!("{icon} {timer_text}");
                tray.set_title(Some(title))
                    .map_err(|e| format!("Failed to set title: {e}"))?;

                let tooltip = if session_mode == "focus" {
                    format!("Presto - Session {current_session}/{total_sessions} ({status})")
                } else {
                    format!(
                        "Presto - {} ({})",
                        if session_mode == "longBreak" {
                            "Long Break"
                        } else {
                            "Short Break"
                        },
                        status
                    )
                };

                tray.set_tooltip(Some(tooltip))
                    .map_err(|e| format!("Failed to set tooltip: {e}"))?;
            }
            Ok(())
        })();
    })
    .map_err(|e| format!("Failed to run on main thread: {e}"))?;

    // Extract the result from the mutex (named binding required by borrow checker:
    // the temporary MutexGuard must drop before `result` does).
    let final_result = helpers::lock_or_recover(&result).clone();
    final_result
}

#[tauri::command]
async fn show_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        if let Ok(settings) = load_settings(app.clone()).await {
            if settings.hide_icon_on_close {
                #[cfg(target_os = "macos")]
                {
                    let _ = set_dock_visibility(app.clone(), true).await;
                }
            }
        }

        window
            .show()
            .map_err(|e| format!("Failed to show window: {e}"))?;
        window
            .set_focus()
            .map_err(|e| format!("Failed to focus window: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
async fn save_settings(settings: AppSettings, app: AppHandle) -> Result<(), String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {e}"))?;

    fs::create_dir_all(&app_data_dir).map_err(|e| format!("Failed to create directory: {e}"))?;

    let file_path = app_data_dir.join("settings.json");
    helpers::write_json_atomic(&file_path, &settings)?;

    Ok(())
}

#[tauri::command]
async fn load_settings(app: AppHandle) -> Result<AppSettings, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {e}"))?;
    let file_path = app_data_dir.join("settings.json");

    if !file_path.exists() {
        return Ok(AppSettings::default());
    }

    let contents =
        fs::read_to_string(file_path).map_err(|e| format!("Failed to read settings file: {e}"))?;
    let settings: AppSettings =
        serde_json::from_str(&contents).map_err(|e| format!("Failed to parse settings: {e}"))?;

    Ok(settings)
}

#[tauri::command]
async fn register_global_shortcuts(
    app: AppHandle,
    shortcuts: ShortcutSettings,
) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| format!("Failed to unregister shortcuts: {e}"))?;

    for (action, shortcut_str) in [
        ("start-stop", &shortcuts.start_stop),
        ("reset", &shortcuts.reset),
        ("skip", &shortcuts.skip),
    ] {
        if let Some(ref shortcut_str) = shortcut_str {
            let shortcut: Shortcut = shortcut_str
                .parse()
                .map_err(|e| format!("Invalid {action} shortcut '{shortcut_str}': {e}"))?;

            let app_handle = app.clone();
            let action_owned = action.to_string();
            app.global_shortcut()
                .on_shortcut(shortcut, move |_app, _shortcut, _event| {
                    if !should_debounce_shortcut(&action_owned) {
                        let _ = app_handle.emit("global-shortcut", action_owned.as_str());
                    }
                })
                .map_err(|e| format!("Failed to register {action} shortcut: {e}"))?;
        }
    }

    // Emit an event to the frontend to update local shortcuts as well
    app.emit("shortcuts-updated", &shortcuts)
        .map_err(|e| format!("Failed to emit shortcuts update: {e}"))?;

    Ok(())
}

#[tauri::command]
async fn unregister_global_shortcuts(app: AppHandle) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| format!("Failed to unregister shortcuts: {e}"))?;
    Ok(())
}

#[tauri::command]
async fn reset_all_data(app: AppHandle) -> Result<(), String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {e}"))?;

    let files_to_delete = [
        "session.json",
        "tasks.json",
        "history.json",
        "settings.json",
        "manual_sessions.json",
        "tags.json",
        "session_tags.json",
    ];

    for file_name in &files_to_delete {
        let file_path = app_data_dir.join(file_name);
        if file_path.exists() {
            fs::remove_file(&file_path)
                .map_err(|e| format!("Failed to delete {file_name}: {e}"))?;
        }
    }

    Ok(())
}

#[tauri::command]
async fn enable_autostart(app: AppHandle) -> Result<(), String> {
    app.autolaunch()
        .enable()
        .map_err(|e| format!("Failed to enable autostart: {e}"))
}

#[tauri::command]
async fn disable_autostart(app: AppHandle) -> Result<(), String> {
    app.autolaunch()
        .disable()
        .map_err(|e| format!("Failed to disable autostart: {e}"))
}

#[tauri::command]
async fn is_autostart_enabled(app: AppHandle) -> Result<bool, String> {
    app.autolaunch()
        .is_enabled()
        .map_err(|e| format!("Failed to check autostart status: {e}"))
}

#[tauri::command]
async fn save_manual_sessions(sessions: Vec<ManualSession>, app: AppHandle) -> Result<(), String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {e}"))?;

    fs::create_dir_all(&app_data_dir).map_err(|e| format!("Failed to create directory: {e}"))?;

    let file_path = app_data_dir.join("manual_sessions.json");
    helpers::write_json_atomic(&file_path, &sessions)?;

    if are_analytics_enabled(&app).await {
        let properties = Some(serde_json::json!({
            "session_count": sessions.len()
        }));
        let _ = app.track_event("manual_sessions_saved", properties);
    }

    Ok(())
}

#[tauri::command]
async fn load_manual_sessions(app: AppHandle) -> Result<Vec<ManualSession>, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {e}"))?;
    let file_path = app_data_dir.join("manual_sessions.json");

    if !file_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read manual sessions file: {e}"))?;
    let sessions: Vec<ManualSession> = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse manual sessions: {e}"))?;

    Ok(sessions)
}

#[tauri::command]
async fn save_manual_session(session: ManualSession, app: AppHandle) -> Result<(), String> {
    let mut sessions = load_manual_sessions(app.clone()).await?;

    // Remove existing session with same ID if it exists (for updates)
    sessions.retain(|s| s.id != session.id);

    sessions.push(session);

    save_manual_sessions(sessions, app).await
}

#[tauri::command]
async fn delete_manual_session(session_id: String, app: AppHandle) -> Result<(), String> {
    let mut sessions = load_manual_sessions(app.clone()).await?;

    sessions.retain(|s| s.id != session_id);

    save_manual_sessions(sessions, app).await
}

#[tauri::command]
async fn get_manual_sessions_for_date(
    date: String,
    app: AppHandle,
) -> Result<Vec<ManualSession>, String> {
    let sessions = load_manual_sessions(app).await?;
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
                set_status_bar_visibility
            ])
            .setup(|app| {
                let app_handle_analytics = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if are_analytics_enabled(&app_handle_analytics).await {
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
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "start_session" => {
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.emit("tray-start-session", ());
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "pause" => {
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.emit("tray-pause", ());
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "skip" => {
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.emit("tray-skip", ());
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "cancel" => {
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.emit("tray-cancel", ());
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            app_handle.exit(0);
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(move |_tray, event| {
                        if let TrayIconEvent::Click { .. } = event {
                            if let Some(window) = app_handle_for_click.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    })
                    .build(app)?;

                if let Some(window) = app.get_webview_window("main") {
                    let app_handle_for_close = app.handle().clone();
                    window.on_window_event(move |event| {
                        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                            api.prevent_close();

                            let app_handle_clone = app_handle_for_close.clone();
                            tauri::async_runtime::spawn(async move {
                                match load_settings(app_handle_clone.clone()).await {
                                    Ok(settings) => {
                                        if settings.hide_icon_on_close {
                                            if let Some(window) =
                                                app_handle_clone.get_webview_window("main")
                                            {
                                                let _ = window.hide();
                                                #[cfg(target_os = "macos")]
                                                {
                                                    let _ = set_dock_visibility(
                                                        app_handle_clone.clone(),
                                                        false,
                                                    )
                                                    .await;
                                                }
                                            }
                                        } else {
                                            // Just hide the window without hiding from dock
                                            if let Some(window) =
                                                app_handle_clone.get_webview_window("main")
                                            {
                                                let _ = window.hide();
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        // Default behavior: just hide the window
                                        if let Some(window) =
                                            app_handle_clone.get_webview_window("main")
                                        {
                                            let _ = window.hide();
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
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                        // If the app was previously hidden from dock, restore it
                        let app_handle_clone = app_handle.clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = set_dock_visibility(app_handle_clone, true).await;
                        });
                    }
                }
                _ => {}
            });
    });
}

#[tauri::command]
async fn load_tags(app: AppHandle) -> Result<Vec<Tag>, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {e}"))?;

    let file_path = app_data_dir.join("tags.json");

    if file_path.exists() {
        let content =
            fs::read_to_string(&file_path).map_err(|e| format!("Failed to read tags: {e}"))?;
        Ok(
            serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse tags.json: {e}"))?,
        )
    } else {
        // Return default focus tag if no tags exist
        let default_tag = Tag {
            id: "default-focus".to_string(),
            name: "Focus".to_string(),
            icon: "ri-brain-line".to_string(),
            color: "#4CAF50".to_string(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .to_string(),
        };
        // Persist default tag so subsequent loads are consistent
        let tags = vec![default_tag];
        fs::create_dir_all(&app_data_dir)
            .map_err(|e| format!("Failed to create directory: {e}"))?;
        helpers::write_json_atomic(&file_path, &tags)?;
        Ok(tags)
    }
}

#[tauri::command]
async fn save_tags(tags: Vec<Tag>, app: AppHandle) -> Result<(), String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {e}"))?;

    fs::create_dir_all(&app_data_dir).map_err(|e| format!("Failed to create directory: {e}"))?;

    let file_path = app_data_dir.join("tags.json");
    helpers::write_json_atomic(&file_path, &tags)?;

    Ok(())
}

#[tauri::command]
async fn save_tag(tag: Tag, app: AppHandle) -> Result<(), String> {
    let mut tags = load_tags(app.clone()).await?;

    // Remove existing tag with same ID if it exists (for updates)
    tags.retain(|t| t.id != tag.id);

    tags.push(tag);

    save_tags(tags, app).await
}

#[tauri::command]
async fn delete_tag(tag_id: String, app: AppHandle) -> Result<(), String> {
    let mut tags = load_tags(app.clone()).await?;

    tags.retain(|t| t.id != tag_id);

    save_tags(tags, app).await
}

#[tauri::command]
async fn load_session_tags(app: AppHandle) -> Result<Vec<SessionTag>, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {e}"))?;

    let file_path = app_data_dir.join("session_tags.json");

    if file_path.exists() {
        let content = fs::read_to_string(&file_path)
            .map_err(|e| format!("Failed to read session tags: {e}"))?;
        Ok(serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse session_tags.json: {e}"))?)
    } else {
        Ok(Vec::new())
    }
}

#[tauri::command]
async fn save_session_tags(session_tags: Vec<SessionTag>, app: AppHandle) -> Result<(), String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {e}"))?;

    fs::create_dir_all(&app_data_dir).map_err(|e| format!("Failed to create directory: {e}"))?;

    let file_path = app_data_dir.join("session_tags.json");
    helpers::write_json_atomic(&file_path, &session_tags)?;

    Ok(())
}

#[tauri::command]
async fn add_session_tag(session_tag: SessionTag, app: AppHandle) -> Result<(), String> {
    let mut session_tags = load_session_tags(app.clone()).await?;
    session_tags.push(session_tag);
    save_session_tags(session_tags, app).await
}

#[tauri::command]
async fn update_tray_menu(
    app: AppHandle,
    is_running: bool,
    is_paused: bool,
    current_mode: String,
) -> Result<(), String> {
    let tray = app.tray_by_id("main");

    if let Some(tray) = tray {
        let show_item = MenuItem::with_id(&app, "show", "Show Presto", true, None::<&str>)
            .map_err(|e| format!("Failed to create show item: {e}"))?;

        // Start Session: enabled only if not running
        let start_session_item = MenuItem::with_id(
            &app,
            "start_session",
            "Start Session",
            !is_running,
            None::<&str>,
        )
        .map_err(|e| format!("Failed to create start session item: {e}"))?;

        // Pause: enabled only if running and not paused
        let pause_item = MenuItem::with_id(
            &app,
            "pause",
            "Pause",
            is_running && !is_paused,
            None::<&str>,
        )
        .map_err(|e| format!("Failed to create pause item: {e}"))?;

        // Skip: enabled only if running
        let skip_item = MenuItem::with_id(&app, "skip", "Skip Session", is_running, None::<&str>)
            .map_err(|e| format!("Failed to create skip item: {e}"))?;

        // Cancel: enabled if in focus mode, disabled in break/longBreak (undo)
        let cancel_text = if current_mode == "focus" {
            "Cancel"
        } else {
            "Cancel Last"
        };
        let cancel_item = MenuItem::with_id(&app, "cancel", cancel_text, true, None::<&str>)
            .map_err(|e| format!("Failed to create cancel item: {e}"))?;

        let quit_item = MenuItem::with_id(&app, "quit", "Quit", true, None::<&str>)
            .map_err(|e| format!("Failed to create quit item: {e}"))?;

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
        .map_err(|e| format!("Failed to create menu: {e}"))?;

        tray.set_menu(Some(new_menu))
            .map_err(|e| format!("Failed to set tray menu: {e}"))?;
    }

    Ok(())
}

#[tauri::command]
async fn write_excel_file(path: String, data: String) -> Result<(), String> {
    let decoded_data = general_purpose::STANDARD
        .decode(data)
        .map_err(|e| format!("Failed to decode base64 data: {e}"))?;

    fs::write(&path, decoded_data)
        .map_err(|e| format!("Failed to write Excel file to {path}: {e}"))?;

    Ok(())
}

#[tauri::command]
async fn start_oauth_server(window: tauri::Window) -> Result<u16, String> {
    start(move |url| {
        let _ = window.emit("oauth-callback", url);
    })
    .map_err(|err| err.to_string())
}

#[tauri::command]
async fn set_dock_visibility(_app: AppHandle, _visible: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        _app.run_on_main_thread(move || {
            set_dock_visibility_native(_visible);
        })
        .map_err(|e| format!("Failed to run on main thread: {e}"))?;
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("Dock visibility is only supported on macOS".to_string())
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
async fn set_status_bar_visibility(_app: AppHandle, _visible: bool) -> Result<(), String> {
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
                    Err(format!("Failed to set status bar visibility: {e}"))
                }
            };
        })
        .map_err(|e| format!("Failed to run on main thread: {e}"))?;

        // Extract the result from the mutex (named binding required by borrow checker:
        // the temporary MutexGuard must drop before `result` does).
        let final_result = helpers::lock_or_recover(&result).clone();
        final_result
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("Status bar visibility is only supported on macOS".to_string())
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
            return Err("NSApplication shared instance is nil".to_string());
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
            session_type: "focus".to_string(),
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
        assert!(parsed.notes.is_some());
        assert!(parsed.tags.is_some());

        let session_no_extras = ManualSession {
            id: "session-2".to_string(),
            session_type: "break".to_string(),
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
}
