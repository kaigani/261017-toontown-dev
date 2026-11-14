//! Transforms and contour editing (02 §7.1.1–§7.1.4, §8.13, §13.5; M2 C2.3): `draw.transform.apply`,
//! `draw.flip.apply`, `draw.rotate.apply`, `draw.envelope.apply`, `draw.repositionAll.apply`, `draw.point.add`,
//! `draw.point.move`, `draw.point.set`, `draw.point.delete` and `draw.segment.move`.
//!
//! Maps come from `tt_vector::ops::transform` (fill matrices, text and bitmap transforms move with the geometry,
//! reflections swap side fills, brush spines survive only translations, SI-070). Every edit stages entry patches; the
//! transaction's settle pass re-nodes the affected layers and carries fills (02 §3.12), and results report the IDs the
//! settle pass leaves (pieces of split strokes). `draw.transform.apply` also moves a subset of anchors (`points`, or
//! the Contour Editor's point selection) for the M4 contour and selection tools.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_model::document::{Document, EntryKey};
use tt_model::entities::drawing::Drawing;
use tt_model::{Id, ObjectType};
use tt_registry::{CommandError, command};
use tt_vector::drawing::{self as vd, Located, ObjectKind};
use tt_vector::geom::{Affine, P, Rect};
use tt_vector::ops::transform::{self as tf, Envelope, Map};
use tt_vector::path::Path;

use super::draw::{Changed, Point, check_editable, committed_rev, err, nothing_to_do, op, target};
use super::draw_select::AnchorRef;
use crate::cx::Cx;

/// The objects a command acts on: `ids` or the drawing selection, located; zones (derived) are skipped, and an object
/// inside another listed node is covered by it.
fn objects_of(cx: &Cx, d: &Drawing, ids: Option<&Vec<String>>, what: &str) -> Result<Vec<Located>, CommandError> {
    let list: Vec<String> = match ids {
        Some(v) => v.clone(),
        None => super::draw::selection_ids(cx, d),
    };
    let mut found: Vec<Located> = Vec::new();
    for (i, id) in list.iter().enumerate() {
        let local = vd::local_id(d.id().as_str(), id).unwrap_or(id);
        if local.starts_with("zn_") {
            // A zone stands for its bounding strokes; a stale zone ID refuses the call (C23-R1-09).
            let (_, strokes) = super::draw::zone_target(cx, d, local, &format!("/ids/{i}"))?;
            for s in strokes {
                if let Some(l) = vd::locate(d, &s)
                    && !found.iter().any(|f| f.id == l.id)
                {
                    found.push(l);
                }
            }
            continue;
        }
        let Some(l) = vd::locate(d, local) else {
            return Err(err(
                "drawing.objectNotFound",
                format!("`{id}` is not an object of drawing `{}`", d.name()),
                json!({ "id": id, "pointer": format!("/ids/{i}") }),
            ));
        };
        if !found.iter().any(|f| f.id == l.id) {
            found.push(l);
        }
    }
    let pointers: Vec<String> = found.iter().map(|l| l.pointer.clone()).collect();
    found.retain(|l| !pointers.iter().any(|p| *p != l.pointer && l.pointer.starts_with(&format!("{p}/"))));
    if found.is_empty() {
        return Err(nothing_to_do(what));
    }
    Ok(found)
}

/// The bounds of located objects.
fn bounds_of(d: &Drawing, objs: &[Located]) -> Option<Rect> {
    let mut acc: Option<Rect> = None;
    for o in objs {
        let b = match o.kind {
            ObjectKind::Stroke => vd::stroke_at(d, &o.pointer).and_then(vd::stroke_bounds),
            _ => vd::node_at(d, &o.pointer).and_then(|n| vd::node_bounds(n, true)),
        };
        if let Some(b) = b {
            acc = Some(match acc {
                Some(a) => a.with(P::new(b.min_x, b.min_y)).with(P::new(b.max_x, b.max_y)),
                None => b,
            });
        }
    }
    acc
}

fn centre(r: &Rect) -> P {
    P::new((r.min_x + r.max_x) / 2.0, (r.min_y + r.max_y) / 2.0)
}

fn drawing_json(doc: &Document, id: &Id) -> Result<Value, CommandError> {
    Ok(doc.entry_raw_json(&EntryKey::Drawing(id.clone()))?)
}

/// What a transform-like command did, for its result.
struct Applied {
    ids: Vec<String>,
    changed: Changed,
    rev: u64,
}

/// Maps objects of a drawing (or copies of them) and stages the change.
fn map_objects(
    cx: &mut Cx,
    drawing: &Id,
    objs: &[Located],
    m: &Map,
    copy: bool,
    changes: &mut Vec<(EntryKey, Vec<tt_model::change::PatchOp>)>,
) -> Result<Applied, CommandError> {
    let before = cx.document()?.clone();
    let dj = drawing_json(&before, drawing)?;
    let mut ops = Vec::new();
    let mut ids = Vec::new();
    let mut created_ids = Vec::new();
    // Copies go right after their originals: insert from the end of each list so earlier indices stay valid.
    let mut order: Vec<&Located> = objs.iter().collect();
    order.sort_by(|a, b| b.list.cmp(&a.list).then(b.index.cmp(&a.index)));
    for o in order {
        let Some(mut v) = dj.pointer(&o.pointer).cloned() else { continue };
        match o.kind {
            ObjectKind::Stroke => tf::map_stroke(&mut v, m),
            _ => tf::map_node(&mut v, m),
        }
        if copy {
            let mut made = Vec::new();
            super::edit_ops::regenerate(cx, &mut v, &[ObjectType::DrawingNode, ObjectType::Stroke], &mut made)?;
            if let Some((_, id)) = made.first() {
                ids.push(id.clone());
            }
            created_ids.extend(made.iter().map(|(_, i)| i.clone()));
            ops.push(op("add", &format!("{}/{}", o.list, o.index + 1), Some(v)));
        } else {
            ids.push(o.id.clone());
            ops.push(op("replace", &o.pointer, Some(v)));
        }
    }
    ids.reverse();
    // Layers moved as a whole keep each zone's fill with its matrix mapped (C23-R1-01: one repaint path for every
    // transform route).
    let whole = if copy { Vec::new() } else { whole_layers(&before, drawing, objs) };
    let sources = mapped_sources(cx, &before, drawing, &whole)?;
    changes.push((EntryKey::Drawing(drawing.clone()), ops));
    cx.patch_entries(std::mem::take(changes))?;
    repaint_mapped(cx, &[(drawing.clone(), map_affine(m), sources)])?;
    let mut changed = Changed { created_ids: created_ids.clone(), modified_ids: Vec::new(), removed_ids: Vec::new() };
    if !copy {
        changed.modified_ids = ids.clone();
    }
    let added: Vec<String> = created_ids.clone();
    let settled = crate::planar::settled(cx, &before, drawing, &added)?;
    settled.report(&mut changed);
    // IDs after noding: an object noding split is reported by its pieces.
    let ids: Vec<String> = ids.iter().flat_map(|i| cx.pieces_of(drawing, i)).collect();
    Ok(Applied { ids, changed, rev: committed_rev(cx)? })
}

/// The affine part of a map: the map itself, or an envelope's three-corner fit (SI-070).
fn map_affine(m: &Map) -> Affine {
    match m {
        Map::Affine(a) | Map::Envelope(_, a) => *a,
    }
}

/// The zones (with fills) of the layers a transform moves as a whole, before the change.
fn mapped_sources(
    cx: &Cx,
    doc: &tt_model::document::Document,
    drawing: &Id,
    layers: &[String],
) -> Result<BTreeMap<String, crate::planar::LayerZones>, CommandError> {
    let Some(d) = doc.drawing(drawing).filter(|_| !layers.is_empty()) else { return Ok(BTreeMap::new()) };
    Ok(crate::planar::drawing_zones(&cx.planar_cache(), d)?
        .into_iter()
        .filter(|lz| layers.contains(&lz.layer.id))
        .map(|lz| (lz.layer.id.clone(), lz))
        .collect())
}

/// After staging a transform: settles, then gives every zone of each moved layer the fill of the source zone at its
/// interior point mapped back through the map's affine part, with the fill matrix mapped forward (C23-R1-01). The
/// settle pass alone carries fills by overlap with the old geometry, which a disjoint move does not have. A zone
/// whose mapped-back point lies in no source zone keeps what the settle pass gave it.
fn repaint_mapped(
    cx: &mut Cx,
    moves: &[(Id, Affine, BTreeMap<String, crate::planar::LayerZones>)],
) -> Result<(), CommandError> {
    if moves.iter().all(|m| m.2.is_empty()) {
        return Ok(());
    }
    cx.settle()?;
    let now = cx.document()?.clone();
    let cache = cx.planar_cache();
    for (drawing, a, sources) in moves {
        let (Some(inv), Some(dn)) = (inverse(a), now.drawing(drawing)) else { continue };
        for lz in crate::planar::drawing_zones(&cache, dn)? {
            let Some(src) = sources.get(&lz.layer.id) else { continue };
            let mut fills: BTreeMap<String, Option<tt_vector::planar::fill::Fill>> = BTreeMap::new();
            for z in &lz.map.zones {
                let Some(k) = src.map.zone_at(inv.apply(z.interior)) else { continue };
                let f = src.fills.get(k).cloned().flatten().map(|f| tt_vector::planar::fill::Fill {
                    colour_id: f.colour_id,
                    matrix: f.matrix.map(|mm| a.then_after(&Affine(mm)).0.map(|v| v + 0.0)),
                });
                fills.insert(z.id.clone(), f);
            }
            cx.paint_zones(drawing, &lz.layer.id, fills);
        }
    }
    Ok(())
}

fn inverse(m: &Affine) -> Option<Affine> {
    let [a, b, c, d, e, f] = m.0;
    let det = a * d - b * c;
    if det == 0.0 || !det.is_finite() {
        return None;
    }
    Some(Affine([d / det, -b / det, -c / det, a / det, (c * f - d * e) / det, (b * e - a * f) / det]))
}

/// Layers every stroke of which the objects cover (listed layers, layers inside listed groups, layers all of whose
/// strokes are listed).
fn whole_layers(doc: &tt_model::document::Document, drawing: &Id, objs: &[Located]) -> Vec<String> {
    let Some(d) = doc.drawing(drawing) else { return Vec::new() };
    let layers = crate::planar::layer_nodes(d);
    let mut out: Vec<String> = Vec::new();
    let mut strokes: BTreeMap<String, usize> = BTreeMap::new();
    for o in objs {
        match o.kind {
            ObjectKind::Stroke => *strokes.entry(o.parent.clone().unwrap_or_default()).or_default() += 1,
            _ => {
                let prefix = format!("{}/", o.pointer);
                for (p, n) in &layers {
                    if *p == o.pointer || p.starts_with(&prefix) {
                        out.push(n.id().as_str().to_owned());
                    }
                }
            }
        }
    }
    for (_, n) in &layers {
        let total = n.strokes().iter().flatten().count();
        if total > 0 && strokes.get(n.id().as_str()) == Some(&total) {
            out.push(n.id().as_str().to_owned());
        }
    }
    out.sort();
    out.dedup();
    out
}

/// `{ids, removedIds, rev}` and the changed IDs.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TransformResult {
    /// The drawing.
    drawing_id: String,
    /// The transformed objects (the copies with `copy`; pieces where noding split them).
    ids: Vec<String>,
    /// Changed objects.
    #[serde(flatten)]
    changed: Changed,
    /// The drawing revision after the change.
    rev: u64,
}

/// `{x, y}` factors.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Xy {
    /// x.
    x: f64,
    /// y.
    y: f64,
}

/// `draw.transform.apply` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct TransformParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Objects (default: the selection).
    #[serde(default)]
    ids: Option<Vec<String>>,
    /// Anchors to move instead of whole objects (default: the Contour Editor's point selection when no object is
    /// selected; SI-070).
    #[serde(default)]
    points: Option<Vec<AnchorRef>>,
    /// Translation (du).
    #[serde(default)]
    translate: Option<Point>,
    /// Scale factors.
    #[serde(default)]
    scale: Option<Xy>,
    /// Rotation (degrees, counter-clockwise).
    #[serde(default)]
    rotate: Option<f64>,
    /// Skew (degrees).
    #[serde(default)]
    skew: Option<Xy>,
    /// Pivot (default: the centre of the selection's bounds).
    #[serde(default)]
    pivot: Option<Point>,
    /// A full matrix `[a, b, c, d, e, f]` (overrides the others).
    #[serde(default)]
    matrix: Option<[f64; 6]>,
    /// Transform copies, leaving the originals.
    #[serde(default)]
    copy: bool,
    /// The drawing revision the caller read.
    #[serde(default)]
    expect_rev: Option<u64>,
}

fn affine_of(p: &TransformParams, pivot: P) -> Result<Affine, CommandError> {
    if let Some(m) = p.matrix {
        if m.iter().any(|v| !v.is_finite()) || Affine(m).det() == 0.0 {
            return Err(err(
                "arg.invalid",
                "the matrix must be finite and invertible",
                json!({ "pointer": "/matrix" }),
            ));
        }
        return Ok(Affine(m));
    }
    let mut lin = Affine::IDENTITY;
    if let Some(s) = p.scale {
        if s.x == 0.0 || s.y == 0.0 || !s.x.is_finite() || !s.y.is_finite() {
            return Err(err(
                "arg.invalid",
                "scale factors must be finite and non-zero",
                json!({ "pointer": "/scale" }),
            ));
        }
        lin = tf::scale(s.x, s.y).then_after(&lin);
    }
    if let Some(k) = p.skew {
        if k.x.abs() >= 89.0 || k.y.abs() >= 89.0 {
            return Err(err("arg.invalid", "skew angles must be within ±89°", json!({ "pointer": "/skew" })));
        }
        lin = tf::skew(k.x, k.y).then_after(&lin);
    }
    if let Some(r) = p.rotate {
        lin = tf::rotation(r).then_after(&lin);
    }
    let mut m = tf::about(&lin, pivot);
    if let Some(t) = p.translate {
        m = Affine::translate(t.x, t.y).then_after(&m);
    }
    Ok(m)
}

#[command(
    id = "draw.transform.apply",
    precheck = super::draw::precheck_target,
    kind = action,
    label = "Transform",
    scopes = [edit],
    undo = "Move/Scale/Rotate Object",
    needs_project,
    example(title = "Move the pencil line 10 du right", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "ids": ["st_01K742SG0R61SYK9GG0HGYVNBT"], "translate": {"x": 10, "y": 0}}"#),
    example(title = "Rotate a copy of the caption by 15°", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "ids": ["nd_01K742SG0WFAQJFKAZKV4CD56C"], "rotate": 15, "copy": true}"#),
    gui = "Drawing view: Select tool handles and nudges (M4 C4.2)",
)]
pub(crate) fn draw_transform_apply(cx: &mut Cx, p: TransformParams) -> Result<TransformResult, CommandError> {
    let mut changes = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut changes)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    check_editable(&d, p.expect_rev)?;
    let points = match (&p.points, &p.ids) {
        (Some(pts), _) => Some(pts.clone()),
        (None, None) if super::draw::selection_ids(cx, &d).is_empty() => {
            let sel = super::draw_select::point_refs(cx, &d);
            (!sel.is_empty()).then_some(sel)
        }
        _ => None,
    };
    if p.copy && points.is_some() {
        return Err(err("arg.invalid", "`copy` applies to objects, not to anchors", json!({ "pointer": "/copy" })));
    }
    if let Some(pts) = points {
        return move_anchors(cx, &t.id, &d, &pts, |pivot| affine_of(&p, pivot), p.pivot);
    }
    let objs = objects_of(cx, &d, p.ids.as_ref(), "transform")?;
    let pivot =
        p.pivot.map(|q| P::new(q.x, q.y)).or_else(|| bounds_of(&d, &objs).map(|b| centre(&b))).unwrap_or_default();
    let m = affine_of(&p, pivot)?;
    if p.copy {
        cx.set_label("Copy Object");
    }
    let a = map_objects(cx, &t.id, &objs, &Map::Affine(m), p.copy, &mut changes)?;
    Ok(TransformResult { drawing_id: t.id.as_str().to_owned(), ids: a.ids, changed: a.changed, rev: a.rev })
}

/// Moves anchors with an affine map about their own bounds' centre (or `pivot`); brush outlines edited this way
/// become authoritative (spine dropped).
fn move_anchors(
    cx: &mut Cx,
    drawing: &Id,
    d: &Drawing,
    pts: &[AnchorRef],
    make: impl Fn(P) -> Result<Affine, CommandError>,
    pivot: Option<Point>,
) -> Result<TransformResult, CommandError> {
    let before = cx.document()?.clone();
    let dj = drawing_json(&before, drawing)?;
    let mut by_stroke: std::collections::BTreeMap<String, Vec<usize>> = std::collections::BTreeMap::new();
    let mut acc: Option<Rect> = None;
    for (i, a) in pts.iter().enumerate() {
        let s = vd::locate(d, &a.stroke_id).and_then(|l| vd::stroke_at(d, &l.pointer));
        let Some(pt) = s.and_then(|s| s.points().get(a.anchor as usize)) else {
            return Err(err(
                "drawing.objectNotFound",
                format!("stroke `{}` has no anchor {}", a.stroke_id, a.anchor),
                json!({ "pointer": format!("/points/{i}") }),
            ));
        };
        let q = P::new(pt[0], pt[1]);
        acc = Some(acc.map_or(Rect::of(q), |r| r.with(q)));
        by_stroke.entry(a.stroke_id.clone()).or_default().push(a.anchor as usize);
    }
    let pivot = pivot.map(|q| P::new(q.x, q.y)).or_else(|| acc.map(|b| centre(&b))).unwrap_or_default();
    // Every anchor of the same layer at a requested anchor's exact position moves with it (the junction class,
    // as `draw.point.move`; C23-R1-02); the pivot stays that of the requested subset.
    let mut at: Vec<(String, (u64, u64))> = Vec::new();
    for a in pts {
        if let Some(l) = vd::locate(d, &a.stroke_id)
            && let Some(q) = vd::stroke_at(d, &l.pointer).and_then(|s| s.points().get(a.anchor as usize))
        {
            at.push((l.list.clone(), (q[0].to_bits(), q[1].to_bits())));
        }
    }
    for o in vd::objects(d).iter().filter(|o| o.kind == ObjectKind::Stroke) {
        let Some(s) = vd::stroke_at(d, &o.pointer) else { continue };
        for (k, q) in s.points().iter().enumerate() {
            if at.iter().any(|(list, xy)| *list == o.list && *xy == (q[0].to_bits(), q[1].to_bits())) {
                let e = by_stroke.entry(o.id.clone()).or_default();
                if !e.contains(&k) {
                    e.push(k);
                }
            }
        }
    }
    let m = Map::Affine(make(pivot)?);
    let mut ops = Vec::new();
    let mut modified = Vec::new();
    for (sid, anchors) in &by_stroke {
        let Some(l) = vd::locate(d, sid) else { continue };
        let Some(mut v) = dj.pointer(&l.pointer).cloned() else { continue };
        if let Some(pl) = v.get_mut("points") {
            tf::map_points(pl, &m, Some(anchors));
        }
        drop_spine_if_brush(&mut v);
        ops.push(op("replace", &l.pointer, Some(v)));
        modified.push(sid.clone());
    }
    cx.patch_entries(vec![(EntryKey::Drawing(drawing.clone()), ops)])?;
    cx.set_label("Contour Editing");
    let mut changed = Changed { modified_ids: modified.clone(), ..Changed::default() };
    crate::planar::settled(cx, &before, drawing, &[])?.report(&mut changed);
    let ids = modified.iter().flat_map(|i| cx.pieces_of(drawing, i)).collect();
    Ok(TransformResult { drawing_id: drawing.as_str().to_owned(), ids, changed, rev: committed_rev(cx)? })
}

/// A brush stroke whose outline a command edits directly keeps the outline and loses its spine (02 §3.2, SI-054).
fn drop_spine_if_brush(v: &mut Value) {
    if v.get("kind").and_then(Value::as_str) == Some("brush")
        && let Some(o) = v.as_object_mut()
    {
        o.insert("spine".into(), Value::Null);
    }
}

// ----- flip / rotate --------------------------------------------------------------------------------------------------

/// Flip axis.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Axis {
    /// Left ↔ right.
    Horizontal,
    /// Top ↔ bottom.
    Vertical,
}

/// `draw.flip.apply` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct FlipParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Objects (default: the selection).
    #[serde(default)]
    ids: Option<Vec<String>>,
    /// Axis.
    axis: Axis,
}

#[command(
    id = "draw.flip.apply",
    kind = action,
    label = "Flip",
    scopes = [edit],
    undo = "Flip Horizontal",
    needs_project,
    example(title = "Flip the brush stroke horizontally", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "ids": ["st_01K742SG0QD1F2HFZSX8R4V7WP"], "axis": "horizontal"}"#),
    gui = "Drawing view: Flip Horizontal / Flip Vertical (M4 C4.2)",
)]
pub(crate) fn draw_flip_apply(cx: &mut Cx, p: FlipParams) -> Result<TransformResult, CommandError> {
    let mut changes = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut changes)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    check_editable(&d, None)?;
    let objs = objects_of(cx, &d, p.ids.as_ref(), "flip")?;
    let pivot = bounds_of(&d, &objs).map(|b| centre(&b)).unwrap_or_default();
    let (s, label) = match p.axis {
        Axis::Horizontal => (tf::scale(-1.0, 1.0), "Flip Horizontal"),
        Axis::Vertical => (tf::scale(1.0, -1.0), "Flip Vertical"),
    };
    cx.set_label(label);
    let a = map_objects(cx, &t.id, &objs, &Map::Affine(tf::about(&s, pivot)), false, &mut changes)?;
    Ok(TransformResult { drawing_id: t.id.as_str().to_owned(), ids: a.ids, changed: a.changed, rev: a.rev })
}

/// A fixed rotation step.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
pub(crate) enum Step {
    /// 90° clockwise.
    #[serde(rename = "90cw")]
    Cw90,
    /// 90° counter-clockwise.
    #[serde(rename = "90ccw")]
    Ccw90,
    /// 180°.
    #[serde(rename = "180")]
    Half,
}

/// `draw.rotate.apply` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RotateParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Objects (default: the selection).
    #[serde(default)]
    ids: Option<Vec<String>>,
    /// Step.
    by: Step,
}

#[command(
    id = "draw.rotate.apply",
    kind = action,
    label = "Rotate",
    scopes = [edit],
    undo = "Rotate 90° CW",
    needs_project,
    example(title = "Rotate the caption 90° clockwise", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "ids": ["nd_01K742SG0WFAQJFKAZKV4CD56C"], "by": "90cw"}"#),
    gui = "Drawing view: Rotate 90° CW / 90° CCW / 180° (M4 C4.2)",
)]
pub(crate) fn draw_rotate_apply(cx: &mut Cx, p: RotateParams) -> Result<TransformResult, CommandError> {
    let mut changes = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut changes)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    check_editable(&d, None)?;
    let objs = objects_of(cx, &d, p.ids.as_ref(), "rotate")?;
    let pivot = bounds_of(&d, &objs).map(|b| centre(&b)).unwrap_or_default();
    let (deg, label) = match p.by {
        Step::Cw90 => (-90.0, "Rotate 90° CW"),
        Step::Ccw90 => (90.0, "Rotate 90° CCW"),
        Step::Half => (180.0, "Rotate 180°"),
    };
    cx.set_label(label);
    let a = map_objects(cx, &t.id, &objs, &Map::Affine(tf::about(&tf::rotation(deg), pivot)), false, &mut changes)?;
    Ok(TransformResult { drawing_id: t.id.as_str().to_owned(), ids: a.ids, changed: a.changed, rev: a.rev })
}

// ----- envelope / reposition all -----------------------------------------------------------------------------------------

/// Envelope mapping.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Mapping {
    /// Projective when the target quadrilateral is convex, else bilinear.
    #[default]
    Auto,
    /// Projective (perspective).
    Projective,
    /// Bilinear.
    Bilinear,
}

/// `draw.envelope.apply` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct EnvelopeParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Objects (default: the selection).
    #[serde(default)]
    ids: Option<Vec<String>>,
    /// Targets of the bounds' corners: bottom-left, bottom-right, top-right, top-left.
    #[schemars(length(min = 4, max = 4))]
    corners: Vec<Point>,
    /// Mapping.
    #[serde(default)]
    mapping: Mapping,
}

fn envelope_of(b: &Rect, corners: &[Point], mapping: Mapping) -> Result<Map, CommandError> {
    let c: Vec<P> = corners.iter().map(|q| P::new(q.x, q.y)).collect();
    let (min, max) = (P::new(b.min_x, b.min_y), P::new(b.max_x, b.max_y));
    let proj = match mapping {
        Mapping::Auto => None,
        Mapping::Projective => Some(true),
        Mapping::Bilinear => Some(false),
    };
    let quad = [c[0], c[1], c[2], c[3]];
    // The affine fit through three corners, from the targets themselves (exact for exact inputs).
    let (w, h) = ((max.x - min.x).max(1e-12), (max.y - min.y).max(1e-12));
    let (ex, ey) = ((quad[1] - quad[0]) * (1.0 / w), (quad[3] - quad[0]) * (1.0 / h));
    let lin = Affine([ex.x, ex.y, ey.x, ey.y, 0.0, 0.0]);
    let t = quad[0] - lin.apply(min);
    let fit = Affine([ex.x, ex.y, ey.x, ey.y, t.x, t.y]);
    // A parallelogram target is an affine image of the box: map it exactly (SI-070).
    if quad[2] == quad[1] + quad[3] - quad[0] && mapping == Mapping::Auto {
        return Ok(Map::Affine(fit));
    }
    let e = Envelope::new(min, max, quad, proj).ok_or_else(|| {
        err(
            "drawing.invalidGeometry",
            "the corners do not define an envelope of the selection",
            json!({ "pointer": "/corners" }),
        )
    })?;
    Ok(Map::Envelope(e, fit))
}

#[command(
    id = "draw.envelope.apply",
    kind = action,
    label = "Perspective",
    scopes = [edit],
    undo = "Perspective Tool",
    needs_project,
    example(title = "Taper the pencil line's bounds", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "ids": ["st_01K742SG0R61SYK9GG0HGYVNBT"], "corners": [{"x": -50, "y": -50}, {"x": 50, "y": -50}, {"x": 30, "y": 50}, {"x": -30, "y": 50}]}"#),
    gui = "Drawing view: Perspective tool (M4 C4.2)",
)]
pub(crate) fn draw_envelope_apply(cx: &mut Cx, p: EnvelopeParams) -> Result<TransformResult, CommandError> {
    if p.corners.len() != 4 {
        return Err(err("arg.invalid", "four corners are required", json!({ "pointer": "/corners" })));
    }
    let mut changes = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut changes)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    check_editable(&d, None)?;
    let objs = objects_of(cx, &d, p.ids.as_ref(), "distort")?;
    let b = bounds_of(&d, &objs).ok_or_else(|| nothing_to_do("distort"))?;
    let m = envelope_of(&b, &p.corners, p.mapping)?;
    let a = map_objects(cx, &t.id, &objs, &m, false, &mut changes)?;
    Ok(TransformResult { drawing_id: t.id.as_str().to_owned(), ids: a.ids, changed: a.changed, rev: a.rev })
}

/// `draw.repositionAll.apply` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RepositionParams {
    /// The element (default: current).
    #[serde(default)]
    element_id: Option<String>,
    /// Translation.
    #[serde(default)]
    translate: Option<Point>,
    /// Scale.
    #[serde(default)]
    scale: Option<Xy>,
    /// Rotation (degrees CCW).
    #[serde(default)]
    rotate: Option<f64>,
    /// Pivot.
    #[serde(default)]
    pivot: Option<Point>,
    /// Matrix (overrides).
    #[serde(default)]
    matrix: Option<[f64; 6]>,
    /// Envelope corners (of each drawing's bounds).
    #[serde(default)]
    corners: Option<Vec<Point>>,
}

/// `{drawingIds}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RepositionResult {
    /// The drawings changed.
    drawing_ids: Vec<String>,
}

#[command(
    id = "draw.repositionAll.apply",
    precheck = precheck_reposition_all,
    kind = action,
    label = "Reposition all drawings",
    scopes = [edit],
    undo = "Reposition All Drawings",
    needs_project,
    example(title = "Move every drawing of Bob up 20 du", params = r#"{"elementId": "scene:#1/element:\"Bob\"", "translate": {"x": 0, "y": 20}}"#),
    gui = "Drawing view: Reposition All Drawings tool (M4 C4.2)",
)]
pub(crate) fn draw_reposition_all_apply(cx: &mut Cx, p: RepositionParams) -> Result<RepositionResult, CommandError> {
    let element = match p.element_id.as_deref() {
        Some(r) => Id::parse(&cx.resolve_as(r, ObjectType::Element)?)?,
        None => cx.app().element.clone().ok_or_else(|| {
            err("drawing.noTarget", "there is no current element", json!({ "pointer": "/elementId" }))
        })?,
    };
    let doc = cx.document()?.clone();
    let bank = doc.element(&element).and_then(|e| e.drawing_bank_id().clone()).ok_or_else(|| {
        err("drawing.notVectorElement", "the element has no drawings", json!({ "pointer": "/elementId" }))
    })?;
    let ids: Vec<Id> = doc.bank(&bank).map(|b| b.drawings().ids().to_vec()).unwrap_or_default();
    let tp = TransformParams {
        drawing_id: None,
        ids: None,
        points: None,
        translate: p.translate,
        scale: p.scale,
        rotate: p.rotate,
        skew: None,
        pivot: p.pivot,
        matrix: p.matrix,
        copy: false,
        expect_rev: None,
    };
    let mut changes = Vec::new();
    let mut changed_ids = Vec::new();
    let mut moves = Vec::new();
    for id in &ids {
        let Some(d) = doc.drawing(id) else { continue };
        if *d.locked() {
            continue;
        }
        let m = match &p.corners {
            Some(c) => {
                if c.len() != 4 {
                    return Err(err("arg.invalid", "four corners are required", json!({ "pointer": "/corners" })));
                }
                let Some(b) = vd::drawing_bounds(d, true) else { continue };
                envelope_of(&b, c, Mapping::Auto)?
            }
            None => Map::Affine(affine_of(&tp, p.pivot.map(|q| P::new(q.x, q.y)).unwrap_or_default())?),
        };
        let dj = drawing_json(&doc, id)?;
        let mut ops = Vec::new();
        for (k, n) in dj.get("nodes").and_then(Value::as_array).into_iter().flatten().enumerate() {
            let mut v = n.clone();
            tf::map_node(&mut v, &m);
            ops.push(op("replace", &format!("/nodes/{k}"), Some(v)));
        }
        if !ops.is_empty() {
            let layers: Vec<String> =
                crate::planar::layer_nodes(d).iter().map(|(_, n)| n.id().as_str().to_owned()).collect();
            moves.push((id.clone(), map_affine(&m), mapped_sources(cx, &doc, id, &layers)?));
            changes.push((EntryKey::Drawing(id.clone()), ops));
            changed_ids.push(id.as_str().to_owned());
        }
    }
    if changes.is_empty() {
        return Err(nothing_to_do("reposition"));
    }
    cx.patch_entries(changes)?;
    // Every drawing moves whole: its zones keep their fills with mapped matrices (C23-R1-01).
    repaint_mapped(cx, &moves)?;
    Ok(RepositionResult { drawing_ids: changed_ids })
}

// ----- contour editing ---------------------------------------------------------------------------------------------------

/// The located stroke and its JSON.
fn stroke_json(cx: &Cx, d: &Drawing, id: &str, pointer: &str) -> Result<(Located, Value), CommandError> {
    let l = vd::locate(d, id).filter(|l| l.kind == ObjectKind::Stroke).ok_or_else(|| {
        err("drawing.objectNotFound", format!("`{id}` is not a stroke"), json!({ "pointer": pointer }))
    })?;
    let dj = drawing_json(cx.document()?, d.id())?;
    let v = dj.pointer(&l.pointer).cloned().unwrap_or(Value::Null);
    Ok((l, v))
}

fn path_of(v: &Value) -> Path {
    let pts: Vec<[f64; 6]> = v
        .get("points")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|a| serde_json::from_value::<[f64; 6]>(a.clone()).ok())
        .collect();
    Path::from_points(&pts, v.get("closed").and_then(Value::as_bool).unwrap_or(false))
}

fn set_path(v: &mut Value, p: &Path) {
    if let Some(o) = v.as_object_mut() {
        o.insert("points".into(), p.points_json());
    }
    drop_spine_if_brush(v);
}

/// Resolves a stroke for a contour edit: drawing, locked check, stroke JSON.
fn contour_target(cx: &mut Cx, stroke: &str) -> Result<(Id, Drawing, Located, Value), CommandError> {
    let mut none = Vec::new();
    let (drawing_ref, local) = match stroke.split_once('/') {
        Some((d, s)) if d.starts_with("dr_") => (Some(d.to_owned()), s.to_owned()),
        _ => (None, stroke.to_owned()),
    };
    let t = target(cx, drawing_ref.as_deref(), None, None, false, &mut none)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    check_editable(&d, None)?;
    let (l, v) = stroke_json(cx, &d, &local, "/strokeId")?;
    Ok((t.id, d, l, v))
}

fn commit_stroke(cx: &mut Cx, drawing: &Id, l: &Located, v: Value) -> Result<(Changed, u64), CommandError> {
    let before = cx.document()?.clone();
    cx.patch_entries(vec![(EntryKey::Drawing(drawing.clone()), vec![op("replace", &l.pointer, Some(v))])])?;
    let mut changed = Changed { modified_ids: vec![l.id.clone()], ..Changed::default() };
    crate::planar::settled(cx, &before, drawing, &[])?.report(&mut changed);
    Ok((changed, committed_rev(cx)?))
}

/// `draw.point.add` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PointAddParams {
    /// The stroke (local or `<drawingId>/<strokeId>`).
    stroke_id: String,
    /// Insert at the stroke location nearest to this point.
    #[serde(default)]
    at: Option<Point>,
    /// Or: the segment index …
    #[serde(default)]
    segment: Option<u32>,
    /// … and the parameter on it (0–1).
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 1.0))]
    t: Option<f64>,
}

/// `{strokeId, anchor, rev}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PointAddResult {
    /// The stroke.
    stroke_id: String,
    /// The new anchor's index.
    anchor: u32,
    /// Changed objects.
    #[serde(flatten)]
    changed: Changed,
    /// The drawing revision after the change.
    rev: u64,
}

/// The segment and parameter of the location on a path nearest to `q` (sampled, then refined).
fn nearest_location(path: &Path, q: P) -> Option<(usize, f64)> {
    let segs = path.segments();
    let mut best: Option<(f64, usize, f64)> = None;
    for (i, c) in segs.iter().enumerate() {
        for k in 0..=64 {
            let t = f64::from(k) / 64.0;
            let d = c.eval(t).dist(q);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, i, t));
            }
        }
    }
    let (_, i, mut t) = best?;
    let c = segs[i];
    let mut step = 1.0 / 64.0;
    for _ in 0..30 {
        step /= 2.0;
        let (a, b) = ((t - step).max(0.0), (t + step).min(1.0));
        t = [a, t, b].into_iter().min_by(|x, y| c.eval(*x).dist(q).total_cmp(&c.eval(*y).dist(q))).unwrap_or(t);
    }
    Some((i, t))
}

#[command(
    id = "draw.point.add",
    kind = action,
    label = "Add point",
    scopes = [edit],
    undo = "Contour Editing",
    needs_project,
    example(title = "Add an anchor at the middle of the pencil line", params = r#"{"strokeId": "dr_01K742SG0KKMCAP9X1E6Z5P4SW/st_01K742SG0R61SYK9GG0HGYVNBT", "segment": 0, "t": 0.5}"#),
    gui = "Drawing view: Contour Editor, click on a segment (M4 C4.2)",
)]
pub(crate) fn draw_point_add(cx: &mut Cx, p: PointAddParams) -> Result<PointAddResult, CommandError> {
    let (did, _d, l, mut v) = contour_target(cx, &p.stroke_id)?;
    let path = path_of(&v);
    let segs = path.segments();
    let (seg, t) = match (p.at, p.segment, p.t) {
        (Some(q), _, _) => nearest_location(&path, P::new(q.x, q.y)).ok_or_else(|| {
            err("drawing.invalidGeometry", "the stroke has no segment", json!({ "pointer": "/strokeId" }))
        })?,
        (None, Some(s), t) => (s as usize, t.unwrap_or(0.5)),
        (None, None, _) => {
            return Err(err("arg.invalid", "give `at`, or `segment` and `t`", json!({ "pointer": "/at" })));
        }
    };
    if seg >= segs.len() {
        return Err(err(
            "arg.invalid",
            format!("the stroke has {} segments", segs.len()),
            json!({ "pointer": "/segment" }),
        ));
    }
    if t <= 0.0 || t >= 1.0 {
        return Err(err("arg.invalid", "the location is an existing anchor", json!({ "pointer": "/t" })));
    }
    let (left, right) = segs[seg].split(t);
    let mut anchors = path.anchors.clone();
    let n = anchors.len();
    anchors[seg].outh = left.p1 - left.p0;
    anchors[(seg + 1) % n].inh = right.p2 - right.p3;
    let new = tt_vector::path::Anchor { p: left.p3, inh: left.p2 - left.p3, outh: right.p1 - right.p0 };
    anchors.insert(seg + 1, new);
    set_path(&mut v, &Path { anchors, closed: path.closed });
    let (changed, rev) = commit_stroke(cx, &did, &l, v)?;
    Ok(PointAddResult { stroke_id: l.id, anchor: (seg + 1) as u32, changed, rev })
}

/// `draw.point.move` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PointMoveParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Anchors (default: the point selection).
    #[serde(default)]
    points: Option<Vec<AnchorRef>>,
    /// Offset (du).
    delta: Point,
    /// Snap moved anchors to other strokes' anchors within 8 du (Snap to Contour, 02 §8.9; default off).
    #[serde(default)]
    snap: bool,
}

/// `{modifiedIds, removedIds, rev}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContourResult {
    /// The drawing.
    drawing_id: String,
    /// Changed objects.
    #[serde(flatten)]
    changed: Changed,
    /// The drawing revision after the change.
    rev: u64,
}

/// The Snap to Contour radius (02 §8.9: 8 view pixels; 100 % zoom headless, SI-063).
const SNAP_RADIUS: f64 = 8.0;

/// The point of a path nearest to `q`: `(distance, segment, parameter, point)`. Straight segments project exactly;
/// curved ones are searched by sampling and bisection.
fn nearest_on(path: &tt_vector::path::Path, q: P) -> Option<(f64, usize, f64, P)> {
    let mut best: Option<(f64, usize, f64, P)> = None;
    let mut keep = |cand: (f64, usize, f64, P)| {
        if best.as_ref().is_none_or(|b| cand.0 < b.0) {
            best = Some(cand);
        }
    };
    let segs = path.segments();
    if segs.is_empty() {
        let a = path.anchors.first()?.p;
        return Some((a.dist(q), 0, 0.0, a));
    }
    for (i, c) in segs.iter().enumerate() {
        if c.p1 == c.p0 && c.p2 == c.p3 {
            let ab = c.p3 - c.p0;
            let l2 = ab.dot(ab);
            let t = if l2 > 0.0 { ((q - c.p0).dot(ab) / l2).clamp(0.0, 1.0) } else { 0.0 };
            let at = if t == 0.0 {
                c.p0
            } else if t == 1.0 {
                c.p3
            } else {
                c.p0 + ab * t
            };
            keep((at.dist(q), i, t, at));
        } else {
            let mut t = (0..=64)
                .map(|k| f64::from(k) / 64.0)
                .min_by(|a, b| c.eval(*a).dist(q).total_cmp(&c.eval(*b).dist(q)))?;
            let mut step = 1.0 / 64.0;
            for _ in 0..40 {
                step /= 2.0;
                let (lo, hi) = ((t - step).max(0.0), (t + step).min(1.0));
                t = [lo, t, hi]
                    .into_iter()
                    .min_by(|a, b| c.eval(*a).dist(q).total_cmp(&c.eval(*b).dist(q)))
                    .unwrap_or(t);
            }
            let at = c.eval(t);
            keep((at.dist(q), i, t, at));
        }
    }
    best
}

#[command(
    id = "draw.point.move",
    kind = action,
    label = "Move points",
    scopes = [edit],
    undo = "Contour Editing",
    needs_project,
    example(title = "Move the pencil line's first anchor", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "points": [{"strokeId": "st_01K742SG0R61SYK9GG0HGYVNBT", "anchor": 0}], "delta": {"x": -5, "y": 0}}"#),
    gui = "Drawing view: Contour Editor drag (M4 C4.2)",
)]
pub(crate) fn draw_point_move(cx: &mut Cx, p: PointMoveParams) -> Result<ContourResult, CommandError> {
    let mut none = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut none)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    check_editable(&d, None)?;
    let pts = match p.points {
        Some(v) => v,
        None => super::draw_select::point_refs(cx, &d),
    };
    if pts.is_empty() {
        return Err(nothing_to_do("move"));
    }
    // Shared junction anchors move together: every anchor at a moved anchor's exact coordinate in the same layer.
    let mut moved: BTreeSet<(String, usize)> = BTreeSet::new();
    let mut coords: Vec<(String, P)> = Vec::new();
    for (i, a) in pts.iter().enumerate() {
        let l = vd::locate(&d, &a.stroke_id);
        let pt = l.as_ref().and_then(|l| vd::stroke_at(&d, &l.pointer)).and_then(|s| s.points().get(a.anchor as usize));
        let (Some(l), Some(pt)) = (l, pt) else {
            return Err(err(
                "drawing.objectNotFound",
                format!("stroke `{}` has no anchor {}", a.stroke_id, a.anchor),
                json!({ "pointer": format!("/points/{i}") }),
            ));
        };
        moved.insert((a.stroke_id.clone(), a.anchor as usize));
        coords.push((l.list.clone(), P::new(pt[0], pt[1])));
    }
    for o in vd::objects(&d) {
        let Some(s) = vd::stroke_at(&d, &o.pointer) else { continue };
        for (k, pt) in s.points().iter().enumerate() {
            if coords.iter().any(|(list, c)| *list == o.list && c.x == pt[0] && c.y == pt[1]) {
                moved.insert((o.id.clone(), k));
            }
        }
    }
    let delta = P::new(p.delta.x, p.delta.y);
    let dj = drawing_json(&doc, &t.id)?;
    // Snap to Contour (02 §8.9; C23-R1-03): the other strokes of the moved stroke's own layer, by their whole
    // contour (anchors and segment interiors, curves included); cross-layer contours are not candidates (SI-070).
    let contours: Vec<(String, String, tt_vector::path::Path)> = if p.snap {
        vd::objects(&d)
            .iter()
            .filter(|o| o.kind == ObjectKind::Stroke && !moved.iter().any(|(s, _)| *s == o.id))
            .filter_map(|o| {
                let s = vd::stroke_at(&d, &o.pointer)?;
                let closed = *s.closed() || *s.kind() == tt_model::entities::drawing::StrokeKind::Brush;
                Some((o.list.clone(), o.id.clone(), tt_vector::path::Path::from_points(s.points(), closed)))
            })
            .collect()
    } else {
        Vec::new()
    };
    let mut ops = Vec::new();
    let mut modified = Vec::new();
    let strokes: BTreeSet<String> = moved.iter().map(|(s, _)| s.clone()).collect();
    for sid in strokes {
        let Some(l) = vd::locate(&d, &sid) else { continue };
        let Some(mut v) = dj.pointer(&l.pointer).cloned() else { continue };
        if let Some(list) = v.get_mut("points").and_then(Value::as_array_mut) {
            for (k, a) in list.iter_mut().enumerate() {
                if !moved.contains(&(sid.clone(), k)) {
                    continue;
                }
                let (x, y) = (a[0].as_f64().unwrap_or(0.0) + delta.x, a[1].as_f64().unwrap_or(0.0) + delta.y);
                let mut q = P::new(x, y);
                // Nearest within the radius; ties: lower stroke ID, then lower segment and parameter.
                let best = contours
                    .iter()
                    .filter(|(list, _, _)| *list == l.list)
                    .filter_map(|(_, id, path)| nearest_on(path, q).map(|(dist, seg, t, at)| (dist, id, seg, t, at)))
                    .filter(|c| c.0 <= SNAP_RADIUS)
                    .min_by(|a, b| {
                        a.0.total_cmp(&b.0).then_with(|| a.1.cmp(b.1)).then(a.2.cmp(&b.2)).then(a.3.total_cmp(&b.3))
                    });
                if let Some(c) = best {
                    q = c.4;
                }
                a[0] = json!(q.x);
                a[1] = json!(q.y);
            }
        }
        drop_spine_if_brush(&mut v);
        ops.push(op("replace", &l.pointer, Some(v)));
        modified.push(sid);
    }
    let before = doc;
    cx.patch_entries(vec![(EntryKey::Drawing(t.id.clone()), ops)])?;
    let mut changed = Changed { modified_ids: modified, ..Changed::default() };
    crate::planar::settled(cx, &before, &t.id, &[])?.report(&mut changed);
    Ok(ContourResult { drawing_id: t.id.as_str().to_owned(), changed, rev: committed_rev(cx)? })
}

/// Handle behaviour of `draw.point.set`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum HandleMode {
    /// The handle not given mirrors the given one.
    Symmetric,
    /// Handles independent.
    Broken,
    /// Both handles removed (a corner).
    Reset,
}

/// `draw.point.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PointSetParams {
    /// The stroke.
    stroke_id: String,
    /// The anchor.
    anchor: u32,
    /// New position.
    #[serde(default)]
    position: Option<Point>,
    /// Incoming handle (offset).
    #[serde(default)]
    in_handle: Option<Point>,
    /// Outgoing handle (offset).
    #[serde(default)]
    out_handle: Option<Point>,
    /// Handle mode.
    #[serde(default)]
    handle_mode: Option<HandleMode>,
}

/// `{strokeId, rev}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PointSetResult {
    /// The stroke.
    stroke_id: String,
    /// Changed objects.
    #[serde(flatten)]
    changed: Changed,
    /// The drawing revision after the change.
    rev: u64,
}

#[command(
    id = "draw.point.set",
    kind = action,
    label = "Set point",
    scopes = [edit],
    undo = "Contour Editing",
    needs_project,
    example(title = "Pull out symmetric handles on the pencil line's end", params = r#"{"strokeId": "dr_01K742SG0KKMCAP9X1E6Z5P4SW/st_01K742SG0R61SYK9GG0HGYVNBT", "anchor": 1, "inHandle": {"x": -10, "y": 5}, "handleMode": "symmetric"}"#),
    gui = "Drawing view: Contour Editor handle drag (M4 C4.2)",
)]
pub(crate) fn draw_point_set(cx: &mut Cx, p: PointSetParams) -> Result<PointSetResult, CommandError> {
    let (did, _d, l, mut v) = contour_target(cx, &p.stroke_id)?;
    let mut path = path_of(&v);
    let k = p.anchor as usize;
    let Some(a) = path.anchors.get_mut(k) else {
        return Err(err(
            "drawing.objectNotFound",
            format!("the stroke has no anchor {k}"),
            json!({ "pointer": "/anchor" }),
        ));
    };
    if let Some(q) = p.position {
        a.p = P::new(q.x, q.y);
    }
    if let Some(h) = p.in_handle {
        a.inh = P::new(h.x, h.y);
    }
    if let Some(h) = p.out_handle {
        a.outh = P::new(h.x, h.y);
    }
    match p.handle_mode {
        Some(HandleMode::Reset) => {
            a.inh = P::default();
            a.outh = P::default();
        }
        Some(HandleMode::Symmetric) => match (p.in_handle, p.out_handle) {
            (Some(_), None) => a.outh = a.inh * -1.0,
            (None, Some(_)) => a.inh = a.outh * -1.0,
            _ => {}
        },
        Some(HandleMode::Broken) | None => {}
    }
    path.check("/position").map_err(|e| super::draw::invalid_geometry(&e))?;
    set_path(&mut v, &path);
    let (changed, rev) = commit_stroke(cx, &did, &l, v)?;
    Ok(PointSetResult { stroke_id: l.id, changed, rev })
}

/// `draw.point.delete` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PointDeleteParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Anchors (default: the point selection).
    #[serde(default)]
    points: Option<Vec<AnchorRef>>,
}

#[command(
    id = "draw.point.delete",
    precheck = super::draw::precheck_target,
    kind = action,
    label = "Delete points",
    scopes = [edit],
    undo = "Contour Editing",
    needs_project,
    example(title = "Delete the middle anchor of the brush outline", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "points": [{"strokeId": "st_01K742SG0QD1F2HFZSX8R4V7WP", "anchor": 1}]}"#),
    gui = "Drawing view: Contour Editor, Delete (M4 C4.2)",
)]
pub(crate) fn draw_point_delete(cx: &mut Cx, p: PointDeleteParams) -> Result<ContourResult, CommandError> {
    let mut none = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut none)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    check_editable(&d, None)?;
    let pts = match p.points {
        Some(v) => v,
        None => super::draw_select::point_refs(cx, &d),
    };
    if pts.is_empty() {
        return Err(nothing_to_do("delete"));
    }
    let mut by: std::collections::BTreeMap<String, BTreeSet<usize>> = std::collections::BTreeMap::new();
    for (i, a) in pts.iter().enumerate() {
        let ok = vd::locate(&d, &a.stroke_id)
            .and_then(|l| vd::stroke_at(&d, &l.pointer))
            .is_some_and(|s| (a.anchor as usize) < s.points().len());
        if !ok {
            return Err(err(
                "drawing.objectNotFound",
                format!("stroke `{}` has no anchor {}", a.stroke_id, a.anchor),
                json!({ "pointer": format!("/points/{i}") }),
            ));
        }
        by.entry(a.stroke_id.clone()).or_default().insert(a.anchor as usize);
    }
    let dj = drawing_json(&doc, &t.id)?;
    let mut ops = Vec::new();
    let mut removals: Vec<Located> = Vec::new();
    let mut modified = Vec::new();
    for (sid, gone) in &by {
        let Some(l) = vd::locate(&d, sid) else { continue };
        let Some(mut v) = dj.pointer(&l.pointer).cloned() else { continue };
        let path = path_of(&v);
        let kept: Vec<tt_vector::path::Anchor> =
            path.anchors.iter().enumerate().filter(|(k, _)| !gone.contains(k)).map(|(_, a)| *a).collect();
        let min = if path.closed { 3 } else { 2 };
        if kept.len() < min {
            removals.push(l);
            continue;
        }
        set_path(&mut v, &Path { anchors: kept, closed: path.closed });
        ops.push(op("replace", &l.pointer, Some(v)));
        modified.push(sid.clone());
    }
    // Strokes left without enough anchors are removed, from the end of each list first.
    removals.sort_by(|a, b| b.list.cmp(&a.list).then(b.index.cmp(&a.index)));
    let removed: Vec<String> = removals.iter().map(|l| l.id.clone()).collect();
    ops.extend(removals.iter().map(|l| op("remove", &l.pointer, None)));
    cx.patch_entries(vec![(EntryKey::Drawing(t.id.clone()), ops)])?;
    let mut changed = Changed { modified_ids: modified, removed_ids: removed, ..Changed::default() };
    crate::planar::settled(cx, &doc, &t.id, &[])?.report(&mut changed);
    Ok(ContourResult { drawing_id: t.id.as_str().to_owned(), changed, rev: committed_rev(cx)? })
}

/// `draw.segment.move` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SegmentMoveParams {
    /// The stroke.
    stroke_id: String,
    /// The segment index.
    segment: u32,
    /// The parameter of the point that moves (0–1).
    #[serde(default = "half")]
    #[schemars(range(min = 0.0, max = 1.0))]
    t: f64,
    /// Offset of that point (du).
    delta: Point,
}

fn half() -> f64 {
    0.5
}

#[command(
    id = "draw.segment.move",
    kind = action,
    label = "Move segment",
    scopes = [edit],
    undo = "Contour Editing",
    needs_project,
    example(title = "Bend the pencil line's segment", params = r#"{"strokeId": "dr_01K742SG0KKMCAP9X1E6Z5P4SW/st_01K742SG0R61SYK9GG0HGYVNBT", "segment": 0, "delta": {"x": 0, "y": 20}}"#),
    gui = "Drawing view: Contour Editor segment drag (M4 C4.2)",
)]
pub(crate) fn draw_segment_move(cx: &mut Cx, p: SegmentMoveParams) -> Result<PointSetResult, CommandError> {
    if p.t <= 0.0 || p.t >= 1.0 {
        return Err(err("arg.invalid", "`t` must be strictly between 0 and 1", json!({ "pointer": "/t" })));
    }
    let (did, _d, l, mut v) = contour_target(cx, &p.stroke_id)?;
    let mut path = path_of(&v);
    let segs = path.segments();
    let s = p.segment as usize;
    if s >= segs.len() {
        return Err(err(
            "arg.invalid",
            format!("the stroke has {} segments", segs.len()),
            json!({ "pointer": "/segment" }),
        ));
    }
    // Both control points move by the same vector w, so B(t) moves by 3t(1−t)·w = delta.
    let w = P::new(p.delta.x, p.delta.y) * (1.0 / (3.0 * p.t * (1.0 - p.t)));
    let n = path.anchors.len();
    path.anchors[s].outh = path.anchors[s].outh + w;
    path.anchors[(s + 1) % n].inh = path.anchors[(s + 1) % n].inh + w;
    set_path(&mut v, &path);
    let (changed, rev) = commit_stroke(cx, &did, &l, v)?;
    Ok(PointSetResult { stroke_id: l.id, changed, rev })
}

/// `draw.repositionAll.apply` (ruling O-23): a current element when none is given, with a drawing bank.
pub(crate) fn precheck_reposition_all(cx: &Cx, v: &Value) -> Result<(), CommandError> {
    if v.get("elementId").is_some_and(|x| !x.is_null()) {
        return Ok(());
    }
    let element =
        cx.app().element.clone().ok_or_else(|| {
            err("drawing.noTarget", "there is no current element", json!({ "pointer": "/elementId" }))
        })?;
    let doc = cx.document()?;
    if doc.element(&element).and_then(|e| e.drawing_bank_id().clone()).is_none() {
        return Err(err("drawing.notVectorElement", "the element has no drawings", json!({ "pointer": "/elementId" })));
    }
    Ok(())
}
