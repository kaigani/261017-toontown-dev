//! Minimal in-house exact-area-coverage rasteriser (**spike code**, C0.3).
//!
//! Algorithm: signed-area accumulation (the "font-rs" family). Every flattened edge adds, per pixel it crosses, the
//! exact signed area it encloses to the right within that pixel and its row; a left-to-right running sum per row
//! then gives each pixel's winding-weighted coverage. Properties that matter for determinism:
//! * single-threaded; every sum runs in a fixed order (edge order, then left to right);
//! * only `+ - * /`, `sqrt`, `floor` and `ceil` (all exactly specified by IEEE 754 / Rust); no libm transcendental,
//!   no fused multiply-add (Rust never contracts `a * b + c` on its own), no fast-math;
//! * geometry in `f64`, coverage accumulation in `f32`, 8-bit coverage and premultiplied RGBA8 output;
//! * compositing is premultiplied source-over **in sRGB-encoded space** with integer arithmetic and exact rounding
//!   (`div255`). Linear-light blending would need `powf` (libm) or a lookup table; sRGB-space blending is what
//!   browsers' 2-D canvas and `vello_cpu`'s u8 pipeline do, so it is the comparable choice. Documented for C0.4.
//!
//! Fill rules: non-zero = `min(|a|, 1)`; even-odd = the triangle wave of `|a|` with period 2. Both are exact where
//! a pixel is crossed by edges of one subpath only; overlapping edges inside one pixel are approximated (as in every
//! accumulation rasteriser). Strokes are converted to polygons (one oriented quad per segment, bevel-join triangles,
//! butt caps) and filled non-zero, so overlaps at joins union instead of double-covering inside the stroke.

use crate::scene::{Affine, FillRule, Item, Paint, Path, Rgba, Scene, Seg};

/// Flattening tolerance in device pixels.
const TOLERANCE: f64 = 0.05;
/// Upper bound of segments per cubic (keeps extreme coordinates bounded).
const MAX_SEGMENTS: usize = 1000;

type Pt = (f64, f64);

fn apply(t: &Affine, (x, y): Pt) -> Pt {
    (t[0] * x + t[2] * y + t[4], t[1] * x + t[3] * y + t[5])
}

fn invert(t: &Affine) -> Option<Affine> {
    let det = t[0] * t[3] - t[1] * t[2];
    if det == 0.0 || !det.is_finite() {
        return None;
    }
    let (a, b, c, d) = (t[3] / det, -t[1] / det, -t[2] / det, t[0] / det);
    Some([a, b, c, d, -(a * t[4] + c * t[5]), -(b * t[4] + d * t[5])])
}

/// Flattens a path (already in device space) into polylines; returns `(points, closed)` per subpath.
fn flatten(path: &Path, t: &Affine) -> Vec<(Vec<Pt>, bool)> {
    let mut out = Vec::new();
    let mut cur: Vec<Pt> = Vec::new();
    let mut start: Pt = (0.0, 0.0);
    let flush = |cur: &mut Vec<Pt>, out: &mut Vec<(Vec<Pt>, bool)>, closed: bool| {
        if cur.len() > 1 {
            out.push((std::mem::take(cur), closed));
        } else {
            cur.clear();
        }
    };
    for seg in &path.0 {
        match *seg {
            Seg::MoveTo(x, y) => {
                flush(&mut cur, &mut out, false);
                start = apply(t, (x, y));
                cur.push(start);
            }
            Seg::LineTo(x, y) => {
                if cur.is_empty() {
                    cur.push(start);
                }
                cur.push(apply(t, (x, y)));
            }
            Seg::CubicTo(x1, y1, x2, y2, x, y) => {
                if cur.is_empty() {
                    cur.push(start);
                }
                let p0 = cur[cur.len() - 1];
                let (p1, p2, p3) = (apply(t, (x1, y1)), apply(t, (x2, y2)), apply(t, (x, y)));
                let ddx = (p0.0 - 2.0 * p1.0 + p2.0).abs().max((p1.0 - 2.0 * p2.0 + p3.0).abs());
                let ddy = (p0.1 - 2.0 * p1.1 + p2.1).abs().max((p1.1 - 2.0 * p2.1 + p3.1).abs());
                let dd = (ddx * ddx + ddy * ddy).sqrt();
                let n = ((0.75 * dd / TOLERANCE).sqrt().ceil()).clamp(1.0, MAX_SEGMENTS as f64) as usize;
                for i in 1..=n {
                    let s = i as f64 / n as f64;
                    let u = 1.0 - s;
                    let (b0, b1, b2, b3) = (u * u * u, 3.0 * u * u * s, 3.0 * u * s * s, s * s * s);
                    cur.push((
                        b0 * p0.0 + b1 * p1.0 + b2 * p2.0 + b3 * p3.0,
                        b0 * p0.1 + b1 * p1.1 + b2 * p2.1 + b3 * p3.1,
                    ));
                }
            }
            Seg::Close => {
                if !cur.is_empty() {
                    flush(&mut cur, &mut out, true);
                }
            }
        }
    }
    flush(&mut cur, &mut out, false);
    out
}

/// Signed-area accumulation buffer, reused across items; only the touched bounding box is read and cleared.
struct Accumulator {
    w: usize,
    h: usize,
    /// `h` rows of `w + 2` cells.
    acc: Vec<f32>,
    /// Touched cells: rows `y0..y1`, columns `x0..x1` (empty when `y0 >= y1`).
    x0: usize,
    x1: usize,
    y0: usize,
    y1: usize,
}

/// 8-bit coverage of a bounding box (`x0..x1`, `y0..y1`), row-major.
struct Coverage {
    x0: usize,
    x1: usize,
    y0: usize,
    y1: usize,
    data: Vec<u8>,
}

impl Accumulator {
    fn new(w: usize, h: usize) -> Self {
        Self { w, h, acc: vec![0.0; (w + 2) * h], x0: usize::MAX, x1: 0, y0: usize::MAX, y1: 0 }
    }

    fn touch(&mut self, y: usize, xa: usize, xb: usize) {
        self.y0 = self.y0.min(y);
        self.y1 = self.y1.max(y + 1);
        self.x0 = self.x0.min(xa);
        self.x1 = self.x1.max(xb + 1);
    }

    /// Adds an edge, clipping it to the canvas: vertically by splitting and discarding, horizontally by splitting and
    /// moving the outside parts onto `x = 0` / `x = w` (which keeps the cover to the right correct).
    fn edge(&mut self, a: Pt, b: Pt) {
        if !(a.0.is_finite() && a.1.is_finite() && b.0.is_finite() && b.1.is_finite()) || a.1 == b.1 {
            return;
        }
        let (h, w) = (self.h as f64, self.w as f64);
        // Vertical clip.
        let (mut p, mut q) = (a, b);
        let lerp_y = |p: Pt, q: Pt, y: f64| (p.0 + (q.0 - p.0) * ((y - p.1) / (q.1 - p.1)), y);
        let (lo, hi) = if p.1 < q.1 { (p.1, q.1) } else { (q.1, p.1) };
        if hi <= 0.0 || lo >= h {
            return;
        }
        if p.1 < 0.0 {
            p = lerp_y(a, b, 0.0);
        } else if p.1 > h {
            p = lerp_y(a, b, h);
        }
        if q.1 < 0.0 {
            q = lerp_y(a, b, 0.0);
        } else if q.1 > h {
            q = lerp_y(a, b, h);
        }
        // Horizontal split at x = 0 and x = w.
        let mut pieces: Vec<(Pt, Pt)> = vec![(p, q)];
        for xs in [0.0, w] {
            let mut next = Vec::with_capacity(pieces.len() + 1);
            for (p, q) in pieces {
                if (p.0 - xs) * (q.0 - xs) < 0.0 {
                    let m = (xs, p.1 + (q.1 - p.1) * ((xs - p.0) / (q.0 - p.0)));
                    next.push((p, m));
                    next.push((m, q));
                } else {
                    next.push((p, q));
                }
            }
            pieces = next;
        }
        for (p, q) in pieces {
            let clamp = |pt: Pt| (pt.0.clamp(0.0, w), pt.1);
            self.line(clamp(p), clamp(q));
        }
    }

    /// Accumulates one edge inside the canvas (font-rs algorithm, f32).
    fn line(&mut self, p0: Pt, p1: Pt) {
        if p0.1 == p1.1 {
            return;
        }
        let (dir, p0, p1) = if p0.1 < p1.1 { (1.0_f32, p0, p1) } else { (-1.0_f32, p1, p0) };
        let (x0, y0, x1, y1) = (p0.0 as f32, p0.1 as f32, p1.0 as f32, p1.1 as f32);
        if y0 == y1 {
            return;
        }
        let dxdy = (x1 - x0) / (y1 - y0);
        let mut x = x0;
        let row_w = self.w + 2;
        let y_start = y0.floor() as usize;
        let y_end = (y1.ceil() as usize).min(self.h);
        for y in y_start..y_end {
            let line = y * row_w;
            let dy = ((y + 1) as f32).min(y1) - (y as f32).max(y0);
            let xnext = x + dxdy * dy;
            let d = dy * dir;
            let (xa, xb) = if x < xnext { (x, xnext) } else { (xnext, x) };
            let xa_floor = xa.floor();
            let xai = xa_floor as usize;
            let xb_ceil = xb.ceil();
            let xbi = xb_ceil as usize;
            self.touch(y, xai, (xai + 1).max(xbi));
            if xbi <= xai + 1 {
                let xmf = 0.5 * (x + xnext) - xa_floor;
                self.acc[line + xai] += d - d * xmf;
                self.acc[line + xai + 1] += d * xmf;
            } else {
                let s = 1.0 / (xb - xa);
                let xaf = xa - xa_floor;
                let a0 = 0.5 * s * (1.0 - xaf) * (1.0 - xaf);
                let xbf = xb - xb_ceil + 1.0;
                let am = 0.5 * s * xbf * xbf;
                self.acc[line + xai] += d * a0;
                if xbi == xai + 2 {
                    self.acc[line + xai + 1] += d * (1.0 - a0 - am);
                } else {
                    let a1 = s * (1.5 - xaf);
                    self.acc[line + xai + 1] += d * (a1 - a0);
                    for xi in xai + 2..xbi - 1 {
                        self.acc[line + xi] += d * s;
                    }
                    let a2 = a1 + (xbi - xai - 3) as f32 * s;
                    self.acc[line + xbi - 1] += d * (1.0 - a2 - am);
                }
                self.acc[line + xbi] += d * am;
            }
            x = xnext;
        }
    }

    /// 8-bit coverage of the touched box; clears the buffer for the next item. Cells left of the box are zero, so the
    /// row sums start at the box's left edge; cells right of it hold the (constant) row sum and are not needed.
    fn take_coverage(&mut self, rule: FillRule) -> Coverage {
        let row_w = self.w + 2;
        let (x0, y0) = (self.x0, self.y0);
        let (x1, y1) = (self.x1.min(self.w), self.y1.min(self.h));
        let mut data = Vec::new();
        if y0 < y1 && x0 < x1 {
            data = vec![0_u8; (x1 - x0) * (y1 - y0)];
            for y in y0..y1 {
                let mut a = 0.0_f32;
                for x in x0..x1 {
                    a += self.acc[y * row_w + x];
                    let v = a.abs();
                    let c = match rule {
                        FillRule::NonZero => v.min(1.0),
                        FillRule::EvenOdd => {
                            let m = v % 2.0;
                            if m > 1.0 { 2.0 - m } else { m }
                        }
                    };
                    data[(y - y0) * (x1 - x0) + (x - x0)] = (c * 255.0 + 0.5) as u8;
                }
            }
        }
        if self.y0 < self.y1 {
            for y in self.y0..self.y1.min(self.h) {
                for cell in &mut self.acc[y * row_w + self.x0.min(row_w)..(y + 1) * row_w] {
                    *cell = 0.0;
                }
            }
        }
        self.x0 = usize::MAX;
        self.x1 = 0;
        self.y0 = usize::MAX;
        self.y1 = 0;
        Coverage { x0, x1, y0, y1, data }
    }
}

fn add_polygon(acc: &mut Accumulator, pts: &[Pt]) {
    for i in 0..pts.len() {
        acc.edge(pts[i], pts[(i + 1) % pts.len()]);
    }
}

fn signed_area(pts: &[Pt]) -> f64 {
    let mut a = 0.0;
    for i in 0..pts.len() {
        let (p, q) = (pts[i], pts[(i + 1) % pts.len()]);
        a += p.0 * q.1 - q.0 * p.1;
    }
    a
}

/// Adds a convex polygon with positive orientation (so overlapping pieces of one stroke union under non-zero).
fn add_oriented(acc: &mut Accumulator, mut pts: Vec<Pt>, t: &Affine) {
    for p in &mut pts {
        *p = apply(t, *p);
    }
    let a = signed_area(&pts);
    if a == 0.0 || !a.is_finite() {
        return;
    }
    if a < 0.0 {
        pts.reverse();
    }
    add_polygon(acc, &pts);
}

fn stroke_into(acc: &mut Accumulator, path: &Path, width: f64, t: &Affine) {
    let hw = width / 2.0;
    // Stroke in user space, then transform the outline (the same semantics as vello_cpu).
    for (pts, closed) in flatten(path, &crate::scene::IDENTITY) {
        let mut segs: Vec<(Pt, Pt, Pt)> = Vec::new(); // (p, q, normal * hw)
        let n = pts.len();
        let count = if closed { n } else { n - 1 };
        for i in 0..count {
            let (p, q) = (pts[i], pts[(i + 1) % n]);
            let (dx, dy) = (q.0 - p.0, q.1 - p.1);
            let len = (dx * dx + dy * dy).sqrt();
            if len == 0.0 {
                continue;
            }
            segs.push((p, q, (-dy / len * hw, dx / len * hw)));
        }
        for &(p, q, nv) in &segs {
            add_oriented(
                acc,
                vec![
                    (p.0 + nv.0, p.1 + nv.1),
                    (q.0 + nv.0, q.1 + nv.1),
                    (q.0 - nv.0, q.1 - nv.1),
                    (p.0 - nv.0, p.1 - nv.1),
                ],
                t,
            );
        }
        let joins = if closed { segs.len() } else { segs.len().saturating_sub(1) };
        for i in 0..joins {
            let (_, v, na) = segs[i];
            let (_, _, nb) = segs[(i + 1) % segs.len()];
            add_oriented(acc, vec![v, (v.0 + na.0, v.1 + na.1), (v.0 + nb.0, v.1 + nb.1)], t);
            add_oriented(acc, vec![v, (v.0 - na.0, v.1 - na.1), (v.0 - nb.0, v.1 - nb.1)], t);
        }
    }
}

fn div255(x: u32) -> u32 {
    (x + 128 + ((x + 128) >> 8)) >> 8
}

fn premultiply(c: Rgba) -> [u32; 4] {
    let a = u32::from(c[3]);
    [div255(u32::from(c[0]) * a), div255(u32::from(c[1]) * a), div255(u32::from(c[2]) * a), a]
}

fn ramp(stops: &[(f32, Rgba)], t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    let (first, last) = match (stops.first(), stops.last()) {
        (Some(f), Some(l)) => (*f, *l),
        _ => return [0, 0, 0, 0],
    };
    if t <= first.0 {
        return first.1;
    }
    for w in stops.windows(2) {
        let ((o0, c0), (o1, c1)) = (w[0], w[1]);
        if t <= o1 {
            let u = if o1 > o0 { (t - o0) / (o1 - o0) } else { 1.0 };
            let mut out = [0_u8; 4];
            for k in 0..4 {
                let v = f32::from(c0[k]) + (f32::from(c1[k]) - f32::from(c0[k])) * u;
                out[k] = (v + 0.5).clamp(0.0, 255.0) as u8;
            }
            return out;
        }
    }
    last.1
}

/// Premultiplied paint colour at the centre of pixel `(x, y)`.
fn shade(paint: &Paint, inverse: &Option<Affine>, x: usize, y: usize) -> [u32; 4] {
    match paint {
        Paint::Solid(c) => premultiply(*c),
        Paint::Linear { p0, p1, stops } => {
            let Some(inv) = inverse else { return [0; 4] };
            let p = apply(inv, (x as f64 + 0.5, y as f64 + 0.5));
            let d = (p1.0 - p0.0, p1.1 - p0.1);
            let dd = d.0 * d.0 + d.1 * d.1;
            let t = if dd > 0.0 { ((p.0 - p0.0) * d.0 + (p.1 - p0.1) * d.1) / dd } else { 0.0 };
            premultiply(ramp(stops, t as f32))
        }
        Paint::Radial { c, r, stops } => {
            let Some(inv) = inverse else { return [0; 4] };
            let p = apply(inv, (x as f64 + 0.5, y as f64 + 0.5));
            let (dx, dy) = (p.0 - c.0, p.1 - c.1);
            let t = if *r > 0.0 { (dx * dx + dy * dy).sqrt() / r } else { 0.0 };
            premultiply(ramp(stops, t as f32))
        }
    }
}

/// A rendered scene: premultiplied RGBA8, row-major, `width * height * 4` bytes.
pub struct Image {
    pub rgba: Vec<u8>,
}

struct Ctx {
    w: usize,
    h: usize,
    pixels: Vec<u8>,
    /// Full-size clip masks, innermost last.
    masks: Vec<Vec<u8>>,
    acc: Accumulator,
}

impl Ctx {
    fn composite(&mut self, cov: &Coverage, paint: &Paint, transform: &Affine) {
        let inverse = invert(transform);
        let mask = self.masks.last();
        let bw = cov.x1.saturating_sub(cov.x0);
        for y in cov.y0..cov.y1 {
            for x in cov.x0..cov.x1 {
                let i = y * self.w + x;
                let mut c = u32::from(cov.data[(y - cov.y0) * bw + (x - cov.x0)]);
                if let Some(m) = mask {
                    c = div255(c * u32::from(m[i]));
                }
                if c == 0 {
                    continue;
                }
                let s = shade(paint, &inverse, x, y);
                let sa = div255(s[3] * c);
                let px = &mut self.pixels[i * 4..i * 4 + 4];
                for k in 0..4 {
                    let sk = div255(s[k] * c);
                    let dk = u32::from(px[k]);
                    px[k] = (sk + div255(dk * (255 - sa))).min(255) as u8;
                }
            }
        }
    }

    fn draw(&mut self, item: &Item) {
        match item {
            Item::Fill { path, paint, rule, transform } => {
                for (pts, _) in flatten(path, transform) {
                    add_polygon(&mut self.acc, &pts);
                }
                let cov = self.acc.take_coverage(*rule);
                self.composite(&cov, paint, transform);
            }
            Item::Stroke { path, paint, width, transform } => {
                stroke_into(&mut self.acc, path, *width, transform);
                let cov = self.acc.take_coverage(FillRule::NonZero);
                self.composite(&cov, paint, transform);
            }
            Item::Clip { path, transform, children } => {
                for (pts, _) in flatten(path, transform) {
                    add_polygon(&mut self.acc, &pts);
                }
                let cov = self.acc.take_coverage(FillRule::NonZero);
                let mut mask = vec![0_u8; self.w * self.h];
                let bw = cov.x1.saturating_sub(cov.x0);
                for y in cov.y0..cov.y1 {
                    for x in cov.x0..cov.x1 {
                        mask[y * self.w + x] = cov.data[(y - cov.y0) * bw + (x - cov.x0)];
                    }
                }
                if let Some(outer) = self.masks.last() {
                    for (m, o) in mask.iter_mut().zip(outer) {
                        *m = div255(u32::from(*m) * u32::from(*o)) as u8;
                    }
                }
                self.masks.push(mask);
                for child in children {
                    self.draw(child);
                }
                self.masks.pop();
            }
        }
    }
}

pub fn render(scene: &Scene) -> Image {
    let (w, h) = (usize::from(scene.width), usize::from(scene.height));
    let bg = premultiply(scene.background);
    let mut pixels = Vec::with_capacity(w * h * 4);
    for _ in 0..w * h {
        pixels.extend(bg.iter().map(|&v| v as u8));
    }
    let mut ctx = Ctx { w, h, pixels, masks: Vec::new(), acc: Accumulator::new(w, h) };
    for item in &scene.items {
        ctx.draw(item);
    }
    Image { rgba: ctx.pixels }
}
