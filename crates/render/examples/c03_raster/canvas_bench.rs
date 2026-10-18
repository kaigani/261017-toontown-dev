//! Host-raster fallback estimate for the C0.3 canvas spike (**spike code**): renders the canvas spike's three scenes
//! (the very same generator, included from the desktop spike module) with `vello_cpu` at the spike's starting camera,
//! so the per-frame CPU raster cost can be added to the measured pixel-frame IPC cost.

#[path = "../../../../desktop/src-tauri/src/spikes/canvas/renderlist.rs"]
#[allow(dead_code, reason = "shared spike module; only the generator is used here")]
mod renderlist;

use std::time::Instant;

use serde_json::{Value, json};
use vello_cpu::color::AlphaColor;
use vello_cpu::kurbo::{self, BezPath, Cap, Join};
use vello_cpu::peniko::Fill;
use vello_cpu::{Pixmap, RenderContext, Resources};

use crate::vello::Config;

/// The canvas spike's camera at t = 0 (desktop/src/spikes/canvas/camera.ts): world fits the smaller side, centre
/// offset by 900 world units along +x.
fn camera(w: f64, h: f64) -> kurbo::Affine {
    let world = f64::from(renderlist::WORLD);
    let zoom = w.min(h) / world;
    let (cx, cy) = (world / 2.0 + 900.0, world / 2.0);
    kurbo::Affine::new([zoom, 0.0, 0.0, zoom, w / 2.0 - cx * zoom, h / 2.0 - cy * zoom])
}

fn path(scene: &renderlist::Scene, rec: &renderlist::PathRec) -> BezPath {
    let mut b = BezPath::new();
    let mut pt = rec.point_start as usize * 2;
    let p = &scene.points;
    let mut next = |n: usize| {
        let at = pt;
        pt += 2 * n;
        at
    };
    for &verb in &scene.verbs[rec.verb_start as usize..(rec.verb_start + rec.verb_count) as usize] {
        match verb {
            renderlist::VERB_MOVE => {
                let i = next(1);
                b.move_to((f64::from(p[i]), f64::from(p[i + 1])));
            }
            renderlist::VERB_LINE => {
                let i = next(1);
                b.line_to((f64::from(p[i]), f64::from(p[i + 1])));
            }
            renderlist::VERB_CUBIC => {
                let i = next(3);
                b.curve_to(
                    (f64::from(p[i]), f64::from(p[i + 1])),
                    (f64::from(p[i + 2]), f64::from(p[i + 3])),
                    (f64::from(p[i + 4]), f64::from(p[i + 5])),
                );
            }
            _ => b.close_path(),
        }
    }
    b
}

fn render(scene: &renderlist::Scene, w: u16, h: u16, config: Config) -> f64 {
    let t0 = Instant::now();
    let mut ctx = RenderContext::new_with(w, h, config.settings());
    let cam = camera(f64::from(w), f64::from(h));
    for rec in &scene.paths {
        let g = scene.groups[rec.group as usize];
        let group = kurbo::Affine::new(g.map(f64::from));
        ctx.set_transform(cam * group);
        let [r, gg, b, a] = rec.paint;
        ctx.set_paint(AlphaColor::from_rgba8(r, gg, b, a));
        let bez = path(scene, rec);
        if rec.style == renderlist::STYLE_STROKE {
            ctx.set_stroke(kurbo::Stroke::new(f64::from(rec.width)).with_join(Join::Bevel).with_caps(Cap::Butt));
            ctx.stroke_path(&bez);
        } else {
            ctx.set_fill_rule(if rec.style == renderlist::STYLE_FILL_EVENODD { Fill::EvenOdd } else { Fill::NonZero });
            ctx.fill_path(&bez);
        }
    }
    ctx.flush();
    let mut pixmap = Pixmap::new(w, h);
    ctx.render(&mut pixmap, &mut Resources::new());
    t0.elapsed().as_secs_f64() * 1000.0
}

/// Renders every canvas scene at each size with single-threaded SIMD and 3-thread `vello_cpu`; median of `repeat`.
pub fn run(sizes: &[(u16, u16)], repeat: usize) -> Value {
    let mut rows = Vec::new();
    for kind in [renderlist::Kind::Paths100k, renderlist::Kind::Groups1k, renderlist::Kind::Brush10k] {
        let scene = renderlist::generate(kind, 1);
        for &(w, h) in sizes {
            for config in [Config::Detected, Config::Threads3] {
                let mut times: Vec<f64> = (0..repeat).map(|_| render(&scene, w, h, config)).collect();
                times.sort_by(f64::total_cmp);
                rows.push(json!({
                    "scene": format!("{kind:?}"),
                    "width": w,
                    "height": h,
                    "renderer": config.name(),
                    "msMedian": times[times.len() / 2],
                    "msAll": times,
                }));
            }
        }
    }
    json!({ "note": "vello_cpu render of the canvas spike scenes at the spike's starting camera (frame 0); \
        includes scene encoding into vello_cpu and rasterisation, excludes PNG encoding", "rows": rows })
}
