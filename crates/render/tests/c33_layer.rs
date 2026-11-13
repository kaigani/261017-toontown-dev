//! M3 C3.3 (builder): `Item::Layer` against independent pixel oracles. Geometry inside a layer is in the layer's space;
//! the layer's affine takes it to pixels, so fills, stroke widths and paint mappings transform exactly.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

use tt_render::{El, Item, Paint, Rule, Scene, render};

const RED: [u8; 4] = [255, 0, 0, 255];
const CLEAR: [u8; 4] = [0, 0, 0, 0];

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<El> {
    vec![El::MoveTo(x0, y0), El::LineTo(x1, y0), El::LineTo(x1, y1), El::LineTo(x0, y1), El::Close]
}

fn fill(path: Vec<El>, c: [u8; 4]) -> Item {
    Item::Fill { path, rule: Rule::NonZero, paint: Paint::Solid(c) }
}

fn draw(w: u32, h: u32, items: Vec<Item>) -> Vec<u8> {
    render(&Scene { width: w, height: h, background: CLEAR, items }, &mut |_| Ok(())).unwrap()
}

fn px(buf: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
}

/// Alpha of each pixel, summed: the covered area in pixels × 255.
fn coverage(buf: &[u8]) -> f64 {
    buf.as_chunks::<4>().0.iter().map(|p| f64::from(p[3])).sum::<f64>() / 255.0
}

#[test]
fn a_scaled_and_translated_layer_places_geometry_exactly() {
    // Unit square [0, 10]² scaled by 2 and moved by (5, 5): pixels [5, 25)² are covered, nothing else.
    let layer =
        Item::Layer { transform: [2.0, 0.0, 0.0, 2.0, 5.0, 5.0], items: vec![fill(rect(0.0, 0.0, 10.0, 10.0), RED)] };
    let buf = draw(32, 32, vec![layer]);
    for y in 0..32 {
        for x in 0..32 {
            let inside = (5..25).contains(&x) && (5..25).contains(&y);
            assert_eq!(px(&buf, 32, x, y), if inside { RED } else { CLEAR }, "({x}, {y})");
        }
    }
    // The same square pre-transformed gives the same bytes.
    assert_eq!(buf, draw(32, 32, vec![fill(rect(5.0, 5.0, 25.0, 25.0), RED)]));
}

#[test]
fn nested_layers_compose_outer_after_inner() {
    // Inner: translate (1, 0); outer: scale 4 — the square [0, 2]² lands at [4, 12] x [0, 8].
    let inner =
        Item::Layer { transform: [1.0, 0.0, 0.0, 1.0, 1.0, 0.0], items: vec![fill(rect(0.0, 0.0, 2.0, 2.0), RED)] };
    let outer = Item::Layer { transform: [4.0, 0.0, 0.0, 4.0, 0.0, 0.0], items: vec![inner] };
    assert_eq!(draw(16, 16, vec![outer]), draw(16, 16, vec![fill(rect(4.0, 0.0, 12.0, 8.0), RED)]));
}

#[test]
fn rotation_and_skew_map_area_by_the_determinant() {
    // A 10 x 10 square through a 30° rotation keeps its area; through a skew of shear 0.5 too (det 1); scaled by
    // (3, 0.5) it has area 150. Coverage sums to the area within the antialiasing error of the edges.
    let (c, s) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
    let cases = [
        ([c, s, -s, c, 32.0, 32.0], 100.0),
        ([1.0, 0.0, 0.5, 1.0, 20.0, 20.0], 100.0),
        ([3.0, 0.0, 0.0, 0.5, 10.0, 30.0], 150.0),
    ];
    for (t, area) in cases {
        let buf = draw(64, 64, vec![Item::Layer { transform: t, items: vec![fill(rect(0.0, 0.0, 10.0, 10.0), RED)] }]);
        let got = coverage(&buf);
        assert!((got - area).abs() < 0.5, "{t:?}: {got} vs {area}");
        // The centre of the transformed square is fully covered.
        let (cx, cy) = (t[0] * 5.0 + t[2] * 5.0 + t[4], t[1] * 5.0 + t[3] * 5.0 + t[5]);
        assert_eq!(px(&buf, 64, cx as u32, cy as u32), RED, "{t:?}");
    }
}

#[test]
fn stroke_widths_scale_with_the_layer_anisotropically() {
    // A vertical line of width 2 under a (4, 1) scale is 8 pixels wide; a horizontal one stays 2 pixels high.
    let line = |x0: f64, y0: f64, x1: f64, y1: f64| Item::Stroke {
        path: vec![El::MoveTo(x0, y0), El::LineTo(x1, y1)],
        width: 2.0,
        paint: Paint::Solid(RED),
    };
    let t = [4.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let vertical = draw(64, 64, vec![Item::Layer { transform: t, items: vec![line(8.0, 16.0, 8.0, 48.0)] }]);
    // Round caps add a half-disc at each end; the straight part covers 8 x 32 pixels.
    let row = 32;
    let wide: Vec<u32> = (0..64).filter(|x| px(&vertical, 64, *x, row)[3] == 255).collect();
    assert_eq!(wide, (28..36).collect::<Vec<_>>(), "8 pixels wide around x = 32");
    let horizontal = draw(64, 64, vec![Item::Layer { transform: t, items: vec![line(2.0, 32.0, 14.0, 32.0)] }]);
    let high: Vec<u32> = (0..64).filter(|y| px(&horizontal, 64, 32, *y)[3] == 255).collect();
    assert_eq!(high, vec![31, 32], "2 pixels high around y = 32");
}

#[test]
fn paint_mappings_are_in_the_layer_space() {
    // A linear gradient mapped in layer space equals the same gradient with the layer folded into its mapping.
    let stops = vec![(0.0, [0, 0, 255, 255]), (1.0, [255, 255, 0, 255])];
    let t = [2.0, 0.0, 0.0, 2.0, 4.0, 4.0];
    let layered = Item::Layer {
        transform: t,
        items: vec![Item::Fill {
            path: rect(0.0, 0.0, 12.0, 12.0),
            rule: Rule::NonZero,
            paint: Paint::Linear { stops: stops.clone(), unit_to_pixel: [12.0, 0.0, 0.0, 12.0, 0.0, 0.0] },
        }],
    };
    let flat = Item::Fill {
        path: rect(4.0, 4.0, 28.0, 28.0),
        rule: Rule::NonZero,
        paint: Paint::Linear { stops, unit_to_pixel: [24.0, 0.0, 0.0, 24.0, 4.0, 4.0] },
    };
    assert_eq!(draw(32, 32, vec![layered]), draw(32, 32, vec![flat]));
}

#[test]
fn scenes_without_layers_render_as_before_and_the_count_reaches_nested_items() {
    let items = vec![fill(rect(1.0, 1.0, 9.0, 9.0), RED)];
    let a = draw(10, 10, items.clone());
    let b = draw(10, 10, vec![Item::Layer { transform: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], items }]);
    assert_eq!(a, b, "an identity layer changes nothing");
    // Cancellation sees every item, nested ones included.
    let mut seen = Vec::new();
    let nested =
        Item::Layer { transform: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], items: vec![fill(rect(0.0, 0.0, 1.0, 1.0), RED); 3] };
    let scene = Scene { width: 4, height: 4, background: CLEAR, items: vec![nested] };
    render(&scene, &mut |i| {
        seen.push(i);
        Ok(())
    })
    .unwrap();
    // The entry check, the layer and its three items, then the checks before the flush and the rasterisation.
    assert_eq!(seen, vec![0, 1, 2, 3, 4, 4], "every item, nested ones included, and every boundary");
    let stopped = render(&scene, &mut |i| if i == 2 { Err("stop".into()) } else { Ok(()) });
    assert!(matches!(stopped, Err(tt_render::RenderError::Stopped(_))));
}
