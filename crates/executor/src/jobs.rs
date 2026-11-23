//! The job runner (11 §7.7, §15.2): long-running commands return `{jobId}` at once and run on their own thread,
//! outside the document thread, reporting progress; `job.list`, `job.wait` and `job.cancel` read and control them.
//! `job.wait` blocks the caller's thread, never the document thread (11 P5).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU8, Ordering};
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
    /// The job can still be cancelled: not final, not past its commit point, no cancellation requested yet (the
    /// progress window offers Cancel only then, 01 §9.5; C43B-R2-06).
    #[serde(default)]
    pub cancellable: bool,
    /// Warnings the job recorded (recovery receipts of `render.range.run`, C33-R2-02); kept in every final state.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<Value>,
}

/// A job is running and cancellable.
const RUNNING: u8 = 0;
/// Cancellation was requested before the job's commit point.
const CANCEL_REQUESTED: u8 = 1;
/// The job passed its commit point (an open installed, a save writing): it can no longer be cancelled.
const COMMITTED: u8 = 2;

#[derive(Debug, Default)]
struct Table {
    jobs: BTreeMap<String, JobRecord>,
    cancel: BTreeMap<String, Arc<AtomicU8>>,
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
    cancel: Arc<AtomicU8>,
}

impl JobProgress {
    /// Reports progress (clamped to 0..1) and a step text.
    pub fn report(&self, progress: f64, message: &str) {
        self.table.update(&self.job_id, |r| {
            r.progress = progress.clamp(0.0, 1.0);
            message.clone_into(&mut r.message);
        });
    }

    /// Whether cancellation was requested (before the commit point).
    #[must_use]
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst) == CANCEL_REQUESTED
    }

    /// Passes the job's commit point (10 LOAD-02: an open installs its project; SAVE-03: a save starts writing): from
    /// here on the job cannot be cancelled. Returns `false` when cancellation was requested first.
    #[must_use]
    pub fn commit(&self) -> bool {
        match self.cancel.compare_exchange(RUNNING, COMMITTED, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => {
                self.table.update(&self.job_id, |r| r.cancellable = false);
                true
            }
            Err(state) => state == COMMITTED,
        }
    }

    /// Records a warning in the job's record, kept whatever the final state (a recovery receipt, C33-R2-02).
    pub fn warn(&self, warning: Value) {
        self.table.update(&self.job_id, |r| r.warnings.push(warning));
    }

    /// The job's ID.
    #[must_use]
    pub fn job_id(&self) -> &str {
        &self.job_id
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
        // The event is broadcast before the table lock is released, so whoever observes the new state (a waiter,
        // `job.list`) can rely on its `job.progress`/`job.finished` event having been sent already. The notifier only
        // sends to subscriber channels; it never touches the table.
        if let (Some(notify), Some(r)) = (&self.notify, record) {
            let name = if r.state.is_final() { "job.finished" } else { "job.progress" };
            notify(crate::session::Event {
                name: name.to_owned(),
                payload: serde_json::to_value(&r).unwrap_or(Value::Null),
            });
        }
        drop(t);
        self.changed.notify_all();
    }

    /// Starts a job on its own thread and returns its id at once.
    ///
    /// # Errors
    /// `internal` if the thread cannot be spawned.
    pub fn submit<F>(self: &Arc<Self>, command_id: &str, session: &str, work: F) -> Result<String, CommandError>
    where
        F: FnOnce(&JobProgress) -> Result<Value, CommandError> + Send + 'static,
    {
        let cancel = Arc::new(AtomicU8::new(RUNNING));
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
                    cancellable: true,
                    warnings: Vec::new(),
                },
            );
            t.cancel.insert(id.clone(), cancel.clone());
            id
        };
        let progress = JobProgress { table: self.clone(), job_id: job_id.clone(), cancel };
        let command = command_id.to_owned();
        std::thread::Builder::new()
            .name(format!("tt-{job_id}"))
            .spawn(move || {
                progress.table.update(&progress.job_id, |r| r.state = JobState::Running);
                // A panicking job ends failed with a typed error instead of leaving its record running for ever (a
                // running record holds a bound such as the capture slots; C42A-R3 capture bound).
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(&progress)))
                    .unwrap_or_else(|_| Err(CommandError::internal(format!("job `{command}` failed unexpectedly"))))
                    .and_then(|v| check_final_result(&command, v));
                let cancelled = progress.cancelled();
                progress.table.update(&progress.job_id, |r| {
                    r.cancellable = false;
                    match outcome {
                        // A job that stops at the cancellation and reports what it completed keeps that report
                        // (`draw.vectorize.run` keeps its finished images, 02 §11.2).
                        Ok(v) if cancelled => {
                            r.state = JobState::Cancelled;
                            r.result = Some(v);
                        }
                        // A cancelled job keeps its typed error (C33-R3-01): the cancellation itself, or what met it
                        // (a rollback the file system refused stays `export.rollbackIncomplete`); its warnings stay too.
                        Err(e) if cancelled => {
                            r.state = JobState::Cancelled;
                            r.error = Some(e);
                        }
                        Ok(v) => {
                            r.state = JobState::Succeeded;
                            r.progress = 1.0;
                            r.result = Some(v);
                        }
                        Err(e) => {
                            r.state = JobState::Failed;
                            r.error = Some(e);
                        }
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

    /// Requests cancellation; returns the record. A job past its commit point is not cancelled (it completes).
    ///
    /// # Errors
    /// `job.notFound` for an unknown job.
    pub fn cancel(&self, job_id: &str) -> Result<JobRecord, CommandError> {
        let requested = {
            let t = self.lock();
            let Some(flag) = t.cancel.get(job_id) else { return Err(not_found(job_id)) };
            flag.compare_exchange(RUNNING, CANCEL_REQUESTED, Ordering::SeqCst, Ordering::SeqCst).is_ok()
        };
        // Once requested, the job offers no second Cancel (C43B-R2-06).
        if requested {
            self.update(job_id, |r| r.cancellable = false);
        }
        self.lock().jobs.get(job_id).cloned().ok_or_else(|| not_found(job_id))
    }

    /// The number of the last job submitted (job ids are `job-<n>`, numbered from 1): a mark taken before an invocation
    /// names the jobs it starts (ruling E-29).
    pub(crate) fn watermark(&self) -> u64 {
        self.lock().next
    }

    /// Requests cancellation of every job submitted after `mark` that has not finished (an invocation that failed
    /// unexpectedly started them; ruling E-29); returns their ids. A finished job is not reversed (ruling E-33), and a job
    /// past its commit point completes (`cancel`).
    pub(crate) fn cancel_since(&self, mark: u64) -> Vec<String> {
        let ids: Vec<String> = self
            .lock()
            .jobs
            .iter()
            .filter(|(id, r)| {
                !r.state.is_final()
                    && id.strip_prefix("job-").and_then(|n| n.parse::<u64>().ok()).is_some_and(|n| n > mark)
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in &ids {
            let _ = self.cancel(id);
        }
        ids
    }
}

fn not_found(job_id: &str) -> CommandError {
    CommandError::with_details("job.notFound", format!("no job `{job_id}`"), serde_json::json!({ "jobId": job_id }))
}

/// A job's final result must satisfy the `x-jobResult` schema its command declares (11 §15.2, §14.2; C12-R1-13); a
/// result that does not fails the job with `internal` (a defect of the job, never handed to a client).
fn check_final_result(command_id: &str, v: Value) -> Result<Value, CommandError> {
    let Some(result) = crate::registry().ok().and_then(|r| r.get(command_id)).map(tt_registry::Entry::result_schema)
    else {
        return Ok(v);
    };
    let Some(mut schema) = result.get("x-jobResult").cloned() else { return Ok(v) };
    if let Some(m) = schema.as_object_mut() {
        m.insert("$schema".into(), Value::String("https://json-schema.org/draft/2020-12/schema".into()));
        // The final result's named types are defined beside the call result (`$defs`).
        if let Some(defs) = result.get("$defs") {
            m.insert("$defs".into(), defs.clone());
        }
    }
    let violations = tt_registry::schema_violations(&schema, &v, 10)
        .map_err(|e| CommandError::internal(format!("final-result schema of `{command_id}`: {}", e.message)))?;
    if violations.is_empty() {
        Ok(v)
    } else {
        Err(CommandError::with_details(
            tt_registry::codes::INTERNAL,
            format!("the result of `{command_id}` violates its declared final-result schema"),
            serde_json::json!({ "commandId": command_id, "violations": violations }),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ruling E-29 (the handler-panic note §3.2): a panic while the job table's lock is held poisons it; every later use
    /// still reads it (`PoisonError::into_inner`), so the panic does not cascade into the next call.
    #[test]
    fn a_poisoned_table_lock_is_still_served() -> Result<(), CommandError> {
        let table = Arc::new(JobTable::default());
        let t = table.clone();
        let poisoned = std::thread::spawn(move || {
            let _guard = t.table.lock();
            std::panic::panic_any("test: a panic while the job table is locked".to_owned());
        })
        .join();
        assert!(poisoned.is_err());
        assert!(table.table.is_poisoned());
        let id = table.submit("test.job", "", |_| Ok(Value::Null))?;
        assert!(table.list(Some(&id), None).len() == 1);
        assert_eq!(table.watermark(), 1);
        Ok(())
    }

    /// The jobs submitted after a mark are the ones cancelled.
    #[test]
    fn cancel_since_names_only_the_jobs_after_the_mark() -> Result<(), CommandError> {
        let table = Arc::new(JobTable::default());
        let slow = |p: &JobProgress| {
            while !p.cancelled() {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            Ok(Value::Null)
        };
        let before = table.submit("test.job", "", slow)?;
        let mark = table.watermark();
        let after = table.submit("test.job", "", slow)?;
        assert_eq!(table.cancel_since(mark), vec![after]);
        let _ = table.cancel(&before);
        Ok(())
    }
}
