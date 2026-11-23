//! Facts prechecks read about large session values, computed once per value (ruling O-24;
//! reviews/M4-design-availability.md §3).
//!
//! A precheck must not walk a selection, the clipboard or an exposure column on every availability pass. The facts it
//! needs (the kinds of the selected elements, whether a copied drawing is still in the project, the cells of a column)
//! are a pure function of shared values (selection lists and the clipboard, all held behind `Arc`), of the document
//! revision and of what the tag names. [`memo_many`] computes that function once and answers later calls with a lookup
//! until a value is replaced or the document changes: the first pass after such a change reads the value once for
//! every command that shares the facts, the passes after it read nothing.
//!
//! Soundness: an entry is keyed by the shared values' addresses and holds weak references to them, so an address
//! cannot be reused by another value while the entry exists; a value edited in place through `Arc::make_mut` while an
//! entry holds a weak reference is moved to a new allocation first. Entries live in a small table per thread
//! (prechecks run on the document thread); they are not session state, so a pass still changes nothing.

use std::any::Any;
use std::cell::RefCell;
use std::sync::{Arc, Weak};

/// Entries kept per thread (the oldest is dropped beyond this).
const CAPACITY: usize = 256;

struct Entry {
    tag: String,
    values: Vec<Weak<dyn Any + Send + Sync>>,
    rev: u64,
    facts: Box<dyn Any>,
}

thread_local! {
    static TABLE: RefCell<Vec<Entry>> = const { RefCell::new(Vec::new()) };
    /// Set while availability evaluates prechecks on the lent, committed session ([`enabled`]). Elsewhere (the
    /// pipeline runs prechecks inside transactions, whose working document changes between the steps of a batch
    /// before its revision does) every call computes its facts directly.
    static ACTIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Runs `f` with memoisation enabled (availability's read-only context on the committed session).
pub(crate) fn enabled<T>(f: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ACTIVE.with(|a| a.set(self.0));
        }
    }
    let _restore = Restore(ACTIVE.with(|a| a.replace(true)));
    f()
}

fn address<T: ?Sized>(p: *const T) -> *const () {
    p.cast::<()>()
}

/// `f()` for the identities of `values` and the document revision `rev`, under `tag` (the kind of fact and anything
/// else it depends on: a context name, the parameters, the current element and frame, the project).
pub(crate) fn memo_many<T: Clone + 'static>(
    tag: &str,
    values: &[Arc<dyn Any + Send + Sync>],
    rev: u64,
    f: impl FnOnce() -> T,
) -> T {
    if !ACTIVE.with(std::cell::Cell::get) {
        return f();
    }
    let hit = TABLE.with(|t| {
        t.borrow().iter().find_map(|e| {
            let same = e.tag == tag
                && e.rev == rev
                && e.values.len() == values.len()
                && e.values.iter().zip(values).all(|(w, v)| std::ptr::eq(address(w.as_ptr()), address(Arc::as_ptr(v))));
            if same { e.facts.downcast_ref::<T>().cloned() } else { None }
        })
    });
    if let Some(v) = hit {
        return v;
    }
    let v = f();
    let weak: Vec<Weak<dyn Any + Send + Sync>> = values.iter().map(Arc::downgrade).collect();
    TABLE.with(|t| {
        let mut t = t.borrow_mut();
        t.retain(|e| e.tag != tag);
        if t.len() >= CAPACITY {
            t.remove(0);
        }
        t.push(Entry { tag: tag.to_owned(), values: weak, rev, facts: Box::new(v.clone()) });
    });
    v
}

/// [`memo_many`] for one shared value.
pub(crate) fn memo<S, T>(tag: &str, shared: &Arc<S>, rev: u64, f: impl FnOnce() -> T) -> T
where
    S: Any + Send + Sync,
    T: Clone + 'static,
{
    let value: Arc<dyn Any + Send + Sync> = shared.clone();
    memo_many(tag, &[value], rev, f)
}

/// [`memo_many`] for the document revision alone (facts read from the document, such as the cells of an exposure
/// column; the tag names the project, the element, the frame and the parameters).
pub(crate) fn memo_rev<T: Clone + 'static>(tag: &str, rev: u64, f: impl FnOnce() -> T) -> T {
    memo_many(tag, &[], rev, f)
}

/// The document's identity and revision, for facts that read the document: the project ID (revisions of different
/// projects can coincide) and `documentRev`. Empty and 0 without a project.
pub(crate) fn document_version(cx: &crate::cx::Cx) -> (String, u64) {
    cx.document().map_or_else(|_| (String::new(), 0), |d| (d.project().id().as_str().to_owned(), d.document_rev()))
}

/// The shared selection lists of `contexts` (an empty or absent list as a fixed marker), for [`memo_many`].
pub(crate) fn selections(cx: &crate::cx::Cx, contexts: &[&str]) -> Vec<Arc<dyn Any + Send + Sync>> {
    thread_local! {
        static NONE: Arc<dyn Any + Send + Sync> = Arc::new(());
    }
    contexts
        .iter()
        .map(|c| match cx.app().selection_shared(c).filter(|l| !l.is_empty()) {
            Some(l) => {
                let v: Arc<dyn Any + Send + Sync> = l.clone();
                v
            }
            None => NONE.with(Arc::clone),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, reason = "test")]

    use super::*;

    /// Ruling E-29 (the handler-panic note §3.2): an entry is written only after its value is complete, so a computation
    /// that panics leaves no entry, and the next call with the same key computes its value afresh (never a wrong one).
    #[test]
    fn a_computation_that_panics_leaves_no_entry() {
        let value: Arc<dyn Any + Send + Sync> = Arc::new(vec![1u8, 2, 3]);
        let values = [value.clone()];
        let entries = || TABLE.with(|t| t.borrow().iter().filter(|e| e.tag == "e29").count());
        enabled(|| {
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                memo_many::<u32>("e29", &values, 7, || panic!("test: a fact computation panics"))
            }));
            assert!(caught.is_err());
            assert_eq!(entries(), 0, "no entry for the panicked computation");
            assert_eq!(memo_many("e29", &values, 7, || 42u32), 42, "computed afresh");
            assert_eq!(entries(), 1);
            assert_eq!(memo_many("e29", &values, 7, || 0u32), 42, "then served from the entry");
        });
    }
}
