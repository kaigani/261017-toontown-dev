# M4 C4.4b design note 4: Perspective and Reposition All Drawings

Status: before code (C4.4b). Plan `reviews/M4-C4.4-plan.md` §5 intro, §5.2, §5.3, §5.5, §6 item 4, §7; §9 (ruling
D-40, answer 8); `reviews/M4-C4.3-plan.md` §15 rulings D-23, D-32, D-33, D-39, D-45. Covers the envelope and box
handles, the gestures, the non-affine preview, prechecks and refusals, and the tests. Builds on the gestures, pointer
and preview notes (SI-154), SI-070, SI-141 and the Contour Editor note (`reviews/M4-design-contour-editor.md`).

## 1. What exists today (read from the code)

- **Commands** (`crates/executor/src/commands/draw_transform.rs`), parameters as A lines 239 and 261 and the inventory
  rows: `draw.envelope.apply {drawingId?, ids?, corners: 4 points from bottom-left counter-clockwise, mapping:
  auto|projective|bilinear}` and `draw.repositionAll.apply {elementId?, translate?, scale?, rotate?, pivot? = {0,0},
  matrix?, corners?}`, both with `space`, `view`, `frame` (02 §13.1 item 5). Undo labels "Perspective Tool" and
  "Reposition All Drawings" (02 §7.1.3 line 473, §7.1.4 line 481).
- **The map** (`envelope_of`, line 661): a parallelogram target under `auto` is the exact affine map (SI-070); else
  `Envelope::new` (`crates/vector/src/ops/transform.rs` line 110) is projective for a strictly convex target and
  bilinear otherwise, self-intersecting targets included; `None` (bilinear on a box without width or height, a
  singular solve) is `drawing.invalidGeometry`. `map_stroke` maps anchors and handle ends **as points** (`anchor`,
  line 212), keeps a pencil line's thickness, gives fill matrices, text and bitmaps the three-corner affine fit.
- **Reposition All** (line 774) skips locked drawings silently (line 810), lets `corners` override the rest, and with
  `corners` maps **each drawing's own bounds** onto the same four points (line 818): different sizes, different maps.
- **Prechecks.** `precheck_envelope` = `edit_objects`: `drawing.noTarget`, `drawing.locked`, `drawing.nothingToDo`
  (`docs/availability-inventory.md` rows 473-476). `precheck_reposition_all` (line 1516): no current element, no bank,
  nothing movable (rows 622, 624); no `drawing.locked` row, since locked drawings are skipped.
- **Driver** (`crates/executor/src/gesture.rs`): `COVERED` (line 265) lacks both tools (`command.notImplemented`,
  owner null). Select's handles: `handle_at` (line 681; corners, then side midpoints within `HANDLE_PX` = 6 view px,
  then the rotation zone within `ROTATE_PX` = 18 px outside a corner) and `handle_drag` (line 702), on
  `draw_select::selection_bounds`. That box is **not** the one `draw.envelope.apply` maps (`bounds_of(objects_of(..))`):
  a layer or group counts its invisible strokes there (`node_bounds(n, true)`), and a zone stands for its strokes.
- **Options.** 02 §7.7 line 707 gives Perspective `mapping = auto`; `default_options` (`crates/executor/src/drawing.rs`
  line 39) has no Perspective entry and A no command to set it.
- **Window.** `CanvasView.tsx` previews only the Drawing view's Select (line 877; no Camera-view drawing tool has a
  preview), moving the selection part by one affine (`webgl2.ts` line 499, `webgpu.ts` line 602). TTRD vertices are
  drawing units tessellated by the kernel (`crates/render/src/canvas.rs`, SI-137); `flatten_into` ends each segment
  on its anchor, so brush-outline and fill-boundary anchors are mesh vertices; a pencil line's vertices sit half its
  width off its centre line.

## 2. The tool box and its handles

- **One box, computed in the kernel.** Perspective: `envelope_box(cx, d, ids)` (new in `draw_transform.rs`:
  `objects_of` then `bounds_of`, what the command calls). Reposition All: the current drawing's
  `vd::drawing_bounds(d, true)` (SI-189). Driver, command and scene answer call the same functions, so the handles
  drawn, the handles hit and the box mapped cannot differ.
- **Scene answer, form 1.2:** `canvas_scene` gains `toolBox: {box: [minX, minY, maxX, maxY], locked: bool} | null`
  for the active tool; `docs/canvas-format.md` §11 is amended first (the member's domain under R7), then `canvas.rs`
  and `desktop/src/canvas/scene.ts`.
- **Handles:** eight 8 px squares (`HANDLE_SIZE`, SI-138's accent and white centre) at the box's corners and side
  midpoints, through the drawing-to-view mapping, or in the Camera view the press mapping's inverse (SI-156). Hit
  radius 6 view px (SI-141 item 2), corners before sides. Reposition All adds the rotation zone (SI-141 item 3);
  neither tool shows the drawing pivot. `selection.ts` builds them as `ViewTris`, drawn alike by both back ends.
- **Disabled:** when the availability store (`useAvailability`) refuses the bare reference (neither precheck reads
  `corners`), the handles are grey and the cursor over them `not-allowed`, as the Contour Editor's locked form.
- **Cursors** (on pointer moves and the alternate key's keydown and keyup). Perspective: `move` over a handle; with
  alternate held over a side, that side's resize cursor (SI-141 item 5); else the default. Reposition All: Select's
  cursors over the box handles, `move` over a handle with alternate held, `move` elsewhere (any drag offsets).
- **During a drag** the overlay draws the target quadrilateral, its handles and the object outlines through the map.

## 3. Perspective gestures (02 §7.1.3 lines 468-470; §13.3 line 1227; SI-188)

The press sample is classified against the tool box at the release, in drawing units; `δ` is `pn.d − p0.d`; `c0..c3`
are `box_corners` (bottom-left first, counter-clockwise).

| Gesture | Command | Label |
|---|---|---|
| click, drag off the handles, alternate drag off the handles | `draw.selection.set {by: point / rect / lasso, mode}` as Select (constrain `add`, Select's `rectRule`) | none |
| drag corner `i` | `draw.envelope.apply {corners: cᵢ + δ, the rest unchanged, mapping}` | "Perspective Tool" |
| drag side `i` (corners `a`, `b`) | `{corners: a + δ, b + δ, the rest unchanged, mapping}` | "Perspective Tool" |
| alternate + drag side `i` | `δ = s·u + t·n` (`u` the side's axis, +x or +y; `n` outward): the corner with the larger coordinate along `u` moves `+s·u + t·n`, the other `−s·u + t·n` | "Perspective Tool" |
| Escape, lost capture, restart | nothing | — |

`mapping` is the tool's option, always explicit (02 §13.3); no `space` or `view` (no view-pixel tolerance), as
Select's `draw.transform.apply`. Constrain and temporary-select are ignored on handles. A commit changes the geometry,
so the next gesture's handles sit on the new axis-aligned bounds: envelopes do not accumulate.

## 4. Reposition All Drawings gestures (02 §7.1.4 lines 478-481; §13.3 line 1228; SI-189)

One step, "Reposition All Drawings", for all drawings of the current element (02 §12.1 line 1070); `elementId` is
omitted (current), as Select omits `drawingId`. Exactly one of the four operations per gesture.

| Gesture | Command |
|---|---|
| click | none (`to_command` answers `None`) |
| drag a corner or side handle | `draw.repositionAll.apply {scale, pivot}`: Select's factor and pivot (opposite corner or side; constrain keeps a corner's proportions) |
| drag in the rotation zone | `{rotate, pivot}`: degrees counter-clockwise about the box's centre |
| any other drag (alternate ignored: no copy) | `{translate: δ}` |
| alternate + drag corner / side; alternate + constrain + drag side | `{corners}` as Perspective's corner / plain side / alternate side drag |

`handle_drag` is split into a function returning the operation, shared by Select and this tool. With an empty current
drawing there is no box, and every drag offsets.

## 5. Pointer phases

Press: the collector starts and reports the modal gesture (`order.report`, kind `gesture`); the preview kind is chosen
(§6) and, for Perspective, the `selection` and `rest` parts load (`loadParts`, now in both views). Moves: samples
appended, preview recomputed, no command (PV1). Release: `gesture_commit`; `to_commands` builds one command (§3, §4)
or none, invoked once; agents' `pointer` takes the same driver headless, or is played by the window when attached
(`pointer_in_window`). Escape, lost capture, a float closing or an engine restart: nothing (G6).

## 6. The preview (plan §9 answer 8; SI-190, extending SI-154)

- **The map.** `desktop/src/canvas/envelope.ts` (new) ports `envelope_of` and `Envelope::new`/`apply` operation for
  operation in f64 (parallelogram test, strict convexity, partial pivoting, the 1e-12 guards), so the window takes
  the kernel's branch for the same corners. `preview.ts` gains `envelopePreview(toolBox, d2v, p0, pn, mods, tool)`:
  §3 and §4 in drawing units, answering `{kind: affine, m}`, `{kind: projective, h}`, `{kind: bilinear, min, max,
  corners}` or null.
- **The vertices.** Perspective: the `selection` part of `canvas_load` (preview note §3), the kernel's tessellation in
  drawing units. Reposition All: the current drawing's resident payload. Nothing is re-tessellated during the drag.
- **Mapping them.** `split.transform` becomes `split.map`; the vertex stage of both back ends (GLSL in `webgl2.ts`,
  WGSL in `webgpu.ts`) maps each vertex through it in drawing units, then through drawing-to-clip; one uniform block
  of 12 floats packed by one tested function, no per-vertex CPU work. Bitmap quads take the three-corner affine fit,
  as the commit does. In the Camera view the parts go through the current entry's own matrix (`entryToView`) after the
  map, drawn on top during the drag as Show Current Drawing on Top does (02 §10.5).
- **What moves.** Perspective: the selection part; zones the commit re-derives stay put (SI-154). Reposition All: the
  current drawing unless `toolBox.locked`; onion skins and the element's other drawings are not previewed and redraw
  at their new revisions when the commit lands.
- **Exact:** every affine map (Reposition's box handles and offset, parallelogram envelopes); under any map, anchor
  vertices and bitmap corners. **Approximate until the commit:** curves between anchors (the commit is the Bézier
  through mapped control points, not the projective image of the curve; projective maps keep straight segments
  straight, bilinear only those parallel to the box's axes), pencil widths (scaled locally here, constant at the
  commit; also true of SI-154's Select scale preview, which PV2 checked only at box corners), text glyphs, gradient
  colours (affine fit at the commit) and the flattening tolerance under enlargement. The plan's "exact at vertices"
  holds at anchor vertices, not at a pencil line's offset vertices.
- **Lifecycle** as PV5 and D-53. No preview when availability refuses at the press or the port answers null.

## 7. Prechecks and refusals

- **Perspective:** no current drawing or nothing selected: no handles, a drag is a marquee, the envelope is never sent
  by the window (an agent's selection change during a held drag waits, `ui.busy`, D-12). Locked: grey handles, no
  preview; the release still sends the gesture and the kernel refuses `drawing.locked`, nothing changed.
  `drawing.invalidGeometry`: no preview, the kernel's refusal at release. Concave and self-intersecting targets are
  not refused: they are bilinear (02 line 472) and previewed so.
- **Reposition All:** all drawings locked or empty: grey handles, `drawing.nothingToDo`. `corners` with no drawing at
  the current frame (no reference box): `drawing.noTarget`, now returned by the precheck, its availability row
  testable (an empty cell at the current frame).
- **Shown:** the refusal is the gesture's answer (`lastGesture` in the canvas summary) and its message goes to the
  status line (`messages.status(text, 'error')`), which the Contour Editor note §7 adds for every tool.

## 8. Code changes

Kernel: `COVERED` gains `perspective` and `repositionAll`, with `perspective()` and `reposition_all()` in `gesture.rs`
and a rotation-zone flag on `handle_at`; `envelope_box` and the reference box; the Reposition handler builds **one**
map from the reference box for every unlocked drawing (SI-189, a declared behaviour change; no test sends it
`corners` today); the precheck's `corners` case; `default_options("perspective") = {mapping: "auto"}`; `Scene.tool_box`.
Data: `gesture-table.json` rows `{view: drawing|camera, tool: perspective, commands: [draw.selection.set,
draw.envelope.apply]}` and `{…, tool: repositionAll, commands: [draw.repositionAll.apply]}`; both `deferred.json`
entries leave; `parity.json` regenerated; availability rows updated. Window: `envelope.ts`, `preview.ts`,
`selection.ts`, `CanvasView.tsx` (preview kinds `envelope` and `reposition`, both views), `renderer.ts`, `webgl2.ts`,
`webgpu.ts`, `scene.ts`, and the Tool options page's `mapping` choice (open question 1).

## 9. Tests

- **Driver rows** (`crates/executor/tests/c44b/envelope.rs`, new): every row of §3 and §4 through headless `pointer`
  in both views gives the named command and parameters (corners, factors, pivots derived in the test from the tool
  box) and step count; Escape and a Reposition click give none; a group holding an invisible stroke and a selected
  zone (where Select's box differs) move exactly the bound corner by `δ`; 6 px hits and 6.01 px misses at zoom 0.5,
  1, 2 and in a rotated view. SI-189: two drawings of different bounds take the one map; a locked one is unchanged
  and absent from `drawingIds`.
- **D-23 equivalence:** each row first asserts the spec (02 line 1227 or 1228; line 473 or 481 for the label; no step
  for selection rows; SI-188 and SI-189 rows say they state interim mappings), then headless `pointer` against the
  direct command on the same start: equal params, canonical saved bytes and ids, one step with the same label.
- **Golden maps and Vitest:** a kernel test writes `desktop/src/canvas/testdata/envelopes.json` (box, corners,
  mapping, branch, mapped sample points) byte for byte; `envelope.test.ts` reproduces it within 1e-12 du.
  `preview.test.ts`: the window's corners per handle row against values derived in the test (PV4); `selection.test.ts`:
  handles, grey form, cursors with and without alternate; `input.test.ts`: PV1; PV5 for both preview kinds.
- **Three-way** (`scripts/c44b-gesture-equivalence.py`, byte-gate job, an `EXPECTED` statement per row): Perspective
  click, marquee add, lasso, corner, side, alternate side (convex and concave); Reposition offset, corner and side
  scale, rotation, alternate corner, alternate constrained side; both views; exact where the headless replay takes the
  window's view size and transform, else 1e-9 relative with the difference recorded (D-32).
- **Preview against commit (the plan's 1 view px):** the sample drawing's unpainted rectangle (fixture of
  `scripts/c42-gui-e2e.py`, 30 to 150 by −60 to 60 du), Windows OS input held at the drag's end (`held_drag` of
  `scripts/c43-camera-e2e.py`): a keystone (top corners 24 du inwards, projective), a concave target (bilinear), a
  Reposition rotation and an alternate corner. Capture A: `inspect.screenshot {target: view}` while held, 0.6 s after
  the last sample; capture B after the release, once the summary shows the committed revision drawn. Ink mask: pixels
  with luminance below 0.35 (the overlay's accent and white handles are lighter). The symmetric Hausdorff distance of
  the two masks divided by the summary's `dpr` is at most 1. Both back ends and views; the receipt keeps the distance,
  the branch and the captures' hashes.
- **D-39 rows** (the C4.4 bindings table): Perspective corner, alternate side, locked refusal; Reposition offset,
  rotation, all-locked refusal; each with command and parameters observed, document effect, label, and grey handles
  plus the refusal where refused. MCP form in CI (`pointer` played by the window), OS form on the workstation; the
  held-capture rows are Windows-only, named in the handoff. **Availability:** a matrix (both commands × unlocked,
  locked, no current drawing, empty selection, all locked, empty current cell with `corners`) asserts availability
  equals execution. **Parity:** the battery of `crates/executor/tests/c43/parity.rs` gains both tools in both views.

## 10. Budgets (plan §5.5; D-33, D-45)

| Quantity | Bound | Measured |
|---|---|---|
| Release-to-committed, sample drawing | ≤ 50 ms p95 | `CanvasMetrics` (`metrics.ts`: release to the first frame drawing the committed revision), 30 OS drags per row (Perspective corner, Reposition offset and alternate corner), release build, both back ends |
| Release-to-committed, 50,000 points | ≤ 150 ms p95 | the same on the drawing `scripts/c42-canvas-measure.py` builds: Perspective after select all, Reposition offset (open question 2) |
| Work per frame during drags | ≤ 8 ms p95 | frame-callback work on frames drawn during a gesture only (C43B-R1S-08), both drawings, D-33's control, `scripts/c43_measure_assess.py`'s rules (incomplete fails) |

Workstation receipts, debug and release, 30 samples, regenerated at each handoff touching the code (D-45), in the
C4.4b measure script shared with notes 3 and 5. In CI no wall-clock budget: `c44b/budgets.rs` times each command on
25,000 and 50,000 points (`large()` of `c42/budgets.rs`, halved) in one process, asserting a ratio at most 2.5 and a
5 s debug ceiling (`common::timing::record`); Vitest has no clocks. Per-frame preview work is constant (one 8×8 solve
and the overlay): the vertices are mapped on the GPU.

## 11. Spec issues (proposed; to be entered in `plan/spec-issues.md` with the code)

- **SI-188 — Perspective's handles, side drags and `mapping` option (interim).** *Spec:* 02 lines 468-470 give corner
  and side handles without positions or radii, a side drag only with alternate, and "changes the size and angle of the
  opposite sides symmetrically" without a direction; line 707 gives a `mapping` option no A command sets. *Decision:*
  §2, §3: handles on the command's own box, re-taken after each commit; 6 px; a plain side drag moves the side; the
  alternate side drag spreads or closes its corners symmetrically about the midpoint (the larger-coordinate corner
  follows the drag along the side) and moves both by the perpendicular part, so the adjacent sides tilt as mirror
  images; constrain ignored on handles; an alternate drag off the handles lassos wherever it starts (the inside has no
  other meaning here); `mapping` a kernel option (default `auto`) sent with every envelope. *Why:* a symmetric
  keystone is line 467's change of perspective; one box to draw, hit and map. *Question:* the side rules (another
  reading moves the opposite side by the mirrored drag) and the option's command.
- **SI-189 — Reposition All: handles, the envelope's reference box, locked drawings (interim; behaviour change).**
  *Spec:* lines 478-480 give Select's box handles and envelope handles at the same positions; A line 261's `corners`
  names no reference box, and the kernel maps each drawing's own bounds, against "applied identically to every
  drawing"; line 414 refuses tools on a locked drawing. *Decision:* §4: box handles without modifiers, envelope
  handles with alternate; `corners` target the corners of the bounds of the element's drawing at the current frame,
  and that one map applies to every drawing; locked drawings are skipped, the call refused only when nothing can move.
  *Why:* one transform for all drawings is the purpose (line 477); the current drawing is what the user sees.
  *Question:* the reference box (the union of all drawings' bounds is the alternative).
- **SI-190 — the envelope preview (interim; D-40 answer 8, SI-154 extended).** *Spec:* 05 line 399 asks for live
  views, not their accuracy; the commit's map (02 lines 471-472, SI-070) is not affine. *Decision:* §6: the window
  maps the kernel's tessellated vertices through its port of the kernel's map in the vertex stage; exact for affine
  maps, anchor vertices and bitmap corners; approximate between anchors, for pencil widths, text and gradient colours;
  at most 1 view px on the sample drawing (§9). *Why:* ADR-0007 decision 1; constant per-frame cost. *Question:* as
  SI-154's.

## 12. Open questions for the orchestrator

1. **Perspective's `mapping` in the GUI.** SI-183 made `tool.options.set` agent-only, the page writing through each
   tool's own command; Perspective has none. Interim here: the Tool options page sends `tool.options.set {toolId:
   "perspective", options: {mapping}}`, binding it for this control and amending SI-183. Or a registry addition
   `tool.perspective.options.set {mapping}` by SI. Which?
2. **150 ms on the 50,000-point drawing.** Both tools move the whole selection or drawing, which is one layer; C4.2b's
   handoff §10 records 603 ms p50 for an edit re-noding it (owner M11 C11.1). Affine and convex projective maps keep
   the planar map's topology, so a mapped-map fast path is possible but changes M2's settle pass. This note measures
   and reports; is the bound asserted here or recorded as M11's residual?
3. **SI-189 changes a command agents use** (`draw.repositionAll.apply {corners}`). Confirm before the code.
