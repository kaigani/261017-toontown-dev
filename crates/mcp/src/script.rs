//! Batch-script mode (11 §3): a JSON file with the structure of the `batch` tool (§7.3) runs without an MCP client,
//! through the same command layer and the same agent session machinery (scopes, roots, confirmations, audit log),
//! then the process exits 0 when every step succeeded and non-zero otherwise.
//!
//! The log (`--out`) is JSON lines: a `start` record, one `step` record per step envelope, then an `end` record with
//! the batch envelope's status, error and `documentRev`. With no client to ask, a step that needs confirmation fails
//! the script unless its category was pre-authorised (`--preauthorize`) and the step passes `confirm: true`.

use std::io::Write;
use std::path::Path;
use std::time::Instant;

use serde_json::{Value, json};
use tt_executor::agents::AgentConnect;
use tt_executor::{BatchRequest, Envelope, ExecutorHandle, Origin, Status};

use crate::ServerOptions;

/// The outcome of a batch script.
#[derive(Debug)]
pub struct ScriptOutcome {
    /// The batch envelope.
    pub envelope: Envelope,
    /// Whether every step succeeded.
    pub ok: bool,
}

/// Reads a batch script.
///
/// # Errors
/// A message naming the file and what is wrong with it.
pub fn load(path: &Path) -> Result<BatchRequest, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("{} is not JSON: {e}", path.display()))?;
    // The batch tool's input, or a log written by `agent.activity.export {format: "batch"}` (same structure).
    serde_json::from_value(v)
        .map_err(|e| format!("{} is not a batch (label, steps[{{id, params, as?}}]): {e}", path.display()))
}

/// Runs a batch script as an agent session named `batch-script`, writing the log to `log` when given.
///
/// # Errors
/// A message when the session cannot be connected or the log cannot be written.
pub fn run(
    handle: &ExecutorHandle,
    opts: &ServerOptions,
    req: BatchRequest,
    log: Option<&mut dyn Write>,
) -> Result<ScriptOutcome, String> {
    let session = handle
        .connect_agent(AgentConnect {
            client: "batch-script".to_owned(),
            policy: opts.policy,
            scopes: opts.scopes.clone(),
            roots: opts.roots.clone(),
            preauthorized: opts.preauthorized.clone(),
            transport: "batch".to_owned(),
        })
        .map_err(|e| e.message)?;
    let origin = Origin::Agent {
        session: session.session_id.clone(),
        name: session.client.clone(),
        scopes: session.scopes.clone(),
    };
    let mut lines: Vec<Value> = vec![json!({
        "record": "start", "time": tt_persist::time::now_iso(), "label": req.label, "steps": req.steps.len(),
        "atomic": req.atomic, "dryRun": req.dry_run, "sessionId": session.session_id, "policy": session.policy.as_str(),
    })];
    let started = Instant::now();
    let envelope = handle.batch_audited(req, origin, "batch-script");
    for (i, step) in envelope.steps.iter().flatten().enumerate() {
        lines.push(json!({ "record": "step", "index": i, "envelope": step }));
    }
    let ok = envelope.status == Status::Ok;
    lines.push(json!({
        "record": "end", "status": envelope.status, "ok": ok, "error": envelope.error, "documentRev": envelope.document_rev,
        "undo": envelope.undo, "durationMs": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    }));
    let _ = handle.disconnect_agent(&session.session_id, "batch script finished");
    if let Some(out) = log {
        for l in &lines {
            writeln!(out, "{l}").map_err(|e| format!("cannot write the log: {e}"))?;
        }
        out.flush().map_err(|e| format!("cannot write the log: {e}"))?;
    }
    Ok(ScriptOutcome { envelope, ok })
}
