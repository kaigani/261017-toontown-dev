//! Independent entry and raster-boundary cancellation for empty and painted frames.
use tt_render::{El, Item, Paint, RenderError, Rule, Scene};
#[test]
fn cancellation_at_every_empty_and_nonempty_boundary() {
    let fill = Item::Fill {
        path: vec![El::MoveTo(0.0, 0.0), El::LineTo(8.0, 0.0), El::LineTo(8.0, 8.0), El::Close],
        rule: Rule::NonZero,
        paint: Paint::Solid([255, 0, 0, 255]),
    };
    for items in [vec![], vec![fill]] {
        let expected = if items.is_empty() { vec![0, 0] } else { vec![0, 1, 1] };
        for stop in 0..expected.len() {
            let scene = Scene { width: 16, height: 16, background: [0; 4], items: items.clone() };
            let mut seen = Vec::new();
            let r = tt_render::render(&scene, &mut |n| {
                seen.push(n);
                if seen.len() == stop + 1 { Err("stop".to_owned()) } else { Ok(()) }
            });
            assert!(matches!(r, Err(RenderError::Stopped(_))));
            assert_eq!(seen, expected[..=stop]);
        }
    }
}
