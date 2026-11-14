# M4 C4.2b design note 1: the render-list transport

Before the code (plan §3.2, §4 item 1; ruling D-3: C4.2b). Requirements: 06 §6.1 (one render list for the interactive
and final renderers), §6.3 (colours resolved at draw time), §6.9 (software path; overlay buffering; bitmap caps), §6.10
(the interactive renderer is not bound by determinism); 10 REV-02 (object revisions); ADR-0007 decisions 1-5 and open
questions 1-7; ADR-0006 decision 7; ADR-0001 (IPC boundary, no worker in the webview); rulings D-5, D-7a, D-7b.

## 1. What crosses the boundary

The webview never derives document state. Per view window (D-7a: per window, so a float has its own stream) the host
sends three kinds of message, in order, on one channel:

| Message | Carries | Encoding |
|---|---|---|
| `canvas.load` / `canvas.resync` | one drawing's tessellated geometry at a revision | binary (`tauri::ipc::Response`), the product format of §2 |
| `canvas.delta` | per drawing: transform, palette colours, visibility, layer order; deletions; dirty notices | small JSON or binary, whichever measures better (ADR-0007 decision 4); the same message type either way |
| `canvas.view` | the view's mapping matrices, size, zoom (the `shell.view` payload of design note 6) | JSON |

Geometry crosses IPC only on load, on resync and for drawings whose geometry changed (ADR-0007 decision 1). The webview
answers with `canvas.ack {generation, seq}` and, when it finds a mismatch or loses state, `canvas.resyncRequest
{drawingId}`.

## 2. The product format (replaces the spike's TTRL v1)

- **Header** (fixed 64 bytes, little endian): magic `TTRD`, `major`, `minor`, `generation` (u64, the engine generation,
  D-7b), `drawingId` (26-byte ULID text), `rev` (u64, the drawing's revision, REV-02), counts (layers, stroke vertices,
  fill vertices, fill indices, colour slots), the bounds, and the byte length of each section. The decoder validates
  the header against its schema, checks every section length against its counts and the total against the payload
  length, and refuses an unknown major version (ADR-0007 decision 4).
- **Sections**, in painter order of the drawing's render-list entries (06 §6.1; question 7): per layer a record of its
  fill triangles (vertex: x, y as f32 in drawing units, colour slot u16, flags u16) and indices (u32), then its stroke
  outlines, already triangulated (stroke outlines are tessellated with real caps and joins in Rust; question 6), then
  bitmap references (the image id and its display-capped size; pixels travel separately within 06 §6.9's cap).
- **Colour slots**: a slot per colour id, so a palette change is a `canvas.delta` with new slot values and no geometry
  (06 §6.3, ADR-0007 decision 1).
- **Flattening per zoom bucket** (question 7): curves are flattened in Rust to a tolerance of 0.25 view pixels at the
  bucket's top zoom; buckets are powers of two of the zoom table (01 §6.5); crossing a bucket schedules a reload of the
  visible drawings at the new bucket (a reload, not a resync: same revision, new tolerance).
- **One encoder (Rust, `crates/render`), one decoder (TypeScript)** and one golden corpus of byte files under
  `desktop/src/canvas/testdata/`: `cargo test` encodes the corpus' source drawings and compares bytes; Vitest decodes
  the same files and compares the decoded buffers with the corpus' expected JSON.

## 3. Identity, revisions, generations (questions 1, 2, 4; D-7b)

- Resident entries are keyed by `drawingId`; their content is valid for exactly one `rev`. Undo and redo change the
  drawing's revision like any edit (10 REV-02), so they need no special case.
- Every `canvas.delta` names the revision it applies to; a delta for another revision is not applied and the webview
  sends `canvas.resyncRequest` for that drawing (never a silent apply).
- Every message carries the engine `generation`. A message of another generation is discarded; on a new generation the
  webview drops every resident entry and the host sends a full load (D-7b: an engine restart is a generation change).
- A resync replaces one drawing's entry atomically: until the new geometry is decoded and uploaded the previous
  revision stays on screen.

## 4. Queue and backpressure (question 3)

- One bounded queue per view window on the host side: at most 64 messages, a stated constant measured in `c42.rs` and
  a Vitest test.
- When the queue is full the host coalesces in place, never grows it: transforms, colours and visibility of a drawing
  keep the latest value; geometry (loads, resyncs, dirty drawings), deletions and generation changes are never dropped
  and never reordered with respect to each other. If coalescing cannot make room because every entry is geometry, the
  host replaces the queued geometry of a drawing with its newest revision (the older one would be resynced anyway).
- The host sends at most one batch per webview frame acknowledgement, so a slow webview slows the stream instead of
  filling memory.

## 5. Residency and eviction (question 5)

A resident-memory budget per back end derived from 06 §6.9 (`displayBitmapMaxSize` for bitmaps; geometry counted in
bytes, default 256 MB per window, a preference later). Off-screen drawings are evicted least recently used and rebuilt
from the render list by `canvas.resyncRequest` when they return (ADR-0007 decision 2: the resident data is a cache).

## 6. Placement

Tessellation runs in Rust on a worker of the host (not the document thread: the document thread hands over the
drawing's immutable snapshot and its revision, so its hold stays within the 20 ms of plan §6); the webview decodes and
uploads only. ADR-0001's no-worker rule in the webview stands; ADR-0007 gets an amendment recording the placement and
the measured costs (E-C2).

## 7. Invariants (one builder test each)

- **T1** A resident drawing is shown only at the revision the kernel announced last, except during its resync, which
  shows the previous revision until the new one is uploaded.
- **T2** Geometry, deletions, resyncs and generation changes are never dropped or reordered with respect to each other;
  transforms, colours and visibility may coalesce (latest wins).
- **T3** The queue never exceeds 64 messages.
- **T4** The encoder and the decoder round-trip the golden corpus byte for byte; a truncated, overlong or unknown-major
  payload is refused before any geometry is built.
- **T5** An evicted drawing is rebuilt identically (same decoded buffers for the same revision and zoom bucket).
- **T6** A message of an old generation is discarded and a new generation reloads every resident drawing.

## 8. Amendments after C4.2a (before the code)

C4.2a's corrections and review lessons change the following; where this section and §1 to §7 differ, this section
holds.

- **The view message.** `canvas.view` is the host-only `shell.view` event C4.2a built (SI-132: `viewId`, `type`,
  `size`, `transform`, `drawingToView`, `viewToDrawing`), delivered to the window showing that view. At mount and after
  a generation change the canvas reads `view.transform.get` once. It never derives a mapping (design note 6).
- **One walk, then tessellation.**
  - The drawing is walked by the executor's `draw_render` builder, the same walk `draw.drawing.render` and the
    headless screenshot use (06 §6.1: one render list), in drawing units and with each paint's colour id kept.
  - `tt-render::canvas` tessellates those items with `lyon_tessellation` (MIT/Apache-2.0, added to the dependency
    audit), using the final renderer's own rules:
    - planar fill rings: even-odd;
    - brush outlines and glyphs: nonzero;
    - pencil strokes: round caps and round joins (`tt-render` strokes with `Join::Round`, `Cap::Round`).
  - Curves are flattened at the zoom bucket's tolerance (SI-137).
  - The encoding is `tt-render::canvas::encode`. The decoder is `desktop/src/canvas/format.ts`.
- **Record kinds** (every one present in the golden corpus):
  1. solid mesh: vertices `x, y` (f32), a colour slot (u16), flags (u16), and u32 indices;
  2. gradient mesh: the same vertices, plus the gradient's kind (linear or radial), its points and its stops (colour
     slots), evaluated per fragment;
  3. bitmap reference: the image id and its placement; pixels travel in their own capped message (06 §6.9).
  Text and glyph outlines are solid or gradient meshes. A texture-filled zone is drawn with its mean colour until
  bitmap transport exists. That is a declared preview approximation (06 §6.10), recorded in SI-137.
- **Colour table.** One slot per colour id: the id and its RGBA at the revision. A palette change is a `canvas.delta`
  with new slot values (06 §6.3).
- **Validation rules, each with its own corrupt file in the golden corpus**, refused before any geometry is built,
  in Rust (`decode` exists for the tests) and in TypeScript:
  - V1 magic;
  - V2 major version;
  - V3 header length;
  - V4 a section length against its count;
  - V5 the sum of the sections against the payload length (truncated and overlong);
  - V6 an index out of range;
  - V7 a colour slot out of range;
  - V8 an unknown record kind;
  - V9 a non-finite coordinate;
  - V10 a drawing id that is not 26 ULID characters.
- **Matrices, not samples.**
  - Every header field is varied in the corpus and checked after decoding, in both languages.
  - Every record kind round-trips.
  - Every validation rule has a cell that must refuse, plus the unmodified file, which must decode. A decoder that
    skipped the rule fails its cell.
- **Absent, null and cleared (D-7e).** Every JSON message (`canvas.delta`, `canvas.ack`, `canvas.resyncRequest`, and
  `gesture.commit` of note 2) states, field by field, whether it may be absent, null or an empty value, and what each
  means. One table-driven wire test sends every field in every allowed state through the real serialisation, in both
  directions.
- **Invariants added:** T7 (D-7a) — a float window has its own queue, frame loop and resident set, and closing it drops
  them without affecting the main window.
