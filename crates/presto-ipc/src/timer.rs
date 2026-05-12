// Timer-domain enums.
//
// `TimerMode` — closed-domain enum for the live-engine session mode.
// `SessionType` — closed-domain enum for manual-session entries.
//
// Both share the camelCase wire form (`"focus"`, `"break"`,
// `"longBreak"`); `SessionType` adds `"custom"` for user-defined
// manual entries (the live engine cannot run this variant). Wire
// form mirrors data-model.md §`TimerMode` / §`SessionType`.

use serde::{Deserialize, Serialize};

/// Closed-domain variant for the live-engine session mode.
///
/// Wire form: camelCase strings.
///
/// Distinct from `SessionType`: `Custom` exists there for manual
/// entries and has no equivalent here — the live engine only runs
/// `Focus`, `Break`, and `LongBreak` per the pomodoro contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum TimerMode {
    Focus,
    Break,
    LongBreak,
}

/// Closed-domain variant for the `session_type` field on a manual
/// session record (`ManualSession.session_type`).
///
/// Wire form: camelCase strings. The `Custom` variant exists on the
/// manual-entry side (user-defined session shapes); the live engine's
/// `TimerMode` has no equivalent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum SessionType {
    Focus,
    Break,
    LongBreak,
    Custom,
}

#[cfg(test)]
mod tests {
    use super::{SessionType, TimerMode};

    #[test]
    fn timer_mode_focus_serialises_camelcase() {
        let json = serde_json::to_string(&TimerMode::Focus).unwrap();
        assert_eq!(json, r#""focus""#);
    }

    #[test]
    fn timer_mode_break_serialises_camelcase() {
        let json = serde_json::to_string(&TimerMode::Break).unwrap();
        assert_eq!(json, r#""break""#);
    }

    #[test]
    fn timer_mode_long_break_serialises_camelcase() {
        let json = serde_json::to_string(&TimerMode::LongBreak).unwrap();
        assert_eq!(json, r#""longBreak""#);
    }

    #[test]
    fn timer_mode_round_trips() {
        for (json, variant) in [
            (r#""focus""#, TimerMode::Focus),
            (r#""break""#, TimerMode::Break),
            (r#""longBreak""#, TimerMode::LongBreak),
        ] {
            let decoded: TimerMode = serde_json::from_str(json).unwrap();
            assert_eq!(decoded, variant);
        }
    }

    /// `Custom` is a `SessionType`-only variant. Pinning that the legacy
    /// JS-era wire form `"custom"` does NOT deserialise as a `TimerMode`
    /// guards against an accidental enum widening.
    #[test]
    fn timer_mode_rejects_custom_variant_unique_to_session_type() {
        let result: Result<TimerMode, _> = serde_json::from_str(r#""custom""#);
        assert!(
            result.is_err(),
            "TimerMode must reject the SessionType-only Custom variant"
        );
    }

    #[test]
    fn timer_mode_embedded_in_record_round_trips_long_break() {
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

    #[test]
    fn session_type_all_variants_serialise_camelcase() {
        assert_eq!(
            serde_json::to_string(&SessionType::Focus).unwrap(),
            r#""focus""#
        );
        assert_eq!(
            serde_json::to_string(&SessionType::Break).unwrap(),
            r#""break""#
        );
        assert_eq!(
            serde_json::to_string(&SessionType::LongBreak).unwrap(),
            r#""longBreak""#
        );
        assert_eq!(
            serde_json::to_string(&SessionType::Custom).unwrap(),
            r#""custom""#
        );
    }

    #[test]
    fn session_type_all_variants_round_trip() {
        for (json, variant) in [
            (r#""focus""#, SessionType::Focus),
            (r#""break""#, SessionType::Break),
            (r#""longBreak""#, SessionType::LongBreak),
            (r#""custom""#, SessionType::Custom),
        ] {
            let decoded: SessionType = serde_json::from_str(json).unwrap();
            assert_eq!(decoded, variant);
        }
    }

    #[test]
    fn session_type_embedded_in_record_round_trips_long_break() {
        #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
        struct Record {
            session_type: SessionType,
        }
        let json = r#"{"session_type":"longBreak"}"#;
        let decoded: Record = serde_json::from_str(json).unwrap();
        assert_eq!(
            decoded,
            Record {
                session_type: SessionType::LongBreak
            }
        );
        let re_encoded = serde_json::to_string(&decoded).unwrap();
        assert_eq!(re_encoded, json);
    }
}
