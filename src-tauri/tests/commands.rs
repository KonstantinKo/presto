// Integration tests for #[tauri::command] functions via Tauri 2's MockRuntime.
//
// MockRuntime spins up a lightweight Tauri app without a real webview or OS
// window. Commands are dispatched through Tauri's actual IPC pipeline via
// `get_ipc_response`, so these tests validate the Rust IPC contract — the
// stable boundary between any frontend and the persistence layer — without the
// cost of a full Playwright/WebDriver rig.
//
// Why `write_excel_file` as the first target: unlike most commands it accepts
// the destination path as an explicit argument, so no app_data_dir override is
// needed; the command can be pointed at a tempdir and exercised end-to-end.
// This satisfies issue #9's "at least one IPC integration test" acceptance gate.
//
// Implementation note: `#[tauri::command]` on a `pub` function in the library
// generates `#[macro_export]` for the wrapper macro, which conflicts with the
// same function appearing in the library's own `generate_handler!`. The
// workaround is to keep the library command private and expose the business
// logic as `presto_lib::decode_and_write_file`. This test wires that function
// into a fresh `#[tauri::command]` under the same IPC name so `get_ipc_response`
// dispatches it via the real Tauri invoke pipeline.
//
// TODO(stack-swap): if the command name or arg shape changes when migrating to
// Leptos/WASM, update `cmd`, `path`, and `data` in the InvokeRequest below.

use base64::{engine::general_purpose, Engine as _};
use tauri::http::HeaderMap;
use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY};
use tauri::WebviewUrl;

// Local command that delegates to the library's pub business-logic function.
// The IPC name "write_excel_file" matches the production command so the
// integration test exercises the same contract the frontend invokes.
// String args are required by Tauri's IPC deserialization; clippy's
// needless_pass_by_value suggestion of &str would break the command contract.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn write_excel_file(path: String, data: String) -> Result<(), presto_lib::BridgeError> {
    presto_lib::decode_and_write_file(&path, &data)
}

fn make_app() -> tauri::App<tauri::test::MockRuntime> {
    mock_builder()
        .invoke_handler(tauri::generate_handler![write_excel_file])
        .build(mock_context(noop_assets()))
        .expect("failed to build mock app")
}

fn invoke_write_excel_file(
    webview: &tauri::WebviewWindow<tauri::test::MockRuntime>,
    path: &str,
    data: &str,
) -> Result<tauri::ipc::InvokeResponseBody, serde_json::Value> {
    get_ipc_response(
        webview,
        tauri::webview::InvokeRequest {
            cmd: "write_excel_file".into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "http://tauri.localhost".parse().expect("valid url"),
            body: tauri::ipc::InvokeBody::Json(serde_json::json!({
                "path": path,
                "data": data
            })),
            headers: HeaderMap::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
}

#[test]
fn write_excel_file_writes_decoded_bytes_to_provided_path() {
    let app = make_app();
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
        .build()
        .expect("webview");

    let expected = b"hello-presto";
    let encoded = general_purpose::STANDARD.encode(expected);

    let dir = tempfile::tempdir().expect("tempdir");
    let out_path = dir.path().join("out.xlsx");

    let result = invoke_write_excel_file(&webview, out_path.to_str().expect("path str"), &encoded);
    assert!(result.is_ok(), "command returned error: {result:?}");

    let written = std::fs::read(&out_path).expect("read output file");
    assert_eq!(written, expected);
}

#[test]
fn write_excel_file_invalid_base64_returns_error() {
    let app = make_app();
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
        .build()
        .expect("webview");

    let dir = tempfile::tempdir().expect("tempdir");
    let out_path = dir.path().join("out.xlsx");

    let result = invoke_write_excel_file(
        &webview,
        out_path.to_str().expect("path str"),
        "not-valid-base64!!!",
    );
    assert!(result.is_err(), "expected error for invalid base64");
    assert!(
        !out_path.exists(),
        "file should not be created on decode error"
    );
}
