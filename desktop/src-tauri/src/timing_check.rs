//! M5 C5.1b (plan §9, design note `reviews/M5-design-timeline-view.md` §10): the Timeline's three-way check through the
//! window's event path, `toontown --timing-check <out.json> [--timing-check-external]`. A diagnostics build only
//! (`m0-diagnostics`, as `--ipc-bench`).
//!
//! The host opens the kernel's sample document (`tt_model::fixture::sample_document(1)`, the document the kernel's
//! three-way tests use) and the window plays each gesture of the plan's §6.3 on the Timeline
//! (`desktop/src/diagnostics/timingCheck.ts`): real DOM pointer events on its canvases and layer list, or, with
//! `--timing-check-external`, a person's or a script's real input (`scripts/c51b-timing-three-way.py`). For each gesture
//! the window replays the samples it is about to send through [`timing_replay`] (the kernel interpreter, nothing
//! committed) before its own release commits them, then compares the two plans, and the state its commit made with
//! the state the plan's commands make run directly. The report is written here and the app exits; without one in
//! [`TIMEOUT`] the app writes a failure and exits with status 2.

use std::io::Write as _;
use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Manager, State};

use crate::diagnostics::{Diagnostics, TimingCheckSettings};
use crate::host::Host;

/// The host gives up and exits with status 2 if no report arrives within this time.
pub const TIMEOUT: Duration = Duration::from_secs(900);

/// What the window runs.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimingCheckPlan {
    /// See [`TimingCheckSettings::external`].
    pub external: bool,
    /// See [`TimingCheckSettings::only`].
    pub only: Option<String>,
    /// See [`TimingCheckSettings::budgets`].
    pub budgets: bool,
    /// The report's folder: where the budgets save their scene while building it.
    pub work_dir: Option<String>,
}

/// The plan when the app was started with `--timing-check`, otherwise `null`.
#[tauri::command]
#[must_use]
pub fn timing_check_plan(diagnostics: State<'_, Diagnostics>) -> Option<TimingCheckPlan> {
    diagnostics.timing_check.as_ref().map(|t| TimingCheckPlan {
        external: t.external,
        only: t.only.clone(),
        budgets: t.budgets,
        // The out path as given may be relative; the work folder holds a project, whose path must be absolute.
        work_dir: std::path::absolute(&t.out)
            .ok()
            .and_then(|out| out.parent().map(|p| p.to_string_lossy().into_owned())),
    })
}

/// The kernel interpreter's plan for the window's layout and samples (`ExecutorHandle::timing_replay`, nothing
/// committed): `{"layout": ..., "samples": [...]}` in, the replay's value out.
///
/// # Errors
/// The JSON text of the interpreter's refusal, or of `arg.invalid` for a request that is not JSON.
#[tauri::command]
pub async fn timing_replay(app: AppHandle, json: String) -> Result<Value, String> {
    let request: Value = serde_json::from_str(&json).map_err(|e| refusal("arg.invalid", &e.to_string()))?;
    let layout = request.get("layout").cloned().unwrap_or(Value::Null);
    let samples = request.get("samples").cloned().unwrap_or(Value::Null);
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<Host>().exec().timing_replay(tt_executor::Origin::User, layout, samples, false)
    })
    .await
    .map_err(|e| refusal("internal", &e.to_string()))?
    .map_err(|e| refusal(&e.code, &e.message))
}

/// Writes the window's report next to host facts and exits the app.
///
/// # Errors
/// `diagnostics.notRequested` without `--timing-check`; `fs.unwritable` when the file cannot be written (exit 3).
#[tauri::command]
pub fn timing_check_report(
    app: AppHandle,
    diagnostics: State<'_, Diagnostics>,
    engine: State<'_, tt_engine::Engine>,
    report: Value,
) -> Result<(), tt_engine::ErrorInfo> {
    let Some(settings) = diagnostics.timing_check.as_ref() else {
        return Err(info("diagnostics.notRequested", "the app was not started with --timing-check".into()));
    };
    match write_json(&settings.out, &json!({ "host": engine.info(), "frontend": report })) {
        Ok(()) => {
            app.exit(0);
            Ok(())
        }
        Err(e) => {
            app.exit(3);
            Err(info("fs.unwritable", format!("cannot write {}: {e}", settings.out.display())))
        }
    }
}

/// Opens the sample document again, its changes discarded: each gesture starts from the same document.
///
/// # Errors
/// The JSON text of the open's refusal.
#[tauri::command]
pub async fn timing_check_reset(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || open_sample(&app))
        .await
        .map_err(|e| refusal("internal", &e.to_string()))?
}

fn open_sample(app: &AppHandle) -> Result<(), String> {
    let doc = tt_model::fixture::sample_document(1).map_err(|e| refusal("internal", &format!("{e:?}")))?;
    let options =
        tt_executor::OpenOptions { path: None, read_only: false, on_unsaved: tt_executor::UnsavedPolicy::Discard };
    let env = app.state::<Host>().exec().open_document_with(doc, options, tt_executor::Origin::User);
    match env.error {
        Some(e) => Err(refusal(&e.code, &e.message)),
        None => Ok(()),
    }
}

/// At start-up: the sample document opened, and the timeout armed.
pub fn start(app: &AppHandle, settings: &TimingCheckSettings) {
    if let Err(e) = open_sample(app) {
        let _ = writeln!(std::io::stderr(), "toontown: --timing-check could not open the sample document: {e}");
    }
    let app = app.clone();
    let out = settings.out.clone();
    std::thread::spawn(move || {
        std::thread::sleep(TIMEOUT);
        let _ = write_json(
            &out,
            &json!({ "frontend": { "ok": false, "error": "timeout waiting for timing_check_report" } }),
        );
        app.exit(2);
    });
}

fn write_json(path: &Path, value: &Value) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
    std::fs::write(path, text + "\n")
}

fn refusal(code: &str, message: &str) -> String {
    json!({ "code": code, "message": message }).to_string()
}

fn info(code: &str, message: String) -> tt_engine::ErrorInfo {
    tt_engine::ErrorInfo { code: code.to_owned(), message, details: json!({}) }
}
