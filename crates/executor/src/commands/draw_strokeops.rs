//! Stroke conversions (02 §8.3, §8.6, §13.5; M2 C2.3): `draw.smooth.apply` and `draw.linesToBrush.apply`.
//!
//! **Smooth** removes anchors within a tolerance of the original shape (`tt_vector::ops::smooth`). Path ends, anchors
//! shared with another stroke of the layer (junctions) and, with a point selection, unselected anchors are kept, so
//! zones stay closed and fills carry over through the settle pass. A brush outline that loses anchors becomes
//! authoritative (spine dropped, 02 §3.2); one left unchanged keeps spine, pressure and pen. Each invocation smooths
//! further.
//!
//! **Convert Lines to Brush** sweeps the current brush pen (tip shape and maximum size, no pressure) along each
//! selected pencil line, fills the outline with the line's colour and keeps the centre path as an invisible stroke in
//! place, so zones, holes, fills and colour mappings stay as they were.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_model::ObjectType;
use tt_model::document::EntryKey;
use tt_model::entities::drawing::{Drawing, StrokeKind};
use tt_registry::{CommandError, command};
use tt_vector::brush::BrushPen;
use tt_vector::drawing::{self as vd, Located, ObjectKind};
use tt_vector::path::Path;

use super::draw::{Changed, check_editable, committed_rev, err, nothing_to_do, op, target};
use super::draw_select::AnchorRef;
use crate::cx::Cx;

/// The strokes a command acts on (`ids`, or the selection): strokes listed directly and every stroke inside listed
/// layers and groups; zones are skipped.
pub(crate) fn strokes_of(
    cx: &Cx,
    d: &Drawing,
    ids: Option<&Vec<String>>,
    what: &str,
) -> Result<Vec<Located>, CommandError> {
    let list = match ids {
        Some(v) => v.clone(),
        None => super::draw::selection_ids(cx, d),
    };
    let all = vd::objects(d);
    let mut out: Vec<Located> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for (i, id) in list.iter().enumerate() {
        let local = vd::local_id(d.id().as_str(), id).unwrap_or(id);
        if local.starts_with("zn_") {
            // A zone stands for its bounding strokes; a stale zone ID refuses the call (C23-R1-09).
            let (_, strokes) = super::draw::zone_target(cx, d, local, &format!("/ids/{i}"))?;
            for s in strokes {
                if let Some(l) = vd::locate(d, &s)
                    && seen.insert(l.id.clone())
                {
                    out.push(l);
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
        let found: Vec<Located> = if l.kind == ObjectKind::Stroke {
            vec![l]
        } else {
            let prefix = format!("{}/", l.pointer);
            all.iter().filter(|o| o.kind == ObjectKind::Stroke && o.pointer.starts_with(&prefix)).cloned().collect()
        };
        for f in found {
            if seen.insert(f.id.clone()) {
                out.push(f);
            }
        }
    }
    if out.is_empty() {
        return Err(nothing_to_do(what));
    }
    Ok(out)
}

/// Work units (point-to-segment distance evaluations) one Smooth call may spend: a few seconds of work, well before
/// the 30 s watchdog (never-block rule; C23-R1-07).
const SMOOTH_WORK_BUDGET: u64 = 150_000_000;

/// `draw.smooth.apply` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SmoothParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Strokes, layers or groups (default: the selection).
    #[serde(default)]
    ids: Option<Vec<String>>,
    /// Anchors that may go (default: the Contour Editor's point selection when no object is selected).
    #[serde(default)]
    points: Option<Vec<AnchorRef>>,
    /// Tolerance (du).
    #[serde(default = "one")]
    #[schemars(range(min = 0.0, max = 100.0))]
    tolerance: f64,
}

fn one() -> f64 {
    1.0
}

/// `{removedPointCount, rev}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SmoothResult {
    /// The drawing.
    drawing_id: String,
    /// Anchors removed.
    removed_point_count: u32,
    /// Changed objects.
    #[serde(flatten)]
    changed: Changed,
    /// The drawing revision after the change.
    rev: u64,
}

#[command(
    id = "draw.smooth.apply",
    kind = action,
    label = "Smooth",
    scopes = [edit],
    undo = "Smooth Selection",
    needs_project,
    example(title = "Smooth the brush stroke with the recommended tolerance", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "ids": ["st_01K742SG0QD1F2HFZSX8R4V7WP"]}"#),
    gui = "Drawing view: Smooth (M4 C4.2)",
)]
pub(crate) fn draw_smooth_apply(cx: &mut Cx, p: SmoothParams) -> Result<SmoothResult, CommandError> {
    if !(p.tolerance.is_finite() && p.tolerance >= 0.0) {
        return Err(err(
            "arg.invalid",
            "the tolerance must be a non-negative number",
            json!({ "pointer": "/tolerance" }),
        ));
    }
    let mut none = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut none)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    check_editable(&d, None)?;
    let points = match (&p.points, &p.ids) {
        (Some(v), _) => Some(v.clone()),
        (None, None) if super::draw::selection_ids(cx, &d).is_empty() => {
            let v = super::draw_select::point_refs(cx, &d);
            (!v.is_empty()).then_some(v)
        }
        _ => None,
    };
    let (targets, allowed): (Vec<Located>, Option<BTreeMap<String, BTreeSet<usize>>>) = match points {
        Some(pts) => {
            let mut by: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
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
            (by.keys().filter_map(|s| vd::locate(&d, s)).collect(), Some(by))
        }
        None => (strokes_of(cx, &d, p.ids.as_ref(), "smooth")?, None),
    };
    let dj = doc.entry_raw_json(&EntryKey::Drawing(t.id.clone()))?;
    let mut ops = Vec::new();
    let mut modified = Vec::new();
    let mut removed_points = 0usize;
    // One work account for the whole call (never-block rule; C23-R1-07).
    let mut work = tt_vector::work::Work::new(SMOOTH_WORK_BUDGET, "smoothing these strokes");
    for l in &targets {
        let Some(s) = vd::stroke_at(&d, &l.pointer) else { continue };
        let Some(mut v) = dj.pointer(&l.pointer).cloned() else { continue };
        let brush = *s.kind() == StrokeKind::Brush;
        let path = Path::from_points(s.points(), brush || *s.closed());
        // Anchors shared with another stroke of the same layer are junctions: kept.
        let others: BTreeSet<(u64, u64)> = vd::objects(&d)
            .iter()
            .filter(|o| o.list == l.list && o.id != l.id && o.kind == ObjectKind::Stroke)
            .filter_map(|o| vd::stroke_at(&d, &o.pointer))
            .flat_map(|o| o.points().iter().map(|a| (a[0].to_bits(), a[1].to_bits())).collect::<Vec<_>>())
            .collect();
        let allow = allowed.as_ref().and_then(|a| a.get(&l.id));
        let fixed = |i: usize| {
            let a = path.anchors[i].p;
            others.contains(&(a.x.to_bits(), a.y.to_bits())) || allow.is_some_and(|set| !set.contains(&i))
        };
        let (smoothed, n) = tt_vector::ops::smooth::smooth_within(&path, p.tolerance, &fixed, &mut work)
            .map_err(|e| crate::planar::geom_error(&e))?;
        if n == 0 {
            continue;
        }
        removed_points += n;
        if let Some(o) = v.as_object_mut() {
            o.insert("points".into(), smoothed.points_json());
            if brush {
                o.insert("spine".into(), Value::Null);
            }
        }
        ops.push(op("replace", &l.pointer, Some(v)));
        modified.push(l.id.clone());
    }
    if ops.is_empty() {
        return Ok(SmoothResult {
            drawing_id: t.id.as_str().to_owned(),
            removed_point_count: 0,
            changed: Changed::default(),
            rev: *d.rev(),
        });
    }
    cx.patch_entries(vec![(EntryKey::Drawing(t.id.clone()), ops)])?;
    let mut changed = Changed { modified_ids: modified, ..Changed::default() };
    crate::planar::settled(cx, &doc, &t.id, &[])?.report(&mut changed);
    Ok(SmoothResult {
        drawing_id: t.id.as_str().to_owned(),
        removed_point_count: u32::try_from(removed_points).unwrap_or(u32::MAX),
        changed,
        rev: committed_rev(cx)?,
    })
}

/// `draw.linesToBrush.apply` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct LinesToBrushParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Pencil lines, or layers and groups holding them (default: the selection).
    #[serde(default)]
    ids: Option<Vec<String>>,
    /// The brush pen (default: the current brush pen).
    #[serde(default)]
    pen_id: Option<String>,
}

/// `{strokeIds, rev}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LinesToBrushResult {
    /// The drawing.
    drawing_id: String,
    /// The new brush strokes.
    stroke_ids: Vec<String>,
    /// The invisible centre paths the lines became (their pieces where the new outlines split them).
    centre_ids: Vec<String>,
    /// Changed objects.
    #[serde(flatten)]
    changed: Changed,
    /// The drawing revision after the change.
    rev: u64,
}

#[command(
    id = "draw.linesToBrush.apply",
    kind = action,
    label = "Convert Lines to Brush",
    scopes = [edit],
    undo = "Convert Lines to Brush",
    needs_project,
    example(title = "Convert the pencil line to a brush stroke", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "ids": ["st_01K742SG0R61SYK9GG0HGYVNBT"]}"#),
    gui = "Drawing view: Convert Lines to Brush (M4 C4.2)",
)]
pub(crate) fn draw_lines_to_brush_apply(
    cx: &mut Cx,
    p: LinesToBrushParams,
) -> Result<LinesToBrushResult, CommandError> {
    let mut none = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut none)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    check_editable(&d, None)?;
    let lines: Vec<Located> = strokes_of(cx, &d, p.ids.as_ref(), "convert")?
        .into_iter()
        .filter(|l| vd::stroke_at(&d, &l.pointer).is_some_and(|s| *s.kind() == StrokeKind::Pencil))
        .collect();
    if lines.is_empty() {
        return Err(nothing_to_do("convert"));
    }
    let pc = super::draw_stroke::pen_choice(cx, p.pen_id.as_deref(), None)?;
    let pen =
        BrushPen { tip: pc.pen.tip, min_size: pc.pen.max_size, max_size: pc.pen.max_size, tip_angle: pc.pen.tip_angle };
    let dj = doc.entry_raw_json(&EntryKey::Drawing(t.id.clone()))?;
    let mut ops = Vec::new();
    let mut created = Vec::new();
    let mut centres = Vec::new();
    // Insert from the end of each list first so earlier indices stay valid.
    let mut order: Vec<&Located> = lines.iter().collect();
    order.sort_by(|a, b| b.list.cmp(&a.list).then(b.index.cmp(&a.index)));
    let mut made: Vec<(String, String)> = Vec::new();
    for l in order {
        let Some(s) = vd::stroke_at(&d, &l.pointer) else { continue };
        let Some(mut centre) = dj.pointer(&l.pointer).cloned() else { continue };
        let path = Path::from_points(s.points(), *s.closed());
        let spec = tt_vector::stroke::Spec {
            kind: StrokeKind::Brush,
            geometry: tt_vector::stroke::Geometry::Bezier(path.clone()),
            closed: path.closed,
            pressure: None,
            smoothness: 0.0,
            pen,
            pen_id: pc.pen_id.clone(),
            colour_id: s.colour_id().as_ref().map(|c| c.as_str().to_owned()),
            thickness: 0.0,
        };
        let id = cx.new_id(ObjectType::Stroke)?;
        let mut brush = tt_vector::stroke::build(&spec, id.as_str()).map_err(|e| super::draw::invalid_geometry(&e))?;
        // The line's gradient or texture mapping moves to the brush (SI-068); the invisible centre keeps no paint
        // fields (C23-R1-05).
        brush["fillMatrix"] = centre.get("fillMatrix").cloned().unwrap_or(Value::Null);
        if let Some(o) = centre.as_object_mut() {
            o.insert("kind".into(), json!("invisible"));
            o.insert("colourId".into(), Value::Null);
            o.insert("lineStyle".into(), Value::Null);
            o.insert("fillMatrix".into(), Value::Null);
        }
        ops.push(op("replace", &l.pointer, Some(centre)));
        ops.push(op("add", &format!("{}/{}", l.list, l.index + 1), Some(brush)));
        made.push((l.id.clone(), id.as_str().to_owned()));
    }
    made.reverse();
    for (c, b) in made {
        centres.push(c);
        created.push(b);
    }
    cx.patch_entries(vec![(EntryKey::Drawing(t.id.clone()), ops)])?;
    let mut changed = Changed { created_ids: created.clone(), modified_ids: centres.clone(), ..Changed::default() };
    let settled = crate::planar::settled(cx, &doc, &t.id, &created)?;
    settled.report(&mut changed);
    let stroke_ids = created.iter().flat_map(|c| cx.pieces_of(&t.id, c)).collect();
    // A centre path the new outline meets is split by noding: its pieces are reported.
    let centres = centres.iter().flat_map(|c| cx.pieces_of(&t.id, c)).collect();
    Ok(LinesToBrushResult {
        drawing_id: t.id.as_str().to_owned(),
        stroke_ids,
        centre_ids: centres,
        changed,
        rev: committed_rev(cx)?,
    })
}

/// `draw.breakApart.apply` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct BreakApartParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Text objects (default: the selected text objects).
    #[serde(default)]
    text_ids: Option<Vec<String>>,
}

/// `{groupIds, rev}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BreakApartResult {
    /// The drawing.
    drawing_id: String,
    /// One group per character, in text order.
    group_ids: Vec<String>,
    /// Changed objects.
    #[serde(flatten)]
    changed: Changed,
    /// The drawing revision after the change.
    rev: u64,
}

#[command(
    id = "draw.breakApart.apply",
    kind = action,
    label = "Break Text Apart",
    scopes = [edit],
    undo = "Break Text Apart",
    needs_project,
    example(title = "Break the caption into glyph shapes", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "textIds": ["nd_01K742SG0WFAQJFKAZKV4CD56C"]}"#),
    gui = "Drawing view: Break Text Apart (M4 C4.2)",
)]
pub(crate) fn draw_break_apart_apply(cx: &mut Cx, p: BreakApartParams) -> Result<BreakApartResult, CommandError> {
    use tt_model::entities::drawing::{LayerHint, NodeKind};
    let mut none = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut none)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    check_editable(&d, None)?;
    let list = match &p.text_ids {
        Some(v) => v.clone(),
        None => super::draw::selection_ids(cx, &d),
    };
    let mut texts: Vec<Located> = Vec::new();
    for (i, id) in list.iter().enumerate() {
        let local = vd::local_id(d.id().as_str(), id).unwrap_or(id);
        let l =
            vd::locate(&d, local).filter(|l| vd::node_at(&d, &l.pointer).is_some_and(|n| *n.kind() == NodeKind::Text));
        match l {
            Some(l) => texts.push(l),
            None if p.text_ids.is_some() => {
                return Err(err(
                    "drawing.objectNotFound",
                    format!("`{id}` is not a text object of drawing `{}`", d.name()),
                    json!({ "pointer": format!("/textIds/{i}") }),
                ));
            }
            None => {}
        }
    }
    if texts.is_empty() {
        return Err(nothing_to_do("break apart"));
    }
    // Replace from the end of each list first so earlier indices stay valid.
    texts.sort_by(|a, b| b.list.cmp(&a.list).then(b.index.cmp(&a.index)));
    let mut ops = Vec::new();
    let mut groups_all: Vec<Vec<String>> = Vec::new();
    let mut created = Vec::new();
    for l in &texts {
        let Some(n) = vd::node_at(&d, &l.pointer) else { continue };
        let Some(content) = vd::text_content(n) else { continue };
        let fixed = (n.width_mode().as_ref().map(|w| format!("{w:?}").to_lowercase()).as_deref() == Some("fixed"))
            .then(|| n.width().unwrap_or(0.0));
        let shapes = tt_vector::ops::glyphs::shapes(&content, fixed);
        let visible = content.chars.iter().filter(|c| !c.is_whitespace()).count();
        if shapes.len() < visible {
            return Err(err(
                "text.glyphsUnavailable",
                "the text's font has no outlines for every character on this machine",
                json!({ "id": l.id, "missingFonts": vd::text_layout(n).map(|l| l.missing_fonts).unwrap_or_default() }),
            ));
        }
        let m = n.transform().map_or(tt_vector::geom::Affine::IDENTITY, tt_vector::geom::Affine);
        let mut nodes = Vec::new();
        let mut gids = Vec::new();
        for g in shapes {
            let Some(path) = tt_vector::ops::glyphs::bridge_paths(&g.contours) else { continue };
            let path = path.transformed(&m);
            let sid = cx.new_id(ObjectType::Stroke)?.as_str().to_owned();
            let lid = cx.new_id(ObjectType::DrawingNode)?.as_str().to_owned();
            let gid = cx.new_id(ObjectType::DrawingNode)?.as_str().to_owned();
            let stroke = json!({
                "id": sid, "type": "stroke", "rev": 0, "kind": "brush", "closed": true, "points": path.points_json(),
                "colourId": g.colour_id, "fillMatrix": null, "lineStyle": null, "spine": null,
                "sides": { "left": null, "right": null },
            });
            let mut layer = vd::layer_json(&lid, LayerHint::Brush);
            layer["strokes"] = json!([stroke]);
            nodes.push(json!({ "id": gid, "type": "drawingNode", "rev": 0, "kind": "group", "children": [layer] }));
            created.extend([gid.clone(), lid, sid]);
            gids.push(gid);
        }
        ops.push(op("remove", &l.pointer, None));
        for (k, node) in nodes.into_iter().enumerate() {
            ops.push(op("add", &format!("{}/{}", l.list, l.index + k), Some(node)));
        }
        groups_all.push(gids);
    }
    groups_all.reverse();
    let group_ids: Vec<String> = groups_all.into_iter().flatten().collect();
    cx.patch_entries(vec![(EntryKey::Drawing(t.id.clone()), ops)])?;
    let removed: Vec<String> = texts.iter().map(|l| l.id.clone()).collect();
    let mut changed = Changed { created_ids: created.clone(), removed_ids: removed, ..Changed::default() };
    crate::planar::settled(cx, &doc, &t.id, &created)?.report(&mut changed);
    Ok(BreakApartResult { drawing_id: t.id.as_str().to_owned(), group_ids, changed, rev: committed_rev(cx)? })
}

/// `draw.centreLine.apply` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CentreLineParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Brush strokes, or layers and groups holding them (default: the selection).
    #[serde(default)]
    ids: Option<Vec<String>>,
    /// Line thickness (du; default: the source's average width).
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 1000.0))]
    thickness: Option<f64>,
}

/// `{strokeIds, removedIds, warnings, rev}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CentreLineResult {
    /// The drawing.
    drawing_id: String,
    /// The new pencil lines.
    stroke_ids: Vec<String>,
    /// `drawing.centreLineOnFill` (painted zones that are not brush strokes selected), `drawing.centreLineNoSpine`
    /// (outlines edited directly, kept; SI-076).
    warnings: Vec<String>,
    /// Brush strokes left as they were (no spine).
    skipped_ids: Vec<String>,
    /// Changed objects.
    #[serde(flatten)]
    changed: Changed,
    /// The drawing revision after the change.
    rev: u64,
}

#[command(
    id = "draw.centreLine.apply",
    kind = action,
    label = "Extract Centre Line",
    scopes = [edit],
    undo = "Extract Centre Line",
    needs_project,
    example(title = "Turn the brush stroke into a pencil line", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "ids": ["st_01K742SG0QD1F2HFZSX8R4V7WP"]}"#),
    gui = "Drawing view: Extract Centre Line (M4 C4.2)",
)]
pub(crate) fn draw_centre_line_apply(cx: &mut Cx, p: CentreLineParams) -> Result<CentreLineResult, CommandError> {
    let mut none = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut none)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    check_editable(&d, None)?;
    let list = p.ids.clone().unwrap_or_else(|| super::draw::selection_ids(cx, &d));
    let mut warnings = Vec::new();
    // Painted zones in the selection that are not brush strokes would give unwanted centre lines (02 §8.5).
    if list.iter().any(|i| i.starts_with("zn_") || i.contains("/zn_")) {
        let zones = crate::planar::drawing_zones(&cx.planar_cache(), &d)?;
        let painted = list.iter().any(|i| {
            let local = vd::local_id(d.id().as_str(), i).unwrap_or(i);
            zones.iter().any(|lz| {
                lz.map
                    .zone_by_id(local)
                    .is_some_and(|z| lz.fills.get(z).cloned().flatten().is_some_and(|f| f.painted()))
            })
        });
        if painted {
            warnings.push("drawing.centreLineOnFill".to_owned());
        }
    }
    let brushes: Vec<Located> = strokes_of(cx, &d, Some(&list), "extract")?
        .into_iter()
        .filter(|l| vd::stroke_at(&d, &l.pointer).is_some_and(|s| *s.kind() == StrokeKind::Brush))
        .collect();
    if brushes.is_empty() {
        // Only painted zones that are not brush strokes: the typed reason (02 §8.5; C23-R1-12).
        if warnings.iter().any(|w| w == "drawing.centreLineOnFill") {
            return Err(err(
                "drawing.centreLineOnFill",
                "the selection holds painted zones but no brush strokes; extract centre lines before filling",
                json!({ "pointer": "/ids" }),
            ));
        }
        return Err(nothing_to_do("extract"));
    }
    let mut order: Vec<&Located> = brushes.iter().collect();
    order.sort_by(|a, b| b.list.cmp(&a.list).then(b.index.cmp(&a.index)));
    let mut ops = Vec::new();
    let mut made: Vec<(String, String)> = Vec::new();
    let mut skipped = Vec::new();
    for l in order {
        let Some(s) = vd::stroke_at(&d, &l.pointer) else { continue };
        let Some(spine) = s.spine().as_ref() else {
            skipped.push(l.id.clone());
            continue;
        };
        let pen = &spine.pen;
        // Average width over the pressure profile (pen width at pressure q: min + (max - min) q).
        let mean_p = if spine.pressure.is_empty() {
            1.0
        } else {
            spine.pressure.iter().map(|q| q[1]).sum::<f64>() / spine.pressure.len() as f64
        };
        let width = p.thickness.unwrap_or(pen.min_size + (pen.max_size - pen.min_size) * mean_p).max(0.1);
        let path = tt_vector::path::Path::from_points(&spine.points, false);
        let id = cx.new_id(ObjectType::Stroke)?.as_str().to_owned();
        let line = json!({
            "id": id, "type": "stroke", "rev": 0, "kind": "pencil", "closed": false, "points": path.points_json(),
            "colourId": s.colour_id().as_ref().map(|c| c.as_str()), "fillMatrix": s.fill_matrix(),
            "lineStyle": { "thickness": width, "lineType": "solid" }, "spine": null,
            "sides": { "left": null, "right": null },
        });
        ops.push(op("replace", &l.pointer, Some(line)));
        made.push((l.id.clone(), id));
    }
    if !skipped.is_empty() {
        warnings.push("drawing.centreLineNoSpine".to_owned());
    }
    if ops.is_empty() {
        return Ok(CentreLineResult {
            drawing_id: t.id.as_str().to_owned(),
            stroke_ids: Vec::new(),
            warnings,
            skipped_ids: skipped,
            changed: Changed::default(),
            rev: *d.rev(),
        });
    }
    made.reverse();
    cx.patch_entries(vec![(EntryKey::Drawing(t.id.clone()), ops)])?;
    let created: Vec<String> = made.iter().map(|m| m.1.clone()).collect();
    let mut changed = Changed {
        created_ids: created.clone(),
        removed_ids: made.iter().map(|m| m.0.clone()).collect(),
        ..Changed::default()
    };
    crate::planar::settled(cx, &doc, &t.id, &created)?.report(&mut changed);
    let stroke_ids = created.iter().flat_map(|c| cx.pieces_of(&t.id, c)).collect();
    Ok(CentreLineResult {
        drawing_id: t.id.as_str().to_owned(),
        stroke_ids,
        warnings,
        skipped_ids: skipped,
        changed,
        rev: committed_rev(cx)?,
    })
}
