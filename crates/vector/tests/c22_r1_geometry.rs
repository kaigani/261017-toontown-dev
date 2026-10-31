#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review regression tests")]
use tt_vector::geom::P;
use tt_vector::path::{Anchor, Path};
use tt_vector::planar::fill::{Fill, carry};
use tt_vector::planar::graph::{EdgeKind, StrokeIn, build};
use tt_vector::planar::node::{NodeStroke, Outcome, node};
use tt_vector::work::Work;
fn path(points: &[(f64, f64)], closed: bool) -> Path {
    Path { anchors: points.iter().map(|&(x, y)| Anchor::corner(P::new(x, y))).collect(), closed }
}
fn rect(id: &str, x0: f64, y0: f64, x1: f64, y1: f64) -> StrokeIn {
    StrokeIn { id: id.into(), kind: EdgeKind::Pencil, path: path(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], true) }
}
#[test]
fn overlapping_zones_carry_fill_even_when_neither_interior_point_is_in_the_other() {
    let old = build(&[rect("old", 0., 4., 10., 6.)], &mut Work::unlimited("review")).unwrap();
    let new = build(&[rect("new", 1., 0., 3., 10.)], &mut Work::unlimited("review")).unwrap();
    assert_eq!(old.zones.len(), 1);
    assert_eq!(new.zones.len(), 1);
    let red = Some(Fill { colour_id: Some("col_red".into()), matrix: None });
    let fills = carry(&old, &[red.clone()], &new, &mut Work::unlimited("review")).unwrap();
    assert_eq!(
        fills,
        vec![red],
        "the exact intersection is a 2 x 2 square; old interior {:?}, new interior {:?}",
        old.zones[0].interior,
        new.zones[0].interior
    );
}
#[test]
fn a_crossing_near_an_existing_anchor_has_one_shared_junction() {
    let a = NodeStroke {
        id: "old".into(),
        kind: EdgeKind::Pencil,
        path: path(&[(0., 0.), (5., 0.), (10., 0.)], false),
        changed: false,
    };
    let b = NodeStroke {
        id: "new".into(),
        kind: EdgeKind::Pencil,
        path: path(&[(5.01, -5.), (5.01, 5.)], false),
        changed: true,
    };
    let result = node(&[a, b], &mut Work::unlimited("review")).unwrap();
    let Outcome::Split(a) = &result.outcomes[&0] else { panic!("old line not split") };
    let Outcome::Split(b) = &result.outcomes[&1] else { panic!("new line not split") };
    assert_eq!(
        a[0].anchors.last().unwrap().p,
        b[0].anchors.last().unwrap().p,
        "junctions must share an exact coordinate"
    );
}

// Independent convex clipping oracle: it never reads engine edges/faces to construct the expected cells.
fn clip(poly: &[P], a: P, b: P, sign: f64) -> Vec<P> {
    let mut out = Vec::new();
    for i in 0..poly.len() {
        let p = poly[i];
        let q = poly[(i + 1) % poly.len()];
        let dp = (b - a).cross(p - a) * sign;
        let dq = (b - a).cross(q - a) * sign;
        if dp >= 0. {
            out.push(p);
        }
        if (dp > 0. && dq < 0.) || (dp < 0. && dq > 0.) {
            out.push(p.lerp(q, dp / (dp - dq)));
        }
    }
    out
}
fn area(poly: &[P]) -> f64 {
    (0..poly.len()).map(|i| poly[i].cross(poly[(i + 1) % poly.len()])).sum::<f64>().abs() / 2.
}
#[test]
fn fixed_seed_line_arrangements_match_independent_convex_clipping() {
    let mut rng = 9173u64;
    for case in 0..30 {
        let square = vec![P::new(0., 0.), P::new(100., 0.), P::new(100., 100.), P::new(0., 100.)];
        let mut cells = vec![square];
        let mut strokes = vec![rect("outer", 0., 0., 100., 100.)];
        for n in 0..8 {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let y0 = 5. + ((rng >> 32) % 90) as f64;
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let y1 = 5. + ((rng >> 32) % 90) as f64;
            let a = P::new(0., y0);
            let b = P::new(100., y1);
            strokes.push(StrokeIn {
                id: format!("line{n}"),
                kind: EdgeKind::Pencil,
                path: path(&[(a.x, a.y), (b.x, b.y)], false),
            });
            cells = cells
                .iter()
                .flat_map(|c| [clip(c, a, b, 1.), clip(c, a, b, -1.)])
                .filter(|c| c.len() > 2 && area(c) > 1e-7)
                .collect();
        }
        if cells.iter().any(|c| area(c) < 1.) {
            continue;
        }
        let map = build(&strokes, &mut Work::unlimited("oracle")).unwrap();
        assert_eq!(map.zones.len(), cells.len(), "seed 9173 case {case}");
        for cell in &cells {
            let mid = cell.iter().fold(P::default(), |p, q| p + *q) * (1. / cell.len() as f64);
            let z = map.zone_at(mid).unwrap();
            // Snap rounding moves boundaries by at most a grid cell; allow its perimeter-area envelope.
            assert!(
                (map.zones[z].area - area(cell)).abs() < 4.,
                "case {case}, {} vs {}",
                map.zones[z].area,
                area(cell)
            );
        }
        assert!((map.zones.iter().map(|z| z.area).sum::<f64>() - 10000.).abs() < 1e-6);
    }
}

#[test]
fn changing_a_boundarys_geometry_changes_its_zone_id() {
    let a = build(&[rect("same", 0., 0., 10., 10.)], &mut Work::unlimited("review")).unwrap();
    let b = build(&[rect("same", 0., 0., 20., 10.)], &mut Work::unlimited("review")).unwrap();
    assert_ne!(
        a.zones[0].id, b.zones[0].id,
        "02 3.12(4), 03 16.1(5): changed zone geometry needs a new boundary-derived ID"
    );
}

#[test]
fn analytic_holes_parents_sides_retracing_tangency_and_brush_winding() {
    let m = build(&[rect("outer", 0., 0., 20., 20.), rect("inner", 5., 5., 15., 15.)], &mut Work::unlimited("oracle"))
        .unwrap();
    assert_eq!(m.zones.len(), 2);
    let outer = m.zone_at(P::new(1., 1.)).unwrap();
    let inner = m.zone_at(P::new(10., 10.)).unwrap();
    assert_eq!(m.zones[outer].area, 300.);
    assert_eq!(m.zones[inner].area, 100.);
    assert_eq!(m.zones[inner].parent, Some(outer));
    assert_eq!(m.zones[outer].holes[0].zone, Some(inner));
    assert_eq!(m.stroke_sides(0), (Some(outer), None));
    assert_eq!(m.stroke_sides(1), (Some(inner), Some(outer)));
    let m = build(
        &[rect("a", 0., 0., 10., 10.), rect("b", 10., 10., 20., 20.), rect("retrace", 0., 0., 10., 10.)],
        &mut Work::unlimited("oracle"),
    )
    .unwrap();
    assert_eq!(m.zones.len(), 2);
    assert!(m.zones.iter().all(|z| z.area == 100. && z.parent.is_none()));
    // One brush contour with a zero-width bridge and a clockwise hole; non-zero winding is independently known.
    let ring = StrokeIn {
        id: "ring".into(),
        kind: EdgeKind::Brush,
        path: path(
            &[
                (0., 0.),
                (10., 0.),
                (10., 10.),
                (0., 10.),
                (0., 0.),
                (2., 2.),
                (2., 8.),
                (8., 8.),
                (8., 2.),
                (2., 2.),
                (0., 0.),
            ],
            true,
        ),
    };
    let m = build(&[ring], &mut Work::unlimited("oracle")).unwrap();
    assert_eq!(m.zones.len(), 1);
    assert_eq!(m.zones[0].area, 36.);
    assert!(m.zone_at(P::new(5., 5.)).is_some());
    assert!(m.zone_at(P::new(1., 5.)).is_none());
    let face = m.face_at(P::new(1., 5.)).unwrap();
    assert_eq!(m.faces[face].brushes, vec![0]);
}
#[test]
fn topology_changes_at_the_documented_snap_grid_are_repeatable() {
    let collapsed = vec![rect("thin", 0., 0., 10., 0.004)];
    let retained = vec![rect("thin", 0., 0., 10., 0.016)];
    let a = build(&collapsed, &mut Work::unlimited("oracle")).unwrap();
    let b = build(&retained, &mut Work::unlimited("oracle")).unwrap();
    assert!(a.zones.is_empty());
    assert_eq!(b.zones.len(), 1);
    assert_eq!(b.zones[0].area, 10. / 64.);
    assert_eq!(a, build(&collapsed, &mut Work::unlimited("oracle")).unwrap());
    assert_eq!(b, build(&retained, &mut Work::unlimited("oracle")).unwrap());
}
