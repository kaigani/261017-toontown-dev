//! `vello_cpu` adapter for the C0.3 fixture scenes (**spike code**).

use vello_cpu::color::{AlphaColor, Srgb};
use vello_cpu::kurbo::{self, BezPath, Cap, Join};
use vello_cpu::peniko::{Fill, Gradient};
use vello_cpu::{Level, Pixmap, RasterizerSettings, RenderContext, RenderMode, RenderSettings, Resources};

use crate::scene::{Affine, FillRule, Item, Paint, Path, Rgba, Scene, Seg};

/// Which `vello_cpu` configuration to use. Every configuration is a fixed, named combination with its own committed
/// hashes (C03-R1-04): the scalar level and the detected level are different arithmetic paths and may differ in
/// pixels; the report records the level each configuration actually used.
#[derive(Clone, Copy, Debug)]
pub enum Config {
    /// SIMD level detected at run time (AVX2/AVX-512 on x86-64, NEON on arm64), u8 pipeline.
    Detected,
    /// `Level::fallback()`: no platform SIMD intrinsics, u8 pipeline.
    Fallback,
    /// Detected SIMD level with 3 worker threads (`multithreading` feature), u8 pipeline.
    Threads3,
    /// `Level::fallback()` with the f32 pipeline (`RenderMode::OptimizeQuality`, C03-R1-05).
    QualityFallback,
    /// Detected SIMD level with the f32 pipeline.
    QualityDetected,
}

impl Config {
    pub fn name(self) -> &'static str {
        match self {
            Config::Detected => "vello_cpu_simd",
            Config::Fallback => "vello_cpu_fallback",
            Config::Threads3 => "vello_cpu_mt3",
            Config::QualityFallback => "vello_cpu_f32_fallback",
            Config::QualityDetected => "vello_cpu_f32_simd",
        }
    }

    pub fn render_mode(self) -> RenderMode {
        match self {
            Config::QualityFallback | Config::QualityDetected => RenderMode::OptimizeQuality,
            _ => RenderMode::OptimizeSpeed,
        }
    }

    pub fn settings(self) -> RenderSettings {
        match self {
            Config::Detected => RenderSettings { level: Level::new(), num_threads: 0 },
            Config::Fallback => RenderSettings { level: Level::fallback(), num_threads: 0 },
            Config::Threads3 => RenderSettings { level: Level::new(), num_threads: 3 },
            Config::QualityFallback => RenderSettings { level: Level::fallback(), num_threads: 0 },
            Config::QualityDetected => RenderSettings { level: Level::new(), num_threads: 0 },
        }
    }
}

fn colour(c: Rgba) -> AlphaColor<Srgb> {
    AlphaColor::from_rgba8(c[0], c[1], c[2], c[3])
}

fn affine(t: &Affine) -> kurbo::Affine {
    kurbo::Affine::new(*t)
}

fn bez(path: &Path) -> BezPath {
    let mut b = BezPath::new();
    for seg in &path.0 {
        match *seg {
            Seg::MoveTo(x, y) => b.move_to((x, y)),
            Seg::LineTo(x, y) => b.line_to((x, y)),
            Seg::CubicTo(x1, y1, x2, y2, x, y) => b.curve_to((x1, y1), (x2, y2), (x, y)),
            Seg::Close => b.close_path(),
        }
    }
    b
}

fn set_paint(ctx: &mut RenderContext, paint: &Paint) {
    match paint {
        Paint::Solid(c) => ctx.set_paint(colour(*c)),
        Paint::Linear { p0, p1, stops } => {
            let stops: Vec<(f32, AlphaColor<Srgb>)> = stops.iter().map(|(o, c)| (*o, colour(*c))).collect();
            ctx.set_paint(Gradient::new_linear(*p0, *p1).with_stops(stops.as_slice()));
        }
        Paint::Radial { c, r, stops } => {
            let stops: Vec<(f32, AlphaColor<Srgb>)> = stops.iter().map(|(o, c)| (*o, colour(*c))).collect();
            ctx.set_paint(Gradient::new_radial(*c, *r as f32).with_stops(stops.as_slice()));
        }
    }
}

fn draw(ctx: &mut RenderContext, item: &Item) {
    match item {
        Item::Fill { path, paint, rule, transform } => {
            ctx.set_transform(affine(transform));
            set_paint(ctx, paint);
            ctx.set_fill_rule(match rule {
                FillRule::NonZero => Fill::NonZero,
                FillRule::EvenOdd => Fill::EvenOdd,
            });
            ctx.fill_path(&bez(path));
        }
        Item::Stroke { path, paint, width, transform } => {
            ctx.set_transform(affine(transform));
            set_paint(ctx, paint);
            ctx.set_stroke(kurbo::Stroke::new(*width).with_join(Join::Bevel).with_caps(Cap::Butt));
            ctx.stroke_path(&bez(path));
        }
        Item::Clip { path, transform, children } => {
            ctx.set_transform(affine(transform));
            ctx.set_fill_rule(Fill::NonZero);
            ctx.push_clip_layer(&bez(path));
            for child in children {
                draw(ctx, child);
            }
            ctx.pop_layer();
        }
    }
}

/// Premultiplied RGBA8, row-major.
pub fn render(scene: &Scene, config: Config) -> Vec<u8> {
    let mut ctx = RenderContext::new_with(scene.width, scene.height, config.settings());
    for item in &scene.items {
        draw(&mut ctx, item);
    }
    ctx.flush();
    let mut pixmap = Pixmap::new(scene.width, scene.height);
    let settings = RasterizerSettings {
        render_mode: config.render_mode(),
        target_init: vello_cpu::TargetInit::Clear(colour(scene.background)),
        ..RasterizerSettings::default()
    };
    ctx.render_with(&mut pixmap, &mut Resources::new(), settings);
    pixmap.data_as_u8_slice().to_vec()
}
