// `TimerMode` — closed-domain enum for the live-engine session mode.
//
// Spec 001-leptos-migration §Phase 1A T027 (Tauri-side); Phase 1C T076-T079
// (Leptos-side mirror); data-model.md §`TimerMode`.
//
// Wire form: camelCase strings (`"focus"`, `"break"`, `"longBreak"`) via
// `#[serde(rename_all = "camelCase")]`. Distinct from `SessionType` (T028-
// T029) because manual sessions can carry the `Custom` variant; the live
// engine cannot. Tauri-side handlers `update_tray_icon.session_mode` and
// `update_tray_menu.current_mode` tightened from `String` to this enum in
// Phase 1A T027 — Phase 1C wires the Leptos-side mirror so the same wire
// shape round-trips with no string drift (FR-008 compile-time-mismatch
// promise; FR-013 closed-domain enum).

use serde::{Deserialize, Serialize};

/// Closed-domain variant for the live-engine session mode.
///
/// Wire form: camelCase strings. The Tauri-side mirror at
/// `src-tauri/src/lib.rs:84` has byte-identical serde shape so a value
/// crosses the bridge without translation.
///
/// Distinct from `SessionType` (`Custom` exists there for manual entries
/// and has no equivalent here — the live engine only runs `Focus`,
/// `Break`, and `LongBreak` modes per the pomodoro contract).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TimerMode {
    Focus,
    Break,
    LongBreak,
}

#[cfg(test)]
mod tests {
    use super::TimerMode;

    #[test]
    fn focus_serialises_camelcase() {
        let json = serde_json::to_string(&TimerMode::Focus).unwrap();
        assert_eq!(json, r#""focus""#);
    }

    #[test]
    fn break_serialises_camelcase() {
        let json = serde_json::to_string(&TimerMode::Break).unwrap();
        assert_eq!(json, r#""break""#);
    }

    #[test]
    fn long_break_serialises_camelcase() {
        let json = serde_json::to_string(&TimerMode::LongBreak).unwrap();
        assert_eq!(json, r#""longBreak""#);
    }

    #[test]
    fn focus_round_trips() {
        let decoded: TimerMode = serde_json::from_str(r#""focus""#).unwrap();
        assert_eq!(decoded, TimerMode::Focus);
    }

    #[test]
    fn break_round_trips() {
        let decoded: TimerMode = serde_json::from_str(r#""break""#).unwrap();
        assert_eq!(decoded, TimerMode::Break);
    }

    #[test]
    fn long_break_round_trips() {
        let decoded: TimerMode = serde_json::from_str(r#""longBreak""#).unwrap();
        assert_eq!(decoded, TimerMode::LongBreak);
    }

    /// `Custom` is a `SessionType`-only variant. Pinning that the legacy
    /// JS-era wire form `"custom"` does NOT deserialise as a `TimerMode`
    /// guards against an accidental enum widening — the Tauri-side mirror
    /// at `src-tauri/src/lib.rs:84` has the same closed shape and would
    /// fail the same way; the wrapper boundary surfaces the failure as
    /// `BridgeError::SerdeRoundtrip`.
    #[test]
    fn rejects_custom_variant_unique_to_session_type() {
        let result: Result<TimerMode, _> = serde_json::from_str(r#""custom""#);
        assert!(
            result.is_err(),
            "TimerMode must reject the SessionType-only Custom variant"
        );
    }

    /// Embedded as a struct field, the wire form pins the JSON object literal
    /// shape `{"session_mode": "longBreak"}` per data-model.md §`UpdateTrayIconArgs`.
    #[test]
    fn embedded_in_record_round_trips_long_break() {
        #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
        struct Record {
            session_mode: TimerMode,
        }
        let json = r#"{"session_mode":"longBreak"}"#;
        let decoded: Record = serde_json::from_str(json).unwrap();
        assert_eq!(
            decoded,
            Record {
                session_mode: TimerMode::LongBreak
            }
        );
        let re_encoded = serde_json::to_string(&decoded).unwrap();
        assert_eq!(re_encoded, json);
    }
}
