//! Cancellation after rendering has begun, rather than before the first item.
#![allow(clippy::unwrap_used, missing_docs, reason = "review tests")]
use tt_render::{El, Item, Paint, Rule, Scene};
#[test]
fn render_stops_between_processed_items() {
    let item = Item::Fill {
        path: vec![El::MoveTo(0., 0.), El::LineTo(8., 0.), El::LineTo(8., 8.), El::Close],
        rule: Rule::NonZero,
        paint: Paint::Solid([255, 0, 0, 255]),
    };
    let scene = Scene { width: 16, height: 16, background: [0; 4], items: vec![item; 8] };
    let mut seen = vec![];
    let result = tt_render::render(&scene, &mut |i| {
        seen.push(i);
        if i == 3 { Err("review cancellation inside render".into()) } else { Ok(()) }
    });
    assert!(matches!(result, Err(tt_render::RenderError::Stopped(_))));
    assert_eq!(seen, vec![0, 1, 2, 3]);
}
