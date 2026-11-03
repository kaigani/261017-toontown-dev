//! Engine view over a `tt-model` drawing: object lookup through groups, layer-insertion policies (02 §3.1, §7.3.6,
//! §8.11), bounds and the read-back JSON of `draw.drawing.get` (02 §13.4).

use serde_json::{Map, Value, json};
use tt_model::entities::drawing::{Drawing, DrawingNode, LayerHint, NodeKind, Stroke, StrokeKind};

use crate::geom::{Affine, P, Rect};
use crate::path::Path;
use crate::text;

/// What a drawing-local object is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    /// A layer node.
    Layer,
    /// A group node.
    Group,
    /// A text node.
    Text,
    /// A bitmap node.
    Bitmap,
    /// A stroke of a layer.
    Stroke,
}

impl ObjectKind {
    /// The spelling used in results.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            ObjectKind::Layer => "layer",
            ObjectKind::Group => "group",
            ObjectKind::Text => "text",
            ObjectKind::Bitmap => "bitmap",
            ObjectKind::Stroke => "stroke",
        }
    }
}

/// A located object: its kind, JSON Pointer inside the drawing, the pointer of the list holding it, its index there
/// and the ID of its parent node (`None` at the drawing's top level).
#[derive(Debug, Clone, PartialEq)]
pub struct Located {
    /// Local ID.
    pub id: String,
    /// Kind.
    pub kind: ObjectKind,
    /// Pointer of the object (`/nodes/0/children/2/strokes/5`).
    pub pointer: String,
    /// Pointer of the list it is in.
    pub list: String,
    /// Index in that list.
    pub index: usize,
    /// Parent node ID.
    pub parent: Option<String>,
}

fn kind_of(n: &DrawingNode) -> ObjectKind {
    match n.kind() {
        NodeKind::Layer => ObjectKind::Layer,
        NodeKind::Group => ObjectKind::Group,
        NodeKind::Text => ObjectKind::Text,
        NodeKind::Bitmap => ObjectKind::Bitmap,
    }
}

fn walk(nodes: &[DrawingNode], list: &str, parent: Option<&str>, out: &mut Vec<Located>) {
    for (i, n) in nodes.iter().enumerate() {
        let at = format!("{list}/{i}");
        out.push(Located {
            id: n.id().as_str().to_owned(),
            kind: kind_of(n),
            pointer: at.clone(),
            list: list.to_owned(),
            index: i,
            parent: parent.map(str::to_owned),
        });
        if let Some(strokes) = n.strokes() {
            for (j, s) in strokes.iter().enumerate() {
                out.push(Located {
                    id: s.id().as_str().to_owned(),
                    kind: ObjectKind::Stroke,
                    pointer: format!("{at}/strokes/{j}"),
                    list: format!("{at}/strokes"),
                    index: j,
                    parent: Some(n.id().as_str().to_owned()),
                });
            }
        }
        if let Some(children) = n.children() {
            walk(children, &format!("{at}/children"), Some(n.id().as_str()), out);
        }
    }
}

/// Every object of the drawing in document order (back to front, depth first).
#[must_use]
pub fn objects(d: &Drawing) -> Vec<Located> {
    let mut out = Vec::new();
    walk(d.nodes(), "/nodes", None, &mut out);
    out
}

/// The local part of an ID given in local or project-wide `<drawingId>/<localId>` form (ID-03).
#[must_use]
pub fn local_id<'a>(drawing_id: &str, id: &'a str) -> Option<&'a str> {
    match id.split_once('/') {
        Some((d, l)) if d == drawing_id => Some(l),
        Some(_) => None,
        None => Some(id),
    }
}

/// Finds an object by local or project-wide ID.
#[must_use]
pub fn locate(d: &Drawing, id: &str) -> Option<Located> {
    let local = local_id(d.id().as_str(), id)?;
    objects(d).into_iter().find(|o| o.id == local)
}

/// The node at a pointer from [`objects`].
#[must_use]
pub fn node_at<'a>(d: &'a Drawing, pointer: &str) -> Option<&'a DrawingNode> {
    let mut nodes: &'a [DrawingNode] = d.nodes();
    let mut current: Option<&'a DrawingNode> = None;
    let toks: Vec<&str> = pointer.trim_start_matches('/').split('/').collect();
    let mut k = 0;
    while k < toks.len() {
        match toks[k] {
            "nodes" | "children" => {
                if toks[k] == "children" {
                    nodes = current?.children().as_deref()?;
                }
                let i: usize = toks.get(k + 1)?.parse().ok()?;
                current = Some(nodes.get(i)?);
                k += 2;
            }
            _ => return None,
        }
    }
    current
}

/// The stroke at a pointer from [`objects`].
#[must_use]
pub fn stroke_at<'a>(d: &'a Drawing, pointer: &str) -> Option<&'a Stroke> {
    let (node, idx) = pointer.rsplit_once("/strokes/")?;
    let n = node_at(d, node)?;
    n.strokes().as_ref()?.get(idx.parse::<usize>().ok()?)
}

/// Where a new stroke goes (02 §13.5 `layer`).
#[derive(Debug, Clone, PartialEq)]
pub enum LayerTarget {
    /// An existing layer: its ID and the pointer of its `strokes` list.
    Existing {
        /// Layer ID.
        id: String,
        /// Pointer of its `strokes`.
        strokes: String,
    },
    /// A new layer on top of the drawing.
    NewTop,
    /// A new layer behind all artwork.
    NewBehind,
}

/// The `layer` parameter.
#[derive(Debug, Clone, PartialEq)]
pub enum LayerPolicy {
    /// Brush/Pencil/shapes: a new top layer (behind when Draw Behind is on); invisible strokes: the top-most layer whose
    /// strokes the new stroke crosses, else a new top layer (§7.3.6, §14 decision 10).
    Auto,
    /// A new top layer.
    New,
    /// The top-most top-level layer (a new top layer when there is none).
    Top,
    /// A new layer behind all artwork.
    Behind,
    /// A given layer.
    Id(String),
}

/// Resolves the layer policy. `polyline` is the flattened new stroke (used by `auto` for invisible strokes).
///
/// # Errors
/// `drawing.objectNotFound` for an unknown layer ID; `arg.invalid` when the ID is not a layer.
pub fn layer_target(
    d: &Drawing,
    policy: &LayerPolicy,
    kind: StrokeKind,
    place_behind: bool,
    polyline: &[P],
) -> Result<LayerTarget, (&'static str, String)> {
    match policy {
        LayerPolicy::New => Ok(LayerTarget::NewTop),
        LayerPolicy::Behind => Ok(LayerTarget::NewBehind),
        LayerPolicy::Top => Ok(d.nodes().iter().enumerate().rev().find(|(_, n)| *n.kind() == NodeKind::Layer).map_or(
            LayerTarget::NewTop,
            |(i, n)| LayerTarget::Existing { id: n.id().as_str().to_owned(), strokes: format!("/nodes/{i}/strokes") },
        )),
        LayerPolicy::Id(id) => {
            let Some(o) = locate(d, id) else {
                return Err(("drawing.objectNotFound", format!("no object `{id}` in the drawing")));
            };
            if o.kind != ObjectKind::Layer {
                return Err(("arg.invalid", format!("`{id}` is a {}, not a layer", o.kind.as_str())));
            }
            Ok(LayerTarget::Existing { id: o.id, strokes: format!("{}/strokes", o.pointer) })
        }
        LayerPolicy::Auto => {
            if kind == StrokeKind::Invisible {
                // The top-most layer (any depth, front first) with a stroke the new one crosses.
                let objs = objects(d);
                for o in objs.iter().rev().filter(|o| o.kind == ObjectKind::Layer) {
                    let Some(n) = node_at(d, &o.pointer) else { continue };
                    let crosses = n.strokes().as_ref().is_some_and(|ss| ss.iter().any(|s| crosses(s, polyline)));
                    if crosses {
                        return Ok(LayerTarget::Existing {
                            id: o.id.clone(),
                            strokes: format!("{}/strokes", o.pointer),
                        });
                    }
                }
                return Ok(LayerTarget::NewTop);
            }
            Ok(if place_behind { LayerTarget::NewBehind } else { LayerTarget::NewTop })
        }
    }
}

/// Whether the polyline crosses or touches the stroke's path (flattened within 0.05 du).
fn crosses(s: &Stroke, poly: &[P]) -> bool {
    let path = Path::from_points(s.points(), *s.closed());
    let (Some(a), Some(b)) = (path.bounds(), poly_bounds(poly)) else { return false };
    if !a.touches(&b) {
        return false;
    }
    let other = path.flatten(0.05);
    let mut other_closed = other.clone();
    if *s.closed()
        && let Some(f) = other.first()
    {
        other_closed.push(*f);
    }
    for w in poly.windows(2) {
        for v in other_closed.windows(2) {
            if segments_intersect(w[0], w[1], v[0], v[1]) {
                return true;
            }
        }
    }
    false
}

fn poly_bounds(poly: &[P]) -> Option<Rect> {
    let first = poly.first()?;
    Some(poly.iter().fold(Rect::of(*first), |r, p| r.with(*p)))
}

fn segments_intersect(a: P, b: P, c: P, d: P) -> bool {
    let o = |p: P, q: P, r: P| (q - p).cross(r - p);
    let (d1, d2, d3, d4) = (o(c, d, a), o(c, d, b), o(a, b, c), o(a, b, d));
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0)) && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0)) {
        return true;
    }
    let on =
        |p: P, q: P, r: P| r.x >= p.x.min(q.x) && r.x <= p.x.max(q.x) && r.y >= p.y.min(q.y) && r.y <= p.y.max(q.y);
    (d1 == 0.0 && on(c, d, a)) || (d2 == 0.0 && on(c, d, b)) || (d3 == 0.0 && on(a, b, c)) || (d4 == 0.0 && on(a, b, d))
}

/// A new empty layer node JSON.
#[must_use]
pub fn layer_json(id: &str, hint: LayerHint) -> Value {
    json!({
        "id": id, "type": "drawingNode", "rev": 0, "kind": "layer",
        "hint": match hint { LayerHint::Brush => "brush", LayerHint::Pencil => "pencil" },
        "strokes": [],
    })
}

/// The bounds of a stroke: outline or centre line, grown by half the line thickness for pencil lines.
#[must_use]
pub fn stroke_bounds(s: &Stroke) -> Option<Rect> {
    let r = Path::from_points(s.points(), *s.closed()).bounds()?;
    Some(match s.line_style() {
        Some(ls) => r.grown(ls.thickness / 2.0),
        None => r,
    })
}

/// The text content of a text node.
#[must_use]
pub fn text_content(n: &DrawingNode) -> Option<text::Content> {
    let t = n.text().as_ref()?;
    let runs: Vec<text::Run> = serde_json::from_value(serde_json::to_value(n.runs().as_ref()?).ok()?).ok()?;
    let paras: Vec<text::Para> = serde_json::from_value(serde_json::to_value(n.paragraphs().as_ref()?).ok()?).ok()?;
    Some(text::Content::from_parts(t, runs, paras))
}

/// The layout of a text node.
#[must_use]
pub fn text_layout(n: &DrawingNode) -> Option<text::Layout> {
    let c = text_content(n)?;
    let fixed = if *n.width_mode() == Some(tt_model::entities::drawing::WidthMode::Fixed) { *n.width() } else { None };
    Some(text::layout(&c, fixed))
}

fn affine(m: Option<&[f64; 6]>) -> Affine {
    m.map_or(Affine::IDENTITY, |m| Affine(*m))
}

/// The bounds of a node (layers: their strokes; `include_invisible` counts invisible strokes).
#[must_use]
pub fn node_bounds(n: &DrawingNode, include_invisible: bool) -> Option<Rect> {
    match n.kind() {
        NodeKind::Layer => n
            .strokes()
            .as_ref()?
            .iter()
            .filter(|s| include_invisible || *s.kind() != StrokeKind::Invisible)
            .filter_map(stroke_bounds)
            .reduce(Rect::union),
        NodeKind::Group => {
            n.children().as_ref()?.iter().filter_map(|c| node_bounds(c, include_invisible)).reduce(Rect::union)
        }
        NodeKind::Text => Some(text::bounds(&text_layout(n)?, &affine(n.transform().as_ref()))),
        NodeKind::Bitmap => {
            let (w, h) = ((*n.width())?, (*n.height())?);
            let m = affine(n.transform().as_ref());
            // Pixel space: origin top-left, y down (10 §4.9).
            let corners = [P::new(0.0, 0.0), P::new(w, 0.0), P::new(w, h), P::new(0.0, h)];
            Some(corners.iter().skip(1).fold(Rect::of(m.apply(corners[0])), |r, c| r.with(m.apply(*c))))
        }
    }
}

/// The bounds of the whole drawing (`None` when it has no visible geometry).
#[must_use]
pub fn drawing_bounds(d: &Drawing, include_invisible: bool) -> Option<Rect> {
    d.nodes().iter().filter_map(|n| node_bounds(n, include_invisible)).reduce(Rect::union)
}

/// What `draw.drawing.get` includes (02 §13.4 `include`; zones are the planar map's, M2 C2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Include {
    /// Pressure maps.
    pub pressure: bool,
    /// Brush outlines.
    pub outline: bool,
    /// Bounds.
    pub bounds: bool,
}

impl Default for Include {
    fn default() -> Self {
        Self { pressure: true, outline: true, bounds: true }
    }
}

/// A stroke in the 02 §13.4 shape.
#[must_use]
pub fn stroke_json(s: &Stroke, inc: Include) -> Value {
    let path = Path::from_points(s.points(), *s.closed());
    let mut m = Map::new();
    m.insert("type".into(), json!("stroke"));
    m.insert("id".into(), json!(s.id().as_str()));
    match s.kind() {
        StrokeKind::Brush => {
            m.insert("kind".into(), json!("brush"));
            match s.spine() {
                Some(sp) => {
                    let spine = Path::from_points(&sp.points, sp.points.len() == 1);
                    m.insert("closed".into(), json!(spine.closed));
                    m.insert("bezier".into(), spine.bezier_json());
                    if inc.pressure {
                        m.insert(
                            "pressure".into(),
                            Value::Array(sp.pressure.iter().map(|[t, p]| json!({ "t": t, "p": p })).collect()),
                        );
                    }
                    m.insert("penId".into(), json!(sp.pen.pen_id.as_ref().map(|i| i.as_str())));
                    m.insert(
                        "pen".into(),
                        json!({
                            "tipShape": crate::stroke::tip_name(sp.pen.tip), "minSize": sp.pen.min_size, "maxSize": sp.pen.max_size,
                            "tipAngle": crate::stroke::tip_angle(s),
                        }),
                    );
                }
                None => {
                    m.insert("closed".into(), json!(true));
                    m.insert("bezier".into(), Value::Null);
                    if inc.pressure {
                        m.insert("pressure".into(), Value::Null);
                    }
                    m.insert("penId".into(), Value::Null);
                    m.insert("pen".into(), Value::Null);
                }
            }
            m.insert("colourId".into(), json!(s.colour_id().as_ref().map(|i| i.as_str())));
            m.insert("textureMatrix".into(), json!(s.fill_matrix()));
            if inc.outline {
                m.insert("outline".into(), json!([path.bezier_json()]));
            }
        }
        StrokeKind::Pencil | StrokeKind::Invisible => {
            let pencil = *s.kind() == StrokeKind::Pencil;
            m.insert("kind".into(), json!(if pencil { "pencil" } else { "invisible" }));
            m.insert("closed".into(), json!(*s.closed()));
            m.insert("bezier".into(), path.bezier_json());
            m.insert("colourId".into(), json!(s.colour_id().as_ref().map(|i| i.as_str())));
            if pencil {
                let ls = s.line_style().as_ref();
                m.insert(
                    "lineStyle".into(),
                    json!({ "thickness": ls.map_or(0.0, |l| l.thickness), "lineType": "solid" }),
                );
            }
            if inc.pressure {
                m.insert("pressure".into(), Value::Null);
            }
        }
    }
    if inc.bounds {
        m.insert("bounds".into(), stroke_bounds(s).map_or(Value::Null, |r| r.to_json()));
    }
    Value::Object(m)
}

/// A node in the 02 §13.4 shape (children nested).
#[must_use]
pub fn node_json(n: &DrawingNode, inc: Include) -> Value {
    let mut m = Map::new();
    m.insert("type".into(), json!(kind_of(n).as_str()));
    m.insert("id".into(), json!(n.id().as_str()));
    if let Some(name) = n.name() {
        m.insert("name".into(), json!(name));
    }
    match n.kind() {
        NodeKind::Layer => {
            m.insert(
                "layerKind".into(),
                json!(match n.hint() {
                    Some(LayerHint::Pencil) => "pencil",
                    _ => "brush",
                }),
            );
            m.insert(
                "strokes".into(),
                Value::Array(
                    n.strokes().as_ref().map_or(Vec::new(), |ss| ss.iter().map(|s| stroke_json(s, inc)).collect()),
                ),
            );
        }
        NodeKind::Group => {
            let children: Vec<Value> =
                n.children().as_ref().map_or(Vec::new(), |cs| cs.iter().map(|c| node_json(c, inc)).collect());
            m.insert("children".into(), Value::Array(children));
        }
        NodeKind::Text => {
            m.insert("text".into(), json!(n.text()));
            m.insert("transform".into(), json!(n.transform()));
            let fixed = *n.width_mode() == Some(tt_model::entities::drawing::WidthMode::Fixed);
            m.insert("widthMode".into(), json!(if fixed { "fixed" } else { "auto" }));
            m.insert("width".into(), if fixed { json!(n.width()) } else { Value::Null });
            let runs: Vec<Value> = n.runs().as_ref().map_or(Vec::new(), |rs| {
                rs.iter()
                    .map(|r| {
                        let script = serde_json::to_value(r.script)
                            .ok()
                            .and_then(|v| v.as_str().map(str::to_owned))
                            .unwrap_or_default();
                        json!({
                            "start": r.start, "length": r.length, "font": r.font, "size": r.size, "bold": r.bold,
                            "italic": r.italic, "baseline": script, "kerning": r.kerning,
                            "autoKern": r.auto_kerning, "colourId": r.colour_id.as_ref().map(|i| i.as_str()),
                            "outlineColourId": null,
                        })
                    })
                    .collect()
            });
            m.insert("runs".into(), Value::Array(runs));
            let paras: Vec<Value> = n.paragraphs().as_ref().map_or(Vec::new(), |ps| {
                ps.iter()
                    .map(|p| {
                        let align = serde_json::to_value(p.alignment)
                            .ok()
                            .and_then(|v| v.as_str().map(str::to_owned))
                            .unwrap_or_default();
                        json!({
                            "start": p.start, "length": p.length, "align": align, "leftMargin": p.margin_left,
                            "rightMargin": p.margin_right, "indent": p.indent, "lineSpacing": p.line_spacing,
                        })
                    })
                    .collect()
            });
            m.insert("paragraphs".into(), Value::Array(paras));
        }
        NodeKind::Bitmap => {
            let image = n.image().as_ref();
            m.insert("mediaId".into(), json!(image.map(|b| b.entry.clone())));
            m.insert("pixelWidth".into(), json!(n.width()));
            m.insert("pixelHeight".into(), json!(n.height()));
            m.insert("transform".into(), json!(n.transform()));
            m.insert("image".into(), json!(image));
        }
    }
    if inc.bounds && *n.kind() != NodeKind::Layer {
        m.insert("bounds".into(), node_bounds(n, false).map_or(Value::Null, |r| r.to_json()));
    }
    Value::Object(m)
}

/// The flat object list (`flatten: true`): every node and stroke with `parentId`, children not nested.
#[must_use]
pub fn flat_json(d: &Drawing, inc: Include) -> Vec<Value> {
    let mut out = Vec::new();
    for o in objects(d) {
        let mut v = if o.kind == ObjectKind::Stroke {
            stroke_at(d, &o.pointer).map(|s| stroke_json(s, inc))
        } else {
            node_at(d, &o.pointer).map(|n| node_json(n, inc))
        };
        if let Some(Value::Object(m)) = v.as_mut() {
            m.remove("strokes");
            m.remove("children");
            m.insert("parentId".into(), json!(o.parent));
        }
        if let Some(v) = v {
            out.push(v);
        }
    }
    out
}

/// Keeps only the objects with the given local IDs (and nothing else) in a flat list or a nested tree.
#[must_use]
pub fn select_ids(list: Vec<Value>, ids: &[String]) -> Vec<Value> {
    list.into_iter()
        .filter(|v| v.get("id").and_then(Value::as_str).is_some_and(|i| ids.iter().any(|x| x == i)))
        .collect()
}

/// The JSON Schema of the `draw.drawing.get` result (02 §13.4; closed, draft 2020-12).
#[must_use]
pub fn readback_schema() -> Value {
    serde_json::from_str(include_str!("readback.schema.json")).unwrap_or(Value::Null)
}
