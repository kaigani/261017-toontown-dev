//! Replaying history patches modulo `rev` (ADR-0013 §4).
//!
//! An undo step stores the canonical undo and redo patches `Document::with_patched_entries` returned when the step
//! committed (C11-R6-01). Those patches describe content; revisions are stamped afterwards, and every later
//! transaction — undo and redo included (REV-03) — stamps the objects it touches again. When a stored patch is
//! replayed, the `oldValue` preconditions it carries may therefore name an older `rev` inside an object value (a
//! removed stroke, a replaced node) or at a `…/rev` path, although the content is exactly what the patch expects.
//!
//! [`align`] rewrites only those preconditions: an `oldValue` that equals the live value **modulo the `rev` fields of
//! model objects** is replaced by the live value; every other precondition is kept and still checked strictly by the
//! model. The values the patch writes are not changed (restored objects carry their old revisions until the
//! transaction stamps them with its own). Patches whose preconditions mention no revision — every patch of a
//! property edit, which is all C1.2 produces — are returned unchanged without reading the entry.

use serde_json::Value;
use tt_model::ModelError;
use tt_model::ObjectType;
use tt_model::change::{self, PatchOp};
use tt_model::document::{Document, EntryKey};

/// A model object value (`type` names an object type) carrying a `rev`.
fn is_object_with_rev(m: &serde_json::Map<String, Value>) -> bool {
    m.contains_key("rev") && m.get("type").and_then(Value::as_str).and_then(ObjectType::parse).is_some()
}

fn mentions_rev(v: &Value) -> bool {
    match v {
        Value::Object(m) => is_object_with_rev(m) || m.values().any(mentions_rev),
        Value::Array(a) => a.iter().any(mentions_rev),
        _ => false,
    }
}

fn needs_alignment(op: &PatchOp) -> bool {
    op.path.ends_with("/rev") || op.old_value.as_ref().is_some_and(mentions_rev)
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

/// The patch with its `oldValue` preconditions aligned to the live entry modulo revisions.
///
/// # Errors
/// `ref.notFound` for a missing entry; a patch error if an operation does not apply (the model then reports the
/// same error when the aligned patch is committed).
pub(crate) fn align(doc: &Document, entry: &EntryKey, patch: &[PatchOp]) -> Result<Vec<PatchOp>, ModelError> {
    if !patch.iter().any(needs_alignment) {
        return Ok(patch.to_vec());
    }
    let mut cur = doc.entry_raw_json(entry)?;
    let mut out = Vec::with_capacity(patch.len());
    for op in patch {
        let mut op = op.clone();
        if needs_alignment(&op)
            && let (Some(old), Some(live)) = (op.old_value.as_ref(), cur.pointer(&op.path))
            && old != live
        {
            let rev_only = if op.path.ends_with("/rev") {
                old.is_u64() && live.is_u64()
            } else {
                without_revs(old) == without_revs(live)
            };
            if rev_only {
                op.old_value = Some(live.clone());
            }
        }
        cur = change::apply(&cur, std::slice::from_ref(&op))?;
        out.push(op);
    }
    Ok(out)
}
