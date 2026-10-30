// What other crates can do: run commands through the executor and read immutable snapshots.
fn main() {
    let h = tt_executor::ExecutorHandle::spawn(tt_executor::ExecutorConfig::default()).unwrap();
    let doc = tt_model::fixture::sample_document(1).unwrap();
    assert!(h.open_document(doc, None, false).is_ok());
    let snap = h.snapshot().unwrap().unwrap();
    let rev = snap.document_rev();
    let scene = snap.scenes().next().unwrap().id().as_str().to_owned();
    let env = h.invoke(tt_executor::Invocation::user(
        "doc.object.patch",
        serde_json::json!({ "targets": [scene], "props": { "meta": { "from": "outside" } } }),
    ));
    assert!(env.is_ok());
    assert_eq!(snap.document_rev(), rev, "a snapshot never changes");
    assert_eq!(h.snapshot().unwrap().unwrap().document_rev(), rev + 1);
}
