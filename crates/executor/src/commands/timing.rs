//! Shared machinery of the element, cell and exposure-sheet commands (04 §3–§17; M3 C3.1): target resolution with
//! the 04 §17.6 error codes, the interned frame arrays the exposure algebra of [`tt_anim::exposure`] runs on, the
//! timing work account, name rules, and the change events of 04 §17.4.
//!
//! **Work.** Every element, cell and exposure-sheet command charges one [`Budget`] (units of about one cell, frame or
//! tree node read or written) and is refused with `limit.exceeded` before the step that would pass
//! [`TIMING_WORK_BUDGET`], so the document worker is never held past the watchdog and nothing is staged; the same
//! account observes the watchdog's per-request cancellation (ADR-0015). A result longer than the 100 000-frame scene
//! limit is refused with `doc.limitExceeded` (10 §5) before anything is staged.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_anim::exposure as x;
use tt_model::change::PatchOp;
use tt_model::document::{Document, EntryKey};
use tt_model::entities::element::{Element, ElementType};
use tt_model::entities::scene::{Scene, TreeNode};
use tt_model::{Id, ObjectType};
use tt_registry::{CommandError, codes};
use tt_vector::work::Work;

use crate::app::{cell_ref, parse_cell_ref};
use crate::cx::Cx;

/// The work one element, cell or exposure-sheet command may do, in units of about one cell, frame or tree node read
/// or written. A 100 000-frame column costs 100 000 units per pass; the accepted worst cases (insert cells in all
/// 1 000 columns of 100 000 frames, a 999-fold cycle, 10 000-drawing banks) and their times are recorded by
/// `crates/executor/tests/c31_budgets.rs`.
pub(crate) const TIMING_WORK_BUDGET: u64 = 300_000_000;

/// A work account of one command.
pub(crate) struct Budget(Work);

impl Budget {
    pub(crate) fn new(what: &'static str) -> Self {
        Self(Work::new(TIMING_WORK_BUDGET, what))
    }

    /// Charges `n` units.
    pub(crate) fn spend(&mut self, n: usize) -> Result<(), CommandError> {
        self.0.spend(x::cost(n)).map_err(|e| {
            let error = CommandError::with_details(e.code, e.message, json!({ "budget": TIMING_WORK_BUDGET }));
            if e.code == codes::COMMAND_TIMEOUT {
                error.hint("the caller's watchdog budget passed: the call was cancelled and changed nothing")
            } else {
                error.hint("act on fewer elements or a shorter frame range per call")
            }
        })
    }
}

pub(crate) fn op(kind: &'static str, path: &str, value: Option<Value>) -> PatchOp {
    PatchOp { op: kind, from: None, path: path.into(), range: None, value, old_value: None, elided: false }
}

pub(crate) fn err(code: &str, message: impl Into<String>, details: Value) -> CommandError {
    CommandError::with_details(code, message, details)
}

pub(crate) fn internal(e: impl std::fmt::Display) -> CommandError {
    CommandError::internal(e.to_string())
}

pub(crate) fn to_json<T: Serialize>(v: &T) -> Result<Value, CommandError> {
    serde_json::to_value(v).map_err(internal)
}

// ----- references ---------------------------------------------------------------------------------------------------

/// Resolves an element reference; an unknown one is `element.notFound` (04 §17.6).
pub(crate) fn element_id(cx: &Cx, r: &str, pointer: &str) -> Result<Id, CommandError> {
    let key = cx.resolve_as(r, ObjectType::Element).map_err(|e| renamed(e, "element.notFound", r, pointer))?;
    Id::parse(&key).map_err(|e| err("element.notFound", e.to_string(), json!({ "ref": r, "pointer": pointer })))
}

/// Resolves a drawing reference; an unknown one is `drawing.notFound` (04 §17.6).
pub(crate) fn drawing_id(cx: &Cx, r: &str, pointer: &str) -> Result<Id, CommandError> {
    let key = cx.resolve_as(r, ObjectType::Drawing).map_err(|e| renamed(e, "drawing.notFound", r, pointer))?;
    Id::parse(&key).map_err(|e| err("drawing.notFound", e.to_string(), json!({ "ref": r, "pointer": pointer })))
}

/// Resolves a scene reference, or the current scene.
pub(crate) fn scene_id(cx: &Cx, r: Option<&str>) -> Result<Id, CommandError> {
    match r {
        Some(r) => Ok(Id::parse(&cx.resolve_as(r, ObjectType::Scene)?)?),
        None => cx.app().scene.clone().ok_or_else(|| {
            err(
                codes::COMMAND_DISABLED,
                "there is no current scene and no sceneId was given",
                json!({ "reason": "noScene" }),
            )
            .hint("pass sceneId, or make a scene current (app.context.set)")
        }),
    }
}

fn renamed(e: CommandError, code: &str, r: &str, pointer: &str) -> CommandError {
    if e.code == tt_model::codes::REF_NOT_FOUND {
        let mut details = (*e.details).clone();
        if let Value::Object(m) = &mut details {
            m.insert("ref".into(), json!(r));
            m.insert("pointer".into(), json!(pointer));
        }
        CommandError::with_details(code, e.message, details)
    } else {
        e
    }
}

/// The element (it must exist: resolved IDs only).
pub(crate) fn element<'a>(doc: &'a Document, id: &Id) -> Result<&'a Element, CommandError> {
    doc.element(id).ok_or_else(|| {
        err("element.notFound", format!("element `{id}` does not exist"), json!({ "elementId": id.as_str() }))
    })
}

/// The scene of an element.
pub(crate) fn scene_of<'a>(doc: &'a Document, id: &Id) -> Result<&'a Scene, CommandError> {
    doc.scene_of(id).and_then(|s| doc.scene(s)).ok_or_else(|| internal("element without a scene"))
}

// ----- frames -------------------------------------------------------------------------------------------------------

/// The exposure of a cell-based element as a frame array of indices into `table` (the drawing IDs it exposes).
#[derive(Debug, Clone)]
pub(crate) struct Column {
    pub id: Id,
    pub table: Vec<Id>,
    index: BTreeMap<Id, u32>,
    pub frames: Vec<Option<u32>>,
}

impl Column {
    /// Reads an element's exposure, charging its frame count.
    pub(crate) fn read(doc: &Document, id: &Id, budget: &mut Budget) -> Result<Column, CommandError> {
        let e = element(doc, id)?;
        let runs = e.exposure().clone().unwrap_or_default();
        budget.spend(x::runs_len(&runs))?;
        let mut c = Column { id: id.clone(), table: Vec::new(), index: BTreeMap::new(), frames: Vec::new() };
        let interned: Vec<(Option<u32>, u32)> =
            runs.iter().map(|(d, n)| (d.as_ref().map(|d| c.intern(d)), *n)).collect();
        c.frames = x::expand(&interned);
        Ok(c)
    }

    /// The index of a drawing, adding it to the table.
    pub(crate) fn intern(&mut self, d: &Id) -> u32 {
        if let Some(i) = self.index.get(d) {
            return *i;
        }
        let i = u32::try_from(self.table.len()).unwrap_or(u32::MAX);
        self.table.push(d.clone());
        self.index.insert(d.clone(), i);
        i
    }

    /// The drawing of an index.
    pub(crate) fn drawing(&self, i: u32) -> Option<&Id> {
        self.table.get(i as usize)
    }

    /// The canonical runs of a frame array as stored JSON (10 §4.7).
    pub(crate) fn runs_json(&self, frames: &[Option<u32>]) -> Value {
        Value::Array(
            x::compress(frames)
                .into_iter()
                .map(|(d, n)| json!([d.and_then(|i| self.drawing(i)).map(Id::as_str), n]))
                .collect(),
        )
    }
}

/// Refuses frame arrays longer than the scene limit (10 §5).
pub(crate) fn check_length(len: usize, element: &Id) -> Result<(), CommandError> {
    if len > x::MAX_FRAMES {
        return Err(err(
            "doc.limitExceeded",
            format!(
                "the change would make element `{element}` {len} frames long; a scene has at most {} frames",
                x::MAX_FRAMES
            ),
            json!({ "elementId": element.as_str(), "frames": len, "limit": x::MAX_FRAMES }),
        ));
    }
    Ok(())
}

/// Checks a frame position (1 … 100 000; 04 §17.6 `cell.frameOutOfRange`).
pub(crate) fn frame(v: i64, pointer: &str) -> Result<usize, CommandError> {
    match usize::try_from(v) {
        Ok(f) if (1..=x::MAX_FRAMES).contains(&f) => Ok(f),
        _ => Err(err(
            "cell.frameOutOfRange",
            format!("frame {v} is outside 1…{}", x::MAX_FRAMES),
            json!({ "pointer": pointer, "frame": v, "minimum": 1, "maximum": x::MAX_FRAMES }),
        )),
    }
}

/// A change of one element's cells: `{elementId, from, to}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CellChange {
    /// The element.
    pub element_id: String,
    /// First changed frame.
    pub from: u32,
    /// Last changed frame.
    pub to: u32,
}

/// Stages a column's new frames when they differ; records the change and its event. Returns the changed window.
pub(crate) fn write_column(
    col: &Column,
    after: &[Option<u32>],
    changes: &mut Changes,
    out: &mut Vec<CellChange>,
) -> Result<Option<x::Window>, CommandError> {
    check_length(after.len(), &col.id)?;
    let Some((from, to)) = x::diff(&col.frames, after) else { return Ok(None) };
    changes.push(EntryKey::Element(col.id.clone()), vec![op("replace", "/exposure", Some(col.runs_json(after)))]);
    out.push(CellChange { element_id: col.id.as_str().to_owned(), from: as_u32(from), to: as_u32(to) });
    Ok(Some((from, to)))
}

pub(crate) fn as_u32(v: usize) -> u32 {
    u32::try_from(v).unwrap_or(u32::MAX)
}

/// Emits `cell.exposures.changed` once per changed element (coalesced range) after the commit.
pub(crate) fn emit_cells(cx: &mut Cx, changed: &[CellChange]) -> Result<(), CommandError> {
    if changed.is_empty() {
        return Ok(());
    }
    let rev = cx.predicted_rev()?;
    let doc = cx.document()?.clone();
    let mut by: BTreeMap<&str, (u32, u32)> = BTreeMap::new();
    for c in changed {
        let e = by.entry(c.element_id.as_str()).or_insert((c.from, c.to));
        e.0 = e.0.min(c.from);
        e.1 = e.1.max(c.to);
    }
    for (e, (from, to)) in by {
        let scene = Id::parse(e).ok().and_then(|id| doc.scene_of(&id).cloned()).map(|s| s.as_str().to_owned());
        cx.emit(
            "cell.exposures.changed",
            json!({ "sceneId": scene, "elementId": e, "from": from, "to": to, "rev": rev }),
        );
    }
    Ok(())
}

/// Emits `element.state.changed`.
pub(crate) fn emit_state(cx: &mut Cx, scene: &Id, ids: &[String], change: &str) {
    if !ids.is_empty() {
        cx.emit("element.state.changed", json!({ "sceneId": scene.as_str(), "elementIds": ids, "change": change }));
    }
}

/// Emits `element.drawing.changed` per element of the bank's clone group.
pub(crate) fn emit_bank(cx: &mut Cx, doc: &Document, bank: &Id, drawings: &[String], change: &str) {
    if drawings.is_empty() {
        return;
    }
    for e in doc.bank_users(bank) {
        cx.emit(
            "element.drawing.changed",
            json!({ "elementId": e.as_str(), "drawingIds": drawings, "change": change }),
        );
    }
}

// ----- cell targets (04 §17.1 item 2) ---------------------------------------------------------------------------------

/// The cells a cell command acts on: elements and a frame window.
#[derive(Debug, Clone)]
pub(crate) struct CellTarget {
    pub elements: Vec<Id>,
    pub from: usize,
    pub to: usize,
}

/// The cell selection (`xsheet` context; shared with the Timeline): elements in selection order and frame bounds.
pub(crate) fn selected_cells(cx: &Cx) -> Option<(Vec<Id>, usize, usize)> {
    let refs = cx.app().selection_of("xsheet");
    let mut elements: Vec<Id> = Vec::new();
    let (mut lo, mut hi) = (usize::MAX, 0usize);
    for r in &refs {
        if let Some((e, f)) = parse_cell_ref(r) {
            let Ok(id) = Id::parse(&e) else { continue };
            if !elements.contains(&id) {
                elements.push(id);
            }
            lo = lo.min(f as usize);
            hi = hi.max(f as usize);
        }
    }
    (!elements.is_empty()).then_some((elements, lo, hi))
}

/// Resolves the target of a cell command: explicit `elementIds`/`from`/`to` (or `frame`), else the cell selection,
/// else the current element and frame; `cell.noTarget` when none exists. Explicit targets never touch the selection.
pub(crate) fn cell_target(
    cx: &Cx,
    elements: Option<&[String]>,
    from: Option<i64>,
    to: Option<i64>,
) -> Result<CellTarget, CommandError> {
    let selection = selected_cells(cx);
    let ids: Vec<Id> = match elements {
        Some(list) => {
            if list.is_empty() {
                return Err(no_target());
            }
            let mut out = Vec::new();
            for (i, r) in list.iter().enumerate() {
                let id = element_id(cx, r, &format!("/elementIds/{i}"))?;
                if !out.contains(&id) {
                    out.push(id);
                }
            }
            out
        }
        None => match (&selection, cx.app().element.clone()) {
            (Some((e, _, _)), _) => e.clone(),
            (None, Some(e)) => vec![e],
            (None, None) => return Err(no_target()),
        },
    };
    let explicit = elements.is_some() || selection.is_none();
    let (sel_from, sel_to) = match (&selection, explicit) {
        (Some((_, a, b)), false) => (*a, *b),
        _ => {
            let f = (cx.app().frame as usize).max(1);
            (f, f)
        }
    };
    let from = match from {
        Some(v) => frame(v, "/from")?,
        None => sel_from,
    };
    let to = match to {
        Some(v) => frame(v, "/to")?,
        None if explicit => from,
        None => sel_to.max(from),
    };
    if to < from {
        return Err(err(
            codes::ARG_INVALID,
            format!("`to` ({to}) is before `from` ({from})"),
            json!({ "pointer": "/to", "from": from, "to": to }),
        ));
    }
    Ok(CellTarget { elements: ids, from, to })
}

pub(crate) fn no_target() -> CommandError {
    err("cell.noTarget", "no element and frame range was given and nothing is selected", json!({}))
        .hint("pass elementIds (or elementId) with from/to or frame, or select cells (cell.select)")
}

/// The availability rule of 04 §13.20: the element must be one of the listed types, else `cell.notApplicable`.
/// Media and sound columns (whose cells are sequences and placements, 10 §4.14–§4.15) are refused with their owner
/// (SI-088).
pub(crate) fn applicable(doc: &Document, id: &Id, command: &str, allowed: &[ElementType]) -> Result<(), CommandError> {
    let e = element(doc, id)?;
    let t = *e.element_type();
    if allowed.contains(&t) && t.has_exposure() {
        return Ok(());
    }
    let owner = match t {
        ElementType::Media if allowed.contains(&t) => Some("M8 C8.3"),
        ElementType::Sound if allowed.contains(&t) => Some("M8 C8.1"),
        _ => None,
    };
    let mut details = json!({ "elementId": id.as_str(), "elementType": t.as_str(), "commandId": command });
    let mut message = format!("`{command}` is not available for {} element `{}` (04 §13.20)", t.as_str(), e.name());
    if let (Some(o), Value::Object(m)) = (owner, &mut details) {
        m.insert("owner".into(), json!(o));
        m.insert("specIssue".into(), json!("SI-088"));
        message = format!("{message}; {} cells are edited from {o}", t.as_str());
    }
    Err(err("cell.notApplicable", message, details))
}

/// Drawing, image and annotation elements (and media/sound, refused with their owner).
pub(crate) const EXPOSED: &[ElementType] =
    &[ElementType::Drawing, ElementType::Image, ElementType::Annotation, ElementType::Media];
/// Drawing and annotation elements (vector).
pub(crate) const VECTOR: &[ElementType] = &[ElementType::Drawing, ElementType::Annotation];
/// Drawing, image and annotation elements.
pub(crate) const LABELLED: &[ElementType] = &[ElementType::Drawing, ElementType::Image, ElementType::Annotation];

// ----- names (04 §4.2, 10 §1.3) -------------------------------------------------------------------------------------

/// Validates an element name: `element.nameEmpty`, `.nameInvalid`, `.nameTooLong` (uniqueness is the caller's).
pub(crate) fn element_name(input: &str, pointer: &str) -> Result<String, CommandError> {
    checked_name(input, pointer, ObjectType::Element, "element")
}

/// Validates a drawing name: `drawing.nameEmpty`, `.nameInvalid`, `.nameTooLong`.
pub(crate) fn drawing_name(input: &str, pointer: &str) -> Result<String, CommandError> {
    checked_name(input, pointer, ObjectType::Drawing, "drawing")
}

fn checked_name(input: &str, pointer: &str, ty: ObjectType, area: &str) -> Result<String, CommandError> {
    let n = tt_model::name::normalize(input);
    match tt_model::name::check(&tt_model::name::NameRule::for_type(ty), &n) {
        Ok(()) => Ok(n),
        Err(e) => {
            let rule = e.details().get("rule").and_then(Value::as_str).unwrap_or("forbidden").to_owned();
            let code = match rule.as_str() {
                "empty" => "nameEmpty",
                "tooLong" => "nameTooLong",
                _ => "nameInvalid",
            };
            Err(err(
                &format!("{area}.{code}"),
                format!(
                    "{}: names must not be empty, contain any of \\ / : * ? < > \" | . or exceed 64 characters",
                    e.message()
                ),
                json!({ "pointer": pointer, "name": input, "rule": rule, "forbidden": "\\ / : * ? < > \" | .", "maxLength": 64 }),
            ))
        }
    }
}

/// The element names used in a scene.
pub(crate) fn scene_names(doc: &Document, scene: &Scene) -> BTreeSet<String> {
    scene.elements().ids().iter().filter_map(|e| doc.element(e)).map(|e| e.name().clone()).collect()
}

/// `element.nameDuplicate`.
pub(crate) fn duplicate_name(name: &str, pointer: &str) -> CommandError {
    err(
        "element.nameDuplicate",
        format!("an element of this scene is already named `{name}`"),
        json!({ "pointer": pointer, "name": name }),
    )
    .hint("choose another name, or omit it for a unique default")
}

/// The drawing names of a bank.
pub(crate) fn bank_names(doc: &Document, bank: &Id) -> Vec<String> {
    doc.bank(bank)
        .map(|b| b.drawings().ids().iter().filter_map(|d| doc.drawing(d)).map(|d| d.name().clone()).collect())
        .unwrap_or_default()
}

/// The drawings of a bank in natural name order (bank order, 10 §4.8), ties by ID.
pub(crate) fn bank_order(doc: &Document, bank: &Id) -> Vec<(Id, String)> {
    let mut v: Vec<(Id, String)> = doc
        .bank(bank)
        .map(|b| {
            b.drawings()
                .ids()
                .iter()
                .filter_map(|d| doc.drawing(d))
                .map(|d| (d.id().clone(), d.name().clone()))
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| x::natural_cmp(&a.1, &b.1).then_with(|| a.0.cmp(&b.0)));
    v
}

// ----- hierarchy ------------------------------------------------------------------------------------------------------

/// The stored JSON of a tree.
pub(crate) fn tree_json(t: &[TreeNode]) -> Result<Value, CommandError> {
    to_json(&t)
}

/// The category of an element type (04 §5.1).
pub(crate) const fn category(t: ElementType) -> &'static str {
    match t {
        ElementType::Drawing | ElementType::Annotation => "drawings",
        ElementType::Image => "images",
        ElementType::Sound => "sounds",
        ElementType::Peg => "pegs",
        ElementType::Skeleton | ElementType::Bone => "skeletons",
        ElementType::Media => "media",
        ElementType::Camera => "cameras",
        ElementType::EffectParameter => "effectParameters",
        _ => "effects",
    }
}

/// The recommended category colours (04 §5.1), opaque.
pub(crate) const CATEGORY_COLOURS: [(&str, &str); 9] = [
    ("drawings", "#9DC3E6FF"),
    ("images", "#A9D18EFF"),
    ("sounds", "#F4B183FF"),
    ("pegs", "#C9A0DCFF"),
    ("skeletons", "#E6B9B8FF"),
    ("media", "#FFE699FF"),
    ("cameras", "#BFBFBFFF"),
    ("effects", "#F8CBADFF"),
    ("effectParameters", "#FBE5D6FF"),
];

/// Whether an element type may parent others (04 §9.2: pegs, effects, skeletons and bones, cameras).
pub(crate) const fn parentable(t: ElementType) -> bool {
    matches!(
        t,
        ElementType::Peg
            | ElementType::Camera
            | ElementType::Skeleton
            | ElementType::Bone
            | ElementType::EffectParameter
    ) || t.is_effect()
}

/// Cell references of an element over a frame window (selection entries).
pub(crate) fn cell_refs(elements: &[Id], from: usize, to: usize) -> Vec<String> {
    elements.iter().flat_map(|e| (from..=to).map(move |f| cell_ref(e.as_str(), as_u32(f)))).collect()
}

// ----- staged changes -------------------------------------------------------------------------------------------------

/// Entry patches of one command, ops of the same entry kept together in order.
#[derive(Debug, Default)]
pub(crate) struct Changes(Vec<(EntryKey, Vec<PatchOp>)>);

impl Changes {
    pub(crate) fn push(&mut self, key: EntryKey, ops: Vec<PatchOp>) {
        if ops.is_empty() {
            return;
        }
        match self.0.iter_mut().find(|(k, _)| *k == key) {
            Some((_, v)) => v.extend(ops),
            None => self.0.push((key, ops)),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Stages every change (nothing when empty).
    pub(crate) fn stage(self, cx: &mut Cx) -> Result<(), CommandError> {
        if self.0.is_empty() {
            return Ok(());
        }
        cx.patch_entries(self.0)
    }
}

/// A new drawing in a bank: an empty one, or a copy of `source` (its JSON, with drawing-local node and stroke IDs
/// kept verbatim so zone-owner resolution and fill references survive the copy, C24-T1). Returns its ID.
pub(crate) fn add_drawing(
    cx: &mut Cx,
    changes: &mut Changes,
    bank: &Id,
    name: &str,
    source: Option<&Value>,
) -> Result<Id, CommandError> {
    let id = cx.new_id(ObjectType::Drawing)?;
    let mut v = match source {
        Some(s) => s.clone(),
        None => to_json(&tt_model::entities::drawing::Drawing::empty(
            id.clone(),
            name.to_owned(),
            tt_model::value::Timestamp::now(),
        ))?,
    };
    v["id"] = json!(id.as_str());
    v["rev"] = json!(0);
    v["name"] = json!(name);
    if source.is_some() {
        v["locked"] = json!(false);
        v["createdAt"] = to_json(&tt_model::value::Timestamp::now())?;
        // A copy is persisted content, taken as it is: the settle pass never re-nodes or splits its strokes (C24-T1:
        // owner resolution, stretch fills and brush outlines survive the copy).
        cx.keep_verbatim([id.clone()]);
    }
    changes.push(EntryKey::Drawing(id.clone()), vec![op("add", "", Some(v))]);
    changes.push(
        EntryKey::Bank(bank.clone()),
        vec![op("add", "/drawings/-", Some(json!({ "id": id.as_str(), "type": "drawing" })))],
    );
    Ok(id)
}

/// The drawing bank of a cell-based element (`element.notCellBased` otherwise).
pub(crate) fn bank_of_element(doc: &Document, id: &Id) -> Result<Id, CommandError> {
    let e = element(doc, id)?;
    e.drawing_bank_id().clone().ok_or_else(|| {
        err(
            "element.notCellBased",
            format!("element `{}` ({}) has no drawing bank", e.name(), e.element_type().as_str()),
            json!({ "elementId": id.as_str(), "elementType": e.element_type().as_str() }),
        )
    })
}

/// `drawing.locked`.
pub(crate) fn locked(doc: &Document, d: &Id) -> CommandError {
    let name = doc.drawing(d).map(|x| x.name().clone()).unwrap_or_default();
    err("drawing.locked", format!("drawing `{name}` is locked"), json!({ "drawingId": d.as_str(), "name": name }))
        .hint("unlock it first (element.drawingLock.set)")
}

/// The drawing of a bank with a name.
pub(crate) fn drawing_named(doc: &Document, bank: &Id, name: &str) -> Option<Id> {
    doc.bank(bank)?.drawings().ids().iter().find(|d| doc.drawing(d).is_some_and(|x| x.name() == name)).cloned()
}

/// `cell.drawingNotFound`.
pub(crate) fn drawing_not_found(name: &str, element: &Id, pointer: &str) -> CommandError {
    err(
        "cell.drawingNotFound",
        format!("the element's drawing bank has no drawing `{name}`"),
        json!({ "pointer": pointer, "drawing": name, "elementId": element.as_str() }),
    )
}

// ----- view state -------------------------------------------------------------------------------------------------------

/// One static light-table item (04 §15.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LightItem {
    /// Item ID.
    pub item_id: String,
    /// The source element.
    pub element_id: String,
    /// The drawing.
    pub drawing_id: String,
    /// Shown.
    pub visible: bool,
    /// `over` or `under` the Drawing View drawings.
    pub placement: String,
}

/// Timeline and Exposure Sheet view state of the session (never document content, never undoable).
#[derive(Debug, Clone, Default)]
pub(crate) struct TimelineState {
    /// Solo mode: the soloed elements while on (04 §10.3).
    pub solo: Option<Vec<Id>>,
    /// Category colours set by `element.typeColour.set` (preference; 04 §5.1), by category.
    pub type_colours: BTreeMap<String, String>,
    /// Static light tables by scene ID, in stacking order (04 §15; stored per scene with the session state, SI-085).
    pub light_tables: BTreeMap<String, Vec<LightItem>>,
}
