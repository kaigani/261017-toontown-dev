# Appendix A — Command registry

Every command of the application, compiled from the "Agent interface" section of each area specification (sections 01–11). The command layer is shared by the GUI and the MCP agent interface: every menu item, tool, dialog, shortcut target and panel action invokes one of these commands, and every command is callable by an agent through `run_command` / `batch` (see `11_agent_interface_mcp.md`). The area sections are normative for behaviour; this appendix is the index.

Kinds: `action` undoable document change · `toggle` boolean state set explicitly · `query` read-only · `job` long-running, returns a job id · `ui` UI/view state only · `event` notification (not callable). Parameters: `name: type = default (range)`, `?` = optional.

Total: 796 entries.

## Contents

- [`app`](#namespace-app) — 10 entries, section 01
- [`project`](#namespace-project) — 21 entries, section 01
- [`edit`](#namespace-edit) — 20 entries, section 01
- [`view`](#namespace-view) — 9 entries, section 01
- [`window`](#namespace-window) — 14 entries, section 01
- [`workspace`](#namespace-workspace) — 12 entries, section 01
- [`prefs`](#namespace-prefs) — 5 entries, section 01
- [`keymap`](#namespace-keymap) — 8 entries, section 01
- [`ui`](#namespace-ui) — 12 entries, section 01
- [`draw`](#namespace-draw) — 51 entries, section 02
- [`tool`](#namespace-tool) — 27 entries, section 02
- [`drawAid`](#namespace-drawaid) — 17 entries, section 02
- [`colour`](#namespace-colour) — 51 entries, section 03
- [`element`](#namespace-element) — 40 entries, section 04
- [`cell`](#namespace-cell) — 20 entries, section 04
- [`timeline`](#namespace-timeline) — 16 entries, section 04
- [`xsheet`](#namespace-xsheet) — 12 entries, section 04
- [`anim`](#namespace-anim) — 101 entries, section 05
- [`fx`](#namespace-fx) — 61 entries, section 06
- [`sound`](#namespace-sound) — 40 entries, section 07
- [`lipsync`](#namespace-lipsync) — 20 entries, section 07
- [`media`](#namespace-media) — 14 entries, section 07
- [`library`](#namespace-library) — 46 entries, section 08
- [`storyline`](#namespace-storyline) — 31 entries, section 08
- [`playback`](#namespace-playback) — 17 entries, section 09
- [`render`](#namespace-render) — 3 entries, section 09
- [`import`](#namespace-import) — 11 entries, section 09
- [`export`](#namespace-export) — 18 entries, section 09
- [`capture`](#namespace-capture) — 25 entries, section 09
- [`print`](#namespace-print) — 7 entries, section 09
- [`doc`](#namespace-doc) — 26 entries, section 10
- [`agent`](#namespace-agent) — 16 entries, section 11
- [`inspect`](#namespace-inspect) — 6 entries, section 11
- [`checkpoint`](#namespace-checkpoint) — 4 entries, section 11
- [`job`](#namespace-job) — 5 entries, section 11

## Namespace `app`

Defined in [`01_application_shell.md`](01_application_shell.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `app.context.set` | ui | Set several parts of the current context at once | `sceneId?: id; frame?: int; elementId?: id; drawingId?: id` | `AppState` | no |
| `app.currentElement.move` | ui | Previous/next element (Timeline order, exposure-sheet order, or by mode) | `direction: enum "previous"\|"next"; order: enum "auto"\|"timeline"\|"xsheet" = "auto"` | `{elementId}` | no |
| `app.currentElement.set` | ui | Make an element current | `elementId: id` | `{elementId}` | no |
| `app.frame.move` | ui | Previous/next frame, or previous/next different drawing in the current element | `direction: enum "previous"\|"next"; unit: enum "frame"\|"drawing" = "frame"; count: int = 1 (1–10000)` | `{frame}` | no |
| `app.frame.set` | ui | Set the current frame | `frame: int (1–scene length)` | `{frame}` | no |
| `app.info.get` | query | Application name, version, platform, locale, installed languages, project format version | — | `{name, version, platform, locale, languages[], formatVersion}` | n/a |
| `app.mode.set` | ui | Switch to drawing or staging mode by activating (or switching) a view of that type | `mode: enum "drawing"\|"staging"; viewId?: id` | `{mode, viewId}` | no |
| `app.quit` | action | Quit the application | `onUnsaved: enum "fail"\|"save"\|"discard" = "fail"` | `{quitting: bool}` | n/a |
| `app.state.changed` | event | Current context changed (project, scene, frame, element, mode, view, tool, workspace, modal state) | `{changed: [string], state: AppState}` | — | n/a |
| `app.state.get` | query | Current application state (project, scene, frame, element, drawing, mode, active view, focused panel, active tool, selection, workspace, modal state, agent sessions) | — | `AppState` (§12.4) | n/a |

## Namespace `project`

Defined in [`01_application_shell.md`](01_application_shell.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `project.autosaved` | event | A recovery copy was written | `{projectId, recoveryId, savedAt}` | — | n/a |
| `project.backup.list` | query | Backups of a project | `path?: path (default: current project)` | `[{backupId, savedAt, sizeBytes}]` | n/a |
| `project.backup.open` | action | Open a backup as an untitled copy | `backupId: id; onUnsaved: enum "fail"\|"save"\|"discard" = "fail"` | `{projectId}` | n/a |
| `project.close` | action | Close the current project | `onUnsaved: enum "fail"\|"save"\|"discard" = "fail"` | `{closed: bool}` | n/a |
| `project.closed` | event | The project was closed | `{projectId}` | — | n/a |
| `project.create` | action | Create a new project and make it current; if `path` is given it is saved there immediately | `name: string; path?: path; preset?: string; frameRate?: number (1–120); cameraWidth?: int (16–8192); cameraHeight?: int (16–8192); onUnsaved: enum "fail"\|"save"\|"discard" = "fail"; onExists: enum "fail"\|"overwrite" = "fail"` | `{projectId, path?, sceneId, cameraId, elementId}` | n/a |
| `project.dirty.changed` | event | Modified state changed | `{projectDirty, globalLibraryDirty}` | — | n/a |
| `project.info.get` | query | Project summary | — | `ProjectInfo` (§12.4) | n/a |
| `project.open` | job | Open a project package | `path: path; readOnly: bool = false; onUnsaved: enum "fail"\|"save"\|"discard" = "fail"` | `{jobId}` → `{projectId, readOnly, formatVersion, warnings[]}` | n/a |
| `project.opened` | event | A project was opened or created | `{projectId, path?, readOnly}` | — | n/a |
| `project.presets.list` | query | Format presets for new projects | — | `[{id, name, kind: "mostRecent"\|"default"\|"bundled", frameRate, width, height}]` | n/a |
| `project.properties.get` | query | Frame rate and camera size | — | `{frameRate, cameraWidth, cameraHeight}` | n/a |
| `project.properties.set` | action | Change frame rate / camera size ("Change Project Properties"); optionally store as defaults for new projects | `frameRate?: number (1–120); cameraWidth?: int (16–8192); cameraHeight?: int (16–8192); setAsDefault: bool = false` | `{frameRate, cameraWidth, cameraHeight}` | yes |
| `project.recent.list` | query | Recent-projects list | — | `[{path, name, lastOpened, exists}]` | n/a |
| `project.recent.remove` | ui | Remove one entry or clear the recent list | `path?: path; all: bool = false` | `{remaining: int}` | no |
| `project.recovery.delete` | ui | Discard recovery data | `recoveryId?: id; all: bool = false` | `{deleted: int}` | no |
| `project.recovery.list` | query | Autosave recovery data available | — | `[{recoveryId, projectName, originalPath?, savedAt}]` | n/a |
| `project.recovery.open` | job | Open recovery data as a modified project targeting its original path | `recoveryId: id; onUnsaved: enum "fail"\|"save"\|"discard" = "fail"` | `{jobId}` → `{projectId, warnings[]}` | n/a |
| `project.save` | job | Save the project (and the global library if modified) to its location | `includeGlobalLibrary: bool = true; allowFormatUpgrade: bool = false` | `{jobId}` → `{path, projectSaved, globalLibrarySaved, backupId?}` | no |
| `project.saveAs` | job | Save the project to a new location; the copy becomes current | `path: path; onExists: enum "fail"\|"overwrite" = "fail"; includeGlobalLibrary: bool = true; allowFormatUpgrade: bool = false` | `{jobId}` → `{path, projectSaved, globalLibrarySaved}` | no |
| `project.saved` | event | The project and/or global library was saved | `{projectId, path, projectSaved, globalLibrarySaved}` | — | n/a |

## Namespace `edit`

Defined in [`01_application_shell.md`](01_application_shell.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `edit.clipboard.changed` | event | Clipboard content changed | `{kind, itemCount}` | — | n/a |
| `edit.clipboard.get` | query | Clipboard content as structured data | `includeData: bool = false` | `Clipboard` (§12.4) | n/a |
| `edit.clipboard.set` | ui | Put structured content on the clipboard (validated against the schema of its kind) | `content: Clipboard` | `{kind, itemCount}` | no |
| `edit.copy` | ui | Copy the selection (or the given objects) to the clipboard | `context: enum (as edit.cut) = "auto"; objectIds?: [id]` | `{clipboard: {kind, itemCount}}` | no |
| `edit.cut` | action | Cut the selection (or the given objects) of an edit context to the clipboard ("Cut") | `context: enum "auto"\|"drawing"\|"staging"\|"xsheet"\|"timeline"\|"library"\|"functionEditor" = "auto"; objectIds?: [id]` | `{clipboard: {kind, itemCount}}` | yes |
| `edit.cutSpecial` | action | Cut cells with timeline edit options ("Cut Special") | `objectIds?: [id]; options?: TimelineEditOptions (§12.4); remember: bool = true` | `{clipboard: {kind, itemCount}}` | yes |
| `edit.delete` | action | Delete the selection or the given objects; clears cells in cell contexts ("Delete") | `context: enum (as edit.cut) = "auto"; objectIds?: [id]` | `{deletedIds: [id]}` | yes |
| `edit.deselectAll` | ui | Clear the selection of an edit context | `context: enum (as edit.cut) = "auto"` | `{context}` | no |
| `edit.history.changed` | event | Undo history changed | `{position, entryCount, lastLabel, source}` | — | n/a |
| `edit.history.get` | query | Undo history with attribution | `offset: int = 0; limit: int = 100 (1–1000)` | `{entries: [{index, label, source: "user"\|"agent", agentSessionId?, agentName?, time}], position, cleanPosition}` | n/a |
| `edit.history.move` | action | Undo/redo to just after a given history entry (History panel click) | `position: int (0–entry count)` | `{position}` | n/a |
| `edit.paste` | action | Paste the clipboard into an edit context ("Paste") | `context: enum (as edit.cut) = "auto"; destination?: object ({drawingId} \| {elementId, frame} \| {catalogId} \| {functionId, frame} \| {objectIds})` | `{createdIds: [id], modifiedIds: [id], warnings[]}` | yes |
| `edit.pasteAsNewElement` | action | Create a new element from copied cells, exposed from a frame ("Paste as New Element") | `sceneId?: id; frame?: int` | `{elementId}` | yes |
| `edit.pasteSpecial` | action | Paste with timeline or motion-point options; `useLast` = paste special again ("Paste Special") | `destination?: object; options?: TimelineEditOptions \| MotionPointPasteOptions; useLast: bool = false; remember: bool = true` | `{createdIds: [id], modifiedIds: [id], warnings[]}` | yes |
| `edit.redo` | action | Redo one or more steps | `steps: int = 1 (1–1000)` | `{redone: [label], position}` | n/a |
| `edit.selectAll` | ui | Select all in an edit context | `context: enum (as edit.cut) = "auto"` | `{context, ids: [id]}` | no |
| `edit.selection.changed` | event | A selection changed | `{context, ids: [id]}` | — | n/a |
| `edit.selection.get` | query | Selection of an edit context | `context: enum (as edit.cut) = "auto"` | `{context, ids: [id], kinds: [string]}` | n/a |
| `edit.selection.set` | ui | Change the selection of an edit context | `context: enum (as edit.cut) = "auto"; ids: [id]; mode: enum "replace"\|"add"\|"remove"\|"toggle" = "replace"` | `{context, ids: [id]}` | no |
| `edit.undo` | action | Undo one or more steps; with `ifTop`, only if that step is still the most recent (else `undo.notTop`) | `steps: int = 1 (1–1000); ifTop?: string` | `{undone: [label], position}` | n/a |

## Namespace `view`

Defined in [`01_application_shell.md`](01_application_shell.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `view.active.set` | ui | Make a view window active (focus it) | `viewId: id` | `{viewId}` | no |
| `view.coordinates.convert` | query | Convert a point between view pixels, drawing space and scene space for a view | `viewId: id; point: point; from: enum "view"\|"drawing"\|"scene"; to: enum "view"\|"drawing"\|"scene"` | `{point}` | n/a |
| `view.list` | query | View windows with type, focus, size and transform | — | `[{viewId, panelId, type, focused, size: {width, height}, transform}]` | n/a |
| `view.pan.set` | ui | Pan: centre on a document point, by a pixel offset, or on the pointer (centre on pointer) | `viewId?: id; centre?: point; by?: point (view pixels); atPointer: bool = false` | `{zoom, rotationDegrees, centre}` | no |
| `view.rotation.set` | ui | Set view rotation absolutely or in 30° steps | `viewId?: id; degrees?: number (−360–360); steps?: int` | `{zoom, rotationDegrees, centre}` | no |
| `view.transform.get` | query | Zoom, rotation and centre of a view | `viewId?: id` | `{zoom, rotationDegrees, centre: point}` | n/a |
| `view.transform.reset` | ui | Reset view / reset zoom / reset rotation / recentre | `viewId?: id; parts: [enum "zoom"\|"rotation"\|"pan"] = ["zoom","rotation","pan"]` | `{zoom, rotationDegrees, centre}` | no |
| `view.type.set` | ui | Switch a view window's type; `"previous"` = switch active view | `viewId?: id; type: enum "drawing"\|"camera"\|"top"\|"side"\|"previous"` | `{viewId, type}` | no |
| `view.zoom.set` | ui | Set zoom absolutely or by table steps (zoom in = +1, zoom out = −1) | `viewId?: id; zoom?: number (0.0625–64); steps?: int; anchor?: point (view pixels)` | `{zoom, rotationDegrees, centre}` | no |

## Namespace `window`

Defined in [`01_application_shell.md`](01_application_shell.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `window.floatingPanels.set` | toggle | Hide/show all floating panels and tool palettes | `visible: bool` | `{visible}` | no |
| `window.fullScreen.set` | toggle | Full-screen presentation | `enabled: bool` | `{enabled}` | no |
| `window.geometry.get` | query | Main window geometry and screens | — | `{x, y, width, height, maximized, fullScreen, screens: [{x, y, width, height, scale}]}` | n/a |
| `window.geometry.set` | ui | Move/resize/maximise the main window | `x?: int; y?: int; width?: int; height?: int; maximized?: bool` | window geometry | no |
| `window.minimized.set` | toggle | Minimise or restore the main window | `minimized: bool` | `{minimized}` | no |
| `window.panel.list` | query | Panels with visibility, dock area, tab group, floating geometry and focus | — | `[{panelId, title, visible, floating, area?, tabGroup?, current, geometry?, focused}]` | n/a |
| `window.panel.move` | ui | Dock a panel into an area, next to or tabbed with another panel | `panelId: string; area: enum "left"\|"right"\|"top"\|"bottom"\|"central"; relativeTo?: string; position: enum "tab"\|"before"\|"after"\|"above"\|"below" = "tab"; size?: int` | `Layout` | no |
| `window.panel.set` | toggle | Show/hide, float/dock, focus a panel; `"active"` addresses the focused panel (close active panel = `visible: false`) | `panelId: string; visible?: bool; floating?: bool; focus: bool = true; geometry?: {x, y, width, height}` | panel entry | no |
| `window.raiseAll.run` | ui | Bring all application windows to the front | — | `{}` | no |
| `window.sidePanelsExtended.set` | toggle | Side dock areas own the corners (true) or top/bottom areas do (false) | `enabled: bool` | `{enabled}` | no |
| `window.toolbar.items.reset` | ui | Restore the default main toolbar items | `toolbarId: enum "main" = "main"` | `{items: [string]}` | no |
| `window.toolbar.items.set` | ui | Customise the main toolbar (stored in the current workspace) | `toolbarId: enum "main" = "main"; items: [string] (command references, group IDs, "separator", "space", "flexibleSpace")` | `{items: [string]}` | no |
| `window.toolbar.list` | query | Toolbars and tool palettes with visibility, items and allowed items | — | `[{toolbarId, visible, floating, position?, items: [string], allowedItems: [string]}]` | n/a |
| `window.toolbar.set` | toggle | Show/hide (and float/position) the main toolbar or a tool palette | `toolbarId: enum "main"\|"drawingTools"\|"animationTools"\|"boneTools"\|"effectTools"\|"soundTools"; visible: bool; floating?: bool; position?: point` | toolbar entry | no |

## Namespace `workspace`

Defined in [`01_application_shell.md`](01_application_shell.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `workspace.changed` | event | Current workspace switched or modified | `{name, modified}` | — | n/a |
| `workspace.create` | ui | Create a workspace from the current arrangement or a given layout | `name: string; position?: int (1–n+1); layout?: Layout` | `{name, position}` | no |
| `workspace.delete` | ui | Delete a user workspace | `name: string` | `{deleted: string}` | no |
| `workspace.export` | ui | Write a workspace to a file | `name: string; path: path; onExists: enum "fail"\|"overwrite" = "fail"` | `{path}` | no |
| `workspace.get` | query | Full layout of a workspace (default: the current arrangement) | `name?: string` | `Layout` (§12.4) | n/a |
| `workspace.import` | ui | Read a workspace from a file | `path: path; onExists: enum "fail"\|"overwrite"\|"rename" = "fail"` | `{name}` | no |
| `workspace.layout.set` | ui | Apply a layout to the window without saving it to a workspace | `layout: Layout` | `Layout` | no |
| `workspace.list` | query | Workspaces | — | `[{name, builtIn, modified, position, current}]` | n/a |
| `workspace.rename` | ui | Rename a workspace | `name: string; newName: string` | `{name}` | no |
| `workspace.reset` | ui | Restore built-in workspace(s) to the bundled arrangement; `select` also applies it (restore default workspace = `name: "Default", select: true`) | `name?: string; all: bool = false; select: bool = false` | `{reset: [string]}` | no |
| `workspace.save` | ui | Store the current arrangement into a workspace | `name?: string (default: current)` | `{name}` | no |
| `workspace.select` | ui | Switch workspace | `name: string; saveCurrent?: bool (default: general.workspace.autoSave)` | `{name}` | no |

## Namespace `prefs`

Defined in [`01_application_shell.md`](01_application_shell.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `prefs.changed` | event | Preferences changed | `{keys: [string], source: "user"\|"agent"}` | — | n/a |
| `prefs.get` | query | Values of preferences by key | `keys: [string]` | `{values: object}` | n/a |
| `prefs.list` | query | Preference schema and values | `prefix?: string` | `[{key, type, default, value, min?, max?, values?, applies: "live"\|"nextLaunch"\|"restart", description}]` | n/a |
| `prefs.reset` | ui | Reset preferences to defaults by key or prefix (`prefix: ""` = all) | `keys?: [string]; prefix?: string` | `{reset: [string]}` | no |
| `prefs.set` | ui | Set one or more preferences atomically | `values: object (key → value)` | `{applied: [string], restartRequired: [string]}` | no |

## Namespace `keymap`

Defined in [`01_application_shell.md`](01_application_shell.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `keymap.changed` | event | Bindings changed | `{commandIds: [string]}` | — | n/a |
| `keymap.export` | ui | Write the keymap to a file | `path: path; onExists: enum "fail"\|"overwrite" = "fail"` | `{path}` | no |
| `keymap.get` | query | Bindings of a command, or the command a key sequence runs in a context | `commandId?: string; keys?: string; context: enum "global"\|"drawing"\|"staging" = "global"` | `{bindings: [Binding], effectiveCommand?: {commandId, params?}}` | n/a |
| `keymap.import` | ui | Read a keymap file | `path: path; mode: enum "merge"\|"replace" = "merge"; onConflict: enum "fail"\|"override" = "fail"` | `{imported: int, conflicts: [Binding]}` | no |
| `keymap.list` | query | Bindings, filterable | `context?: enum "global"\|"drawing"\|"staging"; commandId?: string; prefix?: string; onlyModified: bool = false` | `[Binding]` (§12.4) | n/a |
| `keymap.remove` | ui | Remove bindings of a command | `commandId: string; keys?: string; context?: enum "global"\|"drawing"\|"staging"` | `{removed: [Binding]}` | no |
| `keymap.reset` | ui | Restore default bindings for one command or all | `commandId?: string; all: bool = false` | `{reset: int}` | no |
| `keymap.set` | ui | Bind a key sequence to a command (with optional fixed parameters) | `commandId: string; params?: object; keys: string; context: enum "global"\|"drawing"\|"staging" = "global"; onConflict: enum "fail"\|"override" = "fail"; replaceExisting: bool = false` | `{binding: Binding, removed: [Binding]}` | no |

## Namespace `ui`

Defined in [`01_application_shell.md`](01_application_shell.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `ui.contextHelpMode.set` | toggle | Enter/leave context-help mode | `enabled: bool` | `{enabled}` | no |
| `ui.help.get` | query | Help text of a command, panel, preference or message | `commandId?: string; panelId?: string; preferenceKey?: string; messageId?: string` | `{title, text (Markdown)}` | n/a |
| `ui.help.open` | ui | Open the documentation, optionally at a topic | `topic?: string` | `{}` | no |
| `ui.message.shown` | event | A message was shown or returned as a warning | `{messageId, severity: "info"\|"warning"\|"error"\|"question", text, details?, source: "user"\|"agent", suppressed}` | — | n/a |
| `ui.preferences.open` | ui | Open the preferences window at a category (GUI convenience) | `category?: string` | `{}` | no |
| `ui.startScreen.set` | toggle | Show/hide the start screen | `visible: bool` | `{visible}` | no |
| `ui.statusBar.get` | query | Status bar segment texts | — | `{visible, description, context, pointer, playback, agentIndicator}` | n/a |
| `ui.statusBar.set` | toggle | Show/hide the status bar (same as `interface.statusBar.visible`) | `visible: bool` | `{visible}` | no |
| `ui.theme.list` | query | Available themes (selected via `interface.theme`) | — | `[{themeId, name, builtIn, dark}]` | n/a |
| `ui.tips.list` | query | Suppressible messages and their state | — | `[{messageId, description, suppressed}]` | n/a |
| `ui.tips.reset` | ui | Show all tips again | — | `{reenabled: int}` | no |
| `ui.tips.set` | ui | Suppress or re-enable one message | `messageId: string; suppressed: bool` | `{messageId, suppressed}` | no |

## Namespace `draw`

Defined in [`02_drawing.md`](02_drawing.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `draw.arrange.apply` | action | Change stacking order ("Bring to Front", "Bring Forward", "Send Backward", "Send to Back") | `drawingId?: id; ids?: [id] = selection; to: enum "front"\|"forward"\|"backward"\|"back"` | `{order: [id], rev}` | yes |
| `draw.bounds.get` | query | Bounding box of the drawing or of given objects | `drawingId?: id; ids?: [id]; includeInvisible: bool = false` | `{bounds: {x,y,width,height}\|null}` | n/a |
| `draw.breakApart.apply` | action | Convert text to per-character vector groups ("Break Text Apart") | `drawingId?: id; textIds?: [id] = selected text` | `{groupIds: [id], rev}` | yes |
| `draw.centreLine.apply` | action | Convert brush strokes to pencil lines ("Extract Centre Line") | `drawingId?: id; ids?: [id] = selection; thickness?: number (default: average source width)` | `{strokeIds: [id], removedIds: [id], warnings: [code], rev}` | yes |
| `draw.content.add` | action | Insert drawing content given in the §13.4 JSON shape (IDs reassigned; colour recovery applies) ("Paste") | `drawingId?: id; content: object; offset: point = {0,0}; layer: enum "top"\|"behind" = "top"; select: bool = true` | `{idMap: object, rev}` | yes |
| `draw.current.get` | query | Current drawing target and context | `view?: string` | `{drawingId\|null, elementId, sceneId, frame, cellEmpty: bool, locked: bool, activeView}` | n/a |
| `draw.cut.apply` | action | Cut artwork along a polygon or rectangle; inside part selected or deleted ("Cutter Tool"/"Scissor Tool") | `drawingId?: id; polygon?: [point]; rect?: {x,y,width,height}; then: enum "select"\|"delete" = "select"` | `{insideIds: [id], removedIds: [id], rev}` | yes |
| `draw.drawing.changed` | event | A drawing's content changed | — | `{drawingId, rev, commandId, createdIds: [id], modifiedIds: [id], removedIds: [id]}` | n/a |
| `draw.drawing.get` | query | Full vector content as JSON (§13.4) | `drawingId?: id; elementId?: id; frame?: int; include?: [enum "zones"\|"pressure"\|"outline"\|"bounds"]; flatten: bool = false; ids?: [id]` | drawing JSON (§13.4) | n/a |
| `draw.drawing.rename` | action | Rename a drawing ("Rename Drawing") | `drawingId?: id; name: string` | `{drawingId, name}` | yes |
| `draw.drawing.render` | query | Render a drawing to PNG at a requested size | `drawingId?: id; width: int (1–8192); height?: int (1–8192); region: enum "bounds"\|"field12"\|"field16"\|"camera" \| object {x,y,width,height} = "bounds"; padding: number = 0; background: colour = #00000000; styleId?: id; showStrokes: bool = false; showSelection: bool = false; path?: path` | `{png?, path?, width, height, drawingToPixel: [number], rev}` | n/a |
| `draw.ellipse.add` | action | Add an ellipse as a closed pencil path ("Ellipse Tool") | `drawingId?: id; from: point; to: point; circle: bool = false; fromCentre: bool = false; colourId?: id; thickness?: number; penId?: id; layer: enum "auto"\|"new"\|"top"\|"behind" \| id = "auto"; symmetry: bool = true` | `{strokeIds: [id], zoneIds: [id], layerId, rev}` | yes |
| `draw.envelope.apply` | action | Perspective/envelope distortion of the selection ("Perspective Tool") | `drawingId?: id; ids?: [id] = selection; corners: [point] (4 targets for the bounds corners: bottom-left, bottom-right, top-right, top-left); mapping: enum "auto"\|"projective"\|"bilinear" = "auto"` | `{ids: [id], rev}` | yes |
| `draw.erase.apply` | action | Erase along a path with the eraser tip, or everything inside a region ("Eraser Tool") | `drawingId?: id; path?: [{x,y,pressure?}]; penId?: id = current eraser pen; pen?: object; region?: [point] (closed polygon); layers: enum "all" \| [id] = "all"` | `{removedIds: [id], createdIds: [id], rev}` | yes |
| `draw.feather.apply` | job | Feather edges of filled shapes ("Feather Edges") | `drawingId?: id; ids?: [id] = selection; stepWidth: int = 10 (5–100); steps: int = 5 (3–100); confirm: bool = false` | `{jobId}` → `{layerIds: [id], skippedTextured: int, warnings: [code], rev}` or `{needs_confirmation}` | yes |
| `draw.flatten.apply` | action | Merge selected layers into one ("Flatten Drawing") | `drawingId?: id; ids?: [id] = selection` | `{layerId, removedIds: [id], rev}` | yes |
| `draw.flip.apply` | action | Flip selection ("Flip Horizontal"/"Flip Vertical") | `drawingId?: id; ids?: [id] = selection; axis: enum "horizontal"\|"vertical"` | `{ids: [id], rev}` | yes |
| `draw.group.create` | action | Group objects ("Group Selection") | `drawingId?: id; ids?: [id] = selection` | `{groupId, rev}` | yes |
| `draw.group.delete` | action | Ungroup: dissolve groups, keep members ("Ungroup Selection") | `drawingId?: id; groupIds?: [id] = selected groups` | `{memberIds: [id], rev}` | yes |
| `draw.hitTest.get` | query | Objects and zone under a point, front to back | `drawingId?: id; point: point; tolerance: number = 3 (view px); includeInvisible: bool = false` | `{hits: [object], zoneId\|null, rev}` | n/a |
| `draw.inspectorPreview.set` | ui | Show/hide the drawing thumbnail in the inspector | `visible: bool` | `{visible}` | no |
| `draw.line.add` | action | Add a straight pencil line ("Line Tool") | `drawingId?: id; from?: point; to: point; continuePrevious: bool = false; constrainAngle: bool = false; colourId?: id; thickness?: number; penId?: id; layer: enum "auto"\|"new"\|"top"\|"behind" \| id = "auto"; symmetry: bool = true` | `{strokeIds: [id], zoneIds: [id], layerId, rev}` | yes |
| `draw.linesToBrush.apply` | action | Convert centre lines to brush strokes ("Convert Lines to Brush") | `drawingId?: id; ids?: [id] = selection; penId?: id = current brush pen` | `{strokeIds: [id], rev}` | yes |
| `draw.lock.set` | toggle | Lock or unlock a drawing ("Lock Drawing"/"Unlock Drawing") | `drawingId?: id; locked: bool` | `{drawingId, locked}` | yes |
| `draw.object.delete` | action | Delete objects ("Delete Object") | `drawingId?: id; ids?: [id] = selection` | `{removedIds: [id], rev}` | yes |
| `draw.optimize.apply` | action | Merge layers only where appearance is unchanged ("Optimize Drawing") | `drawingId?: id; ids?: [id] = selection` | `{layerIds: [id], merged: int, rev}` | yes |
| `draw.pivot.set` | action | Move the drawing pivot ("Change Drawing Pivot") | `drawingId?: id; point: point` | `{pivot, rev}` | yes |
| `draw.point.add` | action | Add an anchor on a stroke without changing its shape ("Contour Editing") | `strokeId: id; at?: point (nearest location); segment?: int; t?: number (0–1)` | `{strokeId, anchor: int, rev}` | yes |
| `draw.point.delete` | action | Delete anchors; strokes rebuilt or removed ("Contour Editing") | `points?: [{strokeId, anchor: int}] = point selection` | `{modifiedIds: [id], removedIds: [id], rev}` | yes |
| `draw.point.move` | action | Move anchors (shared junction anchors move together) ("Contour Editing") | `points?: [{strokeId, anchor: int}] = point selection; delta: point; snap?: bool = Snap to Contour setting` | `{modifiedIds: [id], rev}` | yes |
| `draw.point.set` | action | Set an anchor's position and handles ("Contour Editing") | `strokeId: id; anchor: int; position?: point; inHandle?: point; outHandle?: point; handleMode?: enum "symmetric"\|"broken"\|"reset"` | `{strokeId, rev}` | yes |
| `draw.pointSelection.set` | ui | Select anchor points (Contour Editor selection) | `drawingId?: id; points?: [{strokeId, anchor: int}]; rect?: {x,y,width,height}; mode: enum "replace"\|"add"\|"toggle"\|"remove" = "replace"` | `{selectedPoints: [{strokeId, anchor}]}` | no |
| `draw.polyline.add` | action | Add a polyline of straight/curved segments ("Polyline Tool") | `drawingId?: id; points: [{x,y,inHandle?: point,outHandle?: point}]; closed: bool = false; snap: bool = true; snapRadius: number = 8 (view px); colourId?: id; thickness?: number; penId?: id; layer: enum "auto"\|"new"\|"top"\|"behind" \| id = "auto"; symmetry: bool = true` | `{strokeIds: [id], zoneIds: [id], layerId, rev}` | yes |
| `draw.rectangle.add` | action | Add a rectangle as a closed pencil path ("Rectangle Tool") | `drawingId?: id; from: point; to: point; square: bool = false; fromCentre: bool = false; colourId?: id; thickness?: number; penId?: id; layer: enum "auto"\|"new"\|"top"\|"behind" \| id = "auto"; symmetry: bool = true` | `{strokeIds: [id], zoneIds: [id], layerId, rev}` | yes |
| `draw.repositionAll.apply` | action | Apply one transform to every drawing of an element ("Reposition All Drawings") | `elementId?: id = current; translate?: point; scale?: {x,y}; rotate?: number; pivot?: point = {0,0}; matrix?: [number]; corners?: [point]` | `{drawingIds: [id]}` | yes |
| `draw.rotate.apply` | action | Rotate selection by a fixed step ("Rotate 90° CW"/"Rotate 90° CCW"/"Rotate 180°") | `drawingId?: id; ids?: [id] = selection; by: enum "90cw"\|"90ccw"\|"180"` | `{ids: [id], rev}` | yes |
| `draw.segment.move` | action | Reshape a segment so its point at t moves by delta ("Contour Editing") | `strokeId: id; segment: int; t: number = 0.5 (0–1); delta: point` | `{strokeId, rev}` | yes |
| `draw.selection.changed` | event | Drawing selection changed | — | `{drawingId, selectedIds: [id], selectedPoints: [object]}` | n/a |
| `draw.selection.get` | query | Current drawing selection | `drawingId?: id` | `{selectedIds: [id], bounds, selectedPoints: [{strokeId, anchor}]}` | n/a |
| `draw.selection.set` | ui | Select drawing objects by IDs, point, rectangle, lasso, all or none | `drawingId?: id; by: enum "ids"\|"point"\|"rect"\|"lasso"\|"all"\|"none" = "ids"; ids?: [id]; point?: point; rect?: {x,y,width,height}; polygon?: [point]; rectRule: enum "inside"\|"intersect" = "inside"; mode: enum "replace"\|"add"\|"toggle"\|"remove" = "replace"` | `{selectedIds: [id], bounds}` | no |
| `draw.smooth.apply` | action | Remove unnecessary points ("Smooth Selection") | `drawingId?: id; ids?: [id] = selection; points?: [{strokeId, anchor}]; tolerance: number = 1 (du)` | `{removedPointCount: int, rev}` | yes |
| `draw.stroke.add` | action | Add a brush stroke, pencil line or invisible stroke ("Brush Tool"/"Pencil Tool"/"Stroke Tool") | `drawingId?: id; elementId?: id; frame?: int; createDrawing: bool = true; kind: enum "brush"\|"pencil"\|"invisible" = "brush"; points?: [{x,y,pressure?}]; bezier?: [point]; closed: bool = false; pressure?: [{t,p}]; fit: bool = true; smoothness?: number (0–10) = pen's; penId?: id = current pen; pen?: object {tipShape,minSize,maxSize,tipAngle}; colourId?: id = current colour; thickness?: number (0–100) = pen max; layer: enum "auto"\|"new"\|"top"\|"behind" \| id = "auto"; symmetry: bool = true; expectRev?: int` | `{drawingId, createdDrawing: bool, strokeIds: [id], zoneIds: [id], layerId, removedIds: [id], rev, hints: [code]}` | yes |
| `draw.stroke.set` | action | Replace a stroke's geometry, pressure, thickness or visibility ("Edit Stroke") | `strokeId: id; bezier?: [point]; closed?: bool; pressure?: [{t,p}]; thickness?: number (0–100); kind?: enum "pencil"\|"invisible"; expectRev?: int` | `{strokeIds: [id], removedIds: [id], rev}` | yes |
| `draw.text.add` | action | Add a text object ("New Text Object") | `drawingId?: id; position: point; text: string; width?: number (fixed width; omit = auto); format?: object (Text panel fields); colourId?: id` | `{textId, bounds, rev}` | yes |
| `draw.text.set` | action | Replace or edit text content, width mode or position ("Typing"/"Set Text Width"/"Move Text Object") | `textId: id; text?: string; insert?: {at: int, text: string}; delete?: {start: int, length: int}; widthMode?: enum "auto"\|"fixed"; width?: number; position?: point` | `{textId, bounds, rev}` | yes |
| `draw.textFormat.set` | action | Set character/paragraph formatting of a text range ("Text Property Modified") | `textId: id; start: int = 0; length?: int (default to end); font?: string; size?: number (1–512); bold?: bool; italic?: bool; baseline?: enum "normal"\|"subscript"\|"superscript"; kerning?: int (−60–60); autoKern?: bool; align?: enum "left"\|"centre"\|"right"\|"justify"; leftMargin?: number (0–1000); rightMargin?: number (0–1000); indent?: number (−99–1000); lineSpacing?: number (−99–1000); colourId?: id; outlineColourId?: id\|null` | `{textId, rev}` | yes |
| `draw.transform.apply` | action | Move/scale/rotate/skew objects numerically, optionally as a copy ("Move/Scale/Rotate Object", "Copy Object") | `drawingId?: id; ids?: [id] = selection; translate?: point; scale?: {x,y}; rotate?: number (degrees, CCW); skew?: {x,y} (degrees); pivot?: point = selection bounds centre; matrix?: [number] (overrides the others); copy: bool = false` | `{ids: [id], removedIds: [id], rev}` | yes |
| `draw.vectorFile.import` | job | Import an Illustrator/PDF file into a drawing ("Import Vector File") | `file: path; drawingId?: id; elementId?: id; frame?: int; page: int = 1` | `{jobId}` → `{drawingId, objectIds: [id], addedColourIds: [id], warnings: [string], rev}` | yes |
| `draw.vectorize.preview` | query | Preview vectorization of one image as PNG | `file?: path; imageElementId?: id; frame?: int; type: enum "blackAndWhite"\|"withTexture" = "blackAndWhite"; filter: enum "none"\|"smooth"\|"smoothMore"\|"edgeEnhance"\|"sharpen" = "none"; threshold: int = 70 (0–100); width: int = 512` | `{png, strokeCount: int}` | n/a |
| `draw.vectorize.run` | job | Import and vectorize image files, or convert an image element, into drawings ("Import and Vectorize"); omitted options default to `import.vectorize.settings.get` (09) | `files?: [path]; imageElementId?: id; targetElementId?: id (default: selected; new element if none); startFrame?: int = selected cell; type: enum "blackAndWhite"\|"withTexture" = "blackAndWhite"; filter: enum "none"\|"smooth"\|"smoothMore"\|"edgeEnhance"\|"sharpen" = "none"; threshold: int = 70 (0–100); onConflict: enum "ask"\|"replace"\|"skip" = "ask"` | `{jobId}` → `{elementId, drawingIds: [id], completed: int, cancelled: bool}` | yes |
| `draw.viewMapping.get` | query | Mapping between a view's pixels and drawing space | `view: string = "drawing"` | `{drawingId, drawingToView: [number], viewToDrawing: [number], zoom, rotation, viewSize: {width,height}}` | n/a |

## Namespace `tool`

Defined in [`02_drawing.md`](02_drawing.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `tool.active.changed` | event | Active tool changed (including temporary overrides) | — | `{toolId, temporary: bool}` | n/a |
| `tool.active.get` | query | Active tool (and temporary override, if any) | — | `{toolId, temporaryToolId\|null, group}` | n/a |
| `tool.active.set` | ui | Activate a tool | `toolId: enum "select"\|"contourEditor"\|"perspective"\|"repositionAll"\|"brush"\|"pencil"\|"rectangle"\|"ellipse"\|"polyline"\|"line"\|"paint"\|"unpaint"\|"paintUnpainted"\|"dropper"\|"closeGap"\|"stroke"\|"editTexture"\|"eraser"\|"cutter"\|"scissor"\|"zoom"\|"grabber"\|"text"` | `{toolId}` | no |
| `tool.brush.options.set` | ui | Brush options (current brush pen) | `penId: id` | `{options}` | no |
| `tool.drawTopLayer.set` | toggle | Draw on Top Layer | `enabled: bool` | `{enabled}` | no |
| `tool.eraser.options.set` | ui | Eraser options (current eraser pen) | `penId: id` | `{options}` | no |
| `tool.group.select` | ui | Select a tool group; advance within it if already active (group cycling) | `group: enum "select"\|"brush"\|"paint"\|"eraser"\|"navigate"\|"text"; advance: bool = true` | `{toolId}` | no |
| `tool.list` | query | All drawing tools with group, option schema and current options | — | `{tools: [{toolId, group, options: object, optionSchema: object}], activeToolId}` | n/a |
| `tool.options.changed` | event | A tool option or global drawing setting changed | — | `{toolId\|null, options}` | n/a |
| `tool.options.get` | query | Options of a tool | `toolId: string` | `{toolId, options: object}` | n/a |
| `tool.options.set` | ui | Set any option(s) of any tool (generic form of the rows below) | `toolId: string; options: object` | `{toolId, options}` | no |
| `tool.pen.changed` | event | Pen list or pen settings changed | — | `{penId, change: enum "created"\|"modified"\|"deleted"\|"selected"}` | n/a |
| `tool.pen.create` | action | New pen from current or given pen ("New Pen") | `type: enum "brush"\|"eraser" = "brush"; fromPenId?: id = current; name?: string; settings?: object` | `{penId, name}` | yes |
| `tool.pen.delete` | action | Delete a pen ("Delete Pen") | `penId: id` | `{removedPenId, currentPenId}` | yes |
| `tool.pen.list` | query | Pens of the project with all settings | `type?: enum "brush"\|"eraser"` | `{pens: [{penId, name, type, tipShape, minSize, maxSize, smoothness, tipAngle}], currentBrushPenId, currentEraserPenId}` | n/a |
| `tool.pen.rename` | action | Rename a pen ("Rename Pen") | `penId: id; name: string` | `{penId, name}` | yes |
| `tool.pen.select` | ui | Make a pen current for its tool family | `penId: id` | `{penId, type}` | no |
| `tool.pen.set` | action | Change pen settings ("Set Pen Style") | `penId: id; tipShape?: enum "round"\|"oval"\|"square"\|"flat"; minSize?: number (0–100); maxSize?: number (0–100); smoothness?: number (0–10); tipAngle?: number (0–360)` | `{pen}` | yes |
| `tool.pencil.options.set` | ui | Pencil and shape-tool line thickness source | `thicknessSource: enum "penMax"\|"fixed" = "penMax"; thickness?: number (0–100)` | `{options}` | no |
| `tool.placeBehind.set` | toggle | Draw Behind | `enabled: bool` | `{enabled}` | no |
| `tool.polyline.options.set` | ui | Polyline snapping | `snap: bool = true; snapRadius: number = 8 (1–50 view px)` | `{options}` | no |
| `tool.select.options.set` | ui | Select tool options | `rectRule: enum "inside"\|"intersect" = "inside"` | `{options}` | no |
| `tool.settings.get` | query | All global drawing-tool settings | — | `{placeBehind, drawTopLayer, snapToContour, symmetry, tablet, gapClosing}` | n/a |
| `tool.snapToContour.set` | toggle | Snap to Contour (Contour Editor) | `enabled: bool` | `{enabled}` | no |
| `tool.symmetry.set` | ui | Symmetry drawing mode | `mode: enum "none"\|"mirror"\|"kaleidoscope" = "none"; angle?: number (0–360) = 0; mirrors?: int (2–16) = 6` | `{mode, angle, mirrors}` | no |
| `tool.tablet.set` | ui | Tablet input settings | `ignoreTablet?: bool = false; portraitCorrection?: bool = false; compatibilityMode?: bool = false; inputInterface?: string` | `{settings, restartRequired: bool}` | no |
| `tool.text.options.set` | ui | Text panel defaults for new text | `font?: string; size?: number (1–512) = 36; bold?: bool = false; italic?: bool = false; baseline?: enum "normal"\|"subscript"\|"superscript" = "normal"; kerning?: int (−60–60) = 0; autoKern?: bool = true; align?: enum "left"\|"centre"\|"right"\|"justify" = "left"; leftMargin?: number = 0; rightMargin?: number = 0; indent?: number = 0; lineSpacing?: number = 0; outlineColourId?: id\|null` | `{options}` | no |

## Namespace `drawAid`

Defined in [`02_drawing.md`](02_drawing.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `drawAid.autoLightTable.set` | ui | Auto light table | `enabled: bool` | `{enabled}` | no |
| `drawAid.background.set` | ui | Drawing View background | `colour?: colour; useSceneColour?: bool` | `{colour, useSceneColour}` | no |
| `drawAid.changed` | event | A drawing aid setting changed | — | `{aid, state: object}` | n/a |
| `drawAid.currentOnTop.set` | ui | Show current drawing on top in the Camera View | `enabled: bool` | `{enabled}` | no |
| `drawAid.grid.set` | ui | Grid visibility, type and overlay | `visible?: bool; type?: enum "normal"\|"field12"\|"field16"; overlay?: bool` | `{visible, type, overlay}` | no |
| `drawAid.lightTableShade.set` | ui | Shading of auto and static light tables | `auto?: bool; static?: bool` | `{auto, static}` | no |
| `drawAid.onionSkin.set` | ui | Onion skin on/off and counts | `enabled?: bool; previous?: int (0–3); next?: int (0–3)` | `{enabled, previous, next}` | no |
| `drawAid.onionSkinCycle.run` | ui | Cycle previous or next onion-skin count 0→1→2→3→0 | `direction: enum "previous"\|"next"` | `{previous, next}` | no |
| `drawAid.onionSkinStyle.set` | ui | Onion-skin appearance | `outline?: bool; transparency?: bool; colourShading?: bool; previousColour?: colour; nextColour?: colour` | `{style}` | no |
| `drawAid.selectionDisplay.set` | ui | Camera View selection display options | `outlineOnSelection?: bool; boxHighlighting?: bool; keepSelectionVisible?: bool` | `{settings}` | no |
| `drawAid.state.get` | query | State of all drawing aids | — | `{grid, onionSkin, onionSkinStyle, autoLightTable, staticLightTable, lightTableShade, strokes, currentOnTop, selectionDisplay, background}` | n/a |
| `drawAid.staticLightTable.add` | action | Send a drawing or image to the static light table | `drawingId?: id; elementId?: id; frame?: int; placement: enum "overlay"\|"underlay" = "underlay"; visible: bool = true` | `{entryId}` | no |
| `drawAid.staticLightTable.list` | query | Static light-table entries | `sceneId?: id` | `{entries: [{entryId, sourceId, sourceKind, visible, placement}]}` | n/a |
| `drawAid.staticLightTable.remove` | action | Delete entries (sources untouched) | `entryIds?: [id]; all: bool = false` | `{removedEntryIds: [id]}` | no |
| `drawAid.staticLightTable.set` | action | Show/hide, hide all others, overlay/underlay entries | `entryIds?: [id]; all: bool = false; visible?: bool; placement?: enum "overlay"\|"underlay"; onlyVisibleEntryId?: id` | `{entries: [object]}` | no |
| `drawAid.staticLightTablePanel.set` | ui | Show/hide the static light-table panel | `visible: bool` | `{visible}` | no |
| `drawAid.strokes.set` | ui | Show Strokes (invisible strokes and open ends) | `visible: bool` | `{visible}` | no |

## Namespace `colour`

Defined in [`03_colour.md`](03_colour.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `colour.autoColourUpdate.set` | toggle | Project flag: template colours overwrite existing swatch values | `enabled: bool` | `{enabled}` | yes |
| `colour.current.changed` | event | Current palette or colour changed | — | `{paletteId, colourId}` | n/a |
| `colour.current.get` | query | Current palette, swatch and style | — | `{paletteId, colourId, styleId, swatch}` | n/a |
| `colour.current.set` | ui | Set the current colour (switches current palette if needed) | `colourId: id` | `{paletteId, colourId}` | no |
| `colour.editor.close` | ui | Close the editor, committing or discarding the pending edit | `commit: bool = true` | `{}` | n/a |
| `colour.editor.open` | ui | Open the colour editor / texture properties for a swatch | `colourId?: id` | `{}` | no |
| `colour.fillMapping.get` | query | Fill mapping of a gradient/texture-filled object (optionally copy to clipboard) | `drawingId?: id; objectId: id; toClipboard: bool = false` | `{colourId, kind, matrix: [number], linear?, radial?, texture?}` | n/a |
| `colour.fillMapping.set` | action | Set fill mapping on objects: explicit, default fit, or from clipboard ("Edit Fill Mapping") | `drawingId?: id; objectIds: [id]; mapping: object\|"default"\|"clipboard"` | `{changed: [id], skipped: [{objectId, reason}], rev}` | yes |
| `colour.gap.close` | action | Close a gap by adding an invisible stroke between the nearest stroke ends ("Close Gap") | `drawingId?: id; from: point; to: point; space: enum "drawing"\|"scene"\|"view" = "drawing"; sceneId?: id; frame?: int; view?: string` | `{strokeId\|null, rev}` | yes |
| `colour.gapClosing.get` | query | Current gap closing mode and tolerances | — | `{mode, tolerancesPx: {small, medium, large}}` | n/a |
| `colour.gapClosing.set` | ui | Set the automatic gap closing mode (persisted user setting) | `mode: enum "off"\|"small"\|"medium"\|"large"` | `{mode}` | no |
| `colour.missing.list` | query | Colour IDs referenced by artwork but not found in any palette | `scope: enum "project"\|"scene"\|"element"\|"drawing" = "project"; sceneId?: id; elementIds?: [id]; drawingIds?: [id]` | `{missing: [{colourId, drawingIds[], recoverable: bool, recordedPaletteName}]}` | n/a |
| `colour.paint.apply` | action | Paint zones, lines, brush strokes or text by ID, point or crossing path ("Paint") | `drawingId?: id; objectIds?: [id]; points?: [point]; path?: [point]; space: enum "drawing"\|"scene"\|"view" = "drawing"; sceneId?: id; frame?: int; elementId?: id; view?: string; colourId?: id; onlyUnpainted: bool = false; mapping?: {from: point, to: point}; gapClosing: enum "current"\|"off"\|"small"\|"medium"\|"large" = "current"; expectRev?: int` | `{painted: [{drawingId, objectId, objectKind, previousColourId}], skipped: [{target, reason}], addedStrokeIds: [id], rev}` | yes |
| `colour.paint.remove` | action | Unpaint zones or lines by ID or point ("Unpaint") | `drawingId?: id; objectIds?: [id]; points?: [point]; space: enum "drawing"\|"scene"\|"view" = "drawing"; sceneId?: id; frame?: int; view?: string; gapClosing: enum "current"\|"off"\|"small"\|"medium"\|"large" = "current"; expectRev?: int` | `{unpainted: [{drawingId, objectId, previousColourId}], skipped: [{target, reason}], rev}` | yes |
| `colour.paintUnpainted.apply` | action | Fill unpainted zones only, by ID, point, lasso or all ("Paint Unpainted") | `drawingId?: id; objectIds?: [id]; points?: [point]; lasso?: [point]; all: bool = false; space: enum "drawing"\|"scene"\|"view" = "drawing"; sceneId?: id; frame?: int; view?: string; colourId?: id; gapClosing: enum "current"\|"off"\|"small"\|"medium"\|"large" = "current"; expectRev?: int` | `{painted: [{drawingId, objectId}], skippedPainted: int, addedStrokeIds: [id], rev}` | yes |
| `colour.palette.changed` | event | Palette created, deleted, renamed, reordered, imported or edited | — | `{paletteId, change: enum "created"\|"deleted"\|"renamed"\|"imported"\|"edited"}` | n/a |
| `colour.palette.create` | action | Create a palette ("Add Palette") | `name?: string; preset: enum "blank"\|"default" = "blank"; styles?: [string]; swatches?: [object]; makeCurrent: bool = true` | `{paletteId, styleIds[], colourIds[]}` | yes |
| `colour.palette.delete` | action | Delete a palette and all its styles ("Remove Palette") | `paletteId: id; confirm: bool = false` | `{deleted: bool, orphanedReferences: int}` or `needs_confirmation {usageCount}` | yes |
| `colour.palette.duplicate` | action | Duplicate a palette with new IDs ("Duplicate Palette") | `paletteId: id; name?: string` | `{paletteId, idMap: object}` | yes |
| `colour.palette.export` | action | Write palettes to an `.animpal` file | `paletteIds: [id]; path: path; overwrite: bool = false` | `{path, paletteCount, textureCount}` | n/a |
| `colour.palette.get` | query | Full palette as JSON (§12.3) | `paletteId: id; includeUsage: bool = false; thumbnails: bool = false; thumbnailSize: int = 32 (8..256)` | Palette object (+ `usageCount` and base64 PNG `thumbnail` per swatch when requested) | n/a |
| `colour.palette.import` | action | Import palettes from an `.animpal` file ("Import Palette") | `path: path; onIdConflict: enum "copy"\|"replace"\|"skip" = "copy"; paletteNames?: [string]` | `{paletteIds[], renamed: [{from, to}], reIdentified: [id], missingTextures: [string]}` | yes |
| `colour.palette.list` | query | List palettes in order | `includeHidden: bool = false` | `{palettes: [{id, name, hidden, styles: [{id, name}], swatchCount}], currentPaletteId}` | n/a |
| `colour.palette.rename` | action | Rename a palette ("Rename Palette") | `paletteId: id; name: string` | `{paletteId, name}` | yes |
| `colour.palette.select` | ui | Make a palette current (selects its first swatch) | `paletteId: id` | `{paletteId, currentColourId}` | no |
| `colour.palette.set` | action | Replace or merge a palette's styles and swatches from a palette JSON object | `paletteId: id; palette: object; mode: enum "merge"\|"replace" = "merge"` | `{paletteId, created: [id], updated: [id], removed: [id]}` | yes |
| `colour.pick` | query | Colour at a rendered point (dropper read-back) | `point: point; space: enum "drawing"\|"scene"\|"view" = "view"; drawingId?: id; sceneId?: id; frame?: int; elementId?: id; view?: string` | `{colourId\|null, paletteId\|null, swatchName, rgba: colour, object: {drawingId, elementId, objectId, objectKind}\|null, missing: bool}` | n/a |
| `colour.recolour.apply` | action | Replace colour A with colour B across drawings/elements/scene/project ("Replace Colour") | `fromColourId?: id; toColourId?: id; map?: [{from: id, to: id}]; scope: enum "project"\|"scene"\|"element"\|"drawing" = "project"; sceneId?: id; elementIds?: [id]; drawingIds?: [id]; targets: [enum "zones"\|"lines"\|"brushStrokes"\|"text"] = ["zones","lines","brushStrokes","text"]; includePens: bool = false` | `{changed: [{drawingId, count}], pensChanged: [id], skippedLocked: [id]}` | yes |
| `colour.recovery.performed` | event | Colour recovery added swatches | — | `{colourIds[], paletteIds[]}` | n/a |
| `colour.recovery.run` | action | Recreate missing swatches from recovery copies ("Colour Recovery") | `drawingIds?: [id]; all: bool = false` | `{recovered: [{colourId, paletteId}], placeholders: [id], createdPaletteIds: [id]}` | yes |
| `colour.sceneStyle.changed` | event | A scene's style selection changed | — | `{sceneId, paletteId, styleId}` | n/a |
| `colour.sceneStyles.get` | query | Style selected for every palette in a scene | `sceneId?: id` | `{sceneId, styles: object}` (paletteId → styleId) | n/a |
| `colour.selection.apply` | action | Recolour selected drawing objects ("Set Colour") | `colourId?: id; drawingId?: id; objectIds?: [id]` | `{changed: [id], rev}` | yes |
| `colour.style.changed` | event | Style created, renamed, deleted or tinted | — | `{paletteId, styleId, change}` | n/a |
| `colour.style.delete` | action | Delete a style ("Delete Style") | `paletteId: id; styleId: id; replacementStyleId?: id` | `{reassignedSceneIds[]}` | yes |
| `colour.style.duplicate` | action | Duplicate a style, same colour IDs ("Duplicate Style") | `paletteId: id; styleId?: id; name?: string; selectForScene: bool = true; sceneId?: id` | `{styleId, name}` | yes |
| `colour.style.list` | query | List a palette's styles and which scenes use each | `paletteId: id` | `{styles: [{id, name, index, sceneIds[]}]}` | n/a |
| `colour.style.rename` | action | Rename a style ("Rename Style") | `paletteId: id; styleId: id; name: string` | `{styleId, name}` | yes |
| `colour.style.select` | action | Set the style of a palette for a scene ("Style Change") | `paletteId: id; styleId: id; sceneId?: id` | `{sceneId, paletteId, styleId}` | yes |
| `colour.swatch.changed` | event | Swatch created, edited, moved or deleted | — | `{paletteId, colourId, styleId?, change}` | n/a |
| `colour.swatch.create` | action | Add a colour or texture swatch ("Add Colour" / "Add Texture") | `paletteId?: id; name?: string; kind: enum "solid"\|"linearGradient"\|"radialGradient"\|"texture" = "solid"; value?: object; values?: object; copyFrom?: id; deformable: bool = false; index?: int; makeCurrent: bool = true` | `{colourId, paletteId, index}` | yes |
| `colour.swatch.delete` | action | Delete a swatch, optionally recolouring its uses ("Remove Colour") | `colourId: id; replaceWith?: id` | `{orphanedReferences: int, replaced: int}` | yes |
| `colour.swatch.get` | query | One swatch, all styles, plus value resolved for a scene | `colourId: id; sceneId?: id; thumbnail: bool = false; thumbnailSize: int = 64 (8..512)` | `{paletteId, index, swatch (§12.3), resolved: value, width?, height?, thumbnail?}` | n/a |
| `colour.swatch.list` | query | List swatches of a palette with values for a style | `paletteId?: id; styleId?: id; sceneId?: id; kind?: enum "solid"\|"linearGradient"\|"radialGradient"\|"texture"; includeUsage: bool = false` | `{paletteId, styleId, swatches: [{colourId, name, kind, index, deformable, value, usageCount?}]}` | n/a |
| `colour.swatch.move` | action | Reorder a swatch within its palette ("Move Colour") | `colourId: id; index: int` | `{colourId, index}` | yes |
| `colour.swatch.set` | action | Edit a swatch's name, kind, value(s) or flags ("Edit Colour") | `colourId: id; styleId?: id\|"all"; name?: string; kind?: enum "solid"\|"linearGradient"\|"radialGradient"; rgba?: colour; stops?: [{position: number, rgba: colour}]; image?: path; tile?: bool; deformable?: bool; values?: object` | `{swatch}` | yes |
| `colour.swatchNames.set` | toggle | Show or hide swatch names in the palette view | `visible: bool` | `{visible}` | no |
| `colour.tintBlend.apply` | action | Blend a colour into all colours of a style ("Blend Style") | `paletteId?: id; styleId?: id; colour: colour = #000000FF; blending: number = 50 (0..100); dryRun: bool = false` | `{changed: [{colourId, before, after}]}` | yes |
| `colour.tintDialog.open` | ui | Open the tint offset or tint blend UI for a style | `mode: enum "offset"\|"blend"; paletteId?: id; styleId?: id` | `{}` | no |
| `colour.tintOffset.apply` | action | Offset all colours of a style ("Offset Style") | `paletteId?: id; styleId?: id; mode: enum "rgb"\|"hsb" = "rgb"; red: int = 0 (-255..255); green: int = 0 (-255..255); blue: int = 0 (-255..255); hue: number = 0 (-360..360); saturation: number = 0 (-100..100); brightness: number = 0 (-100..100); alpha: int = 0 (-255..255); blending: number = 100 (0..100); dryRun: bool = false` | `{changed: [{colourId, before, after}]}` | yes |
| `colour.usage.list` | query | Where colours are used (by swatch or palette) | `colourId?: id; paletteId?: id; scope: enum "project"\|"scene"\|"element"\|"drawing" = "project"; sceneId?: id; elementIds?: [id]; drawingIds?: [id]; includePens: bool = true; includeObjectIds: bool = false` | `{uses: [{colourId, elementId, drawingId, sceneIds[], zones, lines, brushStrokes, texts, objectIds?}], pens: [{penId, colourId}], total}` | n/a |

## Namespace `element`

Defined in [`04_timeline_xsheet.md`](04_timeline_xsheet.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `element.annotation.set` | toggle | Set a drawing element as annotation element, or back | `elementId: id; annotation: bool` | `{elementId, type}` | yes |
| `element.childExposure.apply` | action | Extend the exposure of a peg's children up to a frame | `pegId: id; toFrame: int (≥1)` | `{changed: [{elementId, from, to}]}` | yes |
| `element.clone` | action | Clone a cell-based element (shared drawing bank, independent timing) | `elementId: id; name?: string` | `{elementId, name, cloneGroupId}` | yes |
| `element.collapsed.set` | toggle | Collapse / expand parent rows in the Timeline (saved with the scene) | `elementIds?: [id]; collapsed: bool` | `{elementIds[]}` | no |
| `element.colour.set` | action | Set marker and/or Timeline background colour; an empty string reverts to the category default | `elementIds?: [id]; marker?: colour; background?: colour` | `{elementIds[]}` | yes |
| `element.create` | action | Create one or more elements of a type (batch creation; the per-type add commands are this with `count: 1`) | `type: enum "drawing"\|"image"\|"sound"\|"media"\|"annotation"\|"peg"\|"camera"\|"skeleton"\|"colourTransform"\|"clipping"\|"dropShadow"\|"blur"\|"shake"\|"particleNature"\|"particleImpact"; name?: string; count: int = 1 (1–99); sceneId?: id; parentId?: id; above?: id; xsheetAfter?: id` | `{elementIds[], names[], warnings[]}` | yes |
| `element.current.move` | ui | Previous / next element in Timeline or Exposure Sheet order | `direction: enum "previous"\|"next"; order: enum "timeline"\|"xsheet" = "xsheet"` | `{elementId}` | no |
| `element.current.set` | ui | Make an element the current element | `elementId: id` | `{elementId}` | no |
| `element.delete` | action | Delete elements; children are re-attached to the deleted element's parent | `elementIds?: [id]` | `{deletedIds[], reparentedIds[], deletedDrawingCount}` | yes |
| `element.drawing.changed` | event | Drawing bank changed (created, renamed, deleted, locked, cleared) | — | `{elementId, drawingIds[], change}` | n/a |
| `element.drawing.create` | action | Create an empty drawing in the bank, optionally exposed over a range | `elementId: id; name?: string; exposeFrom?: int; exposeTo?: int` | `{drawingId, name}` | yes |
| `element.drawing.delete` | action | Delete drawings from the bank; exposed drawings need `ifInUse: "clearCells"` | `drawingIds: [id]; ifInUse: enum "fail"\|"clearCells" = "fail"` | `{deletedIds[], clearedCells: [{elementId, from, to}]}` or `needs_confirmation` | yes |
| `element.drawing.duplicate` | action | Copy a drawing into a new bank entry without exposing it | `drawingId: id; name?: string` | `{drawingId, name}` | yes |
| `element.drawing.list` | query | Drawings of an element's bank with usage and optional thumbnails | `elementId: id; filter: enum "all"\|"used"\|"unused" = "all"; thumbnails?: int (16–512)` | `{drawings: [{id, name, exposures: [{from, to}], useCount, locked, empty, missing, contentType, thumbnail?}]}` | n/a |
| `element.drawing.rename` | action | Rename a drawing (all cells, all clones) | `drawingId: id; name: string` | `{drawingId, name}` | yes |
| `element.drawingContent.delete` | action | Clear drawing: delete content, keep the drawing and its labels | `drawingIds?: [id]` | `{drawingIds[]}` | yes |
| `element.drawingLock.set` | toggle | Lock / unlock drawings | `drawingIds?: [id]; locked: bool` | `{drawingIds[]}` | yes |
| `element.duplicate` | action | Independent copy of elements including copies of their drawings | `elementIds?: [id]` | `{elementIds[], names[]}` | yes |
| `element.get` | query | Full state of one element (17.5) | `elementId: id` | element object | n/a |
| `element.hierarchy.get` | query | Parent, children and siblings of an element | `elementId: id` | `{parentId, childIds[], previousSiblingId, nextSiblingId, depth}` | n/a |
| `element.hierarchy.select` | ui | Select parent, children, first child, previous/next sibling or skeleton root | `relation: enum "parent"\|"children"\|"firstChild"\|"previousSibling"\|"nextSibling"\|"skeletonRoot"; elementId?: id` | `{selectedIds[]}` | no |
| `element.list` | query | List the elements of a scene in Timeline or Exposure Sheet order | `sceneId?: id; order: enum "timeline"\|"xsheet" = "timeline"; types?: [string]; parentId?: id; recursive: bool = true` | `{elements: [{id, name, type, parentId, depth, index}]}` | n/a |
| `element.lock.set` | toggle | Lock / unlock in the scene-planning views: given, all, all others | `elementIds?: [id]; locked: bool; apply: enum "given"\|"all"\|"others" = "given"` | `{changedIds[]}` | yes |
| `element.mediaLength.set` | action | Reset a media element's exposure to the natural media length, or set a length | `elementId: id; length?: int (≥1)` | `{elementId, from, to}` | yes |
| `element.note.set` | action | Set, update or remove (empty text) an element note | `elementId: id; text: string` | `{elementId}` | yes |
| `element.order.move` | action | Bring to front / forward, send backward / to back within the level (Timeline) or column order (Exposure Sheet) | `elementIds?: [id]; to: enum "front"\|"forward"\|"backward"\|"back"; order: enum "timeline"\|"xsheet" = "timeline"` | `{changed: bool, order: [id]}` | yes |
| `element.order.set` | action | Set the complete order of one Timeline level or of the Exposure Sheet columns (drag reordering) | `order: enum "timeline"\|"xsheet"; elementIds: [id]; parentId?: id; sceneId?: id` | `{order: [id]}` | yes |
| `element.parent.remove` | action | Detach elements from their parent (they move to the parent's level) | `elementIds?: [id]` | `{elementIds[], parentId}` | yes |
| `element.parent.set` | action | Attach elements to a peg, effect, skeleton or camera, or to the nearest peg/effect above (attach to next object) | `elementIds?: [id]; parentId?: id; nearest: bool = false; index?: int` | `{elementIds[], parentId}` | yes |
| `element.parentPeg.create` | action | Create a peg and attach the given elements to it | `elementIds?: [id]; name?: string` | `{pegId, name}` | yes |
| `element.properties.set` | action | Set composite role and deformable flag (transform values: section 05) | `elementId: id; role?: enum "normal"\|"foreground"\|"background"\|"envelope"; deformable?: bool` | `{elementId}` | yes |
| `element.rename` | action | Rename an element | `elementId: id; name: string` | `{elementId, name}` | yes |
| `element.select` | ui | Select elements (rows / columns) | `elementIds: [id]; mode: enum "replace"\|"add"\|"remove"\|"toggle" = "replace"` | `{selectedIds[]}` | no |
| `element.solo.set` | toggle | Solo mode on/off (view state) | `enabled: bool; elementIds?: [id]` | `{enabled, elementIds[]}` | no |
| `element.state.changed` | event | Elements created, deleted, renamed, reordered, reparented, or flags/properties changed | — | `{sceneId, elementIds[], change}` | n/a |
| `element.timing.set` | action | Change start frame, duration and loops of bar elements; move or stretch bars | `elementIds?: [id]; start?: int (≥1); startDelta?: int; duration?: int (≥1); anchor: enum "start"\|"end" = "start"; keyframes: enum "keep"\|"scale" = "keep"; loops?: int (≥1)` | `{elements: [{id, start, duration, loops}]}` | yes |
| `element.typeColour.get` | query | Default colours of all categories | — | `{colours: object}` | n/a |
| `element.typeColour.set` | ui | Set the default colour of an element category (preference) | `category: enum "drawings"\|"images"\|"sounds"\|"pegs"\|"skeletons"\|"media"\|"cameras"\|"effects"\|"effectParameters"; colour: colour` | `{category, colour}` | no |
| `element.unusedDrawing.list` | query | Find drawings not exposed in any cell, for given elements or the whole scene | `sceneId?: id; elementIds?: [id]` | `{items: [{elementId, drawingId, name}]}` | n/a |
| `element.visibility.set` | toggle | Show/hide in the scene or the Exposure Sheet for the given elements, all, or all others; optional type filter (e.g. all annotation elements) | `elementIds?: [id]; scope: enum "scene"\|"xsheet"; visible: bool; apply: enum "given"\|"all"\|"others" = "given"; types?: [string]` | `{changedIds[]}` | yes |

## Namespace `cell`

Defined in [`04_timeline_xsheet.md`](04_timeline_xsheet.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `cell.blank.add` | action | Insert blank cell(s) at a frame; following cells shift later | `elementIds?: [id]; frame?: int; count: int = 1 (1–9999)` | `{changed: [{elementId, from, to}]}` | yes |
| `cell.blank.remove` | action | Delete blank cell(s) at a frame; fails with `cell.notBlank` on a non-blank cell | `elementIds?: [id]; frame?: int; count: int = 1 (1–9999)` | `{changed: [{elementId, from, to}]}` | yes |
| `cell.clipboard.apply` | action | Paste the clipboard at a target, normally or as new drawings, with optional transfer options | `elementId?: id; frame?: int; mode: enum "paste"\|"newDrawings" = "paste"; options?: object` | `{elementIds[], from, to, createdDrawingIds[]}` | yes |
| `cell.clipboard.set` | action | Copy (or cut, undoable) an explicit range or whole elements to the clipboard | `elementIds?: [id]; from?: int; to?: int; wholeElements: bool = false; cut: bool = false; options?: object` | `{contentType, elementCount, frames}` | yes |
| `cell.columnate.apply` | action | Advanced cycle: write first..last drawings with exposure, loop mode and placement (13.10) | `elementId?: id; frame?: int; firstDrawing: string; lastDrawing: string; exposeEach: int = 1 (1–9999); exposeLast?: int (1–9999); loopMode: enum "none"\|"continuous"\|"forwardBackward" = "none"; loops?: int (1–999); totalFrames?: int; placement: enum "overwrite"\|"insert" = "overwrite"; dryRun: bool = false` | `{from, to, totalFrames, loops, sequence: [{drawing, from, to}]}` | yes |
| `cell.cycle.create` | action | Repeat the range so that it appears N times in total; following cells shift | `elementId?: id; from?: int; to?: int; cycles: int (1–999)` | `{from, to}` | yes |
| `cell.drawing.duplicate` | action | Duplicate drawing: copy the drawing at the range and expose the copy there | `elementId?: id; from?: int; to?: int; name?: string` | `{drawingId, name}` | yes |
| `cell.exposure.add` | action | Add exposure: hold longer by N frames, following cells shift later | `elementIds?: [id]; from?: int; to?: int; frames: int = 1 (1–9999)` | `{changed: [{elementId, from, to}]}` | yes |
| `cell.exposure.remove` | action | Remove exposure: shorten by N frames, following cells shift earlier | `elementIds?: [id]; from?: int; to?: int; frames: int = 1 (1–9999)` | `{changed: [{elementId, from, to}]}` | yes |
| `cell.exposure.set` | action | Set every exposure in the range to N frames (ones / twos / threes / N) | `elementIds?: [id]; from?: int; to?: int; length: int (1–9999)` | `{changed: [{elementId, from, to}]}` | yes |
| `cell.exposures.changed` | event | Cells of an element changed in a frame range | — | `{sceneId, elementId, from, to, rev}` | n/a |
| `cell.fill.apply` | action | Extend exposure: expose the drawing at `frame` through `toFrame`, overwriting | `elementIds?: [id]; frame?: int; toFrame: int (1–9999)` | `{changed: [{elementId, from, to}]}` | yes |
| `cell.label.set` | action | Change label / substitute the drawing in the range by name, id, or by stepping through the bank | `elementIds?: [id]; from?: int; to?: int; drawing?: string; drawingId?: id; step?: int; createMissing: bool = false` | `{drawingId, name, changed[]}` | yes |
| `cell.note.set` | action | Set, update or remove (empty text) a cell note | `elementId: id; frame: int; text: string` | `{elementId, frame}` | yes |
| `cell.range.add` | action | Insert cells (blank or named; one element or all elements) | `elementIds?: [id]; allElements: bool = false; frame?: int; count: int = 1 (1–9999); position: enum "before"\|"after" = "after"; named: bool = false` | `{from, to, drawingIds[]}` | yes |
| `cell.range.delete` | action | Delete cells: clear (leave blanks) or remove and shift earlier | `elementIds?: [id]; from?: int; to?: int; shift: bool = false` | `{changed: [{elementId, from, to}]}` | yes |
| `cell.range.move` | action | Move / copy / link / copy-as-new-drawings a cell range to another frame or element (drag-and-drop equivalent) | `sourceElementIds: [id]; from: int; to: int; targetElementId?: id; targetFrame: int; mode: enum "move"\|"copy"\|"link"\|"copyNew" = "move"; options?: object` | `{from, to, createdDrawingIds[]}` | yes |
| `cell.select` | ui | Select cells (frames × elements), a whole track, or a sound clip extent | `elementIds: [id]; from: int; to: int; mode: enum "replace"\|"add" = "replace"; extent: enum "cells"\|"track"\|"soundClip"\|"soundClipToStart"\|"soundClipToEnd" = "cells"` | `{elementIds[], from, to}` | no |
| `cell.selection.changed` | event | Cell or row selection or current position changed | — | `{elementIds[], from, to, currentElementId, currentFrame}` | n/a |
| `cell.selection.get` | query | Current cell/row selection and current position | — | `{elementIds[], from, to, rowsSelected, currentElementId, currentFrame}` | n/a |

## Namespace `timeline`

Defined in [`04_timeline_xsheet.md`](04_timeline_xsheet.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `timeline.backgroundColour.set` | ui | Timeline background colour preference | `colour: colour` | `{colour}` | no |
| `timeline.currentFrame.changed` | event | Current frame changed (throttled to at most 10 per second during playback) | — | `{sceneId, frame, source: "playback"\|"navigation"\|"command"}` | n/a |
| `timeline.currentFrame.move` | ui | First / previous / next / last frame, or previous / next drawing of an element; `withinRange` limits movement to the playback range | `direction: enum "first"\|"previous"\|"next"\|"last"; unit: enum "frame"\|"drawing" = "frame"; elementId?: id; count: int = 1; withinRange: bool = false` | `{frame, moved: bool}` | no |
| `timeline.currentFrame.set` | ui | Set the current frame (clamped to 1..scene length) | `frame: int (≥1); sceneId?: id` | `{frame, clamped: bool}` | no |
| `timeline.effectParameterRows.set` | toggle | Show/hide effect-parameter rows of one effect (omit = all effects) | `visible: bool; elementId?: id` | `{visible}` | no |
| `timeline.frameLabel.get` | query | Format frame numbers per the current time display | `frames: [int]` | `{labels: [string]}` | n/a |
| `timeline.playbackRange.changed` | event | Playback range changed | — | `{mode, start, end}` | n/a |
| `timeline.playbackRange.get` | query | Current playback range | — | `{mode, start, end}` | n/a |
| `timeline.playbackRange.set` | ui | Automatic or free playback range, start and end (clamped) | `mode?: enum "auto"\|"free"; start?: int; end?: int` | `{mode, start, end}` | no |
| `timeline.rows.list` | query | Timeline read model: rows, bars, keyframe markers, clips (17.5) | `sceneId?: id; from?: int; to?: int` | timeline object | n/a |
| `timeline.singleCellSelection.set` | toggle | Single cell selection preference | `enabled: bool` | `{enabled}` | no |
| `timeline.soundWaveforms.set` | toggle | Show sound rows as waveforms or solid bars | `enabled: bool` | `{enabled}` | no |
| `timeline.timeDisplay.set` | ui | Display frames or feet and frames (preference) | `unit: enum "frames"\|"feetFrames"` | `{unit}` | no |
| `timeline.transferOptions.get` | query | Read a stored option set | `operation: enum "paste"\|"cut"\|"dragDrop"` | `{options}` | n/a |
| `timeline.transferOptions.set` | ui | Store the paste-special, cut-special or drag-and-drop option set (17.3) | `operation: enum "paste"\|"cut"\|"dragDrop"; options: object` | `{operation, options}` | no |
| `timeline.view.set` | ui | Zoom level, scroll position, reveal an element (show Timeline selection) | `zoomLevel?: int; firstFrame?: int; revealElementId?: id` | `{zoomLevel, firstFrame, lastFrame}` | no |

## Namespace `xsheet`

Defined in [`04_timeline_xsheet.md`](04_timeline_xsheet.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `xsheet.columnWidth.set` | ui | Set one column's width, or reset all widths | `elementId?: id; width?: int (20–1000); reset: bool = false` | `{widths: object}` | no |
| `xsheet.currentCell.set` | ui | Set the current element and frame together | `elementId: id; frame: int (≥1)` | `{elementId, frame}` | no |
| `xsheet.display.set` | ui | Thumbnails, exposure text in held cells, element-name prefix, notes | `thumbnails?: bool; exposureText?: bool; elementNames?: bool; notes?: bool` | `{thumbnails, exposureText, elementNames, notes}` | no |
| `xsheet.exposures.get` | query | Exposure sheet of a frame window as run-length JSON (17.2) | `sceneId?: id; elementIds?: [id]; from: int = 1; to?: int; include?: [string]` | exposure-sheet object | n/a |
| `xsheet.exposures.set` | action | Atomically replace exposures of one or more columns over frame windows (17.2) | `sceneId?: id; columns: [object]; createMissingDrawings: bool = false; expectRev?: int; dryRun: bool = false` | `{rev, sceneLength, changed[], createdDrawings[]}` | yes |
| `xsheet.lightTable.add` | ui | Send drawings to the static light table (omit items = selection) | `items?: [object]` | `{itemIds[]}` | no |
| `xsheet.lightTable.changed` | event | Static light table contents or item state changed | — | `{itemIds[]}` | n/a |
| `xsheet.lightTable.list` | query | Light-table items in stacking order | — | `{items: [{itemId, elementId, drawingId, label, visible, placement}]}` | n/a |
| `xsheet.lightTable.move` | ui | Reorder a light-table item | `itemId: id; index: int (≥0)` | `{order: [id]}` | no |
| `xsheet.lightTable.remove` | ui | Remove light-table items, or all | `itemIds?: [id]; all: bool = false` | `{removedIds[]}` | no |
| `xsheet.lightTable.set` | ui | Show/hide and over/under for given items, all, or all others | `itemIds?: [id]; apply: enum "given"\|"all"\|"others" = "given"; visible?: bool; placement?: enum "over"\|"under"` | `{changedIds[]}` | no |
| `xsheet.panel.set` | ui | Show/hide the element list panel, static light table panel and toolbar | `elementList?: bool; lightTable?: bool; toolbar?: bool` | `{elementList, lightTable, toolbar}` | no |

## Namespace `anim`

Defined in [`05_scene_planning.md`](05_scene_planning.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `anim.bone.create` | action | Create a bone (chained to a parent bone, or freestanding) | `skeletonId: id; parentBoneId?: id; root?: point; tip: point; name?: string` | `{boneId}` | yes |
| `anim.bone.delete` | action | Delete bones | `boneIds: [id]; children: enum "reparent"\|"delete" = "reparent"` | `{removed}` | yes |
| `anim.bone.influence.set` | action | Influence type and zone | `boneId: id; type?: enum "infinite"\|"elliptical"; lengthRadius?: number; widthRadius?: number; regionPath?: [point]` | `{boneId}` | yes |
| `anim.bone.parent.set` | action | Reparent a bone (null = make it a root bone) | `boneId: id; parentBoneId: id \| null` | `{boneId}` | yes |
| `anim.bone.pose.set` | action | Pose bones by angles/lengths at a frame (keys) | `frame?: int; bones: [object {boneId, rotation?, length?, x?, y?}]; rotationSpace: enum "local"\|"skeleton" = "local"` | `{keyed: [{boneId, channels[]}]}` | yes |
| `anim.bone.reach.apply` | action | Natural movement: IK reach to a target, or FK rotation with follow-through | `boneId: id; frame?: int; mode: enum "ik"\|"fk"; target?: point; rotationDelta?: number` | `{keyed: [{boneId, rotation}], reached: bool, error: number}` | yes |
| `anim.bone.rest.set` | action | Set rest (setup) values | `boneId: id; x?: number; y?: number; rotation?: number; length?: number (0.01..)` | `{boneId}` | yes |
| `anim.camera.create` | action | Add a camera | `name?: string; parentId?: id; position?: object {x,y,z} = {0,0,16}; fov?: number = 41.1121 (1..179); makeCurrent: bool = false` | `{cameraId}` | yes |
| `anim.camera.current.get` | query | Current camera of a scene | `sceneId?: id` | `{cameraId}` | n/a |
| `anim.camera.current.set` | action | Make a camera the scene's current camera | `cameraId: id` | `{cameraId}` | yes |
| `anim.camera.currentChanged` | event | The current camera of a scene changed | — | `{sceneId, cameraId}` | n/a |
| `anim.camera.delete` | action | Delete a camera (not the last one) | `cameraId: id` | `{currentCameraId}` | yes |
| `anim.camera.get` | query | Camera properties and evaluated view at a frame | `cameraId?: id; frame?: int` | `{id, position, roll, fovMode, fov, effectiveFov, worldPosition, viewMatrix[16], frameCornersAtZ0[4], resolution {w,h}}` | n/a |
| `anim.camera.list` | query | Cameras of a scene | `sceneId?: id` | `{cameras: [{id, name, current}]}` | n/a |
| `anim.camera.select` | ui | Select the current camera | — | `{cameraId}` | no |
| `anim.camera.set` | action | Set camera position, roll, FOV mode and FOV (keyed at `frame` when dynamic) | `cameraId: id; position?: object {x,y,z}; roll?: number; fovMode?: enum "static"\|"dynamic"; fov?: number (1..179); frame?: int` | `{cameraId}` | yes |
| `anim.changed` | event | Node animation data changed | — | `{nodeIds[], channels[], frames?: {from, to}}` | n/a |
| `anim.channel.copy` | ui | Copy chosen channels' keys of a node | `nodeId: id; channels: [string]; from?: int; to?: int` | `{keyCount}` | no |
| `anim.channel.cut` | action | Cut special: copy then remove chosen channels' keys | `nodeIds?: [id]; channels: [string]; from?: int; to?: int` | `{keyCount}` | yes |
| `anim.channel.paste` | action | Paste copied channel keys at a frame | `nodeIds: [id]; frame: int; channels?: [string]; mode: enum "merge"\|"replace" = "merge"` | `{keyCount}` | yes |
| `anim.depth.move` | action | Depth nudge forward/backward | `nodeIds?: [id]; direction: enum "forward"\|"backward"; steps: int = 1 (1..1000)` | `{nodeIds, z[]}` | yes |
| `anim.display.allPegs.set` | toggle | Show all pegs' paths | `enabled: bool` | `{enabled}` | no |
| `anim.display.antialiasToolOverlays.set` | toggle | Anti-alias tool furniture | `enabled: bool` | `{enabled}` | no |
| `anim.display.arms.set` | toggle | Show element arms while rotating pegs | `enabled: bool` | `{enabled}` | no |
| `anim.display.changed` | event | A display option or preference of this area changed | — | `{option, value}` | n/a |
| `anim.display.envelopes.set` | toggle | Show bone influence envelopes | `enabled: bool` | `{enabled}` | no |
| `anim.display.frameMarkers.set` | toggle | Show frame markers on paths | `enabled: bool` | `{enabled}` | no |
| `anim.display.get` | query | All display options of §4 | — | `{allPegs, pivotPath, pegGhosts, arms, frameMarkers, effects, envelopes, smoothPaths, antialiasToolOverlays}` | n/a |
| `anim.display.pegGhosts.set` | toggle | Show peg ghosts | `enabled: bool` | `{enabled}` | no |
| `anim.display.pivotPath.set` | toggle | Show pivot paths | `enabled: bool` | `{enabled}` | no |
| `anim.display.smoothPaths.set` | toggle | Anti-alias motion paths | `enabled: bool` | `{enabled}` | no |
| `anim.evaluate` | query | Evaluate transforms of nodes at frames (no rendering) | `nodeIds: [id]; frames?: [int]; include?: [enum "static"\|"animated"\|"local"\|"world"\|"bounds"\|"screen"] = all; cameraId?: id` | `{frames: [{frame, nodes: [NodeEval]}]}` (§19.4) | n/a |
| `anim.flip.apply` | action | Flip nodes horizontally or vertically | `nodeIds?: [id]; axis: enum "horizontal"\|"vertical"; target: enum "static"\|"key" = "static"; frame?: int` | `{nodeIds}` | yes |
| `anim.function.curveType.set` | action | Change a channel's curve type | `nodeId: id; channel: string; curveType: enum "spline"\|"interpolation"\|"velocityBased"` | `{channel, curveType}` | yes |
| `anim.function.get` | query | Function curve as a key-point list | `nodeId: id; channel: string` | `Function` (§19.4) | n/a |
| `anim.function.key.add` | action | Add a key point (value defaults to current curve value) | `nodeId: id; channel: string; frame: number; value?: number` | `{frame, value}` | yes |
| `anim.function.key.duplicate` | action | Copy a key point to another frame | `nodeId: id; channel: string; frame: int; toFrame: int` | `{frame}` | yes |
| `anim.function.key.remove` | action | Remove key points | `nodeId: id; channel: string; frames: [number]` | `{removed}` | yes |
| `anim.function.key.set` | action | Edit a key point: move in time, value, handles, handle mode, constant flag, lock in time | `nodeId: id; channel: string; frame: number; newFrame?: number; value?: number; left?: object {dt,dv}; right?: object {dt,dv}; handleMode?: enum "smooth"\|"broken"; constantSegment?: bool; lockInTime?: bool` | `{frame, value}` | yes |
| `anim.function.list` | query | Channels of a node with curve type and key count | `nodeId: id` | `{channels: [{channel, curveType, keyCount, allowedCurveTypes[]}]}` | n/a |
| `anim.function.preset.apply` | action | Apply an easing preset to a segment or the whole curve | `nodeId: id; channel: string; preset: enum "constant"\|"linear"\|"easeInOut"\|"fastInOut"\|"extremeEaseInOut"\|"extremeFastInOut"\|"slowAcceleration"\|"slowDeceleration"; frame?: int; scope: enum "segment"\|"all" = "segment"` | `{segments}` | yes |
| `anim.function.sample` | query | Evaluate a channel over a frame range | `nodeId: id; channel: string; from: number; to: number; step: number = 1` | `{samples: [{frame, value}]}` | n/a |
| `anim.function.set` | action | Replace a 1-D channel's key list and curve type | `nodeId: id; channel: string; curveType?: enum "spline"\|"interpolation"\|"velocityBased"; keys: [object {frame, value, constantSegment?, handleMode?, left?, right?, preset?}]` | `{nodeId, channel, keyCount}` | yes |
| `anim.functionEditor.get` | query | Function-editor state | — | `{nodeId, channel, projection, selectedFrames[], syncFrame, view}` | n/a |
| `anim.functionEditor.open` | ui | Open the function editor on a node/channel/projection | `nodeId?: id; channel: string; projection?: enum "x"\|"y"\|"z"` | `{editorId}` | no |
| `anim.functionEditor.syncFrame.set` | ui | Sync the editor's selected frame with the current frame | `enabled: bool` | `{enabled}` | no |
| `anim.functionEditor.view.set` | ui | Function-editor view: fit, grid, zooms, panel and toolbar visibility, panel side | `fit?: bool; grid?: bool; zoomX?: number; zoomY?: number; showPanel?: bool; showToolbars?: bool; panelSide?: enum "left"\|"right"\|"top"\|"bottom"` | `{state}` | no |
| `anim.hierarchy.attach` | action | Attach/reparent nodes under a parent (or the next peg/effect above) | `childIds: [id]; parentId: id \| "next"; index?: int; keepWorld: bool = false` | `{childIds, parentId, approximated}` | yes |
| `anim.hierarchy.changed` | event | Attachment/order of nodes changed | — | `{sceneId, nodeIds[]}` | n/a |
| `anim.hierarchy.detach` | action | Detach nodes from their parent (up one level) | `nodeIds: [id]; keepWorld: bool = false` | `{nodeIds, approximated}` | yes |
| `anim.hierarchy.get` | query | Scene node tree with kinds and attachment | `sceneId?: id; rootId?: id` | `{nodes: [{id, kind, name, parentId, childIds[]}]}` | n/a |
| `anim.hierarchy.select` | ui | Select parent / descendants / first child / previous or next sibling | `fromId?: id; relation: enum "parent"\|"children"\|"firstChild"\|"prevSibling"\|"nextSibling"` | `{selectedIds}` | no |
| `anim.key.add` | action | Add keyframes (full pose or chosen channels); optionally on a whole hierarchy or at segment bounds | `nodeIds?: [id]; frame?: int; channels?: [string]; includeDescendants: bool = false; atSegmentBounds?: enum "left"\|"right"\|"both"` | `{keys: [{nodeId, frame, channels[]}]}` | yes |
| `anim.key.list` | query | Keyed frames and channels of a node | `nodeId: id; from?: int; to?: int; includeDescendants: bool = false` | `{keys: [{frame, channels[], constant[], descendantKeyed}]}` | n/a |
| `anim.key.move` | action | Move keys of a node to another frame | `nodeId: id; fromFrame: int; toFrame: int; channels?: [string]` | `{nodeId, toFrame}` | yes |
| `anim.key.remove` | action | Remove keyframes at a frame (default channels position/rotation/scale/skew) or all keyframes | `nodeIds?: [id]; frame?: int; channels?: [string]; all: bool = false` | `{removed}` | yes |
| `anim.loop.set` | action | Set looping of a peg, effect, skeleton or bone | `nodeId: id; loops: int = 1 (1..10000); loopType: enum "count"\|"fill" = "count"; positionLoop: enum "cycle"\|"offset" = "cycle"` | `{nodeId, cycleLength, duration}` | yes |
| `anim.node.get` | query | All animation properties of a node | `nodeId: id; frame?: int` | `{id, kind, static, options, channels[], loop, parentId, childIds[]}` | n/a |
| `anim.node.options.set` | action | Set non-transform node options | `nodeIds?: [id]; orientToPath?: bool; keepProportions?: bool; rotateAboutPivot?: bool; pivotAppliesTo?: enum "rotation"\|"scaling"\|"both"; lockDepth?: bool; useChildDrawingPivot?: bool; deformable?: bool; role?: enum "normal"\|"background"; groupOnExport?: bool; compositeForExportGroup?: bool` | `{nodeIds}` | yes |
| `anim.node.pathDisplay.set` | ui | Per-node motion-path display switch (as in the Timeline) | `nodeIds: [id]; visible: bool` | `{nodeIds}` | no |
| `anim.path.get` | query | Motion path with all points, parameters and arc lengths | `nodeId: id` | `Path` (§19.4) | n/a |
| `anim.path.lockDepth.set` | action | Constant Z for the segment starting at a keyframe | `nodeId: id; frame: int; enabled: bool` | `{nodeId, frame}` | yes |
| `anim.path.point.add` | action | Add a keyframe or control point (by frame, or at a curve location) | `nodeId: id; kind: enum "key"\|"control"; frame?: number; at?: object {segment: int, s: number (0..1)}; position?: object {x,y,z}` | `{pointId, frame}` | yes |
| `anim.path.point.remove` | action | Delete motion points | `pointIds: [id]` | `{removed, warnings[]}` | yes |
| `anim.path.point.select` | ui | Select a motion point, or the previous/next one | `pointIds?: [id]; nodeId?: id; relation?: enum "next"\|"prev"; moveCurrentFrame: bool = true` | `{pointIds, frame}` | no |
| `anim.path.point.set` | action | Edit a motion point (position, frame, lock in time, TCB, constant, per-key rotation/scale/pivot) | `pointIds: [id]; x?: number; y?: number; z?: number; frame?: int; lockInTime?: bool; tension?: number (-1..1); continuity?: number (-1..1); bias?: number (-1..1); constantSegment?: bool; rotation?: number; scale?: point; pivot?: point` | `{pointIds, warnings[]}` | yes |
| `anim.path.sample` | query | Positions per frame (frame markers, ghosts, pivot path) | `nodeId: id; from?: int; to?: int; step: number = 1; what: enum "origin"\|"pivot" = "origin"; space: enum "parent"\|"world" = "world"` | `{samples: [{frame, x, y, z}]}` | n/a |
| `anim.path.set` | action | Replace a node's whole motion path | `nodeId: id; points: [object {kind, frame?, x, y, z, tension?, continuity?, bias?, constantSegment?, lockDepth?}]; lockDepth?: bool` | `{nodeId, pointIds[], warnings[]}` | yes |
| `anim.peg.create` | action | Add a peg; with `wrapIds`, add a parent peg around those nodes | `name?: string; parentId?: id; aboveId?: id; wrapIds?: [id]` | `{pegId}` | yes |
| `anim.pegOnlyMode.set` | toggle | Peg-only mode on/off (§6.1) | `enabled: bool` | `{enabled}` | no |
| `anim.point.copy` | ui | Copy a motion point to the clipboard | `pointId: id` | `{pointId, isKeyframe}` | no |
| `anim.point.paste` | action | Paste a copied point (all properties, or chosen ones = paste special; `again` repeats the last choice) | `targetPointIds: [id]; properties?: [enum "x"\|"y"\|"z"\|"tension"\|"continuity"\|"bias"\|"rotation"\|"rotationPivot"\|"scaleX"\|"scaleY"\|"scalePivot"]; again: bool = false` | `{applied[], skipped[]}` | yes |
| `anim.prefs.get` | query | Scene-planning preferences (§16) | — | `{defaultTension, defaultContinuity, defaultBias, units, displayUnits, boxHighlighting, keepSelectionVisible, linearByDefault, constantKeysByDefault, depthNudgeStep}` | n/a |
| `anim.prefs.set` | ui | Set scene-planning preferences | `defaultTension?: number (-1..1); defaultContinuity?: number (-1..1); defaultBias?: number (-1..1); units?: enum "fields"\|"inches"\|"centimetres"; displayUnits?: bool; boxHighlighting?: bool; keepSelectionVisible?: bool; linearByDefault?: bool; constantKeysByDefault?: bool; depthNudgeStep?: number (0.0001..1)` | `{prefs}` | no |
| `anim.projection.get` | query | Project world points or node bounds into output pixels | `frame?: int; cameraId?: id; points?: [object]; nodeIds?: [id]` | `{points: [{x, y, depth, visible}], nodes: [{id, bbox {x,y,width,height}, visible}]}` | n/a |
| `anim.record.completed` | event | A motion recording (interactive or by command) finished | — | `{nodeId, firstFrame, lastFrame}` | n/a |
| `anim.record.options.get` | query | Motion-recording options | — | `{captureSpeed, smoothness, autoExtendExposure}` | n/a |
| `anim.record.options.set` | ui | Motion-recording options (persisted user settings) | `captureSpeed?: number (0.1..10); smoothness?: int (0..100); autoExtendExposure?: bool` | `{options}` | no |
| `anim.record.run` | action | Record motion from a sample list (§14.4) | `nodeId: id; kind: enum "translate"\|"rotate"\|"scale"; handle?: enum "corner"\|"horizontal"\|"vertical" = "corner"; startFrame?: int; samples: [object {t, x, y}]; captureSpeed?: number; smoothness?: int; autoExtendExposure?: bool` | `{firstFrame, lastFrame, keyCount, channels[]}` | yes |
| `anim.reset.apply` | action | Reset nodes: remove keys and restore default static values | `nodeIds?: [id]` | `{nodeIds}` | yes |
| `anim.segment.set` | action | Make segments starting at a keyframe constant or tweened | `nodeIds?: [id]; frame: int; channels?: [string]; constant: bool` | `{nodeIds, channels[]}` | yes |
| `anim.segment.uniform.apply` | action | Make the segment left/right of a key uniform (linear) | `nodeId: id; frame: int; side: enum "left"\|"right"; channels?: [string]` | `{nodeId, channels[]}` | yes |
| `anim.skeleton.create` | action | Add a skeleton, optionally moving elements into its skin group | `name?: string; parentId?: id; skinIds?: [id]` | `{skeletonId, boneGroupId, skinGroupId}` | yes |
| `anim.skeleton.deform.get` | query | Deformed positions and weights of given rest-space points | `skeletonId: id; frame?: int; points: [point]` | `{points: [{x, y, weights: [{boneId, w}]}]}` | n/a |
| `anim.skeleton.delete` | action | Delete a skeleton (skin elements are moved out to its level) | `skeletonId: id; keepSkin: bool = true` | `{skeletonId}` | yes |
| `anim.skeleton.get` | query | Bone tree with rest and current values | `skeletonId: id; frame?: int` | `Skeleton` (§19.4) | n/a |
| `anim.skeleton.restPose.apply` | action | Copy the rest pose to the current frame or to all keyframes | `skeletonId: id; scope: enum "frame"\|"allKeyframes" = "frame"; frame?: int` | `{frames[]}` | yes |
| `anim.skeleton.root.select` | ui | Select the skeleton root of the current bone/skin element | `fromId?: id` | `{skeletonId}` | no |
| `anim.skeleton.set` | action | Skeleton display and quality options | `skeletonId: id; boneColour?: colour; showTrack?: bool; showRoot?: bool; showBones?: bool; showInfluence?: bool; vectorQuality?: enum "low"\|"medium"\|"high"\|"highest"; deformationEnabled?: bool` | `{skeletonId}` | yes |
| `anim.skeleton.skin.add` | action | Put elements into the skin group | `skeletonId: id; elementIds: [id]` | `{elementIds}` | yes |
| `anim.skeleton.skin.remove` | action | Take elements out of the skin group | `skeletonId: id; elementIds: [id]` | `{elementIds}` | yes |
| `anim.snapLastKey.set` | toggle | Snap-last-keyframe on/off (§6.5) | `enabled: bool` | `{enabled}` | no |
| `anim.stretch.apply` | action | Re-time a node's channels proportionally to a new duration | `nodeId: id; duration: int (1..)` | `{nodeId, movedKeys, mergedKeys}` | yes |
| `anim.tool.changed` | event | Scene-planning tool changed | — | `{tool}` | n/a |
| `anim.tool.get` | query | Current scene-planning tool | — | `{tool}` | n/a |
| `anim.tool.set` | ui | Select the active scene-planning tool (independent of the drawing tool) | `tool: enum "select"\|"transform"\|"record"\|"rotate"\|"skew"\|"scale"\|"motion"\|"boneCreate"\|"boneSetup"\|"boneManipulate"\|"boneInverse"\|"boneForward"\|"boneInfluence"\|"boneParent"; cycleRotate?: bool = false` | `{tool}` | no |
| `anim.transform.set` | action | Set static values or key animated values (Select/Transform/Rotate/Scale/Skew tool equivalent) | `nodeIds?: [id]; target: enum "static"\|"key" = "key"; frame?: int; position?: object {x,y,z}; positionSpace?: enum "parent"\|"world" = "parent"; rotation?: number; scale?: point; skew?: number (-89..89); pivot?: point; rotationPivot?: point; scalePivot?: point; relative?: bool = false` | `{nodeIds, keyedChannels[], warnings[]}` | yes |
| `anim.view.coords.get` | query | Map points between scene space and view pixels (for pointer simulation) | `viewId: id; points: [object]; from: enum "scene"\|"view"; depth?: number = 0` | `{points: [object]}` | n/a |
| `anim.view.get` | query | View state of a scene view | `viewId: id` | `{type, zoom, pan {x,y}, rotation, visibleRect {minX,minY,maxX,maxY}, slot}` | n/a |
| `anim.view.slot.select` | ui | Switch active view: store current state and restore the other slot | `viewId: id; slot?: enum "A"\|"B"\|"other" = "other"` | `{viewId, slot, state}` | no |
| `anim.view.type.set` | ui | Switch a view window between Drawing, Camera, Top and Side | `viewId: id; type: enum "drawing"\|"camera"\|"top"\|"side"` | `{viewId, type}` | no |

## Namespace `fx`

Defined in [`06_effects.md`](06_effects.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `fx.allEffects.changed` | event | All-effects display switch changed | — | `{enabled}` | n/a |
| `fx.allEffects.get` | query | Current all-effects display state | — | `{enabled}` | n/a |
| `fx.allEffects.set` | toggle | Enable or disable effect display in interactive views | `enabled: bool` | `{enabled}` | no |
| `fx.block.add` | action | Add a block at a frame (split or append) ("Add Particle Block") | `effectId: id; frame?: int = current; type?: string; variant?: string; presetId?: id; seed?: int` | `{blockId, startFrame, duration}` | yes |
| `fx.block.defaults.apply` | action | Reset a block's parameters to its type's defaults ("Reset to Defaults") | `blockId: id; keepTiming?: bool = true; keepSeed?: bool = true` | `{block: Block}` | yes |
| `fx.block.get` | query | Full block record (static values, dynamic curves/keyframes, seed, sprite set) | `blockId: id` or `effectId: id; frame: int` | `Block` | n/a |
| `fx.block.list` | query | List a particle effect's blocks | `effectId: id` | `{blocks: [BlockSummary]}` | n/a |
| `fx.block.remove` | action | Remove a block; frames merge into a neighbour ("Remove Particle Block") | `blockId?: id; effectId?: id; frame?: int` | `{mergedIntoBlockId}` | yes |
| `fx.block.seed.set` | action | Set a block's seed, or draw a new random one | `blockId: id; seed?: int (0–4294967295)` | `{seed}` | yes |
| `fx.block.set` | action | Set block static values (timing, background, region, floor, caps, lifetime, fade, sprite set) ("Change Particle Settings") | `blockId: id; values: object; clamp?: bool = false` | `{block: Block, clamped: [string]}` | yes |
| `fx.block.type.set` | action | Change a block's particle type and variant, loading that type's defaults ("Change Particle Type") | `blockId: id; type: string; variant?: string` | `{block: Block}` | yes |
| `fx.changed` | event | An effect's parameters, keyframes, timing or slots changed | — | `{effectId, blockId?, what: enum "params"\|"keyframes"\|"timing"\|"slots"\|"blocks"}` | n/a |
| `fx.create` | action | Create one or more effects with full parameters ("Create Effect") | `type: enum (effect type); name?: string (1–20 chars); count?: int = 1 (1–99); sceneId?: id; parentId?: id; index?: int; childIds?: [id]; startFrame?: int; duration?: int; loops?: int = 1 (1–999); params?: object; presetId?: id; blocks?: [BlockSpec]` | `{effectIds: [id], paramElementIds: {slot: id}, warnings: [code]}` | yes |
| `fx.curve.get` | query | Function-curve ids of animatable parameters (for editing with section 05 commands) | `effectId: id; blockId?: id; param?: string` | `{curves: [{param, functionId}]}` | n/a |
| `fx.dropShadow.transform.set` | action | Set or reset the shadow transform ("Change Drop Shadow") | `effectId: id; matrix?: {xAxis: point, yAxis: point, origin: point}; offset?: point; scale?: point; skew?: number (−89–89); reset?: bool = false` | `{transform}` | yes |
| `fx.get` | query | Full effect record (static values, keyframes, blocks, slots, timing) | `effectId: id; includeCurves?: bool = false` | `Effect` | n/a |
| `fx.hitTest.run` | query | Identify pickable objects under a view position | `viewId: id; x: number; y: number (view pixels); radius?: int = 3 (0–20)` | `{hits: [{objectId, kind, elementId}]}` (top-most first) | n/a |
| `fx.keyframe.add` | action | Add an effect (or block) keyframe, optionally with values ("Add Effect Keyframe") | `effectId: id; frame?: int = current; blockId?: id; values?: object` | `{frame, created: bool}` | yes |
| `fx.keyframe.list` | query | List keyframes with their values | `effectId: id; blockId?: id` | `{keyframes: [{frame, values}]}` | n/a |
| `fx.keyframe.remove` | action | Remove a keyframe ("Remove Effect Keyframe") | `effectId: id; frame: int; blockId?: id` | `{removed: bool, nowStatic: bool}` | yes |
| `fx.list` | query | List effects in a scene | `sceneId?: id; type?: enum (effect type); underElementId?: id` | `{effects: [{id, name, type, parentId, startFrame, duration, loops, animated, paramElementIds}]}` | n/a |
| `fx.paramElement.add` | action | Attach elements to a parameter slot (mask, sprites, bones, skin) | `effectId: id; slot: enum "mask"\|"sprites"\|"bones"\|"skin"; elementIds: [id]; index?: int` | `{attachedIds: [id]}` | yes |
| `fx.paramElement.list` | query | List an effect's parameter elements and the elements attached to each | `effectId: id` | `{slots: [{slot, elementId, attachedIds: [id]}]}` | n/a |
| `fx.paramElement.remove` | action | Detach elements from a parameter slot | `effectId: id; slot: enum "mask"\|"sprites"\|"bones"\|"skin"; elementIds: [id]; newParentId?: id` | `{detachedIds: [id]}` | yes |
| `fx.paramRows.get` | query | Parameter-row visibility per effect | `sceneId?: id` | `{effects: [{effectId, visible}]}` | n/a |
| `fx.paramRows.set` | ui | Show or hide parameter rows in the Timeline for given effects or all effects | `scope: enum "effects"\|"all" = "effects"; effectIds?: [id] (default: selection); visible: bool` | `{changedEffectIds: [id]}` | no |
| `fx.params.get` | query | Resolved parameter values at a frame plus derived values | `effectId: id; frame?: int = current; blockId?: id` | `{values: object, isKeyframe: bool, derived: object}` | n/a |
| `fx.params.set` | action | Set any subset of an effect's (or block's) parameters ("Change Effect Parameters") | `effectId: id; values: object; frame?: int = current; blockId?: id; keyMode?: enum "auto"\|"static" = "auto"; clamp?: bool = false; confirm?: bool = false` | `{values: object, keyframeCreated: bool, clamped: [string]}` or `needs_confirmation` | yes |
| `fx.particles.get` | query | Deterministic particle snapshot of a block at a frame | `effectId: id; frame: int; blockId?: id; space?: enum "effect"\|"scene" = "effect"; limit?: int = 3000 (1–3000); fields?: [string]` | `{blockId, seed, frame, count, truncated: bool, particles: [{index, pos: {x,y,z}, vel: {x,y,z}, size, rotation, alpha, age, variant}]}` | n/a |
| `fx.preset.apply` | action | Apply a preset to an effect or particle block ("Apply Preset") | `presetId: id; effectId: id; blockId?: id; frame?: int` | `{effectId, blockId?, values: object}` | yes |
| `fx.preset.changed` | event | User preset library changed | — | `{presetId, change: enum "created"\|"renamed"\|"deleted"\|"imported"}` | n/a |
| `fx.preset.create` | action | Save an effect's or block's current parameters (or an explicit body) as a user preset | `name: string (1–60); effectId?: id; blockId?: id; body?: Preset; tags?: [string]; includeSeed?: bool = false; onConflict?: enum "fail"\|"replace"\|"rename" = "fail"` | `{presetId}` or `needs_confirmation` | no |
| `fx.preset.delete` | action | Delete a user preset | `presetId: id; confirm?: bool = false` | `{deleted: bool}` or `needs_confirmation` | no |
| `fx.preset.export` | action | Export user (or built-in) presets to a preset file | `presetIds: [id]; path: path; overwrite?: bool = false` | `{path}` | no |
| `fx.preset.get` | query | Full preset body, optionally with thumbnail PNG | `presetId: id; includeThumbnail?: bool = false` | `Preset` | n/a |
| `fx.preset.import` | action | Import user presets from a preset file | `path: path; onConflict?: enum "fail"\|"replace"\|"rename" = "rename"` | `{presetIds: [id], skipped: [string]}` | no |
| `fx.preset.list` | query | List presets | `effectType?: enum (effect type); particleType?: string; source?: enum "builtin"\|"user"\|"all" = "all"; tags?: [string]` | `{presets: [{id, name, source, effectType, particleType, variant, tags}]}` | n/a |
| `fx.preset.rename` | action | Rename a user preset | `presetId: id; name: string; onConflict?: enum "fail"\|"rename" = "fail"` | `{name}` | no |
| `fx.preview.cache.delete` | ui | Clear the rendered-frame cache | — | `{freedMB}` | no |
| `fx.preview.close` | ui | Close the effect preview panel | — | `{}` | no |
| `fx.preview.frame.get` | query | Cached preview render of a frame as PNG | `frame?: int = current; size?: enum "quarter"\|"half"\|"full"` | `{cached: bool, width, height, png?}` | n/a |
| `fx.preview.frameRendered` | event | A preview frame finished rendering | — | `{frame, size, jobId}` | n/a |
| `fx.preview.invalidated` | event | Preview cache frames invalidated by an edit | — | `{frames: [int] \| "all"}` | n/a |
| `fx.preview.open` | ui | Open (or focus) the effect preview panel | — | `{panelId}` | no |
| `fx.preview.options.get` | query | Effect preview settings | — | `{size, followCurrentFrame, defaultRange}` | n/a |
| `fx.preview.options.set` | ui | Change effect preview settings | `size?: enum "quarter"\|"half"\|"full"; followCurrentFrame?: bool; defaultRange?: enum "single"\|"playbackRange"\|"fromCurrent"` | `{size, followCurrentFrame, defaultRange}` | no |
| `fx.preview.render` | job | Render frames into the effect preview cache | `range?: enum "single"\|"playbackRange"\|"fromCurrent" = defaultRange; startFrame?: int; endFrame?: int; size?: enum "quarter"\|"half"\|"full"` | `{jobId}` → `{framesRendered}` | n/a |
| `fx.preview.status.get` | query | Rendering state and cache usage | — | `{rendering, jobId?, cacheUsedMB, cacheBudgetMB, usedPercent, cachedFrames: [{frame, size}]}` | n/a |
| `fx.preview.stop` | ui | Stop the running preview render (same as `job_cancel`) | `jobId?: id` | `{stopped: bool}` | no |
| `fx.renderList.get` | query | Compositor output for a frame: ordered drawables with transforms, colour transforms and effect scopes | `sceneId?: id; frame: int; quality?: enum "draft"\|"final" = "final"; includeParticles?: bool = false` | `{items: [RenderItem]}` | n/a |
| `fx.renderer.capabilities.get` | query | Renderer capabilities and computed recommendations | — | `{gpuAccelerated, maxTextureSize, interactiveAntialiasingSupported, systemMemoryMB, recommendedTextureCacheMB}` | n/a |
| `fx.renderer.options.changed` | event | Renderer options changed | — | `{values, restartRequired}` | n/a |
| `fx.renderer.options.get` | query | Display-quality and renderer options (§6.9) | — | `{values: object}` | n/a |
| `fx.renderer.options.set` | ui | Change display-quality and renderer options | `values: object` | `{values: object, restartRequired: bool}` | no |
| `fx.shake.displacement.get` | query | Shake displacement samples | `effectId: id; startFrame?: int; endFrame?: int` | `{samples: [{frame, dx, dy, dz}]}` (fields) | n/a |
| `fx.simulation.run` | job | Pre-simulate a block up to a frame (warms the cache) | `effectId: id; blockId?: id; toFrame: int` | `{jobId}` → `{framesSimulated, peakCount}` | n/a |
| `fx.spriteSet.get` | query | Sprite-set details and variant thumbnails | `spriteSetId: id; includeThumbnails?: bool = false` | `{id, name, variants: [{index, thumbnail?}]}` | n/a |
| `fx.spriteSet.list` | query | List built-in sprite sets | `particleType?: string` | `{spriteSets: [{id, name, particleTypes, variantCount}]}` | n/a |
| `fx.timing.set` | action | Change start frame, duration and/or loop count ("Change Effect Timing") | `effectId: id; startFrame?: int; duration?: int (≥1); loops?: int (1–999); scaleKeys?: bool = true; confirm?: bool = false` | `{startFrame, duration, loops, removedKeyframes: [int]}` or `needs_confirmation` | yes |
| `fx.types.get` | query | Descriptor of one effect type, optionally narrowed to one particle type | `effectType: enum (as above); particleType?: string` | `TypeDescriptor` | n/a |
| `fx.types.list` | query | List all effect types with parameter JSON schemas, slots, flags, particle types, variants, defaults, caps and sprite sets | `effectType?: enum "skeleton"\|"colourTransform"\|"clipping"\|"dropShadow"\|"blur"\|"shake"\|"natureParticles"\|"actionParticles"; includeDefaults?: bool = true` | `{types: [TypeDescriptor]}` | n/a |

## Namespace `sound`

Defined in [`07_sound_lipsync.md`](07_sound_lipsync.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `sound.asset.get` | query | Get one asset's format, status and users | `assetId: id` | `{assetId, name, channels, bitDepth, sampleRate, durationSec, fileSize, missing, usedBy: [{placementId, elementId}]}` | n/a |
| `sound.asset.import` | action | Add an audio file to the project's sound assets without placing it | `path: path; allowLarge?: bool = false` | `{assetId, reusedAsset, durationSec}` | yes |
| `sound.asset.list` | query | List sound assets of the project | `missingOnly?: bool = false; unusedOnly?: bool = false` | `{assets: [{assetId, name, channels, bitDepth, sampleRate, durationSec, missing, placementCount}]}` | n/a |
| `sound.asset.missing` | event | A referenced asset could not be found on open | `{assetId, path}` | — | n/a |
| `sound.asset.probe` | query | Inspect an audio (or movie) file without importing it | `path: path` | `{decodable, format, channels, bitDepth, sampleRate, durationSec, fileSize, exceedsLimit}` | n/a |
| `sound.asset.relink` | action | Point a (missing) asset at a new file; placements keep their timing | `assetId: id; path: path` | `{assetId, durationSec, durationChanged}` | yes |
| `sound.editor.close` | action | Close the editor, committing all its edits as one *Edit Sound* step or discarding them | `commit: bool` | `{committed}` | yes |
| `sound.editor.open` | ui | Open the sound element editor on an element | `elementId: id; placementId?: id` | `{editorId}` | no |
| `sound.editor.state.get` | query | Editor state | — | `{open, elementId, selectedPlacementId, currentFrame, zoom, scrubbing, loop, dirty}` | n/a |
| `sound.editor.state.set` | ui | Set editor current frame, selection, zoom, scrubbing, loop | `currentFrame?: int; selectedPlacementId?: id; zoom?: int (0–100); scrubbing?: bool; loop?: bool` | editor state | no |
| `sound.element.changed` | event | Placements, envelope or flags of a sound element changed | `{elementId, placementIds[]}` | — | n/a |
| `sound.element.streamed.set` | toggle | Mark a sound element for streamed SWF export | `elementId: id; enabled: bool` | `{enabled}` | yes |
| `sound.envelope.clear` | action | Remove the envelope (unity gain) | `placementId: id` | `{}` | yes |
| `sound.envelope.get` | query | Read a placement's envelope | `placementId: id` | `{points: [{time, left, right}]}` | n/a |
| `sound.envelope.point.add` | action | Add one envelope point | `placementId: id; time: number; level?: number (0–1) = interpolated; left?: number; right?: number` | `{index, points[]}` | yes |
| `sound.envelope.point.move` | action | Move/re-level one envelope point (time clamped between neighbours) | `placementId: id; index: int; time?: number; level?: number (0–1); left?: number; right?: number` | `{points[]}` | yes |
| `sound.envelope.point.remove` | action | Remove one envelope point | `placementId: id; index: int` | `{points[]}` | yes |
| `sound.envelope.set` | action | Replace the whole envelope | `placementId: id; points: [{time: number, level?: number (0–1), left?: number, right?: number}]` | `{points[]}` | yes |
| `sound.fade.apply` | action | Apply a fade preset | `placementId: id; type: enum "in"\|"out"\|"inOut"; inDuration?: number = 0.5; outDuration?: number = 0.5` | `{points[]}` | yes |
| `sound.import` | action | Import an audio file and place it in a sound element (Import Sound) | `path: path; elementId?: id; newElementName?: string; sceneId?: id; frame?: int = current frame (≥1); onOverlap?: enum "insertShift"\|"reject"\|"nextFree" = "insertShift"; allowLarge?: bool = false` | `{elementId, placementId, assetId, startFrame, stopFrame, durationSec, reusedAsset, warnings[]}` | yes |
| `sound.placement.add` | action | Place an existing asset or Library sound item in a sound element | `elementId: id; assetId?: id; libraryItemId?: id; frame: int (≥1); onOverlap?: enum "insertShift"\|"reject"\|"nextFree" = "reject"` | `{placementId, startFrame, stopFrame}` | yes |
| `sound.placement.get` | query | Full parameters of one placement | `placementId: id` | `Placement` (§15.4) | n/a |
| `sound.placement.list` | query | List placements of an element or a scene | `elementId?: id; sceneId?: id` | `{placements: [Placement]}` (§15.4) | n/a |
| `sound.placement.move` | action | Move a placement to a new start frame, keeping its length | `placementId: id; startFrame: int (≥1); onOverlap?: enum "reject"\|"clamp"\|"insertShift" = "reject"` | `{startFrame, stopFrame}` | yes |
| `sound.placement.remove` | action | Remove a placement (Remove Sound) | `placementId: id` | `{}` | yes |
| `sound.placement.set` | action | Set loop count and/or volume (and any trim/move field) of a placement | `placementId: id; loops?: int (≥1); volume?: int (−50–50); inPoint?: number; outPoint?: number; startFrame?: int; stopFrame?: int \| "natural"` | `Placement` | yes |
| `sound.placement.split` | action | Split a placement into two at a frame | `placementId: id; frame: int` | `{firstPlacementId, secondPlacementId}` | yes |
| `sound.placement.trim` | action | Set in/out points (source seconds) and/or truncate at a stop frame | `placementId: id; inPoint?: number (≥0); outPoint?: number (≤ duration); stopFrame?: int \| "natural"` | `{inPoint, outPoint, startFrame, stopFrame, naturalEndFrame, clamped}` | yes |
| `sound.play` | ui | Play audio without picture: one placement, one element's mix, an asset, or the scene mix | `placementId?: id; elementId?: id; assetId?: id; sceneId?: id; fromFrame?: int; toFrame?: int; loop?: bool = false` | `{playbackId}` | no |
| `sound.playback.set` | toggle | Enable/disable sound during interactive playback (per user) | `enabled: bool` | `{enabled}` | no |
| `sound.range.delete` | action | Empty a frame range of a sound element, splitting and trimming placements (§7.2) | `elementId: id; startFrame: int; endFrame: int; closeGap?: bool = false` | `{removedPlacementIds[], createdPlacementIds[]}` | yes |
| `sound.range.select` | ui | Select frames of a placement in the Exposure Sheet/Timeline (whole, from start, to end) | `placementId: id; mode: enum "whole"\|"fromStart"\|"toEnd" = "whole"; frame?: int = current frame` | `{elementId, startFrame, endFrame}` | no |
| `sound.recent.list` | query | Recently imported sounds (for quick re-import) | `limit?: int = 10 (1–50)` | `{items: [{path, assetId?, name}]}` | n/a |
| `sound.scrubbing.set` | toggle | Enable/disable Timeline sound scrubbing (per user) | `enabled: bool` | `{enabled, effective}` | no |
| `sound.settings.get` | query | Read sound playback/scrubbing/waveform settings and audio device status | — | `{playback, scrubbing, waveforms, deviceAvailable, outputDevice}` | n/a |
| `sound.silence.list` | query | Silence and speech ranges | `placementId?: id; elementId?: id; assetId?: id; thresholdDb?: number = -40; minSilenceMs?: int = 100` | `{silences: [Range], speech: [Range]}` with `Range = {startFrame, endFrame, startTime, endTime}` | n/a |
| `sound.stop` | ui | Stop one-shot sound playback (one or all) | `playbackId?: id; all?: bool = true` | `{}` | no |
| `sound.waveform.get` | query | Numeric waveform summary per frame (or per time bucket for an asset) | `placementId?: id; elementId?: id; sceneId?: id; assetId?: id; startFrame?: int; endFrame?: int; bucketsPerFrame?: int = 1 (1–32); bucketSeconds?: number = 0.04 (assets only); channels?: enum "mix"\|"separate" = "mix"; wait?: bool = true` | `WaveformSummary` (§15.4) | n/a |
| `sound.waveform.ready` | event | Waveform cache computed for an asset | `{assetId}` | — | n/a |
| `sound.waveforms.set` | toggle | Show waveforms or solid bars in Timeline/Exposure Sheet (per user) | `enabled: bool` | `{enabled}` | no |

## Namespace `lipsync`

Defined in [`07_sound_lipsync.md`](07_sound_lipsync.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `lipsync.apply` | action | Write the drawing element's exposures from the mouth track (Apply Lip-sync; a dry run changes nothing) | `soundElementId?: id; placementId?: id; drawingElementId: id; overwrite: enum "replace"\|"emptyOnly"\|"failIfOccupied"; mapping?: object; startFrame?: int; endFrame?: int; unmapped?: enum "empty"\|"rest"\|"keep" = "empty"; detectIfMissing?: bool = true; storeMapping?: bool = true; dryRun?: bool = false` | `ApplyResult` (§15.4) | yes |
| `lipsync.detect` | job | Detect the mouth track for a placement (or all placements of an element), optionally transcript- or phoneme-assisted | `placementId?: id; elementId?: id; transcript?: string; phonemes?: [{phoneme: string, start?: number, end?: number}]; phonemeAlphabet?: enum "arpabet"\|"ipa" = "arpabet"; timeBase?: enum "placement"\|"source" = "placement"; language?: string = "auto"; minHoldFrames?: int; anticipationFrames?: int; silenceThresholdDb?: number; phonemeMap?: object; manualEdits?: enum "replace"\|"keep"` | `{jobId}`; on completion `{tracks: [LipTrack], warnings[]}` | yes |
| `lipsync.display.set` | toggle | Show/hide lip-sync mode for a sound column (detects first if needed) | `elementId: id; enabled: bool; detectIfMissing?: bool = true` | `{enabled, jobId?}` | yes |
| `lipsync.mapping.get` | query | Read the stored mapping, or resolve the automatic mapping, for a sound/drawing pair | `soundElementId: id; drawingElementId: id; resolve?: enum "stored"\|"auto" = "auto"` | `{mapping: {A..X: {drawingId?, drawingName?, rule: "stored"\|"name"\|"property"\|"order"\|"none"}}, complete, autoReapply}` | n/a |
| `lipsync.mapping.mouth.set` | action | Map one shape to a drawing, or step to the previous/next drawing | `soundElementId: id; drawingElementId: id; mouth: enum "A"\|"B"\|"C"\|"D"\|"E"\|"F"\|"G"\|"X"; drawing?: id \| string \| null; step?: enum "next"\|"previous"` | `{mouth, drawingId, drawingName}` | yes |
| `lipsync.mapping.set` | action | Store a full or partial mapping (by drawing id or name; null = empty) | `soundElementId: id; drawingElementId: id; mapping: object {A..X: id \| string \| null}; autoReapply?: bool` | `{mapping, complete}` | yes |
| `lipsync.mouth.set` | action | Set one mouth on a frame range and mark it manual (Set Mouth) | `elementId: id; startFrame: int; endFrame?: int = startFrame; mouth: enum "A"\|"B"\|"C"\|"D"\|"E"\|"F"\|"G"\|"X"` | `{framesWritten}` | yes |
| `lipsync.panel.close` | ui | Close the lip-sync panel | — | `{}` | no |
| `lipsync.panel.open` | ui | Open/raise the lip-sync panel | `soundElementId?: id; drawingElementId?: id` | `{}` | no |
| `lipsync.panel.state.set` | ui | Set the panel's selected sound, mouth element and advanced-area visibility | `soundElementId?: id; drawingElementId?: id; showAdvanced?: bool` | `{soundElementId, drawingElementId, showAdvanced}` | no |
| `lipsync.settings.get` | query | Project defaults for detection | — | `{minHoldFrames, anticipationFrames, silenceThresholdDb, language, phonemeMap}` | n/a |
| `lipsync.settings.set` | action | Set project defaults for detection (including the phoneme map) | `minHoldFrames?: int (0–6); anticipationFrames?: int (0–4); silenceThresholdDb?: number (10–60); language?: string; phonemeMap?: object; resetPhonemeMap?: bool` | settings | yes |
| `lipsync.shapes.list` | query | The eight mouth shapes with descriptions, covered sounds and reference images | — | `{shapes: [{mouth: "A".."X", index, description, sounds[], referenceImage}]}` | n/a |
| `lipsync.template.import` | action | Import a mouth template as a new drawing element (eight tagged drawings) | `libraryItemId: id; sceneId?: id; name?: string` | `{drawingElementId, pegId, drawings: {A..X: drawingId}}` | yes |
| `lipsync.template.list` | query | Mouth templates available in the Library | — | `{templates: [{libraryItemId, name, thumbnail}]}` | n/a |
| `lipsync.track.changed` | event | A mouth track was detected, set or edited | `{placementId, elementId, source}` | — | n/a |
| `lipsync.track.clear` | action | Delete the lip-sync track of a placement or element | `placementId?: id; elementId?: id` | `{}` | yes |
| `lipsync.track.get` | query | Read the per-frame mouth track | `placementId?: id; elementId?: id; startFrame?: int; endFrame?: int; includeSegments?: bool = false; format?: enum "list"\|"string" = "list"` | `{tracks: [LipTrack]}` (§15.4) | n/a |
| `lipsync.track.set` | action | Write a mouth track explicitly (whole or part) | `placementId: id; startFrame?: int = placement start; mouths: string ("AABBX…") \| [enum "A"\|"B"\|"C"\|"D"\|"E"\|"F"\|"G"\|"X"]; markManual?: bool = true` | `{framesWritten, track: LipTrack}` | yes |
| `lipsync.track.stale` | event | A track no longer matches its placement's timing or the frame rate | `{placementId}` | — | n/a |

## Namespace `media`

Defined in [`07_sound_lipsync.md`](07_sound_lipsync.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `media.movie.import` | job | Import a movie as an image sequence (plus optional audio) | `path: path; elementId?: id; newElementName?: string; sceneId?: id; frame?: int = current frame; sampling?: enum "sceneRate"\|"sourceFrames" = "sceneRate"; sourceStart?: number; sourceEnd?: number; opacity?: int (1–100) = 100; importAudio?: bool = true` | `{jobId}`; on completion `{elementId, drawingIds[], startFrame, stopFrame, soundElementId?, placementId?, warnings[]}` | yes |
| `media.movie.probe` | query | Inspect a movie file | `path: path` | `{decodable, durationSec, fps, frameCount, width, height, hasAudio, audioFormat?}` | n/a |
| `media.object.list` | query | Project media registry (linked sources, location, missing state, usage) | `missingOnly?: bool = false` | `{objects: [{mediaObjectId, name, kind, location, missing, frameCount, durationSec?, sequenceCount}]}` | n/a |
| `media.object.relink` | action | Point a media object at a new file or Library item | `mediaObjectId: id; path?: path; libraryItemId?: id` | `{mediaObjectId, frameCount}` | yes |
| `media.sequence.add` | action | Link a Library item or external file into a media element as a new sequence | `elementId?: id; sceneId?: id; newElementName?: string; libraryItemId?: id; path?: path; frame: int (≥1); linkOffset?: int = 1; length?: int; onOverlap?: enum "insertShift"\|"reject" = "insertShift"` | `{elementId, sequenceId, mediaObjectId, startFrame, stopFrame}` | yes |
| `media.sequence.get` | query | One sequence (name, location, frames, link offset, source length) | `sequenceId: id` | `MediaSequence` (§15.4) | n/a |
| `media.sequence.list` | query | List sequences of a media element | `elementId: id` | `{sequences: [MediaSequence]}` | n/a |
| `media.sequence.move` | action | Move a sequence to a new start frame | `sequenceId: id; startFrame: int (≥1); onOverlap?: enum "reject"\|"insertShift" = "reject"` | `{startFrame, stopFrame}` | yes |
| `media.sequence.naturalTiming.apply` | action | Reset a sequence to one exposure per source frame from its link offset (Reset Media Timing) | `sequenceId: id` | `MediaSequence` | yes |
| `media.sequence.remove` | action | Remove a sequence | `sequenceId: id; closeGap?: bool = false` | `{}` | yes |
| `media.sequence.set` | action | Change a sequence's stop frame or link offset | `sequenceId: id; stopFrame?: int; linkOffset?: int (≥1)` | `MediaSequence` | yes |
| `media.sound.play` | ui | Play the sound contained in a media cell's linked source | `elementId: id; frame: int` | `{playbackId}` | no |
| `media.sound.stop` | ui | Stop media sound playback | — | `{}` | no |
| `media.source.missing` | event | A linked media source could not be found | `{mediaObjectId, location}` | — | n/a |

## Namespace `library`

Defined in [`08_library_storyline.md`](08_library_storyline.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `library.changed` | event | A library's content or state changed | — | `{libraryId, nodeIds: [id], change: enum "created"\|"renamed"\|"moved"\|"deleted"\|"modified"\|"saved"\|"writeAccess"}` | n/a |
| `library.close` | action | Close a library (not the project-local one) | `libraryId: id; discardChanges: bool = false` | `{closed}` or `needs_confirmation` | no |
| `library.closed` | event | A library was closed | — | `{libraryId}` | n/a |
| `library.create` | action | Create a new empty library in a folder and open it | `path: path; name?: string` | `{libraryId}` | no |
| `library.drawing.insert` | action | Copy a drawing (project branch or single-drawing item) to an element frame ("Copy drawing") | `drawingId: id; elementId?: id; frame?: int` | `{newDrawingId, elementId, frame}` | yes |
| `library.element.duplicate` | action | Copy an element into another scene (project branch) | `elementId: id; targetSceneId: id; layerIndex?: int; frame: int = 1` | `{elementIds: [id]}` | yes |
| `library.folder.create` | action | Create a folder ("New folder") | `parentId: id; name?: string; index?: int (1-based)` | `{folderId, name}` | yes |
| `library.folder.delete` | action | Delete a folder and its contents | `folderId: id; confirm: bool = false` | `{deletedNodeCount}` or `needs_confirmation` | yes |
| `library.folder.rename` | action | Rename a folder | `folderId: id; name: string` | `{name}` | yes |
| `library.item.delete` | action | Delete items | `itemIds: [id]` | `{deletedIds: [id]}` | yes |
| `library.item.duplicate` | action | Copy folders/items into a folder | `nodeIds: [id]; targetFolderId: id; onConflict: enum "rename"\|"fail" = "rename"` | `{newNodeIds: [id]}` | yes |
| `library.item.get` | query | Get item properties (§9) | `itemId: id` | `ItemDetails` | n/a |
| `library.item.import` | action | Import media or template files into a folder | `folderId: id; paths: [path]; as: enum "auto"\|"image"\|"movie"\|"sound"\|"template" = "auto"; onConflict: enum "rename"\|"skip"\|"replace" = "rename"` | `{imported: [{path, itemId, name, renamed}], failed: [{path, code}]}` | yes |
| `library.item.list` | query | List the contents of a folder | `folderId: id; recursive: bool = false; types?: [enum "folder"\|"template"\|"image"\|"movie"\|"sound"]; sort: enum "name"\|"type"\|"size"\|"modified"\|"artist" = "name"; includeThumbnails: bool = false; thumbnailSize: int = 128 (16..1024)` | `{items: [ItemSummary]}` | n/a |
| `library.item.move` | action | Move folders/items to a folder (same or another library) | `nodeIds: [id]; targetFolderId: id; index?: int (1-based); onConflict: enum "rename"\|"fail" = "rename"` | `{nodeIds: [id]}` | yes |
| `library.item.rename` | action | Rename an item | `itemId: id; name: string` | `{name}` | yes |
| `library.item.thumbnail.get` | query | Get an item's thumbnail (generated if missing) | `itemId: id; size: int = 256 (16..1024); regenerate: bool = false` | `{png, width, height}` | n/a |
| `library.list` | query | List open libraries | `kinds?: [enum "global"\|"project"\|"external"\|"starter"]` | `{libraries: [LibraryInfo]}` | n/a |
| `library.node.get` | query | Get one library, folder or item with its location (library name and folder path) | `nodeId: id` | `LibraryNode & {libraryName, path}` | n/a |
| `library.open` | action | Open an existing library | `path: path` | `{libraryId, readOnly}` | no |
| `library.opened` | event | A library was opened | — | `{libraryId, kind, readOnly}` | n/a |
| `library.preview.get` | query | Get preview state | — | `{itemId, frame, frameCount, playing, enabled}` | n/a |
| `library.preview.set` | ui | Set preview item, frame, playing and enabled state | `itemId?: id; frame?: int; playing?: bool; enabled?: bool` | `{itemId, frame, frameCount, playing, enabled}` | no |
| `library.project.get` | query | Get the project branch: scenes, elements, optionally drawings | `sceneId?: id; includeDrawings: bool = false` | `{scenes: [{sceneId, name, elements: [{elementId, name, type, drawings?: [{drawingId, name}]}]}]}` | n/a |
| `library.refresh.run` | action | Re-read a library or folder from disk and regenerate missing thumbnails | `libraryId: id; folderId?: id` | `{changedNodeIds: [id]}` | no |
| `library.save` | action | Save modified libraries (project-local is saved with the project) | `libraryIds?: [id]; scope: enum "global"\|"all" = "global"; confirmUpgrade: bool = false` | `{results: [{libraryId, saved, code?}]}` | no |
| `library.selection.get` | query | Get the library selection | — | `{selectedNodeIds}` | n/a |
| `library.selection.set` | ui | Select library nodes (tree or item list) | `nodeIds: [id]; reveal: bool = true` | `{selectedNodeIds}` | no |
| `library.starter.open` | action | Restore the starter library (re-open; optionally reinstall its files) | `reinstall: bool = false` | `{libraryId}` | no |
| `library.template.changed` | event | A template's content changed (version ID bumped) | — | `{templateId, versionId}` | n/a |
| `library.template.create` | action | Create a template from project objects (elements, a scene, a drawing, strokes) | `folderId: id; objectIds: [id]; frameRange?: {start: int, end: int}; name?: string` | `{templateId, name, frameCount, elementCount}` | yes |
| `library.template.edit.changed` | event | Template edit mode entered or left | — | `{active, templateId?, editSceneId?}` | n/a |
| `library.template.edit.get` | query | Get template edit mode state | — | `{active, templateId?, editSceneId?, returnSceneId?}` | n/a |
| `library.template.edit.start` | action | Enter template edit mode | `templateId: id` | `{editSceneId, returnSceneId}` | no |
| `library.template.edit.stop` | action | Return to scene, committing edits into the template ("Edit template") | — | `{templateId, versionId}` | yes |
| `library.template.export` | action | Export a template as a standalone `.animtpl` file | `templateId: id; path: path; overwrite: bool = false` | `{path, sizeBytes}` | no |
| `library.template.frame.render` | query | Render one frame of a template (preview) | `templateId: id; frame: int = 1; width: int = 320; height?: int; background?: colour` | `{png, width, height, frameCount}` | n/a |
| `library.template.get` | query | Get full template metadata and element tree | `templateId: id` | `TemplateDetails` | n/a |
| `library.template.insert` | action | Insert a template or media item into a scene by copy or link | `templateId: id; sceneId?: id; target: enum "timeline"\|"xsheet"\|"camera"\|"drawing" = "timeline"; frame: int = 1; position?: point; layerIndex?: int; intoElementId?: id; drawingId?: id; mode: enum "copy"\|"link" = "copy"; options?: InsertOptions` | `InsertResult` | yes |
| `library.template.search` | query | Search templates and items across libraries with metadata | `query?: string; libraryIds?: [id]; folderId?: id; recursive: bool = true; types: [enum "template"\|"image"\|"movie"\|"sound"] = ["template"]; elementTypes?: [string]; hasSound?: bool; singleDrawing?: bool; artist?: string; modifiedAfter?: string; sort: enum "relevance"\|"name"\|"modified" = "relevance"; limit: int = 50 (1..500); offset: int = 0; includeThumbnails: bool = false; thumbnailSize: int = 128` | `{total, items: [ItemSummary]}` | n/a |
| `library.templateUpdate.apply` | action | Update chosen drawings from their template | `elementId: id; drawingIds: [id]; updateColours: bool = false` | `{updatedDrawingIds: [id], updatedColourIds: [id]}` | yes |
| `library.templateUpdate.list` | query | List drawings of an element derived from templates, with changed state | `elementId: id; templateId?: id` | `{drawings: [{drawingId, name, templateId, templateName, changed}]}` | n/a |
| `library.tree.get` | query | Get the folder/item hierarchy | `libraryId?: id; folderId?: id; depth: int = -1; includeItems: bool = true` | `{nodes: [LibraryNode]}` (nested) | n/a |
| `library.view.get` | query | Get panel state | — | `{mode, currentNodeId, selectedNodeIds, preview}` | n/a |
| `library.view.set` | ui | Set the item-list display mode | `mode: enum "thumbnails"\|"list"\|"details"` | `{mode}` | no |
| `library.writeAccess.set` | toggle | Request or release exclusive write access to a library | `libraryId: id; enabled: bool; discardChanges: bool = false` | `{writable}` or `needs_confirmation` | no |

## Namespace `storyline`

Defined in [`08_library_storyline.md`](08_library_storyline.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `storyline.changed` | event | Scenes, order, visibility, lengths or transitions changed | — | `{reason: enum "scenes"\|"order"\|"visibility"\|"length"\|"transition"\|"properties", sceneIds: [id], editPoints: [int], globalLength}` | n/a |
| `storyline.currentScene.changed` | event | The current scene changed | — | `{sceneId}` | n/a |
| `storyline.currentScene.get` | query | Get the current scene | — | `{sceneId, index, name}` | n/a |
| `storyline.currentScene.set` | ui | Make a scene current in all editing views | `sceneId: id` | `{sceneId}` | no |
| `storyline.frame.get` | query | Map a global frame to scene(s), local frame(s) and transition | `globalFrame: int` | `{globalFrame, kind: enum "scene"\|"transition"\|"blank"\|"padding", sceneId?, localFrame?, transition?: {editPoint, t, outgoing?: {sceneId, localFrame}, incoming?: {sceneId, localFrame}}}` | n/a |
| `storyline.frame.render` | query | Render a global storyline frame including transitions | `globalFrame: int; width?: int; height?: int` | `{png, width, height}` | n/a |
| `storyline.get` | query | Get the full storyline model with computed global timing | `includeHidden: bool = true` | `StorylineModel` | n/a |
| `storyline.play.changed` | event | Storyline playback started/stopped or crossed an edit point | — | `{playing, globalFrame, sceneId}` | n/a |
| `storyline.play.start` | ui | Start Play Storyline (whole movie with transitions) | `fromGlobalFrame?: int; loop?: bool` | `{playing: true, globalFrame}` | no |
| `storyline.play.stop` | ui | Stop Play Storyline | — | `{playing: false, globalFrame}` | no |
| `storyline.playhead.get` | query | Get the storyline playhead | — | `{globalFrame, sceneId, localFrame}` | n/a |
| `storyline.playhead.set` | ui | Move the storyline playhead (scrub) | `globalFrame?: int; editPoint?: int` | `{globalFrame}` | no |
| `storyline.scene.create` | action | Add a scene ("Add scene") | `index?: int (1..sceneCount+1; default after selection, else end); name?: string; makeCurrent: bool = false` | `{sceneId, index, name}` | yes |
| `storyline.scene.delete` | action | Delete a scene and its content ("Delete scene") | `sceneId: id` | `{currentSceneId}` | yes |
| `storyline.scene.get` | query | Get one scene | `sceneId: id` | `SceneInfo` | n/a |
| `storyline.scene.list` | query | List scenes in order | — | `{scenes: [SceneInfo]}` | n/a |
| `storyline.scene.move` | action | Move a scene to an index ("Reorder scene") | `sceneId: id; index: int (1..sceneCount)` | `{order: [id]}` | yes |
| `storyline.scene.rename` | action | Rename a scene ("Change scene name") | `sceneId: id; name: string` | `{name}` | yes |
| `storyline.scene.set` | action | Set scene properties in one step ("Change scene properties") | `sceneId: id; name?: string; backgroundColour?: colour` | `SceneInfo` | yes |
| `storyline.scene.visible.set` | toggle | Show or hide a scene in the movie ("Show/Hide scene") | `sceneId: id; visible: bool` | `{visible}` | yes |
| `storyline.selection.changed` | event | Storyline selection changed | — | `{kind, sceneId?, editPoint?}` | n/a |
| `storyline.selection.get` | query | Get the storyline selection | — | `{kind: enum "none"\|"scene"\|"transition", sceneId?, editPoint?}` | n/a |
| `storyline.selection.set` | ui | Select a scene or an edit point in the Storyline/Scene Manager | `sceneId?: id; editPoint?: int; afterSceneId?: id` | `{selection}` | no |
| `storyline.transition.get` | query | Get the transition at an edit point | `editPoint?: int; afterSceneId?: id; transitionId?: id` | `TransitionInfo` | n/a |
| `storyline.transition.list` | query | List all edit points with their transitions and computed spans | — | `{transitions: [TransitionInfo]}` | n/a |
| `storyline.transition.remove` | action | Set an edit point's transition to none (cut) | `editPoint?: int; afterSceneId?: id` | `TransitionInfo` | yes |
| `storyline.transition.set` | action | Set transition type and parameters at an edit point (one undo step) | `editPoint?: int; afterSceneId?: id; type?: enum "none"\|"fade"\|"crossDissolve"\|"wipe"\|"slide"\|"clockWipe"\|"checker"; durationLeft?: int (0..1000); blankDuration?: int (0..1000); durationRight?: int (0..1000); colour?: colour; angle?: number (0..360); direction?: enum "in"\|"out"\|"clockwise"\|"counterclockwise"; size?: int (5..100, step 5)` | `TransitionInfo` | yes |
| `storyline.transition.types.list` | query | List transition types with their parameters, ranges, defaults and allowed directions | — | `{types: [{type, parameters: [{name, type, min?, max?, step?, default, values?}]}]}` | n/a |
| `storyline.view.get` | query | Get storyline view state | — | `{zoom, playing, globalFrame}` | n/a |
| `storyline.view.set` | ui | Set storyline zoom or fit to view | `zoom?: int (0..100, step 5); fit?: bool` | `{zoom}` | no |
| `storyline.warning.changed` | event | A scene's too-short warning appeared or cleared | — | `{sceneId, warning, paddingFrames}` | n/a |

## Namespace `playback`

Defined in [`09_playback_export.md`](09_playback_export.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `playback.forceRate.set` | toggle | Force frame rate (drop frames to keep real time) | `enabled: bool` | `{enabled}` | n/a |
| `playback.loop.set` | toggle | Loop playback on/off | `enabled: bool` | `{enabled}` | n/a |
| `playback.player.close` | ui | Close the player | `playerId?: id` | `{closed: true}` | n/a |
| `playback.player.get` | query | Read player state | `playerId?: id` | `PlayerState` | n/a |
| `playback.player.open` | ui | Open the preview player on a preview or a movie file | `previewId?: id; path?: path; autoplay: bool = true` | `{playerId, frameCount, fps, durationSec}` | n/a |
| `playback.player.set` | ui | Control the player (play/pause, seek, loop, volume, mute, size report) | `playerId?: id; playing?: bool; frame?: int; loop?: bool; volume?: int (0–100); mute?: bool; showSizeReport?: bool` | `PlayerState` | n/a |
| `playback.preview.run` | job | Render a preview (movie / scene / exposure sheet / selection / quick) and optionally open the player | `type: "movie"\|"scene"\|"xsheet"\|"xsheetSelection"\|"quick"; openPlayer: bool = true; wait: bool = false` | `{jobId}` → `{previewId, manifest}` | n/a |
| `playback.range.changed` | event | Playback range or mode changed | — | `{sceneId, mode, start, end}` | n/a |
| `playback.range.get` | query | Read a scene's playback range | `sceneId?: id` | `{mode: "auto"\|"free", start, end, sceneLength}` | n/a |
| `playback.range.mode.set` | action | Set range mode Automatic Fit / Free (restores last free values) | `sceneId?: id; mode: "auto"\|"free"` | `{mode, start, end}` | no |
| `playback.range.set` | action | Set free range ends (switches to Free; validated and adjusted) | `sceneId?: id; start?: int; end?: int` | `{mode, start, end, adjusted: bool}` | no |
| `playback.rate.set` | ui | Interactive playback rate (does not change project fps) | `fps: number (1–120) \| "project" = "project"` | `{fps, effectiveFps}` | n/a |
| `playback.start` | ui | Start interactive playback of the current scene in the views (Play); storyline playback is `storyline.play.start` | `sceneId?: id; fromFrame?: int` | `{playing: true, sceneId, frame}` | n/a |
| `playback.started` | event | Playback started | — | `{sceneId, frame, mode}` | n/a |
| `playback.state.get` | query | Read playback state and options | — | `PlaybackState` | n/a |
| `playback.stop` | ui | Stop interactive playback (scene or storyline); current frame stays where stopped | — | `{frame}` | n/a |
| `playback.stopped` | event | Playback stopped | — | `{sceneId, frame, reason: "user"\|"end"\|"navigation"\|"sceneChanged"}` | n/a |

## Namespace `render`

Defined in [`09_playback_export.md`](09_playback_export.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `render.frame.run` | query | Deterministically render one frame to PNG/JPEG, inline or to file | `sceneId?: id; frame?: int; composition: "camera"\|"xsheet"\|"element"\|"activeView" = "camera"; elementIds?: [id]; drawingId?: id; width?: int; height?: int; scale: number = 1 (0.05–8); quality: "final"\|"draft" = "final"; background: "scene"\|"transparent"\|colour = "scene"; cameraFrame: bool = false; format: "png"\|"jpeg" = "png"; jpegQuality: int = 90 (0–100); output: "inline"\|"file" = "inline"; path?: path; onConflict: "fail"\|"overwrite"\|"rename" = "fail"` | `RenderResult` | n/a |
| `render.limits.get` | query | Inline size limits, max output size, supported render formats | — | `{maxInlinePixels, maxInlineBytes, maxWidth, maxHeight, formats[]}` | n/a |
| `render.range.run` | job | Render a frame range of one scene to an image sequence (movie ranges with transitions: `export.movie.run` with a sequence format) | `sceneId?: id; from?: int; to?: int; step: int = 1; composition: "camera"\|"xsheet"\|"element" = "camera"; elementIds?: [id]; width?: int; height?: int; scale: number = 1; quality: "final"\|"draft" = "final"; background: "scene"\|"transparent"\|colour = "scene"; format: "png"\|"jpeg"\|"tga"\|"bmp" = "png"; directory: path; namePattern: string = "{name}_{frame:4}"; onConflict = "fail"; createDirs: bool = false; wait: bool = false` | `{jobId}` → `Manifest` | n/a |

## Namespace `import`

Defined in [`09_playback_export.md`](09_playback_export.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `import.completed` | event | An import job finished | — | `{jobId, status, elementIds[], warnings[]}` | n/a |
| `import.file.get` | query | Inspect a file before import | `path: path` | `ImportFileInfo` | n/a |
| `import.file.run` | job | Import files (any family) at a target; one undo step | `paths: [path]; sceneId?: id; elementId?: id \| "new"; frame?: int; mode: "copy"\|"link"\|"move" = "copy"; fitToCamera?: bool; preserveLayers?: bool; layers?: [{path?: path; index?: int; name?: string; include: bool = true; elementName?: string}]; pages?: [int]; flatten?: bool = false; movieOpacity?: int (1–100); vectorAnimationTarget?: "newScene"\|"currentScene"; onNameConflict: "rename"\|"error" = "rename"; confirmLarge: bool = false; wait: bool = false` | `{jobId}` → `ImportResult` | yes |
| `import.formats.list` | query | Supported import formats by family | — | `{families: [{family, formats: [{id, extensions[], optional}]}]}` | n/a |
| `import.keyedImage.get` | query | Preview keying on one image (PNG) | `path: path; keying: Keying; display: "keyed"\|"matte"\|"checkerboard"\|"white" = "checkerboard"; maxSize: int = 1024` | `{data: base64, width, height, transparentFraction}` | n/a |
| `import.keyedImages.run` | job | Import images with keying, one cell per file | `paths: [path]; keying: Keying; perFile?: [{path: path; keying: Keying}]; sceneId?: id; elementId?: id \| "new"; frame?: int; wait: bool = false` | `{jobId}` → `ImportResult` | yes |
| `import.movie.run` | job | Import a movie as an image sequence (and its audio) | `path: path; sceneId?: id; elementId?: id \| "new"; frame?: int; opacity?: int (1–100); matchProjectRate: bool = false; includeAudio: bool = true; sourceFrom?: int; sourceTo?: int; wait: bool = false` | `{jobId}` → `ImportResult` | yes |
| `import.options.get` | query | Read persisted import options | — | `{fitToCamera, preserveImageLayers, preserveVectorLayers, movieOpacity?, lastFolder}` | n/a |
| `import.options.set` | ui | Write persisted import options | `fitToCamera?: bool; preserveImageLayers?: bool; preserveVectorLayers?: bool; movieOpacity?: int (1–100) \| null` | same as get | n/a |
| `import.vectorize.settings.get` | query | Read import-and-vectorize settings | — | `VectorizeSettings` | n/a |
| `import.vectorize.settings.set` | ui | Write the remembered import-and-vectorize settings (defaults for `draw.vectorize.run`, 02) | `type?: "blackAndWhite"\|"withTexture"; filter?: "none"\|"smooth"\|"smoothMore"\|"edgeEnhance"\|"sharpen"; threshold?: int (0–100)` | `VectorizeSettings` | n/a |

## Namespace `export`

Defined in [`09_playback_export.md`](09_playback_export.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `export.drawingPdf.run` | job | Export one vector drawing to a single-page PDF | `path: path; drawingId?: id; elementId?: id; frame?: int; page: "bounds"\|"camera" = "bounds"; scale: number = 1; onConflict = "fail"; wait: bool = true` | `{jobId}` → `Manifest` (+ `warnings` e.g. `export.pdfFeatureFlattened`) | n/a |
| `export.formats.list` | query | List export formats with capabilities and settings schemas | `kind?: "movie"\|"sequence"\|"still"\|"vector"\|"document"` | `{formats: [{id, name, extensions[], kind, video, audio, alpha, optional, settingsSchema, presets[]}]}` | n/a |
| `export.job.finished` | event | An export/render/print/publish job finished | — | `{jobId, command, status, outputPaths[], errorCode?}` | n/a |
| `export.movie.defaults.get` | query | Read persisted export-dialog values (path, format, type, range, publishing options) | — | `ExportMovieParams` | n/a |
| `export.movie.defaults.set` | ui | Write persisted export-dialog values | `values: object` | `ExportMovieParams` | n/a |
| `export.movie.run` | job | Export movie / animated GIF / image sequence / vector animation | `path: path; format: string = "mp4"; preset?: "default"\|"custom"\|string; settings?: object; type: "wholeMovie"\|"currentSceneCamera"\|"currentSceneXsheet" = "wholeMovie"; range: "all"\|"allNoTransitions"\|"frames"\|"selection"\|"playbackRange" = "allNoTransitions"; from?: int; to?: int; sceneId?: id; elementIds?: [id]; width?: int; height?: int; fps?: number; audio: bool = true; alpha: bool = false; writeHtml: bool = false; addToMediaLibrary: bool = false; openPlayer: bool = false; deterministic: bool = false; onConflict = "fail"; createDirs: bool = false; wait: bool = false` | `{jobId}` → `Manifest` | n/a |
| `export.movie.validate` | query | Dry-run an export: resolve frames, path, conflicts, estimates, enabled ranges | same as `export.movie.run` | `{frameCount, durationSec, outputPaths[], conflicts[], estimatedBytes?, enabledRanges[], warnings[], errors[]}` | n/a |
| `export.publish.account.close` | ui | Sign out and forget the stored token | `serviceId: id` | `{connected: false}` | n/a |
| `export.publish.account.get` | query | Sign-in status for a service | `serviceId: id` | `{connected: bool, accountName?, remembered: bool, pending: bool}` | n/a |
| `export.publish.account.open` | ui | Start browser sign-in for a service (user completes it) | `serviceId: id; remember: bool = false` | `{pending: true}` | n/a |
| `export.publish.categories.list` | query | Categories offered by a service | `serviceId: id` | `{categories: [{id, label}]}` | n/a |
| `export.publish.run` | job | Upload a movie to a service (outward action; needs scope and confirm) | `serviceId: id; path?: path; export?: object; metadata: {title: string; description?: string; tags?: [string]; category?: string; visibility?: "public"\|"unlisted"\|"private" = "private"}; confirm?: bool; confirmToken?: string` | `{jobId}` or `needs_confirmation` → `{remoteId, url?, manifest}` | n/a |
| `export.publish.service.list` | query | Configured publishing services, limits and metadata requirements | — | `{services: [{id, name, formats[], limits: {minSec, maxSec, maxBytes}, fields: [{name, required}], visibilities[], connected}]}` | n/a |
| `export.publish.validate` | query | Check a file or export parameters against a service's limits and required metadata | `serviceId: id; path?: path; export?: object; metadata?: object` | `{ok: bool, problems: [{code, limit?, actual?}]}` | n/a |
| `export.report.get` | query | Size report of a finished vector-format export | `jobId: id` | `{file, totalBytes, cameraSize, fps, groups: [{kind, items: [{name, bytes}]}]}` | n/a |
| `export.settings.get` | query | Read stored settings of a format and active preset | `format: string` | `{preset, settings}` | n/a |
| `export.settings.set` | ui | Store Custom settings for a format, or reset to Default | `format: string; preset?: "default"\|"custom"; settings?: object` | `{preset, settings}` | n/a |
| `export.snapshot.run` | job | Export the current (or given) frame as a still image, active-view composition by default | `path: path; format: "png"\|"jpeg"\|"tga"\|"bmp"\|"tiff" = "png"; quality?: int (0–100); alpha: bool = true; sceneId?: id; frame?: int; composition: "activeView"\|"camera"\|"xsheet"\|"element" = "activeView"; elementId?: id; onConflict = "fail"; wait: bool = true` | `{jobId}` → `Manifest` | n/a |

## Namespace `capture`

Defined in [`09_playback_export.md`](09_playback_export.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `capture.auto.start` | job | Start auto capture (each capture is its own undo step) | `sessionId?: id; interval: int = 1 (1–999); unit: "ms"\|"s"\|"min"\|"h"\|"day" = "s"; maxCount?: int; lockInput: bool = false` | `{jobId}` → `{captures: int, cellIds[]}` | yes |
| `capture.auto.stop` | ui | Stop auto capture | `sessionId?: id` | `{captures: int}` | n/a |
| `capture.device.added` | event | A device was connected | — | `{device: Device}` | n/a |
| `capture.device.dialog.open` | ui | Show the device's own settings interface, if it has one (user-facing) | `deviceId: id` | `{opened: bool}` | n/a |
| `capture.device.get` | query | Device descriptor, current settings and settings schema | `deviceId: id` | `Device & {settings, settingsSchema}` | n/a |
| `capture.device.list` | query | List connected capture devices and scanners | `type?: "motionCamera"\|"photoCamera"\|"scanner"` | `{devices: [Device]}` | n/a |
| `capture.device.removed` | event | A device was disconnected | — | `{deviceId}` | n/a |
| `capture.device.select` | ui | Set the default acquisition device (remembered) | `deviceId: id` | `{deviceId}` | n/a |
| `capture.device.settings.set` | ui | Set device settings (exposure, ISO, white balance, focus, zoom, scan options…) | `deviceId: id; values: object` | `{applied: object, rejected: [{name, reason}]}` | n/a |
| `capture.display.set` | ui | Preview display aids | `sessionId?: id; matte?: bool; hideBackground?: bool` | `{matte, hideBackground}` | n/a |
| `capture.frame.add` | action | Capture from the session's device into the target (Capture Image) | `sessionId?: id; count: int = 1; useDeviceUi: bool = false` | `CaptureResult` | yes |
| `capture.frame.addFromFile` | action | Feed supplied image files through the capture pipeline (Capture Image) | `paths: [path]; sessionId?: id; keying?: Keying \| "session" \| "none" = "session"; vectorize?: bool; sceneId?: id; elementId?: id \| "new"; frame?: int; insertMode?: "insert"\|"overwrite"; advance: bool = true` | `CaptureResult` | yes |
| `capture.frame.captured` | event | A capture was inserted | — | `{sessionId?, elementId, drawingId, cellId, frame, nextFrame}` | n/a |
| `capture.inputLock.set` | toggle | Ignore pointer input during auto capture | `enabled: bool` | `{enabled}` | n/a |
| `capture.keyColour.set` | ui | Set key colour directly or by sampling the preview | `sessionId?: id; colour?: colour; sample?: point; sampleRadius: int = 2 (0–20)` | `{colour}` | n/a |
| `capture.keying.set` | ui | Set keying parameters of the session (persisted) | `sessionId?: id; keying: Keying` | `{keying}` | n/a |
| `capture.lightTable.references.list` | query | Available light-table references for the current target frame | `sessionId?: id` | `{references: [{refId, label, elementId?, drawingId?, frame?}]}` | n/a |
| `capture.lightTable.set` | ui | Light-table reference, opacity and aspect-ratio mode | `sessionId?: id; reference?: "none"\|"previousCapture"\|id; opacity?: int = 50 (0–100); aspectRatio?: "fit"\|"fill"\|"stretch"` | `{reference, opacity, aspectRatio}` | n/a |
| `capture.preview.get` | query | Current processed preview as PNG (keying, display aids, light table) | `sessionId?: id; display: "processed"\|"raw"\|"matte" = "processed"; includeLightTable: bool = true; refresh: bool = true; maxSize: int = 1024` | `{data: base64, width, height, capturedAt}` | n/a |
| `capture.session.close` | ui | Close the session (stops auto capture) | `sessionId?: id` | `{closed: true}` | n/a |
| `capture.session.error` | event | Session error (busy, disconnected, no image…) | — | `{sessionId, code, message}` | n/a |
| `capture.session.get` | query | Read session state | `sessionId?: id` | `CaptureSession` | n/a |
| `capture.session.open` | ui | Open a capture session (and optionally the capture window) | `deviceId?: id; mode: "image"\|"vectorize" = "image"; sceneId?: id; elementId?: id \| "new"; showWindow: bool = true` | `{sessionId, deviceId, target}` | n/a |
| `capture.source.set` | ui | Set source, resolution / image size, noise reduction | `sessionId?: id; source?: string; resolution?: string; imageSize?: {w: int, h: int}; noiseReduction?: bool` | `CaptureSession` | n/a |
| `capture.target.set` | ui | Target element/frame and insert/advance behaviour | `sessionId?: id; elementId?: id \| "new"; frame?: int; insertMode: "insert"\|"overwrite" = "insert"; advance: bool = true` | `{elementId, frame, insertMode, advance}` | n/a |

## Namespace `print`

Defined in [`09_playback_export.md`](09_playback_export.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `print.pageSetup.get` | query | Read page setup | `printerId?: id` | `{paperSize, orientation, printableArea}` | n/a |
| `print.pageSetup.set` | ui | Set paper size and orientation | `printerId?: id; paperSize?: string; orientation?: "portrait"\|"landscape"` | `{paperSize, orientation}` | n/a |
| `print.preview.get` | query | Lay out pages and return page count and one page image | `PrintParams; page: int = 1; maxSize: int = 1024` | `{pageCount, pages: [{page, frames[]}], data: base64, width, height}` | n/a |
| `print.printer.list` | query | Installed printers and their capabilities | — | `{printers: [{id, name, isDefault, paperSizes[], duplex, colour}]}` | n/a |
| `print.run` | job | Print to a printer (needs scope + confirm) or to a file/PDF | `PrintParams; destination: "printer"\|"file" = "file"; printerId?: id; path?: path; pdfOutput: bool = true; onConflict = "fail"; confirm?: bool; confirmToken?: string; wait: bool = false` | `{jobId}` or `needs_confirmation` → `{pagesPrinted}` or `Manifest` | n/a |
| `print.settings.get` | query | Read persisted print parameters | — | `PrintParams` | n/a |
| `print.settings.set` | ui | Write persisted print parameters | `values: object` | `PrintParams` | n/a |

## Namespace `doc`

Defined in [`10_data_model_project_format.md`](10_data_model_project_format.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `doc.asset.embed` | action | Copy linked files into the package ("Embed Media") | `assetIds?: [id]; all: bool = false` | `{embedded: [id], bytes}` | yes |
| `doc.asset.export` | query | Write an asset's file to disk (e.g. to share or edit externally) | `assetId: id; path: path; onExists: enum "fail"\|"overwrite" = "fail"` | `{path, bytes}` | n/a |
| `doc.asset.list` | query | List assets with storage, status and references | `kind?: enum "sound"\|"image"\|"movie"\|"vector"; status: enum "all"\|"missing"\|"changed"\|"linked"\|"embedded"\|"unused" = "all"` | `{assets: [{id, name, kind, storage, status, path?, size, refs: int}]}` | n/a |
| `doc.asset.relink` | action | Point linked assets to a new file, or search a folder for missing ones ("Relink Media"); generic form of `sound.asset.relink` / `media.object.relink` (07) | `assetId?: id; path?: path; searchFolder?: path; recursive: bool = true; matchBy: enum "name"\|"nameAndSize"\|"hash" = "nameAndSize"` | `{relinked: [{assetId, path}], stillMissing: [id]}` | yes |
| `doc.asset.remove` | action | Remove assets; only unreferenced ones unless `force` ("Remove Media") | `assetIds?: [id]; unused: bool = false; force: bool = false` | `{removed: [id], bytes, detachedRefs: [id]}` | yes |
| `doc.asset.status.changed` | event | Assets became missing, changed or available again | — | `{assets: [{assetId, status}]}` | n/a |
| `doc.blob.get` | query | Read a binary payload (asset file, bitmap node pixels, texture) as base64 | `ref?: id; entry?: string; maxBytes: int = 8388608 (1–67108864)` | `{mediaType, size, sha256, base64}` | n/a |
| `doc.changed` | event | A transaction was committed (payload defined in 11 §9.2 and §2.6) | — | `{undoStepId, documentRev, label, origin, changes: [change]}` | n/a |
| `doc.diff` | query | Net changes since a revision or checkpoint (= `doc_diff`) | `sinceRev?: int; sinceCheckpoint?: id; untilRev?: int; within?: id; types?: [string]; detail: enum "summary"\|"full" = "full"; limit: int = 1000` | `{fromRev, toRev, changes: [change], truncated}` | n/a |
| `doc.export.json` | query | Export the project, scenes or elements as an interchange bundle (§6.8), inline or to a file | `scope: enum "project"\|"scenes"\|"elements"; refs?: [id]; path?: path; binaries: enum "embed"\|"reference"\|"omit" = "reference"; pretty: bool = true; onExists: enum "fail"\|"overwrite" = "fail"` | `{bundle?: object, path?: path, bytes, objectCount}` | n/a |
| `doc.import.json` | action | Import an interchange bundle as new scenes or into a scene ("Import JSON") | `source: path \| object; target?: id (scene; omit = new scenes); index?: int (scene position); atFrame: int = 1; idPolicy: enum "new"\|"preserve" = "new"; onConflict: enum "rename"\|"replace"\|"fail" = "rename"; colours: enum "keepExisting"\|"updateExisting" = "keepExisting"; dryRun: bool = false` | `{created: {scenes[], elements[], drawings[], palettes[], assets[]}, renamed: [{id, from, to}], recoveredColourIds[], issues[]}` | yes |
| `doc.loadReport.get` | query | The report of the last open (§7.3) | — | `LoadReport` | n/a |
| `doc.object.find` | query | Find objects by type, name and predicate (= `find_objects` / `query`) | `type?: string; within?: id; name?: string (exact or glob); where?: object (§10.3); limit: int = 100 (1–1000); offset: int = 0; fields?: [string]; sort?: enum "path"\|"name"\|"rev"\|"createdAt" = "path"` | `{items: [{id, type, name, path, rev, …fields}], total, nextOffset}` | n/a |
| `doc.object.get` | query | Read one or more objects as object-model JSON (= `get_object`) | `ref?: id; refs?: [id]; depth: int = 1 (-1 = all, 0–20); fields?: [string]; include?: [enum "derived"\|"stats"]; page?: object {pointer: string, offset: int = 0, limit: int = 1000}` | `{object}` or `{objects: [object]}`; paged arrays carry `{total, nextOffset}` | n/a |
| `doc.object.patch` | action | Generic validated property patch on one or more objects (= `set_properties`) | `targets: [id] (1–1000); props?: object; ops?: [object {op: enum "add"\|"remove"\|"replace"\|"move"\|"copy"\|"test", path: string, value?: any, from?: string}]; expectRev?: int \| object; label?: string; dryRun: bool = false` | `{changes: [change], documentRev}` | yes |
| `doc.object.refs` | query | List objects that reference an object (incoming) or that it references (outgoing) | `ref: id; direction: enum "incoming"\|"outgoing" = "incoming"; limit: int = 500` | `{refs: [{id, type, path, property}], total}` | n/a |
| `doc.object.resolve` | query | Resolve paths to IDs and IDs to canonical paths | `refs: [string]` | `{items: [{ref, id, type, name, path, rev} \| {ref, error}]}` | n/a |
| `doc.package.changedOnDisk` | event | The open project's file or folder was modified by another process | — | `{path, detectedAt}` | n/a |
| `doc.package.export` | job | Write a copy of the current project in package or folder form without changing the open project's location | `path: path; form: enum "package"\|"folder" = "package"; onExists: enum "fail"\|"overwrite" = "fail"` | `{jobId}` → `{path, form, bytes, entries}` | n/a |
| `doc.package.get` | query | Inspect a package, folder, template or palette file on disk without opening it | `path: path; validate: bool = false` | `{kind: enum "project"\|"template"\|"palette"\|"library", form, formatVersion, minReaderVersion, readable, projectId?, name, savedAt, entryCount, issues[]}` | n/a |
| `doc.repair` | action | Apply the default fix of selected issues ("Repair Project") | `issueIds?: [id]; codes?: [string]; within?: id; all: bool = false; dryRun: bool = false` | `{fixed: [issueId], skipped: [{issueId, reason}], changes: [change]}` | yes |
| `doc.schema.get` | query | JSON Schema of an object type (or of one property) | `type: string; property?: string (JSON Pointer)` | `{type, formatVersion, schema}` | n/a |
| `doc.schema.list` | query | List object types with their parents and addressing | — | `{formatVersion, types: [{type, aliases[], parents[], nameAddressable, description}]}` | n/a |
| `doc.schema.validate` | query | Validate a candidate object or bundle against the schema and semantic rules without applying it | `type: string; object: object; mode: enum "create"\|"replace" = "replace"` | `{valid: bool, errors: [{pointer, code, message}]}` | n/a |
| `doc.state.get` | query | Document state | — | `{projectId, path, form, formatVersion, migratedFrom, documentRev, savedRev, dirty, readOnly, lock, historyFromRev}` | n/a |
| `doc.validate` | query | Lint the project or a subtree (§10.3) | `within?: id; checks?: [string] (default all); minSeverity: enum "info"\|"warning"\|"error" = "info"; limit: int = 500` | `{documentRev, issues: [issue], counts: {error, warning, info}, truncated}` | n/a |

## Namespace `agent`

Defined in [`11_agent_interface_mcp.md`](11_agent_interface_mcp.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `agent.activity.export` | action | Write the activity log to a file | `path; format?: enum "jsonl"\|"batch" = "jsonl"; onExists?: enum "fail"\|"overwrite"\|"rename" = "fail"` | `{path, entries}` | no |
| `agent.activity.list` | query | Activity log entries | `sessionId?; since?: time; limit?: int = 100` | `[{seq, time, sessionId, tool, commandId, status, undoStepId, changedCount, durationMs}]` | n/a |
| `agent.activity.persist.set` | toggle | Persist the activity log beside the project | `enabled: bool` | `{enabled}` | no |
| `agent.confirmation.requested` | event | A confirmation is waiting for the user (attached mode) | — | `{confirmationId, sessionId, summary, consequences[]}` | n/a |
| `agent.confirmation.respond` | ui | The user's answer to a pending agent confirmation shown in the app | `confirmationId; accept: bool` | `{confirmationId, accepted}` | no |
| `agent.policy.defaults.set` | ui | Default policy and limits for new sessions (scope `app`) | `policy?: enum "observe"\|"assist"\|"autonomous"; maxJobs?: int (1..8) = 2; maxBatchSteps?: int (1..100000) = 1000; maxInlineImagePx?: int (256..8192) = 2048; maxInlineBytes?: int = 52428800; askInApp?: bool = true; letAgentMoveView?: bool = true; highlightAgentChanges?: bool = true` | Effective settings | no |
| `agent.server.set` | ui | Enable/disable the MCP server and set transport options (scope `app`) | `enabled: bool; http?: bool = true; port?: int (1024..65535); allowRemote?: bool = false` | `{enabled, endpoint?, discoveryFile?}` | no |
| `agent.server.status` | query | Server status, endpoint, connected sessions | — | `{enabled, endpoint, sessions: [{sessionId, client, policy, scopes[], connectedAt, lastActivity}]}` | n/a |
| `agent.session.connected` | event | A session connected | — | `{sessionId, client, policy}` | n/a |
| `agent.session.disconnect` | ui | Disconnect a session | `sessionId` | `{}` | no |
| `agent.session.disconnected` | event | A session disconnected | — | `{sessionId, reason}` | n/a |
| `agent.session.grant` | ui | Change a session's policy preset or scopes (scope `app`) | `sessionId; policy?: enum "observe"\|"assist"\|"autonomous"; scopes?: [string]; roots?: [path]` | `{sessionId, policy, scopes[], roots[]}` | no |
| `agent.session.pause` | ui | Pause an agent session (its calls wait, then fail with `agent.paused`) | `sessionId? (all when omitted)` | `{paused: [sessionId]}` | no |
| `agent.session.resume` | ui | Resume a paused session | `sessionId?` | `{resumed: [sessionId]}` | no |
| `agent.token.regenerate` | ui | Regenerate the HTTP bearer token, disconnecting HTTP sessions (scope `app`) | — | `{discoveryFile}` | no |
| `agent.typedTools.set` | ui | Expose commands as individual MCP tools (scope `app`) | `enabled: bool; namespaces?: [string]` | `{toolCount}` | no |

## Namespace `inspect`

Defined in [`11_agent_interface_mcp.md`](11_agent_interface_mcp.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `inspect.contactSheet.render` | query | Grid image of several frames (same as `render_contact_sheet`) | `sceneId?: id; frames: [int] \| {from,to,step}; columns?: int; cellWidth?: int = 240; labels?: bool = true` | `{png, width, height, cells: [{frame, x, y, w, h}]}` | n/a |
| `inspect.coordinates.convert` | query | Convert points between coordinate spaces | `from: string; to: string; points: [{x,y,z?}]; sceneId?: id; frame?: int; elementId?: id; view?: string` | `{points: [{x,y,z?}]}` | n/a |
| `inspect.frame.describe` | query | Structured description of what is visible and audible at a frame (same as `describe_frame`) | `sceneId?: id; frame?: int; cameraId?: id; detail?: enum "summary"\|"full" = "summary"` | `{elements: [{elementId, name, drawingId?, worldTransform, cameraBounds, opacity, effects[]}], sounds: [{elementId, level}], mouthShape?, warnings[]}` | n/a |
| `inspect.hitTest` | query | Objects under a point, top-most first | `space: enum "view"\|"drawing"\|"scene"\|"camera"\|"image"; point: point; sceneId?: id; frame?: int; view?: string` | `{hits: [{elementId, drawingId?, layerId?, objectId?, kind, colourId?}]}` | n/a |
| `inspect.screenshot` | query | PNG of the real GUI (attached) or a virtual view (headless) | `target?: string = "window"; region?: {x,y,w,h}` | `{png, width, height, sha256}` | n/a |
| `inspect.ui.tree` | query | UI tree of the window, a view, a panel or the open dialogs (same as `inspect_ui`) | `target?: string = "window"; depth?: int = 3` | `{nodes: [{role, id, label, value?, enabled, visible, commandId?, children[]}]}` | n/a |

## Namespace `checkpoint`

Defined in [`11_agent_interface_mcp.md`](11_agent_interface_mcp.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `checkpoint.create` | ui | Create a named in-memory snapshot of the document | `name: string` | `{name, documentRev}` | no |
| `checkpoint.delete` | ui | Delete a checkpoint | `name: string` | `{}` | no |
| `checkpoint.list` | query | List checkpoints | — | `[{name, createdAt, documentRev, origin}]` | n/a |
| `checkpoint.restore` | action | Restore a checkpoint (one undo step "Restore checkpoint") | `name: string` | `{name, documentRev}` | yes |

## Namespace `job`

Defined in [`11_agent_interface_mcp.md`](11_agent_interface_mcp.md).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `job.cancel` | ui | Cancel a job; partial results are discarded unless the command documents otherwise | `jobId: string` | Job record | no |
| `job.finished` | event | Job reached a final state | — | Job record | n/a |
| `job.list` | query | Jobs of this session (or all, for the GUI) | `jobId?: string; all?: bool = false` | `[{jobId, commandId, state, progress, message, result?, error?}]` | n/a |
| `job.progress` | event | Job progress changed | — | Job record | n/a |
| `job.wait` | query | Wait for a job to finish or a timeout | `jobId: string; timeoutMs?: int = 60000` | Job record, `timedOut?: bool` | n/a |
