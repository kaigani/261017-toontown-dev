//! C0.3 rasteriser determinism (C03-R1-04/05): the fixed scalar configurations must reproduce the committed hashes
//! on every OS. CI runs this test on Windows and macOS (check jobs) and on Linux (raster job). The detected-SIMD
//! configurations are checked separately by `c03_raster --check`; they are allowed to differ from the scalar ones
//! (a C0.4 policy question), never from their own committed hashes.
#![allow(dead_code, missing_docs, clippy::unwrap_used, reason = "test harness around the spike example modules")]
#[path = "../examples/c03_raster/scene.rs"]
mod scene;
#[path = "../examples/c03_raster/ttraster.rs"]
mod ttraster;
#[path = "../examples/c03_raster/vello.rs"]
mod vello;

use sha2::{Digest, Sha256};

/// The example's pixel hash: SHA-256 over `TTPIX1`, big-endian width and height, premultiplied RGBA8.
fn pixel_hash(w: u16, h: u16, rgba: &[u8]) -> String {
    let mut d = Sha256::new();
    d.update(b"TTPIX1");
    d.update(u32::from(w).to_be_bytes());
    d.update(u32::from(h).to_be_bytes());
    d.update(rgba);
    d.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn committed() -> serde_json::Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/c03_raster/hashes.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn fixed_scalar_configuration_hashes_identically_across_oses() {
    let hashes = committed();
    let mut checked = 0;
    for s in scene::scenes() {
        for (name, rgba) in [
            ("ttraster", ttraster::render(&s).rgba),
            ("vello_cpu_fallback", vello::render(&s, vello::Config::Fallback)),
            ("vello_cpu_f32_fallback", vello::render(&s, vello::Config::QualityFallback)),
        ] {
            let want = hashes["renderers"][name][s.name]["pixels"].as_str().unwrap_or("missing");
            assert_eq!(pixel_hash(s.width, s.height, &rgba), want, "{name}/{} on {}", s.name, std::env::consts::OS);
            checked += 1;
        }
    }
    assert_eq!(checked, 3 * 39);
}

#[test]
fn u8_pipeline_loses_opacity_over_an_opaque_background_known_limitation() {
    // Documented limitation of vello_cpu 0.3.0's u8 pipeline (C03-R1-05): source-over on an opaque background must
    // stay opaque, but alpha drops to 250 in this scene. The f32 pipeline keeps 255 (c03_r1_stress.rs). If an
    // upgrade fixes this, this test fails and the limitation note in the C0.3 handoff must be revisited.
    let scenes = scene::scenes();
    let s = scenes.iter().find(|s| s.name == "many_small_paths").unwrap();
    assert_eq!(s.background[3], 255);
    let min = |c| vello::render(s, c).as_chunks::<4>().0.iter().map(|p| p[3]).min().unwrap();
    assert_eq!(min(vello::Config::Fallback), 250);
    assert_eq!(min(vello::Config::QualityFallback), 255);
}
