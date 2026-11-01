//! Work accounts of bounded geometry operations (M2 C2.1 obligation 11, C2.2 readiness item 3): an operation charges
//! units of about one point operation as it goes and is refused with `limit.exceeded` before the step that would pass
//! its budget, so the document worker is never held past the watchdog and nothing is staged.

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::path::GeomError;

/// The watchdog's cancellation state of a worker thread (ADR-0015 history, C23-R1-07): the sequence number of the
/// request the worker is running and the one a timed-out caller asked to cancel. Every work account checks it at each
/// charge, so an overrunning operation stops at its next budget check and the worker answers the next request.
#[derive(Debug, Default)]
pub struct CancelState {
    /// The request now running (0: none, or not cancellable).
    pub current: AtomicU64,
    /// The running request a timed-out caller cancelled (0: none).
    pub cancel: AtomicU64,
    /// Request tokens handed out before enqueueing (C23-R2-01).
    pub next: AtomicU64,
    /// Whether `queued` holds any token (the fast path of every work charge).
    pending: std::sync::atomic::AtomicBool,
    /// Tokens whose callers timed out, still queued or running.
    queued: std::sync::Mutex<std::collections::BTreeSet<u64>>,
}

impl CancelState {
    /// A fresh request token (never 0).
    pub fn token(&self) -> u64 {
        self.next.fetch_add(1, Ordering::AcqRel).wrapping_add(1).max(1)
    }

    /// Marks exactly `token` cancelled: its caller has been answered. Another caller's request is never affected.
    pub fn mark(&self, token: u64) {
        if let Ok(mut q) = self.queued.lock() {
            q.insert(token);
            self.pending.store(true, Ordering::Release);
        }
        self.cancel.store(token, Ordering::Release);
    }

    /// Whether `token` was cancelled.
    pub fn is_cancelled(&self, token: u64) -> bool {
        token != 0
            && (self.cancel.load(Ordering::Acquire) == token
                || (self.pending.load(Ordering::Acquire) && self.queued.lock().is_ok_and(|q| q.contains(&token))))
    }

    /// Forgets `token` once the worker is done with it.
    pub fn forget(&self, token: u64) {
        if let Ok(mut q) = self.queued.lock() {
            q.remove(&token);
            self.pending.store(!q.is_empty(), Ordering::Release);
        }
    }
}

thread_local! {
    static CANCEL: RefCell<Option<Arc<CancelState>>> = const { RefCell::new(None) };
}

/// Installs the cancellation state of the calling (worker) thread.
pub fn set_cancel_state(state: Option<Arc<CancelState>>) {
    CANCEL.with(|c| *c.borrow_mut() = state);
}

/// Whether the request the calling thread runs has been cancelled by the watchdog.
#[must_use]
pub fn cancel_requested() -> bool {
    CANCEL.with(|c| c.borrow().as_ref().is_some_and(|s| s.is_cancelled(s.current.load(Ordering::Acquire))))
}

/// Test hooks (never in products): make every work charge spin, so any work-charging command overruns on demand.
#[cfg(feature = "test-hooks")]
pub mod hooks {
    use std::sync::atomic::{AtomicU64, Ordering};

    static SPIN_MICROS: AtomicU64 = AtomicU64::new(0);

    /// Each later charge spins this long (0: off).
    pub fn set_spin_micros(us: u64) {
        SPIN_MICROS.store(us, Ordering::Release);
    }

    pub(crate) fn spin() {
        let us = SPIN_MICROS.load(Ordering::Acquire);
        if us > 0 {
            let start = std::time::Instant::now();
            while start.elapsed() < std::time::Duration::from_micros(us) {
                std::hint::spin_loop();
            }
        }
    }
}

/// The work one planar-map update may do (persistent noding, the derived graph, fill carry-over and gap closing), in
/// units of about one point operation. The adversarial planar corpus of `crates/vector/tests/c22_worst_case.rs`
/// records what the accepted worst cases use and how long they take.
pub const PLANAR_WORK_BUDGET: u64 = 1_200_000_000;

/// A work account.
#[derive(Debug, Clone)]
pub struct Work {
    used: u64,
    limit: u64,
    what: &'static str,
}

impl Work {
    /// An account of `limit` units for the operation `what` (named in the refusal).
    #[must_use]
    pub fn new(limit: u64, what: &'static str) -> Self {
        Self { used: 0, limit, what }
    }

    /// An unlimited account (library callers that bound their input themselves).
    #[must_use]
    pub fn unlimited(what: &'static str) -> Self {
        Self::new(u64::MAX, what)
    }

    /// Charges `n` units.
    ///
    /// # Errors
    /// `limit.exceeded` once the budget is passed.
    pub fn spend(&mut self, n: u64) -> Result<(), GeomError> {
        self.used = self.used.saturating_add(n);
        if self.used > self.limit {
            return Err(self.exceeded());
        }
        #[cfg(feature = "test-hooks")]
        hooks::spin();
        if cancel_requested() {
            return Err(GeomError {
                code: "command.timeout",
                message: format!("{} was cancelled: the call overran its watchdog budget", self.what),
                pointer: String::new(),
            });
        }
        Ok(())
    }

    /// Charges `n` units counted as `usize`.
    ///
    /// # Errors
    /// As [`Work::spend`].
    pub fn spend_n(&mut self, n: usize) -> Result<(), GeomError> {
        self.spend(u64::try_from(n).unwrap_or(u64::MAX))
    }

    /// The units used so far.
    #[must_use]
    pub fn used(&self) -> u64 {
        self.used
    }

    /// The refusal.
    #[must_use]
    pub fn exceeded(&self) -> GeomError {
        GeomError {
            code: "limit.exceeded",
            message: format!("{} needs more than {} units of work", self.what, self.limit),
            pointer: "/points".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mark_cancels_exactly_its_own_token() {
        let s = CancelState::default();
        let (a, b) = (s.token(), s.token());
        assert_ne!(a, b);
        s.mark(a);
        assert!(s.is_cancelled(a) && !s.is_cancelled(b));
        s.forget(a);
        // The atomic slot still names `a` (it never matches another token); the queued set is empty.
        assert!(!s.is_cancelled(b) && !s.is_cancelled(0));
    }
}
