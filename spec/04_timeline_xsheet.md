# 04 — Elements, Timeline and Exposure Sheet

This section specifies **elements** (the layers of a scene), the **Timeline view**, the **Exposure Sheet view** (also
called X-sheet or dope sheet), the cell and exposure editing commands that operate on them (exposures, blanks, fill,
cycles, columnate, drawing substitution, cut/copy/paste of cells), the management of the drawings owned by an element,
and the static light table panel of the exposure sheet. It also defines the agent-facing command namespaces
`element`, `cell`, `timeline` and `xsheet` (section 17).

Related sections:

| Topic | Section |
|---|---|
| Main window, docking, window toggles, preferences shell, key bindings, undo/redo framework, generic clipboard commands | 01_application_shell.md |
| Drawing View, drawing tools, onion skin, automatic light table, how static-light-table drawings are rendered in the Drawing View, vectorisation options | 02_drawing.md |
| Palettes and colours (colour update when pasting templates) | 03_colour.md |
| Pegs, keyframes and interpolation, motion paths, Camera/Top/Side views, function editor, cameras, bones/skeletons, element transforms | 05_scene_planning.md |
| Effect elements and their parameters | 06_effects.md |
| Sound elements, sound editor, lip-sync, mouth positions per cell | 07_sound_lipsync.md |
| Library, templates, storyline and scene list, scene transitions | 08_library_storyline.md |
| Interactive playback, previews, import paths (images, movies, capture, scanner), export | 09_playback_export.md |
| Project package, persistence of elements, cells and drawings | 10_data_model_project_format.md |
| MCP server, generic tools, object addressing | 11_agent_interface_mcp.md; all command IDs are compiled into A_command_registry.md |

Conventions used in this section:

- Frames are 1-based integers. A frame range `from..to` is inclusive.
- "The selection" means the shared cell/element selection of the Timeline and Exposure Sheet (11.8). Every command
  that acts on the selection also has an explicit-target form (element ID(s) plus a frame range) for agents (17).
- Every command listed here is a registered command (17.4); the GUI reaches it through menus, context menus, toolbar
  buttons and key bindings, whose layout is a design task. Where this section says a command "must be reachable from"
  a context, that context must offer it.

---

## 1. Core concepts

### 1.1 Vocabulary

| Term | Meaning |
|---|---|
| **Scene** | One shot of the movie. Each scene owns its own elements, its own Timeline and its own Exposure Sheet. The Timeline and Exposure Sheet always show the *current* scene (the scene list is specified in 08). |
| **Element** (layer) | A named track in a scene. Every element is a **row** in the Timeline; cell-based elements are additionally a **column** in the Exposure Sheet. |
| **Cell** | The intersection of a cell-based element and a frame. A cell is either **blank** (empty) or holds a **label** that references one drawing, image or media item of that element. |
| **Drawing** | A piece of artwork (vector drawing, bitmap image or linked media item) owned by an element. A drawing has a **name** (shown as the cell label, e.g. `1`, `2`, `A`). One drawing can be referenced by any number of cells of its element. |
| **Drawing bank** | The set of all drawings owned by an element, whether or not they are currently exposed in any cell. |
| **Exposure** | A run of consecutive cells showing the same drawing. "Exposing a drawing for 3 frames" means three consecutive cells with the same label (a *hold*). |
| **Trackbar** (bar) | In the Timeline, the coloured bar showing over which frames an element exists (start frame and duration). |
| **Keyframe** | A frame where a peg, element, camera or effect parameter is explicitly set; shown as a symbol on the element's track (semantics in 05). |
| **Parent / child** | Elements can be attached to pegs, effects and skeletons (parents). A child inherits its parent's transformation or effect. The hierarchy is shown by indentation in the Timeline. |
| **Clone** | An element that shares its drawing bank with one or more other elements (its *clone group*) but has its own exposure and transformation. |

### 1.2 Two composition orders

A scene has two independent orderings of its elements:

1. **Exposure Sheet order** — the left-to-right column order of cell-based elements. It is used to composite the scene
   in drawing mode (Drawing View) and by previews launched from drawing mode (09). Left is back, right is front.
2. **Timeline order** — the top-to-bottom row order *within each hierarchy level*, plus the parenting. It is used by
   the Camera, Top and Side views (scene-planning mode), by scene previews and by export. A front/back depth offset
   set in scene planning can further change the stacking (05).

Reordering in one order does not change the other. **Design decision:** arrange commands invoked while the Exposure
Sheet has focus reorder Exposure Sheet columns; invoked from the Timeline or Camera view they reorder Timeline rows
within the same hierarchy level. The agent form of every reorder command names the order explicitly.

### 1.3 Per-element state

| State | Scope | Effect |
|---|---|---|
| Visible in Exposure Sheet | Exposure Sheet and Drawing View | A hidden column is not shown in the Exposure Sheet and its drawings are not shown in the Drawing View. |
| Visible in scene (show/hide) | Camera/Top/Side views, playback, render and export | A hidden element is not drawn in the scene-planning views and is **not rendered** in playback or export. |
| Locked | Camera/Top/Side views | A locked element cannot be selected or edited in the scene-planning views. |
| Collapsed | Timeline | The child rows of a peg, effect or skeleton are hidden. |
| Effect-parameter rows shown | Timeline | The parameter sub-rows (e.g. masks) below an effect element are shown or hidden. |
| Annotation | All | The element is an annotation layer (2.6). |
| Marker colour, background colour | Timeline and Exposure Sheet | Display only (5). |
| Element note | Timeline and Exposure Sheet | Free text shown on hover (7.1). |
| Column width | Exposure Sheet | Display only, persisted per element. |

Per-drawing state: **locked** (cannot be edited, 13.12). Per-cell data: **cell note** (7.2).

The two visibility flags are independent of each other.

### 1.4 Drawing mode and scene-planning mode

The application has a drawing-oriented mode (Drawing View, composited in Exposure Sheet order) and a scene-planning
mode (Camera/Top/Side views, composited in Timeline order). Pegs, cameras and effects only exist in scene planning.
When such elements are added while the user is in drawing mode, the application MUST warn that the new elements
cannot be seen or modified in drawing mode and that switching to scene-planning mode makes them visible. The warning
MUST be non-blocking for agents (returned as warning code `element.hiddenInDrawingMode`).

---

## 2. Element types

### 2.1 Summary

| Type | Exposure Sheet column | Timeline row | Can be a parent | Colour category (5.1) | Properties panel |
|---|---|---|---|---|---|
| **Drawing** (vector) | Yes | Yes | No | Drawings | Element panel + Cells panel |
| **Image** (bitmap) | Yes | Yes | No | Images | Element panel + Cells panel |
| **Sound** | Yes (sound column) | Yes (waveform track) | No | Sounds | Sound panel (07) |
| **Media** (linked files) | Yes | Yes | No | Media | Element panel |
| **Annotation** (note drawing) | Yes | Yes | No | Drawings | Element panel (type shown as Annotation) |
| **Peg** | No | Yes | Yes | Pegs | Peg panel |
| **Camera** | No | Yes | Yes (camera peg) | Cameras | Camera panel |
| **Skeleton effect** | No | Yes | Yes (bones, skin) | Skeletons and Bones | Skeleton / Bone panels |
| **Colour transform effect** | No | Yes | Yes | Effects | Colour transform panel |
| **Clipping effect** | No | Yes | Yes, plus effect-parameter (mask) rows | Effects / Effect parameters | — |
| **Drop shadow effect** | No | Yes | Yes | Effects | Drop shadow panel |
| **Blur effect** | No | Yes | Yes | Effects | Blur panel |
| **Shake effect** | No | Yes | Yes | Effects | Shake panel |
| **Natural-phenomena particle effect** | No | Yes | Yes (auto-created sprite drawing child) | Effects | Particle panel |
| **Action/impact particle effect** | No | Yes | Yes (auto-created sprite drawings child) | Effects | Particle panel |

Additionally a **parent peg** command creates a peg and attaches the selected elements to it (2.7). Bones are
created by the bone tools of section 05 and appear as rows under their skeleton.

Notes:

- Effect behaviour is specified in 06; this section covers only how effect elements behave as Timeline rows (bars,
  durations, loops, children, parameter rows).
- When a drawing, image, media, camera or effect element acts as a matte/mask (e.g. under a clipping effect), its
  row and column MUST show a distinct matte indicator.
- Rows that group sub-rows (effect groups, bone groups, skin groups) MUST be visually distinguishable from element rows.

### 2.2 Drawing element (vector)

Holds vector drawings made with the drawing tools or imported vector artwork.

- Each cell label refers to one vector drawing of the element's drawing bank. Drawings are stored in the project
  package (10).
- Drawing in the Drawing or Camera view while a **blank** cell of a drawing element is current MUST create a new
  drawing with the next free name (14.2) and expose it in that cell.
- A new scene starts with one drawing element and one camera already present.
- A drawing element can be converted to an annotation element and back (2.6).

### 2.3 Image element (bitmap)

Organises bitmap images (imported or captured).

- Cells reference bitmap images stored in the project package.
- The image-import commands of section 09 MUST be reachable from an image column and its cells, with the target
  element and frame preset: import images from files, import images with keying, import a movie file as an image
  sequence, acquire from a capture device, acquire from a scanner (and choose the scanner source).
- **Convert element to vector**: vectorises all bitmaps of the element into vector drawings (vectorisation options
  in 02). **Design decision:** convert in place — the element becomes a drawing element, drawing names and exposures
  are preserved; one undo step; runs as a job with progress.
- Where the GUI refers to a drawing of an image element (rename, substitution) it uses the word "image".

### 2.4 Sound element

Holds imported sound files; shown as a waveform track in the Timeline and as a sound column in the Exposure Sheet.
Full behaviour in 07. Timeline/Exposure Sheet aspects covered here: sound column commands (12.8), waveform display
toggle (11.11) and sound clip placement in the track (11.12).

### 2.5 Media element (linked files)

References external files instead of copying them into the project, to save disk space. It accepts the bitmap, vector
and sound formats supported by the application (at least JPEG, BMP, GIF, TGA, TIFF, SWF, WAV, AIFF, MP3) and elements
of templates.

- Cells show thumbnails of the linked media.
- For each media item the application MUST make available (hover info and query): name, file location, the frame range
  it occupies and its length, the media start frame and the media length.
- **Reset media**: restores the media element's exposure to the media's natural length. Enabled only when the media
  can be extended. One undo step.
- For media containing sound (e.g. templates with sound) play/stop sound commands MUST be reachable from the media
  column (commands in 07).

### 2.6 Annotation element

A vector drawing layer for corrections and written or drawn notes (teacher / art-director use).

- Created like any element (3); the Exposure Sheet and Timeline quick-add contexts MUST offer it.
- **Set as annotation element** turns a drawing element into an annotation element; **set as drawing element** turns
  it back. Each is one undo step.
- **Show all / hide all annotation elements** shows or hides all annotation elements in the Camera, Top and Side views.
- The Exposure Sheet column header MUST identify annotation columns as such.
- **Design decision:** annotation elements are excluded from final render and export by default (a per-export option
  in 09 MAY include them).

### 2.7 Peg

A transformation-only element used to move, rotate and scale its children over time. It has no cells or drawings; it
has a **start frame**, a **duration** and an optional **loop**; its keyframes are shown on its track (semantics in 05).

- **Add peg** adds an unattached peg.
- **Add parent peg** adds a peg *and attaches the selected element(s) to it*. The new peg is inserted at the level the
  selected elements occupied and its bar covers their combined frame range.

### 2.8 Camera

Adds a camera element (an alternative viewpoint). Any number of cameras may exist; one of them is the current camera
(05). Camera rows show start/duration bars and camera-peg keyframes.

### 2.9 Effect elements and effect-parameter rows

Effect elements apply an effect to their children over a frame range (start and duration), optionally looped when the
effect type supports looping.

- Children are attached by dropping rows onto the effect row in the Timeline or by the parenting commands (9.2).
- **Effect-parameter rows** are elements that appear below an effect and are required to build the effect, e.g. the
  mask elements of a clipping effect. They use their own colour category. Their display is controlled by three
  commands: show effect parameters of the selected clipping effect (enabled only when a clipping effect is selected),
  show all effect parameters, hide all effect parameters.
- Particle effects (natural-phenomena and action/impact presets, 06) automatically add a child sprite-drawing element
  holding the particle artwork.
- Effect keyframes are shown with a keyframe symbol on the effect track.
- A global display toggle for all effects is specified in 06.

### 2.10 Skeleton effect and bones

Skeleton elements contain bone rows; bone rows are created automatically when bones are drawn (05). Bone keyframes
appear on bone rows with distinct symbols. Skeleton and bone bars can be moved and stretched like peg bars (11.10).

---

## 3. Creating elements

### 3.1 Contexts

Element-creation commands MUST be reachable from:

- the main menu (every element type, plus the batch-creation command of 3.2);
- the Timeline layer list: an add control listing every element type, and quick-add controls for drawing element,
  peg, parent peg and effect;
- the Exposure Sheet: quick-add controls for the cell-based types (drawing, image, sound, media, annotation) and the
  context of the empty grid area;
- an effects palette/toolbar for the effect types;
- importing files (09), which creates an element of the appropriate type when needed.

Creation commands work regardless of which view has focus.

### 3.2 Batch creation ("new elements")

Parameters:

| Parameter | Default | Range | Effect |
|---|---|---|---|
| Type | Drawing | Any type of 2.1 | Type of the element(s) created. |
| Name | Default name for the type (3.3) | Non-empty, valid characters (4.2), unique in the scene, ≤ 64 characters | Base name of the new element(s). |
| Count | 1 | 1 – 99 | How many elements of that type to create. |

Validation errors (nothing is created): empty name (`element.nameEmpty`), forbidden character (`element.nameInvalid`,
the message lists the forbidden characters), name already used in the scene (`element.nameDuplicate`), name too long
(`element.nameTooLong`), count out of range (`element.countOutOfRange`), creation failure for any other reason
(`element.createFailed`), operation not permitted while editing a library template (`element.templateEditRestricted`,
11.15).

Behaviour:

- When count > 1 the elements receive unique names derived from the base name. **Design decision:** `Name`, `Name_1`,
  `Name_2`, …, skipping names already used.
- Creation is one undo step.
- The new element(s) become the selection and the first one becomes the current element.
- Placement: in the Timeline, directly above the selected row at the same hierarchy level (at the top of the root level
  if nothing is selected); in the Exposure Sheet, immediately to the right of the selected column (rightmost if none).
- Adding pegs, cameras or effects in drawing mode triggers the warning of 1.4.

### 3.3 Direct add commands and initial state

Each per-type add command creates one element of its type with a default name and no prompt, as one undo step.

Default names are derived from the type ("Drawing", "Image", "Peg", "Camera", the effect type name, …) with a numeric
suffix to stay unique in the scene. Sound elements use a sound-track style default name with a zero-padded counter.

| Type | Initial content |
|---|---|
| Drawing / Annotation / Image / Media | All cells blank; the element exists over the current scene length. |
| Sound | Empty; the sound panel indicates that no sound is loaded and offers import. |
| Peg / Camera / Effect / Skeleton | Bar starting at frame 1 for the scene length; no keyframes. Parent peg: bar covers the children's range. |

---

## 4. Naming and renaming elements

### 4.1 Rename element

- Reachable from the main menu, the Timeline layer context, the Exposure Sheet column-header context, by
  double-clicking a layer name in the Timeline and by double-clicking a column header in the Exposure Sheet.
- Parameter: the new name, prefilled with the current name in the GUI.
- Validation: same rules and error codes as 3.2. Renaming to the current name is a silent no-op.
- One undo step; the undo label names the element type.
- Renaming MUST NOT break any reference to the element's drawings (storage naming is defined in 10).
- Clones have their own element names; renaming one clone does not rename the others.

### 4.2 Naming rules (elements and drawings)

- Names MUST NOT be empty and MUST NOT contain any of `\ / : * ? < > " |` or `.`.
- Element names MUST be unique within the scene. Drawing names MUST be unique within the element's drawing bank.
- **Design decision:** maximum name length 64 characters.

---

## 5. Element colours

### 5.1 Default colour per type category (preference)

The preferences (01) MUST offer a colour per category:

| Category | Applies to | Recommended default |
|---|---|---|
| Drawings | Drawing and annotation elements | `#9DC3E6` (blue) |
| Images | Image elements | `#A9D18E` (green) |
| Sounds | Sound elements | `#F4B183` (orange) |
| Pegs | Peg elements | `#C9A0DC` (violet) |
| Skeletons and Bones | Skeleton and bone rows | `#E6B9B8` |
| Media | Media elements | `#FFE699` (yellow) |
| Cameras | Camera elements | `#BFBFBF` (grey) |
| Effects | Effect elements | `#F8CBAD` |
| Effect parameters | Effect-parameter rows (e.g. clipping masks) | `#FBE5D6` |

The storyline colours (scene, scene extended with blank frames to accommodate transitions, transition) are specified in
08. Window background colour preferences exist for the Timeline and for the Drawing View.

Changing a category colour affects every element that has not been given an individual marker colour.

### 5.2 Per-element colours

| Command | Behaviour |
|---|---|
| **Set marker colour** | Sets the marker colour of the selected element(s): Timeline trackbar and Exposure Sheet column marker. |
| **Set background colour** | Sets the colour drawn behind the element's trackbar in the Timeline (Timeline only). |
| **Default colour** | Reverts the marker colour of the selected element(s) to the category default. |

Reachable from the main menu, the Timeline layer context and the Exposure Sheet column-header context (marker and
default colour). Colours are stored per scene per element. Each change is one undo step.

---

## 6. Element properties

The Properties window (01) shows a panel appropriate to the selection. A separate **Cells** window shows the Cells
panel on its own.

### 6.1 Element panel (drawing, image, media, annotation)

Values apply to the **current frame**; changing a transform value creates or updates a keyframe on the element
(semantics and the transform commands in 05).

| Field | Editable | Meaning |
|---|---|---|
| Name | No | Element name. |
| Start | No | First frame of the element's bar. |
| Duration | No | Number of frames of the element's bar. |
| Velocity | — | Opens the function editor on the element's velocity function (05). |
| Type (composite role) | Yes | **Normal** \| **Foreground** \| **Background** \| **Envelope** (one undo step). For annotation elements the panel reports the type as annotation. |
| Rotation | Yes | Rotation angle in degrees. |
| Skew | Yes | Skew value. |
| Offset EW / NS / FB | Yes | Position offset east-west, north-south, front-back, in the scene-planning coordinate unit (fields, inches or centimetres, preference in 05). |
| Scale EW / NS | Yes | Horizontal / vertical scale. |
| Keep proportions | Yes | Locks EW and NS scale together. |
| Rotation pivot EW / NS | Yes, when "use rotation pivot" is on | Pivot used for rotation. |
| Use rotation pivot | Yes | Enables a separate rotation pivot. |
| Scale pivot EW / NS | Yes | Pivot used for scaling. |
| Orient to path | Yes | Element rotation follows the motion-path direction. |
| Deformable | Yes | Whether the element can be deformed by skeleton bones (one undo step). |

Composite roles (**Design decision**, recommended semantics):

- *Normal*: regular element.
- *Foreground* / *Background*: the element composites in front of / behind all Normal elements regardless of order;
  an indicator is shown in its Exposure Sheet column header. Background elements automatically extend their last
  exposure to the scene length.
- *Envelope*: the element is used as a deformation envelope (skin) for bones; the Timeline shows an envelope on/off
  indicator on its row.

Text fields commit on confirm or focus loss. Invalid numeric input is rejected and the previous value restored.

### 6.2 Cells panel (drawing substitution)

Shows and changes **which drawing is exposed** in the selected cell(s) of a drawing, image, media or annotation
element.

| Field | Behaviour |
|---|---|
| Element name | Element of the selected cell(s). |
| Frame label | The frame of a single selected cell, or the first and last frame of a selected range. |
| Preview | Thumbnail of the drawing currently in the cell; shown only when "show preview" is on. |
| Show preview | Toggles the preview. |
| Drawing name | Name of the exposed drawing. Entering another existing drawing name substitutes that drawing in the selected cell(s). An unknown name reports `cell.drawingNotFound`. One undo step. |
| Drawing slider | Steps through all drawings of the element's drawing bank in bank order; moving it substitutes the drawing in the selected cell(s) live, and the name field and preview follow. One undo step per drag (press to release). |

The panel is disabled when the selection is not a cell of a drawing, image, media or annotation element. It refreshes
when the current frame changes.

### 6.3 Peg, camera and sound panels (summary)

- **Peg panel**: name, start, duration, velocity (function editor), loop mode (not looped, or the number of frames the loop spans),
  scale EW/NS with keep proportions, skew, rotation, pivot application (rotation / scaling / both), constant Z,
  orient to path. Semantics in 05.
- **Camera panel**: name, resolution, offset EW/NS/FB, field-of-view type (static with value, or dynamic). See 05.
- **Sound panel**: sound element name (or an indication that no sound is loaded), import, edit sound and lip-sync
  commands. See 07.

The properties of library templates (type, size, author, contents with element type and frame count) are specified in 08.

---

## 7. Element notes and cell notes

### 7.1 Element note

- **Set element note** is reachable from the main menu, the Timeline layer context and the Exposure Sheet
  column-header context. When the element already has a note, the command is presented as "update element note" and
  the editor opens prefilled.
- Parameter: multi-line text. Setting empty text removes the note.
- Display: a note indicator on the Timeline row and the Exposure Sheet column header; hovering shows the text.
- One undo step.

### 7.2 Cell note

- **Set cell note** is reachable from the main menu (cell commands) and the drawing and image cell contexts; presented
  as "update cell note" when a note exists.
- Notes are stored per element per frame.
- Display: a small note indicator in the cell; hovering shows the text; double-clicking the indicator edits the note.
- A **show notes** display option shows or hides all element and cell notes in the Exposure Sheet (12.4).
- One undo step.

---

## 8. Clone, duplicate and delete

### 8.1 Clone element

- Reachable from the main menu, the Timeline layer context and the Exposure Sheet column-header contexts (drawing,
  image, media, annotation and sound columns).
- Creates a new element linked to the original: the **drawing bank is shared**. Editing a drawing in any clone updates
  all clones; a new drawing created in one clone is available to all clones. Exposure (timing) and transformation are
  **independent** per clone.
- Typical use: reuse the same material several times in a scene.
- Allowed only for cell-based elements; otherwise `element.notCloneable`.
- A clone of a clone joins the same clone group.
- Name: original name plus a unique suffix (3.2 rule).
- One undo step.

### 8.2 Duplicate element

- Reachable from the main menu, the Timeline layer context and the column-header context of drawing, image, media and
  annotation columns.
- Creates an independent copy: a copy of every drawing (new drawing bank), of the exposure and of the transformation.
  The copy is not linked to the original. In the Camera view the copy appears at exactly the same place.
- One undo step.

### 8.3 Delete element

- Reachable from the main menu, the Timeline layer context and the Exposure Sheet column-header contexts.
- Removes the selected element(s) from the scene.
- The element's drawings are deleted **unless** still used by another element (a remaining clone keeps the shared bank).
  Deleting the last clone of a group deletes the bank if nothing else uses it.
- Children of a deleted peg, effect or skeleton are re-attached to the deleted element's parent (or the root level),
  keeping their order, rather than deleted.
- One undo step; undo restores the elements, their drawings and the children's parenting. Drawing storage is only
  physically purged when the project is saved (14.6).
- The generic delete command (01) with rows selected in the Timeline deletes the elements; with cells selected it
  deletes cells (13.17).

---

## 9. Arranging and hierarchy

### 9.1 Stacking order

| Command | Behaviour |
|---|---|
| **Bring to front** | Moves the selected element above all other elements of the same hierarchy level (same parent). |
| **Bring forward** | Moves it one position up within the same level. |
| **Send backward** | Moves it one position down within the same level. |
| **Send to back** | Moves it below all other elements of the same level. |

Reachable from the main menu, the Timeline layer context, the Camera view context and the Exposure Sheet column-header
contexts. Each is one undo step. Commands are disabled at the boundary (already front/back). In the Exposure Sheet the
column order changes accordingly (left = back, right = front). Scope of each order: 1.2.

Drag reordering:

- **Timeline**: dragging a row and dropping it *between* two rows reorders it (possibly moving it to another level);
  dropping it *onto* a peg/effect/skeleton row attaches it (9.2).
- **Exposure Sheet**: dragging a column header horizontally reorders columns; dragging items in the element list panel
  (12.6) does the same.

### 9.2 Parenting

| Command / gesture | Behaviour |
|---|---|
| Drop row(s) onto a peg/effect/skeleton row | Attaches the dropped rows as children; they are shown indented. |
| **Attach to next object** | Attaches the selected element to the nearest peg or effect above it at the same level. |
| **Detach from parent** | Detaches the element from its parent; it moves to the parent's level, directly above the parent. |
| **Add parent peg** | Creates a peg and attaches the selected element(s) to it (2.7). |

Rules:

- Valid parents: pegs, effects, skeletons (bones) and cameras (camera peg). Drawing, image, media, sound and annotation
  elements cannot have children (`element.notParentable`).
- An element cannot be attached to itself or to one of its descendants (`element.hierarchyCycle`).
- After attaching, the child keeps its own (local) transformation values and additionally follows the parent.
- Each parenting change is one undo step.

### 9.3 Hierarchy navigation

| Command | Behaviour |
|---|---|
| **Select parent** | Selects the element the current element is attached to. |
| **Select children** | Selects the peg/effect together with all elements attached to it. |
| **Select child** | Selects the first element attached directly under the selected peg/effect. |
| **Select previous sibling** | Selects the sibling above the current element under the same parent. |
| **Select next sibling** | Selects the sibling below the current element under the same parent. |
| **Select skeleton root** | Selects the root of the current skeleton (05). |

Each is disabled when the target does not exist. Reachable from the main menu, the Camera view context and the
Timeline layer context. Selection changes are not undoable.

### 9.4 Collapse / expand

- Shows or hides the rows attached to a peg, effect or skeleton in the Timeline; also by clicking the disclosure
  control left of the parent row.
- A collapsed parent shows a **mini-bar** summarising its children; when any child has a keyframe at a frame, a
  distinct marker is drawn on the parent row at that frame.
- The collapsed state is saved with the scene.
- Adding a keyframe to a collapsed parent keys all elements of the collapsed group (05).

---

## 10. Visibility, solo, lock and display toggles

### 10.1 Scene visibility

| Command | Behaviour |
|---|---|
| **Show/hide** | Sets the scene visibility of the selected element(s) (Camera/Top/Side views, playback, render). |
| **Show all** | Shows all elements. |
| **Hide all others** | Hides all unselected elements. |
| **Show all others** / **Hide all** | Available in the Timeline layer context. |

Also via a per-row visibility control in the Timeline layer list and a show/hide-all control in the layer-list
toolbar. Hiding a parent hides its children; a child hidden only through its parent shows a distinct "hidden by
parent" state. Each change is one undo step.

Preference **selection always visible** (05): elements selected in the Timeline or Exposure Sheet are displayed in the
scene views even if hidden.

### 10.2 Exposure Sheet visibility

| Command | Behaviour |
|---|---|
| **Hide in Exposure Sheet** | Hides the selected element's column; its drawings are also not shown in the Drawing View. |
| **Show all Exposure Sheet elements** | Shows all columns. |
| **Hide all other Exposure Sheet elements** | Hides all unselected columns. |

Reachable from the main menu, the column-header context and the visibility control of each item in the element list
panel (12.6). Each change is one undo step. If the current element is hidden, the current element moves to the next
visible column.

### 10.3 Solo mode

A Timeline toggle **solo mode**: while on, only the selected element(s) (with their parents) are displayed in the
scene-planning views; turning it off restores the previous visibility. Solo mode is a view state; it does not change
the elements' stored visibility and is not undoable.

### 10.4 Lock

| Command | Behaviour |
|---|---|
| **Lock** | Prevents the selected elements from being selected or edited in the scene-planning views. |
| **Unlock** | Allows scene-planning edits again. |
| **Lock all others** | Locks all elements except the selection. |
| **Lock all** | Locks all elements. |
| **Unlock all** | Unlocks all elements. |

Reachable from the main menu, the Timeline layer context and the Camera view context (lock, unlock all). Locked rows
show a lock indicator in the Timeline. Locked elements can still be edited in the Drawing View and their exposure
changed in the Timeline and Exposure Sheet. Typical use: protect a rough layer while tracing over it. Each change is
one undo step.

### 10.5 Other display toggles relevant to the Timeline

| Toggle / command | Behaviour |
|---|---|
| **Sound waveforms** | Sound rows show a waveform (on) or a solid trackbar (off). |
| **Show Timeline selection** | Scrolls the Timeline so that the element selected in any view is visible. |
| **Effect parameters** (selected / show all / hide all) | 2.9. |
| **Show all / hide all annotation elements** | 2.6. |
| **Show all pegs** | Forces pegs to show in the scene-planning views regardless of their visibility (05). |
| **Current drawing on top** | The drawing being edited in the Camera view is drawn on top regardless of order (02). |

---

## 11. Timeline view

### 11.1 Purpose and access

The Timeline is used to add and lay out the elements of the current scene (drawings, images, sounds, media, pegs,
cameras, effects, skeletons) and to change the timing of all elements.

- Shown/hidden through the window commands of 01; docked at the bottom of the main window in the default workspace.
- The window title shows the current scene name and the current camera name.
- Background colour: preference (5.1).

### 11.2 Components

| Component | Requirements |
|---|---|
| Layer-list toolbar | Controls for: add element (list of all types), add drawing element, add peg, add parent peg, solo mode (toggle), show/hide all, and a control opening the Timeline context commands. |
| Layer list (left pane) | One row per element. Row content: disclosure control (elements with children), indentation by hierarchy depth, type icon, element name, visibility control, lock indicator, note indicator, colour marker. Fixed row height. |
| Frame ruler | Frame numbers (or feet and frames, 11.4). Clicking or dragging sets the current frame. Shows the playback-range markers when the range is free. Has its own context commands (11.9). |
| Playhead | A vertical line/handle at the current frame across all tracks. Dragging it scrubs; with sound scrubbing on (09) sound is heard while dragging. |
| Playback-range markers | Two draggable markers at the start and end of the free playback range; hidden and locked when the range is automatic. |
| Tracks (right pane) | One track per row: trackbar, cells, exposure/drawing symbols, keyframe symbols, waveforms. |
| Zoom control | Stepwise zoom of the cell width. |
| Scroll bars | Horizontal (frames) and vertical (rows); the layer list and tracks scroll vertically together. |
| Splitter | Adjustable divider between layer list and tracks. |

### 11.3 Row rendering

| Row type | Track content |
|---|---|
| Drawing / image / annotation element | Trackbar in the element colour over its exposure range; each cell that starts a new exposure shows a drawing symbol; held frames are drawn as a continuation; re-use of a drawing exposed earlier is marked with a distinct repetition symbol; blank cells are empty. Symbols distinguish vector from bitmap content. |
| Media element | Track showing the media's linked range. |
| Sound element | Waveform (or solid bar when waveforms are off); each sound file is a clip that can be dragged along the track (11.12). |
| Peg | Bar from start for its duration; keyframe symbols per channel; segments between keyframes drawn as a line when interpolated (non-constant). |
| Camera | Camera bar with camera-peg keyframes. |
| Effect | Effect bar (start/duration, stretchable) with effect keyframe symbols; may contain child groups. |
| Skeleton / bone | Bar with bone keyframe symbols. |
| Group row | Grouping row (effect group, bone group, skin group). |

Keyframe symbols MUST distinguish the four transform channels — **position (motion)**, **scale**, **rotation**,
**skew** — and MUST support combined symbols when several channels have keyframes on the same frame. A segment drawn
as a line between keyframes means interpolated (non-constant); no line means constant. A parent row (and a collapsed
parent's mini-bar) shows a distinct marker at frames where a child has a keyframe. Exact glyph design is an
implementation choice. Keyframe semantics: 05.

Row states that MUST be visually distinguishable: selected, current, child-selected (parent highlighted when a child is
selected), excluded/hidden (dimmed), locked, collapsed, drag preview, has note.

Double-clicking a row name renames the element (4.1).

### 11.4 Time measurement (frames vs feet and frames)

A preference selects the time display of the ruler, the Exposure Sheet frame column and frame read-outs:

- **Frames** — frame numbers only.
- **Feet and frames** — the equivalent film-footage measure is also displayed. **Design decision:** 16 frames per
  foot (35 mm), displayed as `feet+frames` with two-digit frames, e.g. frame 20 → `1+04`.

Commands and agents always use integer frame numbers regardless of the display setting.

### 11.5 Zoom and scrolling

- Zoom in / zoom out (the generic view-zoom commands of 01 act on the Timeline when it has focus) change the cell width
  in discrete steps; at low zoom one cell may represent several frames.
- The zoom control has discrete steps; the mouse wheel over it changes the step.
- Mouse wheel over the tracks scrolls vertically; with the horizontal-scroll modifier held it scrolls horizontally.
  Dragging near an edge auto-scrolls.
- **Show Timeline selection** scrolls so the selected element is visible.
- During playback the view follows the playhead.

### 11.6 Current frame and playback range

| Operation | Behaviour |
|---|---|
| Set current frame | Click in the ruler or on a track cell, drag the playhead, or enter a frame number (command parameter). |
| Previous / next frame | Moves the current frame by one. |
| Previous / next drawing | Moves the current frame to the previous/next cell of the current element where a different drawing starts. |
| Playback range: **automatic** | Playback covers the whole scene. |
| Playback range: **free** | Two markers appear at the start and end of the range; dragging them, or entering a start/end frame, defines the range. |
| Set playback start / end frame | Integer parameters; meaningful only when the range is free. |

Rules: start ≥ 1, end ≥ start, end ≤ scene length; out-of-range values are clamped and the clamped values returned.
Reachable from the main menu (playback commands) and the ruler context. Playback behaviour: 09. The current frame is
shared by all views.

### 11.7 Scene length

- The scene length is the last frame used by any element (exposures, bars, sounds).
- The Timeline and Exposure Sheet display empty frames beyond the scene end so that cells there can be selected and
  drawn into; exposing a drawing beyond the current end extends the scene.
- Removing exposures at the end shortens the scene.
- Storyline transitions may extend a scene with blank frames (08).

### 11.8 Selection model

| Gesture | Result |
|---|---|
| Click a row name | Selects that element and makes it the current element (not undoable). |
| Extend / toggle modifier + click on row names | Extends / toggles a multi-selection of rows. |
| Click a cell in a track | With the preference **single cell selection** on: selects that cell. With it off: selects the whole track of that row; holding the single-cell modifier while clicking selects one cell. |
| Drag across cells | Selects a frame range, optionally spanning several rows (rectangular selection). |
| Extend modifier + click a cell | Extends the cell range to that cell. |
| Select all (01) | In scene-planning mode: selects all elements in the Timeline. |
| Deselect all (01) | Clears the selection. |
| Select an element in the Camera/Top/Side view | Selects its row in the Timeline. |

The Timeline and Exposure Sheet share one cell selection and one current position (current element + current frame),
so a selection in one is reflected in the other.

### 11.9 Timeline command contexts

The Timeline offers context commands (right-click, or the toolbar control) depending on what is under the pointer.
Inapplicable commands are disabled or omitted. Required coverage:

| Context | Commands that MUST be reachable |
|---|---|
| Row (layer) | All element-creation commands; image import, import-and-vectorise, vector/PDF artwork import, sound import, movie-as-image-sequence import (09); cut, copy, paste, delete; arrange (9.1); collapse/expand; hierarchy selection (9.3); attach to next object, detach from parent; visibility (10.1); marker/background/default colour; effect-parameter display; set as annotation / set as drawing element; show/hide all annotation elements; lock commands (10.4); change start frame, duration and loops (11.10); clone, duplicate, delete, rename element; set element note; peg reset, remove all keyframes, toggle constant Z, add default keyframe (05); edit sound, sound waveforms (07). |
| Cells (track) | Add/remove exposure; set exposure to 1/2/3/N; extend exposure (fill); extend children exposure (pegs); insert cells; insert blank cell; delete blank cell; create cycle; columnate (advanced cycle); duplicate drawing; add/remove keyframe, set constant/non-constant segment, uniform keyframe, add default keyframe (05); cut, copy, paste, delete; edit/split/fit sound (07). |
| Ruler / empty track area | Set current frame; playback range automatic / free; set playback start / end frame; sound waveforms; sound playback and sound scrubbing (09). |

Toggle commands MUST present their current state (checked state or on/off wording).

### 11.10 Timing operations on bars (pegs, cameras, effects, skeletons, bones)

| Operation | Parameters / gesture | Rules |
|---|---|---|
| **Change start frame** | New start frame (integer ≥ 1) | Moves the element's bar and its keyframes to start at the given frame. |
| **Change duration** | New duration (integer ≥ 1) | Number of frames the peg, camera or effect lasts. Keyframes keep their frames; keyframes beyond the new end are removed (**Design decision**). |
| **Change loops** | Number of loops (integer ≥ 1; 1 = not looped) | Repeats the element's animation N times; the peg panel then shows the looped span in frames. Only for element types that can loop (`element.notLoopable` otherwise). |
| Move bar | Drag the bar body horizontally | Changes the start frame. With several rows selected, all move together. |
| Stretch from end | Drag the end of the bar (the pointer indicates a horizontal resize) | Changes the duration; keyframes are scaled proportionally. |
| Stretch from start | Drag the start of the bar | Changes start and duration keeping the end. |
| Loop handle | Drag the loop handle | Adjusts the loop extent. |
| **Extend children exposure** | Target frame (as 13.5) | Extends the exposure of the elements attached to the selected peg up to the given frame. |

Each operation is one undo step (a drag is one step from press to release). For cell-based elements the "bar" is the
set of exposed cells; dragging a cell selection moves exposures (11.13).

### 11.11 Sound rows

- **Sound waveforms** on: a waveform is drawn in the track; off: a solid trackbar.
- Waveforms are computed asynchronously; a progress indication is shown meanwhile, and the view MUST stay responsive.
- Double-clicking a sound clip opens the sound editor (07).

### 11.12 Sound clip placement

Each imported sound is a clip in its sound track:

- Dragging a clip horizontally changes its start frame; clips in one track MUST NOT overlap (the drag stops at the
  neighbouring clip).
- A ghost clip shows the drop position while dragging. Dropping a sound from the library or the file system onto a
  track creates a clip at the drop frame.
- The track's extent adjusts to fit its clips.
- Selecting a clip and deleting removes it (one undo step).
- Dropping where a clip already starts requires confirmation to overwrite (agent form: an `onConflict` parameter of
  the sound commands in 07).

### 11.13 Drag and drop in the Timeline

| Drag source → target | Result |
|---|---|
| Row → between two rows | Reorder (same or different level). |
| Row → onto a peg/effect/skeleton row | Attach as child. |
| Selected cells → another frame or another row of a compatible type | **Move**, **copy** or **link** the exposures. Behaviour is governed by the drag-and-drop transfer options (11.14). **Design decision:** no modifier = move, copy modifier = copy, link modifier = link (target cells reference the same drawings; only within the same element or clone group). |
| Library template → track or row list | Inserts the template content (new elements, or drawings into the target element) at the drop frame; same-named drawings are handled per the transfer options and the update-drawings flow (14.7). |
| Cells or rows → Library | Creates a new template from the selection (08). |
| External files → Timeline | Imports the files (09). Errors for individual files are collected and reported together after the drop. |

### 11.14 Transfer options (paste special, cut special, drag and drop)

One option set exists for each of three operations — **paste special**, **cut special** and **drag and drop** — each
persisted across sessions. Paste special and cut special (generic edit commands of 01) apply the corresponding set
when the Timeline has focus; **paste special again** repeats the last paste special with the same options without
prompting. Options irrelevant to an operation are disabled.

Drawings group:

| Option | Default | Effect |
|---|---|---|
| **Overwrite existing drawings** | Off | When adding exposures to a drawing or image element, drawings with the same names as the transferred drawings are overwritten. When off (or no same-named drawing exists), new drawings are created. |
| **Create/delete exposure** | On | Paste adds cell labels and cut removes them (following cells shift). When off, the destination timing is not changed; only drawing content and properties are transferred. |
| **Automatically extend the exposure** | Off | Extends the exposure of transferred drawings to fill the range of cells up to the destination frame. |
| **Update colours** | Off | When transferring templates, updates the colours of the current palette with those from the template (03). |
| **Keyframe substitution** | Off | Replaces the keyframe properties at the current frame and in all following frames in sequence. |

Pegs group:

| Option | Default | Effect |
|---|---|---|
| **Create/delete motion keyframes** | On | Paste copies (cut removes) motion keyframes. |
| **Create/delete rotation keyframes** | On | Same for rotation. |
| **Create/delete scale keyframes** | On | Same for scale. |
| **Create/delete pivot keyframes** | On | Same for pivot. |
| **Create/delete velocity keyframes** | On | Same for velocity. |
| **Create/delete skew keyframes** | On | Same for skew. |
| **Create additional keyframes at start and end of the selection** | Off | Adds a keyframe at the start and end of the transferred range so that the surrounding animation is preserved. |

The peg-property paste-special used in the Camera view is specified in 05.

### 11.15 Restrictions

- Some element-structure operations (adding cameras, cloning or duplicating elements, adding media elements) are
  refused while a library template is being edited (`element.templateEditRestricted`).
- Paste is refused while bone deformation is being computed (`cell.busy`); the operation MAY be retried when the
  computation ends.
- Commands that require a (vector) drawing element (e.g. lip-sync mapping in 07, motion recording in 05) report
  `element.notDrawing` / `element.notVectorDrawing` when the selection or target is not such an element.

---

## 12. Exposure Sheet view

### 12.1 Purpose and access

The Exposure Sheet organises and times the contents (drawings, images, sounds, media) of elements in the traditional
exposure-sheet layout: **frames run vertically, elements are columns**.

- Shown/hidden through the window commands of 01; the title shows the current scene name.
- Only cell-based elements (drawing, image, sound, media, annotation) appear as columns. Pegs, cameras, effects and
  skeletons do not.
- The Exposure Sheet toolbar can be shown or hidden.

### 12.2 Components

| Component | Requirements |
|---|---|
| Toolbar | Quick-add for drawing, image, sound, media and annotation elements; thumbnails toggle; static light table panel toggle; element list panel toggle; scene selector; a control opening the context commands. |
| Element list panel (optional, left) | List of all cell-based elements of the scene (12.6). |
| Frame-number column | Leftmost fixed column with frame numbers 1..N (or feet and frames, 11.4); the current frame row is highlighted. |
| Column headers | Per element: name, type icon, foreground/background indicator, annotation indicator, colour marker, note indicator. Double-click renames the element; context commands per column type (12.8); dragging the border changes the width; dragging the header reorders. |
| Grid | Cells; vertical and horizontal scrolling; auto-scroll when dragging near an edge. |
| Static light table panel (optional, right) | Section 15. |

The shown/hidden state of the element list and static light table panels is remembered across sessions.

### 12.3 Cell display

| Cell content | Display |
|---|---|
| Blank cell | Empty. |
| First cell of an exposure | The drawing name. With the **element name** option on, the element name is prefixed: `<element>-<drawing>`. |
| Held cells of the same exposure | With the **exposure** option on, the drawing name is repeated in every held cell; otherwise a vertical hold line is drawn. |
| Thumbnails on | A small image of each drawing/image is shown in its cell; thumbnails are computed in the background with a progress indication. Rows become taller. |
| Locked drawing | Lock indicator. |
| Cell with note | Note indicator. |
| Sound column | Waveform segment per frame; when lip-sync is shown, the mouth letter (A–G, X) per frame (07). |
| Media column | Thumbnail of the linked media. |
| Selected cells | Highlight; focus rectangle on the current cell. |
| Current frame | Row highlight following the playhead. |

The column header marker (and optionally a cell background tint) uses the element colour (5).

### 12.4 Display options

| Option | Type | Effect |
|---|---|---|
| **Thumbnails** | Toggle | Shows/hides small versions of each drawing in cells. |
| **Exposure** | Toggle | Shows/hides the drawing name in every held frame. |
| **Element name** | Toggle | Shows/hides the element name prefix in cell labels. |
| **Notes** | Toggle | Shows/hides element and cell note indicators. |
| **Reset column width** | Command | Returns all column widths to the default. |

All display options are persisted and reachable from the grid context.

### 12.5 Selection and navigation

| Gesture | Result |
|---|---|
| Click a cell | Selects the cell and makes its frame and element current. |
| Drag / extend modifier + click | Rectangular range of cells (frames × columns). |
| Click a column header | Selects the element (whole column). |
| Arrow keys | Move the current cell up/down (frame) and left/right (element). |
| Previous / next frame | Global frame stepping (11.6). |
| Previous / next drawing | Jump to the previous/next cell where a different drawing starts. |
| Previous / next element | Moves the current element left/right (in the Exposure Sheet or Timeline, depending on focus). |
| Previous / next element in Exposure Sheet | Moves the current element among Exposure Sheet columns regardless of focus. |
| Typing in a cell / double-click a cell | Opens an in-cell editor to type a drawing name (change label, 13.13). |

**Design decision:** the generic deselect-all command (01) also clears an Exposure Sheet cell selection.

### 12.6 Element list panel

- Lists the scene's cell-based elements in Exposure Sheet order with name and a visibility control.
- The visibility control sets the element's Exposure Sheet visibility (10.2).
- Dragging an item to another position reorders the columns (one undo step).
- Selecting an item selects the element.

### 12.7 Scene selector

The Exposure Sheet toolbar has a scene selector choosing which scene is current and therefore shown (scene list
semantics in 08).

### 12.8 Command availability per column context

Column-header contexts (drawing, image, media, annotation columns): arrange (9.1); hide, show all, hide all others
(10.2); marker colour and default colour; cut and copy of the whole element; clone, duplicate, delete, rename element;
set element note; send to static light table (sends the element's current drawing). Sound column header: the same
without duplicate, plus edit sound; when lip-sync is shown, also modify lip-sync mapping (07).

Grid empty area: add drawing/image/sound/media/annotation element; display options (12.4); import file (09).

Cell contexts:

| Column type | Commands that MUST be reachable |
|---|---|
| Drawing / annotation | Add/remove exposure; set exposure to 1/2/3/N; extend exposure; insert cells; insert blank cell; delete blank cell; create cycle; columnate; duplicate drawing; clear drawing; lock/unlock drawing; import vector/PDF artwork, import-and-vectorise from file, capture device or scanner (09); cut, copy, paste, paste as new drawings, delete; change label; rename drawing; set cell note; send to static light table. |
| Image | Import movie as image sequence; import images (from file, with keying, capture device, scanner) (09); convert element to vector; add/remove exposure; set exposure to 1/2/3/N; extend exposure; insert cells; insert blank cell; delete blank cell; create cycle; columnate; cut, copy, paste, paste as new drawings, delete; change label; rename image; set cell note; send to static light table. |
| Media | Add/remove exposure; set exposure to 1/2/3/N; extend exposure; insert cells; insert blank cell; delete blank cell; create cycle; columnate; reset media; cut, copy, paste, delete; play/stop sound (07). |
| Sound | Import sound file; insert cells; insert blank cell; delete blank cell; select whole sound, select from the current frame back to the sound's start, select from the current frame to the sound's end; play, stop; cut, copy, paste, delete; delete sound; show/hide lip-sync; recompute lip-sync (07). |
| Sound with lip-sync shown | As sound, plus set the mouth position (A–G, X) of the cell (07). Recomputing lip-sync overwrites manual mouth changes. |

Commands are enabled only when applicable: paste only with clipboard content of a compatible type; delete blank cell
only on a blank cell; lock drawing only on a non-blank drawing cell; convert to vector only for image elements.

### 12.9 Drag and drop in the Exposure Sheet

| Drag | Result |
|---|---|
| Selected cells → other cells of a compatible column | Move / copy / link exposures (same modes and transfer options as the Timeline, 11.13–11.14). |
| Bottom edge of an exposure | Lengthens or shortens the exposure of that drawing (following cells shift). |
| Column header → other position | Reorders columns. |
| Library item → column or cell | Inserts template drawings or elements at that frame (08, 14.7). |
| External files → grid | Imports the files; per-file errors are collected and reported together. |
| Cells → static light table panel | Adds those drawings to the static light table (15). |

---

## 13. Cell and exposure commands

General rules for all commands in this section:

- They act on the current cell selection (Timeline or Exposure Sheet) of cell-based elements unless stated otherwise;
  each also has an explicit-target form (element ID(s) + frame range) for agents (17).
- They are reachable from the main menu (cell commands) and from the cell contexts of 11.9 and 12.8; availability per
  element type is given in 13.20.
- Each invocation is exactly one undo step.
- With a selection spanning several elements, the command is applied to each element independently.
- "Shift" means: all cells of that element after the affected position move later (insert) or earlier (remove) by the
  stated number of frames. Other elements are never shifted unless stated.

### 13.1 Add exposure

Adds one frame to the exposure at the selected frame: the drawing at the current cell is held one frame longer and
the following cells of that element shift later by one frame.

- With a multi-cell selection: one frame is added to each distinct exposure that intersects the selection.
- On a blank cell: inserts one blank frame (equivalent to 13.7).
- On a peg: lengthens the peg's bar by one frame.
- Agent form takes a frame count (default 1).

### 13.2 Remove exposure

Removes one frame from the exposure containing the selected frame; following cells shift earlier by one. If the
exposure is one frame long, the cell is removed. The drawing always stays in the drawing bank. On a peg: shortens the
peg's bar by one frame (minimum 1).

### 13.3 Set exposure to 1, 2 or 3

Every distinct exposure that intersects the selection is re-timed to N frames ("on ones / twos / threes"); the
following cells of the element shift to close or open the gap so that the sequence of drawings is preserved.
Example: `1 1 2 2 3 3` set to 1 → `1 2 3` (the rest of the column moves up 3 frames). **Design decision:** blank runs
inside the selection keep their length.

### 13.4 Set exposure to N

As 13.3 with a numeric parameter: frames (integer 1 – 9999; GUI default = current length of the exposure at the
current cell, else 1).

### 13.5 Extend exposure (fill)

Exposes the selected drawing up to and including a target frame.

- Parameter: target frame (integer; minimum = the first frame of the selection; maximum 9999; GUI default = current
  frame).
- A target below the first selected frame reports `cell.frameBeforeSelection` (the message states the minimum).
- Cells between the end of the current exposure and the target frame are **overwritten** with the selected drawing
  (not inserted); later cells are not shifted.
- Enabled only when the selection contains a drawing that can be extended.
- The same parameter is used by **extend children exposure** on pegs (11.10).

### 13.6 Insert cells

| Parameter | Default | Effect |
|---|---|---|
| Number | 1 (1 – 9999) | Number of cells to insert. |
| Named cells | Off | When on, each inserted cell gets a new empty drawing named by incrementing from the element's highest numeric drawing name (e.g. after `5`: `6`, `7`, …). When off, blank cells are inserted. |
| In all elements | Off | Inserts the given number of **blank** cells in **every** cell-based element of the scene (keeps all columns in sync). When on, named cells is unavailable. Pegs, cameras and effects are not shifted. |
| Position | After | Insert **before** or **after** the selected cell. |

Following cells shift later by Number frames.

### 13.7 Insert blank cell

Inserts one blank frame at the selected frame; the current and following cells shift later by one.

### 13.8 Delete blank cell

Removes the selected **blank** cell; following cells shift earlier by one. Unavailable (`cell.notBlank`) if the
selected cell is not blank.

### 13.9 Create cycle

Parameter: **number of cycles** (integer 1 – 999; 1 = no change) = how many times the selected cell range appears in
total. The copies (with their exposures) are inserted directly after the selection and following cells shift later.
Example: `1 1 2 2 3 3` with 3 cycles → `1 1 2 2 3 3 1 1 2 2 3 3 1 1 2 2 3 3`. Blank cells inside the selection are
part of the repeated pattern.

### 13.10 Columnate (advanced cycle)

Writes a run of the element's drawings into the column in a pattern, starting at the first selected cell.

| Parameter | Default | Effect |
|---|---|---|
| First drawing | First drawing of the selection | Picked from the element's drawing bank (slider in bank order, or by name; the GUI shows a thumbnail and the drawing's content type). Unknown name → `cell.drawingNotFound`. |
| Last drawing | Last drawing of the selection | As above. |
| Expose each drawing for | 1 frame (or the current exposure) | Frames each drawing is exposed. |
| Expose last drawing for | Same as "each" | Frames the final drawing of the whole cycle is exposed. |
| Loop mode | **None** | **None**: one pass. **Continuous**: the run restarts from the first drawing each loop (1-2-3-1-2-3). **Forward-backward**: each loop goes forward then backward without repeating the end drawings (2 loops of 1-2-3 → 1-2-3-2-1-2-3-2). |
| Loops | 1 | Number of repetitions; editable only for continuous and forward-backward. |
| Total frames | Computed | Total frame count of the cycle including exposures. Editing it recomputes loops and vice versa. |
| Placement | **Overwrite** | **Overwrite**: the cycle overwrites any cells it overlaps. **Insert**: the cycle is inserted at the selected cell and following cells shift later. |

Computation — let D be the drawings from first to last in drawing-bank order (reversed when last precedes first),
n = |D|, e = expose-each, l = expose-last:

- None: frames = (n − 1)·e + l.
- Continuous, k loops: D repeated k times; every drawing is exposed e frames except the final drawing of the whole
  cycle, which uses l; frames = (k·n − 1)·e + l.
- Forward-backward, k loops: each loop is D followed by reverse(D) without its first and last elements
  (2n − 2 drawings); frames = (k·(2n − 2) − 1)·e + l.
- When first = last (n = 1), the cycle is a single drawing; forward-backward behaves as continuous.
- When the user edits total frames, loops = ceil(frames ÷ frames-per-loop) and the last loop is truncated to the
  requested frame count.

The GUI MUST show the resulting total frames live while parameters change.

### 13.11 Clear drawing

Deletes the content of the selected drawing or image while keeping the drawing and the cell labels (the cells still
reference a drawing of that name, now empty). Because a drawing may be exposed in many cells, all cells showing it
become empty. Refused for locked drawings (`drawing.locked`).

### 13.12 Lock / unlock drawing

Locks the selected drawing against editing: drawing and painting tools and content commands refuse to modify it
(`drawing.locked`). The command presents itself as "unlock" when the drawing is locked. Locked drawings show a lock
indicator in their cells. Lock is per drawing: all cells exposing it are affected.

### 13.13 Change label (substitution by name)

Changes which drawing the selected cell(s) reference.

- GUI: an in-cell editor prefilled with the current label.
- Entering the name of an existing drawing of the element exposes it in the selected cells.
- Entering a new name, for drawing and annotation elements, creates a new empty drawing with that name and exposes
  it; for image and media elements it reports `cell.drawingNotFound`.
- The previously referenced drawing is not renamed and stays in the bank.

### 13.14 Rename drawing

Renames the drawing (or image) referenced by the selected cell.

- Parameter: the new name (GUI prefilled with the current name; for image elements the GUI says "image").
- It is the drawing that is renamed, so every cell referencing it shows the new name, in every element of the clone
  group. The drawing's stored data follows the new name (10).
- Validation: empty name `drawing.nameEmpty`; forbidden character `drawing.nameInvalid`; name already used in the bank
  `drawing.nameDuplicate` (the message names the drawing and asks for a different name).

### 13.15 Duplicate drawing

Creates a **new drawing** that is a copy of the drawing in the selected cell, named with the next free name, and
exposes the copy in the selected cell(s) in place of the original. Other cells still show the original. Typical use:
make a variation of a held drawing without affecting the rest of the hold.

### 13.16 Cut, copy, paste, paste as new drawings

| Command | Behaviour |
|---|---|
| **Cut** | Copies the selected cells (labels, drawings, properties) to the clipboard and removes them according to the cut-special "create/delete exposure" option: on = cells removed and following cells shift earlier; off = cells cleared. |
| **Copy** | Copies the selected cells, or whole elements when rows/headers are selected. |
| **Paste** | Pastes at the selected cell. Within the same element (or clone group) the pasted cells reference the **same** drawings; into another element the drawings are copied into that element's bank (same-named drawings are reused or overwritten per the "overwrite existing drawings" option). With whole elements on the clipboard, new elements are created. |
| **Paste as new drawings** | Creates **new drawings** (new names) from the copied cells, exposes them at the selected frame — the pasted cells are independent of the originals. |
| **Cut special / paste special / paste special again** | Apply the transfer options of 11.14 (Timeline) or the peg-property options (Camera view, 05). |

Clipboard content is typed (drawing cells, image cells, sound, media, pegs, whole elements). Pasting into an
incompatible column type is refused (`cell.typeMismatch`), e.g. images into a vector drawing column. Pasting onto
locked drawings is refused (`drawing.locked`). Long pastes show progress.

### 13.17 Delete

- With cells selected: deletes the selected cells. **Design decision:** in the Exposure Sheet, delete clears the
  selected cells (leaving blanks); in the Timeline it follows the cut-special "create/delete exposure" option (remove
  and shift earlier when on, clear when off). The agent form states the behaviour explicitly (`shift`).
- With rows selected in the Timeline: deletes the elements (8.3).
- With a sound clip selected: removes the clip.

### 13.18 Send to static light table

Adds the selected drawing(s) or image(s) — or, from a column header, the element's drawing at the current frame — to
the static light table so they stay visible while working in the Drawing View (15).

### 13.19 Reset media

See 2.5.

### 13.20 Availability by element type

| Command | Drawing | Image | Media | Sound | Annotation | Peg |
|---|---|---|---|---|---|---|
| Add / remove exposure | ✓ | ✓ | ✓ | — | ✓ | ✓ (bar length) |
| Set exposure 1/2/3/N | ✓ | ✓ | ✓ | — | ✓ | — |
| Extend exposure | ✓ | ✓ | ✓ | — | ✓ | Extend children exposure |
| Insert cells / insert blank / delete blank | ✓ | ✓ | ✓ | ✓ | ✓ | — |
| Create cycle / columnate | ✓ | ✓ | ✓ | — | ✓ | — |
| Duplicate drawing | ✓ | — | — | — | ✓ | — |
| Clear drawing / lock drawing | ✓ | — | — | — | ✓ | — |
| Change label / rename drawing | ✓ | ✓ | — | — | ✓ | — |
| Paste as new drawings | ✓ | ✓ | — | — | ✓ | — |
| Cell note | ✓ | ✓ | — | — | ✓ | — |
| Send to static light table | ✓ | ✓ | — | — | ✓ | — |

A command invoked on an element type marked "—" reports `cell.notApplicable`.

### 13.21 Edge cases

| Case | Required behaviour |
|---|---|
| Add exposure on a blank cell | Inserts one blank frame. |
| Remove exposure on the last remaining frame of a drawing | Removes the cell; following cells shift earlier; drawing stays in the bank. |
| Set exposure on a selection spanning several elements | Applied per element independently. |
| Extend exposure over existing drawings | Overwrites them up to the target frame. |
| Insert cells in all elements | Inserts blank cells in every cell-based element; pegs, cameras and effects are not shifted. |
| Create cycle with a selection ending in blank cells | The blank cells are part of the cycle pattern. |
| Columnate with first = last | Single-drawing cycle; forward-backward behaves as continuous. |
| Clear drawing on a locked drawing | Refused. |
| Paste into a selection containing locked drawings | Refused. |
| Paste images into a vector drawing column | Refused (type mismatch). |
| Hide in Exposure Sheet the current element | The current element moves to the next visible column. |
| Rename element to its current name | Silent no-op. |
| Delete a peg with children | Children are kept and re-attached to the peg's parent. |
| Operation would move exposures before frame 1 | Refused (`cell.frameOutOfRange`). |

---

## 14. Drawing management

### 14.1 Drawings belong to elements

- Each drawing, image, annotation and media element owns a **drawing bank**: the set of its drawings, independent of
  whether they are currently exposed in cells.
- Clones share one drawing bank (8.1).
- A drawing can be exposed in any number of cells of its element; cells never reference drawings of another element.

### 14.2 Storage and naming

| Item | Rule |
|---|---|
| Vector drawings and bitmap images | Stored inside the project package (layout in 10). |
| Sound files | Stored inside the project package (07, 10). |
| Media | Not copied: the element stores links to external files. |
| Display name | `<element>-<drawing>` when the element-name option is on, otherwise `<drawing>`. |
| Default new drawing name | The next integer after the highest purely numeric drawing name in the bank (`1` if there is none). Non-numeric names do not affect the computation. The algorithm MUST be deterministic. |
| Missing data | When a drawing's data or a media element's linked file cannot be found on load, the application reports it, shows a placeholder, and suppresses further reports of the same kind for that load. Missing items are flagged in queries (`missing: true`). |

### 14.3 Creating drawings

| How | Result |
|---|---|
| Draw into a blank cell (Drawing or Camera view) | New drawing with the next free name, exposed in the current cell. |
| Insert cells with named cells | New, empty, consecutively named drawings. |
| Change label to a new name | New empty drawing with that name. |
| Duplicate drawing | Copy of the current drawing under a new name (13.15). |
| Paste as new drawings | New drawings from clipboard cells (13.16). |
| Create drawing (command) | New empty drawing in the bank, optionally exposed over a frame range. |
| Import (images, import-and-vectorise, vector/PDF artwork, SWF, movie as image sequence, capture device, scanner) | New drawings/images in the selected element or a new element (09). |

### 14.4 Renaming drawings

- **Rename drawing** (13.14) renames the drawing everywhere it is used.
- Drawings of scene elements can also be renamed from the library's view of the scene (08), with the same validation.

### 14.5 Substituting drawings

Drawing substitution (changing which drawing a cell shows) is available via:

1. the Cells panel slider and drawing-name field (6.2);
2. change label and in-cell editing (13.13);
3. for agents, `cell.label.set` and `xsheet.exposures.set` (17).

Previous / next drawing (11.6) navigates between drawings but does not substitute.

### 14.6 Deleting drawings and unused drawings

- **Clear drawing** empties a drawing's content but keeps it (13.11).
- Deleting or clearing all cells that reference a drawing does **not** delete the drawing; it stays in the bank and
  remains available to the Cells panel, columnate and the library.
- An **unused drawing** is a drawing of the bank not exposed in any cell of its element or of any clone. The
  application MUST be able to list the unused drawings of an element or of a whole scene, and to delete selected ones.
- **Delete drawing** removes drawings from the bank (reachable from the library's view of the scene's elements, 08,
  and as a command). If a drawing is still exposed, the user MUST confirm; on confirmation it is deleted and the cells
  that referenced it become blank. One undo step; undo restores the drawing and its cells.
- Delete element removes the element's drawings unless shared by a clone (8.3).
- Storage of drawings no longer referenced by any element is purged from the project package when the project is
  saved; until then undo can restore them.

### 14.7 Library interactions (cells and drawings)

| Operation | Behaviour |
|---|---|
| Copy to current frame (library) | Copies a drawing or image from a library element to the current frame of the current element. Wrong element type → `cell.typeMismatch`. |
| Drag cells/elements from the Timeline or Exposure Sheet to the library | Creates a template from the selection (08). |
| Drag a template from the library into the Timeline or Exposure Sheet | Inserts it at the drop frame (new elements, or drawings into the target element). |
| Update drawings from template | Lists, for the target element, each drawing and its template counterpart, flagged *modified* or *new*; the user selects which to update (with select-all / clear-all) and whether to also update palette colours; checked drawings are replaced by the template's version. One undo step. (Commands in 08.) |

---

## 15. Static light table (Exposure Sheet panel)

### 15.1 Purpose

Keeps chosen drawings (vector drawings or bitmap images, from any element and frame of the scene) visible in the
Drawing View at all times, as reference while drawing. How they are rendered in the Drawing View (including the
optional shading preference) is specified in 02.

### 15.2 Access

- The panel is shown or hidden from the Exposure Sheet toolbar.
- Drawings are added with **send to static light table** (13.18) from the cell commands, the cell contexts or the
  column-header contexts, or by dragging cells onto the panel.

### 15.3 Panel contents

An ordered list of items; each item shows a visibility control, an over/under control and the label
`<drawing> in <element>`.

- Dragging items reorders them; the list order is the stacking order of the light-table drawings.
- The over/under control sets whether that drawing is displayed **over** or **under** the drawings of the Drawing View.
- Items track their source: renaming the drawing renames the item; deleting the drawing, element or scene removes the
  item; colour changes refresh it.
- Removing an item never deletes the drawing itself.
- The panel contents are saved with the scene.

### 15.4 Commands

| Command | Scope | Behaviour |
|---|---|---|
| Show / hide | Item | Toggles that item's visibility. |
| Hide all others | Item | Hides all items except the selected one. |
| Show all | Panel | Shows all items. |
| Hide all | Panel | Hides all items. |
| Overlay all | Panel | Displays all items over the Drawing View drawings. |
| Underlay all | Panel | Displays all items under the Drawing View drawings. |
| Delete | Item | Removes the item from the light table. |
| Delete all | Panel | Removes all items. |

Item commands are reachable from the item's context; panel commands from both the item and empty-panel contexts.

---

## 16. Key bindings

- Only platform-standard shortcuts are prescribed: cut, copy, paste, delete, select all, undo, redo (01).
- Every command of this section MUST be bindable to a key (01).
- Gestural requirements: double-clicking a row name or column header renames the element; a single-cell modifier
  selects one cell in the Timeline when single cell selection is off; extend/toggle modifiers apply to row and cell
  selection.
- **Design decision:** ship single-key default bindings for the high-frequency timing commands (add/remove exposure,
  insert/delete blank cell, set exposure 1/2/3/N, extend exposure, previous/next frame, previous/next drawing,
  previous/next element, show Timeline selection); the concrete key map is a UX task.

---

## 17. Agent interface

### 17.1 Agent-specific requirements

1. **Full coverage.** Everything a user can do or see in the Timeline, Exposure Sheet, Cells panel, element list and
   static light table MUST be doable or readable through the commands below (or the generic tools of 11). No dialog,
   prompt, drag gesture or visual-only feedback is the only path.
2. **Explicit targets.** Every cell command accepts an explicit target — `elementIds` / `elementId` plus `from`/`to`
   (or `frame`). When the target is omitted, the shared selection and current position are used (the
   selection-based variant used by the GUI). A command called with an explicit target MUST NOT change the UI
   selection or the current frame. If neither a target nor a usable selection exists the command fails with
   `cell.noTarget`.
3. **Addressing.** Elements are addressed by stable `id` or by path (`scene:"Intro"/element:"Bob"`); drawings by `id`,
   by path (`scene:"Intro"/element:"Bob"/drawing:"3"`), or — where a parameter is a drawing *name* (`drawing`,
   `firstDrawing`) — by name within the target element. Frames are 1-based integers; there are no spatial
   coordinates in this area (scene-space positions of elements belong to 05).
4. **Dialogs become parameters.** Batch creation → `element.create {type, name, count}`; rename → `element.rename`;
   set exposure N → `cell.exposure.set {length}`; extend exposure → `cell.fill.apply {toFrame}`; insert cells →
   `cell.range.add`; create cycle → `cell.cycle.create {cycles}`; advanced cycle → `cell.columnate.apply`; change start
   frame / duration / loops → `element.timing.set`; set current frame / playback start / end →
   `timeline.currentFrame.set`, `timeline.playbackRange.set`; notes → `element.note.set`, `cell.note.set`; colour
   pickers → `colour` parameters; transfer-option dialogs → `timeline.transferOptions.set` or an inline `options`
   object.
5. **Gestures become commands.** Row drag → `element.order.set` / `element.parent.set`; bar move/stretch/loop handle →
   `element.timing.set`; cell drag-and-drop → `cell.range.move`; exposure-edge drag → `cell.exposure.set` on that
   exposure; column-header drag and element-list drag → `element.order.set {order: "xsheet"}`; drop on the light-table
   panel → `xsheet.lightTable.add`; library drops → library commands (08); sound clip drags → sound commands (07).
6. **No blocking modals.** Warnings (e.g. `element.hiddenInDrawingMode`) are returned in a `warnings[]` array of the
   result. Confirmations become parameters with a safe default: `element.drawing.delete {ifInUse: "fail"}` returns a
   structured `needs_confirmation` result listing where the drawing is exposed; repeating with `ifInUse: "clearCells"`
   performs it. Overwrite behaviour on paste/move is controlled by the transfer options (`overwriteDrawings`).
7. **Atomic bulk edit.** `xsheet.exposures.set` replaces the exposures of several columns over a frame range in one
   validated, atomic, single-undo-step call (17.2). Any other combination of commands can be made atomic with the
   generic `batch` tool (11).
8. **Determinism.** Default names (elements, clones, drawings, named cells) follow the deterministic rules of 3.2 and
   14.2; every creating command returns the IDs and names it created. The same command sequence on the same document
   MUST produce the same result.
9. **Read-back.** `xsheet.exposures.get` (compact run-length grid), `timeline.rows.list` (bars, keyframes, clips),
   `element.list` / `element.get`, `element.drawing.list` (with optional thumbnails), `element.unusedDrawing.list`,
   `cell.selection.get`, `xsheet.lightTable.list`; visual verification through `render_frame` / `screenshot` (11).
   Thumbnails requested from `element.drawing.list` are rendered synchronously at the requested size (no
   "pending" placeholder).
10. **Long-running work.** Converting an image element to vector drawings is `draw.vectorize.run` with `imageElementId`
    (02); it is a job (`job_status` / `job_cancel`, 11) and the element is changed only when the job completes (one
    undo step). Background thumbnail and waveform computation never blocks
    commands.
11. **Errors** use `area.reason` codes (17.6); a failed command changes nothing. Out-of-range frames in cell commands
    are errors (`cell.frameOutOfRange`); playback-range values are clamped and the clamped values returned.
12. **Undo.** Every `action` command is exactly one undo step; `ui`, `toggle` commands marked "no" and queries are not
    in the undo history.

### 17.2 Exposure sheet as JSON

`xsheet.exposures.get` returns the exposure sheet for a frame window as run-length encoded columns, in Exposure Sheet
order (hidden columns included with `visible: false`). Only cell-based elements appear; bar elements are read with
`timeline.rows.list`.

```json
{
  "sceneId": "s1", "rev": 42, "sceneLength": 48, "from": 1, "to": 24,
  "columns": [
    { "elementId": "e7", "name": "Bob", "elementType": "drawing", "visible": true,
      "exposures": [ {"drawing": "1", "drawingId": "d11", "from": 1, "to": 6},
                     {"drawing": "2", "drawingId": "d12", "from": 7, "to": 8},
                     {"drawing": "3", "drawingId": "d13", "from": 11, "to": 24, "continues": "after"} ],
      "notes": [ {"frame": 3, "text": "fix hand"} ] },
    { "elementId": "e9", "name": "Dialogue", "type": "sound", "visible": true,
      "clips":  [ {"clipId": "c1", "file": "line1.wav", "from": 1, "to": 40, "continues": "after"} ],
      "mouths": [ {"mouth": "A", "from": 1, "to": 3}, {"mouth": "X", "from": 4, "to": 5} ] }
  ]
}
```

Rules: runs are sorted and non-overlapping; blank frames are omitted (gaps between runs); a run that extends beyond the
window is clipped to it and flagged with `continues: "before" | "after" | "both"`. `include` selects optional parts
(`"notes"`, `"mouths"`, `"thumbnails"`); `drawingId` is always present. Media columns use the media item name as
`drawing`. `rev` changes whenever any cell of the scene changes.

`xsheet.exposures.set` writes the same shape back:

```json
{ "sceneId": "s1", "expectRev": 42, "createMissingDrawings": true,
  "columns": [ { "elementId": "e7", "from": 1, "to": 24,
                 "exposures": [ {"drawing": "1", "from": 1, "to": 6},
                                {"drawing": "2", "from": 7, "to": 12},
                                {"drawing": "4", "from": 13, "to": 24} ] } ] }
```

Semantics:

- For each column, every cell in `from..to` is replaced: frames covered by a run show that drawing, frames not covered
  become blank. Cells outside `from..to` are unchanged; nothing is shifted. `from` defaults to 1; `to` defaults to the
  later of the last run's end and the element's current last exposed frame (so the whole tail is replaced).
- A run names its drawing by `drawing` (name) or `drawingId`. Unknown names fail with `cell.drawingNotFound` unless
  `createMissingDrawings` is true, which creates empty drawings with those names (drawing and annotation elements only).
- Validation before any change: runs have `from ≤ to`, lie within the column window, are sorted and do not overlap
  (`xsheet.runInvalid`, `xsheet.runOutOfRange`, `xsheet.runOverlap`); the element is cell-based and not a sound column
  (`xsheet.columnReadOnly` — sound clips and mouths are written with the commands of 07); `expectRev`, if given,
  matches (`xsheet.revisionMismatch`).
- Atomic: either every column is applied or none; errors report the column and run index. One undo step.
- `dryRun: true` validates and returns the diff without applying it.
- Result: `{rev, sceneLength, changed: [{elementId, from, to}], createdDrawings: [{elementId, drawingId, name}]}`.

### 17.3 Transfer options object

Used by `timeline.transferOptions.set`, `cell.range.move`, `cell.clipboard.set` and `cell.clipboard.apply` (11.14).
Omitted fields take the stored option set of the operation.

```json
{ "overwriteDrawings": false, "exposure": true, "autoExtend": false, "updateColours": false,
  "keyframeSubstitution": false, "boundaryKeys": false,
  "pegKeys": { "motion": true, "rotation": true, "scale": true, "pivot": true, "velocity": true, "skew": true } }
```

### 17.4 Command table

Cell commands with `elementIds?`, `from?`, `to?` or `frame?` use the selection / current position when these are
omitted (17.1-2).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `element.create` | action | Create one or more elements of a type (batch creation; the per-type add commands are this with `count: 1`) | `type: enum "drawing"\|"image"\|"sound"\|"media"\|"annotation"\|"peg"\|"camera"\|"skeleton"\|"colourTransform"\|"clipping"\|"dropShadow"\|"blur"\|"shake"\|"particleNature"\|"particleImpact"; name?: string; count: int = 1 (1–99); sceneId?: id; parentId?: id; above?: id; xsheetAfter?: id` | `{elementIds[], names[], warnings[]}` | yes |
| `element.parentPeg.create` | action | Create a peg and attach the given elements to it | `elementIds?: [id]; name?: string` | `{pegId, name}` | yes |
| `element.list` | query | List the elements of a scene in Timeline or Exposure Sheet order | `sceneId?: id; order: enum "timeline"\|"xsheet" = "timeline"; types?: [string]; parentId?: id; recursive: bool = true` | `{elements: [{id, name, type, parentId, depth, index}]}` | n/a |
| `element.get` | query | Full state of one element (17.5) | `elementId: id` | element object | n/a |
| `element.rename` | action | Rename an element | `elementId: id; name: string` | `{elementId, name}` | yes |
| `element.clone` | action | Clone a cell-based element (shared drawing bank, independent timing) | `elementId: id; name?: string` | `{elementId, name, cloneGroupId}` | yes |
| `element.duplicate` | action | Independent copy of elements including copies of their drawings | `elementIds?: [id]` | `{elementIds[], names[]}` | yes |
| `element.delete` | action | Delete elements; children are re-attached to the deleted element's parent | `elementIds?: [id]` | `{deletedIds[], reparentedIds[], deletedDrawingCount}` | yes |
| `element.order.move` | action | Bring to front / forward, send backward / to back within the level (Timeline) or column order (Exposure Sheet) | `elementIds?: [id]; to: enum "front"\|"forward"\|"backward"\|"back"; order: enum "timeline"\|"xsheet" = "timeline"` | `{changed: bool, order: [id]}` | yes |
| `element.order.set` | action | Set the complete order of one Timeline level or of the Exposure Sheet columns (drag reordering) | `order: enum "timeline"\|"xsheet"; elementIds: [id]; parentId?: id; sceneId?: id` | `{order: [id]}` | yes |
| `element.parent.set` | action | Attach elements to a peg, effect, skeleton or camera, or to the nearest peg/effect above (attach to next object) | `elementIds?: [id]; parentId?: id; nearest: bool = false; index?: int` | `{elementIds[], parentId}` | yes |
| `element.parent.remove` | action | Detach elements from their parent (they move to the parent's level) | `elementIds?: [id]` | `{elementIds[], parentId}` | yes |
| `element.hierarchy.get` | query | Parent, children and siblings of an element | `elementId: id` | `{parentId, childIds[], previousSiblingId, nextSiblingId, depth}` | n/a |
| `element.select` | ui | Select elements (rows / columns) | `elementIds: [id]; mode: enum "replace"\|"add"\|"remove"\|"toggle" = "replace"` | `{selectedIds[]}` | no |
| `element.hierarchy.select` | ui | Select parent, children, first child, previous/next sibling or skeleton root | `relation: enum "parent"\|"children"\|"firstChild"\|"previousSibling"\|"nextSibling"\|"skeletonRoot"; elementId?: id` | `{selectedIds[]}` | no |
| `element.current.set` | ui | Make an element the current element | `elementId: id` | `{elementId}` | no |
| `element.current.move` | ui | Previous / next element in Timeline or Exposure Sheet order | `direction: enum "previous"\|"next"; order: enum "timeline"\|"xsheet" = "xsheet"` | `{elementId}` | no |
| `element.collapsed.set` | toggle | Collapse / expand parent rows in the Timeline (saved with the scene) | `elementIds?: [id]; collapsed: bool` | `{elementIds[]}` | no |
| `element.visibility.set` | toggle | Show/hide in the scene or the Exposure Sheet for the given elements, all, or all others; optional type filter (e.g. all annotation elements) | `elementIds?: [id]; scope: enum "scene"\|"xsheet"; visible: bool; apply: enum "given"\|"all"\|"others" = "given"; types?: [string]` | `{changedIds[]}` | yes |
| `element.solo.set` | toggle | Solo mode on/off (view state) | `enabled: bool; elementIds?: [id]` | `{enabled, elementIds[]}` | no |
| `element.lock.set` | toggle | Lock / unlock in the scene-planning views: given, all, all others | `elementIds?: [id]; locked: bool; apply: enum "given"\|"all"\|"others" = "given"` | `{changedIds[]}` | yes |
| `element.annotation.set` | toggle | Set a drawing element as annotation element, or back | `elementId: id; annotation: bool` | `{elementId, type}` | yes |
| `element.colour.set` | action | Set marker and/or Timeline background colour; an empty string reverts to the category default | `elementIds?: [id]; marker?: colour; background?: colour` | `{elementIds[]}` | yes |
| `element.typeColour.set` | ui | Set the default colour of an element category (preference) | `category: enum "drawings"\|"images"\|"sounds"\|"pegs"\|"skeletons"\|"media"\|"cameras"\|"effects"\|"effectParameters"; colour: colour` | `{category, colour}` | no |
| `element.typeColour.get` | query | Default colours of all categories | — | `{colours: object}` | n/a |
| `element.note.set` | action | Set, update or remove (empty text) an element note | `elementId: id; text: string` | `{elementId}` | yes |
| `element.properties.set` | action | Set composite role and deformable flag (transform values: section 05) | `elementId: id; role?: enum "normal"\|"foreground"\|"background"\|"envelope"; deformable?: bool` | `{elementId}` | yes |
| `element.timing.set` | action | Change start frame, duration and loops of bar elements; move or stretch bars | `elementIds?: [id]; start?: int (≥1); startDelta?: int; duration?: int (≥1); anchor: enum "start"\|"end" = "start"; keyframes: enum "keep"\|"scale" = "keep"; loops?: int (≥1)` | `{elements: [{id, start, duration, loops}]}` | yes |
| `element.childExposure.apply` | action | Extend the exposure of a peg's children up to a frame | `pegId: id; toFrame: int (≥1)` | `{changed: [{elementId, from, to}]}` | yes |
| `element.mediaLength.set` | action | Reset a media element's exposure to the natural media length, or set a length | `elementId: id; length?: int (≥1)` | `{elementId, from, to}` | yes |
| `element.drawing.list` | query | Drawings of an element's bank with usage and optional thumbnails | `elementId: id; filter: enum "all"\|"used"\|"unused" = "all"; thumbnails?: int (16–512)` | `{drawings: [{id, name, exposures: [{from, to}], useCount, locked, empty, missing, contentType, thumbnail?}]}` | n/a |
| `element.unusedDrawing.list` | query | Find drawings not exposed in any cell, for given elements or the whole scene | `sceneId?: id; elementIds?: [id]` | `{items: [{elementId, drawingId, name}]}` | n/a |
| `element.drawing.create` | action | Create an empty drawing in the bank, optionally exposed over a range | `elementId: id; name?: string; exposeFrom?: int; exposeTo?: int` | `{drawingId, name}` | yes |
| `element.drawing.rename` | action | Rename a drawing (all cells, all clones) | `drawingId: id; name: string` | `{drawingId, name}` | yes |
| `element.drawing.duplicate` | action | Copy a drawing into a new bank entry without exposing it | `drawingId: id; name?: string` | `{drawingId, name}` | yes |
| `element.drawing.delete` | action | Delete drawings from the bank; exposed drawings need `ifInUse: "clearCells"` | `drawingIds: [id]; ifInUse: enum "fail"\|"clearCells" = "fail"` | `{deletedIds[], clearedCells: [{elementId, from, to}]}` or `needs_confirmation` | yes |
| `element.drawingContent.delete` | action | Clear drawing: delete content, keep the drawing and its labels | `drawingIds?: [id]` | `{drawingIds[]}` | yes |
| `element.drawingLock.set` | toggle | Lock / unlock drawings | `drawingIds?: [id]; locked: bool` | `{drawingIds[]}` | yes |
| `cell.select` | ui | Select cells (frames × elements), a whole track, or a sound clip extent | `elementIds: [id]; from: int; to: int; mode: enum "replace"\|"add" = "replace"; extent: enum "cells"\|"track"\|"soundClip"\|"soundClipToStart"\|"soundClipToEnd" = "cells"` | `{elementIds[], from, to}` | no |
| `cell.selection.get` | query | Current cell/row selection and current position | — | `{elementIds[], from, to, rowsSelected, currentElementId, currentFrame}` | n/a |
| `cell.exposure.add` | action | Add exposure: hold longer by N frames, following cells shift later | `elementIds?: [id]; from?: int; to?: int; frames: int = 1 (1–9999)` | `{changed: [{elementId, from, to}]}` | yes |
| `cell.exposure.remove` | action | Remove exposure: shorten by N frames, following cells shift earlier | `elementIds?: [id]; from?: int; to?: int; frames: int = 1 (1–9999)` | `{changed: [{elementId, from, to}]}` | yes |
| `cell.exposure.set` | action | Set every exposure in the range to N frames (ones / twos / threes / N) | `elementIds?: [id]; from?: int; to?: int; length: int (1–9999)` | `{changed: [{elementId, from, to}]}` | yes |
| `cell.fill.apply` | action | Extend exposure: expose the drawing at `frame` through `toFrame`, overwriting | `elementIds?: [id]; frame?: int; toFrame: int (1–9999)` | `{changed: [{elementId, from, to}]}` | yes |
| `cell.blank.add` | action | Insert blank cell(s) at a frame; following cells shift later | `elementIds?: [id]; frame?: int; count: int = 1 (1–9999)` | `{changed: [{elementId, from, to}]}` | yes |
| `cell.blank.remove` | action | Delete blank cell(s) at a frame; fails with `cell.notBlank` on a non-blank cell | `elementIds?: [id]; frame?: int; count: int = 1 (1–9999)` | `{changed: [{elementId, from, to}]}` | yes |
| `cell.range.add` | action | Insert cells (blank or named; one element or all elements) | `elementIds?: [id]; allElements: bool = false; frame?: int; count: int = 1 (1–9999); position: enum "before"\|"after" = "after"; named: bool = false` | `{from, to, drawingIds[]}` | yes |
| `cell.range.delete` | action | Delete cells: clear (leave blanks) or remove and shift earlier | `elementIds?: [id]; from?: int; to?: int; shift: bool = false` | `{changed: [{elementId, from, to}]}` | yes |
| `cell.range.move` | action | Move / copy / link / copy-as-new-drawings a cell range to another frame or element (drag-and-drop equivalent) | `sourceElementIds: [id]; from: int; to: int; targetElementId?: id; targetFrame: int; mode: enum "move"\|"copy"\|"link"\|"copyNew" = "move"; options?: object` | `{from, to, createdDrawingIds[]}` | yes |
| `cell.clipboard.set` | action | Copy (or cut, undoable) an explicit range or whole elements to the clipboard | `elementIds?: [id]; from?: int; to?: int; wholeElements: bool = false; cut: bool = false; options?: object` | `{contentType, elementCount, frames}` | yes |
| `cell.clipboard.apply` | action | Paste the clipboard at a target, normally or as new drawings, with optional transfer options | `elementId?: id; frame?: int; mode: enum "paste"\|"newDrawings" = "paste"; options?: object` | `{elementIds[], from, to, createdDrawingIds[]}` | yes |
| `cell.label.set` | action | Change label / substitute the drawing in the range by name, id, or by stepping through the bank | `elementIds?: [id]; from?: int; to?: int; drawing?: string; drawingId?: id; step?: int; createMissing: bool = false` | `{drawingId, name, changed[]}` | yes |
| `cell.drawing.duplicate` | action | Duplicate drawing: copy the drawing at the range and expose the copy there | `elementId?: id; from?: int; to?: int; name?: string` | `{drawingId, name}` | yes |
| `cell.cycle.create` | action | Repeat the range so that it appears N times in total; following cells shift | `elementId?: id; from?: int; to?: int; cycles: int (1–999)` | `{from, to}` | yes |
| `cell.columnate.apply` | action | Advanced cycle: write first..last drawings with exposure, loop mode and placement (13.10) | `elementId?: id; frame?: int; firstDrawing: string; lastDrawing: string; exposeEach: int = 1 (1–9999); exposeLast?: int (1–9999); loopMode: enum "none"\|"continuous"\|"forwardBackward" = "none"; loops?: int (1–999); totalFrames?: int; placement: enum "overwrite"\|"insert" = "overwrite"; dryRun: bool = false` | `{from, to, totalFrames, loops, sequence: [{drawing, from, to}]}` | yes |
| `cell.note.set` | action | Set, update or remove (empty text) a cell note | `elementId: id; frame: int; text: string` | `{elementId, frame}` | yes |
| `timeline.rows.list` | query | Timeline read model: rows, bars, keyframe markers, clips (17.5) | `sceneId?: id; from?: int; to?: int` | timeline object | n/a |
| `timeline.currentFrame.set` | ui | Set the current frame (clamped to 1..scene length) | `frame: int (≥1); sceneId?: id` | `{frame, clamped: bool}` | no |
| `timeline.currentFrame.move` | ui | First / previous / next / last frame, or previous / next drawing of an element; `withinRange` limits movement to the playback range | `direction: enum "first"\|"previous"\|"next"\|"last"; unit: enum "frame"\|"drawing" = "frame"; elementId?: id; count: int = 1; withinRange: bool = false` | `{frame, moved: bool}` | no |
| `timeline.playbackRange.set` | ui | Automatic or free playback range, start and end (clamped) | `mode?: enum "auto"\|"free"; start?: int; end?: int` | `{mode, start, end}` | no |
| `timeline.playbackRange.get` | query | Current playback range | — | `{mode, start, end}` | n/a |
| `timeline.timeDisplay.set` | ui | Display frames or feet and frames (preference) | `unit: enum "frames"\|"feetFrames"` | `{unit}` | no |
| `timeline.frameLabel.get` | query | Format frame numbers per the current time display | `frames: [int]` | `{labels: [string]}` | n/a |
| `timeline.view.set` | ui | Zoom level, scroll position, reveal an element (show Timeline selection) | `zoomLevel?: int; firstFrame?: int; revealElementId?: id` | `{zoomLevel, firstFrame, lastFrame}` | no |
| `timeline.soundWaveforms.set` | toggle | Show sound rows as waveforms or solid bars | `enabled: bool` | `{enabled}` | no |
| `timeline.effectParameterRows.set` | toggle | Show/hide effect-parameter rows of one effect (omit = all effects) | `visible: bool; elementId?: id` | `{visible}` | no |
| `timeline.singleCellSelection.set` | toggle | Single cell selection preference | `enabled: bool` | `{enabled}` | no |
| `timeline.transferOptions.set` | ui | Store the paste-special, cut-special or drag-and-drop option set (17.3) | `operation: enum "paste"\|"cut"\|"dragDrop"; options: object` | `{operation, options}` | no |
| `timeline.transferOptions.get` | query | Read a stored option set | `operation: enum "paste"\|"cut"\|"dragDrop"` | `{options}` | n/a |
| `timeline.backgroundColour.set` | ui | Timeline background colour preference | `colour: colour` | `{colour}` | no |
| `xsheet.exposures.get` | query | Exposure sheet of a frame window as run-length JSON (17.2) | `sceneId?: id; elementIds?: [id]; from: int = 1; to?: int; include?: [string]` | exposure-sheet object | n/a |
| `xsheet.exposures.set` | action | Atomically replace exposures of one or more columns over frame windows (17.2) | `sceneId?: id; columns: [object]; createMissingDrawings: bool = false; expectRev?: int; dryRun: bool = false` | `{rev, sceneLength, changed[], createdDrawings[]}` | yes |
| `xsheet.currentCell.set` | ui | Set the current element and frame together | `elementId: id; frame: int (≥1)` | `{elementId, frame}` | no |
| `xsheet.display.set` | ui | Thumbnails, exposure text in held cells, element-name prefix, notes | `thumbnails?: bool; exposureText?: bool; elementNames?: bool; notes?: bool` | `{thumbnails, exposureText, elementNames, notes}` | no |
| `xsheet.columnWidth.set` | ui | Set one column's width, or reset all widths | `elementId?: id; width?: int (20–1000); reset: bool = false` | `{widths: object}` | no |
| `xsheet.panel.set` | ui | Show/hide the element list panel, static light table panel and toolbar | `elementList?: bool; lightTable?: bool; toolbar?: bool` | `{elementList, lightTable, toolbar}` | no |
| `xsheet.lightTable.add` | ui | Send drawings to the static light table (omit items = selection) | `items?: [object]` | `{itemIds[]}` | no |
| `xsheet.lightTable.remove` | ui | Remove light-table items, or all | `itemIds?: [id]; all: bool = false` | `{removedIds[]}` | no |
| `xsheet.lightTable.list` | query | Light-table items in stacking order | — | `{items: [{itemId, elementId, drawingId, label, visible, placement}]}` | n/a |
| `xsheet.lightTable.set` | ui | Show/hide and over/under for given items, all, or all others | `itemIds?: [id]; apply: enum "given"\|"all"\|"others" = "given"; visible?: bool; placement?: enum "over"\|"under"` | `{changedIds[]}` | no |
| `xsheet.lightTable.move` | ui | Reorder a light-table item | `itemId: id; index: int (≥0)` | `{order: [id]}` | no |
| `element.state.changed` | event | Elements created, deleted, renamed, reordered, reparented, or flags/properties changed | — | `{sceneId, elementIds[], change}` | n/a |
| `element.drawing.changed` | event | Drawing bank changed (created, renamed, deleted, locked, cleared) | — | `{elementId, drawingIds[], change}` | n/a |
| `cell.exposures.changed` | event | Cells of an element changed in a frame range | — | `{sceneId, elementId, from, to, rev}` | n/a |
| `cell.selection.changed` | event | Cell or row selection or current position changed | — | `{elementIds[], from, to, currentElementId, currentFrame}` | n/a |
| `timeline.currentFrame.changed` | event | Current frame changed (throttled to at most 10 per second during playback) | — | `{sceneId, frame, source: "playback"\|"navigation"\|"command"}` | n/a |
| `timeline.playbackRange.changed` | event | Playback range changed | — | `{mode, start, end}` | n/a |
| `xsheet.lightTable.changed` | event | Static light table contents or item state changed | — | `{itemIds[]}` | n/a |

Commands used by this area but owned elsewhere: window show/hide, cut/copy/paste/delete/select-all on the selection,
paste special again, zoom (01); transforms and keyframes (05); effect parameters (06); sound clips, lip-sync mouths,
sound play/stop (07); library templates, update drawings from template, scene selection (08); import, playback,
sound playback/scrubbing (09).

### 17.5 Queries and events

`element.get` returns:

```json
{ "id": "e7", "name": "Bob", "elementType": "drawing", "sceneId": "s1", "parentId": "p2", "childIds": [],
  "depth": 1, "timelineIndex": 0, "xsheetIndex": 3, "start": 1, "duration": 48, "loops": 1,
  "annotation": false, "role": "normal", "deformable": false,
  "visibleInScene": true, "hiddenByParent": false, "visibleInXsheet": true, "locked": false, "collapsed": false,
  "markerColour": null, "backgroundColour": null, "effectiveMarkerColour": "#9DC3E6FF",
  "note": "", "columnWidth": 80, "cloneGroupId": null, "cloneIds": [], "drawingCount": 12,
  "media": null }
```

(`media`, for media elements: `{path, startFrame, length, missing}`.)

`timeline.rows.list` returns `{sceneId, sceneLength, currentFrame, playbackRange: {mode, start, end}, rows: [...]}`
where each row is `{elementId, name, type, depth, parentId, collapsed, visibleInScene, hiddenByParent, locked,
bar: {start, end, loops}, keyframes: [{frame, channels: ["position"|"scale"|"rotation"|"skew"], segmentAfter:
"constant"|"interpolated"}], childKeyframeFrames: [int], clips?: [{clipId, file, from, to}]}`, in Timeline order
(parents before children). Cell-based rows give `bar` as their exposed extent; their cells are read with
`xsheet.exposures.get`.

Events are delivered through `app://events` (11). `cell.exposures.changed` is emitted once per command per affected
element (coalesced ranges), so an agent can re-read only the changed window.

### 17.6 Error and warning codes

| Code | Meaning |
|---|---|
| `element.nameEmpty` / `element.nameInvalid` / `element.nameDuplicate` / `element.nameTooLong` | Element name validation (4.2). |
| `element.countOutOfRange` | Batch count outside 1–99. |
| `element.createFailed` | Element could not be created. |
| `element.notFound` | Unknown element ID or path. |
| `element.notCloneable` | Clone requested for a non-cell-based element. |
| `element.notParentable` / `element.hierarchyCycle` | Invalid parent / attaching to itself or a descendant. |
| `element.notLoopable` | Loops set on a type that cannot loop. |
| `element.notCellBased` / `element.notDrawing` / `element.notVectorDrawing` | Command requires a cell-based / drawing / vector drawing element. |
| `element.templateEditRestricted` | Operation not allowed while editing a library template. |
| `element.hiddenInDrawingMode` (warning) | Added pegs/cameras/effects are not visible in drawing mode. |
| `cell.noTarget` | No explicit target and no usable selection. |
| `cell.drawingNotFound` | Drawing name not in the element's bank. |
| `cell.notBlank` | Delete blank cell on a non-blank cell. |
| `cell.frameOutOfRange` / `cell.frameBeforeSelection` | Frame below 1, above 9999, or extend target before the selection start (message states the minimum). |
| `cell.typeMismatch` | Clipboard / source content incompatible with the target column type. |
| `cell.notApplicable` | Command not available for this element type (13.20). |
| `cell.linkNotPossible` | Link mode across elements that do not share a drawing bank. |
| `cell.clipboardEmpty` | Paste with nothing compatible on the clipboard. |
| `cell.busy` | Paste refused while bone deformation is being computed. |
| `drawing.nameEmpty` / `drawing.nameInvalid` / `drawing.nameDuplicate` | Drawing name validation. |
| `drawing.notFound` | Unknown drawing ID or path. |
| `drawing.locked` | Modification of a locked drawing. |
| `drawing.inUse` | Delete of an exposed drawing without `ifInUse: "clearCells"` (returned as `needs_confirmation`). |
| `drawing.fileMissing` | Drawing data or linked media file not found. |
| `xsheet.runInvalid` / `xsheet.runOverlap` / `xsheet.runOutOfRange` | Invalid run in `xsheet.exposures.set`. |
| `xsheet.columnReadOnly` | Column cannot be written with `xsheet.exposures.set` (sound). |
| `xsheet.revisionMismatch` | `expectRev` does not match the current revision. |

---

## 18. Design decisions / open questions

| # | Decision to make | Recommended default |
|---|---|---|
| 1 | Which order arrange commands change when invoked from the Exposure Sheet vs the Timeline/Camera view. | Exposure Sheet focus → column order; otherwise Timeline order (1.2). Agents always specify `order`. |
| 2 | Default element category colours. | Values in 5.1. |
| 3 | Semantics of the composite roles foreground / background / envelope and of "deformable". | As in 6.1 (foreground/background composite outside Normal elements; background auto-extends; envelope = bone skin). |
| 4 | Suffix format for multiple new elements and clones. | `Name`, `Name_1`, `Name_2`, … skipping used names. |
| 5 | Maximum element/drawing name length. | 64 characters. |
| 6 | Feet-and-frames base and format. | 16 frames per foot, `F+FF`. |
| 7 | Drag-and-drop modifiers for move / copy / link. | None = move, copy modifier = copy, link modifier = link (only within an element or clone group). |
| 8 | Delete key on cells in the Exposure Sheet vs the Timeline. | Exposure Sheet clears; Timeline follows the cut-special "create/delete exposure" option. |
| 9 | Annotation elements in render/export. | Excluded by default; export option to include. |
| 10 | Keyframes beyond a shortened duration (change duration command). | Removed; drag-stretch scales keyframes instead. |
| 11 | Blank runs inside a set-exposure selection. | Keep their length. |
| 12 | Convert image element to vector: in place or as a new element. | In place, one undo step, run as a job. |
| 13 | Whether static light table changes are undoable. | Not undoable (view aid), but saved with the scene. |
| 14 | Placement of the show-notes option. | Exposure Sheet display options (12.4). |
| 15 | Single-key default bindings for timing commands. | Provide them (16); concrete keys chosen by UX. |
| 16 | Whether deselect-all clears an Exposure Sheet cell selection. | Yes. |
| 17 | Whether media elements and cameras may be added while editing a library template. | No (`element.templateEditRestricted`). |
| 18 | Whether a "recent drawings" list is offered. | Not required. |
| 19 | When unreferenced drawing storage is purged. | On project save; undo can restore until then. |

