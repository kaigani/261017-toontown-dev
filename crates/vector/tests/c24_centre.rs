//! M2 C2.4 (ticket C23-T1, 02 §8.5): the chordal-axis centre line of spineless brush outlines — a bar gives one line
//! along its middle with its width, a plus gives branches meeting at a junction, a ring with a bridged hole gives one
//! closed line, short spurs are pruned, and the refusal conditions are typed.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "test")]

use tt_vector::geom::P;
use tt_vector::ops::centre::{CENTRE_WORK_BUDGET, Refusal, centre_line};
use tt_vector::path::{Anchor, Path};
use tt_vector::work::Work;

fn poly(pts: &[(f64, f64)]) -> Path {
    Path { anchors: pts.iter().map(|&(x, y)| Anchor::corner(P::new(x, y))).collect(), closed: true }
}

fn work() -> Work {
    Work::new(CENTRE_WORK_BUDGET, "extracting this centre line")
}

fn span(lines: &[Path]) -> (f64, f64, f64, f64) {
    let pts: Vec<P> = lines.iter().flat_map(|l| l.flatten(0.25)).collect();
    let f = |g: fn(&P) -> f64, m: fn(f64, f64) -> f64, init: f64| pts.iter().map(g).fold(init, m);
    (
        f(|p| p.x, f64::min, f64::INFINITY),
        f(|p| p.x, f64::max, f64::NEG_INFINITY),
        f(|p| p.y, f64::min, f64::INFINITY),
        f(|p| p.y, f64::max, f64::NEG_INFINITY),
    )
}

#[test]
fn a_bar_gives_one_line_along_its_middle_with_its_width() {
    let a = centre_line(&poly(&[(0.0, 0.0), (200.0, 0.0), (200.0, 10.0), (0.0, 10.0)]), &mut work()).unwrap();
    assert_eq!(a.lines.len(), 1, "{:?}", a.lines);
    let (x0, x1, y0, y1) = span(&a.lines);
    assert!(x0 < 10.0 && x1 > 190.0, "{x0} {x1}");
    assert!(y0 > 3.0 && y1 < 7.0, "along the middle: {y0} {y1}");
    assert!((a.width - 10.0).abs() < 1.5, "width {}", a.width);
}

#[test]
fn a_plus_gives_branches_meeting_at_a_junction() {
    let plus = poly(&[
        (40.0, 0.0),
        (60.0, 0.0),
        (60.0, 40.0),
        (100.0, 40.0),
        (100.0, 60.0),
        (60.0, 60.0),
        (60.0, 100.0),
        (40.0, 100.0),
        (40.0, 60.0),
        (0.0, 60.0),
        (0.0, 40.0),
        (40.0, 40.0),
    ]);
    let a = centre_line(&plus, &mut work()).unwrap();
    // Four arms (the corner spurs at each arm's end and the second junction of the centre square are pruned/merged)
    // meeting at the centre; area 3600 ÷ axis length 200 = 18.
    assert_eq!(a.lines.len(), 4, "{:?}", a.lines);
    for l in &a.lines {
        let f = l.flatten(0.25);
        assert!(
            [f[0], f[f.len() - 1]].iter().any(|p| p.dist(P::new(50.0, 50.0)) < 1.0),
            "each arm meets the centre: {f:?}"
        );
    }
    let (x0, x1, y0, y1) = span(&a.lines);
    assert!(x0 < 1.0 && x1 > 99.0 && y0 < 1.0 && y1 > 99.0, "all four arms to their tips: {x0} {x1} {y0} {y1}");
    assert!((a.width - 18.0).abs() < 0.5, "width {}", a.width);
}

#[test]
fn a_frame_split_by_a_bar_gives_three_lines_between_two_junctions() {
    // A 200 × 100 frame 10 wide with a bar at x = 100 between its two holes (bridged, SI-053).
    let frame = poly(&[
        (0.0, 0.0),
        (200.0, 0.0),
        (200.0, 100.0),
        (0.0, 100.0),
        (0.0, 50.0),
        (10.0, 50.0),
        (10.0, 90.0),
        (95.0, 90.0),
        (95.0, 50.0),
        (105.0, 50.0),
        (105.0, 90.0),
        (190.0, 90.0),
        (190.0, 10.0),
        (105.0, 10.0),
        (105.0, 50.0),
        (95.0, 50.0),
        (95.0, 10.0),
        (10.0, 10.0),
        (10.0, 50.0),
        (0.0, 50.0),
    ]);
    let a = centre_line(&frame, &mut work()).unwrap();
    assert_eq!(a.lines.len(), 3, "{:?}", a.lines);
    let ends: Vec<P> = a
        .lines
        .iter()
        .flat_map(|l| {
            let f = l.flatten(0.25);
            [f[0], f[f.len() - 1]]
        })
        .collect();
    for e in &ends {
        assert!(
            e.dist(P::new(100.0, 5.0)) < 3.0 || e.dist(P::new(100.0, 95.0)) < 3.0,
            "every line ends at a T junction: {e:?}"
        );
    }
    // Area 6400 ÷ axis length 650 ≈ 9.85.
    assert!((a.width - 9.85).abs() < 1.0, "width {}", a.width);
}

#[test]
fn a_ring_with_a_bridged_hole_gives_a_closed_line() {
    // Outer square (counter-clockwise), a zero-width bridge to the hole (clockwise) and back (SI-053).
    let ring = poly(&[
        (0.0, 0.0),
        (100.0, 0.0),
        (100.0, 100.0),
        (0.0, 100.0),
        (0.0, 50.0),
        (10.0, 50.0),
        (10.0, 90.0),
        (90.0, 90.0),
        (90.0, 10.0),
        (10.0, 10.0),
        (10.0, 50.0),
        (0.0, 50.0),
    ]);
    let a = centre_line(&ring, &mut work()).unwrap();
    assert!(
        a.lines.iter().any(|l| l.closed),
        "a closed loop around the hole: {:?}",
        a.lines.iter().map(|l| l.closed).collect::<Vec<_>>()
    );
    assert!((a.width - 10.0).abs() < 2.5, "width {}", a.width);
}

#[test]
fn refusals_are_typed() {
    let bowtie = poly(&[(0.0, 0.0), (100.0, 100.0), (100.0, 0.0), (0.0, 100.0)]);
    assert!(matches!(centre_line(&bowtie, &mut work()), Err(Refusal::NotSupported(_))));
    let line = poly(&[(0.0, 0.0), (10.0, 0.0)]);
    assert!(matches!(centre_line(&line, &mut work()), Err(Refusal::NotSupported(_))));
    let bar = poly(&[(0.0, 0.0), (200.0, 0.0), (200.0, 10.0), (0.0, 10.0)]);
    let mut tiny = Work::new(50, "extracting this centre line");
    assert!(matches!(centre_line(&bar, &mut tiny), Err(Refusal::Geometry(e)) if e.code == "limit.exceeded"));
}

#[test]
fn the_result_is_deterministic() {
    let plus = poly(&[
        (40.0, 0.0),
        (60.0, 0.0),
        (60.0, 40.0),
        (100.0, 40.0),
        (100.0, 60.0),
        (60.0, 60.0),
        (60.0, 100.0),
        (40.0, 100.0),
        (40.0, 60.0),
        (0.0, 60.0),
        (0.0, 40.0),
        (40.0, 40.0),
    ]);
    assert_eq!(centre_line(&plus, &mut work()).unwrap(), centre_line(&plus, &mut work()).unwrap());
}
