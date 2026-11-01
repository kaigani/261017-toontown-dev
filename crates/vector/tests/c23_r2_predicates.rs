//! Shared exact predicates: translated/reversed determinant cases and account cancellation.
#![allow(clippy::unwrap_used, missing_docs, reason = "review tests")]
use std::sync::{Arc, atomic::Ordering};
use tt_vector::{
    geom::P,
    ops::cut,
    planar::predicates,
    work::{CancelState, Work, set_cancel_state},
};
fn g(x: i64, y: i64) -> P {
    P::new(x as f64 / 64.0, y as f64 / 64.0)
}
#[test]
fn translated_reversed_unit_determinants_and_shared_edges() {
    let n = 400_000_000;
    for sign in [-1, 1] {
        for offset in [-100_000_000, 100_000_000] {
            let a = [g(offset, offset), g(offset + n, offset + n - 1)];
            let q = g(offset + n - 1, offset + n - 2);
            let o =
                predicates::orient([offset, offset], [offset + n, offset + n - 1], [offset + n - 1, offset + n - 2]);
            assert_eq!(o, -1);
            let tri = if sign == 1 { vec![a[0], a[1], q] } else { vec![q, a[1], a[0]] };
            assert!(cut::polygons_meet(&tri, &tri));
            assert!(!cut::inside(&tri, q));
            assert!(predicates::inside_polygon(&tri, q, true));
            let edge = [a[0], a[1]];
            assert!(cut::polyline_meets(&edge, &tri));
        }
    }
}
#[test]
fn every_work_account_observes_request_cancellation_and_next_request_is_clean() {
    let state = Arc::new(CancelState::default());
    state.current.store(17, Ordering::Release);
    state.cancel.store(17, Ordering::Release);
    set_cancel_state(Some(state.clone()));
    for name in ["planar", "merge", "smooth"] {
        assert_eq!(Work::unlimited(name).spend(1).unwrap_err().code, "command.timeout");
    }
    state.current.store(18, Ordering::Release);
    assert!(Work::unlimited("next").spend(1).is_ok());
    set_cancel_state(None);
}
