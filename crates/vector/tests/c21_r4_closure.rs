//! Scoped ruled-item closure variants.
#![allow(clippy::unwrap_used, missing_docs, reason = "review tests")]
use tt_vector::fit::{self, Sample};
use tt_vector::geom::P;
use tt_vector::text::{Content, default_para, default_run};

fn s(x: f64, y: f64, pressure: f64) -> Sample {
    Sample { p: P::new(x, y), pressure }
}

#[test]
fn closed_stationary_samples_keep_every_pressure() {
    let raw = [s(3.0, 4.0, 0.0), s(3.0, 4.0, 1.0), s(3.0, 4.0, 0.0)];
    assert_eq!(fit::clean(&raw, "/points").unwrap(), raw);
    for f in [fit::fit(&raw, 0.0, true), fit::polyline(&raw, true, 0.0)] {
        assert_eq!(f.path.anchors.len(), 1);
        assert_eq!(f.pressure.iter().map(|k| k[1]).fold(0.0, f64::max), 1.0);
        let pen = tt_vector::brush::BrushPen {
            tip: tt_model::entities::pen::Tip::Round,
            min_size: 0.0,
            max_size: 100.0,
            tip_angle: 0.0,
        };
        let outline = tt_vector::brush::outline(&f.path, &f.pressure, &pen).unwrap();
        let b = outline.bounds().unwrap();
        for (actual, expected) in [(b.min_x, -47.0), (b.max_x, 53.0), (b.min_y, -46.0), (b.max_y, 54.0)] {
            assert!((actual - expected).abs() < 0.15, "radius-max dab: {b:?}");
        }
    }
}

#[test]
fn several_closing_samples_form_ordered_seam_and_open_return_stays_open() {
    let raw = [
        s(0.0, 0.0, 1.0),
        s(100.0, 0.0, 1.0),
        s(100.0, 100.0, 1.0),
        s(0.0, 100.0, 1.0),
        s(0.0, 0.0, 0.2),
        s(0.0, 0.0, 0.8),
        s(0.0, 0.0, 0.1),
    ];
    for smooth in 0..=3 {
        for fitted in [false, true] {
            let run = |closed| {
                if fitted {
                    fit::fit(&raw, f64::from(smooth), closed)
                } else {
                    fit::polyline(&raw, closed, f64::from(smooth))
                }
            };
            let closed = run(true);
            let tail: Vec<_> = closed.pressure.iter().filter(|k| k[0] == 1.0).map(|k| k[1]).collect();
            let expected: Vec<_> = (4usize..7)
                .map(|i| {
                    let lo = i.saturating_sub(smooth as usize);
                    let hi = (i + smooth as usize).min(6);
                    raw[lo..=hi].iter().map(|s| s.pressure).sum::<f64>() / (hi - lo + 1) as f64
                })
                .collect();
            assert_eq!(tail.len(), expected.len(), "fit={fitted} smooth={smooth}: {:?}", closed.pressure);
            assert!(
                tail.iter().zip(expected).all(|(a, b)| (a - b).abs() < 1e-12),
                "fit={fitted} smooth={smooth}: {:?}",
                closed.pressure
            );
            assert!(!run(false).path.closed);
        }
    }
}

#[test]
fn distinct_return_groups_keep_their_arc_positions_and_tail_rotates_with_geometry() {
    // Spatial collapse compares to the last kept position. These returns are distinct groups, even though all are
    // near the first. Only the final group merges into the seam; the others keep their own arc positions.
    let raw = [
        s(0.0, 0.0, 1.0),
        s(100.0, 0.0, 1.0),
        s(100.0, 100.0, 1.0),
        s(0.0, 100.0, 1.0),
        s(0.01, 0.0, 0.2),
        s(-0.01, 0.0, 0.8),
        s(0.01, 0.0, 0.1),
    ];
    let f = fit::fit(&raw, 0.0, true);
    // A detected corner rotates the start. The final tail must immediately precede the original first pressure.
    let tail: Vec<_> = f.pressure.iter().filter(|k| [0.2, 0.8, 0.1].contains(&k[1])).collect();
    assert_eq!(tail.len(), 3, "{:?}", f.pressure);
    assert!(tail.windows(2).all(|w| w[0][0] < w[1][0]), "{:?}", f.pressure);
    let at = f.pressure.iter().position(|k| k[1] == 0.1).unwrap();
    assert_eq!(f.pressure[at + 1], [f.pressure[at][0], 1.0]);
    let mut grouped = raw;
    grouped[4].p = P::new(0.001, 0.0);
    grouped[5].p = P::new(0.002, 0.0);
    grouped[6].p = P::new(0.003, 0.0);
    let g = fit::fit(&grouped, 0.0, true);
    let tail: Vec<_> = g.pressure.iter().filter(|k| [0.2, 0.8, 0.1].contains(&k[1])).collect();
    assert_eq!(tail.len(), 3);
    assert!(tail.iter().all(|k| k[0] == tail[0][0]), "{:?}", g.pressure);
}

fn content(text: &str) -> Content {
    let runs = text
        .chars()
        .enumerate()
        .map(|(i, _)| {
            let mut r = default_run(None);
            r.start = i as u32;
            r.length = 1;
            r.size = 10.0 * (i + 1) as f64;
            r
        })
        .collect();
    Content::from_parts(text, runs, vec![default_para(), default_para()])
}
fn sizes(c: &Content) -> Vec<f64> {
    (0..c.chars.len()).map(|i| c.run_at(i).size).collect()
}

#[test]
fn reordered_marks_composites_and_paragraph_boundaries_keep_owners() {
    let c = content("\u{5e9}\u{5c1}\u{5b8}\u{5b4}");
    assert_eq!(c.text(), "\u{5e9}\u{5b4}\u{5b8}\u{5c1}");
    assert_eq!(sizes(&c), [10.0, 40.0, 30.0, 20.0]);
    let c = content("e\u{301}Z");
    assert_eq!(c.text(), "éZ");
    assert_eq!(sizes(&c), [10.0, 30.0]);
    let mut c = content("\u{5e9}\u{5c1}\n\u{5b8}\u{5b4}");
    assert_eq!(c.text(), "\u{5e9}\u{5c1}\n\u{5b4}\u{5b8}");
    assert_eq!(sizes(&c), [10.0, 20.0, 30.0, 50.0, 40.0]);
    assert_eq!(c.paras.len(), 2);
    c.delete(2, 1);
    assert_eq!(c.text(), "\u{5e9}\u{5b4}\u{5b8}\u{5c1}");
    assert_eq!(sizes(&c), [10.0, 50.0, 40.0, 20.0]);
    assert_eq!(c.paras.len(), 1);
}
