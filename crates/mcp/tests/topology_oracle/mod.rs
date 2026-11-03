//! The independent topology oracle of the history tests (C22-R1-11): a layer holding a closed pencil square
//! (512…640 du, anchors every 32 du along its bottom and top edges) and vertical pencil cuts through it at
//! x = 528 + 32k (k = 0…3), each crossing one square segment at its midpoint and its own first and last segments at
//! their midpoints. Every junction is therefore at an exactly representable point and every piece is a straight
//! segment (corner handles; ADR-0017 history, M2 C2.4) or a whole one, so this module predicts, without reading the
//! engine's graph:
//!
//! - the persisted pieces of the square and of every cut (02 §3.12 item 1: split at the crossings, shared junction
//!   anchors, pieces in place of the stroke they split, never merged back);
//! - the zones (the rectangles between consecutive cuts), their IDs (the C22-R1-03 ruling: SHA-256 of each boundary
//!   cycle in traversal order, snapped coordinates with the stroke IDs and sides of the edge leaving each vertex,
//!   straight-on vertices of the same strokes left out, started at the smallest token sequence) and the side fills of
//!   every piece (10 §4.9);
//! - carry-over: a split zone's fill on both parts, a merged zone the fill of the larger part with the lower old zone
//!   ID on ties (02 §3.12; SI-060).
//!
//! The engine allocates the IDs of new pieces; the oracle predicts which pieces exist and where they are in the
//! layer, and takes their IDs from those positions.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// Cut positions.
pub const CUTS: usize = 4;

/// The x of cut position `k`.
pub fn cut_x(k: usize) -> f64 {
    528.0 + 32.0 * k as f64
}

/// The square's anchors, counter-clockwise (y up): bottom edge left to right, top edge right to left.
pub fn square_anchors() -> Vec<(f64, f64)> {
    let mut a: Vec<(f64, f64)> = (0..5).map(|i| (512.0 + 32.0 * f64::from(i), 512.0)).collect();
    a.extend((0..5).map(|i| (640.0 - 32.0 * f64::from(i), 640.0)));
    a
}

/// A cut's anchors (bottom to top).
pub fn cut_anchors(k: usize) -> Vec<(f64, f64)> {
    let x = cut_x(k);
    vec![(x, 496.0), (x, 528.0), (x, 624.0), (x, 656.0)]
}

/// The layer's state.
#[derive(Debug, Clone, PartialEq)]
pub struct Topo {
    /// The square's pieces in stroke order: ID and path parameters (`to` may pass 10 across the start).
    pub square: Vec<(String, f64, f64)>,
    /// Whether the square is still one closed stroke.
    pub closed: bool,
    /// Cuts in stroke order: position and the IDs of their pieces (bottom stub, middle, top stub).
    pub cuts: Vec<(usize, [String; 3])>,
    /// Zone fills by x-range.
    pub fills: BTreeMap<(i64, i64), Option<String>>,
}

/// An ID the engine allocates (filled in from the layer after the command).
pub const NEW: &str = "?";

type Cubic = [(f64, f64); 4];

fn lerp(a: (f64, f64), b: (f64, f64), t: f64) -> (f64, f64) {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

fn split(c: &Cubic, t: f64) -> (Cubic, Cubic) {
    let a = lerp(c[0], c[1], t);
    let b = lerp(c[1], c[2], t);
    let cc = lerp(c[2], c[3], t);
    let d = lerp(a, b, t);
    let e = lerp(b, cc, t);
    let m = lerp(d, e, t);
    ([c[0], a, d, m], [m, e, cc, c[3]])
}

fn sub_cubic(c: &Cubic, t0: f64, t1: f64) -> Cubic {
    if t0 <= 0.0 && t1 >= 1.0 {
        return *c;
    }
    let right = if t0 > 0.0 { split(c, t0).1 } else { *c };
    if t1 >= 1.0 {
        return right;
    }
    split(&right, (t1 - t0) / (1.0 - t0)).0
}

/// The persisted anchors of the part of a polyline (corner anchors) between two path parameters.
fn sub_path(anchors: &[(f64, f64)], closed: bool, from: f64, to: f64) -> Value {
    let n = anchors.len();
    let segs = if closed { n } else { n - 1 };
    let mut cubics: Vec<Cubic> = Vec::new();
    let mut s = from;
    while s < to - 1e-12 {
        let j = s.floor() as usize;
        let (p0, p3) = (anchors[j % segs.max(1) % n], anchors[(j % segs.max(1) + 1) % n]);
        let seg: Cubic = [p0, p0, p3, p3];
        let t0 = s - j as f64;
        let t1 = ((j + 1) as f64).min(to) - j as f64;
        cubics.push(sub_cubic(&seg, t0, t1));
        s = if t1 >= 1.0 { (j + 1) as f64 } else { j as f64 + t1 };
    }
    // [x, y, inX, inY, outX, outY] per anchor. Every segment here is straight, and a piece of a straight segment stays
    // a straight segment with corner handles between its junction ends (ADR-0017 history, M2 C2.4 fix 3).
    let mut out: Vec<[f64; 6]> = Vec::new();
    for c in &cubics {
        if out.is_empty() {
            out.push([c[0].0, c[0].1, 0.0, 0.0, 0.0, 0.0]);
        }
        out.push([c[3].0, c[3].1, 0.0, 0.0, 0.0, 0.0]);
    }
    json!(out)
}

/// The point at a square path parameter.
fn square_point(p: f64) -> (f64, f64) {
    let a = square_anchors();
    let q = p.rem_euclid(10.0);
    let j = q.floor() as usize;
    lerp(a[j], a[(j + 1) % 10], q - j as f64)
}

impl Topo {
    /// The unsplit square.
    pub fn new(square_id: &str) -> Self {
        let mut fills = BTreeMap::new();
        fills.insert((512, 640), None);
        Topo { square: vec![(square_id.to_owned(), 0.0, 10.0)], closed: true, cuts: Vec::new(), fills }
    }

    /// Zone x-ranges, left to right.
    pub fn zones(&self) -> Vec<(i64, i64)> {
        let mut xs: Vec<i64> = vec![512, 640];
        xs.extend(self.cuts.iter().map(|(k, _)| cut_x(*k) as i64));
        xs.sort_unstable();
        xs.windows(2).map(|w| (w[0], w[1])).collect()
    }

    /// The square piece over a path parameter (not a split point).
    fn piece_at(&self, q: f64) -> &str {
        let q = q.rem_euclid(10.0);
        for (id, from, to) in &self.square {
            if (q > *from && q < *to) || (q + 10.0 > *from && q + 10.0 < *to) {
                return id;
            }
        }
        "missing"
    }

    fn cut_at(&self, x: i64) -> &str {
        self.cuts.iter().find(|(k, _)| cut_x(*k) as i64 == x).map_or("missing", |(_, ids)| ids[1].as_str())
    }

    /// The square's split points (piece starts), ascending; none while closed.
    fn splits(&self) -> Vec<f64> {
        if self.closed {
            return Vec::new();
        }
        let mut s: Vec<f64> = self.square.iter().map(|(_, f, _)| f.rem_euclid(10.0)).collect();
        s.sort_by(f64::total_cmp);
        s
    }

    /// The canonical boundary text of the zone over `[xl, xr]` (ruling C22-R1-03), computed from the oracle's pieces.
    pub fn zone_text(&self, xl: i64, xr: i64) -> String {
        let g = |x: f64| (x * 64.0) as i64;
        let tok = |x: f64, y: f64, label: String| format!("{},{};{label}", g(x), g(y));
        let (fx, tx) = (xl as f64, xr as f64);
        let eps = 1.0 / 1024.0;
        let mut tokens = Vec::new();
        // Bottom edge, left to right.
        tokens.push(tok(fx, 512.0, format!("{}:L", self.piece_at((fx - 512.0) / 32.0 + eps))));
        let (b0, b1) = ((fx - 512.0) / 32.0, (tx - 512.0) / 32.0);
        for p in self.splits().into_iter().filter(|p| *p > b0 && *p < b1) {
            let (x, y) = square_point(p);
            tokens.push(tok(x, y, format!("{}:L", self.piece_at(p + eps))));
        }
        // Up the right boundary (the square's side, or the cut with the zone on its left).
        let up = if xr == 640 { format!("{}:L", self.piece_at(4.0 + eps)) } else { format!("{}:L", self.cut_at(xr)) };
        tokens.push(tok(tx, 512.0, up));
        // Top edge, right to left.
        let (t0, t1) = (5.0 + (640.0 - tx) / 32.0, 5.0 + (640.0 - fx) / 32.0);
        tokens.push(tok(tx, 640.0, format!("{}:L", self.piece_at(t0 + eps))));
        for p in self.splits().into_iter().filter(|p| *p > t0 && *p < t1) {
            let (x, y) = square_point(p);
            tokens.push(tok(x, y, format!("{}:L", self.piece_at(p + eps))));
        }
        // Down the left boundary (the square's side, or the cut with the zone on its right).
        let down = if xl == 512 { format!("{}:L", self.piece_at(9.0 + eps)) } else { format!("{}:R", self.cut_at(xl)) };
        tokens.push(tok(fx, 640.0, down));
        let m = tokens.len();
        let first = tokens.iter().min().cloned().unwrap_or_default();
        let best = (0..m)
            .filter(|&x| tokens[x] == first)
            .min_by(|&x, &y| (0..m).map(|i| &tokens[(x + i) % m]).cmp((0..m).map(|i| &tokens[(y + i) % m])))
            .unwrap_or(0);
        (0..m).map(|i| tokens[(best + i) % m].as_str()).collect::<Vec<_>>().join("|")
    }

    /// The zone ID over `[xl, xr]`.
    pub fn zone_id(&self, xl: i64, xr: i64) -> String {
        const A: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
        let digest = Sha256::digest(self.zone_text(xl, xr).as_bytes());
        let mut out = String::from("zn_");
        let (mut acc, mut bits) = (0u32, 0);
        for &byte in digest.iter() {
            acc = (acc << 8) | u32::from(byte);
            bits += 8;
            while bits >= 5 && out.len() < 29 {
                bits -= 5;
                out.push(char::from(A[((acc >> bits) & 31) as usize]));
            }
        }
        out
    }

    fn zone_of_x(&self, x: f64) -> Option<(i64, i64)> {
        self.zones().into_iter().find(|(l, r)| (*l as f64) < x && x < *r as f64)
    }

    /// The state after adding a cut at position `k` (new IDs are [`NEW`]).
    pub fn cut(&self, k: usize) -> Topo {
        let mut t = self.clone();
        let x = cut_x(k);
        let zone = self.zone_of_x(x).unwrap_or((512, 640));
        let fill = t.fills.remove(&zone).flatten();
        t.fills.insert((zone.0, x as i64), fill.clone());
        t.fills.insert((x as i64, zone.1), fill);
        let (b, top) = (k as f64 + 0.5, 8.5 - k as f64);
        if t.closed {
            t.square = vec![(NEW.into(), b, top), (NEW.into(), top, b + 10.0)];
            t.closed = false;
        } else {
            for q in [b, top] {
                if t.square.iter().any(|(_, f, _)| (f.rem_euclid(10.0) - q).abs() < 1e-9) {
                    continue;
                }
                let Some(i) =
                    t.square.iter().position(|(_, f, to)| (q > *f && q < *to) || (q + 10.0 > *f && q + 10.0 < *to))
                else {
                    continue;
                };
                let (_, f, to) = t.square[i].clone();
                let q = if q > f { q } else { q + 10.0 };
                let second = if q >= 10.0 { (NEW.to_owned(), q - 10.0, to - 10.0) } else { (NEW.to_owned(), q, to) };
                t.square.splice(i..=i, [(NEW.to_owned(), f, q), second]);
            }
        }
        t.cuts.push((k, [NEW.into(), NEW.into(), NEW.into()]));
        t
    }

    /// The state after deleting the cut at position `k` (pieces of the square are not merged back).
    pub fn merge(&self, k: usize) -> Topo {
        let mut t = self.clone();
        let x = cut_x(k) as i64;
        t.cuts.retain(|(c, _)| *c != k);
        let zones = self.zones();
        let (Some(a), Some(b)) = (zones.iter().find(|z| z.1 == x), zones.iter().find(|z| z.0 == x)) else { return t };
        let (fa, fb) = (self.fills.get(a).cloned().flatten(), self.fills.get(b).cloned().flatten());
        let (wa, wb) = (a.1 - a.0, b.1 - b.0);
        // The larger old zone wins; equal overlaps go to the lower old zone ID.
        let fill = if wa != wb {
            if wa > wb { fa } else { fb }
        } else if self.zone_id(a.0, a.1) < self.zone_id(b.0, b.1) {
            fa
        } else {
            fb
        };
        t.fills.remove(a);
        t.fills.remove(b);
        t.fills.insert((a.0, b.1), fill);
        t
    }

    /// The layer's strokes in order (without revisions) from a template stroke (the square's colour and line style).
    pub fn strokes(&self, template: &Value) -> Vec<(String, Value)> {
        let side = |f: Option<&Option<String>>| match f.cloned().flatten() {
            Some(c) => json!({ "colourId": c, "matrix": null }),
            None => Value::Null,
        };
        let mut out = Vec::new();
        for (id, from, to) in &self.square {
            let mut s = template.clone();
            s["id"] = json!(id);
            if self.closed {
                s["closed"] = json!(true);
                s["points"] =
                    json!(square_anchors().iter().map(|(x, y)| [*x, *y, 0.0, 0.0, 0.0, 0.0]).collect::<Vec<_>>());
            } else {
                s["closed"] = json!(false);
                s["points"] = sub_path(&square_anchors(), true, *from, *to);
            }
            let x = square_point(from + 0.25).0;
            let zone = if self.closed { Some((512, 640)) } else { self.zone_of_x(x) };
            s["sides"] = json!({ "left": side(zone.as_ref().and_then(|z| self.fills.get(z))), "right": null });
            out.push((id.clone(), s));
        }
        for (k, ids) in &self.cuts {
            let x = cut_x(*k);
            for (part, (from, to)) in [(0.0, 0.5), (0.5, 2.5), (2.5, 3.0)].into_iter().enumerate() {
                let mut s = template.clone();
                s["id"] = json!(ids[part]);
                s["closed"] = json!(false);
                s["points"] = sub_path(&cut_anchors(*k), false, from, to);
                s["sides"] = if part == 1 {
                    json!({
                        "left": side(self.zone_of_x(x - 1.0).as_ref().and_then(|z| self.fills.get(z))),
                        "right": side(self.zone_of_x(x + 1.0).as_ref().and_then(|z| self.fills.get(z))),
                    })
                } else {
                    json!({ "left": null, "right": null })
                };
                out.push((ids[part].clone(), s));
            }
        }
        out
    }

    /// Fills in the IDs the engine allocated from the layer's stroke IDs (same order); `false` when the layer does not
    /// hold the predicted number of strokes or reuses an ID.
    pub fn adopt(&mut self, actual: &[String], known: &std::collections::BTreeSet<String>) -> bool {
        let n = self.square.len() + 3 * self.cuts.len();
        if actual.len() != n {
            return false;
        }
        let mut k = 0;
        let mut fresh = true;
        let mut take = |id: &mut String, k: &mut usize| {
            if id == NEW {
                *id = actual[*k].clone();
                fresh &= !known.contains(id);
            } else {
                fresh &= *id == actual[*k];
            }
            *k += 1;
        };
        for p in &mut self.square {
            take(&mut p.0, &mut k);
        }
        for (_, ids) in &mut self.cuts {
            for id in ids.iter_mut() {
                take(id, &mut k);
            }
        }
        fresh
    }
}
