# M5 design note: the Exposure Sheet view

Builder: Claude Opus 5.5. For C5.2 (`reviews/M5-C5.2-plan.md`, its rulings E-54 to E-60 in §17), written during the
C5.1b round 3 GUI hold: nothing in it is built yet. It follows the Timeline's note (`reviews/M5-design-timeline-view.md`)
section for section where the two views share a path, and says where they differ. Spec: 04 §12 (the view), §13 (cell
commands), §14 (drawing management), §15 (static light table), §6.1 to §6.3 (properties), §16 (keys), §17 (agents);
01 §5, §7.3, §8.4; 02 §10.4 (the light table in the Drawing view).

## 1. Anatomy and where each part's state lives

| Part (04 §12.2) | Window id | State (owner) | Read with |
|---|---|---|---|
| Toolbar: quick-add ×5, thumbnails, light-table panel, element list, scene selector, context control | `xsheet.toolbar`, `xsheet.add.<type>`, `xsheet.thumbnails`, `xsheet.lightTablePanel`, `xsheet.elementList`, `xsheet.scene`, `xsheet.contextButton` | toggles: `xsheet.display.set`, `xsheet.panel.set` (kernel, persisted); the scene: the current scene (`storyline.currentScene.set`, 08's semantics) | `ui.state.get` slices; `storyline.scene.list`, `storyline.currentScene.get` |
| Element list panel (left, optional) | `xsheet.elementList.<elementId>` (+ `.visible`) | Exposure Sheet order and visibility in the document (`element.order.set {order: "xsheet"}`, `element.visibility.set {scope: "xsheet"}`) | `timeline.rows.list`-like read of cell-based elements in sheet order (C5.2a: `xsheet.exposures.get` columns carry order and visibility) |
| Frame-number column | `xsheet.frames` | the current frame (kernel), frame labels (`interface.timeMeasurement`) | `timeline.frameLabel.get`, the shared frame slice |
| Column headers | `xsheet.header.<elementId>` (+ `.name` editor) | name, type, colours, note, width (`xsheet.columnWidth.set`, persisted with the workspace) | `xsheet.exposures.get` column records; `element.get` for colours and the note |
| Grid | `xsheet.grid` (one canvas surface, as `timeline.tracks`) | exposures (document), cell selection and current cell (kernel `cell.selection`, shared with the Timeline), scroll (window) | `xsheet.exposures.get {elementIds, from, to, include}` for the window in view |
| Static light table panel (right, optional) | `xsheet.lightTable`, items `xsheet.lightTable.<itemId>` | the light table's items, saved with the scene (C5.2a, 04 §15.3) | `xsheet.lightTable.list`, `xsheet.lightTable.changed` |

The view is one dockable panel (`xsheet`, the catalogue placeholder of `desktop/src/panels/catalogue.ts`), floatable as
every panel. Only cell-based elements are columns (12.1). The shown state of the element list and the light-table panel
is the kernel's `xsheet.panel.set` state, persisted across sessions (12.2).

## 2. Rendering and reading

- **One canvas for the grid**, as the Timeline's tracks: cells, hold lines, the selection, the focus rectangle and the
  current-frame row are drawn from a draw model; headers, the frame column and the panels are DOM (their controls are in
  the window tree, with their references). The grid reports `data-tt-value` with the revision of the exposures drawn
  (the C5.1b lesson: a harness and the agent tree can tell what is shown).
- **Virtualised in both directions**: the rows (frames) and the columns in view plus a margin; a scroll reads only the
  new window.
- **Reads follow the C5.1b discipline** (`desktop-async-reads-and-dock-remount`): read gates on every read (answers out
  of order are dropped), one heavy read in flight with the newest waiting request sent when it returns, re-read only
  what a notice changed (`cell.exposures.changed` with its element and frame range; `element.state.changed`;
  `cell.selection.changed` reads the selection alone; a frame notice moves the current-frame row with no read), the
  subscription made once per engine generation. The budget receipt lists the commands per phase.
- **Display options** (12.4): thumbnails, exposure text, element names, notes, from `xsheet.display.set`'s state (one
  state with any 01 §7.3 preference that names them; E-4's test per pair); every option shows an agent's change after
  the notice alone (E-6).
- **Thumbnails (E-56)**: never through the document thread. The window asks the host for the visible cells' thumbnails
  (a host read, `thumbnails_read {items: [{drawingId, rev, size}]}`, answering what the cache has) after the cells are
  drawn without them; the host renders the rest off the document thread from an immutable snapshot of each drawing at
  its revision into the cache (keyed by drawing id, revision and size, bounded by its share of
  `display.cacheMemoryPercent`), and a host-only notice (`shell.thumbnailsReady {items}`) says what became ready. While
  work is out, the toolbar shows a progress line ("Thumbnails 40 of 120"). A scroll drops the work for cells out of
  view. A drawing edited meanwhile has a new revision: the stale image is never shown (the revision is part of the key
  and of the answer). Agents keep the declared synchronous forms (`element.drawing.list {thumbnails}`,
  `xsheet.exposures.get {include: ["thumbnails"]}`, the derived `thumbnail`), 04 §17.1 item 9.

## 3. Gestures: the window's path

As the Timeline's (E-7): the window captures the pointer at each press, sends press, moves and release to the kernel's
timing-gesture interpreter, and calls it again at release, where the kernel decides the commit; the window draws the
interpreter's plan while dragging. The sheet is a second surface family of the interpreter (C5.2a), with its layout
(frames down at the row height, which depends on the thumbnails option; columns across at their widths; the scroll
offsets) sent with the press as the Timeline's layout is.

| Gesture (04 §12.5, §12.9) | Surface | Command at release |
|---|---|---|
| Click a cell | `xsheet.grid` | `cell.select` (the click's frame and element current, as the Timeline's click) |
| Drag, or extend-click | `xsheet.grid` | `cell.select {from, to, elementIds}` (a rectangle) |
| Click a column header | `xsheet.headers` | `element.select` (the whole column) |
| Drag selected cells to a compatible column | `xsheet.grid` | `cell.range.move` (move, copy, link, copy as new drawings) with the drag-and-drop transfer options (11.13-11.14, `timeline.transferOptions.get {operation: "dragDrop"}`), as the Timeline's |
| Drag the bottom edge of an exposure | `xsheet.grid` | `cell.exposure.add` / `cell.exposure.remove {frames}` (following cells shift) |
| Drag a column header | `xsheet.headers` | `element.order.set {order: "xsheet"}` |
| Drag a header border | `xsheet.headers` | `xsheet.columnWidth.set` (window-only drag, one command at release) |
| Drag cells onto the light-table panel | `xsheet.grid` → `xsheet.lightTable` | `xsheet.lightTable.add {items}` |
| Library item, external files onto the grid | — | refused with their owners (M9 08, M10 09) until then |

Each row is a D-23 row and a three-way row (window, interpreter replay, direct commands; event path in CI, real input
on the workstation), with E-10's change under a drag. Auto-scroll near an edge while dragging (12.2) moves the window
through the view, not through a command.

## 4. Typing in a cell and the keys

- **The in-cell editor (E-54)**: typing in the grid, or a double click on a cell, opens an editor over the current cell
  prefilled with its label (13.13). Enter sends `cell.label.set {elementIds, from, to, drawing, createMissing: true}`
  for drawing and annotation elements (a new name creates the drawing in the same step: one command, one undo step),
  `createMissing: false` for image and media elements (an unknown name: `cell.drawingNotFound`). A refusal follows E-53:
  the status line says why, the editor stays open with the text; Escape and the unchanged label close it with nothing
  sent. While it is open the keys the sheet binds act in the field (the one key rule).
- **Column-header rename**: a double click on a header's name opens the same in-place editor as the Timeline's row
  (E-50), sending `element.rename` with E-53's rules.
- **Keys (12.5, §16)**: the arrows move the current cell (frame up and down, element left and right among the sheet's
  columns); previous / next drawing; previous / next element in the Exposure Sheet (`element.current.move`, regardless
  of focus); the high-frequency timing commands have single-key defaults (§16's design decision; the key map is a UX
  task, recorded in the keymap with their spec lines). Deselect-all clears the cell selection (12.5).
- `xsheet.exposures.set` and `element.drawing.create` are agent-only (E-54: the latter's only place in 04 §14.3 is
  "Create drawing (command)", line 1161).

## 5. Context menus (04 §12.8)

Transcribed item by item into a coverage test, as E-47 did for the Timeline:

- **Column headers** (drawing, image, media, annotation): Arrange (9.1); Hide, Show All, Hide All Others (10.2); Marker
  Colour, Default Colour; Cut and Copy of the element; Clone, Duplicate, Delete, Rename; Set Element Note; Send to Static
  Light Table (the element's current drawing). **Sound header**: the same without Duplicate, plus Edit Sound and, with
  lip-sync shown, Modify Lip-sync Mapping (both disabled with owner M8).
- **Grid empty area**: Add Drawing / Image / Sound / Media / Annotation Element; the display options (12.4) with their
  states; Import File (disabled with owner M10).
- **Cells**, per column type: the 13.x commands bound (the C5.2 deferred entries leave `deferred.json`): Add / Remove
  Exposure, Set Exposure 1 / 2 / 3 / N…, Extend Exposure, Insert Cells…, Insert Blank Cell, Delete Blank Cell, Create
  Cycle…, Columnate…, Duplicate Drawing, Clear Drawing, Lock / Unlock Drawing (a toggle with its state), Rename
  Drawing…, Set Cell Note…, Reset Media (media), Cut, Copy, Paste, Delete (the `edit.*` forms, E-5 and E-17; the
  explicit `cell.clipboard.*` and `cell.range.delete` are agent-only, E-55, with the equivalence test extended to the
  sheet); imports, capture, scanner, convert to vector, sound and lip-sync items present and disabled with their owners
  (M10 09, M8 07). Enabled only when applicable (12.8's last paragraph): paste with compatible clipboard content, Delete
  Blank Cell on a blank cell, Lock on a non-blank drawing cell.
- The Timeline's cell context gains the same C5.2 items enabled (E-2 (a)).
- Every item disabled for a later owner states the owner; while the engine is stopped every item gives the engine's
  reason (E-50 item 3, E-52's observation). An agent opens these menus with the `key` tool's Shift+F10 aimed at
  `xsheet.grid`, `xsheet.header.<id>` or `xsheet.elementList` (E-48, E-52), one request at a time (E-62).

## 6. Dialogs (shared modal manager; D-44, D-49)

Set Exposure to N (the one numeric field, D-61); Insert Cells (count, blank or named, one element or all, before or
after); Create Cycle (cycles); Columnate (first and last drawing, exposure each, last exposure, loop mode, placement,
with the live total of frames it will write); Rename Drawing (prefilled, 13.14, with `drawing.name*` refusals shown in
the dialog and on the status line, E-53); Set Cell Note (prefilled; Update when one exists, as the element note); Paste
Special and Cut Special in the sheet (the transfer-options dialog shared with the Timeline). Every choice of every
dialog is a D-44 row; a typed value goes to the kernel and a refusal keeps the dialog open (E-53, E-62: only a truly
empty field is the "leave unchanged" sentinel where a field has one).

## 7. The static light table panel (04 §15; E-57)

One panel, in the Exposure Sheet: an ordered list, each item with a visibility control, an over / under control and the
label `<drawing> in <element>`; drag to reorder (`xsheet.lightTable.move`); its context and the panel's commands of
15.4 (`xsheet.lightTable.set {apply: given | all | others}`, `xsheet.lightTable.remove`); cells dropped on it add them.
The window sends the `xsheet.lightTable.*` family. The Drawing view's aid section loses the M4 interim controls; it
keeps 02 §10.4's part (how the entries are drawn there, and the shade toggle `drawAid.lightTableShade.set`).
`drawAid.staticLightTable.*` and `drawAid.staticLightTablePanel.set` are agent-only equivalents of the same state, with
the one-state test per pair (SI-218). The panel's visibility stays a user preference (one state with `xsheet.panel.set
{lightTable}`), and saving the items with the scene is not a document change (the M4 reviewer test that restarts with
the aids kept and the document unchanged).

## 8. The Properties and Cells panels (04 §6; E-58, E-59)

- **Element panel** (drawing, image, media, annotation): name, start, duration (read-only), Type (`element.properties.set
  {role}`) and Deformable (`element.properties.set`). The transform fields (position, scale, skew, rotation, pivots,
  orient to path) and Velocity are present and disabled with owner M6 (05's semantics; SI-096).
- **Peg panel**: name, start, duration; loop mode (`anim.loop.set`, one state with `element.timing.set {loops}`, E-3);
  Constant Z (as the Timeline's toggle); the transform fields and Velocity disabled with owner M6. **Camera panel**: all
  fields disabled with owner M6. **Sound panel**: owner M8.
- **Cells panel** (in the Properties window and as its own Cells window): element name, frame label, preview (the
  thumbnail path of §2) with its toggle, drawing name (Enter: `cell.label.set {drawing}`; an unknown name:
  `cell.drawingNotFound`, E-53's rules), drawing slider (bank order). **The slider (E-58)**: one `cell.label.set` at
  release, one undo step; Escape cancels; during the drag the panel's name field and preview follow each step. 04 line
  383 asks for the substitution to be live in the views: if the existing preview path can draw a substituted drawing
  without a commit, the views show it during the drag (to be established in C5.2b and stated here); if not, the views
  follow at release, recorded as a known limit and a spec issue. No interpreter surface for a slider. Disabled when the
  selection is not a cell of a drawing, image, media or annotation element; follows the current frame.
- **Drawing inspector** (SI-168): its subject becomes the drawing of the cell selected in the Cells panel or the sheet.

## 9. Preferences rows (E-6)

Every preference the sheet shows is followed after an agent's change through the `prefs.changed` notice alone, each with
a Vitest: `interface.timeMeasurement` (the frame column), `interface.elementColours.*` (header markers),
`interface.background.*` for the sheet if 01 §7.3 names one, and the display options if they are preferences (else the
`xsheet.display.set` state, followed through its notice).

## 10. What an agent does instead (04 §17.1)

Reads the whole sheet or a window of it (`xsheet.exposures.get`, run-length JSON, 17.2), writes whole windows atomically
(`xsheet.exposures.set` with `expectRev`), uses the cell commands with explicit targets (`cell.*`, `cell.clipboard.*`,
`cell.range.delete`), the light table through either family, the display and panel state through `xsheet.display.set`
and `xsheet.panel.set`, and the thumbnails through the synchronous queries. It opens a context menu with Shift+F10 and
runs an item's reference itself (its own invocation, origin and scopes), as in the Timeline (note §6b).

## 11. Tests

- Kernel (C5.2a): the plan's §13 list; E-51's tests (history.rs, `c52a_history_bound`); the seven `xsheet.*` commands
  (schemas, examples, sweep, availability, coexistence); the light table saved and restored with the scene; thumbnails
  (E-56's three tests: 200 pending while commands answer at normal speed, a stale image never shown, the cache's
  bound); the interpreter's sheet surface (D-23 table, headless per gesture, E-10); the shapes at 1,000 columns.
- Window (C5.2b): Vitest per control and per preference; the context tables against 12.8 item by item; the in-cell
  editor and header rename with E-53's refusals; D-39 rows in both forms (the context rows through Shift+F10); D-44
  rows; three-way rows; the read discipline (commands per phase); budgets in debug and release, after idle, several
  runs; every harness wait a wait for a state (D-45), every failure printing the full answer.

## 12. Budgets

The plan's §11: a window of 40 columns by 60 frames read in 5 ms p95 (release); draw per frame while scrolling 8 ms
with 1 % over twice the control; a click to the selection drawn and a wheel step to the repaint within two frames;
thumbnails for a screenful: a first batch within 250 ms, the screen within 2 s (a workstation receipt, E-56); release to
commit for a cell drag 50 ms (E-44: on the keyed 1,000-row scene this misses until M11 C11.1, stated as such).

## 13. Open points for C5.2b

- Whether the preview path can draw a substituted drawing without a commit (E-58), established at the start of C5.2b
  and written in §8.
- The row height with thumbnails on (a fixed height per option, so the layout sent to the interpreter is exact).
- The scene selector: `storyline.scene.list` for the list in order, `storyline.currentScene.set` for the choice (spec A
  lines 844, 856), followed through `storyline.currentScene.changed`.
