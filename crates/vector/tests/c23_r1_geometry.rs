//! Independent dense distance and glyph placement controls.
#![allow(clippy::unwrap_used, missing_docs, reason = "review tests")]
use tt_vector::{geom::P, path::Path, text};

#[test]
fn cutter_overlap_classification_keeps_exact_grid_crossings() {
    let n = 500_000_000i128;
    assert_eq!(n * (n - 2) - (n - 1) * (n - 1), -1);
    let (a, b, c) = (n as f64 / 64.0, (n - 1) as f64 / 64.0, (n - 2) as f64 / 64.0);
    let actual = tt_vector::ops::cut::polylines_cross(&[P::new(-a, -b), P::new(a, b)], &[P::new(-b, -c), P::new(b, c)]);
    println!("grid determinant -1; f64 determinant {}; crossing {actual}", a * c - b * b);
    assert!(actual, "the new Cutter/Optimize helpers must use the binding exact grid predicates too");
}
fn distance(q: P, p: &[P]) -> f64 {
    p.windows(2)
        .map(|s| {
            let dx = s[1].x - s[0].x;
            let dy = s[1].y - s[0].y;
            let t = (((q.x - s[0].x) * dx + (q.y - s[0].y) * dy) / (dx * dx + dy * dy).max(1e-30)).clamp(0.0, 1.0);
            let x = q.x - s[0].x - t * dx;
            let y = q.y - s[0].y - t * dy;
            (x * x + y * y).sqrt()
        })
        .fold(f64::INFINITY, f64::min)
}
#[test]
fn smooth_dense_bidirectional_distance_and_selected_anchors() {
    for offset in 0..4 {
        let points: Vec<[f64; 6]> =
            (0..24).map(|i| [i as f64 * 3.0, (((i + offset) * 17) % 11) as f64 * 0.1, 0.0, 0.0, 0.0, 0.0]).collect();
        let src = Path::from_points(&points, false);
        let (out, removed) = tt_vector::ops::smooth::smooth(&src, 0.8, &|i| i == 12);
        assert!(removed > 0);
        assert!(out.anchors.iter().any(|a| a.p == src.anchors[12].p));
        let (a, b) = (src.flatten(0.0005), out.flatten(0.0005));
        let there = a.iter().map(|p| distance(*p, &b)).fold(0.0, f64::max);
        let back = b.iter().map(|p| distance(*p, &a)).fold(0.0, f64::max);
        println!("offset {offset}: removed {removed}, original-to-result {there}, result-to-original {back}");
        assert!(there <= 0.802 && back <= 0.802);
    }
}
#[test]
fn rich_rtl_and_combining_glyphs_match_the_same_font_layout() {
    for value in ["AbO", "אבג", "A\u{301}x\u{327}"] {
        let mut c = text::Content::new(value, text::default_run(Some("col_first".into())), text::default_para());
        c.format(
            1,
            1,
            &text::Format { size: Some(22.0), colour_id: Some(Some("col_second".into())), ..Default::default() },
        );
        let (_, placed) = text::layout_placed(&c, None);
        let shapes = tt_vector::ops::glyphs::shapes(&c, None);
        assert_eq!(
            shapes.len(),
            c.chars.iter().filter(|c| !c.is_whitespace()).count(),
            "font outlines required on this review machine"
        );
        for shape in shapes {
            let g = placed.iter().find(|g| g.index == shape.index).unwrap();
            let r = c.run_at(g.index);
            let face = tt_vector::fonts::resolve(&r.font, r.bold, r.italic).face;
            let outlines = face.outline(g.ch).unwrap();
            let first = &outlines[0][0];
            let actual = shape.contours[0].anchors[0].p;
            assert_eq!(actual, P::new(g.x + first[0] * g.size, g.y + first[1] * g.size));
            assert_eq!(shape.colour_id, r.colour_id);
        }
    }
}
