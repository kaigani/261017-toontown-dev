//! Independent coverage probe, C4.2b round 1.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review test")]
use tt_render::canvas::{self, CanvasItem, Record};
use tt_render::{El, Item, Paint, Scene};

#[test]
fn degenerate_and_limit_geometry_is_finite_and_repeatable() {
    let paths = [
        vec![El::MoveTo(0.0, 0.0), El::LineTo(0.0, 0.0)],
        vec![El::MoveTo(-10_000_000.0, -10_000_000.0), El::LineTo(10_000_000.0, 10_000_000.0)],
        std::iter::once(El::MoveTo(0.0, 0.0)).chain(std::iter::repeat_n(El::LineTo(0.0, 0.0), 99_999)).collect(),
    ];
    for path in paths {
        let input = vec![CanvasItem {
            item: Item::Stroke { path, width: 2.0, paint: Paint::Solid([255; 4]) },
            slot: 0,
            bitmap: None,
        }];
        let records = canvas::tessellate(&input, 0);
        assert_eq!(records, canvas::tessellate(&input, 0));
        for record in records {
            if let Record::Solid { vertices, indices } = record {
                assert!(vertices.iter().all(|v| v.x.is_finite() && v.y.is_finite() && v.slot == 0));
                assert!(indices.iter().all(|i| (*i as usize) < vertices.len()));
            }
        }
    }
}

#[test]
fn anisotropic_layer_preserves_final_stroke_coverage() {
    let item = Item::Layer {
        transform: [3.0, 0.0, 0.0, 0.5, 0.0, 40.0],
        items: vec![Item::Stroke {
            path: vec![El::MoveTo(10.0, 40.0), El::LineTo(30.0, 40.0)],
            width: 20.0,
            paint: Paint::Solid([255; 4]),
        }],
    };
    let pixels = tt_render::render(
        &Scene { width: 160, height: 160, background: [0; 4], items: vec![item.clone()] },
        &mut |_| Ok(()),
    )
    .unwrap();
    let records = canvas::tessellate(&[CanvasItem { item, slot: 0, bitmap: None }], 0);
    let mut different = 0;
    for y in 0..160usize {
        for x in 0..160usize {
            let p = (x as f32 + 0.5, y as f32 + 0.5);
            let covered = records.iter().any(|r| {
                let Record::Solid { vertices, indices } = r else { panic!("solid fixture") };
                indices.as_chunks::<3>().0.iter().any(|t| {
                    let cross = |a: usize, b: usize| {
                        let a = vertices[a];
                        let b = vertices[b];
                        (b.x - a.x) * (p.1 - a.y) - (b.y - a.y) * (p.0 - a.x)
                    };
                    let a = cross(t[0] as usize, t[1] as usize);
                    let b = cross(t[1] as usize, t[2] as usize);
                    let c = cross(t[2] as usize, t[0] as usize);
                    (a >= 0.0 && b >= 0.0 && c >= 0.0) || (a <= 0.0 && b <= 0.0 && c <= 0.0)
                })
            });
            different += usize::from(covered != (pixels[(y * 160 + x) * 4 + 3] >= 128));
        }
    }
    let fraction = different as f64 / 25600.0;
    eprintln!("anisotropic stroke differing sample fraction: {fraction}");
    assert!(fraction < 0.01, "gross stroke shape mismatch: {fraction}");
}
