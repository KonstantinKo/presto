// Typed wrappers for every surviving Tauri command.
//
// Spec 001-leptos-migration §Phase 1C T032-T083; contracts/tauri-bridge.md
// §"Surviving commands". One wrapper per command; the wrapper enforces
// FR-008's compile-time-mismatch promise (a Leptos call site whose
// argument or return type drifts from the Rust handler IS a compile
// error) and the FR-009 short-circuit: every wrapper checks
// `bridge_available()` and returns `BridgeError::BridgeUnavailable` when
// the Tauri JS bridge is not present.
//
// Commands are grouped by domain (sessions, tasks, manual sessions, tags,
// settings, …) in the order of contracts/tauri-bridge.md. Tests sit in
// the `tests` submodule below; each command has at least one
// `wasm-bindgen-test` covering the bridge-absent short-circuit, and a
// signature-pinning compile-time assertion.
//
// Lint allowance: `clippy::future_not_send` is allowed at the module level
// because the bridge runs exclusively on `wasm32-unknown-unknown`, where
// the runtime is single-threaded and `JsValue` (plus everything
// transitively built on it: `JsFuture`, `Promise`, `serde-wasm-bindgen`
// values) is `!Send` by construction. Demanding `Send` here would force
// every wrapper to invent a Send-erasure shim that does nothing on the
// WASM target. Spec 001 plan.md §Modules makes the same call; no
// non-WASM consumer of this module exists.
#![allow(clippy::future_not_send)]

use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use super::availability::bridge_available;
use super::error::BridgeError;
use super::types::Session;

#[wasm_bindgen]
extern "C" {
    /// Tauri 2.x JS bridge entry point. Bound to
    /// `window.__TAURI_INTERNALS__.invoke(cmd, args)`. Callers MUST
    /// short-circuit on `bridge_available().is_absent()` before invoking
    /// — the binding panics in environments where the global is missing
    /// (the `__TAURI_INTERNALS__` object is created by the Tauri webview
    /// bootstrap; it does not exist in node, in the Trunk dev server, or
    /// in the e2e mock harness).
    #[wasm_bindgen(
        js_namespace = ["__TAURI_INTERNALS__"],
        js_name = invoke,
        catch
    )]
    fn tauri_invoke(cmd: &str, args: JsValue) -> Result<js_sys::Promise, JsValue>;
}

/// Generic invoke helper. Performs the FR-009 bridge-availability
/// short-circuit, then serialises the typed argument bag, calls
/// `window.__TAURI_INTERNALS__.invoke`, awaits the resulting `Promise`,
/// and deserialises the typed return.
///
/// The helper is intentionally `async fn` rather than a hand-written
/// `impl Future` so call sites compose with the rest of the Leptos async
/// surface (every wrapper is `async fn ... -> Result<R, BridgeError>`).
async fn invoke_serde<A, R>(cmd: &'static str, args: &A) -> Result<R, BridgeError>
where
    A: Serialize + ?Sized,
    R: DeserializeOwned,
{
    if bridge_available().is_absent() {
        return Err(BridgeError::BridgeUnavailable);
    }
    let js_args = serde_wasm_bindgen::to_value(args).map_err(|e| BridgeError::SerdeRoundtrip {
        command: cmd.to_string(),
        error: format!("serialise args: {e}"),
    })?;
    let promise = tauri_invoke(cmd, js_args).map_err(|e| BridgeError::Internal {
        msg: format!("invoke('{cmd}') failed at the bridge boundary: {e:?}"),
    })?;
    let resolved = JsFuture::from(promise)
        .await
        .map_err(|e| map_promise_rejection(cmd, &e))?;
    serde_wasm_bindgen::from_value(resolved).map_err(|e| BridgeError::SerdeRoundtrip {
        command: cmd.to_string(),
        error: format!("deserialise return: {e}"),
    })
}

/// Translate a rejected Tauri-side `Promise` into a `BridgeError`. The
/// Tauri runtime wraps Rust-side `Err(BridgeError)` returns as the
/// rejected value; if it deserialises cleanly we keep the structured
/// variant, otherwise we fall back to `Internal` with the raw string.
fn map_promise_rejection(cmd: &'static str, raw: &JsValue) -> BridgeError {
    if let Ok(typed) = serde_wasm_bindgen::from_value::<BridgeError>(raw.clone()) {
        return typed;
    }
    if let Some(s) = raw.as_string() {
        return BridgeError::Internal {
            msg: format!("invoke('{cmd}') rejected: {s}"),
        };
    }
    BridgeError::Internal {
        msg: format!("invoke('{cmd}') rejected with non-string value"),
    }
}

// ---------------------------------------------------------------------------
// Persistence — sessions
// ---------------------------------------------------------------------------

/// Persist the live pomodoro session to disk. Tauri-side handler:
/// `save_session_data(session: PomodoroSession) -> Result<(), BridgeError>`
/// at `src-tauri/src/lib.rs:462`.
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present (Trunk dev server, e2e mock, node tests). Otherwise returns
/// whatever variant the Tauri-side handler maps its filesystem failure to
/// (typically `BridgeError::Internal`).
pub async fn save_session_data(session: Session) -> Result<(), BridgeError> {
    #[derive(Serialize)]
    struct Args {
        session: Session,
    }
    invoke_serde("save_session_data", &Args { session }).await
}

/// Read the persisted live session from disk. Tauri-side handler:
/// `load_session_data() -> Result<Option<PomodoroSession>, BridgeError>`
/// at `src-tauri/src/lib.rs:483`.
///
/// `Option<Session>` is the load-bearing shape — the handler returns
/// `None` for the cold-start "no session yet" case rather than surfacing
/// it as `BridgeError::NotFound`.
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever variant the Tauri-side handler
/// maps its filesystem failure to (typically `BridgeError::Internal`).
pub async fn load_session_data() -> Result<Option<Session>, BridgeError> {
    invoke_serde("load_session_data", &serde_json::Value::Null).await
}

/// Read the persisted full session history. Tauri-side handler:
/// `get_stats_history() -> Result<Vec<PomodoroSession>, BridgeError>`
/// at `src-tauri/src/lib.rs:517`.
///
/// Returns an empty `Vec` if no history file exists yet (the Tauri-side
/// helper at `helpers::read_history_from` treats `NotFound` as empty —
/// a cold-start convention, not an error).
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri JS bridge is
/// not present. Otherwise returns whatever variant the Tauri-side handler
/// maps its filesystem failure to (typically `BridgeError::Internal`).
pub async fn get_stats_history() -> Result<Vec<Session>, BridgeError> {
    invoke_serde("get_stats_history", &serde_json::Value::Null).await
}

// Tests gated on `wasm32` because every wrapper-test is a
// `#[wasm_bindgen_test]` — running them via `cargo test` on the host
// target would produce dead-code lint failures (the host-side
// `cfg(target_arch = "wasm32")` removal silently drops the test bodies).
// `wasm-pack test --node` is the canonical test driver per
// `quickstart.md` line 105 and tasks.md T030/T032 done-signals.
#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::{get_stats_history, load_session_data, save_session_data};
    use crate::bridge::error::BridgeError;
    use crate::bridge::types::Session;
    use wasm_bindgen_test::wasm_bindgen_test;

    fn sample_session() -> Session {
        Session {
            completed_pomodoros: 3,
            total_focus_time: 4_500,
            current_session: 4,
            date: "Sat May 10 2026".to_string(),
        }
    }

    /// Under `wasm-pack test --node`, no `__TAURI_INTERNALS__` is installed,
    /// so the wrapper MUST short-circuit with `BridgeError::BridgeUnavailable`
    /// rather than calling into a missing global. Pins FR-009.
    #[wasm_bindgen_test]
    async fn save_session_data_round_trip_short_circuits_when_bridge_absent() {
        let result = save_session_data(sample_session()).await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin: the wrapper must accept `Session` by value
    /// and return `Result<(), BridgeError>` per contracts/tauri-bridge.md row 1.
    /// Bind to a typed `async fn` pointer; if the signature drifts, this
    /// stops compiling — that's exactly the FR-008 promise.
    #[wasm_bindgen_test]
    async fn save_session_data_round_trip_signature_pinned() {
        async fn assert_signature(s: Session) -> Result<(), BridgeError> {
            save_session_data(s).await
        }
        // Drive the future once so the binding isn't elided. Under node the
        // bridge is absent, so the call resolves immediately to
        // BridgeUnavailable — but the load-bearing assertion is the typed
        // `async fn` shape captured by `assert_signature`'s declaration.
        let _ = assert_signature(sample_session()).await;
    }

    #[wasm_bindgen_test]
    async fn load_session_data_round_trip_short_circuits_when_bridge_absent() {
        let result = load_session_data().await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 2:
    /// `load_session_data() -> Result<Option<Session>, BridgeError>`.
    /// `Option<Session>` is the load-bearing shape — the Tauri handler
    /// returns `None` for the cold-start "no session yet" case rather than
    /// surfacing it as a `NotFound` error.
    #[wasm_bindgen_test]
    async fn load_session_data_round_trip_signature_pinned() {
        async fn assert_signature() -> Result<Option<Session>, BridgeError> {
            load_session_data().await
        }
        let _ = assert_signature().await;
    }

    #[wasm_bindgen_test]
    async fn get_stats_history_round_trip_short_circuits_when_bridge_absent() {
        let result = get_stats_history().await;
        match result {
            Err(BridgeError::BridgeUnavailable) => {}
            other => panic!("expected BridgeUnavailable, got {other:?}"),
        }
    }

    /// Compile-time signature pin per contracts/tauri-bridge.md row 3:
    /// `get_stats_history() -> Result<Vec<Session>, BridgeError>`.
    #[wasm_bindgen_test]
    async fn get_stats_history_round_trip_signature_pinned() {
        async fn assert_signature() -> Result<Vec<Session>, BridgeError> {
            get_stats_history().await
        }
        let _ = assert_signature().await;
    }
}
