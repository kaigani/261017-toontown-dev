# 01 — Application Shell, Workspace and Global Behaviour

This section specifies everything that does not belong to one editing domain: the application life-cycle, the start
screen, the project life-cycle (new, open, save, save as, close, autosave, backups), project properties, the two
working modes (drawing and staging), the main window and its panels, docking and workspaces, toolbars, themes, the
generic Edit and View commands (undo/redo, clipboard, selection, view navigation), preferences, the keyboard-shortcut
editor, status bar / tooltips / context help, the "don't show again" tip mechanism, message conventions and persisted
user settings. It also defines the agent-facing commands of the `app`, `project`, `edit`, `view`, `window`,
`workspace`, `prefs`, `keymap` and `ui` namespaces.

Cross references: 02_drawing.md (drawing tools, drawing aids, Drawing view content), 03_colour.md (palettes),
04_timeline_xsheet.md (elements, Timeline, exposure sheet, cells), 05_scene_planning.md (Camera/Top/Side views, pegs,
cameras, function editor, motion recording), 06_effects.md, 07_sound_lipsync.md, 08_library_storyline.md (library,
scene manager, storyline), 09_playback_export.md (playback, preview, import, export, print),
10_data_model_project_format.md (project package), 11_agent_interface_mcp.md (MCP server, agent sessions),
A_command_registry.md (compiled command registry).

## 0. Conventions

* **Primary modifier.** Shortcuts are written with *Primary* = Command (⌘) on macOS and Ctrl on Windows/Linux, and
  *Alt* = Option (⌥) on macOS. Menus and tooltips show shortcuts in the platform's native notation.
* **Commands.** Every user-reachable capability is a registered command with a stable ID of the form
  `<namespace>.<object>.<verb>`, a JSON-schema'd parameter set and a structured result. Menus, toolbars, context menus,
  shortcuts, dialogs and the MCP server are all clients of the same command layer. When a command is invoked
  interactively from the GUI without its required parameters (for example *New Project* from a menu), the GUI MUST
  collect the parameters with a dialog and then run the command; when invoked through the agent interface, missing
  required parameters are an error (`arg.missing`), never a dialog.
* **Messages and error codes.** Every warning, error, confirmation and tip has a stable message ID of the form
  `area.reason` (for example `project.notFound`). The same ID is used as the agent error code. Message wording is a
  UX-writing task; this section describes only what each message must convey.
* **Context.** "Available in context X" means the command is reachable from the GUI in that context (main menu,
  context menu, toolbar or shortcut); exact menu placement is design work.

---

## 1. Application life-cycle

### 1.1 Launch sequence

1. **Single instance.** Only one instance runs per user session. If the application is launched while an instance is
   running, any document passed to the second instance (command line or OS "open document" event) MUST be forwarded
   to the running instance, which opens it; the second instance then exits.
2. **Splash / loading feedback.** While loading, the application shows a splash or loading indicator.
3. **Settings load.** Persisted settings (§11) are loaded. On the first run, defaults are written and a first-run flag
   is recorded. The interface language is taken from the `general.language` preference or, if unset, from the system
   locale when a matching localisation is installed, otherwise English.
4. **First-run content.** On the first run, the bundled template library is copied into the user's global library
   (08_library_storyline.md), and this is recorded so that it happens only once.
5. **Main window.** The main window is built and the workspace stored as the start-up workspace (normally the last one
   used) is restored (§5.5). The selected theme is applied (§5.8).
6. **Crash recovery check.** If autosave recovery data from a previous session exists (§3.7), the start screen lists
   it as recoverable work (or, if the start screen is disabled, the user is notified that recoverable work exists).
7. **Start-up behaviour.** The `general.startup.behaviour` preference decides what happens next: show the start screen
   (default), reopen the last project, show the Open dialog, show the New Project dialog, or create a new project from
   the default settings.
8. **Deferred open requests.** Documents requested by the OS before the main window existed are opened once it exists
   (taking precedence over the start-up behaviour).

### 1.2 Quit

* Quit is available from the platform-standard location (application menu on macOS, File menu elsewhere) with the
  platform-standard shortcut (Primary+Q).
* If the open project has unsaved changes, the user is asked whether to save (§3.5). Cancel aborts the quit.
* On quit the application saves: the current workspace if it was modified and `general.workspace.autoSave` is on,
  preferences, keymap, toolbar configuration, the recent-projects list and the main window geometry.
* Closing the last main window is equivalent to Quit.
* Autosave recovery data for the session is deleted after a clean quit.

### 1.3 Command availability without a project

When no project is open, only these commands are enabled: new project, open project, open recent / recent-list
management, recovery and backup opening, preferences, keymap, workspace and window/panel commands, theme, help and
context help, saving the global library (when it has unsaved changes) and quit. Every other command (edit, element,
playback, tool, import, export, print…) is disabled in the GUI; through the agent interface it fails with
`project.none`.

---

## 2. Start screen

**Purpose.** A start hub shown at launch (by default) and after closing a project.

**When shown.** At launch when `general.startup.behaviour` = `startScreen`, and after a project is closed when the
same preference is `startScreen`. There is no need for a separate command to reach it other than closing the project,
but it MUST be reachable through `ui.startScreen.set` for agents and SHOULD be reachable from the Help or File menu.

**Content.**

| Region | Requirement |
|---|---|
| New project | The new-project form (§3.1): name, format preset, frame rate, camera size, and a Create action. Pressing Enter in any field is equivalent to Create when the form is valid. |
| Recent projects | The recent-projects list (§3.3), most recent first. Each entry shows the project name and, at least on hover, its full path; a thumbnail SHOULD be shown when available. Entries whose project no longer exists are marked as missing. Activating an entry opens that project. |
| Open other | Opens the standard Open dialog (§3.2). |
| Recoverable work | Present only when autosave recovery data exists (§3.7): lists the affected projects with the autosave time, and offers *Recover* and *Discard* per entry. |
| Sample project | Opens a bundled sample project (content authored fresh: a short animated scene that exercises drawing, cut-out animation, staging and lip-sync). The sample opens as an untitled copy so that the first save asks for a location. |
| Learning resources | Links to the documentation and tutorials, which open in the default browser. Tutorial coverage SHOULD include: quick start (drawing, painting, timing), cut-out animation, staging and multiplane camera work, lip-sync, effects. Content is authored fresh. |
| Show at start-up | A check box. Unchecking it sets `general.startup.behaviour` to `newDefaultProject`; checking it sets `startScreen`. |

**Closing the start screen** without choosing anything (Escape or window close) leaves the application with no open
project (§1.3).

---

## 3. Projects

A project is stored as one **project package** (extension `.animpkg`; structure in 10_data_model_project_format.md).
Only one project is open at a time.

### 3.1 New project

Available from the start screen, the main menu, the main toolbar and the platform-standard shortcut (Primary+N).

If a project with unsaved changes is open, the unsaved-changes rule (§3.5) applies first.

**Fields.**

| Field | Type | Default | Validation | Effect |
|---|---|---|---|---|
| Name | text | "Untitled" | Shared name rule (§9.6): not empty, at most 100 characters, no leading/trailing whitespace, no trailing dot, none of `/ \ : * ? " < > \|` or control characters | Project name and the default package file name. Create is disabled while the name is invalid. |
| Format | preset choice | *Most recent* if one exists, else *Default* | one of the entries below | Selecting a preset fills Frame Rate and Camera Size. |
| Frame rate | number (fps) | from the selected format | 1–120; the NTSC rates 23.976, 29.97 and 59.94 MUST be accepted | Project frame rate. |
| Camera size | width × height (pixels) | from the selected format | each 16–8192, integers | Camera / output resolution. |
| Create | action | — | enabled when all fields are valid | Creates and opens the project. |

* **Format entries.** *Most recent* (the values used for the previous new project), *Default* (the
  `project.defaults.*` preferences), then the bundled presets, then *Custom*. Editing Frame Rate or Camera Size by hand
  switches the format to *Custom*, unless the values match an existing preset exactly.
* After Create, the frame rate and camera size are remembered as *Most recent*.
* **Built-in presets.** At least these (display names authored fresh):

| Category | Width × Height | fps |
|---|---|---|
| Web small 4:3 | 480 × 360 | 12 |
| Web medium 4:3 | 640 × 480 | 12 |
| Web 16:9 | 960 × 540 | 24 |
| SD NTSC 4:3 (square pixels) | 720 × 540 | 29.97 |
| SD NTSC 16:9 (square pixels) | 854 × 480 | 29.97 |
| SD PAL 4:3 (square pixels) | 768 × 576 | 25 |
| SD PAL 16:9 (square pixels) | 1024 × 576 | 25 |
| HD 720p | 1280 × 720 | 24, 25 and 30 variants |
| HD 1080p | 1920 × 1080 | 24, 25 and 30 variants |
| UHD 4K | 3840 × 2160 | 24, 25 and 30 variants |
| DCI 2K | 2048 × 1080 | 24 |
| DCI 4K | 4096 × 2160 | 24 |
| Social square | 1080 × 1080 | 30 |
| Social vertical | 1080 × 1920 | 30 |

* Presets are data, loaded from a preset definition file bundled with the application. A missing or unreadable preset
  file is logged; the form then offers only *Most recent*, *Default* and *Custom*.

**Result of Create.**
* A new project exists in memory with the given name, frame rate and camera size. It contains one scene, one camera
  and one empty drawing element, so that the user can start drawing immediately.
* The project has no file location yet unless one was given (agent interface). The first Save behaves as Save As.
* The window title shows the project name.
* The undo history starts empty and the project is clean.

### 3.2 Open

Available from the start screen, the main menu, the main toolbar and the platform-standard shortcut (Primary+O). Also
triggered by OS open-document requests and drag-and-drop of a project package onto the application.

* The Open dialog filters on project packages (`.animpkg`) and offers an "all files" option.
* If a project with unsaved changes is open, the unsaved-changes rule (§3.5) applies first; the current project is
  closed before the new one is loaded.
* Loading shows progress with step text (properties, palettes, media, scenes, drawings, finalisation). Loading is
  cancellable; cancelling leaves no project open (the previous one has already been closed).
* After a successful open, the project goes to the top of the recent-projects list.
* **Errors** (the project is not opened): `project.notFound` (the path does not exist; if it came from the recent list
  the entry is removed or marked missing), `project.invalidFormat` (not a project package or corrupt),
  `project.unsupportedVersion` (written by a newer format version), `project.permissionDenied`.
* **Read-only projects.** A project package on a read-only location, or locked, opens read-only: editing is allowed
  but Save is refused with `project.readOnly`; Save As is available. The window title marks the project read-only.
* **Missing media.** Drawings, images, sounds or movies referenced by the project but missing on disk do not stop the
  open. The user is warned once per open with a list of the missing items (not one message per item); the elements
  remain, with placeholders. (`project.mediaMissing`.)
* **Older format versions** open normally. The first save of such a project warns that saving upgrades the package to
  the current format version and that older versions of the application will no longer open it; the user may
  continue or cancel (`project.formatUpgrade`). Save As to a new location leaves the original untouched; the warning
  then says so. The same rule applies to libraries saved with the project.
* **Global library** files and palette exchange files (`.animpal`) and template files (`.animtpl`) are not opened as
  projects; they are handled by the library and palette import (08, 03).

### 3.3 Recent projects

* A list of recently opened or saved projects, most recent first, shown in the main menu (*Open Recent*) and on the
  start screen.
* Maximum length: `general.recentProjects.maxCount` (default 10). Re-opening a project moves its entry to the top (no
  duplicates; paths compared after normalisation).
* Each menu entry shows the project name; the full path is shown in the status bar or tooltip.
* Choosing an entry whose project no longer exists reports `project.notFound` and removes the entry.
* The list ends with a *Clear* command which empties it.
* The list is persisted (§11).

### 3.4 Close

* Close Project (platform-standard Primary+W when no panel/window-close binding takes precedence — see §6.7 and the
  design decisions) closes the current project without quitting.
* The unsaved-changes rule (§3.5) applies.
* Afterwards the start screen is shown if `general.startup.behaviour` = `startScreen`; otherwise the main window remains
  with no project (§1.3).
* Closing clears the undo history and deletes the project's autosave recovery data.

### 3.5 Dirty state and the unsaved-changes rule

* The undo system keeps a **clean marker** for the project and a separate one for the **global library**. The
  project is *modified* when the current undo position differs from its clean marker; the same for the global library.
* Undoing back to the saved position makes the project clean again; redoing away from it makes it modified again.
* Changes that are not undoable but must be saved (none are expected in the project; see §6.1) MUST set a permanent
  modified flag until the next save.
* The window title and the platform's document-modified indicator (for example the dot in the close button on macOS,
  an asterisk elsewhere) show the modified state.
* **Unsaved-changes rule.** Before New, Open, Close, opening a recent project, opening a backup/recovery, or Quit, if
  the project is modified, the user is asked whether to save the changes, with three choices: **Save** (save, then
  continue; if the save fails or the user cancels a Save As location choice, the operation is aborted), **Don't Save**
  (discard, then continue) and **Cancel** (abort the operation).

### 3.6 Save, Save As, global library

**Save** (Primary+S; main menu and main toolbar):
* Writes the project package, including the project-local library. If the global library (or another opened library
  with write access, 08) has unsaved changes, it is saved as well (see the design decisions). A dedicated *Save
  Global Libraries* command (`library.save`, 08_library_storyline.md) saves only the libraries. In this section
  "global library" stands for all such opened libraries.
* A project without a location behaves as Save As.
* A project opened read-only fails with `project.readOnly` and the GUI offers Save As instead.
* Long saves show progress with step text; saving is not cancellable once writing has started.
* **Atomic write.** The package is written to a temporary location and swapped into place only when complete, so a
  failed save never damages the previously saved version.
* **Partial failure.** Project and global library are saved independently. The user is told precisely which of the two
  were saved and which were not (`project.saveFailed`, `library.saveFailed`); the clean marker of each is reset only
  for the one that was saved.
* **Failure reasons** reported: disk full, location read-only or not writable (`project.permissionDenied`), file in
  use, generic write error (`project.saveFailed`).
* On success: both clean markers are reset, the project moves to the top of the recent list, a backup of the
  previous version is kept (§3.7) and the project's autosave recovery data is deleted.

**Save As** (Shift+Primary+S; main menu):
* Asks for a new name and location; the whole project package is written there and the copy becomes the current
  project (the original remains as it was at its last save).
* If the destination exists, the user must confirm overwriting it (`project.exists`).
* Saving into a location inside the current project package is refused (`project.pathInsideProject`).
* Failure to create the destination is reported (`project.saveFailed` / `project.permissionDenied`); the current
  project and its location are unchanged.
* Cancelling the location choice cancels Save As with no change.

### 3.7 Autosave, crash recovery and backups

* **Autosave.** When `files.autosave.enabled` (default on) and the project has unsaved changes, the application writes
  a recovery copy of the project every `files.autosave.intervalMinutes` (default 5) minutes, to a per-user recovery
  folder (never over the user's project file). Autosave MUST NOT change the dirty state, the undo history or the
  project location, MUST NOT interrupt the user (it runs without a modal progress window; if it needs more than a
  moment it runs in the background or is postponed while the user is in the middle of a gesture) and is skipped
  while a modal operation is running.
* **Recovery.** After an abnormal termination, recovery data is offered at next launch (§1.1, §2). *Recover* opens the
  recovery copy as a modified project whose location is the original project path (if it had one), so that Save writes
  to the original location. *Discard* deletes the recovery data. Recovery data is deleted after a successful save, a
  close that discards changes, or a clean quit.
* **Backups.** On every successful save, the previously saved version of the package is kept as a backup.
  `files.backups.count` (default 3, 0 = off) versions are kept per project, oldest removed first, in a per-user
  backups folder keyed by project identity. Backups can be listed and opened; a backup opens as an untitled copy (first
  save asks for a location).

### 3.8 Project properties

**Purpose.** Change the frame rate and camera size of the open project, and optionally store them as the defaults for
new projects. Requires an open project.

| Field | Type | Default | Range | Effect |
|---|---|---|---|---|
| Frame rate | number (fps) | current project value | 1–120 (NTSC fractional rates accepted) | Playback and export frame rate. |
| Camera size | width × height (pixels) | current camera size | each 16–8192 | Output resolution and camera aspect. |
| Set as default | action | — | — | Stores the values in `project.defaults.frameRate`, `project.defaults.cameraWidth`, `project.defaults.cameraHeight`; the user is told this affects only new projects. |

* Applying the change is one undoable step ("Change Project Properties"). OK is disabled while a field is empty or not
  a valid number.
* Changing the camera size keeps the camera centre fixed and rescales the field guide; existing artwork is not
  scaled.
* Changing the frame rate does not retime exposures; frame counts are kept (durations in seconds change).

---

## 4. Working modes: drawing and staging

The application has two editing contexts. They share one project, one current scene and one current frame, but
show different things and offer different tools.

| Aspect | **Drawing mode** | **Staging mode** (scene planning) |
|---|---|---|
| Main view | **Drawing view**: one drawing at a time, flat, without camera or pegs | **Camera view** (plus **Top view** and **Side view**): the composed scene through the active camera, in multiplane 3-D space |
| Element list / timing | **Exposure sheet** (X-sheet columns) | **Timeline** (layers with pegs, effects, cameras) |
| Elements visible | Drawing, image and media elements present in the exposure sheet. Pegs, cameras and effects are not shown. | All elements, including pegs, cameras and effects |
| Tools | Drawing tools (02). Drawing tools also work directly in the Camera view. | Animation tools, bone tools, effect tools (05, 06) |
| Compositing order for quick preview | Exposure sheet column order | Timeline layer order |
| Element visibility commands | Exposure-sheet visibility (hide, show all, hide all others) | Timeline visibility (show/hide, show all, hide all others) |

**How the mode is determined.**
* The mode is derived from the type of the **active view**: a Drawing view means drawing mode; a Camera, Top or Side
  view means staging mode. `app.state.get` reports the mode; `app.mode.set` activates (or switches the active view
  window to) a view of the requested type.
* Each view window can switch its own type between Drawing, Camera, Side and Top using view-type controls in the view
  window (§5.3).
* Selecting a workspace also decides which view is in the central area (§5.5).
* Shortcuts can be context-specific: the same key may run different commands in drawing and staging mode (§8).

**Cross-mode warning.** When the user adds elements from the exposure sheet that cannot appear in drawing mode (peg,
camera, effect), warn that the new elements are not visible or editable in drawing mode and that switching to staging
mode shows them. This is a "don't show again" tip (`timing.elementsHiddenInDrawingMode`, §9.4).

**Switch active view** (available in every view window and from the main menu) toggles the active view window
between its current type and the type it had before. The zoom, pan and rotation of the type being left are stored
and restored when the user switches back.

---

## 5. Main window

### 5.1 Anatomy

A single main window contains:

1. **Menu bar** with the global commands. On macOS, About, Preferences (Primary+,) and Quit are in the application
   menu.
2. **Main toolbar** (customisable, §5.7).
3. **Tool palettes**: drawing tools, animation tools, bone tools, effect tools, sound tools. They can float or dock.
4. **Central area** holding the main view of the current workspace, normally the Camera view or the Drawing view.
5. **Dock areas** (left, right, top, bottom) holding the other panels, with tabbing and stacking.
6. **Status bar** (§9.1), which also hosts the agent-session indicator (§12.1).

**Window title.** Shows the project name, a modified indicator, a read-only indicator when applicable, the current
scene name and, where it applies, the current frame. With no project, only the application name.

### 5.2 Panel catalogue

Every panel has a stable panel ID used by workspaces, the window commands and the agent interface. Every panel can
be shown or hidden from the main menu (Window menu or equivalent) with a checkable item; frequently used panels SHOULD
have default shortcuts.

| Panel ID | Panel | Purpose (details in) |
|---|---|---|
| `cameraView` | Camera view | The composed scene through the current camera; staging, animation tools and drawing in place (05) |
| `drawingView` | Drawing view | Edit a single drawing with light table and onion skin (02) |
| `topView` | Top view | Scene seen from above; depth placement of elements and cameras (05) |
| `sideView` | Side view | Scene seen from the side (05) |
| `timeline` | Timeline | Layers, pegs, keyframes and element timing; the title shows the current scene and element (04) |
| `xsheet` | Exposure sheet | Columns of drawings and their exposure; the title shows the current scene (04) |
| `functionEditor` | Function editor | Animation curves: position, angle, scale, velocity (05) |
| `properties` | Properties | Context-dependent properties of the selected object or active tool: drawing, image, media, peg, camera, bone, skeleton, motion point, colour transform, drop shadow, cells, pen, text, palette, tool options; shows an explicit "no properties" state when empty (02, 05, 06) |
| `colour` | Colour | Palettes, styles and swatches (03) |
| `pen` | Pen | Pen/brush size list and pen style editor (02) |
| `text` | Text | Font, size, style and alignment for the text tool (02) |
| `cells` | Cells | Information and drawing thumbnails of the current element (04) |
| `library` | Library | Global and project libraries, catalogs, drag-and-drop reuse (08) |
| `storyline` | Storyline | Scenes in order and the transitions between them (08) |
| `sceneManager` | Scene manager | List of scenes: add, delete, reorder, rename, show/hide, make current (08) |
| `effectPreview` | Effect preview | Full-quality render of the current frame or range with effects; full/half/quarter size; render frame, render range, render from current frame; clear cache (06) |
| `motionRecording` | Motion recording | Settings for recording motion from pointer movement (05) |
| `lipSync` | Lip-sync | Lip-sync mapping (07) |
| `history` | History | Undo history list (§10) |
| `soundEditor` | Sound element editor (window) | Trimming, looping, volume, lip-sync preview of a sound element (07) |
| `capture` | Capture window | Stop-motion and camera capture (09) |

Tool palettes have their own IDs: `drawingTools`, `animationTools`, `boneTools`, `effectTools`, `soundTools`.

### 5.3 View window chrome (Drawing, Camera, Top, Side)

Each view window provides:
* **View-type controls** to switch the window between Drawing, Camera, Side and Top (§4).
* **Navigation controls**: switch view, reset view, recentre, reset rotation, reset zoom (§6.5).
* **Scroll bars** for panning.
* **Wheel and gestures**: wheel zooms about the pointer; with the horizontal-scroll modifier it scrolls horizontally;
  trackpad pinch zooms and two-finger drag pans; trackpad rotate SHOULD rotate the view.
* **Context menu**: a view-type-specific context menu with the commands relevant to the view (contents listed in the
  owning sections).
* **Drop target**: library templates and external files (images, sounds, movies) can be dropped onto a view, with a
  drop preview during the drag (08, 09).
* An optional per-view toolbar for the staging views.

### 5.4 Docking, floating and tabbing

* Every panel except the central view can be:
  * docked in the left, right, top or bottom dock area;
  * stacked with other panels in the same area, with resizable splitters;
  * tabbed with other panels in one slot;
  * floated as a separate window.
* **Tabs.** Each tab shows the panel title. Dragging a tab out undocks the panel (after a small drag threshold).
  Double-clicking a tab or panel title toggles floating/docked. The tab bar's context menu lists the panels with
  show/hide checkmarks.
* **Floating panels** remember their last floating size and position.
* Dragging inside a view MUST NOT accidentally start docking operations.
* **Corner ownership.** A toggle (*Extend side panels*) chooses whether the left and right dock areas run full height
  (owning the window corners) or the top and bottom areas run full width.
* **Show/hide.** Showing a panel that is visible but behind another tab brings its tab forward and focuses it. Hiding
  removes it from the layout, remembering its place so that showing it again restores it there.
* **Close active panel.** Hides the focused panel or floating window.
* **Toggle floating windows.** One command hides all floating panels and floating tool palettes at once; running it
  again shows them again.
* **macOS window commands.** Minimize (Primary+M) and Bring All to Front are provided as the platform requires.
* **Visibility guarantee.** When a layout is restored on a smaller or different screen arrangement, panel geometry is
  clamped so that every window remains reachable on screen.

### 5.5 Workspaces

A **workspace** is a named, saved arrangement of the main window. It stores:
* the dock layout (which panels are docked where, sizes, tab groups, current tab of each group, visibility);
* the floating geometry of each panel and tool palette, and their visibility;
* which panel is the central view;
* corner ownership;
* the main toolbar configuration (§5.7);
* a separate arrangement for full-screen mode (§5.6).

**Built-in workspaces.** The application ships at least these (arrangements are recommended defaults):

| Workspace | Central view | Visible, docked | Tool palettes |
|---|---|---|---|
| Default | Camera view | Right: tab group Colour (front) / Properties / Library, and below it tab group Pen (front) / Text / Cells. Bottom: tab group Timeline (front) / Exposure sheet / Function editor / Storyline. Side panels extended. | drawing, animation, bone, effect |
| Staging | Camera view | Right: Properties / Cells tab group beside a vertical stack of Side view above Top view. Bottom: Timeline / Function editor tab group beside Library. Bottom panels full width. | drawing, animation, bone, effect |
| Drawing | Drawing view | Right: Exposure sheet on top; below it tab group Colour (front) / Pen / Text. | drawing, animation |
| Focused drawing | Drawing view | Right: Colour above Pen; everything else hidden. | drawing |

**Workspace commands** (main menu workspace submenu and a workspace drop-down on the main toolbar):

| Command | Behaviour |
|---|---|
| Switch to workspace | Applies the chosen workspace. If the current one was modified and `general.workspace.autoSave` is on (default), it is saved first. The list is ordered by each workspace's stored position. |
| New workspace | Asks for a name (default "User Workspace") and a position in the list (1 = first); creates a workspace from the current arrangement. Empty or duplicate names are refused (`workspace.nameInvalid`, `workspace.exists`). |
| Save workspace | Stores the current arrangement into the current workspace. |
| Rename workspace | Renames the current workspace (same validation). |
| Delete workspace | Removes the current workspace. Disabled for built-in workspaces (`workspace.builtIn`) and when only one workspace remains (`workspace.last`). |
| Restore workspace | Returns a modified built-in workspace to its bundled arrangement. |
| Restore default workspace | Applies the bundled Default workspace. |

* **Persistence.** User workspaces and modifications of built-in workspaces are stored per user (§11). The workspace
  in use at quit is restored at the next launch. Built-in arrangements are read-only and always restorable.
* Workspace and layout changes are not undoable and do not modify the project.

### 5.6 Full screen

* A full-screen toggle (platform-standard shortcut: Ctrl+Primary+F on macOS, F11 elsewhere) switches the main window
  in and out of full-screen presentation.
* Each workspace has a separate full-screen arrangement: entering full screen applies it, leaving restores the normal
  arrangement.
* Combined with *toggle floating windows* (§5.4) it gives an unobstructed canvas.

### 5.7 Toolbars and customisation

**Main toolbar.**
* The user can add, remove and reorder items, separators, fixed spaces and flexible spaces from a palette of allowed
  items, and restore the default set. The configuration is stored per workspace. The toolbar can be shown or hidden.
* **Allowed items** (at least): file (new, open, save), edit (cut, copy, paste, undo, redo), playback (first frame,
  previous frame, play/stop, next frame, last frame, loop), grid (grid on/off, grid type, overlay), onion skin (onion
  skin on/off, number of previous drawings, number of next drawings, auto light table), pegs (show all pegs, peg
  ghosts, pivot path, element arms), sound (lip-sync, import sound, edit sound), drop-downs (camera list, scene list,
  workspace list).
* **Default set, in order:** grid · grid type · overlay · | · onion skin · previous drawings · next drawings · auto
  light table · | · first frame · play/stop · last frame · loop · space · lip-sync · flexible space · camera list ·
  scene list · workspace list.
* **Group items** (grid type, previous drawings, next drawings) are pop-up buttons: the icon shows the current member
  and a click opens the list of members.
* **Drop-downs.** The camera list shows the scene's cameras and makes the chosen one current; the scene list shows the
  scenes and makes the chosen one current; the workspace list shows the workspaces, switches on selection, and offers
  the workspace commands.
* Every toolbar item is a command reference (command ID plus optional fixed parameters), so customisation can include
  any registered command.

**Tool palettes** (drawing, animation, bone, effect, sound): contents are defined in 02, 05, 06 and 07. Members of an
exclusive group behave as radio tools. Each palette can be shown or hidden from the main menu; position and
visibility are stored per workspace.

### 5.8 Themes

* The application ships at least a **light** and a **dark** theme, and a *follow system* option that switches with the
  OS appearance.
* A theme defines the interface colours, panel and tab styling, tooltip styling, and icon variants as needed for
  contrast. Canvas colours (drawing background, element colours) are preferences (§7), not theme properties, unless
  the user chooses theme-default values.
* Changing the theme applies immediately, without restart.
* The preferences show the available themes with a preview.
* User themes MAY be added as theme files in the per-user settings folder; invalid theme files are ignored with a
  logged warning.

---

## 6. Global commands

### 6.1 Undo and redo

* **Multi-level** undo and redo across all editing operations of the open project, including palette edits and global
  library edits. Undo reverses changes in reverse order; redo is available only after undo and is cleared when a new
  change is made.
* **Labels.** The menu items name the operation to be undone or redone (for example "Undo Move Peg", "Redo Add
  Keyframe"). Every undoable command defines its label. When there is nothing to name, plain "Undo"/"Redo" is shown and
  the item is disabled.
* **Grouping.** A multi-step interaction (a drag, pasting many cells, applying a dialog, an agent `batch`) is recorded
  as **one** step. Cancelling an interaction (for example Escape during a drag) records nothing.
* **Scope.** One undo stack per open project, shared by all views. Creating, opening or closing a project clears it.
  Save does not clear it but moves the clean markers (§3.5).
* **Depth.** At least 200 steps; SHOULD be unlimited subject to a memory bound, discarding the oldest steps first.
* **Not undoable:** view navigation (zoom, pan, rotate), selection changes, workspace/panel/toolbar layout, playback,
  preferences, keymap, theme, and tip suppression.
* **Attribution.** Each step records its source: the user, or an agent session (§12.1). Agent steps are visibly marked
  in the History panel and in the undo/redo menu labels (details in 11_agent_interface_mcp.md).
* Platform-standard shortcuts: Undo Primary+Z, Redo Shift+Primary+Z (Windows/Linux also Ctrl+Y).

### 6.2 Cut, copy, paste, paste as new element, delete

All edit commands act on the **current edit context**: the focused panel and, in views, the kind of the active tool.
The same command therefore acts on drawing objects, elements, cells, keyframes or templates.

| Context | Cut/Copy acts on | Paste behaviour |
|---|---|---|
| Drawing view, or Camera view with a drawing tool | Selected drawing objects (strokes, zones, groups, text) | Into the current drawing at the same position (02) |
| Camera/Top/Side view with an animation tool | Selected elements, or peg / keyframe properties | Element properties or keyframes onto the selection (05) |
| Exposure sheet | Selected cells (drawings and exposure) | At the selected cell, using the paste options (§6.3) (04) |
| Timeline | Selected cell ranges (exposures and keyframes) | Using the timeline paste options (§6.3) (04) |
| Library | Templates | Copies templates between catalogs (08) |
| Function editor | Function points | Onto the selected function (05) |

* **Cut** removes the selection and places it on the clipboard with all its properties; **Copy** leaves it in place.
  Both are available in every context's context menu and via Primary+X / Primary+C; Paste via Primary+V.
* **Paste as new element** (exposure sheet and Timeline) creates a new element from the copied cells, exposes it
  starting at the selected frame and shows the content there.
* **Delete** (Delete / Backspace) removes the selected objects; in cell context it clears the cells.
* **Paste errors**: pasting content into a context that cannot accept it (`edit.pasteNotAllowed`); pasting while bone
  deformation is being computed (`edit.busy`); pasting a drawing into an element of the wrong type
  (`edit.wrongElementType`).
* **Clipboard.** The internal clipboard holds a typed, structured payload preserving the full object model (§12.4).
  When drawing objects are copied, the application SHOULD also place a bitmap (PNG) and a vector (SVG) representation on
  the system clipboard for other applications. Pasting plain images from the system clipboard into a drawing context
  SHOULD import them as an image (09).

### 6.3 Cut special, paste special, paste special again

**Timeline / exposure-sheet options.** The same option set is used, and remembered separately, for three operations:
*paste special*, *cut special*, and *drag-and-drop of cells* in the Timeline. Running Paste Special or Cut Special from
the GUI shows the options (pre-filled with the remembered values); confirming runs the operation and remembers the
options for that operation type.

| Group | Option | Effect | Default |
|---|---|---|---|
| Drawings | Overwrite existing drawings | When pasting exposures into a drawing or image element, drawings with the same names are overwritten; when off (or when no drawing of that name exists) a new drawing is created. | on |
| Drawings | Create/delete exposure | Adds or removes exposures in drawing and image elements as part of the operation. | on |
| Drawings | Extend exposure automatically | Extends the exposure of drawings and images to fill the range up to the destination frame. | off |
| Drawings | Update colours | When pasting templates, update the project's colours with those in the template. | on |
| Drawings | Keyframe substitution | Replaces keyframe properties at the current frame and all following frames in sequence. | off |
| Pegs | Create/delete motion keyframes | Copies or removes motion keyframes. | on |
| Pegs | Create/delete rotation keyframes | Same for rotation. | on |
| Pegs | Create/delete scale keyframes | Same for scale. | on |
| Pegs | Create/delete pivot keyframes | Same for pivot. | on |
| Pegs | Create/delete velocity keyframes | Same for velocity. | on |
| Pegs | Create/delete skew keyframes | Same for skew. | on |
| Pegs | Add boundary keyframes | Adds a keyframe at both ends of the pasted range so the surrounding animation is preserved. | off |

**Motion-point paste special.** When a motion point or keyframe is selected (staging views or function editor), Paste
Special pastes selected *values* of the copied keyframe onto the selected motion point:

| Group | Option | Pastes | Condition |
|---|---|---|---|
| Motion | X (east–west) | X position | — |
| Motion | Y (north–south) | Y position | — |
| Motion | Z (front–back) | Z position | — |
| Motion | Tension / Continuity / Bias | path parameters at the point | — |
| Rotation | Angle | rotation angle | copied keyframe is locked in time |
| Rotation | Pivot | rotation pivot | offset rotation pivot and time-locked copied keyframe |
| Scale | X / Y | width / height scale | time-locked copied keyframe |
| Scale | Pivot | scale pivot | offset scale pivot and time-locked copied keyframe |

Options that do not apply to the clipboard content are disabled.

**Paste special again** repeats the last paste special with the same options without showing them. It is disabled
until a paste special has been done in the session.

### 6.4 Select all, deselect all, generic selection

* **Select all** (Primary+A) selects every selectable object in the current edit context: all drawing objects of the
  current drawing (drawing contexts), all elements of the scene (staging views), all cells (exposure sheet / Timeline),
  all items of the current catalog (library), all points of the current function (function editor).
* **Deselect all** (Shift+Primary+A) clears the selection of the current edit context, including cell selections.
* Each context has its own selection; switching focus does not clear the other contexts' selections. Selection changes
  are not undoable.

### 6.5 View navigation

Generic navigation applies to the active view window (Drawing, Camera, Top, Side; the Timeline, exposure sheet and
function editor use the same zoom commands where they support zoom).

| Command | Behaviour |
|---|---|
| Zoom in / Zoom out | One step up/down the zoom table, keeping the view centre fixed. |
| Reset zoom | Returns to 100 %. |
| Rotate view clockwise / counter-clockwise | Rotates the *view* (not the artwork) by 30°. |
| Reset rotation | View rotation back to 0°. |
| Recentre | Pans back to the centre of the drawing area or camera frame. |
| Reset view | Removes all zoom, pan and rotation. |
| Centre on pointer | Pans so that the point under the pointer becomes the view centre. |
| Switch active view | §4. |

* **View transform.** Each view window has its own (pan offset, zoom factor, rotation angle), remembered per view type
  so that switching type and back restores it.
* **Zoom table.** 6.25, 12.5, 25, 33.3, 50, 66.7, 100, 150, 200, 300, 400, 600, 800, 1200, 1600, 3200, 6400 %. Zoom is
  clamped to 6.25 %–6400 %. Continuous zoom (wheel, pinch, zoom tool drag) may produce intermediate values; stepping
  then moves to the next table value.
* **Navigation tools** (available in every view; selected like other tools, 02): *Zoom* (click zooms in on a point,
  drag a rectangle zooms to it, with the alternate modifier zooms out), *Pan* (drag pans; holding Space gives the pan
  tool temporarily from any tool), *Rotate view* (drag rotates the view; with the constrain modifier it snaps to 15°
  steps). A command cycles between Zoom and Pan.
* View-specific display toggles (grid, onion skin, light tables, strokes, scene background colour, pegs, effects,
  waveforms, annotation elements, outline on selection, current drawing on top) are defined in their owning sections
  (02, 04, 05, 06, 07).

### 6.6 Frame and element navigation

Global keyboard navigation, available in any view when no text field has focus:

| Command | Behaviour |
|---|---|
| Previous / next frame | Current frame −1 / +1, clamped to the scene's frame range. |
| Previous / next drawing | Moves to the previous / next frame at which a *different* drawing is exposed in the current element. |
| Previous / next element | Makes the previous / next element current, in Timeline order (staging) or exposure-sheet order (drawing). |
| Previous / next exposure-sheet column | As above, always in exposure-sheet order. |
| Show current element in timeline | Scrolls the Timeline to show the current element (04). |

Skeleton-root selection, motion-point stepping and onion-skin cycling are defined in 05 and 02.

### 6.7 Window and help commands

* Show/hide each panel and tool palette (§5.2), close active panel, toggle floating windows, extend side panels, full
  screen, show/hide status bar, show/hide main toolbar, customise main toolbar, minimise, bring all to front.
* **Help.** Open the documentation (platform-standard shortcut: Primary+? on macOS, F1 elsewhere) in the default
  browser or a built-in help viewer; enter context-help mode (§9.3); open the learning resources (§2).

---

## 7. Preferences

### 7.1 Behaviour

* Opened from the platform-standard location (application menu on macOS with Primary+, ; Edit or Tools menu
  elsewhere).
* Settings are grouped by category (below). The grouping in the window is design work; a search field over setting
  names and descriptions SHOULD be provided.
* Every setting shows a short description and its context help (§9.3).
* Edits are validated inline. **OK** (or equivalent) validates all edited values and, if valid, applies and saves
  them; values that fail validation keep the window open and focus the offending field (`prefs.outOfRange`,
  `prefs.invalidValue`). **Cancel** discards all edits made since the window was opened, including keymap edits.
* Settings take effect immediately where possible. Settings marked *restart* take effect at the next launch; the
  window says so next to the setting when it is changed.
* Each category offers *Restore defaults* for that category.
* Preference changes are not undoable and do not modify the project.

### 7.2 Preference keys

Every preference has a stable key of the form `<category>.<group>.<name>` (lower camelCase segments; the group may be
omitted, as in `user.name` or `library.globalPath`), for example `drawing.onionSkin.previousCount`. Keys are the same
in the persisted settings, the GUI and the agent interface. Other sections MAY register further preferences in their
own category (for example `staging.motionRecording.*` in 05, `agent.*` in 11); they follow the same rules.

Types: `bool`, `int`, `number`, `string`, `enum`, `colour` (#RRGGBBAA), `path`.

### 7.3 Preference catalogue

**General**

| Key | Type | Default | Range / values | Effect | Applies |
|---|---|---|---|---|---|
| `user.name` | string | "" (empty = the OS account's full name) | ≤ 100 chars | Author name stamped into templates created afterwards (08). | live |
| `user.copyright` | string | "" | ≤ 200 chars | Copyright owner stamped into new templates (08). | live |
| `library.globalPath` | path | per-user documents folder / application library folder | existing writable folder | Location of the global library (08). Changing it switches libraries immediately; templates linked from the old location keep displaying only if they are imported into the new location or the old library folder is moved there (the user is told this). | live |
| `general.startup.behaviour` | enum | `startScreen` | `startScreen` \| `lastProject` \| `openDialog` \| `newDialog` \| `newDefaultProject` | What happens at launch and after Close (§1.1, §3.4). | next launch |
| `general.language` | enum | system language if installed, else `en` | installed localisations, shown by their own names | Interface language. | restart |
| `general.recentProjects.maxCount` | int | 10 | 0–30 | Length of the recent-projects list. | live |
| `general.workspace.autoSave` | bool | true | — | Save the current workspace automatically before switching workspace and at quit. | live |

**Files and safety**

| Key | Type | Default | Range / values | Effect | Applies |
|---|---|---|---|---|---|
| `files.autosave.enabled` | bool | true | — | Periodic recovery copies (§3.7). | live |
| `files.autosave.intervalMinutes` | int | 5 | 1–60 | Autosave interval. | live |
| `files.backups.count` | int | 3 | 0–20 (0 = off) | Previous versions kept per project (§3.7). | live |

**New-project defaults**

| Key | Type | Default | Range / values | Effect | Applies |
|---|---|---|---|---|---|
| `project.defaults.frameRate` | number | 24 | 1–120 | *Default* format of the new-project form. | live |
| `project.defaults.cameraWidth` | int | 1920 | 16–8192 | Same. | live |
| `project.defaults.cameraHeight` | int | 1080 | 16–8192 | Same. | live |

**Interface**

| Key | Type | Default | Range / values | Effect | Applies |
|---|---|---|---|---|---|
| `interface.theme` | enum | `system` | `system` \| `light` \| `dark` \| installed user themes | §5.8. | live |
| `interface.statusBar.visible` | bool | true | — | Show the status bar. | live |
| `interface.mainToolbar.visible` | bool | true | — | Show the main toolbar. | live |
| `interface.timeMeasurement` | enum | `frames` | `frames` \| `feetAndFrames` | Timeline ruler and frame displays show frame numbers, or feet+frames (16 frames per foot). | live |
| `interface.elementColours.drawing` | colour | #8FD98FFF | any | Default marker colour of drawing elements in Timeline and exposure sheet (per-element override in 04). | live |
| `interface.elementColours.image` | colour | #A8E0A8FF | any | Image elements. | live |
| `interface.elementColours.sound` | colour | #B8A8D8FF | any | Sound elements. | live |
| `interface.elementColours.peg` | colour | #8080FFFF | any | Peg elements. | live |
| `interface.elementColours.skeleton` | colour | #6A8FB5FF | any | Skeleton elements. | live |
| `interface.elementColours.bone` | colour | #7FA3C8FF | any | Bone elements. | live |
| `interface.elementColours.media` | colour | #A0A050FF | any | Media (movie) elements. | live |
| `interface.elementColours.camera` | colour | #FF8080FF | any | Camera elements. | live |
| `interface.elementColours.effect` | colour | #C88040FF | any | Effect elements. | live |
| `interface.elementColours.effectParameter` | colour | #A05030FF | any | Effect-parameter (mask/matte) elements. | live |
| `interface.storylineColours.scene` | colour | #9CC3E6FF | any | Scene blocks in the storyline (08). | live |
| `interface.storylineColours.sceneWarning` | colour | #F2A65AFF | any | Scene blocks with a warning (for example extended by transitions). | live |
| `interface.storylineColours.transition` | colour | #C9C9C9FF | any | Transition blocks in the storyline. | live |
| `interface.background.timeline` | colour | theme default | any | Timeline background. | live |
| `interface.background.drawingView` | colour | #FFFFFFFF | any | Background of the Drawing view and of the Camera view when the scene background colour is not used. | live |

**Drawing aids** (used by 02; the toolbar and view commands of 02 write the same keys)

| Key | Type | Default | Range / values | Effect | Applies |
|---|---|---|---|---|---|
| `drawing.onionSkin.enabled` | bool | false | — | Onion skin on/off. | live |
| `drawing.onionSkin.previousCount` | int | 1 | 0–3 | Number of previous drawings shown. | live |
| `drawing.onionSkin.nextCount` | int | 1 | 0–3 | Number of next drawings shown. | live |
| `drawing.onionSkin.transparency` | bool | false | — | Onion-skin drawings are semi-transparent. | live |
| `drawing.onionSkin.colourShading` | bool | true | — | Onion-skin drawings are tinted with the colours below. | live |
| `drawing.onionSkin.previousColour` | colour | #FF0000FF | any | Tint of previous drawings. | live |
| `drawing.onionSkin.nextColour` | colour | #00C000FF | any | Tint of next drawings. | live |
| `drawing.onionSkin.outlineOnly` | bool | false | — | Show onion-skin drawings as outlines. | live |
| `drawing.lightTable.staticShade` | bool | true | — | Wash out drawings shown on the static light table. | live |
| `drawing.lightTable.autoShade` | bool | true | — | Wash out drawings shown by the automatic light table. | live |

**Display**

| Key | Type | Default | Range / values | Effect | Applies |
|---|---|---|---|---|---|
| `display.antiAliasing` | bool | true | — | Anti-alias the whole view. | live |
| `display.smoothBitmaps` | bool | true | — | Filtered sampling of bitmap images and texture fills on screen. | live |
| `display.smoothToolOverlays` | bool | true | — | Anti-alias tool overlays (handles, rotation circle). | live |
| `display.smoothMotionPaths` | bool | true | — | Anti-alias motion paths in staging views. | live |
| `display.fastPencilRendering` | bool | false | — | Faster, approximate on-screen rendering of centre-line (pencil) strokes when many of them slow the display. | restart |
| `display.bitmapPreviewMaxSize` | int | 0 | 0 (full) or 256–8192 | Maximum on-screen resolution (longest side, pixels) for bitmaps and textures; lower is faster. Never affects export. | live |
| `display.cacheMemoryPercent` | int | 30 | 5–80 | Share of system memory used for display caches. Does not affect output quality. | live |
| `display.sceneBackground.drawingView` | bool | false | — | Use the scene background colour in the Drawing view (toggled by a view command, 05). | live |
| `display.sceneBackground.stagingViews` | bool | true | — | Use the scene background colour in staging views. | live |

**Tablet**

| Key | Type | Default | Range / values | Effect | Applies |
|---|---|---|---|---|---|
| `tablet.ignore` | bool | false | — | Treat the tablet as a mouse (no pressure or tilt). | live |
| `tablet.alternativeInput` | bool | false | — | Use the alternative OS tablet input API (for tablets that misbehave with the default one). | restart |
| `tablet.portraitCorrection` | bool | false | — | Correct coordinates and pressure on tablet computers used in portrait orientation. | live |

**Staging** (used by 05)

| Key | Type | Default | Range / values | Effect | Applies |
|---|---|---|---|---|---|
| `staging.motionPoint.defaultTension` | number | 0.0 | −1.0–1.0 | Tension of newly created motion points (+1 = sharp corner). | live |
| `staging.motionPoint.defaultContinuity` | number | 0.0 | −1.0–1.0 | Continuity of new points (0 smooth, ±1 abrupt change of direction). | live |
| `staging.motionPoint.defaultBias` | number | 0.0 | −1.0–1.0 | Bias of new points (0 neutral). | live |
| `staging.units.showSuffix` | bool | true | — | Show the unit suffix next to coordinate values. | live |
| `staging.units.coordinate` | enum | `fields` | `fields` \| `inches` \| `centimetres` | Unit for positions in Properties, function editor and elsewhere. | live |
| `staging.units.angle` | enum | `degrees` | `degrees` \| `radians` | Unit for angles. | live |
| `staging.selection.boxHighlight` | bool | true | — | Draw a box around the selected element in the Camera view; when off the selection is shown by fading the rest. Required by *show outline on selection*. | live |
| `staging.selection.alwaysVisible` | bool | true | — | Show elements selected in the Timeline or exposure sheet even when they are hidden. | live |
| `staging.selection.outline` | bool | false | — | Draw the contour of the selected elements (*show outline on selection*). | live |
| `staging.keyframes.linearVelocity` | bool | true | — | New peg functions get linear velocity; when off, a default ease-in/ease-out. | live |
| `staging.keyframes.constantByDefault` | bool | false | — | New keyframes are constant (no interpolation). | live |
| `staging.keyframes.snapLastFrame` | bool | false | — | Snap the last keyframe to the last frame (05). | live |

**Timing** (used by 04)

| Key | Type | Default | Range / values | Effect | Applies |
|---|---|---|---|---|---|
| `timing.timeline.singleCellSelection` | bool | true | — | A click selects one cell; when off, a click selects the whole track and the alternate modifier + click selects one cell. | live |

**Edit**

| Key | Type | Default | Range / values | Effect | Applies |
|---|---|---|---|---|---|
| `edit.pasteSpecial.timeline` | object | defaults of §6.3 | option set of §6.3 | Remembered paste-special options. | live |
| `edit.cutSpecial.timeline` | object | defaults of §6.3 | option set of §6.3 | Remembered cut-special options. | live |
| `edit.dragDrop.timeline` | object | defaults of §6.3 | option set of §6.3 | Options for dragging cells in the Timeline. | live |
| `edit.pasteSpecial.motionPoint` | object | all on | option set of §6.3 | Remembered motion-point paste-special options. | live |

**Agent access** (defined in 11_agent_interface_mcp.md)

The preferences window MUST include an *Agent access* category that shows and edits the `agent.*` preferences
(for example `agent.server.enabled`, whether the MCP server runs) and the agent policy defaults. Keys, defaults and the
`agent.*` commands that manage them (`agent.server.set`, `agent.policy.defaults.set`, …) are specified in section 11
and are not redefined here; they are also readable and writable through `prefs.get` / `prefs.set` like any other
preference, subject to the restrictions section 11 places on agents changing their own access.

---

## 8. Keyboard shortcuts (keymap)

### 8.1 Data model

* A **binding** is (key sequence, context) → command reference. A command reference is a command ID plus an optional
  fixed parameter object (so that, for example, "zoom in" and "zoom out" can be two bindings of one zoom command).
* **Contexts**: `global`, `drawing` (drawing mode), `staging` (staging mode). A key may be bound once per context. At
  dispatch, a binding in the current mode's context wins over a `global` binding of the same key.
* A command may have any number of bindings.
* **Every command is rebindable**, including those without a default binding.
* The default keymap is data bundled with the application. User changes are stored as overrides on top of the
  defaults (§11), so defaults can be updated without losing user changes.

### 8.2 Default bindings

Only platform-standard shortcuts are fixed by this specification:

| Command | Default |
|---|---|
| New project | Primary+N |
| Open project | Primary+O |
| Save | Primary+S |
| Save As | Shift+Primary+S |
| Close project | Primary+W (see design decisions) |
| Quit | Primary+Q |
| Undo / Redo | Primary+Z / Shift+Primary+Z (Windows/Linux also Ctrl+Y) |
| Cut / Copy / Paste | Primary+X / Primary+C / Primary+V |
| Delete | Delete and Backspace |
| Select all / Deselect all | Primary+A / Shift+Primary+A |
| Preferences | Primary+, (macOS) |
| Help | Primary+? (macOS), F1 (elsewhere) |
| Full screen | Ctrl+Primary+F (macOS), F11 (elsewhere) |
| Minimise | Primary+M (macOS) |
| Print | Primary+P (09) |

The rest of the default keymap is design work, with these requirements: frequent animation operations (frame
stepping, drawing stepping, element stepping, zoom, view rotation, tool selection, exposure editing, keyframe
insertion, play/stop, quick preview, panel show/hide) SHOULD have single-key or simple-modifier defaults; the defaults
MUST be conflict-free within each context; holding Space MUST give the temporary pan tool and holding the primary
modifier with most drawing tools MUST give the temporary selection tool (02).

### 8.3 Shortcut editor

Part of Preferences.

1. **Browse.** Commands are listed by category (the command's namespace/area, plus a category for context-menu
   commands) and can be filtered by a text search over label, description and current shortcut. Commands that are
   unavailable on the current platform are omitted.
2. **Details.** Selecting a command shows its label, its long description and its current bindings (or "none") per
   context.
3. **Remove** clears a selected binding.
4. **Capture.** A capture control records the next key press including modifiers and shows it in native notation.
   Shift with a printable key is accepted. A bare modifier is not accepted. Losing focus ends the capture. Escape
   cancels the capture.
5. **Reserved keys.** Sequences reserved by the platform or the application cannot be bound; the user is told so and
   nothing changes (`keymap.reservedKey`). Reserved at least: Escape alone, bare modifiers, and OS-reserved
   combinations (for example Cmd+Tab, Cmd+H, Cmd+Option+Esc on macOS; Alt+Tab, Ctrl+Alt+Del, the Windows key on
   Windows).
6. **Conflict handling.** If the captured sequence is already bound in the same context (or in `global` when editing a
   mode context, or vice versa where it would shadow), the editor shows the command it is bound to (label and
   category) and offers **Override** (move the binding to the selected command, removing it from the other) or
   **Cancel** (no change). Without a conflict the binding is assigned at once.
7. **Reset.** Reset the selected command to its default bindings; *Reset all* restores the whole default keymap after a
   confirmation.
8. **Import / export.** The keymap MAY be exported to and imported from a file (merge or replace).
9. All keymap edits take effect when Preferences is confirmed; Cancel discards them.

### 8.4 Dispatch rules

* While a text field, numeric field or rename editor has focus, unmodified keys (letters, digits, Space, Return, Tab,
  Backspace, Delete) go to the field; shortcuts with Primary or Alt still work, except platform text-editing shortcuts
  (cut/copy/paste/undo/select all), which act on the field.
* Shortcuts are suspended while a modal dialog is open.
* A disabled command consumes its key silently (no beep, no fall-through to another binding).
* Shortcuts are shown in native notation in menus and tooltips.

---

## 9. Status bar, tooltips, context help, tips and messages

### 9.1 Status bar

At the bottom of the main window; can be shown or hidden. Segments:
1. **Description**: the long description of the menu item, toolbar button or tool under the pointer; cleared when the
   pointer leaves.
2. **Context**: current scene and frame.
3. **Pointer**: pointer coordinates in the current view, in the current coordinate unit, and the current selection
   summary.
4. **Playback**: during playback, the achieved frame rate and the frame range being played.
5. **Agent indicator**: the agent-session indicator (§12.1).

Other panels (for example the function editor) may have their own status lines (05).

### 9.2 Tooltips

* Every toolbar button, tool, panel control and preference shows a tooltip after the platform hover delay: the
  command's short description followed by its current shortcut in native notation (or the label when there is no short
  description).
* Tooltips update when bindings change. Rich tooltips (formatted text, images) MAY be used where helpful.

### 9.3 Context help

* A context-help mode (platform-standard shortcut: Shift+F1 on Windows/Linux, a Help-menu item on macOS) changes the
  pointer to a help pointer. The next click on any control, toolbar button, menu item or panel does **not** run it;
  instead a pop-up shows that item's help content (rich text, may include icons and lists).
* Escape or a click on empty space leaves the mode.
* Every command, panel, preference and dialog field has a help content. The same texts are available to agents
  (`ui.help.get`).

### 9.4 Tips and "don't show again"

* Informational warnings that users may not want to see repeatedly offer a **Don't show this again** check box.
* If the box is checked when the message is dismissed, the message ID is recorded as suppressed and the message is not
  shown again; the default action of the message is taken silently.
* Preferences offer **Show all tips again**, which clears every suppression immediately; the list of suppressible
  messages with their state SHOULD also be shown so that individual ones can be re-enabled.
* At least these messages are suppressible (each section may add more):
  * new elements not visible in drawing mode (§4) — `timing.elementsHiddenInDrawingMode`;
  * a stroke was added that is invisible until stroke display is turned on (02) — `drawing.strokeHidden`;
  * motion points found between the last two keyframes; the last one was converted to a keyframe to preserve the path
    (05) — `anim.motionPointConverted`;
  * a scene was extended with blank frames to accommodate transitions (08) — `storyline.sceneExtended`;
  * layered vector files keep their layers when imported via the import command (09) — `import.layersHint`.

### 9.5 Message conventions

* Severity levels: information, warning, error, question. Titles show the severity or the application name.
* Each message carries its stable ID (`area.reason`); error messages show it in a details area so users can report
  it.
* Buttons use specific verbs (Save, Don't Save, Replace, Create, Import, Export, Paste, Rename…) rather than generic
  Yes/No wherever possible; the default and cancel buttons follow platform conventions.
* Repeated identical warnings arising from one operation are aggregated into one message with a list.
* **Progress.** Long operations show a progress window with a title, the current step and a Cancel button where
  cancellation is supported; short blocking work shows a wait pointer. Long operations are jobs (11) so that progress
  and cancellation are also available to agents.
* **Out of memory.** An operation that fails for lack of memory is rolled back and reported (`app.outOfMemory`).

### 9.6 Shared name rule

Names that become file names or must be unique (project, scene, element, drawing, template, workspace, palette…)
obey one shared rule unless a section specifies otherwise: not empty (`name.empty`), at most 100 characters
(`name.tooLong`), no leading or trailing whitespace, no trailing dot, none of `/ \ : * ? " < > |` or control characters
(`name.invalidCharacter`). The message lists the forbidden characters.

---

## 10. Undo history panel

* The **History** panel lists the undo steps of the open project in order, with the current position marked and the
  saved (clean) position indicated. Each entry shows the operation label, its time and its source (user or agent
  session, with the agent's display name).
* Clicking an entry undoes or redoes to just after that step (one combined jump; intermediate states are not shown).
* Steps after the current position are shown dimmed until they are discarded by a new change.
* The panel is not required for any workflow (Undo/Redo remain the primary path) but MUST exist so that agent actions
  are reviewable.

---

## 11. Persisted user settings

* Stored per user in the platform's conventional location (for example `%APPDATA%\<App>` on Windows,
  `~/Library/Application Support/<App>` on macOS, `$XDG_CONFIG_HOME/<app>` on Linux), as human-readable, versioned
  files (JSON recommended). Older versions are migrated on load.
* **Contents:**

| Item | Notes |
|---|---|
| Preferences | Only values that differ from the defaults are stored, by key (§7.2). |
| Keymap overrides | §8.1. |
| Workspaces | User workspaces, modified built-in workspaces, full-screen arrangements, the workspace to restore at launch. |
| Toolbar configuration | Per workspace. |
| Main window geometry | Position, size, screen, maximised/full-screen state. |
| Recent projects | §3.3. |
| Last-used values | Most recent new-project frame rate and camera size; last folders used in Open, Save As, import and export dialogs. |
| Suppressed messages | §9.4. |
| First-run flags | First run done; bundled templates copied. |
| Autosave recovery index and backups index | §3.7 (data in per-user folders). |

* **Robustness.** If a settings file is missing, defaults are used. If it is corrupt, it is kept aside as a copy, the
  defaults are used for that file only, and the user is told once (`prefs.corrupt`). Settings are written atomically.
* **Per-project UI state** (exposure-sheet column widths, per-element colour overrides, element visibility in each
  mode, current scene and frame at save) is stored in the project package (10), not in user settings.

---

## 12. Agent interface

### 12.1 Agent-specific requirements

1. **No blocking interaction.** Every capability of this section is reachable through the commands in §12.2 with
   explicit parameters. No agent command opens a file picker, a save prompt, a confirmation box or any other modal UI.
   Interactive-only paths (Open dialog, Save As location choice, new-project dialog, shortcut capture, drag-docking)
   all have a parameterised equivalent.
2. **Explicit paths.** Project, backup, keymap and workspace file operations take absolute file-system paths (`path`).
   Relative or malformed paths fail with `fs.invalidPath`. Section 11 restricts agent access to allowed root
   folders (`fs.outsideRoots`).
3. **Unsaved-changes policy.** Commands that would discard the open project (`project.create`, `project.open`,
   `project.close`, `project.recovery.open`, `project.backup.open`, `app.quit`) take `onUnsaved`:
   * `"fail"` (default): if the project or the global library is modified, nothing happens and the command fails with
     `project.unsavedChanges`; `details` contains `{projectId, name, path, projectDirty, globalLibraryDirty}` so the
     agent can decide.
   * `"save"`: save first (as `project.save`); if the project has no location, fail with `project.pathRequired`; if
     saving fails, the command fails with the save error and nothing is discarded.
   * `"discard"`: discard the changes and continue.
4. **Overwrite policy.** Commands that write files take `onExists`: `"fail"` (default, error `project.exists` /
   `fs.exists`) or `"overwrite"`. Workspace import additionally accepts `"rename"`.
5. **Format upgrade.** Saving a project from an older format version requires `allowFormatUpgrade: true`; otherwise
   the save fails with `project.formatUpgrade` (nothing written). Save As to a new path leaves the original untouched.
6. **Messages become data.** Warnings and tips that the GUI would show during an agent command are returned in the
   result's `warnings: [{id, message, details}]` (for example `project.mediaMissing` with the list of missing files) and
   are also emitted as `ui.message.shown` events. Tip suppression (§9.4) affects only the GUI; agents always receive the
   warnings. Agent-initiated messages are not shown to the user as modal boxes; they MAY appear as non-modal
   notifications.
7. **Errors.** Failures return the `area.reason` code, a human-readable message and structured `details`. Codes used
   by this section (in addition to the generic codes of section 11 §10, e.g. `arg.missing`, `fs.invalidPath`,
   `fs.outsideRoots`, `fs.exists`, `ui.busy`, `command.disabled`): `app.outOfMemory`, `project.none`, `project.notFound`, `project.invalidFormat`,
   `project.unsupportedVersion`, `project.permissionDenied`, `project.readOnly`, `project.exists`,
   `project.pathRequired`, `project.pathInsideProject`, `project.unsavedChanges`, `project.formatUpgrade`,
   `project.saveFailed`, `library.saveFailed`, `name.empty`, `name.tooLong`, `name.invalidCharacter`,
   `edit.nothingToUndo`, `edit.nothingToRedo`, `edit.clipboardEmpty`, `edit.clipboardInvalid`,
   `edit.pasteNotAllowed`, `edit.wrongElementType`, `edit.busy`, `edit.noPreviousPasteSpecial`, `view.notFound`,
   `window.panelNotFound`, `window.toolbarItemUnknown`, `workspace.notFound`, `workspace.exists`,
   `workspace.nameInvalid`, `workspace.builtIn`, `workspace.last`, `prefs.unknownKey`, `prefs.invalidValue`,
   `prefs.outOfRange`, `prefs.corrupt`, `keymap.unknownCommand`, `keymap.invalidSequence`, `keymap.reservedKey`,
   `keymap.conflict`, `ui.unknownMessage`.
8. **GUI modal state.** While the GUI user has a modal dialog open or is in the middle of a gesture (drag, stroke),
   document-changing agent commands that conflict with it wait up to the session's `waitForUserMs` (section 11 §6.4;
   0 = fail immediately) and then fail with `ui.busy` (`details.blockedBy` describes what is open); queries and
   non-conflicting commands still run. `app.state.get` reports the modal state.
9. **Agent-session visibility.** The GUI MUST show a persistent, clearly visible indicator (in the status bar, §9.1)
   whenever an agent session is connected, with a distinct *acting* state while an agent command is executing,
   showing the agent's display name. From the indicator the user can see recent agent actions and pause or disconnect
   the session. Details: 11_agent_interface_mcp.md.
10. **Undo attribution.** Every undo step created by an agent command carries the agent session's attribution and
    appears in the History panel and the undo/redo labels marked as agent-made (§6.1, §10). An agent `batch` is one
    step. Details: 11_agent_interface_mcp.md.
11. **Long-running work.** `project.open`, `project.save`, `project.saveAs` and `project.recovery.open` are jobs: they
    return `{jobId}` immediately, report progress (step text and fraction) through `job_status`, and deliver their
    final result there. Open is cancellable (`job_cancel`); a cancelled open leaves no project open. Save is not
    cancellable once writing starts.
12. **Read-back.** Agents verify their work with `app.state.get` (§12.4), `project.info.get`, `edit.history.get`,
    `edit.selection.get`, `edit.clipboard.get`, `view.transform.get`, `window.panel.list`, `workspace.get`,
    `prefs.list`/`prefs.get`, `keymap.list`, `ui.statusBar.get`, and the generic `screenshot` / `inspect_ui` tools of
    section 11, which accept a `panelId` from §5.2.
13. **Coordinate spaces.** *View pixels*: origin at the top-left of a view's canvas area, x right, y down, in logical
    (device-independent) pixels. *Drawing space* and *scene (field) space* are defined in 02 and 05.
    `view.coordinates.convert` converts between them for a given view; `view.zoom.set` anchors and `view.pan.set`
    offsets use view pixels; `view.pan.set` centres use the view's document space (drawing space for Drawing views,
    scene space for staging views).
14. **Determinism.** The same command sequence applied to the same project produces the same document state. Object
    IDs are stable across save and reopen. Preference, keymap and workspace commands are idempotent.
15. **Preferences by key.** All preferences are readable and writable by key (§7.2); `prefs.list` returns each key's
    type, default, range or allowed values, current value, whether it needs a restart, and its description, so agents
    can discover settings without the preferences window. `prefs.set` is atomic: if any value is invalid nothing is
    changed.
16. **Edit context.** Generic edit commands take `context`: `"auto"` (default: the focused panel, or the context implied
    by `objectIds`), `"drawing"`, `"staging"`, `"xsheet"`, `"timeline"`, `"library"`, `"functionEditor"`. Agents SHOULD
    pass the context or object IDs explicitly instead of relying on focus.
17. **Key sequence notation.** Platform-neutral strings: modifiers `Primary`, `Shift`, `Alt`, `Ctrl` (the physical
    Control key; equals Primary on Windows/Linux), joined with `+` before the key name, for example `Primary+Shift+Z`,
    `Shift+F1`, `Space`, `X`. Letters are case-insensitive.

### 12.2 Command table

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `app.state.get` | query | Current application state (project, scene, frame, element, drawing, mode, active view, focused panel, active tool, selection, workspace, modal state, agent sessions) | — | `AppState` (§12.4) | n/a |
| `app.info.get` | query | Application name, version, platform, locale, installed languages, project format version | — | `{name, version, platform, locale, languages[], formatVersion}` | n/a |
| `app.quit` | action | Quit the application | `onUnsaved: enum "fail"\|"save"\|"discard" = "fail"` | `{quitting: bool}` | n/a |
| `app.mode.set` | ui | Switch to drawing or staging mode by activating (or switching) a view of that type | `mode: enum "drawing"\|"staging"; viewId?: id` | `{mode, viewId}` | no |
| `app.frame.set` | ui | Set the current frame | `frame: int (1–scene length)` | `{frame}` | no |
| `app.frame.move` | ui | Previous/next frame, or previous/next different drawing in the current element | `direction: enum "previous"\|"next"; unit: enum "frame"\|"drawing" = "frame"; count: int = 1 (1–10000)` | `{frame}` | no |
| `app.currentElement.set` | ui | Make an element current | `elementId: id` | `{elementId}` | no |
| `app.currentElement.move` | ui | Previous/next element (Timeline order, exposure-sheet order, or by mode) | `direction: enum "previous"\|"next"; order: enum "auto"\|"timeline"\|"xsheet" = "auto"` | `{elementId}` | no |
| `app.context.set` | ui | Set several parts of the current context at once | `sceneId?: id; frame?: int; elementId?: id; drawingId?: id` | `AppState` | no |
| `app.state.changed` | event | Current context changed (project, scene, frame, element, mode, view, tool, workspace, modal state) | `{changed: [string], state: AppState}` | — | n/a |
| `project.create` | action | Create a new project and make it current; if `path` is given it is saved there immediately | `name: string; path?: path; preset?: string; frameRate?: number (1–120); cameraWidth?: int (16–8192); cameraHeight?: int (16–8192); onUnsaved: enum "fail"\|"save"\|"discard" = "fail"; onExists: enum "fail"\|"overwrite" = "fail"` | `{projectId, path?, sceneId, cameraId, elementId}` | n/a |
| `project.open` | job | Open a project package | `path: path; readOnly: bool = false; onUnsaved: enum "fail"\|"save"\|"discard" = "fail"` | `{jobId}` → `{projectId, readOnly, formatVersion, warnings[]}` | n/a |
| `project.close` | action | Close the current project | `onUnsaved: enum "fail"\|"save"\|"discard" = "fail"` | `{closed: bool}` | n/a |
| `project.save` | job | Save the project (and the global library if modified) to its location | `includeGlobalLibrary: bool = true; allowFormatUpgrade: bool = false` | `{jobId}` → `{path, projectSaved, globalLibrarySaved, backupId?}` | no |
| `project.saveAs` | job | Save the project to a new location; the copy becomes current | `path: path; onExists: enum "fail"\|"overwrite" = "fail"; includeGlobalLibrary: bool = true; allowFormatUpgrade: bool = false` | `{jobId}` → `{path, projectSaved, globalLibrarySaved}` | no |
| `project.info.get` | query | Project summary | — | `ProjectInfo` (§12.4) | n/a |
| `project.properties.get` | query | Frame rate and camera size | — | `{frameRate, cameraWidth, cameraHeight}` | n/a |
| `project.properties.set` | action | Change frame rate / camera size ("Change Project Properties"); optionally store as defaults for new projects | `frameRate?: number (1–120); cameraWidth?: int (16–8192); cameraHeight?: int (16–8192); setAsDefault: bool = false` | `{frameRate, cameraWidth, cameraHeight}` | yes |
| `project.presets.list` | query | Format presets for new projects | — | `[{id, name, kind: "mostRecent"\|"default"\|"bundled", frameRate, width, height}]` | n/a |
| `project.recent.list` | query | Recent-projects list | — | `[{path, name, lastOpened, exists}]` | n/a |
| `project.recent.remove` | ui | Remove one entry or clear the recent list | `path?: path; all: bool = false` | `{remaining: int}` | no |
| `project.recovery.list` | query | Autosave recovery data available | — | `[{recoveryId, projectName, originalPath?, savedAt}]` | n/a |
| `project.recovery.open` | job | Open recovery data as a modified project targeting its original path | `recoveryId: id; onUnsaved: enum "fail"\|"save"\|"discard" = "fail"` | `{jobId}` → `{projectId, warnings[]}` | n/a |
| `project.recovery.delete` | ui | Discard recovery data | `recoveryId?: id; all: bool = false` | `{deleted: int}` | no |
| `project.backup.list` | query | Backups of a project | `path?: path (default: current project)` | `[{backupId, savedAt, sizeBytes}]` | n/a |
| `project.backup.open` | action | Open a backup as an untitled copy | `backupId: id; onUnsaved: enum "fail"\|"save"\|"discard" = "fail"` | `{projectId}` | n/a |
| `project.opened` | event | A project was opened or created | `{projectId, path?, readOnly}` | — | n/a |
| `project.closed` | event | The project was closed | `{projectId}` | — | n/a |
| `project.saved` | event | The project and/or global library was saved | `{projectId, path, projectSaved, globalLibrarySaved}` | — | n/a |
| `project.dirty.changed` | event | Modified state changed | `{projectDirty, globalLibraryDirty}` | — | n/a |
| `project.autosaved` | event | A recovery copy was written | `{projectId, recoveryId, savedAt}` | — | n/a |
| `edit.undo` | action | Undo one or more steps; with `ifTop`, only if that step is still the most recent (else `undo.notTop`) | `steps: int = 1 (1–1000); ifTop?: string` | `{undone: [label], position}` | n/a |
| `edit.redo` | action | Redo one or more steps | `steps: int = 1 (1–1000)` | `{redone: [label], position}` | n/a |
| `edit.history.get` | query | Undo history with attribution | `offset: int = 0; limit: int = 100 (1–1000)` | `{entries: [{index, label, source: "user"\|"agent", agentSessionId?, agentName?, time}], position, cleanPosition}` | n/a |
| `edit.history.move` | action | Undo/redo to just after a given history entry (History panel click) | `position: int (0–entry count)` | `{position}` | n/a |
| `edit.cut` | action | Cut the selection (or the given objects) of an edit context to the clipboard ("Cut") | `context: enum "auto"\|"drawing"\|"staging"\|"xsheet"\|"timeline"\|"library"\|"functionEditor" = "auto"; objectIds?: [id]` | `{clipboard: {kind, itemCount}}` | yes |
| `edit.copy` | ui | Copy the selection (or the given objects) to the clipboard | `context: enum (as edit.cut) = "auto"; objectIds?: [id]` | `{clipboard: {kind, itemCount}}` | no |
| `edit.paste` | action | Paste the clipboard into an edit context ("Paste") | `context: enum (as edit.cut) = "auto"; destination?: object ({drawingId} \| {elementId, frame} \| {catalogId} \| {functionId, frame} \| {objectIds})` | `{createdIds: [id], modifiedIds: [id], warnings[]}` | yes |
| `edit.pasteAsNewElement` | action | Create a new element from copied cells, exposed from a frame ("Paste as New Element") | `sceneId?: id; frame?: int` | `{elementId}` | yes |
| `edit.delete` | action | Delete the selection or the given objects; clears cells in cell contexts ("Delete") | `context: enum (as edit.cut) = "auto"; objectIds?: [id]` | `{deletedIds: [id]}` | yes |
| `edit.cutSpecial` | action | Cut cells with timeline edit options ("Cut Special") | `objectIds?: [id]; options?: TimelineEditOptions (§12.4); remember: bool = true` | `{clipboard: {kind, itemCount}}` | yes |
| `edit.pasteSpecial` | action | Paste with timeline or motion-point options; `useLast` = paste special again ("Paste Special") | `destination?: object; options?: TimelineEditOptions \| MotionPointPasteOptions; useLast: bool = false; remember: bool = true` | `{createdIds: [id], modifiedIds: [id], warnings[]}` | yes |
| `edit.clipboard.get` | query | Clipboard content as structured data | `includeData: bool = false` | `Clipboard` (§12.4) | n/a |
| `edit.clipboard.set` | ui | Put structured content on the clipboard (validated against the schema of its kind) | `content: Clipboard` | `{kind, itemCount}` | no |
| `edit.selection.get` | query | Selection of an edit context | `context: enum (as edit.cut) = "auto"` | `{context, ids: [id], kinds: [string]}` | n/a |
| `edit.selection.set` | ui | Change the selection of an edit context | `context: enum (as edit.cut) = "auto"; ids: [id]; mode: enum "replace"\|"add"\|"remove"\|"toggle" = "replace"` | `{context, ids: [id]}` | no |
| `edit.selectAll` | ui | Select all in an edit context | `context: enum (as edit.cut) = "auto"` | `{context, ids: [id]}` | no |
| `edit.deselectAll` | ui | Clear the selection of an edit context | `context: enum (as edit.cut) = "auto"` | `{context}` | no |
| `edit.history.changed` | event | Undo history changed | `{position, entryCount, lastLabel, source}` | — | n/a |
| `edit.selection.changed` | event | A selection changed | `{context, ids: [id]}` | — | n/a |
| `edit.clipboard.changed` | event | Clipboard content changed | `{kind, itemCount}` | — | n/a |
| `view.list` | query | View windows with type, focus, size and transform | — | `[{viewId, panelId, type, focused, size: {width, height}, transform}]` | n/a |
| `view.active.set` | ui | Make a view window active (focus it) | `viewId: id` | `{viewId}` | no |
| `view.type.set` | ui | Switch a view window's type; `"previous"` = switch active view | `viewId?: id; type: enum "drawing"\|"camera"\|"top"\|"side"\|"previous"` | `{viewId, type}` | no |
| `view.transform.get` | query | Zoom, rotation and centre of a view | `viewId?: id` | `{zoom, rotationDegrees, centre: point}` | n/a |
| `view.zoom.set` | ui | Set zoom absolutely or by table steps (zoom in = +1, zoom out = −1) | `viewId?: id; zoom?: number (0.0625–64); steps?: int; anchor?: point (view pixels)` | `{zoom, rotationDegrees, centre}` | no |
| `view.rotation.set` | ui | Set view rotation absolutely or in 30° steps | `viewId?: id; degrees?: number (−360–360); steps?: int` | `{zoom, rotationDegrees, centre}` | no |
| `view.pan.set` | ui | Pan: centre on a document point, by a pixel offset, or on the pointer (centre on pointer) | `viewId?: id; centre?: point; by?: point (view pixels); atPointer: bool = false` | `{zoom, rotationDegrees, centre}` | no |
| `view.transform.reset` | ui | Reset view / reset zoom / reset rotation / recentre | `viewId?: id; parts: [enum "zoom"\|"rotation"\|"pan"] = ["zoom","rotation","pan"]` | `{zoom, rotationDegrees, centre}` | no |
| `view.coordinates.convert` | query | Convert a point between view pixels, drawing space and scene space for a view | `viewId: id; point: point; from: enum "view"\|"drawing"\|"scene"; to: enum "view"\|"drawing"\|"scene"` | `{point}` | n/a |
| `window.panel.list` | query | Panels with visibility, dock area, tab group, floating geometry and focus | — | `[{panelId, title, visible, floating, area?, tabGroup?, current, geometry?, focused}]` | n/a |
| `window.panel.set` | toggle | Show/hide, float/dock, focus a panel; `"active"` addresses the focused panel (close active panel = `visible: false`) | `panelId: string; visible?: bool; floating?: bool; focus: bool = true; geometry?: {x, y, width, height}` | panel entry | no |
| `window.panel.move` | ui | Dock a panel into an area, next to or tabbed with another panel | `panelId: string; area: enum "left"\|"right"\|"top"\|"bottom"\|"central"; relativeTo?: string; position: enum "tab"\|"before"\|"after"\|"above"\|"below" = "tab"; size?: int` | `Layout` | no |
| `window.floatingPanels.set` | toggle | Hide/show all floating panels and tool palettes | `visible: bool` | `{visible}` | no |
| `window.sidePanelsExtended.set` | toggle | Side dock areas own the corners (true) or top/bottom areas do (false) | `enabled: bool` | `{enabled}` | no |
| `window.fullScreen.set` | toggle | Full-screen presentation | `enabled: bool` | `{enabled}` | no |
| `window.minimized.set` | toggle | Minimise or restore the main window | `minimized: bool` | `{minimized}` | no |
| `window.raiseAll.run` | ui | Bring all application windows to the front | — | `{}` | no |
| `window.geometry.get` | query | Main window geometry and screens | — | `{x, y, width, height, maximized, fullScreen, screens: [{x, y, width, height, scale}]}` | n/a |
| `window.geometry.set` | ui | Move/resize/maximise the main window | `x?: int; y?: int; width?: int; height?: int; maximized?: bool` | window geometry | no |
| `window.toolbar.list` | query | Toolbars and tool palettes with visibility, items and allowed items | — | `[{toolbarId, visible, floating, position?, items: [string], allowedItems: [string]}]` | n/a |
| `window.toolbar.set` | toggle | Show/hide (and float/position) the main toolbar or a tool palette | `toolbarId: enum "main"\|"drawingTools"\|"animationTools"\|"boneTools"\|"effectTools"\|"soundTools"; visible: bool; floating?: bool; position?: point` | toolbar entry | no |
| `window.toolbar.items.set` | ui | Customise the main toolbar (stored in the current workspace) | `toolbarId: enum "main" = "main"; items: [string] (command references, group IDs, "separator", "space", "flexibleSpace")` | `{items: [string]}` | no |
| `window.toolbar.items.reset` | ui | Restore the default main toolbar items | `toolbarId: enum "main" = "main"` | `{items: [string]}` | no |
| `workspace.list` | query | Workspaces | — | `[{name, builtIn, modified, position, current}]` | n/a |
| `workspace.get` | query | Full layout of a workspace (default: the current arrangement) | `name?: string` | `Layout` (§12.4) | n/a |
| `workspace.select` | ui | Switch workspace | `name: string; saveCurrent?: bool (default: general.workspace.autoSave)` | `{name}` | no |
| `workspace.create` | ui | Create a workspace from the current arrangement or a given layout | `name: string; position?: int (1–n+1); layout?: Layout` | `{name, position}` | no |
| `workspace.save` | ui | Store the current arrangement into a workspace | `name?: string (default: current)` | `{name}` | no |
| `workspace.rename` | ui | Rename a workspace | `name: string; newName: string` | `{name}` | no |
| `workspace.delete` | ui | Delete a user workspace | `name: string` | `{deleted: string}` | no |
| `workspace.reset` | ui | Restore built-in workspace(s) to the bundled arrangement; `select` also applies it (restore default workspace = `name: "Default", select: true`) | `name?: string; all: bool = false; select: bool = false` | `{reset: [string]}` | no |
| `workspace.layout.set` | ui | Apply a layout to the window without saving it to a workspace | `layout: Layout` | `Layout` | no |
| `workspace.export` | ui | Write a workspace to a file | `name: string; path: path; onExists: enum "fail"\|"overwrite" = "fail"` | `{path}` | no |
| `workspace.import` | ui | Read a workspace from a file | `path: path; onExists: enum "fail"\|"overwrite"\|"rename" = "fail"` | `{name}` | no |
| `workspace.changed` | event | Current workspace switched or modified | `{name, modified}` | — | n/a |
| `prefs.list` | query | Preference schema and values | `prefix?: string` | `[{key, type, default, value, min?, max?, values?, applies: "live"\|"nextLaunch"\|"restart", description}]` | n/a |
| `prefs.get` | query | Values of preferences by key | `keys: [string]` | `{values: object}` | n/a |
| `prefs.set` | ui | Set one or more preferences atomically | `values: object (key → value)` | `{applied: [string], restartRequired: [string]}` | no |
| `prefs.reset` | ui | Reset preferences to defaults by key or prefix (`prefix: ""` = all) | `keys?: [string]; prefix?: string` | `{reset: [string]}` | no |
| `prefs.changed` | event | Preferences changed | `{keys: [string], source: "user"\|"agent"}` | — | n/a |
| `keymap.list` | query | Bindings, filterable | `context?: enum "global"\|"drawing"\|"staging"; commandId?: string; prefix?: string; onlyModified: bool = false` | `[Binding]` (§12.4) | n/a |
| `keymap.get` | query | Bindings of a command, or the command a key sequence runs in a context | `commandId?: string; keys?: string; context: enum "global"\|"drawing"\|"staging" = "global"` | `{bindings: [Binding], effectiveCommand?: {commandId, params?}}` | n/a |
| `keymap.set` | ui | Bind a key sequence to a command (with optional fixed parameters) | `commandId: string; params?: object; keys: string; context: enum "global"\|"drawing"\|"staging" = "global"; onConflict: enum "fail"\|"override" = "fail"; replaceExisting: bool = false` | `{binding: Binding, removed: [Binding]}` | no |
| `keymap.remove` | ui | Remove bindings of a command | `commandId: string; keys?: string; context?: enum "global"\|"drawing"\|"staging"` | `{removed: [Binding]}` | no |
| `keymap.reset` | ui | Restore default bindings for one command or all | `commandId?: string; all: bool = false` | `{reset: int}` | no |
| `keymap.export` | ui | Write the keymap to a file | `path: path; onExists: enum "fail"\|"overwrite" = "fail"` | `{path}` | no |
| `keymap.import` | ui | Read a keymap file | `path: path; mode: enum "merge"\|"replace" = "merge"; onConflict: enum "fail"\|"override" = "fail"` | `{imported: int, conflicts: [Binding]}` | no |
| `keymap.changed` | event | Bindings changed | `{commandIds: [string]}` | — | n/a |
| `ui.tips.list` | query | Suppressible messages and their state | — | `[{messageId, description, suppressed}]` | n/a |
| `ui.tips.set` | ui | Suppress or re-enable one message | `messageId: string; suppressed: bool` | `{messageId, suppressed}` | no |
| `ui.tips.reset` | ui | Show all tips again | — | `{reenabled: int}` | no |
| `ui.statusBar.set` | toggle | Show/hide the status bar (same as `interface.statusBar.visible`) | `visible: bool` | `{visible}` | no |
| `ui.statusBar.get` | query | Status bar segment texts | — | `{visible, description, context, pointer, playback, agentIndicator}` | n/a |
| `ui.theme.list` | query | Available themes (selected via `interface.theme`) | — | `[{themeId, name, builtIn, dark}]` | n/a |
| `ui.startScreen.set` | toggle | Show/hide the start screen | `visible: bool` | `{visible}` | no |
| `ui.contextHelpMode.set` | toggle | Enter/leave context-help mode | `enabled: bool` | `{enabled}` | no |
| `ui.help.get` | query | Help text of a command, panel, preference or message | `commandId?: string; panelId?: string; preferenceKey?: string; messageId?: string` | `{title, text (Markdown)}` | n/a |
| `ui.help.open` | ui | Open the documentation, optionally at a topic | `topic?: string` | `{}` | no |
| `ui.preferences.open` | ui | Open the preferences window at a category (GUI convenience) | `category?: string` | `{}` | no |
| `ui.message.shown` | event | A message was shown or returned as a warning | `{messageId, severity: "info"\|"warning"\|"error"\|"question", text, details?, source: "user"\|"agent", suppressed}` | — | n/a |

Saving only the global (and other opened) libraries is `library.save` (08). Tool selection (including the zoom, pan and
rotate-view navigation tools) is `tool.*` (02). Making a scene current is also available as a scene command in 08;
`app.context.set {sceneId}` is equivalent. Agent-access settings and session control are `agent.*` (11).

### 12.3 Queries and events

* `app.state.get` is the primary orientation query; `app.state.changed` is emitted (coalesced, at most once per
  command or per 100 ms of GUI interaction) whenever any of its fields changes.
* `project.info.get` returns the project summary including scenes; full object content is read with the generic
  `get_object` / `query` tools (10, 11).
* `workspace.get` returns the complete layout in the `Layout` shape; `workspace.layout.set` and `workspace.create`
  accept the same shape, so an agent can read, modify and re-apply a layout.
* `edit.history.get` exposes the same entries as the History panel, including attribution.
* Events: `app.state.changed`, `project.opened`, `project.closed`, `project.saved`, `project.dirty.changed`,
  `project.autosaved`, `edit.history.changed`, `edit.selection.changed`, `edit.clipboard.changed`,
  `workspace.changed`, `prefs.changed`, `keymap.changed`, `ui.message.shown`, all delivered through `app://events`
  (11).

### 12.4 Result shapes

**AppState**

```json
{
  "project": {"id": "p1", "name": "Walk cycle", "path": "/Users/me/Walk cycle.animpkg",
              "dirty": true, "globalLibraryDirty": false, "readOnly": false},
  "scene": {"id": "s1", "name": "Scene 1", "frameCount": 120},
  "frame": 12,
  "element": {"id": "e7", "name": "Bob", "type": "drawing"},
  "drawing": {"id": "d3", "name": "3"},
  "mode": "drawing",
  "activeView": {"viewId": "v1", "panelId": "drawingView", "type": "drawing"},
  "focusedPanel": "drawingView",
  "activeTool": {"toolId": "brush"},
  "selection": {"context": "drawing", "ids": ["k12", "k13"], "kinds": ["stroke"]},
  "workspace": {"name": "Drawing", "modified": false},
  "playback": {"playing": false},
  "startScreenVisible": false,
  "modal": null,
  "agentSessions": [{"sessionId": "a1", "name": "Assistant", "acting": true}]
}
```

Fields are `null` when not applicable (for example `project: null` with no project open, `mode: null` when no view is
active). `modal` is `{kind: "dialog"|"gesture"|"progress", title}` while the GUI is in a modal state.

**ProjectInfo**

```json
{
  "id": "p1", "name": "Walk cycle", "path": "/Users/me/Walk cycle.animpkg", "readOnly": false,
  "formatVersion": 1, "dirty": true, "globalLibraryDirty": false, "lastSaved": "2026-10-17T10:22:00Z",
  "frameRate": 24, "cameraWidth": 1920, "cameraHeight": 1080,
  "scenes": [{"id": "s1", "name": "Scene 1", "frameCount": 120}],
  "currentSceneId": "s1"
}
```

**Clipboard**

```json
{
  "kind": "drawingObjects",
  "sourceContext": "drawing",
  "sourceProjectId": "p1",
  "itemCount": 2,
  "items": [ { "...": "objects in the section-10 schema for this kind" } ],
  "options": null
}
```

`kind` ∈ `drawingObjects`, `elements`, `elementProperties`, `cells`, `keyframes`, `motionPoint`, `templates`,
`functionPoints`, `image` (from the system clipboard). `items` is present only when `includeData` is true. Cells
carry `{elementId, frameOffset, drawingRef, drawingContent?}`; drawing objects carry their full vector content in the
same shape as the drawing read-back of section 02. `options` holds the cut-special options when the content was cut
with them.

**TimelineEditOptions** — `{overwriteDrawings, createDeleteExposure, extendExposures, updateColours,
keyframeSubstitution, motionKeys, rotationKeys, scaleKeys, pivotKeys, velocityKeys, skewKeys, boundaryKeys}`, all
`bool`, defaults as in §6.3. Omitted fields take the remembered values.

**MotionPointPasteOptions** — `{x, y, z, tension, continuity, bias, angle, rotationPivot, scaleX, scaleY,
scalePivot}`, all `bool`, default true; options that do not apply to the clipboard content are ignored and listed in
the result's `warnings`.

**Layout**

```json
{
  "centralPanel": "cameraView",
  "corners": "sides",
  "docks": {
    "right":  [{"tabs": ["colour", "properties", "library"], "current": "colour", "size": 330},
               {"tabs": ["pen", "text", "cells"], "current": "pen"}],
    "bottom": [{"tabs": ["timeline", "xsheet", "functionEditor", "storyline"], "current": "timeline", "size": 190}],
    "left": [], "top": []
  },
  "floating": [{"panelId": "drawingView", "visible": false,
                "geometry": {"x": 80, "y": 80, "width": 800, "height": 600}}],
  "toolPalettes": [{"id": "drawingTools", "visible": true, "floating": true, "position": {"x": 10, "y": 120}}],
  "mainToolbar": {"visible": true, "items": ["edit.undo", "edit.redo", "separator", "workspace.select"]},
  "fullScreen": {"...": "same shape without fullScreen"}
}
```

`corners` ∈ `sides` | `topBottom`. Sizes and geometry are logical pixels. Each dock area is an ordered stack of tab
groups; `size` on the first group is the area's width (left/right) or height (top/bottom). Panels not mentioned are
hidden.

**Binding**

```json
{"commandId": "view.zoom.set", "params": {"steps": 1}, "keys": "X", "context": "global", "isDefault": true}
```

---

## 13. Design decisions / open questions

1. **Primary+W.** Platform convention binds Primary+W to closing the document; some users want it to close the focused
   floating panel. **Recommended:** Primary+W = close project; *close active panel* has no default binding (floating
   panels have their own close buttons).
2. **Default new-project format** on a fresh install. **Recommended:** 1920 × 1080 at 24 fps.
3. **Fractional frame rates.** **Recommended:** frame rate is a number; 23.976, 29.97 and 59.94 are accepted and
   stored exactly as rationals (24000/1001 etc.) in the project package (10).
4. **Libraries on Save.** **Recommended:** Save writes modified writable libraries together with the project without
   asking (as 08 specifies) and reports per-library results; the agent parameter `includeGlobalLibrary` controls it
   explicitly. Close/quit with modified libraries goes through the unsaved-changes rule (§3.5).
5. **Agent commands during GUI modal states.** **Decided with section 11:** conflicting agent commands never act on a half-finished user
   interaction; they wait up to `waitForUserMs` (default 10 s, 0 = fail fast) and then fail with `ui.busy`.
6. **Autosave and backup locations.** **Recommended:** per-user application-data folders keyed by project identity
   (not next to the project, to avoid clutter in shared folders); a later preference MAY allow "next to project".
7. **Mouse wheel.** **Recommended:** the wheel zooms about the pointer in view windows and scrolls in list-like
   panels; a preference to make the wheel scroll in views MAY be added.
8. **Preferences apply model.** **Recommended:** OK/Cancel as specified (Cancel reverts all edits including keymap);
   live preview of theme and colours while the window is open is allowed.
9. **Shortcut contexts.** Only `global`, `drawing` and `staging` are required. Panel-specific contexts (Timeline vs
   exposure sheet vs library) MAY be added later.
10. **Default element and storyline colours** are placeholders; final values are a UX decision but MUST remain
    distinguishable from each other in both light and dark themes.
11. **Command naming exceptions.** The generic clipboard and selection commands use the conventional verbs `cut`,
    `copy`, `paste`, `selectAll`, `deselectAll`, and keymap/prefs/view/workspace use `reset`; these are accepted
    additions to the verb list.
12. **History jumps.** **Recommended:** `edit.history.move` and History-panel clicks perform one combined jump without
    rendering intermediate states.
13. **Multiple open projects.** Not required; one project per instance. Revisit if the data model (10) allows it.
14. **Zoom table.** The table in §6.5 is a recommendation; the UX team may adjust the steps, but the limits
    (6.25 %–6400 %) and the rotation step (30°) are requirements.
15. **Deselect all in cell contexts** clears cell selections too (§6.4); this is deliberate.
16. **Sample project and tutorial content** are authored fresh, with the coverage listed in §2.
