//! Drawing-bank commands (04 §13.11–§13.14, §14, §17.4; M3 C3.1): `element.drawing.list/.create/.rename/
//! .duplicate/.delete`, `element.drawingContent.delete`, `element.drawingLock.set`, `element.unusedDrawing.list` and
//! the `element.drawing.changed` event.
//!
//! A bank belongs to its clone group (10 §4.8): renames, locks, clears and deletions show in every clone; "unused"
//! means exposed in no cell of any element of the clone group and used by no lip-sync mapping (10 §7.2
//! `unusedDrawings`). Deleting an exposed drawing needs `ifInUse: "clearCells"`; with the default `"fail"` the call
//! returns `needs_confirmation` listing the exposures (04 §17.1 item 6) and changes nothing.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_anim::exposure as x;
use tt_model::Id;
use tt_model::document::{Document, EntryKey};
use tt_model::entities::drawing::NodeKind;
use tt_registry::{CommandError, codes, command};

use super::common::NoParams;
use super::timing::{
    Budget, CellChange, Changes, Column, VECTOR, add_drawing, applicable, as_u32, bank_names, bank_of_element,
    bank_order, drawing_id, drawing_name, element, element_id, emit_bank, emit_cells, err, frame, internal, locked, op,
    scene_id, to_json, write_column,
};
use crate::cx::Cx;

/// The exposures of one drawing in one element: `(from, to)` windows.
fn exposures_of(col: &Column, d: &Id) -> Vec<(usize, usize)> {
    x::runs(&col.frames)
        .into_iter()
        .filter(|r| r.value.and_then(|i| col.drawing(i)) == Some(d))
        .map(|r| (r.from, r.to()))
        .collect()
}

/// Drawings used by lip-sync mappings of the project (10 §7.2: not unused).
fn lip_sync_used(doc: &Document) -> BTreeSet<Id> {
    let mut out = BTreeSet::new();
    for e in doc.elements() {
        for m in e.lip_sync_mappings().iter().flatten() {
            if let Ok(Value::Object(map)) = serde_json::to_value(&m.mapping) {
                for v in map.values() {
                    if let Some(d) = v.as_str().and_then(|s| Id::parse(s).ok()) {
                        out.insert(d);
                    }
                }
            }
        }
    }
    out
}

/// Drawing → `(element, from, to)` windows exposing it.
type Usage = BTreeMap<Id, Vec<(Id, usize, usize)>>;

/// Cells exposing each drawing of a bank across its clone group.
fn usage(doc: &Document, bank: &Id, budget: &mut Budget) -> Result<Usage, CommandError> {
    let mut out: Usage = BTreeMap::new();
    for e in doc.bank_users(bank) {
        if !doc.element(e).is_some_and(|x| x.element_type().has_exposure()) {
            continue;
        }
        let col = Column::read(doc, e, budget)?;
        for r in x::runs(&col.frames) {
            if let Some(d) = r.value.and_then(|i| col.drawing(i)) {
                out.entry(d.clone()).or_default().push((e.clone(), r.from, r.to()));
            }
        }
    }
    Ok(out)
}

// ----- element.drawing.list ----------------------------------------------------------------------------------------------

/// Usage filter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Filter {
    /// Every drawing.
    #[default]
    All,
    /// Drawings exposed somewhere in the clone group.
    Used,
    /// Drawings exposed nowhere in the clone group.
    Unused,
}

/// `element.drawing.list` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ListParams {
    /// A cell-based element.
    element_id: String,
    /// All, used or unused drawings.
    #[serde(default)]
    filter: Filter,
    /// Thumbnail size in pixels (16-512), rendered synchronously (04 §17.1 item 9).
    #[serde(default)]
    #[schemars(range(min = 16, max = 512))]
    thumbnails: Option<u32>,
}

/// A frame window.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Window {
    /// First frame.
    from: u32,
    /// Last frame.
    to: u32,
}

/// One drawing of the bank.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DrawingItem {
    /// ID.
    id: String,
    /// Name.
    name: String,
    /// Where this element exposes it.
    exposures: Vec<Window>,
    /// Cells exposing it in the whole clone group.
    use_count: u32,
    /// Locked against editing.
    locked: bool,
    /// No content.
    empty: bool,
    /// Its data could not be loaded (a placeholder).
    missing: bool,
    /// `vector`, `bitmap` or `empty`.
    content_type: String,
    /// PNG thumbnail, base64 (with `thumbnails`).
    #[serde(skip_serializing_if = "Option::is_none")]
    thumbnail: Option<String>,
}

/// `{drawings}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ListResult {
    /// The drawings in bank order (natural name order).
    drawings: Vec<DrawingItem>,
}

#[command(
    id = "element.drawing.list",
    kind = query,
    label = "List drawings",
    scopes = [read],
    needs_project,
    example(title = "Bob's drawings with usage", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\""}"#),
    example(title = "Bob's unused drawings with 32-pixel thumbnails", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "filter": "unused", "thumbnails": 32}"#),
    agent_only = "the Cells panel, Exposure Sheet and library show the drawings directly",
)]
pub(crate) fn element_drawing_list(cx: &mut Cx, p: ListParams) -> Result<ListResult, CommandError> {
    let id = element_id(cx, &p.element_id, "/elementId")?;
    let doc = cx.document()?.clone();
    let bank = bank_of_element(&doc, &id)?;
    let mut budget = Budget::new("listing drawings");
    let used = usage(&doc, &bank, &mut budget)?;
    let lips = lip_sync_used(&doc);
    let own = if element(&doc, &id)?.element_type().has_exposure() {
        Some(Column::read(&doc, &id, &mut budget)?)
    } else {
        None
    };
    let mut out = Vec::new();
    for (d, name) in bank_order(&doc, &bank) {
        let Some(dr) = doc.drawing(&d) else { continue };
        let uses: usize = used.get(&d).map_or(0, |v| v.iter().map(|(_, a, b)| b - a + 1).sum());
        let is_used = uses > 0 || lips.contains(&d);
        if (p.filter == Filter::Used && !is_used) || (p.filter == Filter::Unused && is_used) {
            continue;
        }
        budget.spend(dr.nodes().len())?;
        let content_type = if dr.nodes().is_empty() {
            "empty"
        } else if dr.nodes().iter().all(|n| *n.kind() == NodeKind::Bitmap) {
            "bitmap"
        } else {
            "vector"
        };
        let thumbnail = match p.thumbnails {
            Some(size) => {
                budget.spend((size as usize) * (size as usize) / 64)?;
                Some(thumbnail(cx, &d, size)?)
            }
            None => None,
        };
        out.push(DrawingItem {
            id: d.as_str().to_owned(),
            name,
            exposures: own
                .as_ref()
                .map(|c| {
                    exposures_of(c, &d).into_iter().map(|(a, b)| Window { from: as_u32(a), to: as_u32(b) }).collect()
                })
                .unwrap_or_default(),
            use_count: as_u32(uses),
            locked: *dr.locked(),
            empty: dr.nodes().is_empty(),
            missing: dr.meta().get("missing").and_then(Value::as_bool).unwrap_or(false),
            content_type: content_type.to_owned(),
            thumbnail,
        });
    }
    Ok(ListResult { drawings: out })
}

/// A synchronous square PNG thumbnail of a drawing (the `draw.drawing.render` path, transparent background).
fn thumbnail(cx: &mut Cx, d: &Id, size: u32) -> Result<String, CommandError> {
    let params: super::draw_render::RenderParams = serde_json::from_value(json!({
        "drawingId": d.as_str(), "width": size, "height": size,
    }))
    .map_err(internal)?;
    let r = super::draw_render::draw_drawing_render(cx, params)?;
    to_json(&r)?.get("png").and_then(Value::as_str).map(str::to_owned).ok_or_else(|| internal("thumbnail without png"))
}

// ----- element.unusedDrawing.list --------------------------------------------------------------------------------------------

/// `element.unusedDrawing.list` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct UnusedParams {
    /// Scene (default: the current scene) when `elementIds` is omitted.
    #[serde(default)]
    scene_id: Option<String>,
    /// Elements (default: every cell-based element of the scene).
    #[serde(default)]
    element_ids: Option<Vec<String>>,
}

/// One unused drawing.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UnusedItem {
    /// The element (the first of its clone group in Exposure Sheet order).
    element_id: String,
    /// The drawing.
    drawing_id: String,
    /// Its name.
    name: String,
}

/// `{items}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct UnusedResult {
    /// Unused drawings.
    items: Vec<UnusedItem>,
}

#[command(
    id = "element.unusedDrawing.list",
    kind = query,
    label = "List unused drawings",
    scopes = [read],
    needs_project,
    example(title = "Unused drawings of the Intro scene", params = r#"{"sceneId": "scene:\"Intro\""}"#),
    agent_only = "the library's view of the scene lists unused drawings directly",
)]
pub(crate) fn element_unused_drawing_list(cx: &mut Cx, p: UnusedParams) -> Result<UnusedResult, CommandError> {
    let doc = cx.document()?.clone();
    let elements: Vec<Id> = match &p.element_ids {
        Some(list) => {
            let mut out = Vec::new();
            for (i, r) in list.iter().enumerate() {
                out.push(element_id(cx, r, &format!("/elementIds/{i}"))?);
            }
            out
        }
        None => {
            let s = scene_id(cx, p.scene_id.as_deref())?;
            doc.scene(&s).map(|sc| sc.column_order().clone()).unwrap_or_default()
        }
    };
    let mut budget = Budget::new("listing unused drawings");
    let lips = lip_sync_used(&doc);
    let mut seen: BTreeSet<Id> = BTreeSet::new();
    let mut items = Vec::new();
    for e in &elements {
        let Some(bank) = doc.element(e).and_then(|x| x.drawing_bank_id().clone()) else { continue };
        if !seen.insert(bank.clone()) {
            continue;
        }
        let used = usage(&doc, &bank, &mut budget)?;
        for (d, name) in bank_order(&doc, &bank) {
            if !used.contains_key(&d) && !lips.contains(&d) {
                items.push(UnusedItem { element_id: e.as_str().to_owned(), drawing_id: d.as_str().to_owned(), name });
            }
        }
    }
    Ok(UnusedResult { items })
}

// ----- element.drawing.create / .rename / .duplicate -----------------------------------------------------------------------------

/// `element.drawing.create` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CreateParams {
    /// A drawing or annotation element.
    element_id: String,
    /// Name (default: the next integer after the highest numeric name).
    #[serde(default)]
    name: Option<String>,
    /// Expose the new drawing from this frame.
    #[serde(default)]
    expose_from: Option<i64>,
    /// … through this frame (default: `exposeFrom`).
    #[serde(default)]
    expose_to: Option<i64>,
}

/// `{drawingId, name}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DrawingResult {
    /// The drawing.
    drawing_id: String,
    /// Its name.
    name: String,
}

fn unique_drawing_name(
    doc: &Document,
    bank: &Id,
    given: Option<&str>,
    except: Option<&Id>,
) -> Result<String, CommandError> {
    match given {
        Some(n) => {
            let n = drawing_name(n, "/name")?;
            if let Some(o) = super::timing::drawing_named(doc, bank, &n)
                && Some(&o) != except
            {
                return Err(err(
                    "drawing.nameDuplicate",
                    format!("drawing `{n}` already exists in this drawing bank; choose a different name"),
                    json!({ "pointer": "/name", "name": n, "drawingId": o.as_str() }),
                ));
            }
            Ok(n)
        }
        None => Ok(x::next_drawing_name(bank_names(doc, bank).iter().map(String::as_str))),
    }
}

#[command(
    id = "element.drawing.create",
    kind = action,
    label = "Create drawing",
    scopes = [edit],
    undo = "Create Drawing",
    needs_project,
    example(title = "A new empty drawing in Bob's bank", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\""}"#),
    example(title = "Drawing A exposed at frames 45-48", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "name": "A", "exposeFrom": 45, "exposeTo": 48}"#),
    gui = "Exposure Sheet: type a new name in a cell; Drawing view: draw in a blank cell (M5 C5.2, M4 C4.2)",
)]
pub(crate) fn element_drawing_create(cx: &mut Cx, p: CreateParams) -> Result<DrawingResult, CommandError> {
    let id = element_id(cx, &p.element_id, "/elementId")?;
    let doc = cx.document()?.clone();
    applicable(&doc, &id, "element.drawing.create", VECTOR)?;
    let bank = bank_of_element(&doc, &id)?;
    let name = unique_drawing_name(&doc, &bank, p.name.as_deref(), None)?;
    let window = match (p.expose_from, p.expose_to) {
        (Some(a), b) => {
            let a = frame(a, "/exposeFrom")?;
            let b = match b {
                Some(b) => frame(b, "/exposeTo")?,
                None => a,
            };
            if b < a {
                return Err(err(
                    codes::ARG_INVALID,
                    "`exposeTo` is before `exposeFrom`",
                    json!({ "pointer": "/exposeTo" }),
                ));
            }
            Some((a, b))
        }
        (None, Some(_)) => {
            return Err(err(codes::ARG_INVALID, "`exposeTo` needs `exposeFrom`", json!({ "pointer": "/exposeTo" })));
        }
        (None, None) => None,
    };
    let mut budget = Budget::new("creating a drawing");
    let mut changes = Changes::default();
    let d = add_drawing(cx, &mut changes, &bank, &name, None)?;
    let mut out = Vec::new();
    if let Some((a, b)) = window {
        let mut col = Column::read(&doc, &id, &mut budget)?;
        budget.spend(col.frames.len().max(b))?;
        let i = col.intern(&d);
        let after = x::overwrite(&col.frames, a, &vec![Some(i); b - a + 1]);
        write_column(&col, &after, &mut changes, &mut out)?;
    }
    changes.stage(cx)?;
    emit_cells(cx, &out)?;
    emit_bank(cx, &doc, &bank, &[d.as_str().to_owned()], "created");
    Ok(DrawingResult { drawing_id: d.as_str().to_owned(), name })
}

/// `element.drawing.rename` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RenameParams {
    /// The drawing (ID or path).
    drawing_id: String,
    /// The new name (unique in the bank; at most 64 characters, none of `\ / : * ? < > " |` or `.`).
    name: String,
}

#[command(
    id = "element.drawing.rename",
    kind = action,
    label = "Rename drawing",
    scopes = [edit],
    undo = "Rename Drawing",
    needs_project,
    example(title = "Rename Bob's drawing 10 to 3", params = r#"{"drawingId": "scene:\"Intro\"/element:\"Bob\"/drawing:\"10\"", "name": "3"}"#),
    gui = "Exposure Sheet cell context: Rename Drawing; library view of the scene (M5 C5.2, M9)",
)]
pub(crate) fn element_drawing_rename(cx: &mut Cx, p: RenameParams) -> Result<DrawingResult, CommandError> {
    let d = drawing_id(cx, &p.drawing_id, "/drawingId")?;
    let doc = cx.document()?.clone();
    let bank = doc.bank_of(&d).cloned().ok_or_else(|| internal("drawing without bank"))?;
    let name = unique_drawing_name(&doc, &bank, Some(&p.name), Some(&d))?;
    let current = doc.drawing(&d).map(|x| x.name().clone()).unwrap_or_default();
    if current != name {
        let mut changes = Changes::default();
        changes.push(EntryKey::Drawing(d.clone()), vec![op("replace", "/name", Some(json!(name)))]);
        changes.stage(cx)?;
        // Light-table items track their source (04 §15.3): the label follows the drawing's name.
        emit_bank(cx, &doc, &bank, &[d.as_str().to_owned()], "renamed");
    }
    Ok(DrawingResult { drawing_id: d.as_str().to_owned(), name })
}

/// `element.drawing.duplicate` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DuplicateParams {
    /// The drawing (ID or path).
    drawing_id: String,
    /// Name of the copy (default: the next free name).
    #[serde(default)]
    name: Option<String>,
}

#[command(
    id = "element.drawing.duplicate",
    kind = action,
    label = "Duplicate drawing",
    scopes = [edit],
    undo = "Duplicate Drawing",
    needs_project,
    example(title = "Copy Bob's drawing 1 into a new bank entry", params = r#"{"drawingId": "scene:\"Intro\"/element:\"Bob\"/drawing:\"1\""}"#),
    gui = "Library view of the scene: Duplicate Drawing (M9)",
)]
pub(crate) fn element_drawing_duplicate(cx: &mut Cx, p: DuplicateParams) -> Result<DrawingResult, CommandError> {
    let d = drawing_id(cx, &p.drawing_id, "/drawingId")?;
    let doc = cx.document()?.clone();
    let bank = doc.bank_of(&d).cloned().ok_or_else(|| internal("drawing without bank"))?;
    let name = unique_drawing_name(&doc, &bank, p.name.as_deref(), None)?;
    let mut budget = Budget::new("duplicating a drawing");
    budget.spend(doc.drawing(&d).map_or(1, |x| x.nodes().len()))?;
    let json = doc.entry_raw_json(&EntryKey::Drawing(d.clone())).map_err(internal)?;
    let mut changes = Changes::default();
    let copy = add_drawing(cx, &mut changes, &bank, &name, Some(&json))?;
    changes.stage(cx)?;
    emit_bank(cx, &doc, &bank, &[copy.as_str().to_owned()], "created");
    Ok(DrawingResult { drawing_id: copy.as_str().to_owned(), name })
}

// ----- element.drawing.delete ---------------------------------------------------------------------------------------------------

/// What to do with exposed drawings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum IfInUse {
    /// Return `needs_confirmation` listing the exposures; change nothing.
    #[default]
    Fail,
    /// Delete and blank the cells that showed them.
    ClearCells,
}

/// `element.drawing.delete` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DeleteParams {
    /// The drawings.
    #[schemars(length(min = 1))]
    drawing_ids: Vec<String>,
    /// Exposed drawings: fail (confirmation) or clear their cells.
    #[serde(default)]
    if_in_use: IfInUse,
}

/// `{deletedIds, clearedCells}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeleteResult {
    /// Deleted drawings.
    deleted_ids: Vec<String>,
    /// Cells blanked.
    cleared_cells: Vec<CellChange>,
}

#[command(
    id = "element.drawing.delete",
    kind = action,
    label = "Delete drawings",
    scopes = [edit],
    undo = "Delete Drawings",
    needs_project,
    example(title = "Delete Bob's unused drawing 10", params = r#"{"drawingIds": ["scene:\"Intro\"/element:\"Bob\"/drawing:\"10\""]}"#),
    example(title = "Delete drawing 1 and blank the cells showing it", params = r#"{"drawingIds": ["scene:\"Intro\"/element:\"Bob\"/drawing:\"1\""], "ifInUse": "clearCells"}"#),
    gui = "Library view of the scene: Delete Drawing (with confirmation when exposed) (M9)",
)]
pub(crate) fn element_drawing_delete(cx: &mut Cx, p: DeleteParams) -> Result<DeleteResult, CommandError> {
    let mut ids = Vec::new();
    for (i, r) in p.drawing_ids.iter().enumerate() {
        let d = drawing_id(cx, r, &format!("/drawingIds/{i}"))?;
        if !ids.contains(&d) {
            ids.push(d);
        }
    }
    let doc = cx.document()?.clone();
    let mut budget = Budget::new("deleting drawings");
    let mut by_bank: BTreeMap<Id, Vec<Id>> = BTreeMap::new();
    for d in &ids {
        by_bank
            .entry(doc.bank_of(d).cloned().ok_or_else(|| internal("drawing without bank"))?)
            .or_default()
            .push(d.clone());
    }
    // Where each drawing is exposed.
    let mut exposed: Vec<Value> = Vec::new();
    for (bank, list) in &by_bank {
        let used = usage(&doc, bank, &mut budget)?;
        for d in list {
            for (e, a, b) in used.get(d).into_iter().flatten() {
                exposed.push(json!({ "kind": "clearCells", "code": "drawing.inUse", "drawingId": d.as_str(), "elementId": e.as_str(), "from": a, "to": b }));
            }
        }
    }
    if !exposed.is_empty() && p.if_in_use == IfInUse::Fail {
        return Err(err(
            "drawing.inUse",
            format!(
                "{} exposure(s) show the drawing(s); repeat with ifInUse: \"clearCells\" to delete them and blank those cells",
                exposed.len()
            ),
            json!({ "x-needsConfirmation": true, "consequences": exposed, "retryWith": { "ifInUse": "clearCells" } }),
        ));
    }
    let lips = lip_sync_used(&doc);
    let gone: BTreeSet<&Id> = ids.iter().collect();
    let mut changes = Changes::default();
    let mut cleared = Vec::new();
    for (bank, list) in &by_bank {
        for e in doc.bank_users(bank) {
            if !doc.element(e).is_some_and(|x| x.element_type().has_exposure()) {
                continue;
            }
            let col = Column::read(&doc, e, &mut budget)?;
            let after: Vec<Option<u32>> =
                col.frames.iter().map(|c| c.filter(|i| !col.drawing(*i).is_some_and(|d| gone.contains(d)))).collect();
            write_column(&col, &after, &mut changes, &mut cleared)?;
        }
        let stubs: Vec<Value> = doc
            .bank(bank)
            .map(|b| {
                b.drawings()
                    .ids()
                    .iter()
                    .filter(|d| !gone.contains(d))
                    .map(|d| json!({ "id": d.as_str(), "type": "drawing" }))
                    .collect()
            })
            .unwrap_or_default();
        changes.push(EntryKey::Bank(bank.clone()), vec![op("replace", "/drawings", Some(Value::Array(stubs)))]);
        for d in list {
            changes.push(EntryKey::Drawing(d.clone()), vec![op("remove", "", None)]);
        }
    }
    // Lip-sync mappings that pointed at a deleted drawing map that mouth to nothing.
    if ids.iter().any(|d| lips.contains(d)) {
        for e in doc.elements() {
            let key = EntryKey::Element(e.id().clone());
            let j = doc.entry_raw_json(&key).map_err(internal)?;
            let mut ops = Vec::new();
            for (mi, m) in j.get("lipSyncMappings").and_then(Value::as_array).into_iter().flatten().enumerate() {
                for (k, v) in m.get("mapping").and_then(Value::as_object).into_iter().flatten() {
                    if v.as_str().and_then(|s| Id::parse(s).ok()).is_some_and(|d| gone.contains(&d)) {
                        ops.push(op("replace", &format!("/lipSyncMappings/{mi}/mapping/{k}"), Some(Value::Null)));
                    }
                }
            }
            changes.push(key, ops);
        }
    }
    changes.stage(cx)?;
    emit_cells(cx, &cleared)?;
    for (bank, list) in &by_bank {
        emit_bank(cx, &doc, bank, &list.iter().map(|d| d.as_str().to_owned()).collect::<Vec<_>>(), "deleted");
    }
    Ok(DeleteResult { deleted_ids: ids.iter().map(|d| d.as_str().to_owned()).collect(), cleared_cells: cleared })
}

// ----- clear and lock --------------------------------------------------------------------------------------------------------------

/// The drawings of a `drawingIds?` parameter: given, else the drawing at the current cell.
fn drawings_or_current(cx: &Cx, given: Option<&[String]>) -> Result<Vec<Id>, CommandError> {
    match given {
        Some(list) => {
            let mut out = Vec::new();
            for (i, r) in list.iter().enumerate() {
                let d = drawing_id(cx, r, &format!("/drawingIds/{i}"))?;
                if !out.contains(&d) {
                    out.push(d);
                }
            }
            Ok(out)
        }
        None => {
            let doc = cx.document()?;
            match cx.app().current_drawing(doc) {
                Some(d) => Ok(vec![d]),
                None => Err(err(
                    "cell.noTarget",
                    "no drawing was given and the current cell is blank",
                    json!({ "pointer": "/drawingIds" }),
                )),
            }
        }
    }
}

/// `element.drawingContent.delete` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ClearParams {
    /// Drawings (default: the drawing at the current cell).
    #[serde(default)]
    drawing_ids: Option<Vec<String>>,
}

/// `{drawingIds}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DrawingIds {
    /// The drawings.
    drawing_ids: Vec<String>,
}

#[command(
    id = "element.drawingContent.delete",
    precheck = precheck_drawing_content_delete,
    kind = action,
    label = "Clear drawing",
    scopes = [edit],
    undo = "Clear Drawing",
    needs_project,
    example(title = "Clear Bob's drawing 1 (cells keep their label)", params = r#"{"drawingIds": ["scene:\"Intro\"/element:\"Bob\"/drawing:\"1\""]}"#),
    gui = "Exposure Sheet and Timeline cell context: Clear Drawing (M5 C5.2)",
)]
pub(crate) fn element_drawing_content_delete(cx: &mut Cx, p: ClearParams) -> Result<DrawingIds, CommandError> {
    let ids = drawings_or_current(cx, p.drawing_ids.as_deref())?;
    let doc = cx.document()?.clone();
    let mut budget = Budget::new("clearing drawings");
    let mut changes = Changes::default();
    let mut by_bank: BTreeMap<Id, Vec<String>> = BTreeMap::new();
    for d in &ids {
        let dr = doc.drawing(d).ok_or_else(|| super::draw::not_found(d.as_str()))?;
        let bank = doc.bank_of(d).cloned().ok_or_else(|| internal("drawing without bank"))?;
        // 13.20: clear is for drawing and annotation elements (an image drawing holds its one bitmap).
        if let Some(e) = doc.bank_users(&bank).first() {
            applicable(&doc, e, "element.drawingContent.delete", VECTOR)?;
        }
        if *dr.locked() {
            return Err(locked(&doc, d));
        }
        budget.spend(dr.nodes().len())?;
        if !dr.nodes().is_empty() {
            changes.push(EntryKey::Drawing(d.clone()), vec![op("replace", "/nodes", Some(json!([])))]);
            by_bank.entry(bank).or_default().push(d.as_str().to_owned());
        }
    }
    changes.stage(cx)?;
    for (bank, list) in &by_bank {
        emit_bank(cx, &doc, bank, list, "cleared");
    }
    Ok(DrawingIds { drawing_ids: ids.iter().map(|d| d.as_str().to_owned()).collect() })
}

/// `element.drawingLock.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct LockParams {
    /// Drawings (default: the drawing at the current cell).
    #[serde(default)]
    drawing_ids: Option<Vec<String>>,
    /// Locked or unlocked.
    locked: bool,
}

#[command(
    id = "element.drawingLock.set",
    kind = toggle,
    label = "Lock drawing",
    scopes = [edit],
    undo = "Lock Drawing",
    needs_project,
    example(title = "Lock Bob's drawing 1", params = r#"{"drawingIds": ["scene:\"Intro\"/element:\"Bob\"/drawing:\"1\""], "locked": true}"#),
    gui = "Exposure Sheet cell context: Lock / Unlock Drawing (M5 C5.2)",
)]
pub(crate) fn element_drawing_lock_set(cx: &mut Cx, p: LockParams) -> Result<DrawingIds, CommandError> {
    let ids = drawings_or_current(cx, p.drawing_ids.as_deref())?;
    let doc = cx.document()?.clone();
    let mut changes = Changes::default();
    let mut by_bank: BTreeMap<Id, Vec<String>> = BTreeMap::new();
    for d in &ids {
        let dr = doc.drawing(d).ok_or_else(|| super::draw::not_found(d.as_str()))?;
        // 04 §13.20: lock drawing is for drawing and annotation elements (C31-R1-16), also through delegated writes.
        if let Some(e) = doc.bank_of(d).and_then(|b| doc.bank_users(b).first()) {
            applicable(&doc, e, "element.drawingLock.set", VECTOR)?;
        }
        if *dr.locked() != p.locked {
            changes.push(EntryKey::Drawing(d.clone()), vec![op("replace", "/locked", Some(json!(p.locked)))]);
            by_bank
                .entry(doc.bank_of(d).cloned().ok_or_else(|| internal("bank"))?)
                .or_default()
                .push(d.as_str().to_owned());
        }
    }
    changes.stage(cx)?;
    cx.set_label(if p.locked { "Lock Drawing" } else { "Unlock Drawing" });
    for (bank, list) in &by_bank {
        emit_bank(cx, &doc, bank, list, if p.locked { "locked" } else { "unlocked" });
    }
    Ok(DrawingIds { drawing_ids: ids.iter().map(|d| d.as_str().to_owned()).collect() })
}

// ----- event ------------------------------------------------------------------------------------------------------------------------

/// `element.drawing.changed` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DrawingChanged {
    /// An element of the clone group (one event per element).
    element_id: String,
    /// The drawings.
    drawing_ids: Vec<String>,
    /// `created`, `renamed`, `deleted`, `locked`, `unlocked` or `cleared`.
    change: String,
}

#[command(
    id = "element.drawing.changed",
    kind = event,
    label = "Drawing bank changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn element_drawing_changed(_cx: &mut Cx, _p: NoParams) -> Result<DrawingChanged, CommandError> {
    Err(super::common::event_not_invocable("element.drawing.changed"))
}

/// `element.drawingContent.delete` (ruling O-23): the drawings given, else the drawing at the current cell.
pub(crate) fn precheck_drawing_content_delete(cx: &Cx, v: &Value) -> Result<(), CommandError> {
    let given: Option<Vec<String>> = v
        .get("drawingIds")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect());
    drawings_or_current(cx, given.as_deref()).map(|_| ())
}
