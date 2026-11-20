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
- **One numeric field (ruling D-61, C44A-R1-20, -21).** Every numeric control is `NumericField.tsx`: it parses
  strictly (an empty or non-numeric entry is never a number: nothing is sent and the field returns to the retained
  value), refuses a value outside the command's declared range with the reason shown, commits on Enter and focus loss,
  restores on Escape, shows a command's refusal on the field, and always displays the kernel's (or the form's staged)
  value. A draft belongs to the value it was typed from, so a change made elsewhere shows at once.
- **Keyboard reach.** Every control is reachable with Tab and Shift+Tab in reading order, operable with Space or Enter,
  arrows in lists and sliders (Page Up and Down for coarse steps), Escape leaves an edit unchanged. Each panel's manual
  script (`reviews/manual/M4/C4.4a-keyboard.md`) walks it with the keyboard only.
- **A text field keeps its editing keys (ruling D-66 item 2, C44A-R2-01; 01 §8.4).** With the focus in any text-entry
  element (the numeric field, a text input, a text area, an inline rename, anything editable), the platform's
  text-editing keys belong to the field and never reach the keymap: select all, cut, copy, paste, undo and redo of the
  field's own text, word and line navigation and selection, word deletion, Home, End, Delete, Backspace, and plain
  characters, tool letters included; a character typed with AltGr (or Option on macOS) too. Escape and Enter keep the
  field's own rules. Other chords (Save) still reach the keymap; inside a dialog none does (D-49 item 3). The decision is
  made in one place, `fieldOwnsKey` in the key dispatcher (`windowHooks.ts`), ahead of the dialog guard, which had also
  cancelled Ctrl+A in the Preferences fields. Tests: `textFieldKeys.test.tsx` (the rule over every key and element kind
  on both platforms; each key in each text-entry kind and in every text field the main window renders, its default kept
  and nothing sent; Save still dispatched; the Preferences numeric fields inside the modal), and the D-39 rows "Pen panel
  › Maximum size", "Pen panel › rename" and "Preferences › search" (text-editing keys): the real Ctrl+A selects the
  whole text and a typed key replaces it, the real B types "b", the real Ctrl+Z leaves the document's history where it
  was, nothing is sent.
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
list is the system's and determinism holds only for a bundled font (correction, D-57: the product bundles none today; the
default lays out with the platform's sans-serif face, SI-169). The note recommends the second, with
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
- **Inside the client area (ruling D-60 item 1, C44A-R1-01).** A context menu is always fully inside the window's client
  area. `placeMenu` (`contextMenuModel.ts`) places it from its natural size, measured in the layout pass before the first
  paint: on each axis it opens from the pointer toward the side with room, flips to end at the pointer when it would cross
  the far edge, and shifts inside when neither side has room, keeping a 4 px margin. A menu taller than the client area
  starts at the top margin and scrolls within the client area's height, with an arrow at each end (dimmed when there is
  nothing more that way) that scrolls about three items per press; the wheel scrolls it, and the keyboard's focus brings
  each item into view. The arrows are not items (no role, `aria-hidden`). A wider menu is bounded the same way. The menu
  is placed again when the window's size changes. Submenus, when a menu has one, are placed by the same function from
  their parent item (none of today's menus has a submenu). Tests: `contextMenuPlacement.test.tsx` (the four corners, the
  reported 499,454 case, a menu taller and one wider than the window, the component's placement and arrows), and the D-39
  row "context menus › inside the client area at 1280 by 800" (both views' menus opened where they would cross an edge,
  every item's box inside the client area, the last item activated by a real click).
- **At every window size down to the minimum (ruling D-66 item 1, C44A-R1-01 at 800 by 500).** The live run at an 800 by
  500 client area saw a menu 502 px tall from top 4, its lower arrow at 501, an arrow click that did not scroll and the
  keyboard's focus moving onto items below the client area. Two causes: the placement's bound was applied as the content
  box's `max-height`, so the menu's padding and border (10 px) fell outside it; and the menu was measured after every
  render with its bound removed for a moment, so the items stopped overflowing and the browser dropped their scroll
  position: every scroll (arrow, wheel, the focus) re-rendered the arrows and jumped back to the top. Now the menu is
  sized `border-box` (the bound is the outer box's, arrows inside; it has no margin), the items' scroll position is kept
  across the measurement and a scroll that changes nothing re-renders nothing, and a bounded width overrides the menu's
  180 px least width (a float's client area can be 160 px wide). An arrow scrolls about three items on a press and, after
  250 ms resting on it (or held), 12 px every 40 ms until the end or until the pointer leaves; the keyboard's focus (the
  arrows, Home, End) scrolls the focused item fully into view; the wheel scrolls the items. The least client areas are the
  main window's 800 by 500 (`tauri.conf.json` `minWidth`, `minHeight`: the host enforces them as the inner size) and a
  float's 160 by 120 (`floats.rs` `min_inner_size`). Tests: `contextMenuPlacement.test.tsx` (every corner and edge
  placement at both least sizes; the component at both, the outer box and both arrows inside; an arrow press scrolls and
  the position survives a new measurement, the wheel's too; resting on an arrow scrolls to the last item and stops when
  the pointer leaves; the keyboard's focus keeps each item fully in view, the browser's loss of the position on an
  unbounded measurement reproduced), and the D-39 row "context menus › inside the client area at 1280 by 800 and at the
  minimum window size" (the window asked for 320 by 240 and kept at its minimum; both views' menus; the outer box and
  arrows inside; End and Home by the real keyboard, a real click on the lower arrow, the real wheel, the pointer resting
  on the arrow; the last item clicked by a real click after that real scrolling, sending its declared reference once).
- **No browser chrome in the product (ruling D-70 item 2).** The webview's own context menu opens only on text-entry
  elements, where it offers text editing (cut, copy, paste, select all, undo, redo, and what the platform adds for
  text); a window-level `contextmenu` listener, in the bubbling phase after the application's own menus, cancels the
  default everywhere else (`useNoBrowserMenus`, `windowHooks.ts`), so a secondary click opens the application's menu or
  nothing and no Back, Forward, Reload, Save as, Print, Inspect or View source entry is ever shown. The person cannot
  reload or navigate the page: F5, Shift+F5, Ctrl+F5, Ctrl+R, Ctrl+Shift+R, Alt+Left, Alt+Right and Backspace outside a
  field (and Cmd+R, Cmd+Shift+R, Cmd+[ and Cmd+] on macOS) are cancelled by the accelerator guard unless the keymap
  binds them, in a text field too (they are not text editing, D-69 item 2), and the browser's own keys (BrowserBack,
  BrowserForward, BrowserRefresh, BrowserHome, BrowserStop) and the mouse's Back and Forward buttons are cancelled
  everywhere. **An engine restart does not reload the page:** `restart_engine` starts a new engine process behind the
  same window; the page stays (its state, the token controller, any marker), learns the new generation
  (`GENERATION_EVENT`), resyncs its mirror from the new engine and reports its facts again (ruling O-9, O-14). Tests:
  `browserChrome.test.tsx` (a text field keeps the default; a toolbar button, the toolbar, a Pen panel row, the canvas,
  empty panel space and the page cancel it; every reload and navigation key cancelled in the token field, a text field,
  on the canvas and on the page) and the D-39 row "browser chrome › no native menu, no reload" (real right-clicks on a
  toolbar button and a Pen panel row add no visible top-level window and their defaults are cancelled; real F5 and
  Ctrl+R with the focus on the canvas and in the token field leave a marker set in the page in place).
- **Shift+F10 (ruling D-66 item 3).** The dispatcher's function-key guard (which keeps the page's own F-key accelerators
  off) cancelled the browser's `contextmenu` for Shift+F10, so only the context-menu key opened a menu. Shift+F10 now sends
  the same `contextmenu` event to the focused control (no pointer position: the menu opens at the control), in the
  dispatcher, before the keymap; in a text field the field keeps it (its own menu). Test: `contextMenu.test.tsx` (the
  dock tab's checklist and the Camera view's menu opened by Shift+F10, the key consumed, nothing sent).
- **Each family's edit items name their selection context (ruling D-60 item 2, C44A-R1-02).** 01 §6.4 keeps a selection
  per context, and the Camera view's focus says nothing about which family's menu was opened. The edit items therefore
  send the commands' own `context` parameter (spec A: `auto|drawing|staging|...`) and never rely on focus:

  | Family | View | Command | Parameters | The selection that changes (the other is left as it was) |
  |---|---|---|---|---|
  | Drawing | Drawing | `edit.cut` / `edit.copy` / `edit.paste` | `{context: "drawing"}` | the current drawing's (cut removes, paste adds; copy fills the clipboard) |
  | Drawing | Drawing | `edit.selectAll` / `edit.deselectAll` | `{context: "drawing"}` | the current drawing's |
  | Drawing | Camera (a drawing tool) | `edit.cut` / `edit.copy` / `edit.paste` | `{context: "drawing"}` | the current drawing's; the staging selection unchanged |
  | Drawing | Camera (a drawing tool) | `edit.selectAll` / `edit.deselectAll` | `{context: "drawing"}` | the current drawing's; the staging selection unchanged |
  | Scene | Camera (a scene tool) | `edit.cut` / `edit.copy` / `edit.paste` | `{context: "staging"}` | the staging selection (elements) |
  | Scene | Camera (a scene tool) | `edit.selectAll` / `edit.deselectAll` | `{context: "staging"}` | the staging selection |

  The Drawing view has no scene family. The Drawing family's other items (`draw.*`) act on the current drawing's
  selection already. A refusal names the context the item named (Cut with nothing selected in the drawing names the
  drawing, never staging). Tests: the Vitest "each family's edit items name its selection context, in either view" and
  the D-39 rows "Camera view context menu (drawing tool) › Cut, Copy, Paste, Select All, Deselect All" (each with a
  staging selection it must leave unchanged), beside the existing Drawing view and scene-family rows, which now expect
  the explicit context.

## 10. Spec issues raised before code (numbers from SI-165: SI-163 and SI-164 were taken by D-51 and D-53)

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

## 13. Open Recent's full path (01 §3.3 line 190; D-48 item 8; the first item of C4.4a)

01 §3.3: "Each menu entry shows the project name; the full path is shown in the status bar or tooltip." The start
screen's entries show it as a tooltip; the native Open Recent menu did not. Tauri's menu API gives a native item
neither a tooltip nor a highlight event, so the host adds a little platform code.

- **One model.** Each Open Recent node of the menu model (`menu_model.rs`, D-38) carries `hint`: the entry's full path.
  The window tree's menu bar reports it, so the D-39 MCP form reads what the host shows, on both OSes.
- **Windows: the status bar.** The host subclasses the main window (`SetWindowSubclass`) and watches `WM_MENUSELECT`.
  A highlighted item of the menu whose leading items are exactly the current Open Recent labels, at position `i`, is
  entry `i` (`dyn:recent:i` is inserted at position `i`): the host emits `menu-hint {text: path}`. Any other highlighted
  item, and the menu closing (`flags 0xFFFF`, no menu), emit `menu-hint {text: null}`. Tauri does not expose the menu
  bar's own handle of the submenu (only the popup copy's), hence the label match.
- **The status bar.** While a hint is set, the left line shows it; when it clears, the left line shows its status line
  again (a menu hint never replaces the last status message). The front end listens through the transport layer
  (`hostApi.onMenuHint`), as for host notices.
- **macOS: the tooltip.** After each rebuild of Open Recent the host sets each entry's `NSMenuItem` tool tip to its path
  (found under the application's main menu by the same label match). The macOS runner checks the model's `hint` in the
  MCP form; the native tool tip is a manual script line (`reviews/manual/M4/`).
- **D-39 row.** OS form (Windows workstation): real keys open File › Open Recent and highlight the first entry; the
  window tree's `status.line` is its full path; Escape twice closes the menu and the line returns to what it was. MCP
  form (CI, both OSes): the menu bar's Open Recent node `dyn:recent:0` has `hint` equal to `project.recent.list`'s path.
- **No command, no state:** a highlight changes nothing in the document or the application; the parity gate is
  unchanged (the entry's binding is `project.open {path}`, already declared).
