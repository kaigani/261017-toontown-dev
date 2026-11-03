// Round 3 independent seed variant; source assertions retained.
//! Independent reviewer decoder corpus and seeded mutations; no product changes.
#![allow(clippy::unwrap_used, clippy::print_stdout, clippy::panic, missing_docs, reason = "review test")]
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use std::alloc::System;
use tt_io::image::{LIMITS, decode};
#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

#[test]
fn independent_corpus_and_seeded_mutations_stay_typed_and_bounded() {
    let root = std::env::var_os("TT_R3_CORPUS").map(std::path::PathBuf::from).unwrap_or_else(|| {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vectorize/review-r1")
    });
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("manifest.json")).unwrap()).unwrap();
    let files: Vec<_> =
        manifest.as_object().unwrap().keys().map(|n| (n.clone(), std::fs::read(root.join(n)).unwrap())).collect();
    let mut seed = 0xc24a_1103_a51c_d39fu64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut accepted = 0;
    let mut max_alloc = 0;
    for i in 0..30_000usize {
        let (name, base) = &files[i % files.len()];
        let mut b = base.clone();
        if i >= files.len() && !b.is_empty() {
            let at = next() as usize % b.len();
            if i.is_multiple_of(3) {
                b.truncate(at);
            } else {
                b[at] ^= next() as u8;
            }
        }
        let region = Region::new(GLOBAL);
        let result = decode(&b, name.rsplit('.').next(), &LIMITS);
        let allocated = region.change().bytes_allocated;
        max_alloc = max_alloc.max(allocated);
        assert!(allocated <= 1 << 30, "case {i} {name}: allocation {allocated}");
        match result {
            Ok(image) => {
                accepted += 1;
                assert!(image.width > 0 && image.height > 0);
                assert_eq!(image.pixels.len(), image.width as usize * image.height as usize * 4);
            }
            Err(e) => assert!(matches!(e.code(), "limit.exceeded" | "vectorize.unsupportedFile"), "{e:?}"),
        }
    }
    println!("30000 cases; accepted={accepted}; max cumulative allocation={max_alloc}");
}
