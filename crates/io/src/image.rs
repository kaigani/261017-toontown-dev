//! Bounded bitmap decoding for import and vectorize (02 §11.2; M2 C2.4). The input is untrusted: every format is
//! sniffed from its bytes, its declared size is checked against [`LIMITS`] before any pixel buffer is allocated, and
//! every failure is a typed [`DecodeError`] — no input panics.
//!
//! PNG, JPEG, BMP, GIF (first frame), TIFF and TGA are decoded by the `image` crate with its allocation limits set;
//! flattened PSD is read here from the file's composite image (header, colour-mode data, raw or PackBits channels;
//! bitmap, greyscale, duotone, indexed, RGB and CMYK at 1, 8 or 16 bits). CMYK is converted to RGB. The result is
//! always 8-bit straight RGBA.

use std::io::Cursor;

/// Input limits (also the limits of the model's bitmap nodes, 10 §4.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Largest file accepted, in bytes.
    pub max_bytes: u64,
    /// Largest width or height, in pixels.
    pub max_side: u32,
    /// Largest pixel count.
    pub max_pixels: u64,
}

/// The limits of import and vectorize: 256 MiB files, 16 384 px sides, 64 M pixels (256 MiB of RGBA).
pub const LIMITS: Limits = Limits { max_bytes: 256 << 20, max_side: 16_384, max_pixels: 64 << 20 };

/// A decoded image: 8-bit straight RGBA, rows top to bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba {
    /// Width in pixels (≥ 1).
    pub width: u32,
    /// Height in pixels (≥ 1).
    pub height: u32,
    /// `width × height × 4` bytes.
    pub pixels: Vec<u8>,
}

/// A recognised file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// PNG.
    Png,
    /// JPEG.
    Jpeg,
    /// Windows bitmap.
    Bmp,
    /// GIF (first frame).
    Gif,
    /// TIFF.
    Tiff,
    /// Truevision TGA (no signature: recognised by its file extension only).
    Tga,
    /// Photoshop document (flattened composite).
    Psd,
}

impl Format {
    /// The format's short name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Bmp => "bmp",
            Self::Gif => "gif",
            Self::Tiff => "tiff",
            Self::Tga => "tga",
            Self::Psd => "psd",
        }
    }
}

/// Why an image could not be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Not an image format import accepts.
    Unsupported {
        /// What was found.
        reason: String,
    },
    /// A recognised format whose content is damaged or uses a feature this reader does not support.
    Corrupt {
        /// The format.
        format: &'static str,
        /// What is wrong.
        reason: String,
    },
    /// The file or the image it declares is larger than [`Limits`] allows.
    TooLarge {
        /// `bytes`, `side` or `pixels`.
        limit: &'static str,
        /// The limit.
        maximum: u64,
        /// What the file has or declares.
        actual: u64,
    },
}

impl DecodeError {
    /// The registry code (02 §12.2): `vectorize.unsupportedFile`, or `limit.exceeded` for size limits.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unsupported { .. } | Self::Corrupt { .. } => "vectorize.unsupportedFile",
            Self::TooLarge { .. } => "limit.exceeded",
        }
    }

    /// A message for the error envelope.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Unsupported { reason } => format!("not a supported image file: {reason}"),
            Self::Corrupt { format, reason } => format!("the {format} file cannot be read: {reason}"),
            Self::TooLarge { limit, maximum, actual } => {
                format!("the image exceeds the {limit} limit ({actual} > {maximum})")
            }
        }
    }
}

fn corrupt(format: &'static str, reason: impl Into<String>) -> DecodeError {
    DecodeError::Corrupt { format, reason: reason.into() }
}

/// Recognises a format from the leading bytes, or from `extension` for TGA (which has no signature).
#[must_use]
pub fn sniff(bytes: &[u8], extension: Option<&str>) -> Option<Format> {
    let starts = |sig: &[u8]| bytes.starts_with(sig);
    if starts(b"\x89PNG\r\n\x1a\n") {
        Some(Format::Png)
    } else if starts(b"\xFF\xD8\xFF") {
        Some(Format::Jpeg)
    } else if starts(b"BM") {
        Some(Format::Bmp)
    } else if starts(b"GIF87a") || starts(b"GIF89a") {
        Some(Format::Gif)
    } else if starts(b"II*\0") || starts(b"MM\0*") {
        Some(Format::Tiff)
    } else if starts(b"8BPS") {
        Some(Format::Psd)
    } else if extension.is_some_and(|e| e.eq_ignore_ascii_case("tga") || e.eq_ignore_ascii_case("targa")) {
        Some(Format::Tga)
    } else {
        None
    }
}

fn check_size(width: u64, height: u64, limits: &Limits) -> Result<(), DecodeError> {
    let side = width.max(height);
    if side > u64::from(limits.max_side) {
        return Err(DecodeError::TooLarge { limit: "side", maximum: u64::from(limits.max_side), actual: side });
    }
    let pixels = width.saturating_mul(height);
    if pixels > limits.max_pixels {
        return Err(DecodeError::TooLarge { limit: "pixels", maximum: limits.max_pixels, actual: pixels });
    }
    Ok(())
}

/// Decodes `bytes` (a file whose extension, if known, is `extension`) within `limits`.
///
/// # Errors
/// [`DecodeError`]: unsupported or damaged files, and files or images over the limits.
pub fn decode(bytes: &[u8], extension: Option<&str>, limits: &Limits) -> Result<Rgba, DecodeError> {
    let len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if len > limits.max_bytes {
        return Err(DecodeError::TooLarge { limit: "bytes", maximum: limits.max_bytes, actual: len });
    }
    let Some(format) = sniff(bytes, extension) else {
        return Err(DecodeError::Unsupported {
            reason: "the file is not PNG, JPEG, BMP, GIF, TIFF, TGA or PSD".to_owned(),
        });
    };
    let image = match format {
        Format::Psd => psd::decode(bytes, limits)?,
        _ => decode_with_image(bytes, format, limits)?,
    };
    if image.width == 0 || image.height == 0 {
        return Err(corrupt(format.name(), "the image has no pixels"));
    }
    Ok(image)
}

fn decode_with_image(bytes: &[u8], format: Format, limits: &Limits) -> Result<Rgba, DecodeError> {
    let fmt = match format {
        Format::Png => image::ImageFormat::Png,
        Format::Jpeg => image::ImageFormat::Jpeg,
        Format::Bmp => image::ImageFormat::Bmp,
        Format::Gif => image::ImageFormat::Gif,
        Format::Tiff => image::ImageFormat::Tiff,
        Format::Tga => image::ImageFormat::Tga,
        Format::Psd => return Err(corrupt("psd", "internal: PSD is not decoded here")),
    };
    let name = format.name();
    let mut l = image::Limits::default();
    l.max_image_width = Some(limits.max_side);
    l.max_image_height = Some(limits.max_side);
    // Room for the decoded image in its own layout (up to 16-bit RGBA) plus the RGBA8 conversion.
    l.max_alloc = Some(limits.max_pixels.saturating_mul(12));
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), fmt);
    reader.limits(l);
    let decoder = reader.into_decoder().map_err(|e| image_error(name, &e))?;
    let (w, h) = image::ImageDecoder::dimensions(&decoder);
    check_size(u64::from(w), u64::from(h), limits)?;
    let img = image::DynamicImage::from_decoder(decoder).map_err(|e| image_error(name, &e))?;
    let rgba = img.into_rgba8();
    let (width, height) = rgba.dimensions();
    Ok(Rgba { width, height, pixels: rgba.into_raw() })
}

fn image_error(format: &'static str, e: &image::ImageError) -> DecodeError {
    match e {
        image::ImageError::Limits(l) => match l.kind() {
            image::error::LimitErrorKind::DimensionError => {
                DecodeError::TooLarge { limit: "side", maximum: u64::from(LIMITS.max_side), actual: 0 }
            }
            _ => DecodeError::TooLarge { limit: "pixels", maximum: LIMITS.max_pixels, actual: 0 },
        },
        image::ImageError::Unsupported(u) => corrupt(format, format!("unsupported feature: {u}")),
        other => corrupt(format, other.to_string()),
    }
}

/// The flattened composite of a Photoshop document (PSD version 1; PSB and zip-compressed data are refused).
mod psd {
    use super::{DecodeError, Limits, Rgba, check_size, corrupt};

    const F: &str = "psd";

    struct Reader<'a> {
        bytes: &'a [u8],
        at: usize,
    }

    impl<'a> Reader<'a> {
        fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
            let end = self.at.checked_add(n).ok_or_else(|| corrupt(F, "a length overflows"))?;
            let s = self.bytes.get(self.at..end).ok_or_else(|| corrupt(F, "the file is truncated"))?;
            self.at = end;
            Ok(s)
        }

        fn u16(&mut self) -> Result<u16, DecodeError> {
            let b = self.take(2)?;
            Ok(u16::from_be_bytes([b[0], b[1]]))
        }

        fn u32(&mut self) -> Result<u32, DecodeError> {
            let b = self.take(4)?;
            Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        }

        fn section(&mut self) -> Result<&'a [u8], DecodeError> {
            let n = self.u32()?;
            self.take(usize::try_from(n).map_err(|_| corrupt(F, "a section length overflows"))?)
        }
    }

    /// Unpacks one PackBits row into `out` (exactly `out.len()` bytes).
    fn packbits(src: &[u8], out: &mut [u8]) -> Result<(), DecodeError> {
        let (mut i, mut o) = (0usize, 0usize);
        while o < out.len() {
            let n = *src.get(i).ok_or_else(|| corrupt(F, "a compressed row is truncated"))?;
            i += 1;
            if n < 128 {
                let count = usize::from(n) + 1;
                let lit = src.get(i..i + count).ok_or_else(|| corrupt(F, "a compressed row is truncated"))?;
                let dst = out.get_mut(o..o + count).ok_or_else(|| corrupt(F, "a compressed row is too long"))?;
                dst.copy_from_slice(lit);
                i += count;
                o += count;
            } else if n > 128 {
                let count = 257 - usize::from(n);
                let v = *src.get(i).ok_or_else(|| corrupt(F, "a compressed row is truncated"))?;
                let dst = out.get_mut(o..o + count).ok_or_else(|| corrupt(F, "a compressed row is too long"))?;
                dst.fill(v);
                i += 1;
                o += count;
            }
        }
        Ok(())
    }

    pub(super) fn decode(bytes: &[u8], limits: &Limits) -> Result<Rgba, DecodeError> {
        let mut r = Reader { bytes, at: 0 };
        if r.take(4)? != b"8BPS" {
            return Err(corrupt(F, "missing signature"));
        }
        match r.u16()? {
            1 => {}
            2 => return Err(corrupt(F, "large documents (PSB) are not supported")),
            v => return Err(corrupt(F, format!("unknown version {v}"))),
        }
        r.take(6)?;
        let channels = usize::from(r.u16()?);
        let height = r.u32()?;
        let width = r.u32()?;
        let depth = r.u16()?;
        let mode = r.u16()?;
        if !(1..=56).contains(&channels) {
            return Err(corrupt(F, format!("{channels} channels")));
        }
        if width == 0 || height == 0 {
            return Err(corrupt(F, "the image has no pixels"));
        }
        check_size(u64::from(width), u64::from(height), limits)?;
        let base = match mode {
            0..=2 | 8 => 1, // bitmap, greyscale, indexed, duotone (its composite is greyscale)
            3 => 3,
            4 => 4,
            m => return Err(corrupt(F, format!("colour mode {m} is not supported"))),
        };
        if channels < base {
            return Err(corrupt(F, format!("{channels} channels for colour mode {mode}")));
        }
        let ok_depth = match mode {
            0 => depth == 1,
            _ => depth == 8 || depth == 16,
        };
        if !ok_depth {
            return Err(corrupt(F, format!("{depth}-bit colour mode {mode} is not supported")));
        }
        let palette = r.section()?;
        if mode == 2 && palette.len() < 768 {
            return Err(corrupt(F, "an indexed image needs a 768-byte palette"));
        }
        r.section()?; // image resources
        r.section()?; // layers and masks
        let compression = r.u16()?;
        let (w, h) = (
            usize::try_from(width).map_err(|_| corrupt(F, "width"))?,
            usize::try_from(height).map_err(|_| corrupt(F, "height"))?,
        );
        let row_bytes = match depth {
            1 => w.div_ceil(8),
            8 => w,
            _ => w * 2,
        };
        // Each channel the colour mode uses is decoded row by row straight into its slot of the RGBA output (C, M,
        // Y into R, G, B and K into A for CMYK; grey, indexed and bitmap into R), so memory is the output alone.
        // Extra (alpha, spot) channels are ignored.
        let counts: Vec<usize> = if compression == 1 {
            let mut c = Vec::with_capacity(channels * h);
            for _ in 0..channels * h {
                c.push(usize::from(r.u16()?));
            }
            c
        } else if compression == 0 {
            Vec::new()
        } else {
            return Err(corrupt(F, format!("compression {compression} is not supported")));
        };
        // The channel data must be present before the output is allocated: raw rows in full, and PackBits rows whose
        // byte counts can expand to a whole row (at most 64 output bytes per input byte).
        let remaining = bytes.len().saturating_sub(r.at);
        if compression == 0 && remaining < base.saturating_mul(row_bytes).saturating_mul(h) {
            return Err(corrupt(F, "the file is truncated"));
        }
        if compression == 1 {
            let used = counts.get(..base * h).ok_or_else(|| corrupt(F, "row counts"))?;
            if used.iter().any(|&n| n.saturating_mul(64) < row_bytes) {
                return Err(corrupt(F, "a compressed row is too short for the image width"));
            }
            if used.iter().sum::<usize>() > remaining {
                return Err(corrupt(F, "the file is truncated"));
            }
        }
        let mut pixels = vec![0u8; w * h * 4];
        let mut row = vec![0u8; row_bytes];
        for c in 0..base {
            for y in 0..h {
                if compression == 1 {
                    let n = counts.get(c * h + y).copied().ok_or_else(|| corrupt(F, "row counts"))?;
                    packbits(r.take(n)?, &mut row)?;
                } else {
                    row.copy_from_slice(r.take(row_bytes)?);
                }
                let out = pixels.get_mut(y * w * 4..(y + 1) * w * 4).ok_or_else(|| corrupt(F, "rows"))?;
                for x in 0..w {
                    let v = match depth {
                        // Bitmap mode: 1 is black.
                        1 => {
                            if row.get(x / 8).copied().unwrap_or(0) & (0x80 >> (x % 8)) != 0 {
                                0
                            } else {
                                255
                            }
                        }
                        8 => row.get(x).copied().unwrap_or(0),
                        _ => row.get(x * 2).copied().unwrap_or(0),
                    };
                    if let Some(slot) = out.get_mut(x * 4 + c) {
                        *slot = v;
                    }
                }
            }
        }
        for px in pixels.as_chunks_mut::<4>().0 {
            let rgb = match mode {
                2 => {
                    let i = usize::from(px[0]);
                    [palette[i], palette[256 + i], palette[512 + i]]
                }
                3 => [px[0], px[1], px[2]],
                4 => {
                    // PSD stores CMYK inverted (255 = no ink).
                    let k = u16::from(px[3]);
                    let ch = |v: u8| u8::try_from(u16::from(v) * k / 255).unwrap_or(255);
                    [ch(px[0]), ch(px[1]), ch(px[2])]
                }
                _ => [px[0], px[0], px[0]],
            };
            *px = [rgb[0], rgb[1], rgb[2], 255];
        }
        Ok(Rgba { width, height, pixels })
    }
}
