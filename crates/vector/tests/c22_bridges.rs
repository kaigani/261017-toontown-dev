//! SI-053 corpus (M2 C2.2 readiness item 1): brush outlines with holes, nested loops, disconnected pieces from
//! zero-pressure gaps, retraced spines and both orientations. Each outline is one closed path whose contours are joined
//! by zero-width bridges; checked against an independent swept-tip union (lattice membership away from the boundary)
//! and through the planar map: the zones are exactly the expected holes, every hole centre is a zone, the stroke body is
//! the brush fill, and no bridge invents a painted strip, a zone or a gap. Accepted near-budget coils and refused larger
//! coils are in `c21_worst_case` (brush corpus).
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "test")]

use tt_model::entities::pen::Tip;
use tt_vector::brush::{BrushPen, outline};
use tt_vector::geom::P;
use tt_vector::path::{Anchor, Path};
use tt_vector::planar::gap::inside;
use tt_vector::planar::{EdgeKind, StrokeIn, build};
use tt_vector::work::Work;

fn poly(pts: &[(f64, f64)], closed: bool) -> Path {
    Path { anchors: pts.iter().map(|&(x, y)| Anchor::corner(P::new(x, y))).collect(), closed }
}

/// Points of a spine with their normalised arc-length positions (dense sampling).
fn samples(spine: &Path) -> Vec<(P, f64)> {
    let segs = spine.segments();
    let mut pts = Vec::new();
    for s in &segs {
        for k in 0..400 {
            pts.push(s.eval(f64::from(k) / 400.0));
        }
    }
    if let Some(last) = segs.last() {
        pts.push(last.eval(1.0));
    }
    let mut len = vec![0.0];
    for w in pts.windows(2) {
        let l = len[len.len() - 1] + w[0].dist(w[1]);
        len.push(l);
    }
    let total = len[len.len() - 1].max(1e-12);
    pts.into_iter().zip(len).map(|(p, l)| (p, l / total)).collect()
}

fn pressure_at(map: &[[f64; 2]], t: f64) -> f64 {
    tt_vector::fit::pressure_at(map, t)
}

struct Case {
    name: &'static str,
    spine: Path,
    pressure: Vec<[f64; 2]>,
    width: f64,
    /// Points expected to be zones (hole centres).
    holes: Vec<P>,
}

fn cases() -> Vec<Case> {
    let square = |x: f64, y: f64, s: f64| vec![(x, y), (x + s, y), (x + s, y + s), (x, y + s)];
    let full = vec![[0.0, 1.0], [1.0, 1.0]];
    let mut ring = square(0.0, 0.0, 60.0);
    ring.push((0.0, 0.0));
    let mut ring_rev = ring.clone();
    ring_rev.reverse();
    // Two crossing loops: the spine crosses itself twice and encloses two triangles and a diamond.
    let eight = vec![(0.0, 0.0), (40.0, 40.0), (80.0, 0.0), (80.0, 40.0), (40.0, 0.0), (0.0, 40.0), (0.0, 0.0)];
    // A big loop with a small loop drawn inside it (nested loops of one stroke).
    let nested = vec![
        (0.0, 0.0),
        (100.0, 0.0),
        (100.0, 100.0),
        (0.0, 100.0),
        (0.0, 0.0),
        (30.0, 30.0),
        (70.0, 30.0),
        (70.0, 70.0),
        (30.0, 70.0),
        (30.0, 30.0),
    ];
    // Retraced: a ring drawn twice over itself.
    let mut twice = ring.clone();
    twice.extend(ring.iter().skip(1).copied());
    vec![
        Case {
            name: "ring",
            spine: poly(&ring, false),
            pressure: full.clone(),
            width: 6.0,
            holes: vec![P::new(30.0, 30.0)],
        },
        Case {
            name: "ring-reversed",
            spine: poly(&ring_rev, false),
            pressure: full.clone(),
            width: 6.0,
            holes: vec![P::new(30.0, 30.0)],
        },
        Case {
            name: "figure-eight",
            spine: poly(&eight, false),
            pressure: full.clone(),
            width: 4.0,
            holes: vec![P::new(9.0, 20.0), P::new(40.0, 20.0), P::new(71.0, 20.0)],
        },
        Case {
            name: "nested-loops",
            spine: poly(&nested, false),
            pressure: full.clone(),
            width: 6.0,
            holes: vec![P::new(50.0, 50.0), P::new(15.0, 50.0)],
        },
        Case {
            name: "retraced-ring",
            spine: poly(&twice, false),
            pressure: full.clone(),
            width: 6.0,
            holes: vec![P::new(30.0, 30.0)],
        },
        Case {
            name: "zero-pressure-pieces",
            spine: poly(&[(0.0, 0.0), (200.0, 0.0)], false),
            pressure: vec![
                [0.0, 1.0],
                [0.2, 1.0],
                [0.21, 0.0],
                [0.4, 0.0],
                [0.41, 1.0],
                [0.6, 1.0],
                [0.61, 0.0],
                [0.8, 0.0],
                [0.81, 1.0],
                [1.0, 1.0],
            ],
            width: 10.0,
            holes: vec![],
        },
    ]
}

#[test]
fn holes_pieces_and_bridges_match_the_swept_union_and_the_planar_map() {
    let mut failures = Vec::new();
    for c in cases() {
        let pen = BrushPen { tip: Tip::Round, min_size: 0.0, max_size: c.width, tip_angle: 0.0 };
        let out = outline(&c.spine, &c.pressure, &pen).unwrap();
        assert!(out.closed, "{}", c.name);
        let tips = samples(&c.spine);
        let mut work = Work::unlimited("t");
        let b = out.bounds().unwrap();
        // Lattice membership away from the boundary (more than 0.3 du from the swept edge).
        let mut checked = 0;
        let (x0, y0, x1, y1) = (
            b.min_x.floor() as i64 - 2,
            b.min_y.floor() as i64 - 2,
            b.max_x.ceil() as i64 + 2,
            b.max_y.ceil() as i64 + 2,
        );
        for xi in x0..=x1 {
            for yi in y0..=y1 {
                let p = P::new(xi as f64 + 0.37, yi as f64 + 0.41);
                // Signed clearance of the union of tips: max over tips of (radius − distance).
                let clearance = tips
                    .iter()
                    .map(|(q, t)| c.width * pressure_at(&c.pressure, *t) / 2.0 - p.dist(*q))
                    .fold(f64::NEG_INFINITY, f64::max);
                if clearance.abs() < 0.3 {
                    continue;
                }
                checked += 1;
                let got = inside(&out, p, &mut work).unwrap();
                if got != (clearance > 0.0) {
                    failures.push(format!("{}: {p:?} union {} outline {got}", c.name, clearance > 0.0));
                }
            }
        }
        assert!(checked > 100, "{}", c.name);
        // The planar map of the stroke alone: zones are exactly the holes; bridges add nothing.
        let m = build(&[StrokeIn { id: "b".into(), kind: EdgeKind::Brush, path: out.clone() }], &mut work).unwrap();
        if m.zones.len() != c.holes.len() {
            failures.push(format!(
                "{}: {} zones, {} holes expected: {:?}",
                c.name,
                m.zones.len(),
                c.holes.len(),
                m.zones.iter().map(|z| (z.area, z.interior)).collect::<Vec<_>>()
            ));
        }
        for h in &c.holes {
            if m.zone_at(*h).is_none() {
                failures.push(format!("{}: hole centre {h:?} is no zone", c.name));
            }
        }
        for z in &m.zones {
            if z.area < c.width * c.width {
                failures.push(format!("{}: a strip-like zone of {} du²", c.name, z.area));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
