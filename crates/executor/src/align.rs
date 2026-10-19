//! Verified replay of history patches (ADR-0013 §4).
//!
//! An undo step stores the canonical undo and redo patches `Document::with_patched_entries` returned when the step
//! committed (C11-R6-01). Every operation carries `oldValue`, but the model checks it only where an `add` overwrites a
//! member; `replace` and `remove` apply to whatever is there. A replay therefore first checks every precondition
//! itself: the value at each `replace`/`remove` path must equal the stored `oldValue` **modulo the `rev` fields of
//! model objects** (inside values, and an operation on a model object's `rev` member itself) (revisions are stamped after each commit and again by every undo and redo, REV-03, so a stored
//! value may name an older `rev`). A difference in content is `history.diverged` — the live document is not the
//! state the step was recorded against — and nothing is applied. Matching preconditions are rewritten to the live
//! value, so the model's own checks hold too; the values the patch writes are unchanged (restored objects carry their
//! old revisions until the replay stamps them).
//!
//! Cost: the entry's JSON is read once per replayed entry. Operations are checked against it directly unless the patch
//! inserts into or removes from an array (later indices shift) or addresses one value twice (an atomic batch's steps
//! on one property), in which case each operation is applied to a working copy before the next is checked.

use serde_json::{Value, json};
use tt_model::ObjectType;
use tt_model::change::{self, PatchOp};
use tt_model::document::{Document, EntryKey};
use tt_registry::CommandError;

/// A model object value (`type` names an object type) carrying a `rev`.
fn is_object_with_rev(m: &serde_json::Map<String, Value>) -> bool {
    m.contains_key("rev") && m.get("type").and_then(Value::as_str).and_then(ObjectType::parse).is_some()
}

/// The value with the `rev` of every model object removed.
fn without_revs(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            let strip = is_object_with_rev(m);
            Value::Object(
                m.iter()
                    .filter(|(k, _)| !(strip && k.as_str() == "rev"))
                    .map(|(k, v)| (k.clone(), without_revs(v)))
                    .collect(),
            )
        }
        Value::Array(a) => Value::Array(a.iter().map(without_revs).collect()),
        other => other.clone(),
    }
}

/// Whether an operation inserts into or removes from an array (later array indices shift).
fn shifts(op: &PatchOp) -> bool {
    matches!(op.op, "add" | "remove" | "move" | "copy")
        && op.path.rsplit_once('/').is_some_and(|(_, last)| last == "-" || last.parse::<usize>().is_ok())
}

/// Whether two operations address the same value or one inside the other (a later precondition then depends on an
/// earlier operation: e.g. two batch steps replacing the same property, undone in reverse order).
fn overlapping(patch: &[PatchOp]) -> bool {
    let mut paths: Vec<&str> = patch.iter().map(|o| o.path.as_str()).collect();
    paths.sort_unstable();
    paths.windows(2).any(|w| w[0] == w[1] || w[1].starts_with(&format!("{}/", w[0])) || w[0].is_empty())
}

fn diverged(entry: &EntryKey, op: &PatchOp, reason: &str) -> CommandError {
    CommandError::with_details(
        "history.diverged",
        format!("the document is not in the state this history step was recorded against: {reason} at `{}`", op.path),
        json!({ "entry": format!("{entry:?}"), "pointer": op.path, "op": op.op }),
    )
}

fn checked(op: &PatchOp) -> bool {
    op.old_value.is_some() && op.range.is_none() && matches!(op.op, "replace" | "remove" | "add")
}

/// The patch with every `oldValue` precondition checked against the live entry modulo revisions and rewritten to the
/// live value.
///
/// # Errors
/// `history.diverged` when a precondition differs in content or names a missing value; `ref.notFound` for a missing
/// entry; a patch error if an operation does not apply.
pub(crate) fn prepare(doc: &Document, entry: &EntryKey, patch: &[PatchOp]) -> Result<Vec<PatchOp>, CommandError> {
    if !patch.iter().any(checked) {
        return Ok(patch.to_vec());
    }
    let progressive = patch.iter().any(shifts) || overlapping(patch);
    let mut cur = doc.entry_raw_json(entry)?;
    let mut out = Vec::with_capacity(patch.len());
    for op in patch {
        let mut op = op.clone();
        if checked(&op) {
            let old = op.old_value.as_ref().map(without_revs);
            // The `rev` member of a model object is revision bookkeeping, not content.
            let rev_member = op.path.rsplit_once('/').is_some_and(|(parent, last)| {
                last == "rev" && cur.pointer(parent).and_then(Value::as_object).is_some_and(is_object_with_rev)
            });
            match cur.pointer(&op.path) {
                Some(live) if rev_member && live.is_u64() && old.as_ref().is_some_and(Value::is_u64) => {
                    op.old_value = Some(live.clone());
                }
                Some(live) if Some(without_revs(live)) == old => op.old_value = Some(live.clone()),
                Some(_) => return Err(diverged(entry, &op, "the value differs")),
                // An `add` whose `oldValue` names nothing restores a member that does not exist yet: the model treats
                // it as an insertion of `value`, and there is nothing to check.
                None if op.op == "add" => op.old_value = None,
                None => return Err(diverged(entry, &op, "nothing is there")),
            }
        }
        if progressive {
            cur = change::apply(&cur, std::slice::from_ref(&op))?;
        }
        out.push(op);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test")]

    use serde_json::json;

    use super::*;

    fn op(kind: &'static str, path: &str, value: Option<Value>, old: Option<Value>) -> PatchOp {
        PatchOp { op: kind, from: None, path: path.into(), range: None, value, old_value: old, elided: false }
    }

    fn scene() -> (Document, EntryKey, Value) {
        let d = tt_model::fixture::sample_document(1).unwrap();
        let key = EntryKey::Scene(d.scenes().next().unwrap().id().clone());
        let live = d.entry_raw_json(&key).unwrap();
        (d, key, live)
    }

    #[test]
    fn preconditions_hold_modulo_rev_and_are_rewritten() {
        let (d, key, live) = scene();
        let mut stale = live["outTransition"].clone();
        stale["rev"] = json!(0);
        let mut target = live["outTransition"].clone();
        target["meta"] = json!({ "restored": true });
        let patch = vec![op("replace", "/outTransition", Some(target), Some(stale))];
        let ready = prepare(&d, &key, &patch).unwrap();
        assert_eq!(ready[0].old_value.as_ref(), Some(&live["outTransition"]));
        assert!(d.with_patched_entries(&[(key, ready)]).is_ok());
    }

    #[test]
    fn content_differences_are_divergence() {
        let (d, key, live) = scene();
        let mut wrong = live["outTransition"].clone();
        wrong["meta"] = json!({ "other": 1 });
        let e = prepare(&d, &key, &[op("replace", "/outTransition", Some(live["outTransition"].clone()), Some(wrong))])
            .unwrap_err();
        assert_eq!(e.code, "history.diverged");
        let e = prepare(&d, &key, &[op("remove", "/meta/nothing", None, Some(json!(1)))]).unwrap_err();
        assert_eq!(e.code, "history.diverged");
    }

    #[test]
    fn shifting_patches_are_checked_in_order() {
        let (d, key, live) = scene();
        let name = live["name"].clone();
        let patch = vec![
            op("add", "/meta", Some(json!({ "list": [1, 2, 3] })), None),
            op("remove", "/meta/list/0", None, Some(json!(1))),
            op("remove", "/meta/list/0", None, Some(json!(2))),
            op("replace", "/name", Some(name.clone()), Some(name)),
        ];
        assert!(prepare(&d, &key, &patch).is_ok());
        let mut bad = patch;
        bad[2].old_value = Some(json!(1));
        assert_eq!(prepare(&d, &key, &bad).unwrap_err().code, "history.diverged");
    }

    #[test]
    fn patches_without_preconditions_are_returned_unchanged() {
        let (d, key, _) = scene();
        let patch = vec![op("add", "/meta", Some(json!({})), None)];
        assert_eq!(prepare(&d, &key, &patch).unwrap(), patch);
    }
}
