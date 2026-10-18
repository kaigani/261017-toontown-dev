//! Supplemental encoder probes and alternating before/after microbenchmark.
#![allow(dead_code)]
#[path = "../../../desktop/src-tauri/src/spikes/canvas/renderlist.rs"]
mod current;
#[path = "../../../target/review-r3/previous-renderlist.rs"]
mod previous;
use current::{encode, EncodeError, Kind, PathRec, Scene};
use std::hint::black_box;

fn main() {
    let valid = Scene {
        seed: 1, groups: vec![[1., 0., 0., 1., 0., 0.]],
        paths: vec![PathRec {verb_start:0, verb_count:2, point_start:0, group:0, paint:[255;4], style:2, width:1.}],
        verbs:vec![0,1], points:vec![0.,0.,1.,1.], bounds:[0.,0.,1.,1.],
    };
    let mut s=valid.clone(); s.points.push(0.);
    assert!(matches!(encode(Kind::Paths100k,&s,0,0),Err(EncodeError::UnpairedCoordinate{..})));
    let mut s=valid.clone(); s.paths[0].style=3;
    assert!(matches!(encode(Kind::Paths100k,&s,0,0),Err(EncodeError::UnknownStyle{..})));
    for verb in [1,2] {
        let mut s=valid.clone(); s.verbs[0]=verb;
        assert!(matches!(encode(Kind::Paths100k,&s,0,0),Err(EncodeError::MissingMove{..})));
    }
    let mut s=valid.clone(); s.verbs=vec![0,3,1]; s.paths[0].verb_count=3;
    assert!(matches!(encode(Kind::Paths100k,&s,0,0),Err(EncodeError::MissingMove{..})));
    let mut s=valid.clone(); s.verbs[1]=2;
    assert!(matches!(encode(Kind::Paths100k,&s,0,0),Err(EncodeError::BadPath{..})));
    let mut s=valid.clone(); s.paths[0].point_start=u32::MAX;
    assert!(matches!(encode(Kind::Paths100k,&s,0,0),Err(EncodeError::BadPath{..})));
    assert_eq!(encode(Kind::Paths100k,&Scene::default(),0,0).unwrap().len(),72);
    println!("typed-error/empty-scene probes: 8 passed; no panic");
    let old=previous::generate(previous::Kind::Brush10k,1);
    let new=current::generate(Kind::Brush10k,1);
    for i in 0..100 {
        let before=|| {let b=previous::encode(previous::Kind::Brush10k,black_box(&old),0,0).unwrap(); black_box(b)};
        let after=|| {let b=encode(Kind::Brush10k,black_box(&new),0,0).unwrap(); black_box(b)};
        let (a,b)=if i%2==0 {(before(),after())} else {let b=after(); (before(),b)};
        assert_eq!(a.len(),10_085_716); assert_eq!(a.len(),b.len());
        assert_eq!(&a[..56],&b[..56]); assert_eq!(&a[64..],&b[64..]);
        if i>=20 {
            println!("sample,{},{},{}",i,u64::from_le_bytes(a[56..64].try_into().unwrap()),u64::from_le_bytes(b[56..64].try_into().unwrap()));
        }
    }
}
