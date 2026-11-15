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
    error_json(&tt_engine::ErrorInfo { code: "internal".into(), message: message.into(), details: json!({}) })
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
pub async fn run_command(app: AppHandle, window: tauri::Window, json: String) -> Result<Response, String> {
    let req: RunRequest = parse(&json)?;
    let app2 = app.clone();
    let label = window.label().to_owned();
    let env = tauri::async_runtime::spawn_blocking(move || {
        let host = app2.state::<Host>();
        host.run_from(
            &label,
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
pub fn cancel_request(host: State<'_, Host>, window: tauri::Window, json: String) -> Result<Response, String> {
    let req: CancelRequest = parse(&json)?;
    host.cancel_from(window.label(), &req.token);
    respond(&json!({}))
}

/// Applies a sensor report (ADR-0021). Never blocks.
///
/// # Errors
/// The JSON text of an `arg.invalid` error for a malformed report.
#[tauri::command]
pub fn report_shell(host: State<'_, Host>, window: tauri::Window, json: String) -> Result<Response, String> {
    let value: Value = parse(&json)?;
    // A report names the engine generation the window last saw; one of another generation is ignored (ruling O-14).
    // Each window's facts are its own; the executor gets their combination (ruling O-2).
    respond(&host.report_from(window.label(), value)?)
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
        Err(e) => Err(error_json(&tt_engine::ErrorInfo { code: e.code, message: e.message, details: *e.details })),
    }
}

/// `availability` request: the command references a GUI surface shows.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AvailabilityRequest {
    refs: Vec<CommandRef>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CommandRef {
    command_id: String,
    #[serde(default)]
    params: Map<String, Value>,
}

/// The most references one availability request may name.
const MAX_REFS: usize = 1024;

/// The availability of command references (ruling O-10): enabled, check mark, dynamic label and the parameters as run,
/// from the kernel, the one source the native menu, toolbars, the tool rail and the key dispatch share.
///
/// # Errors
/// The JSON text of `arg.invalid` for a malformed or oversized request, or `command.timeout`.
#[tauri::command]
pub async fn availability(app: AppHandle, json: String) -> Result<Response, String> {
    let req: AvailabilityRequest = parse(&json)?;
    if req.refs.len() > MAX_REFS {
        return Err(error_json(&tt_engine::ErrorInfo {
            code: "arg.invalid".into(),
            message: format!("at most {MAX_REFS} references per request"),
            details: json!({ "pointer": "/refs", "max": MAX_REFS }),
        }));
    }
    let refs = req.refs.into_iter().map(|r| (r.command_id, r.params)).collect();
    let r = tauri::async_runtime::spawn_blocking(move || app.state::<Host>().exec().availability(refs))
        .await
        .map_err(|e| internal(e.to_string()))?;
    match r {
        Ok(list) => respond(&list),
        Err(e) => Err(error_json(&tt_engine::ErrorInfo { code: e.code, message: e.message, details: *e.details })),
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
    // The canvas's launch options (M4 C4.2b measurements): metrics published, a forced back end.
    let canvas = json!({ "metrics": host.options.canvas_metrics, "backend": host.options.canvas_backend });
    respond(&json!({ "open": open, "canvas": canvas }))
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
    let (generation, restarted) = tauri::async_runtime::spawn_blocking(move || app2.state::<Host>().restart())
        .await
        .map_err(|e| internal(e.to_string()))?
        .map_err(|e| error_json(&tt_engine::ErrorInfo { code: e.code, message: e.message, details: *e.details }))?;
    // Only the call that restarted starts the new generation's event pump (a waiting second call does not).
    if restarted {
        crate::shell::start_event_pump(&app);
    }
    crate::menu::refresh(&app);
    respond(&json!({ "generation": generation }))
}

/// `canvas_scene` request (design note 1 §9).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CanvasSceneRequest {
    view_id: String,
}

/// The composition of a Drawing view for the interactive canvas (M4 C4.2b; design note 1 §9): `{generation, scene}`,
/// the drawings shown with their revisions, the background, the grid and the solid colour values. A read; it runs off
/// the main thread.
///
/// # Errors
/// The JSON text of `arg.invalid` for a malformed request, or of the kernel's refusal (`view.notFound`, `no.project`).
#[tauri::command]
pub async fn canvas_scene(app: AppHandle, json: String) -> Result<Response, String> {
    let req: CanvasSceneRequest = parse(&json)?;
    let answer = tauri::async_runtime::spawn_blocking(move || {
        let host = app.state::<Host>();
        let generation = host.generation();
        host.exec().canvas_scene(&req.view_id).map(|scene| json!({ "generation": generation, "scene": scene }))
    })
    .await
    .map_err(|e| internal(e.to_string()))?;
    respond(&answer.map_err(|e| error_json(&engine_error(&e)))?)
}

/// `canvas_load` request (design note 1 §9).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CanvasLoadRequest {
    drawing_id: String,
    bucket: i16,
    /// The outline-only form (02 §10.2); absent: false.
    #[serde(default)]
    outline: bool,
}

/// One drawing's TTRD payload for the interactive canvas (M4 C4.2b; design note 1 §2, §8, §9), binary, at the
/// drawing's current revision and the engine's generation. The walk, tessellation and encoding run off the main thread
/// and off the document thread.
///
/// # Errors
/// The JSON text of `arg.invalid` for a malformed request, or of the kernel's refusal (`ref.notFound`, ...).
#[tauri::command]
pub async fn canvas_load(app: AppHandle, json: String) -> Result<Response, String> {
    let req: CanvasLoadRequest = parse(&json)?;
    let answer = tauri::async_runtime::spawn_blocking(move || {
        let host = app.state::<Host>();
        let generation = host.generation();
        host.exec().canvas_drawing(Some(&req.drawing_id), generation, req.bucket.clamp(-8, 8), req.outline)
    })
    .await
    .map_err(|e| internal(e.to_string()))?;
    answer.map(|p| Response::new(p.bytes)).map_err(|e| error_json(&engine_error(&e)))
}

/// A gesture the user made in a Drawing view window (M4 C4.2b; design note 2 §6): the `pointer` request's events,
/// run by the same gesture driver as the headless `pointer` tool, with the user's origin, so the window path and the
/// headless path produce the same command (G2, ruling D-4). Answers the `pointer` envelope.
///
/// # Errors
/// The JSON text of `arg.invalid` for a request that is not JSON; every other refusal is the envelope's.
#[tauri::command]
pub async fn gesture_commit(app: AppHandle, json: String) -> Result<Response, String> {
    let mut request: Value = parse(&json)?;
    // An agent's gesture the window played (design note 2 §6) carries its request id: it runs with the agent's origin
    // and answers the waiting agent call too.
    let agent = agent_request_of(&mut request)?;
    let env = tauri::async_runtime::spawn_blocking(move || {
        let host = app.state::<Host>();
        match (agent, host.attached()) {
            (Some(id), Some(io)) => io.finish(id, request),
            _ => host.exec().pointer(tt_executor::Origin::User, request),
        }
    })
    .await
    .map_err(|e| internal(e.to_string()))?;
    respond(&env)
}

fn engine_error(e: &tt_executor::CommandError) -> tt_engine::ErrorInfo {
    tt_engine::ErrorInfo { code: e.code.clone(), message: e.message.clone(), details: (*e.details).clone() }
}

/// `gesture_abort` request.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GestureAbortRequest {
    agent_request: u64,
}

/// A user press during an agent gesture the window was playing (P2; SI-135): the agent's call answers `ui.busy` with
/// `reason: "userInput"`; nothing is committed.
///
/// # Errors
/// The JSON text of `arg.invalid` for a malformed request.
#[tauri::command]
pub fn gesture_abort(host: State<'_, Host>, json: String) -> Result<Response, String> {
    let req: GestureAbortRequest = parse(&json)?;
    if let Some(io) = host.attached() {
        io.abort(req.agent_request);
    }
    respond(&json!({}))
}

/// `canvas_presence` request.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CanvasPresenceRequest {
    view_id: String,
    /// The canvas's rectangle in the window's view pixels; `null` when the canvas went away.
    rect: Option<PresenceRect>,
    #[serde(default)]
    dpr: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PresenceRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// A window's canvas for a view appeared, moved or went away (attached mode's routing and screenshot targets).
///
/// # Errors
/// The JSON text of `arg.invalid` for a malformed request.
#[tauri::command]
pub fn canvas_presence(host: State<'_, Host>, window: tauri::Window, json: String) -> Result<Response, String> {
    let (view, presence) = presence_of(&json)?;
    if let Some(io) = host.attached() {
        io.set_presence(window.label(), &view, presence);
    }
    respond(&json!({}))
}

/// The `agentRequest` member of a `gesture_commit` request, removed from it (D-7e): absent, a user gesture; a request
/// id, an agent gesture the window played; anything else (`null` included) is refused.
fn agent_request_of(request: &mut Value) -> Result<Option<u64>, String> {
    let Some(m) = request.as_object_mut() else { return Ok(None) };
    match m.remove("agentRequest") {
        None => Ok(None),
        Some(v) => {
            v.as_u64().map(Some).ok_or_else(|| arg_invalid("/agentRequest", "a request id (a non-negative integer)"))
        }
    }
}

/// A `canvas_presence` request (D-7e): `viewId` required; `rect` required, an object (the canvas's rectangle) or `null`
/// (the canvas went away); `dpr` absent (1) or a positive number, never `null`.
fn presence_of(json: &str) -> Result<(String, Option<crate::attached::Presence>), String> {
    let v: Value = parse(json)?;
    let o = v.as_object().ok_or_else(|| arg_invalid("", "an object"))?;
    if !o.contains_key("rect") {
        return Err(arg_invalid("/rect", "the canvas's rectangle, or null when it went away"));
    }
    if o.get("dpr").is_some_and(Value::is_null) {
        return Err(arg_invalid("/dpr", "a positive number (absent: 1)"));
    }
    let req: CanvasPresenceRequest = parse(json)?;
    let rect = match req.rect {
        Some(r) if [r.x, r.y, r.w, r.h].iter().all(|v| v.is_finite()) && r.w >= 1.0 && r.h >= 1.0 => Some(r),
        Some(_) => return Err(arg_invalid("/rect", "finite numbers with a width and height of at least 1")),
        None => None,
    };
    let dpr = match req.dpr {
        Some(d) if d.is_finite() && d > 0.0 => d,
        Some(_) => return Err(arg_invalid("/dpr", "a positive number")),
        None => 1.0,
    };
    Ok((req.view_id, rect.map(|r| crate::attached::Presence { rect: [r.x, r.y, r.w, r.h], dpr })))
}

fn arg_invalid(pointer: &str, what: &str) -> String {
    error_json(&tt_engine::ErrorInfo {
        code: "arg.invalid".into(),
        message: format!("{pointer}: {what}"),
        details: json!({ "pointer": pointer }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(r: &Result<impl std::fmt::Debug, String>) -> Option<String> {
        r.as_ref()
            .err()
            .and_then(|e| serde_json::from_str::<Value>(e).ok())
            .and_then(|v| v["code"].as_str().map(str::to_owned))
    }

    /// D-7e: every field of every new message, in every state (absent, null, a value, an empty value), through the real
    /// parsing: accepted with its meaning, or refused with `arg.invalid`.
    #[test]
    fn every_canvas_message_field_distinguishes_absent_null_and_values() {
        let scene = |j: Value| parse::<CanvasSceneRequest>(&j.to_string()).map(|r| r.view_id);
        assert_eq!(scene(json!({ "viewId": "drawingView" })), Ok("drawingView".into()));
        assert_eq!(
            scene(json!({ "viewId": "" })),
            Ok(String::new()),
            "empty: parsed, refused by the kernel (view.notFound)"
        );
        for bad in [json!({}), json!({ "viewId": null }), json!({ "viewId": 1 }), json!({ "viewId": "v", "x": 1 })] {
            assert_eq!(code(&scene(bad.clone())).as_deref(), Some("arg.invalid"), "{bad}");
        }

        let load = |j: Value| parse::<CanvasLoadRequest>(&j.to_string()).map(|r| (r.drawing_id, r.bucket, r.outline));
        assert_eq!(
            load(json!({ "drawingId": "dr_X", "bucket": 2 })),
            Ok(("dr_X".into(), 2, false)),
            "outline absent: the filled form"
        );
        assert_eq!(load(json!({ "drawingId": "dr_X", "bucket": -1, "outline": true })), Ok(("dr_X".into(), -1, true)));
        for bad in [
            json!({ "bucket": 0 }),
            json!({ "drawingId": null, "bucket": 0 }),
            json!({ "drawingId": "dr_X" }),
            json!({ "drawingId": "dr_X", "bucket": null }),
            json!({ "drawingId": "dr_X", "bucket": 0, "outline": null }),
        ] {
            assert_eq!(code(&load(bad.clone())).as_deref(), Some("arg.invalid"), "{bad}");
        }

        let abort = |j: Value| parse::<GestureAbortRequest>(&j.to_string()).map(|r| r.agent_request);
        assert_eq!(abort(json!({ "agentRequest": 7 })), Ok(7));
        for bad in
            [json!({}), json!({ "agentRequest": null }), json!({ "agentRequest": -1 }), json!({ "agentRequest": "7" })]
        {
            assert_eq!(code(&abort(bad.clone())).as_deref(), Some("arg.invalid"), "{bad}");
        }

        let agent = |mut j: Value| agent_request_of(&mut j).map(|a| (a, j));
        assert_eq!(
            agent(json!({ "view": "drawingView", "events": [] })).map(|(a, _)| a),
            Ok(None),
            "absent: a user gesture"
        );
        let (a, rest) = agent(json!({ "view": "drawingView", "events": [], "agentRequest": 3 })).unwrap();
        assert_eq!((a, rest.get("agentRequest").is_none()), (Some(3), true), "an agent gesture; the member is removed");
        for bad in [json!({ "agentRequest": null }), json!({ "agentRequest": "3" }), json!({ "agentRequest": 1.5 })] {
            assert_eq!(code(&agent(bad.clone())).as_deref(), Some("arg.invalid"), "{bad}");
        }

        let presence = |j: Value| presence_of(&j.to_string());
        let rect = json!({ "x": 1, "y": 2, "w": 300, "h": 200 });
        let (v, p) = presence(json!({ "viewId": "drawingView", "rect": rect, "dpr": 2 })).unwrap();
        assert_eq!((v.as_str(), p.map(|p| (p.rect, p.dpr))), ("drawingView", Some(([1.0, 2.0, 300.0, 200.0], 2.0))));
        assert_eq!(
            presence(json!({ "viewId": "v", "rect": rect })).unwrap().1.map(|p| p.dpr),
            Some(1.0),
            "dpr absent: 1"
        );
        assert_eq!(
            presence(json!({ "viewId": "v", "rect": null })).unwrap().1,
            None,
            "rect null: the canvas went away"
        );
        for bad in [
            json!({ "viewId": "v" }),
            json!({ "rect": null }),
            json!({ "viewId": "v", "rect": rect, "dpr": null }),
            json!({ "viewId": "v", "rect": rect, "dpr": 0 }),
            json!({ "viewId": "v", "rect": { "x": 0, "y": 0, "w": 0, "h": 5 } }),
            json!({ "viewId": "v", "rect": {} }),
        ] {
            assert_eq!(code(&presence(bad.clone())).as_deref(), Some("arg.invalid"), "{bad}");
        }
    }
}
