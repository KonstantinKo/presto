// `BridgeError` — typed error variant for every Tauri command return value.
//
// Spec 001-leptos-migration §Phase 1A T023-T024; data-model.md §`BridgeError`.
//
// Wire shape: externally-tagged JSON via `#[serde(tag = "kind",
// rename_all = "snake_case")]`. Six variants:
//
// - `BridgeUnavailable`         — `window.__TAURI_INTERNALS__` absent
//                                 (Leptos-side only; never produced by Tauri)
// - `NotAuthenticated`          — caller missing required session
// - `InvalidArgument {f, r}`    — argument validation failure
// - `NotFound {resource}`       — requested file/key/row missing
// - `SerdeRoundtrip {c, e}`     — `serde-wasm-bindgen` deserialise failure
// - `Internal {msg}`            — catch-all for unexpected Tauri-side failures
//
// RED-phase content: this file currently contains only the failing serde
// round-trip tests (T023). The `BridgeError` type itself lands in T024 GREEN.
// Per AGENTS.md §"Test-first commit ordering": a single combined commit is
// rejected; the diff has to show RED first, then GREEN.

#[cfg(test)]
mod tests {
    use super::BridgeError;

    #[test]
    fn bridge_unavailable_serialises_kind_only() {
        let json = serde_json::to_string(&BridgeError::BridgeUnavailable).unwrap();
        assert_eq!(json, r#"{"kind":"bridge_unavailable"}"#);
    }

    #[test]
    fn not_authenticated_serialises_kind_only() {
        let json = serde_json::to_string(&BridgeError::NotAuthenticated).unwrap();
        assert_eq!(json, r#"{"kind":"not_authenticated"}"#);
    }

    #[test]
    fn invalid_argument_carries_field_and_reason() {
        let err = BridgeError::InvalidArgument {
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
    fn not_found_carries_resource() {
        let err = BridgeError::NotFound {
            resource: "settings.json".to_string(),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(
            json,
            r#"{"kind":"not_found","resource":"settings.json"}"#
        );
    }

    #[test]
    fn serde_roundtrip_carries_command_and_error() {
        let err = BridgeError::SerdeRoundtrip {
            command: "load_settings",
            error: "missing field `timer`".to_string(),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(
            json,
            r#"{"kind":"serde_roundtrip","command":"load_settings","error":"missing field `timer`"}"#
        );
    }

    #[test]
    fn internal_carries_msg() {
        let err = BridgeError::Internal {
            msg: "boom".to_string(),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, r#"{"kind":"internal","msg":"boom"}"#);
    }

    #[test]
    fn invalid_argument_round_trips_through_serde() {
        let original = BridgeError::InvalidArgument {
            field: "password".to_string(),
            reason: "too short".to_string(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let decoded: BridgeError = serde_json::from_str(&json).unwrap();
        match decoded {
            BridgeError::InvalidArgument { field, reason } => {
                assert_eq!(field, "password");
                assert_eq!(reason, "too short");
            }
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    #[test]
    fn unit_variants_round_trip_through_serde() {
        for variant in [
            BridgeError::BridgeUnavailable,
            BridgeError::NotAuthenticated,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let decoded: BridgeError = serde_json::from_str(&json).unwrap();
            // Matching on Debug repr keeps this assertion compatible with any
            // future PartialEq derivation without forcing it now.
            assert_eq!(format!("{variant:?}"), format!("{decoded:?}"));
        }
    }
}
