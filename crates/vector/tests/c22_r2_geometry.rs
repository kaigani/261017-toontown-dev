#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "independent QA tests")]
use tt_vector::geom::P;
use tt_vector::path::{Anchor, Path};
use tt_vector::planar::fill::{Fill, carry};
use tt_vector::planar::graph::{EdgeKind, LayerMap, StrokeIn, build};
use tt_vector::planar::node::{NodeStroke, Outcome, node};
use tt_vector::work::Work;

fn path(p: &[(f64, f64)], closed: bool) -> Path {
    Path { anchors: p.iter().map(|&(x,y)| Anchor::corner(P::new(x,y))).collect(), closed }
}
fn line(id: &str, p: &[(f64,f64)], closed: bool) -> StrokeIn {
    StrokeIn { id: id.into(), kind: EdgeKind::Pencil, path: path(p, closed) }
}
fn rect(id: &str, x: f64, y: f64, w: f64, h: f64) -> StrokeIn {
    line(id, &[(x,y),(x+w,y),(x+w,y+h),(x,y+h)], true)
}
fn map(s: &[StrokeIn]) -> LayerMap { build(s, &mut Work::unlimited("r2")).unwrap() }
fn ns(id: &str, p: &[(f64,f64)], changed: bool) -> NodeStroke {
    NodeStroke { id: id.into(), kind: EdgeKind::Pencil, path: path(p,false), changed }
}
fn paint(id: &str) -> Option<Fill> { Some(Fill { colour_id: Some(id.into()), matrix: None }) }

#[test]
fn shared_anchor_rotated_reversed_and_at_tolerance() {
    for d in [1./64., 1./32., f64::from_bits((1f64/32.).to_bits()+1)] {
        let a = ns("old", &[(0.,10.),(0.,5.),(0.,0.)], false);
        let b = ns("new", &[(-5.,5.+d),(5.,5.+d)], true);
        let n = node(&[b,a], &mut Work::unlimited("r2")).unwrap();
        let Outcome::Split(a) = &n.outcomes[&1] else { panic!("old: {n:?}") };
        let Outcome::Split(b) = &n.outcomes[&0] else { panic!("new: {n:?}") };
        assert_eq!(a[0].anchors.last().unwrap().p, b[0].anchors.last().unwrap().p);
        if d <= 1./32. { assert_eq!(a[0].anchors.last().unwrap().p, P::new(0.,5.)); }
    }
}

#[test]
fn vertex_crossing_uses_exact_orientation_for_grid_coordinates() {
    // Integer determinant N*(N-2)-(N-1)^2 = -1. Each coordinate is exactly on the 1/64 grid.
    let n = 500_000_000i128;
    assert_eq!(n*(n-2)-(n-1)*(n-1), -1);
    let (a,b,c) = (n as f64/64., (n-1) as f64/64., (n-2) as f64/64.);
    let r = node(&[ns("a", &[(-a,-b),(0.,0.),(a,b)], false),
                   ns("b", &[(-b,-c),(0.,0.),(b,c)], true)], &mut Work::unlimited("r2")).unwrap();
    eprintln!("exact grid determinant=-1; floating determinant={}; result={r:?}", a*c-b*b);
    for i in 0..2 {
        assert!(matches!(r.outcomes.get(&i), Some(Outcome::Split(p)) if p.len()==2),
                "strictly crossing grid directions must split both at zero: {r:?}");
    }
}

#[test]
fn vertex_touches_turnbacks_overlaps_and_degenerate_anchors() {
    for pts in [vec![(-4.,2.),(0.,0.),(4.,2.)], vec![(-4.,2.),(0.,0.),(-4.,2.)],
                vec![(-10.,0.),(0.,0.),(10.,0.)]] {
        let n = node(&[ns("a", &[(-10.,0.),(0.,0.),(10.,0.)],false), ns("b",&pts,true)],
                     &mut Work::unlimited("r2")).unwrap();
        assert!(n.outcomes.is_empty(), "touch/overlap is not a crossing: {n:?}");
    }
    for x in [0.,f64::from_bits(1)] {
        let n = node(&[ns("a", &[(-10.,0.),(0.,0.),(0.,0.),(10.,0.)],false),
                       ns("b", &[(x,-5.),(x,5.)],true)], &mut Work::unlimited("r2")).unwrap();
        assert!(matches!(n.outcomes.get(&0),Some(Outcome::Split(p)) if p.len()==2), "{n:?}");
        assert!(matches!(n.outcomes.get(&1),Some(Outcome::Split(p)) if p.len()==2), "{n:?}");
    }
}

#[test]
fn overlap_equal_merge_three_way_split_sliver_and_holes() {
    let old = map(&[rect("outer",0.,0.,12.,12.),line("cut",&[(6.,0.),(6.,12.)],false)]);
    let fills: Vec<_> = old.zones.iter().map(|z| paint(&z.id)).collect();
    let new = map(&[rect("outer2",0.,0.,12.,12.)]);
    let got = carry(&old,&fills,&new,&mut Work::unlimited("r2")).unwrap();
    assert_eq!(got, vec![paint(old.zones.iter().map(|z| z.id.as_str()).min().unwrap())]);
    let full = map(&[rect("full",0.,0.,12.,12.)]);
    let split = map(&[rect("full",0.,0.,12.,12.),line("c1",&[(4.,0.),(4.,12.)],false),line("c2",&[(8.,0.),(8.,12.)],false)]);
    assert_eq!(carry(&full,&[paint("red")],&split,&mut Work::unlimited("r2")).unwrap(),vec![paint("red");3]);
    let sliver = map(&[rect("sliver", 11.984375,0.,1./64.,12.)]);
    assert_eq!(carry(&full,&[paint("red")],&sliver,&mut Work::unlimited("r2")).unwrap(),vec![paint("red")]);
    let hole = map(&[rect("outer",0.,0.,20.,20.),rect("hole",5.,5.,10.,10.)]);
    let fills: Vec<_> = hole.zones.iter().map(|z| paint(if z.area==300. {"ring"} else {"inner"})).collect();
    let in_hole = map(&[rect("tiny",6.,6.,1.,1.)]);
    assert_eq!(carry(&hole,&fills,&in_hole,&mut Work::unlimited("r2")).unwrap(),vec![paint("inner")]);
}

#[test]
fn zone_identity_hole_spur_straight_elision_and_nonboundary_edits() {
    let mut strokes=vec![rect("outer",0.,0.,20.,20.),rect("hole",5.,5.,10.,10.)];
    let before=map(&strokes);
    let ring=before.zones.iter().find(|z| z.area==300.).unwrap().id.clone();
    strokes.push(line("spur", &[(-5.,10.),(0.,10.)],false));
    strokes.push(line("inside_hole", &[(7.,7.),(8.,8.)],false));
    assert_eq!(map(&strokes).zones.iter().find(|z| z.area==300.).unwrap().id,ring);
    strokes[3]=line("inside_hole", &[(8.,7.),(7.,8.)],false);
    assert_eq!(map(&strokes).zones.iter().find(|z| z.area==300.).unwrap().id,ring);
    strokes[0]=line("outer", &[(0.,0.),(20.,0.),(20.,20.),(0.,20.),(0.,10.)],true);
    assert_eq!(map(&strokes).zones.iter().find(|z| z.area==300.).unwrap().id,ring);
    strokes[0].path.anchors[4].p.x=1./64.;
    assert!(!map(&strokes).zones.iter().any(|z| z.id==ring), "nonstraight vertex must be hashed");
}

#[test]
fn along_edge_vertex_reentry_and_brush() {
    let m=map(&[rect("a",0.,0.,10.,10.),rect("b",10.,10.,10.,10.)]);
    let hit=m.along(&[P::new(-5.,-5.),P::new(25.,25.),P::new(-5.,-5.)],0.,&mut Work::unlimited("r2")).unwrap();
    assert_eq!(hit.zones.len(),2,"each reentered zone once: {hit:?}");
    let edge=m.along(&[P::new(-5.,0.),P::new(15.,0.)],0.,&mut Work::unlimited("r2")).unwrap();
    assert!(edge.near.contains_key(&0));
    let mut b=rect("brush",0.,0.,10.,10.); b.kind=EdgeKind::Brush;
    let m=map(&[b]);
    assert_eq!(m.along(&[P::new(-5.,5.),P::new(15.,5.)],0.,&mut Work::unlimited("r2")).unwrap().brushes,vec![0]);
}
