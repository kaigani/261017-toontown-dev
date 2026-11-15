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
- **CV2** A drawing shown by several occurrences appears once among the payloads the canvas must load. (In this
  model a drawing belongs to one element, so occurrences never share a drawing; the test checks the load set has no
  duplicates.)
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
