//! Independent quadratic touch family: y=3s(1-s), then linear 0 -> 1.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review assertions")]
use tt_anim::function::KeyView;
use tt_anim::tcb::{Path, Point, Velocity};
use tt_model::entities::channel::CurveType;
fn passage(arc: f64) -> f64 {
    let point = |x, frame| Point { pos: [x,0.0,0.0], t:0.0,c:0.0,b:0.0,frame:Some(frame),constant:false,lock_depth:false };
    let path=Path::new(vec![point(0.0,1.0),point(1.0,3.0)],false);
    let v=Velocity { curve:CurveType::Spline, keys:vec![
        KeyView { right:(1.0/3.0,1.0), ..KeyView::plain(1.0,0.0) },
        KeyView { left:(-1.0/3.0,1.0), ..KeyView::plain(2.0,0.0) },
    ]};
    v.passage(&path,0,arc,&mut |_| Ok::<_,()>(())).unwrap()
}
#[test]
fn candidate_value_tolerance_must_not_replace_frame_accuracy() {
    // Exact rational family: peak=3/4 at frame=3/2, later linear crossing=2+arc.
    // These are arc offsets relative to the exact peak; expected times are analytic.
    let mut failures=Vec::new();
    for relative in [0.0, -1e-9,-1e-12,-1e-13,-1e-15,1e-9,1e-12,1e-13,1e-15] {
        let arc=0.75*(1.0+relative);
        let exact=if relative>0.0 {2.0+arc} else {1.0+(1.0-(1.0-4.0*arc/3.0).max(0.0).sqrt())/2.0};
        let got=passage(arc);
        eprintln!("relative={relative:e} arc={arc:.17} got={got} exact={exact:.17}");
        if (got-exact).abs()>0.0006 { failures.push((relative,got,exact)); }
    }
    assert!(failures.is_empty(), "C34-R4-01: candidate value tolerance exceeds frame accuracy: {failures:?}");
}
