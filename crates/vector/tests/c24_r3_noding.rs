//! Round 3 repeated-anchor variants; compare independent source and output traversals.
#![allow(clippy::unwrap_used, clippy::panic, clippy::print_stdout, missing_docs, reason = "review test")]
use tt_vector::geom::P;
use tt_vector::path::{Anchor, Path};
use tt_vector::planar::EdgeKind;
use tt_vector::planar::flat::flatten;
use tt_vector::planar::node::{NodeStroke, Outcome, node};
use tt_vector::work::Work;

fn path(points: &[(i64, i64)], dx: f64, dy: f64, closed: bool) -> Path {
    Path {
        anchors: points.iter().map(|&(x, y)| Anchor::corner(P::new(x as f64 + dx, y as f64 + dy))).collect(),
        closed,
    }
}

fn passes(paths: &[Path], dx: f64, dy: f64) -> usize {
    let mut count = 0;
    for p in paths {
        let mut edges: Vec<_> = p.anchors.windows(2).map(|w| (w[0].p, w[1].p)).collect();
        if p.closed {
            edges.push((p.anchors.last().unwrap().p, p.anchors[0].p));
        }
        for (a, b) in edges {
            let x = dx + 10.0;
            if (a.x < x && x < b.x) || (b.x < x && x < a.x) {
                let y = a.y + (x - a.x) * (b.y - a.y) / (b.x - a.x);
                if (y - dy - 72.5).abs() < 1e-9 {
                    count += 1;
                }
            }
        }
    }
    count
}

#[test]
fn repeated_interior_consecutive_and_terminal_anchors_with_crossings_4096() {
    let variants = [
        vec![(0, 80), (80, 20), (80, 20), (0, 80)],
        vec![(0, 80), (80, 20), (80, 20), (80, 20), (0, 80)],
        vec![(0, 80), (80, 20), (0, 80), (0, 80)],
        vec![(0, 80), (80, 20), (40, 50), (40, 50), (0, 80)],
        vec![(0, 80), (80, 20), (100, 20), (80, 20), (0, 80)],
    ];
    let mut seed = 0xc243_d39f_a18b_6517u64;
    for i in 0..4096 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let dx = (seed as i32 % 5000) as f64;
        let dy = ((seed >> 32) as i32 % 5000) as f64;
        let p = path(&variants[i % variants.len()], dx, dy, true);
        let f = flatten(&p, &mut Work::unlimited("review")).unwrap();
        for (a, b, sa, sb) in f.edges(p.segments().len()) {
            assert_ne!(a, b);
            assert!(sa < sb);
            // A nonzero chord cannot include an intervening zero-length source segment.
            for j in sa.ceil() as usize..sb.floor() as usize {
                let c = p.segments()[j];
                assert_ne!(c.p0, c.p3, "case {i}: {sa}..{sb} spans zero-length segment {j}");
            }
        }
        let input = vec![
            NodeStroke { id: "a_fold".into(), kind: EdgeKind::Pencil, path: p.clone(), changed: i % 2 == 0 },
            NodeStroke {
                id: "b_cross".into(),
                kind: EdgeKind::Pencil,
                path: path(&[(0, 40), (80, 80), (40, 20)], dx, dy, false),
                changed: true,
            },
            NodeStroke {
                id: "c_cross".into(),
                kind: EdgeKind::Pencil,
                path: path(&[(20, 40), (40, 60)], dx, dy, false),
                changed: true,
            },
        ];
        let out = node(&input, &mut Work::unlimited("review")).unwrap();
        let pieces = match out.outcomes.get(&0) {
            Some(Outcome::Split(p)) => p.clone(),
            Some(Outcome::Moved(p)) => vec![p.clone()],
            None => vec![p.clone()],
        };
        assert_eq!(passes(std::slice::from_ref(&p), dx, dy), 2);
        assert_eq!(passes(&pieces, dx, dy), 2, "case {i}: {pieces:?}");
    }
    println!("4096 repeated-anchor variants; all dispatched; source/output multiplicity=2");
}
