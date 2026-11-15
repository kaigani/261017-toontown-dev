# M4 C4.2 design note 3: gestures, tool state and equivalence

Before the code (plan §4, ruling D-3: the Rust half in C4.2a, the webview half in C4.2b). Requirements: 02 §1.3, §5.1-5.2,
§6, §7.1-7.5, §7.7, §12.1, §12.3, §13.1 item 7, §13.3; 03 §8; 10 REV-06; ADR-0007 decision 1; ADR-0009 lines 70-72;
rulings D-4, D-7, D-9.

## 1. Tool state (kernel)

- `tool.active.set`, `tool.group.select`, `tool.active.get`, `tool.list`, `tool.options.*`, `tool.*.options.set`,
  `tool.settings.get`, `tool.pen.*`, `tool.placeBehind.set`, `tool.drawTopLayer.set`, `tool.snapToContour.set`,
  `tool.tablet.set`, `tool.symmetry.set` (exists). Scopes per 02 §12.3: pens and the current brush and eraser pens in
  the **project** (document content, undoable as pen commands, saved with the project); everything else per **user**
  in the preferences file through the settings rules of ADR-0021 decision 13 (version, shape, corrupt-file notice;
  D-7d). Tool selection and options are not undoable (02 §12.1).
- One active tool shared by the Drawing and Camera views (02 §1.3); the last-used tool per group (02 §5.1).

## 2. One input model, one gesture code (Rust)

- **Input source abstraction** (ADR-0009 lines 70-72; D-9): a sample is `{x, y}` in view pixels, `pressure` 0..1
  (default 1), `tiltX`, `tiltY` (recorded, unused by any tool, SI-125), pointer kind (`mouse`, `pen`, `penEraser`),
  buttons, modifiers and a timestamp in simulated milliseconds (the sum of `dt`). The webview (C4.2b) and the `pointer`
  tool both produce this; native tablet sources wait for ADR-0009's evidence.
- **A gesture** is press, samples as given (no resampling), release; or press and cancel. The view mapping is captured
  at the press and used for every sample (02 §13.3); a navigation event inside a gesture is refused.
- **`gesture::to_command(session, tool, gesture) -> Option<Invocation>`** in `crates/executor/src/gesture.rs` builds the
  equivalent command of 02 §13.3's table with every current setting explicit (current pen, colour, symmetry, Draw
  Behind, Draw on Top Layer, gap-closing mode, the view for tolerances): the same function serves the headless
  `pointer` path (C4.2a) and the webview (C4.2b, through IPC). The release invokes that command once.
- Tools covered in C4.2 (plan scope): Brush, Pencil, Rectangle, Ellipse, Line, Polyline, Select (click, marquee, lasso,
  drag-move, pivot drag), Eraser, Cutter, Scissor, Paint, Unpaint, Paint Unpainted, Dropper, Close Gap, Zoom, Grabber.
  The other tools of 02 §7 (Contour Editor, Perspective, Reposition All, Text, Edit Texture, the invisible Stroke tool)
  can be made active but their gestures answer `command.notImplemented` naming their owner (SI-129 records the owner
  question; interim owner M4 C4.3).

## 3. Equivalence (D-4)

For the same samples and settings, the gesture (webview or `pointer`) and the directly sent equivalent command produce
the same command parameters value for value, the same canonical saved content byte for byte, the same ids (the id
stream is deterministic from the same starting state) and one undo step with the same label: the label is the
command's own (`draw.stroke.add` → "Brush Tool"/"Pencil Tool", the shapes "Rectangle Tool" etc., set by the handlers
since M2), so a gesture that invokes exactly the equivalent command cannot differ. 02 §13.3 grants a tolerance (1e-6 du
for geometry and pressure); it is cited here and not used: the tests assert exact equality per tool.

## 4. Cancellation and failure (D-7)

- Escape, a lost pointer capture, a float closing (C4.2b) or an engine restart (generation change) mid-gesture discards
  the gesture before any command: the document, the undo history and the selection are untouched (D-7b, D-7c).
- A refused equivalent command (a locked drawing, no current drawing) is the gesture's result: nothing changes, the typed
  refusal is reported (the `pointer` envelope; the status line in the GUI).
- Tool and navigation changes during a gesture are refused (`ui.busy` for agents, 11 §6.4; ignored for the user's own
  keys until release).

## 5. Invariants (one builder test each)

- **G1** One gesture ends in exactly one command, or none when cancelled or refused before invoking.
- **G2** For every covered tool, the `pointer` path and the direct command give equal parameters, equal canonical saved
  bytes, equal ids and one undo step with equal labels (per-tool tests in `tests/c42.rs`; the webview path joins in
  C4.2b).
- **G3** The mapping used is the one at the press: a view change requested between press and release does not change
  the mapped samples (and is refused).
- **G4** One undo step per gesture, labelled as the command's.
- **G5** Samples are applied as given: the command's points are the mapped samples, in order, none added or dropped.
- **G6** A cancelled gesture leaves the document, the history and the selection byte-identical.
- **G7** Tool selection and options are not undoable; pens are, as document content.

## 6. Amendments after review round 1

- **One gesture, its commands.** Every tool's gesture produces one command, except two that 02 §13.3 itself gives as
  several: the Dropper (`colour.pick`, then `colour.current.set` with the picked colour; ruling D-11) and a Zoom-tool
  marquee (`view.zoom.set` to fit, then `view.pan.set` to centre the rectangle; 02 §7.5 line 658). G1 reads "exactly the
  gesture's equivalent commands, or none when cancelled or refused before invoking".
- **Zoom.** A Zoom-tool click sends table `steps` about the click (the review's permanent test); the wheel steps along
  the 02 §4.3 ladder (SI-122, D-10); a marquee fits its rectangle exactly: zoom = current zoom × min(view width /
  rectangle width, view height / rectangle height), clamped to 6.25 %-6400 %. The fitted value is not snapped to the
  ladder: the ladder applies to steps, not to fits.
- **Paint.** A drag with a gradient or texture colour sends `points` and `mapping {from, to}`; a click fits the default
  mapping; a solid colour's drag sends `path` (02 §7.3.1 lines 557-561).
- **Polyline.** A constrained handle drag turns the handle in 15° steps, its length kept; a constrained click on an
  anchor of the path being drawn removes it (02 §7.2.5).
- **Stroke** (the invisible-line tool) is built: the Brush gesture with `kind: "invisible"`.
- **`pointer` answers** every command's complete envelope (C42A-R1-22) and needs the `input` scope (C42A-R1-06).
- **Busy** (ruling D-12): while the user is mid-gesture, agent commands that write document or session state wait, then
  answer `ui.busy`.

## 7. Amendment for C4.3: scene tools and the Camera view (C4.3a; plan §3.4, rulings D-23, D-27)

**Scene tool state.** `anim.tool.set`/`get` (05 lines 1217-1218) hold the current scene tool (`select`, `transform`;
the others of 05 §5.0's table refuse `command.notImplemented` with owner M6), separate from the drawing tool (05 lines
29-31): choosing one never changes the other. UI state, not undoable (05 §17).

**Which gesture runs in the Camera view.** A `pointer` request whose `tool` is a scene tool id (`select` is shared:
SI-148 settles how a request names the scene Select rather than the drawing Select, before the code) runs the scene
tool; otherwise the drawing tool runs. A drawing tool's gesture in the Camera view is C4.2's gesture with its samples
mapped through the current element's inverse full scene transform (views note §6): the same command, parameters and
label as the equivalent gesture in the Drawing view (02 §7, line 61, line 413).

**Scene-tool gestures** (05 §3.5, §5.0-§5.2), each ending in exactly one command or none:

| Gesture (Camera view) | Select (05 §5.1) | Transform (05 §5.2) |
|---|---|---|
| click on element content (4 px tolerance, front-most, locked skipped; extend modifier toggles) | `edit.selection.set` | `edit.selection.set` |
| drag in empty space past 3 px (rubber band: every pickable element touched) | `edit.selection.set` | `edit.selection.set` |
| drag inside the selection box | `anim.transform.set {target: static, position, relative}` (parent space at the node's depth) | `anim.transform.set {target: key, frame: current, position, relative}` |
| drag a corner or side handle (constrain: a corner keeps proportions) | `{target: static, scale}` | `{target: key, scale}` |
| drag the rotation handle (constrain: 15° steps) | `{target: static, rotation}` | `{target: key, rotation}` |
| drag a skew handle | `{target: static, skew}` | `{target: key, skew}` |
| drag the pivot marker | (no pivot marker) | `{target: static, rotationPivot and scalePivot, or pivot}`: no key (05 line 418) |
| Escape during a drag | nothing | nothing |

A keyframing drag keys only the channels it changes (05 line 398; the result's `keyedChannels` is asserted). Arrow
keys nudge by 1 view pixel converted at the node's depth (10 times with the coarse modifier) through the same command
(the window sends them, C4.3b).

**Undo labels** (05 §17 line 1135: "name the operation and the object kind; wording is a UX-writing task"; ruling D-23
asks each test to state the spec's value, so the interim wording is recorded first, SI-144): one changed group gives
`Move`, `Rotate`, `Scale`, `Skew` or `Move Pivot`, several give `Transform`; then the node kind (`Element`, `Peg`,
`Camera`, `Skeleton`), plural for several nodes of one kind (`Elements`), `Nodes` when kinds differ: "Move Element",
"Rotate Peg", "Scale Elements", "Transform Nodes". `anim.transform.set` sent directly carries the same label for the
same parameters (C4.2's rule: the gesture's label is the command's); its earlier fixed label "Transform" is a declared
behaviour change. `edit.selection.set` is not undoable (05 §17).

**Invariants (`tests/c43.rs`; each table names the spec line and asserts the spec's value before comparing paths,
ruling D-23):**
- **G7** Every row of the table ends in the command it names with the parameters it names (target, channels keyed,
  static against keyed) and the SI-144 label, through `pointer` headless; Escape ends in none.
- **G8** For every row, the headless `pointer` path and the command sent directly give the same document, ids, one undo
  step and the same label (the window path joins in C4.3b's three-way).
- **G9** A keyframing drag keys only the channels it changed; Select never keys (05 lines 397-398).
- **G10** Each drawing tool's gesture in the Camera view produces the command, parameters (in drawing space) and 02 §7
  label of the same gesture in the Drawing view, as a full table over C4.2's tools.
- **G11** `anim.tool.set` never changes `tool.active.get`, and `tool.active.set` never changes `anim.tool.get`.

**Round 1 (C4.3a; `reviews/M4-C4.3a-r1-response.md`):**
- **The temporary Skew** (05 §5.0 line 394; C43A-R1-21; SI-151): the Transform tool with the alternate modifier held at
  the press runs the Skew tool for that gesture; the temporary-select modifier wins over it (line 393). A drag from the
  top or bottom side is `{target: key, skew}`, from the left or right side the vertical shear of 05 §5.4 decomposed by
  §2.5 (`rotation`, `skew`, `scale`, only the changed channels); any other drag commits nothing; a click selects. The
  stored scene tool stays Transform.
- **A side handle sends only its axis** (05 line 398; C43A-R1-22): a horizontal side `scale {x}`, a vertical side
  `scale {y}`, a corner both. `anim.transform.set` keys every channel it is given (A row 613: "key animated values"), so
  the gesture, not the command, omits the axis it did not change; the direct command keeps its meaning.
- **A move's delta is a vector** (ruling D-28's note): the view-pixel delta goes through the linear parts of the chain
  (view to output pixels, output pixels to camera space at the node's depth, camera to world, world to parent), never
  as the difference of two converted points, so a round drag gives a round delta.
- **A drawing tool on an element whose plane cannot be projected** (at or behind the near plane, or on the camera's
  plane; C43A-R1-03, SI-145): the press refuses the `pointer` call with `inspect.behindCamera` and nothing is created;
  the scene tools and the navigation tools, which work in view pixels, are not affected. The final hover's drawing
  point is null there.
- **`view` in a Camera-view gesture** (beside ruling D-16): D-16 states the view whenever the zoom is not exactly
  100 %, because at 100 % in the Drawing view a view pixel is a drawing unit. In the Camera view a view pixel is never a
  drawing unit (the camera projection and the element's transform are part of the mapping), so a Camera-view gesture
  states `view` at every zoom, 100 % included, and its view-pixel tolerances are taken through that view.
- **Start-up** (SI-152): a new session's scene tool is Select and the Camera view runs the scene tools until a
  `tool.active.set`.
