//! Independent transformed tile-off alpha and renderer cancellation variants.
#![allow(clippy::unwrap_used, missing_docs, reason = "review tests")]
use std::sync::Arc;
use tt_render::{El, Item, Paint, Rule, Scene};
fn scene(matrix: [f64; 6]) -> Scene {
    Scene {
        width: 40,
        height: 40,
        background: [0, 0, 0, 0],
        items: vec![Item::Fill {
            path: vec![
                El::MoveTo(0.0, 0.0),
                El::LineTo(40.0, 0.0),
                El::LineTo(40.0, 40.0),
                El::LineTo(0.0, 40.0),
                El::Close,
            ],
            rule: Rule::NonZero,
            paint: Paint::Image {
                rgba: Arc::new(vec![200, 100, 50, 128]),
                width: 1,
                height: 1,
                unit_to_pixel: matrix,
                repeat: false,
            },
        }],
    }
}
#[test]
fn rotated_skewed_and_reflected_tile_off_alpha_has_transparent_exterior() {
    for m in
        [[10.0, 10.0, -10.0, 10.0, 20.0, 5.0], [12.0, 0.0, 8.0, 12.0, 5.0, 5.0], [-12.0, 0.0, 5.0, 12.0, 25.0, 5.0]]
    {
        let p = tt_render::render(&scene(m), &mut |_| Ok(())).unwrap();
        let [a, b, c, d, e, f] = m;
        let det = a * d - b * c;
        let mut inside = 0;
        let mut outside = 0;
        for y in 0..40 {
            for x in 0..40 {
                let dx = x as f64 + 0.5 - e;
                let dy = y as f64 + 0.5 - f;
                let u = (d * dx - c * dy) / det;
                let v = (-b * dx + a * dy) / det;
                let q = &p[(y * 40 + x) * 4..(y * 40 + x) * 4 + 4];
                if (0.2..0.8).contains(&u) && (0.2..0.8).contains(&v) {
                    assert_eq!(q, [100, 50, 25, 128]);
                    inside += 1;
                }
                if u < -0.2 || u > 1.2 || v < -0.2 || v > 1.2 {
                    assert_eq!(q, [0, 0, 0, 0], "m {m:?} at {x},{y}");
                    outside += 1;
                }
            }
        }
        assert!(inside > 0 && outside > 100);
    }
}
#[test]
fn renderer_observes_cancellation_before_drawing() {
    let mut calls = 0;
    let e = tt_render::render(&scene([10.0, 0.0, 0.0, 10.0, 0.0, 0.0]), &mut |_| {
        calls += 1;
        Err("review cancellation".into())
    });
    assert!(matches!(e, Err(tt_render::RenderError::Stopped(_))));
    assert!(calls > 0);
}
