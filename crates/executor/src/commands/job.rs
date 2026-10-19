//! `job.*` (11 §7.7, §15.2): the job table. Job commands run on their own threads; `job.wait` waits on the caller's
//! thread (`ExecutorHandle::invoke` handles it there; inside a batch it only polls).
//!
//! **Ownership (11 §15.2, §6; C12-R1-05).** A job belongs to the session that started it. An agent session sees,
//! waits for and cancels only its own jobs — another session's job is `job.notFound`, never revealed — and `all`
//! (every session's jobs) is for the GUI's own origin. Cancelling is deferred to the commit of the call (never by a
//! dry run or a failed batch; C12-R1-02).

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tt_registry::{CommandError, command};

use super::common::{NoParams, event_not_invocable};
use crate::cx::{Cx, Effect};
use crate::jobs::JobRecord;
use crate::types::Origin;

/// Whether an origin may see a job started by `session` (the GUI's own origins see every job).
pub(crate) fn may_see(origin: &Origin, session: &str) -> bool {
    match origin {
        Origin::Agent { .. } => origin.session_key() == session,
        Origin::User | Origin::System | Origin::Load => true,
    }
}

fn not_found(job_id: &str) -> CommandError {
    CommandError::with_details("job.notFound", format!("no job `{job_id}`"), serde_json::json!({ "jobId": job_id }))
}

/// The record of a job the caller may see.
fn visible(cx: &Cx, job_id: &str) -> Result<JobRecord, CommandError> {
    let (record, _) = cx.session().jobs.wait(job_id, Duration::ZERO)?;
    if may_see(cx.origin(), &record.session) { Ok(record) } else { Err(not_found(job_id)) }
}

/// `job.list` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ListParams {
    /// One job.
    #[serde(default)]
    job_id: Option<String>,
    /// Every session's jobs (the GUI only; default: the caller's own).
    #[serde(default)]
    all: bool,
}

#[command(
    id = "job.list",
    kind = query,
    label = "List jobs",
    scopes = [read],
    example(title = "The caller's jobs", params = "{}"),
    gui = "Progress area of the status bar (M4)",
)]
pub(crate) fn job_list(cx: &mut Cx, p: ListParams) -> Result<Vec<JobRecord>, CommandError> {
    if p.all && matches!(cx.origin(), Origin::Agent { .. }) {
        return Err(CommandError::with_details(
            tt_registry::codes::PERMISSION_DENIED,
            "every session's jobs are listed for the GUI only (11 §15.2)",
            serde_json::json!({ "commandId": "job.list", "parameter": "all" }),
        ));
    }
    let session = if p.all { None } else { Some(cx.origin().session_key()) };
    let origin = cx.origin().clone();
    Ok(cx
        .session()
        .jobs
        .list(p.job_id.as_deref(), session.as_deref())
        .into_iter()
        .filter(|r| may_see(&origin, &r.session))
        .collect())
}

fn sixty_seconds() -> u64 {
    60_000
}

/// `job.wait` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct WaitParams {
    /// The job.
    pub(crate) job_id: String,
    /// How long to wait.
    #[serde(default = "sixty_seconds")]
    #[schemars(range(min = 0, max = 3_600_000))]
    pub(crate) timeout_ms: u64,
}

/// `job.wait` result: the job record and whether the wait timed out.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WaitResult {
    /// The record.
    #[serde(flatten)]
    record: JobRecord,
    /// `true` when the job was still running when the timeout passed; absent otherwise (A / 11 §15.2 `timedOut?`;
    /// C12-R1-13).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    timed_out: Option<bool>,
}

pub(crate) fn wait_result(record: JobRecord, timed_out: bool) -> Result<Value, CommandError> {
    tt_registry::encode_result(&WaitResult { record, timed_out: timed_out.then_some(true) })
}

#[command(
    id = "job.wait",
    kind = query,
    label = "Wait for a job",
    scopes = [read],
    example(title = "Wait up to 5 s", params = r#"{"jobId": "job-1", "timeoutMs": 5000}"#),
    agent_only = "agents wait for jobs; the GUI shows progress",
)]
pub(crate) fn job_wait(cx: &mut Cx, p: WaitParams) -> Result<WaitResult, CommandError> {
    // On the document thread the wait never blocks: it reports the current record, and `ExecutorHandle::invoke`
    // completes a pending wait on the caller's thread.
    let record = visible(cx, &p.job_id)?;
    let timed_out = !record.state.is_final();
    Ok(WaitResult { timed_out: timed_out.then_some(true), record })
}

/// `{jobId}` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct JobIdParams {
    /// The job.
    job_id: String,
}

#[command(
    id = "job.cancel",
    kind = ui,
    label = "Cancel job",
    scopes = [ui],
    example(title = "Cancel a job", params = r#"{"jobId": "job-1"}"#),
    gui = "Cancel button of the progress area (M4)",
)]
pub(crate) fn job_cancel(cx: &mut Cx, p: JobIdParams) -> Result<JobRecord, CommandError> {
    let record = visible(cx, &p.job_id)?;
    if !record.state.is_final() {
        cx.defer(Effect::CancelJob(p.job_id));
    }
    Ok(record)
}

#[command(
    id = "job.progress",
    kind = event,
    label = "Job progress",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn job_progress(_cx: &mut Cx, _p: NoParams) -> Result<JobRecord, CommandError> {
    Err(event_not_invocable("job.progress"))
}

#[command(
    id = "job.finished",
    kind = event,
    label = "Job finished",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn job_finished(_cx: &mut Cx, _p: NoParams) -> Result<JobRecord, CommandError> {
    Err(event_not_invocable("job.finished"))
}
