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
        let (smoothed, n) = tt_vector::ops::smooth::smooth(&path, p.tolerance, &fixed);
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
        let brush = tt_vector::stroke::build(&spec, id.as_str()).map_err(|e| super::draw::invalid_geometry(&e))?;
        if let Some(o) = centre.as_object_mut() {
            o.insert("kind".into(), json!("invisible"));
            o.insert("colourId".into(), Value::Null);
            o.insert("lineStyle".into(), Value::Null);
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
