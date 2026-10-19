//! `job.*` (11 §7.7, §15.2): the job table. Job commands run on their own threads; `job.wait` waits on the caller's
//! thread (`ExecutorHandle::invoke` handles it there; inside a batch it only polls).

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tt_registry::{CommandError, command};

use super::common::{NoParams, event_not_invocable};
use crate::cx::Cx;
use crate::jobs::JobRecord;

/// `job.list` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ListParams {
    /// One job.
    #[serde(default)]
    job_id: Option<String>,
    /// Every session's jobs (default: the caller's own).
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
    let session = if p.all { None } else { Some(cx.origin().session_key()) };
    Ok(cx.session().jobs.list(p.job_id.as_deref(), session.as_deref()))
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
    /// The job was still running when the timeout passed.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    timed_out: bool,
}

pub(crate) fn wait_result(record: JobRecord, timed_out: bool) -> Result<Value, CommandError> {
    tt_registry::encode_result(&WaitResult { record, timed_out })
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
    // On the document thread (inside a batch) the wait never blocks: it reports the current record.
    let (record, timed_out) = cx.session().jobs.wait(&p.job_id, Duration::ZERO)?;
    Ok(WaitResult { timed_out: timed_out && p.timeout_ms > 0, record })
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
    cx.session().jobs.cancel(&p.job_id)
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
