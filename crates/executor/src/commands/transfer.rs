//! Artwork transfer shared by every copy path of the cell and element clipboards and of `cell.range.move`
//! (04 §13.16, §14.1; M3 C3.1 round 1: C31-R1-04, -07, -13).
//!
//! **Payload.** A copy captures an [`ArtPayload`]: the full JSON of every drawing the copied cells or elements
//! reference (and, for whole elements, every drawing of their banks), the binary entries those drawings use, and the
//! copied cells' notes. A paste reads drawings from the payload first, so deleting or replacing the source document
//! (another project opened, the last clone deleted) never invalidates a copy; paste remaps drawing IDs into the
//! destination and keeps drawing-local node/stroke IDs, owner/stretch records, `colourRecovery` and opaque `meta`/`ext`
//! verbatim.
//!
//! **Placement.** [`Transfer`] is the one implementation that places a source drawing into a destination bank:
//! the same drawing when it already belongs to that bank (paste within an element or clone group), a full overwrite
//! of a same-named destination drawing (keeping the destination's ID, creation time and revision; locked drawings
//! refused), or a new verbatim copy. Copies and overwrites are taken verbatim by the settle pass.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_anim::exposure as x;
use tt_model::Id;
use tt_model::document::{Document, EntryKey};
use tt_model::entities::element::ElementType;
use tt_registry::CommandError;

use super::timing::{Changes, add_drawing, internal, locked, op, to_json};
use crate::cx::Cx;

/// The self-contained artwork of a copy.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ArtPayload {
    /// The project copied from.
    pub project_id: String,
    /// Full drawing objects by source drawing ID.
    #[schemars(with = "BTreeMap<String, serde_json::Map<String, Value>>")]
    pub drawings: BTreeMap<String, Value>,
    /// Source banks and their drawings (whole-element copies).
    pub banks: BTreeMap<String, Vec<String>>,
    /// Binary entries the drawings use: entry name → base64.
    pub blobs: BTreeMap<String, String>,
    /// Cell notes of copied cells: element ID → frame offset (decimal) → text.
    pub notes: BTreeMap<String, BTreeMap<String, String>>,
    /// The element type of every copied column (compatibility when the source element no longer exists).
    pub element_types: BTreeMap<String, String>,
}

/// Entry names a drawing references (`entry` of bitmap blobs, texture images).
fn entries(v: &Value, out: &mut BTreeSet<String>, depth: usize) {
    if depth > 200 {
        return;
    }
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                if k == "meta" || k == "ext" {
                    continue;
                }
                match (k.as_str(), x) {
                    ("entry", Value::String(s)) => {
                        out.insert(s.clone());
                    }
                    ("image", Value::String(s)) if s.starts_with("textures/") => {
                        out.insert(s.clone());
                    }
                    _ => entries(x, out, depth + 1),
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| entries(x, out, depth + 1)),
        _ => {}
    }
}

impl ArtPayload {
    /// An empty payload of the current project.
    pub(crate) fn of(doc: &Document) -> Self {
        Self { project_id: doc.project().id().as_str().to_owned(), ..Self::default() }
    }

    /// Captures a drawing (and its binary entries) once.
    pub(crate) fn capture_drawing(&mut self, cx: &Cx, doc: &Document, d: &Id) -> Result<(), CommandError> {
        if self.drawings.contains_key(d.as_str()) {
            return Ok(());
        }
        let v = doc.entry_raw_json(&EntryKey::Drawing(d.clone())).map_err(internal)?;
        let mut names = BTreeSet::new();
        entries(&v, &mut names, 0);
        if let Ok(p) = cx.project() {
            for n in names {
                if !self.blobs.contains_key(&n)
                    && let Ok(bytes) = p.store.blobs.read(&n, 1 << 32)
                {
                    self.blobs.insert(n, tt_persist::b64::encode(&bytes));
                }
            }
        }
        self.drawings.insert(d.as_str().to_owned(), v);
        Ok(())
    }

    /// Captures every drawing of a bank.
    pub(crate) fn capture_bank(&mut self, cx: &Cx, doc: &Document, bank: &Id) -> Result<(), CommandError> {
        let ids: Vec<Id> = doc.bank(bank).map(|b| b.drawings().ids().to_vec()).unwrap_or_default();
        for d in &ids {
            self.capture_drawing(cx, doc, d)?;
        }
        self.banks.insert(bank.as_str().to_owned(), ids.iter().map(|d| d.as_str().to_owned()).collect());
        Ok(())
    }

    /// The JSON of a source drawing: the payload's copy, else the live document's.
    pub(crate) fn drawing(&self, doc: &Document, d: &Id) -> Option<Value> {
        self.drawings.get(d.as_str()).cloned().or_else(|| doc.entry_raw_json(&EntryKey::Drawing(d.clone())).ok())
    }

    /// The element type of a copied column: as recorded by the copy (authoritative across projects, where the same ID
    /// may name another element), else the live element's.
    pub(crate) fn source_type(&self, doc: &Document, source: &str) -> Option<ElementType> {
        self.element_types
            .get(source)
            .and_then(|t| serde_json::from_value::<ElementType>(json!(t)).ok())
            .or_else(|| Id::parse(source).ok().and_then(|s| doc.element(&s)).map(|e| *e.element_type()))
    }

    /// Source/destination column compatibility (04 §13.16, §13.21), checked for every pasted column before any
    /// drawing or binary entry is placed, by every cell paste entry point (`cell.clipboard.apply`, `edit.paste`,
    /// `edit.pasteSpecial`; C31-R2-03). An unknown source type is accepted.
    pub(crate) fn check_column(&self, doc: &Document, source: &str, target: &Id) -> Result<(), CommandError> {
        let Some(tt) = doc.element(target).map(|e| *e.element_type()) else { return Ok(()) };
        match self.source_type(doc, source) {
            Some(st) if !compatible(st, tt) => Err(type_mismatch(source, target.as_str(), st, tt)),
            _ => Ok(()),
        }
    }

    /// The copied drawing `v` as placed in this project: every binary entry it references is the copy's bytes. An
    /// entry missing here is added under its name; an entry holding the same bytes is shared; an entry holding other
    /// bytes keeps them, and the copy gets a fresh deterministic entry (`<name>-<sha256 prefix>.<ext>`) with its typed
    /// references (`entry`, texture `image`) rewritten; opaque `meta`/`ext` are not touched. New entries are deferred
    /// effects of the call, accepted with its commit: a dry run or a refused call adds nothing (C31-R2-06).
    pub(crate) fn place_blobs(&self, cx: &mut Cx, v: &Value) -> Value {
        let mut names = BTreeSet::new();
        entries(v, &mut names, 0);
        let mut map: BTreeMap<String, String> = BTreeMap::new();
        for n in names {
            let Some(bytes) = self.blobs.get(&n).and_then(|b| tt_persist::b64::decode(b)) else { continue };
            let hash = tt_persist::fsx::sha256(&bytes);
            let mut name = n.clone();
            let mut attempt = 0u32;
            loop {
                match cx.blob_bytes(&name) {
                    None => {
                        cx.stage_blob(name.clone(), bytes);
                        break;
                    }
                    Some(existing) if *existing == bytes => break,
                    Some(_) => {
                        name = fresh_entry(&n, &hash, attempt);
                        attempt = attempt.saturating_add(1);
                    }
                }
            }
            if name != n {
                map.insert(n, name);
            }
        }
        let mut out = v.clone();
        if !map.is_empty() {
            rewrite_entries(&mut out, &map, 0);
        }
        out
    }
}

/// Whether two element types hold compatible drawings (vector drawing and annotation; image with image).
pub(crate) fn compatible(a: ElementType, b: ElementType) -> bool {
    let vector = |t: ElementType| matches!(t, ElementType::Drawing | ElementType::Annotation);
    (vector(a) && vector(b)) || (a == ElementType::Image && b == ElementType::Image)
}

pub(crate) fn type_mismatch(from: &str, to: &str, a: ElementType, b: ElementType) -> CommandError {
    CommandError::with_details(
        "cell.typeMismatch",
        format!("{} cells cannot go into a {} column", a.as_str(), b.as_str()),
        json!({ "sourceElementId": from, "targetElementId": to, "source": a.as_str(), "target": b.as_str() }),
    )
}

/// A fresh entry name for `name` holding content with `hash`: the hash prefix before the extension, then a counter.
fn fresh_entry(name: &str, hash: &str, attempt: u32) -> String {
    let (dir, file) = name.rsplit_once('/').map_or(("", name), |(d, f)| (d, f));
    let (stem, ext) = file.rsplit_once('.').map_or((file, ""), |(s, e)| (s, e));
    let prefix = hash.get(..16).unwrap_or(hash);
    let counter = if attempt == 0 { String::new() } else { format!("-{attempt}") };
    let dot = if ext.is_empty() { "" } else { "." };
    let slash = if dir.is_empty() { "" } else { "/" };
    format!("{dir}{slash}{stem}-{prefix}{counter}{dot}{ext}")
}

/// Rewrites the typed entry references [`entries`] finds.
fn rewrite_entries(v: &mut Value, map: &BTreeMap<String, String>, depth: usize) {
    if depth > 200 {
        return;
    }
    match v {
        Value::Object(m) => {
            for (k, x) in m.iter_mut() {
                if k == "meta" || k == "ext" {
                    continue;
                }
                let typed = matches!(k.as_str(), "entry" | "image");
                match x {
                    Value::String(s) if typed => {
                        if let Some(to) = map.get(s.as_str()) {
                            *s = to.clone();
                        }
                    }
                    _ => rewrite_entries(x, map, depth + 1),
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| rewrite_entries(x, map, depth + 1)),
        _ => {}
    }
}

/// How a new copy is named when its name is taken in the destination bank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Naming {
    /// 04 §14.2: the next integer after the highest numeric name.
    NextNumeric,
    /// 01 paste: `name-2`, `name-3`, ….
    Dashed,
}

/// How [`Transfer::place`] treats a source drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Policy {
    /// A source already in the destination bank is used as it is (paste within an element or clone group; move and
    /// link).
    pub reuse: bool,
    /// Always a new copy under a new name (paste as new drawings).
    pub fresh: bool,
    /// A same-named destination drawing is overwritten with the full source drawing (otherwise the copy gets a free
    /// name).
    pub overwrite: bool,
}

/// Places source drawings into one destination bank (see the module documentation). One `Transfer` per destination
/// bank for a whole command, so a drawing copied for one column is reused by the next (C31-R1-07).
pub(crate) struct Transfer {
    pub bank: Id,
    names: Vec<String>,
    map: BTreeMap<Id, Id>,
    pub created: Vec<String>,
    pub modified: Vec<String>,
    naming: Naming,
}

impl Transfer {
    pub(crate) fn new(doc: &Document, bank: Id, naming: Naming) -> Self {
        let names = super::timing::bank_names(doc, &bank);
        Self { bank, names, map: BTreeMap::new(), created: Vec::new(), modified: Vec::new(), naming }
    }

    fn next_name(&self, want: &str, fresh: bool) -> String {
        match self.naming {
            Naming::NextNumeric if fresh || self.names.iter().any(|n| n == want) => {
                x::next_drawing_name(self.names.iter().map(String::as_str))
            }
            Naming::NextNumeric => want.to_owned(),
            Naming::Dashed => {
                let taken: BTreeSet<&str> = self.names.iter().map(String::as_str).collect();
                x::plan_names(want, "", "-", 2, &|n| taken.contains(n), 1, false).concat()
            }
        }
    }

    /// The destination drawing for source drawing `d` under `policy`.
    pub(crate) fn place(
        &mut self,
        cx: &mut Cx,
        doc: &Document,
        payload: &ArtPayload,
        changes: &mut Changes,
        d: &Id,
        policy: Policy,
    ) -> Result<Id, CommandError> {
        let Policy { reuse, fresh, overwrite } = policy;
        if reuse && !fresh && doc.bank_of(d) == Some(&self.bank) {
            return Ok(d.clone());
        }
        if let Some(m) = self.map.get(d) {
            return Ok(m.clone());
        }
        let json = payload.drawing(doc, d).ok_or_else(|| {
            CommandError::with_details(
                "cell.clipboardEmpty",
                "a copied drawing is neither in the copy nor in this project",
                json!({ "drawingId": d.as_str() }),
            )
        })?;
        let json = payload.place_blobs(cx, &json);
        let name = json.get("name").and_then(Value::as_str).unwrap_or("1").to_owned();
        let existing = super::timing::drawing_named(doc, &self.bank, &name);
        let target = match existing {
            Some(t) if !fresh && overwrite => {
                if doc.drawing(&t).is_some_and(|x| *x.locked()) {
                    return Err(locked(doc, &t));
                }
                // The destination keeps its identity, creation time and revision; everything else is the source's.
                let mut v = json.clone();
                v["id"] = json!(t.as_str());
                v["name"] = json!(name);
                v["locked"] = json!(false);
                if let Some(old) = doc.drawing(&t) {
                    v["createdAt"] = to_json(old.created_at())?;
                    v["rev"] = json!(old.rev());
                }
                changes.push(EntryKey::Drawing(t.clone()), vec![op("replace", "", Some(v))]);
                cx.keep_verbatim([t.clone()]);
                self.modified.push(t.as_str().to_owned());
                t
            }
            _ => {
                let name = self.next_name(&name, fresh);
                let id = add_drawing(cx, changes, &self.bank, &name, Some(&json))?;
                self.names.push(name);
                self.created.push(id.as_str().to_owned());
                id
            }
        };
        self.map.insert(d.clone(), target.clone());
        Ok(target)
    }
}

/// Copies of a payload bank as a new bank of the destination project (a whole-element paste whose source bank is
/// not there): `(new bank ID, old drawing ID → new drawing ID)`.
pub(crate) fn new_bank_from(
    cx: &mut Cx,
    payload: &ArtPayload,
    changes: &mut Changes,
    old_bank: &str,
) -> Result<(Id, BTreeMap<String, String>), CommandError> {
    let bank = cx.new_id(tt_model::ObjectType::DrawingBank)?;
    changes.push(
        EntryKey::Project,
        vec![op("add", "/drawingBanks/-", Some(json!({ "id": bank.as_str(), "type": "drawingBank" })))],
    );
    changes.push(
        EntryKey::Bank(bank.clone()),
        vec![op("add", "", Some(to_json(&tt_model::entities::drawing::DrawingBank::new(bank.clone(), Vec::new()))?))],
    );
    let mut map = BTreeMap::new();
    for old in payload.banks.get(old_bank).cloned().unwrap_or_default() {
        let Some(v) = payload.drawings.get(&old) else { continue };
        let v = &payload.place_blobs(cx, v);
        let name = v.get("name").and_then(Value::as_str).unwrap_or("1").to_owned();
        let locked_flag = v.get("locked").and_then(Value::as_bool).unwrap_or(false);
        let id = add_drawing(cx, changes, &bank, &name, Some(v))?;
        if locked_flag {
            changes.push(EntryKey::Drawing(id.clone()), vec![op("replace", "/locked", Some(json!(true)))]);
        }
        map.insert(old, id.as_str().to_owned());
    }
    Ok((bank, map))
}
