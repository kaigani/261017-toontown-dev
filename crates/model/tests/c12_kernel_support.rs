//! Model primitives the C1.2 executor commits through: revision stamping (10 REV-01…REV-03), single-object own
//! views and entry pointers (touched-object change sets), and atomic multi-entry staged commits.
#![allow(clippy::unwrap_used, clippy::panic, reason = "test")]

use serde_json::{Value, json};
use tt_model::change::{self, PatchOp};
use tt_model::document::{Document, EntryKey, Severity};
use tt_model::{ObjectType, fixture};

fn op(kind: &'static str, path: &str, value: Option<Value>) -> PatchOp {
    PatchOp { op: kind, path: path.into(), from: None, value, old_value: None, range: None, elided: false }
}

fn errors(doc: &Document) -> usize {
    doc.issues().iter().filter(|i| i.severity == Severity::Error).count()
}

/// The first drawing that has a stroke, with the stroke's index key.
fn stroke_key(doc: &Document) -> (String, String) {
    for d in doc.drawings() {
        let entry = doc.entry_raw_json(&EntryKey::Drawing(d.id().clone())).unwrap();
        for n in entry["nodes"].as_array().unwrap() {
            if let Some(s) = n.get("strokes").and_then(Value::as_array).and_then(|s| s.first()) {
                return (d.id().as_str().to_owned(), format!("{}/{}", d.id(), s["id"].as_str().unwrap()));
            }
        }
    }
    panic!("fixture has no stroke")
}

#[test]
fn stamping_sets_document_and_object_revisions_and_the_drawing_rule() {
    let doc = fixture::sample_document(3).unwrap();
    let (drawing, stroke) = stroke_key(&doc);
    let pen = doc.project().pens()[0].id().as_str().to_owned();
    let rev = doc.document_rev() + 1;
    let next = doc.with_revisions(rev, &[stroke.clone(), pen.clone()]).unwrap();
    assert_eq!(next.document_rev(), rev);
    assert_eq!(next.locate(&stroke).unwrap().rev, rev);
    assert_eq!(next.locate(&drawing).unwrap().rev, rev, "REV-02: a stroke edit changes the drawing's rev");
    assert_eq!(next.locate(&pen).unwrap().rev, rev);
    // Everything else keeps its revision; the project entry holding the pen is restamped only for the pen.
    let project = doc.project().id().as_str();
    assert_eq!(next.locate(project).unwrap().rev, doc.locate(project).unwrap().rev);
    assert_eq!(errors(&next), 0);
    // Revisions never decrease; unknown keys are refused.
    assert!(doc.with_revisions(doc.document_rev(), &[]).is_err());
    assert_eq!(doc.with_revisions(rev, &["nope".into()]).unwrap_err().code(), "ref.notFound");
    // Untouched entries stay shared (copy-on-write).
    for key in doc.entry_keys() {
        if !matches!(&key, EntryKey::Project) && !matches!(&key, EntryKey::Drawing(d) if d.as_str() == drawing) {
            assert!(doc.same_entry(&next, &key), "{key:?}");
        }
    }
}

#[test]
fn own_views_and_pointers_of_nested_objects() {
    let doc = fixture::sample_document(3).unwrap();
    let (drawing, stroke) = stroke_key(&doc);
    let entry = doc.entry_raw_json(&EntryKey::Drawing(tt_model::Id::parse(&drawing).unwrap())).unwrap();
    let at = change::object_pointer(&entry, &stroke).unwrap();
    assert!(at.starts_with("/nodes/") && at.contains("/strokes/"), "{at}");
    let (ty, own) = change::own_view(&doc, &stroke).unwrap();
    assert_eq!(ty, ObjectType::Stroke);
    assert!(own.get("rev").is_none());
    let (ty, own) = change::own_view(&doc, &drawing).unwrap();
    assert_eq!(ty, ObjectType::Drawing);
    for n in own["nodes"].as_array().unwrap() {
        assert_eq!(n.as_object().unwrap().len(), 2, "children are {{id, type}} stubs: {n}");
    }
    assert_eq!(change::own_view(&doc, "missing").unwrap_err().code(), "ref.notFound");
}

#[test]
fn multi_entry_commits_are_atomic_and_carry_verified_undo_and_redo() {
    let doc = fixture::sample_document(3).unwrap();
    let project = EntryKey::Project;
    let library = EntryKey::Library;
    let ok_project = vec![op("add", "/meta", Some(json!({"c12": "project"})))];
    let ok_library = vec![op("add", "/meta", Some(json!({"c12": "library"})))];
    let (next, commits) =
        doc.with_patched_entries(&[(project.clone(), ok_project.clone()), (library.clone(), ok_library)]).unwrap();
    assert_eq!(commits.len(), 2);
    for c in &commits {
        let before = doc.entry_raw_json(&c.key).unwrap();
        let after = next.entry_raw_json(&c.key).unwrap();
        assert_eq!(change::apply(&after, &c.undo).unwrap(), before);
        assert_eq!(change::apply(&before, &c.redo).unwrap(), after);
    }
    // One invalid entry refuses the whole commit; the input is untouched.
    let bad_library = vec![op("replace", "/kind", Some(json!("global")))];
    let before = doc.entry_raw_json(&project).unwrap();
    assert!(doc.with_patched_entries(&[(project.clone(), ok_project), (library, bad_library)]).is_err());
    assert_eq!(doc.entry_raw_json(&project).unwrap(), before);
}

#[test]
fn validated_entry_cache_never_hides_object_issues() {
    // A document assembled without validation reports object-local issues; one built by from_parts has none.
    let mut parts = fixture::sample_parts(3).unwrap();
    let mut pen = tt_model::object::to_json(&parts.project).unwrap();
    pen["pens"][0]["minSize"] = json!(60);
    pen["pens"][0]["maxSize"] = json!(10);
    parts.project = serde_json::from_value(pen).unwrap();
    let assembled = Document::assemble(parts.clone());
    assert!(errors(&assembled) > 0, "minSize > maxSize is an object-local error");
    assert!(Document::from_parts(parts).is_err());
    let valid = fixture::sample_document(3).unwrap();
    assert_eq!(errors(&valid), 0);
}

#[test]
fn typed_object_pointers_agree_with_the_entry_json() {
    let doc = fixture::sample_document(1).unwrap();
    let mut n = 0;
    for loc in doc.locations() {
        let (entry, ty, at) = doc.object_pointer(&loc.key).unwrap();
        assert_eq!((entry.clone(), ty), (loc.entry.clone(), loc.ty), "{}", loc.key);
        let json = doc.entry_raw_json(&entry).unwrap();
        assert_eq!(change::object_pointer(&json, &loc.key).as_deref(), Some(at.as_str()), "{}", loc.key);
        assert_eq!(json.pointer(&at), Some(&doc.object_raw_json(&loc.key).unwrap().1), "{}", loc.key);
        n += 1;
    }
    assert_eq!(n, doc.object_count());
    assert_eq!(doc.object_pointer("nope").unwrap_err().code(), "ref.notFound");
}

/// Inserting or removing one item of a list of full inline objects is one operation in the verified undo/redo, not a
/// copy of the list; reorders still replace the list; every patch is verified against the canonical snapshots.
#[test]
fn list_insertions_and_removals_diff_by_id() {
    let a = json!({ "items": [{ "id": "a", "v": 1 }, { "id": "b", "v": 2 }, { "id": "c", "v": 3 }] });
    let b = json!({ "items": [{ "id": "a", "v": 1 }, { "id": "x", "v": 9 }, { "id": "c", "v": 4 }, { "id": "y", "v": 8 }] });
    let p = change::diff_json(&a, &b, "", ObjectType::Drawing);
    let ops: Vec<(&str, &str)> = p.iter().map(|o| (o.op, o.path.as_str())).collect();
    assert_eq!(ops, [("remove", "/items/1"), ("replace", "/items/1/v"), ("add", "/items/1"), ("add", "/items/3")]);
    assert_eq!(change::apply(&a, &p).unwrap(), b);
    let back = change::diff_json(&b, &a, "", ObjectType::Drawing);
    assert_eq!(change::apply(&b, &back).unwrap(), a);
    // Reordered survivors: replaced whole.
    let r = json!({ "items": [{ "id": "c", "v": 3 }, { "id": "a", "v": 1 }] });
    let p = change::diff_json(&a, &r, "", ObjectType::Drawing);
    assert_eq!((p.len(), p[0].op, p[0].path.as_str()), (1, "replace", "/items"));
    // Through the staged commit: adding one stroke to a layer gives a one-operation undo and redo.
    let doc = fixture::sample_document(1).unwrap();
    let (drawing, key) = stroke_key(&doc);
    let (entry, _, at) = doc.object_pointer(&key).unwrap();
    let (_, mut s) = doc.object_raw_json(&key).unwrap();
    s["id"] = json!("st_c12_extra");
    let list = at.rsplit_once('/').unwrap().0;
    let (next, commits) = doc.with_patched_entries(&[(entry, vec![op("add", &format!("{list}/-"), Some(s))])]).unwrap();
    assert!(next.locate(&format!("{drawing}/st_c12_extra")).is_some());
    assert_eq!((commits[0].undo.len(), commits[0].undo[0].op), (1, "remove"));
    assert_eq!((commits[0].redo.len(), commits[0].redo[0].op), (1, "add"));
}
