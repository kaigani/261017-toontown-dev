//! M2 C2.4: unit behaviour of the vectorization pipeline (rings, connectivity, order, filters, threshold, alpha,
//! frame scaling, refusals).
#![allow(clippy::unwrap_used, missing_docs, reason = "test")]

use tt_vector::vectorize::*;
use tt_vector::work::Work;

/// Twice the signed area of a lattice polygon (positive clockwise on screen, image y down).
fn lattice_area2(poly: &[(u32, u32)]) -> i64 {
    let n = poly.len();
    (0..n)
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            i64::from(a.0) * i64::from(b.1) - i64::from(b.0) * i64::from(a.1)
        })
        .sum()
}

fn mask(rows: &[&str]) -> (Vec<bool>, u32, u32) {
    let h = rows.len() as u32;
    let w = rows[0].len() as u32;
    (rows.iter().flat_map(|r| r.chars().map(|c| c == '#')).collect(), w, h)
}

fn work() -> Work {
    Work::new(VECTORIZE_WORK_BUDGET, "vectorizing this image")
}

#[test]
fn a_ring_is_one_region_with_one_hole() {
    let (ink, w, h) = mask(&["#####", "#...#", "#...#", "#####"]);
    let r = trace(&ink, w, h, &mut work()).unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].holes.len(), 1);
    assert_eq!(lattice_area2(&r[0].outer), 2 * 20);
    assert_eq!(lattice_area2(&r[0].holes[0]), -2 * 6);
}

#[test]
fn diagonal_ink_is_connected_and_paper_is_not() {
    // Two ink pixels touching at a corner are one region (8-connected ink).
    let (ink, w, h) = mask(&["#.", ".#"]);
    let r = trace(&ink, w, h, &mut work()).unwrap();
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(r[0].holes.is_empty());
    assert_eq!(lattice_area2(&r[0].outer), 2 * 2);
    // A diagonal paper pair inside ink stays two separate holes (4-connected paper).
    let (ink, w, h) = mask(&["####", "#.##", "##.#", "####"]);
    let r = trace(&ink, w, h, &mut work()).unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].holes.len(), 2, "{r:?}");
}

#[test]
fn separate_regions_come_in_row_major_order() {
    let (ink, w, h) = mask(&["#..#", "....", ".##."]);
    let r = trace(&ink, w, h, &mut work()).unwrap();
    assert_eq!(r.len(), 3);
    assert_eq!(r[0].outer.iter().min(), Some(&(0, 0)));
    assert_eq!(r[1].outer.iter().min(), Some(&(3, 0)));
}

#[test]
fn filters_and_threshold() {
    let g = vec![0u8, 255, 255, 255];
    assert_eq!(filter(&g, 2, 2, Filter::None, &mut work()).unwrap(), g);
    let smooth = filter(&g, 2, 2, Filter::SmoothMore, &mut work()).unwrap();
    assert!(smooth[0] > 0 && smooth[0] < 255, "{smooth:?}");
    assert_eq!(threshold(&[0, 178, 179, 255], 70), vec![true, true, false, false]);
    assert_eq!(threshold(&[0, 254], 0), vec![false, false]);
    assert_eq!(threshold(&[254, 255], 100), vec![true, false]);
}

#[test]
fn transparent_pixels_are_paper() {
    let g = grey(2, 1, &[0, 0, 0, 0, 0, 0, 0, 255], &mut work()).unwrap();
    assert_eq!(g, vec![255, 0]);
}

#[test]
fn an_image_becomes_one_outline_per_region_spanning_the_frame_width() {
    let (ink, w, h) = mask(&["##########", "#........#", "#........#", "##########"]);
    let rgba: Vec<u8> = ink.iter().flat_map(|&i| if i { [0, 0, 0, 255] } else { [255, 255, 255, 255] }).collect();
    let paths = vectorize(w, h, &rgba, Filter::None, 70, &mut work()).unwrap();
    assert_eq!(paths.len(), 1);
    let xs: Vec<f64> = paths[0].anchors.iter().map(|a| a.p.x).collect();
    let (lo, hi) =
        (xs.iter().copied().fold(f64::INFINITY, f64::min), xs.iter().copied().fold(f64::NEG_INFINITY, f64::max));
    assert!(lo < -500.0 && hi > 500.0 && lo >= -600.0 && hi <= 600.0, "{lo} {hi}");
}

#[test]
fn oversize_and_mismatched_input_is_refused() {
    assert_eq!(grey(1, 1, &[0, 0, 0], &mut work()).unwrap_err().code, "drawing.invalidGeometry");
    assert_eq!(grey(8192, 4096, &[], &mut work()).unwrap_err().code, "limit.exceeded");
    let mut tiny = Work::new(10, "vectorizing this image");
    assert_eq!(grey(4, 4, &[0; 64], &mut tiny).unwrap_err().code, "limit.exceeded");
}
