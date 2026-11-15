//! The interactive canvas's render-list product format, TTRD 1.0 (M4 C4.2b; design note 1 §2 and §8; ADR-0007
//! decisions 1, 2 and 4).
//!
//! One drawing at one revision and zoom bucket, tessellated here so the webview only decodes and uploads (ADR-0007
//! question 6). The walk that produces the [`CanvasItem`]s is the final renderer's own (06 §6.1); this module turns
//! its fills and strokes into triangles with the final renderer's rules (even-odd and non-zero fills, round caps and
//! joins) and encodes them. Tessellation is deterministic on every platform: curves are flattened with
//! `tt_vector::geom::flatten_into` (arithmetic and `sqrt` only), round caps and joins come from a unit circle built by
//! repeated rotation with constant factors (no `sin`/`cos`), and `lyon_tessellation` triangulates polygons with
//! arithmetic only. A stroke is the non-zero union of its segment quads and its round caps and joins, so a translucent
//! stroke never blends twice where its pieces overlap (ADR-0007 question 7).
//!
//! # Layout
//!
//! The byte layout, the validation rules V1-V10 (and the order a decoder applies them in), the version rule and the
//! numeric rules are `docs/canvas-format.md` (ruling D-18), the single source this encoder and [`decode`] follow; the
//! TypeScript decoder (`desktop/src/canvas/format.ts`) follows it too.

use lyon_tessellation::geometry_builder::{BuffersBuilder, VertexBuffers};
use lyon_tessellation::math::point;
use lyon_tessellation::path::Path as LyonPath;
use lyon_tessellation::{FillOptions, FillRule, FillTessellator, FillVertex};
use tt_vector::geom::{Cubic, P, flatten_into};

use crate::{El, Item, Paint, Rgba, Rule};

/// The four magic bytes.
pub const MAGIC: [u8; 4] = *b"TTRD";
/// The major version this crate writes and reads.
pub const MAJOR: u16 = 1;
/// The minor version this crate writes.
pub const MINOR: u16 = 0;
/// The header's length.
pub const HEADER_LEN: usize = 80;
/// The record kind of a solid mesh.
pub const KIND_SOLID: u16 = 1;
/// The record kind of a gradient mesh.
pub const KIND_GRADIENT: u16 = 2;
/// The record kind of a bitmap reference.
pub const KIND_BITMAP: u16 = 3;
/// A vertex's bytes.
pub const VERTEX_LEN: usize = 12;
/// Vertex flag: the vertex belongs to a stroke (a pencil line) rather than a fill.
pub const VERTEX_STROKE: u16 = 1;
/// The interactive flattening tolerance at a bucket's top zoom, in view pixels (SI-137).
pub const TOLERANCE_PX: f64 = 0.25;

/// What a colour slot holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotKind {
    /// A solid colour: its RGBA is the colour.
    Solid,
    /// A gradient: the records carry the stops; the RGBA is the first stop.
    Gradient,
    /// A texture or a bitmap: the RGBA is the image's mean colour (SI-137).
    Texture,
}

impl SlotKind {
    fn code(self) -> u8 {
        match self {
            Self::Solid => 0,
            Self::Gradient => 1,
            Self::Texture => 2,
        }
    }

    fn from_code(c: u8) -> Option<Self> {
        match c {
            0 => Some(Self::Solid),
            1 => Some(Self::Gradient),
            2 => Some(Self::Texture),
            _ => None,
        }
    }
}

/// One colour slot: a colour id and its value at the revision (06 §6.3: colours resolve at draw time).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    /// The colour id (or, for a bitmap node, its image entry).
    pub id: String,
    /// What the slot holds.
    pub kind: SlotKind,
    /// Straight-alpha sRGB RGBA8.
    pub rgba: Rgba,
}

/// One item of the walk, in drawing units, with the slot of its colour.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasItem {
    /// The item (a fill or a stroke; a layer's transform is applied to its geometry).
    pub item: Item,
    /// Its colour slot.
    pub slot: u16,
    /// For a bitmap node: its image entry id (drawn as a bitmap reference).
    pub bitmap: Option<String>,
}

/// The identifying header fields of one encoded drawing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    /// The engine generation (D-7b).
    pub generation: u64,
    /// The drawing's revision (10 REV-02).
    pub rev: u64,
    /// The drawing id (26 characters).
    pub drawing_id: String,
    /// The zoom bucket: log2 of the bucket's top zoom.
    pub bucket: i16,
}

/// A vertex of a mesh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    /// Drawing units, x.
    pub x: f32,
    /// Drawing units, y.
    pub y: f32,
    /// The colour slot.
    pub slot: u16,
    /// [`VERTEX_STROKE`] or 0.
    pub flags: u16,
}

/// A decoded or to-be-encoded record.
#[derive(Debug, Clone, PartialEq)]
pub enum Record {
    /// Triangles in solid slot colours.
    Solid {
        /// Vertices.
        vertices: Vec<Vertex>,
        /// Triangle indices.
        indices: Vec<u32>,
    },
    /// Triangles painted with a gradient evaluated per fragment.
    Gradient {
        /// Radial when true, else linear.
        radial: bool,
        /// Drawing → unit space.
        drawing_to_unit: [f32; 6],
        /// Stops `(position, RGBA)`.
        stops: Vec<(f32, Rgba)>,
        /// Vertices.
        vertices: Vec<Vertex>,
        /// Triangle indices.
        indices: Vec<u32>,
    },
    /// A bitmap node's quad.
    Bitmap {
        /// The image's width in pixels.
        width: u16,
        /// The image's height in pixels.
        height: u16,
        /// The quad's corners in drawing units: top-left, top-right, bottom-right, bottom-left of the image.
        quad: [f32; 8],
        /// The image entry id.
        image_id: String,
        /// The slot holding its mean colour.
        slot: u16,
    },
}

/// One drawing in the canvas format.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasDrawing {
    /// Generation, revision, id, bucket.
    pub identity: Identity,
    /// Header flags (0 in 1.0).
    pub flags: u16,
    /// The colour table.
    pub slots: Vec<Slot>,
    /// Records in painter order.
    pub records: Vec<Record>,
    /// Bounds of every vertex (min x, min y, max x, max y); zeros when empty.
    pub bounds: [f32; 4],
}

/// A refusal of the decoder; `rule` names the validation rule of design note 1 §8 (V1 to V10).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{rule}: {message}")]
pub struct FormatError {
    /// `V1` to `V10`.
    pub rule: &'static str,
    /// What was wrong.
    pub message: String,
}

fn bad(rule: &'static str, message: impl Into<String>) -> FormatError {
    FormatError { rule, message: message.into() }
}

/// The flattening tolerance in drawing units for a zoom bucket (SI-137): 0.25 view pixels at the bucket's top zoom.
#[must_use]
pub fn tolerance(bucket: i16) -> f64 {
    TOLERANCE_PX / 2f64.powi(i32::from(bucket.clamp(-8, 8)))
}

/// The zoom bucket of a zoom factor: the smallest power of two at or above it, as its exponent, within ±8.
#[must_use]
pub fn bucket_of(zoom: f64) -> i16 {
    if !(zoom.is_finite() && zoom > 0.0) {
        return 0;
    }
    let mut b: i16 = -8;
    let mut top = 1.0 / 256.0;
    while top < zoom && b < 8 {
        b += 1;
        top *= 2.0;
    }
    b
}

// ---------------------------------------------------------------------------------------------------------------
// Tessellation
// ---------------------------------------------------------------------------------------------------------------

/// cos and sin of 2π/256, the unit circle's step (constants, so no platform `sin`/`cos` is called).
const STEP_COS: f64 = 0.999_698_818_696_204_2;
const STEP_SIN: f64 = 0.024_541_228_522_912_288;
/// Points on the unit circle at 256 equal steps, counter-clockwise from (1, 0), by repeated rotation.
fn unit_circle() -> &'static [P; 256] {
    static CIRCLE: std::sync::OnceLock<[P; 256]> = std::sync::OnceLock::new();
    CIRCLE.get_or_init(|| {
        let mut out = [P::new(0.0, 0.0); 256];
        let mut p = P::new(1.0, 0.0);
        for slot in &mut out {
            *slot = p;
            p = P::new(p.x * STEP_COS - p.y * STEP_SIN, p.x * STEP_SIN + p.y * STEP_COS);
            // Renormalise so the rounding of 256 rotations never drifts off the circle.
            let l = p.len();
            if l > 0.0 {
                p = P::new(p.x / l, p.y / l);
            }
        }
        out
    })
}

/// A circle of radius `r` around `c` whose chords deviate by at most `tol`, counter-clockwise.
fn circle(c: P, r: f64, tol: f64) -> Vec<P> {
    // The chord error of k equal pieces is r (1 − cos(π / k)) ≈ r π² / (2 k²).
    let mut k = 8usize;
    while k < 256 && r * 4.934_802_200_544_679 / ((k * k) as f64) > tol {
        k *= 2;
    }
    let step = 256 / k;
    unit_circle().iter().step_by(step).map(|u| P::new(c.x + u.x * r, c.y + u.y * r)).collect()
}

/// Polylines (each with a closed flag) of a path, curves flattened to `tol`.
fn polylines(path: &[El], tol: f64) -> Vec<(Vec<P>, bool)> {
    let mut out: Vec<(Vec<P>, bool)> = Vec::new();
    let mut cur: Vec<P> = Vec::new();
    for el in path {
        match *el {
            El::MoveTo(x, y) => {
                if cur.len() > 1 {
                    out.push((std::mem::take(&mut cur), false));
                }
                cur.clear();
                cur.push(P::new(x, y));
            }
            El::LineTo(x, y) => cur.push(P::new(x, y)),
            El::CubicTo(a, b, c, d, e, f) => {
                let p0 = cur.last().copied().unwrap_or(P::new(e, f));
                if cur.is_empty() {
                    cur.push(p0);
                }
                let cubic = Cubic { p0, p1: P::new(a, b), p2: P::new(c, d), p3: P::new(e, f) };
                flatten_into(&cubic, tol, &mut cur);
            }
            El::Close => {
                if !cur.is_empty() {
                    out.push((std::mem::take(&mut cur), true));
                }
            }
        }
    }
    if cur.len() > 1 {
        out.push((cur, false));
    }
    out
}

fn lyon_path(rings: &[Vec<P>]) -> Option<LyonPath> {
    let mut b = LyonPath::builder();
    let mut any = false;
    for r in rings {
        let mut pts = r.iter().filter(|p| p.is_finite());
        let Some(first) = pts.next() else { continue };
        b.begin(point(first.x as f32, first.y as f32));
        let mut n = 1;
        for p in pts {
            b.line_to(point(p.x as f32, p.y as f32));
            n += 1;
        }
        b.end(true);
        any |= n >= 3;
    }
    any.then(|| b.build())
}

/// Triangles of `rings` under `rule`, appended to `out` with `slot` and `flags`.
fn fill_rings(rings: &[Vec<P>], rule: Rule, tol: f64, slot: u16, flags: u16, out: &mut Mesh) -> bool {
    let Some(path) = lyon_path(rings) else { return false };
    let mut buffers: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    let options = FillOptions::tolerance(tol as f32).with_fill_rule(match rule {
        Rule::NonZero => FillRule::NonZero,
        Rule::EvenOdd => FillRule::EvenOdd,
    });
    let mut tess = FillTessellator::new();
    let built = tess.tessellate_path(
        &path,
        &options,
        &mut BuffersBuilder::new(&mut buffers, |v: FillVertex| v.position().to_array()),
    );
    if built.is_err() || buffers.indices.is_empty() {
        return false;
    }
    out.push(&buffers.vertices, &buffers.indices, slot, flags)
}

/// The closed pieces whose non-zero union is a round-capped, round-joined stroke of `line` (width `w`); without
/// `joins`, only the caps are round (`display.fastPencilRendering`: approximate pencil strokes, 01 §7.2).
fn stroke_pieces(line: &[P], closed: bool, w: f64, tol: f64, joins: bool) -> Vec<Vec<P>> {
    let r = w / 2.0;
    let mut pts: Vec<P> = Vec::with_capacity(line.len());
    for p in line.iter().copied().filter(|p| p.is_finite()) {
        if pts.last().is_none_or(|q: &P| q.dist(p) > 0.0) {
            pts.push(p);
        }
    }
    if closed && pts.len() > 1 && pts.first().zip(pts.last()).is_some_and(|(a, b)| a.dist(*b) == 0.0) {
        pts.pop();
    }
    let mut pieces = Vec::new();
    let Some(&first) = pts.first() else { return pieces };
    if pts.len() == 1 {
        pieces.push(circle(first, r, tol));
        return pieces;
    }
    let n = pts.len();
    let segs = if closed { n } else { n - 1 };
    let ccw = |mut q: Vec<P>| {
        if tt_vector::geom::signed_area(&q) < 0.0 {
            q.reverse();
        }
        q
    };
    for i in 0..segs {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        let Some(d) = (b - a).unit() else { continue };
        let nrm = d.perp() * r;
        pieces.push(ccw(vec![a + nrm, b + nrm, b - nrm, a - nrm]));
    }
    // Round joins where the turn leaves a gap wider than the tolerance, and round caps at both ends of an open line.
    for i in 0..n {
        let is_end = !closed && (i == 0 || i == n - 1);
        let needs = is_end
            || joins && {
                let prev = pts[(i + n - 1) % n];
                let next = pts[(i + 1) % n];
                match ((pts[i] - prev).unit(), (next - pts[i]).unit()) {
                    // The outer gap of a turn is r (1 − cos(θ / 2)) = r (1 − sqrt((1 + cos θ) / 2)).
                    (Some(u), Some(v)) => r * (1.0 - ((1.0 + u.dot(v)).max(0.0) / 2.0).sqrt()) > tol,
                    _ => true,
                }
            };
        if needs {
            pieces.push(circle(pts[i], r, tol));
        }
    }
    pieces
}

/// Triangles collected for one record.
#[derive(Debug, Default)]
struct Mesh {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl Mesh {
    fn push(&mut self, verts: &[[f32; 2]], idx: &[u32], slot: u16, flags: u16) -> bool {
        let Ok(base) = u32::try_from(self.vertices.len()) else { return false };
        if verts.iter().any(|v| !(v[0].is_finite() && v[1].is_finite())) {
            return false;
        }
        self.vertices.extend(verts.iter().map(|v| Vertex { x: v[0], y: v[1], slot, flags }));
        self.indices.extend(idx.iter().map(|i| base.saturating_add(*i)));
        true
    }

    fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }
}

fn apply(m: &[f64; 6], p: P) -> P {
    P::new(m[0] * p.x + m[2] * p.y + m[4], m[1] * p.x + m[3] * p.y + m[5])
}

fn compose(outer: &[f64; 6], inner: &[f64; 6]) -> [f64; 6] {
    [
        outer[0] * inner[0] + outer[2] * inner[1],
        outer[1] * inner[0] + outer[3] * inner[1],
        outer[0] * inner[2] + outer[2] * inner[3],
        outer[1] * inner[2] + outer[3] * inner[3],
        outer[0] * inner[4] + outer[2] * inner[5] + outer[4],
        outer[1] * inner[4] + outer[3] * inner[5] + outer[5],
    ]
}

fn invert(m: &[f64; 6]) -> Option<[f64; 6]> {
    let det = m[0] * m[3] - m[1] * m[2];
    if !(det.is_finite() && det.abs() > 1e-300) {
        return None;
    }
    let (a, b, c, d) = (m[3] / det, -m[1] / det, -m[2] / det, m[0] / det);
    Some([a, b, c, d, -(a * m[4] + c * m[5]), -(b * m[4] + d * m[5])])
}

fn f32x6(m: &[f64; 6]) -> [f32; 6] {
    m.map(|v| v as f32)
}

/// Tessellates the walk's items (back to front) into records in painter order: consecutive solid items share one
/// solid mesh (one draw); a gradient or a bitmap starts its own record.
#[must_use]
pub fn tessellate(items: &[CanvasItem], bucket: i16) -> Vec<Record> {
    tessellate_with(items, bucket, true)
}

/// [`tessellate`], with stroke joins round (`joins`) or not (approximate pencil strokes: only the caps are round).
#[must_use]
pub fn tessellate_with(items: &[CanvasItem], bucket: i16, joins: bool) -> Vec<Record> {
    let tol = tolerance(bucket);
    let mut records = Vec::new();
    let mut solid = Mesh::default();
    let mut flush = |solid: &mut Mesh, records: &mut Vec<Record>| {
        if !solid.is_empty() {
            let m = std::mem::take(solid);
            records.push(Record::Solid { vertices: m.vertices, indices: m.indices });
        }
    };
    let mut stack: Vec<([f64; 6], &CanvasItem, &Item)> = items.iter().rev().map(|c| (IDENTITY, c, &c.item)).collect();
    while let Some((m, ci, item)) = stack.pop() {
        match item {
            Item::Layer { transform, items: inner } => {
                let mm = compose(&m, transform);
                for it in inner.iter().rev() {
                    stack.push((mm, ci, it));
                }
            }
            Item::Fill { path, rule, paint } => {
                // Curves are flattened in the item's own coordinates, to the tolerance that maps to `tol` here.
                let Some(local) = local_tolerance(&m, tol) else { continue };
                let rings: Vec<Vec<P>> = polylines(path, local)
                    .into_iter()
                    .map(|(l, _)| l.into_iter().map(|p| apply(&m, p)).collect())
                    .collect();
                emit(&mut records, &mut solid, &mut flush, ci, paint, &m, |mesh| {
                    fill_rings(&rings, *rule, tol, ci.slot, 0, mesh)
                });
            }
            Item::Stroke { path, width, paint } => {
                // The final renderer strokes in the item's own coordinates and draws the affine image of that round
                // stroke (C42B-R1-01): the pieces are built there, with the tolerance that maps to `tol`, and their
                // outlines transformed. Under a non-uniform scale the round caps and joins become ellipses.
                if !(width.is_finite() && *width > 0.0) {
                    continue;
                }
                let Some(local) = local_tolerance(&m, tol) else { continue };
                let mut pieces = Vec::new();
                for (line, closed) in polylines(path, local) {
                    for piece in stroke_pieces(&line, closed, *width, local, joins) {
                        pieces.push(piece.into_iter().map(|p| apply(&m, p)).collect());
                    }
                }
                emit(&mut records, &mut solid, &mut flush, ci, paint, &m, |mesh| {
                    fill_rings(&pieces, Rule::NonZero, tol, ci.slot, VERTEX_STROKE, mesh)
                });
            }
        }
    }
    flush(&mut solid, &mut records);
    records
}

const IDENTITY: [f64; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// The flattening tolerance in an item's own coordinates that keeps every chord within `tol` after `m`: `tol` divided by
/// `m`'s largest singular value (its largest stretch; exactly 1 for the identity and for rotations), or `None` when `m`
/// collapses the plane (nothing to draw).
fn local_tolerance(m: &[f64; 6], tol: f64) -> Option<f64> {
    let (a, b, c, d) = (m[0], m[1], m[2], m[3]);
    let sum = a * a + b * b + c * c + d * d;
    let det = a * d - b * c;
    let stretch = ((sum + (sum * sum - 4.0 * det * det).max(0.0).sqrt()) / 2.0).sqrt();
    let local = tol / stretch;
    (det.is_finite() && det != 0.0 && local.is_finite() && local > 0.0).then_some(local)
}

/// Adds one item's triangles to the right record.
fn emit(
    records: &mut Vec<Record>,
    solid: &mut Mesh,
    flush: &mut impl FnMut(&mut Mesh, &mut Vec<Record>),
    ci: &CanvasItem,
    paint: &Paint,
    m: &[f64; 6],
    build: impl FnOnce(&mut Mesh) -> bool,
) {
    match (paint, &ci.bitmap) {
        (Paint::Image { width, height, unit_to_pixel, .. }, Some(image)) => {
            let u2d = compose(m, unit_to_pixel);
            let corners = [P::new(0.0, 0.0), P::new(1.0, 0.0), P::new(1.0, 1.0), P::new(0.0, 1.0)];
            let mut quad = [0f32; 8];
            for (i, c) in corners.iter().enumerate() {
                let q = apply(&u2d, *c);
                quad[2 * i] = q.x as f32;
                quad[2 * i + 1] = q.y as f32;
            }
            if quad.iter().all(|v| v.is_finite()) {
                flush(solid, records);
                records.push(Record::Bitmap {
                    width: *width,
                    height: *height,
                    quad,
                    image_id: image.clone(),
                    slot: ci.slot,
                });
            }
        }
        (Paint::Linear { stops, unit_to_pixel } | Paint::Radial { stops, unit_to_pixel }, _) => {
            let Some(d2u) = invert(&compose(m, unit_to_pixel)) else {
                // A degenerate mapping paints the first stop, as a solid.
                build(solid);
                return;
            };
            let mut mesh = Mesh::default();
            if build(&mut mesh) {
                flush(solid, records);
                records.push(Record::Gradient {
                    radial: matches!(paint, Paint::Radial { .. }),
                    drawing_to_unit: f32x6(&d2u),
                    stops: stops.iter().take(255).copied().collect(),
                    vertices: mesh.vertices,
                    indices: mesh.indices,
                });
            }
        }
        // Solid colours, and texture fills drawn with their mean colour (SI-137).
        _ => {
            build(solid);
        }
    }
}

/// The bounds of every vertex and bitmap corner.
#[must_use]
pub fn bounds(records: &[Record]) -> [f32; 4] {
    let mut b = [f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY];
    let mut add = |x: f32, y: f32| {
        b[0] = b[0].min(x);
        b[1] = b[1].min(y);
        b[2] = b[2].max(x);
        b[3] = b[3].max(y);
    };
    for r in records {
        match r {
            Record::Solid { vertices, .. } | Record::Gradient { vertices, .. } => {
                vertices.iter().for_each(|v| add(v.x, v.y));
            }
            Record::Bitmap { quad, .. } => quad.as_chunks::<2>().0.iter().for_each(|c| add(c[0], c[1])),
        }
    }
    if b[0].is_finite() { b } else { [0.0; 4] }
}

// ---------------------------------------------------------------------------------------------------------------
// Encoding
// ---------------------------------------------------------------------------------------------------------------

fn pad4(out: &mut Vec<u8>) {
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

fn padded(n: usize) -> usize {
    n.div_ceil(4) * 4
}

/// The byte length of a record.
fn record_len(r: &Record) -> usize {
    let mesh = |v: &[Vertex], i: &[u32]| 8 + v.len() * VERTEX_LEN + i.len() * 4;
    8 + match r {
        Record::Solid { vertices, indices } => mesh(vertices, indices),
        Record::Gradient { stops, vertices, indices, .. } => 4 + 24 + stops.len() * 8 + mesh(vertices, indices),
        Record::Bitmap { image_id, .. } => 4 + 32 + 4 + padded(image_id.len()) + 4,
    }
}

/// A refusal of the encoder: the drawing does not fit the format's fields.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the drawing does not fit the canvas format: {0}")]
pub struct EncodeError(pub String);

/// Encodes one drawing. The drawing id must be 26 characters; counts must fit their fields.
///
/// # Errors
/// [`EncodeError`] when a field does not fit.
pub fn encode(d: &CanvasDrawing) -> Result<Vec<u8>, EncodeError> {
    let id = d.identity.drawing_id.as_bytes();
    if id.len() != 26 {
        return Err(EncodeError(format!("drawing id of {} bytes", id.len())));
    }
    let fit = |n: usize, what: &str| u32::try_from(n).map_err(|_| EncodeError(format!("{what}: {n}")));
    let mut out = Vec::with_capacity(HEADER_LEN + d.records.iter().map(record_len).sum::<usize>());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&MAJOR.to_le_bytes());
    out.extend_from_slice(&MINOR.to_le_bytes());
    out.extend_from_slice(&(HEADER_LEN as u16).to_le_bytes());
    out.extend_from_slice(&d.identity.bucket.to_le_bytes());
    out.extend_from_slice(&fit(d.slots.len(), "colour slots")?.to_le_bytes());
    out.extend_from_slice(&d.identity.generation.to_le_bytes());
    out.extend_from_slice(&d.identity.rev.to_le_bytes());
    out.extend_from_slice(id);
    out.extend_from_slice(&d.flags.to_le_bytes());
    out.extend_from_slice(&fit(d.records.len(), "records")?.to_le_bytes());
    for v in d.bounds {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for s in &d.slots {
        let len = u16::try_from(s.id.len()).map_err(|_| EncodeError("colour id length".into()))?;
        out.extend_from_slice(&len.to_le_bytes());
        out.push(s.kind.code());
        out.push(0);
        out.extend_from_slice(&s.rgba);
        out.extend_from_slice(s.id.as_bytes());
        pad4(&mut out);
    }
    let mesh = |out: &mut Vec<u8>, vertices: &[Vertex], indices: &[u32]| -> Result<(), EncodeError> {
        out.extend_from_slice(&fit(vertices.len(), "vertices")?.to_le_bytes());
        out.extend_from_slice(&fit(indices.len(), "indices")?.to_le_bytes());
        for v in vertices {
            out.extend_from_slice(&v.x.to_le_bytes());
            out.extend_from_slice(&v.y.to_le_bytes());
            out.extend_from_slice(&v.slot.to_le_bytes());
            out.extend_from_slice(&v.flags.to_le_bytes());
        }
        for i in indices {
            out.extend_from_slice(&i.to_le_bytes());
        }
        Ok(())
    };
    for r in &d.records {
        let start = out.len();
        let (kind, len) = match r {
            Record::Solid { .. } => (KIND_SOLID, record_len(r)),
            Record::Gradient { .. } => (KIND_GRADIENT, record_len(r)),
            Record::Bitmap { .. } => (KIND_BITMAP, record_len(r)),
        };
        out.extend_from_slice(&kind.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&fit(len, "record length")?.to_le_bytes());
        match r {
            Record::Solid { vertices, indices } => mesh(&mut out, vertices, indices)?,
            Record::Gradient { radial, drawing_to_unit, stops, vertices, indices } => {
                out.push(u8::from(*radial));
                out.push(u8::try_from(stops.len()).map_err(|_| EncodeError("gradient stops".into()))?);
                out.extend_from_slice(&0u16.to_le_bytes());
                for v in drawing_to_unit {
                    out.extend_from_slice(&v.to_le_bytes());
                }
                for (pos, rgba) in stops {
                    out.extend_from_slice(&pos.to_le_bytes());
                    out.extend_from_slice(rgba);
                }
                mesh(&mut out, vertices, indices)?;
            }
            Record::Bitmap { width, height, quad, image_id, slot } => {
                out.extend_from_slice(&width.to_le_bytes());
                out.extend_from_slice(&height.to_le_bytes());
                for v in quad {
                    out.extend_from_slice(&v.to_le_bytes());
                }
                let len = u16::try_from(image_id.len()).map_err(|_| EncodeError("image id length".into()))?;
                out.extend_from_slice(&len.to_le_bytes());
                out.extend_from_slice(&slot.to_le_bytes());
                out.extend_from_slice(image_id.as_bytes());
                pad4(&mut out);
                out.extend_from_slice(&0u32.to_le_bytes());
            }
        }
        debug_assert_eq!(out.len() - start, len);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------------------------
// Decoding (the same rules as the TypeScript decoder; used by the tests and the golden corpus)
// ---------------------------------------------------------------------------------------------------------------

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
    end: usize,
    /// The rule an overrun breaks: V5 for the payload, V4 inside a record (its length against its counts).
    overrun: &'static str,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize, what: &str) -> Result<&'a [u8], FormatError> {
        let stop = self.at.checked_add(n).filter(|s| *s <= self.end).ok_or_else(|| {
            bad(self.overrun, format!("{what} at byte {} needs {n} bytes; the section ends at {}", self.at, self.end))
        })?;
        let s = &self.b[self.at..stop];
        self.at = stop;
        Ok(s)
    }
    fn u8(&mut self, w: &str) -> Result<u8, FormatError> {
        Ok(self.take(1, w)?[0])
    }
    fn u16(&mut self, w: &str) -> Result<u16, FormatError> {
        let s = self.take(2, w)?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }
    fn i16(&mut self, w: &str) -> Result<i16, FormatError> {
        let s = self.take(2, w)?;
        Ok(i16::from_le_bytes([s[0], s[1]]))
    }
    fn u32(&mut self, w: &str) -> Result<u32, FormatError> {
        let s = self.take(4, w)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn u64(&mut self, w: &str) -> Result<u64, FormatError> {
        let s = self.take(8, w)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(s);
        Ok(u64::from_le_bytes(a))
    }
    fn f32(&mut self, w: &str) -> Result<f32, FormatError> {
        let v = f32::from_bits(self.u32(w)?);
        if v.is_finite() { Ok(v) } else { Err(bad("V9", format!("{w}: a non-finite number"))) }
    }
    fn pad(&mut self, n: usize, w: &str) -> Result<(), FormatError> {
        self.take(padded(n) - n, w).map(|_| ())
    }
}

fn is_ulid(id: &[u8]) -> bool {
    id.len() == 26
        && id
            .iter()
            .all(|c| matches!(c, b'0'..=b'9' | b'A'..=b'H' | b'J' | b'K' | b'M' | b'N' | b'P'..=b'T' | b'V'..=b'Z'))
}

/// Decodes and validates one payload (design note 1 §8, V1 to V10), refusing before building any geometry the caller
/// could draw.
///
/// # Errors
/// [`FormatError`] naming the first rule the payload breaks.
pub fn decode(b: &[u8]) -> Result<CanvasDrawing, FormatError> {
    if b.len() < 4 || b[..4] != MAGIC {
        return Err(bad("V1", "not a TTRD payload (magic)"));
    }
    if b.len() < 8 {
        return Err(bad("V3", format!("a payload of {} bytes is shorter than the header", b.len())));
    }
    let major = u16::from_le_bytes([b[4], b[5]]);
    if major != MAJOR {
        return Err(bad("V2", format!("major version {major}; this decoder reads {MAJOR}")));
    }
    if b.len() < HEADER_LEN {
        return Err(bad("V3", format!("a payload of {} bytes is shorter than the {HEADER_LEN}-byte header", b.len())));
    }
    let header_len = u16::from_le_bytes([b[8], b[9]]);
    if usize::from(header_len) != HEADER_LEN {
        return Err(bad("V3", format!("header length {header_len}; 1.x headers are {HEADER_LEN} bytes")));
    }
    let mut r = Reader { b, at: 10, end: b.len(), overrun: "V5" };
    let bucket = r.i16("bucket")?;
    let slot_count = r.u32("slot count")?;
    let generation = r.u64("generation")?;
    let rev = r.u64("revision")?;
    let id = r.take(26, "drawing id")?;
    if !is_ulid(id) {
        return Err(bad("V10", "the drawing id is not 26 Crockford base-32 characters"));
    }
    let drawing_id = String::from_utf8_lossy(id).into_owned();
    let flags = r.u16("flags")?;
    let record_count = r.u32("record count")?;
    let mut bounds = [0f32; 4];
    for v in &mut bounds {
        *v = r.f32("bounds")?;
    }
    let mut slots = Vec::new();
    for _ in 0..slot_count {
        let len = usize::from(r.u16("colour id length")?);
        let kind = SlotKind::from_code(r.u8("slot kind")?).ok_or_else(|| bad("V8", "an unknown colour slot kind"))?;
        let _ = r.u8("reserved")?;
        let c = r.take(4, "RGBA")?;
        let id = String::from_utf8_lossy(r.take(len, "colour id")?).into_owned();
        r.pad(len, "colour id padding")?;
        slots.push(Slot { id, kind, rgba: [c[0], c[1], c[2], c[3]] });
    }
    let slot_ok = |s: u16| -> Result<u16, FormatError> {
        if u32::from(s) < slot_count { Ok(s) } else { Err(bad("V7", format!("colour slot {s} of {slot_count}"))) }
    };
    let mut records = Vec::new();
    for n in 0..record_count {
        let start = r.at;
        let kind = r.u16("record kind")?;
        let _flags = r.u16("record flags")?;
        let len = r.u32("record length")? as usize;
        if len < 8 || start.checked_add(len).is_none_or(|e| e > b.len()) {
            return Err(bad("V5", format!("record {n} of {len} bytes overruns the payload")));
        }
        let mut rr = Reader { b, at: r.at, end: start + len, overrun: "V4" };
        let mesh = |rr: &mut Reader<'_>| -> Result<(Vec<Vertex>, Vec<u32>), FormatError> {
            let vc = rr.u32("vertex count")? as usize;
            let ic = rr.u32("index count")? as usize;
            let need = vc.checked_mul(VERTEX_LEN).and_then(|v| ic.checked_mul(4).and_then(|i| v.checked_add(i)));
            if need != Some(rr.end - rr.at) {
                return Err(bad("V4", format!("record {n}: {vc} vertices and {ic} indices do not fill its length")));
            }
            if !ic.is_multiple_of(3) {
                return Err(bad("V4", format!("record {n}: {ic} indices are not whole triangles")));
            }
            let mut vs = Vec::with_capacity(vc);
            for _ in 0..vc {
                let x = rr.f32("vertex x")?;
                let y = rr.f32("vertex y")?;
                let slot = slot_ok(rr.u16("vertex slot")?)?;
                let flags = rr.u16("vertex flags")?;
                vs.push(Vertex { x, y, slot, flags });
            }
            let mut is = Vec::with_capacity(ic);
            for _ in 0..ic {
                let i = rr.u32("index")?;
                if i as usize >= vc {
                    return Err(bad("V6", format!("record {n}: index {i} of {vc} vertices")));
                }
                is.push(i);
            }
            Ok((vs, is))
        };
        let rec = match kind {
            KIND_SOLID => {
                let (vertices, indices) = mesh(&mut rr)?;
                Record::Solid { vertices, indices }
            }
            KIND_GRADIENT => {
                let radial = match rr.u8("gradient kind")? {
                    0 => false,
                    1 => true,
                    other => return Err(bad("V8", format!("record {n}: unknown gradient kind {other}"))),
                };
                let stop_count = usize::from(rr.u8("stop count")?);
                let _ = rr.u16("reserved")?;
                let mut m = [0f32; 6];
                for v in &mut m {
                    *v = rr.f32("gradient matrix")?;
                }
                let mut stops = Vec::with_capacity(stop_count);
                for _ in 0..stop_count {
                    let p = rr.f32("stop position")?;
                    let c = rr.take(4, "stop RGBA")?;
                    stops.push((p, [c[0], c[1], c[2], c[3]]));
                }
                let (vertices, indices) = mesh(&mut rr)?;
                Record::Gradient { radial, drawing_to_unit: m, stops, vertices, indices }
            }
            KIND_BITMAP => {
                let width = rr.u16("bitmap width")?;
                let height = rr.u16("bitmap height")?;
                let mut quad = [0f32; 8];
                for v in &mut quad {
                    *v = rr.f32("bitmap quad")?;
                }
                let idl = usize::from(rr.u16("image id length")?);
                let slot = slot_ok(rr.u16("bitmap slot")?)?;
                let image_id = String::from_utf8_lossy(rr.take(idl, "image id")?).into_owned();
                rr.pad(idl, "image id padding")?;
                let _ = rr.u32("reserved")?;
                if rr.at != rr.end {
                    return Err(bad("V4", format!("record {n}: a bitmap reference of the wrong length")));
                }
                Record::Bitmap { width, height, quad, image_id, slot }
            }
            other => return Err(bad("V8", format!("record {n}: unknown kind {other}"))),
        };
        r.at = start + len;
        records.push(rec);
    }
    if r.at != b.len() {
        return Err(bad("V5", format!("{} bytes after the last record", b.len() - r.at)));
    }
    Ok(CanvasDrawing { identity: Identity { generation, rev, drawing_id, bucket }, flags, slots, records, bounds })
}
