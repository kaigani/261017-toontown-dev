# 08 — Library and Storyline

This section specifies two related areas:

- **Library** (§1–§12): libraries of reusable content (templates and imported media files) that live outside a single
  scene — a machine-wide global library, the project-local library, and any other libraries the user opens; the Library
  panel (tree, item list, preview); folder and template management; creating templates from project content; inserting
  templates into a scene by copy or by link; editing templates in place; template properties; import/export; saving;
  the bundled starter content.
- **Storyline** (§13–§18): the project as an ordered list of scenes; scene management (add, delete, reorder, rename,
  show/hide, current scene, scene properties); the Storyline panel; transitions between scenes and their parameters;
  the global timing model; playing the storyline; how previews and exports treat transitions.

§19 defines the agent interface (namespaces `library` and `storyline`). §20 lists design decisions.

Related sections:
- `01_application_shell.md` — panels and docking (the Library and Storyline panels are opened/closed with the generic
  `window` commands), preferences (`prefs`), project open/save, undo history, element colour preferences.
- `04_timeline_xsheet.md` — elements, exposure, Timeline and Exposure Sheet, media (linked-template) elements, drawing
  rename/delete, and the cut/drag/drop option set referenced in §7.4.
- `05_scene_planning.md` — cameras, pegs, keyframes, scene/field coordinate space.
- `07_sound_lipsync.md` — lip-sync, which picks mouth-shape templates from the library (§10.2).
- `09_playback_export.md` — interactive playback, rendered previews, export ranges.
- `10_data_model_project_format.md` — the project package (`.animpkg`), the library index and the template file
  (`.animtpl`) formats.
- `11_agent_interface_mcp.md` and `A_command_registry.md` — the command layer and the compiled command registry.

---

# Part A — Library

## 1. Concepts

| Concept | Definition |
|---|---|
| **Library** | A named, self-contained collection of reusable content stored as one folder on disk (format in section 10). It contains a hierarchy of folders whose leaves are items. Several libraries can be open at the same time. |
| **Global library** | A library that is independent of any project and shared by every project on the machine. Its location is a preference (§12). More than one global library MAY be registered; all registered global libraries are loaded at start-up. |
| **Project-local library** | The library stored inside the project package. Every project has exactly one; it is created empty with the project, cannot be closed while the project is open, and is saved together with the project. Its root is displayed with a fixed name identifying it as the project's library. |
| **Opened (external) library** | Any other library the user opens explicitly (§5.1). It behaves like a global library (independent of the project) and is remembered across sessions until closed. |
| **Starter library** | The bundled library of starter content (§3). It is loaded as a global library on first run and can be restored at any time. |
| **Folder** | A named node inside a library. Folders nest to any depth and hold items and sub-folders. |
| **Template** | A reusable piece of animation stored in a library as one template file (`.animtpl`, defined in section 10): one or more elements with their drawings, exposures, peg motion and functions, bones, effects, sound and colour information, plus metadata (§6.3) and a thumbnail. |
| **Media file item** | An image, movie or sound file imported into a library folder as-is (not converted to a template). Item types: *image*, *movie*, *sound*. |
| **Project branch** | A read-mostly branch of the Library tree that mirrors the open project: one node per scene and, under each scene, one node per element. It lets the user browse an element's drawings, copy elements between scenes, and create templates from scenes or elements. |

### 1.1 Node types

The Library tree and item list MUST distinguish, by icon and in queries, the following node types: global library root,
project-local library root, opened library root, read-only (write-locked) variant of a library root, folder (with a
distinct look when empty), project root, scene, element (sub-typed as drawing, image, peg, camera, sound, effect, media),
template, image file, movie file, sound file, and a broken/unreadable item (missing or corrupt file).

## 2. Library storage, identity and naming

- **LIB-001** Each library is stored as a folder containing an index of its hierarchy and the content files it lists
  (template files, thumbnails, imported media). The exact layout is defined in section 10. Content files MAY be stored
  flat; the folder hierarchy is a property of the index only.
- **LIB-002** Every library, folder and item has a **stable, globally unique ID** that survives renames, moves and
  save/reload. Linked usages of templates in projects (§7.2) refer to the template by library ID + template ID.
- **LIB-003** Display names are independent of stored file names. Two items in different folders MAY share a display name.
  Within one folder, display names of siblings MUST be unique (case-insensitive).
- **LIB-004** Names (folders, items) MUST NOT be empty and MUST NOT contain the characters `\ / : * ? " < > |`. Violations
  are rejected with `library.nameEmpty` / `library.nameInvalid`; a duplicate sibling name with `library.nameInUse`.
- **LIB-005** When an imported file's name collides with an existing stored file, the stored copy is given a unique name
  by appending a numeric suffix (e.g. `name (2)`); the user is informed which name was used. Display names follow the
  same suffix rule.
- **LIB-006** Each library is opened with **exclusive write access**. If another application instance or user already
  holds write access, the library opens **read-only**, shows the read-only indicator, and every modifying command fails
  with `library.readOnly`. The user MAY explicitly release write access on a library; if it has unsaved changes, the
  user must confirm that the changes are discarded and the library is reloaded from disk. The user MAY re-request write
  access on a read-only library (succeeds if no one else holds it).
- **LIB-007** Templates carry a **version ID** that changes whenever the template content changes (template edit mode,
  §8). Linked usages compare version IDs to know when to refresh.

## 3. Starter content

The application MUST bundle a starter library authored fresh for the application (no third-party or legacy content).
It is installed read-only with the application and loaded as a global library on first run; a flag in the user's settings
records that this happened so a user who closes it is not forced to see it again. A command restores it (re-opens it
and, if its installed files are damaged, reinstalls them from the application). The user can also copy items out of it into
a writable library.

Required categories and minimum coverage (folder names are the implementer's choice):

| Category | Minimum content |
|---|---|
| Backgrounds | ≥ 7 full-frame backgrounds covering interior rooms, exterior buildings, a city, a beach/water scene. |
| Storyboard symbols | ≥ 20 single-drawing arrows in varied shapes/directions and camera-move indicators (push in, pull out, pan, combinations). |
| Props | ≥ 40 props: furniture, vehicles, plants/trees, weather drawings (rain, clouds, lightning), smoke/explosion/impact drawings, beams/zaps. |
| Character pose sets | ≥ 3 characters, each with ≥ 8 poses/views: front, three-quarter front, side, three-quarter back, back, walk, run, sit. |
| Cut-out puppets | ≥ 2 rigged cut-out characters built from drawings on a peg hierarchy with pivots set. |
| Clip art | ≥ 40 single-drawing clip-art items (animals, vehicles, stick figures, everyday objects). |
| Seasonal sets | ≥ 2 seasonal themes, each with backgrounds/overlays, animated character cycles and props (≥ 20 items per theme). |
| Bone/deformation demos | ≥ 15 templates showing skeleton/bone deformation (swimming creature, swaying plants, growing plant, swinging object, etc.). |
| Skeleton motion presets | Human and animal skeleton-driven motions in three views (three-quarter, side, front): walk, run, jump, sit/squat, stretch, wave, head tilt, ≥ 3 dance moves; ≥ 35 in total. |
| Mouth-shape sets | A separate starter library of ≥ 10 mouth-shape sets in distinct styles (human, cartoon, animal, stylised), each covering the full mouth-shape set defined in section 07. Used by lip-sync (§10.2). |

Every starter template MUST have a thumbnail and the metadata of §6.3. The sample project offered by the welcome screen
(section 01) SHOULD contain a project-local library with a few templates. Learning content (section 01) SHOULD include one
tutorial on templates/the library and one on the storyline and transitions.

## 4. Library panel

### 4.1 Panes

The Library panel (opened through section 01's window commands) has three linked panes:

1. **Preview** (§4.4) — plays or shows the selected item.
2. **Tree** — the hierarchy of libraries, folders and the project branch.
3. **Item list** — the contents of the node selected in the tree.

Linkage: selecting a tree node fills the item list; selecting an item loads it in the preview; double-clicking a folder in
the item list opens it and synchronises the tree selection. The panel title shows the current location. Pane split
positions are part of the workspace (section 01).

### 4.2 Tree contents at start-up

In order: the registered global libraries (including the starter library on first run), the project-local library of the
open project, the project branch of the open project, and the opened external libraries remembered from the previous
session. If a remembered or configured library path no longer exists, the application reports `library.pathMissing`
(distinguishing global and local) and, for the global path, tells the user that the path can be changed in preferences
with immediate effect. Missing libraries are skipped; start-up continues.

### 4.3 Item list display

Three mutually exclusive display modes, persisted across sessions:

| Mode | Display |
|---|---|
| **Thumbnails** | Large thumbnails with the name underneath. Template thumbnails come from the stored thumbnail; missing thumbnails are rendered on demand with progress feedback and then stored. Images are scaled to fit; movie and sound items show generic type icons. Thumbnail size follows the pane width. |
| **List** | Names with small type icons. |
| **Details** | Table with columns Name, Type (template / image / movie / sound / folder), Size, Last modified (locale date format), Artist, Copyright. Columns are sortable. |

Other item-list behaviour:
- **Double-click**: folder → open it; template → enter template edit mode (§8); sound item → play it; image/movie → preview.
- **Sound preview**: selecting a sound item, or a template that contains sound, plays its sound in the preview when the
  preview is playing; selecting another item stops it.
- When the tree selection is an element of the project branch, the item list shows that element's drawings (§7.6).

### 4.4 Preview pane

Contains the image area, a play/stop toggle, a frame slider and a preview enable toggle (disabling the live preview saves
processing). The preview resizes with its pane.

| Item type | Preview behaviour |
|---|---|
| Template | Renders the template's elements composited at the slider frame. Play steps through all template frames at the project frame rate and loops until stopped. |
| Image file | Shows the image; slider disabled. |
| Movie file | Plays the movie frames. |
| Sound file | Play/stop plays the sound; no picture. |
| Drawing (project branch) | Shows the drawing. |
| Folder | Shows the folder thumbnail (if set) or nothing. |

## 5. Library management operations

All operations below that modify a library are undoable (one step each) and mark the library as modified; changes reach
disk when the library is saved (§11). Each operation is available from the tree and/or item-list context menus and the
main menu, and through commands (§19).

### 5.1 Libraries

| Operation | Behaviour |
|---|---|
| **Create library** | Creates a new empty library (index + root folder) in a chosen folder and adds it to the tree as an opened library. Fails with `library.alreadyExists` if the folder already contains a library. |
| **Open library** | Adds an existing library to the tree. Fails with `library.alreadyOpen` if it is already open, `library.pathMissing` / `library.corrupt` / `library.openFailed` if it cannot be read. |
| **Close library** | Removes a library (not the project-local one) from the tree; files on disk are kept. If it has unsaved changes the user must confirm that the changes are discarded. |
| **Properties** | Shows the library name and its folder path for a selected library or folder. |
| **Restore starter library** | Re-opens the starter library and reinstalls its files if they are damaged (§3). |
| **Refresh** | Re-reads the selected library or folder from disk and regenerates missing thumbnails. Standard refresh shortcut. |
| **Release / request write access** | See LIB-006. |

### 5.2 Folders

| Operation | Behaviour |
|---|---|
| **New folder** | Creates a child folder of the selected library root or folder with a default generic name, made unique with a numeric suffix. The new folder is selected and enters in-place rename. Fails with `library.createFailed`. |
| **Rename folder** | In-place rename subject to LIB-003/LIB-004. |
| **Delete folder** | Removes the folder and everything under it after confirmation. Content files are queued for deletion and removed from disk when the library is saved (so undo can restore them before saving). |
| **Move / copy** | Dragging a folder or item onto another folder moves it; with the copy modifier held it copies. Copy/paste through the generic clipboard (section 01) also works on the Library selection. Pasting onto an invalid target fails with `library.pasteNotAllowed`; failed moves with `library.moveFailed`. Moving between libraries copies the content files into the target library. |

### 5.3 Items

| Operation | Behaviour |
|---|---|
| **Import image / movie / sound files** | Copies the chosen files into the library and adds them as media file items in the current folder. Multiple files at once. Supported formats are those of section 09's import. Copy-protected movie files are refused (`library.protectedMedia`); unsupported files `library.unsupportedFormat`. |
| **Import template file** | Copies a `.animtpl` file (and its thumbnail if present) into the library as a template item. Failures: `library.importFailed`, `library.corrupt`, `library.unsupportedFormat`. |
| **Drag files in** | Dropping files from the operating system onto a folder or the item list imports them: images, movies and sounds become media file items, `.animtpl` files become templates. Dropping items from another folder adds them (copy). Dropping a Timeline/Exposure Sheet/Drawing selection creates a template (§6). Failure: `library.addFailed`. |
| **Rename item** | In-place rename subject to LIB-003/LIB-004. Changes the display name; the stored file MAY be renamed too. |
| **Delete item** | Deletes the selected items (with the platform delete key). |
| **Edit template** | Enters template edit mode (§8). |
| **Return to scene** | Leaves template edit mode (§8); the command label names the scene being returned to. |
| **Properties** | Shows template/item properties (§9). |
| **Export template** | Saves a copy of a template as a standalone `.animtpl` file (with its thumbnail) at a chosen path (§10.1). |
| **Copy drawing to current frame** | See §7.6. |

### 5.4 Template count

There is no limit on the number of templates per folder or library.

## 6. Creating templates

### 6.1 Sources

The user creates a template by dragging project content onto a folder in the tree, or onto the item list while it shows a
folder, or by an explicit "create template from selection" command that asks for the destination folder.

| Source | Selection | Resulting template |
|---|---|---|
| Timeline | One or more elements (any type, including pegs with children, effects, skeletons, cameras, sound, media), optionally limited to a frame range | Those elements with their exposures, keyframes and functions over the selected frames. Parent/child hierarchy among selected elements is kept. |
| Exposure Sheet | One or more columns or a cell range | Those columns and cells. |
| Drawing View | A selection of strokes/zones within a drawing | A single-drawing template. |
| Project branch | An element node or a scene node | A copy of that element, or of all of the scene's elements. |
| Operating system | `.animtpl` file | Imported as a template (§5.3). |
| Operating system | Image, movie or sound file | Imported as a media file item, not a template. |

The operation is undoable as one step. Default name: the element's name when exactly one element is captured; otherwise a
generic default template name made unique within the folder. The new item is selected and enters in-place rename. Failure
reports `library.templateCreateFailed`.

### 6.2 What is captured

| Captured | Notes |
|---|---|
| Elements | Every element type that can be selected, including cameras. |
| Vector drawings | Full drawing data for every drawing referenced by the captured cells. |
| Bitmaps | Embedded in the template. |
| Exposure / timing | Cell sequences relative to the first selected frame (the template starts at its frame 1). |
| Motion | Peg motion paths, keyframes, rotation/scale/skew/velocity functions, pivots, skeleton/bone setup. |
| Effects | Effect elements with their parameters and parameter groups. |
| Colours | The palette colours (by colour ID with their values) used by the captured drawings, so an insertion can add or update them (§7.4 *Update colours*). |
| Sound | Sound elements and their audio data. |
| Metadata | §6.3. |
| Thumbnail | A raster thumbnail rendered from the template (representative frame: frame 1) and stored with it. |

### 6.3 Template metadata

Each template stores: display name, artist name, copyright, creation and modification dates, size in bytes, template format
version, version ID (LIB-007), an optional source (path or link it was created from), and a summary of its element tree
(element type, name, frame count, hierarchy).

- **LIB-010** Artist name and copyright are stamped from the *user name* and *copyright* preferences (§12) **when the
  template is created**. Changing those preferences affects only templates created afterwards.

## 7. Using templates in a scene

### 7.1 Insertion targets

A template (or a media file item) is inserted into a scene by dragging it onto a view, or by an explicit insert command
(§19). Targets:

| Target | Result |
|---|---|
| Timeline, empty area or between layers | Creates new elements from the template at that position in the layer order. Exposures start at the frame under the drop point (frame 1 if none). |
| Timeline, onto an existing element | Merges into that element when the types match: a drawing template inserts its drawings/cells into the drawing element at the drop frame; a sound template creates or extends a sound element. Incompatible types fail with `library.pasteNotAllowed`. |
| Exposure Sheet | Same as Timeline: onto a column header or cell merges into that column at that frame; onto empty columns creates new columns. |
| Drawing View | A single-drawing template pastes its strokes into the current drawing, with a live preview of the strokes while dragging. |
| Camera View | Adds the template's elements to the current scene, starting at the current frame, placed at the drop point. |
| Static light table | Adds the dragged drawing to the static light table (section 02/04). |
| Library folder | Moves/copies the item (§5.2). |

Media file items inserted into a scene become the corresponding element type (image → image element, sound → sound
element, movie → media/movie element) as defined in sections 04 and 07.

Insertion is not allowed while bone deformation is being processed (`library.pasteNotAllowed`).

### 7.2 Copy versus link

A template enters the project in one of two modes:

- **Copy** (default for a normal drag): new elements and drawings are created in the scene. Drawing names are offset so
  they do not collide with existing drawings in the target element (deterministic rule: the next unused numeric names, in
  template order), unless *Overwrite existing drawings* applies (§7.4).
- **Link**: a **media element** (section 04) is created that references the template (library ID + template ID) instead of
  copying it. Its cells show the link information (template name, location, frame range, length, start frame in the
  template). Linked usages re-read the template when its version ID changes (§8). A link whose template cannot be found
  is shown as broken, keeps its timing, and renders nothing. The media element's reset command restores the full length
  of the linked template.

Link mode is chosen by dropping onto an existing media element, through the media element's add-template workflow, or
with the link modifier held while dragging. Linking to a template in the project-local library is always resolvable; linking
to a global/external library depends on that library remaining at the registered location — if the global library location
changes, linked templates keep working only if the library content is moved or re-imported there.

Export to formats that support reusable symbols (section 09) MAY write each linked template once and reuse it wherever it
is linked.

### 7.3 Colour handling

On insertion, colours referenced by the template are reconciled with the project palettes (section 03):
- Colour IDs missing from the project are added.
- With **Update colours** on, existing colours whose IDs match template colour IDs are set to the template's values.
- With it off, existing colours are kept unchanged.

### 7.4 Insertion options

Insertions follow the project's cut/drag/drop option set (section 04). The options relevant to templates:

| Option | Type | Effect |
|---|---|---|
| Overwrite existing drawings | bool | When merging into an existing element, a template drawing with the same name as an existing drawing replaces it; otherwise a new drawing is created. |
| Create exposure | bool | Writes cells (exposures) for the inserted drawings. |
| Extend exposure | bool | Extends the exposure up to the destination range. |
| Update colours | bool | §7.3. |
| Keyframe substitution | bool | Inserted peg keyframes replace existing keyframes from the insertion frame onward. |
| Function keyframes carried | set of {motion, rotation, scale, pivot, velocity, skew} | Which peg function keyframes are inserted. |
| Bounding keyframes | bool | Adds keyframes at the beginning and end of the inserted range. |

### 7.5 Updating drawings from a template

Drawings created by copying from a template remember the source template ID. After that template has changed, the user can
re-pull selected drawings: a list shows each drawing derived from the template whose source differs, with per-drawing
check boxes, select-all / clear-all, and an *Update colours* option. Applying replaces the chosen drawings with the
template's current versions (one undo step). The list is offered when a template is inserted onto an element that was
previously created from the same template, and is also available as an explicit command on an element.

### 7.6 Project branch operations

- Selecting an element node lists its drawings in the item list; drawings can be previewed.
- **Rename drawing** and **Delete drawing** use the section 04 drawing commands (including their name-collision error and
  the confirmation when deleting a drawing that is exposed in cells).
- **Copy drawing to current frame** copies the selected drawing into the current element at the current frame (one undo
  step). Fails with `library.wrongElementType` if the current element cannot hold the drawing (e.g. a vector drawing into
  a sound element).
- Dragging an element node onto another scene node, or into a Timeline/Exposure Sheet, copies the element into that scene.
- Dragging a scene or element node onto a library folder creates a template (§6.1).

## 8. Template edit mode

| Aspect | Requirement |
|---|---|
| Enter | *Edit template* on a template item (also double-click). Not allowed on read-only libraries (`library.readOnly`). |
| Behaviour | The template is loaded into a temporary scene whose name identifies the template being edited. That scene becomes current in all editing views (Timeline, Exposure Sheet, Drawing View, Camera View) and the user edits drawings, timing and motion with the normal tools. A persistent indicator shows that template edit mode is active and which scene will be returned to. |
| Refused while editing | Scene management (create, delete, reorder, show/hide scenes), entering edit mode on another template, and operations that would place content outside the template (e.g. importing whole projects). Refusals report `library.templateEditMode`. |
| Leave | *Return to scene*: the edited content is written back into the template, its version ID changes, its thumbnail and modification date are regenerated, and the previous current scene becomes current again. The template file reaches disk on the next library save (§11). |
| Effect on usages | Linked usages (§7.2) show the new content. Copied usages are unaffected unless refreshed (§7.5). |
| Undo | Edits inside the template are undoable while in edit mode. Returning to the scene commits the session as one undoable step (*Edit template*) that restores the previous template content. |
| Save / close / quit while editing | The application first returns to the scene automatically (committing the edits), then proceeds. |

## 9. Template properties

A read-only properties view (modeless) for the selected item shows: thumbnail; stored file name; type (template / image /
movie / sound / folder); template format version; artist name; copyright; size in bytes; last-modified date; source (may be
empty); and a details table with one row per element (element type and name, indented to show the peg/effect hierarchy,
frame count).

## 10. Import, export and other entry points

### 10.1 Import and export

- Import of media files and `.animtpl` files into a folder: §5.3.
- **Export template** writes a standalone `.animtpl` file (self-contained: drawings, bitmaps, sound, colours, metadata,
  thumbnail) to a chosen path. Exporting does not modify the library.
- Libraries are portable: a library folder copied to another machine can be opened with *Open library*.

### 10.2 Mouth-shape sets for lip-sync

Lip-sync (section 07) offers a picker that lists the templates of the mouth-shape starter library (§3) and any library
folder the user designates as a mouth-shape source, as a thumbnail grid. Single click selects, double click or OK
confirms. The chosen template's mouth drawings are imported into the drawing element mapped to the lip-sync (section 07).

## 11. Saving libraries

| Operation | Behaviour |
|---|---|
| Project save (section 01) | Saves the project and its project-local library, and also saves every modified global/opened library that has write access. |
| **Save global libraries** | Saves only modified global/opened libraries; the project is not touched. Enabled only when such a library is modified. |
| Close project / quit with modified global libraries | The user is asked whether to save them (save / discard / cancel). |
| Close an individual modified library | §5.1 (confirm discarding). |
| Format upgrade | If a library was written by an older version of the library format, saving converts it; the user must confirm first because older application versions will no longer open it. Declining skips saving that library. |

The result of a save MUST report separately whether the project and each library were saved, so a partial failure (e.g.
project saved, global library failed) is reported precisely. Failures: `library.saveFailed` (per library, naming the
template that could not be written where applicable), `library.folderCreateFailed`.

## 12. Preferences affecting the library

| Preference | Default | Effect |
|---|---|---|
| Global library location | A platform-appropriate per-user folder (e.g. under the user's documents or application-data folder). | Location of the user's writable global library. A change takes effect immediately and the tree reloads. Templates linked from the old location keep working only if the content is moved too. |
| User name | The operating-system account's full name. | Stamped as artist name into new templates (LIB-010). |
| Copyright | Empty (meaning no copyright). | Stamped as copyright into new templates. |
| Element colour for media (linked-template) elements | See section 01 | Colour of media elements in the Timeline. |
| Storyline colours (scene, scene with warning, transition) | §15.2 | Colours used in the Storyline track. |

---

# Part B — Scenes and Storyline

## 13. Concepts and data model

| Concept | Definition |
|---|---|
| **Movie** | The project as a whole: an **ordered list of scenes** plus movie-wide settings (frame rate, camera size; sections 01/10). |
| **Scene** | A self-contained piece of animation with its own Timeline/Exposure Sheet (elements, cameras, pegs, effects, sounds), its own length in frames, a **name**, a **background colour** and a **visible** flag. Exactly one scene is **current** at any time; all editing views show the current scene. |
| **Scene order** | The order of scenes in the Scene Manager. It determines playback and export order. |
| **Edit point** | A boundary in the storyline: before the first scene, between each pair of consecutive scenes, and after the last scene. A movie with N scenes has N+1 edit points. Each edit point holds exactly one **transition** (possibly *none* = cut). |
| **Transition** | A record: type, left duration, blank duration, right duration, colour, angle, direction, size (§16). |
| **Global frame** | A 1-based frame position on the whole-movie timeline, counting every visible scene in order with the frames that transitions add or overlap (§16.4). |

### 13.1 Data model

```
Movie
  frameRate, cameraSize, ...
  scenes: [Scene]                 // ordered = Scene Manager order
  startTransition: Transition     // edit point before the first scene
Scene
  id (stable), name
  backgroundColour: RGBA          // default #FFFFFFFF
  visible: bool                   // default true
  length: frames                  // derived from its Timeline/Exposure Sheet
  elements ...                    // sections 04/05
  outTransition: Transition       // edit point after this scene
Transition
  id (stable)
  type: "none" | "fade" | "crossDissolve" | "wipe" | "slide" | "clockWipe" | "checker"
  durationLeft:  int 0..1000 frames   // the single "duration" for overlap types
  blankDuration: int 0..1000 frames   // fade only
  durationRight: int 0..1000 frames   // fade only
  colour: RGBA                        // fade
  angle: number 0..360 degrees        // wipe, slide, clockWipe
  direction: "in"|"out"|"clockwise"|"counterclockwise"   // slide, clockWipe
  size: int 5..100, step 5            // checker
```

A transition record that cannot be read on load falls back to type *none*, and the load reports the warning
`storyline.transitionInvalid` naming the scene.

### 13.2 Scene names

- **STL-001** Default name of a new scene: `Scene-N` where N is the next unused number. Scenes created by importing a
  movie or another project get a distinguishable default (e.g. `Imported Scene-N`).
- **STL-002** Scene names MUST be non-empty (`storyline.nameEmpty`), MUST NOT contain `\ / : * ? " < > |`
  (`storyline.nameInvalid`), MUST be unique among the project's scenes (`storyline.nameInUse`) and MUST NOT exceed the
  maximum name length (`storyline.nameTooLong`; recommended 128 characters).
- The display name is independent of any storage name used inside the project package (section 10).

## 14. Scene management

### 14.1 Entry points

| Entry point | Function |
|---|---|
| **Storyline panel** (§15) | Storyline track, transitions and the embedded Scene Manager. Opened with a dedicated rebindable shortcut and through section 01's window commands; part of the default layout as a tab next to the Timeline. |
| **Scene Manager** (inside the Storyline panel) | Create, delete, select, make current, reorder, show/hide, rename, scene properties. |
| **Current-scene selector** | Available from the main toolbar and from the Exposure Sheet: lists all scenes; choosing one makes it current. |
| **Library project branch** (§7.6) | Browse each scene's elements; copy elements between scenes. |
| **Properties panel** | Shows the scene page or the transition page for the current Storyline/Scene Manager selection (§14.4, §16.2). |

### 14.2 Scene Manager list

A list with one row per scene in movie order, with add, delete and menu buttons above it and a context menu. Each row
shows a current-scene marker (on the current scene only), the scene name, and a visibility toggle. Rows can be reordered
by dragging.

### 14.3 Scene commands

| Command | Behaviour |
|---|---|
| **New scene** | Adds an empty scene named per STL-001 (undo *Add scene*), inserted after the selected scene, or at the end if nothing is selected. It receives the default contents and default length of a new scene (sections 04/05; typically one camera). Its outgoing edit point gets the default transition (§16.3). |
| **Delete scene** | Removes the selected scene and all its elements (undo *Delete scene*). Not allowed when only one scene exists (`storyline.lastScene`). If the current scene is deleted, the next scene (or the previous one if it was last) becomes current. The deleted scene's outgoing transition is removed; the transition before it is kept. |
| **Make current** | Switches all editing views to the scene (double-click a row or a scene block in the track). |
| **Select** | Single-click a row or a scene block: selects the scene for delete/properties and shows its scene page in the Properties panel. Selection alone does not change the current scene. |
| **Reorder** | Drag a row to a new position (undo *Reorder scene*). A scene's outgoing transition moves with it; the start transition stays at the movie start. The track redraws. |
| **Show / hide** | Click the visibility toggle (undo *Hide scene* / *Show scene*). Hidden scenes are excluded from Play Storyline, the movie preview and full-movie export, and take no space in the Storyline track (§15.2). |
| **Scene properties** | Opens the scene properties editor (§14.4). Available from the Scene Manager context menu. |
| **Rename** | From scene properties, the Properties panel scene page, or in-place in the list; rules STL-002. |

### 14.4 Scene properties

| Field | Default | Effect / undo |
|---|---|---|
| Name | current name | Renames the scene (STL-002). In the Properties panel it commits on Enter or focus-out (undo *Change scene name*). |
| Background colour | white | Colour drawn behind all elements of this scene in views, previews and exports (undo *Change scene background colour*). |
| Warning (read-only) | hidden | Shown only when the scene is too short for its transitions (§16.5): an icon and text explaining that the scene has been padded with blank frames to fit its transitions. |

When both fields are changed together in the properties editor, they are applied as one undo step (*Change scene
properties*). A view option (sections 01/05) switches the view background between the scene background colour and the
view's default background.

## 15. Storyline panel

### 15.1 Layout and controls

The panel contains the storyline area and the Scene Manager, separated by a resizable splitter.

| Control | Type / range | Function |
|---|---|---|
| Play Storyline | toggle | Starts or stops storyline playback from the storyline playhead (§17). |
| Zoom | slider 0–100, default 50, step 5 | Horizontal zoom of the storyline. When the panel is shown or resized, the storyline is fitted to the view. |
| Storyline area | — | A time ruler above a single horizontal track showing the whole movie (§15.2). |
| Scene Manager | — | §14.2. |

### 15.2 Track display and interaction

| Aspect | Requirement |
|---|---|
| Layout | One horizontal track. Each **visible** scene is a block whose width is proportional to its length in global frames (including padding), in movie order. Transition markers are drawn at edit points; a transition with duration is drawn as a span covering the frames it uses. Hidden scenes are not drawn (they remain listed in the Scene Manager with their hidden state). |
| Colours | Scene blocks use the *scene* colour; scenes with a duration warning use the *scene with warning* colour; transition spans use the *transition* colour. All three are user-settable element-colour preferences (section 01); defaults in §20. |
| Labels | The scene name is drawn inside each block (truncated to fit). |
| Time ruler | In seconds, computed from the movie frame rate, tick labels with two decimals. |
| Playhead | A playhead line. Clicking moves the playhead to that position; a click on or near an edit point snaps to it. Dragging scrubs; while scrubbing, the Camera View shows the movie frame under the playhead (switching scenes as needed) including transitions. |
| Selecting a scene | Click a scene block: selects the scene; the Properties panel shows the scene page. |
| Selecting a transition | Click a transition marker/span or an edit point: selects and highlights it, the playhead snaps to it, the Properties panel shows the transition page. |
| Double-click | On a transition: opens/raises the Properties panel on it. On a scene: makes it current. |
| Auto-scroll | During playback, or when dragging near the edges, the view scrolls to keep the playhead visible. |
| Live update | The track redraws when scenes, exposures, scene lengths, visibility or transitions change, and on each playback frame. |
| Selection sync | Selection is synchronised between the Storyline track, the Scene Manager list and the Properties panel. |

## 16. Transitions

### 16.1 Editing a transition

1. Select a transition or an edit point in the Storyline track.
2. The Properties panel shows the transition page.
3. Choose a type and edit its parameters; each change is one undo step and the track redraws.
4. Play Storyline to review.

Removing a transition sets its type to *none* (straight cut).

### 16.2 Transition page fields

Only the fields relevant to the selected type are shown; labels adapt (e.g. *Duration* for single-duration types, *Left
duration* for fade).

| Field | Type | Range | Default | Unit | Undo label |
|---|---|---|---|---|---|
| Type | enum | §16.3 | see §16.3 | — | *Change transition type* |
| Duration / Left duration | int | 0–1000 | 12 | frames | *Change transition length* |
| Blank duration | int | 0–1000 | 0 | frames | *Change transition length* |
| Right duration | int | 0–1000 | 12 | frames | *Change transition length* |
| Colour | RGBA | — | opaque black | — | *Change transition colour* |
| Angle | number | 0–360 | 0 | degrees | *Change transition angle*. Edited with a dial and a numeric field kept in sync; the dial commits on release, the field on edit completion. |
| Direction | enum | type-dependent (§16.3) | first allowed value | — | *Change transition direction* |
| Size | int | 5–100, step 5 | 25 | — | *Change transition size* |

### 16.3 Transition types

New edit points (new scenes, new projects) start as **none**. When the user applies a transition to an edit point that
has none, **fade** is pre-selected. When a type is chosen, parameters that have not been set take the defaults of §16.2.

Angle convention for all types: 0° points to the right (+x), angles increase counter-clockwise (90° points up).
Progress `t` for frame *i* (1-based) of a transition of *D* frames is `t = i / (D + 1)`, so neither end frame is purely one
scene.

| Type | Parameters | Visual behaviour |
|---|---|---|
| **none** | — | Hard cut: the last frame of the outgoing scene is followed directly by the first frame of the incoming one. At movie start/end: no effect. |
| **fade** | left duration, blank duration, right duration, colour | The outgoing scene fades to *colour* over its last *left* frames; then *blank* frames of solid colour are held; then the incoming scene fades in from the colour over its first *right* frames. At movie start only the right part applies (fade in from the colour); at movie end only the left part (fade out to the colour). |
| **crossDissolve** | duration | The two scenes overlap for *duration* frames; the incoming scene's opacity rises linearly with `t` over the outgoing scene. |
| **wipe** | duration, angle | A straight edge perpendicular to *angle* sweeps across the whole frame in the *angle* direction over *duration* frames, revealing the incoming scene behind the edge. |
| **slide** | duration, angle, direction `in`/`out` | `in`: the incoming scene slides into the frame travelling in the *angle* direction and covers the outgoing scene. `out`: the outgoing scene slides out of the frame in the *angle* direction and uncovers the incoming scene. |
| **clockWipe** | duration, angle, direction `clockwise`/`counterclockwise` | A radial hand starting at *angle* sweeps 360° around the frame centre in the chosen direction over *duration* frames, revealing the incoming scene in the swept area. |
| **checker** | duration, size | The frame is divided into a checkerboard of square cells whose edge length is (*size*/100) × (shorter frame side / 4) (size 25 gives 16 cells across the shorter side). During the first half of the transition the incoming scene appears in the cells of one parity, during the second half in the other parity; within each half each cell's revealed area grows from its centre with progress. |

At the movie start and end, overlap-type transitions (crossDissolve, wipe, slide, clockWipe, checker) operate against a
blank picture filled with the adjacent scene's background colour; fade uses the transition colour.

### 16.4 Timing model

1. The movie timeline consists of the **visible** scenes in Scene Manager order; scene *i* contributes its length *Lᵢ*.
2. The transition used between two consecutive visible scenes A and C is A's outgoing transition (outgoing transitions
   of hidden scenes are ignored). The movie-start transition applies before the first visible scene; the last visible
   scene's outgoing transition applies at the movie end.
3. **Overlap types** (crossDissolve, wipe, slide, clockWipe, checker) between A and B: A's last *D* frames and B's first
   *D* frames play simultaneously and are composited with progress `t`. The global length is reduced by *D*.
4. **Fade** between A and B: nothing overlaps. A fades out over its last *left* frames, *blank* frames of solid colour are
   **inserted** (they add to the global length), then B fades in over its first *right* frames.
5. Movie start/end edit points use only the side that touches a scene; a fade's blank frames at start/end are inserted;
   overlap types at start/end consume the scene's frames without shortening the global length.
6. **Global length** = Σ *Lᵢ* + Σ blank durations − Σ overlap durations (between scenes) + Σ padding (rule 7).
7. **Too-short scenes**: if the frames a scene must supply to its incoming transition (right part, or overlap *D*) plus
   its outgoing transition (left part, or overlap *D*) exceed *Lᵢ*, the scene is **padded** with blank frames filled with
   its background colour, appended after its own last frame, until it is long enough. The scene's own Timeline is not
   modified; padding exists only in the storyline, storyline playback, previews and export. The scene is flagged with a
   warning (§16.5).
8. A transition whose durations are all 0 is equivalent to a cut.
9. Sound: each scene's sound starts at that scene's global start frame; sounds of overlapping scenes are mixed; blank and
   padding frames are silent.

### 16.5 Scene with warning

A scene padded by §16.4 rule 7 is drawn in the *scene with warning* colour and its scene page shows the warning. The
warning disappears when the scene is lengthened or the transition durations are reduced. Queries report the warning and
the number of padding frames (§19).

## 17. Playing, previewing and exporting with transitions

| Function | Behaviour |
|---|---|
| **Play scene** (section 09) | Plays the current scene only, within its playback range. No transitions. |
| **Play Storyline** | Plays the whole movie (visible scenes) with transitions in real time in the Camera View, starting from the storyline playhead. As the playhead crosses an edit point the transition is drawn and the following scene is shown; the storyline playhead follows and auto-scrolls. Toggling again stops. Follows the loop and force-frame-rate playback settings (section 09). Sound plays along when sound playback is on, laid out per §16.4 rule 9. |
| **Preview scene** (section 09) | Renders and plays the current scene only. No transitions. |
| **Preview movie** (section 09) | Renders all visible scenes into one preview **with** transitions and plays it. Hidden scenes are excluded. Insufficient memory reports `storyline.outOfMemory`. |
| **Export** (section 09) | Export type *full movie* covers all visible scenes; the range choice includes *all with transitions* and *all without transitions* (scenes simply concatenated; default). Formats that cannot render transitions disable the *with transitions* choice. The sound track is laid out per §16.4 rule 9. |

## 18. Acceptance scenarios

1. Create 3 scenes; give scene 2 a red background.
2. Set a fade (left 12, blank 6, right 12, black) between scenes 1 and 2, and a wipe (angle 90°, duration 10) between
   scenes 2 and 3. Verify the global length = L₁ + L₂ + L₃ + 6 − 10.
3. Shorten scene 3 below 10 frames; verify the warning colour, the warning on the scene page, the padding count in the
   query, and that scene 3's own Timeline is unchanged.
4. Play Storyline from the middle of scene 1; verify both transitions render and playback stops at the end (loop off).
5. Hide scene 2; verify it disappears from the track and the movie preview, and that scene 1's outgoing transition (the
   fade) now applies between scenes 1 and 3.
6. Export with and without transitions and compare the frame counts.
7. Create a template from two elements of scene 1 in a new library folder; insert it into scene 3 by copy at frame 5 and
   by link; edit the template in template edit mode; verify the linked usage updates and the copied one does not until it
   is updated from the template.

---

## 19. Agent interface

### 19.1 Agent-specific requirements

- **AGT-08-01 No gesture-only paths.** Every drag-and-drop in this section has an explicit command: inserting a template
  or media item into a scene (`library.template.insert`), creating a template from project content
  (`library.template.create`), moving/copying library nodes (`library.item.move` / `library.item.duplicate`), importing
  files (`library.item.import`), copying a drawing to a frame (`library.drawing.insert`), copying an element between scenes
  (`library.element.duplicate`), reordering scenes (`storyline.scene.move` with an explicit target index), and scrubbing
  the storyline (`storyline.playhead.set`). File pickers are replaced by `path` parameters.
- **AGT-08-02 Panels.** The Library and Storyline panels are opened/closed with section 01's generic window commands
  (panel ids `library` and `storyline`). None of the commands below requires the panel to be open.
- **AGT-08-03 Addressing.** Libraries, folders, items, scenes and transitions have stable opaque IDs. Human-readable paths
  are also accepted: `library:"Starter"/folder:"Props"/template:"Chair"`, `scene:"Intro"`, `scene:#2`. All indices are
  **1-based**, consistent with section 11: scene positions are 1-based indices into the ordered scene list (hidden
  scenes included; 1 = first scene, sceneCount = last), and `index` parameters for positions among library siblings are
  1-based likewise. **Edit points** are *boundaries*, not positions, and are numbered **0..sceneCount**: edit point 0 is
  the boundary before the first scene (the movie-start transition), and edit point *k* (1 ≤ *k* ≤ sceneCount) is the
  boundary after scene *k* (that scene's outgoing transition; *k* = sceneCount is the movie end). They may also be
  addressed by `afterSceneId`. Frames (scene-local and global) are 1-based integers.
- **AGT-08-04 Coordinates.** `position` in `library.template.insert` is in scene/field space (section 05): the template's
  origin is placed at `position` by applying the offset to the inserted top-level elements (a top-level peg if the
  template has one, otherwise each top-level element's transform). Drawing-target insertions place strokes in drawing
  space (section 02) of the target drawing, offset by `position` if given.
- **AGT-08-05 Confirmations.** Operations that prompt in the GUI take an explicit parameter instead and otherwise return a
  structured `needs_confirmation` result (with `code` and a description of what would be lost) without changing anything:
  `library.close` / `library.writeAccess.set` (`discardChanges`), `library.folder.delete` (`confirm`), `library.save`
  of an older-format library (`confirmUpgrade`), and drawing deletion (section 04). Name collisions take
  `onConflict: "rename"|"skip"|"replace"|"fail"`.
- **AGT-08-06 Read-back.** The agent can verify its work without screenshots: library tree and item metadata as JSON,
  thumbnails and rendered template frames as PNG, the template's element tree, the result of an insertion (IDs of every
  created element/drawing/colour), the full storyline model with computed global timing, padding and warnings, the
  mapping of a global frame to scene/local frame/transition, and a rendered storyline frame (PNG) including transitions.
- **AGT-08-07 Determinism.** Given the same project state and parameters, `library.template.insert` produces the same
  element order, element names, drawing names (deterministic offset rule, §7.2) and colour changes.
  `storyline.frame.render` of a given global frame is deterministic (progress formula §16.3).
- **AGT-08-08 Long operations.** Opening a very large library, regenerating thumbnails for a folder, and saving many
  libraries MAY run as jobs (`job_status` / `job_cancel`, section 11) when they exceed the job threshold of section 11;
  the command then returns `{jobId}` and the final result is retrieved from the job.
- **AGT-08-09 Template edit mode** is a modal state, not a dialog: `library.template.edit.start` returns the ID of the
  temporary edit scene, the agent edits it with the normal commands of sections 02–07, and `library.template.edit.stop`
  commits. Commands refused in edit mode fail with `library.templateEditMode`.
- **AGT-08-10 Errors** use the codes of §19.5; every failing command returns `{code, message, details}` and changes
  nothing.

### 19.2 Command table

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `library.list` | query | List open libraries | `kinds?: [enum "global"\|"project"\|"external"\|"starter"]` | `{libraries: [LibraryInfo]}` | n/a |
| `library.create` | action | Create a new empty library in a folder and open it | `path: path; name?: string` | `{libraryId}` | no |
| `library.open` | action | Open an existing library | `path: path` | `{libraryId, readOnly}` | no |
| `library.close` | action | Close a library (not the project-local one) | `libraryId: id; discardChanges: bool = false` | `{closed}` or `needs_confirmation` | no |
| `library.save` | action | Save modified libraries (project-local is saved with the project) | `libraryIds?: [id]; scope: enum "global"\|"all" = "global"; confirmUpgrade: bool = false` | `{results: [{libraryId, saved, code?}]}` | no |
| `library.refresh.run` | action | Re-read a library or folder from disk and regenerate missing thumbnails | `libraryId: id; folderId?: id` | `{changedNodeIds: [id]}` | no |
| `library.starter.open` | action | Restore the starter library (re-open; optionally reinstall its files) | `reinstall: bool = false` | `{libraryId}` | no |
| `library.writeAccess.set` | toggle | Request or release exclusive write access to a library | `libraryId: id; enabled: bool; discardChanges: bool = false` | `{writable}` or `needs_confirmation` | no |
| `library.tree.get` | query | Get the folder/item hierarchy | `libraryId?: id; folderId?: id; depth: int = -1; includeItems: bool = true` | `{nodes: [LibraryNode]}` (nested) | n/a |
| `library.node.get` | query | Get one library, folder or item with its location (library name and folder path) | `nodeId: id` | `LibraryNode & {libraryName, path}` | n/a |
| `library.folder.create` | action | Create a folder ("New folder") | `parentId: id; name?: string; index?: int (1-based)` | `{folderId, name}` | yes |
| `library.folder.rename` | action | Rename a folder | `folderId: id; name: string` | `{name}` | yes |
| `library.folder.delete` | action | Delete a folder and its contents | `folderId: id; confirm: bool = false` | `{deletedNodeCount}` or `needs_confirmation` | yes |
| `library.item.list` | query | List the contents of a folder | `folderId: id; recursive: bool = false; types?: [enum "folder"\|"template"\|"image"\|"movie"\|"sound"]; sort: enum "name"\|"type"\|"size"\|"modified"\|"artist" = "name"; includeThumbnails: bool = false; thumbnailSize: int = 128 (16..1024)` | `{items: [ItemSummary]}` | n/a |
| `library.item.get` | query | Get item properties (§9) | `itemId: id` | `ItemDetails` | n/a |
| `library.item.rename` | action | Rename an item | `itemId: id; name: string` | `{name}` | yes |
| `library.item.delete` | action | Delete items | `itemIds: [id]` | `{deletedIds: [id]}` | yes |
| `library.item.move` | action | Move folders/items to a folder (same or another library) | `nodeIds: [id]; targetFolderId: id; index?: int (1-based); onConflict: enum "rename"\|"fail" = "rename"` | `{nodeIds: [id]}` | yes |
| `library.item.duplicate` | action | Copy folders/items into a folder | `nodeIds: [id]; targetFolderId: id; onConflict: enum "rename"\|"fail" = "rename"` | `{newNodeIds: [id]}` | yes |
| `library.item.import` | action | Import media or template files into a folder | `folderId: id; paths: [path]; as: enum "auto"\|"image"\|"movie"\|"sound"\|"template" = "auto"; onConflict: enum "rename"\|"skip"\|"replace" = "rename"` | `{imported: [{path, itemId, name, renamed}], failed: [{path, code}]}` | yes |
| `library.item.thumbnail.get` | query | Get an item's thumbnail (generated if missing) | `itemId: id; size: int = 256 (16..1024); regenerate: bool = false` | `{png, width, height}` | n/a |
| `library.template.search` | query | Search templates and items across libraries with metadata | `query?: string; libraryIds?: [id]; folderId?: id; recursive: bool = true; types: [enum "template"\|"image"\|"movie"\|"sound"] = ["template"]; elementTypes?: [string]; hasSound?: bool; singleDrawing?: bool; artist?: string; modifiedAfter?: string; sort: enum "relevance"\|"name"\|"modified" = "relevance"; limit: int = 50 (1..500); offset: int = 0; includeThumbnails: bool = false; thumbnailSize: int = 128` | `{total, items: [ItemSummary]}` | n/a |
| `library.template.get` | query | Get full template metadata and element tree | `templateId: id` | `TemplateDetails` | n/a |
| `library.template.frame.render` | query | Render one frame of a template (preview) | `templateId: id; frame: int = 1; width: int = 320; height?: int; background?: colour` | `{png, width, height, frameCount}` | n/a |
| `library.template.create` | action | Create a template from project objects (elements, a scene, a drawing, strokes) | `folderId: id; objectIds: [id]; frameRange?: {start: int, end: int}; name?: string` | `{templateId, name, frameCount, elementCount}` | yes |
| `library.template.insert` | action | Insert a template or media item into a scene by copy or link | `templateId: id; sceneId?: id; target: enum "timeline"\|"xsheet"\|"camera"\|"drawing" = "timeline"; frame: int = 1; position?: point; layerIndex?: int; intoElementId?: id; drawingId?: id; mode: enum "copy"\|"link" = "copy"; options?: InsertOptions` | `InsertResult` | yes |
| `library.template.export` | action | Export a template as a standalone `.animtpl` file | `templateId: id; path: path; overwrite: bool = false` | `{path, sizeBytes}` | no |
| `library.template.edit.start` | action | Enter template edit mode | `templateId: id` | `{editSceneId, returnSceneId}` | no |
| `library.template.edit.stop` | action | Return to scene, committing edits into the template ("Edit template") | — | `{templateId, versionId}` | yes |
| `library.template.edit.get` | query | Get template edit mode state | — | `{active, templateId?, editSceneId?, returnSceneId?}` | n/a |
| `library.templateUpdate.list` | query | List drawings of an element derived from templates, with changed state | `elementId: id; templateId?: id` | `{drawings: [{drawingId, name, templateId, templateName, changed}]}` | n/a |
| `library.templateUpdate.apply` | action | Update chosen drawings from their template | `elementId: id; drawingIds: [id]; updateColours: bool = false` | `{updatedDrawingIds: [id], updatedColourIds: [id]}` | yes |
| `library.drawing.insert` | action | Copy a drawing (project branch or single-drawing item) to an element frame ("Copy drawing") | `drawingId: id; elementId?: id; frame?: int` | `{newDrawingId, elementId, frame}` | yes |
| `library.element.duplicate` | action | Copy an element into another scene (project branch) | `elementId: id; targetSceneId: id; layerIndex?: int; frame: int = 1` | `{elementIds: [id]}` | yes |
| `library.project.get` | query | Get the project branch: scenes, elements, optionally drawings | `sceneId?: id; includeDrawings: bool = false` | `{scenes: [{sceneId, name, elements: [{elementId, name, type, drawings?: [{drawingId, name}]}]}]}` | n/a |
| `library.view.set` | ui | Set the item-list display mode | `mode: enum "thumbnails"\|"list"\|"details"` | `{mode}` | no |
| `library.view.get` | query | Get panel state | — | `{mode, currentNodeId, selectedNodeIds, preview}` | n/a |
| `library.selection.set` | ui | Select library nodes (tree or item list) | `nodeIds: [id]; reveal: bool = true` | `{selectedNodeIds}` | no |
| `library.selection.get` | query | Get the library selection | — | `{selectedNodeIds}` | n/a |
| `library.preview.set` | ui | Set preview item, frame, playing and enabled state | `itemId?: id; frame?: int; playing?: bool; enabled?: bool` | `{itemId, frame, frameCount, playing, enabled}` | no |
| `library.preview.get` | query | Get preview state | — | `{itemId, frame, frameCount, playing, enabled}` | n/a |
| `library.changed` | event | A library's content or state changed | — | `{libraryId, nodeIds: [id], change: enum "created"\|"renamed"\|"moved"\|"deleted"\|"modified"\|"saved"\|"writeAccess"}` | n/a |
| `library.opened` | event | A library was opened | — | `{libraryId, kind, readOnly}` | n/a |
| `library.closed` | event | A library was closed | — | `{libraryId}` | n/a |
| `library.template.changed` | event | A template's content changed (version ID bumped) | — | `{templateId, versionId}` | n/a |
| `library.template.edit.changed` | event | Template edit mode entered or left | — | `{active, templateId?, editSceneId?}` | n/a |
| `storyline.get` | query | Get the full storyline model with computed global timing | `includeHidden: bool = true` | `StorylineModel` | n/a |
| `storyline.scene.list` | query | List scenes in order | — | `{scenes: [SceneInfo]}` | n/a |
| `storyline.scene.get` | query | Get one scene | `sceneId: id` | `SceneInfo` | n/a |
| `storyline.scene.create` | action | Add a scene ("Add scene") | `index?: int (1..sceneCount+1; default after selection, else end); name?: string; makeCurrent: bool = false` | `{sceneId, index, name}` | yes |
| `storyline.scene.delete` | action | Delete a scene and its content ("Delete scene") | `sceneId: id` | `{currentSceneId}` | yes |
| `storyline.scene.move` | action | Move a scene to an index ("Reorder scene") | `sceneId: id; index: int (1..sceneCount)` | `{order: [id]}` | yes |
| `storyline.scene.rename` | action | Rename a scene ("Change scene name") | `sceneId: id; name: string` | `{name}` | yes |
| `storyline.scene.set` | action | Set scene properties in one step ("Change scene properties") | `sceneId: id; name?: string; backgroundColour?: colour` | `SceneInfo` | yes |
| `storyline.scene.visible.set` | toggle | Show or hide a scene in the movie ("Show/Hide scene") | `sceneId: id; visible: bool` | `{visible}` | yes |
| `storyline.currentScene.set` | ui | Make a scene current in all editing views | `sceneId: id` | `{sceneId}` | no |
| `storyline.currentScene.get` | query | Get the current scene | — | `{sceneId, index, name}` | n/a |
| `storyline.selection.set` | ui | Select a scene or an edit point in the Storyline/Scene Manager | `sceneId?: id; editPoint?: int; afterSceneId?: id` | `{selection}` | no |
| `storyline.selection.get` | query | Get the storyline selection | — | `{kind: enum "none"\|"scene"\|"transition", sceneId?, editPoint?}` | n/a |
| `storyline.transition.list` | query | List all edit points with their transitions and computed spans | — | `{transitions: [TransitionInfo]}` | n/a |
| `storyline.transition.get` | query | Get the transition at an edit point | `editPoint?: int; afterSceneId?: id; transitionId?: id` | `TransitionInfo` | n/a |
| `storyline.transition.set` | action | Set transition type and parameters at an edit point (one undo step) | `editPoint?: int; afterSceneId?: id; type?: enum "none"\|"fade"\|"crossDissolve"\|"wipe"\|"slide"\|"clockWipe"\|"checker"; durationLeft?: int (0..1000); blankDuration?: int (0..1000); durationRight?: int (0..1000); colour?: colour; angle?: number (0..360); direction?: enum "in"\|"out"\|"clockwise"\|"counterclockwise"; size?: int (5..100, step 5)` | `TransitionInfo` | yes |
| `storyline.transition.remove` | action | Set an edit point's transition to none (cut) | `editPoint?: int; afterSceneId?: id` | `TransitionInfo` | yes |
| `storyline.transition.types.list` | query | List transition types with their parameters, ranges, defaults and allowed directions | — | `{types: [{type, parameters: [{name, type, min?, max?, step?, default, values?}]}]}` | n/a |
| `storyline.frame.get` | query | Map a global frame to scene(s), local frame(s) and transition | `globalFrame: int` | `{globalFrame, kind: enum "scene"\|"transition"\|"blank"\|"padding", sceneId?, localFrame?, transition?: {editPoint, t, outgoing?: {sceneId, localFrame}, incoming?: {sceneId, localFrame}}}` | n/a |
| `storyline.frame.render` | query | Render a global storyline frame including transitions | `globalFrame: int; width?: int; height?: int` | `{png, width, height}` | n/a |
| `storyline.playhead.set` | ui | Move the storyline playhead (scrub) | `globalFrame?: int; editPoint?: int` | `{globalFrame}` | no |
| `storyline.playhead.get` | query | Get the storyline playhead | — | `{globalFrame, sceneId, localFrame}` | n/a |
| `storyline.play.start` | ui | Start Play Storyline (whole movie with transitions) | `fromGlobalFrame?: int; loop?: bool` | `{playing: true, globalFrame}` | no |
| `storyline.play.stop` | ui | Stop Play Storyline | — | `{playing: false, globalFrame}` | no |
| `storyline.view.set` | ui | Set storyline zoom or fit to view | `zoom?: int (0..100, step 5); fit?: bool` | `{zoom}` | no |
| `storyline.view.get` | query | Get storyline view state | — | `{zoom, playing, globalFrame}` | n/a |
| `storyline.changed` | event | Scenes, order, visibility, lengths or transitions changed | — | `{reason: enum "scenes"\|"order"\|"visibility"\|"length"\|"transition"\|"properties", sceneIds: [id], editPoints: [int], globalLength}` | n/a |
| `storyline.currentScene.changed` | event | The current scene changed | — | `{sceneId}` | n/a |
| `storyline.selection.changed` | event | Storyline selection changed | — | `{kind, sceneId?, editPoint?}` | n/a |
| `storyline.warning.changed` | event | A scene's too-short warning appeared or cleared | — | `{sceneId, warning, paddingFrames}` | n/a |
| `storyline.play.changed` | event | Storyline playback started/stopped or crossed an edit point | — | `{playing, globalFrame, sceneId}` | n/a |

Library preferences (§12) are read and written with section 01's `prefs` commands (recommended keys
`library.globalPath`, `user.name`, `user.copyright`, and the storyline colour keys `interface.storylineColours.*`).
Generic clipboard commands (`edit.copy` / `edit.paste`, section 01) act on the Library selection when the Library has focus.

### 19.3 Result shapes

```jsonc
// LibraryInfo
{ "libraryId": "lib_…", "name": "Starter", "kind": "global|project|external|starter",
  "path": "…", "readOnly": false, "writable": true, "modified": false, "formatVersion": "1.0" }

// LibraryNode (tree)
{ "nodeId": "…", "type": "library|folder|template|image|movie|sound|broken", "name": "Props",
  "children": [ /* LibraryNode */ ], "empty": false }

// ItemSummary (list/search)
{ "itemId": "tpl_…", "libraryId": "lib_…", "folderPath": ["Props", "Furniture"], "name": "Chair",
  "type": "template", "artist": "…", "copyright": "…", "created": "2026-10-17T10:00:00Z",
  "modified": "…", "sizeBytes": 48211, "versionId": "…", "frameCount": 24,
  "elementTypes": ["peg", "drawing"], "hasSound": false, "singleDrawing": false,
  "thumbnail": { "png": "<base64>", "width": 128, "height": 96 } /* only if requested */ }

// TemplateDetails = ItemSummary + 
{ "fileName": "…", "formatVersion": "1.0", "source": "…",
  "elements": [ { "name": "Peg 1", "type": "peg", "frames": 24,
                  "children": [ { "name": "Arm", "type": "drawing", "frames": 24, "drawingCount": 6 } ] } ],
  "colours": [ { "colourId": "…", "name": "…", "value": "#RRGGBBAA" } ],
  "linkedUsages": [ { "sceneId": "…", "elementId": "…" } ] }

// InsertOptions (defaults = the project's cut/drag/drop option set, section 04)
{ "overwriteDrawings": false, "createExposure": true, "extendExposures": false,
  "updateColours": false, "keyframeSubstitution": false,
  "functionKeyframes": ["motion", "rotation", "scale", "pivot", "velocity", "skew"],
  "boundingKeyframes": false }

// InsertResult
{ "mode": "copy|link", "sceneId": "…", "elementIds": ["…"], "drawingIds": ["…"],
  "mergedIntoElementId": null, "addedColourIds": ["…"], "updatedColourIds": [],
  "frameRange": { "start": 5, "end": 28 } }

// StorylineModel
{ "frameRate": 24, "globalLength": 312, "startTransition": { /* TransitionInfo */ },
  "scenes": [ { /* SceneInfo */ } ] }

// SceneInfo
{ "sceneId": "…", "index": 1, "name": "Scene-1", "visible": true, "current": true,
  "backgroundColour": "#FFFFFFFF", "length": 120, "globalStart": 1, "globalEnd": 120,
  "paddingFrames": 0, "warning": false, "outTransition": { /* TransitionInfo */ } }

// TransitionInfo
{ "transitionId": "…", "editPoint": 1, "afterSceneId": "…", "beforeSceneId": "…",
  "active": true /* false if ignored because of hidden scenes */,
  "type": "fade", "durationLeft": 12, "blankDuration": 0, "durationRight": 12,
  "colour": "#000000FF", "angle": 0, "direction": null, "size": 25,
  "globalSpan": { "start": 109, "end": 132 } }
```

### 19.4 Queries and events

- `library.template.search` is the primary discovery tool: it searches names, folder paths and artist across all open
  libraries (or a subset) and returns metadata and, on request, inline PNG thumbnails, so an agent can choose content
  without opening the panel. Results are paginated (`total`, `limit`, `offset`).
- `library.template.get` returns the element tree, colours and linked usages, enough to predict what an insertion creates.
- `storyline.get` returns everything the Storyline track shows: scene order, visibility, lengths, computed global start/end
  per scene, padding and warnings, every transition with its parameters and global span, and the total global length.
- Events are delivered through the `app://events` resource subscription (section 11).

### 19.5 Error codes

| Code | When |
|---|---|
| `library.notFound` | Unknown library/folder/item/template ID or path |
| `library.pathMissing` | Configured or requested library path does not exist |
| `library.openFailed` | Library cannot be opened (unreadable, empty path) |
| `library.corrupt` | Index or template file is damaged |
| `library.alreadyOpen` | Opening a library that is already open |
| `library.alreadyExists` | Creating a library in a folder that already contains one |
| `library.readOnly` | Modifying a library without write access |
| `library.nameEmpty` / `library.nameInvalid` / `library.nameInUse` | Name rules LIB-003/LIB-004 |
| `library.createFailed` | Folder creation failed |
| `library.moveFailed` | Move failed |
| `library.pasteNotAllowed` | Invalid paste/insert target (library tree, incompatible element, bone deformation in progress) |
| `library.addFailed` | Adding dropped content to a folder failed |
| `library.importFailed` | Import of a file failed |
| `library.unsupportedFormat` | File type not supported |
| `library.protectedMedia` | Copy-protected movie file refused |
| `library.templateCreateFailed` | Create template failed |
| `library.templateInsertFailed` | Inserting a template into a scene failed |
| `library.wrongElementType` | Copy drawing to an element that cannot hold it |
| `library.templateEditMode` | Command not allowed in template edit mode, or edit mode not active for `library.template.edit.stop` |
| `library.saveFailed` | A library or template could not be written |
| `library.folderCreateFailed` | A folder on disk could not be created while saving |
| `library.upgradeRequired` | Saving an older-format library without `confirmUpgrade` (returned as `needs_confirmation`) |
| `storyline.notFound` | Unknown scene or edit point |
| `storyline.indexOutOfRange` | Scene index or edit point outside the valid range |
| `storyline.nameEmpty` / `storyline.nameInvalid` / `storyline.nameInUse` / `storyline.nameTooLong` | Scene name rules STL-002 |
| `storyline.lastScene` | Deleting the only scene |
| `storyline.parameterNotApplicable` | A transition parameter that does not apply to the type (e.g. `size` for fade); the rest of the change is not applied |
| `storyline.transitionInvalid` | Unreadable transition on load (warning; falls back to none) |
| `storyline.templateEditMode` | Scene management while in template edit mode |
| `storyline.outOfMemory` | Movie preview cannot be generated |

---

## 20. Design decisions / open questions

1. **Default transition for new edit points.** Decision: *none* (cut); fade is pre-selected when the user first applies a
   transition. Alternative: default fade 12/0/12 black everywhere (would fade the movie in/out by default — not recommended).
2. **Overlap vs inserted frames.** Decision: overlap types overlap the adjacent scenes and shorten the global length; fade
   inserts only its blank frames (§16.4). Keep this consistent across playback, preview and export.
3. **Transition parameter conventions** (angle origin/direction, slide/clock-wipe semantics, checker size unit, progress
   formula) are as stated in §16.3. Defaults: durations 12 frames, blank 0, colour opaque black, angle 0, size 25.
4. **Hidden scenes and transitions.** Decision: hidden scenes take no space in the track; the outgoing transition of the
   last visible scene before a hidden run applies. Alternative: show hidden scenes as dimmed zero-width markers.
5. **Reorder and transitions.** Decision: a scene's outgoing transition travels with it.
6. **Storyline colours** (preference defaults). Recommended: scene `#7FA6D9FF`, scene with warning `#E8A33DFF`,
   transition `#9B6FD1FF`; new scene background white.
7. **Copy vs link.** Decision: a plain drag copies; link is chosen explicitly (link modifier, drop onto a media element, or
   `mode: "link"`). Linking to templates outside the project-local library makes the project depend on external files;
   the project save SHOULD warn about external links and offer to copy the linked templates into the project-local library.
8. **Template edit mode undo.** Decision: the whole edit session becomes one undo step on return; save/close auto-commits.
9. **Library write locking** across instances uses a lock file in the library folder (format in section 10); stale locks
   (owner process gone) are reclaimed automatically.
10. **Template count limits.** Decision: none.
11. **Global library default location.** Decision: per-user application-data folder on each platform, created on first run.
12. **Duplicate scene** is not required by this section; MAY be added later as `storyline.scene.duplicate`.
13. **Play Storyline ownership.** `storyline.play.start/stop` are the canonical commands for storyline playback; section 09's
    playback commands cover single-scene playback and SHOULD reference these rather than define duplicates.
14. **Template drawing name offset rule** (§7.2): new drawings take the next unused numeric names in the target element in
    template order. Implementers may choose another scheme provided it is deterministic and documented.
