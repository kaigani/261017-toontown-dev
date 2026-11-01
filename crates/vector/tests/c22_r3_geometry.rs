//! Independent final-round grid, exact-area and accounting probes.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "independent QA")]
use num_rational::BigRational;
use tt_vector::geom::P;
use tt_vector::path::{Anchor, Path};
use tt_vector::planar::exact::overlap_area2;
use tt_vector::planar::fill::{Fill, carry};
use tt_vector::planar::graph::{EdgeKind, LayerMap, StrokeIn, build};
use tt_vector::planar::node::{NodeStroke, Outcome, node};
use tt_vector::work::Work;

fn path(p: &[(f64, f64)], closed: bool) -> Path {
    Path { anchors: p.iter().map(|&(x, y)| Anchor::corner(P::new(x, y))).collect(), closed }
}
fn ns(id: &str, p: &[(f64, f64)], changed: bool) -> NodeStroke {
    NodeStroke { id: id.into(), kind: EdgeKind::Pencil, path: path(p, false), changed }
}
fn line(id: &str, p: &[(f64, f64)]) -> StrokeIn {
    StrokeIn { id: id.into(), kind: EdgeKind::Pencil, path: path(p, true) }
}
fn map(s: &[StrokeIn]) -> LayerMap {
    build(s, &mut Work::unlimited("r3")).unwrap()
}
fn fill(s: &str) -> Option<Fill> {
    Some(Fill { colour_id: Some(s.into()), matrix: None })
}

#[test]
fn exact_near_collinear_crossings_in_every_quadrant_and_both_vertex_forms() {
    let n = 500_000_000f64;
    for sx in [-1., 1.] {
        for sy in [-1., 1.] {
            for vertex in [false, true] {
                let a = (sx * n / 64., sy * (n - 1.) / 64.);
                let b = (sx * (n - 1.) / 64., sy * (n - 2.) / 64.);
                let ap = if vertex { vec![(-a.0, -a.1), (0., 0.), a] } else { vec![(-a.0, -a.1), a] };
                let r = node(
                    &[ns("a", &ap, false), ns("b", &[(-b.0, -b.1), (0., 0.), b], true)],
                    &mut Work::unlimited("r3"),
                )
                .unwrap();
                eprintln!("quadrant {sx},{sy}, both vertices {vertex}: {:?}", r.outcomes.keys());
                for i in 0..2 {
                    assert!(matches!(r.outcomes.get(&i),Some(Outcome::Split(p)) if p.len()==2), "{r:?}");
                }
            }
        }
    }
}

#[test]
fn exact_interior_endpoint_and_snapped_collinear_controls() {
    let r = node(
        &[ns("a", &[(-10., 0.), (10., 0.)], false), ns("b", &[(0., 5.), (0., 0.)], true)],
        &mut Work::unlimited("r3"),
    )
    .unwrap();
    assert!(matches!(r.outcomes.get(&0),Some(Outcome::Split(p)) if p.len()==2));
    assert!(!matches!(r.outcomes.get(&1), Some(Outcome::Split(_))));
    // Both endpoints lie on the other infinite line; then a raw crossing whose endpoints snap onto that line.
    for p in [vec![(-5., 0.), (5., 0.)], vec![(-5., 0.001), (5., -0.001)]] {
        let r =
            node(&[ns("a", &[(-10., 0.), (10., 0.)], false), ns("b", &p, true)], &mut Work::unlimited("r3")).unwrap();
        assert!(!r.outcomes.values().any(|v| matches!(v, Outcome::Split(_))), "collinear after snapping: {r:?}");
    }
}

#[test]
fn nonconvex_holed_sloping_shared_edges_have_analytic_areas() {
    // Unit-determinant shear (x,y)->(x+y,y) preserves area. L has area 300, hole 4, total 296.
    let shear = |r: &[[i64; 2]]| r.iter().map(|p| [p[0] + p[1], p[1]]).collect::<Vec<_>>();
    let outer = shear(&[[0, 0], [10, 0], [20, 0], [20, 10], [10, 10], [10, 20], [0, 20]]);
    let hole = shear(&[[2, 2], [2, 4], [4, 4], [4, 2]]);
    let a = vec![outer, hole];
    let b = vec![shear(&[[0, 0], [20, 0], [20, 10], [0, 10]])];
    for (x, y, want) in [(&a, &a, 592), (&a, &b, 392), (&b, &a, 392)] {
        let got = overlap_area2(x, y, &mut Work::unlimited("r3")).unwrap();
        eprintln!("sheared L/hole exact twice-area={got}, expected {want}");
        assert_eq!(got, BigRational::from_integer(want.into()));
    }
}

#[test]
fn three_subcell_differences_and_exact_ties_rank_before_ids() {
    // h=1.02 - x/1_000_000; all three cells lose successively larger fractional corners.
    let new = map(&[line("slope", &[(0., 0.), (1_000_000., 0.), (0., 65. / 64.)])]);
    let cells = (0..3)
        .map(|i| {
            let x = i as f64;
            line(&format!("cell{i}"), &[(x, 0.), (x + 1., 0.), (x + 1., 65. / 64.), (x, 65. / 64.)])
        })
        .collect::<Vec<_>>();
    let old = map(&cells);
    let first = old.zone_at(P::new(0.5, 0.5)).unwrap();
    let f = old
        .zones
        .iter()
        .enumerate()
        .map(|(i, _)| fill(if i == first { "largest" } else { "smaller" }))
        .collect::<Vec<_>>();
    assert_eq!(carry(&old, &f, &new, &mut Work::unlimited("r3")).unwrap(), vec![fill("largest")]);
    let even = map(&[line("even", &[(0., 0.), (3., 0.), (3., 65. / 64.), (0., 65. / 64.)])]);
    let f = old.zones.iter().map(|z| fill(&z.id)).collect::<Vec<_>>();
    let lowest = old.zones.iter().map(|z| z.id.as_str()).min().unwrap();
    assert_eq!(carry(&old, &f, &even, &mut Work::unlimited("r3")).unwrap(), vec![fill(lowest)]);
}

#[test]
fn large_candidate_set_charges_and_refuses_without_partial_result() {
    let cells = (0..128)
        .map(|i| {
            let x = i as f64 * 2.;
            line(&format!("cell{i}"), &[(x, 0.), (x + 1., 0.), (x + 1., 1.), (x, 1.)])
        })
        .collect::<Vec<_>>();
    let old = map(&cells);
    let new = map(&[line("all", &[(0., 0.), (256., 0.), (256., 2.), (0., 2.)])]);
    let f = vec![fill("red"); old.zones.len()];
    let mut full = Work::unlimited("r3");
    assert_eq!(carry(&old, &f, &new, &mut full).unwrap(), vec![fill("red")]);
    let mut limited = Work::new(full.used() / 2, "r3 candidate refusal");
    assert_eq!(carry(&old, &f, &new, &mut limited).unwrap_err().code, "limit.exceeded");
    eprintln!("candidates={}, full units={}, refused units={}", old.zones.len(), full.used(), limited.used());
}
