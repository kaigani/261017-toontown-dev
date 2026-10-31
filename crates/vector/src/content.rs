//! Drawing content in the 02 §13.4 shape as input (`draw.content.add`, "IDs reassigned"): validated, given new IDs,
//! offset, and converted to the persisted form (10 §4.9). A `draw.drawing.get` result can be passed back as is
//! (its read-only members — `drawingId`, `rev`, `bounds`, … — are accepted and ignored).
//!
//! Brush strokes keep their outline when given (it is authoritative, §3.2) and their spine when `bezier` is given;
//! with only a spine the outline is swept from the spine, pressure and pen. Bitmaps refer to an image the document
//! already holds (`mediaId` = its package entry); new pixels arrive with image import (M10, SI-052).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use tt_model::ObjectType;
use tt_model::entities::pen::Tip;
use tt_model::value::Blob;

use crate::brush::BrushPen;
use crate::geom::{Affine, P};
use crate::path::{GeomError, Path, check_coord};

/// A point `{x, y}`.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PointIn {
    /// X (du).
    pub x: f64,
    /// Y (du).
    pub y: f64,
}

/// `{x, y, width, height}` (read-only; ignored).
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoundsIn {
    /// X.
    pub x: f64,
    /// Y.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

/// The drawing space description of a read-back (ignored).
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SpaceIn {
    /// `du`.
    pub units: String,
    /// 100.
    pub du_per_field: f64,
    /// `up`.
    pub y_axis: String,
    /// `drawingCentre`.
    pub origin: String,
}

/// Content to insert: `{nodes}` (a whole `draw.drawing.get` result is accepted).
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContentIn {
    /// Nodes, back to front.
    #[schemars(length(max = 100_000))]
    pub nodes: Vec<NodeIn>,
    /// Read-only (ignored).
    #[serde(default)]
    pub drawing_id: Option<String>,
    /// Read-only (ignored).
    #[serde(default)]
    pub element_id: Option<String>,
    /// Read-only (ignored).
    #[serde(default)]
    pub name: Option<String>,
    /// Read-only (ignored).
    #[serde(default)]
    pub rev: Option<u64>,
    /// Read-only (ignored).
    #[serde(default)]
    pub locked: Option<bool>,
    /// Read-only (ignored).
    #[serde(default)]
    pub space: Option<SpaceIn>,
    /// Read-only (ignored; the target drawing keeps its pivot).
    #[serde(default)]
    pub pivot: Option<PointIn>,
    /// Read-only (ignored).
    #[serde(default)]
    pub bounds: Option<BoundsIn>,
    /// Recovery copies of the colours the content uses (10 §4.11 entries; the clipboard's colour map, 03 §11.1):
    /// colours the target project lacks are recreated from them (03 §11.2).
    #[serde(default)]
    #[schemars(length(max = 100_000))]
    pub colour_recovery: Option<Vec<Value>>,
}

/// A node of the 02 §13.4 shape.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum NodeIn {
    /// A layer.
    Layer(LayerIn),
    /// A group.
    Group(GroupIn),
    /// A text object.
    Text(TextIn),
    /// An embedded bitmap.
    Bitmap(BitmapIn),
}

/// Layer type hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum LayerKindIn {
    /// Contour artwork.
    Brush,
    /// Centre-line artwork.
    Pencil,
}

/// A layer.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LayerIn {
    /// ID (replaced).
    pub id: String,
    /// Optional name.
    #[serde(default)]
    pub name: Option<String>,
    /// Type hint.
    pub layer_kind: LayerKindIn,
    /// Strokes.
    #[schemars(length(max = 100_000))]
    pub strokes: Vec<StrokeIn>,
    /// Zones as read back (02 §13.4): their fills are painted onto the zones of the inserted layer that contain their
    /// interior points; everything else about them is derived and ignored.
    #[serde(default)]
    #[schemars(length(max = 1_000_000))]
    pub zones: Option<Vec<ZoneIn>>,
    /// Read-only (ignored).
    #[serde(default)]
    pub bounds: Option<BoundsIn>,
}

/// A zone of read-back content (only `colourId`, `textureMatrix` and `interiorPoint` are used).
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ZoneIn {
    /// `zone` (optional).
    #[serde(default, rename = "type")]
    pub ty: Option<String>,
    /// Derived ID (ignored).
    #[serde(default)]
    pub id: Option<String>,
    /// Fill colour (`null` = unpainted).
    #[serde(default)]
    pub colour_id: Option<String>,
    /// Gradient/texture placement.
    #[serde(default)]
    pub texture_matrix: Option<[f64; 6]>,
    /// A point inside the zone.
    pub interior_point: PointIn,
    /// Derived (ignored).
    #[serde(default)]
    pub boundary_stroke_ids: Option<Vec<String>>,
    /// Derived (ignored).
    #[serde(default)]
    pub holes: Option<Vec<Value>>,
    /// Derived (ignored).
    #[serde(default)]
    pub parent_zone_id: Option<String>,
    /// Derived (ignored).
    #[serde(default)]
    pub area: Option<f64>,
    /// Derived (ignored).
    #[serde(default)]
    pub bounds: Option<BoundsIn>,
    /// Flat read-back parent (ignored).
    #[serde(default)]
    pub parent_id: Option<String>,
}

/// A group.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GroupIn {
    /// ID (replaced).
    pub id: String,
    /// Optional name.
    #[serde(default)]
    pub name: Option<String>,
    /// Children, back to front.
    pub children: Vec<NodeIn>,
    /// Read-only (ignored).
    #[serde(default)]
    pub bounds: Option<BoundsIn>,
}

/// Stroke kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum StrokeKindIn {
    /// Brush.
    Brush,
    /// Pencil.
    Pencil,
    /// Invisible.
    Invisible,
}

/// `stroke` (the `type` of a stroke in a read-back).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum StrokeTypeIn {
    /// A stroke.
    Stroke,
}

/// Line style.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LineStyleIn {
    /// Thickness (du).
    #[schemars(range(min = 0.0, max = 100.0))]
    pub thickness: f64,
    /// `solid`.
    pub line_type: LineTypeIn,
}

/// Line type.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum LineTypeIn {
    /// Solid.
    Solid,
}

/// Tip shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum TipIn {
    /// Round.
    Round,
    /// Oval.
    Oval,
    /// Square.
    Square,
    /// Flat.
    Flat,
}

/// A brush pen.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PenIn {
    /// Tip shape.
    pub tip_shape: TipIn,
    /// Minimum size (du).
    #[schemars(range(min = 0.0, max = 100.0))]
    pub min_size: f64,
    /// Maximum size (du).
    #[schemars(range(min = 0.0, max = 100.0))]
    pub max_size: f64,
    /// Tip angle (degrees).
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 360.0))]
    pub tip_angle: f64,
}

/// A pressure sample.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PressureIn {
    /// Position 0–1.
    #[schemars(range(min = 0.0, max = 1.0))]
    pub t: f64,
    /// Pressure 0–1.
    #[schemars(range(min = 0.0, max = 1.0))]
    pub p: f64,
}

/// A stroke of the 02 §13.4 shape.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StrokeIn {
    /// `stroke` (optional).
    #[serde(default, rename = "type")]
    pub ty: Option<StrokeTypeIn>,
    /// ID (replaced).
    pub id: String,
    /// Kind.
    pub kind: StrokeKindIn,
    /// Whether `bezier` is closed.
    #[serde(default)]
    pub closed: bool,
    /// Centre line (pencil, invisible) or spine (brush; `null` once the outline was edited).
    #[serde(default)]
    pub bezier: Option<Vec<PointIn>>,
    /// Colour (kept as given, ID-05).
    #[serde(default)]
    pub colour_id: Option<String>,
    /// Pencil line style.
    #[serde(default)]
    pub line_style: Option<LineStyleIn>,
    /// Brush pen ID (kept when the pen exists in this project).
    #[serde(default)]
    pub pen_id: Option<String>,
    /// Brush pen.
    #[serde(default)]
    pub pen: Option<PenIn>,
    /// Brush spine pressure.
    #[serde(default)]
    pub pressure: Option<Vec<PressureIn>>,
    /// Gradient/texture placement of a brush fill.
    #[serde(default)]
    pub texture_matrix: Option<[f64; 6]>,
    /// Brush outline contours (closed Bézier lists; one contour, SI-053).
    #[serde(default)]
    pub outline: Option<Vec<Vec<PointIn>>>,
    /// Derived side zone (ignored).
    #[serde(default)]
    pub left_zone_id: Option<String>,
    /// Derived side zone (ignored).
    #[serde(default)]
    pub right_zone_id: Option<String>,
    /// Read-only (ignored).
    #[serde(default)]
    pub bounds: Option<BoundsIn>,
}

/// Width mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum WidthModeIn {
    /// No wrapping.
    Auto,
    /// Word wrap at `width`.
    Fixed,
}

/// Baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum BaselineIn {
    /// Normal.
    Normal,
    /// Subscript.
    Subscript,
    /// Superscript.
    Superscript,
}

/// Alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum AlignIn {
    /// Left.
    Left,
    /// Centre.
    Centre,
    /// Right.
    Right,
    /// Justify.
    Justify,
}

/// A character run of the 02 §13.4 shape.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RunIn {
    /// First code point.
    pub start: u32,
    /// Number of code points.
    pub length: u32,
    /// Font family.
    #[schemars(length(min = 1, max = 256))]
    pub font: String,
    /// Size (du).
    #[schemars(range(min = 1.0, max = 512.0))]
    pub size: f64,
    /// Bold.
    pub bold: bool,
    /// Italic.
    pub italic: bool,
    /// Baseline.
    pub baseline: BaselineIn,
    /// Kerning (1/100 em).
    #[schemars(range(min = -60.0, max = 60.0))]
    pub kerning: f64,
    /// Automatic kerning.
    pub auto_kern: bool,
    /// Fill colour.
    #[serde(default)]
    pub colour_id: Option<String>,
    /// Outline colour (must be `null`: 10 §4.9 runs have no outline colour, SI-055).
    #[serde(default)]
    pub outline_colour_id: Option<String>,
}

/// A paragraph of the 02 §13.4 shape.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ParaIn {
    /// First code point.
    pub start: u32,
    /// Number of code points.
    pub length: u32,
    /// Alignment.
    pub align: AlignIn,
    /// Left margin (du).
    #[schemars(range(min = 0.0, max = 1000.0))]
    pub left_margin: f64,
    /// Right margin (du).
    #[schemars(range(min = 0.0, max = 1000.0))]
    pub right_margin: f64,
    /// First-line indent (du).
    #[schemars(range(min = -99.0, max = 1000.0))]
    pub indent: f64,
    /// Line spacing (du).
    #[schemars(range(min = -99.0, max = 1000.0))]
    pub line_spacing: f64,
}

/// A text object of the 02 §13.4 shape.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TextIn {
    /// ID (replaced).
    pub id: String,
    /// Optional name.
    #[serde(default)]
    pub name: Option<String>,
    /// The characters.
    #[schemars(length(max = 100_000))]
    pub text: String,
    /// Placement (origin = top-left of the box).
    pub transform: [f64; 6],
    /// Auto or fixed width.
    pub width_mode: WidthModeIn,
    /// Wrap width (fixed mode).
    #[serde(default)]
    pub width: Option<f64>,
    /// Runs covering the text.
    pub runs: Vec<RunIn>,
    /// Paragraphs.
    pub paragraphs: Vec<ParaIn>,
    /// Read-only (ignored).
    #[serde(default)]
    pub bounds: Option<BoundsIn>,
}

/// An embedded bitmap of the 02 §13.4 shape.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BitmapIn {
    /// ID (replaced).
    pub id: String,
    /// Optional name.
    #[serde(default)]
    pub name: Option<String>,
    /// The package entry of an image the document holds.
    pub media_id: String,
    /// Pixel width (1–16384).
    #[schemars(range(min = 1.0, max = 16384.0))]
    pub pixel_width: f64,
    /// Pixel height (1–16384).
    #[schemars(range(min = 1.0, max = 16384.0))]
    pub pixel_height: f64,
    /// Pixel space → drawing space.
    pub transform: [f64; 6],
    /// The image blob as read back (must match `mediaId`).
    #[serde(default)]
    pub image: Option<Blob>,
    /// Read-only (ignored).
    #[serde(default)]
    pub bounds: Option<BoundsIn>,
}

/// An image the document holds: its blob and pixel size.
#[derive(Debug, Clone, PartialEq)]
pub struct KnownImage {
    /// The blob.
    pub blob: Blob,
    /// Pixel width.
    pub width: f64,
    /// Pixel height.
    pub height: f64,
}

/// Why content is refused (`E`: the caller's error when drawing an ID fails).
#[derive(Debug)]
pub enum ContentError<E> {
    /// Malformed geometry.
    Geometry(GeomError),
    /// Invalid content (pointer inside `content`, message).
    Invalid(String, String),
    /// Drawing a new ID failed.
    Id(E),
}

/// A zone fill of inserted content: new layer ID, interior point (offset applied), colour, mapping matrix.
pub type ZoneFillIn = (String, P, Option<String>, Option<[f64; 6]>);

/// The converted content.
#[derive(Debug, Clone, PartialEq)]
pub struct Built {
    /// Persisted node JSON (rev 0), back to front.
    pub nodes: Vec<Value>,
    /// Old ID → new ID.
    pub id_map: BTreeMap<String, String>,
    /// Colour IDs the content uses, sorted.
    pub colours: Vec<String>,
    /// Zone fills to paint after insertion: new layer ID, interior point (offset applied), colour, matrix.
    pub zone_fills: Vec<ZoneFillIn>,
}

struct Ctx<'a, F> {
    offset: Affine,
    images: &'a [KnownImage],
    pens: &'a [String],
    fresh: F,
    id_map: BTreeMap<String, String>,
    colours: Vec<String>,
    zone_fills: Vec<ZoneFillIn>,
}

fn inv<E>(at: &str, m: impl Into<String>) -> ContentError<E> {
    ContentError::Invalid(at.to_owned(), m.into())
}

fn pts<E>(list: &[PointIn], at: &str) -> Result<Vec<P>, ContentError<E>> {
    for (i, p) in list.iter().enumerate() {
        check_coord(p.x, &format!("{at}/{i}/x")).map_err(ContentError::Geometry)?;
        check_coord(p.y, &format!("{at}/{i}/y")).map_err(ContentError::Geometry)?;
    }
    Ok(list.iter().map(|p| P::new(p.x, p.y)).collect())
}

fn tip(t: TipIn) -> Tip {
    match t {
        TipIn::Round => Tip::Round,
        TipIn::Oval => Tip::Oval,
        TipIn::Square => Tip::Square,
        TipIn::Flat => Tip::Flat,
    }
}

impl<E, F: FnMut(ObjectType) -> Result<String, E>> Ctx<'_, F> {
    fn new_id(&mut self, old: &str, ty: ObjectType, at: &str) -> Result<String, ContentError<E>> {
        if self.id_map.contains_key(old) {
            return Err(inv(&format!("{at}/id"), format!("the ID `{old}` is given twice")));
        }
        let id = (self.fresh)(ty).map_err(ContentError::Id)?;
        self.id_map.insert(old.to_owned(), id.clone());
        Ok(id)
    }

    fn colour(&mut self, c: Option<&String>) {
        if let Some(c) = c
            && !self.colours.contains(c)
        {
            self.colours.push(c.clone());
        }
    }

    fn matrix(&self, m: &[f64; 6], at: &str) -> Result<Value, ContentError<E>> {
        for (i, v) in m.iter().enumerate() {
            if !v.is_finite() {
                return Err(ContentError::Geometry(GeomError::invalid(
                    format!("{at}/{i}"),
                    "matrix entries are finite",
                )));
            }
        }
        let t = self.offset.then_after(&Affine(*m));
        check_coord(t.0[4], &format!("{at}/4")).map_err(ContentError::Geometry)?;
        check_coord(t.0[5], &format!("{at}/5")).map_err(ContentError::Geometry)?;
        Ok(json!(t.0))
    }

    fn node(&mut self, n: &NodeIn, at: &str) -> Result<Value, ContentError<E>> {
        match n {
            NodeIn::Layer(l) => {
                let id = self.new_id(&l.id, ObjectType::DrawingNode, at)?;
                let mut strokes = Vec::with_capacity(l.strokes.len());
                for (i, s) in l.strokes.iter().enumerate() {
                    strokes.push(self.stroke(s, &format!("{at}/strokes/{i}"))?);
                }
                let mut v = json!({
                    "id": id, "type": "drawingNode", "rev": 0, "kind": "layer",
                    "hint": if l.layer_kind == LayerKindIn::Brush { "brush" } else { "pencil" },
                    "strokes": strokes,
                });
                name(&mut v, l.name.as_ref());
                for (k, z) in l.zones.iter().flatten().enumerate() {
                    let at_z = format!("{at}/zones/{k}");
                    let p = pts(std::slice::from_ref(&z.interior_point), &format!("{at_z}/interiorPoint"))?;
                    if let Some(c) = &z.colour_id {
                        if !self.colours.contains(c) {
                            self.colours.push(c.clone());
                        }
                        let q = self.offset.apply(p[0]);
                        // The fill mapping moves with the content (03 §7.2).
                        let (dx, dy) = (self.offset.0[4], self.offset.0[5]);
                        let m = z.texture_matrix.map(|m| [m[0], m[1], m[2], m[3], m[4] + dx, m[5] + dy]);
                        self.zone_fills.push((id.clone(), q, Some(c.clone()), m));
                    }
                }
                Ok(v)
            }
            NodeIn::Group(g) => {
                let id = self.new_id(&g.id, ObjectType::DrawingNode, at)?;
                let mut children = Vec::with_capacity(g.children.len());
                for (i, c) in g.children.iter().enumerate() {
                    children.push(self.node(c, &format!("{at}/children/{i}"))?);
                }
                let mut v = json!({ "id": id, "type": "drawingNode", "rev": 0, "kind": "group", "children": children });
                name(&mut v, g.name.as_ref());
                Ok(v)
            }
            NodeIn::Text(t) => self.text(t, at),
            NodeIn::Bitmap(b) => {
                let id = self.new_id(&b.id, ObjectType::DrawingNode, at)?;
                let Some(img) = self.images.iter().find(|k| k.blob.entry == b.media_id) else {
                    return Err(inv(
                        &format!("{at}/mediaId"),
                        format!(
                            "`{}` is not an image of this document (new images arrive with image import)",
                            b.media_id
                        ),
                    ));
                };
                if b.pixel_width != img.width || b.pixel_height != img.height {
                    return Err(inv(&format!("{at}/pixelWidth"), "the pixel size differs from the image's"));
                }
                if b.image.as_ref().is_some_and(|i| *i != img.blob) {
                    return Err(inv(&format!("{at}/image"), "the image differs from the one mediaId names"));
                }
                let mut v = json!({
                    "id": id, "type": "drawingNode", "rev": 0, "kind": "bitmap", "image": img.blob,
                    "width": img.width, "height": img.height, "transform": self.matrix(&b.transform, &format!("{at}/transform"))?,
                });
                name(&mut v, b.name.as_ref());
                Ok(v)
            }
        }
    }

    fn text(&mut self, t: &TextIn, at: &str) -> Result<Value, ContentError<E>> {
        let id = self.new_id(&t.id, ObjectType::DrawingNode, at)?;
        let n = t.text.chars().count();
        let mut pos = 0u64;
        let mut runs = Vec::with_capacity(t.runs.len());
        for (i, r) in t.runs.iter().enumerate() {
            if u64::from(r.start) != pos {
                return Err(inv(&format!("{at}/runs/{i}/start"), "runs cover the text consecutively"));
            }
            if r.outline_colour_id.is_some() {
                return Err(inv(
                    &format!("{at}/runs/{i}/outlineColourId"),
                    "outline colours are not stored by the project format (SI-055)",
                ));
            }
            pos += u64::from(r.length);
            self.colour(r.colour_id.as_ref());
            runs.push(json!({
                "start": r.start, "length": r.length, "font": r.font, "size": r.size, "bold": r.bold, "italic": r.italic,
                "script": match r.baseline { BaselineIn::Normal => "normal", BaselineIn::Subscript => "subscript", BaselineIn::Superscript => "superscript" },
                "kerning": r.kerning, "autoKerning": r.auto_kern, "colourId": r.colour_id,
            }));
        }
        if pos != n as u64 {
            return Err(inv(&format!("{at}/runs"), "runs cover the whole text"));
        }
        let paras: Vec<Value> = t
            .paragraphs
            .iter()
            .map(|p| {
                json!({
                    "start": p.start, "length": p.length,
                    "alignment": match p.align { AlignIn::Left => "left", AlignIn::Centre => "centre", AlignIn::Right => "right", AlignIn::Justify => "justify" },
                    "marginLeft": p.left_margin, "marginRight": p.right_margin, "indent": p.indent, "lineSpacing": p.line_spacing,
                })
            })
            .collect();
        // Paragraph ranges follow the lines of the text (one per line, 02 §3.7).
        let runs_typed: Vec<crate::text::Run> =
            serde_json::from_value(Value::Array(runs.clone())).map_err(|e| inv(at, e.to_string()))?;
        let paras_typed: Vec<crate::text::Para> =
            serde_json::from_value(Value::Array(paras)).map_err(|e| inv(at, e.to_string()))?;
        let c = crate::text::Content::from_parts(&t.text, runs_typed, paras_typed);
        let mut v = json!({
            "id": id, "type": "drawingNode", "rev": 0, "kind": "text",
            "transform": self.matrix(&t.transform, &format!("{at}/transform"))?,
            "widthMode": if t.width_mode == WidthModeIn::Fixed { "fixed" } else { "auto" },
            "text": t.text, "runs": c.runs, "paragraphs": c.paras,
        });
        if t.width_mode == WidthModeIn::Fixed {
            let w = t.width.ok_or_else(|| inv(&format!("{at}/width"), "fixed-width text has a width"))?;
            if !(w.is_finite() && w > 0.0 && w <= 1.0e7) {
                return Err(inv(&format!("{at}/width"), "the width is positive and finite"));
            }
            v["width"] = json!(w);
        }
        name(&mut v, t.name.as_ref());
        Ok(v)
    }

    fn stroke(&mut self, s: &StrokeIn, at: &str) -> Result<Value, ContentError<E>> {
        let id = self.new_id(&s.id, ObjectType::Stroke, at)?;
        let off = self.offset;
        let bezier = match &s.bezier {
            Some(b) => {
                let p = Path::from_bezier_list(&pts(b, &format!("{at}/bezier"))?, s.closed, &format!("{at}/bezier"))
                    .map_err(ContentError::Geometry)?;
                Some(p.transformed(&off))
            }
            None => None,
        };
        let mut v = json!({ "id": id, "type": "stroke", "rev": 0, "fillMatrix": null, "sides": { "left": null, "right": null } });
        match s.kind {
            StrokeKindIn::Pencil | StrokeKindIn::Invisible => {
                let path =
                    bezier.ok_or_else(|| inv(&format!("{at}/bezier"), "a pencil or invisible stroke has a path"))?;
                if path.anchors.len() < 2 {
                    return Err(ContentError::Geometry(GeomError::invalid(
                        format!("{at}/bezier"),
                        "a line needs at least two anchors",
                    )));
                }
                v["kind"] = json!(if s.kind == StrokeKindIn::Pencil { "pencil" } else { "invisible" });
                v["closed"] = json!(path.closed);
                v["points"] = path.points_json();
                v["spine"] = Value::Null;
                if s.kind == StrokeKindIn::Pencil {
                    let ls = s
                        .line_style
                        .ok_or_else(|| inv(&format!("{at}/lineStyle"), "a pencil line has a line style"))?;
                    self.colour(s.colour_id.as_ref());
                    v["colourId"] = json!(s.colour_id);
                    v["lineStyle"] = json!({ "thickness": ls.thickness, "lineType": "solid" });
                } else {
                    v["colourId"] = Value::Null;
                    v["lineStyle"] = Value::Null;
                }
            }
            StrokeKindIn::Brush => {
                self.colour(s.colour_id.as_ref());
                let pressure: Vec<[f64; 2]> = s
                    .pressure
                    .as_ref()
                    .map_or_else(|| vec![[0.0, 1.0], [1.0, 1.0]], |p| p.iter().map(|x| [x.t, x.p]).collect());
                crate::fit::check_pressure(&pressure, &format!("{at}/pressure")).map_err(ContentError::Geometry)?;
                let pen = s.pen.map(|p| BrushPen {
                    tip: tip(p.tip_shape),
                    min_size: p.min_size,
                    max_size: p.max_size,
                    tip_angle: p.tip_angle,
                });
                let outline = match &s.outline {
                    Some(o) if o.len() == 1 => Path::from_bezier_list(
                        &pts(&o[0], &format!("{at}/outline/0"))?,
                        true,
                        &format!("{at}/outline/0"),
                    )
                    .map_err(ContentError::Geometry)?
                    .transformed(&off),
                    Some(_) => {
                        return Err(inv(&format!("{at}/outline"), "a brush outline is one closed contour (SI-053)"));
                    }
                    None => {
                        let (Some(spine), Some(pen)) = (&bezier, pen) else {
                            return Err(inv(
                                &format!("{at}/outline"),
                                "a brush stroke has an outline, or a spine with its pen",
                            ));
                        };
                        crate::brush::outline(spine, &pressure, &pen).map_err(ContentError::Geometry)?
                    }
                };
                v["kind"] = json!("brush");
                v["closed"] = json!(true);
                v["points"] = outline.points_json();
                v["colourId"] = json!(s.colour_id);
                v["lineStyle"] = Value::Null;
                if let Some(m) = &s.texture_matrix {
                    v["fillMatrix"] = self.matrix(m, &format!("{at}/textureMatrix"))?;
                }
                v["spine"] = match (&bezier, pen) {
                    (Some(spine), Some(pen)) => {
                        let stored =
                            if spine.anchors.len() == 1 { spine.clone() } else { crate::stroke::open_spine(spine) };
                        let pen_id = s.pen_id.as_ref().filter(|p| self.pens.contains(p));
                        if pen.tip != Tip::Round && pen.tip_angle != 0.0 {
                            v["ext"] = json!({ crate::stroke::EXT_NAMESPACE: { "tipAngle": pen.tip_angle } });
                        }
                        json!({
                            "points": stored.points_json(), "pressure": pressure,
                            "pen": { "penId": pen_id, "tip": crate::stroke::tip_name(pen.tip), "minSize": pen.min_size, "maxSize": pen.max_size },
                        })
                    }
                    _ => Value::Null,
                };
            }
        }
        Ok(v)
    }
}

fn name(v: &mut Value, n: Option<&String>) {
    if let (Some(n), Some(m)) = (n, v.as_object_mut()) {
        m.insert("name".into(), json!(n));
    }
}

/// Converts content: new IDs from `fresh`, geometry and transforms offset by `offset`.
///
/// # Errors
/// [`ContentError`].
pub fn build<E, F>(
    c: &ContentIn,
    offset: P,
    images: &[KnownImage],
    pens: &[String],
    fresh: F,
) -> Result<Built, ContentError<E>>
where
    F: FnMut(ObjectType) -> Result<String, E>,
{
    let mut ctx = Ctx {
        offset: Affine::translate(offset.x, offset.y),
        images,
        pens,
        fresh,
        id_map: BTreeMap::new(),
        colours: Vec::new(),
        zone_fills: Vec::new(),
    };
    let mut nodes = Vec::with_capacity(c.nodes.len());
    for (i, n) in c.nodes.iter().enumerate() {
        nodes.push(ctx.node(n, &format!("/nodes/{i}"))?);
    }
    let mut colours = ctx.colours;
    colours.sort();
    Ok(Built { nodes, id_map: ctx.id_map, colours, zone_fills: ctx.zone_fills })
}
