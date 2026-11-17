# M4 C4.4a design note 1: panels and settings

Status: before code (C4.4a). Plan `reviews/M4-C4.4-plan.md` §1 (D-40 additions), §2, §4.1-§4.5, §4.8-§4.10, §6 item 1,
§9 (ruling D-40). Covers the Pen panel, the tool-options page and its toggles, the Tablet preferences, the Text panel as
the defaults for new text, the drawing inspector, the toolbar customisation sheet, the camera list and the two views'
context menus. The Preferences dialog's Agent access category is with the other agent surfaces in design note 2
(`reviews/M4-design-agent-surfaces.md`, §2).

## 1. Conventions every surface here follows

- **One command layer.** Every control sends one registered command through `actions.run` (or `perform` where a dialog
  completes parameters). No control writes state locally; what a control shows comes from a kernel read and is re-read on
  the kernel's events (§2).
- **Commit on release, one undo step per commit** (the C4.1 conventions; 02 §6.1 line 376 for pens). A slider sends
  nothing while dragged and one command at release; a text field commits on Enter or blur; a list choice commits at once.
  Commands that are UI state (kind `ui` or `toggle`) make no undo step (02 §12.1).
- **Keyboard reach.** Every control is reachable with Tab and Shift+Tab in reading order, operable with Space or Enter,
  arrows in lists and sliders (Page Up and Down for coarse steps), Escape leaves an edit unchanged. Each panel's manual
  script (`reviews/manual/M4/C4.4a-keyboard.md`) walks it with the keyboard only.
- **Disabled rather than hidden.** A control whose command the kernel refuses now is shown disabled with the refusal's
  reason as its tooltip, from the shared availability store (`useAvailability`, ruling O-10, C43B-R1S-09). A control
  appears or goes only with its surface (a project, a selection that gives the inspector a drawing).
- **Declared references.** Each control declares the command and the parameters its activation sends
  (`data-tt-command`, `data-tt-params`), so `inspect_ui` states what it invokes and the D-39 table's MCP form can check
  it (D-40). A slider declares the reference of its current value.
- **Agent changes reflected** (D-40): every control shows the kernel's state after a change made by anyone; one Vitest per
  control drives the change through a kernel event only.
- **macOS.** The application menu holds Settings (01 §5.1); the secondary click is a right click or a Control-click (§9);
  key equivalents are the menu's. The macOS runner exercises every surface through the D-39 table's MCP form.

## 2. What each surface listens to

| Surface | Reads | Re-read on |
|---|---|---|
| Pen panel | `tool.pen.list`, `tool.settings.get` | `tool.pen.changed`, `tool.options.changed` (`toolId: null`), `doc.undone`, `doc.redone`, `project.opened`, `project.closed` |
| Tool-options page | `tool.options.get {toolId}`, `tool.settings.get`, `colour.gapClosing.get` | `tool.active.changed`, `tool.options.changed`, `tool.pen.changed` (`selected`), `prefs.changed`, the project events |
| Text panel (defaults) | `tool.options.get {toolId: text}`, the font list (§5) | `tool.options.changed {toolId: text}`, the project events |
| Drawing inspector | `draw.drawing.get`, `element.drawing.list`, `draw.drawing.render` (thumbnail) | `draw.drawing.changed`, `doc.changed`, `doc.undone`, `doc.redone`, `app.state.changed` (the current element or frame), `prefs.changed` |
| Toolbar sheet | `window.toolbar.list` | `workspace.changed` |
| Camera list | `anim.camera.list`, `anim.camera.current.get` | `anim.camera.currentChanged`, `doc.changed`, `doc.undone`, `doc.redone`, `app.state.changed` (the scene) |

Undo and redo emit only `doc.undone` and `doc.redone` (`edit.rs:63`): every panel showing document state listens to them
too, or an undone pen edit would stay on screen.

Two kernel gaps (§10, SIs before code): `colour.gapClosing.set` and `draw.inspectorPreview.set` stage a preference
through `Effect::Prefs`, which `run.rs:873-883` applies without an event, so an agent's change could not be reflected. The
fix is in the kernel: applying a staged preference emits `prefs.changed {keys, source}` as `prefs.set` does
(`prefs.rs:191-202`), and both panels re-read on it.

## 3. Pen panel (02 §6.1 lines 354-384, §6.2 lines 386-398)

- **Where:** the `pen` panel (01 §5.2) and the Pen tab of Properties (the same component). It replaces the placeholder
  (`PanelBody.tsx:17-30`; its owner string in `catalogue.ts:28` still names M4 C4.2 and is corrected).
- **List.** The pens of the active tool's type (Brush or Eraser; switching the active tool switches the list) from
  `tool.pen.list {type}`; the current pen pressed. Row click: `tool.pen.select {penId}` (ui, no step). New:
  `tool.pen.create {type, fromPenId: current}` ("New Pen"). Delete: `tool.pen.delete {penId}` ("Delete Pen"; the last
  pen of a type is refused `pen.lastPen`, shown disabled with that reason). Double-click or F2 on a row: in-place rename,
  committed with `tool.pen.rename {penId, name}` on Enter or blur ("Rename Pen"; `pen.nameExists` shown under the field;
  Escape restores the name; a name equal to the old one sends nothing).
- **Settings.** Tip shape (round, flat, as `tipShape`), minimum and maximum size 0-100, smoothness 0-10, tip angle 0-360:
  each a slider with a numeric field, committed on release or field commit with one `tool.pen.set {penId, ...}` ("Set
  Pen Style"). The push rule (02 line 378: a minimum set above the maximum raises the maximum, and the reverse) is the
  kernel's (`apply_settings`, `tool.rs:976-1029`); the panel shows the answer's pen, never its own arithmetic.
- **Preview.** A stroke drawn from the pen's values on a small canvas (02 §13.2 lines 1182-1183: no render call): the
  brush outline the vector engine would produce is not needed for a preview; a pressure ramp from minimum to maximum
  size with the tip shape and angle, drawn in the panel.
- **Advanced options** (hidden by default, a disclosure, `disclose`): symmetry (`tool.symmetry.set`, already bound in
  the view chrome; the same reference here) and Draw Behind (`tool.placeBehind.set {enabled}`, toggle).
- **Labels (D-23):** "New Pen", "Delete Pen", "Rename Pen" (02 §12.1 line 1068), "Set Pen Style" (02 line 376); select,
  symmetry and Draw Behind make none.
- **Open (SI):** which pen is current after the current pen is deleted. The kernel clears the slot and the default pen
  takes over without a `selected` event (`tool.rs:1140`); the SI fixes it as "the next pen in the list, else the
  previous", with a `selected` event, so the panel and the canvas cursor follow.
- **Inventory:** `tool.pen.set` and `tool.pen.create` can return `tool.valueOutOfRange` and `arg.invalid` from
  `apply_settings`, which the inventory does not list (the scanner misses codes returned as tuples): the rows are added
  and the scanner's gap is fixed with a fixture.

## 4. Tool-options page and the persistent toggles (02 §7.7 lines 701-720, §8.8-§8.11 lines 786-810, §12.3)

- **Where:** Properties (01 §5.2 `properties`), page "Tool options", following the active tool (`tool.active.changed`).
- **Per tool:** Select: rectangle rule `tool.select.options.set {rectRule: inside | intersect}`. Brush and Eraser: the
  pen (a list of that type's pens) `tool.brush.options.set {penId}` / `tool.eraser.options.set {penId}`. Pencil,
  Rectangle, Ellipse, Line, Polyline: line thickness source and value `tool.pencil.options.set {thicknessSource,
  thickness}` (the kernel writes the five tools together, `tool.rs:666-669`); Polyline also snap and snap radius
  `tool.polyline.options.set`. Text: §5. Other tools: their generic options through `tool.options.set {toolId, options}`
  (the kernel checks all values first, all or nothing).
- **Toggles** (each a check button, kind toggle, no step): Draw on Top Layer `tool.drawTopLayer.set`, Draw Behind
  `tool.placeBehind.set` (Draw Behind wins when both are on, 02 line 810: the page shows Draw on Top Layer disabled with
  that reason while Draw Behind is on), Snap to Contour `tool.snapToContour.set`.
- **Gap closing** (03 §8.8 lines 678-697) on the Paint, Unpaint and Paint Unpainted pages: a list off, small, medium,
  large sending `colour.gapClosing.set {mode}`, reading `colour.gapClosing.get` (which becomes a declared read and leaves
  agent-only, D-40 answer 5). 03 line 689 also requires it "from the main menu": a Colour › Auto Close Gap submenu bound to
  the same command (four radio items), with the D-39 rows of both places. `prefs.set {paint.gapClosing}` writes the same
  preference: one state, two commands, both emitting `prefs.changed` after §2's fix (an SI records it); `prefs.json:65`'s
  stale command name `colour.gap.mode.set` is corrected.
- **Persistence** is the kernel's (02 lines 1111-1113): the page has none. The e2e restarts the application and finds the
  options as set.

## 5. Tablet preferences and the Text panel's defaults

**Tablet (01 §7.3 lines 743-749; 02 §6.3 lines 400-407; D-40 answer 1).** The Preferences dialog's Tablet category edits
the `tablet.*` preference keys through `prefs.set` at OK, as every category but Agent access does. `tool.tablet.set` is
declared agent-only as the structured equivalent. Today the two write different states: `tool.tablet.set` writes
`DrawingSettings.tablet` and emits `tool.options.changed`; `prefs.set` writes the keys and emits `prefs.changed`. The fix
makes one state: `tool.tablet.set` stages the `tablet.*` keys (as `window.toolbar.set` stages its key, SI-114) and emits
`prefs.changed` like `prefs.set`; `DrawingSettings.tablet` is read from the keys. An SI records the overlap and the key
mapping, which also settles a spec mismatch: 01 lists three keys (`tablet.ignore`, `tablet.alternativeInput` bool,
`tablet.portraitCorrection`), 02 four options (with "Compatibility tablet handling" and "Tablet input interface", a choice).
Proposal: keys `tablet.ignore`, `tablet.portraitCorrection`, `tablet.compatibilityMode` (bool, new) and
`tablet.inputInterface` (a choice, `default | wintab | windowsInk` on Windows, `default` on macOS, replacing the boolean
`alternativeInput`), each "restart" where 01 says so. The kernel test shows both commands write the same state and raise
the same event. No consumer of the tablet state exists yet (the canvas sends raw pressure, `input.ts:112-114`); that stays
so in M4.

**Text panel as the defaults (02 §7.6.1 lines 683-699).** With no text being edited, the `text` panel edits the defaults
for new text through `tool.text.options.set` (font, size 1-512 in steps of 2, bold, italic, baseline, kerning, auto kern,
alignment, margins, indent, line spacing, outline colour; `null` clears the outline). C4.4b's Text tool gives the panel
its second role (`draw.textFormat.set` on highlighted text) and makes `draw.text.add` read the defaults, which it does not
today. **Fonts (D-40 answer 3):** no query lists fonts or gives glyph metrics (02 §13.5, spec A, the registry). M2 decided
the source (ADR-0016 lines 112-117): system fonts listed with `fontdb`, metrics with `skrifa`, a missing font falling back
to the platform's sans-serif with `drawing.fontMissing`; `tt_vector::fonts::installed_families()` exists and no command
exposes it. Proposal (SI and registry addition before code): `draw.font.list` (query, read scope, visible to agents)
answering `{families: [{name, styles: [regular, bold, italic, boldItalic present]}], default}` from that function; C4.4b
adds the metrics query. **For the orchestrator:** ADR-0016 line 39 makes layout depend on the installed font files, so a
font present on one OS and not the other lays out differently; D-40 asks for rendering that stays deterministic across the
two OSes. Either the font list is restricted to fonts bundled with the application (deterministic, smaller choice), or the
list is the system's and determinism holds only for the bundled default (as today). The note recommends the second, with
the panel marking fonts that are not bundled, and asks for the ruling.

## 6. Drawing inspector (02 §9 lines 858-872)

- **Where:** Properties, page "Drawing", when a drawing is the subject: until M5, the current element's current drawing
  (D-40 answer 4, an SI), from `app.state.get`'s current element and frame; nothing shown when the current cell is
  empty.
- **Contents:** element name and exposure (read-only), the drawing's name (a field committed with
  `draw.drawing.rename {drawingId, name}` on Enter or blur, "Rename Drawing"; `drawing.nameInvalid` and
  `drawing.nameExists` under the field), Show preview (`draw.inspectorPreview.set {visible}`, a preference, no step), the
  thumbnail (`draw.drawing.render {width: 160, region: bounds, background}` when shown), and the Browse slider over the
  element's drawings, which reads `element.drawing.list` (today agent-only: it becomes the inspector's declared read,
  D-40 answer 5) and renders the thumbnail of the drawing at the slider's position without changing state.
- **Two commands rename a drawing:** `draw.drawing.rename` and `element.drawing.rename` (`element_bank.rs:416`, the same
  label). The inspector uses `draw.drawing.rename` (02 §9's command); the SI records the overlap.
- **Refusals:** `drawing.noTarget` (precheck) disables the name field and the thumbnail with the reason.

## 7. Toolbar customisation sheet (01 §5.7 lines 445-464)

- **Where:** Window › Customize Toolbar…, and the main toolbar's context menu (§9). A sheet with two lists: the toolbar's
  current items (`window.toolbar.list`, main toolbar) and the allowed items (its `allowedItems`: the group items, spaces,
  separators and every non-event command), with a filter field.
- **Editing:** drag from the allowed list or press Add to insert after the selection; drag within the current list or
  Move Up and Move Down (Alt+Up and Alt+Down) to reorder; Remove or Delete; Restore Default sends
  `window.toolbar.items.reset` (already bound in the menu). Edits are staged in the sheet; Apply sends one
  `window.toolbar.items.set {toolbarId: main, items}` (no step: workspace state); Cancel discards.
- **Open (SI):** 01 line 463 allows an item to be a command id "plus optional fixed parameters"; the items are strings
  and cannot carry parameters. The SI proposes leaving references with parameters out of the sheet in M4 (no default
  item needs one) and records the item format for a later checkpoint.

## 8. Camera list (01 §5.7 lines 453-460)

- **Where:** the main toolbar's `cameraList` item, a list built like the workspace list (`Toolbars.tsx`), replacing its
  placeholder.
- **Contents:** the current scene's cameras (`anim.camera.list`), the current one selected (`anim.camera.current.get`);
  a choice sends `anim.camera.current.set {cameraId}`. Disabled without a project or without a camera, with the reason.
- **Undo (D-40 answer 10):** the command keeps its registered behaviour, undoable (05 line 1282: "yes"). 05 gives no
  label; the registry's "Set Current Camera" is recorded by SI before the D-39 row states it (D-23).
- The three commands' `gui` strings still name M6 C6.2 and are corrected; the list's refusals are parameter class
  (`anim.notFound`), not prechecks.

## 9. The views' context menus (01 §5.3 line 373; 02 §4.4 lines 291-296; 05 §3.2 lines 326-328; D-40 answer 12)

- **One mechanism** (`desktop/src/shell/ContextMenu.tsx`): opened at the pointer by a secondary click, or by the
  context-menu key or Shift+F10 at the focused control; a `role="menu"` of `role="menuitem"` (and `menuitemcheckbox`
  with `aria-checked`) items; arrows move, Home and End jump, Enter or Space activate, Escape or a click outside closes and
  returns focus; disabled items are shown disabled with the reason; each item is a command reference with the main
  menu's availability. The dock tab's existing checklist (`DockArea.tsx:100-129`, which closes only on mouse leave, has no
  keyboard and no check marks despite 01 lines 387-388) moves to the same component, with check marks and the panel's
  visibility toggled (its reference completed by the kernel like a toggle).
- **Drawing family** (the Drawing view, and the Camera view while a drawing tool is active, 02 line 293): Cut, Copy,
  Paste, Select All, Deselect All; Arrange (four); Transform (Flip Horizontal, Flip Vertical, Rotate 90° CW, 90° CCW,
  180°); Group, Ungroup; Flatten, Optimize, Smooth, Extract Centre Line, Convert Lines to Brush; Break Text Apart, each
  with the command its main-menu item sends. Feather Edges has no item until its owner implements the command (D-40 answer
  11).
- **Scene family** (the Camera view with a scene tool): Cut, Copy, Paste, Select All, Deselect All (01 §6) only, sending
  `edit.cut`, `edit.copy`, `edit.paste`, `edit.selectAll`, `edit.deselectAll` as the Edit menu does (bound today; the
  kernel picks the target from the view's context). The items of 05 §3.2 lines 326-328 are M6 C6.1's (D-30) and keep
  their `deferred` owners (M6 C6.1, as the Element menu's Select Parent / Children / Sibling entry already states); the
  merge note says the menu grows in M6. Each item's row in the D-39 table names its command and owner.
- **The canvas and the secondary click.** Today a right press is a tool gesture (`input.ts:71`, `:137-141`;
  `CanvasView.tsx:757-776`) and the kernel's `Button::Right` is never told apart. The window stops passing secondary presses
  to the gesture collector: a right button press opens the menu at release without a drag; a right drag does nothing (no
  tool maps it in 02 §13.3). On macOS a Control-click is the secondary click, but Control (or Command) is also taken as the
  temporary-select modifier (`CanvasView.tsx:784`): the SI proposes that on macOS the temporary-select modifier is Command
  alone and Control-click is the context menu, as macOS applications behave.

## 10. Spec issues raised before code (numbers from SI-163)

1. Two commands, one state: `tool.tablet.set` and `prefs.set` on `tablet.*`, the key mapping and the 01/02 mismatch (§5).
2. `colour.gapClosing.set` and `prefs.set {paint.gapClosing}`; a staged preference emits `prefs.changed` (§2, §4).
3. The current pen after the current pen is deleted (§3).
4. The drawing inspector's subject before M5 (D-40 answer 4) and the two rename commands (§6).
5. `draw.font.list` (registry addition) and the font determinism question (§5).
6. Toolbar items with fixed parameters (§7).
7. "Set Current Camera" as the label of `anim.camera.current.set` (§8).
8. The secondary click on the canvas and the macOS temporary-select modifier (§9).

## 11. Tests and the D-39 rows

- **Vitest per surface:** every control sends its declared reference; sliders send nothing until release; each control
  shows the kernel's state after an event alone (agent change); disabled with the reason when the kernel refuses.
- **Kernel:** the pen push rule and one step per commit; the tablet overlap (same state, same event from both commands);
  `prefs.changed` after a staged preference; `draw.font.list`; the deleted current pen's successor with its event.
- **D-39 rows, both forms** (`scripts/c43-d39-bindings.py`'s rows extended or a C4.4 table): each Pen panel control
  (select, New, Delete, rename, each setting at release, the advanced options); each tool's options and the three toggles
  (Draw on Top Layer and Draw Behind each clicked twice); gap closing in the page and in the menu; each Tablet key; each
  Text panel default; the inspector's rename, Show preview (twice) and Browse; the toolbar sheet's Apply and Restore
  Default; the camera list's choice; each context-menu item in each family and view (the dock tab menu's items too).
  Each row: the action, the command and parameters observed, the effect, the spec's label where there is a step, the
  disabled state without a project. Rows that need real input (a drag in the toolbar sheet, the secondary click on macOS)
  are named with the reason if the MCP form cannot perform them.
- **e2e:** options and a customised toolbar survive a restart; a pen edit changes the next stroke's width, not existing
  strokes (02 line 376).

## 12. Budgets

A committed edit's redraw of its panel within one frame of its answer (work per frame p95 at most 8 ms, D-33); the
inspector's thumbnail rendered off the document thread's critical path (a query answered within the availability
budgets of O-23, measured in the workstation receipt).
