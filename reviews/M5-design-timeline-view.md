# M5 C5.1b design note: the Timeline view

Before the code (plan `reviews/M5-C5.1-plan.md` §6-§10, §12; rulings E-1 to E-16). Kernel half: 
`reviews/M5-design-timeline-kernel.md`. Spec: 04 §11 lines 589-795 (the view), §2.6, §2.9, §3, §4, §5, §7.1, §8, §9,
§10 (the row commands), §17.1 (agent equivalents); 09 §2.2-§2.8 (Play controls); 01 §5.7 (toolbar), §6.2-§6.6.

## 1. Anatomy and where each part's state lives

| Part (04 §11.2 line) | DOM or canvas | Reads | Sends |
|---|---|---|---|
| Layer-list toolbar (604) | DOM buttons and one list | `prefs.list` slice, availability | `element.create`, `element.parentPeg.create`, `element.solo.set`, `element.visibility.set {apply: "all"}`, the context-menu button |
| Layer list (605) | DOM, virtualised: only the rows in view plus 10 above and below | `timeline.rows.list`, `element.get` per visible row | disclosure `element.collapsed.set`, visibility control `element.visibility.set`, name click and drag through the timing-gesture path, double click: in-place rename `element.rename` |
| Frame ruler (606) | canvas | rows answer's `currentFrame`, `playbackRange`; `prefs` time display | press/drag/release through the timing-gesture path (`timeline.currentFrame.set`, `timeline.playbackRange.set`) |
| Tracks (609) | canvas | `timeline.rows.list {from, to}` (bars, keyframes, child markers); `xsheet.exposures.get {elementIds, from, to}` for the visible cell-based rows; `cell.selection.get` | press/drag/release through the timing-gesture path |
| Zoom control (610) | DOM slider of the ladder's steps | `timeline.view.set {}` at mount, `shell.timelineView` | `timeline.view.set {zoomLevel}`; the wheel over it steps |
| Scroll bars (611) | DOM | the view state; vertical position window-local (SI-206) | `timeline.view.set {firstFrame}` (horizontal) |
| Splitter (612) | DOM | workspace panel geometry | the existing workspace geometry path |

The panel mounts, subscribes to `app://events`, then reads (D-85, D-86). Every read passes the panel's read gate per
source (`desktop/src/state/readGate.ts`): an older read answering after a newer one is dropped. Re-reads follow
`element.state.changed`, `cell.exposures.changed`, `cell.selection.changed`, `anim.changed`, `doc.undone`,
`doc.redone`, `timeline.currentFrame.changed`, `timeline.playbackRange.changed`, `prefs.changed`, `shell.timelineView`
and `shell.playbackFrame`; a scroll that leaves the read margin reads the new window.

## 2. Rendering

- The tracks canvas draws only rows in view × frames in view, at device-pixel scale; the drawing function is pure
  (`draw(ctx, model, geometry)`), so Vitest drives it with a recording 2-D context and counts operations (the CI shape
  of the plan's §6.2).
- Per row type (04 §11.3 lines 616-625): drawing/image/annotation rows: the bar of the exposed extent in the element's
  colour, a drawing symbol at each exposure start in the lower half (vector and bitmap differ), held frames as a line, a
  repetition mark where a drawing exposed earlier returns, blanks empty; pegs, cameras, effects, skeletons, bones: the
  bar from start for its duration, the looped part lighter after the first cycle with the loop handle at its end (upper
  half), keyframe glyphs per channel; group rows (effect parameters) tinted with their category colour.
- **Keyframe glyphs (04 line 627):** position a circle, scale a square, rotation a triangle, skew a diamond, drawn in
  the upper half of a cell-based row and centred on a bar row; several channels on one frame draw one glyph split in
  quadrants; a line to the next keyframe when `segmentAfter` is `interpolated`, none when constant; a child's keyframe
  frame a small tick on the parent (and on a collapsed parent's mini-bar).
- **Row states (04 line 633):** selected (accent fill), current (outline), child-selected (lighter accent on the
  parent), hidden or hidden by parent (dimmed, hatched for hidden by parent), locked (lock glyph, not dimmed), collapsed
  (the mini-bar), drag preview (ghost), has note (note glyph in the layer list).
- Colours: the element's `effectiveMarkerColour` (bar), `backgroundColour` (behind the bar), the Timeline background
  preference; every colour of the canvas is read, none hard-coded except theme-neutral greys.

## 3. Gestures: the window's path

1. **Press:** the window computes nothing about the document. It sends `timing_gesture {phase: "press", layout,
   sample}` (a new host IPC command calling `ExecutorHandle::timing_gesture`); the answer is the grab id and what is
   under the press (`target`). The pointer is captured.
2. **Preview (local):** from the target and the pointer's travel the window draws the preview: a ghost bar moved or
   stretched, the loop extent, a ghost keyframe, ghost cells at the destination, an insertion line or an attach
   highlight between or on rows, the selection rectangle, the playhead under a ruler drag. The arithmetic (frame under
   x, row under y, the drop zones) is `desktop/src/timeline/geometry.ts`, checked against the kernel by the shared case
   table `crates/executor/src/timeline-geometry-cases.json` (one Rust test, one Vitest test). A ruler drag also sends
   `timeline.currentFrame.set` for the newest frame while no call is outstanding: frames crossed meanwhile are
   dropped, never queued (E-11).
3. **Release:** `timing_gesture {phase: "release", grabId, samples}`; the host answers the plan and the envelopes. A
   refusal or a cancellation (`timeline.targetChanged`) is shown on the status line by the one refusal path (D-87);
   the preview clears at once (C4.3b rule: a refusal clears it).
4. **Abort:** Escape, a lost capture, a blur of the window, an engine generation change, the panel unmounting:
   `timing_gesture {phase: "abort", grabId}` and the preview is dropped; nothing is sent (01 line 495).
5. **Preview until the committed frame:** the preview holds until the panel has drawn a read at the document revision
   the commit answered with (the M4 preview rule, D-53), then clears.

**E-10 (a change under the drag).** The kernel decides at the release (kernel note §5.2): the plan against the
press-time snapshot must equal the plan against the present state, else nothing is committed and the status line says
`timeline.targetChanged`. Deterministic tests: the kernel's (`c51_timeline.rs::e10_…`) and the window's: Vitest with a
host double that answers the release with the cancellation (the preview clears, the status line shows the message);
an end-to-end row with the existing commit-delay hook (`TT_TEST_COMMIT_DELAY_MS`, dev-only `test-hooks`): an agent
moves the peg between the person's press and release of a bar drag, the release reports the cancellation and the
document holds the agent's change only. Bar, row, keyframe and cell drags each have one such row.

## 4. Gestures and their commands (D-23 rows; spec lines)

As the plan's §6.3 table, built in the kernel (`timing_gesture.rs`), with these window details:

| Gesture | Where (hit test, `timeline-layout.json`) | Command |
|---|---|---|
| click / drag the ruler | anywhere in the ruler but a free range's markers (±5 px) | `timeline.currentFrame.set` |
| drag a range marker | ±5 px of a free range's start or end | `timeline.playbackRange.set {start}` / `{end}` |
| click a row name, extend, toggle | the layer list (not its controls) | `element.select` + `element.current.set`, or `element.select {mode: "toggle"}` |
| drag a row | the layer list; drop zones: upper and lower quarter between, middle half onto a parent-capable row | `element.order.set`, `element.parent.set {index}`, or one batch (to the root from deeper levels) |
| drag a bar body | inside a bar; every selected bar moves together | `element.timing.set {startDelta}` |
| drag a bar end / start | ±5 px of the bar's ends (end: lower half of a loopable bar) | `element.timing.set {duration, anchor, keyframes: "scale"}` |
| drag the loop handle | upper half at a loopable bar's end | `element.timing.set {loops}` |
| drag a keyframe | ±5 px of a glyph (upper half of a cell-based row; bar rows full height), not inside the cell selection | `anim.key.move` |
| click a cell | elsewhere in a track | `cell.select` + `xsheet.currentCell.set` |
| drag across cells | from a cell outside the selection | `cell.select {rectangle}` + `xsheet.currentCell.set` |
| drag the cell selection | from a cell inside it; modifiers copy / link (SI-211) | `cell.range.move {mode}` |

Auto-scroll: while dragging within 24 px of the tracks' edge, the window scrolls one cell per animation frame (a
`timeline.view.set {firstFrame}` per step, E-11's newest-only rule) and keeps the samples in surface coordinates
corrected for the scroll at each sample (the release's layout is the press's; samples carry the travelled offset).

## 5. Context menus (04 §11.9 lines 697-708)

The shared `ContextMenu` (D-60, D-66), opened by the secondary click, Shift+F10, the context-menu key and the
toolbar's context control. Each item: the command reference it runs (`data-tt-command`, `data-tt-params`), the
kernel's availability (O-10), the check state for toggles (04 line 708), and the reason on a disabled item. Items owned
elsewhere are present and disabled with their owner on the control (E-2, E-9):

- **Row:** Add *type* (submenu of every type), Add Drawing, Add Peg, Add Parent Peg; Import… (09: disabled, owner M10
  C10.2 / M8 C8.3); Cut, Copy, Paste, Delete (`edit.*`, context `timeline`); Arrange › four (`element.order.move`);
  Collapse / Expand; Select › Parent, Children, Child, Previous / Next Sibling, Skeleton Root; Attach to Next Object,
  Detach from Parent; Show/Hide, Show All, Hide All Others, Show All Others, Hide All; Marker Colour…, Background
  Colour…, Default Colour; Show Effect Parameters (on a clipping effect), Show All / Hide All Effect Parameters
  (`timeline.effectParameterRows.set`, E-9); Set as Annotation / Set as Drawing Element; Show / Hide All Annotation
  Elements (`element.visibility.set {types: ["annotation"]}`); Lock, Unlock, Lock All Others, Lock All, Unlock All;
  Change Start Frame…, Change Duration…, Change Loops…; Clone, Duplicate, Delete, Rename…; Element Note…; Peg Reset,
  Add Default Keyframe (disabled, owner M6 C6.2); Remove All Keyframes (`anim.key.remove {all: true}`), Toggle Constant
  Z (`anim.path.lockDepth.set`); Edit Sound, Sound Waveforms (disabled, owner M8 C8.1).
- **Cells:** Cut, Copy, Paste, Delete; Extend Children Exposure… (peg rows); Add / Remove Keyframe (`anim.key.add`,
  `anim.key.remove`), Set Constant / Non-Constant Segment (`anim.segment.set`), Uniform Left / Right
  (`anim.segment.uniform.apply`) on bar rows; the 04 §11.9 cell commands (Add / Remove Exposure, Set Exposure to 1 / 2 /
  3 / N, Extend Exposure, Insert Cells, Insert Blank Cell, Delete Blank Cell, Create Cycle, Columnate, Duplicate
  Drawing) disabled with owner M5 C5.2 (E-2); Edit / Split / Fit Sound disabled, owner M8.
- **Ruler:** Set Current Frame…; Playback Range › Automatic Fit / Free (`timeline.playbackRange.set {mode}`); Set
  Playback Start Frame…, End Frame…; Sound Waveforms, Sound Playback, Sound Scrubbing (disabled, owner M8 C8.1).

The 05 items take their parameters from the clicked row and frame (`nodeIds`, `frame`); an item the kernel's command
cannot serve from that context as built is disabled with owner M6 C6.2 and listed in the handoff (E-2).

## 6. Main menus, toolbar, keys

- **Element menu** and **Play menu** as the plan's §7, in `desktop/src/ui-bindings/menu.json` and the host's native
  menu model (`menu_model.rs`); their items in `inspect_ui`'s menu bar (D-38).
- **Main toolbar:** `playStop` becomes a toggle (`playback.start` / `playback.stop` by `playback.state.get`'s
  `playing`), `lastFrame` `timeline.currentFrame.move {direction: "last", withinRange: true}`; `firstFrame`
  `timeline.currentFrame.move {direction: "first", withinRange: true}` (09 line 127: the range's first frame; it sent
  `app.frame.set {frame: 1}`: a declared change of the toolbar item).
- **Keys (04 §16):** C5.1 adds Previous / Next Element and Show Timeline Selection to the keymap defaults; arrow keys
  with the Timeline focused move the current cell (up/down: element, left/right: frame) through
  `xsheet.currentCell.set`. The in-place rename editor is a text field under the one key rule (D-69, D-73). The Play
  menu's Stop takes Shift+Enter, so the D-44 flow script's conflict holder (`scripts/c43-d44-flows.py` line 655, which
  needs a global key on no menu item) moves to another unmenued key.

## 7. Dialogs (shared modal manager; D-44, D-49)

Batch creation (type, name, count; 04 §3.2), Change Start Frame / Duration / Loops (one numeric field each, D-61),
Element Note (multi-line; empty removes), Marker / Background Colour (a swatch field with a hex entry; empty reverts
for Default), Extend Children Exposure (target frame), Set Current Frame, Set Playback Start / End Frame, and the
transfer-options dialog shared by Paste Special, Cut Special and drag-and-drop (04 §11.14's two groups, opened with
`timeline.transferOptions.get`, OK sends the operation with the options and `remember`, Paste Special Again repeats
without the dialog). Every choice of every dialog is a D-44 row; destructive questions default to Cancel.

## 8. Preferences rows (E-6)

The Preferences dialog's Interface and Timing categories gain: time display (`interface.timeMeasurement`), Timeline
background (`interface.background.timeline`), the element category colours (`interface.elementColours.*`, ten keys),
single cell selection (`timing.timeline.singleCellSelection`); each edits its key with `prefs.set`, and follows an
agent's change through `prefs.changed` (a test per control).

## 9. What an agent does instead (04 §17.1)

Every gesture is one structured command (the §4 table); every dialog a command's parameters; the view's state is
`timeline.view.set` (read with `{}`, SI-210); the rows are `timeline.rows.list`, the cells `xsheet.exposures.get`. An
attached agent reads the panel's controls in `inspect_ui` (the visible rows' DOM controls; the tracks canvas as one node
with its view state, its visible window and the selection).

## 10. Tests

- **Vitest:** geometry against the shared case table; the virtualised list (rows in view only); the draw function on a
  recording context (operation counts at 10 and 1,000 rows); each context menu model (items, availability, the
  reference sent, disabled with owner); each dialog through the modal manager; the read gates; the gesture controller
  against a host double (press, preview, release, refusal, cancellation, abort on Escape and blur, the newest-only
  scrub); an agent's change reflected per control.
- **Parity:** the collector renders the panel with rows, each context menu, each dialog, a collapsed parent and a drag
  in progress; the gestures go in the manifest's gesture section as window gestures with their commands.
- **D-39** rows for every control and menu item (MCP form in CI, OS form on the workstation) and **D-44** rows for every
  dialog choice, both checked against the generated inventories.
- **Three-way** rows for every gesture of §4: the window's recorded samples and layout replayed through
  `ExecutorHandle::timing_replay` give the window's command, and the direct command gives the same state, exactly.
- **E-10** rows (§3) and **E-11** tests: a scrub with a slow host sends only the newest frame; an agent's
  `timeline.view.set` moves the window; the budget of the plan's §6.2 measured for the round trip, with the optimistic
  draw-and-reconcile path taken only if the round trip misses it.

## 11. Budgets

As the plan's §6.2, measured by a C5.1b script on the workstation (release, 30 samples, D-33 control) and as shapes in
CI.
