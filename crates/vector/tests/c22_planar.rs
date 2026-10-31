//! The derived planar graph (M2 C2.2): zones of pencil, invisible and brush strokes, holes and parents, brush fills,
//! point location, derived zone IDs and determinism.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "test")]

use tt_model::entities::pen::Tip;
use tt_vector::brush::{BrushPen, outline};
use tt_vector::geom::P;
use tt_vector::path::{Anchor, Path};
use tt_vector::planar::{EdgeKind, LayerMap, StrokeIn, build};
use tt_vector::work::Work;

fn poly(id: &str, kind: EdgeKind, pts: &[(f64, f64)], closed: bool) -> StrokeIn {
    StrokeIn {
        id: id.to_owned(),
        kind,
        path: Path { anchors: pts.iter().map(|&(x, y)| Anchor::corner(P::new(x, y))).collect(), closed },
    }
}

fn circle(id: &str, kind: EdgeKind, c: (f64, f64), r: f64) -> StrokeIn {
    let k = 0.552_284_749_830_793_4 * r;
    let a = |x: f64, y: f64, ix: f64, iy: f64, ox: f64, oy: f64| Anchor {
        p: P::new(c.0 + x, c.1 + y),
        inh: P::new(ix, iy),
        outh: P::new(ox, oy),
    };
    StrokeIn {
        id: id.to_owned(),
        kind,
        path: Path {
            anchors: vec![
                a(r, 0.0, 0.0, -k, 0.0, k),
                a(0.0, r, k, 0.0, -k, 0.0),
                a(-r, 0.0, 0.0, k, 0.0, -k),
                a(0.0, -r, -k, 0.0, k, 0.0),
            ],
            closed: true,
        },
    }
}

fn map(strokes: &[StrokeIn]) -> LayerMap {
    build(strokes, &mut Work::unlimited("test")).unwrap()
}

fn square(id: &str, x0: f64, y0: f64, s: f64) -> StrokeIn {
    poly(id, EdgeKind::Pencil, &[(x0, y0), (x0 + s, y0), (x0 + s, y0 + s), (x0, y0 + s)], true)
}

#[test]
fn four_lines_meeting_at_corners_bound_one_zone() {
    let m = map(&[
        poly("a", EdgeKind::Pencil, &[(0.0, 0.0), (100.0, 0.0)], false),
        poly("b", EdgeKind::Pencil, &[(100.0, 0.0), (100.0, 100.0)], false),
        poly("c", EdgeKind::Invisible, &[(100.0, 100.0), (0.0, 100.0)], false),
        poly("d", EdgeKind::Pencil, &[(0.0, 100.0), (0.0, 0.0)], false),
    ]);
    assert_eq!(m.zones.len(), 1);
    let z = &m.zones[0];
    assert!((z.area - 10_000.0).abs() < 1e-9, "{}", z.area);
    assert_eq!(z.boundary, vec!["a", "b", "c", "d"]);
    assert!(z.visible_boundary);
    assert_eq!(m.zone_at(P::new(50.0, 50.0)), Some(0));
    assert_eq!(m.zone_at(P::new(150.0, 50.0)), None);
    // The interior point is inside.
    assert_eq!(m.zone_at(z.interior), Some(0));
    // Line a runs left to right along the bottom: the zone is on its left.
    assert_eq!(m.stroke_sides(0), (Some(0), None));
}

#[test]
fn a_closed_circle_is_one_zone_with_its_area() {
    let m = map(&[circle("c", EdgeKind::Pencil, (0.0, 0.0), 50.0)]);
    assert_eq!(m.zones.len(), 1);
    let area = m.zones[0].area;
    assert!((area - std::f64::consts::PI * 2500.0).abs() < 2.0, "{area}");
    assert_eq!(m.zone_at(P::new(0.0, 0.0)), Some(0));
}

#[test]
fn crossing_squares_make_three_zones() {
    let m = map(&[square("p", 0.0, 0.0, 100.0), square("q", 50.0, 50.0, 100.0)]);
    assert_eq!(m.zones.len(), 3, "{:#?}", m.zones.iter().map(|z| z.area).collect::<Vec<_>>());
    let mut areas: Vec<f64> = m.zones.iter().map(|z| z.area).collect();
    areas.sort_by(f64::total_cmp);
    assert!((areas[0] - 2500.0).abs() < 1e-9 && (areas[1] - 7500.0).abs() < 1e-9 && (areas[2] - 7500.0).abs() < 1e-9);
    let mid = m.zone_at(P::new(75.0, 75.0)).unwrap();
    assert_eq!(m.zones[mid].boundary, vec!["p", "q"]);
}

#[test]
fn a_nested_square_is_a_hole_and_a_child_zone() {
    let m = map(&[square("outer", 0.0, 0.0, 100.0), square("inner", 25.0, 25.0, 50.0)]);
    assert_eq!(m.zones.len(), 2);
    let o = m.zone_at(P::new(5.0, 5.0)).unwrap();
    let i = m.zone_at(P::new(50.0, 50.0)).unwrap();
    assert!((m.zones[o].area - 7500.0).abs() < 1e-9);
    assert_eq!(m.zones[o].holes.len(), 1);
    assert_eq!(m.zones[o].holes[0].boundary, vec!["inner"]);
    assert_eq!(m.zones[o].holes[0].zone, Some(i));
    assert_eq!(m.zones[i].parent, Some(o));
    assert_eq!(m.zones[o].parent, None);
}

#[test]
fn a_dangling_line_inside_a_zone_does_not_split_it() {
    let m = map(&[square("s", 0.0, 0.0, 100.0), poly("t", EdgeKind::Pencil, &[(20.0, 50.0), (80.0, 50.0)], false)]);
    assert_eq!(m.zones.len(), 1);
    assert!((m.zones[0].area - 10_000.0).abs() < 1e-9);
    // A line across splits it.
    let m = map(&[square("s", 0.0, 0.0, 100.0), poly("t", EdgeKind::Pencil, &[(-10.0, 50.0), (110.0, 50.0)], false)]);
    assert_eq!(m.zones.len(), 2);
}

#[test]
fn a_brush_ring_fills_itself_and_bounds_its_hole() {
    let spine = Path { anchors: circle("x", EdgeKind::Brush, (0.0, 0.0), 40.0).path.anchors, closed: true };
    let pen = BrushPen { tip: Tip::Round, min_size: 8.0, max_size: 8.0, tip_angle: 0.0 };
    let ring = outline(&spine, &[[0.0, 1.0], [1.0, 1.0]], &pen).unwrap();
    let m = map(&[StrokeIn { id: "b".into(), kind: EdgeKind::Brush, path: ring }]);
    // The ring itself is the brush fill; its hole is a zone.
    assert_eq!(m.zones.len(), 1, "{:?}", m.zones.iter().map(|z| (z.area, z.interior)).collect::<Vec<_>>());
    assert_eq!(m.zone_at(P::new(0.0, 0.0)), Some(0));
    assert_eq!(m.zone_at(P::new(40.0, 0.0)), None);
    assert!(m.zones[0].pieces.iter().all(|p| m.strokes[p.stroke].1 == EdgeKind::Brush));
}

#[test]
fn a_pencil_line_across_a_brush_stroke_meets_it_in_the_graph() {
    let spine =
        Path { anchors: vec![Anchor::corner(P::new(-50.0, 0.0)), Anchor::corner(P::new(50.0, 0.0))], closed: false };
    let pen = BrushPen { tip: Tip::Round, min_size: 10.0, max_size: 10.0, tip_angle: 0.0 };
    let bar = outline(&spine, &[[0.0, 1.0], [1.0, 1.0]], &pen).unwrap();
    let m = map(&[StrokeIn { id: "bar".into(), kind: EdgeKind::Brush, path: bar }, square("box", -40.0, -40.0, 80.0)]);
    // The bar splits the box into a top and a bottom zone; its own inside is no zone.
    assert_eq!(m.zones.len(), 2);
    let top = m.zone_at(P::new(0.0, 30.0)).unwrap();
    let bottom = m.zone_at(P::new(0.0, -30.0)).unwrap();
    assert_ne!(top, bottom);
    assert_eq!(m.zone_at(P::new(0.0, 0.0)), None);
    assert_eq!(m.zones[top].boundary, vec!["bar", "box"]);
}

#[test]
fn zone_ids_follow_the_boundary_only_and_are_reproducible() {
    let a = map(&[square("s", 0.0, 0.0, 100.0)]);
    let b = map(&[square("s", 0.0, 0.0, 100.0), square("far", 1000.0, 1000.0, 10.0)]);
    let c = map(&[square("t", 0.0, 0.0, 100.0)]);
    assert_eq!(a, map(&[square("s", 0.0, 0.0, 100.0)]));
    let za = &a.zones[a.zone_at(P::new(50.0, 50.0)).unwrap()].id;
    let zb = &b.zones[b.zone_at(P::new(50.0, 50.0)).unwrap()].id;
    let zc = &c.zones[c.zone_at(P::new(50.0, 50.0)).unwrap()].id;
    assert_eq!(za, zb, "an unrelated stroke leaves the zone's ID alone");
    assert_ne!(za, zc, "a different boundary stroke gives another ID");
    assert!(za.starts_with("zn_") && za.len() == 29);
}

#[test]
fn a_figure_eight_line_bounds_two_zones() {
    let m = map(&[poly("eight", EdgeKind::Pencil, &[(0.0, 0.0), (100.0, 100.0), (100.0, 0.0), (0.0, 100.0)], true)]);
    assert_eq!(m.zones.len(), 2);
    for z in &m.zones {
        assert!((z.area - 2500.0).abs() < 1e-9, "{}", z.area);
    }
}

#[test]
fn the_work_budget_refuses_large_input() {
    let strokes: Vec<StrokeIn> =
        (0..200)
            .map(|i| poly(&format!("h{i}"), EdgeKind::Pencil, &[(0.0, f64::from(i)), (500.0, f64::from(i))], false))
            .chain((0..200).map(|i| {
                poly(&format!("v{i}"), EdgeKind::Pencil, &[(f64::from(i), -1.0), (f64::from(i), 500.0)], false)
            }))
            .collect();
    let e = build(&strokes, &mut Work::new(10_000, "the planar map")).unwrap_err();
    assert_eq!(e.code, "limit.exceeded");
    let m = build(&strokes, &mut Work::unlimited("t")).unwrap();
    assert_eq!(m.zones.len(), 199 * 199);
}
