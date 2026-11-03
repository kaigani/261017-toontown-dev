//! M2 C2.4 (02 §11.2): the vectorization tracer against an independent oracle on seeded random masks, and the whole
//! pipeline on random images (never a panic; a typed error or outlines).
//!
//! Oracle: ink regions are the 8-connected components of the ink pixels (breadth-first search here); each region's
//! outer boundary minus its holes encloses exactly its pixels; the holes are the 4-connected paper components that do
//! not touch the image border, each inside exactly one region.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

use std::collections::VecDeque;

use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use tt_vector::vectorize::{self, Filter, VECTORIZE_WORK_BUDGET};
use tt_vector::work::Work;

fn runner(seed: &[u8; 32], cases: u32) -> TestRunner {
    TestRunner::new_with_rng(Config { cases, ..Config::default() }, TestRng::from_seed(RngAlgorithm::ChaCha, seed))
}

fn work() -> Work {
    Work::new(VECTORIZE_WORK_BUDGET, "vectorizing this image")
}

/// Components of the cells where `keep` holds, 8- or 4-connected: per component, its cells.
fn components(w: usize, h: usize, keep: &dyn Fn(usize, usize) -> bool, eight: bool) -> Vec<Vec<(usize, usize)>> {
    let mut seen = vec![false; w * h];
    let mut out = Vec::new();
    let steps: &[(isize, isize)] = if eight {
        &[(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)]
    } else {
        &[(0, -1), (-1, 0), (1, 0), (0, 1)]
    };
    for y in 0..h {
        for x in 0..w {
            if seen[y * w + x] || !keep(x, y) {
                continue;
            }
            let mut comp = Vec::new();
            let mut q = VecDeque::from([(x, y)]);
            seen[y * w + x] = true;
            while let Some((cx, cy)) = q.pop_front() {
                comp.push((cx, cy));
                for (dx, dy) in steps {
                    let (nx, ny) = (cx as isize + dx, cy as isize + dy);
                    if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
                        continue;
                    }
                    let (nx, ny) = (nx as usize, ny as usize);
                    if !seen[ny * w + nx] && keep(nx, ny) {
                        seen[ny * w + nx] = true;
                        q.push_back((nx, ny));
                    }
                }
            }
            out.push(comp);
        }
    }
    out
}

fn area2(poly: &[(u32, u32)]) -> i64 {
    let n = poly.len();
    (0..n)
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            i64::from(a.0) * i64::from(b.1) - i64::from(b.0) * i64::from(a.1)
        })
        .sum()
}

fn masks() -> impl Strategy<Value = (usize, usize, Vec<bool>)> {
    (1usize..=20, 1usize..=20, 0u8..=100)
        .prop_flat_map(|(w, h, density)| (Just(w), Just(h), prop::collection::vec(0u8..100, w * h), Just(density)))
        .prop_map(|(w, h, cells, density)| (w, h, cells.into_iter().map(|c| c < density).collect()))
}

#[test]
fn traced_regions_match_the_independent_oracle() {
    runner(b"toontown-m2-c24-trace-oracle-001", 512)
        .run(&masks(), |(w, h, ink)| {
            let regions = vectorize::trace(&ink, w as u32, h as u32, &mut work()).unwrap();
            let comps = components(w, h, &|x, y| ink[y * w + x], true);
            prop_assert_eq!(regions.len(), comps.len(), "8-connected ink regions");
            for (r, comp) in regions.iter().zip(&comps) {
                // Regions come in row-major order of their first pixel, as the oracle finds them.
                let first = comp.iter().min_by_key(|(x, y)| (*y, *x)).copied().unwrap();
                prop_assert!(r.outer.contains(&(first.0 as u32, first.1 as u32)), "region order");
                let enclosed = area2(&r.outer) + r.holes.iter().map(|hl| area2(hl)).sum::<i64>();
                prop_assert_eq!(enclosed, 2 * comp.len() as i64, "outer minus holes encloses the region's pixels");
                prop_assert!(r.holes.iter().all(|hl| area2(hl) < 0), "holes run the other way");
            }
            let paper = components(w, h, &|x, y| !ink[y * w + x], false);
            let enclosed = paper.iter().filter(|c| c.iter().all(|&(x, y)| x > 0 && y > 0 && x < w - 1 && y < h - 1));
            let holes: usize = regions.iter().map(|r| r.holes.len()).sum();
            prop_assert_eq!(holes, enclosed.count(), "one hole per enclosed 4-connected paper component");
            Ok(())
        })
        .unwrap();
}

#[test]
fn random_images_vectorize_or_refuse_typed_and_never_panic() {
    let images = (1u32..=24, 1u32..=24, 0u8..=100, 0usize..5).prop_flat_map(|(w, h, t, f)| {
        (Just(w), Just(h), Just(t), Just(f), prop::collection::vec(any::<u8>(), (w * h * 4) as usize))
    });
    runner(b"toontown-m2-c24-vectorize-fuzz01", 256)
        .run(&images, |(w, h, t, f, rgba)| {
            let filter = [Filter::None, Filter::Smooth, Filter::SmoothMore, Filter::EdgeEnhance, Filter::Sharpen][f];
            match vectorize::vectorize(w, h, &rgba, filter, t, &mut work()) {
                Ok(paths) => {
                    for p in &paths {
                        prop_assert!(p.closed && !p.anchors.is_empty());
                        prop_assert!(
                            p.anchors
                                .iter()
                                .all(|a| a.p.x.abs() <= 600.0 + 1e-6 && a.p.x.is_finite() && a.p.y.is_finite())
                        );
                    }
                }
                Err(e) => prop_assert!(e.code == "limit.exceeded" || e.code == "drawing.invalidGeometry", "{e:?}"),
            }
            Ok(())
        })
        .unwrap();
}

#[test]
fn a_tiny_budget_refuses_before_finishing() {
    let rgba = vec![0u8; 64 * 64 * 4];
    let mut w = Work::new(1000, "vectorizing this image");
    assert_eq!(vectorize::vectorize(64, 64, &rgba, Filter::None, 70, &mut w).unwrap_err().code, "limit.exceeded");
}
