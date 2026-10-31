//! Work accounts of bounded geometry operations (M2 C2.1 obligation 11, C2.2 readiness item 3): an operation charges
//! units of about one point operation as it goes and is refused with `limit.exceeded` before the step that would pass
//! its budget, so the document worker is never held past the watchdog and nothing is staged.

use crate::path::GeomError;

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
