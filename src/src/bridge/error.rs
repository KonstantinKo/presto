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
// The Tauri-side mirror lives in `src-tauri/src/lib.rs`; both definitions
// are byte-identical on the wire so a serde-incompatible change breaks
// both crates' tests at once (Principle VI — typed boundary).

use serde::{Deserialize, Serialize};

/// Typed error variant returned by every bridge command wrapper.
///
/// Wire form is externally-tagged JSON (the `kind` discriminator carries the
/// variant name; data fields sit alongside it). The Tauri-side enum at
/// `src-tauri/src/lib.rs` mirrors this exactly so a `Result<T, BridgeError>`
/// round-trips through `invoke()` without a translation layer.
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BridgeError {
    /// `window.__TAURI_INTERNALS__` is absent (vite/trunk dev server, e2e
    /// mock context). Never produced by the Tauri handler — the Leptos
    /// wrapper short-circuits with this variant before calling `invoke()`.
    /// Per AGENTS.md §Bridge availability and FR-009.
    #[error("bridge unavailable")]
    BridgeUnavailable,
    /// The caller is in a state where the command is invalid (e.g., a
    /// Supabase command without an active session).
    #[error("not authenticated")]
    NotAuthenticated,
    /// An argument failed validation at the boundary.
    #[error("invalid argument {field}: {reason}")]
    InvalidArgument { field: String, reason: String },
    /// The requested file, key, or row does not exist.
    #[error("not found: {resource}")]
    NotFound { resource: String },
    /// `serde-wasm-bindgen` (or `serde_json`) failed to deserialise the
    /// command return on the Leptos side.
    ///
    /// Note: data-model.md §`BridgeError` originally specified
    /// `command: &'static str` to dodge a per-call allocation, but that
    /// makes the enum's `Deserialize` impl require `'de: 'static`, which
    /// breaks deserialising any non-static input (e.g., from `from_str`
    /// of a heap-allocated JSON buffer). The variant is owning in both
    /// crates so a `BridgeError` round-trips through serde for every
    /// possible discriminant. Allocations are minor (one short literal
    /// per failure path; this is the error path, not the hot path).
    #[error("serde roundtrip failed in {command}: {error}")]
    SerdeRoundtrip { command: String, error: String },
    /// Catch-all for unexpected Tauri-side failures (filesystem errors,
    /// plugin errors, etc.). The Tauri-side mapping defaults to this
    /// variant for any `.map_err(|e| e.to_string())` call site that
    /// lacks semantic context (per data-model.md §`BridgeError` mapping
    /// strategy).
    #[error("internal: {msg}")]
    Internal { msg: String },
}

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
