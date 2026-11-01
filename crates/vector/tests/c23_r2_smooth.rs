//! Independent work-budget fixtures for the revised Smooth algorithm.
#![allow(clippy::unwrap_used, missing_docs, reason = "review tests")]
use tt_vector::{ops::smooth::smooth_within, path::Path, work::Work};
#[test]
fn twenty_thousand_anchor_work_observations() {
    for mode in 0..5 {
        let points: Vec<[f64; 6]> = (0..20_000)
            .map(|i| {
                let x = i as f64;
                let y = match mode {
                    0 => 0.0,
                    1 => (i % 2) as f64 * 0.1,
                    2 => ((i * 17) % 11) as f64 * 0.01,
                    3 => (i % 32) as f64 * 0.01,
                    _ => (i % 2) as f64,
                };
                [x, y, 0.0, 0.0, 0.0, 0.0]
            })
            .collect();
        let mut work = Work::new(150_000_000, "review Smooth");
        let start = std::time::Instant::now();
        let r = smooth_within(&Path::from_points(&points, false), 1.0, &|_| false, &mut work);
        println!(
            "mode {mode}: units {}, seconds {}, result {:?}",
            work.used(),
            start.elapsed().as_secs_f64(),
            r.as_ref().map(|(_, n)| *n)
        );
        assert!(r.is_ok() || r.as_ref().unwrap_err().code == "limit.exceeded");
    }
}
