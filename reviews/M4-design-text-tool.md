# M4 C4.4b design note 5: the Text tool

Status: before code (C4.4b). Plan `reviews/M4-C4.4-plan.md` §4.4, §5 (intro), §5.4, §5.5, §6 item 5, §7, §9 (ruling
D-40, answers 2, 3 and 7); rulings D-23, D-32, D-33, D-39, D-45 and D-57 (`reviews/M4-C4.3-plan.md` §15). Covers
in-canvas editing, glyph metrics, input methods, commit granularity and how the kernel coalesces a typing run, the Text
panel's two roles, prechecks and the tests. Requirements: 02 §3.7 lines 206-216, §7.6 lines 664-681, §7.6.1 lines
683-699, §12.1 lines 1064-1069, §13.3 lines 1199-1203 and 1229, §13.5 lines 1323-1328; 01 §8.4 lines 860-866; 11 §7.6
lines 348-365.

## 1. What exists today (read at `cebeb7a9`)

- **Commands** (`crates/executor/src/commands/draw_text.rs`, each equal to its spec A row in
  `crates/registry/src/inventory.rs` lines 1550-1578): `draw.text.add` (undo "New Text Object", precheck
  `precheck_add_content`); `draw.text.set` (undo "Typing", "Set Text Width" or "Move Text Object" chosen by
  `cx.set_label`, content before width before position, line 529; precheck `precheck_text_set`); `draw.textFormat.set`
  ("Text Property Modified"). `draw.textFormat.set` takes `start` and `length` (02 line 1325): there is no `range`
  parameter, and the plan's `{range}` means those two.
- `draw.text.add` without `format` uses `text::default_run`, not the Text panel defaults (its doc comment says it uses
  them; panels note §5 recorded the gap).
- **No command deletes a text object under "Delete Text Object"** (02 line 681): `draw.object.delete` labels every
  deletion "Delete Object" (`draw.rs:697`).
- **Layout** is the kernel's: `tt_vector::text::layout_placed` (M2 C2.3) places each visible character (index, origin
  on the baseline, size) in the box's local frame, and the canvas draws kernel glyph outlines (`draw_render.rs` `text`,
  `ops::glyphs::shapes`). **No query** in 02 §13 or spec A gives caret positions, line boxes or the character under a
  point: `draw.drawing.get` answers runs, paragraphs and bounds; `draw.hitTest.get` answers objects.
- **History** (`history.rs`): `History::push` records one step per transaction; nothing coalesces. A step's
  `StepEdit::Patches` may already hold several commits of one entry (an atomic batch), which `Cx::replay` merges per
  entry in commit order (`cx.rs` lines 1021-1031).
- **Gestures:** `text` is not in `gesture::COVERED`; `not_built` answers `command.notImplemented` and says no milestone
  names the Text tool, which plan §5.4 now does.
- **`type_text` does not exist:** `crates/mcp/src/tools.rs:661` answers `Plan::Unavailable`. The plan's headless path
  (`pointer` then `type_text`) needs C4.4b to build the part that types into the canvas (§4).
- **Keys:** with a text field focused the window leaves unmodified keys to the field (`windowHooks.ts` `useShortcuts`,
  `isTextField`) and reports `textFocus`, with which the kernel suspends unmodified keys for the `key` tool
  (`host.rs:396`). Primary shortcuts still reach the keymap.
- **D-57 came after D-40:** no registry command or event spec A does not declare. Answer 3's "new queries by SI and
  registry addition" cannot be built; §5 says what replaces it, and the query goes to the user as a proposed row.

## 2. The edit session and the gestures

The kernel owns the edit session: `crate::app::AppContext` gains `text_edit: Option<TextEdit>`, transient like the point
selection: `{text_id (qualified), view_id, origin, anchor, caret (code points), goal_x, pending: Format}`, one per
application. A new module `crates/executor/src/text_edit.rs` holds its transitions; the driver and the input path (§4)
call them. Opening the session, placing the caret and highlighting are not commands (no row exists; the class of
SI-155, like a Polyline's pending clicks); every document change is one of the commands below.

**Hit zones at the press** (view pixels through the press mapping, as `gesture::handle_at`), in this order: (1) the
edited object's two width handles, squares at the middle of its box's left and right edges, radius 6 px (SI-131's
handle radius); (2) the frame, a band 4 px either side of any text object's box outline; (3) inside a text box; (4)
anything else is empty canvas (strokes and zones do not count).

| Gesture (02 line) | Driver result (`gesture::to_commands`) | Label |
|---|---|---|
| click empty canvas (669) | end the session (below), then `draw.text.add {position: the click point, text: "", format: the Text panel defaults, colourId: the current colour, space: drawing, view}`; caret 0 | "New Text Object" |
| drag empty canvas, beyond `CLICK_SLOP_PX` (672) | as a click, with `position` = (min x, max y) of press and release in drawing units (the box's top-left, lines run down, `text.rs` header) and `width` = their x distance | "New Text Object" |
| click / drag / double-click inside a box (674) | caret at `text::caret_at`; anchor at the press, caret at the release; the word at the point | none |
| typing, Backspace, Delete, Return (669-670) | `draw.text.set {insert}` / `{delete}` / `{delete, insert}` through the input path (§3, §4) | "Typing", one per run |
| drag a width handle (672-673) | `draw.text.set {textId, width}` (right handle) or `{textId, width, position}` (left), width along the box's local x, at least 1 du | "Set Text Width" |
| drag the frame (676) | `draw.text.set {textId, position}`: the translation plus the drag (answer 7 interim; SI-194) | "Move Text Object" |
| Text panel change on highlighted text (677-678) | `draw.textFormat.set {textId, start, length, field}` (§6) | "Text Property Modified" |
| leave an empty object (681) | `draw.object.delete {ids: [textId]}` | "Delete Text Object" (SI-194) |

**Ending the session:** a click on empty canvas or on another text object, Escape, `tool.active.set` to another tool, a
change of current drawing or frame, the drawing locked, or the object removed by anyone. After every accepted
invocation `Core` calls `text_edit::settle`, which ends a session whose condition holds; ending runs
`draw.object.delete` when the object exists and is empty, with the origin of the call that ended it, as
`gesture::finish` runs the Dropper's second command. A click elsewhere is one gesture ending in two commands (the
deletion, then `draw.text.add`), which `to_commands` already allows. A drawing the first click created in an empty cell
stays when its empty text object is deleted (02 line 1069 joins the drawing to the creation step, not to the deletion).
The Select tool's double-click on a text object (02 line 434) sends `tool.active.set {toolId: "text"}` and opens the
session at the point.

**Preview:** a frame drag draws the occurrence translated (design note 8's `P`, exact); a width drag draws the box
outline at the new width and the text reflows at the commit (SI-154's rule: furniture exact, content at the commit).
Typing has no preview: glyphs arrive with the commit (§8 budget). `COVERED` gains `text`; `gesture-table.json` gets
`text` rows in both views (`draw.text.add`, `draw.text.set`, `draw.object.delete`); `deferred.json` loses the three
text entries (`draw.textFormat.set` is bound by the Text panel).

## 3. Commit granularity: the typing run (answer 2; SI-191)

Every keystroke changes the document: one `draw.text.set`, one revision, `doc.changed` and `draw.drawing.changed`, so
agents and renders see the text. The history holds one "Typing" step per contiguous run of the session's input path.

- **Where the run lives:** `History` gains `run: Option<TypingRun { step_id, text_id, origin }>`. The input path (§4)
  invokes a keystroke's commands inside `Core::set_typing(Some(TypingKey { text_id }))` and `set_typing(None)`, as
  `gesture::finish` brackets `set_label_override`; staging copies the key onto the `Txn`. `commit()` (`run.rs:638`)
  extends the top step instead of pushing a `TxnKind::Step` that carries a key when all hold: `run` is open, its
  `step_id` is the top step and the position is the end (nothing undone), same text object, same origin, and the
  position is not the clean position. Otherwise it pushes and opens a new run.
- **Extending** (`History::extend_top`): the step keeps its id and label; the commits are appended in commit order
  (an atomic batch's shape, which replay already handles); when the last commit and the new one are each one `replace`
  at the same pointer of the same entry, they fold into one commit (the first undo, the new redo), so a run of any
  length undoes with one patch per entry; `touched` is unioned; `time` is the last keystroke's. Every keystroke's
  envelope and `doc.changed` name the same `undoStepId`.
- **What ends the run** (`History::end_run`): a step pushed without the key, undo, redo, a checkpoint restore, a save
  (`mark_clean_at`), and from `text_edit`: a caret move (arrows, Home, End, a click), a highlight change, a format
  change (a step ends it by being pushed; a pending-format change ends it explicitly, §6), leaving the session, and a
  keystroke of another origin. **No timers:** the decision reads only the history and the key; nothing reads a clock.
- Backspace, Delete and Return act at the caret and continue the run. Typing over a highlight is one
  `draw.text.set {delete, insert}` (the handler deletes first) and begins a new run, the highlight having ended the
  last. Paste and Cut are one step each and end the run before and after.
- **A command is always its own step** (02 lines 1064-1065): `run_command draw.text.set` never extends a run; the run
  belongs to the input path, not to the command.
- **Three-way:** a person typing H, e, l, l, o sends five `draw.text.set {insert: {at: i, text: c}}`, one step; an
  agent's `type_text "Hello"` sends one `{insert: {at: 0, text: "Hello"}}`, one step; the commands `draw.text.add
  {text: ""}` and `draw.text.set {insert}` give the same history, ["New Text Object", "Typing"]. `Content::insert` grows
  the run before the caret, so one insert of n characters should equal n inserts; the kernel test asserts it.
- 02 §13.3 line 1229 pairs "click, then `type_text`" with one `draw.text.add {position, text}`, and lines 1199-1201 ask
  for one undo step with the same label, while §12.1 lines 1066-1067 make creating and typing separate steps. The
  gesture's equivalent is the two-command sequence (exact, labels included); the single `draw.text.add` is compared on
  the document only. SI-191 records it.

## 4. In-canvas editing

**One input path:** `text_edit::input(core, origin, Input)` on the document thread, `Input` = `Text(String)` |
`Key(Sequence)` | `Paste(String)` | `Cut`. Reached from (1) the window, by a host IPC `text_edit {viewId, generation,
input}` (as `gesture_commit`; another generation's is refused, nothing run, D-7b); (2) MCP `type_text`, now
`Plan::Server`: without `target` it types into the caller's session (headless or attached); with the editor's widget
path it is routed to the window that owns it, which feeds the same `text_edit` call (as attached `pointer` gestures are
played by the window); other widget targets keep today's unavailable answer, listed in the C4.4b handoff; (3) MCP `key`:
with a session of the caller's origin open, `key_tool` sends editing keys to the input path before the keymap. With no
session, `type_text` answers `arg.missing` (`/target`: no text is being edited); a session of another origin answers
`ui.busy` (11 line 491).

**Editing keys:** Left, Right, Up, Down, Home, End (Shift extends the highlight), Backspace, Delete, Enter (inserts
`\n`), Escape (ends the session), Primary+A (selects all the object's text, 02 line 675). Up and Down use the kernel
layout and keep `goal_x` until a horizontal move. Tab is consumed and inserts nothing; Escape is the way out.

**Word keys (01 §8.4; ruling D-81 item 4b).** Ctrl+Left and Ctrl+Right (Option+Left and Option+Right on macOS) move by
words, with Shift extending the highlight; Ctrl+Backspace and Ctrl+Delete (Option+Backspace and Option+Delete) delete
from the caret to the word boundary, or the highlight when there is one. On macOS Command with the arrows goes to the
line's start or end (Home, End). **What a word is:** the characters fall in three classes, word characters (a letter,
a digit, or `_`, by Unicode's alphabetic and numeric properties), white space, and everything else (punctuation and
symbols). Word left skips white space backwards, then the run of one class before it; word right skips white space,
then the run of one class after it. So a run of punctuation is a word of its own ("two, three": the comma is one word),
and a double click selects the run of one class under the point (`word_at`). Positions are code points, as everywhere
in the session (the grapheme question is answered in the C4.4b round 2 handoff).

**Double click (02 §7.6; finding C44B-R1-41).** The window sends its double click as a request of its own after the
second click's gesture (`GestureCollector.lone`), and an agent's `pointer` sends its two clicks then `doubleClick`; the
kernel's collector turns a `doubleClick` with the Text tool active and no gesture pending into a click marked double at
that place, and the session's anchor and caret go to the word's ends. No command, no undo step.

**A locked target (finding C44B-R1-42; rulings D-80, D-81).** A press that would open a session is refused like any
gesture on a target that cannot be edited there (`drawing.locked`; `element.locked` or `element.hidden` in a scene
view), and the window tells the person on the status line.

**Characters outside the basic plane (D-81 item 4a).** Synthesised key events can deliver such a character as two
`insertText` inputs of one UTF-16 unit each; the editor keeps a lone high surrogate until its low surrogate arrives and
sends the pair as one character (`joinSurrogates`). A half is never sent: an unpaired surrogate is dropped.

**Window** (`desktop/src/canvas/TextEditor.tsx`): a hidden `<textarea>` (`data-tt-role="textEditor"`, `data-tt-id`,
and the session's text id, anchor and caret as attributes, so `inspect_ui` shows the session), placed at the caret's
view position and focused while the session is in that view. `beforeinput` `insertText` sends `Text`;
`insertLineBreak` and `insertParagraph` send `Key Enter`; `deleteContentBackward` and `deleteContentForward` send their
keys; its keydown sends the other editing keys with `preventDefault`; `paste` sends `Paste`; `cut` writes the highlight
with `copyText` and sends `Cut`; `copy` only writes it (SI-180's single writer), the text sliced by code points from
`draw.drawing.get`. Its value is emptied after each input, so the browser never holds a second copy of the text.

**Input methods:** on `compositionstart` the textarea becomes visible at the caret, at the run's size in view pixels in
the system UI font; the platform draws the pre-edit with its underline and anchors the candidate window there. Nothing
is sent while composing; `compositionend` sends one `Text` with its data, which continues the run. The pre-edit is
furniture, not laid-out text; the committed characters are laid out by the kernel. Manual script
`reviews/manual/M4/C4.4b-text-input-method.md` (Microsoft IME Japanese and Pinyin on Windows, the Japanese and Pinyin
input sources on macOS).

**Focus and shortcuts (SI-193):** the textarea is a text field, so `useShortcuts` leaves it unmodified keys and
`useTextFocusReport` suspends them for agents' `key`. Its own keydown takes Primary+A, X, C and V with `preventDefault`,
which `useShortcuts` honours (`e.defaultPrevented`). Primary+Z and Shift+Primary+Z reach the keymap: the document's
undo and redo, which end the run. Other Primary and Alt shortcuts work (01 lines 862-864). The Edit menu's Cut, Copy,
Paste and Select All act on the editor while it has the focus. Losing the window's focus keeps the session.

**Drawing:** a steady caret (no blink, no idle frames), the highlight and the edited box's frame and handles, in both
views (D-32), from the scene answer in drawing units through each view's drawing-to-view mapping (in the Camera view
the current element's plane, as the Select handles, SI-156). The cursor (I-beam in a box, move over a frame, resize
over a handle) is picked from the same geometry, as `selection.ts` picks the Select cursors.

## 5. Glyph metrics (answer 3 and D-57; SI-192)

The kernel is the layout authority and the window derives no metric. New functions in `tt_vector::text`, from the
advances `layout_placed` already computes: `carets(c, width) -> Vec<Caret { index, line, x, top, bottom }>` (a stop at
every position 0..=n), `caret_at(c, width, local point)`, `line_move(c, width, caret, goal_x, dir)` and `word_at(c, i)`
(the maximal run of one class: letters, digits and underscore; white space; anything else). Clicks, drags,
double-clicks, Up and Down use them in `text_edit.rs`.

The window receives the geometry in the scene answer (`canvas_scene`, the host IPC that already carries the kernel's
selection geometry): `Scene.text_edit: Option<SceneTextEdit { text_id, box_quad, caret: [P; 2], highlight: Vec<[P; 4]>,
handles: [P; 2], size }>` and `text_boxes` (each text object's box in the current drawing while the Text tool is
active, for frames and cursors). The scene form becomes 1.2: `docs/canvas-format.md` §11.2 and §11.6 are amended first
(D-24; a 1.1 reader ignores the members). Session changes reach the Text panel by the host-only notice
`shell.textEdit {viewId, textId, anchor, caret, format}` (or `null`): never sent to an MCP client (tested), and carrying
nothing an agent cannot read through `inspect_ui` (the editor node) and `draw.drawing.get` (D-57). Proposed for
`plan/spec-issues.md` "Spec A additions proposed by M4, for the user": `draw.text.layout.get` | query | Caret stops and
line boxes of a text object | `textId: id; drawingId?: id` | `{lines: [{start, length, top, baseline, bottom}], carets:
[{index, x, top, bottom}], rev}` | n/a. **Without it:** the window draws from the scene answer; an agent places the
caret with `pointer` and `key` and reads the result in `draw.drawing.get`.

## 6. The Text panel's two roles (02 §7.6.1; plan §4.4)

- **No session: the defaults** (C4.4a, `tool.text.options.set`). The driver makes them explicit in `draw.text.add`'s
  `format` (02 line 1202), and `draw.text.add` without `format` reads them too: a declared behaviour change that makes
  the handler do what its comment says.
- **A session with a highlight:** each control commits `draw.textFormat.set {textId, start: min(anchor, caret), length:
  |caret - anchor|, field}` on release, Enter or click (the C4.1 conventions): one "Text Property Modified" step per
  commit, ending the run.
- **A session with a caret only (SI-195):** paragraph fields send `draw.textFormat.set {start: caret, length: 0}`, which
  formats the caret's paragraph (`Content::format`), one step. Character fields send the same command; the handler, when
  the call is the session's (same object, `start` at its caret, same origin), stores them in `TextEdit::pending` and
  changes no document, so no step. The next keystroke runs `draw.text.set {insert}` then
  `draw.textFormat.set {start: at, length: n}` with the pending fields, both in the run's step; a caret move clears
  `pending`. The current colour (02 line 679) is made explicit the same way when it differs from the colour at the
  caret.
- **Shown values:** the kernel computes `format` for `shell.textEdit`: per field, the value over the range (character
  fields over the runs it covers, paragraph fields over the paragraphs it touches, the sets `Content::format` changes),
  or `null` where they differ; at a caret, the run before it with `pending` applied (`Content::insert`'s rule). A mixed
  numeric field is empty with the placeholder "Mixed", a mixed toggle `aria-checked="mixed"`, a mixed choice has no
  option chosen. A heading says which role is shown ("Selected text" or "New text"). Re-read on `shell.textEdit` and
  on `draw.drawing.changed` for the drawing, so an agent's `draw.textFormat.set` shows (D-40).
- In the formatting role each control declares `data-tt-command="draw.textFormat.set"` and `data-tt-params` with
  `textId`, `start`, `length` and its field; its availability is read with those parameters (`useAvailability`).

## 7. Prechecks and refusals

- **`drawing.locked`:** the existing prechecks refuse `draw.text.add`, `draw.text.set` and `draw.textFormat.set`
  (`docs/availability-inventory.md` lines 676, 682, 691). A click creates nothing and the status line shows the refusal
  (gestures note §4); a session whose drawing gets locked ends without a deletion; the panel's formatting controls are
  disabled with the reason as tooltip.
- **No current drawing:** the first click in an empty cell creates one (`createDrawing` defaults to true; 02 line 1069);
  the driver never sends `createDrawing: false`, so `drawing.noTarget` (line 677) does not arise from the window. An
  element that is not a drawing refuses `drawing.notVectorElement` (line 678), shown in the status line.
- **Fonts (D-57, SI-169):** no refusal. System fonts are allowed and the product bundles none, so the font field's note
  marks every font. A missing font lays out with the fallback and warns `drawing.fontMissing` on every edit
  (`bounds_and_fonts`); the window shows it once per session, not per keystroke.
- No new precheck: `draw.object.delete`'s label is the handler's. The handlers' parameter refusals (an insertion past
  the end) cannot come from the input path, which clamps to the session.

## 8. Tests

- **Kernel** (`crates/executor/tests/c44b_text.rs`): one driver row per row of §2's table naming the 02 line, the
  command, the parameters and the label (D-23; answer 7's interim stated); each hit zone at its pixel boundary in both
  views. The run, as a matrix that fails if the rule is ignored: five keystrokes give one step, undo empties, redo
  restores; each ending (caret move, highlight, format, Escape, another origin, undo, save, an interleaved command)
  gives two steps; two `run_command draw.text.set` give two; after 1,000 keystrokes the step holds one commit per entry
  (a count). `type_text` equals five keystrokes (document and labels). "Delete Text Object" only when every removed
  object is an empty text object; a non-empty one stays "Delete Object". Pending format and current colour; each
  session ending; `ui.busy` and `arg.missing` for `type_text`.
- **Headless equivalence:** `pointer` then `type_text` (and `key` for Backspace, Delete, Enter) against the commands of
  §3, document and labels, with the window's view size where the row has one (D-32: exact).
- **Three-way through the real window** (the successor of `scripts/c43-gesture-equivalence.py`), Drawing and Camera
  views: click and type, drag-create, width, move, a format on a highlight, Escape on an empty object. The typing is
  real keystrokes in the workstation's OS form and `type_text` targeting the editor in CI's MCP form. Each row compares
  the window, the headless replay and the direct commands.
- **Parity:** the `text` rows of `gesture-table.json` checked both ways by `crates/executor/tests/c43/parity.rs`; the
  editor declares `draw.text.set`; the Text panel's controls declare `draw.textFormat.set`.
- **Availability:** the existing precheck pairs (lines 676-678, 682-683, 691-692) stay swept; new rows: a formatting
  control disabled on a locked drawing, the Text tool's click refused there.
- **D-39 rows the formatting role adds**, both forms: each control (font, size, bold, italic, baseline, kerning, auto
  kern, alignment, both margins, indent, line spacing, outline removal) over a highlight, each commit path (slider
  release, numeric entry, a toggle clicked twice), with parameters, effect, label and the locked refusal; the caret-only
  rows (no step; the next keystroke carries it); a range over two runs showing "Mixed"; an agent's change shown; the
  Edit menu's Cut, Copy and Paste while editing.
- **Vitest:** `TextEditor` (the input-type mapping, one send per composition, the value emptied, Primary+A/X/C/V
  prevented and Primary+Z passed), the panel's mixed display, `metrics.ts`.
- **Budgets (plan §5.5).** Keystroke to glyph drawn ≤ 33 ms p95: from `KeyboardEvent.timeStamp` to the frame that
  submits a settled model at the commit's revision with the caret moved (`metrics.ts` gains `keyToGlyph`, pure, fed
  numbers by Vitest), 30 OS keystrokes. Release-to-committed ≤ 50 ms p95 on the sample drawing and ≤ 150 ms on the
  50,000-point drawing for width and frame drags. Work per frame ≤ 8 ms p95 (D-33) over the frames drawn during those
  drags and during typing only (C43B-R1S-08), incomplete measurements failing. All are workstation receipts, release
  build, both back ends, regenerated at the handoff (D-45). CI asserts shape only: a keystroke on 10,000 characters
  costs at most 20 times one on 1,000 (layout is linear) under a gross ceiling, and the step count above.

## 9. Spec issues (proposed; for `plan/spec-issues.md` when the code lands)

- **SI-191, the typing run.** *Spec:* 02 §12.1 lines 1066-1067 make "a run of typing" one step without defining a run;
  §13.3 lines 1199-1201 and 1229 equate "click, then `type_text`" with one `draw.text.add` and one step. *Decided:* §3's
  rule (the input path's keystrokes on one object by one origin, ended by a caret move, a highlight, a format, leaving,
  undo, redo or a save; no timers; a command is always its own step); the Text row of §13.3 compares on the document,
  the labels against the two-command sequence. *Why:* answer 2, and §12.1 is the more specific rule.
- **SI-192, glyph metrics without a spec A query.** *Spec:* 02 §13 has no caret or line query; answer 3 asked for one
  by registry addition, which D-57 forbids. *Decided:* the kernel lays out and hit-tests (§5); the window gets the
  geometry in the scene answer (form 1.2) and the panel the session by `shell.textEdit`; `draw.text.layout.get` is
  proposed to the user. *Why:* the kernel stays the only layout, the window derives nothing, spec A stays the user's.
- **SI-193, keyboard focus while text is edited in a canvas.** *Spec:* 01 §8.4 lines 862-864 name text fields, numeric
  fields and rename editors, not text edited in a view; 11 line 356 gives `type_text` a widget target only. *Decided:*
  the editing session is a text field for §8.4 in the window and in the kernel's `key` routing; the editing keys of §4;
  Escape ends editing; Primary+Z is the document's undo; Cut, Copy, Paste and Select All act on the text; `type_text`
  without a target types into the caller's session. *Why:* 02 lines 669-675 and 680 need these keys and the headless
  path needs a target.
- **SI-194, the Text tool's ambiguous mappings** (answer 7). *Spec:* 02 lines 672-676 say "border" and "frame" without
  geometry; line 681 names "Delete Text Object", which no command carries. *Decided:* §2's hit zones; a frame drag sends
  `draw.text.set {position}` ("Move Text Object"); a width handle `{width}` or `{width, position}`; a click elsewhere
  ends editing and acts there; the deletion is `draw.object.delete`, labelled "Delete Text Object" by its handler when
  every removed object is an empty text object (a declared behaviour change, so an agent's deletion gets it too).
  *Why:* answer 7's interim, and a label set by the command keeps the three paths equal.
- **SI-195, "the next typed characters".** *Spec:* 02 lines 677-679: panel changes with nothing highlighted apply to
  the next typed characters, and text takes the current colour; the format model cannot hold a zero-length run.
  *Decided:* §6's pending format, set by `draw.textFormat.set {length: 0}` at the session's caret, applied by the next
  keystroke's commands inside its run; paragraph fields apply to the caret's paragraph. *Why:* no spec A change, and
  the same command serves the panel and agents.

## 10. Open questions for the orchestrator

1. Answer 3 asked for glyph-metric queries visible to agents; D-57 forbids rows spec A does not declare. This note
   builds none and proposes `draw.text.layout.get`; until the user adds it, a headless agent cannot read caret
   positions or the session (an attached one reads the session through `inspect_ui`). Confirm that this is acceptable
   for C4.4b's handoff.
2. 10 line 958 names the missing-font message `text.fontMissing` (info); the kernel warns `drawing.fontMissing`
   (`draw_text.rs` `bounds_and_fonts`, since M2). Which code is right, and is the rename C4.4b's?
