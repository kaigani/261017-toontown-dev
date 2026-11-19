# M4 C4.4b design note 3: Contour Editor

Status: before code (C4.4b). Plan `reviews/M4-C4.4-plan.md` §5 intro, §5.1, §5.5, §6 item 3, §7; §9 (ruling D-40,
answer 7); `reviews/M4-C4.3-plan.md` §15 rulings D-23, D-32, D-33, D-39, D-45. Covers point selection and its overlay,
the hit targets, Snap to Contour (02 §8.9), the live preview, prechecks and refusals, and the tests. Builds on the
gestures note (`reviews/M4-design-gestures.md`), the pointer note, the preview note (SI-154) and SI-141.

## 1. What exists today (read from the code)

- **Driver.** `crates/executor/src/gesture.rs`: `COVERED` lists 18 tools without `contourEditor`, so its gestures
  answer `command.notImplemented` through `not_built` (SI-129). Modifiers are taken at the press (`Stroke::mods`,
  `Mods::of`: Shift constrain, Alt alternate, Ctrl or Meta temporary-select); a stroke within `CLICK_SLOP_PX` (3 view
  px) is a click. `effective_tool`'s temporary-select list does not contain `contourEditor`, so Ctrl/Meta reach the
  tool as its own modifier (02 line 452). `to_commands` already returns several commands for one gesture (the Zoom
  marquee).
- **Commands** (all five registered, `crates/executor/src/commands/draw_transform.rs`; inventory rows in
  `crates/registry/src/inventory.rs` match 02 lines 1337-1341): `draw.point.add {strokeId, at?, segment?, t?}`,
  `draw.point.move {points?, delta, snap?}`, `draw.point.set {strokeId, anchor, position?, inHandle?, outHandle?,
  handleMode?: symmetric|broken|reset}`, `draw.point.delete {points?}`,
  `draw.segment.move {strokeId, segment, t, delta}`; each also takes `drawingId` (02 §13.1 item 4) and carries the
  undo label "Contour Editing". Handles are offsets from the anchor (`addressing.rs` declares them vectors).
  `draw.pointSelection.set {drawingId?, points?, rect?, mode}` (`draw_select.rs`) is kind `ui`, makes no step and
  emits `draw.selection.changed` with `selectedPoints`.
- **Point selection** lives in the kernel: `AppContext::points` (`crates/executor/src/app.rs`), keys
  `<drawingId>/<strokeId>` with an anchor index, indexed by `point_drawings` for prechecks (ruling O-24).
- **Window.** The scene answer's `selection.points` gives the selected anchors' positions only, and
  `docs/canvas-format.md` §11.5 R7 makes `selection` null in the Camera view. `selectionOverlay`
  (`desktop/src/canvas/selection.ts`) draws, for `contourEditor`, object bounds and selected anchors. No anchor
  handles, unselected anchors or stroke contours reach the window.
- **Not as the plan assumes.** The plan's `handleMode: smooth` and `corner` are not values of the spec's enum
  (`symmetric|broken|reset`). The kernel's `reset` removes both handles (a corner), while 02 line 453 asks for "a
  flat/smooth state". `draw.point.move`'s `snap` defaults to `false`, not to the Snap to Contour setting (02 line 1338),
  and its doc comment says "8 du" where the code converts 8 view pixels through `view` (`tolerance_du`). The snap is
  computed per moved anchor inside the command, so no single "snapped delta" exists for a multi-point move (§5). The
  prechecks do not check that named anchors, segments or `t` are valid, which the handlers refuse (§7). The point
  selection is not renumbered when `draw.point.add` inserts or `draw.point.delete` removes anchors: `repair_checked`
  keeps an index while it is in range, so after a deletion the selection names the anchors that took the deleted
  ones' places. The plan's "rows of the inventory (12, 6, 7, 5, 6, 3 rows)" matches no table I found (open question 3).

## 2. The gestures in the driver (02 lines 447-463, §13.3 line 1226)

New module `crates/executor/src/contour_gesture.rs` (as `scene_gesture.rs`): `commands(cx, g)`, called from
`to_command`/`to_commands` for `contourEditor`, which joins `COVERED`. The press classifies the target (§4) and the
modifiers held at the press decide the row; moves feed only the window's preview; the release builds the command.
Every current setting is explicit (02 §13.3): `drawingId` (the current drawing), local stroke ids, `points` as a list,
`snap` as the setting. A `view` member is stated whenever the view's pixels per du is not exactly 1 (ruling D-16, as
the Select click). `Δ` is `pn.d - p0.d` through the press mapping. Constrain is Shift, alternate is Alt (Option),
temporary-select is Ctrl (Command on macOS) (02 line 32).

| Press target and modifiers | Pointer phases | Command and params | Step |
|---|---|---|---|
| anchor, click | press, release ≤ 3 px | `draw.pointSelection.set {drawingId, points:[{strokeId, anchor}], mode: replace}` (constrain: `add`, 02 lines 448-449) | none |
| nothing hit or a stroke body, click | as above | `draw.selection.set {by: point, point, mode, view?}` (the Select click; constrain `add`), then without constrain `draw.pointSelection.set {drawingId, points: [], mode: replace}` (SI-184) | none |
| nothing hit, drag | press, moves, release | `draw.pointSelection.set {drawingId, rect: rect(p0.d, pn.d), mode}` (constrain `add`); alternate is ignored | none |
| anchor, drag | as above | `draw.point.move {drawingId, points, delta: Δ, snap, view?}`; `points` is the point selection when the anchor is in it, else that anchor alone | "Contour Editing" |
| segment of a shown stroke, drag | as above | `draw.segment.move {drawingId, strokeId, segment, t, delta: Δ}`, `t` the press's nearest parameter (§4) | "Contour Editing" |
| temporary-select, click on a segment of any stroke | press, release | `draw.point.add {drawingId, strokeId, segment, t}` (02 line 452); on nothing: no command | "Contour Editing" |
| alternate, click on an anchor | press, release | `draw.point.set {drawingId, strokeId, anchor, handleMode: reset}` (02 line 453; SI-186) | "Contour Editing" |
| handle, drag | as above | `draw.point.set {drawingId, strokeId, anchor, inHandle or outHandle: h + Δ, handleMode: symmetric}` (SI-186) | "Contour Editing" |
| alternate, handle, drag | as above | the same with `handleMode: broken` (02 line 454) | "Contour Editing" |
| alternate, anchor, drag | as above | `draw.point.set {drawingId, strokeId, anchor, inHandle: in + Δ, outHandle: out + Δ, handleMode: broken}`, the anchor unmoved (D-40 answer 7; SI-185) | "Contour Editing" |
| alternate, segment of a shown stroke, drag | as above | `draw.point.move {drawingId, points: the whole point selection, delta: Δ, snap, view?}` (02 line 456) | "Contour Editing" |
| key Delete or Backspace | key | `draw.point.delete {drawingId, points: the point selection}` (02 line 457; SI-184) | "Contour Editing" |

- **Undo** (02 §12.1 line 1064, line 463): each row with a step makes exactly one step labelled "Contour Editing", the
  commands' own label, so no `acting_label` override is needed. Selection rows make none (kind `ui`, 02 line 1071).
- **G1 amendment.** A click off every target sends two `ui` commands (object selection, then the point selection
  cleared); with the Zoom marquee it is the second gesture with more than one command. Both make no step; if the first
  is refused the second is not sent (`finish` stops at a refusal).
- **Delete routing.** The key is resolved in the kernel (`resolve_in`, `crates/executor/src/host.rs`), the one dispatch
  the window (`dispatch_key`) and the MCP `key` tool share. In the drawing mode with `contourEditor` active, a sequence
  bound to `edit.delete` resolves to `draw.point.delete` instead; with any other tool it stays `edit.delete` (02 §7.1.1
  "Delete removes the selection"). The keymap file is unchanged.
- The temporary-pan key and the pen's eraser end keep their overrides (`effective_tool`).

## 3. Point selection model

- **Where.** The kernel's `AppContext::points`, per session, per drawing (key prefix), not undoable, shared by every
  view. The window holds no selection state; it draws what the scene answer says.
- **Shown strokes.** A stroke shows its anchors when it is in the drawing's object selection (a click selects it,
  02 line 448) or holds a selected anchor. One kernel function, `contour::shown(cx, drawing)`, serves the driver's hit
  test and the scene answer, so both see the same strokes.
- **Cleared** by a click off every target without constrain (§2), by `draw.pointSelection.set {points: []}` from
  anyone, and per entry when its anchor goes. Switching tools or views clears nothing (01 §6.4, as `app.rs` documents).
- **Kept consistent (kernel fix).** `draw.point.add` shifts the selected indices after the inserted anchor of that
  stroke by one; `draw.point.delete` removes the deleted anchors from the selection and shifts the rest down; a removed
  stroke drops its entries (repair already does). Kernel test: select anchors 0 and 1 of a four-anchor stroke, Delete,
  the selection is empty.
- **Agent changes reflected.** `draw.pointSelection.set` emits `draw.selection.changed`; every engine event marks the
  canvas scene dirty (`CanvasView.tsx`'s `onEvent` ends with `model.markDirty()`), the next answer carries the new
  `contours`, `selectionVersion` bumps and only the overlay is rebuilt (frame-loop note §7.1). Vitest: the overlay
  changes after a kernel event alone, with no gesture.

## 4. Overlay and hit targets (SI-184)

- **Scene answer, form 1.2** (`docs/canvas-format.md` §11 amended before the code, ruling D-24): a member `contours`,
  present in both view types while `contourEditor` is active, else absent:
  `{drawingId, locked, strokes: [{strokeId, closed, partner, anchors: [[x, y, inX, inY, outX, outY]], selected: [int]}]}`
  in drawing units. It holds every shown stroke, plus each same-layer stroke with an anchor at a shown anchor's exact
  coordinate (`partner: true`: not drawn, used by the preview because `draw.point.move` moves shared junction anchors
  together). R7 keeps `selection` null in the
  Camera view; `contours` is the Contour Editor's overlay in both. Built by `crates/executor/src/canvas.rs`.
- **Drawn** in view pixels through the drawing mapping: `drawingToView` in the Drawing view, the mapping from
  `draw.viewMapping.get` in the Camera view (camera-view note §6, D-32 sizing). Each shown stroke's contour is a 1 px
  accent curve (replacing its bounds box for this tool); anchors are 5 px squares, hollow (white fill, accent border)
  or, when selected, filled accent; the in and out handles of selected anchors are 1 px lines ending in 4 px discs
  (zero-length handles are not drawn). A locked drawing draws the same in grey. Colours are SI-138's.
- **Radii in view pixels:** anchor 6 (SI-141's handle radius), handle 5, segment 4. Hit candidates: anchors of shown
  strokes; handles of selected anchors; segments of shown strokes for drags and of every stroke for the
  temporary-select click. An invisible stroke is a candidate only while Show Strokes is on (`drawAid.strokes.set`;
  02 line 144: otherwise it is not drawn).
- **Order:** the nearest anchor or handle within its radius wins, an anchor on a tie; then the nearest segment within
  4 px; ties between strokes go to the front-most. A segment's distance is measured in view pixels on the segment
  mapped through the drawing mapping (an affine map keeps the Bézier parameter, so `t` is the drawing-space `t`),
  searched as `nearest_on` does (64 samples, 40 bisections). A segment press nearer an anchor than 6 px is an anchor
  press, so `t` never reaches 0 or 1 for `draw.segment.move`. A press on none of these is a click (the Select click)
  or a marquee.
- **One rule, two ports.** The kernel's driver decides the command; the window's `contourHitAt`
  (`desktop/src/canvas/contour.ts`, new) repeats the rule for the cursor and the preview only. The kernel test writes
  `desktop/src/canvas/testdata/contour-hits.json` (presses against a fixture at zoom 0.5, 1 and 2, rotated, both views;
  `TT_UPDATE_CANVAS_CORPUS=1` rewrites it, as `drawing.ttrd`) and Vitest reads it: same target, same `t` within 1e-9.
- **Cursor:** `move` over an anchor or a segment, `crosshair` over a handle, `copy` with temporary-select over a
  segment, `not-allowed` on a locked drawing, else the default.
- `inspect_ui`'s canvas summary reports the overlay: strokes, anchors, selected anchors, handles.

## 5. Snap to Contour (02 §8.9 lines 798-799; SI-187)

- The driver sends the raw `Δ` with `snap` set to the persistent setting (`tool.snapToContour.set`) and `view` when
  needed; the command snaps (02 line 1338 gives `draw.point.move` the `snap` parameter, so the agent's equivalent call
  is the same call). The plan's "the snapped delta" is not used: the kernel snaps each moved anchor independently, which
  no single delta can carry.
- Radius 8 view pixels through the given view's zoom (`tolerance_du`, 02 §13.1 item 5 lines 1151-1155), in either view
  type. Candidates as SI-070 decided: the other strokes of the moved stroke's own layer, their whole contour (anchors
  and segment interiors), nearest wins, ties to the lower stroke id, segment and parameter; strokes with a moved anchor
  are not candidates.
- Only `draw.point.move` snaps (the anchor and alternate-segment drags). Segment and handle drags do not move anchors.
- Kernel fix: `snap` defaults to the setting when omitted (02 line 1338), and the doc comment says view pixels.

## 6. The live preview (SI-154 extended)

- **What moves.** At the press the window classifies the target with `contourHitAt`; each frame it applies the row's
  rule to a copy of the affected anchors with `Δ` mapped through the press mapping's inverse, as the kernel will:
  `point.move` translates the moved anchors and their `partner` anchors at the same coordinate; a handle drag sets the
  handle (and mirrors it for `symmetric`); the alternate anchor drag adds `Δ` to both handles; `segment.move` adds
  `w = Δ / (3t(1 - t))` to the segment's out- and in-handles (the kernel's formula). Clicks have no preview.
- **What is drawn.** Only the affected segments (those touching a changed anchor), as 1 px accent curves flattened to
  0.25 view px, with the moved anchors and handles; the committed drawing stays beneath. Thickness, brush fills and
  zones change only at the commit (SI-154: zones are re-derived by the kernel).
- **Exact** for anchors, handles and the curves through them (an affine mapping maps Béziers exactly). With Snap to
  Contour on and a candidate within the radius it is approximate, off by at most the radius, until the committed frame
  replaces it (SI-187); the window has no candidates' geometry and asks the kernel nothing during a drag (PV1).
- **Lifecycle** as Select's: `gestureLifecycle.ts` tags the preview by gesture, engine generation and view (PV5, ruling
  D-53); a step's preview goes on the frame that draws the committed revision; a refusal clears it at once.
- **Cost:** a frame recomputes the affected segments only, never the stroke or the drawing.

## 7. Prechecks and refusals

- **Existing:** `precheck_point_move` and `precheck_point_delete` (`crates/executor/src/commands/draw.rs`): target
  (`drawing.noTarget`), lock (`drawing.locked`), and non-empty `points` or anchors selected in that drawing
  (`drawing.nothingToDo`). `precheck_contour` (add, set, segment move): the stroke's drawing and its lock.
  `draw.pointSelection.set`: `precheck_drawing` (target only).
- **Made equal to the handlers** (the O-24 rule, "prechecks = handlers"): the point prechecks also refuse a named anchor
  that does not exist (`drawing.objectNotFound`), `precheck_contour` a missing stroke or anchor, a `segment` out of
  range and a `t` not strictly inside (0, 1) (`arg.invalid`), as the handlers do before their work.
- **In the window:** on a locked drawing the overlay is grey, the cursor `not-allowed` and no preview is drawn; the
  release still sends the gesture, so the refusal is the kernel's. An alternate segment drag with no selected anchor
  draws no preview and is refused `drawing.nothingToDo`. Delete with nothing selected is consumed silently
  (`command.disabled`, C41-R2-09). A refusal is the gesture's answer (`lastGesture` in the canvas summary) and its
  message is shown in the status line (gestures note §4); I found no status-line display of gesture refusals in
  `desktop/src`, so C4.4b adds it for every tool.

## 8. Tests

- **Driver rows** (`crates/executor/tests/c44b/contour.rs`, new; `COVERED` gains `contourEditor`): every row of §2 in
  both views with its spec line, params and step count; the hit order (anchor against handle ties, 4 px against 4.01 px
  for segments, zoom 0.5, 1, 2, a rotated view); unselected against selected anchor drags; snap on and off; the
  temporary-select click on nothing (no command); Delete with Contour Editor and with Select (`edit.delete`
  unchanged); a locked drawing (refused, document and history unchanged); the renumbering of §3.
- **D-23 equivalence:** for each row, headless `pointer {tool: "contourEditor"}` against the directly sent command on
  the same start: equal params, canonical saved bytes and ids, one step labelled "Contour Editing" (02 line 463) or
  none for selection rows, each row naming its spec line. SI-185's and SI-186's rows say they state interim mappings.
- **Three-way** (`scripts/c44b-gesture-equivalence.py`, the byte-gate job): OS input through the real window, the
  headless replay and the direct commands, in the Drawing view and the Camera view; exact where the replay takes the
  window's view size and transform, else 1e-9 relative with the measured difference recorded (D-32).
- **Window (Vitest):** `contour.test.ts`: the hit corpus; overlay counts and the grey locked form; the preview against
  values derived in the test for each drag row (PV4); no command before release and none after Escape (PV1); preview
  lifecycle and view ownership (PV5); an agent change by event alone. **e2e:** the last preview frame against the
  committed state within 1 view px at the moved anchors and handles (PV2), both back ends, both views, snap off.
- **Availability and D-39:** a matrix test (six commands × unlocked, locked, no current drawing, empty selection, stale
  anchor) asserting availability equals execution, failing if a precheck is dropped. D-39 rows: the Contour Editor tool
  button, Delete in the Contour Editor (disabled with nothing selected), Snap to Contour clicked twice then a drag.
- **Parity:** `parity.json` gesture rows `{view: drawing|camera, tool: contourEditor, commands: [draw.selection.set,
  draw.pointSelection.set, draw.point.move, draw.segment.move, draw.point.add, draw.point.set, draw.point.delete]}`; the
  six `deferred.json` entries owned by M4 C4.4 (02 §7.1.2) leave it; the manifest is regenerated.

## 9. Budgets (plan §5.5; D-33, D-45)

| Quantity | Bound | Measured |
|---|---|---|
| Drag feedback | ≤ 16.7 ms p95 | `CanvasMetrics` input-to-ink (`metrics.ts`: oldest undrawn sample to the frame drawing the preview), `--canvas-metrics`, 30 OS drags per back end, release build |
| Release-to-committed | ≤ 50 ms p95 sample drawing; ≤ 150 ms on the 50,000-point drawing | `CanvasMetrics` release-to-committed in the e2e, 30 drags; the large case moves an anchor of a stroke on its own layer (open question 2) |
| Work per frame during drags | ≤ 8 ms p95 | the measure script on frames drawn during the gesture only (C43B-R1S-08); an incomplete measurement fails |

These are workstation receipts, debug and release, 30 samples, regenerated at each handoff touching the code (D-45).
No CI test asserts wall-clock time. CI asserts the shape: Vitest counts recomputed segments (a one-anchor drag on a
50,000-anchor stroke recomputes 2); a kernel test compares a point move in the 50,000-point drawing with one in the
sample drawing as a ratio, with a ceiling that only catches a gross regression.

## 10. Spec issues (proposed; to be entered in `plan/spec-issues.md` with the code)

- **SI-184 — Contour Editor point selection, hit targets and Delete (interim).** *Spec:* 02 lines 448-449 say "click a
  stroke to select it and show its anchors; click or marquee to select points" with no radii, order or command for
  selecting a stroke; `draw.pointSelection.set` (line 1331) cannot name a stroke without anchors; line 457's Delete
  has no key binding distinct from `edit.delete`. *Decision:* §3 and §4: shown strokes are the object selection plus
  strokes holding selected anchors; radii anchor 6, handle 5, segment 4 view px, nearest wins, anchor on a tie,
  front-most between strokes; handles of selected anchors only; invisible strokes only with Show Strokes; a click off
  every target is the Select click and clears the point selection (two `ui` commands); the marquee is `rect` in drawing
  units over every stroke; constrain is `add`; Delete and Backspace resolve to `draw.point.delete` while the Contour
  Editor is active. *Why:* SI-141's radius convention; the object selection already means "selected stroke" for the
  other selection tools (02 §1.3); one key dispatch serves the window and agents.
- **SI-185 — alternate + drag a point (interim, D-40 answer 7).** *Spec:* line 455 "changes the shape of the two
  segments connected to it", no command or rule. *Decision:* `draw.point.set` with both handles moved by the drag
  delta, `handleMode: broken`, the anchor unmoved, so the two adjacent control points follow the pointer. *Why:* D-40's
  interim; the result is exact to preview and stated in one call an agent can repeat.
- **SI-186 — handle drags and what `reset` does (interim).** *Spec:* line 454 gives only the alternate handle drag;
  line 453 says reset "minimises the handle angle to a flat/smooth state"; the kernel's `reset` zeroes both handles.
  *Decision:* a plain handle drag sends `handleMode: symmetric` (the opposite handle mirrors), the alternate one
  `broken`; `reset` makes the handles collinear: direction `out - in` normalised, each keeping its length; with both
  zero, the direction of next anchor minus previous anchor and lengths one third of the adjacent chords (one neighbour
  at an open end). The builder test `crates/executor/tests/c23_transform.rs` (the `reset` assertion near line 272) is
  updated with the change. Line 456's "entire selection" is the point selection. *Why:* "independently" in line 454
  implies the plain drag is dependent; "smooth" excludes a corner. *Question:* the reading of "flat/smooth".
- **SI-187 — Snap to Contour in the gesture and the preview (interim).** *Spec:* line 461 "a dragged point snaps",
  line 798 "points moved ... snap", line 1338 `snap?: bool = Snap to Contour setting`; nothing on multi-point drags or
  on feedback. *Decision:* the gesture sends the raw delta with `snap` explicit and the command snaps every moved
  anchor (SI-070's candidates); the omitted `snap` defaults to the setting; the preview is approximate (within the
  radius) while a candidate is in range. *Why:* the command owns the snap, so the gesture and the agent's call are one
  call; a per-sample kernel query would break ADR-0007 decision 1. *Question:* is a kernel-computed snap preview wanted?

## 11. Open questions for the orchestrator

1. **Closing a shape with one stroke.** 02 line 799 says snapping makes "shapes close", but SI-070 (decided) excludes
   every stroke with a moved anchor, so an open stroke's end cannot snap to its own start. Should the moved strokes'
   unmoved anchors become candidates? This note keeps SI-070.
2. **The 150 ms bound in a large layer.** The C4.2b handoff §10 records 603 ms p50 for an edit that re-nodes the
   50,000-point drawing's single layer (owner M11 C11.1). This note measures the bound on a stroke in its own layer and
   reports the large-layer case without asserting it. Confirm.
3. **Plan §5.1's "12, 6, 7, 5, 6, 3 rows".** I found no table with these counts; §7 and §8 state the precheck rows
   instead. Which table was meant?
4. **SI-186 changes an M2 command visible to agents** (`draw.point.set {handleMode: reset}`). Confirm before the code.
