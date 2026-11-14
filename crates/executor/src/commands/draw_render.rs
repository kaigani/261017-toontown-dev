//! `draw.drawing.render` (02 §13.5; MCP `render_drawing`; M2 C2.3): one drawing rendered in drawing space to a PNG
//! through the deterministic adapter of `tt-render` (ADR-0006).
//!
//! Content order: nodes back to front; within a layer, painted zones first (their boundary rings, even-odd, so holes
//! stay open), then brush strokes in stroke order (their outline, non-zero, so bridged holes stay open, SI-053), then
//! pencil lines in stroke order (round joins and caps, the line thickness). Invisible strokes are not drawn. Zone
//! fills are the planar map's, including fills stored as brush-exterior records (SI-060). Text is drawn from its
//! glyph outlines; bitmap nodes are drawn from their PNG pixels through their placement (10 §4.9; M3 C3.3, SI-105).
//!
//! Text pixels depend on the machine's fonts (02 §3.7, ADR-0016), so cross-platform goldens avoid text.
//!
//! Colours are indirect (03 §1.1): every colour ID resolves through its swatch to the value of the chosen palette
//! style (`styleId`, for its palette) or, for other palettes, the palette's first style (SI-065). Gradients and
//! textures map their unit space through the stored matrix (or the default box mapping of the object's bounds);
//! textures are decoded from the project's `textures/` entries, repeated when tiled. A colour or texture that cannot
//! be resolved leaves its object undrawn and is reported in `warnings`.
//!
//! Region and size (SI-074): `region` gives the drawing-space rectangle (`bounds`: the visible bounds, the 12-field
//! guide when the drawing is empty), grown by `padding` du on each side. Without `height` the image keeps the region's
//! aspect; with both, the region is fitted and centred. `drawingToPixel` is the uniform map used.

use std::collections::BTreeMap;
use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_model::document::Document;
use tt_model::entities::drawing::{Drawing, DrawingNode, NodeKind, StrokeKind};
use tt_model::entities::palette::{SwatchKind, SwatchValue};
use tt_model::{Id, ObjectType};
use tt_registry::{CommandError, command};
use tt_render::{El, Item, Paint, Rule, Scene};
use tt_vector::drawing as vd;
use tt_vector::geom::{Affine, P, Rect};
use tt_vector::path::Path;

use super::draw::{err, target};
use crate::cx::Cx;

/// Work units (path elements and pixels / 1024) one render may spend.
const RENDER_WORK_BUDGET: u64 = 200_000_000;
/// Wall-clock limit of one render (under the executor's watchdog budget).
const RENDER_TIME_LIMIT: std::time::Duration = std::time::Duration::from_secs(45);
/// Largest texture side decoded.
const MAX_TEXTURE_SIDE: u32 = 4096;

/// A named region.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RegionName {
    /// The drawing's visible bounds.
    #[default]
    Bounds,
    /// The 12-field guide (x ±600, y ±450).
    Field12,
    /// The 16-field guide (x ±800, y ±600).
    Field16,
    /// The default camera frame (12 fields wide, the project's aspect).
    Camera,
}

/// A rectangle `{x, y, width, height}` (du).
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegionRect {
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

/// The region to render.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(untagged)]
pub(crate) enum RegionIn {
    /// A named region.
    Named(RegionName),
    /// A rectangle.
    Rect(RegionRect),
}

impl Default for RegionIn {
    fn default() -> Self {
        RegionIn::Named(RegionName::Bounds)
    }
}

fn transparent() -> String {
    "#00000000".to_owned()
}

/// `draw.drawing.render` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RenderParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Image width (px).
    #[schemars(range(min = 1, max = 8192))]
    width: u32,
    /// Image height (px; default: the region's aspect).
    #[serde(default)]
    #[schemars(range(min = 1, max = 8192))]
    height: Option<u32>,
    /// Region.
    #[serde(default)]
    region: RegionIn,
    /// Margin around the region (du).
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 100000.0))]
    padding: f64,
    /// Background (#RRGGBB or #RRGGBBAA).
    #[serde(default = "transparent")]
    background: String,
    /// Palette style (default: each palette's first style).
    #[serde(default)]
    style_id: Option<String>,
    /// Draw every stroke's centre path (invisible ones included) and mark open ends.
    #[serde(default)]
    show_strokes: bool,
    /// Outline the drawing selection's bounds.
    #[serde(default)]
    show_selection: bool,
    /// Write the PNG here instead of returning it.
    #[serde(default)]
    path: Option<String>,
}

/// `{png?, path?, width, height, drawingToPixel, rev}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RenderResult {
    /// The PNG, base64 (without `path`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) png: Option<String>,
    /// The written file (with `path`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) path: Option<String>,
    /// Width (px).
    pub(crate) width: u32,
    /// Height (px).
    pub(crate) height: u32,
    /// Drawing space → pixel space `[a, b, c, d, e, f]`.
    pub(crate) drawing_to_pixel: [f64; 6],
    /// The drawing revision rendered.
    pub(crate) rev: u64,
    /// SHA-256 of the premultiplied RGBA8 buffer (hex), for determinism checks.
    pub(crate) pixels_sha256: String,
    /// Objects not drawn and why (`render.colourMissing`, `render.textureMissing`, `render.bitmapMissing`).
    pub(crate) warnings: Vec<Value>,
}

fn invalid(pointer: &str, msg: impl Into<String>) -> CommandError {
    err("arg.invalid", msg, json!({ "pointer": pointer }))
}

/// A decoded texture: width, height, straight RGBA8.
type Texture = (u16, u16, Arc<Vec<u8>>);

/// Largest bitmap-node side decoded (10 §4.9: 1–16384 pixels).
const MAX_BITMAP_SIDE: u32 = 16_384;

/// Builds render items of drawing content (shared with the scene compositor, `render_list.rs`, M3 C3.3).
pub(crate) struct Builder<'a> {
    doc: &'a Document,
    blobs: Option<&'a tt_persist::container::Blobs>,
    style: Option<(Id, Id)>,
    /// The scene's active style per palette (03 §4): used for palettes `style` does not name.
    scene_styles: Option<BTreeMap<Id, Id>>,
    d2p: Affine,
    scale: f64,
    pub(crate) items: Vec<Item>,
    pub(crate) warnings: Vec<Value>,
    textures: BTreeMap<String, Option<Texture>>,
    work: u64,
}

fn els(path: &Path, m: &Affine) -> Vec<El> {
    let mut out = Vec::new();
    let Some(first) = path.anchors.first() else { return out };
    let p0 = m.apply(first.p);
    out.push(El::MoveTo(p0.x, p0.y));
    for c in path.segments() {
        let (a, b, e) = (m.apply(c.p1), m.apply(c.p2), m.apply(c.p3));
        out.push(El::CubicTo(a.x, a.y, b.x, b.y, e.x, e.y));
    }
    if path.closed {
        out.push(El::Close);
    }
    out
}

fn ring_els(rings: &[Vec<P>], m: &Affine) -> Vec<El> {
    let mut out = Vec::new();
    for r in rings {
        for (i, p) in r.iter().enumerate() {
            let q = m.apply(*p);
            out.push(if i == 0 { El::MoveTo(q.x, q.y) } else { El::LineTo(q.x, q.y) });
        }
        if !r.is_empty() {
            out.push(El::Close);
        }
    }
    out
}

impl<'a> Builder<'a> {
    /// A builder in drawing units (identity map, unit scale) resolving colours through `scene_styles`: the scene
    /// compositor wraps its items in a layer that takes drawing units to output pixels (ADR-0020). `work` starts the
    /// shared work count of the frame.
    pub(crate) fn in_drawing_units(
        doc: &'a Document,
        blobs: Option<&'a tt_persist::container::Blobs>,
        scene_styles: Option<BTreeMap<Id, Id>>,
        work: u64,
    ) -> Self {
        Self {
            doc,
            blobs,
            style: None,
            scene_styles,
            d2p: Affine::IDENTITY,
            scale: 1.0,
            items: Vec::new(),
            warnings: Vec::new(),
            textures: BTreeMap::new(),
            work,
        }
    }

    /// The work units spent so far.
    pub(crate) fn work(&self) -> u64 {
        self.work
    }

    /// Takes the items built so far.
    pub(crate) fn take_items(&mut self) -> Vec<Item> {
        std::mem::take(&mut self.items)
    }

    fn spend(&mut self, n: usize) -> Result<(), CommandError> {
        self.work = self.work.saturating_add(n as u64);
        if self.work > RENDER_WORK_BUDGET {
            return Err(err(
                "limit.exceeded",
                "the drawing is too large to render within the work limit",
                json!({ "limit": RENDER_WORK_BUDGET }),
            ));
        }
        Ok(())
    }

    fn warn(&mut self, code: &str, object: &str, colour: Option<&str>) {
        self.warnings.push(json!({ "code": code, "objectId": object, "colourId": colour }));
    }

    fn texture(&mut self, image: &str) -> Option<Texture> {
        if let Some(t) = self.textures.get(image) {
            return t.clone();
        }
        let t = self
            .blobs
            .and_then(|b| b.read(image, 64 << 20).ok())
            .and_then(|bytes| tt_render::decode_png(&bytes, MAX_TEXTURE_SIDE))
            .map(|(w, h, px)| (w, h, Arc::new(px)));
        self.textures.insert(image.to_owned(), t.clone());
        t
    }

    /// The paint of a colour over `bounds` (drawing space), with its stored mapping matrix.
    fn paint(&mut self, object: &str, colour: &str, matrix: Option<[f64; 6]>, bounds: Option<Rect>) -> Option<Paint> {
        let found = self
            .doc
            .palettes()
            .find_map(|pal| pal.swatches().iter().find(|s| s.colour_id().as_str() == colour).map(|s| (pal, s)));
        let Some((pal, sw)) = found else {
            self.warn("render.colourMissing", object, Some(colour));
            return None;
        };
        let style = match &self.style {
            Some((p, st)) if p == pal.id() => Some(st.clone()),
            _ => self
                .scene_styles
                .as_ref()
                .and_then(|m| m.get(pal.id()).cloned())
                .or_else(|| pal.styles().first().map(|s| s.id().clone())),
        };
        let Some(value) = style.and_then(|st| sw.values().get(&st).cloned()) else {
            self.warn("render.colourMissing", object, Some(colour));
            return None;
        };
        let kind = *sw.kind();
        let unit = |m: Option<[f64; 6]>| -> Option<[f64; 6]> {
            let m = m.or_else(|| bounds.and_then(|b| default_unit(kind, &b)))?;
            Some(self.d2p.then_after(&Affine(m)).0)
        };
        match value {
            SwatchValue::Solid(s) => Some(Paint::Solid(s.rgba.channels())),
            SwatchValue::Gradient(g) => {
                let stops: Vec<(f32, [u8; 4])> =
                    g.stops.iter().map(|st| (st.position as f32, st.rgba.channels())).collect();
                let unit_to_pixel = unit(matrix)?;
                Some(match kind {
                    SwatchKind::RadialGradient => Paint::Radial { stops, unit_to_pixel },
                    _ => Paint::Linear { stops, unit_to_pixel },
                })
            }
            SwatchValue::Texture(t) => {
                let tile = t.tile;
                let unit_to_pixel = unit(matrix)?;
                match self.texture(&t.image) {
                    Some((w, h, rgba)) => Some(Paint::Image { rgba, width: w, height: h, unit_to_pixel, repeat: tile }),
                    None => {
                        self.warn("render.textureMissing", object, Some(colour));
                        None
                    }
                }
            }
        }
    }

    fn layer(
        &mut self,
        n: &DrawingNode,
        zones: Option<&crate::planar::LayerZones>,
        m: &Affine,
    ) -> Result<(), CommandError> {
        let to_px = self.d2p.then_after(m);
        if let Some(lz) = zones {
            for (z, zone) in lz.map.zones.iter().enumerate() {
                let Some(fill) = lz.fills.get(z).and_then(Option::as_ref).filter(|f| f.painted()) else { continue };
                let Some(colour) = fill.colour_id.clone() else { continue };
                let rings = lz.map.zone_rings(z);
                self.spend(rings.iter().map(Vec::len).sum())?;
                let b = Some(zone.bounds);
                if let Some(paint) = self.paint(&zone.id, &colour, fill.matrix, b) {
                    self.items.push(Item::Fill { path: ring_els(&rings, &to_px), rule: Rule::EvenOdd, paint });
                }
            }
        }
        let strokes = n.strokes().as_deref().unwrap_or_default();
        for s in strokes.iter().filter(|s| *s.kind() == StrokeKind::Brush) {
            let Some(colour) = s.colour_id().as_ref().map(|c| c.as_str().to_owned()) else { continue };
            let path = Path::from_points(s.points(), true);
            self.spend(path.anchors.len())?;
            if let Some(paint) = self.paint(s.id().as_str(), &colour, *s.fill_matrix(), vd::stroke_bounds(s)) {
                self.items.push(Item::Fill { path: els(&path, &to_px), rule: Rule::NonZero, paint });
            }
        }
        for s in strokes.iter().filter(|s| *s.kind() == StrokeKind::Pencil) {
            let Some(colour) = s.colour_id().as_ref().map(|c| c.as_str().to_owned()) else { continue };
            let width = s.line_style().as_ref().map_or(0.0, |l| l.thickness) * self.scale;
            if width <= 0.0 {
                continue;
            }
            let path = Path::from_points(s.points(), *s.closed());
            self.spend(path.anchors.len())?;
            if let Some(paint) = self.paint(s.id().as_str(), &colour, *s.fill_matrix(), vd::stroke_bounds(s)) {
                self.items.push(Item::Stroke { path: els(&path, &to_px), width, paint });
            }
        }
        Ok(())
    }

    fn text(&mut self, n: &DrawingNode) -> Result<(), CommandError> {
        let Some(content) = vd::text_content(n) else { return Ok(()) };
        let fixed = (n.width_mode().as_ref().map(|w| format!("{w:?}").to_lowercase()).as_deref() == Some("fixed"))
            .then(|| n.width().unwrap_or(0.0));
        let t = n.transform().map_or(Affine::IDENTITY, Affine);
        let to_px = self.d2p.then_after(&t);
        for g in tt_vector::ops::glyphs::shapes(&content, fixed) {
            let Some(path) = tt_vector::ops::glyphs::bridge_paths(&g.contours) else { continue };
            self.spend(path.anchors.len())?;
            let Some(colour) = g.colour_id.clone() else { continue };
            let b = vd::node_bounds(n, true);
            if let Some(paint) = self.paint(n.id().as_str(), &colour, None, b) {
                self.items.push(Item::Fill { path: els(&path, &to_px), rule: Rule::NonZero, paint });
            }
        }
        Ok(())
    }

    /// A bitmap node (10 §4.9; SI-105): its pixel space (origin top-left, y down) maps to drawing space through
    /// `transform`; the image covers its quad, sampled through the unit square (row 0 at unit y = 0, SI-074), clipped
    /// to the quad. A node whose pixels cannot be read is reported (`render.bitmapMissing`) and not drawn.
    fn bitmap(&mut self, n: &DrawingNode) -> Result<(), CommandError> {
        let (Some(w), Some(h), Some(image)) = (*n.width(), *n.height(), n.image().as_ref()) else {
            self.warn("render.bitmapMissing", n.id().as_str(), None);
            return Ok(());
        };
        let entry = image.entry.clone();
        let decoded = match self.textures.get(&entry) {
            Some(t) => t.clone(),
            None => {
                let t = self
                    .blobs
                    .and_then(|b| b.read(&entry, 256 << 20).ok())
                    .and_then(|bytes| tt_render::decode_png(&bytes, MAX_BITMAP_SIDE))
                    .map(|(bw, bh, px)| (bw, bh, Arc::new(px)));
                self.textures.insert(entry.clone(), t.clone());
                t
            }
        };
        let Some((bw, bh, rgba)) = decoded else {
            self.warn("render.bitmapMissing", n.id().as_str(), None);
            return Ok(());
        };
        self.spend((usize::from(bw) * usize::from(bh)) / 1024 + 4)?;
        let to_px = self.d2p.then_after(&n.transform().map_or(Affine::IDENTITY, Affine));
        let quad = [P::new(0.0, 0.0), P::new(w, 0.0), P::new(w, h), P::new(0.0, h)];
        let mut path: Vec<El> = quad
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let q = to_px.apply(*c);
                if i == 0 { El::MoveTo(q.x, q.y) } else { El::LineTo(q.x, q.y) }
            })
            .collect();
        path.push(El::Close);
        let unit_to_pixel = to_px.then_after(&Affine([w, 0.0, 0.0, h, 0.0, 0.0])).0;
        let paint = Paint::Image { rgba, width: bw, height: bh, unit_to_pixel, repeat: false };
        self.items.push(Item::Fill { path, rule: Rule::NonZero, paint });
        Ok(())
    }

    /// Items of a drawing's nodes, back to front.
    pub(crate) fn nodes(
        &mut self,
        list: &[DrawingNode],
        zones: &BTreeMap<String, crate::planar::LayerZones>,
    ) -> Result<(), CommandError> {
        for n in list {
            match n.kind() {
                NodeKind::Layer => self.layer(n, zones.get(n.id().as_str()), &Affine::IDENTITY)?,
                NodeKind::Group => self.nodes(n.children().as_deref().unwrap_or_default(), zones)?,
                NodeKind::Text => self.text(n)?,
                NodeKind::Bitmap => self.bitmap(n)?,
            }
        }
        Ok(())
    }
}

/// The default unit-space mapping of a gradient or texture over a box (03 §7.2, as painting stores it).
fn default_unit(kind: SwatchKind, b: &Rect) -> Option<[f64; 6]> {
    super::colour::default_matrix(kind, b)
}

fn region_rect(cx: &Cx, d: &Drawing, r: RegionIn) -> Result<Rect, CommandError> {
    let field12 = Rect { min_x: -600.0, min_y: -450.0, max_x: 600.0, max_y: 450.0 };
    Ok(match r {
        RegionIn::Named(RegionName::Bounds) => vd::drawing_bounds(d, false).unwrap_or(field12),
        RegionIn::Named(RegionName::Field12) => field12,
        RegionIn::Named(RegionName::Field16) => Rect { min_x: -800.0, min_y: -600.0, max_x: 800.0, max_y: 600.0 },
        RegionIn::Named(RegionName::Camera) => {
            let doc = cx.document()?;
            let (w, h) = (f64::from(*doc.project().camera_width()), f64::from(*doc.project().camera_height()));
            let half = 600.0 * h / w.max(1.0);
            Rect { min_x: -600.0, min_y: -half, max_x: 600.0, max_y: half }
        }
        RegionIn::Rect(q) => {
            if !(q.x.is_finite() && q.y.is_finite() && q.width.is_finite() && q.height.is_finite())
                || q.width <= 0.0
                || q.height <= 0.0
            {
                return Err(invalid("/region", "the region must have a positive finite size"));
            }
            Rect { min_x: q.x, min_y: q.y, max_x: q.x + q.width, max_y: q.y + q.height }
        }
    })
}

#[command(
    id = "draw.drawing.render",
    precheck = super::draw::precheck_drawing,
    kind = query,
    label = "Render drawing",
    scopes = [read],
    needs_project,
    example(title = "A 256-pixel-wide render of the first drawing's strokes", params = r##"{"drawingId": "drawingBank:#1/drawing:#1", "width": 256, "region": {"x": -60, "y": -60, "width": 170, "height": 170}}"##),
    example(title = "The same area on white with strokes shown", params = r##"{"drawingId": "drawingBank:#1/drawing:#1", "width": 320, "region": {"x": -60, "y": -60, "width": 170, "height": 170}, "padding": 10, "background": "#FFFFFFFF", "showStrokes": true}"##),
    gui = "Drawing inspector thumbnail and agent verification (M4 C4.2)",
)]
pub(crate) fn draw_drawing_render(cx: &mut Cx, p: RenderParams) -> Result<RenderResult, CommandError> {
    let started = std::time::Instant::now();
    if !(1..=tt_render::MAX_SIDE).contains(&p.width) {
        return Err(invalid("/width", "the width must be 1-8192"));
    }
    if p.height.is_some_and(|h| !(1..=tt_render::MAX_SIDE).contains(&h)) {
        return Err(invalid("/height", "the height must be 1-8192"));
    }
    if !(p.padding.is_finite() && p.padding >= 0.0) {
        return Err(invalid("/padding", "the padding must be a non-negative number"));
    }
    let background = tt_model::value::Colour::parse(&p.background)
        .map_err(|_| invalid("/background", "the background is #RRGGBB or #RRGGBBAA"))?
        .channels();
    let mut none = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut none)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    let style = match &p.style_id {
        Some(r) => {
            let key = cx
                .resolve_as(r, ObjectType::PaletteStyle)
                .map_err(|_| invalid("/styleId", format!("no palette style `{r}`")))?;
            let pal = doc
                .palettes()
                .find(|pal| pal.styles().iter().any(|s| s.id().as_str() == key))
                .ok_or_else(|| invalid("/styleId", format!("no palette style `{r}`")))?;
            Some((pal.id().clone(), Id::parse(&key)?))
        }
        None => None,
    };
    let r = region_rect(cx, &d, p.region)?;
    let r = Rect {
        min_x: r.min_x - p.padding,
        min_y: r.min_y - p.padding,
        max_x: r.max_x + p.padding,
        max_y: r.max_y + p.padding,
    };
    let (rw, rh) = ((r.max_x - r.min_x).max(1e-9), (r.max_y - r.min_y).max(1e-9));
    let width = p.width;
    let height = p.height.unwrap_or_else(|| {
        let h = (f64::from(width) * rh / rw).round();
        if h.is_finite() { h.clamp(1.0, f64::from(tt_render::MAX_SIDE)) as u32 } else { 1 }
    });
    let s = (f64::from(width) / rw).min(f64::from(height) / rh);
    let (cxm, cym) = ((r.min_x + r.max_x) / 2.0, (r.min_y + r.max_y) / 2.0);
    let d2p = Affine([s, 0.0, 0.0, -s, f64::from(width) / 2.0 - s * cxm, f64::from(height) / 2.0 + s * cym]);
    let (pixels, warnings) = rasterise(
        cx,
        &doc,
        &d,
        style,
        Raster {
            d2p,
            scale: s,
            width,
            height,
            background,
            show_strokes: p.show_strokes,
            show_selection: p.show_selection,
        },
        started,
    )?;
    let png =
        tt_render::encode_png(&pixels, width, height).map_err(|e| err("render.failed", e.to_string(), json!({})))?;
    let pixels_sha256 = {
        use sha2::Digest;
        sha2::Sha256::digest(&pixels).iter().map(|b| format!("{b:02x}")).collect()
    };
    let rev = *d.rev();
    let (png_out, path_out) = match &p.path {
        Some(path) => {
            let target = crate::persistence::path_param(cx, path, "/path")?;
            if !cx.origin().has_scope(tt_registry::Scope::FilesWrite) {
                return Err(CommandError::with_details(
                    tt_registry::codes::PERMISSION_DENIED,
                    "writing the render to a file needs the scope files.write",
                    json!({ "commandId": "draw.drawing.render", "missingScopes": ["files.write"] }),
                ));
            }
            cx.defer(crate::cx::Effect::WriteFile(Box::new(crate::cx::FileWrite::new(target.clone(), png))));
            (None, Some(target.to_string_lossy().into_owned()))
        }
        None => (Some(tt_persist::b64::encode(&png)), None),
    };
    Ok(RenderResult {
        png: png_out,
        path: path_out,
        width,
        height,
        drawing_to_pixel: d2p.0,
        rev,
        pixels_sha256,
        warnings,
    })
}

/// What [`rasterise`] draws besides the drawing.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Raster {
    /// Drawing space to pixels.
    pub d2p: Affine,
    /// Its linear scale (line widths).
    pub scale: f64,
    /// Image width (px).
    pub width: u32,
    /// Image height (px).
    pub height: u32,
    /// Background RGBA.
    pub background: [u8; 4],
    /// Every stroke's centre path and open ends.
    pub show_strokes: bool,
    /// The selection's bounds.
    pub show_selection: bool,
}

/// Rasterises a drawing through `d2p` (drawing space to pixels, `scale` its linear factor) with the final software
/// rasteriser: the one path `draw.drawing.render` and the headless `inspect.screenshot` share (design note 5, S4).
/// Returns the RGBA pixels and the render warnings.
///
/// # Errors
/// `limit.exceeded` past the time limit, `render.failed`, planar-map errors.
pub(crate) fn rasterise(
    cx: &Cx,
    doc: &Document,
    d: &Drawing,
    style: Option<(Id, Id)>,
    o: Raster,
    started: std::time::Instant,
) -> Result<(Vec<u8>, Vec<Value>), CommandError> {
    let selection = if o.show_selection {
        let ids = super::draw::selection_ids(cx, d);
        super::draw_select::selection_bounds(cx, d, &ids)?
    } else {
        None
    };
    let cache = cx.planar_cache();
    let project = cx.project().ok();
    let never = || false;
    let src = Sources { cache: &cache, blobs: project.map(|p| &p.store.blobs), selection, stop: &never };
    rasterise_with(&src, doc, d, style, o, started)
}

/// What [`rasterise_with`] reads besides the document: owned or shared handles, so the work can run off the document
/// thread (C42A-R1-02).
pub(crate) struct Sources<'a> {
    /// The planar-map cache (shared).
    pub cache: &'a crate::planar::SharedCache,
    /// The project's blob store (bitmaps, textures).
    pub blobs: Option<&'a tt_persist::container::Blobs>,
    /// The selection's bounds, when drawn.
    pub selection: Option<Rect>,
    /// A stop request checked between work slices (a job's cancellation).
    pub stop: &'a dyn Fn() -> bool,
}

/// [`rasterise`] from [`Sources`]: needs no session, so it runs on a job thread.
///
/// # Errors
/// As [`rasterise`]; `job.cancelled` when `stop` holds.
pub(crate) fn rasterise_with(
    src: &Sources<'_>,
    doc: &Document,
    d: &Drawing,
    style: Option<(Id, Id)>,
    o: Raster,
    started: std::time::Instant,
) -> Result<(Vec<u8>, Vec<Value>), CommandError> {
    let Raster { d2p, scale: s, width, height, background, .. } = o;
    let zones: BTreeMap<String, crate::planar::LayerZones> =
        crate::planar::drawing_zones(src.cache, d)?.into_iter().map(|lz| (lz.layer.id.clone(), lz)).collect();
    let mut b = Builder {
        doc,
        blobs: src.blobs,
        style,
        scene_styles: None,
        d2p,
        scale: s,
        items: Vec::new(),
        warnings: Vec::new(),
        textures: BTreeMap::new(),
        work: u64::from(width) * u64::from(height) / 1024,
    };
    b.nodes(d.nodes(), &zones)?;
    if o.show_strokes {
        let magenta = Paint::Solid([255, 0, 255, 255]);
        for o in vd::objects(d).iter().filter(|o| o.kind == vd::ObjectKind::Stroke) {
            let Some(st) = vd::stroke_at(d, &o.pointer) else { continue };
            let closed = *st.closed() || *st.kind() == StrokeKind::Brush;
            let path = Path::from_points(st.points(), closed);
            b.spend(path.anchors.len())?;
            b.items.push(Item::Stroke { path: els(&path, &d2p), width: 1.0, paint: magenta.clone() });
            if !closed {
                for a in [path.anchors.first(), path.anchors.last()].into_iter().flatten() {
                    let q = d2p.apply(a.p);
                    let sq = vec![
                        El::MoveTo(q.x - 2.0, q.y - 2.0),
                        El::LineTo(q.x + 2.0, q.y - 2.0),
                        El::LineTo(q.x + 2.0, q.y + 2.0),
                        El::LineTo(q.x - 2.0, q.y + 2.0),
                        El::Close,
                    ];
                    b.items.push(Item::Fill { path: sq, rule: Rule::NonZero, paint: magenta.clone() });
                }
            }
        }
    }
    if o.show_selection {
        if let Some(sb) = src.selection {
            let (a, c) = (d2p.apply(P::new(sb.min_x, sb.min_y)), d2p.apply(P::new(sb.max_x, sb.max_y)));
            let rect =
                vec![El::MoveTo(a.x, a.y), El::LineTo(c.x, a.y), El::LineTo(c.x, c.y), El::LineTo(a.x, c.y), El::Close];
            b.items.push(Item::Stroke { path: rect, width: 1.0, paint: Paint::Solid([32, 128, 255, 255]) });
        }
    }
    let scene = Scene { width, height, background, items: std::mem::take(&mut b.items) };
    let warnings = std::mem::take(&mut b.warnings);
    drop(b);
    let mut keep_going = |_: usize| {
        if tt_vector::work::cancel_requested() || (src.stop)() {
            Err("the call overran its watchdog budget and was cancelled".to_owned())
        } else if started.elapsed() > RENDER_TIME_LIMIT {
            Err(format!("the render took longer than {} s", RENDER_TIME_LIMIT.as_secs()))
        } else {
            Ok(())
        }
    };
    let pixels = tt_render::render(&scene, &mut keep_going).map_err(|e| match e {
        tt_render::RenderError::Stopped(m) => {
            err("limit.exceeded", m, json!({ "limitSeconds": RENDER_TIME_LIMIT.as_secs() }))
        }
        other => err("render.failed", other.to_string(), json!({})),
    })?;
    Ok((pixels, warnings))
}
