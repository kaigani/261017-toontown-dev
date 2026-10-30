// Snapshots handed out are immutable: no field of a document is writable outside the model.
fn main() {
    let mut doc = tt_model::fixture::sample_document(1).unwrap();
    doc.document_rev = 99;
}
