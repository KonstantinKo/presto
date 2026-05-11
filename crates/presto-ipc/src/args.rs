// Top-level command Args structs.
//
// **camelCase** wire (Tauri 2's `#[tauri::command]` codegen
// auto-renames the args bag to camelCase; client structs must
// match). Single-source-of-truth for every Args struct that
// crosses the IPC boundary so the snake_case-vs-camelCase silent-
// rejection bug class can't recur.

use serde::{Deserialize, Serialize};

use crate::timer::TimerMode;

/// Argument bundle for `update_tray_icon`.
///
/// The Tauri-side handler takes a single `args: UpdateTrayIconArgs`
/// parameter (refactored from 6 positional args in Phase F so the
/// shape stays symmetric with every other command). Wire shape is
/// camelCase keys (`timerText`, `isRunning`, `sessionMode`,
/// `currentSession`, `totalSessions`, `modeIcon`).
///
/// `mode_icon: Option<String>` — when `None`, the handler picks the
/// emoji per `TimerMode` variant; when `Some("")`, the icon-only
/// status-bar mode is engaged.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UpdateTrayIconArgs {
    pub timer_text: String,
    pub is_running: bool,
    pub session_mode: TimerMode,
    pub current_session: u32,
    pub total_sessions: u32,
    pub mode_icon: Option<String>,
}

/// Argument bundle for `update_tray_menu`. Drives the start /
/// pause / skip / cancel menu-item enable state on the macOS
/// status item.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UpdateTrayMenuArgs {
    pub is_running: bool,
    pub is_paused: bool,
    pub current_mode: TimerMode,
}

/// Argument bundle for `update_activity_timeout`.
///
/// Hoisted out of `bridge::commands` to make the camelCase wire
/// shape part of the shared single-source-of-truth surface — the
/// original local `struct Args { timeout_seconds }` shipped without
/// `rename_all = "camelCase"` and silently failed at runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UpdateActivityTimeoutArgs {
    /// Seconds.
    pub timeout_seconds: u64,
}

/// Argument bundle for `start_activity_monitoring`. Same camelCase
/// rationale as `UpdateActivityTimeoutArgs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StartActivityMonitoringArgs {
    /// Seconds.
    pub timeout_seconds: u64,
}

/// Argument bundle for `delete_tag`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DeleteTagArgs {
    pub tag_id: String,
}

/// Argument bundle for `add_session_tag`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AddSessionTagArgs {
    pub session_tag: crate::tags::SessionTag,
}

/// Argument bundle for `supabase_sign_out` — Supabase REST
/// `/auth/v1/logout` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SupabaseSignOutArgs {
    pub refresh_token: String,
}

/// Argument bundle for `supabase_refresh_session` — Supabase REST
/// `/auth/v1/token?grant_type=refresh_token` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SupabaseRefreshSessionArgs {
    pub refresh_token: String,
}

#[cfg(test)]
mod tests {
    use super::{
        AddSessionTagArgs, DeleteTagArgs, StartActivityMonitoringArgs, SupabaseRefreshSessionArgs,
        SupabaseSignOutArgs, UpdateActivityTimeoutArgs, UpdateTrayIconArgs, UpdateTrayMenuArgs,
    };
    use crate::tags::SessionTag;
    use crate::timer::TimerMode;

    #[test]
    fn update_tray_icon_args_round_trips_camelcase() {
        let args = UpdateTrayIconArgs {
            timer_text: "24:59".to_string(),
            is_running: true,
            session_mode: TimerMode::Focus,
            current_session: 3,
            total_sessions: 10,
            mode_icon: Some("🍅".to_string()),
        };
        let json = serde_json::to_string(&args).unwrap();
        assert!(json.contains(r#""timerText":"24:59""#));
        assert!(json.contains(r#""isRunning":true"#));
        assert!(json.contains(r#""sessionMode":"focus""#));
        assert!(json.contains(r#""currentSession":3"#));
        assert!(json.contains(r#""totalSessions":10"#));
        assert!(json.contains(r#""modeIcon":"🍅""#));
        let decoded: UpdateTrayIconArgs = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.timer_text, "24:59");
        assert!(decoded.is_running);
        assert_eq!(decoded.session_mode, TimerMode::Focus);
        assert_eq!(decoded.current_session, 3);
        assert_eq!(decoded.mode_icon.as_deref(), Some("🍅"));
    }

    #[test]
    fn update_tray_icon_args_without_mode_icon_round_trips() {
        let args = UpdateTrayIconArgs {
            timer_text: "05:00".to_string(),
            is_running: false,
            session_mode: TimerMode::Break,
            current_session: 1,
            total_sessions: 10,
            mode_icon: None,
        };
        let json = serde_json::to_string(&args).unwrap();
        let decoded: UpdateTrayIconArgs = serde_json::from_str(&json).unwrap();
        assert!(decoded.mode_icon.is_none());
        assert_eq!(decoded.session_mode, TimerMode::Break);
    }

    #[test]
    fn update_tray_menu_args_round_trips_camelcase() {
        let args = UpdateTrayMenuArgs {
            is_running: true,
            is_paused: false,
            current_mode: TimerMode::LongBreak,
        };
        let json = serde_json::to_string(&args).unwrap();
        assert!(json.contains(r#""isRunning":true"#));
        assert!(json.contains(r#""isPaused":false"#));
        assert!(json.contains(r#""currentMode":"longBreak""#));
        let decoded: UpdateTrayMenuArgs = serde_json::from_str(&json).unwrap();
        assert!(decoded.is_running);
        assert!(!decoded.is_paused);
        assert_eq!(decoded.current_mode, TimerMode::LongBreak);
    }

    /// `update_activity_timeout` previously shipped with `snake_case`
    /// `{"timeout_seconds":...}` and silently failed at runtime because
    /// Tauri's auto-generated arg-deserializer expects camelCase. This
    /// test plus the parallel one on `StartActivityMonitoringArgs`
    /// guard against the same drift re-appearing.
    #[test]
    fn update_activity_timeout_args_serialises_camelcase_key() {
        let args = UpdateActivityTimeoutArgs {
            timeout_seconds: 600,
        };
        let json = serde_json::to_string(&args).unwrap();
        assert!(
            json.contains(r#""timeoutSeconds":600"#),
            "expected camelCase wire key (Tauri auto-renames to \
             camelCase); got: {json}",
        );
        assert!(
            !json.contains(r#""timeout_seconds""#),
            "snake_case wire key would silently fail at runtime",
        );
    }

    #[test]
    fn start_activity_monitoring_args_serialises_camelcase_key() {
        let args = StartActivityMonitoringArgs {
            timeout_seconds: 30,
        };
        let json = serde_json::to_string(&args).unwrap();
        assert!(json.contains(r#""timeoutSeconds":30"#));
        assert!(!json.contains(r#""timeout_seconds""#));
    }

    #[test]
    fn delete_tag_args_serialises_camelcase_key() {
        let args = DeleteTagArgs {
            tag_id: "tag-abc".to_string(),
        };
        let json = serde_json::to_string(&args).unwrap();
        assert!(json.contains(r#""tagId":"tag-abc""#));
        assert!(!json.contains(r#""tag_id""#));
    }

    #[test]
    fn add_session_tag_args_serialises_camelcase_outer_key() {
        let args = AddSessionTagArgs {
            session_tag: SessionTag {
                session_id: "s-1".to_string(),
                tag_id: "t-1".to_string(),
                duration: 30,
                created_at: "2026-05-10T00:00:00Z".to_string(),
            },
        };
        let json = serde_json::to_string(&args).unwrap();
        // Outer key is camelCase (Tauri-side); inner `SessionTag`
        // fields stay snake_case because that struct is its own
        // wire shape with its own serde defaults (matches on-disk
        // history.json).
        assert!(json.contains(r#""sessionTag":{"#));
        assert!(json.contains(r#""session_id":"s-1""#));
        assert!(json.contains(r#""tag_id":"t-1""#));
    }

    #[test]
    fn supabase_sign_out_args_serialises_camelcase_key() {
        let args = SupabaseSignOutArgs {
            refresh_token: "rt-1".to_string(),
        };
        let json = serde_json::to_string(&args).unwrap();
        assert!(json.contains(r#""refreshToken":"rt-1""#));
        assert!(!json.contains(r#""refresh_token""#));
    }

    #[test]
    fn supabase_refresh_session_args_serialises_camelcase_key() {
        let args = SupabaseRefreshSessionArgs {
            refresh_token: "rt-2".to_string(),
        };
        let json = serde_json::to_string(&args).unwrap();
        assert!(json.contains(r#""refreshToken":"rt-2""#));
        assert!(!json.contains(r#""refresh_token""#));
    }

    /// Defence-in-depth: every Args struct must have **top-level**
    /// keys without `snake_case` multi-word names. Inner-struct
    /// fields keep `snake_case` (matches on-disk shapes) and are
    /// exempt.
    ///
    /// Parses each payload via `serde_json::Value` so only the
    /// outermost object's keys are inspected — nested `SessionTag`
    /// fields are correctly ignored.
    #[test]
    fn every_args_struct_top_level_keys_are_camel_case() {
        let payloads: &[(&str, serde_json::Value)] = &[
            (
                "UpdateTrayIconArgs",
                serde_json::to_value(UpdateTrayIconArgs {
                    timer_text: "25:00".to_string(),
                    is_running: false,
                    session_mode: TimerMode::Focus,
                    current_session: 1,
                    total_sessions: 10,
                    mode_icon: None,
                })
                .unwrap(),
            ),
            (
                "UpdateTrayMenuArgs",
                serde_json::to_value(UpdateTrayMenuArgs {
                    is_running: false,
                    is_paused: false,
                    current_mode: TimerMode::Focus,
                })
                .unwrap(),
            ),
            (
                "UpdateActivityTimeoutArgs",
                serde_json::to_value(UpdateActivityTimeoutArgs { timeout_seconds: 0 }).unwrap(),
            ),
            (
                "StartActivityMonitoringArgs",
                serde_json::to_value(StartActivityMonitoringArgs { timeout_seconds: 0 }).unwrap(),
            ),
            (
                "DeleteTagArgs",
                serde_json::to_value(DeleteTagArgs {
                    tag_id: String::new(),
                })
                .unwrap(),
            ),
            (
                "AddSessionTagArgs",
                serde_json::to_value(AddSessionTagArgs {
                    session_tag: SessionTag {
                        session_id: String::new(),
                        tag_id: String::new(),
                        duration: 0,
                        created_at: String::new(),
                    },
                })
                .unwrap(),
            ),
            (
                "SupabaseSignOutArgs",
                serde_json::to_value(SupabaseSignOutArgs {
                    refresh_token: String::new(),
                })
                .unwrap(),
            ),
            (
                "SupabaseRefreshSessionArgs",
                serde_json::to_value(SupabaseRefreshSessionArgs {
                    refresh_token: String::new(),
                })
                .unwrap(),
            ),
        ];

        for (name, value) in payloads {
            let obj = value
                .as_object()
                .unwrap_or_else(|| panic!("{name} payload must serialise as a JSON object"));
            for key in obj.keys() {
                assert!(
                    !key.contains('_'),
                    "{name} has snake_case top-level key '{key}'; \
                     Tauri auto-rename to camelCase will reject the \
                     call silently. Add #[serde(rename_all = \
                     \"camelCase\")] to the Args struct.",
                );
            }
        }
    }
}
