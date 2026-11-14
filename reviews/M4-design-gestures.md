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
