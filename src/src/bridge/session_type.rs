// `SessionType` — closed-domain enum for manual-session entries.
//
// Spec 001-leptos-migration §Phase 1A T028-T029; data-model.md §`SessionType`.
//
// Wire form: camelCase strings (`"focus"`, `"break"`, `"longBreak"`,
// `"custom"`) via `#[serde(rename_all = "camelCase")]`. Distinct from
// `TimerMode` (live engine) because manual entries can carry the
// `Custom` variant for user-defined session shapes.
//
// The Tauri-side mirror lives in `src-tauri/src/lib.rs`; both definitions
// have byte-identical serde shapes so a `ManualSession` round-trips
// across the bridge without translation (FR-013 closed-domain enum;
// Principle VI typed boundary).

use serde::{Deserialize, Serialize};

/// Closed-domain variant for the `session_type` field on a manual
/// session record (`ManualSession.session_type`).
///
/// Wire form: camelCase strings. The `Custom` variant exists on the
/// manual-entry side (user-defined session shapes); the live engine's
/// `TimerMode` has no equivalent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionType {
    Focus,
    Break,
    LongBreak,
    Custom,
}

#[cfg(test)]
mod tests {
    use super::SessionType;

    #[test]
    fn focus_serialises_camelcase() {
        let json = serde_json::to_string(&SessionType::Focus).unwrap();
        assert_eq!(json, r#""focus""#);
    }

    #[test]
    fn break_serialises_camelcase() {
        let json = serde_json::to_string(&SessionType::Break).unwrap();
        assert_eq!(json, r#""break""#);
    }

    #[test]
    fn long_break_serialises_camelcase() {
        let json = serde_json::to_string(&SessionType::LongBreak).unwrap();
        assert_eq!(json, r#""longBreak""#);
    }

    #[test]
    fn custom_serialises_camelcase() {
        let json = serde_json::to_string(&SessionType::Custom).unwrap();
        assert_eq!(json, r#""custom""#);
    }

    #[test]
    fn focus_round_trips() {
        let decoded: SessionType = serde_json::from_str(r#""focus""#).unwrap();
        assert_eq!(decoded, SessionType::Focus);
    }

    #[test]
    fn break_round_trips() {
        let decoded: SessionType = serde_json::from_str(r#""break""#).unwrap();
        assert_eq!(decoded, SessionType::Break);
    }

    #[test]
    fn long_break_round_trips() {
        let decoded: SessionType = serde_json::from_str(r#""longBreak""#).unwrap();
        assert_eq!(decoded, SessionType::LongBreak);
    }

    #[test]
    fn custom_round_trips() {
        let decoded: SessionType = serde_json::from_str(r#""custom""#).unwrap();
        assert_eq!(decoded, SessionType::Custom);
    }

    /// Embedded as a struct field, the wire form pins the JSON object literal
    /// shape `{"session_type": "longBreak"}` per data-model.md §`ManualSession`.
    #[test]
    fn embedded_in_manual_session_record_round_trips_long_break() {
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
