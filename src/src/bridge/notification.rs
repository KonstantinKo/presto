// Bridge wrapper around tauri-plugin-notification's JS API
// (window.__TAURI__.notification.sendNotification).
//
// Tauri's notification plugin is exposed via the higher-level
// __TAURI__.notification.* surface rather than __TAURI_INTERNALS__.invoke.
// The existing tauriMock.js fixture mocks the JS-level surface so this
// wrapper works under both real Tauri builds and the e2e mock harness.
#![allow(clippy::future_not_send)]

use serde::Serialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use super::availability::bridge_available;
use super::types::BridgeError;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(
        js_namespace = ["__TAURI__", "notification"],
        js_name = sendNotification,
        catch
    )]
    fn tauri_send_notification(opts: JsValue) -> Result<JsValue, JsValue>;
}

#[derive(Serialize)]
struct NotificationOpts<'a> {
    title: &'a str,
    body: &'a str,
}

/// Send a system notification via the Tauri notification plugin.
///
/// Best-effort: returns `BridgeError::BridgeUnavailable` if the Tauri
/// bridge is absent; returns `BridgeError::Internal` if the plugin call
/// rejects. Callers should generally ignore errors — a missing chime is
/// not fatal.
///
/// # Errors
/// Returns `BridgeError::BridgeUnavailable` when the Tauri bridge is
/// absent. Returns `BridgeError::Internal` on plugin rejection.
pub async fn send_notification(title: &str, body: &str) -> Result<(), BridgeError> {
    if bridge_available().is_absent() {
        return Err(BridgeError::BridgeUnavailable);
    }
    let opts = serde_wasm_bindgen::to_value(&NotificationOpts { title, body }).map_err(|e| {
        BridgeError::SerdeRoundtrip {
            command: "sendNotification".into(),
            error: format!("serialise opts: {e}"),
        }
    })?;
    // sendNotification returns synchronously in v2 but we treat it as
    // possibly-async for forward compatibility. If it returns a Promise,
    // await it; otherwise drop the immediate value.
    match tauri_send_notification(opts) {
        Ok(result) => {
            if let Ok(promise) = result.dyn_into::<js_sys::Promise>() {
                JsFuture::from(promise)
                    .await
                    .map_err(|e| BridgeError::Internal {
                        msg: format!("sendNotification rejected: {e:?}"),
                    })?;
            }
            Ok(())
        }
        Err(e) => Err(BridgeError::Internal {
            msg: format!("sendNotification failed at bridge boundary: {e:?}"),
        }),
    }
}
