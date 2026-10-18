//! Change descriptions (10 §2.5–§2.6, §10.4; 11 §7.2, §9.2).
//!
//! A change set lists, per addressable object, `created` (with `parentId`, `index`), `deleted` (with `parentId`) or
//! `modified` with an RFC 6902 patch against the object's **own view**: its JSON with `rev` removed and each child
//! object reduced to `{id, type}`. A child's property change therefore never changes its parent (REV-02), while
//! membership and order of a child collection do. Every operation carries `oldValue`, so a patch inverts exactly
//! ([`invert`]); exposure and mouth-track edits use the `range` form so long scenes give small patches.
//!
//! **Range operations.** `{op: "replace", path, range: {from, to}, value, oldValue}` on `/exposure` (runs) or
//! `…/mouthTrack/mouths` (one letter per frame): frames `from…` covered by `oldValue` are replaced by `value`
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

/// Splits an entry's JSON into the own views of every addressable object it holds.
fn collect(v: &Value, ty: ObjectType, parent: Option<&str>, index: usize, ctx: (&str, &str), out: &mut Vec<OwnObject>) {
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
    let mut own = v.clone();
    if let Value::Object(m) = &mut own {
        m.shift_remove("rev");
    }
    for (field, slot) in crate::document::slots_of(ty) {
        let Some(child) = own.get_mut(*field) else { continue };
        let inline = !matches!(slot, crate::document::SlotKind::Split(_) | crate::document::SlotKind::SplitOne(_));
        match child {
            Value::Array(items) => {
                for (i, item) in items.iter_mut().enumerate() {
                    if let Some(cty) = type_of(item) {
                        if inline {
                            collect(item, cty, Some(&key), i, (drawing, channel), out);
                        }
                        *item = child_stub(item, cty);
                    }
                }
            }
            Value::Object(m) if matches!(slot, crate::document::SlotKind::InlineMap) => {
                for (i, item) in m.values_mut().enumerate() {
                    if let Some(cty) = type_of(item) {
                        collect(item, cty, Some(&key), i, (drawing, channel), out);
                        *item = child_stub(item, cty);
                    }
                }
            }
            item @ Value::Object(_) => {
                if let Some(cty) = type_of(item) {
                    if inline {
                        collect(item, cty, Some(&key), 0, (drawing, channel), out);
                    }
                    *item = child_stub(item, cty);
                }
            }
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
    collect(&v, ty, parent.as_deref(), index, ("", ""), &mut out);
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

fn is_id_collection(items: &[Value]) -> bool {
    !items.is_empty() && items.iter().all(|v| type_of(v).is_some() && v.as_object().is_some_and(|m| m.len() == 2))
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
fn expand_runs(v: &Value) -> Option<Vec<Value>> {
    let runs: Vec<ExposureRun> = serde_json::from_value(v.clone()).ok()?;
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
    if path.ends_with("/mouthTrack/mouths")
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
        (Value::Array(xa), Value::Array(xb)) if is_id_collection(xa) || is_id_collection(xb) => {
            if xa.iter().chain(xb).all(|v| type_of(v).is_some()) {
                diff_collection(xa, xb, path, out);
            } else {
                out.push(PatchOp::replace(path.to_owned(), a.clone(), b.clone()));
            }
        }
        _ => out.push(PatchOp::replace(path.to_owned(), a.clone(), b.clone())),
    }
}

/// The inverse of a patch: applied after the patch it restores the original exactly.
#[must_use]
pub fn invert(patch: &[PatchOp]) -> Vec<PatchOp> {
    patch
        .iter()
        .rev()
        .filter_map(|op| match op.op {
            "add" => Some(PatchOp::remove(op.path.clone(), op.value.clone().unwrap_or(Value::Null))),
            "remove" => Some(PatchOp::add(op.path.clone(), op.old_value.clone().unwrap_or(Value::Null))),
            "replace" => Some(PatchOp {
                op: "replace",
                from: None,
                path: op.path.clone(),
                // `to = from + max(|old|, |new|) − 1` is symmetric, so the range is unchanged.
                range: op.range,
                value: op.old_value.clone(),
                old_value: op.value.clone(),
                elided: false,
            }),
            "move" => Some(PatchOp::mv(op.path.clone(), op.from.clone().unwrap_or_default())),
            "copy" => Some(PatchOp::remove(op.path.clone(), op.value.clone().unwrap_or(Value::Null))),
            _ => None,
        })
        .collect()
}

fn window_len(v: &Value) -> usize {
    match v {
        Value::String(s) => s.chars().count(),
        Value::Array(runs) => {
            runs.iter().filter_map(|r| r.get(1).and_then(Value::as_u64)).map(|n| usize::try_from(n).unwrap_or(0)).sum()
        }
        _ => 0,
    }
}

fn patch_err(op: &PatchOp, msg: impl Into<String>) -> ModelError {
    ModelError::with_details(codes::ARG_INVALID, msg, json!({ "pointer": op.path, "op": op.op }))
}

fn parent_and_last(path: &str) -> Result<(Vec<String>, String), ModelError> {
    let mut tokens = pointer::parse(path)?;
    let last = tokens.pop().ok_or_else(|| ModelError::invalid(path, "the operation needs a non-root path"))?;
    Ok((tokens, last))
}

fn insert(root: &mut Value, path: &str, value: Value, op: &PatchOp) -> Result<(), ModelError> {
    let (parent, last) = parent_and_last(path)?;
    match pointer::get_mut(root, &parent) {
        Some(Value::Object(m)) => {
            m.insert(last, value);
            Ok(())
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
            a.insert(i, value);
            Ok(())
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

fn apply_range(target: &mut Value, op: &PatchOp) -> Result<(), ModelError> {
    let r = op.range.ok_or_else(|| patch_err(op, "range missing"))?;
    let start = (r.from as usize).saturating_sub(1);
    let old_len = op.old_value.as_ref().map_or(0, window_len);
    match target {
        Value::String(s) => {
            let mut chars: Vec<char> = s.chars().collect();
            if start > chars.len() {
                return Err(patch_err(op, "range starts after the end"));
            }
            let end = (start + old_len).min(chars.len());
            let new: Vec<char> = op.value.as_ref().and_then(Value::as_str).unwrap_or("").chars().collect();
            chars.splice(start..end, new);
            *s = chars.into_iter().collect();
            Ok(())
        }
        Value::Array(_) => {
            let mut frames = expand_runs(target).ok_or_else(|| patch_err(op, "not exposure runs"))?;
            let new = op.value.as_ref().and_then(expand_runs).ok_or_else(|| patch_err(op, "value is not runs"))?;
            if frames.len() < start + old_len {
                frames.resize(start + old_len, Value::Null);
            }
            frames.splice(start..start + old_len, new);
            let runs: Vec<ExposureRun> =
                serde_json::from_value(frames_to_runs(&frames)).map_err(|e| patch_err(op, e.to_string()))?;
            *target =
                serde_json::to_value(crate::object::canonical_runs(&runs)).map_err(|e| patch_err(op, e.to_string()))?;
            Ok(())
        }
        _ => Err(patch_err(op, "range operations apply to exposure runs and mouth strings")),
    }
}

/// Applies a patch to a JSON value (RFC 6902 plus the range form). All-or-nothing: on error the input is returned
/// unchanged through the error path.
///
/// # Errors
/// `arg.invalid` (with `details.pointer`) for a path that does not exist; `doc.patchTestFailed` for a failing `test`.
pub fn apply(value: &Value, patch: &[PatchOp]) -> Result<Value, ModelError> {
    let mut v = value.clone();
    for op in patch {
        match op.op {
            "add" => insert(&mut v, &op.path, op.value.clone().unwrap_or(Value::Null), op)?,
            "remove" => {
                take(&mut v, &op.path, op)?;
            }
            "replace" if op.range.is_some() => {
                let tokens = pointer::parse(&op.path)?;
                let target = pointer::get_mut(&mut v, &tokens)
                    .ok_or_else(|| patch_err(op, format!("nothing at `{}`", op.path)))?;
                apply_range(target, op)?;
            }
            "replace" => {
                let tokens = pointer::parse(&op.path)?;
                let target = pointer::get_mut(&mut v, &tokens)
                    .ok_or_else(|| patch_err(op, format!("nothing at `{}`", op.path)))?;
                *target = op.value.clone().unwrap_or(Value::Null);
            }
            "move" => {
                let from = op.from.clone().unwrap_or_default();
                if op.path.starts_with(&format!("{from}/")) {
                    return Err(patch_err(op, "cannot move a value into itself"));
                }
                let item = take(&mut v, &from, op)?;
                insert(&mut v, &op.path, item, op)?;
            }
            "copy" => {
                let from = pointer::parse(op.from.as_deref().unwrap_or_default())?;
                let item = pointer::get(&v, &from).cloned().ok_or_else(|| patch_err(op, "nothing to copy"))?;
                insert(&mut v, &op.path, item, op)?;
            }
            "test" => {
                let tokens = pointer::parse(&op.path)?;
                if pointer::get(&v, &tokens) != op.value.as_ref() {
                    return Err(ModelError::with_details(
                        codes::PATCH_TEST_FAILED,
                        format!("test failed at `{}`", op.path),
                        json!({ "pointer": op.path }),
                    ));
                }
            }
            other => return Err(patch_err(op, format!("unknown operation `{other}`"))),
        }
    }
    Ok(v)
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
