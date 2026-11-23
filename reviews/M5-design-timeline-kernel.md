# M5 C5.1a design note: the Timeline's kernel

Before the code (plan `reviews/M5-C5.1-plan.md` §12 step 2; rulings E-1 to E-12). Scope: the 28 `timeline.*` and
`playback.*` commands, one state per overlapping pair (SI-202, SI-203, E-3, E-4, E-6), the single transfer-option store
(SI-201, E-8), the playback clock (E-1), the timing-gesture interpreter (E-7, E-10), the `xsheet` render composition
(SI-107), and the E-5 equivalence of `edit.*` with the `cell.*` explicit-target forms. The window's half (the panel,
previews, menus) is `reviews/M5-design-timeline-view.md` (C5.1b).

Spec: 04 §11 lines 589-795, §17.1 lines 1262-1309, §17.3-§17.5 lines 1363-1499; 09 §2 lines 72-212, §11.2 lines
995-1011, §11.4 lines 1096-1100; 01 §6.2-§6.4 lines 505-580, §6.6 lines 611-623, §7.3, §12.2 lines 1063-1067.

## 1. One state, its commands, its events

Every state below has one home. Each command that writes it goes through the same function, and `Core::settle`
(`crates/executor/src/run.rs`), which already compares the session before and after every invocation, raises the
events. So two commands that write the same state raise the same events by construction, and the E-4 test form checks
it: for each pair, the same starting state, each command applied to a copy, then equal document, equal session state
and equal event lists (names and payloads, the `source` field excepted where the spec gives the two commands different
sources).

| State | Home | Written by | Events (from `settle`) |
|---|---|---|---|
| current frame | `AppContext.frame` | `app.frame.set`, `app.frame.move`, `timeline.currentFrame.set`, `timeline.currentFrame.move`, `xsheet.currentCell.set` (C5.2), the playback clock, `cell.select` | `app.state.changed`, `cell.selection.changed`, **new** `timeline.currentFrame.changed {sceneId, frame, source}` |
| current element | `AppContext.element` | `app.currentElement.set` (agent-only, E-4), `element.current.set`, `app.currentElement.move`, `element.current.move`, `xsheet.currentCell.set` | `app.state.changed`, `cell.selection.changed` |
| playback range of a scene | `Session.playback.ranges[scene]` (mode, start, end, last free start and end), saved in `session.json` (SI-204) | `timeline.playbackRange.set`, `playback.range.set`, `playback.range.mode.set` | **new** `timeline.playbackRange.changed {mode, start, end}` and `playback.range.changed {sceneId, mode, start, end}`, both on every change |
| loops of a bar element | the element's `loops` (document) | `element.timing.set {loops}`, `anim.loop.set` | `element.state.changed`, `anim.changed` (E-3 proves the same set) |
| time display, Timeline background, single-cell selection, category colours | the 01 §7.3 preference keys `interface.timeMeasurement`, `interface.background.timeline`, `timing.timeline.singleCellSelection`, `interface.elementColours.*` | `prefs.set` (the Preferences dialog, E-6), `timeline.timeDisplay.set`, `timeline.backgroundColour.set`, `timeline.singleCellSelection.set`, `element.typeColour.set` | `prefs.changed` (what `prefs.set` raises) |
| transfer options per operation | the 01 §7.3 keys `edit.pasteSpecial.timeline`, `edit.cutSpecial.timeline`, `edit.dragDrop.timeline` (§4) | `prefs.set`, `timeline.transferOptions.set`, `edit.pasteSpecial` / `edit.cutSpecial` (remembered after a run, 01 line 537) | `prefs.changed` |
| playback (playing, rate setting, loop, force rate, measured rate, dropped frames) | `Session.playback` | `playback.*` and the clock | `playback.started`, `playback.stopped`; `timeline.currentFrame.changed` with source `playback` |
| Timeline view (zoom level, first frame, viewport width) per scene | `Session.timeline.views[scene]` | `timeline.view.set`; the window's viewport report (§2.9) | host-only `shell.timelineView` (§2.9) |
| sound waveforms on/off | `Session.timeline.waveforms` (session; 04 gives no persistence) | `timeline.soundWaveforms.set` | host-only `shell.timelineView` |
| effect-parameter rows shown, per effect element | `Session.timeline.parameter_rows` | `timeline.effectParameterRows.set` | host-only `shell.timelineView` |

`timeline.currentFrame.changed`'s `source`: `command` for `app.frame.set`, `timeline.currentFrame.set`,
`xsheet.currentCell.set` and selections that move the frame; `navigation` for the stepping commands (`app.frame.move`,
`timeline.currentFrame.move`); `playback` for the clock (04 line 1466). During playback it is throttled to at most ten
per second (the newest frame wins, and the stop always carries the final frame).

Element categories (SI-203 interim, E-6): `element.typeColour.set {category}` writes, and `.get` reads, these keys:
drawings → `interface.elementColours.drawing` (drawing and annotation elements); images → `.image`; sounds → `.sound`;
pegs → `.peg`; skeletons → `.skeleton` and `.bone` (both written; `.get` reports `.skeleton`); media → `.media`;
cameras → `.camera`; effects → `.effect`; effectParameters → `.effectParameter`. `element.get`'s
`effectiveMarkerColour` reads the same keys, so the session map `TimelineState.type_colours` is retired.

## 2. The commands

Each command is hand-declared (`#[command]`) with the spec's parameters and result, examples run on the fixture
under the watchdog, its prechecks as handlers resolve them (shared resolution functions, as M4 required), and its
availability inventory rows. The generated stubs drop out by regenerating `declared.rs`/`declared.json`; the builder
tests that pinned the stub status (`c12_registry.rs:114`, `crates/registry/tests/declared.rs:82-103`) move to
implemented commands.

### 2.1 `timeline.currentFrame.set {frame, sceneId?}` → `{frame, clamped}`

Clamped to 1..scene length (04 line 1438; not an error, unlike `app.frame.set`'s argument check, which stays as built:
SI-202 records the difference). `sceneId` other than the current scene: `arg.invalid` (switching scenes is 08's).
Playing: stops playback first, reason `navigation` (09 line 138), then sets the frame.

### 2.2 `timeline.currentFrame.move {direction, unit, elementId?, count, withinRange}` → `{frame, moved}`

`first` / `last`: the playback range's start / end when `withinRange`, else 1 / scene length. `previous` / `next` by
`count` frames, or by drawing changes of `elementId` (default the current element; with none, the nearest change
across all visible cell-based elements, 09 line 132). Never wraps (09 line 137); `moved: false` at a bound. Stops
playback first.

### 2.3 `timeline.playbackRange.set {mode?, start?, end?}` and `playback.range.*`

One function `set_range(scene, mode, start, end)`: a free value given while `auto` switches to `free` (09 line 1003's
rule, applied to both); a start past the end moves the end to the start and the reverse (09 lines 186-187); then both
are clamped to 1..scene length (04 line 669) and the clamped values returned (`adjusted: true` when anything moved, for
`playback.range.set`). `auto` keeps the last free values; switching back to `free` without values restores them (09
line 188). In `auto` the range reads as 1..scene length and follows the scene length. A scene shorter than a free end
clamps the end when read (09 line 187). `playback.range.get {sceneId?}` and `timeline.playbackRange.get` read it;
`timeline.rows.list` carries it.

### 2.4 `timeline.rows.list {sceneId?, from?, to?}`

The read model of 04 §17.5 lines 1491-1496: `{sceneId, sceneLength, currentFrame, playbackRange, rows}`, rows in
Timeline order (depth first, parents before children), each `{elementId, name, type, depth, parentId, collapsed,
visibleInScene, hiddenByParent, locked, bar: {start, end, loops}, keyframes, childKeyframeFrames, clips?}`. Cell-based
rows give their exposed extent as `bar`. Keyframes come from the evaluator's channel keys (the same source
`anim.key.list` reads): one entry per keyed frame with the keyed transform channels among `position`, `scale`,
`rotation`, `skew`, and `segmentAfter` (`constant` / `interpolated`, from the segment flag of the keys at that frame).
`childKeyframeFrames`: frames where any descendant has a keyframe. `from`/`to` clip keyframes and child frames to the
window (bars are reported whole); omitted means the whole scene. `clips` for sound rows is `[]` until M8. Work is
charged to the timing budget (`Budget`); a 1,000-element scene answers in one pass over the tree.

### 2.5 `timeline.frameLabel.get {frames}` → `{labels}`

Per `interface.timeMeasurement`: `frames` → `"20"`; `feetAndFrames` → `feet+frames` with two-digit frames at 16
frames per foot, frame 20 → `1+04` (04 line 644). Frame 1 is `0+01` (frame numbers count from 1; the footage is the
frame number in feet and frames, as 04's example implies).

### 2.6 `timeline.timeDisplay.set {unit}`, `timeline.backgroundColour.set {colour}`, `timeline.singleCellSelection.set {enabled}`, `element.typeColour.set {category, colour}`, `element.typeColour.get`

Structured equivalents of `prefs.set` on the keys of §1 (E-6): the same validation, the same write, `prefs.changed`.
`unit: "feetFrames"` maps to the key's `feetAndFrames`. All four setters become agent-only (E-6); `element.typeColour.get`
stays agent-only (the Preferences dialog reads `prefs.list`).

### 2.7 `timeline.transferOptions.set {operation, options}` and `.get {operation}`

§4.

### 2.8 `timeline.soundWaveforms.set {enabled}`, `timeline.effectParameterRows.set {visible, elementId?}`

Session state (§1). `elementId` must be an effect element (`element.notFound`; another type `arg.invalid` with the
reason); omitted sets every effect element's flag and the default for new ones. E-9: the window binds these only when
they have something to show (no sound rows until M8; effect-parameter rows exist under clipping effects today).
Ruling E-37 moves the effect-parameter toggle's window binding to M7: until then the setting is stored and reported by
the view notice, and does not hide rows in the Timeline (§13).

### 2.9 `timeline.view.set {zoomLevel?, firstFrame?, revealElementId?}` → `{zoomLevel, firstFrame, lastFrame}`

The zoom ladder and its default come from `crates/executor/src/timeline-layout.json` (§5.1). `firstFrame` clamps to
1..max(1, scene length + the margin of empty frames shown, 04 line 676). `lastFrame` = `firstFrame` + the visible
frame count − 1, the visible frame count being the viewport width in pixels (reported by the window when the
Timeline is shown or resized, through the host's shell report; 960 px for a headless session, which has no Timeline)
divided by the zoom level's cell width, times its frames per cell. `revealElementId` (Show Timeline Selection) is
recorded so every open Timeline scrolls that row into view (SI-206); it does not change the frame window unless the
element's bar lies wholly outside it, in which case `firstFrame` moves to the bar's start. With no parameters the call
answers the current state (an agent's read: SI-210). The window follows an agent's call through the host-only notice
`shell.timelineView` (never sent to an MCP client; it carries only what `timeline.view.set {}` answers), so an agent
moves the window (E-11).

### 2.10 `playback.*`

| Command | Behaviour |
|---|---|
| `playback.start {sceneId?, fromFrame?}` | starts playing the current scene from `fromFrame` or the current frame; outside the range, from the range start (09 line 110). Already playing: `{playing: true}` again, nothing restarts. Another `sceneId`: `arg.invalid` (scene switching is 08's) |
| `playback.stop` | stops; the frame stays (09 line 111); reason `user`. Not playing: answers the frame, no event |
| `playback.state.get` | `PlaybackState` of 09 lines 1096-1100: `sound: false` and `scrubbing: false` until M8 |
| `playback.loop.set {enabled}` | session; default off (09 line 143) |
| `playback.rate.set {fps}` | `"project"` or 1-120 (09 line 162); `effectiveFps` is the project rate for `"project"`; persisted per user in the settings store (09 line 161 says a user preference, but 01 §7.3 has no key: SI-209) |
| `playback.forceRate.set {enabled}` | session; default off |
| `playback.range.*` | §2.3 |

**The clock** (`crates/executor/src/playback_clock.rs`). One thread per executor, started by `playback.start`, posting
`Request::PlaybackTick(generation)` to the document thread at the effective rate; a stop bumps the generation, so a
late tick of an earlier run does nothing. On a tick:

- **Force Frame Rate on** (time-based): frame = range start offset + floor(elapsed × fps) since the run began (a frame
  later than the next is a skip, counted in `droppedFrames`).
- **Off** (frame-based): one frame per tick; when the window has not yet reported the previous frame presented (the
  host's presentation report), the tick is skipped and the frame held, so playback runs slower than the rate (09
  line 170). A session without a window never waits.
- At the range end: Loop on wraps to the start; off stops on the last frame, reason `end` (09 lines 114-115). A
  one-frame range repeats that frame with Loop on.
- Any frame or element navigation by a person or an agent stops playback first, reason `navigation`; a scene change,
  reason `sceneChanged`; a project close stops it. Edits during playback are allowed (09 line 208).

**Deterministic tests (E-1, D-45).** Under the existing `test-hooks` feature (absent from every product build,
`scripts/check-release-features.py`) the clock can be replaced by a stepped clock: `ExecutorHandle::playback_step(n)`
posts n ticks with a simulated elapsed time, and the presentation report can be withheld or given. No CI assertion
reads a wall clock; the product's real rate is a workstation receipt.

**The window** gets every played frame through a host-only notice `shell.playbackFrame {sceneId, frame, playing}`
(D-57; SI-207), and agents the throttled `timeline.currentFrame.changed`.

## 3. Element timing pairs (E-3)

`anim.loop.set {nodeId, loops, loopType, positionLoop}` and `element.timing.set {elementIds, loops}` both write the
element's loop fields through one function; `element.timing.set` leaves `loopType` and `positionLoop` as they are. The
E-3 test: on a peg with keys, each command with the same `loops` on a copy; equal documents, equal undo labels' kinds
(each keeps its own label), equal event lists.

## 4. One store of transfer options (SI-201, E-8)

- **Store:** the three preference keys of 01 §7.3, one per operation (`paste`, `cut`, `dragDrop`), in 01's member names.
- **Defaults:** one table, `crates/executor/src/prefs.json` (the keys' defaults); `TransferOptions::default` and
  `TimelineEditOptions::default` are removed, both reading the stored set and overlaying the given members.
- **Mapping** (04 §17.3 ↔ 01 §6.3): `overwriteDrawings` ↔ `overwriteDrawings`; `exposure` ↔ `createDeleteExposure`;
  `autoExtend` ↔ `extendExposures`; `updateColours`; `keyframeSubstitution`; `boundaryKeys`; `pegKeys.{motion,
  rotation, scale, pivot, velocity, skew}` ↔ `{motion, rotation, scale, pivot, velocity, skew}Keys`.
- **Users:** `timeline.transferOptions.set/get` (04 names); `edit.cutSpecial` (the `cut` set), `edit.pasteSpecial` (the
  `paste` set, remembered after a run as today); `cell.range.move` with omitted members (the `dragDrop` set, 04 line
  1366); `cell.clipboard.set {cut: true}` (the `cut` set), `cell.clipboard.apply` (the `paste` set).
- **The open default (E-8).** Today `cell.range.move` and `cell.clipboard.*` default `overwriteDrawings` and
  `updateColours` to off (04), the preference keys to on (01). One table cannot keep both. Until the ruling, the code
  keeps the defaults table in one place and the commands' behaviour as built: each command family's omitted-member
  default is read from the one table through a per-family overlay of exactly those two members (a single constant, so
  the ruling changes one line). Before that, the reviewer tests that pin either value are found by flipping each set
  locally and running the suites (§9), and sent to the orchestrator.

## 5. The timing-gesture interpreter (E-7, E-10)

`crates/executor/src/timing_gesture.rs`, product code: the window calls it through the host at the press and at the
release (not a copy), and the headless replay calls the same functions.

### 5.1 Layout

`timeline-layout.json` (one source for kernel and window): row height (22 px), ruler height (24 px), the zoom ladder
`[{cellWidth, framesPerCell}]` from 1 px × 10 frames to 48 px × 1 frame and the default level, bar handle width (5
px), keyframe glyph radius (5 px), the drag threshold (4 px), the drop zones of a row (upper and lower quarter: between;
middle half: onto, for rows that can be parents), the margin of empty frames past the scene end, and the modifier roles
per platform (extend: Shift; toggle: Primary; single cell: Alt; copy: Primary on Windows, Alt on macOS; link:
Shift+Primary on Windows, Alt+Primary on macOS; 04 §18 #7 leaves them to design: SI-211). A shared case table
`timeline-geometry-cases.json` (x and y to frame, row, zone at every zoom level) is checked by a Rust test and a Vitest
test, so the window's preview arithmetic and the kernel's cannot drift.

A gesture's layout, captured at the press: `{surface: "layers" | "tracks" | "ruler", zoomLevel, firstFrame,
scrollTop, width, height, platform}`. Rows are not sent: the kernel derives the visible rows from the document (Timeline
order minus collapsed parents' descendants) and `scrollTop`.

### 5.2 Press and release

- `press(doc, session, layout, sample) -> Grab`: hit test and classification: what is under the press (a row name, a
  disclosure, a bar body, start or end handle, the loop handle, a keyframe, a cell inside or outside the cell
  selection, the ruler, the playhead, a range marker), the target ids and frames, and the document revision. The
  window draws its preview from the grab.
- `release(doc, session, layout, grab, samples) -> Outcome`: `Commands(Vec<Invocation>)` or `Cancel(reason)`. A click
  (movement under the threshold) and a drag map to the commands of the plan's §6.3 table; the row click is two `ui`
  commands (`element.select`, `element.current.set`); the ruler drag's commit is the last frame (the window already sent
  the newest frame while dragging, E-11).
- **A change under the drag (E-10).** When the document revision or the relevant selection differs from the grab's,
  the release re-resolves by id: the grab's targets must all still exist with the same parent and type, a reorder's
  neighbours must still be adjacent at that level, a cell drag's source cells must still hold the grabbed drawings;
  then the command is computed against the present state and committed only when it is the command the preview
  showed (the same targets, the same delta or destination). Otherwise `Cancel` with a reason (`timeline.targetChanged`
  details naming what changed), which the window shows on the status line (D-87's one place). Tests: deterministic,
  with the existing commit-delay hook (`TT_TEST_COMMIT_DELAY_MS`) and an agent edit between press and release, for
  bar, row, keyframe and cell drags.
- Escape, a lost capture or an engine restart: no command (01 line 495).

### 5.3 The D-23 table

`crates/executor/tests/c51_gestures.rs`: one row per gesture of the plan's §6.3, each stating its spec line, the
expected command, parameters and undo step (or none) written from the spec, then comparing three paths: the
interpreter's commands, the document after them, and the document after the direct command; rows that must make no
step included (a click on empty track, Escape, a drop onto a drawing row). The window's rows join in C5.1b with its
recorded samples and layout (exact, D-32 as ruled by E-7).

## 6. The `xsheet` render composition (SI-107)

`render.frame.run {composition: "xsheet"}`: the scene at a frame composited in Exposure Sheet order (04 §1.2 line 56:
left is back), cell-based elements only, Exposure Sheet visibility (04 §1.3), no camera transform (drawing space
mapped to the output as the `element` composition maps a drawing, at the camera's size), annotation elements excluded
(04 §2.6 design decision). The golden-image test renders a three-column fixture twice and compares bytes; a hidden
column and a reordered column each change the result as stated.

## 7. E-5: `edit.*` against the explicit-target `cell.*` forms

`crates/executor/tests/c51_edit_equivalence.rs`, for the `timeline` context and for the `xsheet` context, from the same
document, cell selection and clipboard: `edit.copy` / `cell.clipboard.set {}`, `edit.cut` / `cell.clipboard.set {cut:
true}`, `edit.paste` / `cell.clipboard.apply {}`, `edit.delete` / `cell.range.delete {}`; compared: the document, the
clipboard (`edit.clipboard.get`) and the selection (`cell.selection.get`). 04 §13.17 makes the Timeline's Delete follow
the cut-special exposure option while 01 line 523 says Delete clears cells: if the kernel's two commands differ there,
the difference goes to the orchestrator before either changes (E-5).

## 8. Spec issues added by this note

- **SI-209:** the playback rate is a user preference (09 line 161) without a 01 §7.3 key; interim: the user settings
  store, as tool settings are kept.
- **SI-210:** no query reads the Timeline view; interim: `timeline.view.set` with no parameters answers the current
  state; the window follows through a host-only notice.
- **SI-211:** the Timeline's modifier roles (extend, toggle, single cell, copy, link), 04 §18 #7 leaving them to design;
  interim: §5.1's table.

## 9. Tests (C5.1a)

- `c51_commands.rs`: every parameter of every new command through the real path (D-27), the clamps and adjustments,
  `moved`, the range rules, `frameLabel` at 1, 16, 17, 20, 9,999.
- `c51_pairs.rs`: E-3, E-4, E-6 and every SI-202 pair in the one form of §1.
- `c51_playback.rs`: the stepped clock: start inside and outside the range, loop on and off, a one-frame range, rate
  and force rate (a held presentation report slows frame-based playback and makes time-based playback skip),
  navigation and scene change stop it, the throttled event and the per-frame notice, no `shell.*` notice on an MCP
  client's stream.
- `c51_rows.rs`: `timeline.rows.list` against a fixture with pegs, keys of each channel, constant segments, a collapsed
  parent, clipped windows; the CI shape at 1,000 elements (ratio and ceiling, D-45); the workstation receipt ignored in
  CI.
- `c51_gestures.rs`: §5.3, the geometry case table, E-10's schedules.
- `c51_transfer.rs`: one store: each user of §4 reads and writes it; `prefs.set` and `timeline.transferOptions.set` give
  the same stored value.
- `c51_edit_equivalence.rs`: §7.
- The `xsheet` composition golden (§6).
- Availability: every new handler's (command, code) pairs in `docs/availability-inventory.md`
  (`scripts/check-availability-inventory.py`).

## 10. After rulings E-13 to E-20

- **One store, per-field family defaults (E-13, narrowed by E-17, replaced by E-19):** the store keeps the members
  that were set, field by field, as a partial object under the 01 key (`transfer_options::store_members`, the one
  write path of `prefs.set` on the three keys, `timeline.transferOptions.set` and the special commands' `remember`).
  A member never set reads its family's default: `EDIT_DEFAULTS` (01 §6.3, = `prefs.json`) for `edit.*Special`,
  `CELL_DEFAULTS` (04 §11.14) for the `cell.*` forms and `timeline.transferOptions.get`. The store applies a set object
  even at `prefs.json`'s values (it still governs the 04 family); `prefs.reset` stages the empty set, which removes the
  key. `prefs.changed` names a key when its effective (01-side) value changes. A unit test prints the two members in
  which the tables differ and keeps `prefs.json`, `TimelineEditOptions::default` and `TransferOptions::default` equal to
  their tables; the plain commands do not read the store.
- **A cut records its set (E-20):** `cell.clipboard.set {cut: true}` records the option set it applied (01's names) in
  the clipboard's `options`, as `edit.cutSpecial` does; a copy records none. The reviewer suite has no pin on the
  `null` it recorded before.
- **Plain Cut, Paste and Delete (E-17, SI-213; E-14 withdrawn):** as built since M3 and 01 §6.2: Cut clears, Paste
  writes over and overwrites same-named drawings, Delete clears in both views. `cell.range.delete` keeps its explicit
  `shift` (default `false`), so with its target omitted it equals `edit.delete`. The E-5 tests prove the special forms
  equal the structured forms with every member stored, name the two SI-201 members as the only difference with nothing
  stored, and pin the plain ones' differences.
- **`sourceContext` (E-15):** `cell.clipboard.set` with its target omitted records the focused cell context
  (`timeline` or `xsheet`), an explicit target `xsheet`. No paste rule reads the label: `paste` dispatches on the
  clipboard's `kind` and the destination context only (`crates/executor/src/commands/edit_ops.rs`); the label is
  informational (`edit.clipboard.get`).
- **Category colours (E-16):** §1's table is now built: `CATEGORY_KEYS` maps 04's categories to the 01 keys
  ("skeletons" writes the skeleton and bone keys together; a bone element's colour is the bone key); the session map
  `TimelineState.type_colours` is removed.

## 11. After round 1 (rulings E-21 to E-28)

- **A gesture's targets and outcome (E-10, E-23, E-28): one rule for every gesture kind.** At the press the kernel
  keeps the document snapshot and the selection state. At the release the interpreter makes the plan **once, against
  the snapshot**: its targets are the ids resolved at the press and the content it grabbed is the snapshot's; the press
  position is never hit-tested again against the present document. The plan is then resolved to its **outcome in
  absolute values** twice, against the snapshot (what the preview showed) and against the present document
  (`timing_gesture::outcome`): for a bar, the resolved start, duration and loops with the element's keyed frames; for a
  keyframe move, its frames with the element's keyed frames; for a reparenting, the parent's child order after it; for
  a reorder, the order and the level's present children; for a cell move, the destination, the drawings and notes of
  the grabbed cells and the destination cells' contents; for every command, the ids it names that no longer exist;
  the selection and position commands are absolute. **Equal outcomes commit the plan; different outcomes cancel**
  with `timeline.targetChanged` (details: the plan and both outcomes), and nothing changes. So a row inserted above a
  grabbed bar commits (same id, same timing; C51A-R1-04), and a drawing replaced in a grabbed cell cancels
  (C51A-R1-03), as does a relative command on a target changed meanwhile (C51A-R1S-02).
- **Drawing steps at an end (E-21):** `drawing_step` stops before it asks for a range past the end or before the
  start, so Next Drawing at the last frame (or past the range's end) and Previous Drawing at the first frame (or before
  the range's start) leave the frame where it is, `moved: false`, for any `count` (09 line 135).
- **Navigation stops playback (E-22):** every navigation form (`timeline.currentFrame.set`, `.move`, `app.frame.set`,
  `.move`, `xsheet.currentCell.set`) marks the session navigated; settle stops playback with reason `navigation` even
  when the frame does not change (09 line 138). The event's `source` values are unchanged.
- **Another scene's range (E-24):** `playback.range.set` and `.mode.set` with a `sceneId` that is not the current
  scene raise `playback.range.changed` (with that `sceneId`) and `timeline.playbackRange.changed` (A's payload, no
  scene: SI-214) when that scene's effective range changes, without navigating or touching the selection.
- **Reversed windows (E-26):** `timeline.rows.list` refuses `to` before `from` with `arg.invalid` at `/to`, as
  `xsheet.exposures.get` has since M3 (04 line 26, line 1306 item 11); `to` omitted defaults to the scene length or
  `from`, whichever is later.
- **Clamping (E-27):** `timeline.currentFrame.set {frame}` accepts any integer and clamps it to 1..scene length (09
  line 139); `timeline.currentFrame.move {count}` below 1 counts as 1; `timeline.view.set {firstFrame}` and
  `playback.start {fromFrame}` accept any integer and clamp. `app.frame.set` keeps its stated range (spec A line 57).
- **Automatic extension (E-25):** `cell.clipboard.apply` holds the column's last drawing up to a block pasted past the
  last exposed frame when `autoExtend` is on, as `edit.pasteSpecial` does; it no longer warns that the option was not
  applied.
- **Rows shown (C5.1b):** ~~`timeline.rows.list` rows carry `shownInTimeline`~~; superseded by ruling E-37 (§13): the
  window and the interpreter apply one rule to `depth` and `collapsed`.

## 12. After round 2 (rulings E-30 to E-35)

- **Hit zones (E-35).** In a row of the tracks the zones are decided in this order, the first that holds wins, and
  every zone includes its edge pixels (`<=`): (1) a keyframe symbol in its band (a bar row: the whole row; a cell-based
  row: the upper half, outside the cell selection), `|x − centre| <= keyframeRadius` (5), the earliest keyed frame when
  two qualify; (2) the **end column**, `|x − end edge| <= endHandleWidth` (6): the loop handle in the upper half of a
  loopable bar, else the end handle; (3) the start handle, `|x − start edge| <= startHandleWidth` (5); (4) the body,
  strictly between the edges; (5) a cell. The upper half is `y_in_row < rowHeight / 2`. So a keyframe on the start frame
  wins over the start handle, the end column wins over the start handle on a short bar, and the end handle is 6 px
  either side because the end column is one zone shared by the end handle (lower half) and the loop handle (upper
  half): the constant was renamed for what it is (`loopHandleWidth` → `endHandleWidth`, `barHandleWidth` →
  `startHandleWidth`), and the reviewer's pinned pixels of `f2_pixel_zone_matrix` hold unchanged. In the ruler a free
  range's start marker wins over its end marker (`|x − centre| <= rangeMarkerHalfWidth`), else the ruler; in the layer
  list `y = k × rowHeight` is row k, and a drop is onto a parent-capable row when `dropZoneBetween < fraction <
  1 − dropZoneBetween`, else between (above when `fraction < 0.5`). The rule is in `timeline-layout.json`'s comment;
  `timing_gesture::zone_at` is the one function, and the shared case table's `zones` section (322 cases, written by
  `scripts/gen-timeline-zone-cases.py`) is checked by the kernel's unit test and by the window's Vitest.
- **Coordinates (E-31).** A gesture sample, or a layout's scroll position, beyond ±`COORDINATE_BOUND` (10 000 000 px)
  is `arg.invalid` at the window's entry and at `timing_replay`; `frame_at` and the row arithmetic saturate, so no input
  wraps.
- **The drag-and-drop set (E-32).** A cell drag's `cell.range.move` carries the set resolved at the press (`options`,
  04's names), so a change of the stored set while the drag is in progress does not change what is committed.
- **A paste over locked drawings (E-30).** `edit.pasteSpecial` with the exposure option off refuses over cells exposing
  locked drawings (`drawing.locked`), as `cell.clipboard.apply` and `cell.range.move` do.

## 13. After the closure check (rulings E-36 to E-38)

- **The plain paste and the drawing lock (E-36).** Plain `edit.paste` writes over the destination cells (E-17) and
  refuses with `drawing.locked` when a destination cell exposes a locked drawing, in both cell contexts, by the same
  check as the special form with the exposure option off (04 lines 1074 and 1123, ruling D-80). The check runs per
  column, before that column places anything; the local changes of earlier columns are discarded with the refusal and
  nothing reaches the document. Plain Cut and Delete are unchanged.
- **Rows shown (E-37).** Rows come in Timeline order with `depth` and `collapsed`; a row is shown unless one of its
  ancestors is collapsed (a collapsed row itself is shown). The interpreter hit-tests the rows `timing_gesture::shown_rows`
  keeps; the window draws the rows `desktop/src/timeline/rows.ts` keeps; both check every case of
  `crates/executor/src/timeline-rows-cases.json` (1,623 cases: every well-formed tree of up to five rows with every
  collapsed combination, longer hand-written trees; written by `scripts/gen-timeline-rows-cases.py`, whose oracle walks
  each row's ancestors, and copied byte for byte to the window). Effect-parameter rows are rows like any other until
  M7: `timeline.effectParameterRows.set` stores its setting (reported by the view notice) and hides nothing in the
  Timeline; the window binding is deferred to M7. `shownInTimeline` is removed from `timeline.rows.list` (ruling E-39
  authorised the one reviewer helper that read it, `c51a_r2_fixes::row`, to derive the rows by this rule); it stays on
  the list of spec A additions for M7's question.
- **The argument sweep (E-38).** Each member is varied on a valid base call: the other required members take their
  valid values (each enumerated value; the product of them up to 12 bases, else one base per value index), so a
  boundary value is refused only by its own member's schema, which the sweep asserts (every violation points at the
  varied member). Per numeric member the table counts the boundary values that reached the handler; the test fails on
  a numeric member of a callable command with none.
