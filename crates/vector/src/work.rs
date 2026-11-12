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
    /// Tokens handed out and not yet forgotten: only these can be marked, so a caller that marks its token after the
    /// worker forgot it leaves nothing behind (M3 C3.4 round 2).
    outstanding: std::sync::Mutex<std::collections::BTreeSet<u64>>,
    /// Marked tokens whose caller cancelled its request (the rest of `queued` timed out): the cause a refusal reports
    /// (M3 C3.4 round 3, C34-R3-02).
    by_caller: std::sync::Mutex<std::collections::BTreeSet<u64>>,
}

/// Why a request's token was marked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    /// Its caller's watchdog budget passed.
    Timeout,
    /// Its caller cancelled the request.
    Cancelled,
}

impl CancelState {
    /// A fresh request token (never 0).
    pub fn token(&self) -> u64 {
        let token = self.next.fetch_add(1, Ordering::AcqRel).wrapping_add(1).max(1);
        if let Ok(mut o) = self.outstanding.lock() {
            o.insert(token);
            #[cfg(feature = "test-hooks")]
            hooks::count_outstanding(1);
        }
        token
    }

    /// Marks exactly `token` cancelled: its caller has been answered. Another caller's request is never affected. A
    /// token the worker already forgot (the request finished) is not marked: nothing would ever forget it again.
    pub fn mark(&self, token: u64) {
        self.mark_for(token, Cause::Timeout);
    }

    /// Marks exactly `token` cancelled because its caller cancelled the request ([`CancelState::mark`] otherwise).
    pub fn mark_cancelled(&self, token: u64) {
        self.mark_for(token, Cause::Cancelled);
    }

    /// Why `token` was marked, if it was.
    pub fn cause(&self, token: u64) -> Option<Cause> {
        if !self.is_cancelled(token) {
            return None;
        }
        let by_caller = self.by_caller.lock().is_ok_and(|c| c.contains(&token));
        Some(if by_caller { Cause::Cancelled } else { Cause::Timeout })
    }

    fn mark_for(&self, token: u64, cause: Cause) {
        let Ok(outstanding) = self.outstanding.lock() else { return };
        if !outstanding.contains(&token) {
            return;
        }
        if cause == Cause::Cancelled
            && let Ok(mut c) = self.by_caller.lock()
        {
            c.insert(token);
        }
        if let Ok(mut q) = self.queued.lock()
            && q.insert(token)
        {
            self.pending.store(true, Ordering::Release);
            #[cfg(feature = "test-hooks")]
            hooks::count_queued(1);
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
        // The same lock order as `mark` (outstanding, then queued), so a mark either precedes the forget (and is
        // removed here) or finds the token gone.
        let Ok(mut outstanding) = self.outstanding.lock() else { return };
        if outstanding.remove(&token) {
            #[cfg(feature = "test-hooks")]
            hooks::count_outstanding(-1);
        }
        if let Ok(mut c) = self.by_caller.lock() {
            c.remove(&token);
        }
        if let Ok(mut q) = self.queued.lock() {
            if q.remove(&token) {
                #[cfg(feature = "test-hooks")]
                hooks::count_queued(-1);
            }
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
    cancel_cause().is_some()
}

/// Why the request the calling thread runs was cancelled, if it was (C34-R3-02).
#[must_use]
pub fn cancel_cause() -> Option<Cause> {
    CANCEL.with(|c| c.borrow().as_ref().and_then(|s| s.cause(s.current.load(Ordering::Acquire))))
}

/// The `details.reason` of a refusal for the calling thread's cancellation: `"cancelled"` when its caller cancelled
/// the request (SI-111), none for a watchdog timeout.
#[must_use]
pub fn cancel_reason() -> Option<&'static str> {
    (cancel_cause() == Some(Cause::Cancelled)).then_some("cancelled")
}

/// Counts one question a worker asks its caller (a no-op unless `test-hooks`; C34-R3-01).
#[inline]
pub fn note_ask() {
    #[cfg(feature = "test-hooks")]
    hooks::count_ask();
}

/// Test hooks (never in products): make every work charge spin, so any work-charging command overruns on demand.
#[cfg(feature = "test-hooks")]
pub mod hooks {
    use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

    static QUEUED: AtomicI64 = AtomicI64::new(0);
    static OUTSTANDING: AtomicI64 = AtomicI64::new(0);

    pub(super) fn count_queued(delta: i64) {
        QUEUED.fetch_add(delta, Ordering::AcqRel);
    }

    pub(super) fn count_outstanding(delta: i64) {
        OUTSTANDING.fetch_add(delta, Ordering::AcqRel);
    }

    /// Marked (cancelled) tokens not yet forgotten, over every [`super::CancelState`] of the process.
    #[must_use]
    pub fn queued_tokens() -> i64 {
        QUEUED.load(Ordering::Acquire)
    }

    /// Tokens handed out and not yet forgotten, over every [`super::CancelState`] of the process.
    #[must_use]
    pub fn outstanding_tokens() -> i64 {
        OUTSTANDING.load(Ordering::Acquire)
    }

    static ASKS: AtomicI64 = AtomicI64::new(0);

    pub(super) fn count_ask() {
        ASKS.fetch_add(1, Ordering::AcqRel);
    }

    /// Questions workers asked their callers since the last [`reset_asks`] (C34-R3-01).
    #[must_use]
    pub fn asks() -> i64 {
        ASKS.load(Ordering::Acquire)
    }

    /// Resets [`asks`].
    pub fn reset_asks() {
        ASKS.store(0, Ordering::Release);
    }

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
