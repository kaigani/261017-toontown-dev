//! Tauri commands exposed to the web front end. The front end reaches them only through `desktop/src/ipc/`.
//!
//! Values cross as **JSON text** (ADR-0012): the request arrives as one `json` string argument, the result is returned
//! as a UTF-8 JSON body (`ArrayBuffer` in the webview), and errors reject with the JSON text of the engine's
//! `{code, message, details}` envelope (11 §10). The front end parses the text losslessly.
//!
//! M4 C4.1 (ADR-0021): `run_command` is the one entry point of every GUI action — it invokes a registered command on
//! the executor as the user, exactly as an agent's call would, and answers its envelope. `cancel_request` cancels a
//! cancellable request; `report_shell` carries sensor facts; `dispatch_key` resolves a key through the kernel's keymap;
//! `pick_path` shows a native file picker.

use serde::Deserialize;
use serde_json::{Map, Value, json};
use tauri::ipc::Response;
use tauri::{AppHandle, Manager, State};
use tt_engine::wire::{error_json, from_wire_json, to_wire_json};
use tt_engine::{Engine, PingRequest};

use crate::host::Host;

fn respond<T: serde::Serialize>(value: &T) -> Result<Response, String> {
    to_wire_json(value).map(|text| Response::new(text.into_bytes())).map_err(|e| error_json(&e))
}

fn parse<T: serde::de::DeserializeOwned>(json: &str) -> Result<T, String> {
    from_wire_json(json).map_err(|e| error_json(&e))
}

fn internal(message: impl Into<String>) -> String {
    error_json(&tt_engine::ErrorInfo { code: "internal".into(), message: message.into(), details: None })
}

/// Round-trip probe for the IPC latency readout.
///
/// # Errors
/// The JSON text of the engine's error envelope.
#[tauri::command]
pub async fn ping(engine: State<'_, Engine>, json: String) -> Result<Response, String> {
    let request: PingRequest = parse(&json)?;
    let response = engine.ping(request).map_err(|e| error_json(&e.to_info()))?;
    respond(&response)
}

/// Internal engine bootstrap information (`EngineInfo`); not the spec query `app.info.get`.
///
/// # Errors
/// The JSON text of an `internal` error if serialisation fails.
#[tauri::command]
pub fn app_info(engine: State<'_, Engine>) -> Result<Response, String> {
    respond(&engine.info())
}

/// `run_command` request.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RunRequest {
    command: String,
    #[serde(default)]
    params: Map<String, Value>,
    #[serde(default)]
    options: Option<RunOptions>,
    /// Set when the GUI offers Cancel for this request.
    #[serde(default)]
    request_token: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RunOptions {
    #[serde(default)]
    dry_run: bool,
}

/// Runs one registered command as the user and answers its envelope (11 §7.2). The call runs off the main thread, so
/// the window stays responsive while the executor works (never-block, 11 P5). A successful `app.quit` ends the
/// application after the answer is sent.
///
/// # Errors
/// The JSON text of an `arg.invalid` error for a malformed request; command failures are envelopes, not errors.
#[tauri::command]
pub async fn run_command(app: AppHandle, json: String) -> Result<Response, String> {
    let req: RunRequest = parse(&json)?;
    let app2 = app.clone();
    let env = tauri::async_runtime::spawn_blocking(move || {
        let host = app2.state::<Host>();
        host.run(
            &req.command,
            Value::Object(req.params),
            req.options.unwrap_or_default().dry_run,
            req.request_token.as_deref(),
        )
    })
    .await
    .map_err(|e| internal(e.to_string()))?;
    let quit = env.is_ok() && json.contains("\"app.quit\"") && is_quit(&json);
    let response = respond(&env)?;
    if quit {
        crate::shell::quit_soon(&app);
    }
    Ok(response)
}

fn is_quit(json: &str) -> bool {
    from_wire_json::<RunRequest>(json).is_ok_and(|r| r.command == "app.quit")
}

/// `cancel_request` request.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CancelRequest {
    token: String,
}

/// Cancels a cancellable request (ADR-0020 decision 8): its probe sees the flag at the executor's next boundary. A
/// request cancellation never cancels a job.
///
/// # Errors
/// The JSON text of an `arg.invalid` error for a malformed request.
#[tauri::command]
pub fn cancel_request(host: State<'_, Host>, json: String) -> Result<Response, String> {
    let req: CancelRequest = parse(&json)?;
    host.cancel(&req.token);
    respond(&json!({}))
}

/// Applies a sensor report (ADR-0021). Never blocks.
///
/// # Errors
/// The JSON text of an `arg.invalid` error for a malformed report.
#[tauri::command]
pub fn report_shell(host: State<'_, Host>, json: String) -> Result<Response, String> {
    let report: tt_executor::host::ShellReport = parse(&json)?;
    host.exec().report_shell(report);
    respond(&json!({}))
}

/// `dispatch_key` request.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyRequest {
    keys: String,
}

/// Resolves a key sequence through the kernel's keymap (01 §8.4): the one dispatch real keys and the MCP `key` tool
/// share. The front end then runs the command through `run_command` (with a dialog when parameters are missing).
///
/// # Errors
/// The JSON text of `keymap.invalidSequence` / `keymap.reservedKey`, or `command.timeout`.
#[tauri::command]
pub async fn dispatch_key(app: AppHandle, json: String) -> Result<Response, String> {
    let req: KeyRequest = parse(&json)?;
    let r = tauri::async_runtime::spawn_blocking(move || app.state::<Host>().exec().resolve_key(&req.keys))
        .await
        .map_err(|e| internal(e.to_string()))?;
    match r {
        Ok(resolution) => respond(&resolution),
        Err(e) => Err(error_json(&tt_engine::ErrorInfo {
            code: e.code,
            message: e.message,
            details: Some(*e.details),
        })),
    }
}

/// `pick_path` request.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PickRequest {
    kind: String,
    title: String,
    #[serde(default)]
    filters: Vec<PickFilter>,
    #[serde(default)]
    default_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PickFilter {
    name: String,
    extensions: Vec<String>,
}

/// Shows a native Open, Save As or folder picker (01 §3.2, §3.6) and answers `{path}` (`null` when cancelled). The
/// picker runs off the main thread.
///
/// # Errors
/// The JSON text of an `arg.invalid` error for a malformed request.
#[tauri::command]
pub async fn pick_path(app: AppHandle, json: String) -> Result<Response, String> {
    use tauri_plugin_dialog::DialogExt;
    let req: PickRequest = parse(&json)?;
    let path = tauri::async_runtime::spawn_blocking(move || {
        let mut d = app.dialog().file().set_title(&req.title);
        for f in &req.filters {
            let exts: Vec<&str> = f.extensions.iter().map(String::as_str).collect();
            d = d.add_filter(&f.name, &exts);
        }
        if let Some(p) = &req.default_path {
            d = d.set_directory(p);
        }
        let picked = match req.kind.as_str() {
            "save" => d.blocking_save_file(),
            "folder" => d.blocking_pick_folder(),
            _ => d.blocking_pick_file(),
        };
        picked.and_then(|p| p.into_path().ok()).map(|p| p.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| internal(e.to_string()))?;
    respond(&json!({ "path": path }))
}

/// What the front end does at start (01 §1.1 rules 7-8): the documents the OS or the command line asked for before the
/// window existed (taken once), and the start-up behaviour preference.
///
/// # Errors
/// The JSON text of an `internal` error if serialisation fails.
#[tauri::command]
pub fn startup(host: State<'_, Host>) -> Result<Response, String> {
    let open: Vec<String> = host.take_pending_open().iter().map(|p| p.to_string_lossy().into_owned()).collect();
    respond(&json!({ "open": open }))
}

/// Restarts a stuck engine at the user's request (reviews/M4-design-engine-restart.md): a new executor over the same
/// user folders, a new event pump, the HTTP server restarted; the front end resyncs and offers the autosave as
/// recoverable work. Never called automatically.
///
/// # Errors
/// The JSON text of the new executor's start error.
#[tauri::command]
pub async fn restart_engine(app: AppHandle) -> Result<Response, String> {
    let app2 = app.clone();
    let generation = tauri::async_runtime::spawn_blocking(move || app2.state::<Host>().restart())
        .await
        .map_err(|e| internal(e.to_string()))?
        .map_err(|e| error_json(&tt_engine::ErrorInfo { code: e.code, message: e.message, details: Some(*e.details) }))?;
    crate::shell::start_event_pump(&app);
    crate::menu::refresh(&app);
    respond(&json!({ "generation": generation }))
}
