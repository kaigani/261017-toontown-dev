//! # tt-render — renderer
//!
//! The deterministic software rasteriser for offscreen renders (11 §3 design decision; ADR-0006): `vello_cpu`, f32
//! pipeline (`RenderMode::OptimizeQuality`), scalar arithmetic (`Level::fallback()`), single-threaded, behind this
//! crate's own scene type. M2 C2.3 adds what drawing renders need (02 §13.5 `draw.drawing.render`): filled paths
//! (non-zero or even-odd) and stroked paths, painted with solid colours, linear and radial gradients and bitmap
//! textures through a unit-space mapping matrix, and PNG encoding of the result. The render-list compiler for scenes
//! (06 §6) arrives in M3 C3.3 and uses the same adapter.
//!
//! Conventions:
//! - Geometry is in **pixel space** (x right, y down); callers map drawing units with their own `drawingToPixel`.
//! - Paint mappings map the colour's **unit space** to pixel space (03 §7.2): a linear gradient runs along unit x from
//!   0 to 1; a radial gradient is centred on (½, ½) with radius ½; a texture's image covers the unit square with its
//!   first row at unit y = 0 (image pixel (px, py) is at unit (px / width, py / height)), repeated when tiled and
//!   padded otherwise.
//! - Colours are straight-alpha sRGB RGBA8; the rendered buffer is premultiplied RGBA8, and [`encode_png`] converts
//!   it to straight alpha with `round(c × 255 / a)`.

use std::sync::Arc;

use vello_cpu::color::{AlphaColor, Srgb};
use vello_cpu::kurbo::{self, BezPath, Cap, Join};
use vello_cpu::peniko::{self, Extend, Fill as PenikoFill, Gradient, ImageQuality, ImageSampler};
use vello_cpu::{Level, Pixmap, RasterizerSettings, RenderContext, RenderMode, RenderSettings, Resources};

/// Crate name as linked into the engine (the headless self-test lists it to prove the crate is present).
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Spec sections this crate is responsible for.
pub const SPEC_SECTIONS: &[&str] = &[
    "02 §13.5 (draw.drawing.render)",
    "06 §6 (render pipeline)",
    "09 §1",
    "11 §3 (software rasteriser)",
    "11 §7.5 (renders, describe_frame, hit_test)",
];

/// Milestone(s) in which this crate receives its implementation (plan §4).
pub const MILESTONE: &str = "M2 (C2.3: drawing renders), M3 (C3.3: render list)";

/// The largest width or height of a render (02 §13.5).
pub const MAX_SIDE: u32 = 8192;

/// A path element in pixel space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum El {
    /// Start a subpath.
    MoveTo(f64, f64),
    /// Straight segment.
    LineTo(f64, f64),
    /// Cubic segment: two controls and the end.
    CubicTo(f64, f64, f64, f64, f64, f64),
    /// Close the subpath.
    Close,
}

/// Straight-alpha sRGB RGBA8.
pub type Rgba = [u8; 4];

/// How a fill or stroke is painted.
#[derive(Debug, Clone, PartialEq)]
pub enum Paint {
    /// One colour.
    Solid(Rgba),
    /// A linear gradient along unit x; `unit_to_pixel` maps unit space to pixel space.
    Linear {
        /// Stops `(position, colour)`, sorted.
        stops: Vec<(f32, Rgba)>,
        /// Unit space → pixel space.
        unit_to_pixel: [f64; 6],
    },
    /// A radial gradient centred on (½, ½) with radius ½ in unit space.
    Radial {
        /// Stops `(position, colour)`, sorted.
        stops: Vec<(f32, Rgba)>,
        /// Unit space → pixel space.
        unit_to_pixel: [f64; 6],
    },
    /// A bitmap texture covering the unit square.
    Image {
        /// Straight-alpha RGBA8 rows, top first.
        rgba: Arc<Vec<u8>>,
        /// Width in pixels.
        width: u16,
        /// Height in pixels.
        height: u16,
        /// Unit space → pixel space.
        unit_to_pixel: [f64; 6],
        /// Repeat (tile on) or pad (tile off).
        repeat: bool,
    },
}

/// Fill rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// Non-zero winding.
    NonZero,
    /// Even-odd.
    EvenOdd,
}

/// One drawing operation, back to front.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// Fill a path.
    Fill {
        /// The path.
        path: Vec<El>,
        /// Fill rule.
        rule: Rule,
        /// Paint.
        paint: Paint,
    },
    /// Stroke a path with round joins and caps.
    Stroke {
        /// The path.
        path: Vec<El>,
        /// Line width in pixels.
        width: f64,
        /// Paint.
        paint: Paint,
    },
    /// Items drawn through an affine map (M3 C3.3, ADR-0020): their geometry, stroke widths and paint mappings are in
    /// the layer's own space and `transform` (`[a, b, c, d, e, f]`, `x' = a x + c y + e`, `y' = b x + d y + f`) takes
    /// it to the enclosing space, so a skewed or anisotropically scaled element strokes and paints exactly.
    Layer {
        /// The layer's space → the enclosing space.
        transform: [f64; 6],
        /// Items, back to front.
        items: Vec<Item>,
    },
}

/// A scene to render.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    /// Width in pixels (1–8192).
    pub width: u32,
    /// Height in pixels (1–8192).
    pub height: u32,
    /// Background, straight alpha.
    pub background: Rgba,
    /// Items, back to front.
    pub items: Vec<Item>,
}

/// Why a render failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RenderError {
    /// A size outside 1–8192.
    #[error("render size {0} x {1} is outside 1-8192")]
    Size(u32, u32),
    /// An image paint whose buffer does not match its size.
    #[error("an image paint of {0} x {1} has {2} bytes")]
    ImageSize(u16, u16, usize),
    /// The caller's cancellation or work account stopped the render.
    #[error("render stopped: {0}")]
    Stopped(String),
    /// PNG encoding failed.
    #[error("PNG encoding failed: {0}")]
    Png(String),
}

fn colour(c: Rgba) -> AlphaColor<Srgb> {
    AlphaColor::from_rgba8(c[0], c[1], c[2], c[3])
}

fn bez(path: &[El]) -> BezPath {
    let mut b = BezPath::new();
    for el in path {
        match *el {
            El::MoveTo(x, y) => b.move_to((x, y)),
            El::LineTo(x, y) => b.line_to((x, y)),
            El::CubicTo(x1, y1, x2, y2, x, y) => b.curve_to((x1, y1), (x2, y2), (x, y)),
            El::Close => b.close_path(),
        }
    }
    b
}

fn stops(s: &[(f32, Rgba)]) -> Vec<(f32, AlphaColor<Srgb>)> {
    s.iter().map(|(o, c)| (*o, colour(*c))).collect()
}

fn set_paint(ctx: &mut RenderContext, paint: &Paint) -> Result<(), RenderError> {
    match paint {
        Paint::Solid(c) => {
            ctx.reset_paint_transform();
            ctx.set_paint(colour(*c));
        }
        Paint::Linear { stops: s, unit_to_pixel } => {
            ctx.set_paint_transform(kurbo::Affine::new(*unit_to_pixel));
            ctx.set_paint(Gradient::new_linear((0.0, 0.0), (1.0, 0.0)).with_stops(stops(s).as_slice()));
        }
        Paint::Radial { stops: s, unit_to_pixel } => {
            ctx.set_paint_transform(kurbo::Affine::new(*unit_to_pixel));
            ctx.set_paint(Gradient::new_radial((0.5, 0.5), 0.5).with_stops(stops(s).as_slice()));
        }
        Paint::Image { rgba, width, height, unit_to_pixel, repeat } => {
            let want = usize::from(*width) * usize::from(*height) * 4;
            if rgba.len() != want || *width == 0 || *height == 0 {
                return Err(RenderError::ImageSize(*width, *height, rgba.len()));
            }
            let pixmap = Pixmap::from_parts(
                rgba.as_ref().clone(),
                *width,
                *height,
                vello_cpu::PixelMetadata { may_have_transparency: true, alpha_type: peniko::ImageAlphaType::Alpha },
            );
            let extend = if *repeat { Extend::Repeat } else { Extend::Pad };
            let image = peniko::ImageBrush {
                image: vello_cpu::ImageSource::Pixmap(Arc::new(pixmap)),
                sampler: ImageSampler { x_extend: extend, y_extend: extend, quality: ImageQuality::Medium, alpha: 1.0 },
            };
            let unit = kurbo::Affine::new(*unit_to_pixel)
                * kurbo::Affine::scale_non_uniform(1.0 / f64::from(*width), 1.0 / f64::from(*height));
            ctx.set_paint_transform(unit);
            ctx.set_paint(image);
        }
    }
    Ok(())
}

/// A non-repeating image covers only its unit square (03 §7.4: transparent outside the once-stretched image;
/// C23-R1-06): the paint is clipped to the unit square mapped to pixels, rotated or skewed as the mapping is. Returns
/// whether a clip layer was pushed.
fn push_unit_clip(ctx: &mut RenderContext, paint: &Paint) -> bool {
    let Paint::Image { unit_to_pixel, repeat: false, .. } = paint else { return false };
    let m = kurbo::Affine::new(*unit_to_pixel);
    let mut quad = BezPath::new();
    let corners = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
    for (i, c) in corners.iter().enumerate() {
        let p = m * kurbo::Point::new(c.0, c.1);
        if i == 0 {
            quad.move_to(p);
        } else {
            quad.line_to(p);
        }
    }
    quad.close_path();
    ctx.reset_paint_transform();
    ctx.set_fill_rule(PenikoFill::NonZero);
    ctx.push_clip_layer(&quad);
    true
}

/// Draws `items` under `base` (the transform of the enclosing layers); `keep_going` is asked before each item, with
/// a running count over all nesting levels.
fn draw_items(
    ctx: &mut RenderContext,
    items: &[Item],
    base: kurbo::Affine,
    keep_going: &mut dyn FnMut(usize) -> Result<(), String>,
    counter: &mut usize,
) -> Result<(), RenderError> {
    for item in items {
        keep_going(*counter).map_err(RenderError::Stopped)?;
        *counter += 1;
        match item {
            Item::Fill { path, rule, paint } => {
                ctx.set_transform(base);
                let clip = push_unit_clip(ctx, paint);
                set_paint(ctx, paint)?;
                ctx.set_fill_rule(match rule {
                    Rule::NonZero => PenikoFill::NonZero,
                    Rule::EvenOdd => PenikoFill::EvenOdd,
                });
                ctx.fill_path(&bez(path));
                if clip {
                    ctx.pop_layer();
                }
            }
            Item::Stroke { path, width, paint } => {
                ctx.set_transform(base);
                let clip = push_unit_clip(ctx, paint);
                set_paint(ctx, paint)?;
                ctx.set_stroke(kurbo::Stroke::new(*width).with_join(Join::Round).with_caps(Cap::Round));
                ctx.stroke_path(&bez(path));
                if clip {
                    ctx.pop_layer();
                }
            }
            Item::Layer { transform, items } => {
                draw_items(ctx, items, base * kurbo::Affine::new(*transform), keep_going, counter)?;
            }
        }
    }
    ctx.set_transform(base);
    Ok(())
}

/// Renders a scene: premultiplied RGBA8 rows, top first. `keep_going` is asked between items (cancellation and the
/// caller's watchdog); it returns an error message to stop.
///
/// # Errors
/// [`RenderError::Size`], [`RenderError::ImageSize`], [`RenderError::Stopped`].
pub fn render(scene: &Scene, keep_going: &mut dyn FnMut(usize) -> Result<(), String>) -> Result<Vec<u8>, RenderError> {
    let (w, h) = (scene.width, scene.height);
    if !(1..=MAX_SIDE).contains(&w) || !(1..=MAX_SIDE).contains(&h) {
        return Err(RenderError::Size(w, h));
    }
    let (w16, h16) = (
        u16::try_from(w).map_err(|_| RenderError::Size(w, h))?,
        u16::try_from(h).map_err(|_| RenderError::Size(w, h))?,
    );
    let mut ctx = RenderContext::new_with(w16, h16, RenderSettings { level: Level::fallback(), num_threads: 0 });
    ctx.set_transform(kurbo::Affine::IDENTITY);
    let mut counter = 0;
    draw_items(&mut ctx, &scene.items, kurbo::Affine::IDENTITY, keep_going, &mut counter)?;
    ctx.flush();
    let mut pixmap = Pixmap::new(w16, h16);
    let settings = RasterizerSettings {
        render_mode: RenderMode::OptimizeQuality,
        target_init: vello_cpu::TargetInit::Clear(colour(scene.background)),
        ..RasterizerSettings::default()
    };
    ctx.render_with(&mut pixmap, &mut Resources::new(), settings);
    Ok(pixmap.data_as_u8_slice().to_vec())
}

/// Straight alpha from premultiplied RGBA8: `round(c × 255 / a)`, 0 where `a` is 0.
#[must_use]
pub fn unpremultiply(premul: &[u8]) -> Vec<u8> {
    let mut out = premul.to_vec();
    for px in out.as_chunks_mut::<4>().0 {
        let a = u32::from(px[3]);
        for c in &mut px[..3] {
            *c =
                (u32::from(*c) * 255 + a / 2).checked_div(a).map_or(0, |v| u8::try_from(v.min(255)).unwrap_or(u8::MAX));
        }
    }
    out
}

/// Encodes a premultiplied RGBA8 buffer as an RGBA8 PNG (straight alpha; default compression and filter, no
/// ancillary chunks, so equal buffers give equal files).
///
/// # Errors
/// [`RenderError::Png`].
pub fn encode_png(premul: &[u8], width: u32, height: u32) -> Result<Vec<u8>, RenderError> {
    let straight = unpremultiply(premul);
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, width, height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().map_err(|e| RenderError::Png(e.to_string()))?;
        writer.write_image_data(&straight).map_err(|e| RenderError::Png(e.to_string()))?;
        writer.finish().map_err(|e| RenderError::Png(e.to_string()))?;
    }
    Ok(out)
}

/// Decodes a PNG to straight-alpha RGBA8 (8-bit; palette, grey and RGB expanded): `(width, height, rgba)`. `None` for
/// anything unreadable or larger than `max_side` on a side.
#[must_use]
pub fn decode_png(bytes: &[u8], max_side: u32) -> Option<(u16, u16, Vec<u8>)> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(
        png::Transformations::EXPAND | png::Transformations::STRIP_16 | png::Transformations::ALPHA,
    );
    let mut reader = dec.read_info().ok()?;
    let (w, h) = (reader.info().width, reader.info().height);
    if w == 0 || h == 0 || w > max_side || h > max_side {
        return None;
    }
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let data = &buf[..info.buffer_size()];
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::GrayscaleAlpha => {
            data.as_chunks::<2>().0.iter().flat_map(|p| [p[0], p[0], p[0], p[1]]).collect()
        }
        _ => return None,
    };
    Some((u16::try_from(w).ok()?, u16::try_from(h).ok()?, rgba))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f64, y: f64, s: f64) -> Vec<El> {
        vec![El::MoveTo(x, y), El::LineTo(x + s, y), El::LineTo(x + s, y + s), El::LineTo(x, y + s), El::Close]
    }

    fn go(_: usize) -> Result<(), String> {
        Ok(())
    }

    #[test]
    fn an_opaque_square_covers_its_pixels_and_alpha_stays_255() {
        let scene = Scene {
            width: 8,
            height: 8,
            background: [0, 0, 0, 0],
            items: vec![Item::Fill {
                path: square(2.0, 2.0, 4.0),
                rule: Rule::NonZero,
                paint: Paint::Solid([255, 0, 0, 255]),
            }],
        };
        let px = render(&scene, &mut go).unwrap_or_default();
        let at = |x: usize, y: usize| &px[(y * 8 + x) * 4..(y * 8 + x) * 4 + 4];
        assert_eq!(at(3, 3), &[255, 0, 0, 255]);
        assert_eq!(at(0, 0), &[0, 0, 0, 0]);
        let png = encode_png(&px, 8, 8).unwrap_or_default();
        let back = decode_png(&png, 64).map(|v| v.2).unwrap_or_default();
        assert_eq!(back, unpremultiply(&px));
    }

    #[test]
    fn bad_sizes_images_and_a_stop_are_refused() {
        let mut scene = Scene { width: 0, height: 1, background: [0; 4], items: vec![] };
        assert_eq!(render(&scene, &mut go), Err(RenderError::Size(0, 1)));
        scene.width = 2;
        scene.items.push(Item::Fill {
            path: square(0.0, 0.0, 1.0),
            rule: Rule::EvenOdd,
            paint: Paint::Image {
                rgba: Arc::new(vec![0; 3]),
                width: 1,
                height: 1,
                unit_to_pixel: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                repeat: true,
            },
        });
        assert!(matches!(render(&scene, &mut go), Err(RenderError::ImageSize(1, 1, 3))));
        assert!(matches!(render(&scene, &mut |_| Err("cancelled".into())), Err(RenderError::Stopped(_))));
    }
}
