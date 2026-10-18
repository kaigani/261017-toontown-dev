//! Reviewer stress corpus. Includes the unmodified spike adapters; never product code.
#![allow(dead_code, missing_docs)]
#![allow(
    clippy::unwrap_used,
    clippy::print_stdout,
    clippy::approx_constant,
    reason = "review test assertions and fixed bit-pattern fixture constants"
)]
#[path = "../examples/c03_raster/scene.rs"]
mod scene;
#[path = "../examples/c03_raster/ttraster.rs"]
mod ttraster;
#[path = "../examples/c03_raster/vello.rs"]
mod vello;

use scene::{FillRule, Item, Paint, Path, Scene};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn stress(seed: u32) -> Scene {
    let f = f64::from(seed);
    let stops = (0..65)
        .map(|i| {
            (
                i as f32 / 64.0,
                [
                    ((i * 71 + seed * 13) % 256) as u8,
                    ((i * 117 + 19) % 256) as u8,
                    ((i * 41 + seed) % 256) as u8,
                    ((i * 67 + 31) % 256) as u8,
                ],
            )
        })
        .collect::<Vec<_>>();
    let mut items = vec![Item::Fill {
        path: Path::new().m(-30.0, -30.0).l(190.0, -30.0).l(190.0, 190.0).l(-30.0, 190.0).z(),
        paint: Paint::Radial { c: (64.0 + f / 997.0, 51.0 + f / 887.0), r: 51.414213562373095, stops: stops.clone() },
        rule: FillRule::NonZero,
        transform: [0.7071067811865476, 0.7071067811865476, -0.3535533905932738, 0.3535533905932738, 41.0, -13.0],
    }];
    for j in 0..24 {
        let g = f64::from(j);
        items.push(Item::Stroke {
            path: Path::new().m(-3.0 + f / 313.0, g * 5.13).c(
                170.0 - g * 3.17,
                -91.1 + f / 101.0,
                -131.17 + g,
                217.13,
                123.7,
                g * 5.13 + 1.4142135623730951,
            ),
            paint: Paint::Linear { p0: (0.1, -3.7), p1: (111.4142135623731, 97.73205080756888), stops: stops.clone() },
            width: 0.1 + g / 17.0,
            transform: [0.8660254037844386, 0.5, -0.5, 0.8660254037844386, f / 997.0, -11.0],
        });
    }
    Scene {
        name: "review_stress",
        width: 128,
        height: 128,
        background: [0, 0, 0, 0],
        items,
        covers: "65-stop transparent radial/linear gradients, irrational transform literals, thin stroked cubics",
    }
}

#[test]
fn extended_corpus_repeats_and_cross_os_hashes() {
    let mut results = BTreeMap::new();
    for seed in 0..16 {
        let s = stress(seed);
        let tt = ttraster::render(&s).rgba;
        assert_eq!(tt, ttraster::render(&s).rgba, "ttraster repeat seed {seed}");
        results.insert(format!("ttraster/{seed}"), hash(&tt));
        for config in [vello::Config::Fallback, vello::Config::Detected, vello::Config::Threads3] {
            let first = vello::render(&s, config);
            for _ in 0..3 {
                assert_eq!(first, vello::render(&s, config), "repeat seed {seed} {config:?}");
            }
            results.insert(format!("{}/{seed}", config.name()), hash(&first));
        }
    }
    let text = serde_json::to_string_pretty(&results).unwrap();
    println!("{text}");
    if let Ok(output) = std::env::var("C03_REVIEW_STRESS_OUTPUT") {
        std::fs::write(output, &text).unwrap();
    }
    let baseline = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reviews/evidence/M0-C0.3-r1/stress-windows-baseline.json");
    if baseline.exists() {
        let expected: BTreeMap<String, String> =
            serde_json::from_str(&std::fs::read_to_string(baseline).unwrap()).unwrap();
        assert_eq!(results, expected, "extended corpus differs across OS/profile");
    }
}

#[test]
fn c03_r1_04_simd_dispatch_must_not_change_final_pixels() {
    let s = stress(0);
    assert_eq!(
        hash(&vello::render(&s, vello::Config::Fallback)),
        hash(&vello::render(&s, vello::Config::Detected)),
        "same scene, scalar versus detected SIMD"
    );
}

#[test]
fn c03_r1_05_source_over_an_opaque_background_stays_opaque() {
    let scenes = scene::scenes();
    let s = scenes.iter().find(|s| s.name == "many_small_paths").unwrap();
    assert_eq!(s.background[3], 255);
    let rgba = vello::render(s, vello::Config::Fallback);
    let min_alpha = rgba.chunks_exact(4).map(|p| p[3]).min().unwrap();
    assert_eq!(min_alpha, 255, "opaque background cannot lose alpha under source-over");
}
