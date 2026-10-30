//! `draw.*` (02 §13): drawing content commands of M2 C2.1 that do not need the planar map — read-back, current
//! drawing, pivot, delete, group/ungroup — and the targeting rules every drawing command shares (§13.1 items 4, 11;
//! §3.12 revisions). Strokes are in `draw_stroke.rs`, text in `draw_text.rs`. Geometry comes from `tt-vector`; every
//! change is staged through [`Cx::patch_entries`], so the full model integrity gate runs on every commit.
//!
//! Zones, sides and re-noding (§3.12 item 1) are the planar map's (M2 C2.2): results report `zoneIds: []` and a
//! `doc.derivedUnavailable` warning instead of computed zones.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_model::change::PatchOp;
use tt_model::document::{Document, EntryKey};
use tt_model::entities::drawing::Drawing;
use tt_model::{Id, ObjectType};
use tt_registry::{CommandError, codes, command};
use tt_vector::drawing::{self as vd, Include, Located, ObjectKind};

use crate::app::AppContext;
use crate::cx::Cx;

/// The owner of zones and topology (warnings name it).
pub(crate) const ZONES_OWNER: &str = "M2 C2.2";

/// A point `{x, y}` in drawing units.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Point {
    /// X (du).
    pub x: f64,
    /// Y (du, up).
    pub y: f64,
}

/// `{x, y, width, height}`.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
pub(crate) struct Bounds {
    /// Minimum x (du).
    pub x: f64,
    /// Minimum y (du).
    pub y: f64,
    /// Width (du).
    pub width: f64,
    /// Height (du).
    pub height: f64,
}

impl From<tt_vector::geom::Rect> for Bounds {
    fn from(r: tt_vector::geom::Rect) -> Self {
        Bounds { x: r.min_x, y: r.min_y, width: r.max_x - r.min_x, height: r.max_y - r.min_y }
    }
}

pub(crate) fn op(kind: &'static str, path: &str, value: Option<Value>) -> PatchOp {
    PatchOp { op: kind, from: None, path: path.into(), range: None, value, old_value: None, elided: false }
}

pub(crate) fn err(code: &str, message: impl Into<String>, details: Value) -> CommandError {
    CommandError::with_details(code, message, details)
}

pub(crate) fn invalid_geometry(e: &tt_vector::path::GeomError) -> CommandError {
    err(e.code, e.message.clone(), json!({ "pointer": e.pointer }))
        .hint("give finite coordinates within ±10 000 000 du and complete Bézier lists (02 §13.4)")
}

/// The resolved target drawing of a command (02 §13.1 item 4).
pub(crate) struct Target {
    /// Drawing ID.
    pub id: Id,
    /// The element and frame whose cell exposes it, when addressed that way.
    pub element: Option<Id>,
    /// Created by this command (`createDrawing`).
    pub created: bool,
    /// The drawing's JSON when it is created by this command (not yet staged).
    pub new_json: Option<Value>,
}

impl Target {
    /// The drawing as it is before this command's own changes (the new drawing for a created one).
    pub(crate) fn drawing(&self, doc: &Document) -> Result<Drawing, CommandError> {
        if let Some(v) = &self.new_json {
            return tt_model::object::from_json::<Drawing>(v).map_err(crate::cx::model_error);
        }
        doc.drawing(&self.id).cloned().ok_or_else(|| not_found(self.id.as_str()))
    }
}

pub(crate) fn not_found(id: &str) -> CommandError {
    err("drawing.notFound", format!("drawing `{id}` does not exist"), json!({ "drawingId": id }))
}

fn no_target() -> CommandError {
    err("drawing.noTarget", "there is no current drawing and no drawing or cell was given", json!({}))
        .hint("pass drawingId, or elementId and frame, or make a drawing current (app.context.set)")
}

fn element_of(cx: &Cx, given: Option<&str>) -> Result<Option<Id>, CommandError> {
    match given {
        Some(r) => Ok(Some(Id::parse(&cx.resolve_as(r, ObjectType::Element)?)?)),
        None => Ok(cx.app().element.clone()),
    }
}

/// The next unused integer name in a bank (10 §4.8 drawing `name` default).
fn next_name(doc: &Document, bank: &Id) -> String {
    let taken: BTreeSet<String> = doc
        .bank(bank)
        .map(|b| b.drawings().ids().iter().filter_map(|d| doc.drawing(d)).map(|d| d.name().clone()).collect())
        .unwrap_or_default();
    (1u64..).map(|n| n.to_string()).find(|n| !taken.contains(n)).unwrap_or_else(|| "1".to_owned())
}

/// Resolves the drawing a command acts on: `drawingId`, else the drawing exposed at `elementId` (default: current)
/// and `frame` (default: current), else the current drawing; an empty addressed cell gets a new drawing when
/// `create` is set (the drawing and the exposure change are staged into `changes`, 02 §1.1).
///
/// # Errors
/// `drawing.notFound`, `drawing.noTarget`, `drawing.notVectorElement`, reference errors.
pub(crate) fn target(
    cx: &mut Cx,
    drawing_id: Option<&str>,
    element_id: Option<&str>,
    frame: Option<u32>,
    create: bool,
    changes: &mut Vec<(EntryKey, Vec<PatchOp>)>,
) -> Result<Target, CommandError> {
    if let Some(d) = drawing_id {
        let key = cx.resolve_as(d, ObjectType::Drawing)?;
        return Ok(Target { id: Id::parse(&key)?, element: None, created: false, new_json: None });
    }
    let doc = cx.document()?.clone();
    if element_id.is_none() && frame.is_none() {
        if let Some(d) = cx.app().current_drawing(&doc) {
            return Ok(Target { id: d, element: cx.app().element.clone(), created: false, new_json: None });
        }
        if !create || cx.app().element.is_none() {
            return Err(no_target());
        }
    }
    let Some(element) = element_of(cx, element_id)? else { return Err(no_target()) };
    let frame = frame.unwrap_or(cx.app().frame).max(1);
    let e = doc.element(&element).ok_or_else(|| {
        err(tt_model::codes::REF_NOT_FOUND, "no such element", json!({ "elementId": element.as_str() }))
    })?;
    if !matches!(
        e.element_type(),
        tt_model::entities::element::ElementType::Drawing | tt_model::entities::element::ElementType::Annotation
    ) {
        return Err(err(
            "drawing.notVectorElement",
            format!("element `{}` is not a drawing or annotation element", e.name()),
            json!({ "elementId": element.as_str() }),
        ));
    }
    if let Some(d) = AppContext::exposed_drawing(&doc, &element, frame) {
        return Ok(Target { id: d, element: Some(element), created: false, new_json: None });
    }
    if !create {
        return Err(no_target());
    }
    let Some(bank) = e.drawing_bank_id().clone() else {
        return Err(err(
            "drawing.notVectorElement",
            "the element has no drawing bank",
            json!({ "elementId": element.as_str() }),
        ));
    };
    let id = cx.new_id(ObjectType::Drawing)?;
    let name = next_name(&doc, &bank);
    let drawing = json!({
        "id": id.as_str(), "type": "drawing", "rev": 0, "name": name, "locked": false, "pivot": {"x": 0, "y": 0},
        "mouthShape": null, "nodes": [], "colourRecovery": [], "createdAt": crate::time::now_iso(),
    });
    // The cell at `frame` exposes the new drawing (frames past the end are blank until then).
    let mut frames: Vec<Option<String>> = Vec::new();
    for (d, n) in e.exposure().iter().flatten() {
        for _ in 0..*n {
            frames.push(d.as_ref().map(|x| x.as_str().to_owned()));
        }
    }
    let at = (frame - 1) as usize;
    if frames.len() <= at {
        frames.resize(at + 1, None);
    }
    frames[at] = Some(id.as_str().to_owned());
    let mut runs: Vec<(Option<String>, u32)> = Vec::new();
    for f in &frames {
        match runs.last_mut() {
            Some((d, n)) if d == f => *n += 1,
            _ => runs.push((f.clone(), 1)),
        }
    }
    while runs.last().is_some_and(|(d, _)| d.is_none()) {
        runs.pop();
    }
    let runs = Value::Array(runs.into_iter().map(|(d, n)| json!([d, n])).collect());
    changes.push((EntryKey::Drawing(id.clone()), vec![op("add", "", Some(drawing.clone()))]));
    changes.push((
        EntryKey::Bank(bank),
        vec![op("add", "/drawings/-", Some(json!({ "id": id.as_str(), "type": "drawing" })))],
    ));
    changes.push((EntryKey::Element(element.clone()), vec![op("replace", "/exposure", Some(runs))]));
    Ok(Target { id, element: Some(element), created: true, new_json: Some(drawing) })
}

/// `drawing.locked` (02 §1.1) and `drawing.staleRevision` (§3.12) checks for an existing drawing.
///
/// # Errors
/// `drawing.locked`; `drawing.staleRevision` when `expect_rev` differs from the drawing's revision.
pub(crate) fn check_editable(d: &Drawing, expect_rev: Option<u64>) -> Result<(), CommandError> {
    if *d.locked() {
        return Err(err(
            "drawing.locked",
            format!("drawing `{}` is locked", d.name()),
            json!({ "drawingId": d.id().as_str() }),
        )
        .hint("unlock it first (element.drawingLock.set)"));
    }
    if let Some(r) = expect_rev
        && r != *d.rev()
    {
        return Err(err(
            "drawing.staleRevision",
            format!("drawing `{}` changed: revision {}, expected {r}", d.name(), d.rev()),
            json!({ "drawingId": d.id().as_str(), "expectedRev": r, "currentRev": d.rev() }),
        )
        .hint("re-read the drawing (draw.drawing.get) and retry with its rev"));
    }
    Ok(())
}

/// Locates IDs (local or `<drawingId>/<localId>`) in a drawing.
///
/// # Errors
/// `drawing.objectNotFound` naming the first unknown ID.
pub(crate) fn locate_all(d: &Drawing, ids: &[String], pointer: &str) -> Result<Vec<Located>, CommandError> {
    let mut out: Vec<Located> = Vec::new();
    for (i, id) in ids.iter().enumerate() {
        let Some(l) = vd::locate(d, id) else {
            return Err(err(
                "drawing.objectNotFound",
                format!("`{id}` is not an object of drawing `{}`", d.name()),
                json!({ "id": id, "drawingId": d.id().as_str(), "pointer": format!("{pointer}/{i}") }),
            ));
        };
        if !out.iter().any(|o| o.id == l.id) {
            out.push(l);
        }
    }
    Ok(out)
}

/// The drawing selection's objects in drawing `d` (local IDs), for `ids?: [id] = selection`.
pub(crate) fn selection_ids(cx: &Cx, d: &Drawing) -> Vec<String> {
    let prefix = format!("{}/", d.id().as_str());
    cx.app().selection_of("drawing").into_iter().filter_map(|k| k.strip_prefix(&prefix).map(str::to_owned)).collect()
}

pub(crate) fn nothing_to_do(what: &str) -> CommandError {
    err("drawing.nothingToDo", format!("there is nothing to {what}"), json!({}))
        .hint("pass ids, or select drawing objects first (edit.selection.set {context: \"drawing\"})")
}

/// The revision the staged change will commit at (the drawing's new `rev`, REV-02).
pub(crate) fn committed_rev(cx: &mut Cx) -> Result<u64, CommandError> {
    cx.predicted_rev()
}

/// The `doc.derivedUnavailable` warning for zones (planar map, M2 C2.2).
pub(crate) fn warn_zones(cx: &mut Cx, drawing: &Id) {
    cx.warn(
        "doc.derivedUnavailable",
        format!("zones and side assignments are computed by the planar map ({ZONES_OWNER}); zoneIds is empty"),
        vec![drawing.as_str().to_owned()],
    );
}

// ----- draw.drawing.get --------------------------------------------------------------------------------------------

/// What `draw.drawing.get` includes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum IncludePart {
    /// Zones per layer (planar map, M2 C2.2; reported as unavailable).
    Zones,
    /// Pressure maps.
    Pressure,
    /// Brush outlines.
    Outline,
    /// Bounds.
    Bounds,
}

/// `draw.drawing.get` parameters (02 §13.5).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DrawingGetParams {
    /// The drawing (default: `elementId` + `frame`, else the current drawing).
    #[serde(default)]
    drawing_id: Option<String>,
    /// The element whose cell exposes the drawing.
    #[serde(default)]
    element_id: Option<String>,
    /// The frame of that cell (default: current frame).
    #[serde(default)]
    #[schemars(range(min = 1, max = 2_147_483_647))]
    frame: Option<u32>,
    /// Parts to include (default: all).
    #[serde(default)]
    include: Option<Vec<IncludePart>>,
    /// A flat object list with `parentId` instead of nesting.
    #[serde(default)]
    flatten: bool,
    /// Return only these objects (flat list).
    #[serde(default)]
    ids: Option<Vec<String>>,
}

/// The `draw.drawing.get` result: the 02 §13.4 drawing JSON, declared with its closed schema
/// (`tt_vector::drawing::readback_schema`).
#[derive(Debug, Clone)]
pub(crate) struct DrawingReadback(Value);

impl Serialize for DrawingReadback {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

impl JsonSchema for DrawingReadback {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "DrawingReadback".into()
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::Schema::try_from(vd::readback_schema()).unwrap_or_default()
    }
}

/// The largest `draw.drawing.get` answer (bytes of compact JSON); larger drawings are read with `ids` or `include`.
pub(crate) const MAX_READBACK_BYTES: usize = 32 * 1024 * 1024;

#[command(
    id = "draw.drawing.get",
    kind = query,
    label = "Get drawing content",
    scopes = [read],
    needs_project,
    example(title = "The drawing exposed by Bob at frame 1", params = r#"{"elementId": "scene:#1/element:\"Bob\"", "frame": 1}"#),
    example(title = "Strokes only, flat, without outlines", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "flatten": true, "include": ["pressure", "bounds"]}"#),
    agent_only = "the primary read model for agents (02 §13.6); the GUI renders drawings directly",
)]
pub(crate) fn draw_drawing_get(cx: &mut Cx, p: DrawingGetParams) -> Result<DrawingReadback, CommandError> {
    let mut none = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), p.element_id.as_deref(), p.frame, false, &mut none)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?;
    let parts = p
        .include
        .unwrap_or_else(|| vec![IncludePart::Zones, IncludePart::Pressure, IncludePart::Outline, IncludePart::Bounds]);
    let inc = Include {
        pressure: parts.contains(&IncludePart::Pressure),
        outline: parts.contains(&IncludePart::Outline),
        bounds: parts.contains(&IncludePart::Bounds),
    };
    if parts.contains(&IncludePart::Zones) {
        warn_zones(cx, d.id());
    }
    let element = t.element.or_else(|| doc.bank_of(d.id()).and_then(|b| doc.bank_users(b).first().cloned()));
    let nodes = match (&p.ids, p.flatten) {
        (Some(ids), _) => {
            let found = locate_all(&d, ids, "/ids")?;
            let wanted: Vec<String> = found.into_iter().map(|l| l.id).collect();
            Value::Array(vd::select_ids(vd::flat_json(&d, inc), &wanted))
        }
        (None, true) => Value::Array(vd::flat_json(&d, inc)),
        (None, false) => Value::Array(d.nodes().iter().map(|n| vd::node_json(n, inc)).collect()),
    };
    let bounds =
        if inc.bounds { vd::drawing_bounds(&d, false).map_or(Value::Null, |r| r.to_json()) } else { Value::Null };
    let out = json!({
        "drawingId": d.id().as_str(),
        "elementId": element.as_ref().map(Id::as_str),
        "name": d.name(),
        "rev": d.rev(),
        "locked": d.locked(),
        "space": { "units": "du", "duPerField": 100, "yAxis": "up", "origin": "drawingCentre" },
        "pivot": { "x": d.pivot().x, "y": d.pivot().y },
        "bounds": bounds,
        "nodes": nodes,
    });
    let size = serde_json::to_vec(&out).map_or(0, |b| b.len());
    if size > MAX_READBACK_BYTES {
        return Err(err(
            codes::LIMIT_EXCEEDED,
            format!("the drawing's read-back is {size} bytes (limit {MAX_READBACK_BYTES})"),
            json!({ "bytes": size, "limit": MAX_READBACK_BYTES }),
        )
        .hint("read parts with ids, or leave out outline and pressure with include"));
    }
    Ok(DrawingReadback(out))
}

// ----- draw.current.get --------------------------------------------------------------------------------------------

/// `draw.current.get` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CurrentGetParams {
    /// A view id (default: the active view, the central view of the arrangement in use).
    #[serde(default)]
    view: Option<String>,
}

/// The view a drawing is edited in (SI-026 `activeView`, the `AppState.activeView` shape).
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ViewRef {
    /// View id.
    view_id: String,
    /// Panel id.
    panel_id: String,
    /// `camera`, `drawing`, `top` or `side`.
    #[serde(rename = "type")]
    ty: String,
}

/// `draw.current.get` result (02 §13.5).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurrentResult {
    /// The current drawing, `null` for an empty cell.
    drawing_id: Option<String>,
    /// The current element.
    element_id: Option<String>,
    /// The current scene.
    scene_id: Option<String>,
    /// The current frame.
    frame: u32,
    /// The current cell is empty.
    cell_empty: bool,
    /// The current drawing is locked.
    locked: bool,
    /// The active view.
    active_view: ViewRef,
}

#[command(
    id = "draw.current.get",
    kind = query,
    label = "Get current drawing",
    scopes = [read],
    needs_project,
    example(title = "What a drawing tool would edit now", params = "{}"),
    agent_only = "the GUI shows the current drawing in the views",
)]
pub(crate) fn draw_current_get(cx: &mut Cx, p: CurrentGetParams) -> Result<CurrentResult, CommandError> {
    let doc = cx.document()?.clone();
    let app = cx.app();
    let drawing = app.current_drawing(&doc);
    let central = p.view.unwrap_or_else(|| cx.ui().central_panel().to_owned());
    Ok(CurrentResult {
        locked: drawing.as_ref().and_then(|d| doc.drawing(d)).is_some_and(|d| *d.locked()),
        drawing_id: drawing.as_ref().map(|d| d.as_str().to_owned()),
        element_id: app.element.as_ref().map(|e| e.as_str().to_owned()),
        scene_id: app.scene.as_ref().map(|s| s.as_str().to_owned()),
        frame: app.frame,
        cell_empty: drawing.is_none(),
        active_view: ViewRef {
            view_id: central.clone(),
            ty: central.trim_end_matches("View").to_owned(),
            panel_id: central,
        },
    })
}

// ----- draw.pivot.set ----------------------------------------------------------------------------------------------

/// `draw.pivot.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PivotParams {
    /// The drawing (default: `elementId` + `frame`, else the current drawing).
    #[serde(default)]
    drawing_id: Option<String>,
    /// The element whose cell exposes the drawing.
    #[serde(default)]
    element_id: Option<String>,
    /// The frame of that cell.
    #[serde(default)]
    #[schemars(range(min = 1, max = 2_147_483_647))]
    frame: Option<u32>,
    /// The new pivot (du).
    point: Point,
    /// The drawing revision the caller read (02 §3.12; `drawing.staleRevision` when it changed).
    #[serde(default)]
    expect_rev: Option<u64>,
}

/// `{pivot, rev}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct PivotResult {
    /// The pivot.
    pivot: Point,
    /// The drawing revision after the change.
    rev: u64,
}

#[command(
    id = "draw.pivot.set",
    kind = action,
    label = "Change drawing pivot",
    scopes = [edit],
    undo = "Change Drawing Pivot",
    needs_project,
    example(title = "Pivot at the feet", params = r#"{"elementId": "scene:#1/element:\"Bob\"", "frame": 1, "point": {"x": 0, "y": -120}}"#),
    gui = "Drawing view: Select tool pivot handle (M4 C4.2)",
)]
pub(crate) fn draw_pivot_set(cx: &mut Cx, p: PivotParams) -> Result<PivotResult, CommandError> {
    for (v, k) in [(p.point.x, "x"), (p.point.y, "y")] {
        tt_vector::path::check_coord(v, &format!("/point/{k}")).map_err(|e| invalid_geometry(&e))?;
    }
    let mut changes = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), p.element_id.as_deref(), p.frame, false, &mut changes)?;
    let d = t.drawing(cx.document()?)?;
    check_editable(&d, p.expect_rev)?;
    changes.push((
        EntryKey::Drawing(t.id.clone()),
        vec![op("replace", "/pivot", Some(json!({ "x": p.point.x, "y": p.point.y })))],
    ));
    cx.patch_entries(changes)?;
    Ok(PivotResult { pivot: p.point, rev: committed_rev(cx)? })
}

// ----- draw.object.delete ------------------------------------------------------------------------------------------

/// `draw.object.delete` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DeleteParams {
    /// The drawing (default: `elementId` + `frame`, else the current drawing).
    #[serde(default)]
    drawing_id: Option<String>,
    /// The element whose cell exposes the drawing.
    #[serde(default)]
    element_id: Option<String>,
    /// The frame of that cell.
    #[serde(default)]
    #[schemars(range(min = 1, max = 2_147_483_647))]
    frame: Option<u32>,
    /// Objects to delete (default: the drawing selection).
    #[serde(default)]
    ids: Option<Vec<String>>,
    /// The drawing revision the caller read (02 §3.12).
    #[serde(default)]
    expect_rev: Option<u64>,
}

/// `{removedIds, rev}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeleteResult {
    /// Removed objects (their descendants included, and layers left empty).
    removed_ids: Vec<String>,
    /// The drawing revision after the change.
    rev: u64,
}

/// Removal operations for located objects: descendants of removed nodes are skipped; layers whose every stroke is
/// removed go too. Returns the operations (deepest and last first, so indices stay valid) and the removed IDs.
pub(crate) fn removal_ops(d: &Drawing, targets: &[Located]) -> (Vec<PatchOp>, Vec<String>) {
    let all = vd::objects(d);
    let mut gone: BTreeSet<String> = targets.iter().map(|t| t.id.clone()).collect();
    // A layer all of whose strokes go is removed with them.
    for l in all.iter().filter(|o| o.kind == ObjectKind::Layer) {
        let strokes: Vec<&Located> =
            all.iter().filter(|o| o.parent.as_deref() == Some(&l.id) && o.kind == ObjectKind::Stroke).collect();
        if !strokes.is_empty() && strokes.iter().all(|s| gone.contains(&s.id)) {
            gone.insert(l.id.clone());
        }
    }
    // Everything below a removed node is removed with it.
    let mut removed: Vec<String> = Vec::new();
    let mut tops: Vec<&Located> = Vec::new();
    for o in &all {
        let mut ancestor_gone = false;
        let mut p = o.parent.clone();
        while let Some(pid) = p {
            if gone.contains(&pid) {
                ancestor_gone = true;
                break;
            }
            p = all.iter().find(|x| x.id == pid).and_then(|x| x.parent.clone());
        }
        if ancestor_gone || gone.contains(&o.id) {
            removed.push(o.id.clone());
        }
        if gone.contains(&o.id) && !ancestor_gone {
            tops.push(o);
        }
    }
    // Remove from the end of each list first: sort by pointer depth then index, descending.
    tops.sort_by(|a, b| b.list.cmp(&a.list).then(b.index.cmp(&a.index)));
    let ops = tops.iter().map(|o| op("remove", &o.pointer, None)).collect();
    (ops, removed)
}

#[command(
    id = "draw.object.delete",
    kind = action,
    label = "Delete drawing objects",
    scopes = [edit],
    undo = "Delete Object",
    needs_project,
    example(title = "Delete the invisible stroke of Bob's first drawing", params = r#"{"elementId": "scene:#1/element:\"Bob\"", "frame": 1, "ids": ["st_01K742SG0S0NT23BWCZN64SFQ5"]}"#),
    gui = "Drawing view: Delete key / Edit menu (M4 C4.2)",
)]
pub(crate) fn draw_object_delete(cx: &mut Cx, p: DeleteParams) -> Result<DeleteResult, CommandError> {
    let mut changes = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), p.element_id.as_deref(), p.frame, false, &mut changes)?;
    let d = t.drawing(cx.document()?)?;
    check_editable(&d, p.expect_rev)?;
    let ids = p.ids.unwrap_or_else(|| selection_ids(cx, &d));
    if ids.is_empty() {
        return Err(nothing_to_do("delete"));
    }
    let found = locate_all(&d, &ids, "/ids")?;
    let (ops, removed) = removal_ops(&d, &found);
    changes.push((EntryKey::Drawing(t.id.clone()), ops));
    cx.patch_entries(changes)?;
    warn_zones(cx, &t.id);
    Ok(DeleteResult { removed_ids: removed, rev: committed_rev(cx)? })
}

// ----- draw.group.create / draw.group.delete -----------------------------------------------------------------------

/// `draw.group.create` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct GroupCreateParams {
    /// The drawing (default: `elementId` + `frame`, else the current drawing).
    #[serde(default)]
    drawing_id: Option<String>,
    /// The element whose cell exposes the drawing.
    #[serde(default)]
    element_id: Option<String>,
    /// The frame of that cell.
    #[serde(default)]
    #[schemars(range(min = 1, max = 2_147_483_647))]
    frame: Option<u32>,
    /// Objects to group (default: the drawing selection).
    #[serde(default)]
    ids: Option<Vec<String>>,
    /// The drawing revision the caller read (02 §3.12).
    #[serde(default)]
    expect_rev: Option<u64>,
}

/// `{groupId, rev}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GroupCreateResult {
    /// The new group.
    group_id: String,
    /// New layers holding grouped strokes (strokes are grouped in their own layer, 02 §8.4).
    created_ids: Vec<String>,
    /// The drawing revision after the change.
    rev: u64,
}

#[command(
    id = "draw.group.create",
    kind = action,
    label = "Group drawing objects",
    scopes = [edit],
    undo = "Group Selection",
    needs_project,
    example(title = "Group the caption and the group of the first drawing", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "ids": ["nd_01K742SG0TX3WF1BSWWGMWM5WG", "nd_01K742SG0WFAQJFKAZKV4CD56C"]}"#),
    gui = "Drawing view context menu: Group (M4 C4.2)",
)]
pub(crate) fn draw_group_create(cx: &mut Cx, p: GroupCreateParams) -> Result<GroupCreateResult, CommandError> {
    let mut changes = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), p.element_id.as_deref(), p.frame, false, &mut changes)?;
    let d = t.drawing(cx.document()?)?;
    check_editable(&d, p.expect_rev)?;
    let ids = p.ids.unwrap_or_else(|| selection_ids(cx, &d));
    if ids.is_empty() {
        return Err(nothing_to_do("group"));
    }
    let found = locate_all(&d, &ids, "/ids")?;
    // Members are top-level items of one node list: nodes, or layers' strokes (moved into a new layer each).
    let item_of = |o: &Located| -> Result<Located, CommandError> {
        if o.kind == ObjectKind::Stroke {
            let layer = o.parent.as_deref().and_then(|l| vd::locate(&d, l));
            layer.ok_or_else(|| CommandError::internal("stroke without a layer"))
        } else {
            Ok(o.clone())
        }
    };
    let mut items: Vec<Located> = Vec::new();
    for o in &found {
        let it = item_of(o)?;
        if !items.iter().any(|x| x.id == it.id) {
            items.push(it);
        }
    }
    let list = items[0].list.clone();
    if items.iter().any(|i| i.list != list) {
        return Err(err(
            codes::ARG_INVALID,
            "grouped objects must be at the same level of the drawing (one node list)",
            json!({ "pointer": "/ids" }),
        )
        .hint("ungroup first, or group the objects of one level"));
    }
    if found.iter().any(|o| {
        o.kind != ObjectKind::Stroke
            && found.iter().any(|s| s.kind == ObjectKind::Stroke && s.parent.as_deref() == Some(&o.id))
    }) {
        return Err(err(
            codes::ARG_INVALID,
            "a layer and some of its strokes were both given",
            json!({ "pointer": "/ids" }),
        ));
    }
    items.sort_by_key(|i| i.index);
    let mut members: Vec<Value> = Vec::new();
    let mut created: Vec<String> = Vec::new();
    let mut ops: Vec<PatchOp> = Vec::new();
    let raw = cx.document()?.entry_raw_json(&EntryKey::Drawing(t.id.clone()))?;
    for it in &items {
        let node = raw.pointer(&it.pointer).cloned().unwrap_or(Value::Null);
        let strokes: Vec<&Located> =
            found.iter().filter(|s| s.kind == ObjectKind::Stroke && s.parent.as_deref() == Some(&it.id)).collect();
        if strokes.is_empty() {
            members.push(node);
            continue;
        }
        // Selected strokes move into a new layer with the source layer's hint (02 §8.4: separated into their own layer).
        let layer_id = cx.new_id(ObjectType::DrawingNode)?;
        let mut layer = json!({ "id": layer_id.as_str(), "type": "drawingNode", "rev": 0, "kind": "layer", "hint": node.get("hint").cloned().unwrap_or(json!("brush")), "strokes": [] });
        let mut sorted = strokes.clone();
        sorted.sort_by_key(|s| s.index);
        for s in &sorted {
            if let Some(v) = raw.pointer(&s.pointer)
                && let Some(a) = layer.get_mut("strokes").and_then(Value::as_array_mut)
            {
                a.push(v.clone());
            }
        }
        for s in sorted.iter().rev() {
            ops.push(op("remove", &s.pointer, None));
        }
        created.push(layer_id.as_str().to_owned());
        members.push(layer);
    }
    // Whole nodes leave their list (last first); stroke removals stay inside layers that remain.
    let whole: Vec<&Located> = items
        .iter()
        .filter(|i| !found.iter().any(|s| s.kind == ObjectKind::Stroke && s.parent.as_deref() == Some(&i.id)))
        .collect();
    let mut node_ops: Vec<PatchOp> = Vec::new();
    for w in whole.iter().rev() {
        node_ops.push(op("remove", &w.pointer, None));
    }
    // The group takes the place of the front-most member, counted after the removals.
    let front = items.iter().map(|i| i.index).max().unwrap_or(0);
    let removed_before = whole.iter().filter(|w| w.index <= front).count();
    let at = front + 1 - removed_before;
    let group_id = cx.new_id(ObjectType::DrawingNode)?;
    let group =
        json!({ "id": group_id.as_str(), "type": "drawingNode", "rev": 0, "kind": "group", "children": members });
    ops.extend(node_ops);
    ops.push(op("add", &format!("{list}/{at}"), Some(group)));
    changes.push((EntryKey::Drawing(t.id.clone()), ops));
    cx.patch_entries(changes)?;
    if !created.is_empty() {
        warn_zones(cx, &t.id);
    }
    Ok(GroupCreateResult { group_id: group_id.as_str().to_owned(), created_ids: created, rev: committed_rev(cx)? })
}

/// `draw.group.delete` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct GroupDeleteParams {
    /// The drawing (default: `elementId` + `frame`, else the current drawing).
    #[serde(default)]
    drawing_id: Option<String>,
    /// The element whose cell exposes the drawing.
    #[serde(default)]
    element_id: Option<String>,
    /// The frame of that cell.
    #[serde(default)]
    #[schemars(range(min = 1, max = 2_147_483_647))]
    frame: Option<u32>,
    /// Groups to dissolve (default: the selected groups).
    #[serde(default)]
    group_ids: Option<Vec<String>>,
    /// The drawing revision the caller read (02 §3.12).
    #[serde(default)]
    expect_rev: Option<u64>,
}

/// `{memberIds, rev}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GroupDeleteResult {
    /// The former members, in order.
    member_ids: Vec<String>,
    /// The drawing revision after the change.
    rev: u64,
}

#[command(
    id = "draw.group.delete",
    kind = action,
    label = "Ungroup drawing objects",
    scopes = [edit],
    undo = "Ungroup Selection",
    needs_project,
    example(title = "Dissolve the first drawing's group", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "groupIds": ["nd_01K742SG0TX3WF1BSWWGMWM5WG"]}"#),
    gui = "Drawing view context menu: Ungroup (M4 C4.2)",
)]
pub(crate) fn draw_group_delete(cx: &mut Cx, p: GroupDeleteParams) -> Result<GroupDeleteResult, CommandError> {
    let mut changes = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), p.element_id.as_deref(), p.frame, false, &mut changes)?;
    let d = t.drawing(cx.document()?)?;
    check_editable(&d, p.expect_rev)?;
    let ids = match p.group_ids {
        Some(ids) => ids,
        None => {
            let sel = selection_ids(cx, &d);
            sel.into_iter().filter(|i| vd::locate(&d, i).is_some_and(|l| l.kind == ObjectKind::Group)).collect()
        }
    };
    if ids.is_empty() {
        return Err(nothing_to_do("ungroup"));
    }
    let found = locate_all(&d, &ids, "/groupIds")?;
    if let Some((i, o)) = found.iter().enumerate().find(|(_, o)| o.kind != ObjectKind::Group) {
        return Err(err(
            codes::ARG_INVALID,
            format!("`{}` is a {}, not a group", o.id, o.kind.as_str()),
            json!({ "pointer": format!("/groupIds/{i}") }),
        ));
    }
    if found.iter().any(|g| found.iter().any(|o| o.id != g.id && g.pointer.starts_with(&format!("{}/", o.pointer)))) {
        return Err(err(
            codes::ARG_INVALID,
            "a group and a group inside it were both given; dissolve them one level at a time",
            json!({ "pointer": "/groupIds" }),
        ));
    }
    // Each list from the end, so pointers stay valid.
    let mut sorted = found.clone();
    sorted.sort_by(|a, b| b.list.cmp(&a.list).then(b.index.cmp(&a.index)));
    let raw = cx.document()?.entry_raw_json(&EntryKey::Drawing(t.id.clone()))?;
    let mut ops = Vec::new();
    let mut members = Vec::new();
    for g in &sorted {
        let Some(children) = vd::node_at(&d, &g.pointer).and_then(|n| n.children().clone()) else { continue };
        let child_json: Vec<Value> =
            raw.pointer(&format!("{}/children", g.pointer)).and_then(Value::as_array).cloned().unwrap_or_default();
        ops.push(op("remove", &g.pointer, None));
        for (k, c) in child_json.into_iter().enumerate() {
            ops.push(op("add", &format!("{}/{}", g.list, g.index + k), Some(c)));
        }
        members.extend(children.iter().map(|c| c.id().as_str().to_owned()));
    }
    changes.push((EntryKey::Drawing(t.id.clone()), ops));
    cx.patch_entries(changes)?;
    Ok(GroupDeleteResult { member_ids: members, rev: committed_rev(cx)? })
}
