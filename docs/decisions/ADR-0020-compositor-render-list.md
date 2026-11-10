# ADR-0020 — Compositor and render list

Status: accepted (M3 C3.3; amended in C3.3 round 1).

## Context

06 §6 makes one render list per frame the input of every renderer back end, so that final renders, hit-testing and
interactive views differ in quality only. 09 §11 requires deterministic `render.frame.run` (identical pixels and PNG
bytes for equal inputs), inline/file rules and image-sequence jobs; 11 §7.5 requires `describe_frame`, `hit_test` and
`convert_coordinates` that agree with what is rendered; 11 §14 item 5 requires golden images that match byte for byte
across runs and between attached and headless modes. ADR-0006 fixes the rasteriser (vello_cpu, f32, scalar, single
thread); ADR-0019 fixes the evaluation order of transforms.

## Decision

1. **One render list** (`commands/render_list.rs`). For (scene, frame, camera, output size) the compositor walks the
   scene tree, evaluates every node through the C3.2 evaluator (charged, ADR-0019) and produces the content entries:
   drawing, image and annotation elements with their exposed drawing, world matrix, world Z, Timeline rank,
   drawing → pixel map, bounds and a visibility verdict. Renders, `inspect.frame.describe`, `inspect.hitTest`,
   `inspect.coordinates.convert`, the contact sheet and the range job read only this list.
2. **Order** (SI-106): role tier (background, normal and envelope, foreground; 04 §6.1), then the world Z of the origin
   (05 §2.8, smaller first), then the Timeline order (back to front = displayed rows bottom to top). Depths are not
   rounded (ruling C33-R1-01): within a tier the entries are sorted by Z and grouped from the smallest; a group takes
   each following entry whose Z is less than 1e-6 field from the group's first (smallest-Z) member, its anchor, and is
   ordered by the Timeline alone. **Chain rule:** a long run of near-equal depths splits where the distance from the
   anchor reaches 1e-6, although each step is smaller (0, 0.6e-6 and 1.2e-6 make two groups). Sorting and grouping use
   only comparisons of exact f64 values, so the order is platform-independent.
3. **Visibility** as `anim.evaluate` (SI-100): hidden (itself or an ancestor), annotation (view-only), no drawing at the
   frame, empty drawing, plane at or behind the near plane, out of frame. A request may carry a render-time override
   (ruling C33-R1-05): `include` draws an element although hidden or an annotation, `exclude` leaves an element and its
   descendants out (verdict `excluded`; exclusion wins). The override is part of the request, never of the document, so
   `exclude: [e]` renders the bytes of hiding `e`. Background elements extend their last exposure
   to the scene length. Effects pass the transform through (M7); bones are absent (SI-097).
4. **Projection per plane.** Elements rotate about Z only, so each lies in a plane parallel to the table plane and the
   rigid camera's perspective maps that plane to output pixels affinely. Three projected points (origin, +1 field in x
   and y) fix the map exactly; drawing units are 1/100 field.
5. **Rasteriser items.** `tt_render::Item::Layer { transform, items }` (additive): each drawn element's drawing is built
   in drawing units by the drawing builder of `draw.drawing.render` (shared, colours through the scene's active palette
   styles) and wrapped in a layer holding its drawing → pixel map, so vello strokes and paints anisotropic and skewed
   elements exactly. `draw.drawing.render` keeps its own path; the M2 goldens do not move.
6. **Bitmaps** (SI-105): a bitmap node's pixel space (origin top-left, y down) maps to drawing space through its
   `transform` (10 §4.9); the image is drawn as an image paint over its quad, clipped to it, bilinear (`ImageQuality::
   Medium`). Native size is the scale `1200 / cameraWidth` du per pixel with a y flip (`[s, 0, 0, −s, tx, ty]`), so one
   image pixel covers one output pixel of the default camera on the table plane; hit-testing reads the pixel's alpha
   (fully transparent pixels are not hit, 06 §6.8).
7. **Encoding.** PNG through `tt_render::encode_png` (no ancillary chunks: equal buffers give equal files); JPEG, TGA and
   BMP through the pinned `image` crate (`tt_io::image::encode_still`; JPEG keeps the premultiplied colour, i.e. over
   black).
8. **Never block.** Every frame charges a work account (path elements and pixels / 1024, 200 M units) and observes the
   caller's cancellation inside the rasteriser, an empty frame included (the check runs before each item and before
   and after the flush). Known sizes are refused before any evaluation, allocation or encoding (ruling
   C33-R1-04): sizes over 8192 and frames over 100 000 (`limit.exceeded`; per frame at a range's preflight), and an
   inline frame or contact sheet whose long side or pixel count is over the session's inline limit
   (`render.inlineTooLarge`). The encoded-byte limit is checked after encoding, which is bounded and cancellable.
   `render.range.run` runs as a job on a document snapshot, one frame at a time, checking cancellation. With
   `wait: true` the caller's request (never the document thread) waits for the job, bounded by the call's budget and
   cancellable; it answers the manifest with the `jobId`, or `job.waitTimeout` with the `jobId` while the job runs on
   (ruling C33-R1-02).
10. **Publication is a transaction** (ruling C33-R1-03). Frames and the companion manifest are staged beside their
   targets as `.<name>.tt-partial-<token>` (the token is the job's). At the commit point every target is preflighted
   (an existing target must be a writable regular file, else `export.targetInvalid`; the conflict policy was applied
   at submission); then each existing original is moved aside to `.<name>.tt-backup-<token>` and the staged file renamed
   onto the target. On any failure the published subset is removed, the originals are restored, the staging is
   deleted and the job is `failed` with the cause; backups are deleted only when every rename succeeded. The manifest
   exists only after full success.
11. **Recovery contract** (ruling C33-R1-08). Process death between staging and commit may leave `.tt-partial-` and
   `.tt-backup-` files with a job token. The product never publishes them; the next range job into that directory, and
   a cancelled or failed job, sweep the staging and backup files whose token no running job of the process owns, and
   the next job lists the swept names in its warnings (`render.staleStagingRemoved`, `files`). A swept backup whose
   target is missing (death between moving the original aside and renaming the staged file) is renamed back onto its
   target; one whose target exists is deleted (the target holds a frame the dead job published, re-renderable from
   the saved document).
12. **Volatile fields** (ruling C33-R1-06). `renderMs` (in `render.frame.run`, the generic `render_frame` result and
   per run), job IDs and job timestamps are volatile and excluded from deterministic byte comparisons; the PNG bytes
   and their `sha256` are the determinism contract. The retained example results record `renderMs` as varying.
13. **Invalidation.** `anim.changed` is conservative: every frame whose render can change is covered by the announced
   range (all-frame edits announce every frame); a frame inside the range may render unchanged.
9. **Determinism gates.** Golden PNGs (`crates/executor/tests/golden/c33/`) are compared byte for byte on both OSes in
   debug (`check`) and release (`c33 render release`); the render gate (`scripts/c33-render-gate.py`) replays
   `tests/fixtures/c33/gate-scene.json` on the release headless binary and checks the in-process hashes
   (`gate-index.json`), and `c33 cross-OS` compares both OSes' gate images, sequence frames, manifest and the passage
   frames of a non-horizontal path byte for byte.

## Consequences

- A change of `vello_cpu`, `png`, `image` or the compiler that alters a byte fails the goldens and the cross-OS gate; a
  legitimate change re-records the goldens and is declared with the old and new hashes.
- Effects (M7), bone deformation (M6 C6.3), the interactive renderer and views (M4 C4.2) add to the render list and its
  back ends without changing the order, visibility or projection rules above.
- Text pixels come from the machine's fonts (ADR-0016), so goldens contain no text.
