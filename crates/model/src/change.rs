//! Change descriptions (10 §2.5–§2.6, §10.4; 11 §7.2, §9.2).
//!
//! A change set lists, per addressable object, `created` (with `parentId`, `index`), `deleted` (with `parentId`) or
//! `modified` with an RFC 6902 patch against the object's **own view**: its JSON with `rev` removed and each child
//! object reduced to `{id, type}`. A child's property change therefore never changes its parent (REV-02), while
//! membership and order of a child collection do. Every operation carries `oldValue`, and a patch is inverted
//! against the state it applies to ([`invert_against`], [`apply_with_inverse`]; there is no stateless inversion,
//! C11-R3-02); exposure and mouth-track edits use the `range` form so long scenes give small patches.
//!
//! **Range operations.** `{op: "replace", path, range: {from, to}, value, oldValue}` on an element's `/exposure`
//! (runs) or a sound placement's `/mouthTrack/mouths` (one letter per frame) — only these declared fields, never a
//! lookalike inside opaque `meta`/`ext` data, which is replaced as an ordinary value (C11-R3-01): frames `from…` covered by `oldValue` are replaced by `value`
//! (frame `from` is the first changed frame; `to = from + max(|old|, |new|) − 1`). Frames past the end of an
//! exposure are blank, so both windows have equal length there.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::document::{Document, EntryKey};
use crate::entities::element::ExposureRun;
use crate::error::{ModelError, codes};
use crate::pointer;
use crate::types::ObjectType;
use crate::value::FrameRange;

/// Values larger than this are elided in events and `doc.diff` (10 §2.6).
pub const ELIDE_BYTES: usize = 64 * 1024;

/// Object-level change kind (10 §2.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    /// The object was created.
    Created,
    /// The object's own view changed.
    Modified,
    /// The object was deleted.
    Deleted,
}

/// One RFC 6902 operation, extended with `oldValue`, `range` and `elided` (10 §2.6).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchOp {
    /// `add`, `remove`, `replace`, `move`, `copy` or `test`.
    pub op: &'static str,
    /// Source pointer of `move`/`copy`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// Target pointer.
    pub path: String,
    /// Frame window of a range replace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<FrameRange>,
    /// New value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    /// Previous value (what makes the patch invertible).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_value: Option<Value>,
    /// `true` when `value`/`oldValue` were omitted for size (events and `doc.diff` only).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub elided: bool,
}

impl PatchOp {
    fn add(path: String, value: Value) -> Self {
        Self { op: "add", from: None, path, range: None, value: Some(value), old_value: None, elided: false }
    }

    fn remove(path: String, old: Value) -> Self {
        Self { op: "remove", from: None, path, range: None, value: None, old_value: Some(old), elided: false }
    }

    fn replace(path: String, old: Value, value: Value) -> Self {
        Self { op: "replace", from: None, path, range: None, value: Some(value), old_value: Some(old), elided: false }
    }

    fn mv(from: String, path: String) -> Self {
        Self { op: "move", from: Some(from), path, range: None, value: None, old_value: None, elided: false }
    }
}

/// One object's change (10 §10.4 `Change`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    /// Object ID (compound `<containerId>/<localId>` for container-local IDs).
    pub id: String,
    /// Object type.
    #[serde(rename = "type")]
    pub ty: ObjectType,
    /// Created, modified or deleted.
    pub op: ChangeKind,
    /// The object's `rev` after the transaction (the transaction's `documentRev`).
    pub rev: u64,
    /// Parent (created and deleted objects).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// Position in the parent's collection (created objects).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
    /// Own-view patch (modified objects).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch: Option<Vec<PatchOp>>,
}

/// A committed transaction's change set (10 §2.6).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeSet {
    /// Undo step ID.
    pub undo_step_id: String,
    /// `documentRev` after the transaction.
    pub document_rev: u64,
    /// Undo label.
    pub label: String,
    /// `user`, `agent:<client>`, `system` or `load`.
    pub origin: String,
    /// Commit time.
    pub time: crate::value::Timestamp,
    /// Object changes.
    pub changes: Vec<Change>,
}

/// An addressable object's own view, with where it sits.
#[derive(Debug, Clone, PartialEq)]
pub struct OwnObject {
    /// Index key.
    pub key: String,
    /// Type.
    pub ty: ObjectType,
    /// Parent index key.
    pub parent: Option<String>,
    /// Position in the parent's collection.
    pub index: usize,
    /// Own view JSON.
    pub json: Value,
}

fn child_stub(v: &Value, ty: ObjectType) -> Value {
    let mut m = Map::new();
    if let Some(id) = v.get(ty.id_field()) {
        m.insert(ty.id_field().to_owned(), id.clone());
    }
    m.insert("type".into(), json!(ty.as_str()));
    Value::Object(m)
}

fn type_of(v: &Value) -> Option<ObjectType> {
    v.get("type").and_then(Value::as_str).and_then(ObjectType::parse)
}

/// Splits an entry's JSON into the own views of every addressable object it holds. Takes the value by ownership and
/// moves each child out before replacing it with its stub, so nothing is copied (an entry may hold 100 000 strokes).
fn collect(v: Value, ty: ObjectType, parent: Option<&str>, index: usize, ctx: (&str, &str), out: &mut Vec<OwnObject>) {
    let id = v.get(ty.id_field()).and_then(Value::as_str).unwrap_or_default().to_owned();
    let key = match ty.local_id_container() {
        Some(ObjectType::Drawing) if !ctx.0.is_empty() => format!("{}/{id}", ctx.0),
        Some(ObjectType::Channel) if !ctx.1.is_empty() => format!("{}/{id}", ctx.1),
        _ => id.clone(),
    };
    let (drawing, channel) = match ty {
        ObjectType::Drawing => (id.as_str(), ""),
        ObjectType::Channel => (ctx.0, id.as_str()),
        _ => ctx,
    };
    let mut own = v;
    if let Value::Object(m) = &mut own {
        m.shift_remove("rev");
    }
    for (field, slot) in crate::document::slots_of(ty) {
        let Some(child) = own.get_mut(*field) else { continue };
        let inline = !matches!(slot, crate::document::SlotKind::Split(_) | crate::document::SlotKind::SplitOne(_));
        let mut visit = |item: &mut Value, i: usize| {
            if let Some(cty) = type_of(item) {
                let stub = child_stub(item, cty);
                let full = std::mem::replace(item, stub);
                if inline {
                    collect(full, cty, Some(&key), i, (drawing, channel), out);
                }
            }
        };
        match child {
            Value::Array(items) => items.iter_mut().enumerate().for_each(|(i, item)| visit(item, i)),
            Value::Object(m) if matches!(slot, crate::document::SlotKind::InlineMap) => {
                m.values_mut().enumerate().for_each(|(i, item)| visit(item, i));
            }
            item @ Value::Object(_) => visit(item, 0),
            _ => {}
        }
    }
    out.push(OwnObject { key, ty, parent: parent.map(str::to_owned), index, json: own });
}

/// The own views of all objects in one entry of a document.
///
/// # Errors
/// `ref.notFound` for a missing entry; `internal` if serialisation fails.
pub fn own_objects(doc: &Document, entry: &EntryKey) -> Result<Vec<OwnObject>, ModelError> {
    let v = doc.entry_raw_json(entry)?;
    let ty = type_of(&v).ok_or_else(|| ModelError::new(codes::INTERNAL, "entry without type"))?;
    let parent = match entry {
        EntryKey::Project => None,
        EntryKey::Palette(_) | EntryKey::Bank(_) | EntryKey::Scene(_) | EntryKey::Library => {
            Some(doc.project().id().as_str().to_owned())
        }
        EntryKey::Drawing(id) => doc.bank_of(id).map(|b| b.as_str().to_owned()),
        EntryKey::Element(id) => doc.scene_of(id).map(|s| s.as_str().to_owned()),
    };
    let index = match entry {
        EntryKey::Palette(id) => doc.project().palettes().ids().iter().position(|x| x == id),
        EntryKey::Bank(id) => doc.project().drawing_banks().ids().iter().position(|x| x == id),
        EntryKey::Scene(id) => doc.project().scenes().ids().iter().position(|x| x == id),
        EntryKey::Drawing(id) => {
            doc.bank_of(id).and_then(|b| doc.bank(b)).and_then(|b| b.drawings().ids().iter().position(|x| x == id))
        }
        EntryKey::Element(id) => {
            doc.scene_of(id).and_then(|s| doc.scene(s)).and_then(|s| s.elements().ids().iter().position(|x| x == id))
        }
        EntryKey::Project | EntryKey::Library => Some(0),
    }
    .unwrap_or(0);
    let mut out = Vec::new();
    collect(v, ty, parent.as_deref(), index, ("", ""), &mut out);
    Ok(out)
}

/// The object changes between two snapshots (without revisions; see [`Change`] for the stamped form). Entries
/// shared by both snapshots (same `Arc`) are skipped, so the cost is proportional to the changed entries.
///
/// # Errors
/// `internal` if serialisation fails.
pub fn diff(before: &Document, after: &Document) -> Result<Vec<(OwnChange, OwnObject)>, ModelError> {
    let keys_before: Vec<EntryKey> = before.entry_keys();
    let keys_after: Vec<EntryKey> = after.entry_keys();
    let mut changed: Vec<EntryKey> = Vec::new();
    for k in keys_after.iter().chain(keys_before.iter()) {
        if !changed.contains(k) && !before.same_entry(after, k) {
            changed.push(k.clone());
        }
    }
    let mut old: HashMap<String, OwnObject> = HashMap::new();
    let mut old_order: Vec<String> = Vec::new();
    let mut new: Vec<OwnObject> = Vec::new();
    for k in &changed {
        if keys_before.contains(k) {
            for o in own_objects(before, k)? {
                old_order.push(o.key.clone());
                old.insert(o.key.clone(), o);
            }
        }
        if keys_after.contains(k) {
            new.extend(own_objects(after, k)?);
        }
    }
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for o in new {
        seen.insert(o.key.clone());
        match old.get(&o.key) {
            None => out.push((OwnChange::Created, o)),
            Some(prev) if prev.json != o.json || prev.ty != o.ty => {
                let patch = diff_json(&prev.json, &o.json, "", o.ty);
                out.push((OwnChange::Modified(patch), o));
            }
            Some(_) => {}
        }
    }
    for k in old_order {
        if !seen.contains(&k)
            && let Some(o) = old.remove(&k)
        {
            out.push((OwnChange::Deleted, o));
        }
    }
    Ok(out)
}

/// The kind of a computed change, with the patch of a modification.
#[derive(Debug, Clone, PartialEq)]
pub enum OwnChange {
    /// Created.
    Created,
    /// Modified, with the own-view patch.
    Modified(Vec<PatchOp>),
    /// Deleted.
    Deleted,
}

impl OwnChange {
    /// The public change record for a transaction committed at `rev`.
    #[must_use]
    pub fn to_change(&self, o: &OwnObject, rev: u64) -> Change {
        match self {
            Self::Created => Change {
                id: o.key.clone(),
                ty: o.ty,
                op: ChangeKind::Created,
                rev,
                parent_id: o.parent.clone(),
                index: Some(o.index),
                patch: None,
            },
            Self::Deleted => Change {
                id: o.key.clone(),
                ty: o.ty,
                op: ChangeKind::Deleted,
                rev,
                parent_id: o.parent.clone(),
                index: None,
                patch: None,
            },
            Self::Modified(p) => Change {
                id: o.key.clone(),
                ty: o.ty,
                op: ChangeKind::Modified,
                rev,
                parent_id: None,
                index: None,
                patch: Some(p.clone()),
            },
        }
    }
}

/// Whether `path` is a child-list slot of an own view of type `ty` (a top-level `Split`/`Inline` field). Only these
/// arrays are diffed as collections of `{id, type}` stubs; every other array, in particular anything inside the
/// opaque `meta`/`ext` values, is an ordinary value (10 §2.2).
fn is_child_list(ty: ObjectType, path: &str) -> bool {
    path.strip_prefix('/').is_some_and(|field| {
        crate::document::slots_of(ty).iter().any(|(f, slot)| {
            *f == field && matches!(slot, crate::document::SlotKind::Split(_) | crate::document::SlotKind::Inline)
        })
    })
}

fn stub_key(v: &Value) -> String {
    v.get("id").or_else(|| v.get("colourId")).map(Value::to_string).unwrap_or_default()
}

/// Minimal sequence edit between two child-stub lists: removes, then adds and moves (10 §2.6 uses `move` for
/// re-ordering).
fn diff_collection(a: &[Value], b: &[Value], path: &str, out: &mut Vec<PatchOp>) {
    let in_b: std::collections::HashSet<String> = b.iter().map(stub_key).collect();
    let mut cur: Vec<Value> = a.to_vec();
    for i in (0..cur.len()).rev() {
        if cur.get(i).is_some_and(|v| !in_b.contains(&stub_key(v))) {
            let old = cur.remove(i);
            out.push(PatchOp::remove(format!("{path}/{i}"), old));
        }
    }
    for (i, want) in b.iter().enumerate() {
        let k = stub_key(want);
        if cur.get(i).map(stub_key).as_deref() == Some(k.as_str()) {
            continue;
        }
        match cur.iter().position(|v| stub_key(v) == k) {
            Some(j) => {
                let item = cur.remove(j);
                cur.insert(i, item);
                out.push(PatchOp::mv(format!("{path}/{j}"), format!("{path}/{i}")));
            }
            None => {
                cur.insert(i, want.clone());
                out.push(PatchOp::add(format!("{path}/{i}"), want.clone()));
            }
        }
    }
}

/// Exposure runs expanded to one entry per frame.
/// Exposure runs expanded to frames; `None` unless every run has a count ≥ 1 (10 §4.7, C11-R3-03) and the total is
/// bounded.
fn expand_runs(v: &Value) -> Option<Vec<Value>> {
    let runs: Vec<ExposureRun> = serde_json::from_value(v.clone()).ok()?;
    if runs.iter().any(|(_, c)| *c == 0) {
        return None;
    }
    let mut out = Vec::new();
    for (d, c) in runs {
        let cell = d.map_or(Value::Null, |d| Value::String(d.as_str().to_owned()));
        for _ in 0..c {
            out.push(cell.clone());
        }
        if out.len() > crate::document::MAX_SCENE_LENGTH as usize * 2 {
            return None;
        }
    }
    Some(out)
}

/// Frames back to explicit runs (blanks kept, no canonicalisation: windows must invert exactly).
fn frames_to_runs(frames: &[Value]) -> Value {
    let mut runs: Vec<Value> = Vec::new();
    let mut i = 0;
    while let Some(cell) = frames.get(i) {
        let mut n = 1usize;
        while frames.get(i + n) == Some(cell) {
            n += 1;
        }
        runs.push(json!([cell, n]));
        i += n;
    }
    Value::Array(runs)
}

fn frame(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn range_diff<'a, T: PartialEq + Clone>(a: &'a [T], b: &'a [T]) -> Option<(usize, &'a [T], &'a [T])> {
    if a == b {
        return None;
    }
    let p = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let max_q = a.len().min(b.len()) - p;
    let q = a.iter().rev().zip(b.iter().rev()).take(max_q).take_while(|(x, y)| x == y).count();
    Some((p, a.get(p..a.len() - q).unwrap_or(&[]), b.get(p..b.len() - q).unwrap_or(&[])))
}

fn exposure_patch(a: &Value, b: &Value, path: &str) -> Option<PatchOp> {
    let mut fa = expand_runs(a)?;
    let mut fb = expand_runs(b)?;
    let n = fa.len().max(fb.len());
    fa.resize(n, Value::Null);
    fb.resize(n, Value::Null);
    let (p, old, new) = range_diff(&fa, &fb)?;
    let len = old.len().max(new.len()).max(1);
    Some(PatchOp {
        op: "replace",
        from: None,
        path: path.to_owned(),
        range: Some(FrameRange { from: frame(p + 1), to: frame(p + len) }),
        value: Some(frames_to_runs(new)),
        old_value: Some(frames_to_runs(old)),
        elided: false,
    })
}

fn mouths_patch(a: &str, b: &str, path: &str) -> Option<PatchOp> {
    let ca: Vec<char> = a.chars().collect();
    let cb: Vec<char> = b.chars().collect();
    let (p, old, new) = range_diff(&ca, &cb)?;
    let len = old.len().max(new.len()).max(1);
    Some(PatchOp {
        op: "replace",
        from: None,
        path: path.to_owned(),
        range: Some(FrameRange { from: frame(p + 1), to: frame(p + len) }),
        value: Some(Value::String(new.iter().collect())),
        old_value: Some(Value::String(old.iter().collect())),
        elided: false,
    })
}

/// The RFC 6902 patch (with `oldValue`) turning own view `a` into own view `b`.
#[must_use]
pub fn diff_json(a: &Value, b: &Value, path: &str, ty: ObjectType) -> Vec<PatchOp> {
    let mut out = Vec::new();
    diff_into(a, b, path, ty, &mut out);
    out
}

fn diff_into(a: &Value, b: &Value, path: &str, ty: ObjectType, out: &mut Vec<PatchOp>) {
    if a == b {
        return;
    }
    if ty == ObjectType::Element
        && path == "/exposure"
        && let Some(op) = exposure_patch(a, b, path)
    {
        out.push(op);
        return;
    }
    // The declared mouth-track text of a placement's own view; the same suffix inside opaque data is ordinary JSON.
    if ty == ObjectType::SoundPlacement
        && path == "/mouthTrack/mouths"
        && let (Some(x), Some(y)) = (a.as_str(), b.as_str())
        && let Some(op) = mouths_patch(x, y, path)
    {
        out.push(op);
        return;
    }
    match (a, b) {
        (Value::Object(ma), Value::Object(mb)) => {
            for (k, va) in ma {
                let p = format!("{path}/{}", pointer::escape(k));
                match mb.get(k) {
                    Some(vb) => diff_into(va, vb, &p, ty, out),
                    None => out.push(PatchOp::remove(p, va.clone())),
                }
            }
            for (k, vb) in mb {
                if !ma.contains_key(k) {
                    out.push(PatchOp::add(format!("{path}/{}", pointer::escape(k)), vb.clone()));
                }
            }
        }
        (Value::Array(xa), Value::Array(xb))
            if is_child_list(ty, path) && xa.iter().chain(xb).all(|v| type_of(v).is_some()) =>
        {
            diff_collection(xa, xb, path, out);
        }
        _ => out.push(PatchOp::replace(path.to_owned(), a.clone(), b.clone())),
    }
}

fn new_op(kind: &'static str, path: &str, from: Option<String>, value: Option<Value>, old: Option<Value>) -> PatchOp {
    PatchOp { op: kind, from, path: path.to_owned(), range: None, value, old_value: old, elided: false }
}

/// The inverse of a patch computed against the state it applies to (10 §2.6; C11-R2-02): applying the patch to
/// `before` and then the returned inverse restores `before` exactly. Every operation of the inverse uses concrete
/// paths (an append `…/-` becomes the index it created; a `move` is resolved after its source was removed, so a
/// shifted destination is restored where it really is) and carries `oldValue`, so the inverse is itself invertible.
/// This is how change sets, undo and redo invert patches.
///
/// # Errors
/// As [`apply`]: the patch must apply to `before`.
pub fn invert_against(before: &Value, patch: &[PatchOp]) -> Result<Vec<PatchOp>, ModelError> {
    apply_with_inverse(before, patch).map(|(_, inverse)| inverse)
}

/// Applies a patch and returns the result together with its concrete inverse (see [`invert_against`]).
///
/// # Errors
/// As [`apply`].
pub fn apply_with_inverse(value: &Value, patch: &[PatchOp]) -> Result<(Value, Vec<PatchOp>), ModelError> {
    let mut v = value.clone();
    let mut steps: Vec<Vec<PatchOp>> = Vec::with_capacity(patch.len());
    for op in patch {
        steps.push(apply_op(&mut v, op)?);
    }
    Ok((v, steps.into_iter().rev().flatten().collect()))
}

fn patch_err(op: &PatchOp, msg: impl Into<String>) -> ModelError {
    ModelError::with_details(codes::ARG_INVALID, msg, json!({ "pointer": op.path, "op": op.op }))
}

fn parent_and_last(path: &str) -> Result<(Vec<String>, String), ModelError> {
    let mut tokens = pointer::parse(path)?;
    let last = tokens.pop().ok_or_else(|| ModelError::invalid(path, "the operation needs a non-root path"))?;
    Ok((tokens, last))
}

/// RFC 6902 `add` semantics at `path` (the root is replaced; an object member is added or replaced; an array element
/// is inserted, `-` appending). `oldValue`, when the operation carries one, must be exactly the value overwritten;
/// an insertion that overwrites nothing must not carry one. Returns the concrete inverse of the insertion.
fn insert(root: &mut Value, path: &str, value: Value, op: &PatchOp) -> Result<Vec<PatchOp>, ModelError> {
    if path.is_empty() {
        if op.old_value.as_ref().is_some_and(|o| o != root) {
            return Err(patch_err(op, "oldValue does not match the replaced document"));
        }
        let old = std::mem::replace(root, value.clone());
        return Ok(vec![new_op("replace", "", None, Some(old), Some(value))]);
    }
    let (parent, last) = parent_and_last(path)?;
    match pointer::get_mut(root, &parent) {
        Some(Value::Object(m)) => {
            if op.old_value.is_some() && m.get(&last) != op.old_value.as_ref() {
                return Err(patch_err(op, "oldValue does not match the overwritten member"));
            }
            Ok(match m.insert(last, value.clone()) {
                Some(old) => vec![new_op("replace", path, None, Some(old), Some(value))],
                None => vec![new_op("remove", path, None, None, Some(value))],
            })
        }
        Some(Value::Array(_)) if op.old_value.is_some() => {
            Err(patch_err(op, "an array insertion overwrites nothing and carries no oldValue"))
        }
        Some(Value::Array(a)) => {
            let i = if last == "-" {
                a.len()
            } else {
                pointer::array_index(&last).ok_or_else(|| patch_err(op, "invalid array index"))?
            };
            if i > a.len() {
                return Err(patch_err(op, "array index out of range"));
            }
            a.insert(i, value.clone());
            let mut at = parent;
            at.push(i.to_string());
            Ok(vec![new_op("remove", &pointer::join(&at), None, None, Some(value))])
        }
        _ => Err(patch_err(op, format!("no container at `{path}`"))),
    }
}

fn take(root: &mut Value, path: &str, op: &PatchOp) -> Result<Value, ModelError> {
    let (parent, last) = parent_and_last(path)?;
    match pointer::get_mut(root, &parent) {
        Some(Value::Object(m)) => m.shift_remove(&last).ok_or_else(|| patch_err(op, format!("nothing at `{path}`"))),
        Some(Value::Array(a)) => {
            let i = pointer::array_index(&last)
                .filter(|i| *i < a.len())
                .ok_or_else(|| patch_err(op, "array index out of range"))?;
            Ok(a.remove(i))
        }
        _ => Err(patch_err(op, format!("nothing at `{path}`"))),
    }
}

/// Validates and applies a range replace (C11-R2-03): `value` and `oldValue` must have the target's form (strings of
/// mouth letters for a mouth string, exposure runs for exposure), the range must be `from ≥ 1`,
/// `to = from + max(|old|, |new|) − 1`, and `oldValue` must be exactly the content of the window it replaces.
fn apply_range(target: &mut Value, op: &PatchOp) -> Result<(), ModelError> {
    let r = op.range.ok_or_else(|| patch_err(op, "range missing"))?;
    // Range semantics belong to the declared fields only (C11-R3-01): an element's `exposure` and a placement's
    // `mouthTrack/mouths`, never anything inside opaque `meta`/`ext` values (those are replaced wholesale).
    let tokens = pointer::parse(&op.path)?;
    let declared = matches!(tokens.as_slice(), [.., e] if e == "exposure")
        || matches!(tokens.as_slice(), [.., t, m] if t == "mouthTrack" && m == "mouths");
    if !declared || tokens.iter().any(|t| t == "meta" || t == "ext") {
        return Err(patch_err(op, "range operations apply only to `exposure` and `mouthTrack/mouths`"));
    }
    let (Some(value), Some(old)) = (op.value.as_ref(), op.old_value.as_ref()) else {
        return Err(patch_err(op, "a range replace needs value and oldValue"));
    };
    if r.from < 1 {
        return Err(patch_err(op, "range.from is a frame (≥ 1)"));
    }
    let start = (r.from as usize) - 1;
    let check_to = |old_len: usize, new_len: usize| {
        let len = old_len.max(new_len).max(1);
        if u64::from(r.to) + 1 == u64::from(r.from) + len as u64 {
            Ok(())
        } else {
            Err(patch_err(op, format!("range.to must be from + {len} − 1 for these values")))
        }
    };
    match target {
        Value::String(s) => {
            let (Value::String(new), Value::String(old)) = (value, old) else {
                return Err(patch_err(op, "a mouth-string range replace takes string value and oldValue"));
            };
            let old: Vec<char> = old.chars().collect();
            let new: Vec<char> = new.chars().collect();
            if new.iter().any(|c| !"ABCDEFGX".contains(*c)) {
                return Err(patch_err(op, "a mouth-string value holds only the mouth letters A–G and X (10 §4.14)"));
            }
            check_to(old.len(), new.len())?;
            let mut chars: Vec<char> = s.chars().collect();
            if chars.get(start..start + old.len()) != Some(old.as_slice()) {
                return Err(patch_err(op, "oldValue is not the current content of the range"));
            }
            chars.splice(start..start + old.len(), new);
            *s = chars.into_iter().collect();
            Ok(())
        }
        Value::Array(_) => {
            let mut frames = expand_runs(target).ok_or_else(|| patch_err(op, "not exposure runs"))?;
            let new = expand_runs(value)
                .ok_or_else(|| patch_err(op, "value is not exposure runs (each `[drawingId | null, count ≥ 1]`)"))?;
            let old = expand_runs(old)
                .ok_or_else(|| patch_err(op, "oldValue is not exposure runs (each `[drawingId | null, count ≥ 1]`)"))?;
            check_to(old.len(), new.len())?;
            // Frames past the end are blank (10 §4.7).
            if frames.len() < start + old.len() {
                frames.resize(start + old.len(), Value::Null);
            }
            if frames.get(start..start + old.len()) != Some(old.as_slice()) {
                return Err(patch_err(op, "oldValue is not the current content of the range"));
            }
            frames.splice(start..start + old.len(), new);
            let runs: Vec<ExposureRun> =
                serde_json::from_value(frames_to_runs(&frames)).map_err(|e| patch_err(op, e.to_string()))?;
            *target =
                serde_json::to_value(crate::object::canonical_runs(&runs)).map_err(|e| patch_err(op, e.to_string()))?;
            Ok(())
        }
        _ => Err(patch_err(op, "range operations apply to exposure runs and mouth strings")),
    }
}

/// Applies one operation and returns its concrete inverse against the state it was applied to.
fn apply_op(v: &mut Value, op: &PatchOp) -> Result<Vec<PatchOp>, ModelError> {
    // 10 §2.6: an elided value is unknown, never `null`; the receiver rereads the object instead.
    if op.elided {
        return Err(patch_err(op, "the operation's values were elided; reread the object instead of applying it"));
    }
    let value = || op.value.clone().ok_or_else(|| patch_err(op, format!("`{}` needs a value", op.op)));
    // RFC 6902 §4.4/§4.5: `move` and `copy` name their source; a missing `from` is an error, not the root.
    let from = || op.from.clone().ok_or_else(|| patch_err(op, format!("`{}` needs `from`", op.op)));
    match op.op {
        "add" => insert(v, &op.path, value()?, op),
        "remove" => {
            let old = take(v, &op.path, op)?;
            Ok(vec![new_op("add", &op.path, None, Some(old), None)])
        }
        "replace" if op.range.is_some() => {
            let tokens = pointer::parse(&op.path)?;
            let target =
                pointer::get_mut(v, &tokens).ok_or_else(|| patch_err(op, format!("nothing at `{}`", op.path)))?;
            apply_range(target, op)?;
            Ok(vec![PatchOp {
                range: op.range,
                ..new_op("replace", &op.path, None, op.old_value.clone(), op.value.clone())
            }])
        }
        "replace" => {
            let new = value()?;
            let tokens = pointer::parse(&op.path)?;
            let target =
                pointer::get_mut(v, &tokens).ok_or_else(|| patch_err(op, format!("nothing at `{}`", op.path)))?;
            let old = std::mem::replace(target, new.clone());
            Ok(vec![new_op("replace", &op.path, None, Some(old), Some(new))])
        }
        "move" => {
            let from = from()?;
            if op.path.starts_with(&format!("{from}/")) {
                return Err(patch_err(op, "cannot move a value into itself"));
            }
            if from.is_empty() || op.path.is_empty() {
                return Err(patch_err(op, "`move` needs non-root paths"));
            }
            let item = take(v, &from, op)?;
            // The destination is resolved after the source was removed (RFC 6902 §4.4); undo the insertion first,
            // then put the value back at its source.
            let mut inverse = insert(v, &op.path, item.clone(), op)?;
            inverse.push(new_op("add", &from, None, Some(item), None));
            Ok(inverse)
        }
        "copy" => {
            let from = pointer::parse(&from()?)?;
            let item = pointer::get(v, &from).cloned().ok_or_else(|| patch_err(op, "nothing to copy"))?;
            insert(v, &op.path, item, op)
        }
        "test" => {
            let expected = value()?;
            let tokens = pointer::parse(&op.path)?;
            if pointer::get(v, &tokens) != Some(&expected) {
                return Err(ModelError::with_details(
                    codes::PATCH_TEST_FAILED,
                    format!("test failed at `{}`", op.path),
                    json!({ "pointer": op.path }),
                ));
            }
            Ok(Vec::new())
        }
        other => Err(patch_err(op, format!("unknown operation `{other}`"))),
    }
}

/// Applies a patch to a JSON value (RFC 6902 plus the range form). All-or-nothing: on error the input is returned
/// unchanged through the error path.
///
/// # Errors
/// `arg.invalid` (with `details.pointer`) for a path that does not exist, a missing `value` or `from`, an elided
/// operation, an `oldValue` that is not what the operation overwrites, or a malformed range operation;
/// `doc.patchTestFailed` for a failing `test`.
pub fn apply(value: &Value, patch: &[PatchOp]) -> Result<Value, ModelError> {
    apply_with_inverse(value, patch).map(|(v, _)| v)
}

/// Replaces values and old values larger than [`ELIDE_BYTES`] by `elided: true` (events and `doc.diff`).
pub fn elide(changes: &mut [Change]) {
    for c in changes {
        for op in c.patch.iter_mut().flatten() {
            let big = |v: &Option<Value>| {
                v.as_ref().is_some_and(|v| serde_json::to_string(v).map_or(true, |s| s.len() > ELIDE_BYTES))
            };
            if big(&op.value) || big(&op.old_value) {
                op.value = None;
                op.old_value = None;
                op.elided = true;
            }
        }
    }
}

/// Groups changes by type for summaries (`doc.diff {detail: "summary"}`).
#[must_use]
pub fn summary(changes: &[Change]) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for c in changes {
        *m.entry(format!("{}.{:?}", c.ty, c.op)).or_insert(0) += 1;
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(kind: &'static str, path: &str, from: Option<&str>, value: Option<Value>, old: Option<Value>) -> PatchOp {
        PatchOp { from: from.map(str::to_owned), ..new_op(kind, path, None, value, old) }
    }

    fn range_at(path: &str, value: Value, old: Value, from: u32, to: u32) -> PatchOp {
        PatchOp { range: Some(FrameRange { from, to }), ..op("replace", path, None, Some(value), Some(old)) }
    }

    fn mouths(value: Value, old: Value, from: u32, to: u32) -> PatchOp {
        range_at("/mouthTrack/mouths", value, old, from, to)
    }

    fn exposure(value: Value, old: Value, from: u32, to: u32) -> PatchOp {
        range_at("/exposure", value, old, from, to)
    }

    #[test]
    fn range_replace_checks_types_window_and_bounds() {
        let doc = json!({"mouthTrack": {"mouths": "ABC"}});
        assert_eq!(
            apply(&doc, &[mouths(json!("X"), json!("B"), 2, 2)]).unwrap(),
            json!({"mouthTrack": {"mouths": "AXC"}})
        );
        for bad in [
            mouths(json!(42), json!("B"), 2, 2),     // value of the wrong type
            mouths(json!("X"), json!(7), 2, 2),      // oldValue of the wrong type
            mouths(json!("X"), json!("C"), 2, 2),    // oldValue is not the window's content
            mouths(json!("X"), json!("B"), 2, 3),    // `to` inconsistent with the values
            mouths(json!("X"), json!("B"), 0, 0),    // frame 0
            mouths(json!("Z"), json!("B"), 2, 2),    // not a mouth letter
            mouths(json!("XX"), json!("BCD"), 2, 4), // window past the end
        ] {
            assert!(apply(&doc, std::slice::from_ref(&bad)).is_err(), "{bad:?}");
        }
        let runs = json!({"exposure": [["a", 2], ["b", 1]]});
        let ok = exposure(json!([["c", 1]]), json!([["a", 1]]), 2, 2);
        assert_eq!(
            apply(&runs, std::slice::from_ref(&ok)).unwrap(),
            json!({"exposure": [["a", 1], ["c", 1], ["b", 1]]})
        );
        for bad in [
            exposure(json!("X"), json!([["a", 1]]), 2, 2), // value of the wrong form
            exposure(json!([["c", 1]]), json!([["b", 1]]), 2, 2), // oldValue is not the window's content
            exposure(json!([["c", 0]]), json!([["a", 1]]), 2, 2), // zero-count run (C11-R3-03)
            exposure(json!([["c", 1], ["d", 0]]), json!([["a", 1]]), 2, 2),
            exposure(json!([["c", 1]]), json!([["a", 1], ["a", 0]]), 2, 2),
        ] {
            assert!(apply(&runs, std::slice::from_ref(&bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn range_operations_apply_only_to_declared_fields() {
        let doc = json!({"meta": {"tool": {"mouthTrack": {"mouths": "hello"}}},
            "ext": {"com.example": {"mouthTrack": {"mouths": "hello"}}}, "note": "hello"});
        for path in ["/meta/tool/mouthTrack/mouths", "/ext/com.example/mouthTrack/mouths", "/note"] {
            let p = range_at(path, json!("world"), json!("hello"), 1, 5);
            assert!(apply(&doc, std::slice::from_ref(&p)).is_err(), "{path}");
        }
        // A diff of such opaque text is an ordinary replace that applies.
        let mut after = doc.clone();
        after["meta"]["tool"]["mouthTrack"]["mouths"] = json!("world");
        let patch = diff_json(&doc, &after, "", ObjectType::SoundPlacement);
        assert!(patch.iter().all(|o| o.range.is_none()), "{patch:?}");
        assert_eq!(apply(&doc, &patch).unwrap(), after);
    }

    #[test]
    fn move_and_copy_need_from() {
        for kind in ["move", "copy"] {
            assert!(apply(&json!({"a": 1}), &[op(kind, "/b", None, None, None)]).is_err(), "{kind}");
        }
        assert_eq!(
            apply(&json!({"a": 1}), &[op("copy", "/b", Some(""), None, None)]).unwrap(),
            json!({"a": 1, "b": {"a": 1}})
        );
    }

    #[test]
    fn inverse_against_pre_state_is_concrete() {
        let doc = json!({"a": [1, 2], "o": {"x": 1}});
        let patch =
            [op("add", "/a/-", None, Some(json!(3)), None), op("move", "/o/x", Some("/a/0"), None, Some(json!(1)))];
        let (after, inverse) = apply_with_inverse(&doc, &patch).unwrap();
        assert_eq!(inverse[0].path, "/o/x");
        assert!(inverse.iter().all(|o| !o.path.ends_with("/-")), "{inverse:?}");
        assert_eq!(apply(&after, &inverse).unwrap(), doc);
    }
}
