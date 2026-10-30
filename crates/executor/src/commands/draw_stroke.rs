//! `draw.stroke.add`, `draw.stroke.set` and `draw.content.add` (02 §13.5): brush strokes, pencil lines and
//! invisible strokes built by `tt-vector` (fitting §3.3, outlines §3.4, symmetry §6.2), and drawing content inserted
//! in the §13.4 shape.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_model::ObjectType;
use tt_model::document::{Document, EntryKey};
use tt_model::entities::drawing::{Drawing, LayerHint, StrokeKind};
use tt_model::entities::pen::{PenKind, Tip};
use tt_registry::{CommandError, codes, command};
use tt_vector::brush::BrushPen;
use tt_vector::drawing::{self as vd, LayerPolicy, LayerTarget, ObjectKind};
use tt_vector::fit::Sample;
use tt_vector::geom::P;
use tt_vector::path::Path;
use tt_vector::stroke::{Geometry, Spec};
use tt_vector::symmetry::Symmetry;

use super::draw::{Point, check_editable, committed_rev, err, invalid_geometry, locate_all, op, target, warn_zones};
use crate::cx::Cx;

/// Stroke kind parameter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum KindParam {
    /// Filled brush shape.
    #[default]
    Brush,
    /// Centre line with a line style.
    Pencil,
    /// Never rendered; closes and splits zones.
    Invisible,
}

/// A sample `{x, y, pressure?}`.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PointIn {
    /// X (du).
    x: f64,
    /// Y (du).
    y: f64,
    /// Pressure 0–1 (default 1: a mouse, 02 §3.4).
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 1.0))]
    pressure: Option<f64>,
}

/// A pressure sample `{t, p}` (02 §3.2).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PressureIn {
    /// Normalised arc-length position 0–1.
    #[schemars(range(min = 0.0, max = 1.0))]
    t: f64,
    /// Pressure 0–1.
    #[schemars(range(min = 0.0, max = 1.0))]
    p: f64,
}

/// Tip shape parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TipParam {
    /// Round nib.
    Round,
    /// Oval nib.
    Oval,
    /// Square nib.
    Square,
    /// Flat nib.
    Flat,
}

impl From<TipParam> for Tip {
    fn from(t: TipParam) -> Self {
        match t {
            TipParam::Round => Tip::Round,
            TipParam::Oval => Tip::Oval,
            TipParam::Square => Tip::Square,
            TipParam::Flat => Tip::Flat,
        }
    }
}

/// An explicit pen `{tipShape, minSize, maxSize, tipAngle}` (02 §13.4 `pen`).
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PenIn {
    /// Tip shape.
    tip_shape: TipParam,
    /// Width at lowest pressure (du).
    #[schemars(range(min = 0.0, max = 100.0))]
    min_size: f64,
    /// Width at full pressure (du).
    #[schemars(range(min = 0.0, max = 100.0))]
    max_size: f64,
    /// Orientation of non-round tips (degrees).
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 360.0))]
    tip_angle: f64,
}

/// `draw.stroke.add` parameters (02 §13.5).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct StrokeAddParams {
    /// The drawing (default: `elementId` + `frame`, else the current drawing).
    #[serde(default)]
    drawing_id: Option<String>,
    /// The element whose cell receives the stroke.
    #[serde(default)]
    element_id: Option<String>,
    /// The frame of that cell (default: current frame).
    #[serde(default)]
    #[schemars(range(min = 1, max = 2_147_483_647))]
    frame: Option<u32>,
    /// Create a drawing in an empty addressed cell (same undo step, 02 §1.1).
    #[serde(default = "yes")]
    create_drawing: bool,
    /// Stroke kind.
    #[serde(default)]
    kind: KindParam,
    /// Samples (curve-fitted when `fit`).
    #[serde(default)]
    #[schemars(length(min = 1, max = 100_000))]
    points: Option<Vec<PointIn>>,
    /// Bézier control points used verbatim (02 §13.4 Bézier list).
    #[serde(default)]
    #[schemars(length(min = 1, max = 300_001))]
    bezier: Option<Vec<Point>>,
    /// Closed path.
    #[serde(default)]
    closed: bool,
    /// Pressure map `[{t, p}]` (overrides per-point pressure).
    #[serde(default)]
    #[schemars(length(min = 1, max = 100_000))]
    pressure: Option<Vec<PressureIn>>,
    /// Fit Béziers to `points` (§3.3); `false` joins them with straight segments.
    #[serde(default = "yes")]
    fit: bool,
    /// Smoothness 0–10 (default: the pen's).
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 10.0))]
    smoothness: Option<f64>,
    /// The brush pen (default: the current brush pen).
    #[serde(default)]
    pen_id: Option<String>,
    /// An explicit pen (overrides the pen's shape and sizes).
    #[serde(default)]
    pen: Option<PenIn>,
    /// Colour (default: the pen's colour, else the current colour).
    #[serde(default)]
    colour_id: Option<String>,
    /// Pencil thickness 0–100 du (default: the pen's maximum size).
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 100.0))]
    thickness: Option<f64>,
    /// `auto`, `new`, `top`, `behind` or a layer ID.
    #[serde(default = "auto")]
    layer: String,
    /// Apply the current symmetry setting (02 §6.2).
    #[serde(default = "yes")]
    symmetry: bool,
    /// The drawing revision the caller read (02 §3.12; `drawing.staleRevision` when it changed).
    #[serde(default)]
    expect_rev: Option<u64>,
}

fn yes() -> bool {
    true
}

fn auto() -> String {
    "auto".to_owned()
}

/// `draw.stroke.add` result (02 §13.5).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StrokeAddResult {
    /// The drawing.
    drawing_id: String,
    /// A drawing was created in the empty cell.
    created_drawing: bool,
    /// The new strokes (symmetry copies included), back to front.
    stroke_ids: Vec<String>,
    /// Zones created or changed (planar map, M2 C2.2: empty until then).
    zone_ids: Vec<String>,
    /// The layer holding the strokes.
    layer_id: String,
    /// Strokes removed by re-noding (M2 C2.2: empty until then).
    removed_ids: Vec<String>,
    /// The drawing revision after the change.
    rev: u64,
    /// Hint codes (`drawing.strokesHidden` for invisible strokes while Show Strokes is off).
    hints: Vec<String>,
}

/// The pen a stroke uses: `penId`, else the current brush pen (the project's "Pen 3", else its first brush pen; the
/// current-pen selection of 02 §6.1 is `tool.pen.select`, M4 C4.2).
struct PenChoice {
    pen: BrushPen,
    pen_id: Option<String>,
    smoothness: f64,
    colour: Option<String>,
}

fn pen_choice(cx: &Cx, pen_id: Option<&str>, explicit: Option<PenIn>) -> Result<PenChoice, CommandError> {
    let doc = cx.document()?;
    let pens = doc.project().pens();
    let chosen = match pen_id {
        Some(r) => {
            let key = cx.resolve_as(r, ObjectType::Pen)?;
            let p = pens
                .iter()
                .find(|p| p.id().as_str() == key)
                .ok_or_else(|| err("pen.notFound", format!("no pen `{r}`"), json!({ "penId": r })))?;
            if *p.kind() != PenKind::Brush {
                return Err(err(
                    codes::ARG_INVALID,
                    format!("pen `{}` is an eraser pen", p.name()),
                    json!({ "pointer": "/penId" }),
                ));
            }
            Some(p)
        }
        None => pens
            .iter()
            .find(|p| *p.kind() == PenKind::Brush && p.name() == "Pen 3")
            .or_else(|| pens.iter().find(|p| *p.kind() == PenKind::Brush)),
    };
    let mut c = match chosen {
        Some(p) => PenChoice {
            pen: BrushPen {
                tip: *p.tip(),
                min_size: *p.min_size(),
                max_size: *p.max_size(),
                tip_angle: *p.tip_angle(),
            },
            pen_id: Some(p.id().as_str().to_owned()),
            smoothness: f64::from(*p.smoothness()),
            colour: p.colour_id().as_ref().map(|c| c.as_str().to_owned()),
        },
        None => PenChoice {
            pen: BrushPen { tip: Tip::Round, min_size: 2.0, max_size: 8.0, tip_angle: 0.0 },
            pen_id: None,
            smoothness: 3.0,
            colour: None,
        },
    };
    if let Some(e) = explicit {
        if e.min_size > e.max_size {
            return Err(err(
                "tool.valueOutOfRange",
                "pen minSize must not exceed maxSize",
                json!({ "pointer": "/pen/minSize" }),
            ));
        }
        c.pen =
            BrushPen { tip: e.tip_shape.into(), min_size: e.min_size, max_size: e.max_size, tip_angle: e.tip_angle };
        if pen_id.is_none() {
            c.pen_id = None;
        }
    }
    Ok(c)
}

/// Whether a colour ID is defined by a palette swatch.
pub(crate) fn colour_exists(doc: &Document, id: &str) -> bool {
    doc.palettes().any(|p| p.swatches().iter().any(|s| s.colour_id().as_str() == id))
}

/// The current colour (03 §2.4): the session has no `colour.current.set` before M7 C7.1, so it is the first swatch
/// of the first visible palette.
pub(crate) fn current_colour(doc: &Document) -> Option<String> {
    doc.palettes()
        .filter(|p| !*p.hidden())
        .find_map(|p| p.swatches().first().map(|s| s.colour_id().as_str().to_owned()))
}

/// Resolves a given colour (must exist: `colour.notFound`) or the default.
pub(crate) fn colour_of(
    cx: &mut Cx,
    given: Option<&str>,
    pen: Option<&str>,
    pointer: &str,
) -> Result<Option<String>, CommandError> {
    let doc = cx.document()?.clone();
    match given {
        Some(c) => {
            if !colour_exists(&doc, c) {
                return Err(err(
                    "colour.notFound",
                    format!("no swatch has colour ID `{c}`"),
                    json!({ "colourId": c, "pointer": pointer }),
                )
                .hint("use a colour ID from colour.swatch.list or draw.drawing.get"));
            }
            Ok(Some(c.to_owned()))
        }
        None => {
            if let Some(p) = pen.filter(|p| colour_exists(&doc, p)) {
                return Ok(Some(p.to_owned()));
            }
            let c = current_colour(&doc);
            if c.is_none() {
                cx.warn("colour.noCurrentColour", "there is no current colour; the stroke is unpainted", Vec::new());
            }
            Ok(c)
        }
    }
}

/// Symmetry and Draw Behind settings (02 §6.2, §8.11): user settings whose commands (`tool.symmetry.set`,
/// `tool.placeBehind.set`) arrive with the tools in M4 C4.2; until then both are at their defaults (none, off).
fn symmetry_setting(_cx: &Cx) -> (Symmetry, bool) {
    (Symmetry::None, false)
}

fn layer_policy(s: &str) -> LayerPolicy {
    match s {
        "auto" => LayerPolicy::Auto,
        "new" => LayerPolicy::New,
        "top" => LayerPolicy::Top,
        "behind" => LayerPolicy::Behind,
        id => LayerPolicy::Id(id.to_owned()),
    }
}

fn kind_of(k: KindParam) -> StrokeKind {
    match k {
        KindParam::Brush => StrokeKind::Brush,
        KindParam::Pencil => StrokeKind::Pencil,
        KindParam::Invisible => StrokeKind::Invisible,
    }
}

fn label_of(k: StrokeKind) -> &'static str {
    match k {
        StrokeKind::Brush => "Brush Tool",
        StrokeKind::Pencil => "Pencil Tool",
        StrokeKind::Invisible => "Stroke Tool",
    }
}

fn geometry(
    points: Option<&[PointIn]>,
    bezier: Option<&[Point]>,
    fit: bool,
    closed: bool,
) -> Result<Geometry, CommandError> {
    match (points, bezier) {
        (Some(_), Some(_)) => {
            Err(err(codes::ARG_INVALID, "give points or bezier, not both", json!({ "pointer": "/bezier" })))
        }
        (None, None) => Err(err(codes::ARG_MISSING, "give points or bezier", json!({ "pointer": "/points" }))),
        (Some(pts), None) => {
            let raw: Vec<Sample> =
                pts.iter().map(|p| Sample { p: P::new(p.x, p.y), pressure: p.pressure.unwrap_or(1.0) }).collect();
            let samples = tt_vector::fit::clean(&raw, "/points").map_err(|e| invalid_geometry(&e))?;
            Ok(Geometry::Samples { samples, fit })
        }
        (None, Some(b)) => {
            let pts: Vec<P> = b.iter().map(|p| P::new(p.x, p.y)).collect();
            Ok(Geometry::Bezier(Path::from_bezier_list(&pts, closed, "/bezier").map_err(|e| invalid_geometry(&e))?))
        }
    }
}

fn transform_geometry(g: &Geometry, m: &tt_vector::geom::Affine) -> Geometry {
    match g {
        Geometry::Samples { samples, fit } => Geometry::Samples {
            samples: samples.iter().map(|s| Sample { p: m.apply(s.p), pressure: s.pressure }).collect(),
            fit: *fit,
        },
        Geometry::Bezier(p) => Geometry::Bezier(p.transformed(m)),
    }
}

#[command(
    id = "draw.stroke.add",
    kind = action,
    label = "Add stroke",
    scopes = [edit],
    undo = "Brush Tool",
    needs_project,
    example(title = "A pressure brush stroke in Bob's drawing at frame 1", params = r#"{"elementId": "scene:#1/element:\"Bob\"", "frame": 1, "kind": "brush", "points": [{"x": 0, "y": 0, "pressure": 0.2}, {"x": 20, "y": 8, "pressure": 0.6}, {"x": 40, "y": 10, "pressure": 1}, {"x": 60, "y": 4, "pressure": 0.5}]}"#),
    example(title = "A pencil line given as Béziers, into a new layer", params = r#"{"elementId": "scene:#1/element:\"Bob\"", "frame": 1, "kind": "pencil", "bezier": [{"x": -50, "y": 0}, {"x": -20, "y": 30}, {"x": 20, "y": 30}, {"x": 50, "y": 0}], "thickness": 3, "layer": "new"}"#),
    example(title = "An invisible stroke closing a gap", params = r#"{"elementId": "scene:#1/element:\"Bob\"", "frame": 1, "kind": "invisible", "points": [{"x": 0, "y": 0}, {"x": 0, "y": 100}], "fit": false}"#),
    gui = "Drawing view: Brush, Pencil and Stroke tools (M4 C4.2)",
)]
pub(crate) fn draw_stroke_add(cx: &mut Cx, p: StrokeAddParams) -> Result<StrokeAddResult, CommandError> {
    let kind = kind_of(p.kind);
    let geom = geometry(p.points.as_deref(), p.bezier.as_deref(), p.fit, p.closed)?;
    let mut changes = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), p.element_id.as_deref(), p.frame, p.create_drawing, &mut changes)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?;
    check_editable(&d, if t.created { None } else { p.expect_rev })?;
    let pc = pen_choice(cx, p.pen_id.as_deref(), p.pen)?;
    let colour = if kind == StrokeKind::Invisible {
        None
    } else {
        colour_of(cx, p.colour_id.as_deref(), pc.colour.as_deref(), "/colourId")?
    };
    let (symmetry, place_behind) = symmetry_setting(cx);
    let maps = if p.symmetry { tt_vector::symmetry::maps(symmetry) } else { vec![tt_vector::geom::Affine::IDENTITY] };
    let pressure = p.pressure.as_ref().map(|v| v.iter().map(|x| [x.t, x.p]).collect::<Vec<_>>());
    let mut strokes: Vec<Value> = Vec::new();
    let mut ids: Vec<String> = Vec::new();
    let mut first_poly: Vec<P> = Vec::new();
    for (i, m) in maps.iter().enumerate() {
        // A reflected copy mirrors the tip angle too.
        let mut pen = pc.pen;
        if m.det() < 0.0 {
            pen.tip_angle = (360.0 - pen.tip_angle).rem_euclid(360.0);
        }
        let spec = Spec {
            kind,
            geometry: transform_geometry(&geom, m),
            closed: p.closed,
            pressure: pressure.clone(),
            smoothness: p.smoothness.unwrap_or(pc.smoothness),
            pen,
            pen_id: pc.pen_id.clone(),
            colour_id: colour.clone(),
            thickness: p.thickness.unwrap_or(pc.pen.max_size),
        };
        let id = cx.new_id(ObjectType::Stroke)?;
        let v = tt_vector::stroke::build(&spec, id.as_str()).map_err(|e| invalid_geometry(&e))?;
        if i == 0 {
            let path = tt_vector::stroke::centre(&spec).map_err(|e| invalid_geometry(&e))?.0;
            first_poly = path.flatten(0.05);
        }
        ids.push(id.as_str().to_owned());
        strokes.push(v);
    }
    let policy = layer_policy(&p.layer);
    let lt = vd::layer_target(&d, &policy, kind, place_behind, &first_poly)
        .map_err(|(code, m)| err(code, m, json!({ "pointer": "/layer", "layer": p.layer })))?;
    let hint = if kind == StrokeKind::Brush { LayerHint::Brush } else { LayerHint::Pencil };
    let (layer_id, mut ops) = match lt {
        LayerTarget::Existing { id, strokes: at } => {
            let ops: Vec<_> = strokes.into_iter().map(|s| op("add", &format!("{at}/-"), Some(s))).collect();
            (id, ops)
        }
        LayerTarget::NewTop | LayerTarget::NewBehind => {
            let lid = cx.new_id(ObjectType::DrawingNode)?;
            let mut layer = vd::layer_json(lid.as_str(), hint);
            layer["strokes"] = Value::Array(strokes);
            let at = if lt == LayerTarget::NewBehind { "/nodes/0".to_owned() } else { "/nodes/-".to_owned() };
            (lid.as_str().to_owned(), vec![op("add", &at, Some(layer))])
        }
    };
    if t.created {
        // The new drawing's entry is created by an earlier operation of this commit: add the layer to it in place.
        if let Some((_, first)) = changes.iter_mut().find(|(k, _)| *k == EntryKey::Drawing(t.id.clone()))
            && let Some(o) = first.first_mut()
            && let Some(v) = o.value.as_mut()
        {
            for o2 in ops.drain(..) {
                if let (Some(nodes), Some(val)) = (v.get_mut("nodes").and_then(Value::as_array_mut), o2.value) {
                    if o2.path == "/nodes/0" { nodes.insert(0, val) } else { nodes.push(val) }
                }
            }
        }
    } else {
        changes.push((EntryKey::Drawing(t.id.clone()), ops));
    }
    cx.patch_entries(changes)?;
    cx.set_label(label_of(kind));
    warn_zones(cx, &t.id);
    let hints = if kind == StrokeKind::Invisible { vec!["drawing.strokesHidden".to_owned()] } else { Vec::new() };
    Ok(StrokeAddResult {
        drawing_id: t.id.as_str().to_owned(),
        created_drawing: t.created,
        stroke_ids: ids,
        zone_ids: Vec::new(),
        layer_id,
        removed_ids: Vec::new(),
        rev: committed_rev(cx)?,
        hints,
    })
}

// ----- draw.stroke.set ---------------------------------------------------------------------------------------------

/// Kind conversion of `draw.stroke.set`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum LineKind {
    /// A visible centre line.
    Pencil,
    /// An invisible stroke (an unpainted line, 02 §7.3.2).
    Invisible,
}

/// `draw.stroke.set` parameters (02 §13.5).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct StrokeSetParams {
    /// The stroke (`<drawingId>/<localId>`, or local with `drawingId` / the current drawing).
    stroke_id: String,
    /// The drawing holding a local stroke ID (default: the current drawing).
    #[serde(default)]
    drawing_id: Option<String>,
    /// New geometry (02 §13.4 Bézier list): a brush stroke's spine (the outline is swept again), or its outline when
    /// the spine was discarded; a pencil or invisible stroke's path.
    #[serde(default)]
    #[schemars(length(min = 1, max = 300_001))]
    bezier: Option<Vec<Point>>,
    /// Closed path (with `bezier`).
    #[serde(default)]
    closed: Option<bool>,
    /// New pressure map of a brush stroke's spine.
    #[serde(default)]
    #[schemars(length(min = 1, max = 100_000))]
    pressure: Option<Vec<PressureIn>>,
    /// New line thickness of a pencil line (du).
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 100.0))]
    thickness: Option<f64>,
    /// Make a pencil line invisible or an invisible stroke a pencil line.
    #[serde(default)]
    kind: Option<LineKind>,
    /// The drawing revision the caller read (02 §3.12; `drawing.staleRevision` when it changed).
    #[serde(default)]
    expect_rev: Option<u64>,
}

/// `{strokeIds, removedIds, rev}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StrokeSetResult {
    /// The edited stroke (its ID is kept, 02 §3.12 item 4).
    stroke_ids: Vec<String>,
    /// Strokes removed by re-noding (M2 C2.2: empty until then).
    removed_ids: Vec<String>,
    /// The drawing revision after the change.
    rev: u64,
}

/// The document, drawing and object a `strokeId` (or text ID) names: `<drawingId>/<localId>`, or a local ID in
/// `drawingId` or the current drawing. The document is the transaction's (a cheap copy-on-write clone).
pub(crate) fn stroke_target(
    cx: &Cx,
    stroke_id: &str,
    drawing_id: Option<&str>,
) -> Result<(Document, tt_model::Id, vd::Located), CommandError> {
    let doc = cx.document()?.clone();
    let drawing = match (stroke_id.split_once('/'), drawing_id) {
        (Some((d, _)), _) => Some(tt_model::Id::parse(d)?),
        (None, Some(d)) => Some(tt_model::Id::parse(&cx.resolve_as(d, ObjectType::Drawing)?)?),
        (None, None) => cx.app().current_drawing(&doc),
    };
    let Some(did) = drawing else {
        return Err(err(
            "drawing.noTarget",
            "a local stroke ID needs drawingId or a current drawing",
            json!({ "strokeId": stroke_id }),
        ));
    };
    let d = doc.drawing(&did).ok_or_else(|| super::draw::not_found(did.as_str()))?;
    let found = locate_all(d, &[stroke_id.to_owned()], "/strokeId")?;
    let l = found.into_iter().next().ok_or_else(|| CommandError::internal("located nothing"))?;
    Ok((doc.clone(), did, l))
}

/// The drawing `id` of a document from [`stroke_target`].
pub(crate) fn drawing_in<'a>(doc: &'a Document, id: &tt_model::Id) -> Result<&'a Drawing, CommandError> {
    doc.drawing(id).ok_or_else(|| super::draw::not_found(id.as_str()))
}

#[command(
    id = "draw.stroke.set",
    kind = action,
    label = "Edit stroke",
    scopes = [edit],
    undo = "Edit Stroke",
    needs_project,
    example(title = "Make the pencil line of the first drawing thicker", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "strokeId": "st_01K742SG0R61SYK9GG0HGYVNBT", "thickness": 4}"#),
    gui = "Drawing view: Contour Editor and line properties (M4 C4.2)",
)]
pub(crate) fn draw_stroke_set(cx: &mut Cx, p: StrokeSetParams) -> Result<StrokeSetResult, CommandError> {
    if p.bezier.is_none() && p.pressure.is_none() && p.thickness.is_none() && p.kind.is_none() && p.closed.is_none() {
        return Err(err(
            codes::ARG_MISSING,
            "give bezier, closed, pressure, thickness or kind",
            json!({ "pointer": "/bezier" }),
        ));
    }
    let (doc, did, l) = stroke_target(cx, &p.stroke_id, p.drawing_id.as_deref())?;
    let d = drawing_in(&doc, &did)?;
    if l.kind != ObjectKind::Stroke {
        return Err(err(
            codes::ARG_INVALID,
            format!("`{}` is a {}, not a stroke", p.stroke_id, l.kind.as_str()),
            json!({ "pointer": "/strokeId" }),
        ));
    }
    check_editable(d, p.expect_rev)?;
    let s = vd::stroke_at(d, &l.pointer).ok_or_else(|| CommandError::internal("stroke vanished"))?;
    let cur = tt_model::canonical::to_value(s)?;
    let next = tt_vector::stroke::edit(
        s,
        &cur,
        &tt_vector::stroke::Edit {
            bezier: p.bezier.as_ref().map(|b| b.iter().map(|q| P::new(q.x, q.y)).collect()),
            closed: p.closed,
            pressure: p.pressure.as_ref().map(|v| v.iter().map(|x| [x.t, x.p]).collect()),
            thickness: p.thickness,
            to_invisible: p.kind.map(|k| k == LineKind::Invisible),
            default_thickness: pen_choice(cx, None, None)?.pen.max_size,
        },
    )
    .map_err(|e| match e {
        tt_vector::stroke::EditError::Geometry(g) => invalid_geometry(&g),
        tt_vector::stroke::EditError::NotApplicable(pointer, m) => {
            err(codes::ARG_INVALID, m, json!({ "pointer": pointer }))
        }
    })?;
    // An invisible stroke made visible takes the current colour (03 §2.4).
    let colour = if p.kind == Some(LineKind::Pencil) && *s.kind() == StrokeKind::Invisible {
        Some(current_colour(cx.document()?))
    } else {
        None
    };
    let mut next = next;
    if let Some(c) = colour
        && let Some(m) = next.as_object_mut()
    {
        m.insert("colourId".into(), json!(c));
    }
    cx.patch_entries(vec![(EntryKey::Drawing(d.id().clone()), vec![op("replace", &l.pointer, Some(next))])])?;
    warn_zones(cx, d.id());
    Ok(StrokeSetResult { stroke_ids: vec![l.id], removed_ids: Vec::new(), rev: committed_rev(cx)? })
}

// ----- draw.content.add --------------------------------------------------------------------------------------------

/// Where inserted content goes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ContentLayer {
    /// On top of the drawing.
    #[default]
    Top,
    /// Behind all artwork.
    Behind,
}

/// `draw.content.add` parameters (02 §13.5).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ContentAddParams {
    /// The drawing (default: `elementId` + `frame`, else the current drawing).
    #[serde(default)]
    drawing_id: Option<String>,
    /// The element whose cell receives the content.
    #[serde(default)]
    element_id: Option<String>,
    /// The frame of that cell.
    #[serde(default)]
    #[schemars(range(min = 1, max = 2_147_483_647))]
    frame: Option<u32>,
    /// Create a drawing in an empty addressed cell.
    #[serde(default = "yes")]
    create_drawing: bool,
    /// Drawing content in the 02 §13.4 shape: `{nodes: [...]}`, a `draw.drawing.get` result as returned.
    content: tt_vector::content::ContentIn,
    /// Offset added to all geometry (du).
    #[serde(default)]
    offset: Option<Point>,
    /// Insert on top or behind.
    #[serde(default)]
    layer: ContentLayer,
    /// Make the inserted objects the drawing selection.
    #[serde(default = "yes")]
    select: bool,
}

/// `{idMap, rev}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContentAddResult {
    /// The drawing.
    drawing_id: String,
    /// A drawing was created in the empty cell.
    created_drawing: bool,
    /// Old ID → new ID of every inserted node and stroke (SI-026 `idMap`).
    id_map: std::collections::BTreeMap<String, String>,
    /// The drawing revision after the change.
    rev: u64,
}

#[command(
    id = "draw.content.add",
    kind = action,
    label = "Insert drawing content",
    scopes = [edit],
    undo = "Paste",
    needs_project,
    example(title = "Insert a pencil line and a text object", params = r#"{"elementId": "scene:#1/element:\"Bob\"", "frame": 1, "content": {"nodes": [{"type": "layer", "id": "l_1", "layerKind": "pencil", "strokes": [{"id": "s_1", "kind": "pencil", "closed": false, "bezier": [{"x": 0, "y": 0}, {"x": 10, "y": 5}, {"x": 20, "y": 5}, {"x": 30, "y": 0}], "colourId": null, "lineStyle": {"thickness": 2, "lineType": "solid"}}]}, {"type": "text", "id": "t_1", "text": "Hi", "transform": [1, 0, 0, 1, -50, 120], "widthMode": "auto", "width": null, "runs": [{"start": 0, "length": 2, "font": "sans-serif", "size": 36, "bold": false, "italic": false, "baseline": "normal", "kerning": 0, "autoKern": true, "colourId": null, "outlineColourId": null}], "paragraphs": [{"start": 0, "length": 2, "align": "left", "leftMargin": 0, "rightMargin": 0, "indent": 0, "lineSpacing": 0}]}]}, "offset": {"x": 10, "y": -10}}"#),
    gui = "Drawing view: Edit › Paste (M4 C4.2)",
)]
pub(crate) fn draw_content_add(cx: &mut Cx, p: ContentAddParams) -> Result<ContentAddResult, CommandError> {
    let mut changes = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), p.element_id.as_deref(), p.frame, p.create_drawing, &mut changes)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?;
    check_editable(&d, None)?;
    let doc = cx.document()?.clone();
    let offset = p.offset.map_or(P::default(), |o| P::new(o.x, o.y));
    let images: Vec<tt_vector::content::KnownImage> = doc
        .drawings()
        .flat_map(|dr| {
            vd::objects(dr).into_iter().filter_map(move |o| {
                let n = vd::node_at(dr, &o.pointer)?;
                Some(tt_vector::content::KnownImage {
                    blob: n.image().clone()?,
                    width: (*n.width())?,
                    height: (*n.height())?,
                })
            })
        })
        .collect();
    let pens: Vec<String> = doc.project().pens().iter().map(|p| p.id().as_str().to_owned()).collect();
    let fresh = |ty: ObjectType| cx.new_id(ty).map(|i| i.as_str().to_owned());
    let built = tt_vector::content::build(&p.content, offset, &images, &pens, fresh).map_err(|e| match e {
        tt_vector::content::ContentError::Geometry(g) => invalid_geometry(&g),
        tt_vector::content::ContentError::Invalid(pointer, m) => {
            err(codes::ARG_INVALID, m, json!({ "pointer": format!("/content{pointer}") }))
        }
        tt_vector::content::ContentError::Id(e) => e,
    })?;
    // Colour IDs are kept (ID-05); unknown ones are reported until colour recovery (M2 C2.2).
    for c in &built.colours {
        if !colour_exists(&doc, c) {
            cx.warn(
                "colour.unresolved",
                format!("colour `{c}` is not in any palette (recovered with M2 C2.2)"),
                vec![c.clone()],
            );
        }
    }
    let top: Vec<String> =
        built.nodes.iter().filter_map(|n| n.get("id").and_then(Value::as_str).map(str::to_owned)).collect();
    if t.created {
        if let Some((_, first)) = changes.iter_mut().find(|(k, _)| *k == EntryKey::Drawing(t.id.clone()))
            && let Some(o) = first.first_mut()
            && let Some(v) = o.value.as_mut()
            && let Some(nodes) = v.get_mut("nodes").and_then(Value::as_array_mut)
        {
            nodes.extend(built.nodes);
        }
    } else {
        let ops: Vec<_> = match p.layer {
            ContentLayer::Top => built.nodes.into_iter().map(|n| op("add", "/nodes/-", Some(n))).collect(),
            ContentLayer::Behind => {
                built.nodes.into_iter().enumerate().map(|(i, n)| op("add", &format!("/nodes/{i}"), Some(n))).collect()
            }
        };
        changes.push((EntryKey::Drawing(t.id.clone()), ops));
    }
    cx.patch_entries(changes)?;
    if p.select {
        let keys: Vec<String> = top.iter().map(|i| format!("{}/{i}", t.id.as_str())).collect();
        cx.app_mut().set_selection("drawing", keys);
    }
    warn_zones(cx, &t.id);
    Ok(ContentAddResult {
        drawing_id: t.id.as_str().to_owned(),
        created_drawing: t.created,
        id_map: built.id_map,
        rev: committed_rev(cx)?,
    })
}

/// Normalises strokes changed by a generic patch (`doc.object.patch`, 02 §13.6), against their state in `before`:
/// geometry is checked (`drawing.invalidGeometry`); a brush stroke whose spine changed while its outline did not gets
/// its outline swept again; one whose outline changed while its spine did not loses the spine (02 §3.2). The
/// corrections are staged in the same transaction.
///
/// # Errors
/// `drawing.invalidGeometry`; staging errors.
pub(crate) fn normalise_patched_strokes(cx: &mut Cx, before: &Document, keys: &[String]) -> Result<(), CommandError> {
    let mut fixes: Vec<(String, Vec<tt_model::change::PatchOp>)> = Vec::new();
    for key in keys {
        let Some((did, local)) = key.split_once('/') else { continue };
        let did = tt_model::Id::parse(did)?;
        let doc = cx.document()?;
        let (Some(old_d), Some(new_d)) = (before.drawing(&did), doc.drawing(&did)) else { continue };
        let (Some(old_l), Some(new_l)) = (vd::locate(old_d, local), vd::locate(new_d, local)) else { continue };
        let (Some(old), Some(new)) = (vd::stroke_at(old_d, &old_l.pointer), vd::stroke_at(new_d, &new_l.pointer))
        else {
            continue;
        };
        let outline = Path::from_points(new.points(), *new.closed());
        outline.check("/points").map_err(|e| invalid_geometry(&e))?;
        if let Some(sp) = new.spine() {
            Path::from_points(&sp.points, sp.points.len() == 1)
                .check("/spine/points")
                .map_err(|e| invalid_geometry(&e))?;
        }
        if *new.kind() != StrokeKind::Brush {
            continue;
        }
        let spine_changed =
            old.spine() != new.spine() || tt_vector::stroke::tip_angle(old) != tt_vector::stroke::tip_angle(new);
        let outline_changed = old.points() != new.points() || old.closed() != new.closed();
        match (new.spine(), spine_changed, outline_changed) {
            (Some(sp), true, false) => {
                let pen =
                    tt_vector::stroke::spine_pen(new).ok_or_else(|| CommandError::internal("spine without pen"))?;
                let spine = Path::from_points(&sp.points, sp.points.len() == 1);
                let swept =
                    tt_vector::stroke::rederive(&spine, &sp.pressure, &pen).map_err(|e| invalid_geometry(&e))?;
                fixes.push((key.clone(), vec![op("replace", "/points", Some(swept.points_json()))]));
            }
            (Some(_), false, true) => fixes.push((key.clone(), vec![op("replace", "/spine", Some(Value::Null))])),
            _ => {}
        }
    }
    if !fixes.is_empty() {
        cx.patch_objects(fixes)?;
    }
    Ok(())
}
