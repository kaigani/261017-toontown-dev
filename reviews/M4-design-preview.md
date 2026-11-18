# M4 C4.3b design note 8: the live transform preview

Status: before code (C4.3b). Plan `reviews/M4-C4.3-plan.md` §3.4 ("live feedback"), answer 6 ("built with the Camera
view's preview as one mechanism (a local preview of the selection's transform while dragging, committed once;
ADR-0007 decision 1), for move, scale and rotate in both views"), rulings D-22 and D-27. Amends the frame-loop note
§7 (the Select overlay) and the gestures note §7.

## 1. What the spec asks

05 §5.0 line 399: "Views update live during a drag; the whole drag commits as **one** undoable step on release.
Cancelling (escape) during a drag restores the state before the drag." 02 §7.1.1 (the Drawing view's Select) moves,
scales and rotates objects by its handles. ADR-0007 decision 1: the window draws gestures locally and commits once.
C4.2 drew the Drawing view's Select drag as its selection outline only; C4.3a's scene tools draw nothing until the
commit.

## 2. One mechanism

A **preview** is a view-pixel affine `P` that the window applies to what it draws for the selection while a drag is in
progress. It is discarded when the drag ends, whether committed, cancelled or refused. The commit is unchanged: the
gesture's samples go to `gesture_commit`, the kernel's driver produces the one command, and the window draws the
committed state when it arrives. The preview never sends a command (PV1).

`P` is computed by the window from the drag's press and current sample, and the selection's furniture in view pixels,
by the same rule as the kernel's command:
- **move**: the translation by the drag;
- **scale** (a corner or side handle): scaling about the opposite corner or side, one axis for a side, both kept
  equal with the constrain modifier at a corner (SI-141, SI-150);
- **rotate** (the rotation zone): rotation about the box's centre, in 15° steps with the constrain modifier;
- **skew** (a skew handle, and the temporary Skew of SI-151): the shear of the side's axis about the box's centre.

The window takes the furniture from the same places it draws it: the Drawing view's selection geometry (frame-loop
note §7) and the Camera view's overlays (camera-view note §6).

## 3. What the preview moves

- **Camera view (scene tools).** The selected occurrences are drawn through `P · cameraToView · matrix` instead of
  `cameraToView · matrix`; the scene answer names each occurrence's element and the selection's boxes (form 1.1). The
  overlays of the selection (its boxes, pivots and furniture) move with `P`.
- **Drawing view (Select).** The selected objects are part of a resident drawing, whose payload has no object identity
  (TTRD §4). At the press on the selection the canvas loads two more payloads of the current drawing at its revision:
  the selected objects alone, and the rest. During the drag it draws the rest through `drawingToView` and the selected
  objects through `P · drawingToView`. They come from `canvas_load` with a new `part` argument (`all`, the default,
  `selection`, `rest`): the kernel's walk is the same, filtered by object id against the selection at that revision;
  a selected group or layer selects every object inside it, at any depth (C43B-R1-03: the first build matched ids only,
  so a selected group's members were drawn in the rest and did not move).
  The payload format does not change. Until the two parts arrive (a selection of a few objects arrives within a frame
  or two) the whole drawing and the moving outline are drawn, as in C4.2b.
- **Navigation.** A Grabber drag pans the view by `P` (a translation of everything drawn) and commits one
  `view.pan.set` at release; wheel zoom keeps its per-notch commands. This closes the C4.2b residual "Grabber and
  wheel navigation commit at release (owner: M4 C4.3, with the navigation tools' live preview)" for the Grabber.

## 4. Exactness (SI-154)

The preview is exact where the command's result is the same view-pixel affine:
- the selected objects of every Drawing-view Select drag, since a drawing's objects are transformed in drawing space
  and the view mapping is affine. A painted zone the kernel re-derives from moved strokes (02 §3.12) is not a selected
  object: it stays where it was until the commit;
- a Camera-view move of nodes at one depth;
- a Camera-view scale, rotation or skew of one node whose plane faces the camera, about the same point. The kernel takes a handle's factor, angle or
  shear from the union box and applies it to the node about its own pivot (SI-150), so with one node selected the
  window re-anchors the box rule's affine at that node's pivot (`reanchor` in `preview.ts`; C4.3b: the first build
  scaled about the opposite corner and was 16 view pixels off for a centred pivot, found by the e2e's PV2 row).

Elsewhere it approximates the result: several nodes at different depths or about different pivots, and planes turned
out of the image plane. There, the committed state replaces the preview when it is drawn. The preview is kept until
then: the model's `settledVersion` passes the revision before the commit, or 400 ms after a refusal, as C4.2b keeps
the gesture's ink. There is no frame without either.

## 5. Invariants (one test each)

- **PV1** A drag sends no command before its release; the release sends exactly the gesture's command (G7), and
  Escape sends nothing and restores the drawing as before the drag (05 line 399). Vitest (`input.test.ts`, a recording
  engine) and the e2e.
- **PV2** For each exact case of §4, the preview drawn on the last frame before the release equals the committed state
  drawn after it, within 1 view pixel at the selection's corners. e2e, both back ends: corners read from the window's
  summary.
- **PV3** `canvas_load {part}`: `selection` and `rest` together draw the same pixels as `all`, and `selection` holds
  exactly the selected objects (kernel test of the walk; a Vitest decode of both parts). C43B-R1-03 (ruling D-27): as a full table
  (`crates/executor/tests/c43/selection_parts.rs`): every selectable kind (pencil, brush, painted zone, text, bitmap)
  alone on each of two layers, in a group, in a nested group and as part of a partly selected group, and layers and
  groups themselves; each cell checks selection + rest = all and that the selected geometry is in `selection` only.
- **PV5** (C43B-R1S-06, C43B-R1-04) The overlay and the preview belong to their gesture: each press (and each agent
  gesture) is tagged by gesture and engine generation (`desktop/src/canvas/gestureLifecycle.ts`). A commit's answer
  clears the overlay and preview only while its gesture is the current one, so a release followed at once by a second
  press keeps the second drag's preview however late the first answer comes; a new generation and unmounting drop the
  preview with its loaded parts at once, and a later answer acts on nothing. C43B-R2S-02: after a successful answer
  that made an undo step, only the frame that draws the committed state (a scene revision past the one before the
  commit) clears the preview, however long the redraw takes; the time fallback applies only to an answer without a step
  (a refusal or an error at once, a commit that changed nothing after 400 ms). Vitest: a successful step whose model
  settles after more than 400 ms keeps the preview until that frame. Vitest with answers delivered when the
  test says (a delayed answer after a second gesture began; a restart between release and answer; unmount); the
  review's probe repeats the rapid pair 20 times through the real pointer.
  **The view (ruling D-53, C43B-R4-01).** A pending preview or overlay also belongs to the view its gesture was made
  in: the view window's type at the press and the mapping its overlays are drawn with (the Drawing type's
  drawing-to-view, the Camera type's camera-to-view). It is never drawn with another view's transform. While pending
  it is not drawn under any other view type or mapping (the window switched to the Camera view after a Drawing-view
  press); back in its own view before the answer it is drawn again; after the committed frame it is gone everywhere
  (`drawnInView`, `desktop/src/canvas/gestureLifecycle.ts`; Vitest `a pending preview belongs to its view`; the
  lifecycle script's row (f) through the window's type buttons).
- **PV4** The preview's affine follows the furniture rule of the kernel's command for every handle: for each row of
  the gestures note's tables, the window's `P` maps the box's corners where the committed command's result puts them,
  within 1 view pixel, in the exact cases. Vitest, against values derived in the test.

## 6. Budgets

Plan §6: drag feedback, from the sample received to the previewed occurrence drawn, ≤ 16.7 ms p95; release to the
committed matrices drawn ≤ 50 ms p95. Measured on the workstation, both back ends, release build, ≥ 30 samples
(ruling D-5).
