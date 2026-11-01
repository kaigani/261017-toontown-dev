//! Layer operations (02 §8.1, §8.2, §8.4, §13.5; M2 C2.3): `draw.arrange.apply`, `draw.flatten.apply` and
//! `draw.optimize.apply`.
//!
//! **Flatten** merges the selected layers (and the layers inside selected groups) into one layer at the top-most
//! selected position. Where content overlaps, the upper layer's colour replaces the lower one: a lower stroke whose
//! every point lies under an opaque painted zone or opaque brush stroke of a higher layer is discarded, the merged
//! strokes are re-noded by the settle pass, lower pencil and invisible pieces left under such cover are discarded
//! (SI-073), and every zone of the result takes the fill (colour and matrix) of the highest source layer painting its
//! interior. A `policy` (SI-073) generalises this for M4 auto-flatten and Draw Behind: `precedence: "lower"` lets lower
//! layers win, `position: "bottom"` keeps the merged layer at the bottom-most selected position. One selected layer is
//! a no-op.
//!
//! **Optimize** merges adjacent sibling layers of the selection only where the merge cannot change the composited
//! appearance (02 §8.2): no partially transparent zone, line or brush stroke of the upper layer (alpha below 255, any
//! texture) may overlap a painted zone, line or brush stroke of the lower one, and no painted zone of the upper layer
//! may overlap a line or brush stroke of the lower one (which the merged layer would draw above the fill). Overlap is
//! tested on zone outer rings, brush outlines and line centre paths (SI-073).
//!
//! **Arrange** reorders nodes back to front within their parent lists; strokes selected from part of a layer are
//! first separated into their own layer directly above it (02 §8.4).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_model::document::{Document, EntryKey};
use tt_model::entities::drawing::{Drawing, LayerHint, NodeKind, StrokeKind};
use tt_model::{Id, ObjectType};
use tt_registry::{CommandError, command};
use tt_vector::drawing::{self as vd, Located, ObjectKind};
use tt_vector::geom::P;
use tt_vector::ops::cut;
use tt_vector::path::Path;
use tt_vector::planar::fill::Fill;
use tt_vector::planar::graph::LayerMap;

use super::draw::{Changed, check_editable, committed_rev, err, nothing_to_do, op, removal_ops, target};
use crate::cx::Cx;

fn not_found(d: &Drawing, id: &str, i: usize) -> CommandError {
    err(
        "drawing.objectNotFound",
        format!("`{id}` is not an object of drawing `{}`", d.name()),
        json!({ "id": id, "pointer": format!("/ids/{i}") }),
    )
}

/// The layers (back to front) a selection covers: selected layers, the layers inside selected groups and the layers
/// of selected strokes.
fn selected_layers(cx: &Cx, d: &Drawing, ids: Option<&Vec<String>>) -> Result<Vec<String>, CommandError> {
    let list = match ids {
        Some(v) => v.clone(),
        None => super::draw::selection_ids(cx, d),
    };
    let all = crate::planar::layer_nodes(d);
    let mut want: BTreeSet<String> = BTreeSet::new();
    for (i, id) in list.iter().enumerate() {
        let local = vd::local_id(d.id().as_str(), id).unwrap_or(id);
        if local.starts_with("zn_") {
            continue;
        }
        let l = vd::locate(d, local).ok_or_else(|| not_found(d, id, i))?;
        match l.kind {
            ObjectKind::Stroke => {
                want.extend(l.parent.clone());
            }
            ObjectKind::Layer => {
                want.insert(l.id.clone());
            }
            ObjectKind::Group => {
                let prefix = format!("{}/", l.pointer);
                want.extend(
                    all.iter().filter(|(p, _)| p.starts_with(&prefix)).map(|(_, n)| n.id().as_str().to_owned()),
                );
            }
            _ => {}
        }
    }
    Ok(all.iter().map(|(_, n)| n.id().as_str().to_owned()).filter(|id| want.contains(id)).collect())
}

/// What a layer contributes to coverage and overlap tests.
struct Info {
    map: Arc<LayerMap>,
    fills: Vec<Option<Fill>>,
    /// Painted zones: outer ring, opaque.
    zones: Vec<(Vec<P>, bool)>,
    /// Pencil lines: centre path, opaque.
    lines: Vec<(Vec<P>, bool)>,
    /// Brush strokes: outline, opaque.
    brushes: Vec<(Vec<P>, bool)>,
}

impl Info {
    fn fill_at(&self, p: P) -> Option<&Fill> {
        self.map.zone_at(p).and_then(|z| self.fills.get(z)).and_then(Option::as_ref).filter(|f| f.painted())
    }

    /// Whether opaque content of this layer hides `p`.
    fn covers(&self, doc: &Document, p: P) -> bool {
        self.fill_at(p).is_some_and(|f| f.colour_id.as_deref().is_some_and(|c| super::colour::opaque(doc, c)))
            || self.brushes.iter().any(|(b, o)| *o && cut::inside(b, p))
    }
}

fn infos(cx: &Cx, d: &Drawing) -> Result<BTreeMap<String, Info>, CommandError> {
    let doc = cx.document()?;
    let is_opaque = |c: Option<&Id>| c.is_some_and(|c| super::colour::opaque(doc, c.as_str()));
    let mut out = BTreeMap::new();
    for lz in crate::planar::drawing_zones(&cx.planar_cache(), d)? {
        let mut lines = Vec::new();
        let mut brushes = Vec::new();
        if let Some(n) = vd::locate(d, &lz.layer.id).and_then(|l| vd::node_at(d, &l.pointer)) {
            for s in n.strokes().iter().flatten() {
                match s.kind() {
                    StrokeKind::Brush => {
                        brushes.push((
                            Path::from_points(s.points(), true).flatten(0.5),
                            is_opaque(s.colour_id().as_ref()),
                        ));
                    }
                    StrokeKind::Pencil => lines.push((
                        Path::from_points(s.points(), *s.closed()).flatten(0.5),
                        is_opaque(s.colour_id().as_ref()),
                    )),
                    StrokeKind::Invisible => {}
                }
            }
        }
        let zones = lz
            .map
            .zones
            .iter()
            .enumerate()
            .filter_map(|(z, _)| {
                let f = lz.fills.get(z)?.as_ref().filter(|f| f.painted())?;
                let ring = lz.map.zone_rings(z).into_iter().next()?;
                Some((ring, f.colour_id.as_deref().is_some_and(|c| super::colour::opaque(doc, c))))
            })
            .collect();
        out.insert(lz.layer.id.clone(), Info { map: lz.map, fills: lz.fills, zones, lines, brushes });
    }
    Ok(out)
}

fn points_of(s: &Value) -> (Vec<[f64; 6]>, bool) {
    let pts = s
        .get("points")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|a| serde_json::from_value::<[f64; 6]>(a.clone()).ok())
        .collect();
    (pts, s.get("closed").and_then(Value::as_bool).unwrap_or(false))
}

/// Groups (by pointer) every descendant of which is among `gone` layers: nothing would be left in them.
fn emptied_groups(d: &Drawing, gone: &BTreeSet<String>) -> Vec<Located> {
    fn empty(n: &tt_model::entities::drawing::DrawingNode, gone: &BTreeSet<String>) -> bool {
        match n.kind() {
            NodeKind::Layer => gone.contains(n.id().as_str()),
            NodeKind::Group => n.children().iter().flatten().all(|c| empty(c, gone)),
            _ => false,
        }
    }
    vd::objects(d)
        .into_iter()
        .filter(|o| o.kind == ObjectKind::Group)
        .filter(|o| vd::node_at(d, &o.pointer).is_some_and(|n| empty(n, gone)))
        .collect()
}

/// Merges `layers` (back to front, at least two) of `drawing` into one; returns the kept layer and the removed IDs
/// that existed before.
fn merge(
    cx: &mut Cx,
    drawing: &Id,
    layers: &[String],
    keep_top: bool,
    upper_wins: bool,
) -> Result<(String, Vec<String>), CommandError> {
    let before = cx.document()?.clone();
    let d = before.drawing(drawing).ok_or_else(|| super::draw::not_found(drawing.as_str()))?.clone();
    let info = infos(cx, &d)?;
    let order: Vec<&Info> = layers.iter().filter_map(|l| info.get(l)).collect();
    if order.len() != layers.len() {
        return Err(nothing_to_do("flatten"));
    }
    let n = order.len();
    let rank = |i: usize| if upper_wins { i } else { n - 1 - i };
    let covered = |i: usize, p: P| order.iter().enumerate().any(|(j, l)| rank(j) > rank(i) && l.covers(&before, p));
    let dj = before.entry_raw_json(&EntryKey::Drawing(drawing.clone()))?;
    let zones_ns = tt_vector::planar::fill::EXT_NAMESPACE;
    let mut merged: Vec<Value> = Vec::new();
    let mut origin: Vec<(String, usize)> = Vec::new();
    let mut hidden: Vec<String> = Vec::new();
    for (i, lid) in layers.iter().enumerate() {
        let Some(l) = vd::locate(&d, lid) else { continue };
        let strokes =
            dj.pointer(&format!("{}/strokes", l.pointer)).and_then(Value::as_array).cloned().unwrap_or_default();
        for mut s in strokes {
            let id = s.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
            let (pts, closed) = points_of(&s);
            let brush = s.get("kind").and_then(Value::as_str) == Some("brush");
            let samples = Path::from_points(&pts, closed || brush).flatten(0.5);
            if !samples.is_empty() && samples.iter().all(|p| covered(i, *p)) {
                hidden.push(id);
                continue;
            }
            // The merged layer's fills are resolved afresh below.
            if let Some(o) = s.as_object_mut() {
                o.insert("sides".into(), json!({ "left": null, "right": null }));
                if let Some(ext) = o.get_mut("ext").and_then(Value::as_object_mut) {
                    ext.remove(zones_ns);
                }
            }
            origin.push((id, i));
            merged.push(s);
        }
    }
    let keep = if keep_top { layers[n - 1].clone() } else { layers[0].clone() };
    let kl = vd::locate(&d, &keep).ok_or_else(|| nothing_to_do("flatten"))?;
    let mut ops = vec![op("replace", &format!("{}/strokes", kl.pointer), Some(Value::Array(merged)))];
    let gone: BTreeSet<String> = layers.iter().filter(|l| **l != keep).cloned().collect();
    let mut targets: Vec<Located> = gone.iter().filter_map(|l| vd::locate(&d, l)).collect();
    targets.extend(emptied_groups(&d, &gone));
    let (removals, mut removed) = removal_ops(&d, &targets);
    ops.extend(removals);
    cx.patch_entries(vec![(EntryKey::Drawing(drawing.clone()), ops)])?;
    cx.settle()?;
    // Lower pencil and invisible pieces left under cover go.
    let now = cx.document()?.clone();
    let dn = now.drawing(drawing).ok_or_else(|| super::draw::not_found(drawing.as_str()))?.clone();
    // One index of the merged drawing's objects: lookups stay linear in the drawing size (100 000-stroke layers).
    let index: BTreeMap<String, Located> = vd::objects(&dn).into_iter().map(|o| (o.id.clone(), o)).collect();
    let mut drop: Vec<Located> = Vec::new();
    for (orig, i) in &origin {
        for piece in cx.pieces_of(drawing, orig) {
            let Some(l) = index.get(&piece).cloned() else { continue };
            let Some(s) = vd::stroke_at(&dn, &l.pointer) else { continue };
            if *s.kind() != StrokeKind::Brush && cut::middle(s.points(), *s.closed()).is_some_and(|m| covered(*i, m)) {
                drop.push(l);
            }
        }
    }
    if !drop.is_empty() {
        let (ops, _) = removal_ops(&dn, &drop);
        cx.patch_entries(vec![(EntryKey::Drawing(drawing.clone()), ops)])?;
        cx.settle()?;
    }
    // Every zone takes the fill of the winning source layer painting its interior.
    let now = cx.document()?.clone();
    let dn = now.drawing(drawing).ok_or_else(|| super::draw::not_found(drawing.as_str()))?.clone();
    let mut by_rank: Vec<usize> = (0..n).collect();
    by_rank.sort_by_key(|i| std::cmp::Reverse(rank(*i)));
    for lz in crate::planar::drawing_zones(&cx.planar_cache(), &dn)? {
        if lz.layer.id != keep {
            continue;
        }
        let fills: BTreeMap<String, Option<Fill>> = lz
            .map
            .zones
            .iter()
            .map(|z| (z.id.clone(), by_rank.iter().find_map(|i| order[*i].fill_at(z.interior).cloned())))
            .collect();
        cx.paint_zones(drawing, &keep, fills);
    }
    let before_ids: BTreeSet<String> = vd::objects(&d).into_iter().map(|o| o.id).collect();
    let existed = |s: &String| before_ids.contains(s);
    removed.extend(hidden.into_iter().filter(existed));
    removed.extend(drop.into_iter().map(|l| l.id).filter(existed));
    Ok((keep, removed))
}

/// Which layer's colour wins where merged content overlaps.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Precedence {
    /// The upper layer's colour replaces the lower one (02 §8.1).
    #[default]
    Upper,
    /// The lower layer's colour wins (Draw Behind merges, SI-073).
    Lower,
}

/// Where the merged layer goes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Position {
    /// The top-most selected layer's position (02 §8.1).
    #[default]
    Top,
    /// The bottom-most selected layer's position.
    Bottom,
}

/// A flatten policy (SI-073).
#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct FlattenPolicy {
    /// Which colour wins where content overlaps.
    #[serde(default)]
    precedence: Precedence,
    /// Where the merged layer goes.
    #[serde(default)]
    position: Position,
}

/// `draw.flatten.apply` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct FlattenParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Layers, groups or strokes whose layers merge (default: the selection).
    #[serde(default)]
    ids: Option<Vec<String>>,
    /// Merge policy (SI-073; default: upper colour wins, merged layer at the top-most position).
    #[serde(default)]
    policy: FlattenPolicy,
}

/// `{layerId, removedIds, rev}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FlattenResult {
    /// The drawing.
    drawing_id: String,
    /// The merged layer.
    layer_id: String,
    /// Changed objects (`removedIds`: the merged-away layers and the discarded hidden strokes).
    #[serde(flatten)]
    changed: Changed,
    /// The drawing revision after the change.
    rev: u64,
}

#[command(
    id = "draw.flatten.apply",
    kind = action,
    label = "Flatten",
    scopes = [edit],
    undo = "Flatten Drawing",
    needs_project,
    example(title = "Flatten every layer of the first drawing", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "ids": ["nd_01K742SG0TX3WF1BSWWGMWM5WG", "st_01K742SG0R61SYK9GG0HGYVNBT", "st_01K742SG0QD1F2HFZSX8R4V7WP"]}"#),
    example(title = "Merge behind: the lower layer wins and stays at the bottom", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "ids": ["nd_01K742SG0TX3WF1BSWWGMWM5WG", "st_01K742SG0R61SYK9GG0HGYVNBT"], "policy": {"precedence": "lower", "position": "bottom"}}"#),
    gui = "Drawing view: Flatten (M4 C4.2)",
)]
pub(crate) fn draw_flatten_apply(cx: &mut Cx, p: FlattenParams) -> Result<FlattenResult, CommandError> {
    let mut none = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut none)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    check_editable(&d, None)?;
    let layers = selected_layers(cx, &d, p.ids.as_ref())?;
    match layers.len() {
        0 => Err(nothing_to_do("flatten")),
        1 => Ok(FlattenResult {
            drawing_id: t.id.as_str().to_owned(),
            layer_id: layers[0].clone(),
            changed: Changed::default(),
            rev: *d.rev(),
        }),
        _ => {
            let before = doc;
            let (keep, removed) = merge(
                cx,
                &t.id,
                &layers,
                p.policy.position == Position::Top,
                p.policy.precedence == Precedence::Upper,
            )?;
            let mut changed = Changed { modified_ids: vec![keep.clone()], removed_ids: removed, ..Changed::default() };
            crate::planar::settled(cx, &before, &t.id, &[])?.report(&mut changed);
            Ok(FlattenResult { drawing_id: t.id.as_str().to_owned(), layer_id: keep, changed, rev: committed_rev(cx)? })
        }
    }
}

/// Whether merging `upper` onto the `lower` layers leaves the composited appearance unchanged (02 §8.2).
fn mergeable(lower: &[&Info], upper: &Info) -> bool {
    let painted_or_drawn = |l: &Info| -> Vec<Vec<P>> {
        l.zones.iter().map(|z| z.0.clone()).chain(l.brushes.iter().map(|b| b.0.clone())).collect()
    };
    for low in lower {
        let areas = painted_or_drawn(low);
        let low_lines: Vec<&Vec<P>> = low.lines.iter().map(|l| &l.0).collect();
        let low_brushes: Vec<&Vec<P>> = low.brushes.iter().map(|b| &b.0).collect();
        // Partially transparent upper areas over anything painted or drawn below.
        for (area, opaque) in upper.zones.iter().chain(upper.brushes.iter()) {
            if !opaque
                && (areas.iter().any(|a| cut::polygons_meet(area, a))
                    || low_lines.iter().any(|l| cut::polyline_meets(l, area)))
            {
                return false;
            }
        }
        // Partially transparent upper lines over anything painted or drawn below.
        for (line, opaque) in &upper.lines {
            if !opaque
                && (areas.iter().any(|a| cut::polyline_meets(line, a))
                    || low_lines.iter().any(|l| cut::polylines_cross(line, l)))
            {
                return false;
            }
        }
        // Upper painted zones over lower lines or brush strokes (the merged layer would draw those above the fill).
        for (zone, _) in &upper.zones {
            if low_lines.iter().any(|l| cut::polyline_meets(l, zone))
                || low_brushes.iter().any(|b| cut::polygons_meet(b, zone))
            {
                return false;
            }
        }
    }
    true
}

/// `draw.optimize.apply` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct OptimizeParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Layers, groups or strokes whose layers are considered (default: the selection).
    #[serde(default)]
    ids: Option<Vec<String>>,
}

/// `{layerIds, merged, rev}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OptimizeResult {
    /// The drawing.
    drawing_id: String,
    /// The considered layers that remain, back to front.
    layer_ids: Vec<String>,
    /// Layers merged away.
    merged: u32,
    /// Changed objects.
    #[serde(flatten)]
    changed: Changed,
    /// The drawing revision after the change.
    rev: u64,
}

fn siblings_adjacent(d: &Drawing, a: &str, b: &str) -> bool {
    match (vd::locate(d, a), vd::locate(d, b)) {
        (Some(x), Some(y)) => x.list == y.list && x.index + 1 == y.index,
        _ => false,
    }
}

#[command(
    id = "draw.optimize.apply",
    kind = action,
    label = "Optimize",
    scopes = [edit],
    undo = "Optimize Drawing",
    needs_project,
    example(title = "Optimize the layers of the first drawing", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "ids": ["nd_01K742SG0TX3WF1BSWWGMWM5WG", "st_01K742SG0R61SYK9GG0HGYVNBT"]}"#),
    gui = "Drawing view: Optimize (M4 C4.2)",
)]
pub(crate) fn draw_optimize_apply(cx: &mut Cx, p: OptimizeParams) -> Result<OptimizeResult, CommandError> {
    let mut none = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut none)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    check_editable(&d, None)?;
    let layers = selected_layers(cx, &d, p.ids.as_ref())?;
    if layers.is_empty() {
        return Err(nothing_to_do("optimize"));
    }
    let info = infos(cx, &d)?;
    let mut groups: Vec<Vec<String>> = Vec::new();
    for l in &layers {
        let extend = groups.last().is_some_and(|g| {
            let last = g.last().map(String::as_str).unwrap_or_default();
            let lower: Vec<&Info> = g.iter().filter_map(|x| info.get(x)).collect();
            siblings_adjacent(&d, last, l) && info.get(l).is_some_and(|u| mergeable(&lower, u))
        });
        match groups.last_mut() {
            Some(g) if extend => g.push(l.clone()),
            _ => groups.push(vec![l.clone()]),
        }
    }
    let mut merged = 0u32;
    let mut kept = Vec::new();
    let mut changed = Changed::default();
    for g in &groups {
        if g.len() < 2 {
            kept.push(g[0].clone());
            continue;
        }
        let (keep, removed) = merge(cx, &t.id, g, true, true)?;
        merged += u32::try_from(g.len() - 1).unwrap_or(u32::MAX);
        changed.modified_ids.push(keep.clone());
        changed.removed_ids.extend(removed);
        kept.push(keep);
    }
    if merged == 0 {
        return Ok(OptimizeResult {
            drawing_id: t.id.as_str().to_owned(),
            layer_ids: kept,
            merged,
            changed,
            rev: *d.rev(),
        });
    }
    crate::planar::settled(cx, &doc, &t.id, &[])?.report(&mut changed);
    Ok(OptimizeResult {
        drawing_id: t.id.as_str().to_owned(),
        layer_ids: kept,
        merged,
        changed,
        rev: committed_rev(cx)?,
    })
}

// ----- arrange ---------------------------------------------------------------------------------------------------------

/// Arrange direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ArrangeTo {
    /// Above everything.
    Front,
    /// One position up.
    Forward,
    /// One position down.
    Backward,
    /// Behind everything.
    Back,
}

/// `draw.arrange.apply` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ArrangeParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Objects (default: the selection).
    #[serde(default)]
    ids: Option<Vec<String>>,
    /// Direction.
    to: ArrangeTo,
}

/// `{order, rev}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArrangeResult {
    /// The drawing.
    drawing_id: String,
    /// The drawing's top-level node IDs after the change, back to front.
    order: Vec<String>,
    /// Layers created by separating part of a layer.
    created_layer_ids: Vec<String>,
    /// Changed objects.
    #[serde(flatten)]
    changed: Changed,
    /// The drawing revision after the change.
    rev: u64,
}

fn node_id(v: &Value) -> String {
    v.get("id").and_then(Value::as_str).unwrap_or_default().to_owned()
}

/// Reorders a node list in place: `sel` says which are moving.
fn reorder(list: &mut Vec<Value>, sel: &BTreeSet<String>, to: ArrangeTo) {
    let is = |v: &Value| sel.contains(&node_id(v));
    match to {
        ArrangeTo::Front => {
            let (s, u): (Vec<Value>, Vec<Value>) = list.drain(..).partition(is);
            list.extend(u);
            list.extend(s);
        }
        ArrangeTo::Back => {
            let (s, u): (Vec<Value>, Vec<Value>) = list.drain(..).partition(is);
            list.extend(s);
            list.extend(u);
        }
        ArrangeTo::Forward => {
            for i in (0..list.len().saturating_sub(1)).rev() {
                if is(&list[i]) && !is(&list[i + 1]) {
                    list.swap(i, i + 1);
                }
            }
        }
        ArrangeTo::Backward => {
            for i in 1..list.len() {
                if is(&list[i]) && !is(&list[i - 1]) {
                    list.swap(i, i - 1);
                }
            }
        }
    }
}

/// Applies `f` to every node list of a drawing JSON tree (top-level `nodes` and every group's `children`), deepest
/// first.
fn each_list(nodes: &mut Vec<Value>, f: &mut impl FnMut(&mut Vec<Value>)) {
    for n in nodes.iter_mut() {
        if let Some(c) = n.get_mut("children").and_then(Value::as_array_mut) {
            each_list(c, f);
        }
    }
    f(nodes);
}

#[command(
    id = "draw.arrange.apply",
    kind = action,
    label = "Arrange",
    scopes = [edit],
    undo = "Bring to Front",
    needs_project,
    example(title = "Send the caption to the back", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "ids": ["nd_01K742SG0WFAQJFKAZKV4CD56C"], "to": "back"}"#),
    example(title = "Bring the pencil line to the front, separating it from its layer", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "ids": ["st_01K742SG0R61SYK9GG0HGYVNBT"], "to": "front"}"#),
    gui = "Drawing view: Arrange > Bring to Front / Bring Forward / Send Backward / Send to Back (M4 C4.2)",
)]
pub(crate) fn draw_arrange_apply(cx: &mut Cx, p: ArrangeParams) -> Result<ArrangeResult, CommandError> {
    let mut none = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut none)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    check_editable(&d, None)?;
    let list = match &p.ids {
        Some(v) => v.clone(),
        None => super::draw::selection_ids(cx, &d),
    };
    let mut objs: Vec<Located> = Vec::new();
    for (i, id) in list.iter().enumerate() {
        let local = vd::local_id(d.id().as_str(), id).unwrap_or(id);
        if local.starts_with("zn_") {
            continue;
        }
        let l = vd::locate(&d, local).ok_or_else(|| not_found(&d, id, i))?;
        if !objs.iter().any(|o| o.id == l.id) {
            objs.push(l);
        }
    }
    let pointers: Vec<String> = objs.iter().map(|o| o.pointer.clone()).collect();
    objs.retain(|o| !pointers.iter().any(|p| *p != o.pointer && o.pointer.starts_with(&format!("{p}/"))));
    if objs.is_empty() {
        return Err(nothing_to_do("arrange"));
    }
    // Strokes: a layer all of whose strokes are selected moves as a node; otherwise its selected strokes are
    // separated into a new layer directly above it.
    let mut by_layer: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut moving: BTreeSet<String> = BTreeSet::new();
    for o in &objs {
        match (o.kind, &o.parent) {
            (ObjectKind::Stroke, Some(layer)) => by_layer.entry(layer.clone()).or_default().push(o.id.clone()),
            _ => {
                moving.insert(o.id.clone());
            }
        }
    }
    let mut separate: BTreeMap<String, (String, BTreeSet<String>)> = BTreeMap::new();
    for (layer, strokes) in by_layer {
        let total = vd::locate(&d, &layer)
            .and_then(|l| vd::node_at(&d, &l.pointer))
            .map_or(0, |n| n.strokes().iter().flatten().count());
        if strokes.len() == total {
            moving.insert(layer);
        } else {
            let nid = cx.new_id(ObjectType::DrawingNode)?.as_str().to_owned();
            moving.insert(nid.clone());
            separate.insert(layer, (nid, strokes.into_iter().collect()));
        }
    }
    let mut dj = doc.entry_raw_json(&EntryKey::Drawing(t.id.clone()))?;
    let Some(nodes) = dj.get_mut("nodes").and_then(Value::as_array_mut) else {
        return Err(nothing_to_do("arrange"));
    };
    let mut fresh = Vec::new();
    each_list(nodes, &mut |list: &mut Vec<Value>| {
        let mut i = 0;
        while i < list.len() {
            let id = node_id(&list[i]);
            if let Some((nid, strokes)) = separate.get(&id) {
                let hint = if list[i].get("hint").and_then(Value::as_str) == Some("brush") {
                    LayerHint::Brush
                } else {
                    LayerHint::Pencil
                };
                let mut layer = vd::layer_json(nid, hint);
                let mut moved = Vec::new();
                if let Some(all) = list[i].get_mut("strokes").and_then(Value::as_array_mut) {
                    let (m, keep): (Vec<Value>, Vec<Value>) =
                        all.drain(..).partition(|s| strokes.contains(&node_id(s)));
                    *all = keep;
                    moved = m;
                }
                layer["strokes"] = Value::Array(moved);
                list.insert(i + 1, layer);
                fresh.push(nid.clone());
                i += 1;
            }
            i += 1;
        }
        reorder(list, &moving, p.to);
    });
    let order: Vec<String> = nodes.iter().map(node_id).collect();
    let new_nodes = Value::Array(nodes.clone());
    cx.set_label(match p.to {
        ArrangeTo::Front => "Bring to Front",
        ArrangeTo::Forward => "Bring Forward",
        ArrangeTo::Backward => "Send Backward",
        ArrangeTo::Back => "Send to Back",
    });
    cx.patch_entries(vec![(EntryKey::Drawing(t.id.clone()), vec![op("replace", "/nodes", Some(new_nodes))])])?;
    let mut changed = Changed {
        created_ids: fresh.clone(),
        modified_ids: moving.iter().filter(|m| !fresh.contains(m)).cloned().collect(),
        ..Changed::default()
    };
    crate::planar::settled(cx, &doc, &t.id, &[])?.report(&mut changed);
    Ok(ArrangeResult {
        drawing_id: t.id.as_str().to_owned(),
        order,
        created_layer_ids: fresh,
        changed,
        rev: committed_rev(cx)?,
    })
}
