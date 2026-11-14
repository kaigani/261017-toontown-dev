//! Fill mappings and recolouring (03 §8.7, §9.1, §9.3, §16; M2 C2.3): `colour.fillMapping.get`,
//! `colour.fillMapping.set`, `colour.selection.apply` and `colour.recolour.apply`.
//!
//! Zone fills are set through the settle pass (`Cx::paint_zones`), so they are stored on every bounding stroke side
//! and on brush exterior records alike (SI-060); stroke colours and matrices are stroke fields; text colours are the
//! runs' colours. Mappable objects are painted zones, brush strokes and pencil lines whose colour is a gradient or a
//! texture (SI-068); a solid colour keeps but ignores a stored matrix (03 §1.6).

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_model::document::{Document, EntryKey};
use tt_model::entities::drawing::{Drawing, NodeKind, StrokeKind};
use tt_model::entities::palette::SwatchKind;
use tt_model::{Id, ObjectType};
use tt_registry::{CommandError, command};
use tt_vector::drawing::{self as vd, ObjectKind};
use tt_vector::geom::{Affine, P, Rect};
use tt_vector::planar::fill::Fill;

use super::draw::{Point, check_editable, committed_rev, err, nothing_to_do, op, target};
use crate::cx::Cx;

const CLIPBOARD: &str = "fillMapping.clipboard";

/// What an object ID names in a drawing.
enum Target {
    Zone { layer: String, zone: String, fill: Option<Fill>, bounds: Rect },
    Stroke { pointer: String, kind: StrokeKind, colour: Option<String>, matrix: Option<[f64; 6]>, bounds: Option<Rect> },
    Text { pointer: String },
    Other,
}

fn find(cx: &Cx, d: &Drawing, id: &str, pointer: &str) -> Result<(String, Target), CommandError> {
    let local = vd::local_id(d.id().as_str(), id).unwrap_or(id).to_owned();
    if local.starts_with("zn_") {
        for lz in crate::planar::drawing_zones(&cx.planar_cache(), d)? {
            if let Some(z) = lz.map.zone_by_id(&local) {
                let zone = &lz.map.zones[z];
                let fill = lz.fills.get(z).cloned().flatten().filter(Fill::painted);
                return Ok((
                    local,
                    Target::Zone { layer: lz.layer.id.clone(), zone: zone.id.clone(), fill, bounds: zone.bounds },
                ));
            }
        }
        return Err(err(
            "colour.staleZoneId",
            format!(
                "zone `{id}` does not exist in drawing `{}` (zones are re-identified when their boundary changes)",
                d.name()
            ),
            json!({ "pointer": pointer, "id": id }),
        ));
    }
    let l = vd::locate(d, &local).ok_or_else(|| {
        err(
            "drawing.objectNotFound",
            format!("`{id}` is not an object of drawing `{}`", d.name()),
            json!({ "pointer": pointer, "id": id }),
        )
    })?;
    let t = match l.kind {
        ObjectKind::Stroke => match vd::stroke_at(d, &l.pointer) {
            Some(s) => Target::Stroke {
                pointer: l.pointer.clone(),
                kind: *s.kind(),
                colour: s.colour_id().as_ref().map(|c| c.as_str().to_owned()),
                matrix: *s.fill_matrix(),
                bounds: vd::stroke_bounds(s),
            },
            None => Target::Other,
        },
        _ if vd::node_at(d, &l.pointer).is_some_and(|n| *n.kind() == NodeKind::Text) => {
            Target::Text { pointer: l.pointer.clone() }
        }
        _ => Target::Other,
    };
    Ok((local, t))
}

fn yes() -> bool {
    true
}

/// Whether a colour is protected (SI-061): its swatch carries `dev.toontown.paint.protected`.
fn protected(doc: &Document, colour: &str) -> bool {
    super::colour::swatch_info(doc, colour).is_some_and(|(_, p)| p)
}

fn kind_name(k: SwatchKind) -> &'static str {
    match k {
        SwatchKind::Solid => "solid",
        SwatchKind::LinearGradient => "linearGradient",
        SwatchKind::RadialGradient => "radialGradient",
        SwatchKind::Texture => "texture",
    }
}

fn mappable(k: SwatchKind) -> bool {
    k != SwatchKind::Solid
}

/// The default mapping of a colour over a box, as painting computes it (03 §7.2; tile-on textures at native size).
fn default_mapping(doc: &Document, colour: &str, kind: SwatchKind, b: &Rect) -> Option<[f64; 6]> {
    super::colour::paint_matrix(kind, None, None, Some(b), super::colour::tile_size(doc, colour))
}

fn pt(p: P) -> Value {
    json!({ "x": p.x + 0.0, "y": p.y + 0.0 })
}

/// `{colourId, kind, matrix, linear?, radial?, texture?}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FillMappingResult {
    /// The colour.
    colour_id: String,
    /// The swatch kind (`solid`: the kept matrix, which a solid colour ignores; SI-075).
    kind: String,
    /// Unit space → drawing space.
    matrix: [f64; 6],
    /// Linear gradient handles `{start, end}`.
    #[serde(skip_serializing_if = "Option::is_none")]
    linear: Option<Value>,
    /// Radial gradient handles `{centre, radius, secondAxis}`.
    #[serde(skip_serializing_if = "Option::is_none")]
    radial: Option<Value>,
    /// Texture frame `{origin, xAxis, yAxis, tile}`.
    #[serde(skip_serializing_if = "Option::is_none")]
    texture: Option<Value>,
}

/// `colour.fillMapping.get` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct FillMappingGetParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// A painted zone, brush stroke or pencil line.
    object_id: String,
    /// Copy the mapping to the fill-mapping clipboard.
    #[serde(default)]
    to_clipboard: bool,
}

fn not_mappable(id: &str, reason: &str) -> CommandError {
    err(
        "colour.notMappable",
        format!("`{id}` has no gradient or texture fill mapping ({reason})"),
        json!({ "pointer": "/objectId", "objectId": id, "reason": reason }),
    )
}

/// The colour and stored matrix of a target, or why it has no mapping. `solid_too`: a solid colour reports its kept
/// matrix (03 §1.6), or the box mapping of the object when none is stored (SI-075).
fn mapping_of(doc: &Document, t: &Target, solid_too: bool) -> Result<(String, SwatchKind, [f64; 6]), &'static str> {
    let (colour, matrix, bounds) = match t {
        Target::Zone { fill: Some(f), bounds, .. } => (f.colour_id.clone(), f.matrix, Some(*bounds)),
        Target::Zone { fill: None, .. } => return Err("unpainted"),
        Target::Stroke { kind: StrokeKind::Invisible, .. } => return Err("invisible"),
        Target::Stroke { colour, matrix, bounds, .. } => (colour.clone(), *matrix, *bounds),
        Target::Text { .. } => return Err("text"),
        Target::Other => return Err("notPaintable"),
    };
    let colour = colour.ok_or("unpainted")?;
    let (kind, _) = super::colour::swatch_info(doc, &colour).ok_or("colourMissing")?;
    if !mappable(kind) {
        if !solid_too {
            return Err("solid");
        }
        let b = bounds.ok_or("noBounds")?;
        let boxed = [b.max_x - b.min_x, 0.0, 0.0, b.max_y - b.min_y, b.min_x, b.min_y];
        return Ok((colour, kind, matrix.unwrap_or(boxed)));
    }
    let m = matrix.or_else(|| bounds.and_then(|b| default_mapping(doc, &colour, kind, &b))).ok_or("noBounds")?;
    Ok((colour, kind, m))
}

#[command(
    id = "colour.fillMapping.get",
    kind = query,
    label = "Get fill mapping",
    scopes = [read],
    needs_project,
    example(title = "The brush stroke's fill mapping", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "objectId": "st_01K742SG0QD1F2HFZSX8R4V7WP"}"#),
    gui = "Drawing view: Edit Texture tool manipulator and Copy Fill Mapping (M4 C4.2)",
)]
pub(crate) fn colour_fill_mapping_get(cx: &mut Cx, p: FillMappingGetParams) -> Result<FillMappingResult, CommandError> {
    let mut none = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut none)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    let (id, tg) = find(cx, &d, &p.object_id, "/objectId")?;
    let (colour, kind, m) = mapping_of(&doc, &tg, true).map_err(|r| not_mappable(&id, r))?;
    let a = Affine(m);
    let (mut linear, mut radial, mut texture) = (None, None, None);
    match kind {
        SwatchKind::LinearGradient => {
            linear = Some(json!({ "start": pt(a.apply(P::new(0.0, 0.5))), "end": pt(a.apply(P::new(1.0, 0.5))) }))
        }
        SwatchKind::RadialGradient => {
            let c = a.apply(P::new(0.5, 0.5));
            radial = Some(
                json!({ "centre": pt(c), "radius": c.dist(a.apply(P::new(1.0, 0.5))), "secondAxis": pt(a.apply(P::new(0.5, 1.0))) }),
            );
        }
        SwatchKind::Texture => {
            let o = a.apply(P::new(0.0, 0.0));
            texture = Some(json!({
                "origin": pt(o), "xAxis": pt(a.apply(P::new(1.0, 0.0)) - o), "yAxis": pt(a.apply(P::new(0.0, 1.0)) - o),
                "tile": super::colour::tile_size(&doc, &colour).is_some(),
            }));
        }
        SwatchKind::Solid => {}
    }
    if p.to_clipboard {
        cx.app_mut().tool_memory.insert(CLIPBOARD.into(), json!({ "kind": kind_name(kind), "matrix": m }).to_string());
    }
    Ok(FillMappingResult {
        colour_id: colour,
        kind: kind_name(kind).into(),
        matrix: m.map(|v| v + 0.0),
        linear,
        radial,
        texture,
    })
}

/// Linear gradient handles.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct LinearIn {
    /// Where the gradient starts.
    start: Point,
    /// Where it ends.
    end: Point,
}

/// Radial gradient handles.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RadialIn {
    /// Centre.
    centre: Point,
    /// Radius (du).
    #[schemars(range(min = 0.0))]
    radius: f64,
}

/// A texture frame.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct TextureIn {
    /// Where the image's unit origin goes.
    origin: Point,
    /// The image's unit x axis (du).
    x_axis: Point,
    /// The image's unit y axis (du).
    y_axis: Point,
}

/// An explicit mapping (SI-025 placeholder filled, SI-075): exactly one member.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct MappingObject {
    /// A full unit → drawing matrix (any mappable kind).
    #[serde(default)]
    matrix: Option<[f64; 6]>,
    /// Linear gradient handles.
    #[serde(default)]
    linear: Option<LinearIn>,
    /// Radial gradient handles.
    #[serde(default)]
    radial: Option<RadialIn>,
    /// Texture frame.
    #[serde(default)]
    texture: Option<TextureIn>,
}

/// `"default"` or `"clipboard"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MappingName {
    /// Refit to the object's bounds (03 §7.2).
    Default,
    /// The mapping copied with `toClipboard`.
    Clipboard,
}

/// `mapping`.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(untagged)]
pub(crate) enum MappingIn {
    /// Named.
    Named(MappingName),
    /// Explicit.
    Explicit(MappingObject),
}

/// `colour.fillMapping.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct FillMappingSetParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Painted zones, brush strokes and pencil lines.
    object_ids: Vec<String>,
    /// The mapping.
    mapping: MappingIn,
}

/// A skipped object.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkippedObject {
    /// The object.
    object_id: String,
    /// Why (`solid`, `unpainted`, `invisible`, `text`, `notPaintable`, `kindMismatch`, `colourMissing`).
    reason: String,
}

/// `{changed, skipped, rev}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FillMappingSetResult {
    /// Objects whose mapping changed (zones by their ID after the change).
    changed: Vec<String>,
    /// Objects left alone and why.
    skipped: Vec<SkippedObject>,
    /// The drawing revision after the change.
    rev: u64,
}

fn explicit_matrix(m: &MappingObject, kind: SwatchKind) -> Result<[f64; 6], &'static str> {
    let given = [m.matrix.is_some(), m.linear.is_some(), m.radial.is_some(), m.texture.is_some()]
        .iter()
        .filter(|b| **b)
        .count();
    if given != 1 {
        return Err("invalid");
    }
    if let Some(mx) = m.matrix {
        return Ok(mx);
    }
    match (kind, m.linear, m.radial, m.texture) {
        (SwatchKind::LinearGradient, Some(l), _, _) => {
            let (a, b) = (P::new(l.start.x, l.start.y), P::new(l.end.x, l.end.y));
            super::colour::drag_matrix(kind, a, b).ok_or("invalid")
        }
        (SwatchKind::RadialGradient, _, Some(r), _) => {
            let c = P::new(r.centre.x, r.centre.y);
            Ok([2.0 * r.radius, 0.0, 0.0, 2.0 * r.radius, c.x - r.radius, c.y - r.radius])
        }
        (SwatchKind::Texture, _, _, Some(t)) => {
            Ok([t.x_axis.x, t.x_axis.y, t.y_axis.x, t.y_axis.y, t.origin.x, t.origin.y])
        }
        _ => Err("kindMismatch"),
    }
}

#[command(
    id = "colour.fillMapping.set",
    kind = action,
    label = "Edit fill mapping",
    scopes = [edit],
    undo = "Edit Fill Mapping",
    needs_project,
    example(title = "Refit the brush stroke's mapping to its bounds", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "objectIds": ["st_01K742SG0QD1F2HFZSX8R4V7WP"], "mapping": "default"}"#),
    example(title = "Set an explicit matrix", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "objectIds": ["st_01K742SG0QD1F2HFZSX8R4V7WP"], "mapping": {"matrix": [50, 0, 0, 50, 0, 0]}}"#),
    gui = "Drawing view: Edit Texture tool and Paste Fill Mapping (M4 C4.2)",
)]
pub(crate) fn colour_fill_mapping_set(
    cx: &mut Cx,
    p: FillMappingSetParams,
) -> Result<FillMappingSetResult, CommandError> {
    let mut none = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut none)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    check_editable(&d, None)?;
    if p.object_ids.is_empty() {
        return Err(nothing_to_do("map"));
    }
    let clipboard: Option<(String, [f64; 6])> = match p.mapping {
        MappingIn::Named(MappingName::Clipboard) => {
            let v: Value =
                cx.app().tool_memory.get(CLIPBOARD).and_then(|s| serde_json::from_str(s).ok()).ok_or_else(|| {
                    err(
                        "colour.clipboardEmpty",
                        "no fill mapping was copied (colour.fillMapping.get {toClipboard: true})",
                        json!({ "pointer": "/mapping" }),
                    )
                })?;
            let m: [f64; 6] = serde_json::from_value(v["matrix"].clone()).map_err(|_| {
                err("colour.clipboardEmpty", "the copied mapping is unreadable", json!({ "pointer": "/mapping" }))
            })?;
            Some((v["kind"].as_str().unwrap_or_default().to_owned(), m))
        }
        _ => None,
    };
    if let MappingIn::Explicit(m) = &p.mapping
        && let Err("invalid") =
            explicit_matrix(m, SwatchKind::Texture).or_else(|e| if e == "kindMismatch" { Ok([0.0; 6]) } else { Err(e) })
    {
        return Err(err(
            "arg.invalid",
            "give exactly one of `matrix`, `linear`, `radial` and `texture`",
            json!({ "pointer": "/mapping" }),
        ));
    }
    let dj = doc.entry_raw_json(&EntryKey::Drawing(t.id.clone()))?;
    let mut changed = Vec::new();
    let mut skipped = Vec::new();
    let mut ops = Vec::new();
    let mut zone_fills: BTreeMap<String, BTreeMap<String, Option<Fill>>> = BTreeMap::new();
    for (i, id) in p.object_ids.iter().enumerate() {
        let (local, tg) = find(cx, &d, id, &format!("/objectIds/{i}"))?;
        let (colour, kind, current) = match mapping_of(&doc, &tg, false) {
            Ok(v) => v,
            Err(reason) => {
                skipped.push(SkippedObject { object_id: local, reason: reason.into() });
                continue;
            }
        };
        let bounds = match &tg {
            Target::Zone { bounds, .. } => Some(*bounds),
            Target::Stroke { bounds, .. } => *bounds,
            _ => None,
        };
        let m = match (&p.mapping, &clipboard) {
            (MappingIn::Named(MappingName::Default), _) => {
                bounds.and_then(|b| default_mapping(&doc, &colour, kind, &b)).ok_or("noBounds")
            }
            (MappingIn::Named(MappingName::Clipboard), Some((k, m))) => {
                if k == kind_name(kind) {
                    Ok(*m)
                } else {
                    Err("kindMismatch")
                }
            }
            (MappingIn::Explicit(o), _) => explicit_matrix(o, kind),
            _ => Err("kindMismatch"),
        };
        let m = match m {
            Ok(m) if m.iter().all(|v| v.is_finite()) => m.map(|v| v + 0.0),
            Ok(_) | Err(_) => {
                skipped.push(SkippedObject { object_id: local, reason: m.err().unwrap_or("invalid").into() });
                continue;
            }
        };
        if m == current {
            continue;
        }
        match tg {
            Target::Zone { layer, zone, .. } => {
                zone_fills
                    .entry(layer)
                    .or_default()
                    .insert(zone, Some(Fill { colour_id: Some(colour), matrix: Some(m) }));
                changed.push(local);
            }
            Target::Stroke { pointer, .. } => {
                if let Some(mut v) = dj.pointer(&pointer).cloned() {
                    v["fillMatrix"] = json!(m);
                    ops.push(op("replace", &pointer, Some(v)));
                    changed.push(local);
                }
            }
            _ => {}
        }
    }
    if changed.is_empty() {
        return Ok(FillMappingSetResult { changed, skipped, rev: *d.rev() });
    }
    if !ops.is_empty() {
        cx.patch_entries(vec![(EntryKey::Drawing(t.id.clone()), ops)])?;
    }
    for (layer, fills) in zone_fills {
        cx.paint_zones(&t.id, &layer, fills);
    }
    let before = doc;
    let mut ch = super::draw::Changed::default();
    crate::planar::settled(cx, &before, &t.id, &[])?.report(&mut ch);
    Ok(FillMappingSetResult { changed, skipped, rev: committed_rev(cx)? })
}

// ----- Set Colour ----------------------------------------------------------------------------------------------------

/// `colour.selection.apply` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SelectionApplyParams {
    /// The colour (default: the current colour).
    #[serde(default)]
    colour_id: Option<String>,
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Objects (default: the drawing selection).
    #[serde(default)]
    object_ids: Option<Vec<String>>,
    /// Leave objects whose current colour is protected (SI-061); `false` overrides.
    #[serde(default = "yes")]
    respect_protected: bool,
}

/// `{changed, rev}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SelectionApplyResult {
    /// Objects recoloured (zones by their ID after the change).
    changed: Vec<String>,
    /// Objects left alone and why (`colour.protected`).
    skipped: Vec<SkippedObject>,
    /// The drawing revision after the change.
    rev: u64,
}

fn recolour_text(v: &mut Value, colour: &str) -> usize {
    let mut n = 0;
    if let Some(runs) = v.get_mut("runs").and_then(Value::as_array_mut) {
        for r in runs {
            if r.get("colourId").and_then(Value::as_str) != Some(colour) {
                r["colourId"] = json!(colour);
                n += 1;
            }
        }
    }
    n
}

#[command(
    id = "colour.selection.apply",
    precheck = super::draw::precheck_target,
    kind = action,
    label = "Set colour",
    scopes = [edit],
    undo = "Set Colour",
    needs_project,
    example(title = "Recolour the pencil line and the caption with the current colour", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "objectIds": ["st_01K742SG0R61SYK9GG0HGYVNBT", "nd_01K742SG0WFAQJFKAZKV4CD56C"]}"#),
    example(title = "Recolour even where the current colour is protected", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "colourId": "col_01K742SG06BQ08FCT1TT8DF8MA", "objectIds": ["st_01K742SG0R61SYK9GG0HGYVNBT"], "respectProtected": false}"#),
    gui = "Colour view: click a swatch with drawing objects selected (M4 C4.2)",
)]
pub(crate) fn colour_selection_apply(
    cx: &mut Cx,
    p: SelectionApplyParams,
) -> Result<SelectionApplyResult, CommandError> {
    let mut none = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut none)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    check_editable(&d, None)?;
    let (colour, kind) = super::colour::resolve_colour(cx, p.colour_id.as_deref())?;
    let colour = colour
        .ok_or_else(|| err("colour.notFound", "there is no current colour", json!({ "pointer": "/colourId" })))?;
    let ids = match &p.object_ids {
        Some(v) => v.clone(),
        None => super::draw::selection_ids(cx, &d),
    };
    if ids.is_empty() {
        return Err(nothing_to_do("recolour"));
    }
    let dj = doc.entry_raw_json(&EntryKey::Drawing(t.id.clone()))?;
    let mut changed = Vec::new();
    let mut ops = Vec::new();
    let mut zone_fills: BTreeMap<String, BTreeMap<String, Option<Fill>>> = BTreeMap::new();
    let mut selected_strokes: BTreeSet<String> = BTreeSet::new();
    let mut skipped: Vec<SkippedObject> = Vec::new();
    // A target whose current colour is protected is left alone unless overridden (SI-061, as paint does).
    let refused = |current: Option<&str>| -> bool {
        p.respect_protected && current.is_some_and(|c| c != colour && protected(&doc, c))
    };
    for (i, id) in ids.iter().enumerate() {
        let (local, tg) = find(cx, &d, id, &format!("/objectIds/{i}"))?;
        let current: Vec<String> = match &tg {
            Target::Zone { fill: Some(f), .. } => f.colour_id.iter().cloned().collect(),
            Target::Stroke { colour: Some(c), .. } => vec![c.clone()],
            Target::Text { pointer } => dj
                .pointer(pointer)
                .and_then(|v| v.get("runs"))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|r| r.get("colourId").and_then(Value::as_str).map(str::to_owned))
                .collect(),
            _ => Vec::new(),
        };
        if current.iter().any(|c| refused(Some(c))) {
            if let Target::Stroke { .. } = tg {
                selected_strokes.insert(local.clone());
            }
            skipped.push(SkippedObject { object_id: local, reason: "colour.protected".into() });
            continue;
        }
        match tg {
            Target::Zone { layer, zone, fill: Some(f), bounds } => {
                let matrix = super::colour::paint_matrix(
                    kind,
                    None,
                    f.matrix,
                    Some(&bounds),
                    super::colour::tile_size(&doc, &colour),
                );
                zone_fills
                    .entry(layer)
                    .or_default()
                    .insert(zone, Some(Fill { colour_id: Some(colour.clone()), matrix }));
                changed.push(local);
            }
            Target::Zone { .. } => {}
            Target::Stroke { pointer, kind: sk, matrix, bounds, .. } if sk != StrokeKind::Invisible => {
                selected_strokes.insert(local.clone());
                if let Some(mut v) = dj.pointer(&pointer).cloned() {
                    v["colourId"] = json!(colour);
                    let m = super::colour::paint_matrix(
                        kind,
                        None,
                        matrix,
                        bounds.as_ref(),
                        super::colour::tile_size(&doc, &colour),
                    );
                    v["fillMatrix"] = json!(m);
                    ops.push(op("replace", &pointer, Some(v)));
                    changed.push(local);
                }
            }
            Target::Stroke { .. } => {
                selected_strokes.insert(local);
            }
            Target::Text { pointer } => {
                if let Some(mut v) = dj.pointer(&pointer).cloned()
                    && recolour_text(&mut v, &colour) > 0
                {
                    ops.push(op("replace", &pointer, Some(v)));
                    changed.push(local);
                }
            }
            Target::Other => {}
        }
    }
    // Painted zones inside the selection: every bounding stroke selected (03 §9.1); unpainted zones stay unpainted.
    if !selected_strokes.is_empty() {
        for lz in crate::planar::drawing_zones(&cx.planar_cache(), &d)? {
            for (z, zone) in lz.map.zones.iter().enumerate() {
                let Some(f) = lz.fills.get(z).cloned().flatten().filter(Fill::painted) else { continue };
                if zone.boundary.is_empty()
                    || !zone.boundary.iter().all(|s| selected_strokes.contains(s))
                    || changed.contains(&zone.id)
                {
                    continue;
                }
                if refused(f.colour_id.as_deref()) {
                    skipped.push(SkippedObject { object_id: zone.id.clone(), reason: "colour.protected".into() });
                    continue;
                }
                let matrix = super::colour::paint_matrix(
                    kind,
                    None,
                    f.matrix,
                    Some(&zone.bounds),
                    super::colour::tile_size(&doc, &colour),
                );
                zone_fills
                    .entry(lz.layer.id.clone())
                    .or_default()
                    .insert(zone.id.clone(), Some(Fill { colour_id: Some(colour.clone()), matrix }));
                changed.push(zone.id.clone());
            }
        }
    }
    if changed.is_empty() {
        if !skipped.is_empty() {
            let list: Vec<Value> =
                skipped.iter().map(|k| json!({ "objectId": k.object_id, "reason": k.reason })).collect();
            return Err(err(
                "colour.protected",
                "every target has a protected colour (pass respectProtected: false to override)",
                json!({ "skipped": list }),
            ));
        }
        return Ok(SelectionApplyResult { changed, skipped, rev: *d.rev() });
    }
    if !ops.is_empty() {
        cx.patch_entries(vec![(EntryKey::Drawing(t.id.clone()), ops)])?;
    }
    for (layer, fills) in zone_fills {
        cx.paint_zones(&t.id, &layer, fills);
    }
    let mut ch = super::draw::Changed::default();
    crate::planar::settled(cx, &doc, &t.id, &[])?.report(&mut ch);
    Ok(SelectionApplyResult { changed, skipped, rev: committed_rev(cx)? })
}

// ----- Replace Colour ------------------------------------------------------------------------------------------------

/// A colour replacement.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Pair {
    /// Replaced colour.
    from: String,
    /// Replacement.
    to: String,
}

/// Scope of a replacement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RecolourScope {
    /// Every drawing.
    #[default]
    Project,
    /// Drawings of the scene's elements.
    Scene,
    /// Drawings of the elements.
    Element,
    /// The drawings.
    Drawing,
}

/// Reference kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RecolourTarget {
    /// Zone fills.
    Zones,
    /// Pencil lines.
    Lines,
    /// Brush strokes.
    BrushStrokes,
    /// Text runs.
    Text,
}

fn all_targets() -> Vec<RecolourTarget> {
    vec![RecolourTarget::Zones, RecolourTarget::Lines, RecolourTarget::BrushStrokes, RecolourTarget::Text]
}

/// `colour.recolour.apply` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RecolourParams {
    /// Replaced colour (with `toColourId`).
    #[serde(default)]
    from_colour_id: Option<String>,
    /// Replacement.
    #[serde(default)]
    to_colour_id: Option<String>,
    /// Or several replacements.
    #[serde(default)]
    map: Option<Vec<Pair>>,
    /// Scope.
    #[serde(default)]
    scope: RecolourScope,
    /// The scene (scope `scene`; default: current).
    #[serde(default)]
    scene_id: Option<String>,
    /// The elements (scope `element`).
    #[serde(default)]
    element_ids: Option<Vec<String>>,
    /// The drawings (scope `drawing`).
    #[serde(default)]
    drawing_ids: Option<Vec<String>>,
    /// Reference kinds.
    #[serde(default = "all_targets")]
    targets: Vec<RecolourTarget>,
    /// Also pens using a replaced colour.
    #[serde(default)]
    include_pens: bool,
    /// Leave references to protected colours (SI-061); `false` overrides.
    #[serde(default = "yes")]
    respect_protected: bool,
}

/// References changed in one drawing.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DrawingCount {
    /// The drawing.
    drawing_id: String,
    /// References changed.
    count: u32,
}

/// `{changed, pensChanged, skippedLocked}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecolourResult {
    /// Per drawing, the references changed.
    changed: Vec<DrawingCount>,
    /// Pens recoloured.
    pens_changed: Vec<String>,
    /// Locked drawings left alone.
    skipped_locked: Vec<String>,
    /// Protected colours whose references were left alone (SI-061).
    skipped_protected: Vec<String>,
}

#[command(
    id = "colour.recolour.apply",
    precheck = crate::prechecks::colour_recolour_apply,
    kind = action,
    label = "Replace colour",
    scopes = [edit],
    undo = "Replace Colour",
    needs_project,
    example(title = "Replace Skin with Ink in every drawing", params = r#"{"fromColourId": "col_01K742SG059XGSNZ6M9MACZ2ZY", "toColourId": "col_01K742SG09H7QE4RRR6XSYYS88"}"#),
    example(title = "Replace Ink with Skin, overriding protection", params = r#"{"fromColourId": "col_01K742SG09H7QE4RRR6XSYYS88", "toColourId": "col_01K742SG059XGSNZ6M9MACZ2ZY", "respectProtected": false}"#),
    gui = "Colour view: swatch context menu > Replace uses of this colour with... (M7)",
)]
pub(crate) fn colour_recolour_apply(cx: &mut Cx, p: RecolourParams) -> Result<RecolourResult, CommandError> {
    let doc = cx.document()?.clone();
    let mut pairs: Vec<(String, String)> = Vec::new();
    match (&p.from_colour_id, &p.to_colour_id, &p.map) {
        (Some(a), Some(b), None) => pairs.push((a.clone(), b.clone())),
        (None, None, Some(m)) if !m.is_empty() => pairs.extend(m.iter().map(|q| (q.from.clone(), q.to.clone()))),
        _ => {
            return Err(err(
                "arg.invalid",
                "give fromColourId and toColourId, or map",
                json!({ "pointer": "/fromColourId" }),
            ));
        }
    }
    let mut map: BTreeMap<String, (String, SwatchKind, SwatchKind)> = BTreeMap::new();
    let mut skipped_protected: Vec<String> = Vec::new();
    for (i, (a, b)) in pairs.iter().enumerate() {
        if p.respect_protected && a != b && protected(&doc, a) {
            if !skipped_protected.contains(a) {
                skipped_protected.push(a.clone());
            }
            continue;
        }
        let ka = super::colour::swatch_info(&doc, a).map(|s| s.0);
        let kb = super::colour::swatch_info(&doc, b).map(|s| s.0);
        let (Some(ka), Some(kb)) = (ka, kb) else {
            return Err(err(
                "colour.notFound",
                format!("no colour `{a}` or `{b}`"),
                json!({ "pointer": if p.map.is_some() { format!("/map/{i}") } else { "/fromColourId".into() } }),
            ));
        };
        map.insert(a.clone(), (b.clone(), ka, kb));
    }
    // Drawings in scope.
    let mut drawings: Vec<Id> = Vec::new();
    let bank_drawings = |doc: &Document, element: &Id| -> Vec<Id> {
        doc.element(element)
            .and_then(|e| e.drawing_bank_id().clone())
            .and_then(|b| doc.bank(&b).map(|b| b.drawings().ids().to_vec()))
            .unwrap_or_default()
    };
    match p.scope {
        RecolourScope::Project => drawings.extend(doc.drawings().map(|d| d.id().clone())),
        RecolourScope::Scene => {
            let scene = match &p.scene_id {
                Some(r) => Id::parse(&cx.resolve_as(r, ObjectType::Scene)?)?,
                None => cx.app().scene.clone().ok_or_else(|| {
                    err("scene.noTarget", "there is no current scene", json!({ "pointer": "/sceneId" }))
                })?,
            };
            // The drawings exposed in the scene: those its elements' exposure runs reference (03 §9.3; C23-R1-16).
            for e in doc.elements() {
                if doc.scene_of(e.id()) == Some(&scene) {
                    drawings.extend(e.exposure().iter().flatten().filter_map(|(d, _)| d.clone()));
                }
            }
        }
        RecolourScope::Element => {
            let list = p.element_ids.clone().ok_or_else(|| {
                err("arg.invalid", "scope `element` needs elementIds", json!({ "pointer": "/elementIds" }))
            })?;
            for r in &list {
                let e = Id::parse(&cx.resolve_as(r, ObjectType::Element)?)?;
                drawings.extend(bank_drawings(&doc, &e));
            }
        }
        RecolourScope::Drawing => {
            let list = p.drawing_ids.clone().ok_or_else(|| {
                err("arg.invalid", "scope `drawing` needs drawingIds", json!({ "pointer": "/drawingIds" }))
            })?;
            for r in &list {
                drawings.push(Id::parse(&cx.resolve_as(r, ObjectType::Drawing)?)?);
            }
        }
    }
    drawings.sort();
    drawings.dedup();
    let want = |t: RecolourTarget| p.targets.contains(&t);
    // A protected colour counts as skipped only where it is used in scope by the chosen targets (C23-R1-13).
    if !skipped_protected.is_empty() {
        let mut used: BTreeSet<String> = BTreeSet::new();
        for id in &drawings {
            let Some(d) = doc.drawing(id) else { continue };
            if *d.locked() {
                continue;
            }
            for o in vd::objects(d) {
                if let Some(s) = vd::stroke_at(d, &o.pointer) {
                    let wanted = match s.kind() {
                        StrokeKind::Pencil => want(RecolourTarget::Lines),
                        StrokeKind::Brush => want(RecolourTarget::BrushStrokes),
                        StrokeKind::Invisible => false,
                    };
                    if let Some(c) = s.colour_id().as_ref().map(|c| c.as_str())
                        && wanted
                        && skipped_protected.iter().any(|x| x == c)
                    {
                        used.insert(c.to_owned());
                    }
                } else if want(RecolourTarget::Text)
                    && let Some(content) = vd::node_at(d, &o.pointer).and_then(vd::text_content)
                {
                    for r in &content.runs {
                        if let Some(c) = r.colour_id.as_deref()
                            && skipped_protected.iter().any(|x| x == c)
                        {
                            used.insert(c.to_owned());
                        }
                    }
                }
            }
            if want(RecolourTarget::Zones) {
                for lz in crate::planar::drawing_zones(&cx.planar_cache(), d)? {
                    for f in lz.fills.iter().flatten() {
                        if let Some(c) = f.colour_id.as_deref()
                            && skipped_protected.iter().any(|x| x == c)
                        {
                            used.insert(c.to_owned());
                        }
                    }
                }
            }
        }
        skipped_protected.retain(|c| used.contains(c));
    }
    // Matrix rule (03 §9.3): kept when both colours are gradients or textures, else the default for the new colour.
    let new_matrix =
        |from_k: SwatchKind, to: &str, to_k: SwatchKind, old: Option<[f64; 6]>, b: Option<&Rect>| -> Option<[f64; 6]> {
            if mappable(from_k) && mappable(to_k) {
                old
            } else if mappable(to_k) {
                b.and_then(|b| default_mapping(&doc, to, to_k, b))
            } else {
                old
            }
        };
    let mut changed = Vec::new();
    let mut skipped_locked = Vec::new();
    let mut changes: Vec<(EntryKey, Vec<tt_model::change::PatchOp>)> = Vec::new();
    let mut paints: Vec<(Id, String, BTreeMap<String, Option<Fill>>)> = Vec::new();
    for id in &drawings {
        let Some(d) = doc.drawing(id) else { continue };
        let uses = vd::objects(d).iter().any(|o| {
            vd::stroke_at(d, &o.pointer)
                .is_some_and(|s| s.colour_id().as_ref().is_some_and(|c| map.contains_key(c.as_str())))
        });
        let mut count = 0u32;
        let mut ops = Vec::new();
        let dj = doc.entry_raw_json(&EntryKey::Drawing(id.clone()))?;
        let zones =
            if want(RecolourTarget::Zones) { crate::planar::drawing_zones(&cx.planar_cache(), d)? } else { Vec::new() };
        let zone_hit = zones
            .iter()
            .any(|lz| lz.fills.iter().flatten().any(|f| f.colour_id.as_deref().is_some_and(|c| map.contains_key(c))));
        let text_hit = vd::objects(d).iter().any(|o| {
            vd::node_at(d, &o.pointer)
                .and_then(vd::text_content)
                .is_some_and(|c| c.runs.iter().any(|r| r.colour_id.as_deref().is_some_and(|x| map.contains_key(x))))
        });
        if !(uses || zone_hit || text_hit) {
            continue;
        }
        if *d.locked() {
            skipped_locked.push(id.as_str().to_owned());
            continue;
        }
        for o in vd::objects(d) {
            if let Some(s) = vd::stroke_at(d, &o.pointer) {
                let Some((to, ka, kb)) = s.colour_id().as_ref().and_then(|c| map.get(c.as_str())) else { continue };
                let ok = match s.kind() {
                    StrokeKind::Pencil => want(RecolourTarget::Lines),
                    StrokeKind::Brush => want(RecolourTarget::BrushStrokes),
                    StrokeKind::Invisible => false,
                };
                if !ok {
                    continue;
                }
                if let Some(mut v) = dj.pointer(&o.pointer).cloned() {
                    v["colourId"] = json!(to);
                    v["fillMatrix"] = json!(new_matrix(*ka, to, *kb, *s.fill_matrix(), vd::stroke_bounds(s).as_ref()));
                    ops.push(op("replace", &o.pointer, Some(v)));
                    count += 1;
                }
            } else if want(RecolourTarget::Text)
                && vd::node_at(d, &o.pointer).is_some_and(|n| *n.kind() == NodeKind::Text)
                && let Some(mut v) = dj.pointer(&o.pointer).cloned()
            {
                let mut n = 0;
                if let Some(runs) = v.get_mut("runs").and_then(Value::as_array_mut) {
                    for r in runs {
                        if let Some((to, _, _)) = r.get("colourId").and_then(Value::as_str).and_then(|c| map.get(c)) {
                            r["colourId"] = json!(to);
                            n += 1;
                        }
                    }
                }
                if n > 0 {
                    ops.push(op("replace", &o.pointer, Some(v)));
                    count += n;
                }
            }
        }
        for lz in &zones {
            let mut fills = BTreeMap::new();
            for (z, zone) in lz.map.zones.iter().enumerate() {
                let Some(f) = lz.fills.get(z).cloned().flatten() else { continue };
                let Some((to, ka, kb)) = f.colour_id.as_deref().and_then(|c| map.get(c)) else { continue };
                fills.insert(
                    zone.id.clone(),
                    Some(Fill {
                        colour_id: Some(to.clone()),
                        matrix: new_matrix(*ka, to, *kb, f.matrix, Some(&zone.bounds)),
                    }),
                );
                count += 1;
            }
            if !fills.is_empty() {
                paints.push((id.clone(), lz.layer.id.clone(), fills));
            }
        }
        if !ops.is_empty() {
            changes.push((EntryKey::Drawing(id.clone()), ops));
        }
        if count > 0 {
            changed.push(DrawingCount { drawing_id: id.as_str().to_owned(), count });
        }
    }
    let mut pens_changed = Vec::new();
    if p.include_pens {
        let project = doc.entry_raw_json(&EntryKey::Project)?;
        let mut ops = Vec::new();
        for (i, pen) in project.get("pens").and_then(Value::as_array).into_iter().flatten().enumerate() {
            if let Some((to, _, _)) = pen.get("colourId").and_then(Value::as_str).and_then(|c| map.get(c)) {
                ops.push(op("replace", &format!("/pens/{i}/colourId"), Some(json!(to))));
                pens_changed.push(pen.get("id").and_then(Value::as_str).unwrap_or_default().to_owned());
            }
        }
        if !ops.is_empty() {
            changes.push((EntryKey::Project, ops));
        }
    }
    if changes.is_empty() && paints.is_empty() {
        if !skipped_protected.is_empty() {
            return Err(err(
                "colour.protected",
                "every replaced colour is protected (pass respectProtected: false to override)",
                json!({ "colourIds": skipped_protected }),
            ));
        }
        return Ok(RecolourResult { changed, pens_changed, skipped_locked, skipped_protected });
    }
    if !changes.is_empty() {
        cx.patch_entries(changes)?;
    }
    for (d, layer, fills) in paints {
        cx.paint_zones(&d, &layer, fills);
    }
    cx.settle()?;
    Ok(RecolourResult { changed, pens_changed, skipped_locked, skipped_protected })
}
