//! Noding regressions found by the C24-R1-01 command-route properties (M2 C2.4 round 1), kept as fixed tests.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "test")]
use tt_vector::geom::P;
use tt_vector::path::{Anchor, Path};
use tt_vector::planar::EdgeKind;
use tt_vector::planar::node::{NodeStroke, node};
use tt_vector::work::Work;
fn line(id: &str, pts: &[(i64, i64)], changed: bool) -> NodeStroke {
    NodeStroke {
        id: id.into(),
        kind: EdgeKind::Pencil,
        path: Path {
            anchors: pts.iter().map(|&(x, y)| Anchor::corner(P::new(x as f64 / 64.0, y as f64 / 64.0))).collect(),
            closed: false,
        },
        changed,
    }
}
/// A fold (a stroke running out and back) added through junctions where existing pieces end: the T-junction pass cut only
/// one pass at each such end, so the other pass ran between two rounded junctions and crossed the first at a shallow
/// angle, a spurious junction at (726, 3456). Every pass is cut now: both passes have the same junctions.
#[test]
fn a_fold_through_piece_ends_is_cut_at_every_pass() {
    let pre: Vec<Vec<(i64, i64)>> = vec![
        vec![(1280, 2560), (853, 2560)],
        vec![(853, 2560), (0, 2560), (640, 3200)],
        vec![(640, 3200), (1280, 3840)],
        vec![(1280, 3840), (1920, 4480)],
        vec![(1920, 4480), (2560, 5120)],
        vec![(1280, 3840), (960, 4160)],
        vec![(960, 4160), (853, 4267)],
        vec![(853, 4267), (0, 5120), (366, 4023)],
        vec![(366, 4023), (384, 3968)],
        vec![(384, 3968), (640, 3200)],
        vec![(640, 3200), (853, 2560)],
        vec![(853, 2560), (1280, 1280)],
        vec![(2560, 5120), (3840, 5120), (1920, 4480)],
        vec![(1920, 4480), (960, 4160)],
        vec![(960, 4160), (384, 3968)],
        vec![(384, 3968), (0, 3840), (366, 4023)],
        vec![(366, 4023), (853, 4267)],
        vec![(853, 4267), (2560, 5120)],
    ];
    let mut input: Vec<NodeStroke> = pre.iter().enumerate().map(|(i, p)| line(&format!("p{i:02}"), p, false)).collect();
    input.push(line("new", &[(1280, 5120), (0, 1280), (1280, 5120)], true));
    let out = node(&input, &mut Work::unlimited("t")).unwrap();
    let Some(tt_vector::planar::node::Outcome::Split(pieces)) = out.outcomes.get(&(input.len() - 1)) else {
        panic!("the fold is split")
    };
    let ends: Vec<(i64, i64)> = pieces
        .iter()
        .flat_map(|p| [p.anchors[0].p, p.anchors[p.anchors.len() - 1].p])
        .map(|a| ((a.x * 64.0).round() as i64, (a.y * 64.0).round() as i64))
        .collect();
    let mut distinct = ends.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct, vec![(427, 2560), (640, 3200), (960, 4160), (1024, 4353), (1280, 5120)], "{pieces:?}");
    // Out and back through the same junctions: the pieces pair up in reverse.
    assert_eq!(pieces.len(), 9);
}
