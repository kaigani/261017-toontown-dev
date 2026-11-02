# Vectorize decoder corpus (M2 C2.4)

Every file here is generated: `TT_GENERATE_CORPUS=1 cargo test -p tt-io --test c24_decode_fuzz -- --ignored generate`
(`crates/io/tests/c24_decode_fuzz.rs::generate_corpus`). Nothing is copied from elsewhere, and no file has a third-party
licence.

- **Valid images** (`card-*`, `e2-lineart.png`): a deterministic 48 × 32 test card (ring, filled square, diagonal bar,
  grey ramp; computed per pixel) encoded by the `image` crate's PNG, JPEG (quality 90), BMP, GIF (two frames), TIFF and
  TGA (RLE and raw) encoders, and by the test's own PSD writer (RGB raw/PackBits, RGB plus alpha, CMYK, 16-bit grey,
  1-bit bitmap, indexed). `e2-lineart.png` (256 × 192, 8-bit grey) is the line art of the E2 scenario: a frame split
  by a vertical line, a ring and a triangle, 5-px black lines on white.
- **Malformed and oversize** files are derived from those: truncations, an unknown signature, an empty file, a PSB
  version, a PSD declaring 100 000 px, a PSD whose header lies about its size, zip-compressed and Lab PSDs, and a TGA
  too short to have a header.

`manifest.json` records the result each file must give (`ok` or the error code). The fuzz test mutates these files with
the fixed seed `0xC24DEC0DE5EED001` (500 cases in `check`, 20 000 in the release job).
