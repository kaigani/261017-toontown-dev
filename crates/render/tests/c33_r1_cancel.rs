//! Independent cancellation boundary probe for an empty final render.
use tt_render::{RenderError, Scene, render};

#[test]
fn empty_frame_observes_requested_cancellation() {
    let scene = Scene { width: 64, height: 64, background: [0; 4], items: Vec::new() };
    let result = render(&scene, &mut |_| Err("cancelled".to_owned()));
    assert!(matches!(result, Err(RenderError::Stopped(_))), "a render with no items bypassed cancellation");
}
