//! M2 C2.3 round 1 (C23-R1-08): the Cutter, Optimize and selection helpers decide crossings and containment with the
//! planar map's exact grid predicates. Corpus: determinant ±1 at large coordinates (both signs), endpoint contact,
//! collinear overlap, containment on and off the boundary.
#![allow(clippy::unwrap_used, missing_docs, reason = "test")]

use tt_vector::geom::P;
use tt_vector::ops::cut;
use tt_vector::planar::predicates;

fn g(x: i64, y: i64) -> P {
    P::new(x as f64 / 64.0, y as f64 / 64.0)
}

#[test]
fn unit_determinants_far_from_the_origin_are_decided_exactly() {
    let n = 500_000_000i64;
    // Determinant -1 and +1 on the grid: both segments cross (and the f64 cross product rounds to 0).
    for (a, b, c) in [(n, n - 1, n - 2), (n - 2, n - 1, n)] {
        let line = [g(-a, -b), g(a, b)];
        let other = [g(-b, -c), g(b, c)];
        assert!(cut::polylines_cross(&line, &other), "{a} {b} {c}");
        assert!(predicates::segments_cross(line[0], line[1], other[0], other[1]));
    }
}

#[test]
fn endpoint_contact_and_collinear_overlap_are_not_crossings_but_do_meet() {
    let sq = [g(0, 0), g(640, 0), g(640, 640), g(0, 640)];
    // A segment ending on the square's edge: no proper crossing, but it meets the polygon (boundary counts).
    assert!(!cut::polylines_cross(&[g(640, 320), g(1280, 320)], &[g(640, 0), g(640, 640)]));
    assert!(cut::polyline_meets(&[g(640, 320), g(1280, 320)], &sq));
    // Collinear overlap along an edge.
    assert!(!cut::polylines_cross(&[g(0, 0), g(320, 0)], &[g(160, 0), g(960, 0)]));
    assert!(cut::polyline_meets(&[g(160, 0), g(960, 0)], &sq));
    // Polygons sharing only an edge meet; disjoint ones do not.
    assert!(cut::polygons_meet(&sq, &[g(640, 0), g(1280, 0), g(1280, 640), g(640, 640)]));
    assert!(!cut::polygons_meet(&sq, &[g(704, 0), g(1280, 0), g(1280, 640), g(704, 640)]));
}

#[test]
fn containment_is_exact_on_and_off_the_boundary() {
    let n = 400_000_000i64;
    let tri = [g(0, 0), g(n, 1), g(0, 2)];
    // (1, 1) lies inside the thin triangle; (n, 1) is its vertex (boundary); (n + 1, 1) is outside.
    assert!(cut::inside(&tri, g(1, 1)));
    assert!(!cut::inside(&tri, g(n, 1)));
    assert!(predicates::inside_polygon(&tri, g(n, 1), true));
    assert!(!cut::inside(&tri, g(n + 1, 1)));
}
