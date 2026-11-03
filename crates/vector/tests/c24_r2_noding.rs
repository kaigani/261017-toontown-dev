//! Closed-fold traversal conservation, shrunk from the independent round-2 Flatten route property.
#![allow(clippy::unwrap_used, clippy::panic, clippy::print_stdout, missing_docs, reason = "review test")]
use tt_vector::geom::P;
use tt_vector::path::{Anchor, Path};
use tt_vector::planar::EdgeKind;
use tt_vector::planar::node::{NodeStroke, Outcome, node};
use tt_vector::work::Work;

fn path(points: &[(i64, i64)], closed: bool) -> Path {
    Path { anchors: points.iter().map(|&(x, y)| Anchor::corner(P::new(x as f64, y as f64))).collect(), closed }
}

// Independent transversal: at x=10 the folded path passes through (10,72.5) once each way.
// This point is far from every added-stroke intersection; snapping cannot change the multiplicity here.
fn crossings(paths: &[Path]) -> usize {
    let mut count = 0;
    for p in paths {
        let mut edges = p.anchors.windows(2).map(|w| (w[0].p, w[1].p)).collect::<Vec<_>>();
        if p.closed {
            edges.push((p.anchors[p.anchors.len() - 1].p, p.anchors[0].p));
        }
        for (a, b) in edges {
            if (a.x < 10. && b.x > 10.) || (a.x > 10. && b.x < 10.) {
                let y = a.y + (10. - a.x) * (b.y - a.y) / (b.x - a.x);
                if (y - 72.5).abs() < 1e-12 {
                    count += 1;
                }
            }
        }
    }
    count
}

fn control(repeated_end: bool) {
    let fold = if repeated_end { vec![(0, 80), (80, 20), (0, 80)] } else { vec![(0, 80), (80, 20)] };
    let input = vec![
        NodeStroke { id: "a_fold".into(), kind: EdgeKind::Pencil, path: path(&fold, true), changed: true },
        NodeStroke { id: "b0".into(), kind: EdgeKind::Pencil, path: path(&[(0, 40), (40, 60)], false), changed: false },
        NodeStroke {
            id: "b1".into(),
            kind: EdgeKind::Pencil,
            path: path(&[(40, 60), (80, 80), (40, 20)], false),
            changed: false,
        },
        NodeStroke {
            id: "b2".into(),
            kind: EdgeKind::Pencil,
            path: path(&[(20, 40), (40, 60)], false),
            changed: false,
        },
    ];
    assert_eq!(crossings(std::slice::from_ref(&input[0].path)), 2);
    let out = node(&input, &mut Work::unlimited("review")).unwrap();
    let pieces = match out.outcomes.get(&0) {
        Some(Outcome::Split(p)) => p.clone(),
        Some(Outcome::Moved(p)) => vec![p.clone()],
        None => vec![input[0].path.clone()],
    };
    let after = crossings(&pieces);
    println!("repeated terminal anchor={repeated_end}; before=2; after={after}; pieces={pieces:?}");
    assert_eq!(after, 2, "noding must not duplicate the source fold's traversals away from all junctions");
}

#[test]
fn a_closed_fold_with_a_repeated_terminal_anchor_keeps_two_passes() {
    control(true);
}

#[test]
fn a_closed_fold_without_a_repeated_terminal_anchor_keeps_two_passes() {
    control(false);
}

#[test]
fn a_closed_zero_length_last_segment_is_not_part_of_the_nonzero_closing_edge_parameter_range() {
    let p = path(&[(0, 80), (80, 20), (0, 80)], true);
    let f = tt_vector::planar::flat::flatten(&p, &mut Work::unlimited("review")).unwrap();
    let edges = f.edges(p.segments().len());
    let (_, _, from, to) = edges.last().unwrap();
    // Segment 1 runs (80,20) -> (0,80), ending at parameter 2. Segment 2 is the zero-length closing segment.
    assert_eq!((*from, *to), (1.0, 2.0), "the edge parameter must not run into the zero-length closing segment");
}
