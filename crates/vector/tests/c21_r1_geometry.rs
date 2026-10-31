//! Independent C2.1 review regressions, 02 sections 3.3 and 3.4.
#![allow(clippy::unwrap_used, missing_docs, reason = "review tests")]

use tt_model::entities::pen::Tip;
use tt_vector::brush::{BrushPen, outline};
use tt_vector::fit::{Sample, clean, fit};
use tt_vector::geom::P;
use tt_vector::path::{Anchor, Path};

fn distance(p: P, a: P, b: P) -> f64 {
    let d = b - a;
    let t = if d.dot(d) == 0.0 { 0.0 } else { ((p-a).dot(d)/d.dot(d)).clamp(0.0,1.0) };
    p.dist(a+d*t)
}

#[test]
fn fitted_curve_stays_within_tolerance_of_input_polyline() {
    let raw: Vec<Sample> = [(0.0,0.0),(100.0,50.0),(200.0,0.0)].into_iter()
        .map(|(x,y)| Sample { p:P::new(x,y), pressure:1.0 }).collect();
    let f = fit(&clean(&raw,"/points").unwrap(),0.0,false);
    let mut worst: f64 = 0.0;
    for s in f.path.segments() {
        for k in 0..=10000 {
            let p = s.eval(f64::from(k)/10000.0);
            let d = raw.windows(2).map(|w| distance(p,w[0].p,w[1].p)).fold(f64::INFINITY,f64::min);
            worst = worst.max(d);
        }
    }
    eprintln!("maximum fitted-curve to input-polyline distance: {worst} du");
    assert!(worst <= 0.25 + 1e-6, "02 3.3 tolerance 0.25 du; got {worst}");
}

fn contains(poly: &[P], p: P) -> bool {
    let mut winding = 0;
    for i in 0..poly.len() {
        let a = poly[i]; let b = poly[(i+1)%poly.len()];
        if a.y <= p.y && b.y > p.y && (b-a).cross(p-a)>0.0 { winding += 1; }
        if a.y > p.y && b.y <= p.y && (b-a).cross(p-a)<0.0 { winding -= 1; }
    }
    winding != 0
}

#[test]
fn brush_sweep_includes_pressure_peak_between_placements() {
    let spine = Path { anchors: vec![Anchor::corner(P::new(0.0,0.0)),Anchor::corner(P::new(100.0,0.0))], closed:false };
    let pen = BrushPen { tip:Tip::Round,min_size:0.0,max_size:100.0,tip_angle:0.0 };
    let out = outline(&spine,&[[0.0,0.0],[0.1,1.0],[0.2,0.0],[1.0,0.0]],&pen).unwrap();
    let poly = out.flatten(0.001);
    // At t=.1 the exact swept tip is a radius-50 disk centred on (10,0).
    // This probe is one whole du inside that disk, far beyond grid/fitting error.
    let probe = P::new(10.0,49.0);
    eprintln!("pressure peak outline bounds: {:?}",out.bounds());
    assert!(contains(&poly,probe), "02 3.4: swept tip at pressure knot must be included");
}

#[test]
fn invalid_sample_values_are_rejected_before_fitting() {
    for x in [f64::NAN,f64::INFINITY,f64::NEG_INFINITY,1e100,10_000_001.0] {
        assert!(clean(&[Sample {p:P::new(x,0.0),pressure:1.0}],"/points").is_err());
    }
}

#[test]
#[ignore = "subprocess worker; the parent runs this with a bounded timeout"]
fn dense_input_worker() {
    let samples: Vec<Sample> = (0..100_000).map(|i| Sample {
        p:P::new(f64::from(i)*1e-9,0.0),pressure:1.0
    }).collect();
    let start = std::time::Instant::now();
    let result = fit(&clean(&samples,"/points").unwrap(),3.0,true);
    eprintln!("100000 dense straight samples: {:?}, {} anchors",start.elapsed(),result.path.anchors.len());
    assert!(result.path.anchors.len()<=100_000);
}

#[test]
fn dense_100k_input_finishes_within_document_watchdog_budget() {
    use std::time::{Duration,Instant};
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact","dense_input_worker","--ignored","--nocapture"]).spawn().unwrap();
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() { assert!(status.success()); break; }
        if start.elapsed()>Duration::from_secs(30) {
            child.kill().unwrap(); child.wait().unwrap();
            assert!(start.elapsed()<=Duration::from_secs(30),"100000 distinct points on a .0001 du closed path exceeded 30 s; corners reach scans quadratically");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn looping_brush_preserves_hole_and_clean_swept_area() {
    let corners = [P::new(0.0,0.0),P::new(100.0,0.0),P::new(100.0,100.0),P::new(0.0,100.0)];
    let spine = Path { anchors:corners.iter().copied().map(Anchor::corner).collect(),closed:true };
    let pen = BrushPen {tip:Tip::Round,min_size:4.0,max_size:4.0,tip_angle:0.0};
    let out = outline(&spine,&[[0.0,1.0],[1.0,1.0]],&pen).unwrap();
    let poly = out.flatten(0.001);
    for x in -4..=104 { for y in -4..=104 {
        let p = P::new(f64::from(x),f64::from(y));
        let d = (0..4).map(|i|distance(p,corners[i],corners[(i+1)%4])).fold(f64::INFINITY,f64::min);
        if (d-2.0).abs()>0.3 { assert_eq!(contains(&poly,p),d<2.0,"swept-area mismatch at {p:?}, distance {d}"); }
    }}
}

#[test]
fn text_edge_cases_are_finite_and_bounded() {
    use tt_vector::text::{Content,default_run,default_para,layout};
    for s in [String::new(),"AV To\nمرحبا e\u{301}\nאבג".into(),"W".repeat(100_000)] {
        let c = Content::new(&s,default_run(None),default_para());
        for width in [None,Some(1.0),Some(100.0)] {
            let l = layout(&c,width);
            assert!(l.width.is_finite() && l.height.is_finite());
            assert!(l.lines<=100_003);
        }
    }
}
