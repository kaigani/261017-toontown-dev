# ADR-0018 — Import and vectorize

Status: accepted (M2 C2.4, 2026-11-02). Scope: plan §4 row M2, checkpoint C2.4, readiness item 1 of
`reviews/M2-C2.3-r3.md`. Spec: 02 §11.1–§11.3, §12.2, design decision 13; 09 §11.2 (`VectorizeSettings`); spec A
(`draw.vectorize.preview`, `draw.vectorize.run`). Spec issues: SI-080, SI-081.

## Context

Vectorization is the product's first image → vector path; its input is untrusted. 02 §11.2 fixes the pipeline
(greyscale, filter, threshold, trace, Bézier fit at smoothness 3, brush shapes in one layer, 12-field width) but not the
filter kernels, the connectivity of ink and paper, the limits, or how a job commits several drawings.

## Decision

1. **Decoding** (`tt_io::image`): PNG, JPEG, BMP, GIF (first frame), TIFF and TGA through the `image` crate (only those
   formats, its allocation limits set) and an own flattened-PSD composite reader (bitmap, grey, duotone, indexed, RGB,
   CMYK; 1/8/16 bit; raw or PackBits). Limits are checked from headers before any pixel buffer: 256 MiB files, 16 384 px
   sides, 64 M pixels; a PSD's channel data must be present (raw size, or PackBits counts able to fill each row) before
   its output is allocated. Failures are `vectorize.unsupportedFile` or `limit.exceeded`. JPEG 2000 (a MAY) is not
   supported.
2. **Pipeline** (`tt_vector::vectorize`), integer and exact up to the fit: BT.601 grey over white; filters as 3×3
   kernels with clamped edges — `smooth` centre 4 / neighbours 1 (÷12), `smoothMore` box (÷9), `edgeEnhance` centre 10 /
   neighbours −1 (÷2), `sharpen` centre 32 / neighbours −2 (÷16); ink when grey < threshold % of 255. Tracing follows
   pixel cracks with ink 8-connected and paper 4-connected (each boundary is a cycle of a fixed successor map; at a
   saddle the walk turns onto the diagonal ink pixel), outer boundaries and holes grouped by an 8-connected labelling,
   regions in row-major order of their first pixel. Each region is one closed brush outline through its unit-edge
   midpoints (removing the half-pixel staircase), holes joined by zero-width bridges (SI-053), fitted at smoothness 3,
   scaled so the image width is 1200 du, centred on the origin. One work account per image
   (`VECTORIZE_WORK_BUDGET` = 1.5 × 10⁹ units, observing the watchdog's cancellation); images over 16 M pixels are
   refused.
3. **Colours:** a generated palette "Vectorization" with the swatch "Vectorization Black" (`#000000FF`), created on
   first use and reused; with texture, one texture swatch per image named after the file, its bitmap stored as the
   content-addressed entry `textures/vectorize-<hash>.png` and mapped by the shapes' fill matrix onto the image's
   extent. Like `doc.import.json`, the entry is stored with the call.
4. **Run:** a job; each image commits as its own transaction and undo step "Import and Vectorize" through
   `Core::run_staged` (the commands' staging and acceptance path; confirmed by the orchestrator's ruling on the C2.4
   plan), so cancelling keeps the finished images and the job reports `{elementId, drawingIds, completed, cancelled,
   skipped}` — `skipped` lists the images not committed, with reason `occupied` or `cancelled` (SI-083) — and a job
   that stops at a cancellation keeps that report. The command's second example shows two images as two undo steps.
   Images go to consecutive cells from `startFrame`; occupied cells: `ask` → `needs_confirmation` (category
   destructive, consequences coded `drawing.cellOccupied`), `replace` (a new drawing in the cell; the old one stays in
   the bank), `skip`. Target: `targetElementId`, else the current element — refused with `drawing.notVectorElement`
   when it is not a drawing or annotation element — else (a scene without elements) a new drawing element named after
   the first file, at the end of the scene's columns. An image element (§11.3) always becomes a new element named
   "<name> Vector" whose exposure mirrors the source, converted drawing by drawing.
5. **Preview:** the traced shapes rendered by the drawing renderer over white at `width` (1–4096) px; deterministic.

## Consequences

The vectorize corpus and fuzzing (`tests/fixtures/vectorize/`, `crates/io/tests/c24_decode_fuzz.rs`,
`crates/vector/tests/c24_trace.rs`) keep the decoder and tracer panic-free and bounded. The remembered settings
(`import.vectorize.settings.*`) stay M10 C10.2; until then omitted options take the spec defaults.

## History

- **M2 C2.4 (C23-T1): exact bridges.** The outline was fitted as one closed sequence, so a bridge's two passes were
  fitted with different neighbours and no longer coincided (a hairline sliver, and a centre line cut at every bridge).
  Each run of contour edges between bridges is now fitted open with its ends pinned, and the bridges stay straight
  segments with their two passes exactly reversed (SI-053). The `draw.vectorize.preview` example result was re-recorded
  (its PNG only). Evidence: `crates/executor/tests/c24_centre_line.rs`.
