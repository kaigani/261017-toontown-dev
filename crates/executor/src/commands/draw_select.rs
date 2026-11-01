//! Drawing selection, point selection and drawing-level settings (02 §7.1, §9, §13.5; M2 C2.3): `draw.selection.*`,
//! `draw.pointSelection.set`, `draw.bounds.get`, `draw.viewMapping.get`, `draw.inspectorPreview.set`,
//! `draw.drawing.rename` and `draw.lock.set`.
//!
//! The drawing selection is the `drawing` context's selection of the app context (compound keys
//! `<drawingId>/<localId>`), the one `edit.selection.*` reads; every `draw.*` command that defaults `ids` to the
//! selection reads it. Zones are selectable by their derived IDs; like every selected object that no longer exists,
//! they leave the selection when a commit changes them (zone IDs follow their boundary, 02 §3.12 item 4). The point
//! selection (Contour Editor) is kept beside it, pruned the same way.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_model::document::EntryKey;
use tt_model::entities::drawing::Drawing;
use tt_registry::{CommandError, command};
use tt_vector::geom::{P, Rect};

use super::draw::{Bounds, Changed, Point, committed_rev, err, op, target};
use crate::cx::Cx;
use crate::planar::drawing_zones;

/// `{x, y, width, height}` input.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RectIn {
    /// Left.
    x: f64,
    /// Bottom.
    y: f64,
    /// Width (≥ 0).
    #[schemars(range(min = 0.0))]
    width: f64,
    /// Height (≥ 0).
    #[schemars(range(min = 0.0))]
    height: f64,
}

impl RectIn {
    fn rect(self) -> Rect {
        Rect::of(P::new(self.x, self.y)).with(P::new(self.x + self.width, self.y + self.height))
    }
}

/// How `draw.selection.set` selects.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SelectBy {
    /// The given IDs.
    #[default]
    Ids,
    /// The front-most object under a point (02 §8.1 hit order).
    Point,
    /// Objects in a rectangle (`rectRule`).
    Rect,
    /// Objects in a closed polygon (`rectRule`).
    Lasso,
    /// Every stroke, text and bitmap.
    All,
    /// Nothing.
    None,
}

/// Inside or intersecting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RectRule {
    /// The object's bounds lie wholly inside.
    #[default]
    Inside,
    /// The object's bounds meet the region.
    Intersect,
}

/// How a selection combines with the current one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Mode {
    /// Replace it.
    #[default]
    Replace,
    /// Add to it.
    Add,
    /// Toggle membership.
    Toggle,
    /// Remove from it.
    Remove,
}

fn combine<T: PartialEq + Clone>(cur: &mut Vec<T>, keys: Vec<T>, mode: Mode) {
    match mode {
        Mode::Replace => *cur = keys,
        Mode::Add => keys.into_iter().for_each(|k| {
            if !cur.contains(&k) {
                cur.push(k);
            }
        }),
        Mode::Remove => cur.retain(|k| !keys.contains(k)),
        Mode::Toggle => {
            for k in keys {
                match cur.iter().position(|x| *x == k) {
                    Some(i) => {
                        cur.remove(i);
                    }
                    None => cur.push(k),
                }
            }
        }
    }
}

/// `draw.selection.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SelectionSetParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// How to select.
    #[serde(default)]
    by: SelectBy,
    /// IDs (`by: "ids"`; local or compound; zone IDs select zones).
    #[serde(default)]
    ids: Option<Vec<String>>,
    /// The point (`by: "point"`).
    #[serde(default)]
    point: Option<Point>,
    /// The rectangle (`by: "rect"`).
    #[serde(default)]
    rect: Option<RectIn>,
    /// The polygon (`by: "lasso"`).
    #[serde(default)]
    polygon: Option<Vec<Point>>,
    /// Region rule for `rect` and `lasso`.
    #[serde(default)]
    rect_rule: RectRule,
    /// How to combine with the current selection.
    #[serde(default)]
    mode: Mode,
}

/// `{selectedIds, bounds}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SelectionSetResult {
    /// Selected objects (local IDs), in selection order.
    selected_ids: Vec<String>,
    /// Their bounds (`null` when nothing is selected).
    bounds: Option<Bounds>,
}

/// A selectable object of a drawing: local ID and bounds.
fn selectables(cx: &Cx, d: &Drawing, with_zones: bool) -> Result<Vec<(String, Option<Rect>)>, CommandError> {
    let mut out = Vec::new();
    for o in tt_vector::drawing::objects(d) {
        let b = match o.kind {
            tt_vector::drawing::ObjectKind::Stroke => {
                tt_vector::drawing::stroke_at(d, &o.pointer).and_then(tt_vector::drawing::stroke_bounds)
            }
            tt_vector::drawing::ObjectKind::Text | tt_vector::drawing::ObjectKind::Bitmap => {
                tt_vector::drawing::node_at(d, &o.pointer).and_then(|n| tt_vector::drawing::node_bounds(n, false))
            }
            _ => continue,
        };
        out.push((o.id, b));
    }
    if with_zones {
        for lz in drawing_zones(&cx.planar_cache(), d)? {
            for z in &lz.map.zones {
                out.push((z.id.clone(), Some(z.bounds)));
            }
        }
    }
    Ok(out)
}

/// Lasso containment, exact on the snapped grid (the shared planar predicates, C23-R1-08); the boundary counts.
fn point_in_polygon(poly: &[P], q: P) -> bool {
    tt_vector::planar::predicates::inside_polygon(poly, q, true)
}

fn corners(r: &Rect) -> [P; 4] {
    [P::new(r.min_x, r.min_y), P::new(r.max_x, r.min_y), P::new(r.max_x, r.max_y), P::new(r.min_x, r.max_y)]
}

/// The bounds of selected objects (local IDs), zones included.
pub(crate) fn selection_bounds(cx: &Cx, d: &Drawing, ids: &[String]) -> Result<Option<Rect>, CommandError> {
    let all = selectables(cx, d, ids.iter().any(|i| i.starts_with("zn_")))?;
    let mut acc: Option<Rect> = None;
    for (id, b) in all {
        if ids.contains(&id)
            && let Some(b) = b
        {
            acc = Some(match acc {
                Some(a) => a.with(P::new(b.min_x, b.min_y)).with(P::new(b.max_x, b.max_y)),
                None => b,
            });
        }
    }
    // Layers and groups: the bounds of their contents.
    for id in ids {
        if let Some(l) = tt_vector::drawing::locate(d, id)
            && matches!(l.kind, tt_vector::drawing::ObjectKind::Layer | tt_vector::drawing::ObjectKind::Group)
            && let Some(b) =
                tt_vector::drawing::node_at(d, &l.pointer).and_then(|n| tt_vector::drawing::node_bounds(n, false))
        {
            acc = Some(match acc {
                Some(a) => a.with(P::new(b.min_x, b.min_y)).with(P::new(b.max_x, b.max_y)),
                None => b,
            });
        }
    }
    Ok(acc)
}

fn bounds_json(r: Option<Rect>) -> Option<Bounds> {
    r.map(Bounds::from)
}

/// The local IDs of the drawing selection of `d`.
pub(crate) fn selected(cx: &Cx, d: &Drawing) -> Vec<String> {
    super::draw::selection_ids(cx, d)
}

pub(crate) fn set_drawing_selection(cx: &mut Cx, d: &Drawing, local: Vec<String>) {
    let prefix = d.id().as_str();
    let others: Vec<String> =
        cx.app().selection_of("drawing").into_iter().filter(|k| !k.starts_with(&format!("{prefix}/"))).collect();
    let mut keys = others;
    keys.extend(local.iter().map(|l| format!("{prefix}/{l}")));
    cx.app_mut().set_selection("drawing", keys);
    let points = point_json(cx, d);
    cx.emit("draw.selection.changed", json!({ "drawingId": prefix, "selectedIds": local, "selectedPoints": points }));
}

fn point_json(cx: &Cx, d: &Drawing) -> Vec<Value> {
    let prefix = format!("{}/", d.id().as_str());
    cx.app()
        .points
        .iter()
        .filter_map(|(k, a)| k.strip_prefix(&prefix).map(|s| json!({ "strokeId": s, "anchor": a })))
        .collect()
}

#[command(
    id = "draw.selection.set",
    kind = ui,
    label = "Select drawing objects",
    scopes = [ui],
    needs_project,
    example(title = "Select the pencil line of Bob's first drawing", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "ids": ["st_01K742SG0R61SYK9GG0HGYVNBT"]}"#),
    example(title = "Select everything inside a rectangle", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "by": "rect", "rect": {"x": -200, "y": -200, "width": 400, "height": 400}}"#),
    gui = "Drawing view: Select tool click, drag and lasso (M4 C4.2)",
)]
pub(crate) fn draw_selection_set(cx: &mut Cx, p: SelectionSetParams) -> Result<SelectionSetResult, CommandError> {
    let mut none = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut none)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    let need = |what: &str| {
        err("arg.invalid", format!("`by: {what}` needs `{what}`"), json!({ "pointer": format!("/{what}") }))
    };
    let keys: Vec<String> = match p.by {
        SelectBy::None => Vec::new(),
        SelectBy::All => selectables(cx, &d, false)?.into_iter().map(|(id, _)| id).collect(),
        SelectBy::Ids => {
            let ids = p.ids.clone().ok_or_else(|| need("ids"))?;
            let zones = selectables(cx, &d, true)?;
            let mut out = Vec::new();
            for (i, id) in ids.iter().enumerate() {
                let local = tt_vector::drawing::local_id(d.id().as_str(), id).unwrap_or(id).to_owned();
                let ok = tt_vector::drawing::locate(&d, &local).is_some() || zones.iter().any(|(z, _)| *z == local);
                if !ok {
                    return Err(err(
                        "drawing.objectNotFound",
                        format!("`{id}` is not an object of the drawing"),
                        json!({ "pointer": format!("/ids/{i}"), "id": id }),
                    ));
                }
                out.push(local);
            }
            out
        }
        SelectBy::Point => {
            let pt = p.point.ok_or_else(|| need("point"))?;
            let zones = drawing_zones(&cx.planar_cache(), &d)?;
            let hs = super::colour::hits(&d, &zones, P::new(pt.x, pt.y), 0.0, false)?;
            hs.first().map(|h| vec![h.id.clone()]).unwrap_or_default()
        }
        SelectBy::Rect | SelectBy::Lasso => {
            let poly: Vec<P> = if p.by == SelectBy::Rect {
                corners(&p.rect.ok_or_else(|| need("rect"))?.rect()).to_vec()
            } else {
                let poly = p.polygon.clone().ok_or_else(|| need("polygon"))?;
                if poly.len() < 3 {
                    return Err(err(
                        "arg.invalid",
                        "a lasso needs at least 3 points",
                        json!({ "pointer": "/polygon" }),
                    ));
                }
                poly.iter().map(|q| P::new(q.x, q.y)).collect()
            };
            let region = poly.iter().skip(1).fold(Rect::of(poly[0]), |r, q| r.with(*q));
            selectables(cx, &d, true)?
                .into_iter()
                .filter(|(_, b)| {
                    let Some(b) = b else { return false };
                    let cs = corners(b);
                    match p.rect_rule {
                        RectRule::Inside => cs.iter().all(|c| point_in_polygon(&poly, *c)),
                        RectRule::Intersect => {
                            b.min_x <= region.max_x
                                && b.max_x >= region.min_x
                                && b.min_y <= region.max_y
                                && b.max_y >= region.min_y
                                && (cs.iter().any(|c| point_in_polygon(&poly, *c))
                                    || poly
                                        .iter()
                                        .any(|q| q.x >= b.min_x && q.x <= b.max_x && q.y >= b.min_y && q.y <= b.max_y))
                        }
                    }
                })
                .map(|(id, _)| id)
                .collect()
        }
    };
    let mut cur = selected(cx, &d);
    combine(&mut cur, keys, p.mode);
    let bounds = selection_bounds(cx, &d, &cur)?;
    set_drawing_selection(cx, &d, cur.clone());
    Ok(SelectionSetResult { selected_ids: cur, bounds: bounds_json(bounds) })
}

/// `draw.selection.get` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SelectionGetParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
}

/// An anchor of a stroke.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct AnchorRef {
    /// The stroke (local ID).
    pub stroke_id: String,
    /// The anchor index.
    pub anchor: u32,
}

/// `{selectedIds, bounds, selectedPoints}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SelectionGetResult {
    /// Selected objects (local IDs).
    selected_ids: Vec<String>,
    /// Their bounds.
    bounds: Option<Bounds>,
    /// Selected anchors (Contour Editor).
    selected_points: Vec<AnchorRef>,
}

#[command(
    id = "draw.selection.get",
    kind = query,
    label = "Get the drawing selection",
    scopes = [read],
    needs_project,
    example(title = "The current drawing's selection", params = r#"{"drawingId": "drawingBank:#1/drawing:#1"}"#),
    agent_only = "the GUI shows the selection in the Drawing view",
)]
pub(crate) fn draw_selection_get(cx: &mut Cx, p: SelectionGetParams) -> Result<SelectionGetResult, CommandError> {
    let mut none = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut none)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    let ids = selected(cx, &d);
    let bounds = selection_bounds(cx, &d, &ids)?;
    Ok(SelectionGetResult { selected_ids: ids, bounds: bounds_json(bounds), selected_points: point_refs(cx, &d) })
}

/// The point selection of a drawing.
pub(crate) fn point_refs(cx: &Cx, d: &Drawing) -> Vec<AnchorRef> {
    let prefix = format!("{}/", d.id().as_str());
    cx.app()
        .points
        .iter()
        .filter_map(|(k, a)| k.strip_prefix(&prefix).map(|s| AnchorRef { stroke_id: s.to_owned(), anchor: *a }))
        .collect()
}

/// `draw.pointSelection.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PointSelectionParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Anchors.
    #[serde(default)]
    points: Option<Vec<AnchorRef>>,
    /// Every anchor inside a rectangle.
    #[serde(default)]
    rect: Option<RectIn>,
    /// How to combine.
    #[serde(default)]
    mode: Mode,
}

/// `{selectedPoints}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PointSelectionResult {
    /// Selected anchors.
    selected_points: Vec<AnchorRef>,
}

#[command(
    id = "draw.pointSelection.set",
    kind = ui,
    label = "Select anchor points",
    scopes = [ui],
    needs_project,
    example(title = "Select the first anchor of the pencil line", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "points": [{"strokeId": "st_01K742SG0R61SYK9GG0HGYVNBT", "anchor": 0}]}"#),
    gui = "Drawing view: Contour Editor point selection (M4 C4.2)",
)]
pub(crate) fn draw_point_selection_set(
    cx: &mut Cx,
    p: PointSelectionParams,
) -> Result<PointSelectionResult, CommandError> {
    let mut none = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut none)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    let mut keys: Vec<AnchorRef> = Vec::new();
    for (i, a) in p.points.iter().flatten().enumerate() {
        let ok = tt_vector::drawing::locate(&d, &a.stroke_id)
            .and_then(|l| tt_vector::drawing::stroke_at(&d, &l.pointer))
            .is_some_and(|s| (a.anchor as usize) < s.points().len());
        if !ok {
            return Err(err(
                "drawing.objectNotFound",
                format!("stroke `{}` has no anchor {}", a.stroke_id, a.anchor),
                json!({ "pointer": format!("/points/{i}") }),
            ));
        }
        keys.push(a.clone());
    }
    if let Some(r) = p.rect {
        let r = r.rect();
        for o in tt_vector::drawing::objects(&d) {
            let Some(s) = tt_vector::drawing::stroke_at(&d, &o.pointer) else { continue };
            for (k, pt) in s.points().iter().enumerate() {
                if pt[0] >= r.min_x && pt[0] <= r.max_x && pt[1] >= r.min_y && pt[1] <= r.max_y {
                    keys.push(AnchorRef { stroke_id: o.id.clone(), anchor: k as u32 });
                }
            }
        }
    }
    let mut cur = point_refs(cx, &d);
    combine(&mut cur, keys, p.mode);
    let prefix = d.id().as_str().to_owned();
    let others: Vec<(String, u32)> =
        cx.app().points.iter().filter(|(k, _)| !k.starts_with(&format!("{prefix}/"))).cloned().collect();
    let mut all = others;
    all.extend(cur.iter().map(|a| (format!("{prefix}/{}", a.stroke_id), a.anchor)));
    cx.app_mut().points = all;
    let ids = selected(cx, &d);
    let points = point_json(cx, &d);
    cx.emit("draw.selection.changed", json!({ "drawingId": prefix, "selectedIds": ids, "selectedPoints": points }));
    Ok(PointSelectionResult { selected_points: cur })
}

/// `draw.selection.changed` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SelectionChanged {
    /// The drawing.
    drawing_id: String,
    /// Selected objects.
    selected_ids: Vec<String>,
    /// Selected anchors.
    selected_points: Vec<AnchorRef>,
}

#[command(
    id = "draw.selection.changed",
    kind = event,
    label = "Drawing selection changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn draw_selection_changed(
    _cx: &mut Cx,
    _p: super::common::NoParams,
) -> Result<SelectionChanged, CommandError> {
    Err(super::common::event_not_invocable("draw.selection.changed"))
}

// ----- draw.bounds.get -----------------------------------------------------------------------------------------------

/// `draw.bounds.get` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct BoundsParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Objects (default: the whole drawing).
    #[serde(default)]
    ids: Option<Vec<String>>,
    /// Include invisible strokes.
    #[serde(default)]
    include_invisible: bool,
}

/// `{bounds}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct BoundsResult {
    /// The bounding box (`null` for an empty drawing or selection).
    bounds: Option<Bounds>,
}

#[command(
    id = "draw.bounds.get",
    kind = query,
    label = "Get bounds",
    scopes = [read],
    needs_project,
    example(title = "Bounds of Bob's first drawing", params = r#"{"drawingId": "drawingBank:#1/drawing:#1"}"#),
    agent_only = "the GUI shows bounds as the selection box",
)]
pub(crate) fn draw_bounds_get(cx: &mut Cx, p: BoundsParams) -> Result<BoundsResult, CommandError> {
    let mut none = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut none)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    let b = match &p.ids {
        None => tt_vector::drawing::drawing_bounds(&d, p.include_invisible),
        Some(ids) => {
            let local: Vec<String> =
                ids.iter().map(|i| tt_vector::drawing::local_id(d.id().as_str(), i).unwrap_or(i).to_owned()).collect();
            for (i, l) in local.iter().enumerate() {
                if tt_vector::drawing::locate(&d, l).is_none() && !l.starts_with("zn_") {
                    return Err(err(
                        "drawing.objectNotFound",
                        format!("`{l}` is not an object of the drawing"),
                        json!({ "id": l, "pointer": format!("/ids/{i}") }),
                    ));
                }
            }
            selection_bounds(cx, &d, &local)?
        }
    };
    Ok(BoundsResult { bounds: bounds_json(b) })
}

// ----- draw.viewMapping.get --------------------------------------------------------------------------------------------

/// `draw.viewMapping.get` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ViewMappingParams {
    /// The view (`drawing`; views arrive with M4 C4.2).
    #[serde(default = "drawing_view")]
    view: String,
}

fn drawing_view() -> String {
    "drawing".into()
}

/// The mapping between a view's pixels and drawing space.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ViewMappingResult {
    /// The view's drawing (`null` without a current drawing).
    drawing_id: Option<String>,
    /// Drawing space → view pixels.
    drawing_to_view: [f64; 6],
    /// View pixels → drawing space.
    view_to_drawing: [f64; 6],
    /// Zoom (1 = 100 %).
    zoom: f64,
    /// Rotation (degrees).
    rotation: f64,
    /// The view's size in pixels.
    view_size: ViewSize,
}

/// `{width, height}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ViewSize {
    /// Width.
    width: u32,
    /// Height.
    height: u32,
}

/// The headless Drawing view (SI-069): 1920 × 1080 pixels, 100 % zoom (1 view pixel = 1 du, SI-063), no rotation,
/// the drawing origin at the view centre, y up in drawing space and down in pixels.
const VIEW_W: u32 = 1920;
const VIEW_H: u32 = 1080;

#[command(
    id = "draw.viewMapping.get",
    kind = query,
    label = "Get the view mapping",
    scopes = [read],
    example(title = "The Drawing view's mapping", params = r#"{"view": "drawing"}"#),
    agent_only = "agents convert between view pixels and drawing units",
)]
pub(crate) fn draw_view_mapping_get(cx: &mut Cx, p: ViewMappingParams) -> Result<ViewMappingResult, CommandError> {
    if p.view != "drawing" {
        return Err(err(
            "view.notFound",
            format!(
                "view `{}` does not exist (views arrive with M4 C4.2; `drawing` is the headless Drawing view)",
                p.view
            ),
            json!({ "pointer": "/view" }),
        ));
    }
    let drawing_id = if cx.has_project() {
        let mut none = Vec::new();
        target(cx, None, None, None, false, &mut none).ok().map(|t| t.id.as_str().to_owned())
    } else {
        None
    };
    let (cx0, cy0) = (f64::from(VIEW_W) / 2.0, f64::from(VIEW_H) / 2.0);
    Ok(ViewMappingResult {
        drawing_id,
        drawing_to_view: [1.0, 0.0, 0.0, -1.0, cx0, cy0],
        view_to_drawing: [1.0, 0.0, 0.0, -1.0, -cx0, cy0],
        zoom: 1.0,
        rotation: 0.0,
        view_size: ViewSize { width: VIEW_W, height: VIEW_H },
    })
}

// ----- draw.inspectorPreview.set ---------------------------------------------------------------------------------------

/// `{visible}`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PreviewParams {
    /// Show the drawing thumbnail in the inspector.
    visible: bool,
}

/// `{visible}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct PreviewResult {
    /// Shown.
    visible: bool,
}

/// The persisted preference key of the inspector thumbnail.
const PREVIEW_PREF: &str = "drawing.inspectorPreview";

#[command(
    id = "draw.inspectorPreview.set",
    kind = ui,
    label = "Show drawing preview",
    scopes = [prefs],
    example(title = "Show the thumbnail", params = r#"{"visible": true}"#),
    gui = "Properties view: Drawing tab, Show preview (M4 C4.1)",
)]
pub(crate) fn draw_inspector_preview_set(cx: &mut Cx, p: PreviewParams) -> Result<PreviewResult, CommandError> {
    let mut prefs = serde_json::Map::new();
    prefs.insert(PREVIEW_PREF.into(), json!(p.visible));
    cx.defer(crate::cx::Effect::Prefs(prefs));
    Ok(PreviewResult { visible: p.visible })
}

// ----- draw.drawing.rename / draw.lock.set -----------------------------------------------------------------------------

/// `draw.drawing.rename` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RenameParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// The new name (02 §9).
    #[schemars(length(min = 1, max = 256))]
    name: String,
}

/// `{drawingId, name}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RenameResult {
    /// The drawing.
    drawing_id: String,
    /// Its name.
    name: String,
    /// Changed objects.
    #[serde(flatten)]
    changed: Changed,
    /// The drawing revision after the change.
    rev: u64,
}

#[command(
    id = "draw.drawing.rename",
    kind = action,
    label = "Rename drawing",
    scopes = [edit],
    undo = "Rename Drawing",
    needs_project,
    example(title = "Rename Bob's first drawing", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "name": "key-1"}"#),
    gui = "Properties view: Drawing tab, Drawing name (M4 C4.1)",
)]
pub(crate) fn draw_drawing_rename(cx: &mut Cx, p: RenameParams) -> Result<RenameResult, CommandError> {
    let name = p.name.trim().to_owned();
    if name.is_empty() || name.chars().any(|c| "\\/:*?<>\"|.".contains(c)) {
        return Err(err(
            "drawing.nameInvalid",
            format!("`{}` is not a valid drawing name (non-empty, none of \\ / : * ? < > \" | .)", p.name),
            json!({ "pointer": "/name" }),
        ));
    }
    let mut changes = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut changes)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?;
    if let Some(bank) = doc.bank_of(&t.id) {
        let clash = doc
            .bank(bank)
            .into_iter()
            .flat_map(|b| b.drawings().ids().iter())
            .filter(|id| **id != t.id)
            .filter_map(|id| doc.drawing(id))
            .find(|o| *o.name() == name);
        if let Some(o) = clash {
            return Err(err(
                "drawing.nameExists",
                format!("drawing `{}` of the element is already named `{name}`", o.id()),
                json!({ "pointer": "/name", "drawingId": o.id().as_str() }),
            ));
        }
    }
    if *d.name() != name {
        changes.push((EntryKey::Drawing(t.id.clone()), vec![op("replace", "/name", Some(json!(name)))]));
        cx.patch_entries(changes)?;
    }
    let changed = Changed { modified_ids: vec![t.id.as_str().to_owned()], ..Changed::default() };
    Ok(RenameResult { drawing_id: t.id.as_str().to_owned(), name, changed, rev: committed_rev(cx)? })
}

/// `draw.lock.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct LockParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Locked.
    locked: bool,
}

/// `{drawingId, locked}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LockResult {
    /// The drawing.
    drawing_id: String,
    /// Locked.
    locked: bool,
    /// The drawing revision after the change.
    rev: u64,
}

#[command(
    id = "draw.lock.set",
    kind = toggle,
    label = "Lock drawing",
    scopes = [edit],
    undo = "Lock Drawing",
    needs_project,
    example(title = "Lock Bob's first drawing", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "locked": true}"#),
    gui = "Drawing view: context menu Lock/Unlock Drawing (M4 C4.2)",
)]
pub(crate) fn draw_lock_set(cx: &mut Cx, p: LockParams) -> Result<LockResult, CommandError> {
    let mut changes = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut changes)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?;
    if *d.locked() != p.locked {
        changes.push((EntryKey::Drawing(t.id.clone()), vec![op("replace", "/locked", Some(json!(p.locked)))]));
        cx.patch_entries(changes)?;
        if p.locked {
            cx.set_label("Lock Drawing");
        } else {
            cx.set_label("Unlock Drawing");
        }
    }
    Ok(LockResult { drawing_id: t.id.as_str().to_owned(), locked: p.locked, rev: committed_rev(cx)? })
}
