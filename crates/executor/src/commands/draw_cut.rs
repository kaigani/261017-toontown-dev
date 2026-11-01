//! The Cutter and the Eraser (02 §7.2.7, §7.2.2, §13.5; M2 C2.3): `draw.cut.apply` and `draw.erase.apply`.
//!
//! A cut stages its boundary as an invisible stroke in every layer it reaches and lets the settle pass node it
//! (02 §3.12): crossing pencil lines and invisible strokes split exactly on the boundary with reported piece IDs, and
//! zones split along it keeping their fills (exact overlap carry-over). Pieces are then sorted by a point in their
//! middle. Boundary pieces that run through a zone of the layer before the cut stay as invisible edges (the cut edge
//! of the fill); the rest go. `then: "delete"` (and every erase) unpaints the zones inside and removes the inside
//! strokes, as one undo step. Brush strokes are never split (SI-059): one wholly inside is inside, one crossing the
//! boundary is kept and reported (`keptBrushIds`, SI-071). `segment` (SI-071) cuts along an open segment: crossing
//! strokes split and zones divide along it; the new pieces are the inside set.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tt_model::document::EntryKey;
use tt_model::entities::pen::PenKind;
use tt_model::{Id, ObjectType};
use tt_registry::{CommandError, command};
use tt_vector::brush::BrushPen;
use tt_vector::drawing::{self as vd, Located, ObjectKind};
use tt_vector::geom::{P, Rect};
use tt_vector::ops::cut;
use tt_vector::path::{Anchor, Path};

use super::draw::{Changed, Point, check_editable, committed_rev, err, nothing_to_do, op, removal_ops, target};
use crate::cx::Cx;

/// What a cut did.
struct Outcome {
    inside: Vec<String>,
    kept_brush: Vec<String>,
    changed: Changed,
    rev: u64,
}

fn meets(a: &Rect, b: &Rect) -> bool {
    a.min_x <= b.max_x && b.min_x <= a.max_x && a.min_y <= b.max_y && b.min_y <= a.max_y
}

/// Cuts drawing `drawing` along `boundary` (closed polygon or open polyline); `layers`: layer IDs to cut (all when
/// `None`); `delete`: remove the inside.
fn run_cut(
    cx: &mut Cx,
    drawing: &Id,
    boundary: &[P],
    closed: bool,
    delete: bool,
    layers: Option<&[String]>,
) -> Result<Outcome, CommandError> {
    let before = cx.document()?.clone();
    let d = before.drawing(drawing).ok_or_else(|| super::draw::not_found(drawing.as_str()))?.clone();
    let cache = cx.planar_cache();
    let zones_before: BTreeMap<String, std::sync::Arc<tt_vector::planar::graph::LayerMap>> =
        crate::planar::drawing_zones(&cache, &d)?.into_iter().map(|lz| (lz.layer.id.clone(), lz.map)).collect();
    let bb = boundary.iter().skip(1).fold(Rect::of(boundary[0]), |r, p| r.with(*p));
    // 1. The boundary as an invisible stroke in every layer it reaches.
    let mut ops = Vec::new();
    let mut boundaries: Vec<(String, String)> = Vec::new();
    for (pointer, n) in crate::planar::layer_nodes(&d) {
        let lid = n.id().as_str().to_owned();
        if layers.is_some_and(|l| !l.contains(&lid)) || n.strokes().is_none() {
            continue;
        }
        if !vd::node_bounds(n, true).is_some_and(|b| meets(&b, &bb)) {
            continue;
        }
        let bid = cx.new_id(ObjectType::Stroke)?.as_str().to_owned();
        ops.push(op("add", &format!("{pointer}/strokes/-"), Some(cut::boundary_stroke(&bid, boundary, closed))));
        boundaries.push((lid, bid));
    }
    if boundaries.is_empty() {
        return Err(nothing_to_do("cut"));
    }
    cx.patch_entries(vec![(EntryKey::Drawing(drawing.clone()), ops)])?;
    cx.settle()?;
    // 2. Sort the pieces.
    let noded = cx.document()?.clone();
    let dn = noded.drawing(drawing).ok_or_else(|| super::draw::not_found(drawing.as_str()))?.clone();
    let split_pieces: BTreeSet<String> = cx
        .split_strokes(drawing)
        .iter()
        .filter(|s| !boundaries.iter().any(|(_, b)| b == *s))
        .flat_map(|s| cx.pieces_of(drawing, s))
        .collect();
    let mut inside: Vec<String> = Vec::new();
    let mut kept_brush: Vec<String> = Vec::new();
    let mut drop: Vec<String> = Vec::new();
    for (lid, bid) in &boundaries {
        let pieces: BTreeSet<String> = cx.pieces_of(drawing, bid).into_iter().collect();
        let Some(l) = vd::locate(&dn, lid) else { continue };
        let Some(n) = vd::node_at(&dn, &l.pointer) else { continue };
        for s in n.strokes().iter().flatten() {
            let id = s.id().as_str().to_owned();
            let pts = s.points();
            if pieces.contains(&id) {
                let keep = cut::middle(pts, *s.closed())
                    .is_some_and(|m| zones_before.get(lid).and_then(|map| map.zone_at(m)).is_some());
                if !keep {
                    drop.push(id);
                }
                continue;
            }
            if !closed {
                if split_pieces.contains(&id) {
                    inside.push(id);
                }
                continue;
            }
            if *s.kind() == tt_model::entities::drawing::StrokeKind::Brush {
                let outline = Path::from_points(pts, true).flatten(0.25);
                if !outline.is_empty() && outline.iter().all(|p| cut::inside(boundary, *p)) {
                    inside.push(id);
                } else if cut::polygons_meet(&outline, boundary) {
                    kept_brush.push(id);
                }
                continue;
            }
            if cut::middle(pts, *s.closed()).is_some_and(|m| cut::inside(boundary, m)) {
                inside.push(id);
            }
        }
    }
    // 3. Delete: unpaint the zones inside, store that, then remove the inside strokes.
    if delete && closed {
        for lz in crate::planar::drawing_zones(&cache, &dn)? {
            let fills: BTreeMap<String, Option<tt_vector::planar::fill::Fill>> = lz
                .map
                .zones
                .iter()
                .enumerate()
                .filter(|(z, zone)| {
                    lz.fills.get(*z).is_some_and(Option::is_some) && cut::inside(boundary, zone.interior)
                })
                .map(|(_, zone)| (zone.id.clone(), None))
                .collect();
            if !fills.is_empty() {
                cx.paint_zones(drawing, &lz.layer.id, fills);
            }
        }
        cx.settle()?;
    }
    let now = cx.document()?.clone();
    let dnow = now.drawing(drawing).ok_or_else(|| super::draw::not_found(drawing.as_str()))?.clone();
    let mut gone: Vec<String> = drop.clone();
    if delete {
        gone.extend(inside.iter().cloned());
    }
    let targets: Vec<Located> = gone.iter().filter_map(|i| vd::locate(&dnow, i)).collect();
    if !targets.is_empty() {
        let (ops, _) = removal_ops(&dnow, &targets);
        cx.patch_entries(vec![(EntryKey::Drawing(drawing.clone()), ops)])?;
    }
    let mut changed =
        Changed { created_ids: boundaries.iter().map(|(_, b)| b.clone()).collect(), ..Changed::default() };
    let settled = crate::planar::settled(cx, &before, drawing, &changed.created_ids.clone())?;
    settled.report(&mut changed);
    let after = cx.document()?.clone();
    let present = |s: &String| after.drawing(drawing).is_some_and(|d| vd::locate(d, s).is_some());
    changed.created_ids.retain(present);
    if delete {
        // Inside pieces created by this cut and removed again are neither created nor removed.
        let existed = |s: &String| vd::locate(&d, s).is_some();
        changed.removed_ids.retain(|r| existed(r) || r.starts_with("zn_"));
        for i in &inside {
            if existed(i) && !changed.removed_ids.contains(i) {
                changed.removed_ids.push(i.clone());
            }
        }
    }
    let inside = if delete { Vec::new() } else { inside.into_iter().filter(present).collect() };
    Ok(Outcome { inside, kept_brush, changed, rev: committed_rev(cx)? })
}

/// What happens to the inside of a cut.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Then {
    /// Select it.
    #[default]
    Select,
    /// Delete it.
    Delete,
}

/// A rectangle `{x, y, width, height}`.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RectArea {
    /// Minimum x.
    x: f64,
    /// Minimum y.
    y: f64,
    /// Width.
    #[schemars(range(min = 0.0))]
    width: f64,
    /// Height.
    #[schemars(range(min = 0.0))]
    height: f64,
}

/// A segment `{from, to}`.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SegmentIn {
    /// Start.
    from: Point,
    /// End.
    to: Point,
}

/// `draw.cut.apply` parameters (`segment`: SI-071).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CutParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// A closed lasso.
    #[serde(default)]
    polygon: Option<Vec<Point>>,
    /// A rectangle.
    #[serde(default)]
    rect: Option<RectArea>,
    /// An open segment to cut along (splits lines and zones; SI-071).
    #[serde(default)]
    segment: Option<SegmentIn>,
    /// What happens to the inside.
    #[serde(default)]
    then: Then,
}

/// `{insideIds, removedIds, rev}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CutResult {
    /// The drawing.
    drawing_id: String,
    /// The inside objects (now selected) with `then: "select"`; the new pieces of a segment cut.
    inside_ids: Vec<String>,
    /// Brush strokes crossing the boundary, kept whole (SI-059, SI-071).
    kept_brush_ids: Vec<String>,
    /// Changed objects.
    #[serde(flatten)]
    changed: Changed,
    /// The drawing revision after the change.
    rev: u64,
}

fn pts(v: &[Point]) -> Vec<P> {
    v.iter().map(|q| P::new(q.x, q.y)).collect()
}

fn check_finite(v: &[P], pointer: &str) -> Result<(), CommandError> {
    if v.iter().all(|p| p.x.is_finite() && p.y.is_finite()) {
        Ok(())
    } else {
        Err(err("arg.invalid", "coordinates must be finite", json!({ "pointer": pointer })))
    }
}

#[command(
    id = "draw.cut.apply",
    kind = action,
    label = "Cut",
    scopes = [edit],
    undo = "Cutter Tool",
    needs_project,
    example(title = "Cut around the middle of the drawing and select the inside", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "rect": {"x": -20, "y": -20, "width": 40, "height": 40}}"#),
    example(title = "Cut a lasso out and delete it", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "polygon": [{"x": -30, "y": -30}, {"x": 30, "y": -30}, {"x": 0, "y": 30}], "then": "delete"}"#),
    example(title = "Split the lines and zones along a segment", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "segment": {"from": {"x": -100, "y": 0}, "to": {"x": 100, "y": 0}}}"#),
    gui = "Drawing view: Cutter tool (M4 C4.2)",
)]
pub(crate) fn draw_cut_apply(cx: &mut Cx, p: CutParams) -> Result<CutResult, CommandError> {
    let given = [p.polygon.is_some(), p.rect.is_some(), p.segment.is_some()].iter().filter(|b| **b).count();
    if given != 1 {
        return Err(err(
            "arg.invalid",
            "give exactly one of `polygon`, `rect` and `segment`",
            json!({ "pointer": "/polygon" }),
        ));
    }
    let (boundary, closed, pointer) = if let Some(poly) = &p.polygon {
        (cut::distinct(&pts(poly), 3), true, "/polygon")
    } else if let Some(r) = p.rect {
        let c = [
            P::new(r.x, r.y),
            P::new(r.x + r.width, r.y),
            P::new(r.x + r.width, r.y + r.height),
            P::new(r.x, r.y + r.height),
        ];
        (cut::distinct(&c, 4).filter(|_| r.width > 0.0 && r.height > 0.0), true, "/rect")
    } else if let Some(s) = p.segment {
        (cut::distinct(&[P::new(s.from.x, s.from.y), P::new(s.to.x, s.to.y)], 2), false, "/segment")
    } else {
        (None, true, "/polygon")
    };
    let boundary = boundary.ok_or_else(|| {
        err("drawing.invalidGeometry", "the cut boundary is degenerate", json!({ "pointer": pointer }))
    })?;
    check_finite(&boundary, pointer)?;
    if !closed && p.then == Then::Delete {
        return Err(err("arg.invalid", "a segment cut has no inside to delete", json!({ "pointer": "/then" })));
    }
    let mut none = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut none)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    check_editable(&d, None)?;
    let o = run_cut(cx, &t.id, &boundary, closed, p.then == Then::Delete, None)?;
    if p.then == Then::Select {
        let now = cx.document()?.clone();
        if let Some(dn) = now.drawing(&t.id) {
            super::draw_select::set_drawing_selection(cx, &dn.clone(), o.inside.clone());
        }
    }
    Ok(CutResult {
        drawing_id: t.id.as_str().to_owned(),
        inside_ids: o.inside,
        kept_brush_ids: o.kept_brush,
        changed: o.changed,
        rev: o.rev,
    })
}

/// An eraser path point.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ErasePoint {
    /// x (du).
    x: f64,
    /// y (du).
    y: f64,
    /// Pressure (0–1, default 1).
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 1.0))]
    pressure: Option<f64>,
}

/// An explicit round eraser `{minSize, maxSize}` (SI-072).
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct EraserPen {
    /// Width at lowest pressure (du).
    #[schemars(range(min = 0.0, max = 100.0))]
    min_size: f64,
    /// Width at full pressure (du).
    #[schemars(range(min = 0.1, max = 100.0))]
    max_size: f64,
}

/// `"all"`.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AllLayers {
    /// Every layer.
    All,
}

/// Layers an erase reaches.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(untagged)]
pub(crate) enum LayersIn {
    /// Every layer.
    All(AllLayers),
    /// These layers.
    Ids(Vec<String>),
}

impl Default for LayersIn {
    fn default() -> Self {
        LayersIn::All(AllLayers::All)
    }
}

/// `draw.erase.apply` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct EraseParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// The eraser's path (swept with the eraser pen).
    #[serde(default)]
    path: Option<Vec<ErasePoint>>,
    /// The eraser pen (default: the project's first eraser pen, "Eraser 1").
    #[serde(default)]
    pen_id: Option<String>,
    /// An explicit eraser (SI-072).
    #[serde(default)]
    pen: Option<EraserPen>,
    /// Or: a closed region to erase.
    #[serde(default)]
    region: Option<Vec<Point>>,
    /// Layers to erase.
    #[serde(default)]
    layers: LayersIn,
}

/// `{removedIds, createdIds, rev}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EraseResult {
    /// The drawing.
    drawing_id: String,
    /// Brush strokes crossing the erased area, kept whole (SI-059, SI-071).
    kept_brush_ids: Vec<String>,
    /// Changed objects.
    #[serde(flatten)]
    changed: Changed,
    /// The drawing revision after the change.
    rev: u64,
}

fn eraser(cx: &Cx, pen_id: Option<&str>, pen: Option<EraserPen>) -> Result<BrushPen, CommandError> {
    if let Some(p) = pen {
        return Ok(BrushPen {
            tip: tt_model::entities::pen::Tip::Round,
            min_size: p.min_size.min(p.max_size),
            max_size: p.max_size,
            tip_angle: 0.0,
        });
    }
    let doc = cx.document()?;
    let pens = doc.project().pens();
    let chosen = match pen_id {
        Some(r) => {
            let key = cx.resolve_as(r, ObjectType::Pen)?;
            let p = pens
                .iter()
                .find(|p| p.id().as_str() == key)
                .ok_or_else(|| err("pen.notFound", format!("no pen `{r}`"), json!({ "pointer": "/penId" })))?;
            if *p.kind() != PenKind::Eraser {
                return Err(err(
                    "arg.invalid",
                    format!("pen `{}` is not an eraser", p.name()),
                    json!({ "pointer": "/penId" }),
                ));
            }
            Some(p)
        }
        None => pens.iter().find(|p| *p.kind() == PenKind::Eraser),
    };
    Ok(match chosen {
        Some(p) => {
            BrushPen { tip: *p.tip(), min_size: *p.min_size(), max_size: *p.max_size(), tip_angle: *p.tip_angle() }
        }
        None => BrushPen { tip: tt_model::entities::pen::Tip::Round, min_size: 2.5, max_size: 10.0, tip_angle: 0.0 },
    })
}

/// The area an eraser path sweeps, as a polygon.
fn swept(path: &[ErasePoint], pen: &BrushPen) -> Result<Vec<P>, CommandError> {
    let pts: Vec<P> = path.iter().map(|q| P::new(q.x, q.y)).collect();
    check_finite(&pts, "/path")?;
    let distinct = cut::distinct(&pts, 1).unwrap_or_default();
    let spine = if distinct.len() == 1 {
        Path { anchors: vec![Anchor::corner(distinct[0])], closed: true }
    } else {
        Path { anchors: distinct.iter().map(|p| Anchor::corner(*p)).collect(), closed: false }
    };
    let total: f64 = pts.windows(2).map(|w| w[0].dist(w[1])).sum();
    let mut acc = 0.0;
    let mut pressure = Vec::with_capacity(path.len());
    for (i, q) in path.iter().enumerate() {
        if i > 0 {
            acc += pts[i - 1].dist(pts[i]);
        }
        let t = if total > 0.0 { acc / total } else { 0.0 };
        pressure.push([t, q.pressure.unwrap_or(1.0)]);
    }
    let outline = tt_vector::brush::outline(&spine, &pressure, pen).map_err(|e| super::draw::invalid_geometry(&e))?;
    cut::distinct(&outline.flatten(0.25), 3)
        .ok_or_else(|| err("drawing.invalidGeometry", "the eraser path sweeps no area", json!({ "pointer": "/path" })))
}

#[command(
    id = "draw.erase.apply",
    kind = action,
    label = "Erase",
    scopes = [edit],
    undo = "Eraser Tool",
    needs_project,
    example(title = "Erase along a short path with the default eraser", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "path": [{"x": -60, "y": 0}, {"x": 60, "y": 0}]}"#),
    example(title = "Erase a closed region", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "region": [{"x": -10, "y": -10}, {"x": 10, "y": -10}, {"x": 10, "y": 10}, {"x": -10, "y": 10}]}"#),
    gui = "Drawing and Camera views: Eraser tool (M4 C4.2)",
)]
pub(crate) fn draw_erase_apply(cx: &mut Cx, p: EraseParams) -> Result<EraseResult, CommandError> {
    let area = match (&p.path, &p.region) {
        (Some(path), None) if !path.is_empty() => swept(path, &eraser(cx, p.pen_id.as_deref(), p.pen)?)?,
        (None, Some(r)) => {
            let v = cut::distinct(&pts(r), 3).ok_or_else(|| {
                err("drawing.invalidGeometry", "the region is degenerate", json!({ "pointer": "/region" }))
            })?;
            check_finite(&v, "/region")?;
            v
        }
        _ => return Err(err("arg.invalid", "give exactly one of `path` and `region`", json!({ "pointer": "/path" }))),
    };
    let mut none = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut none)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    check_editable(&d, None)?;
    let layers: Option<Vec<String>> = match &p.layers {
        LayersIn::All(_) => None,
        LayersIn::Ids(ids) => {
            let mut out = Vec::new();
            for (i, id) in ids.iter().enumerate() {
                let local = vd::local_id(d.id().as_str(), id).unwrap_or(id);
                match vd::locate(&d, local) {
                    Some(l) if l.kind == ObjectKind::Layer => out.push(l.id),
                    _ => {
                        return Err(err(
                            "drawing.objectNotFound",
                            format!("`{id}` is not a layer of drawing `{}`", d.name()),
                            json!({ "pointer": format!("/layers/{i}") }),
                        ));
                    }
                }
            }
            Some(out)
        }
    };
    let o = run_cut(cx, &t.id, &area, true, true, layers.as_deref())?;
    Ok(EraseResult {
        drawing_id: t.id.as_str().to_owned(),
        kept_brush_ids: o.kept_brush,
        changed: o.changed,
        rev: o.rev,
    })
}
