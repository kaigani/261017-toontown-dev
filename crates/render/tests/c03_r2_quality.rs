//! Check alpha on every scene, and verify that the promoted stress fixtures preserve the review corpus.
#![allow(dead_code, missing_docs, clippy::unwrap_used, reason = "review harness around the spike example modules")]
#[path = "../examples/c03_raster/scene.rs"]
mod scene;
#[path = "../examples/c03_raster/ttraster.rs"]
mod ttraster;
#[path = "../examples/c03_raster/vello.rs"]
mod vello;

#[test]
fn quality_pipeline_opacity_full_corpus() {
    let mut checked = 0;
    let mut opaque = 0;
    for s in scene::scenes() {
        checked += 1;
        for rgba in [
            ttraster::render(&s).rgba,
            vello::render(&s, vello::Config::QualityFallback),
            vello::render(&s, vello::Config::QualityDetected),
        ] {
            assert_eq!(rgba.len(), usize::from(s.width) * usize::from(s.height) * 4);
            if s.background[3] == 255 {
                assert!(rgba.as_chunks::<4>().0.iter().all(|p| p[3] == 255), "{} lost opacity", s.name);
            } else {
                assert!(rgba.as_chunks::<4>().0.iter().any(|p| p[3] < 255), "{} lost transparency", s.name);
            }
        }
        opaque += usize::from(s.background[3] == 255);
    }
    assert_eq!((checked, opaque), (39, 23));
}
