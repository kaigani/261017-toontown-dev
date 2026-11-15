# M4 C4.2b design note 2: the frame loop and the back ends

Before the code (plan §3.3, §4 item 2; ruling D-3: C4.2b). Requirements: 02 §4.2 (the nine layers of the canvas, back to
front), §10 (aids), §13.3 (the overlay of a gesture in progress); 06 §6.9 (antialiasing options, overlay buffering,
software path), §6.10; ADR-0007 decision 3 and the canvas rule set; ADR-0009 lines 70-72 (one input abstraction);
rulings D-5 (GPU quantities on the workstation with receipts, loosely on CI), D-7a (per-window frame loop), D-9.

## 1. One frame

`requestAnimationFrame` drives one frame per view window (D-7a):

1. **Apply deltas** received since the last frame, oldest first, until the queue is empty or the frame's delta budget
   is spent: at most 4 ms of decode and upload per frame, measured with `performance.now()`; work left over waits for
   the next frame (geometry already decoded is uploaded first, so a large resync spreads over frames while the
   previous revision stays visible, T1).
2. **Draw the static scene** in 02 §4.2's order, layers 1 to 8: background; grid (underlay); static light-table
   underlays; auto light table; onion skin; the current drawing; static light-table overlays; grid (overlay). Each
   resident drawing is drawn with the view matrix received in `canvas.view` (the canvas never computes a mapping,
   design note 6) and its colour slots.
3. **Draw the overlay** (layer 9): selection outline and handles, marquee or lasso, the gesture in progress, brush-size
   preview, cursor ghosts, invisible strokes and open ends when Show Strokes is on. With `overlayBuffering` on (06 §6.9,
   default) layers 1 to 8 are rendered to an offscreen target that is reused while only the overlay changes, so a
   gesture redraws one quad and the overlay.
4. **Acknowledge** the frame (`canvas.ack`), which lets the host send the next batch (design note 1 §4).

No step awaits IPC: messages arrive into a queue filled by the IPC callback and the frame only reads it.

## 2. Input

Pointer events (mouse, pen, touch) go through one input abstraction (D-9; ADR-0009 lines 70-72) producing the samples
of design note 3 §2: view point, pressure (`PointerEvent.pressure`, 0.5 for a mouse press is replaced by 1 as 02 §13.3
says), tilt, pointer kind (`pen` with the eraser button or `pointerType` eraser gives `penEraser`), buttons, modifiers
and `timeStamp`. The overlay draws the samples immediately; on release the samples go to the host in one
`gesture.commit` message, which calls the same `gesture::to_command` as the headless `pointer`, so both paths produce
the same command (G2). Escape, a lost capture or the window closing sends `gesture.cancel` (D-7c); nothing is sent
while the gesture is in progress except `gesture.begin` (the busy state of 11 §6.4). Coalesced events
(`getCoalescedEvents`) are delivered as samples so fast strokes keep every sample (G5).

## 3. Back ends

WebGPU and WebGL2 ship with the same features (ADR-0007 decision 3). Which one runs follows the canvas rule set's step 1
(the scorable back end with the higher worst-scene fps; a tie goes to WebGPU; a back end without a valid measurement is
never selected over one with): both are measured at first launch and after a GPU, driver or webview version change,
on the three rule-set scenes, for 2 s each (the spike used 10 s; the product measurement is shorter and repeated on the
next change if inconclusive); the result and the inputs are persisted in the user settings; a failed back end is
excluded until the next version change; the user can override it in preferences. The software path (06 §6.9) is the
webview's own WebGL2 on a software adapter (WARP, SwiftShader), which the Windows CI runner exercises.

Antialiasing follows 06 §6.9: MSAA when `interactiveAntialiasing` is on, analytic edge coverage for tool overlays when
`antialiasToolOverlays` is on.

## 4. Measurements (plan §6; D-5)

Frame time p95 over 300 frames idle and during a scripted gesture, input-to-ink, release-to-committed and the delta
queue length are recorded by the attached e2e on both CI OSes (loose bounds on CI: the runners have no GPU) and on the
workstation with receipts (the plan's bounds). The kernel-side holds stay in `c42.rs`.

## 5. Invariants (one builder test each)

- **F1** A frame's delta work stays within its budget (4 ms of decode and upload); leftover work is carried over, none
  is dropped (Vitest with a fake clock).
- **F2** No frame awaits IPC (the frame function is synchronous over the queue; a test with a stalled channel keeps
  drawing frames).
- **F3** The draw order is 02 §4.2's (a recording back end in Vitest lists the layers drawn).
- **F4** The back end is the rule set's output for the measured cells (the rule table's own Vitest cases, now run on
  the product selector).
- **F5** A cancelled gesture sends no command and leaves the overlay empty (D-7c).

## 6. Amendments after C4.2a (before the code)

Where this section and §1 to §5 differ, this section holds.

- **Gesture messages, concretely.** They reuse the paths C4.2a built, so equivalence holds by construction (G2, D-4):
  - **begin:** the C4.1 shell report with `modal {kind: "gesture", viewId}`. The kernel's mid-gesture user priority
    (ruling D-12) applies from that moment.
  - **commit:** the host command `gesture_commit {viewId, events}`. Its `events` are the `pointer` request's events, so
    `PointerRequest` is the one schema.
    - `dt` is the difference of consecutive `PointerEvent.timeStamp` values, rounded to whole milliseconds.
    - The pointer kind comes from `pointerType`, the pen's eraser button and `pointerType: "eraser"`.
    - A mouse press has pressure 1 (02 §13.3). A pen reports its own pressure; its hover samples are not sent.
    - Tilt is carried (SI-125).
    - The host runs the same gesture driver as `ExecutorHandle::pointer`, with the user's origin, and answers the
      `pointer` envelope.
  - **cancel:** the report with the modal cleared. No command is sent.
- **The attached `pointer` (P1, P2, P4).**
  - In attached mode, an agent's `pointer` call for a view shown in a window is routed to that window's input queue.
    The routing is a host message carrying the request id and the events.
  - The events are played through the same sample collection and overlay as the user's, with simulated time.
  - They are committed through `gesture_commit` with the agent's origin, and the envelope answers the MCP call.
  - The queue runs one gesture at a time, so P1 holds.
  - A user press during an agent gesture discards the agent's gesture with nothing committed, and its call answers
    the SI-135 refusal (P2).
  - While the user is mid-gesture, the agent's call waits up to `waitForUserMs` and then answers `ui.busy` (P4, the
    kernel rule of D-12).
  - A view not shown in any window keeps the headless path.
- **Back-end measurement in the product (SI-136).**
  - Each back end is measured on the rule set's three scenes for 2 s per cell, on an offscreen canvas, once after the
    first launch and after a GPU, driver or webview version change.
  - Until a result exists the canvas runs WebGL2, the mandatory path.
  - The selector is `desktop/src/canvas/backend.ts`. It applies step 1 of the rule set, and its table-driven Vitest
    cases are the rule table's own.
- **Overlay appearance** (selection outline and handles, marquee, gesture ink, cursor ghosts): SI-138.
- **Matrices.**
  - Every covered tool's gesture through the real window equals the headless `pointer` gesture and the direct command,
    in one table over all 18 tools of design note 3.
  - Every overlay kind is absent from `draw.drawing.render` and `render.frame.run` of the same state, in one table
    over the overlay kinds (06 §6.10).
  - Every view-dependent tolerance used by the window's gestures follows the announced zoom (already C4.2a's kernel
    rule; the window path joins the table).
- **Invariants added:**
  - F6 (D-7a, D-7c): a float closing mid-gesture, Escape, or a lost pointer capture leaves the document, the history and
    the selection untouched and the overlay empty.
  - F7 (D-7b): an engine restart mid-gesture discards the gesture, and the new generation reloads every resident
    drawing.

## 7. Amendment after review round 1: the selection overlay (C42B-R1-22; before the code)

02 §4.2 layer 9 starts with "selection outline and handles", and 02 §7.1.1 says "the selection shows a bounding box with
handles". The round-1 build drew only the gesture in progress. This section is the design; where it and §1 to §6
differ, it holds.

### 7.1 Where the selection geometry comes from

- **The kernel computes it**, in drawing space, as part of the `canvas_scene` answer (note 1 §9): a `selection` member,
  `null` when the current drawing has no selection, else:
  - `objects`: each selected object's bounds (`[minX, minY, maxX, maxY]`), from the same `selectables` walk and bounds
    that `draw.selection.get` and the Select tool's own hit test use (`draw_select`);
  - `bounds`: the selection's bounding box (`draw_select::selection_bounds`, the box `draw.selection.get` answers);
  - `pivot`: the transform pivot, the bounding box's centre (02 §13.5: `draw.transform.apply`'s default pivot; 02 §8
    line 828);
  - `drawingPivot`: the current drawing's pivot (`draw.pivot.set`; 02 §7.1.1 "the drawing pivot is shown");
  - `points`: the anchors of the point selection (`selectedPoints` of `draw.selection.get`), as drawing points.
- **Announced** by what already makes the canvas pull its scene: every engine event marks the scene dirty (§1), and
  `draw.selection.changed`, every document change and every undo or redo are events. Mapping changes need no new
  answer: the window maps the drawing-space geometry through the mapping it already follows (`shell.view`,
  `draw.viewMapping.get`), every frame the mapping changed.
- **No static redraw for a selection change.** The model keeps the selection apart from the static composition: a scene
  answer that differs only in its selection updates the overlay, not layers 1-8 (the cached static target is reused,
  06 §6.9 overlay buffering).

### 7.2 How the window draws it (layer 9, view space)

- Built in view pixels from the drawing-space geometry and the current mapping, so it stays 1 view pixel wide at every
  zoom and follows rotation (a rotated view shows the bounding box as a rotated rectangle):
  - each selected object's bounds: a 1-pixel outline in the accent colour (SI-138; an object's outline is its bounds
    box in C4.2, SI-141);
  - the selection's bounding box: a 1-pixel accent outline;
  - eight handles, four corners and four side midpoints: 8-pixel accent squares with a light centre (SI-138);
  - the drawing pivot: an accent ring of the pivot hit radius (6 view pixels, SI-131) with a cross;
  - selected anchors: 5-pixel accent squares.
- **When it shows** (02 §1.3): with the Select tool active, everything above. With Contour Editor or Perspective active,
  the objects' outlines and the selected anchors without the bounding-box handles (their own handles come with their
  gestures, SI-129). With any other tool the selection is kept and nothing of it is drawn ("switching to a creation tool
  keeps the selection but hides its handles").
- It is drawn under the gesture in progress (the drag's own feedback is on top).
- **Never in exported pixels** (06 §6.10): the overlay exists only in the window's renderer; `draw.drawing.render`,
  `render.frame.run` and the headless screenshot never see it. The attached `inspect.screenshot` shows it because it is
  the window's own capture (D-6), like the onion skin and the grid.
- The selection display options of `drawAid.selectionDisplay.*` (02 §10.7: outline on selection, box highlighting,
  selection always visible) are Camera View options; the Drawing view does not use them (C4.3).

### 7.3 The handles and what dragging each does (02 §7.1.1)

| Where the drag starts (view pixels) | Gesture | Command |
|---|---|---|
| within 6 px of the drawing pivot (SI-131 item 2) | move the drawing pivot | `draw.pivot.set {point}` (built) |
| within 6 px of a corner handle | scale both axes about the opposite corner; the constrain modifier keeps proportions | `draw.transform.apply {scale: {x, y}, pivot}` |
| within 6 px of a side handle | scale the one axis across that side about the opposite side | `draw.transform.apply {scale: {x, 1} or {1, y}, pivot}` |
| outside the bounding box, 6 to 18 px from a corner ("just outside a corner") | rotate about the bounding box's centre | `draw.transform.apply {rotate, pivot}` |
| inside the bounding box | move (alternate: a copy) | `draw.transform.apply {translate, copy}` (built) |
| elsewhere | click selects, drag marquee, alternate-drag lasso | `draw.selection.set` (built) |

- The tests are made in that order, so the pivot wins over a handle at the same place, and a handle over the box's
  inside. All radii are view pixels, the convention of SI-131 item 2: a handle is hit like the pivot, by distance from
  its centre, so one 6-pixel radius serves both and the 8-pixel square drawn lies inside the hit disc.
- **Scale factors** are measured in drawing space along the bounding box's axes: `s = (release - pivot) / (press -
  pivot)` per axis, from the press and release samples (the samples between only draw the overlay). With the constrain
  modifier, a corner uses the factor of larger magnitude on both axes. A factor whose magnitude is under 0.001 is
  refused by the gesture (`arg.invalid`, nothing committed): a degenerate scale would collapse the objects.
- **Rotation angle:** the signed angle, counter-clockwise in drawing space (02 §2: y up), from the press to the release
  about the box's centre, in degrees.
- **Skew:** 02 §7.1.1 defines no skew handle; `draw.transform.apply {skew}` stays a numeric command and no gesture
  produces it.
- **Cursor feedback** ("the pointer indicates the resize direction in 15° steps"): the window sets the platform's resize
  cursor of the nearest of its eight directions over a handle and a move cursor inside the box, from the same geometry;
  platforms offer no 15° cursor set (SI-141).
- **Undo labels:** the handle gestures produce `draw.transform.apply`, whose label 02 §13.5 gives as "Move/Scale/Rotate
  Object" (and "Copy Object"), while 02 §7.1.1 lists "Move Object", "Scale Object", "Rotate Object": the registry's
  label is kept and the conflict is recorded in SI-141.
- **Built in C4.2 through the kernel's gesture code** (`gesture.rs`, `select`): the pivot drag, move and copy (already
  built), corner and side scale, rotate. The hit test runs in the kernel, on the selection it computes, for the window
  path and the headless `pointer` alike (G2, D-4); the window's own geometry only draws the handles and picks the
  cursor.
- **Recorded in SI-129 with owner C4.3:** Perspective's envelope handles (02 §7.1.3), Reposition All Drawings (§7.1.4),
  the Contour Editor's anchors, handles and segment drags (§7.1.2), and the Text tool's frame.
- **No live preview in C4.2:** while a handle is dragged the overlay shows the drag; the objects move when the committed
  revision is drawn, like the Grabber (residual risk, owner C4.3).

### 7.4 Tests

- Kernel (`c42/canvas.rs`): the scene's `selection` for no selection, one object, several objects, a point selection,
  and after undo; its bounds equal `draw.selection.get`'s.
- Kernel (`c42/canvas.rs`): one case per row of §7.3's table (the command a drag starting there produces, with its
  parameters), the constrained corner, the degenerate scale, and the order at overlapping places.
- Front end (`selection.test.ts`): the overlay follows a selection change and a mapping change (zoom, pan, rotation),
  shows handles only for the Select tool and nothing for a creation tool; a scene answer differing only in its
  selection leaves the static version unchanged.
- Running application (`scripts/c42-gui-e2e.py`, both OSes): selecting an object makes the overlay appear, asserted
  through the canvas summary (the selection and its handle count) and through a real capture of the view in which the
  handle squares are present at the handle positions and absent after deselecting.
- Equivalence (D-4): one row per handle gesture built (corner scale, side scale, constrained corner, rotate, pivot) in
  the e2e table: the window path, the headless `pointer` and the direct command produce equal parameters, equal content
  and one undo step with the same label.
