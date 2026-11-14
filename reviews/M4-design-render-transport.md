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
