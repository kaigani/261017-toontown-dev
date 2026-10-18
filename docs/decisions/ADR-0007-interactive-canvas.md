# ADR-0007 — Interactive canvas architecture (for M4)

Status: accepted as the M4 direction (C0.4, 2026-10-18; rule set added after review round 1). The delta protocol is open (below). The macOS hardware
result is pending the user.

## Context

Plan §2.1 draws the interactive canvas in the webview from a render list produced in Rust. Its constraints:
- WebGPU, with WebGL2 as fallback;
- in-progress gestures drawn as a local overlay;
- the document single-sourced in Rust.

Spec 06 §6.1 requires the interactive and final-quality renderers to consume the same render list. Spec 06 §6.9
requires a software path for the interactive views when GPU acceleration is unavailable.

The C0.3 spike measured three scenes: paths100k (100,000 stroked paths), groups1k (1,000 transformed groups) and
brush10k (10,000 filled brush outlines). Results (`reviews/M0-C0.3-handoff.md`; logs and analyses in
`reviews/evidence/M0-C0.3/`, rounds 1–3):
- **Resident redraw meets 60 fps on this machine.** RTX 4090, WebView2 155, 1904×757 canvas: every scene runs at
  **60.0 fps** on both WebGPU and WebGL2, with no long frames, in repeated runs.
  - Runs: `canvas-local-windows-rtx4090-*.json.gz`, `canvas-r1-…`, `canvas-r2-local-windows-rtx4090-auto.json.gz`;
    the reviewer's fresh run is `reviews/evidence/M0-C0.3-r3/canvas-auto.json.gz`.
  - WebGPU GPU completion for paths100k is p50 about 8.5 ms. This is "completion latency [that] includes queue work
    and callback scheduling; it is not GPU timestamp duration" (reviewer, r4 closure).
- **Full-list transport does not fit a frame.** Over binary `tauri::ipc::Response`, transfer p50 and JS decode
  (flatten + earcut) are, per workload, across both back ends of the original automatic run
  (`canvas-local-windows-rtx4090-auto.json.gz`) and the reviewer's fresh R3 run
  (`reviews/evidence/M0-C0.3-r3/canvas-auto.json.gz`):
  - **paths100k** (8.7 MB): transfer 50.3–70.4 ms, decode 85.3–130.5 ms;
  - **brush10k** (10.09 MB): transfer 63.6–72.3 ms, decode 225.4–255.4 ms;
  - **groups1k** (2.02 MB): transfer 13.2–16.3 ms, decode 30.4–37.1 ms.

  Binary throughput was 128–172 MB/s in the original runs; JSON IPC is about 100 MB/s round trip (C0.1).
- **CI runners.**
  - Windows runner: software WebGL2 (Microsoft Basic Render Driver) gives paths100k 0.6–0.8 fps, groups1k 16.9 and
    brush10k 8.9.
  - macOS runner (paravirtual "Apple GPU"): 22.1 / 60.0 / 60.0 fps.
  - Neither runner yields a WebGPU adapter. On macOS `navigator.gpu` is present but returns no adapter in the VM.
- **Bitmap fallback.** Transferring and uploading a full 1904×757 frame alone costs 36–44 ms, before any CPU raster.

The reviewers' closure (r4) separates what is established from what is not:
- *Established:* "Evidence strongly supports per-drawing GPU residency and deltas".
- *Not established:* "It does not establish a host-to-GPU delta protocol; the spike animates transforms locally in
  JS". Host raster plus full bitmap transfer "is not supported as a 60 fps fallback at measured sizes". Native wgpu
  child-window composition "remain[s] an unbuilt hypothesis".

## Decision

1. **The scene is GPU-resident.** Geometry is resident on the GPU side per drawing, keyed by stable identities, and
   updated by per-frame deltas:
   - transforms, colours and palette, since spec 06 §6.3 resolves colours at draw time;
   - visibility;
   - dirty drawings.

   Full geometry crosses IPC only on load, on resync, and for drawings whose geometry changed. In-progress gestures
   are drawn in a local overlay (spec 06 §6.9 overlay buffering) and committed as one command (plan §2.1).
2. **There is one authoritative render list.** The resident GPU data is a cache of the logical render list produced
   by the Rust compositor (spec 06 §6.1), never a second scene model. The final-quality renderer (ADR-0006) consumes
   the same list. Any cache entry can be rebuilt from it, and resync is always possible.
3. **WebGPU and WebGL2 both ship; WebGL2 is mandatory and equal; the faster one is used.** Both back ends ship from
   the start with the same feature set and are tested on their own (`--spike-renderer webgl2` in the spike). A
   missing adapter is a normal condition, not an error: both CI runners and the macOS 12 floor (ADR-0005) must work
   on WebGL2. Which back end runs on a machine, and what each measured outcome decides, is fixed by the rule set
   below.
4. **Binary IPC for geometry.** Loads and resyncs use `tauri::ipc::Response` binary payloads with a certified
   envelope:
   - the header is decoded and validated against its schema;
   - the byte length is checked against the counts, as in the C0.3 TTRL v1 envelope.

   The TTRL v1 layout is a spike format. M4 versions a product format whose encoder and decoder contracts are equal,
   as C03-R2-03 required. Small deltas may use JSON or binary, whichever measures better.
5. **The software path (spec 06 §6.9).** The first candidate is the webview's own software rendering: WebGL2 on WARP
   or SwiftShader, which the Windows runner already exercises. Host raster plus bitmap transfer stays a measured
   alternative for GPU-less machines, not a throughput fallback. A native `wgpu` child surface is **not** adopted. It
   remains a hypothesis to revisit only if real Mac or device evidence shows the webview path failing.

## Canvas rule set (normative; the go/no-go report carries an identical copy)

`desktop/src/test/decisionTables.test.ts` fails if the two copies differ.

<!-- canvas-rules:begin -->
**Per-scene target.** The C0.3 scenes and the same threshold for each:
- **paths100k**: one drawing of 100,000 stroked paths;
- **groups1k**: 1,000 transformed groups, the plan's "1 000-element scenes";
- **brush10k**: 10,000 filled brush outlines.

For one back end, a scene is:
- **met** when its run completes and the analyzer reports `meetsTarget: true` (fps ≥ 57 over the 10 s pan/zoom);
- **missed** when the run completes below 57 fps, including a complete run with "insufficient samples" (one sample
  in 10 s is far below target);
- **failed** when the run carries an error after the back end started;
- **unavailable** when there is no WebGPU adapter or no WebGL2 context.

**Back-end selection (product rule, M4).**
- At first launch, and again after a GPU, driver or webview version change, the app measures both back ends that
  start on a short built-in pan/zoom of the three scene shapes.
- It uses the back end whose **worst** scene has the higher fps. A tie goes to WebGPU.
- A back end that failed is excluded until the next version change.
- The user can override the choice in preferences.

So "WebGPU available but slower" selects WebGL2 on that machine; it is not a fault.

**Outcome matrix.** It is applied per machine to the selected back end. For a spike log, that is the back end whose
worst scene is better.

| Outcome | Decision |
|---|---|
| The log does not analyze (analyzer exit 2) | not evidence: rerun; no decision |
| WebGL2 unavailable or failed, WebGPU met or missed | WebGL2 is the mandatory path, so investigate (driver, OS, WebKit version) before any conclusion for that platform or floor; WebGPU is used on that machine meanwhile |
| Both back ends unavailable or failed | no-go for the webview canvas on that platform until investigated |
| WebGPU unavailable, failed or slower; WebGL2 works | the selection rule picks WebGL2; continue with the rows below |
| All three scenes met | **go** as written |
| groups1k and brush10k met, paths100k missed | **go**; M4 adds culling and level-of-detail for drawings of that size before they are supported at 60 fps |
| groups1k met, brush10k missed (paths100k met or missed) | **go-with-condition**: M4 pre-tessellates fills in Rust and adds fill level-of-detail, and E-C2 re-measures brush10k. If it still misses, a native-wgpu integration prototype becomes required M4 work |
| groups1k missed | **no-go** for the webview canvas as designed on that platform: a native-wgpu integration prototype (plan §2.3 fallback) becomes required M4 work |
<!-- canvas-rules:end -->

## Open protocol questions (M4 design, to be measured)

1. **Identity.** Which ids key resident data (drawing, stroke and fill ids from the document model), and how they map
   to render-list entries across edits, undo and redo.
2. **Revision checks.** Each delta names the revision it applies to. A mismatch triggers a resync of that drawing,
   never a silent apply.
3. **Backpressure and batching.** What happens when the host produces deltas faster than the webview applies them:
   - per-frame coalescing (latest transform wins);
   - a bounded queue;
   - what is dropped and what must never be dropped (geometry, deletions).
4. **Resync.** Full or per-drawing snapshots after a reload, a lost message, a webview crash or a revision mismatch;
   how long a resync of a 100k-path drawing takes (C0.3 paths100k: 50.3–70.4 ms transfer plus 85.3–130.5 ms decode;
   to be re-measured on the M4 format).
5. **Ownership, lifetime and eviction.** GPU memory budgets (spec 06 §6.9 caps) and evicting off-screen drawings.
6. **Decode placement.** Decode currently runs on the main thread. Moving it to a worker conflicts with the IPC
   boundary's worker prohibition (C01-R4-02, accepted M1 C1.2 ticket), so that needs an ADR-0001 amendment. The
   alternative is pre-tessellation in Rust.
7. **Painter's order and correct strokes.** The spike's fills-then-strokes order, overlapping translucent joins and
   one-time flattening must be replaced (painter order, real caps and joins, zoom-dependent flattening) without
   losing the measured headroom.

## Consequences and named experiments

- **E-C1** (M4): a host-driven delta stream on groups1k-like scenes, with transforms sent from Rust every frame.
  Measures delivery latency, coalescing and the 60 fps budget, which the spike did not measure.
- **E-C2** (M4): sustained edits, which are dirty-drawing re-tessellation and upload stalls, plus compositor
  behaviour during resync.
- **E-C3** (before M4 sign-off): on a GPU-less machine or VM, compare webview software rendering with tiled host
  bitmaps end to end.
- **E-C4** (pending user): `docs/manual-tests/M0-C0.3-canvas-macos.md` on the user's Mac. Its two logs (automatic,
  and forced WebGL2) are decided by the canvas rule set above, with no further analysis.
- Retina, multiple displays, ProMotion and weaker GPUs remain unmeasured (r4 closure); E-C4 covers one Mac only.
