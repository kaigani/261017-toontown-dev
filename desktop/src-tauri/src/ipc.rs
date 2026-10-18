//! Tauri commands exposed to the web front end. The front end reaches them only through `desktop/src/ipc/`.
//!
//! Each command is a thin adapter over the engine facade; errors cross the boundary as the engine's
//! `{code, message, details}` envelope (11 §10).

use tauri::State;
use tt_engine::{Engine, EngineInfo, ErrorInfo, PingRequest, PingResponse};

/// Round-trip probe for the IPC latency readout. Async, so it runs on the async runtime rather than the main
/// (UI) thread, like engine commands will from M1.
///
/// # Errors
/// The engine's error envelope (`limit.exceeded` for oversized payloads).
#[tauri::command]
pub async fn ping(engine: State<'_, Engine>, request: PingRequest) -> Result<PingResponse, ErrorInfo> {
    engine.ping(request).map_err(|e| e.to_info())
}

/// Static engine information (superset of `app.info.get`).
#[tauri::command]
#[must_use]
pub fn app_info(engine: State<'_, Engine>) -> EngineInfo {
    engine.info()
}
