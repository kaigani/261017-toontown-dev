# M4 C4.3 design note 7: the Camera view

Status: before code (C4.3a: the kernel; C4.3b: the window). Plan `reviews/M4-C4.3-plan.md` §3.2-§3.5 and rulings
D-22 to D-27 (§15). Amends the render-transport note (`M4-design-render-transport.md`: the scene answer gains the
Camera view) and the views note (§2: the Camera view's mapping). The scene answer is defined normatively in
`docs/canvas-format.md` §11, written before the encoder changes (ruling D-24).

## 1. What the Camera view shows (05 §3.2 lines 311-328, §12.4 lines 899-902; answer 3)

The current camera's composition at the current frame: every element the final renderer's camera walk draws, in its
order, plus overlays the renderer never draws (05 lines 283-284): the camera frame (dimmed outside, 05 line 316), the
selected nodes' pivots, the selection highlighting (box highlighting or fading, 05 lines 319-321;
`drawAid.selectionDisplay`, 02 lines 959-961), the field grid, the background; a selected non-current camera as a thin
frame outline on the table plane (05 line 901). Motion paths, peg ghosts, element arms and the `anim.display.*` options
are M6's (answer 3), named in the merge note's deferrals.

## 2. One walk, two consumers (CV1)

The final renderer builds a frame's render list with `render_list::build` (`crates/executor/src/commands/render_list.rs`):
every content element of the scene with its world matrix, depth, Timeline rank, verdict (drawn or why not) and, when its
plane is in front of the camera, `to_pixel`, the affine map from drawing units to output pixels (`plane_to_pixel`: the
projection of three points of the plane). `composite_order` sorts it back to front (05 §2.8, SI-106: role tier, then
world Z grouped from an anchor within 1e-6 field, ties by Timeline order).

The Camera view's scene answer is built from **the same call** at the view's frame and the camera output size, and
each entry with a plane matrix that is drawn, or lies outside the camera frame (the view shows the table around the
frame, 05 line 316; a render leaves those out), becomes an **occurrence**: element id, drawing id, revision, form,
opacity, and `to_pixel` itself. Inside the frame the view and a render draw the same entries. The
canvas draws the occurrences in the list's order, each through its matrix composed with the view transform. So the
Camera view and a render cannot differ in what is drawn, in what order or where (CV1). Its colour slots, outlines,
gradients and the stroke construction are C4.2b's (TTRD 1.0, unchanged).

**The plane mapping is affine, as the final renderer's.** `plane_to_pixel` is exact for a plane facing the camera (the
camera is rigid, translation and roll only, 05 line 236) and is the renderer's own approximation for a plane rotated
out of the image plane; the Camera view uses the same matrix, so no homography exists anywhere (SI-145). A plane whose
reference points are at or behind the near plane (0.1 field, 05 line 241) has no `to_pixel` and is not drawn in either
consumer; a plane crossing the near plane is drawn by both exactly when `plane_to_pixel` gives a matrix (SI-145).

**Tolerance.** A drawing's geometry is flattened in its own units to the view tolerance divided by the largest singular
value of the occurrence's full matrix (drawing units to view pixels), the C42B-R1-01 rule. A drawing shown by several
occurrences is resident once (CV2), at the zoom bucket of its largest occurrence (SI-145).

## 3. The scene answer for a Camera view (normative: `docs/canvas-format.md` §11)

`canvas_scene` with a Camera view id answers the Camera view's composition. Members beyond the Drawing view's answer:
`viewType: "camera"`; `camera` (the camera's element id, its output size `[width, height]` in pixels, and
`cameraToView`, the affine from camera output pixels to view pixels: the view transform); `frame`; per entry
`elementId` and `matrix` (`to_pixel`, six numbers); the overlays: `cameraFrame` (the output rectangle, always
`[0, 0, width, height]` in output pixels), `pivots` (each selected node's pivot in output pixels), `selectionBoxes`
(each selected element's box in output pixels; the selection's union box), `otherCameras` (a selected non-current
camera's frame outline, four output-pixel corners), `selectionDisplay` (02 line 960's mode), `currentOnTop`. A Camera
view with no current camera answers `camera: null`, no entries, the background and the grid (SI-147; no command can
reach that state today: `anim.camera.delete` refuses the last camera). The revisions,
generation and colour table behave as in the Drawing view (C4.2b).

## 4. Invariants (one builder test each, `crates/executor/tests/c43.rs`)

- **CV1** For fixtures with depth, rotation, hidden elements and hidden ancestors, elements behind the camera and
  outside the frame: the Camera view's occurrences are the renderer's drawn and out-of-frame entries, back to front,
  each matrix equal (to 1e-9) to the element's plane through the camera computed by `inspect.coordinates.convert`, a
  path apart from the render list. (Bit equality holds by construction: the answer copies `to_pixel`.)
- **CV2** A drawing shown by several occurrences appears once among the payloads the canvas must load. Elements made
  by `element.clone` share a drawing bank (04 lines 429-431), so two occurrences can show one drawing, each through its
  own matrix; the test builds that case and checks the load set names the drawing once. (The C4.3a note said a drawing
  belongs to one element; that was wrong, round 1.)
- **CV3** The canvas receives every matrix; no window code computes a projection (C4.3b: a source check; C4.3a:
  the scene answer carries the matrices and the decoder validates them, R3-R4).
- **CV4** No overlay reaches a render: `render.frame.run` of a scene with a selection and pivots gives the same bytes
  with and without the selection.
- **M3 merge note §8 item 3, camera half (answer 7):** three agreement tests between the Camera view's mapping and the
  final renderer: (a) a rolled camera, (b) the default field of view, (c) an element under a hidden ancestor, seen
  through the camera (not drawn in either).

## 5. Spec issues recorded before the code

SI-145 (the plane mapping, the near plane, the tolerance and the bucket of a multi-occurrence drawing), SI-147 (a
Camera view without a current camera). The others the Camera view needs are in the views and gestures notes.

## 6. C4.3b: the window (before code; plan §3.2, §3.5; rulings D-22, D-24, D-25)

**Which canvas a view window shows.** `ViewPanel` shows the canvas (`CanvasView`) for every view window whose type is
Drawing or Camera (`drawingView`, `cameraView` and their floats); a Top or Side window keeps its placeholder, naming
M6 C6.1. The canvas follows the scene answer's `viewType`, so a Drawing view window switched to the Camera type shows
the Camera composition (the C4.2b residual "a Drawing view panel switched to another type keeps showing the drawing
canvas", owner C4.3, closes here). One canvas, one model, one back end per window, as in C4.2b (D-7a).

**What the kernel adds to the answer** (the members exist in form 1.1; the answer's form does not change). 05 §3.2
lines 319-321 and 02 §10.7 make the selection display a property of the composition:
- box highlighting off: every unselected occurrence carries a reduced `opacity` (SI-153), the "faded" display;
- outline on selection (it requires box highlighting, 02 §10.7): after the occurrences, one more entry per selected
  occurrence with `outline: true` and `tint` the highlight colour, "even where the element is hidden";
- selection always visible: a selected element hidden in the scene is included as an occurrence.

These entries are view-only. CV1 holds for the rest: they are the renderer's drawn and out-of-frame entries, and
CV4 still holds: none of them reaches a render. The window does not decide any of it; it draws what it is given.

**Drawing an occurrence.** Each back end draws a Camera-view entry through `cameraToView · matrix` (a per-draw matrix:
a uniform per draw in WebGL2, a dynamic uniform offset in WebGPU); a Drawing-view entry through `drawingToView`, as
before. The window composes two matrices it received; it computes no projection (CV3).

**Order.** In the Camera view the canvas keeps the kernel's order (CV1: the renderer's back-to-front order); the
Drawing view's role order of 02 §4.2 (`layers.ts`) does not apply to it. With `currentOnTop` (02 §10.5, Camera view
only) the current drawing's occurrences are drawn after the others. The outline entries come last.

**The grid** (05 §3.2: "the grid and overlay/underlay of section 02"). In the Camera view the field guide is drawn in
camera output pixels, aligned with the camera frame: the 12-field guide has 12 cells across the frame's width (05
§1.2: through the default camera the frame covers 12 fields), the 16-field guide 16, the normal grid as the 12-field
one (SI-153); it goes through `cameraToView`, under or over the drawings as in the Drawing view.

**Overlays (layer 9), all from the answer, in view pixels through `cameraToView`:**
- the area outside the camera frame dimmed, and the frame's outline (05 §3.2 line 316);
- each selected non-current camera's frame as a thin outline (05 §12.4 line 901);
- each selected node's pivot as a cross (05 §3.2 line 317);
- with box highlighting, each selected element's box in the highlight colour (05 lines 319-321);
- the scene tool's furniture on the union box: the handles of SI-150, and the pivot marker with the Transform tool;
- a drawing tool's gesture ink, marquee and selection outline as in the Drawing view (C4.2b), through the drawing
  mapping below.

Colours, widths and the dimming are SI-153's (SI-138 for what the Drawing view already shows).

**Resident geometry and tolerance** (SI-145; CV2). A drawing is loaded once. Its zoom bucket is the bucket of its
largest occurrence: the model takes, per drawing, the largest scale of `cameraToView · matrix` over its entries (in the
Drawing view, the one `drawingToView` as before).

**The drawing mapping for drawing tools.** `draw.viewMapping.get` for the Camera view (C43A-R1-01) gives the current
drawing's mapping, which the canvas uses for gesture ink widths and for the drawing tools' overlays. The canvas asks
again whenever the scene answer's revision, its frame or the current element changes, or the view moves (a camera move
changes it).

**Click to activate** (views note V9; SI-142; answer 4). A press in a view window that is not the active view sends
`view.active.set {viewId}` first (user origin; UI state, no undo step), then the same press is the tool's press: one
gesture, its commands after the activation. The canvas knows the active view from `app.state.get` and the `shell.view`
events.

**Captures and renders.** The attached `inspect.screenshot` of the Camera view is the window's capture (D-6, C4.2b),
compared in the e2e with the `activeView` final render (< 1 % of pixels differ, ruling O-5): plain, zoomed, rolled,
and after each scene gesture.

**Invariants added in C4.3b (one test each):**
- **CV1, window half**: with a recording back end, the canvas draws a Camera-view answer's entries in its order, each
  through `cameraToView · matrix`; `currentOnTop` moves only the current drawing's entries (Vitest, `frame.test.ts`).
- **CV2, window half**: a drawing shown by two occurrences is loaded once, at the bucket of the larger (Vitest).
- **CV3, window half**: a source check over `desktop/src/canvas`: no field of view, camera position, `tan`, `atan` or
  perspective division is read or computed there; every matrix comes from the scene answer or `draw.viewMapping.get`.
- **CV5**: the selection display: box highlighting off fades exactly the unselected occurrences, outline on selection
  adds one outline entry per selected occurrence, selection always visible includes a hidden selected element; none
  changes a render's bytes (kernel, `c43`).
- **V9**: a press in an inactive view window activates it and acts as the tool's press; `edit.history.get` shows no
  step for the activation (e2e on Windows by OS input).

### 6.1 As built (C4.3b)

- **Captures against renders** (`scripts/c43-camera-e2e.py`, the `desktop gui` job). The `activeView` render of the
  Camera view is the camera's output image, not the view's area, so the comparison maps each capture pixel's centre
  through `inspect.coordinates.convert {from: view, to: image, view: cameraView}` (an affine from three points) to the
  render's pixel under it; capture pixels outside the frame (the dimmed outside, an overlay) and within 3 view pixels
  of its edge (the outline) are left out. Under 1 % of the compared pixels may differ by more than 32, and at least a
  quarter of the capture must be compared. Plain, zoomed out to 25 % (the frame and the dimmed outside in view),
  zoomed in to 200 %, at 50 % rolled 20 degrees, and after a scene drag. Workstation: all five within the bound.
- **V9** is checked there with real OS input on Windows; it found SI-159 (the canvas never knew the active view).
- **The three-way** (`scripts/c43-gesture-equivalence.py`, the byte-gate job, beside C4.2b's): the scene tools'
  click, moves, corner scale, skew and pivot, and the drawing tools in the Camera view (Brush, Rectangle, Ellipse,
  Line, Eraser, Cutter, Paint, the drawing Select's marquee). Engine 2 replays a scene tool's samples in camera units
  and a drawing tool's in drawing units; SI-158 records the last-bit differences of the scene moves and the skew.
- **The gesture record and the modal state name the acting tool** (the scene tool in the scene family), so a replay
  knows which tool made a gesture and an agent's `ui.busy` names it.
