//! The native side of the shell (M4 C4.1; ADR-0021):
//! - the event pump: every engine event is numbered and forwarded to the webviews as JSON text; `shell.*` notices
//!   are applied to native windows (geometry, full screen, minimise, raise); preference changes of the agent server
//!   start or stop attached mode; state changes refresh the native menu;
//! - user gestures on native windows become commands (`window.geometry.set` when the user moves or resizes the main
//!   window), and closing the main window runs the quit flow in the front end (unsaved-changes rule, 01 §1.2).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow, WindowEvent};

use crate::host::Host;

/// Event name of the forwarded engine events.
pub const ENGINE_EVENT: &str = "engine-event";

/// A menu refresh is scheduled.
static MENU_PENDING: AtomicBool = AtomicBool::new(false);

/// The latest geometry the user gave the window, sent once the drag pauses (a drag moves the window many times a
/// second; one `window.geometry.set` per pause is enough).
static PENDING_GEOMETRY: Mutex<Option<Value>> = Mutex::new(None);

/// Set while the host applies a geometry the executor asked for, so the resulting OS events are not reported back as
/// user gestures (the kernel ignores equal values anyway; this keeps the reports quiet).
static APPLYING: AtomicBool = AtomicBool::new(false);

fn log(message: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr(), "toontown: {message}");
}

/// Ends the application shortly after the reply of a successful `app.quit` has been sent.
pub fn quit_soon(app: &AppHandle) {
    let app = app.clone();
    let _ = std::thread::Builder::new().name("tt-quit".into()).spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        if let Some(h) = app.try_state::<Host>() {
            h.stop_http();
        }
        app.exit(0);
    });
}

fn main_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window("main")
}

/// Applies the executor's `shell.window` notice to the main window.
fn apply_window(app: &AppHandle, p: &Value) {
    let Some(w) = main_window(app) else { return };
    let scale = w.scale_factor().unwrap_or(1.0);
    APPLYING.store(true, Ordering::Release);
    let full = p.get("fullScreen").and_then(Value::as_bool).unwrap_or(false);
    if w.is_fullscreen().unwrap_or(false) != full {
        let _ = w.set_fullscreen(full);
    }
    let minimized = p.get("minimized").and_then(Value::as_bool).unwrap_or(false);
    if minimized {
        let _ = w.minimize();
    } else if w.is_minimized().unwrap_or(false) {
        let _ = w.unminimize();
    }
    // The floats are tool windows of the main window: hidden while it is minimised (ruling O-2).
    crate::floats::show_all(app, !minimized);
    if !full {
        let maximized = p.get("maximized").and_then(Value::as_bool).unwrap_or(false);
        if maximized {
            let _ = w.maximize();
        } else {
            if w.is_maximized().unwrap_or(false) {
                let _ = w.unmaximize();
            }
            let num = |k: &str| p.get(k).and_then(Value::as_f64);
            if let (Some(x), Some(y)) = (num("x"), num("y")) {
                let _ = w.set_position(PhysicalPosition::new((x * scale).round() as i32, (y * scale).round() as i32));
            }
            if let (Some(wd), Some(ht)) = (num("width"), num("height")) {
                let _ = w.set_size(PhysicalSize::new((wd * scale).round() as u32, (ht * scale).round() as u32));
            }
        }
    }
    // Let the OS events of these calls pass before user gestures are reported again.
    let _ = std::thread::Builder::new().spawn(|| {
        std::thread::sleep(Duration::from_millis(300));
        APPLYING.store(false, Ordering::Release);
    });
}

fn raise_all(app: &AppHandle) {
    for w in app.webview_windows().values() {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Forwards engine events to the webviews and applies the host's share of them. Runs on its own thread for the life
/// of the application.
pub fn start_event_pump(app: &AppHandle) {
    let host = app.state::<Host>();
    let rx = host.exec().subscribe();
    let generation = host.generation();
    let app = app.clone();
    let _ = std::thread::Builder::new().name("tt-event-pump".into()).spawn(move || {
        while let Ok(e) = rx.recv() {
            let host = app.state::<Host>();
            // After a restart the old executor's events are not forwarded (reviews/M4-design-engine-restart.md).
            if host.generation() != generation {
                break;
            }
            let seq = host.next_seq();
            // Each generation numbers its events from 1; the window resyncs on a new generation (ruling O-14).
            let payload = json!({ "seq": seq, "generation": generation, "name": e.name, "payload": e.payload });
            match tt_engine::wire::to_wire_json(&payload) {
                Ok(text) => {
                    if let Err(err) = app.emit(ENGINE_EVENT, text) {
                        log(&format!("cannot forward an engine event: {err}"));
                    }
                }
                Err(err) => log(&format!("cannot serialise an engine event: {}", err.message)),
            }
            match e.name.as_str() {
                "shell.window" => {
                    let (a, p) = (app.clone(), e.payload.clone());
                    let _ = app.run_on_main_thread(move || apply_window(&a, &p));
                }
                "shell.raiseAll" => {
                    let a = app.clone();
                    let _ = app.run_on_main_thread(move || raise_all(&a));
                }
                "prefs.changed" => {
                    let keys: Vec<String> = e
                        .payload
                        .get("keys")
                        .and_then(Value::as_array)
                        .map(|k| k.iter().filter_map(Value::as_str).map(str::to_owned).collect())
                        .unwrap_or_default();
                    if keys.iter().any(|k| k.starts_with("agent.server.")) {
                        sync_agent_server(&app);
                    }
                }
                _ => {}
            }
            // The menu re-reads its facts, and the float windows follow the kernel (ruling O-2), once per burst of
            // state changes (100 ms), not once per event.
            let layout_event = matches!(e.name.as_str(), "app.state.changed" | "workspace.changed");
            if (crate::menu::refreshes(&e.name) || layout_event) && !MENU_PENDING.swap(true, Ordering::AcqRel) {
                let a = app.clone();
                let _ = std::thread::Builder::new().name("tt-menu".into()).spawn(move || {
                    std::thread::sleep(Duration::from_millis(100));
                    MENU_PENDING.store(false, Ordering::Release);
                    crate::menu::refresh(&a);
                    crate::floats::reconcile(&a);
                });
            }
        }
    });
}

/// Applies the geometry the executor restored from the settings (01 §11) to the main window at start.
pub fn restore_window(app: &AppHandle) {
    let host = app.state::<Host>();
    let g = host.run("window.geometry.get", json!({}), false, None).result.unwrap_or(Value::Null);
    if g.is_object() {
        let mut p = g;
        if let Some(m) = p.as_object_mut() {
            m.insert("minimized".into(), Value::Bool(false));
        }
        let a = app.clone();
        let _ = app.run_on_main_thread(move || apply_window(&a, &p));
    }
}

/// Event name of the host's own notices to the front end.
pub const HOST_NOTICE_EVENT: &str = "host-notice";

/// Sends a notice of the host to the front end (shown as a non-modal message with its help entry).
pub fn notice(app: &AppHandle, message_id: &str, severity: &str, text: &str) {
    let payload = json!({ "messageId": message_id, "severity": severity, "text": text });
    if let Ok(text) = tt_engine::wire::to_wire_json(&payload) {
        let _ = app.emit(HOST_NOTICE_EVENT, text);
    }
}

/// Starts or stops attached mode after `agent.server.enabled` (or `--mcp-http`) (11 §4).
pub fn sync_agent_server(app: &AppHandle) {
    let host = app.state::<Host>();
    let enabled = host.options.mcp_http.is_some()
        || host
            .run("prefs.get", json!({ "keys": ["agent.server.enabled"] }), false, None)
            .result
            .and_then(|r| r.pointer("/values/agent.server.enabled").and_then(Value::as_bool))
            .unwrap_or(false);
    if enabled {
        match host.start_http() {
            Ok(url) => log(&format!("agent server on {url}")),
            Err(e) => {
                log(&e);
                // The user sees why attached mode is off (ruling O-6): the token could not be made, the address is
                // not a loopback address, or no port could be bound.
                notice(app, "agent.serverFailed", "error", &format!("The agent server did not start: {e}"));
            }
        }
    } else {
        host.stop_http();
    }
}

/// Window events of the main window: user moves and resizes become `window.geometry.set`; closing runs the quit flow
/// in the front end (01 §1.2: closing the last main window is Quit, with the unsaved-changes rule).
pub fn on_window_event(window: &tauri::Window, event: &WindowEvent) {
    if window.label().starts_with(crate::floats::PREFIX) {
        crate::floats::on_window_event(window, event);
        return;
    }
    if window.label() != "main" {
        return;
    }
    match event {
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let event = tt_engine::wire::MenuCommandEvent {
                command: "app.quit".into(),
                params: serde_json::Map::new(),
                handler: tt_engine::wire::MenuHandler::Frontend,
                milestone: None,
            };
            if let Ok(text) = tt_engine::wire::to_wire_json(&event) {
                let _ = window.emit_to("main", crate::menu::MENU_COMMAND_EVENT, text);
            }
        }
        WindowEvent::Destroyed => {
            // The window's reports end with it (ruling O-9).
            if let Some(host) = window.app_handle().try_state::<Host>() {
                host.forget_window_reports(window.label());
            }
        }
        WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
            if APPLYING.load(Ordering::Acquire) {
                return;
            }
            let scale = window.scale_factor().unwrap_or(1.0);
            let (Ok(pos), Ok(size)) = (window.outer_position(), window.inner_size()) else { return };
            if window.is_minimized().unwrap_or(false) || window.is_fullscreen().unwrap_or(false) {
                return;
            }
            let params = json!({
                "x": (f64::from(pos.x) / scale).round() as i64,
                "y": (f64::from(pos.y) / scale).round() as i64,
                "width": ((f64::from(size.width) / scale).round() as u64).max(1),
                "height": ((f64::from(size.height) / scale).round() as u64).max(1),
                "maximized": window.is_maximized().unwrap_or(false),
            });
            let app = window.app_handle().clone();
            // The gesture produces the command (ADR-0021), off the main thread, once the drag pauses.
            let first = PENDING_GEOMETRY.lock().unwrap_or_else(PoisonError::into_inner).replace(params).is_none();
            if first {
                let _ = std::thread::Builder::new().name("tt-geometry".into()).spawn(move || {
                    std::thread::sleep(Duration::from_millis(250));
                    let latest = PENDING_GEOMETRY.lock().unwrap_or_else(PoisonError::into_inner).take();
                    if let Some(p) = latest {
                        let _ = app.state::<Host>().run("window.geometry.set", p, false, None);
                    }
                });
            }
        }
        _ => {}
    }
}

/// Opens documents the OS or a second instance asked for (01 §1.1 rules 1 and 8): each goes to the front end as
/// `project.open {path}`, so the unsaved-changes rule and the progress window apply as for the user's own Open.
pub fn open_documents(app: &AppHandle, paths: &[std::path::PathBuf]) {
    for p in paths {
        let mut params = serde_json::Map::new();
        params.insert("path".into(), Value::String(p.to_string_lossy().into_owned()));
        let event = tt_engine::wire::MenuCommandEvent {
            command: "project.open".into(),
            params,
            handler: tt_engine::wire::MenuHandler::Frontend,
            milestone: None,
        };
        if let Ok(text) = tt_engine::wire::to_wire_json(&event) {
            let _ = app.emit_to("main", crate::menu::MENU_COMMAND_EVENT, text);
        }
    }
    if let Some(w) = main_window(app) {
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}
