// Entities expose read accessors only.
fn main() {
    let doc = tt_model::fixture::sample_document(1).unwrap();
    let mut scene = doc.scenes().next().unwrap().clone();
    scene.name = String::from("renamed");
}
