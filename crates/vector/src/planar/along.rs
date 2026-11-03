//! What a dragged path passes over in a layer's planar map (03 §8.2 solid-drag rule; C22-R1-04): the path is traversed
//! continuously, not sampled. Each path segment is cut at every crossing with an edge of the map; between consecutive
//! cuts the segment stays inside one face, so the face at the middle of each interval is crossed (its zone, or the
//! brush strokes filling it). Strokes are reported with how close their edges come to the path. All work is charged
//! to the caller's account, so one command pays once for every segment and layer.

use std::collections::{BTreeMap, BTreeSet};

use super::graph::{LayerMap, unsnap};
use crate::geom::P;
use crate::path::GeomError;
use crate::work::Work;

/// What a path passes over in one layer.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PathHits {
    /// Zones crossed, in the order the path first enters them.
    pub zones: Vec<usize>,
    /// Brush strokes (indices into [`LayerMap::strokes`]) whose fill the path crosses, in order of first crossing.
    pub brushes: Vec<usize>,
    /// Strokes (indices into [`LayerMap::strokes`]) with an edge within the reach of the path, with the smallest
    /// distance (du; 0 where the path crosses it).
    pub near: BTreeMap<usize, f64>,
}

fn cross(a: P, b: P) -> f64 {
    a.x * b.y - a.y * b.x
}

fn seg_point(p: P, a: P, b: P) -> f64 {
    let ab = b - a;
    let len2 = ab.dot(ab);
    let t = if len2 > 0.0 { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
    p.dist(a + ab * t)
}

/// The distance between two segments and, where they cross, the fraction along the first.
fn seg_seg(a: P, b: P, c: P, d: P) -> (f64, Option<f64>) {
    let (r, s) = (b - a, d - c);
    let den = cross(r, s);
    if den != 0.0 {
        let t = cross(c - a, s) / den;
        let u = cross(c - a, r) / den;
        if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
            return (0.0, Some(t));
        }
    }
    let d0 = seg_point(a, c, d).min(seg_point(b, c, d)).min(seg_point(c, a, b)).min(seg_point(d, a, b));
    (d0, None)
}

impl LayerMap {
    /// What the polyline `path` (du) passes over, with strokes reported within `reach` du.
    ///
    /// # Errors
    /// `limit.exceeded` when the account runs out.
    pub fn along(&self, path: &[P], reach: f64, work: &mut Work) -> Result<PathHits, GeomError> {
        let mut out = PathHits::default();
        if path.is_empty() || self.edges.is_empty() {
            for p in path {
                self.note_face(*p, &mut out, &mut BTreeSet::new(), &mut BTreeSet::new());
            }
            return Ok(out);
        }
        // A grid of edge boxes, built once per call.
        let segs: Vec<(P, P)> = self.edges.iter().map(|e| (unsnap(self.verts[e.u]), unsnap(self.verts[e.v]))).collect();
        let mean = segs.iter().map(|(a, b)| a.dist(*b)).sum::<f64>() / segs.len() as f64;
        let size = (mean * 4.0).max(reach * 2.0).max(1.0 / 16.0);
        let cell = |x: f64| (x / size).floor() as i64;
        let mut grid: BTreeMap<(i64, i64), Vec<usize>> = BTreeMap::new();
        work.spend_n(segs.len().saturating_mul(4))?;
        for (k, (a, b)) in segs.iter().enumerate() {
            let (x0, x1) = (cell(a.x.min(b.x)), cell(a.x.max(b.x)));
            let (y0, y1) = (cell(a.y.min(b.y)), cell(a.y.max(b.y)));
            work.spend(u64::try_from((x1 - x0 + 1).saturating_mul(y1 - y0 + 1)).unwrap_or(u64::MAX))?;
            for cx in x0..=x1 {
                for cy in y0..=y1 {
                    grid.entry((cx, cy)).or_default().push(k);
                }
            }
        }
        let (mut seen_zones, mut seen_brushes) = (BTreeSet::new(), BTreeSet::new());
        for w in path.windows(2) {
            let (a, b) = (w[0], w[1]);
            // Candidate edges: the cells along the segment (in steps of one cell), widened by the reach.
            let steps = ((a.dist(b) / size).ceil() as usize).max(1);
            work.spend_n(steps)?;
            let mut cand: BTreeSet<usize> = BTreeSet::new();
            for k in 0..steps {
                let (p, q) = (a.lerp(b, k as f64 / steps as f64), a.lerp(b, (k + 1) as f64 / steps as f64));
                let (x0, x1) = (cell(p.x.min(q.x) - reach), cell(p.x.max(q.x) + reach));
                let (y0, y1) = (cell(p.y.min(q.y) - reach), cell(p.y.max(q.y) + reach));
                for cx in x0..=x1 {
                    for cy in y0..=y1 {
                        if let Some(list) = grid.get(&(cx, cy)) {
                            work.spend_n(list.len())?;
                            cand.extend(list.iter().copied());
                        }
                    }
                }
            }
            let mut cuts: Vec<f64> = vec![0.0, 1.0];
            for &k in &cand {
                let (c, d) = segs[k];
                let (dist, t) = seg_seg(a, b, c, d);
                if let Some(t) = t {
                    cuts.push(t);
                }
                if dist <= reach {
                    for s in &self.edges[k].srcs {
                        let e = out.near.entry(s.stroke).or_insert(f64::INFINITY);
                        *e = e.min(dist);
                    }
                }
            }
            cuts.sort_by(f64::total_cmp);
            cuts.dedup();
            work.spend_n(cuts.len().saturating_mul(self.faces.len().min(64) + 1))?;
            self.note_face(a, &mut out, &mut seen_zones, &mut seen_brushes);
            for c in cuts.windows(2) {
                if c[1] - c[0] > 0.0 {
                    self.note_face(a.lerp(b, (c[0] + c[1]) / 2.0), &mut out, &mut seen_zones, &mut seen_brushes);
                }
            }
            self.note_face(b, &mut out, &mut seen_zones, &mut seen_brushes);
        }
        Ok(out)
    }

    fn note_face(&self, p: P, out: &mut PathHits, zones: &mut BTreeSet<usize>, brushes: &mut BTreeSet<usize>) {
        let Some(f) = self.face_at(p) else { return };
        if let Some(z) = self.faces[f].zone
            && zones.insert(z)
        {
            out.zones.push(z);
        }
        for &b in &self.faces[f].brushes {
            if brushes.insert(b) {
                out.brushes.push(b);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::{Anchor, Path};
    use crate::planar::graph::{EdgeKind, StrokeIn, build};

    fn rect(id: &str, x0: f64, y0: f64, x1: f64, y1: f64) -> StrokeIn {
        StrokeIn {
            id: id.into(),
            kind: EdgeKind::Invisible,
            path: Path {
                anchors: [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
                    .iter()
                    .map(|&(x, y)| Anchor::corner(P::new(x, y)))
                    .collect(),
                closed: true,
            },
        }
    }

    #[test]
    fn a_path_crosses_a_zone_narrower_than_any_sampling_step() {
        let Ok(m) =
            build(&[rect("thin", 10.1, 0.0, 10.2, 2.0), rect("wide", 20.0, 0.0, 30.0, 2.0)], &mut Work::unlimited("t"))
        else {
            panic!("build");
        };
        let hits = m.along(&[P::new(0.0, 1.0), P::new(40.0, 1.0)], 0.0, &mut Work::unlimited("t")).unwrap_or_default();
        assert_eq!(hits.zones.len(), 2, "{hits:?}");
        assert_eq!(m.zones[hits.zones[0]].bounds.min_x, 10.09375);
        assert_eq!(hits.near.len(), 2, "both rectangles' edges are crossed");
    }

    #[test]
    fn the_account_is_shared() {
        let Ok(m) = build(&[rect("a", 0.0, 0.0, 10.0, 10.0)], &mut Work::unlimited("t")) else {
            panic!("build");
        };
        let mut work = Work::new(200, "t");
        let long: Vec<P> = (0..1000).map(|i| P::new(f64::from(i % 2) * 20.0 - 5.0, 5.0)).collect();
        assert!(m.along(&long, 0.0, &mut work).is_err(), "one account for the whole path");
    }
}
