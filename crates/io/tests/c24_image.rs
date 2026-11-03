//! M2 C2.4: unit behaviour of the import/vectorize decoder (formats, header limits, PSD composites, typed errors).
#![allow(clippy::unwrap_used, missing_docs, reason = "test")]

use tt_io::image::*;

fn png(w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Grayscale);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().unwrap();
    wr.write_image_data(&vec![0u8; (w * h) as usize]).unwrap();
    drop(wr);
    out
}

fn psd(w: u32, h: u32, mode: u16, channels: u16, data: &[u8]) -> Vec<u8> {
    let mut b = b"8BPS".to_vec();
    b.extend_from_slice(&1u16.to_be_bytes());
    b.extend_from_slice(&[0; 6]);
    b.extend_from_slice(&channels.to_be_bytes());
    b.extend_from_slice(&h.to_be_bytes());
    b.extend_from_slice(&w.to_be_bytes());
    b.extend_from_slice(&8u16.to_be_bytes());
    b.extend_from_slice(&mode.to_be_bytes());
    b.extend_from_slice(&[0; 12]); // three empty sections
    b.extend_from_slice(&0u16.to_be_bytes()); // raw
    b.extend_from_slice(data);
    b
}

#[test]
fn png_decodes_to_rgba() {
    let img = decode(&png(3, 2), None, &LIMITS).unwrap();
    assert_eq!((img.width, img.height, img.pixels.len()), (3, 2, 24));
    assert_eq!(&img.pixels[..4], &[0, 0, 0, 255]);
}

#[test]
fn limits_are_checked_from_the_header() {
    let small = Limits { max_bytes: 1 << 20, max_side: 2, max_pixels: 100 };
    assert_eq!(decode(&png(3, 2), None, &small).unwrap_err().code(), "limit.exceeded");
    let few = Limits { max_bytes: 1 << 20, max_side: 100, max_pixels: 5 };
    assert_eq!(decode(&png(3, 2), None, &few).unwrap_err().code(), "limit.exceeded");
    let tiny = Limits { max_bytes: 10, max_side: 100, max_pixels: 100 };
    assert_eq!(decode(&png(3, 2), None, &tiny).unwrap_err().code(), "limit.exceeded");
    // A PSD declaring a huge image is refused before its data is read.
    assert_eq!(decode(&psd(100_000, 2, 3, 3, &[]), None, &LIMITS).unwrap_err().code(), "limit.exceeded");
}

#[test]
fn psd_composites_rgb_cmyk_and_grey() {
    let rgb = decode(&psd(2, 1, 3, 3, &[10, 20, 30, 40, 50, 60]), None, &LIMITS).unwrap();
    assert_eq!(rgb.pixels, vec![10, 30, 50, 255, 20, 40, 60, 255]);
    // CMYK, inverted: no ink is white; full key is black.
    let cmyk = decode(&psd(1, 1, 4, 4, &[255, 255, 255, 255]), None, &LIMITS).unwrap();
    assert_eq!(cmyk.pixels, vec![255, 255, 255, 255]);
    let black = decode(&psd(1, 1, 4, 4, &[255, 255, 255, 0]), None, &LIMITS).unwrap();
    assert_eq!(black.pixels, vec![0, 0, 0, 255]);
    let grey = decode(&psd(1, 1, 1, 1, &[77]), None, &LIMITS).unwrap();
    assert_eq!(grey.pixels, vec![77, 77, 77, 255]);
}

#[test]
fn malformed_input_is_a_typed_error() {
    for bytes in [&b""[..], b"hello", b"\x89PNG\r\n\x1a\n", b"8BPS\x00\x01", b"BM\x00", b"GIF89a"] {
        let e = decode(bytes, None, &LIMITS).unwrap_err();
        assert_eq!(e.code(), "vectorize.unsupportedFile", "{bytes:?}: {e:?}");
    }
    // Truncated PSD channel data.
    assert!(decode(&psd(4, 4, 3, 3, &[1, 2, 3]), None, &LIMITS).is_err());
    // TGA only by extension.
    assert_eq!(sniff(b"\0\0\x02", Some("TGA")), Some(Format::Tga));
    assert_eq!(sniff(b"\0\0\x02", None), None);
}
