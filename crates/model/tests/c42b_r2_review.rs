//! Incremental staging refusal against the general path.
#![allow(clippy::unwrap_used, missing_docs, reason = "review tests")]
#[test]
fn locally_valid_node_with_duplicate_id_discards_staged_drawing() {
    use tt_model::{change::PatchOp, document::EntryKey};
    let d = tt_model::fixture::sample_document(1).unwrap();
    let drawing = d.drawings().find(|d| d.name() == "1").unwrap();
    let key = EntryKey::Drawing(drawing.id().clone());
    let before = d.entry_raw_json(&key).unwrap();
    let node = before["nodes"][0].clone();
    let ops = vec![PatchOp {
        op: "add",
        path: "/nodes/-".into(),
        from: None,
        range: None,
        value: Some(node),
        old_value: None,
        elided: false,
    }];
    let fast = d.with_patched_entries_undo_only(&[(key.clone(), ops.clone())]).unwrap_err();
    let slow = d.with_patched_entries(&[(key.clone(), ops)]).unwrap_err();
    assert_eq!(serde_json::to_value(fast).unwrap(), serde_json::to_value(slow).unwrap());
    assert_eq!(d.entry_raw_json(&key).unwrap(), before);
}
