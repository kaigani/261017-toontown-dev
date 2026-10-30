//! Tauri commands exposed to the web front end. The front end reaches them only through `desktop/src/ipc/`.
//!
//! Each command is a thin adapter over the engine facade. Values cross as **JSON text** (ADR-0012): the request
//! arrives as one `json` string argument, the result is returned as a UTF-8 JSON body (`ArrayBuffer` in the
//! webview), and errors reject with the JSON text of the engine's `{code, message, details}` envelope (11 §10).
//! The front end parses the text losslessly, so 64-bit integers survive in both directions.

use tauri::State;
use tauri::ipc::Response;
use tt_engine::wire::{error_json, from_wire_json, to_wire_json};
use tt_engine::{Engine, PingRequest};

fn respond<T: serde::Serialize>(value: &T) -> Result<Response, String> {
    to_wire_json(value).map(|text| Response::new(text.into_bytes())).map_err(|e| error_json(&e))
}

/// Round-trip probe for the IPC latency readout. Async, so it runs on the async runtime rather than the main
/// (UI) thread, like engine commands will from M1.
///
/// # Errors
/// The JSON text of the engine's error envelope (`arg.invalid` for a malformed request, `limit.exceeded` for
/// oversized payloads).
#[tauri::command]
pub async fn ping(engine: State<'_, Engine>, json: String) -> Result<Response, String> {
    let request: PingRequest = from_wire_json(&json).map_err(|e| error_json(&e))?;
    let response = engine.ping(request).map_err(|e| error_json(&e.to_info()))?;
    respond(&response)
}

/// Internal engine bootstrap information (`EngineInfo`); not the spec query `app.info.get` (01 §12, C1.4).
///
/// # Errors
/// The JSON text of an `internal` error if serialisation fails.
#[tauri::command]
pub fn app_info(engine: State<'_, Engine>) -> Result<Response, String> {
    respond(&engine.info())
}
