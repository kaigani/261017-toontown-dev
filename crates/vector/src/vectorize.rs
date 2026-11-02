//! Import and vectorize (02 §11.2; M2 C2.4): a bitmap becomes brush-type shapes in one layer.
//!
//! The pipeline is exact and integer up to the fit, so its result is identical on every platform:
//!
//! 1. **Greyscale** of the straight-alpha RGBA input composited over white (BT.601 integer weights).
//! 2. **Filter** ([`Filter`]): an integer 3×3 kernel with clamped edges.
//! 3. **Threshold:** a pixel is ink when its grey level is below `threshold` % of white.
//! 4. **Trace:** ink is 8-connected and paper 4-connected, so boundaries never cross. Every pixel side between ink and
//!    paper is a unit edge of the pixel-corner lattice, directed with ink on its right in image coordinates; at a
//!    saddle vertex (two diagonal ink pixels) the walk turns left onto the other ink pixel. Each closed walk is an outer boundary or
//!    a hole of exactly one ink region (the region of the ink pixel on its first edge, from an 8-connected labelling).
//! 5. **Shape:** each boundary is taken through the midpoints of its unit edges (removing the half-pixel staircase),
//!    mapped so the image width spans 1200 du centred on the origin (y up), the holes joined to the outer boundary
//!    with zero-width bridges (10 §4.9 stores one closed path per stroke; SI-053), and fitted with Bézier curves at
//!    smoothness 3 (02 §3.3).
//!
//! Every pass charges one work account ([`VECTORIZE_WORK_BUDGET`] per image), which also observes the watchdog's
//! cancellation, so an image too large or too detailed is refused before the document worker is held too long.

use crate::fit::{self, Sample};
use crate::geom::{P, signed_area};
use crate::path::{GeomError, MAX_ANCHORS, Path};
use crate::work::Work;

/// The work one image's vectorization may do, in units of about one pixel or vertex visit.
pub const VECTORIZE_WORK_BUDGET: u64 = 1_500_000_000;

/// The largest image vectorized, in pixels (larger decoded images are refused with `limit.exceeded`): 16 M pixels,
/// e.g. an A4 page scanned at 400 dpi.
pub const MAX_PIXELS: u64 = 16 << 20;

/// The width the result spans: the camera frame width, 12 fields (02 §11.2).
pub const FRAME_WIDTH: f64 = 1200.0;

/// The fitting smoothness of traced boundaries (02 §11.2).
pub const SMOOTHNESS: f64 = 3.0;

/// The pre-filter (02 §11.2 Settings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    /// No filter.
    #[default]
    None,
    /// Light mean blur: centre weight 4, neighbours 1 (÷ 12).
    Smooth,
    /// 3×3 box mean (÷ 9).
    SmoothMore,
    /// High-pass edge enhancement: centre 10, neighbours −1 (÷ 2).
    EdgeEnhance,
    /// Sharpening: centre 32, neighbours −2 (÷ 16).
    Sharpen,
}

impl Filter {
    /// `(centre weight, neighbour weight, divisor)`.
    fn kernel(self) -> Option<(i32, i32, i32)> {
        match self {
            Self::None => None,
            Self::Smooth => Some((4, 1, 12)),
            Self::SmoothMore => Some((1, 1, 9)),
            Self::EdgeEnhance => Some((10, -1, 2)),
            Self::Sharpen => Some((32, -2, 16)),
        }
    }
}

fn charge(work: &mut Work, n: usize) -> Result<(), GeomError> {
    work.spend_n(n)
}

fn too_large(what: &str, actual: u64, maximum: u64) -> GeomError {
    GeomError {
        code: "limit.exceeded",
        message: format!("the image has {actual} {what}; vectorization accepts at most {maximum}"),
        pointer: String::new(),
    }
}

/// The grey levels of straight-alpha RGBA pixels composited over white.
///
/// # Errors
/// `drawing.invalidGeometry` when `rgba` is not `width × height × 4` bytes; `limit.exceeded` past [`MAX_PIXELS`] or
/// the work budget.
pub fn grey(width: u32, height: u32, rgba: &[u8], work: &mut Work) -> Result<Vec<u8>, GeomError> {
    let pixels = u64::from(width) * u64::from(height);
    if pixels > MAX_PIXELS {
        return Err(too_large("pixels", pixels, MAX_PIXELS));
    }
    let n = usize::try_from(pixels).map_err(|_| too_large("pixels", pixels, MAX_PIXELS))?;
    if width == 0 || height == 0 || rgba.len() != n * 4 {
        return Err(GeomError::invalid("/image", "the pixel buffer does not match the image size"));
    }
    charge(work, n)?;
    Ok(rgba
        .as_chunks::<4>()
        .0
        .iter()
        .map(|&[r, g, b, a]| {
            let y = (299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b) + 500) / 1000;
            let a = u32::from(a);
            u8::try_from((y * a + 255 * (255 - a) + 127) / 255).unwrap_or(255)
        })
        .collect())
}

/// Applies `filter` to a grey image.
///
/// # Errors
/// `limit.exceeded` past the work budget.
pub fn filter(grey: &[u8], width: u32, height: u32, filter: Filter, work: &mut Work) -> Result<Vec<u8>, GeomError> {
    let Some((centre, side, div)) = filter.kernel() else { return Ok(grey.to_vec()) };
    let (w, h) = (width as usize, height as usize);
    charge(work, grey.len().saturating_mul(9))?;
    let at = |x: isize, y: isize| -> i32 {
        let cx = x.clamp(0, w as isize - 1) as usize;
        let cy = y.clamp(0, h as isize - 1) as usize;
        i32::from(grey.get(cy * w + cx).copied().unwrap_or(255))
    };
    let mut out = Vec::with_capacity(grey.len());
    for y in 0..h as isize {
        for x in 0..w as isize {
            let mut sum = 0;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    sum += at(x + dx, y + dy) * if dx == 0 && dy == 0 { centre } else { side };
                }
            }
            out.push(u8::try_from((sum + div / 2).div_euclid(div).clamp(0, 255)).unwrap_or(255));
        }
    }
    Ok(out)
}

/// Ink pixels: grey level below `threshold` % (0–100) of white.
#[must_use]
pub fn threshold(grey: &[u8], threshold: u8) -> Vec<bool> {
    let t = u32::from(threshold.min(100)) * 255;
    grey.iter().map(|&g| u32::from(g) * 100 < t).collect()
}

/// One ink region: its outer boundary and its holes, as closed lattice polygons (pixel-corner coordinates, image y
/// down) through every unit step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    /// The outer boundary.
    pub outer: Vec<(u32, u32)>,
    /// The holes (paper inside the region).
    pub holes: Vec<Vec<(u32, u32)>>,
}

const E: u8 = 1;
const S: u8 = 2;
const W: u8 = 4;
const N: u8 = 8;

/// Traces the ink regions of a `width × height` ink mask, in a deterministic order (by the first pixel of each region
/// in row-major order).
///
/// # Errors
/// `drawing.invalidGeometry` for a mask of the wrong size; `limit.exceeded` past the work budget.
pub fn trace(ink: &[bool], width: u32, height: u32, work: &mut Work) -> Result<Vec<Region>, GeomError> {
    let (w, h) = (width as usize, height as usize);
    if ink.len() != w * h {
        return Err(GeomError::invalid("/image", "the ink mask does not match the image size"));
    }
    let is_ink = |x: isize, y: isize| {
        x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h && ink[y as usize * w + x as usize]
    };
    // 8-connected labels of the ink pixels (0: paper), in order of each region's first pixel.
    charge(work, ink.len().saturating_mul(2))?;
    let mut label = vec![0u32; w * h];
    let mut regions = 0u32;
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for start in 0..w * h {
        if !ink[start] || label[start] != 0 {
            continue;
        }
        regions += 1;
        label[start] = regions;
        stack.push((start % w, start / w));
        while let Some((x, y)) = stack.pop() {
            for (dx, dy) in [(-1isize, -1isize), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
                let (nx, ny) = (x as isize + dx, y as isize + dy);
                if is_ink(nx, ny) {
                    let i = ny as usize * w + nx as usize;
                    if label[i] == 0 {
                        label[i] = regions;
                        stack.push((nx as usize, ny as usize));
                    }
                }
            }
        }
    }
    // Outgoing unit edges at each lattice vertex ((w + 1) × (h + 1)), ink on the right (image coordinates).
    let vw = w + 1;
    let mut out = vec![0u8; vw * (h + 1)];
    charge(work, ink.len().saturating_mul(4))?;
    for y in 0..h {
        for x in 0..w {
            if !ink[y * w + x] {
                continue;
            }
            let (xi, yi) = (x as isize, y as isize);
            if !is_ink(xi, yi - 1) {
                out[y * vw + x] |= E; // top side, west to east
            }
            if !is_ink(xi + 1, yi) {
                out[y * vw + x + 1] |= S; // right side, north to south
            }
            if !is_ink(xi, yi + 1) {
                out[(y + 1) * vw + x + 1] |= W; // bottom side, east to west
            }
            if !is_ink(xi - 1, yi) {
                out[(y + 1) * vw + x] |= N; // left side, south to north
            }
        }
    }
    let mut outers: Vec<Option<Vec<(u32, u32)>>> = vec![None; regions as usize];
    let mut holes: Vec<Vec<Vec<(u32, u32)>>> = vec![Vec::new(); regions as usize];
    // Each edge's successor depends only on the original edges at its end vertex (at a saddle, the left turn onto the
    // diagonal ink pixel), so the boundaries are exactly the cycles of that successor map.
    let left = |d: u8| match d {
        E => N,
        S => E,
        W => S,
        _ => W,
    };
    let mut used = vec![0u8; out.len()];
    let total_edges: usize = out.iter().map(|m| m.count_ones() as usize).sum();
    for v0 in 0..out.len() {
        for first in [E, S, W, N] {
            if out[v0] & first == 0 || used[v0] & first != 0 {
                continue;
            }
            let (x0, y0) = (v0 % vw, v0 / vw);
            // The ink pixel on the right of the first edge names the region.
            let px = match first {
                E => Some((x0, y0)),
                S => x0.checked_sub(1).map(|x| (x, y0)),
                W => x0.checked_sub(1).zip(y0.checked_sub(1)),
                _ => y0.checked_sub(1).map(|y| (x0, y)),
            };
            let region = px.and_then(|(x, y)| label.get(y * w + x)).copied().unwrap_or(0);
            let mut poly: Vec<(u32, u32)> = Vec::new();
            let (mut v, mut d) = (v0, first);
            loop {
                used[v] |= d;
                poly.push(((v % vw) as u32, (v / vw) as u32));
                v = match d {
                    E => v + 1,
                    S => v + vw,
                    W => v - 1,
                    _ => v - vw,
                };
                let m = out[v];
                d = match m.count_ones() {
                    1 => m,
                    // A saddle: two diagonal ink pixels. Turning left crosses to the other one (8-connected ink).
                    2 => left(d),
                    _ => return Err(GeomError::invalid("/image", "internal: an open boundary")),
                };
                if (v == v0 && d == first) || poly.len() > total_edges {
                    break;
                }
            }
            charge(work, poly.len())?;
            let Some(slot) = (region as usize).checked_sub(1) else { continue };
            // Outer boundaries run clockwise on screen (positive area with y down); holes the other way.
            if lattice_area2(&poly) > 0 {
                if let Some(o) = outers.get_mut(slot) {
                    *o = Some(poly);
                }
            } else if let Some(hs) = holes.get_mut(slot) {
                hs.push(poly);
            }
        }
    }
    Ok(outers.into_iter().zip(holes).filter_map(|(outer, holes)| outer.map(|outer| Region { outer, holes })).collect())
}

/// Twice the signed area of a lattice polygon, positive when it runs clockwise on screen (image y down).
fn lattice_area2(poly: &[(u32, u32)]) -> i64 {
    let n = poly.len();
    (0..n)
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            i64::from(a.0) * i64::from(b.1) - i64::from(b.0) * i64::from(a.1)
        })
        .sum()
}

/// The midpoints of a lattice polygon's unit edges, in drawing coordinates.
fn midpoints(poly: &[(u32, u32)], width: u32, height: u32) -> Vec<P> {
    let s = FRAME_WIDTH / f64::from(width);
    let (cx, cy) = (f64::from(width) / 2.0, f64::from(height) / 2.0);
    let n = poly.len();
    (0..n)
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            let (mx, my) = ((f64::from(a.0) + f64::from(b.0)) / 2.0, (f64::from(a.1) + f64::from(b.1)) / 2.0);
            P::new((mx - cx) * s, (cy - my) * s)
        })
        .collect()
}

/// The closed outline of a region: its boundaries through their unit-edge midpoints, holes bridged to the outer
/// boundary, fitted at [`SMOOTHNESS`].
///
/// # Errors
/// `limit.exceeded` past the work budget or [`MAX_ANCHORS`].
pub fn outline(region: &Region, width: u32, height: u32, work: &mut Work) -> Result<Path, GeomError> {
    let mut contours: Vec<Vec<P>> = Vec::with_capacity(1 + region.holes.len());
    for poly in std::iter::once(&region.outer).chain(&region.holes) {
        charge(work, poly.len())?;
        let c = midpoints(poly, width, height);
        if c.len() >= 3 && signed_area(&c) != 0.0 {
            contours.push(c);
        }
    }
    let Some(merged) = crate::brush::bridge_charged(contours, &mut |n| work.spend(n))? else {
        return Err(GeomError::invalid("/image", "the region has no area"));
    };
    let samples: Vec<Sample> = merged.iter().map(|p| Sample { p: *p, pressure: 1.0 }).collect();
    let budget = VECTORIZE_WORK_BUDGET.saturating_sub(work.used());
    let (f, used) = fit::fit_bounded(&samples, SMOOTHNESS, true, budget)?;
    work.spend(used)?;
    if f.path.anchors.len() > MAX_ANCHORS {
        return Err(GeomError {
            code: "limit.exceeded",
            message: format!("a traced shape needs more than {MAX_ANCHORS} anchors"),
            pointer: "/image".to_owned(),
        });
    }
    Ok(f.path)
}

/// Vectorizes straight-alpha RGBA pixels: one closed outline per ink region, in region order.
///
/// # Errors
/// As [`grey`], [`trace`] and [`outline`].
pub fn vectorize(
    width: u32,
    height: u32,
    rgba: &[u8],
    filter_kind: Filter,
    threshold_pct: u8,
    work: &mut Work,
) -> Result<Vec<Path>, GeomError> {
    let g = grey(width, height, rgba, work)?;
    let g = filter(&g, width, height, filter_kind, work)?;
    charge(work, g.len())?;
    let ink = threshold(&g, threshold_pct);
    drop(g);
    let regions = trace(&ink, width, height, work)?;
    drop(ink);
    regions.iter().map(|r| outline(r, width, height, work)).collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test")]
    use super::*;

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
}
