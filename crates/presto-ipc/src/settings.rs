// `Settings` (+ nested + `SettingsOnDisk` legacy-shape shim) — the
// canonical wire/on-disk record for all user preferences.
//
// Spec 001-leptos-migration §Phase 3a T150-T152;
// data-model.md §"Settings legacy migration".
//
// The Settings tree is the largest cross-crate type and was
// previously duplicated byte-for-byte between the Tauri backend and
// the Leptos frontend. Single-sourcing here closes a documented
// drift hazard (the JS-era `hide_status_bar → status_bar_display`
// migration had to be ported twice — once per crate).

use serde::{Deserialize, Serialize};

/// Status-bar visibility mode.
///
/// Replaces the legacy `hide_status_bar: bool` shape with a typed
/// enum so future "compact" or "hidden" modes don't fork the on-disk
/// encoding.
///
/// Wire shape: kebab-case strings (`"default"`, `"icon-only"`),
/// matching the JS-era on-disk values written by
/// `src/managers/settings-manager.js` after its `hide_status_bar →
/// status_bar_display` migration step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "kebab-case")]
pub enum StatusBarDisplay {
    /// Full status bar (timer text + icon).
    #[default]
    Default,
    /// Icon-only status bar — corresponds to the legacy
    /// `hide_status_bar: true` setting.
    IconOnly,
}

/// Keyboard-shortcut bindings bundle.
///
/// Each field is `Option<String>` because users can clear a binding
/// (the JS era stores `null` for cleared bindings, which serde maps
/// to `None`). Each string is a Tauri shortcut spec like
/// `"CommandOrControl+Alt+Space"`; parsing happens Rust-side at
/// `register_global_shortcuts` time.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ShortcutSettings {
    pub start_stop: Option<String>,
    pub reset: Option<String>,
    pub skip: Option<String>,
}

impl Default for ShortcutSettings {
    fn default() -> Self {
        Self {
            start_stop: Some("CommandOrControl+Alt+Space".to_string()),
            reset: Some("CommandOrControl+Alt+R".to_string()),
            skip: Some("CommandOrControl+Alt+S".to_string()),
        }
    }
}

/// Appearance / theme preferences.
///
/// `theme` is the color-mode preference (`"auto"` / `"light"` /
/// `"dark"`); `timer_theme` is the timer palette stem (e.g.
/// `"espresso"`). Both carry `#[serde(default)]` so pre-widening
/// settings JSONs fill in the JS-era cold-start values.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AppearanceSettings {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_timer_theme")]
    pub timer_theme: String,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            timer_theme: default_timer_theme(),
        }
    }
}

fn default_theme() -> String {
    "auto".to_string()
}

fn default_timer_theme() -> String {
    "espresso".to_string()
}

/// Timer durations & session count.
///
/// `weekly_goal_minutes` and `max_session_time` carry
/// `#[serde(default = "...")]` because settings JSON written by
/// pre-widening builds lacks those fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct TimerSettings {
    /// Minutes.
    pub focus_duration: u32,
    /// Minutes.
    pub break_duration: u32,
    /// Minutes.
    pub long_break_duration: u32,
    pub total_sessions: u32,
    #[serde(default = "default_weekly_goal")]
    pub weekly_goal_minutes: u32,
    /// Maximum continuous session time before auto-pause (minutes).
    #[serde(default = "default_max_session_time")]
    pub max_session_time: u32,
}

impl Default for TimerSettings {
    fn default() -> Self {
        Self {
            focus_duration: 25,
            break_duration: 5,
            long_break_duration: 20,
            total_sessions: 10,
            weekly_goal_minutes: default_weekly_goal(),
            max_session_time: default_max_session_time(),
        }
    }
}

/// Default weekly focus goal — 125 minutes per week.
#[must_use]
pub const fn default_weekly_goal() -> u32 {
    125
}

/// Default max single-session time — 120 minutes before auto-pause.
#[must_use]
pub const fn default_max_session_time() -> u32 {
    120
}

/// Notification preferences.
///
/// `auto_start_focus` and `allow_continuous_sessions` carry
/// `#[serde(default)]` because they were added after the `0.4.0`
/// settings shape and may be missing from older settings JSONs.
///
/// `clippy::struct_excessive_bools` is silenced because every bool
/// maps to an independent UI toggle; collapsing them into a state
/// machine would not match the on-disk JSON or the settings UI.
#[allow(clippy::struct_excessive_bools)]
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
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            desktop_notifications: true,
            sound_notifications: true,
            auto_start_timer: true,
            auto_start_focus: false,
            allow_continuous_sessions: false,
            smart_pause: false,
            smart_pause_timeout: 30,
        }
    }
}

/// Advanced / debug toggles.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AdvancedSettings {
    #[serde(default)]
    pub debug_mode: bool,
}

/// Full application settings record.
///
/// **Wire shape (post-Phase-3a)**: the legacy `hide_status_bar: bool`
/// field is replaced by `status_bar_display: StatusBarDisplay` per
/// the F1/M3 lockstep migration (Phase 3a T150 / T152). Legacy 0.4.x
/// settings JSONs that still carry `hide_status_bar` are read by the
/// `#[serde(from = "SettingsOnDisk")]` shim below: it accepts either
/// shape on the wire, projects through the legacy fallback, and the
/// derived `Serialize` impl then emits only the new shape on next
/// save (legacy field is gone — no field exists for it).
///
/// `clippy::struct_excessive_bools` allowance: every bool is an
/// independent settings toggle exposed in the UI; restructuring
/// would not match the JSON shape on disk or the settings page.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(from = "SettingsOnDisk")]
pub struct Settings {
    pub shortcuts: ShortcutSettings,
    pub timer: TimerSettings,
    pub notifications: NotificationSettings,
    #[serde(default)]
    pub advanced: AdvancedSettings,
    #[serde(default)]
    pub appearance: AppearanceSettings,
    pub autostart: bool,
    #[serde(default)]
    pub hide_icon_on_close: bool,
    pub status_bar_display: StatusBarDisplay,
    /// Update versions the user has dismissed from the update-banner.
    /// `#[serde(default)]` so 0.4.x settings JSONs predating this field
    /// still deserialise into the cold-start shape.
    #[serde(default)]
    pub skipped_versions: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            shortcuts: ShortcutSettings::default(),
            timer: TimerSettings::default(),
            notifications: NotificationSettings::default(),
            advanced: AdvancedSettings::default(),
            appearance: AppearanceSettings::default(),
            autostart: false,
            hide_icon_on_close: false,
            status_bar_display: StatusBarDisplay::Default,
            skipped_versions: Vec::new(),
        }
    }
}

/// On-disk shape of the settings JSON, accepting either the new
/// `status_bar_display: StatusBarDisplay` field or the legacy
/// `hide_status_bar: bool` field.
///
/// Used as the `#[serde(from = "SettingsOnDisk")]` source for
/// `Settings`; the `From<SettingsOnDisk> for Settings` impl below
/// ports the legacy fallback from
/// `src/managers/settings-manager.js:109-119`:
///
/// 1. If `status_bar_display` is present, use it.
/// 2. Else if `hide_status_bar: true`, use `IconOnly`.
/// 3. Else if `hide_status_bar: false`, use `Default`.
/// 4. Else, use `StatusBarDisplay::default()`.
///
/// Tie-breaker: when both fields are present, the new field wins.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SettingsOnDisk {
    pub shortcuts: ShortcutSettings,
    pub timer: TimerSettings,
    pub notifications: NotificationSettings,
    #[serde(default)]
    pub advanced: AdvancedSettings,
    #[serde(default)]
    pub appearance: AppearanceSettings,
    pub autostart: bool,
    #[serde(default)]
    pub hide_icon_on_close: bool,
    #[serde(default)]
    pub status_bar_display: Option<StatusBarDisplay>,
    /// Legacy read-only fallback. Never re-emitted on save.
    #[serde(default)]
    pub hide_status_bar: Option<bool>,
    #[serde(default)]
    pub skipped_versions: Vec<String>,
}

impl From<SettingsOnDisk> for Settings {
    fn from(raw: SettingsOnDisk) -> Self {
        let status_bar_display = raw.status_bar_display.unwrap_or(match raw.hide_status_bar {
            Some(true) => StatusBarDisplay::IconOnly,
            Some(false) | None => StatusBarDisplay::Default,
        });
        Self {
            shortcuts: raw.shortcuts,
            timer: raw.timer,
            notifications: raw.notifications,
            advanced: raw.advanced,
            appearance: raw.appearance,
            autostart: raw.autostart,
            hide_icon_on_close: raw.hide_icon_on_close,
            status_bar_display,
            skipped_versions: raw.skipped_versions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StatusBarDisplay;

    #[test]
    fn status_bar_display_default_is_default() {
        assert_eq!(StatusBarDisplay::default(), StatusBarDisplay::Default);
    }

    #[test]
    fn status_bar_display_serialises_kebab() {
        assert_eq!(
            serde_json::to_string(&StatusBarDisplay::Default).unwrap(),
            r#""default""#
        );
        assert_eq!(
            serde_json::to_string(&StatusBarDisplay::IconOnly).unwrap(),
            r#""icon-only""#
        );
    }

    #[test]
    fn status_bar_display_round_trips_kebab() {
        for (json, variant) in [
            (r#""default""#, StatusBarDisplay::Default),
            (r#""icon-only""#, StatusBarDisplay::IconOnly),
        ] {
            let decoded: StatusBarDisplay = serde_json::from_str(json).unwrap();
            assert_eq!(decoded, variant);
        }
    }
}
