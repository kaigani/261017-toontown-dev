//! Scoped endpoint closure variants.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "independent QA")]
use tt_vector::{
    geom::P,
    path::{Anchor, Path},
    planar::{
        graph::EdgeKind,
        node::{NodeStroke, Outcome, node},
    },
    work::Work,
};
fn s(id: &str, p: &[(f64, f64)], changed: bool) -> NodeStroke {
    NodeStroke {
        id: id.into(),
        kind: EdgeKind::Pencil,
        path: Path { anchors: p.iter().map(|&(x, y)| Anchor::corner(P::new(x, y))).collect(), closed: false },
        changed,
    }
}
#[test]
fn continuation_oblique_parallel_and_original_collinear_cases() {
    for b in [
        vec![(10., 0.), (20., 0.)],
        vec![(-5., 0.02), (5., 0.02)],
        vec![(-5., 0.), (5., 0.)],
        vec![(-5., 0.001), (5., -0.001)],
    ] {
        let r = node(&[s("a", &[(-10., 0.), (10., 0.)], false), s("b", &b, true)], &mut Work::unlimited("r4")).unwrap();
        assert!(r.outcomes.is_empty(), "parallel/continuation: {b:?}: {r:?}");
    }
    for b in [vec![(5. * 3f64.sqrt(), 5.), (0., 0.)], vec![(0., 5.), (0., 0.)]] {
        let r = node(&[s("a", &[(-10., 0.), (10., 0.)], false), s("b", &b, true)], &mut Work::unlimited("r4")).unwrap();
        assert!(matches!(r.outcomes.get(&0),Some(Outcome::Split(p)) if p.len()==2), "{r:?}");
        assert!(!matches!(r.outcomes.get(&1), Some(Outcome::Split(_))));
    }
}
#[test]
fn end_joins_existing_three_stroke_junction_with_unchanged_anchor_authority() {
    let r = node(
        &[
            s("a", &[(-10., 0.), (0., 0.)], false),
            s("b", &[(0., 0.), (10., 0.)], false),
            s("c", &[(0., -10.), (0., 0.)], false),
            s("d", &[(5., 5.), (0.01, 0.01)], true),
        ],
        &mut Work::unlimited("r4"),
    )
    .unwrap();
    let Some(Outcome::Moved(p)) = r.outcomes.get(&3) else { panic!("{r:?}") };
    assert_eq!(p.anchors.last().unwrap().p, P::new(0., 0.));
    assert!(!r.outcomes.values().any(|v| matches!(v, Outcome::Split(_))), "{r:?}");
}
