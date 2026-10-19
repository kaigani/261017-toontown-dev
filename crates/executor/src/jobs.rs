//! The job runner (11 §7.7, §15.2): long-running commands return `{jobId}` at once and run on their own thread,
//! outside the document thread, reporting progress; `job.list`, `job.wait` and `job.cancel` read and control them.
//! `job.wait` blocks the caller's thread, never the document thread (11 P5).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;
use tt_registry::CommandError;

/// Job state (11 §7.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum JobState {
    /// Accepted, not started.
    Queued,
    /// Running.
    Running,
    /// Finished with a result.
    Succeeded,
    /// Finished with an error.
    Failed,
    /// Cancelled.
    Cancelled,
}

impl JobState {
    /// Whether the job reached a final state.
    #[must_use]
    pub fn is_final(self) -> bool {
        matches!(self, JobState::Succeeded | JobState::Failed | JobState::Cancelled)
    }
}

/// A job record (11 §7.7 `job_status`).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobRecord {
    /// Job id.
    pub job_id: String,
    /// The job command.
    pub command_id: String,
    /// State.
    pub state: JobState,
    /// Progress 0..1.
    pub progress: f64,
    /// Current step text.
    pub message: String,
    /// Start time (ISO 8601 UTC).
    pub started_at: String,
    /// Final result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Final error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<CommandError>,
    /// Session that started it.
    pub session: String,
}

#[derive(Debug, Default)]
struct Table {
    jobs: BTreeMap<String, JobRecord>,
    cancel: BTreeMap<String, Arc<AtomicBool>>,
    next: u64,
}

/// Receives `job.progress` and `job.finished` events.
pub type JobNotifier = Box<dyn Fn(crate::session::Event) + Send + Sync>;

/// The shared job table.
#[derive(Default)]
pub struct JobTable {
    table: Mutex<Table>,
    changed: Condvar,
    notify: Option<JobNotifier>,
}

impl std::fmt::Debug for JobTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobTable").field("table", &self.table).finish_non_exhaustive()
    }
}

/// Handle a running job uses to report progress and observe cancellation.
#[derive(Debug, Clone)]
pub struct JobProgress {
    table: Arc<JobTable>,
    job_id: String,
    cancel: Arc<AtomicBool>,
}

impl JobProgress {
    /// Reports progress (clamped to 0..1) and a step text.
    pub fn report(&self, progress: f64, message: &str) {
        self.table.update(&self.job_id, |r| {
            r.progress = progress.clamp(0.0, 1.0);
            message.clone_into(&mut r.message);
        });
    }

    /// Whether cancellation was requested.
    #[must_use]
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
}

impl JobTable {
    /// A table that reports progress and completion as `job.progress` / `job.finished` events (11 §9).
    #[must_use]
    pub fn with_notifier(notify: JobNotifier) -> Self {
        Self { notify: Some(notify), ..Self::default() }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Table> {
        self.table.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn update(&self, id: &str, f: impl FnOnce(&mut JobRecord)) {
        let mut t = self.lock();
        let record = t.jobs.get_mut(id).map(|r| {
            f(r);
            r.clone()
        });
        drop(t);
        self.changed.notify_all();
        if let (Some(notify), Some(r)) = (&self.notify, record) {
            let name = if r.state.is_final() { "job.finished" } else { "job.progress" };
            notify(crate::session::Event {
                name: name.to_owned(),
                payload: serde_json::to_value(&r).unwrap_or(Value::Null),
            });
        }
    }

    /// Starts a job on its own thread and returns its id at once.
    ///
    /// # Errors
    /// `internal` if the thread cannot be spawned.
    pub fn submit<F>(self: &Arc<Self>, command_id: &str, session: &str, work: F) -> Result<String, CommandError>
    where
        F: FnOnce(&JobProgress) -> Result<Value, CommandError> + Send + 'static,
    {
        let cancel = Arc::new(AtomicBool::new(false));
        let job_id = {
            let mut t = self.lock();
            t.next = t.next.saturating_add(1);
            let id = format!("job-{}", t.next);
            t.jobs.insert(
                id.clone(),
                JobRecord {
                    job_id: id.clone(),
                    command_id: command_id.to_owned(),
                    state: JobState::Queued,
                    progress: 0.0,
                    message: String::new(),
                    started_at: crate::time::now_iso(),
                    result: None,
                    error: None,
                    session: session.to_owned(),
                },
            );
            t.cancel.insert(id.clone(), cancel.clone());
            id
        };
        let progress = JobProgress { table: self.clone(), job_id: job_id.clone(), cancel };
        std::thread::Builder::new()
            .name(format!("tt-{job_id}"))
            .spawn(move || {
                progress.table.update(&progress.job_id, |r| r.state = JobState::Running);
                let outcome = work(&progress);
                let cancelled = progress.cancelled();
                progress.table.update(&progress.job_id, |r| match outcome {
                    _ if cancelled => r.state = JobState::Cancelled,
                    Ok(v) => {
                        r.state = JobState::Succeeded;
                        r.progress = 1.0;
                        r.result = Some(v);
                    }
                    Err(e) => {
                        r.state = JobState::Failed;
                        r.error = Some(e);
                    }
                });
            })
            .map_err(|e| CommandError::internal(format!("cannot start job thread: {e}")))?;
        Ok(job_id)
    }

    /// Records, optionally one job or one session's.
    #[must_use]
    pub fn list(&self, job_id: Option<&str>, session: Option<&str>) -> Vec<JobRecord> {
        let t = self.lock();
        t.jobs
            .values()
            .filter(|r| job_id.is_none_or(|j| r.job_id == j) && session.is_none_or(|s| r.session == s))
            .cloned()
            .collect()
    }

    /// Waits until the job is final or the timeout passes; returns the record and whether it timed out.
    ///
    /// # Errors
    /// `job.notFound` for an unknown job.
    pub fn wait(&self, job_id: &str, timeout: Duration) -> Result<(JobRecord, bool), CommandError> {
        let deadline = Instant::now() + timeout;
        let mut t = self.lock();
        loop {
            let Some(r) = t.jobs.get(job_id) else { return Err(not_found(job_id)) };
            if r.state.is_final() {
                return Ok((r.clone(), false));
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok((r.clone(), true));
            }
            t = self.changed.wait_timeout(t, deadline - now).unwrap_or_else(PoisonError::into_inner).0;
        }
    }

    /// Requests cancellation; returns the record.
    ///
    /// # Errors
    /// `job.notFound` for an unknown job.
    pub fn cancel(&self, job_id: &str) -> Result<JobRecord, CommandError> {
        let t = self.lock();
        let Some(flag) = t.cancel.get(job_id) else { return Err(not_found(job_id)) };
        flag.store(true, Ordering::SeqCst);
        t.jobs.get(job_id).cloned().ok_or_else(|| not_found(job_id))
    }
}

fn not_found(job_id: &str) -> CommandError {
    CommandError::with_details("job.notFound", format!("no job `{job_id}`"), serde_json::json!({ "jobId": job_id }))
}
