//! Transforms of drawing content (02 §8.13, §7.1.1, §7.1.3, §7.1.4; M2 C2.3). Pure functions over the persisted JSON
//! of strokes and nodes (10 §4.9), so the executor stages exactly what they return.
//!
//! - **Affine maps** move anchors and their handle offsets (linear part), compose with fill matrices (stroke
//!   `fillMatrix`, side fills, brush exterior records) and with text and bitmap `transform`s, so gradients, textures,
//!   text and images stay attached (02 §8.13).
//! - **Reflections** (negative determinant) reverse the sense of every stroke's left and right, so side fills and
//!   exterior records swap sides and each zone keeps its fill.
//! - **Envelopes** (02 §7.1.3) map the bounds' corners onto four targets, projectively or bilinearly; control points
//!   are mapped as points. Matrices, text and bitmaps take the affine map through three of the corners (SI-070).
//! - **Brush spines** (SI-054, SI-070): a rigid motion (translation, rotation, reflection) moves the spine with the
//!   outline and turns the tip angle, so pen and pressure survive; any other map (scale, skew, envelope) changes the
//!   outline in a way a re-sweep with the same pen would not reproduce, so the outline becomes authoritative and the
//!   spine is dropped, as for any direct outline edit (02 §3.2).
//!
//! Trigonometry uses the soft-float `libm` (ADR-0004 rule 3); quarter and half turns are exact.

use serde_json::{Value, json};

use crate::geom::{Affine, P};

/// Rotation by `deg` degrees counter-clockwise about the origin (exact for multiples of 90°).
#[must_use]
pub fn rotation(deg: f64) -> Affine {
    let q = deg.rem_euclid(360.0);
    let (c, s) = if q == 0.0 {
        (1.0, 0.0)
    } else if q == 90.0 {
        (0.0, 1.0)
    } else if q == 180.0 {
        (-1.0, 0.0)
    } else if q == 270.0 {
        (0.0, -1.0)
    } else {
        let r = deg.to_radians();
        (libm::cos(r), libm::sin(r))
    };
    Affine([c, s, -s, c, 0.0, 0.0])
}

/// Skew by `x`, `y` degrees (x shear from the y axis, y shear from the x axis).
#[must_use]
pub fn skew(x_deg: f64, y_deg: f64) -> Affine {
    Affine([1.0, libm::tan(y_deg.to_radians()), libm::tan(x_deg.to_radians()), 1.0, 0.0, 0.0])
}

/// Scale about the origin.
#[must_use]
pub fn scale(sx: f64, sy: f64) -> Affine {
    Affine([sx, 0.0, 0.0, sy, 0.0, 0.0])
}

/// `m` about `pivot`: translate the pivot to the origin, apply `m`, translate back.
#[must_use]
pub fn about(m: &Affine, pivot: P) -> Affine {
    Affine::translate(pivot.x, pivot.y).then_after(&m.then_after(&Affine::translate(-pivot.x, -pivot.y)))
}

/// A projective or bilinear map of a rectangle onto a quadrilateral (corners bottom-left, bottom-right, top-right,
/// top-left).
#[derive(Debug, Clone, PartialEq)]
pub enum Envelope {
    /// A homography `[h00, h01, h02, h10, h11, h12, h20, h21]` (h22 = 1) from drawing space.
    Projective([f64; 8]),
    /// Bilinear interpolation of the four targets over the source rectangle.
    Bilinear {
        /// Source rectangle: min and max corners.
        min: P,
        /// Max corner.
        max: P,
        /// Targets: bottom-left, bottom-right, top-right, top-left.
        corners: [P; 4],
    },
}

/// Solves an n × n linear system by Gaussian elimination with partial pivoting; `None` when singular.
fn solve<const N: usize>(mut a: [[f64; N]; N], mut b: [f64; N]) -> Option<[f64; N]> {
    for col in 0..N {
        let piv = (col..N).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))?;
        if a[piv][col].abs() < 1e-12 {
            return None;
        }
        a.swap(col, piv);
        b.swap(col, piv);
        for row in col + 1..N {
            let f = a[row][col] / a[col][col];
            let pivot_row = a[col];
            for (k, v) in a[row].iter_mut().enumerate().skip(col) {
                *v -= f * pivot_row[k];
            }
            b[row] -= f * b[col];
        }
    }
    let mut x = [0.0; N];
    for i in (0..N).rev() {
        let s: f64 = (i + 1..N).map(|k| a[i][k] * x[k]).sum();
        x[i] = (b[i] - s) / a[i][i];
    }
    Some(x)
}

fn cross(o: P, a: P, b: P) -> f64 {
    (a - o).cross(b - o)
}

impl Envelope {
    /// The envelope taking the rectangle `min`–`max` onto `corners` (`projective`: `Some(true)` projective,
    /// `Some(false)` bilinear, `None` projective when the target quadrilateral is convex, else bilinear).
    #[must_use]
    pub fn new(min: P, max: P, corners: [P; 4], projective: Option<bool>) -> Option<Envelope> {
        let convex = {
            let s: Vec<f64> = (0..4).map(|i| cross(corners[i], corners[(i + 1) % 4], corners[(i + 2) % 4])).collect();
            s.iter().all(|v| *v > 0.0) || s.iter().all(|v| *v < 0.0)
        };
        if !projective.unwrap_or(convex) {
            return (max.x > min.x && max.y > min.y).then_some(Envelope::Bilinear { min, max, corners });
        }
        let src = [P::new(min.x, min.y), P::new(max.x, min.y), P::new(max.x, max.y), P::new(min.x, max.y)];
        let mut a = [[0.0; 8]; 8];
        let mut b = [0.0; 8];
        for i in 0..4 {
            let (s, t) = (src[i], corners[i]);
            a[2 * i] = [s.x, s.y, 1.0, 0.0, 0.0, 0.0, -s.x * t.x, -s.y * t.x];
            b[2 * i] = t.x;
            a[2 * i + 1] = [0.0, 0.0, 0.0, s.x, s.y, 1.0, -s.x * t.y, -s.y * t.y];
            b[2 * i + 1] = t.y;
        }
        solve(a, b).map(Envelope::Projective)
    }

    /// Maps a point.
    #[must_use]
    pub fn apply(&self, p: P) -> P {
        match self {
            Envelope::Projective(h) => {
                let w = h[6] * p.x + h[7] * p.y + 1.0;
                let w = if w.abs() < 1e-12 { 1e-12_f64.copysign(w) } else { w };
                P::new((h[0] * p.x + h[1] * p.y + h[2]) / w, (h[3] * p.x + h[4] * p.y + h[5]) / w)
            }
            Envelope::Bilinear { min, max, corners } => {
                let u = (p.x - min.x) / (max.x - min.x);
                let v = (p.y - min.y) / (max.y - min.y);
                let bottom = corners[0].lerp(corners[1], u);
                let top = corners[3].lerp(corners[2], u);
                bottom.lerp(top, v)
            }
        }
    }

    /// The affine map through three corners (bottom-left, bottom-right, top-left) for matrices, text and bitmaps.
    #[must_use]
    pub fn affine(&self, min: P, max: P) -> Affine {
        let (o, x, y) = (self.apply(min), self.apply(P::new(max.x, min.y)), self.apply(P::new(min.x, max.y)));
        let (w, h) = ((max.x - min.x).max(1e-12), (max.y - min.y).max(1e-12));
        let (ex, ey) = ((x - o) * (1.0 / w), (y - o) * (1.0 / h));
        let lin = Affine([ex.x, ex.y, ey.x, ey.y, 0.0, 0.0]);
        let t = o - lin.apply(min);
        Affine([ex.x, ex.y, ey.x, ey.y, t.x, t.y])
    }
}

/// A map applied to drawing content.
#[derive(Debug, Clone, PartialEq)]
pub enum Map {
    /// An affine map.
    Affine(Affine),
    /// An envelope with its affine approximation for matrices, text and bitmaps.
    Envelope(Envelope, Affine),
}

impl Map {
    fn point(&self, p: P) -> P {
        match self {
            Map::Affine(a) => a.apply(p),
            Map::Envelope(e, _) => e.apply(p),
        }
    }

    fn affine(&self) -> Affine {
        match self {
            Map::Affine(a) | Map::Envelope(_, a) => *a,
        }
    }

    /// Whether the map reverses orientation.
    #[must_use]
    pub fn reflects(&self) -> bool {
        self.affine().det() < 0.0
    }

    /// Whether the map is a pure translation.
    #[must_use]
    pub fn is_translation(&self) -> bool {
        matches!(self, Map::Affine(Affine([a, b, c, d, _, _])) if *a == 1.0 && *b == 0.0 && *c == 0.0 && *d == 1.0)
    }

    /// Whether the map is a rigid motion (an affine map with an orthonormal linear part: translations, rotations,
    /// reflections), under which a brush outline stays the sweep of its mapped spine.
    #[must_use]
    pub fn is_rigid(&self) -> bool {
        const EPS: f64 = 1e-12;
        matches!(self, Map::Affine(Affine([a, b, c, d, _, _]))
            if (a * a + b * b - 1.0).abs() < EPS && (c * c + d * d - 1.0).abs() < EPS && (a * c + b * d).abs() < EPS)
    }
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.0)
}

/// Maps one anchor `[x, y, inX, inY, outX, outY]`.
fn anchor(a: &Value, m: &Map) -> Value {
    let arr: Vec<f64> = a.as_array().map(|v| v.iter().map(num).collect()).unwrap_or_default();
    if arr.len() < 6 {
        return a.clone();
    }
    let p = P::new(arr[0], arr[1]);
    let (inh, outh) = (P::new(arr[2], arr[3]), P::new(arr[4], arr[5]));
    let (q, qi, qo) = match m {
        Map::Affine(af) => (af.apply(p), af.apply_vec(inh), af.apply_vec(outh)),
        Map::Envelope(..) => {
            let q = m.point(p);
            let ci = if inh == P::default() { q } else { m.point(p + inh) };
            let co = if outh == P::default() { q } else { m.point(p + outh) };
            (q, ci - q, co - q)
        }
    };
    json!([q.x + 0.0, q.y + 0.0, qi.x + 0.0, qi.y + 0.0, qo.x + 0.0, qo.y + 0.0])
}

/// Maps the anchors of a point list; `only`: anchor indices to move (all when `None`).
pub fn map_points(points: &mut Value, m: &Map, only: Option<&[usize]>) {
    if let Some(list) = points.as_array_mut() {
        for (k, a) in list.iter_mut().enumerate() {
            if only.is_none_or(|o| o.contains(&k)) {
                *a = anchor(a, m);
            }
        }
    }
}

/// Composes a matrix `[a, b, c, d, e, f]` (JSON) with an affine map applied after it.
fn matrix(v: &Value, a: &Affine) -> Value {
    let Some(arr) = v.as_array() else { return v.clone() };
    if arr.len() != 6 {
        return v.clone();
    }
    let mut m = [0.0; 6];
    for (k, x) in arr.iter().enumerate() {
        m[k] = num(x);
    }
    let r = a.then_after(&Affine(m)).0;
    json!(r.map(|x| x + 0.0))
}

fn fill(v: &mut Value, a: &Affine) {
    if let Some(mtx) = v.get("matrix").cloned()
        && !mtx.is_null()
        && let Some(o) = v.as_object_mut()
    {
        o.insert("matrix".into(), matrix(&mtx, a));
    }
}

/// Maps a stroke's JSON (10 §4.9): anchors, fill matrix, side fills (swapped under a reflection), brush exterior
/// records and the spine (moved under a translation, dropped otherwise for brush strokes).
pub fn map_stroke(s: &mut Value, m: &Map) {
    let a = m.affine();
    if let Some(p) = s.get_mut("points") {
        map_points(p, m, None);
    }
    if let Some(fm) = s.get("fillMatrix").cloned()
        && !fm.is_null()
        && let Some(o) = s.as_object_mut()
    {
        o.insert("fillMatrix".into(), matrix(&fm, &a));
    }
    if let Some(sides) = s.get_mut("sides").and_then(Value::as_object_mut) {
        for k in ["left", "right"] {
            if let Some(f) = sides.get_mut(k) {
                fill(f, &a);
            }
        }
        if m.reflects() {
            let (l, r) = (sides.remove("left"), sides.remove("right"));
            sides.insert("left".into(), r.unwrap_or(Value::Null));
            sides.insert("right".into(), l.unwrap_or(Value::Null));
        }
    }
    if let Some(records) =
        s.pointer_mut(&format!("/ext/{}/exterior", crate::planar::fill::EXT_NAMESPACE)).and_then(Value::as_array_mut)
    {
        for r in records {
            if let Some(f) = r.get_mut("fill") {
                fill(f, &a);
            }
            if m.reflects()
                && let Some(o) = r.as_object_mut()
            {
                let side = o.get("side").and_then(Value::as_str).unwrap_or("left");
                o.insert("side".into(), json!(if side == "left" { "right" } else { "left" }));
            }
        }
    }
    let brush = s.get("kind").and_then(Value::as_str) == Some("brush");
    if brush && s.get("spine").is_some_and(|v| !v.is_null()) {
        if m.is_rigid() {
            if let Some(sp) = s.pointer_mut("/spine/points") {
                map_points(sp, m, None);
            }
            turn_tip(s, &a);
        } else if let Some(o) = s.as_object_mut() {
            o.insert("spine".into(), Value::Null);
        }
    }
}

/// Turns a brush tip's angle (`dev.toontown.brush` `tipAngle`, degrees) with the linear part of a rigid map. Every
/// tip shape is mirror-symmetric about its axis, so mapping the axis direction covers reflections too.
fn turn_tip(s: &mut Value, a: &Affine) {
    let path = format!("/ext/{}/tipAngle", crate::stroke::EXT_NAMESPACE);
    let Some(deg) = s.pointer(&path).and_then(Value::as_f64) else { return };
    let r = deg.to_radians();
    let d = a.apply_vec(P::new(libm::cos(r), libm::sin(r)));
    let turned = libm::atan2(d.y, d.x).to_degrees();
    if let Some(v) = s.pointer_mut(&path) {
        *v = json!(turned + 0.0);
    }
}

/// Maps a node's JSON: a layer's strokes, a group's children, a text or bitmap `transform`.
pub fn map_node(n: &mut Value, m: &Map) {
    let a = m.affine();
    if let Some(t) = n.get("transform").cloned()
        && !t.is_null()
        && let Some(o) = n.as_object_mut()
    {
        o.insert("transform".into(), matrix(&t, &a));
    }
    if let Some(strokes) = n.get_mut("strokes").and_then(Value::as_array_mut) {
        for s in strokes {
            map_stroke(s, m);
        }
    }
    if let Some(children) = n.get_mut("children").and_then(Value::as_array_mut) {
        for c in children {
            map_node(c, m);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarter_turns_are_exact_and_about_moves_the_pivot() {
        let r = about(&rotation(90.0), P::new(10.0, 0.0));
        assert_eq!(r.apply(P::new(20.0, 0.0)), P::new(10.0, 10.0));
        assert_eq!(rotation(-90.0).apply(P::new(1.0, 0.0)), P::new(0.0, -1.0));
    }

    #[test]
    fn a_reflection_swaps_sides_and_maps_matrices() {
        let mut s = json!({ "kind": "pencil", "points": [[0, 0, 0, 0, 1, 0], [10, 0, -1, 0, 0, 0]],
            "fillMatrix": null, "spine": null,
            "sides": { "left": { "colourId": "a", "matrix": [1, 0, 0, 1, 0, 0] }, "right": null } });
        map_stroke(&mut s, &Map::Affine(scale(-1.0, 1.0)));
        assert_eq!(s["points"][1][0], json!(-10.0));
        assert_eq!(s["points"][0][4], json!(-1.0));
        assert!(s["sides"]["left"].is_null());
        assert_eq!(s["sides"]["right"]["matrix"], json!([-1.0, 0.0, 0.0, 1.0, 0.0, 0.0]));
    }

    #[test]
    fn a_scaled_brush_drops_its_spine_and_a_moved_or_turned_one_keeps_it() {
        let brush = json!({ "kind": "brush", "points": [[0, 0, 0, 0, 0, 0], [10, 0, 0, 0, 0, 0], [5, 5, 0, 0, 0, 0]],
            "spine": { "points": [[0, 0, 0, 0, 0, 0], [10, 0, 0, 0, 0, 0]], "pressure": [[0, 1], [1, 1]], "pen": {} } });
        let mut moved = brush.clone();
        map_stroke(&mut moved, &Map::Affine(Affine::translate(3.0, 4.0)));
        assert_eq!(moved["spine"]["points"][1][0], json!(13.0));
        let mut turned = brush.clone();
        map_stroke(&mut turned, &Map::Affine(rotation(90.0)));
        assert_eq!(turned["spine"]["points"][1], json!([0.0, 10.0, 0.0, 0.0, 0.0, 0.0]));
        let mut scaled = brush;
        map_stroke(&mut scaled, &Map::Affine(scale(2.0, 1.0)));
        assert!(scaled["spine"].is_null());
    }

    #[test]
    fn a_rigid_map_turns_a_flat_tip_and_a_mirror_reflects_it() {
        let brush = json!({ "kind": "brush", "points": [[0, 0, 0, 0, 0, 0], [10, 0, 0, 0, 0, 0], [5, 5, 0, 0, 0, 0]],
            "spine": { "points": [[0, 0, 0, 0, 0, 0], [10, 0, 0, 0, 0, 0]], "pressure": [[0, 1], [1, 1]], "pen": {} },
            "ext": { "dev.toontown.brush": { "tipAngle": 30.0 } } });
        let mut turned = brush.clone();
        map_stroke(&mut turned, &Map::Affine(rotation(90.0)));
        let a = turned["ext"]["dev.toontown.brush"]["tipAngle"].as_f64().unwrap_or(0.0);
        assert!((a - 120.0).abs() < 1e-9, "{a}");
        let mut flipped = brush;
        map_stroke(&mut flipped, &Map::Affine(scale(-1.0, 1.0)));
        let a = flipped["ext"]["dev.toontown.brush"]["tipAngle"].as_f64().unwrap_or(0.0);
        assert!((a - 150.0).abs() < 1e-9, "{a}");
    }

    #[test]
    fn envelopes_hit_their_corners_projectively_and_bilinearly() {
        let (min, max) = (P::new(0.0, 0.0), P::new(10.0, 10.0));
        let corners = [P::new(0.0, 0.0), P::new(20.0, 0.0), P::new(15.0, 10.0), P::new(5.0, 10.0)];
        for proj in [Some(true), Some(false)] {
            let Some(e) = Envelope::new(min, max, corners, proj) else { panic!("envelope") };
            for (s, t) in [min, P::new(10.0, 0.0), max, P::new(0.0, 10.0)].iter().zip(corners) {
                assert!(e.apply(*s).dist(t) < 1e-9, "{proj:?}");
            }
        }
    }
}
