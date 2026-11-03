//! Cell and exposure commands (04 §13, §17.4; M3 C3.1): `cell.exposure.*`, `cell.fill.apply`, `cell.blank.*`,
//! `cell.range.*`, `cell.label.set`, `cell.drawing.duplicate`, `cell.cycle.create`, `cell.columnate.apply`,
//! `cell.note.set`, `cell.clipboard.*`, `cell.select`, `cell.selection.get` and the `cell.*` events.
//!
//! Every command acts on an explicit target (`elementIds`/`elementId` with `from`/`to` or `frame`) or, when omitted,
//! on the shared cell selection and the current position (04 §17.1 item 2); an explicit target never changes the
//! selection or the current frame. Each invocation is one undo step (04 §13), applied to each element independently,
//! computed with the exposure algebra of [`tt_anim::exposure`] on interned frame arrays, charged to the timing work
//! account and staged through the executor. A failed command changes nothing (04 §17.1 item 11).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tt_anim::exposure::{self as x, FillError, LoopMode};
use tt_model::document::{Document, EntryKey};
use tt_model::entities::element::ElementType;
use tt_model::Id;
use tt_registry::{CommandError, codes, command};

use super::common::NoParams;
use super::timing::{
    Budget, CellChange, CellTarget, Changes, Column, EXPOSED, LABELLED, VECTOR, add_drawing, applicable, as_u32,
    bank_of_element, bank_order, cell_refs, cell_target, drawing_id, drawing_name, drawing_not_found, element,
    element_id, emit_bank, emit_cells, err, frame, locked, no_target, op, selected_cells, write_column,
};
use crate::app::parse_cell_ref;
use crate::cx::Cx;

fn one() -> u32 {
    1
}

/// `{changed: [{elementId, from, to}]}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChangedResult {
    /// The changed frame window of each changed element (unchanged elements are omitted).
    changed: Vec<CellChange>,
}

/// Runs a per-element exposure edit on every target element and stages it as one step.
fn per_element(
    cx: &mut Cx,
    t: &CellTarget,
    command: &str,
    allowed: &[ElementType],
    budget: &mut Budget,
    f: &mut dyn FnMut(&Column, &mut Budget) -> Result<Vec<Option<u32>>, CommandError>,
) -> Result<Vec<CellChange>, CommandError> {
    let doc = cx.document()?.clone();
    let mut changes = Changes::default();
    let mut out = Vec::new();
    for e in &t.elements {
        applicable(&doc, e, command, allowed)?;
        let col = Column::read(&doc, e, budget)?;
        let after = f(&col, budget)?;
        budget.spend(after.len())?;
        write_column(&col, &after, &mut changes, &mut out)?;
    }
    changes.stage(cx)?;
    emit_cells(cx, &out)?;
    Ok(out)
}

/// Lengthens or shortens a peg's bar (04 §13.1–§13.2 on a peg).
fn peg_bar(cx: &mut Cx, doc: &Document, id: &Id, delta: i64, changes: &mut Changes) -> Result<(), CommandError> {
    let e = element(doc, id)?;
    let d = i64::from(e.duration().unwrap_or(1));
    let start = i64::from(e.start().unwrap_or(1));
    let nd = (d + delta).max(1);
    super::timing::check_length(usize::try_from(start + nd - 1).unwrap_or(usize::MAX), id)?;
    if nd != d {
        changes.push(EntryKey::Element(id.clone()), vec![op("replace", "/duration", Some(json!(nd)))]);
        cx.emit("element.state.changed", json!({ "sceneId": doc.scene_of(id).map(Id::as_str), "elementIds": [id.as_str()], "change": "timing" }));
    }
    Ok(())
}

// ----- cell.exposure.add / .remove / .set ------------------------------------------------------------------------------

/// `cell.exposure.add` / `cell.exposure.remove` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ExposureStepParams {
    /// Target elements (default: the selection, else the current element).
    #[serde(default)]
    element_ids: Option<Vec<String>>,
    /// First frame of the range (default: the selection, else the current frame).
    #[serde(default)]
    from: Option<i64>,
    /// Last frame of the range.
    #[serde(default)]
    to: Option<i64>,
    /// Frames to add or remove per exposure.
    #[serde(default = "one")]
    #[schemars(range(min = 1, max = 9999))]
    frames: u32,
}

fn exposure_step(cx: &mut Cx, p: &ExposureStepParams, add: bool) -> Result<ChangedResult, CommandError> {
    let command = if add { "cell.exposure.add" } else { "cell.exposure.remove" };
    let t = cell_target(cx, p.element_ids.as_deref(), p.from, p.to)?;
    let doc = cx.document()?.clone();
    let mut budget = Budget::new("changing exposures");
    // Pegs: the bar length (04 §13.20).
    let (pegs, cells): (Vec<Id>, Vec<Id>) = t
        .elements
        .iter()
        .cloned()
        .partition(|e| doc.element(e).is_some_and(|x| *x.element_type() == ElementType::Peg));
    let mut changes = Changes::default();
    let delta = if add { i64::from(p.frames) } else { -i64::from(p.frames) };
    for peg in &pegs {
        budget.spend(1)?;
        peg_bar(cx, &doc, peg, delta, &mut changes)?;
    }
    let mut out = Vec::new();
    let n = p.frames as usize;
    for e in &cells {
        applicable(&doc, e, command, EXPOSED)?;
        let col = Column::read(&doc, e, &mut budget)?;
        let after = if add {
            budget.spend(col.frames.len() + n * (t.to - t.from + 1))?;
            x::add_exposure(&col.frames, t.from, t.to, n)
        } else {
            x::remove_exposure(&col.frames, t.from, t.to, n)
        };
        budget.spend(after.len())?;
        write_column(&col, &after, &mut changes, &mut out)?;
    }
    changes.stage(cx)?;
    emit_cells(cx, &out)?;
    Ok(ChangedResult { changed: out })
}

#[command(
    id = "cell.exposure.add",
    kind = action,
    label = "Add exposure",
    scopes = [edit],
    undo = "Add Exposure",
    needs_project,
    example(title = "Hold Bob's drawing at frame 1 one frame longer", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 1, "to": 1}"#),
    example(title = "Add two frames to every exposure of frames 1-10", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 1, "to": 10, "frames": 2}"#),
    gui = "Exposure Sheet and Timeline cell context: Add Exposure (M5 C5.2)",
)]
pub(crate) fn cell_exposure_add(cx: &mut Cx, p: ExposureStepParams) -> Result<ChangedResult, CommandError> {
    exposure_step(cx, &p, true)
}

#[command(
    id = "cell.exposure.remove",
    kind = action,
    label = "Remove exposure",
    scopes = [edit],
    undo = "Remove Exposure",
    needs_project,
    example(title = "Shorten the exposure at frame 9 by one frame", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 9, "to": 9}"#),
    gui = "Exposure Sheet and Timeline cell context: Remove Exposure (M5 C5.2)",
)]
pub(crate) fn cell_exposure_remove(cx: &mut Cx, p: ExposureStepParams) -> Result<ChangedResult, CommandError> {
    exposure_step(cx, &p, false)
}

/// `cell.exposure.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ExposureSetParams {
    /// Target elements (default: the selection, else the current element).
    #[serde(default)]
    element_ids: Option<Vec<String>>,
    /// First frame of the range.
    #[serde(default)]
    from: Option<i64>,
    /// Last frame of the range.
    #[serde(default)]
    to: Option<i64>,
    /// New length of every exposure intersecting the range (1 = on ones, 2 = on twos, …).
    #[schemars(range(min = 1, max = 9999))]
    length: u32,
}

#[command(
    id = "cell.exposure.set",
    kind = action,
    label = "Set exposure",
    scopes = [edit],
    undo = "Set Exposure",
    needs_project,
    example(title = "Put Bob's frames 1-20 on twos", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 1, "to": 20, "length": 2}"#),
    gui = "Exposure Sheet and Timeline cell context: Set Exposure to 1/2/3/N (M5 C5.2)",
)]
pub(crate) fn cell_exposure_set(cx: &mut Cx, p: ExposureSetParams) -> Result<ChangedResult, CommandError> {
    let t = cell_target(cx, p.element_ids.as_deref(), p.from, p.to)?;
    let mut budget = Budget::new("setting exposures");
    let len = p.length as usize;
    let span = t.to - t.from + 1;
    let changed = per_element(cx, &t, "cell.exposure.set", EXPOSED, &mut budget, &mut |c, b| {
        b.spend(c.frames.len() + span.saturating_mul(len))?;
        Ok(x::set_exposure(&c.frames, t.from, t.to, len))
    })?;
    Ok(ChangedResult { changed })
}

// ----- cell.fill.apply ------------------------------------------------------------------------------------------------

/// `cell.fill.apply` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct FillParams {
    /// Target elements (default: the selection, else the current element).
    #[serde(default)]
    element_ids: Option<Vec<String>>,
    /// The cell whose drawing is extended (default: the first selected frame, else the current frame).
    #[serde(default)]
    frame: Option<i64>,
    /// Last frame to expose the drawing in (minimum: `frame`; 04 §13.5).
    to_frame: i64,
}

#[command(
    id = "cell.fill.apply",
    kind = action,
    label = "Extend exposure",
    scopes = [edit],
    undo = "Extend Exposure",
    needs_project,
    example(title = "Extend Bob's first drawing through frame 8, over the blank cells", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 1, "toFrame": 8}"#),
    gui = "Exposure Sheet and Timeline cell context: Extend Exposure (M5 C5.2)",
)]
pub(crate) fn cell_fill_apply(cx: &mut Cx, p: FillParams) -> Result<ChangedResult, CommandError> {
    let t = cell_target(cx, p.element_ids.as_deref(), p.frame, p.frame)?;
    let to = usize::try_from(p.to_frame).unwrap_or(0);
    if p.to_frame >= 1 && to < t.from {
        return Err(err(
            "cell.frameBeforeSelection",
            format!("the target frame {to} is before the selection; the minimum is {}", t.from),
            json!({ "pointer": "/toFrame", "toFrame": to, "minimum": t.from }),
        ));
    }
    let to = frame(p.to_frame, "/toFrame")?;
    let mut budget = Budget::new("extending exposures");
    let changed = per_element(cx, &t, "cell.fill.apply", EXPOSED, &mut budget, &mut |c, b| {
        b.spend(c.frames.len().max(to))?;
        x::fill(&c.frames, t.from, to).map_err(|e| match e {
            FillError::Blank => err(
                "cell.notApplicable",
                format!("the cell at frame {} of element `{}` is blank: there is no drawing to extend", t.from, c.id),
                json!({ "elementId": c.id.as_str(), "frame": t.from, "reason": "blank" }),
            ),
            FillError::BeforeSelection { minimum } => err(
                "cell.frameBeforeSelection",
                format!("the target frame is before the selection; the minimum is {minimum}"),
                json!({ "pointer": "/toFrame", "minimum": minimum }),
            ),
        })
    })?;
    Ok(ChangedResult { changed })
}

// ----- cell.blank.add / .remove ----------------------------------------------------------------------------------------

/// `cell.blank.add` / `cell.blank.remove` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct BlankParams {
    /// Target elements (default: the selection, else the current element).
    #[serde(default)]
    element_ids: Option<Vec<String>>,
    /// The frame (default: the first selected frame, else the current frame).
    #[serde(default)]
    frame: Option<i64>,
    /// Number of blank cells.
    #[serde(default = "one")]
    #[schemars(range(min = 1, max = 9999))]
    count: u32,
}

const BLANKS: &[ElementType] =
    &[ElementType::Drawing, ElementType::Image, ElementType::Annotation, ElementType::Media, ElementType::Sound];

#[command(
    id = "cell.blank.add",
    kind = action,
    label = "Insert blank cell",
    scopes = [edit],
    undo = "Insert Blank Cell",
    needs_project,
    example(title = "Insert two blank cells at frame 3 of Bob", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 3, "count": 2}"#),
    gui = "Exposure Sheet and Timeline cell context: Insert Blank Cell (M5 C5.2)",
)]
pub(crate) fn cell_blank_add(cx: &mut Cx, p: BlankParams) -> Result<ChangedResult, CommandError> {
    let t = cell_target(cx, p.element_ids.as_deref(), p.frame, p.frame)?;
    let mut budget = Budget::new("inserting blank cells");
    let n = p.count as usize;
    let changed = per_element(cx, &t, "cell.blank.add", BLANKS, &mut budget, &mut |c, b| {
        b.spend(c.frames.len() + n)?;
        Ok(x::insert_blank(&c.frames, t.from, n))
    })?;
    Ok(ChangedResult { changed })
}

#[command(
    id = "cell.blank.remove",
    kind = action,
    label = "Delete blank cell",
    scopes = [edit],
    undo = "Delete Blank Cell",
    needs_project,
    example(title = "Delete the blank cell at frame 7 of Bob", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 7}"#),
    gui = "Exposure Sheet and Timeline cell context: Delete Blank Cell (M5 C5.2)",
)]
pub(crate) fn cell_blank_remove(cx: &mut Cx, p: BlankParams) -> Result<ChangedResult, CommandError> {
    let t = cell_target(cx, p.element_ids.as_deref(), p.frame, p.frame)?;
    let mut budget = Budget::new("deleting blank cells");
    let n = p.count as usize;
    let changed = per_element(cx, &t, "cell.blank.remove", BLANKS, &mut budget, &mut |c, b| {
        b.spend(c.frames.len() + n)?;
        x::remove_blank(&c.frames, t.from, n).ok_or_else(|| {
            err(
                "cell.notBlank",
                format!("a cell of frames {}-{} of element `{}` is not blank", t.from, t.from + n - 1, c.id),
                json!({ "elementId": c.id.as_str(), "frame": t.from, "count": n }),
            )
        })
    })?;
    Ok(ChangedResult { changed })
}

// ----- cell.range.add / .delete -------------------------------------------------------------------------------------------

/// Insert position (13.6).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Position {
    /// Before the selected cell.
    Before,
    /// After the selected cell.
    #[default]
    After,
}

/// `cell.range.add` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RangeAddParams {
    /// Target elements (default: the selection, else the current element).
    #[serde(default)]
    element_ids: Option<Vec<String>>,
    /// Insert blank cells in every cell-based element of the scene (pegs, cameras and effects are not shifted).
    #[serde(default)]
    all_elements: bool,
    /// The selected cell (default: the first selected frame, else the current frame).
    #[serde(default)]
    frame: Option<i64>,
    /// Number of cells.
    #[serde(default = "one")]
    #[schemars(range(min = 1, max = 9999))]
    count: u32,
    /// Insert before or after the selected cell.
    #[serde(default)]
    position: Position,
    /// Each inserted cell gets a new empty drawing named after the highest numeric name (unavailable with
    /// `allElements`).
    #[serde(default)]
    named: bool,
}

/// `{from, to, drawingIds}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RangeAddResult {
    /// First inserted frame.
    from: u32,
    /// Last inserted frame.
    to: u32,
    /// Drawings created for named cells.
    drawing_ids: Vec<String>,
}

#[command(
    id = "cell.range.add",
    kind = action,
    label = "Insert cells",
    scopes = [edit],
    undo = "Insert Cells",
    needs_project,
    example(title = "Insert two named cells after frame 6 of Bob", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 6, "count": 2, "named": true}"#),
    example(title = "Insert three blank cells before frame 1 in every column", params = r#"{"allElements": true, "frame": 1, "count": 3, "position": "before"}"#),
    gui = "Exposure Sheet and Timeline cell context: Insert Cells (M5 C5.2)",
)]
pub(crate) fn cell_range_add(cx: &mut Cx, p: RangeAddParams) -> Result<RangeAddResult, CommandError> {
    if p.all_elements && p.named {
        return Err(err(
            codes::ARG_INVALID,
            "named cells are unavailable when inserting in all elements (04 §13.6)",
            json!({ "pointer": "/named" }),
        ));
    }
    let doc = cx.document()?.clone();
    let mut shifted: Vec<Id> = Vec::new();
    let mut t = if p.all_elements && p.element_ids.is_none() {
        let f = match p.frame {
            Some(v) => frame(v, "/frame")?,
            None => selected_cells(cx).map_or(cx.app().frame as usize, |s| s.1).max(1),
        };
        CellTarget { elements: Vec::new(), from: f, to: f }
    } else {
        cell_target(cx, p.element_ids.as_deref(), p.frame, p.frame)?
    };
    if p.all_elements {
        let scene = match t.elements.first() {
            Some(e) => doc.scene_of(e).cloned(),
            None => cx.app().scene.clone(),
        }
        .ok_or_else(no_target)?;
        let sc = doc.scene(&scene).ok_or_else(no_target)?;
        // Every cell-based column. Sound placements and media sequences that start at or after the insertion point
        // move later with the columns; one spanning it is not split (SI-088).
        let mut cols = Vec::new();
        for e in sc.column_order() {
            let Some(el) = doc.element(e) else { continue };
            if el.element_type().has_exposure() {
                cols.push(e.clone());
            } else {
                shifted.push(e.clone());
            }
        }
        t.elements = cols;
    }
    let at = match p.position {
        Position::Before => t.from,
        Position::After => t.from + 1,
    };
    let n = p.count as usize;
    let mut budget = Budget::new("inserting cells");
    let mut changes = Changes::default();
    let mut out = Vec::new();
    let mut created = Vec::new();
    let mut names_by_bank: BTreeMap<Id, Vec<String>> = BTreeMap::new();
    for e in &t.elements {
        applicable(&doc, e, "cell.range.add", if p.named { VECTOR } else { BLANKS })?;
        let mut col = Column::read(&doc, e, &mut budget)?;
        budget.spend(col.frames.len() + n)?;
        let values: Vec<Option<u32>> = if p.named {
            let bank = bank_of_element(&doc, e)?;
            let taken = names_by_bank.entry(bank.clone()).or_insert_with(|| super::timing::bank_names(&doc, &bank));
            budget.spend(taken.len())?;
            let names = x::next_drawing_names(taken.iter().map(String::as_str), n);
            let mut v = Vec::new();
            for name in names {
                let d = add_drawing(cx, &mut changes, &bank, &name, None)?;
                taken.push(name);
                created.push(d.as_str().to_owned());
                v.push(Some(col.intern(&d)));
            }
            v
        } else {
            vec![None; n]
        };
        // Inserting past the last exposure pads with blanks only when drawings are inserted.
        let after = x::insert(&col.frames, at, &values);
        write_column(&col, &after, &mut changes, &mut out)?;
    }
    for e in &shifted {
        let j = doc.entry_raw_json(&EntryKey::Element(e.clone())).map_err(super::timing::internal)?;
        let mut ops = Vec::new();
        let mut last = 0u64;
        for slot in ["placements", "sequences"] {
            for (i, item) in j.get(slot).and_then(Value::as_array).into_iter().flatten().enumerate() {
                budget.spend(1)?;
                let start = item.get("startFrame").and_then(Value::as_u64).unwrap_or(0);
                let stop = item.get("stopFrame").and_then(Value::as_u64).unwrap_or(0);
                if start < at as u64 {
                    continue;
                }
                let (ns, ne) = (start + n as u64, stop + n as u64);
                last = last.max(ne);
                ops.push(op("replace", &format!("/{slot}/{i}/startFrame"), Some(json!(ns))));
                ops.push(op("replace", &format!("/{slot}/{i}/stopFrame"), Some(json!(ne))));
                // Lip-sync mouths keep their timing relative to the moved clip.
                if let Some(pf) = item.pointer("/mouthTrack/producedFor/stopFrame").and_then(Value::as_u64) {
                    ops.push(op("replace", &format!("/{slot}/{i}/mouthTrack/producedFor/stopFrame"), Some(json!(pf + n as u64))));
                }
            }
        }
        super::timing::check_length(usize::try_from(last).unwrap_or(usize::MAX), e)?;
        if !ops.is_empty() {
            out.push(CellChange { element_id: e.as_str().to_owned(), from: as_u32(at), to: as_u32(usize::try_from(last).unwrap_or(0)) });
        }
        changes.push(EntryKey::Element(e.clone()), ops);
    }
    changes.stage(cx)?;
    emit_cells(cx, &out)?;
    if !created.is_empty()
        && let Some(e) = t.elements.first()
        && let Ok(bank) = bank_of_element(&doc, e)
    {
        emit_bank(cx, &doc, &bank, &created, "created");
    }
    Ok(RangeAddResult { from: as_u32(at), to: as_u32(at + n - 1), drawing_ids: created })
}

/// `cell.range.delete` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RangeDeleteParams {
    /// Target elements (default: the selection, else the current element).
    #[serde(default)]
    element_ids: Option<Vec<String>>,
    /// First frame.
    #[serde(default)]
    from: Option<i64>,
    /// Last frame.
    #[serde(default)]
    to: Option<i64>,
    /// Remove the cells and shift the following cells earlier (`true`), or clear them to blanks (`false`).
    #[serde(default)]
    shift: bool,
}

#[command(
    id = "cell.range.delete",
    kind = action,
    label = "Delete cells",
    scopes = [edit],
    undo = "Delete Cells",
    needs_project,
    example(title = "Clear frames 1-3 of Bob", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 1, "to": 3}"#),
    example(title = "Remove frames 1-3 of Bob and close the gap", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 1, "to": 3, "shift": true}"#),
    gui = "Edit > Delete with cells selected in the Exposure Sheet or Timeline (M5 C5.2)",
)]
pub(crate) fn cell_range_delete(cx: &mut Cx, p: RangeDeleteParams) -> Result<ChangedResult, CommandError> {
    let t = cell_target(cx, p.element_ids.as_deref(), p.from, p.to)?;
    let mut budget = Budget::new("deleting cells");
    let changed = per_element(cx, &t, "cell.range.delete", BLANKS, &mut budget, &mut |c, b| {
        b.spend(c.frames.len())?;
        Ok(x::delete_range(&c.frames, t.from, t.to, p.shift))
    })?;
    Ok(ChangedResult { changed })
}

// ----- transfer options (04 §17.3) ---------------------------------------------------------------------------------------

/// Peg key groups of the transfer options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub(crate) struct PegKeys {
    /// Motion keyframes.
    motion: bool,
    /// Rotation keyframes.
    rotation: bool,
    /// Scale keyframes.
    scale: bool,
    /// Pivot keyframes.
    pivot: bool,
    /// Velocity keyframes.
    velocity: bool,
    /// Skew keyframes.
    skew: bool,
}

impl Default for PegKeys {
    fn default() -> Self {
        Self { motion: true, rotation: true, scale: true, pivot: true, velocity: true, skew: true }
    }
}

/// The transfer options object (04 §17.3, 11.14); omitted fields take the defaults of 11.14.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase", default)]
pub(crate) struct TransferOptions {
    /// Same-named drawings in the destination bank are overwritten (otherwise new drawings are created).
    overwrite_drawings: bool,
    /// Paste inserts cells and cut/move removes them (following cells shift); off: overwrite / clear.
    exposure: bool,
    /// Extend transferred exposures up to the destination frame.
    auto_extend: bool,
    /// Update palette colours from templates.
    update_colours: bool,
    /// Replace keyframe properties at and after the current frame.
    keyframe_substitution: bool,
    /// Add keyframes at the start and end of the transferred range.
    boundary_keys: bool,
    /// Peg key groups.
    peg_keys: PegKeys,
}

impl Default for TransferOptions {
    fn default() -> Self {
        Self {
            overwrite_drawings: false,
            exposure: true,
            auto_extend: false,
            update_colours: false,
            keyframe_substitution: false,
            boundary_keys: false,
            peg_keys: PegKeys::default(),
        }
    }
}

impl TransferOptions {
    /// Options that do not apply to a cell transfer, set to a non-default value (reported as warnings).
    fn not_applied(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.auto_extend {
            v.push("autoExtend");
        }
        if self.update_colours {
            v.push("updateColours");
        }
        if self.keyframe_substitution {
            v.push("keyframeSubstitution");
        }
        if self.boundary_keys {
            v.push("boundaryKeys");
        }
        v
    }
}

fn warn_options(cx: &mut Cx, o: &TransferOptions) {
    for name in o.not_applied() {
        cx.warn(
            "cell.optionNotApplied",
            format!("transfer option `{name}` applies to templates and keyframes, not to a cell transfer; it was ignored"),
            Vec::new(),
        );
    }
}

/// Whether two element types hold compatible drawings (vector drawing and annotation; image with image).
fn compatible(a: ElementType, b: ElementType) -> bool {
    let vector = |t: ElementType| matches!(t, ElementType::Drawing | ElementType::Annotation);
    (vector(a) && vector(b)) || (a == ElementType::Image && b == ElementType::Image)
}

fn type_mismatch(from: &Id, to: &Id, a: ElementType, b: ElementType) -> CommandError {
    err(
        "cell.typeMismatch",
        format!("{} cells cannot go into a {} column", a.as_str(), b.as_str()),
        json!({ "sourceElementId": from.as_str(), "targetElementId": to.as_str(), "source": a.as_str(), "target": b.as_str() }),
    )
}

/// Maps transferred drawings into a target bank (04 §13.16): the same drawing within the same bank (`same`),
/// else a copy (`overwriteDrawings`: a same-named drawing of the target is overwritten; otherwise a same-named
/// drawing gets a new copy under the next free name), or always a new copy under a new name (`fresh`).
struct Mapper<'a> {
    doc: &'a Document,
    bank: Id,
    same: bool,
    fresh: bool,
    overwrite: bool,
    map: BTreeMap<Id, Id>,
    names: Vec<String>,
    created: Vec<String>,
}

impl<'a> Mapper<'a> {
    fn new(doc: &'a Document, source_bank: &Id, bank: Id, fresh: bool, overwrite: bool) -> Self {
        let names = super::timing::bank_names(doc, &bank);
        Self { doc, same: *source_bank == bank && !fresh, bank, fresh, overwrite, map: BTreeMap::new(), names, created: Vec::new() }
    }

    fn map(&mut self, cx: &mut Cx, changes: &mut Changes, d: &Id) -> Result<Id, CommandError> {
        if self.same {
            return Ok(d.clone());
        }
        if let Some(m) = self.map.get(d) {
            return Ok(m.clone());
        }
        let src = self.doc.drawing(d).ok_or_else(|| super::draw::not_found(d.as_str()))?;
        let json = self.doc.entry_raw_json(&EntryKey::Drawing(d.clone())).map_err(super::timing::internal)?;
        let existing = super::timing::drawing_named(self.doc, &self.bank, src.name());
        let target = match existing {
            Some(t) if !self.fresh && self.overwrite => {
                if self.doc.drawing(&t).is_some_and(|x| *x.locked()) {
                    return Err(locked(self.doc, &t));
                }
                let mut v = json.clone();
                v["id"] = json!(t.as_str());
                v["name"] = json!(src.name());
                v["locked"] = json!(false);
                if let Some(old) = self.doc.drawing(&t) {
                    v["createdAt"] = super::timing::to_json(old.created_at())?;
                    v["rev"] = json!(old.rev());
                }
                changes.push(EntryKey::Drawing(t.clone()), vec![op("replace", "", Some(v))]);
                t
            }
            _ => {
                let name = if self.fresh || self.names.iter().any(|n| n == src.name()) {
                    x::next_drawing_name(self.names.iter().map(String::as_str))
                } else {
                    src.name().clone()
                };
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

// ----- cell.range.move ----------------------------------------------------------------------------------------------------

/// Transfer mode (11.13).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MoveMode {
    /// Move the exposures.
    #[default]
    Move,
    /// Copy them (same drawings within the bank, copies elsewhere).
    Copy,
    /// Reference the same drawings (same element or clone group only).
    Link,
    /// Copy as new drawings.
    CopyNew,
}

/// `cell.range.move` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RangeMoveParams {
    /// Source elements (several: they map onto consecutive Exposure Sheet columns from the target).
    #[schemars(length(min = 1))]
    source_element_ids: Vec<String>,
    /// First source frame.
    from: i64,
    /// Last source frame.
    to: i64,
    /// Target element (default: each source element itself).
    #[serde(default)]
    target_element_id: Option<String>,
    /// Target frame (in the column as it is before the move).
    target_frame: i64,
    /// Move, copy, link or copy as new drawings.
    #[serde(default)]
    mode: MoveMode,
    /// Transfer options (default: the drag-and-drop defaults of 11.14).
    #[serde(default)]
    options: Option<TransferOptions>,
}

/// `{from, to, createdDrawingIds}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RangeMoveResult {
    /// First target frame.
    from: u32,
    /// Last target frame.
    to: u32,
    /// Drawings created in target banks.
    created_drawing_ids: Vec<String>,
}

#[command(
    id = "cell.range.move",
    kind = action,
    label = "Move cells",
    scopes = [edit],
    undo = "Move Cells",
    needs_project,
    example(title = "Move Bob's frames 1-6 to frame 45", params = r#"{"sourceElementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 1, "to": 6, "targetFrame": 45}"#),
    example(title = "Copy frames 1-6 of Bob into the Notes annotation column as new drawings", params = r#"{"sourceElementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 1, "to": 6, "targetElementId": "scene:\"Intro\"/element:\"Notes\"", "targetFrame": 1, "mode": "copy"}"#),
    gui = "Exposure Sheet and Timeline: drag selected cells (M5 C5.1, C5.2)",
)]
pub(crate) fn cell_range_move(cx: &mut Cx, p: RangeMoveParams) -> Result<RangeMoveResult, CommandError> {
    let from = frame(p.from, "/from")?;
    let to = frame(p.to, "/to")?;
    if to < from {
        return Err(err(codes::ARG_INVALID, "`to` is before `from`", json!({ "pointer": "/to" })));
    }
    let target_frame = frame(p.target_frame, "/targetFrame")?;
    let opts = p.options.unwrap_or_default();
    warn_options(cx, &opts);
    let doc = cx.document()?.clone();
    let mut sources = Vec::new();
    for (i, r) in p.source_element_ids.iter().enumerate() {
        let id = element_id(cx, r, &format!("/sourceElementIds/{i}"))?;
        if !sources.contains(&id) {
            sources.push(id);
        }
    }
    // Target columns: the target and the following Exposure Sheet columns, one per source.
    let targets: Vec<Id> = match &p.target_element_id {
        None => sources.clone(),
        Some(r) => {
            let t = element_id(cx, r, "/targetElementId")?;
            let scene = doc.scene_of(&t).and_then(|s| doc.scene(s)).ok_or_else(no_target)?;
            let order = scene.column_order();
            let start = order.iter().position(|c| *c == t).ok_or_else(|| {
                err("element.notCellBased", "the target is not an Exposure Sheet column", json!({ "pointer": "/targetElementId" }))
            })?;
            let v: Vec<Id> = order.iter().skip(start).take(sources.len()).cloned().collect();
            if v.len() < sources.len() {
                return Err(err(
                    codes::ARG_INVALID,
                    format!("{} source columns do not fit from the target column", sources.len()),
                    json!({ "pointer": "/sourceElementIds", "available": v.len() }),
                ));
            }
            v
        }
    };
    let mut budget = Budget::new("moving cells");
    let mut changes = Changes::default();
    let mut out = Vec::new();
    let mut created = Vec::new();
    let len = to - from + 1;
    // Columns are read once and updated in place when a column is both a source and a target.
    let mut cols: BTreeMap<Id, (Column, Vec<Option<u32>>)> = BTreeMap::new();
    for (s, t) in sources.iter().zip(targets.iter()) {
        applicable(&doc, s, "cell.range.move", LABELLED)?;
        applicable(&doc, t, "cell.range.move", LABELLED)?;
        let st = *element(&doc, s)?.element_type();
        let tt = *element(&doc, t)?.element_type();
        if !compatible(st, tt) {
            return Err(type_mismatch(s, t, st, tt));
        }
        let sbank = bank_of_element(&doc, s)?;
        let tbank = bank_of_element(&doc, t)?;
        if p.mode == MoveMode::Link && sbank != tbank {
            return Err(err(
                "cell.linkNotPossible",
                "linked cells must stay within one element or clone group (shared drawing bank)",
                json!({ "sourceElementId": s.as_str(), "targetElementId": t.as_str() }),
            ));
        }
        for id in [s, t] {
            if !cols.contains_key(id) {
                let c = Column::read(&doc, id, &mut budget)?;
                let f = c.frames.clone();
                cols.insert(id.clone(), (c, f));
            }
        }
        budget.spend(len * 2)?;
        let (scol, sframes) = cols.get(s).ok_or_else(|| super::timing::internal("column"))?;
        let picked: Vec<Option<Id>> =
            x::slice(sframes, from, to).into_iter().map(|c| c.and_then(|i| scol.drawing(i).cloned())).collect();
        // Remove from the source first (move), then place at the target.
        let mut tf = target_frame;
        if p.mode == MoveMode::Move {
            let (_, sf) = cols.get_mut(s).ok_or_else(|| super::timing::internal("column"))?;
            *sf = x::delete_range(sf, from, to, opts.exposure);
            if s == t && opts.exposure && tf > to {
                tf -= len;
            } else if s == t && opts.exposure && tf > from {
                tf = from;
            }
        }
        let mut mapper = Mapper::new(&doc, &sbank, tbank, p.mode == MoveMode::CopyNew, opts.overwrite_drawings);
        let (tcol, tframes) = cols.get_mut(t).ok_or_else(|| super::timing::internal("column"))?;
        let mut values = Vec::with_capacity(len);
        for c in &picked {
            values.push(match c {
                Some(d) => {
                    let m = if p.mode == MoveMode::Move && sbank == mapper.bank { d.clone() } else { mapper.map(cx, &mut changes, d)? };
                    Some(tcol.intern(&m))
                }
                None => None,
            });
        }
        if !opts.exposure {
            // Overwriting cells that show locked drawings is refused (04 §13.16).
            for c in x::slice(tframes, tf, tf + len - 1).into_iter().flatten() {
                if let Some(d) = tcol.drawing(c)
                    && doc.drawing(d).is_some_and(|x| *x.locked())
                {
                    return Err(locked(&doc, d));
                }
            }
        }
        *tframes = if opts.exposure { x::insert(tframes, tf, &values) } else { x::overwrite(tframes, tf, &values) };
        created.extend(mapper.created);
        out.push((tf, tf + len - 1));
    }
    let mut cells = Vec::new();
    for (col, after) in cols.values() {
        budget.spend(after.len())?;
        write_column(col, after, &mut changes, &mut cells)?;
    }
    changes.stage(cx)?;
    emit_cells(cx, &cells)?;
    let (a, b) = out.first().copied().unwrap_or((target_frame, target_frame + len - 1));
    Ok(RangeMoveResult { from: as_u32(a), to: as_u32(b), created_drawing_ids: created })
}

// ----- cell.label.set ------------------------------------------------------------------------------------------------------

/// `cell.label.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct LabelParams {
    /// Target elements (default: the selection, else the current element).
    #[serde(default)]
    element_ids: Option<Vec<String>>,
    /// First frame.
    #[serde(default)]
    from: Option<i64>,
    /// Last frame.
    #[serde(default)]
    to: Option<i64>,
    /// Drawing name in each element's bank.
    #[serde(default)]
    drawing: Option<String>,
    /// Drawing ID or path (it must belong to the element's bank).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Step through the bank (natural name order) from the drawing at `from`.
    #[serde(default)]
    step: Option<i64>,
    /// Create an empty drawing for an unknown name (drawing and annotation elements).
    #[serde(default)]
    create_missing: bool,
}

/// `{drawingId, name, changed}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LabelResult {
    /// The drawing now exposed (first element).
    drawing_id: String,
    /// Its name.
    name: String,
    /// Changed windows.
    changed: Vec<CellChange>,
}

#[command(
    id = "cell.label.set",
    kind = action,
    label = "Change label",
    scopes = [edit],
    undo = "Change Label",
    needs_project,
    example(title = "Expose drawing 10 in Bob's frames 1-6", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 1, "to": 6, "drawing": "10"}"#),
    example(title = "Create drawing 11 and expose it in frames 7-8", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 7, "to": 8, "drawing": "11", "createMissing": true}"#),
    example(title = "Step to the next drawing of the bank in frames 1-6", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 1, "to": 6, "step": 1}"#),
    gui = "Exposure Sheet: type in a cell; Cells panel: drawing name field and slider (M5 C5.2)",
)]
pub(crate) fn cell_label_set(cx: &mut Cx, p: LabelParams) -> Result<LabelResult, CommandError> {
    let given = usize::from(p.drawing.is_some()) + usize::from(p.drawing_id.is_some()) + usize::from(p.step.is_some());
    if given != 1 {
        return Err(err(
            codes::ARG_INVALID,
            "give exactly one of `drawing`, `drawingId` and `step`",
            json!({ "pointer": "", "given": given }),
        ));
    }
    let t = cell_target(cx, p.element_ids.as_deref(), p.from, p.to)?;
    let doc = cx.document()?.clone();
    let by_id = match &p.drawing_id {
        Some(r) => Some(drawing_id(cx, r, "/drawingId")?),
        None => None,
    };
    let mut budget = Budget::new("changing labels");
    let mut changes = Changes::default();
    let mut out = Vec::new();
    let mut first: Option<(Id, String)> = None;
    let mut created = BTreeMap::new();
    for e in &t.elements {
        applicable(&doc, e, "cell.label.set", LABELLED)?;
        let bank = bank_of_element(&doc, e)?;
        let mut col = Column::read(&doc, e, &mut budget)?;
        let d: Id = if let Some(d) = &by_id {
            if doc.bank_of(d) != Some(&bank) {
                return Err(drawing_not_found(doc.drawing(d).map_or("", |x| x.name().as_str()), e, "/drawingId"));
            }
            d.clone()
        } else if let Some(name) = &p.drawing {
            let name = drawing_name(name, "/drawing")?;
            match super::timing::drawing_named(&doc, &bank, &name).or_else(|| created.get(&(bank.clone(), name.clone())).cloned()) {
                Some(d) => d,
                None if p.create_missing && VECTOR.contains(element(&doc, e)?.element_type()) => {
                    let d = add_drawing(cx, &mut changes, &bank, &name, None)?;
                    created.insert((bank.clone(), name.clone()), d.clone());
                    d
                }
                None => return Err(drawing_not_found(&name, e, "/drawing")),
            }
        } else {
            let step = p.step.unwrap_or(0);
            let order = bank_order(&doc, &bank);
            budget.spend(order.len())?;
            let cur = x::at(&col.frames, t.from).and_then(|i| col.drawing(i).cloned()).ok_or_else(|| {
                err(
                    "cell.notApplicable",
                    format!("the cell at frame {} is blank: there is no drawing to step from", t.from),
                    json!({ "elementId": e.as_str(), "frame": t.from, "reason": "blank" }),
                )
            })?;
            let i = order.iter().position(|(d, _)| *d == cur).unwrap_or(0) as i64 + step;
            let j = usize::try_from(i).ok().filter(|j| *j < order.len()).ok_or_else(|| {
                err(
                    "cell.drawingNotFound",
                    format!("stepping {step} from the drawing at frame {} leaves the bank ({} drawings)", t.from, order.len()),
                    json!({ "pointer": "/step", "elementId": e.as_str(), "step": step }),
                )
            })?;
            order[j].0.clone()
        };
        let name = doc.drawing(&d).map(|x| x.name().clone()).or_else(|| p.drawing.clone()).unwrap_or_default();
        first.get_or_insert((d.clone(), name));
        let i = col.intern(&d);
        budget.spend(col.frames.len().max(t.to))?;
        let values = vec![Some(i); t.to - t.from + 1];
        let after = x::overwrite(&col.frames, t.from, &values);
        write_column(&col, &after, &mut changes, &mut out)?;
    }
    changes.stage(cx)?;
    emit_cells(cx, &out)?;
    let (d, name) = first.ok_or_else(no_target)?;
    Ok(LabelResult { drawing_id: d.as_str().to_owned(), name, changed: out })
}

// ----- cell.drawing.duplicate ---------------------------------------------------------------------------------------------

/// `cell.drawing.duplicate` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DuplicateParams {
    /// Target element (default: the selection, else the current element).
    #[serde(default)]
    element_id: Option<String>,
    /// First frame (its drawing is copied).
    #[serde(default)]
    from: Option<i64>,
    /// Last frame of the cells that show the copy.
    #[serde(default)]
    to: Option<i64>,
    /// Name of the copy (default: the next free name).
    #[serde(default)]
    name: Option<String>,
}

/// `{drawingId, name}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DrawingResult {
    /// The new drawing.
    drawing_id: String,
    /// Its name.
    name: String,
}

#[command(
    id = "cell.drawing.duplicate",
    kind = action,
    label = "Duplicate drawing",
    scopes = [edit],
    undo = "Duplicate Drawing",
    needs_project,
    example(title = "Duplicate the drawing at frames 3-4 of Bob and expose the copy there", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "from": 3, "to": 4}"#),
    gui = "Exposure Sheet and Timeline cell context: Duplicate Drawing (M5 C5.2)",
)]
pub(crate) fn cell_drawing_duplicate(cx: &mut Cx, p: DuplicateParams) -> Result<DrawingResult, CommandError> {
    let one = p.element_id.as_ref().map(|e| vec![e.clone()]);
    let t = cell_target(cx, one.as_deref(), p.from, p.to)?;
    let e = t.elements.first().cloned().ok_or_else(no_target)?;
    let doc = cx.document()?.clone();
    applicable(&doc, &e, "cell.drawing.duplicate", VECTOR)?;
    let bank = bank_of_element(&doc, &e)?;
    let mut budget = Budget::new("duplicating a drawing");
    let mut col = Column::read(&doc, &e, &mut budget)?;
    let orig_i = x::at(&col.frames, t.from).ok_or_else(|| {
        err(
            "cell.notApplicable",
            format!("the cell at frame {} is blank: there is no drawing to duplicate", t.from),
            json!({ "elementId": e.as_str(), "frame": t.from, "reason": "blank" }),
        )
    })?;
    let orig = col.drawing(orig_i).cloned().ok_or_else(|| super::timing::internal("drawing index"))?;
    let names = super::timing::bank_names(&doc, &bank);
    budget.spend(names.len())?;
    let name = match &p.name {
        Some(n) => {
            let n = drawing_name(n, "/name")?;
            if names.contains(&n) {
                return Err(err("drawing.nameDuplicate", format!("drawing `{n}` already exists in the bank; choose a different name"), json!({ "pointer": "/name", "name": n })));
            }
            n
        }
        None => x::next_drawing_name(names.iter().map(String::as_str)),
    };
    let json = doc.entry_raw_json(&EntryKey::Drawing(orig.clone())).map_err(super::timing::internal)?;
    let mut changes = Changes::default();
    let copy = add_drawing(cx, &mut changes, &bank, &name, Some(&json))?;
    let ci = col.intern(&copy);
    let mut after = col.frames.clone();
    budget.spend(after.len())?;
    for c in after.iter_mut().take(t.to).skip(t.from - 1) {
        if *c == Some(orig_i) {
            *c = Some(ci);
        }
    }
    let mut out = Vec::new();
    write_column(&col, &after, &mut changes, &mut out)?;
    changes.stage(cx)?;
    emit_cells(cx, &out)?;
    emit_bank(cx, &doc, &bank, &[copy.as_str().to_owned()], "created");
    Ok(DrawingResult { drawing_id: copy.as_str().to_owned(), name })
}

// ----- cell.cycle.create -----------------------------------------------------------------------------------------------------

/// `cell.cycle.create` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CycleParams {
    /// Target element (default: the selection, else the current element).
    #[serde(default)]
    element_id: Option<String>,
    /// First frame of the pattern.
    #[serde(default)]
    from: Option<i64>,
    /// Last frame of the pattern.
    #[serde(default)]
    to: Option<i64>,
    /// How many times the range appears in total (1 = no change).
    #[schemars(range(min = 1, max = 999))]
    cycles: u32,
}

/// `{from, to}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct WindowResult {
    /// First frame.
    from: u32,
    /// Last frame.
    to: u32,
}

#[command(
    id = "cell.cycle.create",
    kind = action,
    label = "Create cycle",
    scopes = [edit],
    undo = "Create Cycle",
    needs_project,
    example(title = "Repeat Bob's frames 1-8 three times in total", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "from": 1, "to": 8, "cycles": 3}"#),
    gui = "Exposure Sheet and Timeline cell context: Create Cycle (M5 C5.2)",
)]
pub(crate) fn cell_cycle_create(cx: &mut Cx, p: CycleParams) -> Result<WindowResult, CommandError> {
    let one = p.element_id.as_ref().map(|e| vec![e.clone()]);
    let mut t = cell_target(cx, one.as_deref(), p.from, p.to)?;
    t.elements.truncate(1);
    let len = t.to - t.from + 1;
    let k = p.cycles as usize;
    let mut budget = Budget::new("creating a cycle");
    budget.spend(len.saturating_mul(k))?;
    if t.to.saturating_add(len.saturating_mul(k - 1)) > x::MAX_FRAMES {
        let e = t.elements.first().cloned().ok_or_else(no_target)?;
        super::timing::check_length(t.to + len * (k - 1), &e)?;
    }
    per_element(cx, &t, "cell.cycle.create", EXPOSED, &mut budget, &mut |c, b| {
        b.spend(c.frames.len() + len * k)?;
        Ok(x::cycle(&c.frames, t.from, t.to, k))
    })?;
    Ok(WindowResult { from: as_u32(t.from), to: as_u32(t.from + len * k - 1) })
}

// ----- cell.columnate.apply ----------------------------------------------------------------------------------------------------

/// Columnate loop mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ColumnateLoop {
    /// One pass.
    #[default]
    None,
    /// Restart from the first drawing each loop.
    Continuous,
    /// Forward then backward, without repeating the end drawings.
    ForwardBackward,
}

/// Columnate placement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Placement {
    /// Overwrite the cells the cycle covers.
    #[default]
    Overwrite,
    /// Insert at the frame; following cells shift later.
    Insert,
}

/// `cell.columnate.apply` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ColumnateParams {
    /// Target element (default: the selection, else the current element).
    #[serde(default)]
    element_id: Option<String>,
    /// First frame (default: the first selected frame, else the current frame).
    #[serde(default)]
    frame: Option<i64>,
    /// First drawing (name in the bank).
    first_drawing: String,
    /// Last drawing (name in the bank; before the first = reversed order).
    last_drawing: String,
    /// Frames each drawing is exposed.
    #[serde(default = "one")]
    #[schemars(range(min = 1, max = 9999))]
    expose_each: u32,
    /// Frames the final drawing of the whole cycle is exposed (default: `exposeEach`).
    #[serde(default)]
    #[schemars(range(min = 1, max = 9999))]
    expose_last: Option<u32>,
    /// Loop mode.
    #[serde(default)]
    loop_mode: ColumnateLoop,
    /// Loops (continuous and forward-backward).
    #[serde(default)]
    #[schemars(range(min = 1, max = 999))]
    loops: Option<u32>,
    /// Total frames (recomputes the loops; the last loop is truncated).
    #[serde(default)]
    #[schemars(range(min = 1, max = 100_000))]
    total_frames: Option<u32>,
    /// Overwrite or insert.
    #[serde(default)]
    placement: Placement,
    /// Compute and return the cycle without applying it.
    #[serde(default)]
    dry_run: bool,
}

/// One drawing of the computed cycle.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct SequenceItem {
    /// Drawing name.
    drawing: String,
    /// First frame.
    from: u32,
    /// Last frame.
    to: u32,
}

/// `{from, to, totalFrames, loops, sequence}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ColumnateResult {
    /// First frame.
    from: u32,
    /// Last frame.
    to: u32,
    /// Total frames.
    total_frames: u32,
    /// Loops.
    #[schemars(range(min = 1, max = 999))]
    loops: u32,
    /// The drawings with their frames.
    sequence: Vec<SequenceItem>,
}

#[command(
    id = "cell.columnate.apply",
    kind = action,
    label = "Columnate",
    scopes = [edit],
    undo = "Columnate",
    needs_project,
    example(title = "Write drawings 1 to 10 on twos, forward-backward twice, from frame 1", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "10", "exposeEach": 2, "loopMode": "forwardBackward", "loops": 2}"#),
    example(title = "Preview a 10-frame continuous cycle without applying it", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1, "firstDrawing": "1", "lastDrawing": "10", "loopMode": "continuous", "totalFrames": 10, "dryRun": true}"#),
    gui = "Exposure Sheet and Timeline cell context: Columnate dialog (M5 C5.2)",
)]
pub(crate) fn cell_columnate_apply(cx: &mut Cx, p: ColumnateParams) -> Result<ColumnateResult, CommandError> {
    let one = p.element_id.as_ref().map(|e| vec![e.clone()]);
    let t = cell_target(cx, one.as_deref(), p.frame, p.frame)?;
    let e = t.elements.first().cloned().ok_or_else(no_target)?;
    let doc = cx.document()?.clone();
    applicable(&doc, &e, "cell.columnate.apply", EXPOSED)?;
    let bank = bank_of_element(&doc, &e)?;
    let order = bank_order(&doc, &bank);
    let mut budget = Budget::new("columnating");
    budget.spend(order.len())?;
    let find = |name: &str, pointer: &str| {
        order.iter().position(|(_, n)| n == name).ok_or_else(|| drawing_not_found(name, &e, pointer))
    };
    let a = find(&p.first_drawing, "/firstDrawing")?;
    let b = find(&p.last_drawing, "/lastDrawing")?;
    let drawings: Vec<usize> = if a <= b { (a..=b).collect() } else { (b..=a).rev().collect() };
    let mode = match p.loop_mode {
        ColumnateLoop::None => LoopMode::None,
        ColumnateLoop::Continuous => LoopMode::Continuous,
        ColumnateLoop::ForwardBackward => LoopMode::ForwardBackward,
    };
    let each = p.expose_each as usize;
    let last = p.expose_last.map_or(each, |l| l as usize);
    let loops = p.loops.map_or(1, |l| l as usize);
    // The cycle's length before it is built (refuse oversized cycles first).
    let unit = if mode == LoopMode::ForwardBackward && drawings.len() > 2 { 2 * drawings.len() - 2 } else { drawings.len() };
    let predicted = match p.total_frames {
        Some(tf) => tf as usize,
        None => (unit * if mode == LoopMode::None { 1 } else { loops }).saturating_sub(1) * each + last,
    };
    if t.from + predicted - 1 > x::MAX_FRAMES {
        super::timing::check_length(t.from + predicted - 1, &e)?;
    }
    budget.spend(predicted)?;
    let c = x::columnate(&drawings, mode, each, last, loops, p.total_frames.map(|v| v as usize));
    let mut col = Column::read(&doc, &e, &mut budget)?;
    let mut sequence = Vec::new();
    let mut f = t.from;
    let mut values = Vec::with_capacity(c.total_frames);
    for (i, n) in &c.sequence {
        let (d, name) = &order[*i];
        sequence.push(SequenceItem { drawing: name.clone(), from: as_u32(f), to: as_u32(f + n - 1) });
        let idx = col.intern(d);
        values.extend(std::iter::repeat_n(Some(idx), *n));
        f += n;
    }
    let after = match p.placement {
        Placement::Overwrite => x::overwrite(&col.frames, t.from, &values),
        Placement::Insert => x::insert(&col.frames, t.from, &values),
    };
    budget.spend(after.len())?;
    let mut changes = Changes::default();
    let mut out = Vec::new();
    write_column(&col, &after, &mut changes, &mut out)?;
    changes.stage(cx)?;
    // A dry run (the command's own `dryRun`) is discarded by the executor after staging; it announces nothing.
    if !p.dry_run {
        emit_cells(cx, &out)?;
    }
    Ok(ColumnateResult {
        from: as_u32(t.from),
        to: as_u32(t.from + c.total_frames.max(1) - 1),
        total_frames: as_u32(c.total_frames),
        loops: as_u32(c.loops.max(1)),
        sequence,
    })
}

// ----- cell.note.set -----------------------------------------------------------------------------------------------------------

/// `cell.note.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct NoteParams {
    /// The element.
    element_id: String,
    /// The frame.
    frame: i64,
    /// The note (empty removes it; at most 10 000 characters).
    #[schemars(length(max = 10_000))]
    text: String,
}

/// `{elementId, frame}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NoteResult {
    /// The element.
    element_id: String,
    /// The frame.
    frame: u32,
}

#[command(
    id = "cell.note.set",
    kind = action,
    label = "Set cell note",
    scopes = [edit],
    undo = "Set Cell Note",
    needs_project,
    example(title = "Note on Bob's frame 9", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 9, "text": "ease out here"}"#),
    example(title = "Remove the note of frame 3", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 3, "text": ""}"#),
    gui = "Exposure Sheet cell context: Set Cell Note (M5 C5.2)",
)]
pub(crate) fn cell_note_set(cx: &mut Cx, p: NoteParams) -> Result<NoteResult, CommandError> {
    let e = element_id(cx, &p.element_id, "/elementId")?;
    let f = frame(p.frame, "/frame")?;
    let doc = cx.document()?.clone();
    applicable(&doc, &e, "cell.note.set", LABELLED)?;
    let el = element(&doc, &e)?;
    let key = f.to_string();
    let path = format!("/cellNotes/{key}");
    let current = el.cell_notes().get(&as_u32(f));
    let ops = match (current, p.text.is_empty()) {
        (None, true) => Vec::new(),
        (Some(_), true) => vec![op("remove", &path, None)],
        (Some(c), false) if *c == p.text => Vec::new(),
        (_, false) if el.cell_notes().is_empty() => {
            let mut m = Map::new();
            m.insert(key, json!(p.text));
            vec![op("add", "/cellNotes", Some(Value::Object(m)))]
        }
        (_, false) => vec![op("add", &path, Some(json!(p.text)))],
    };
    if !ops.is_empty() {
        let mut changes = Changes::default();
        changes.push(EntryKey::Element(e.clone()), ops);
        changes.stage(cx)?;
        emit_cells(cx, &[CellChange { element_id: e.as_str().to_owned(), from: as_u32(f), to: as_u32(f) }])?;
    }
    Ok(NoteResult { element_id: e.as_str().to_owned(), frame: as_u32(f) })
}

// ----- clipboard ---------------------------------------------------------------------------------------------------------------

/// `cell.clipboard.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ClipboardSetParams {
    /// Source elements (default: the selection, else the current element).
    #[serde(default)]
    element_ids: Option<Vec<String>>,
    /// First frame.
    #[serde(default)]
    from: Option<i64>,
    /// Last frame.
    #[serde(default)]
    to: Option<i64>,
    /// Copy whole elements instead of cells.
    #[serde(default)]
    whole_elements: bool,
    /// Cut (an undoable removal, by the cut-special `exposure` option: remove and shift, or clear).
    #[serde(default)]
    cut: bool,
    /// Transfer options (default: 11.14's cut-special defaults).
    #[serde(default)]
    options: Option<TransferOptions>,
}

/// `{contentType, elementCount, frames}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClipboardSetResult {
    /// `drawingCells`, `imageCells` or `elements`.
    content_type: String,
    /// Elements copied.
    element_count: u32,
    /// Frames per element.
    frames: u32,
}

#[command(
    id = "cell.clipboard.set",
    kind = action,
    label = "Copy cells",
    scopes = [edit],
    undo = "Cut Cells",
    needs_project,
    example(title = "Copy Bob's frames 1-8", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 1, "to": 8}"#),
    example(title = "Cut frames 1-6 and close the gap", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 1, "to": 6, "cut": true}"#),
    gui = "Edit > Cut / Copy with cells selected in the Exposure Sheet or Timeline (M5 C5.2)",
)]
pub(crate) fn cell_clipboard_set(cx: &mut Cx, p: ClipboardSetParams) -> Result<ClipboardSetResult, CommandError> {
    let opts = p.options.unwrap_or_default();
    warn_options(cx, &opts);
    let t = cell_target(cx, p.element_ids.as_deref(), p.from, p.to)?;
    let doc = cx.document()?.clone();
    let mut budget = Budget::new("copying cells");
    if p.whole_elements {
        let ids: Vec<String> = t.elements.iter().map(|e| e.as_str().to_owned()).collect();
        super::edit_ops::copy_elements(cx, &ids, p.cut)?;
        return Ok(ClipboardSetResult { content_type: "elements".to_owned(), element_count: as_u32(ids.len()), frames: 0 });
    }
    let mut items = Vec::new();
    let mut kind = "drawingCells";
    for e in &t.elements {
        applicable(&doc, e, "cell.clipboard.set", LABELLED)?;
        if *element(&doc, e)?.element_type() == ElementType::Image {
            kind = "imageCells";
        }
        let col = Column::read(&doc, e, &mut budget)?;
        budget.spend(t.to - t.from + 1)?;
        for (k, c) in x::slice(&col.frames, t.from, t.to).into_iter().enumerate() {
            items.push((e.clone(), k, c.and_then(|i| col.drawing(i).cloned())));
        }
    }
    super::edit_ops::store_cells(cx, &items)?;
    if p.cut {
        let mut changes = Changes::default();
        let mut out = Vec::new();
        for e in &t.elements {
            let col = Column::read(&doc, e, &mut budget)?;
            let after = x::delete_range(&col.frames, t.from, t.to, opts.exposure);
            write_column(&col, &after, &mut changes, &mut out)?;
        }
        changes.stage(cx)?;
        emit_cells(cx, &out)?;
    }
    Ok(ClipboardSetResult {
        content_type: kind.to_owned(),
        element_count: as_u32(t.elements.len()),
        frames: as_u32(t.to - t.from + 1),
    })
}

/// Paste mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PasteMode {
    /// Same drawings within the bank; copies elsewhere.
    #[default]
    Paste,
    /// New drawings under new names.
    NewDrawings,
}

/// `cell.clipboard.apply` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ClipboardApplyParams {
    /// Target element (default: the current element; following columns receive further copied elements).
    #[serde(default)]
    element_id: Option<String>,
    /// Target frame (default: the first selected frame, else the current frame).
    #[serde(default)]
    frame: Option<i64>,
    /// Paste or paste as new drawings.
    #[serde(default)]
    mode: PasteMode,
    /// Transfer options (default: 11.14's paste-special defaults).
    #[serde(default)]
    options: Option<TransferOptions>,
}

/// `{elementIds, from, to, createdDrawingIds}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClipboardApplyResult {
    /// Elements pasted into (or created).
    element_ids: Vec<String>,
    /// First frame.
    from: u32,
    /// Last frame.
    to: u32,
    /// Drawings created.
    created_drawing_ids: Vec<String>,
}

#[command(
    id = "cell.clipboard.apply",
    kind = action,
    label = "Paste cells",
    scopes = [edit],
    undo = "Paste Cells",
    needs_project,
    example(title = "Paste the copied cells at Bob's frame 45", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 45}"#),
    example(title = "Paste as new drawings at frame 45", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 45, "mode": "newDrawings"}"#),
    gui = "Edit > Paste / Paste Special with cells selected (M5 C5.2)",
)]
pub(crate) fn cell_clipboard_apply(cx: &mut Cx, p: ClipboardApplyParams) -> Result<ClipboardApplyResult, CommandError> {
    let opts = p.options.unwrap_or_default();
    warn_options(cx, &opts);
    let clip = super::edit_ops::clipboard_cells(cx)?;
    let doc = cx.document()?.clone();
    let one = p.element_id.as_ref().map(|e| vec![e.clone()]);
    let clip = match clip {
        super::edit_ops::CellClip::Cells(c) => c,
        super::edit_ops::CellClip::Elements => {
            // Whole elements on the clipboard: new elements in the target's scene (04 §13.16).
            let scene = match &p.element_id {
                Some(r) => {
                    let e = element_id(cx, r, "/elementId")?;
                    doc.scene_of(&e).cloned().ok_or_else(no_target)?
                }
                None => super::timing::scene_id(cx, None)?,
            };
            let created = super::edit_ops::paste_elements(cx, &scene)?;
            return Ok(ClipboardApplyResult { element_ids: created, from: 1, to: 1, created_drawing_ids: Vec::new() });
        }
    };
    let t = cell_target(cx, one.as_deref(), p.frame, p.frame)?;
    let target = t.elements.first().cloned().ok_or_else(no_target)?;
    // Source columns in their clipboard order map onto the target and the following Exposure Sheet columns.
    let mut sources: Vec<Id> = Vec::new();
    for (e, _, _) in &clip {
        if !sources.contains(e) {
            sources.push(e.clone());
        }
    }
    let scene = doc.scene_of(&target).and_then(|s| doc.scene(s)).ok_or_else(no_target)?;
    let order = scene.column_order();
    let start = order.iter().position(|c| *c == target).ok_or_else(|| {
        err("element.notCellBased", "the target is not an Exposure Sheet column", json!({ "pointer": "/elementId" }))
    })?;
    let targets: Vec<Id> = order.iter().skip(start).take(sources.len()).cloned().collect();
    if targets.len() < sources.len() {
        return Err(err(
            codes::ARG_INVALID,
            format!("{} copied columns do not fit from the target column", sources.len()),
            json!({ "pointer": "/elementId", "available": targets.len() }),
        ));
    }
    let mut budget = Budget::new("pasting cells");
    budget.spend(clip.len())?;
    let mut changes = Changes::default();
    let mut out = Vec::new();
    let mut created = Vec::new();
    let mut width = 0usize;
    for (s, tgt) in sources.iter().zip(targets.iter()) {
        applicable(&doc, tgt, "cell.clipboard.apply", LABELLED)?;
        let tt = *element(&doc, tgt)?.element_type();
        let st = doc.element(s).map_or(tt, |e| *e.element_type());
        if !compatible(st, tt) {
            return Err(type_mismatch(s, tgt, st, tt));
        }
        let tbank = bank_of_element(&doc, tgt)?;
        let sbank = doc.element(s).and_then(|e| e.drawing_bank_id().clone()).unwrap_or_else(|| tbank.clone());
        let mut mapper = Mapper::new(&doc, &sbank, tbank.clone(), p.mode == PasteMode::NewDrawings, opts.overwrite_drawings);
        let mut col = Column::read(&doc, tgt, &mut budget)?;
        let cells: Vec<&(Id, usize, Option<Id>)> = clip.iter().filter(|(e, _, _)| e == s).collect();
        let w = cells.iter().map(|(_, k, _)| k + 1).max().unwrap_or(0);
        width = width.max(w);
        let mut values = vec![None; w];
        for (_, k, d) in cells {
            if let Some(d) = d {
                if doc.drawing(d).is_none() {
                    return Err(err("cell.clipboardEmpty", "a copied drawing no longer exists", json!({ "drawingId": d.as_str() })));
                }
                let m = mapper.map(cx, &mut changes, d)?;
                values[*k] = Some(col.intern(&m));
            }
        }
        if !opts.exposure {
            for c in x::slice(&col.frames, t.from, t.from + w.max(1) - 1).into_iter().flatten() {
                if let Some(d) = col.drawing(c)
                    && doc.drawing(d).is_some_and(|x| *x.locked())
                {
                    return Err(locked(&doc, d));
                }
            }
        }
        budget.spend(col.frames.len() + w)?;
        let after = if opts.exposure { x::insert(&col.frames, t.from, &values) } else { x::overwrite(&col.frames, t.from, &values) };
        write_column(&col, &after, &mut changes, &mut out)?;
        created.extend(mapper.created);
    }
    changes.stage(cx)?;
    emit_cells(cx, &out)?;
    Ok(ClipboardApplyResult {
        element_ids: targets.iter().map(|e| e.as_str().to_owned()).collect(),
        from: as_u32(t.from),
        to: as_u32(t.from + width.max(1) - 1),
        created_drawing_ids: created,
    })
}

// ----- selection ---------------------------------------------------------------------------------------------------------------

/// Cell selection extent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Extent {
    /// The given cells.
    #[default]
    Cells,
    /// The whole track of each element (frame 1 to its last exposed frame).
    Track,
    /// A sound clip's extent (M8).
    SoundClip,
    /// From the frame back to the sound clip's start (M8).
    SoundClipToStart,
    /// From the frame to the sound clip's end (M8).
    SoundClipToEnd,
}

/// Selection mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SelectMode {
    /// Replace the selection.
    #[default]
    Replace,
    /// Add to it.
    Add,
}

/// `cell.select` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SelectParams {
    /// Elements (columns or rows).
    element_ids: Vec<String>,
    /// First frame.
    from: i64,
    /// Last frame.
    to: i64,
    /// Replace or add.
    #[serde(default)]
    mode: SelectMode,
    /// Cells, a whole track, or a sound clip extent.
    #[serde(default)]
    extent: Extent,
}

/// `{elementIds, from, to}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SelectResult {
    /// Selected elements.
    element_ids: Vec<String>,
    /// First selected frame.
    from: u32,
    /// Last selected frame.
    to: u32,
}

/// The largest cell selection (frames × elements) one call may make.
const MAX_SELECTED_CELLS: usize = 1_000_000;

#[command(
    id = "cell.select",
    kind = ui,
    label = "Select cells",
    scopes = [ui],
    needs_project,
    example(title = "Select Bob's frames 1-6", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 1, "to": 6}"#),
    example(title = "Select Bob's whole track", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "from": 1, "to": 1, "extent": "track"}"#),
    gui = "Exposure Sheet and Timeline: click and drag over cells (M5 C5.1, C5.2)",
)]
pub(crate) fn cell_select(cx: &mut Cx, p: SelectParams) -> Result<SelectResult, CommandError> {
    let mut from = frame(p.from, "/from")?;
    let mut to = frame(p.to, "/to")?;
    if to < from {
        return Err(err(codes::ARG_INVALID, "`to` is before `from`", json!({ "pointer": "/to" })));
    }
    let doc = cx.document()?.clone();
    let mut ids = Vec::new();
    for (i, r) in p.element_ids.iter().enumerate() {
        let id = element_id(cx, r, &format!("/elementIds/{i}"))?;
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    match p.extent {
        Extent::Cells => {}
        Extent::Track => {
            from = 1;
            to = ids
                .iter()
                .filter_map(|e| doc.element(e))
                .map(|e| e.exposure().as_ref().map_or(0, |r| x::last_exposed(&x::expand(r))))
                .max()
                .unwrap_or(1)
                .max(1);
        }
        _ => {
            return Err(err(
                "cell.notApplicable",
                "sound clip extents select sound placements, which are M8's (SI-088)",
                json!({ "pointer": "/extent", "owner": "M8 C8.1", "specIssue": "SI-088" }),
            ));
        }
    }
    let count = ids.len().saturating_mul(to - from + 1);
    if count > MAX_SELECTED_CELLS {
        return Err(err(
            codes::LIMIT_EXCEEDED,
            format!("{count} cells exceed the {MAX_SELECTED_CELLS}-cell selection limit of one call"),
            json!({ "cells": count, "limit": MAX_SELECTED_CELLS }),
        ));
    }
    let mut refs = if p.mode == SelectMode::Add { cx.app().selection_of("xsheet") } else { Vec::new() };
    for r in cell_refs(&ids, from, to) {
        if !refs.contains(&r) {
            refs.push(r);
        }
    }
    cx.app_mut().set_selection("xsheet", refs.clone());
    cx.app_mut().selections.insert("timeline".to_owned(), refs);
    let (sel, a, b) = selected_cells(cx).ok_or_else(no_target)?;
    let current = cx.app().element.as_ref().map(|e| e.as_str().to_owned());
    let frame_now = cx.app().frame;
    let element_ids: Vec<String> = sel.iter().map(|e| e.as_str().to_owned()).collect();
    cx.emit(
        "cell.selection.changed",
        json!({ "elementIds": element_ids, "from": a, "to": b, "currentElementId": current, "currentFrame": frame_now }),
    );
    Ok(SelectResult { element_ids, from: as_u32(a), to: as_u32(b) })
}

/// `cell.selection.get` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SelectionResult {
    /// Elements of the selected cells (or the selected rows when no cells are selected).
    element_ids: Vec<String>,
    /// First selected frame (0: no cells selected).
    from: u32,
    /// Last selected frame (0: no cells selected).
    to: u32,
    /// Number of selected rows (Timeline element selection).
    rows_selected: u32,
    /// The current element.
    current_element_id: Option<String>,
    /// The current frame.
    current_frame: u32,
}

#[command(
    id = "cell.selection.get",
    kind = query,
    label = "Get cell selection",
    scopes = [read],
    needs_project,
    example(title = "The current cell selection and position", params = "{}"),
    agent_only = "the Exposure Sheet and Timeline show the selection directly",
)]
pub(crate) fn cell_selection_get(cx: &mut Cx, _p: NoParams) -> Result<SelectionResult, CommandError> {
    let doc = cx.document()?;
    let rows: Vec<String> =
        cx.app().selection_of("timeline").into_iter().filter(|k| parse_cell_ref(k).is_none() && Id::parse(k).ok().is_some_and(|id| doc.element(&id).is_some())).collect();
    let (element_ids, from, to) = match selected_cells(cx) {
        Some((e, a, b)) => (e.iter().map(|x| x.as_str().to_owned()).collect(), as_u32(a), as_u32(b)),
        None => (rows.clone(), 0, 0),
    };
    Ok(SelectionResult {
        element_ids,
        from,
        to,
        rows_selected: as_u32(rows.len()),
        current_element_id: cx.app().element.as_ref().map(|e| e.as_str().to_owned()),
        current_frame: cx.app().frame,
    })
}

// ----- events -------------------------------------------------------------------------------------------------------------------

/// `cell.exposures.changed` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExposuresChanged {
    /// The scene.
    scene_id: String,
    /// The element.
    element_id: String,
    /// First changed frame.
    from: u32,
    /// Last changed frame.
    to: u32,
    /// Revision after the change.
    rev: u64,
}

#[command(
    id = "cell.exposures.changed",
    kind = event,
    label = "Cells changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn cell_exposures_changed(_cx: &mut Cx, _p: NoParams) -> Result<ExposuresChanged, CommandError> {
    Err(super::common::event_not_invocable("cell.exposures.changed"))
}

/// `cell.selection.changed` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SelectionChanged {
    /// Elements of the selected cells.
    element_ids: Vec<String>,
    /// First selected frame.
    from: u32,
    /// Last selected frame.
    to: u32,
    /// The current element.
    current_element_id: Option<String>,
    /// The current frame.
    current_frame: u32,
}

#[command(
    id = "cell.selection.changed",
    kind = event,
    label = "Cell selection changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn cell_selection_changed(_cx: &mut Cx, _p: NoParams) -> Result<SelectionChanged, CommandError> {
    Err(super::common::event_not_invocable("cell.selection.changed"))
}
