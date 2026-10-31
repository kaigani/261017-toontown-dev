#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review regression tests")]
use tt_vector::geom::P;
use tt_vector::path::{Anchor, Path};
use tt_vector::planar::graph::{build, EdgeKind, StrokeIn};
use tt_vector::planar::node::{node, NodeStroke, Outcome};
use tt_vector::planar::fill::{carry, Fill};
use tt_vector::work::Work;
fn path(points: &[(f64,f64)], closed: bool) -> Path {
    Path { anchors: points.iter().map(|&(x,y)| Anchor::corner(P::new(x,y))).collect(), closed }
}
fn rect(id: &str, x0:f64,y0:f64,x1:f64,y1:f64) -> StrokeIn {
    StrokeIn {id:id.into(), kind:EdgeKind::Pencil, path:path(&[(x0,y0),(x1,y0),(x1,y1),(x0,y1)],true)}
}
#[test]
fn overlapping_zones_carry_fill_even_when_neither_interior_point_is_in_the_other() {
    let old=build(&[rect("old",0.,4.,10.,6.)], &mut Work::unlimited("review")).unwrap();
    let new=build(&[rect("new",4.,0.,6.,10.)], &mut Work::unlimited("review")).unwrap();
    assert_eq!(old.zones.len(),1); assert_eq!(new.zones.len(),1);
    let red=Some(Fill {colour_id:Some("col_red".into()),matrix:None});
    let fills=carry(&old,&[red.clone()],&new,&mut Work::unlimited("review")).unwrap();
    assert_eq!(fills,vec![red],"the exact intersection is a 2 x 2 square; old interior {:?}, new interior {:?}",old.zones[0].interior,new.zones[0].interior);
}
#[test]
fn a_crossing_near_an_existing_anchor_has_one_shared_junction() {
    let a=NodeStroke{id:"old".into(),kind:EdgeKind::Pencil,path:path(&[(0.,0.),(5.,0.),(10.,0.)],false),changed:false};
    let b=NodeStroke{id:"new".into(),kind:EdgeKind::Pencil,path:path(&[(5.01,-5.),(5.01,5.)],false),changed:true};
    let result=node(&[a,b],&mut Work::unlimited("review")).unwrap();
    let Outcome::Split(a)=&result.outcomes[&0] else {panic!("old line not split")};
    let Outcome::Split(b)=&result.outcomes[&1] else {panic!("new line not split")};
    assert_eq!(a[0].anchors.last().unwrap().p,b[0].anchors.last().unwrap().p,"junctions must share an exact coordinate");
}
