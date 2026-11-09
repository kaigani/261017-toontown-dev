//! Independent query-purity and per-transaction event boundaries.
mod common;
use common::*;
use serde_json::json;
#[test]
fn query_purity_and_transaction_event_boundaries() {
    let h = fixture();
    let bob = "scene:\"Intro\"/element:\"Bob\"";
    let before = h.snapshot().unwrap().unwrap();
    let app = ok(&h, "app.state.get", json!({}));
    let history = ok(&h, "edit.history.get", json!({}));
    let rx = h.subscribe();
    for frame in [1., 1.5, 12., 12.25, 24.] {
        let a = ok(&h, "anim.evaluate", json!({"nodeIds":[bob],"frames":[frame]}));
        let b = ok(&h, "anim.evaluate", json!({"nodeIds":[bob],"frames":[frame]}));
        assert_eq!(a, b);
    }
    assert_eq!(before.document_rev(), rev(&h));
    assert_eq!(app, ok(&h, "app.state.get", json!({})));
    assert_eq!(history, ok(&h, "edit.history.get", json!({})));
    assert!(rx.try_iter().next().is_none());
    for rotation in [13, 27, 41] {
        let rx = h.subscribe();
        let r = rev(&h);
        ok(&h, "anim.transform.set", json!({"nodeIds":[bob],"target":"static","rotation":rotation}));
        assert_eq!(rev(&h), r + 1);
        let events: Vec<_> = rx.try_iter().filter(|e| e.name == "anim.changed").collect();
        assert_eq!(events.len(), 1);
    }
    let after = get(&h, bob);
    ok(&h, "edit.undo", json!({}));
    ok(&h, "edit.redo", json!({}));
    let mut restored = get(&h, bob);
    let mut want = after;
    restored.as_object_mut().unwrap().remove("rev");
    want.as_object_mut().unwrap().remove("rev");
    assert_eq!(restored, want);
}
