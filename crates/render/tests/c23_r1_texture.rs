//! Independent texture extent and cancellation controls.
#![allow(clippy::unwrap_used, missing_docs, reason = "review tests")]
use std::sync::Arc;
use tt_render::{El, Item, Paint, Rule, Scene};

#[test]
fn tile_off_is_transparent_outside_the_mapped_unit_square() {
    let path =
        vec![El::MoveTo(0.0, 0.0), El::LineTo(30.0, 0.0), El::LineTo(30.0, 30.0), El::LineTo(0.0, 30.0), El::Close];
    let s = Scene {
        width: 30,
        height: 30,
        background: [0, 0, 0, 0],
        items: vec![Item::Fill {
            path,
            rule: Rule::NonZero,
            paint: Paint::Image {
                rgba: Arc::new(vec![255, 0, 0, 255]),
                width: 1,
                height: 1,
                unit_to_pixel: [10.0, 0.0, 0.0, 10.0, 10.0, 10.0],
                repeat: false,
            },
        }],
    };
    let p = tt_render::render(&s, &mut |_| Ok(())).unwrap();
    println!(
        "inside {:?}, outside {:?}",
        &p[(15 * 30 + 15) * 4..(15 * 30 + 15) * 4 + 4],
        &p[(5 * 30 + 5) * 4..(5 * 30 + 5) * 4 + 4]
    );
    assert_eq!(&p[(15 * 30 + 15) * 4..(15 * 30 + 15) * 4 + 4], &[255, 0, 0, 255]);
    assert_eq!(
        &p[(5 * 30 + 5) * 4..(5 * 30 + 5) * 4 + 4],
        &[0, 0, 0, 0],
        "03 section 7.4 requires transparency outside a non-tiled image"
    );
}
