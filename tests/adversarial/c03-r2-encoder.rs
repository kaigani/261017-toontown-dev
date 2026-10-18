//! Review the new encoder validation without changing product code.
#![allow(dead_code)]
#[path = "../../desktop/src-tauri/src/spikes/canvas/renderlist.rs"]
mod renderlist;
use renderlist::{EncodeError, Kind, PathRec, Scene, encode};

fn scene() -> Scene {
    Scene {
        seed: 1,
        groups: vec![[1.0, 0.0, 0.0, 1.0, 0.0, 0.0]],
        paths: vec![PathRec {
            verb_start: 0,
            verb_count: 2,
            point_start: 0,
            group: 0,
            paint: [255; 4],
            style: 2,
            width: 1.0,
        }],
        verbs: vec![0, 1],
        points: vec![0.0, 0.0, 1.0, 1.0],
        bounds: [0.0, 0.0, 1.0, 1.0],
    }
}

#[test]
fn nonfinite_points_and_bounds_return_typed_errors() {
    for v in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut s = scene();
        s.points[0] = v;
        assert!(matches!(encode(Kind::Paths100k, &s, 0, 0), Err(EncodeError::NonFinitePoint { .. })));
        let mut s = scene();
        s.bounds[0] = v;
        assert!(matches!(encode(Kind::Paths100k, &s, 0, 0), Err(EncodeError::BadBounds(_))));
    }
}

#[test]
fn adjacent_out_of_bounds_and_negative_width_are_refused() {
    for v in [-f32::from_bits(1), f32::from_bits(1.0_f32.to_bits() + 1)] {
        let mut s = scene();
        s.points[0] = v;
        assert!(matches!(encode(Kind::Paths100k, &s, 0, 0), Err(EncodeError::PointOutOfBounds { .. })));
    }
    let mut s = scene();
    s.paths[0].width = -1.0;
    assert!(matches!(encode(Kind::Paths100k, &s, 0, 0), Err(EncodeError::BadWidth { .. })));
}

#[test]
fn subnormal_and_empty_path_are_valid() {
    let mut s = scene();
    s.points[0] = f32::from_bits(1);
    assert!(encode(Kind::Paths100k, &s, 0, 0).is_ok());
    s.paths[0].verb_count = 0;
    assert!(encode(Kind::Paths100k, &s, 0, 0).is_ok());
}

#[test]
fn c03_r2_03_unpaired_coordinate_must_be_refused() {
    let mut s = scene();
    s.points.push(f32::NAN);
    assert!(
        encode(Kind::Paths100k, &s, 0, 0).is_err(),
        "odd point vector emits an uncertifiable packet, with an unchecked NaN tail"
    );
}

#[test]
fn c03_r2_03_unknown_style_must_be_refused() {
    let mut s = scene();
    s.paths[0].style = 3;
    assert!(encode(Kind::Paths100k, &s, 0, 0).is_err(), "decoder rejects style 3 but encoder reports success");
}
