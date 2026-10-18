# 02 — Drawing

This section specifies everything needed to create and edit the *content* of a vector drawing: the drawing coordinate
space, the vector data model (brush strokes, pencil lines, invisible strokes, zones, layers, groups, text, embedded
bitmaps), curve fitting and pressure, the Drawing View (and drawing directly in the Camera View), every drawing and
editing tool with its options, pens and symmetry, drawing commands (flatten, optimize, smooth, centre-line extraction,
line-to-brush conversion, feathering, arrange, transform, group/ungroup, break text apart), drawing settings (snap to
contour, draw on top layer, draw behind; automatic gap closing is owned by section 03), drawing aids (grids, onion skin, light tables, stroke display,
background), the drawing inspector, and bringing bitmap and vector artwork into drawings (import and vectorize). It ends
with the agent interface for the `draw`, `tool` and `drawAid` namespaces.

**Cross-references.**
- Palettes, swatches, colour IDs, gradients and textures as colour definitions, palette styles, colour recovery rules:
  `03_colour.md`. This section defines *where* colour is applied in a drawing; section 03 defines what a colour is.
- Elements, cells and exposure (how the current drawing is chosen; creating, duplicating, clearing and deleting
  drawings as cells): `04_timeline_xsheet.md`.
- Scene space, element/peg transformations, the Camera View's scene-planning tools: `05_scene_planning.md`.
- Generic view navigation (`view` namespace), windows, preferences, undo/redo, clipboard and generic selection
  (`edit` namespace), key binding: `01_application_shell.md`.
- Library templates and updating drawings from templates: `08_library_storyline.md`.
- Generic file import, image capture devices, scanners and drawing export (PDF): `09_playback_export.md`.
- Persistence of drawings inside the project package (`.animpkg`): `10_data_model_project_format.md`.
- Generic agent tools (`run_command`, `batch`, `pointer`, `key`, `type_text`, `render_frame`, `get_object`,
  `set_properties`, `job_status`, events): `11_agent_interface_mcp.md`. All commands are listed in
  `A_command_registry.md`.

**Terms.**
- **Drawing object** — any selectable item inside a drawing: a brush stroke shape, a pencil line, an invisible stroke,
  a painted zone, a group, a text object or an embedded bitmap.
- **Zone** — a closed region bounded by strokes that can be filled with a colour.
- **Modifiers** are named functionally: the **constrain modifier** (Shift on all platforms), the **alternate modifier**
  (Option/Alt), the **temporary-select modifier** (Command on macOS, Ctrl elsewhere), the **temporary-pan key** (Space).
  These defaults MAY be remapped like any other binding.

---

## 1. Concepts and where drawing happens

### 1.1 Drawings and the current drawing

- Vector drawing happens inside **drawing elements** and **annotation elements** (section 04). Each holds a series of
  *drawings*; each drawing is an independent vector document.
- The drawing being edited — the **current drawing** — is the drawing exposed in the selected cell of the selected
  element at the current frame (selection made in the Exposure Sheet or Timeline, section 04).
- Drawing into an **empty cell** MUST create a new drawing for that cell automatically, named according to the
  element's numbering scheme (section 04); creation and the first stroke form one undo step ("Create Drawing").
- Image (bitmap) elements, sound, pegs, cameras and effects cannot be edited with drawing tools; a drawing tool used on
  them does nothing and reports `tool.notApplicable`.
- When the application is in drawing mode and the user adds elements that are not displayed in drawing mode, the
  application MUST warn that the new elements are not visible there and explain how to see them (switch to
  scene-planning mode). For agents this is a warning code `drawing.elementNotVisibleInDrawingMode` in the result of the
  element-creating command.
- A drawing can be **locked** and unlocked (available from the cell/element context and by command). Every edit to a
  locked drawing MUST be refused with `drawing.locked` and the user informed; nothing is changed.

### 1.2 Views in which drawing tools operate

| View | Behaviour |
|---|---|
| **Drawing View** | Shows only the current drawing of the selected element, plus enabled drawing aids (onion skin, light tables, grid). Coordinates are the drawing's own untransformed space; peg/element transformations are not applied. |
| **Camera View** | Drawing tools also work here, directly on the selected element's current drawing, seen through the camera with all scene transformations applied; other elements remain visible. Pointer positions are mapped back through the element's full scene transform into drawing space. |
| Top / Side views | Not used for drawing (scene planning only, section 05). |

- Each of the Drawing, Camera, Top and Side views MUST offer a quick way to open each of the other views.
- Elements hidden in the Exposure Sheet are not shown in the Drawing View or the Exposure Sheet; Timeline visibility
  governs the Camera View (section 04).
- **Show Current Drawing on Top** (drawing aid, §10.5) forces the drawing being edited in the Camera View to render
  above all other drawings regardless of layering so it is never hidden while editing.

### 1.3 Tool manager

- Exactly one drawing tool is active at a time. The active tool and its settings are shared by the Drawing View and the
  Camera View.
- When the active tool has no editable options, the tool-options area states that the tool has no options.
- Switching among the selection tools (Select, Contour Editor, Perspective) keeps the current drawing selection;
  switching to a creation tool keeps the selection but hides its handles.

---

## 2. Coordinate spaces

### 2.1 Drawing space

All drawing content and all `draw.*` command parameters use **drawing space**:

| Property | Definition |
|---|---|
| Origin | (0, 0) is the **drawing centre**, which coincides with the centre of the field guide (§10.1) and, under an identity element transform, with the scene origin (section 05). |
| Units | **Drawing units (du)**, floating point. **1 field = 100 du.** |
| Axes | +x to the right, **+y up** (north). |
| Angles | Degrees; positive = counter-clockwise in drawing space. Commands that name a direction ("rotate 90° clockwise") are explicit. |
| Field guide | The 12-field guide spans x ∈ [−600, 600], y ∈ [−450, 450] (12 × 9 fields, 4:3). The 16-field guide spans x ∈ [−800, 800], y ∈ [−600, 600]. |
| Relation to camera | Under the default camera and identity element transform, the camera frame width at depth 0 equals 12 fields (1200 du); its height is 1200 du ÷ the project's aspect ratio (section 05 owns the camera model). |
| Sizes | Pen sizes, line thickness, text size, margins and feather widths are all in du. (Gap-closing and snap tolerances are in view pixels, converted at the current zoom.) |

Drawing content is resolution independent; pixels only appear when rendering.

### 2.2 View space

- **View pixels**: origin at the top-left of a view's canvas, +x right, **+y down**, in logical (device-independent)
  pixels. This is the space of the generic `pointer` tool (section 11).
- The mapping view-pixels ↔ drawing space for a given view is an affine transform (Drawing View: pan, zoom, view
  rotation; Camera View: the inverse of the element's full scene transform and camera projection at the current frame,
  restricted to the drawing plane). It MUST be queryable (`draw.viewMapping.get`).
- View rotation, zoom and pan are display transforms only; drawn geometry is stored in unrotated drawing space.
- The status area MUST show the pointer position in drawing space while the pointer is over a drawing view, in the
  coordinate unit chosen in preferences (fields, inches, centimetres or du; section 05 defines unit conversions).

---

## 3. Vector drawing data model

### 3.1 Structure of one drawing

```
Drawing
 ├─ Node list (ordered back-to-front)
 │   ├─ Layer node      → planar stroke graph: strokes + zones
 │   ├─ Group node      → child nodes (layers, groups, text, bitmaps)
 │   ├─ Text node       → rich text object
 │   └─ Bitmap node     → embedded raster image
 ├─ Drawing pivot (point)
 └─ Metadata: name, lock flag (sections 04/10)
```

- Each **layer** is an independent planar map. Strokes in different layers never cut each other and zones never span
  layers. Painting, gap detection, contour editing and topology operations act *within* a layer; the Eraser acts on all
  layers (§7.4.1).
- Nodes stack back-to-front; Arrange commands reorder them (§8.4).
- New artwork created by a drawing tool is placed in a **new layer on top** of the drawing (undoing the stroke removes
  that layer). With **Draw Behind** on, it goes into a new layer at the back (§8.11). With **Draw on Top Layer** on,
  brush strokes of the same colour as the top layer are merged into it instead (§8.10).
- A layer carries a **type hint** — `brush` (contour artwork) or `pencil` (centre-line artwork) — recording which kind
  of tool produced it. A layer MAY contain both kinds of stroke; the hint lets conversions find candidates.

### 3.2 Strokes

A **stroke** is a piecewise cubic Bézier path (open or closed) inside a layer.

| Stroke kind | Created by | Rendering |
|---|---|---|
| **Brush** | Brush; Convert Lines to Brush; Eraser/Cutter results on brush artwork; vectorization; Break Text Apart | The stroke's points run *around* the shape: its outline contours are the boundary of a filled zone. The outline is not stroked; the enclosed zone is filled with the stroke's colour. |
| **Pencil** (centre line) | Pencil, Line, Polyline, Rectangle, Ellipse; Extract Centre Line; stroked paths of imported vector files | The stroke's points run down the *centre* of the line; drawn with a **line style**: colour ID, thickness (du), line type (solid; other types MAY be added). |
| **Invisible** | Stroke tool; Close Gap tool; automatic gap closing; unpainting a pencil line; Convert Lines to Brush (preserved centre path) | Never rendered in output or export; visible only while the **Show Strokes** aid is on. Separates and closes zones. |

- Every stroke has two **sides** (left/right relative to its direction); each side is attached to the zone lying on
  that side. This planar-graph link is how paint operations find the region under a point.
- Within a layer, strokes are split at intersections and junctions so that zones are well defined. Shared anchor points
  at a junction belong to all strokes meeting there.
- A **brush stroke** additionally keeps, while it is unedited, its **spine**: the fitted centre path as drawn, its
  **pressure map** and the pen (tip shape, minimum/maximum size) used. The outline is derived from spine + pressure +
  pen. Once the outline is edited directly (Eraser, Cutter, Contour Editor, merge with Draw on Top Layer, Flatten,
  vectorization), the spine is discarded and the outline becomes authoritative.
- A **pressure map** is an ordered list of (t, p) samples where t ∈ [0, 1] is the normalised arc-length position along
  the spine and p ∈ [0, 1] is pressure; values between samples are linearly interpolated.

### 3.3 Curve fitting

Freehand input (Brush, Pencil, Stroke, Eraser, Close Gap and lasso paths) is converted to Béziers as follows:

1. Collect samples (x, y, pressure) in drawing space; drop consecutive duplicates.
2. Detect **corners** (sharp direction changes) and split the sample list there.
3. Fit each run with cubic Béziers by least squares, estimating end tangents from neighbouring samples; if the maximum
   error exceeds the tolerance, split at the point of maximum error and fit each half recursively.
4. The **tolerance** is derived from the pen's **Smoothness** (0–10): 0 = tight fit (many control points), 10 = very
   smooth (few control points). Recommended: tolerance = 0.25 du + 0.5 du × smoothness.
5. Pressure samples are smoothed with a moving average whose window grows with Smoothness, then stored as the pressure
   map.

Fitting MUST depend only on the sample positions, pressures and settings — never on timing or velocity — so that a
replayed or structured input produces identical geometry (§13.3).

### 3.4 Pressure and variable width

- With a pressure-sensitive tablet, brush width varies continuously between the pen's **Minimum Size** and **Maximum
  Size**: width(p) = Min + p × (Max − Min), with p normalised to [0, 1] from the device's range.
- With a mouse, or when tablet input is ignored, p = 1 throughout (width = Maximum Size).
- The brush outline is the union of the tip shape swept along the spine at the local width. Self-overlaps of one stroke
  form a single clean outline (no internal edges).
- A very short click (no movement) creates a single dab of the tip shape at the current width.

### 3.5 Zones and colour

- A **zone** is a closed region of a layer bounded by strokes of any kind, including invisible ones. Each zone has a
  **fill**: either *unpainted* (transparent) or a reference to a palette colour (solid, gradient or bitmap texture;
  section 03).
- Zones nest: a zone may contain holes and child zones. A single paint operation may set the colour of a zone and its
  children together.
- Colours are stored as **colour IDs**, never as RGB values, so editing a swatch or switching palette style recolours
  every zone and line using it (section 03).
- Gradient and texture fills carry a per-zone **texture matrix** (2×3 affine: position, scale, rotation, skew) mapping
  the colour's unit space into drawing space; it is edited with the Edit Texture tool (§7.3.7) and can be copied from
  one zone to others.
- Brush strokes are filled zones and so carry a fill colour; pencil lines carry their colour in their line style. The
  Paint tool can recolour zones, brush strokes and pencil lines.
- Overlapping partially transparent zones in different layers blend; this is why Flatten can change appearance and
  Optimize declines to merge such layers.

### 3.6 Groups

- Grouping combines the selected drawing objects into one node so they move, rotate and scale together; ungrouping
  dissolves the group and keeps its members in place and order.
- Groups preserve the stacking order of their members. Clicking any member with the Select tool selects the whole
  group; double-clicking a group selects inside it.

### 3.7 Text objects

- Created with the Text tool (§7.6). A text object is rich text with per-character formatting (font, size, bold,
  italic, normal/subscript/superscript, manual kerning, automatic kerning, fill colour ID) and per-paragraph formatting
  (alignment left/centre/right/justify, left and right margins, first-line indent, line spacing), an affine transform,
  and either **auto width** (no wrapping) or a **fixed width** (word wrap).
- Glyphs are rendered from the font outlines. Text objects keep their transform (they are not baked into geometry when
  moved/rotated/scaled).
- **Break Text Apart** turns text into ordinary vector artwork (§8.7).
- When a font is missing on the machine, a fallback font MUST be used for display and the text object MUST keep the
  requested font name.

### 3.8 Drawing pivot

Each drawing has a pivot point that the Select tool can display and move (undo: "Change Drawing Pivot"). It is the
default rotation/scale centre of the drawing when it is transformed in scene planning (section 05).

### 3.9 Embedded bitmaps

A drawing can contain raster images as drawing objects (pasted or dropped images, or the texture source kept by
vectorization "with texture"). They are selectable, transformable (they keep an affine transform), arrangeable and
deletable, but their pixels are not editable with drawing tools.

### 3.10 Rendering order

Within a drawing: nodes back-to-front; within a layer, zone fills (including brush stroke fills) are drawn first, then
pencil lines on top of the layer's fills. Invisible strokes are never rendered except as a display aid.

### 3.11 Colour recovery on paste and import

When drawing objects are pasted or imported from another project or a template and reference colour IDs missing from
the current palettes, the colours MUST be recovered into the palette list (undo: "Colour Recovery"); when the source
colour is unavailable, a clearly marked placeholder colour/texture is created. Rules: section 03.

### 3.12 Topology invariants

1. After any edit, each affected layer is re-noded (strokes split at intersections) and its zones recomputed. Colours
   carry over to the new zones that cover the same area: a zone split by a new stroke gives both parts the old colour;
   zones merged by removing a stroke take the colour of the larger part.
2. Invisible strokes take full part in topology but never in final rendering or export.
3. Every edit is undoable as a single step labelled with the tool or command name.
4. Every drawing has an integer **revision** that increases by at least 1 with every change to its content (including
   undo/redo). Strokes, brush strokes, text objects, groups and bitmaps keep their IDs across edits that do not change
   them; split or merged strokes receive new IDs. Zone IDs are derived from the zone's boundary and change whenever
   that boundary changes. Every command reports created and removed IDs and the new revision. Commands that address
   objects by ID MAY pass the revision they read; if the drawing has changed since, ID targets fail with
   `drawing.staleRevision` (section 03 uses `colour.staleZoneId` for its zone targets), while point targets are always
   re-resolved.

---

## 4. The Drawing View

### 4.1 Purpose

A dedicated canvas for drawing, inking and painting the current drawing of the selected drawing or annotation element,
independent of camera framing and peg animation. It can be opened and closed like any view (section 01) and is part of
the default drawing workspace together with the Exposure Sheet, Library, scene list and Properties views.

### 4.2 Canvas contents (back to front)

1. Background: the Drawing View background colour, or the scene background colour when that aid is on (§10.7).
2. Grid when in underlay mode (§10.1).
3. Static light-table entries in underlay placement (§10.4).
4. Auto light table: the other elements' drawings exposed at the current frame (§10.3), optionally shaded.
5. Onion skin: previous/next drawings of the current element (§10.2).
6. **The current drawing** (editable).
7. Static light-table entries in overlay placement.
8. Grid when in overlay mode.
9. Tool feedback: selection outline and handles, lasso/marquee, brush-size preview, cursor ghosts, invisible strokes
   and open stroke ends when Show Strokes is on.

### 4.3 Navigation

Navigation commands are generic `view` commands (section 01); in the Drawing View and Camera View they MUST behave as
follows:

- **Zoom in / out** about the view centre, in steps (recommended ×2 per step, range 1/16× to 16×); **reset zoom**.
- **Rotate view** clockwise / counter-clockwise in **30°** steps; **reset rotation**.
- **Recentre** (pan back to the centre of the work area); **reset view** (remove zoom, pan and rotation).
- **Centre on pointer** (centre the view on the pointer position).
- **Switch active view state**: toggle between two stored view states (zoom/pan/rotation) of the active view.
- Mouse wheel zooms about the pointer; pinch zooms; two-finger rotate rotates the view.
- Zoom tool and Grabber tool (§7.5); holding the temporary-pan key pans.

### 4.4 Context commands

The Drawing View's context menu (and the current drawing's context menu in the Camera View while a drawing tool is
active) MUST offer: cut, copy, paste, select all, deselect all; Arrange (four commands); Transform (flip horizontal,
flip vertical, rotate 90° CW, rotate 90° CCW, rotate 180°); Group, Ungroup; Flatten, Optimize, Smooth, Extract Centre
Line, Convert Lines to Brush, Feather Edges; Break Text Apart. The same commands MUST be available from the main menu.

### 4.5 Edit commands on drawings

Generic `edit` commands (section 01) act on the drawing selection when a drawing view has focus:

| Command | Drawing behaviour |
|---|---|
| Undo / Redo | Multi-level; each tool gesture or command is one step. |
| Cut / Copy | Cut/copy the selected drawing objects. The clipboard keeps strokes, layer structure, groups, text, bitmaps and the colour IDs used, with the information needed for colour recovery. |
| Paste | Pastes drawing objects into the current drawing at the same drawing coordinates as the source; pasting repeatedly into the same drawing offsets each further copy (recommended +10 du, −10 du per repeat) so copies do not overlap exactly. Pasted objects become the selection. Incompatible clipboard content is refused with `drawing.pasteIncompatible`. |
| Delete | Deletes the selected drawing objects; with the Contour Editor active, deletes the selected points (§7.1.2). |
| Select All | Selects all objects of the current drawing as one selection; with the Contour Editor active, selects all points of the selected strokes. |
| Deselect All | Clears the drawing selection. |

Platform-standard shortcuts apply (undo, redo, cut, copy, paste, select all). Peg/cell-specific "paste special" and
"cut special" (sections 04/05) do not apply to drawing objects.

---

## 5. Tool organisation

### 5.1 Tool groups

Tools are organised into exclusive groups. The tools palette shows one button per group displaying the last-used tool
of the group, with access to the group's other tools.

| Group | Tools (in order) | Tool IDs |
|---|---|---|
| Select | Select, Contour Editor, Perspective, Reposition All Drawings | `select`, `contourEditor`, `perspective`, `repositionAll` |
| Brush | Brush, Pencil, Rectangle, Ellipse, Polyline, Line | `brush`, `pencil`, `rectangle`, `ellipse`, `polyline`, `line` |
| Paint | Paint, Unpaint, Paint Unpainted, Dropper, Close Gap, Stroke, Edit Texture | `paint`, `unpaint`, `paintUnpainted`, `dropper`, `closeGap`, `stroke`, `editTexture` |
| Eraser | Eraser, Cutter, Scissor | `eraser`, `cutter`, `scissor` |
| Navigate | Zoom, Grabber | `zoom`, `grabber` |
| Text | Text | `text` |

**Group cycling.** Each group has a "select group" command. Invoking it while another group is active activates the
group's currently displayed tool; invoking it again while that group is active advances to the next tool of the group,
wrapping around. Each tool also has a direct activation command. All are bindable to keys (section 01).

### 5.2 Temporary overrides

| Gesture | Effect | Applies while these tools are active |
|---|---|---|
| Hold temporary-select modifier | Temporarily the Select tool; release returns to the previous tool. | Zoom, Grabber, Polyline, Line, Rectangle, Ellipse, Brush, Pencil, Eraser, Stroke, Close Gap, Paint, Paint Unpainted, Unpaint, Dropper, Edit Texture |
| Hold alternate modifier | Temporarily the Dropper; release returns. | Paint, Brush, Pencil only |
| Hold temporary-pan key | Temporarily the Grabber. | all |
| Tablet pen eraser end in proximity | Temporarily the Eraser; turning the pen back restores the previous tool. | all |
| Constrain modifier on shape tools | Constrain (square, circle, 15° angles). | Rectangle, Ellipse, Line, Polyline |
| Alternate modifier on shape tools | Draw from centre (Rectangle, Ellipse); continue from the previous end point (Line); break handle symmetry (Polyline). | |

A tool's own meaning of the alternate modifier takes precedence; the alternate→Dropper override applies only to Paint,
Brush and Pencil.

---

## 6. Pens, symmetry and tablet input

### 6.1 Pens

A **pen** is a named size/shape preset used by the Brush (brush pens) and the Eraser (eraser pens); Pencil and the shape
tools take their line thickness from the current brush pen. The pen list is stored **in the project**, so pens travel
with it. The Pen panel (also available as a tab of the Properties view) lists the pens and edits the selected pen.

| Setting | Type | Range | Effect |
|---|---|---|---|
| Name | string | unique within the project, non-empty | Display name. Duplicate names are refused (`pen.nameExists`). |
| Type | enum `brush` \| `eraser` | — | Which tool family uses the pen. Each row shows a stroke preview and a type indicator. |
| **Tip shape** | enum `round` \| `oval` \| `square` \| `flat` | default `round` | Shape of the nib swept along the path. |
| **Minimum size** | number (du) | 0 – 100 | Width at lowest pressure. |
| **Maximum size** | number (du) | 0 – 100 | Width at full pressure, and with a mouse. |
| **Smoothness** | number | 0 – 10 | Curve-fitting tolerance and pressure smoothing (§3.3); higher = smoother, fewer points. |
| Tip angle | number (degrees) | 0 – 360, default 0 | Orientation of non-round tips. |

Rules:
- Selecting a row makes it the current pen for its tool family. Brush and Eraser each remember their own current pen;
  when the active tool changes between them the panel switches to the matching pen.
- **New pen** adds a pen initialised from the current pen's settings, named with the next free "Pen n", and selects
  it. **Delete pen** removes the selected pen; the last pen of each type cannot be deleted (`pen.lastPen`). Pens can be
  renamed in place.
- Slider and numeric field are linked; edits commit on slider release or field commit. Each commit is one undoable step
  ("Set Pen Style"); list edits (new/delete/rename) are undoable too.
- Raising Minimum above Maximum pushes Maximum up to match, and lowering Maximum below Minimum pushes Minimum down.
- A preview shows a sample stroke going from minimum to maximum size with the current tip shape.
- Changing a pen never alters strokes already drawn (only explicit commands such as Convert Lines to Brush use the
  current pen on existing artwork).
- **Default pens** for a new project (design decision, see §14): brush pens "Pen 1"…"Pen 5" with maximum sizes 2, 4, 8,
  16, 32 du, minimum = 25 % of maximum, round, smoothness 3; eraser pens "Eraser 1"…"Eraser 3" with sizes 10, 20, 40 du
  (min = max), round, smoothness 3.

### 6.2 Symmetry and Draw Behind (advanced pen-panel options)

Hidden by default behind a show/hide-advanced-options control.

| Option | Type | Range / default | Effect |
|---|---|---|---|
| Symmetry mode | exclusive: **None** / **Mirror** / **Kaleidoscope** | default None | None: strokes drawn normally. Mirror: each new stroke is also drawn reflected about an axis through the drawing origin. Kaleidoscope: each new stroke is replicated with dihedral symmetry about the drawing origin. |
| Angle | number (degrees) | 0 – 360, default 0 | Mirror: orientation of the axis, measured counter-clockwise from the +y axis (0 = vertical axis, left/right mirror). Kaleidoscope: orientation of the first mirror line. |
| Number of mirrors | int | 2 – 16, default 6; enabled only for Kaleidoscope | n mirror lines through the origin at Angle + k × 180°/n; each stroke produces 2n images (n rotations by 360°/n and their reflections). |
| **Draw Behind** | toggle | off | New strokes are placed behind existing artwork (§8.11). |

All copies produced by symmetry are created in the same undo step as the original stroke. Symmetry applies to Brush,
Pencil, Stroke and the shape tools.

### 6.3 Tablet input settings (preferences)

| Option | Default | Effect |
|---|---|---|
| Ignore tablet input | off | The tablet acts as a mouse; pressure sensitivity disabled. |
| Portrait-mode correction | off | Corrects coordinate and pressure errors on tablet computers used in portrait orientation. |
| Compatibility tablet handling | off | Alternative tablet event handling for tablets that do not work with the default handling. |
| Tablet input interface | platform default | Where the platform offers more than one tablet input interface, choose which one to use; may require a restart. |

---

## 7. Drawing tools — reference

Unless stated otherwise every tool acts on the current drawing in the Drawing View or the Camera View, creates one undo
step per gesture labelled with the tool name ("Brush Tool", "Pencil Tool", …), and is refused on a locked drawing
(`drawing.locked`). Tool option defaults are summarised in §7.7.

### 7.1 Selection group

#### 7.1.1 Select

- **Purpose:** select, move, scale, rotate, copy and delete drawing objects.
- **Options:** rectangle-selection rule (`inside` = objects wholly inside the marquee, default; `intersect` = objects
  touching it).
- **Interaction:**
  - Click on an object selects the front-most object under the pointer (a group selects the whole group; in a layer,
    the clicked stroke shape or zone set). Click on empty space deselects.
  - Drag on empty space draws a rectangle (marquee) selection.
  - With the alternate modifier, a drag starting outside the selection draws a **lasso** (free-form) selection.
  - With the constrain modifier, click/drag adds to or toggles the selection.
  - The selection shows a bounding box with handles: drag inside to move; drag corner/side handles to scale (constrain
    modifier keeps proportions); place the pointer just outside a corner to rotate; the pointer indicates the resize
    direction in 15° steps.
  - Alternate-drag of the selection moves a **copy**.
  - Double-click a text object to edit its text (Text tool behaviour); double-click a group to select inside it.
  - Arrow keys nudge the selection by one view pixel (converted to du); with the constrain modifier, 10 view pixels.
  - Delete removes the selection.
  - The drawing pivot is shown and can be dragged ("Change Drawing Pivot").
- **Result:** transformations are applied permanently to the stroke geometry (gradient/texture matrices of affected
  zones transform with them); text objects and bitmaps instead update their stored transform.
- **Undo labels:** "Move Object", "Scale Object", "Rotate Object", "Copy Object", "Delete Object".
- **Edge case:** marquee and lasso select whole objects only; cutting through shapes is the job of Cutter and Scissor.

#### 7.1.2 Contour Editor

- **Purpose:** edit the Bézier anchor points and handles of pencil lines, brush outlines, invisible strokes and zone
  boundaries.
- **Interaction:**
  - Click a stroke to select it and show its anchors; click or marquee to select points; the constrain modifier adds to
    the point selection.
  - Drag a **point** to move it; drag a **segment** to reshape it (the point on the segment under the pointer follows
    the pointer; the segment's handles are adjusted).
  - Temporary-select-modifier + click on a stroke **adds a point** there without changing the shape.
  - Alternate + click on a point **resets its handles** (minimises the handle angle to a flat/smooth state).
  - Alternate + drag a handle moves it **independently** of the opposite handle (breaks tangent continuity).
  - Alternate + drag a point changes the shape of the two segments connected to it.
  - Alternate + drag on a stroke moves the entire selection.
  - Delete removes the selected points; the stroke is rebuilt from the remaining points. A stroke with no points left
    is removed; deleting an interior anchor of an open line where the remaining shape cannot be kept continuous MAY
    split it.
  - Smooth (§8.3) with points selected smooths the selected portion.
- **Snap to Contour** (§8.9): a dragged point snaps to nearby strokes and anchors so objects join.
- **Result:** all strokes meeting at a moved shared anchor move together; zones are recomputed and keep their colours.
- **Undo label:** "Contour Editing".

#### 7.1.3 Perspective

- **Purpose:** distort the selection with a four-corner envelope to change its apparent orientation or perspective.
- **Interaction:** click/marquee to select (constrain modifier adds; alternate modifier outside the selection draws a
  lasso). Drag a **corner** handle to move that corner freely. Alternate + drag a **side** handle changes the size and
  angle of the opposite sides symmetrically.
- **Result:** geometry is mapped through the envelope permanently when the gesture ends: projective (homography)
  mapping for convex quadrilaterals, bilinear mapping when the quadrilateral is non-convex.
- **Undo label:** "Perspective Tool".

#### 7.1.4 Reposition All Drawings

- **Purpose:** move all drawings of the current element at once (e.g. after importing scans drawn off-centre).
- **Interaction:** like the Select tool applied to the whole drawing: drag to offset; bounding-box handles scale and
  rotate; envelope (perspective) handles are available. The resulting transform is applied identically to **every
  drawing of the element**.
- **Undo label:** "Reposition All Drawings" — one step for all drawings.

### 7.2 Brush group

#### 7.2.1 Brush

- **Purpose:** draw variable-width freehand strokes (filled brush shapes).
- **Options:** current brush pen (§6.1); current colour (section 03); symmetry and Draw Behind (§6.2); Draw on Top
  Layer (§8.10). A size preview follows the pointer while hovering.
- **Interaction:** press–drag–release draws one stroke; tablet pressure modulates width; alternate modifier →
  temporary Dropper; temporary-select modifier → Select.
- **Result:** a brush stroke (outline + filled zone in the current colour) in a new top layer, or merged into the top
  layer (Draw on Top Layer with matching colour), or placed behind (Draw Behind). A texture colour fills the stroke with
  the texture.
- **Undo label:** "Brush Tool".

#### 7.2.2 Pencil

- **Purpose:** draw constant-width centre-line strokes that close zones for painting.
- **Options:** current colour; thickness = current brush pen's Maximum Size; smoothness from the pen.
- **Interaction:** freehand drag; alternate → Dropper; temporary-select → Select.
- **Result:** one pencil line (closed if its end meets its start within the snap radius) with a line style (colour ID,
  thickness). Intersections with other strokes of the same layer split zones.
- **Undo label:** "Pencil Tool".
- Display of drawings with many pencil lines MUST stay interactive.

#### 7.2.3 Rectangle

- **Interaction:** drag from one corner to the opposite corner; constrain modifier → square; alternate modifier → drag
  from the centre.
- **Result:** a closed four-segment pencil-type path in the current colour and thickness, creating one closed,
  unpainted zone.
- **Undo label:** "Rectangle Tool".

#### 7.2.4 Ellipse

- **Interaction:** drag the bounding box; constrain → circle; alternate → from the centre.
- **Result:** a closed ellipse made of four cubic Bézier segments (standard circle approximation, handle length
  0.5523 × radius) as a pencil-type path, creating one closed zone.
- **Undo label:** "Ellipse Tool".

#### 7.2.5 Polyline

- **Purpose:** build a shape point by point with straight or curved segments.
- **Interaction:**
  - Click to add a corner point; click-and-drag to add a point and pull out symmetric Bézier handles.
  - Constrain modifier while dragging handles → handle angle constrained to 15° increments.
  - Alternate modifier → move one handle independently of the other (cusp).
  - Over the **first point** the pointer indicates "close"; clicking closes the shape and ends the polyline.
  - Over any location on the path being drawn the pointer indicates "add"; clicking adds a point there and closes a
    shape.
  - Constrain + click on an existing point of the path being drawn deletes it (pointer indicates "remove").
  - New points snap onto existing anchors and strokes of the drawing within the snap radius (pointer indicates
    "snap").
  - Enter or double-click ends an open polyline; Escape cancels the polyline in progress.
- **Result:** a pencil-type path in the current colour/thickness.
- **Undo label:** "Polyline Tool".

#### 7.2.6 Line

- **Interaction:** drag from start to end. Constrain → angle constrained to multiples of 15°. Alternate → the new line
  starts from the end point of the previous line (connected segments).
- **Result:** one straight pencil-type line. **Undo label:** "Line Tool".

### 7.3 Paint group

The paint tools are specified in full in section 03 (§8 there: painting rules, hit-testing, automatic gap closing,
fill mapping). Their structured commands live in the `colour` namespace and are **not** redefined here:
`colour.paint.apply` (Paint), `colour.paintUnpainted.apply` (Paint Unpainted), `colour.paint.remove` (Unpaint),
`colour.pick` + `colour.current.set` (Dropper), `colour.gap.close` (Close Gap), `colour.fillMapping.get/set` (Edit
Texture) and `colour.gapClosing.set/get` (automatic gap closing). The summary below states how the tools behave as
drawing tools; section 03 is authoritative where details differ.

#### 7.3.1 Paint

- **Purpose:** fill zones, and recolour pencil lines, brush strokes and text, with the current colour.
- **Interaction:** click inside a closed zone → the zone takes the current colour; click on a pencil line, brush stroke
  or text object → it takes the current colour (line thickness unchanged). Dragging with a solid colour paints every
  zone and line crossed, as one undo step; press–drag–release with a gradient or texture colour defines the fill
  mapping axis, while a click fits the default mapping to the object's bounding box. Alternate modifier → temporary
  Dropper; temporary-select modifier → Select.
- **Gap handling:** if the clicked area is not closed, the automatic gap-closing mode (§8.8) may close it; the needed
  closing segments are kept as invisible strokes. Otherwise nothing is painted (`colour.zoneNotClosed`).
- **Undo label:** "Paint".

#### 7.3.2 Unpaint

- **Purpose:** remove colour from zones and lines.
- **Interaction:** click a painted zone → it becomes unpainted (transparent); surrounding lines are untouched. Click a
  pencil line → the line becomes **unpainted, i.e. it is turned into an invisible stroke** (visible only while Show
  Strokes is on), so its geometry and the zone boundaries it forms are preserved. Brush strokes and text cannot be
  unpainted (`colour.cannotUnpaint`; delete them instead). Automatic gap closing applies.
- **Undo label:** "Unpaint".

#### 7.3.3 Paint Unpainted

- **Purpose:** fill only zones that are not yet painted, so many zones can be filled quickly without disturbing painted
  ones.
- **Interaction:** click → fills the zone under the pointer only if it is unpainted. Drag → draws a free-form lasso; on
  release every unpainted zone with an interior point inside the lasso is filled. Lines, brush strokes and text are not
  affected. Automatic gap closing applies.
- **Undo label:** "Paint Unpainted".

#### 7.3.4 Dropper

- **Purpose:** pick the colour of a painted zone, line, brush stroke or text; the matching swatch becomes the current
  colour (switching the displayed palette if needed).
- **Interaction:** click. Unpainted areas and empty canvas: no change. A colour that no longer exists in any palette:
  no change, the user is told it is missing and offered colour recovery (section 03). Available temporarily with the
  alternate modifier from Paint, Brush and Pencil (returns to the previous tool on release).
- **Undo:** none (changes only the current colour).

#### 7.3.5 Close Gap

- **Purpose:** close small openings in outlines so zones can be painted.
- **Interaction:** drag a short line across or near the opening (feedback line while dragging). On release the
  application finds the nearest stroke end point (or closest stroke point) to each end of the drag, within a tolerance
  proportional to the drag length and at least the large gap-closing tolerance, and adds a short straight stroke
  connecting them.
- **Result:** the added stroke is an **invisible stroke**. If no suitable pair is found nothing changes
  (`colour.noGapFound`). If Show Strokes is off, the user is told once that the stroke is not visible (hint
  `drawing.strokesHidden`).
- **Undo label:** "Close Gap".

#### 7.3.6 Stroke

- **Purpose:** draw invisible strokes to close gaps or to split zones without adding line art (colour separations,
  shadow areas).
- **Interaction:** freehand drag (pen smoothing applies). The invisible stroke is added to the top-most layer whose
  zones it crosses (so it splits existing zones), or to a new top layer if it crosses none.
- **Hint:** if Show Strokes is off when a stroke is added, the user is told once that the stroke is invisible until
  Show Strokes is turned on (a re-enableable tip, section 01). Agents receive the hint code `drawing.strokesHidden`.
- **Structured equivalent:** `draw.stroke.add {kind: "invisible"}` (this section).
- **Undo label:** "Stroke Tool".

#### 7.3.7 Edit Texture

- **Purpose:** change the position, size, orientation and skew of gradient and texture fills inside painted zones,
  brush strokes and lines.
- **Interaction:** click an object filled with a gradient or texture to show a manipulator — linear gradient: start
  and end handles and the axis; radial gradient: centre, radius handle and second-axis handle (elliptical); texture:
  a frame with move, scale (constrain modifier keeps proportions), stretch, rotate and skew handles. Solid-colour and
  unpainted objects show no manipulator. While this tool is active the context menu offers copying the fill mapping,
  and paste applies it to the selected objects.
- **Result:** the object's **texture matrix** (§3.5) is updated on release.
- **Undo label:** "Edit Fill Mapping".

### 7.4 Eraser group

#### 7.4.1 Eraser

- **Purpose:** remove regions of drawing objects; erasing through artwork can also create new closed zones that can be
  painted.
- **Options:** current eraser pen (size, tip shape, smoothness; pressure applies).
- **Interaction:** drag over artwork. The swept area of the tip is subtracted from brush shapes and zones and cuts
  pencil and invisible strokes (the part inside the swept area is removed). Acts on all layers of the current drawing.
  Also active temporarily via the tablet pen's eraser end.
- **Result:** new outline edges along the swept boundary, producing new zones where appropriate.
- **Undo label:** "Eraser Tool".

#### 7.4.2 Cutter

- **Purpose:** cut out part of the artwork with a free-form path.
- **Interaction:** draw a closed free-form path around the part to cut. Everything inside is split from the rest along
  the path and becomes the **selection**, which can then be moved, transformed (bounding box as with Select) or
  deleted.
- **Undo label:** "Cutter Tool".

#### 7.4.3 Scissor

- **Purpose:** cut rectangular sections from the artwork; with the alternate modifier, free-form sections.
- **Interaction:** drag a rectangle; the artwork is cut along it and the inside part becomes the selection (as with the
  Cutter). Alternate → free-form path like the Cutter.
- **Undo label:** "Scissor Tool".

### 7.5 Navigation group

- **Zoom tool:** click zooms in centred on the click; drag a rectangle zooms to fit it; alternate + click zooms out;
  double-click resets zoom; temporary-select modifier → Select.
- **Grabber:** drag pans the view; works in all views; also available by holding the temporary-pan key.

These change only view state (section 01 `view` commands).

### 7.6 Text tool

- **Purpose:** add and edit text in a drawing.
- **Options:** the Text panel (§7.6.1).
- **Interaction:**
  - Click on empty canvas → new text object at that point in auto-width mode ("New Text Object"); a text cursor
    appears. Typing inserts characters ("Typing"); Backspace/Delete, arrow keys, Home/End edit and move; Return starts
    a new line.
  - Drag on empty canvas → new text object with a **fixed width** equal to the drag width (word wrap). Dragging the
    border of an existing text box changes its width and switches it to fixed width ("Set Text Width").
  - Click inside existing text places the cursor; drag highlights a range; double-click selects a word; select-all
    selects all text of the object being edited.
  - Drag a text object by its frame to move it ("Move Text Object").
  - Text-panel changes apply to the highlighted characters, or to the next typed characters if nothing is highlighted
    ("Text Property Modified").
  - Text colour = the current colour when typing; painting a text object with the Paint tool recolours it.
  - Cut/copy/paste act on highlighted text while editing.
  - Clicking elsewhere or switching tools ends editing; a text object left empty is deleted ("Delete Text Object").

#### 7.6.1 Text panel

| Setting | Type | Range | Default | Effect |
|---|---|---|---|---|
| Font | font family | installed fonts | platform default sans-serif | Typeface. |
| Size | number (du) | 1 – 512 (step 2 in the spin control) | 36 | Font size. |
| Bold | toggle | — | off | Bold style. |
| Italic | toggle | — | off | Italic style. |
| Baseline | enum `normal` \| `subscript` \| `superscript` | — | normal | Baseline shift with size reduction (recommended 60 % size). |
| Kerning | int (1/100 em) | −60 – 60 | 0 | Manual inter-character spacing. |
| Auto kern | toggle | — | on | Use the font's kerning pairs. |
| Alignment | enum `left` \| `centre` \| `right` \| `justify` | — | left | Paragraph alignment. |
| Left margin | number (du) | 0 – 1000 | 0 | Paragraph left margin. |
| Right margin | number (du) | 0 – 1000 | 0 | Paragraph right margin. |
| Indent | number (du) | −99 – 1000 | 0 | First-line indent (negative = hanging). |
| Line spacing | number (du) | −99 – 1000 | 0 | Additional spacing between lines. |
| Outline colour | colour ID or none | — | none | Optional outline drawn around glyphs. |

### 7.7 Tool option defaults (summary)

| Tool | Options and defaults |
|---|---|
| Select | rectRule = `inside` |
| Contour Editor | uses Snap to Contour (default off) |
| Perspective | mapping = `auto` (projective, bilinear if non-convex) |
| Brush | brush pen = "Pen 3"; symmetry = none; Draw Behind off; Draw on Top Layer off |
| Pencil | thickness from current brush pen's maximum size; smoothness from pen |
| Rectangle, Ellipse, Line | thickness from current brush pen; constrain angle step 15° |
| Polyline | snap = on, snap radius 8 view px; handle angle step 15° |
| Paint, Paint Unpainted, Unpaint | automatic gap closing = off (section 03, `colour.gapClosing.set`) |
| Dropper | — |
| Close Gap | tolerance proportional to drag length, at least the large gap tolerance (section 03) |
| Stroke | smoothness from current brush pen |
| Edit Texture | — |
| Eraser | eraser pen = "Eraser 2" |
| Cutter, Scissor | after cut: select the inside part |
| Zoom, Grabber | — |
| Text | Text panel defaults (§7.6.1) |

---

## 8. Drawing commands and drawing settings

Commands act on the **current drawing selection** (made with Select, Contour Editor, Perspective, Cutter or Scissor)
unless stated, and are unavailable when there is nothing to act on (`drawing.nothingToDo` for agents).

### 8.1 Flatten

Merges all selected layers (and the contents of selected groups) into **one layer**, reducing memory and file size.
Overlapping transparent colours are not preserved: where objects overlap, the upper object's colour replaces the lower
one. The merged layer takes the stacking position of the top-most selected layer; strokes and zones of lower layers
completely hidden by opaque upper zones are discarded; the result is re-noded (all strokes now intersect). A selection
that is already a single layer is a no-op. Undo: "Flatten Drawing".

### 8.2 Optimize

Reduces the number of layers in the selection by merging only layers whose merge does **not** change the final
appearance. Two adjacent layers may merge if no partially transparent zone or line of the upper layer overlaps any
painted zone or line of the lower layer. Layers stacked to create additive colour effects stay separate. Undo:
"Optimize Drawing".

### 8.3 Smooth

Removes unnecessary points from the selected strokes (brush outlines, pencil lines, invisible strokes) while keeping
their shape within a tolerance, reducing file size. With the Contour Editor, applies only to the selected points. Each
invocation smooths further. Recommended tolerance per invocation: 1 du. Undo: "Smooth Selection".

### 8.4 Arrange

| Command | Behaviour |
|---|---|
| Bring to Front | Moves the selected objects above all other objects of the drawing. |
| Bring Forward | Moves the selection up one position in the drawing's node order. |
| Send Backward | Moves the selection down one position. |
| Send to Back | Moves the selection behind all other objects. |

If the selection is only part of a layer, the selected objects are first separated into their own layer. Undo label =
the command name. (Arranging whole elements is a section 04/05 command.)

### 8.5 Extract Centre Line

Converts the selected **brush strokes into pencil lines** (fewer points, smaller files, lines suitable for painting).
Algorithm: triangulate each selected brush shape; compute its medial axis from the triangulation; remove short spurious
branches (shorter than the local stroke width); split the centre line where the source colour changes; create pencil
lines along the result with the source colour and a thickness equal to the average width of the source shape; remove
the original brush shape. It should be applied to brush strokes *before* filling zones with colour, because a paint
fill is itself a shape and would produce unwanted centre lines; the command warns (`drawing.centreLineOnFill`) when the
selection contains painted zones that are not brush strokes. Undo: "Extract Centre Line".

### 8.6 Convert Lines to Brush

Converts the selected centre-line strokes (from Pencil, Line, Polyline, Rectangle, Ellipse, Extract Centre Line) into
brush strokes so they can be edited like brush artwork. The outline is generated by sweeping the **current brush pen**
(tip shape and maximum size, no pressure) along each line and filling it with the line's colour. Zone fills adjacent
to the original line are preserved: the line's centre path is kept as an invisible stroke so zones stay closed. Undo:
"Convert Lines to Brush".

### 8.7 Break Text Apart

Converts each character of the selected text objects into discrete drawing objects: glyph outlines become brush-type
shapes filled with the text colour, one group per character, positioned exactly where the glyphs were. The text is no
longer editable as text. Undo: "Break Text Apart".

### 8.8 Auto Close Gap

A persistent user setting with four exclusive modes: **Off** (default), **Small**, **Medium**, **Large**, reachable from
the main menu and the paint tools' options. It is owned by section 03 (§8.8 there; commands `colour.gapClosing.set` /
`colour.gapClosing.get`). In summary: when a paint, paint-unpainted or unpaint click lands in a region that is not
closed, candidate gaps no wider than the mode's tolerance are tested, closing segments are inserted, and the region is
found again; the segments needed to bound the painted zone are kept as invisible strokes. Tolerances are measured in
view pixels (Small 3, Medium 8, Large 16) and converted to drawing units at the current zoom; commands without a view
use 100 % zoom.

### 8.9 Snap to Contour

Persistent toggle (default off). When on, points moved with the Contour Editor snap to nearby strokes and anchor points
(radius 8 view pixels) so that objects join and shapes close. Applies only to the Contour Editor.

### 8.10 Draw on Top Layer

Persistent toggle (default off). When on, each Brush stroke whose colour matches the colour of the existing top layer's
brush artwork is merged into that top layer instead of creating a new layer; same-colour strokes fuse into one shape.

### 8.11 Draw Behind

Persistent toggle (default off; also in the pen panel's advanced options). When on, new strokes from Brush, Pencil,
Stroke and the shape tools are created in a new layer placed **behind** all existing artwork of the current drawing
(useful for colouring behind line art). If both Draw Behind and Draw on Top Layer are on, Draw Behind wins.

### 8.12 Show Strokes

Persistent display toggle (default off; a drawing aid, §10.6). Shows the strokes that compose the drawn shapes,
including invisible strokes, as thin lines in a contrasting colour with open stroke ends marked (small squares) so gaps
are easy to find. Never affects rendering or export.

### 8.13 Transform

| Command | Behaviour | Undo label |
|---|---|---|
| Flip Horizontal | Mirrors the selection left↔right. | "Flip Horizontal" |
| Flip Vertical | Mirrors the selection top↔bottom. | "Flip Vertical" |
| Rotate 90° CW | Rotates the selection 90° clockwise. | "Rotate 90° CW" |
| Rotate 90° CCW | Rotates 90° counter-clockwise. | "Rotate 90° CCW" |
| Rotate 180° | Rotates 180°. | "Rotate 180°" |

The pivot is the centre of the selection's bounding box. Geometry is modified permanently; gradient/texture matrices
transform with their zones; text and bitmaps update their transforms. Free scale/rotate/skew uses the Select tool
handles or Perspective; element-level flips act on the element (section 05).

### 8.14 Group / Ungroup

Group combines the selection into one group node ("Group Selection"); Ungroup dissolves the selected groups
("Ungroup Selection"). See §3.6.

### 8.15 Feather Edges

Softens the edges of the selected filled shapes. Parameters (presented in a small dialog in the GUI):

| Parameter | Type | Range | Default | Effect |
|---|---|---|---|---|
| Step width | int (du) | 5 – 100 | 10 | Width of each feathering band. |
| Number of steps | int | 3 – 100 (recommended 3 – 30) | 5 | Number of bands. |

- For each selected filled shape (per colour), generate N concentric bands outward around its outline, each *step
  width* wide, filled with the shape's colour at linearly decreasing opacity (from (N−1)/N down to 1/N of the colour's
  alpha), so the shape fades into its surroundings. The bands are placed in a layer directly behind the shape. Runs
  with progress and can be cancelled.
- A step count above 30 requires confirmation (performance warning; `needs_confirmation` with reason
  `drawing.featherStepsHigh` for agents).
- Texture-filled shapes are skipped and the user is told that textured shapes were ignored (warning
  `drawing.featherTexturesIgnored`).
- Undo: "Feather Edges".

---

## 9. Drawing inspector

When a drawing cell is selected, the Properties view shows a **Drawing** tab:

| Item | Behaviour |
|---|---|
| Element name | Read-only. |
| Exposure | Read-only: the frame, or frame range, where the selected drawing is exposed. |
| Drawing name | Editable; renames the drawing. Names must be non-empty and must not contain `\ / : * ? < > " \|` or `.`; invalid names are refused (`drawing.nameInvalid`); a name already used in the element is refused with a message naming the conflict (`drawing.nameExists`). |
| Show preview | Toggle for the thumbnail. |
| Preview | Thumbnail of the drawing. |
| Browse | A slider to browse through the element's drawings in the preview. |

If the cell refers to a drawing that cannot be found, the tab states that the drawing is missing
(`drawing.notFound`). Related cell operations (rename cell, change label, clear, duplicate) are in section 04.

---

## 10. Drawing aids

Drawing aids are display-only: they never change the drawing and never appear in rendered output or export. Their
state is persisted per user unless stated otherwise.

### 10.1 Grids

| Setting | Values | Default | Behaviour |
|---|---|---|---|
| Grid visible | bool | off | Shows the selected grid type in the Drawing View and Camera View. |
| Grid type | `normal` \| `field12` \| `field16` | `normal` | **Normal:** uniform square grid (1:1), aligned to the drawing origin; major lines every 100 du (1 field) and minor lines every 10 du (minor lines hidden when closer than 4 view pixels). **12 Field:** traditional field guide, 4:3, 12 × 9 fields. **16 Field:** 16 × 12 fields, 4:3. |
| Overlay | bool | off (= underlay) | Overlay: grid drawn on top of the drawings (useful on colour-filled art). Underlay: grid drawn behind (useful for line art). Only meaningful while the grid is visible. |

- Choosing a grid type does not show the grid; visibility is a separate setting.
- Field guides are drawn as nested rectangles for each field size (1 … 12 or 1 … 16) centred on the origin, with a
  centre cross-hair and field numbering/graduations along the axes.

### 10.2 Onion skin

**Purpose:** show previous and/or next drawings of the current element, dimmed or tinted, while drawing the current one.

| Setting | Values | Default | Behaviour |
|---|---|---|---|
| Onion skin enabled | bool | off | Master switch. |
| Previous drawings | 0 – 3 | 1 | Number of previous drawings shown. |
| Next drawings | 0 – 3 | 0 | Number of next drawings shown. |
| Outline only | bool | off | Onion-skin drawings displayed as outlines (brush outlines and centre lines, no fills). |
| Transparency | bool | on | Onion-skin drawings rendered semi-transparent so the composition shows through. |
| Colour shading | bool | on | Onion-skin drawings tinted with the previous/next colours. |
| Previous colour | colour | red `#FF0000FF` | Tint of previous drawings. |
| Next colour | colour | green `#00C000FF` | Tint of next drawings. |

Rules:
- Previous/next are counted in **drawings**, not frames: the distinct drawings exposed before/after the current one in
  the element's exposure order (a drawing held for several frames counts once).
- Onion-skin drawings appear lighter than the current drawing; farther drawings are fainter than nearer ones
  (recommended opacity 50 %, 30 %, 15 % for distances 1, 2, 3).
- **Cycle previous / cycle next** commands step the previous (or next) count 0 → 1 → 2 → 3 → 0; dedicated commands set
  the count to 0. All are bindable.
- Onion skin applies in the Drawing View and the Camera View to the selected drawing element; onion-skin drawings are
  not editable.

### 10.3 Auto light table

- Toggle (default off). In the Drawing View (which normally shows only the current drawing) it shows **all elements as
  they are layered at the current frame**, so the user can draw in context (trace over another element, clean up over
  a rough layer).
- The other elements are not editable and, when **shade** is on (default on), are displayed washed out.
- Elements hidden in the Exposure Sheet are not shown.

### 10.4 Static light table

**Purpose:** pin specific drawings or images so they stay visible in the Drawing View regardless of the current frame or
element (model sheets, backgrounds).

- **Add:** from a selected cell (drawing or image), the user sends that drawing/image to the static light table
  (available from the cell's context menu, the element context and the main menu). The entry references its source;
  the source is not copied or modified.
- **Panel:** a list (shown on demand inside the Exposure Sheet area) with one row per entry, each with a shown/hidden
  state and an overlay/underlay placement.
- **Entry commands:** show/hide entry; show all; hide all; hide all others (show only the chosen entry); overlay all
  (display over the current drawing); underlay all (display under it); delete entry; delete all. Deleting an entry
  never deletes its source.
- **Shade** (default on): static light-table entries are displayed washed out.
- Entries track their source: when the source drawing or its colours change, the display updates. If the source is
  deleted, the entry is removed.
- The static light-table list is saved with the scene.

### 10.5 Show Current Drawing on Top

Toggle (default off), Camera View only: the drawing being edited is rendered above all other drawings regardless of
layering (§1.2).

### 10.6 Show Strokes

See §8.12.

### 10.7 Background colour and selection display

| Setting | Default | Behaviour |
|---|---|---|
| Drawing View background colour | light grey `#F0F0F0FF` | Background of the Drawing View (a user preference). |
| Use scene background colour | off | The view background shows the current scene's background colour (section 08) instead. |
| Show outline on selection | off | In the Camera View, displays the brush outlines and pencil centre lines of selected elements in the highlight colour, even where the element is hidden; requires box highlighting. |
| Box highlighting | on | In the Camera View, a box in the highlight colour is drawn around selected elements; when off, unselected elements appear faded instead. |
| Selection always visible | off | Elements selected in the Timeline/Exposure Sheet are displayed in the Camera View even if hidden. |

### 10.8 Display quality settings

The following preferences (section 01) affect how drawings are displayed, never how they are rendered for export:
antialiasing of drawings, antialiasing of tool manipulators, full-canvas antialiasing, smooth (filtered) display of
textures and bitmaps, maximum texture/image resolution used for display (a slider from faster display to better
quality), and the memory budget for bitmap material. A fallback mode that disables accelerated tool-feedback drawing
MAY be offered for systems where drawing tools respond slowly. Settings that require a restart MUST say so.

---

## 11. Bringing artwork into drawings

Generic import (sound, images, movies, layered image files, vector files as new elements) is specified in section 09.
This section covers the paths that create or fill **vector drawings**.

### 11.1 Entry points

| Capability | Target |
|---|---|
| Import and vectorize from file(s) | The selected drawing element, from the selected cell onward. |
| Import and vectorize from a capture device | Same target; each captured frame is vectorized (§11.5). |
| Import and vectorize from a scanner | Same target; each scanned page is vectorized (§11.5). |
| Convert image element to vector | Vectorizes all bitmaps of an image element (§11.3). |
| Import vector file (Illustrator `.ai` / PDF) into a drawing | The selected drawing cell (§11.4). |
| Drag-and-drop of an image file onto a view | Imports the image (section 09). |

These MUST be reachable from the main menu and from the relevant cell context menus.

### 11.2 Import and vectorize

- **Purpose:** convert scanned or bitmap drawings into vector drawings that can be painted.
- **Accepted inputs:** at least PNG, JPEG, BMP, GIF, TIFF, TGA and PSD (flattened); JPEG 2000 MAY be supported. CMYK
  images are converted to RGB first.
- **Target:** the selected drawing element; each image becomes one drawing in consecutive cells starting at the
  selected cell, creating drawings as needed. Cells that already hold drawings are replaced only after confirmation
  (`onConflict` for agents). If no element is selected a new drawing element is created. If the selected element is
  not a vector drawing element, the command is refused (`drawing.notVectorElement`).
- **Settings:**

| Setting | Values | Default | Effect |
|---|---|---|---|
| Files | list of image files | from the file picker | Images to vectorize, in order. |
| Vectorization type | `blackAndWhite` \| `withTexture` | `blackAndWhite` | **Black and white:** pixels darker than the threshold become black line art (brush-type shapes) using a black colour in a generated vectorization palette; lighter pixels become unpainted. **With texture:** line art is vectorized as above and the vector shapes are filled with the source bitmap as a texture colour, so the image's colours and greys are preserved. |
| Filter | `none` \| `smooth` \| `smoothMore` \| `edgeEnhance` \| `sharpen` | `none` | Pre-filter before thresholding: `smooth` = light mean blur (removes noise, loses some sharpness); `smoothMore` = stronger 3×3 mean blur; `edgeEnhance` = high-pass edge enhancement; `sharpen` = sharpening. |
| Threshold % | int 0 – 100 | 70 | Grey level (as % of white) separating ink from paper. Values outside the range are refused (`vectorize.thresholdOutOfRange`). |
| Preview | — | — | Renders the vectorization result of the current file with the current settings, with zoom in/out. |

- **Algorithm:** convert to greyscale, apply the selected filter, threshold, trace the boundaries of the dark regions
  into polygons, fit Béziers (§3.3, smoothness 3), build brush-type shapes in one layer; with texture, also create the
  texture colour and fill; scale the result so the image width fits the camera frame width (12 fields = 1200 du),
  centred on the origin.
- Runs as a cancellable job with progress per image ("vectorizing image n of m"). Cancelling keeps the drawings already
  completed and reports how many were done.
- After vectorizing scans drawn off-centre, Reposition All Drawings (§7.1.4) corrects them in one step.

### 11.3 Convert image element to vector

Vectorizes the bitmap images of the selected image element with the same settings as §11.2 and produces a **new
drawing element** with matching exposure; the image element is kept unchanged.

### 11.4 Import vector file into a drawing

- Imports an Illustrator (`.ai`, PDF-compatible) or PDF file into the selected drawing.
- Conversion: filled paths → zones (brush-type outlines) with their colours added to the palette (CMYK converted to RGB;
  gradients converted to gradient colours); stroked paths → pencil lines with matching thickness and colour; text →
  outlines. All layers of the source are merged into the one drawing; the user is told that importing through the
  generic import (section 09) can preserve source layers as separate elements.
- Errors: not a supported vector file (`import.notVectorFile`); version unreadable (`import.versionUnreadable`). Parser
  warnings (e.g. missing fonts) are collected and shown as a list after import (`import.warnings` with the list).

### 11.5 Capture and scanner acquisition with vectorization

Device capture (cameras/webcams) and scanner acquisition are specified in section 09 (`capture` namespace). When used
from the import-and-vectorize entry points, each captured frame or scanned page MUST be vectorized with the settings of
§11.2 into the next cell of the target drawing element. The capture function MUST offer at least: device and source
selection, resolution, noise reduction, device-specific properties, a light-table overlay of a reference
element/drawing with adjustable opacity, aspect-ratio handling (ignore / keep / keep by expanding), colour keying
(chroma RGB, chroma YUV, luma YUV; key colour picker, luma correction, matte view, hide background, minimum/maximum
tolerance with maximum default 99, post-filter dilate/erode/gaussian/mean/distance with intensity), and timed automatic
capture (interval 0 – 999, default 1, unit ms/s/min/h/day). Device errors (cannot open device, device in use, no image
received, not enough memory, scanner not ready) are reported with `capture.*` codes defined in section 09.

### 11.6 Import scaling options for bitmaps

When bitmaps are imported into an element, the import offers these scaling choices (section 09 owns the command; listed
here because they also apply to images used as light-table references and vectorization sources): fit image to camera
width (default); no change to element scale; scale so the image spans 12 fields horizontally; spans 12 fields
vertically; scale to a resolution in pixels per field; scale to a given size in fields (0 in one dimension = keep
proportions).

### 11.7 Related

- Updating drawings from library templates (choose which drawings to update, optionally update colours): section 08.
- Exporting a drawing to PDF: section 09.

---

## 12. Undo, messages and persistence

### 12.1 Undo granularity

- Every tool gesture is one undo step labelled with the tool name; every command is one step labelled with the command
  name (labels given in §7–§8 and in the command table).
- Text editing: creating a text object, a run of typing, changing width, moving and deleting text, and each text
  property change are separate steps.
- Pen edits ("Set Pen Style", "New Pen", "Delete Pen", "Rename Pen") are undoable project edits.
- Creating a drawing by drawing into an empty cell is part of the first gesture's step.
- Reposition All Drawings and Feather Edges are single steps however many objects they touch.
- Tool switching, tool option changes, drawing aids and view navigation are not undoable; static light-table edits are
  scene data but are not undoable either (design decision §14).

### 12.2 Messages (as codes)

| Code | Kind | When |
|---|---|---|
| `drawing.locked` | error | Any edit of a locked drawing. |
| `drawing.noTarget` | error | A drawing command with no current drawing and no drawing/cell given. |
| `drawing.notVectorElement` | error | Target element is not a drawing/annotation element. |
| `drawing.notFound` | error | Referenced drawing missing. |
| `drawing.objectNotFound` | error | A referenced stroke/zone/group/text/point does not exist in the drawing. |
| `drawing.nameExists` / `drawing.nameInvalid` | error | Drawing rename conflicts or invalid characters. |
| `drawing.pasteIncompatible` | error | Clipboard content cannot be pasted into a drawing. |
| `drawing.nothingToDo` | error | Command has nothing to act on (empty selection, nothing to merge). |
| `drawing.invalidGeometry` | error | Too few points, non-finite values, malformed Bézier list. |
| `drawing.staleRevision` | error | An ID target was given with a revision older than the drawing's current revision. |
| `drawing.strokesHidden` | hint | Invisible stroke added (Stroke tool, Close Gap, line unpainted) while Show Strokes is off. |
| `drawing.centreLineOnFill` | warning | Extract Centre Line applied to painted fills. |
| `drawing.featherStepsHigh` | confirmation | Feather Edges with more than 30 steps. |
| `drawing.featherTexturesIgnored` | warning | Feather Edges skipped texture-filled shapes. |
| `drawing.elementNotVisibleInDrawingMode` | warning | Elements added in drawing mode that drawing mode cannot show. |
| `drawing.cellOccupied` | confirmation | Vectorization would replace drawings in occupied cells. |
| `tool.notApplicable` | error | Tool used on an element type it cannot edit, or an option the tool does not have. |
| `tool.valueOutOfRange` | error | A tool option or pen value outside its range. |
| `pen.nameExists` / `pen.lastPen` / `pen.notFound` | error | Pen list edits. |
| `vectorize.thresholdOutOfRange` / `vectorize.unsupportedFile` | error | Import and vectorize. |
| `import.notVectorFile` / `import.versionUnreadable` | error | Vector file import. |
| `import.warnings` | warning | Parser warnings after vector import (list attached). |

Paint-tool messages (`colour.zoneNotClosed`, `colour.noGapFound`, `colour.cannotUnpaint`, `colour.missing`,
`colour.staleZoneId`) are defined in section 03. Message wording is a UX-writing task; every message shown to the user
MUST correspond to a code.

### 12.3 Persistence

| Setting | Scope | Default |
|---|---|---|
| Pen list, current brush/eraser pen | project | §6.1 |
| Symmetry mode, angle, number of mirrors | user | none, 0°, 6 |
| Draw Behind, Draw on Top Layer, Snap to Contour | user | off |
| Automatic gap-closing mode | user | off (owned by section 03) |
| Active tool, last-used tool per group, per-tool options | user | Select; first tool of each group |
| Text panel settings for new text | user | §7.6.1 |
| Show Strokes, Show Current Drawing on Top, selection display options | user | §10 |
| Grid visible/type/overlay | user | off / normal / underlay |
| Onion skin enabled/previous/next and appearance | user | §10.2 |
| Auto light table on/shade, static light-table shade | user | off / on / on |
| Static light-table entries (source, shown, placement) | scene | empty |
| Drawing View background, use scene background | user | §10.7 |
| Tablet settings, display quality settings | user | §6.3, §10.8 |
| Vectorization settings (last used) | user | §11.2 |

Every command, tool and tool-group selection MUST be bindable to keys (section 01); apart from platform-standard
shortcuts, default bindings are a UX decision (recommended: single-key bindings for the tool groups, tools and the most
common drawing aids).


---

## 13. Agent interface

### 13.1 Agent-specific requirements

1. **Everything is a command.** Every tool, tool option, pen edit, drawing command, drawing aid, inspector field and
   import path in this section MUST be reachable through the commands in §13.5 (namespaces `draw`, `tool`, `drawAid`)
   or through the section 03 painting commands. No gesture, dialog or visual-only feedback may be the only way to do
   or see something.
2. **Structured creation without tools.** Structured `draw.*` commands never require activating a tool and never
   change the active tool. They do not change the drawing selection unless documented (`draw.cut.apply`,
   `draw.selection.set`, `draw.content.add {select: true}`).
3. **Painting, gap closing, dropper and fill mapping** are section 03 commands: `colour.paint.apply` (zones, lines,
   brush strokes, text by ID, point or crossing path), `colour.paintUnpainted.apply`, `colour.paint.remove`
   (unpaint), `colour.gap.close`, `colour.pick` + `colour.current.set` (dropper), `colour.fillMapping.get/set` (Edit
   Texture), `colour.gapClosing.set/get` (automatic gap closing), `colour.selection.apply` (recolour selection). They
   address objects by the IDs returned by `draw.drawing.get` and accept its `rev`.
4. **Target addressing.** Every `draw.*` command that edits or reads a drawing accepts `drawingId?: id`; if omitted,
   it uses `elementId?` + `frame?` (the drawing exposed in that cell), and if those are omitted, the current drawing
   (§1.1). Creation commands accept `createDrawing: bool = true`: if the addressed cell is empty a drawing is created
   in the same undo step and its ID returned. No target → `drawing.noTarget`.
5. **Coordinates.** Points are in drawing space (§2.1) by default. Every point-taking `draw.*` command also accepts
   the common addressing parameters `space: enum "drawing"|"scene"|"view" = "drawing"; view?: string; frame?: int`
   (same meaning as in section 03; omitted from the table rows for brevity). View-dependent tolerances (hit
   tolerance, polyline/contour snap radius, gap tolerances) use the given view's zoom, or 100 % zoom when no view is
   given.
6. **Bézier input.** Geometry can be given either as `points` (samples, optionally with pressure; curve-fitted per
   §3.3 when `fit` is true, or joined by straight segments when false) or as `bezier` control points (§13.4, used
   verbatim). `pressure` may be given per point or as a pressure map.
7. **Gesture equivalence.** Driving an interactive tool with simulated input (`pointer`, `key`, `type_text` from
   section 11) and running the equivalent structured command (§13.3) on the same starting state MUST produce the same
   result (§13.3 defines "same").
8. **Confirmations never block.** Commands that would ask the user take `confirm: bool = false` or
   `onConflict: enum` parameters; without them they return `{needs_confirmation: {code, summary}}` and change nothing.
9. **Determinism.** Identical starting state + identical parameters MUST give identical geometry, colours and
   topology. Curve fitting, sweeping, re-noding and vectorization must not depend on timing, thread scheduling or
   display state. ID assignment SHOULD be deterministic for a given starting state and command sequence.
10. **Read-back.** `draw.drawing.get` returns the full vector content (§13.4); `draw.hitTest.get`, `draw.bounds.get`,
    `draw.selection.get` and `draw.viewMapping.get` support verification and targeting; `draw.drawing.render` returns
    a PNG of the drawing at a requested size with the pixel↔drawing mapping. Generic `render_frame` / `screenshot`
    (section 11) render whole frames or views.
11. **Errors** are returned as structured `{code, message, details}` using the codes of §12.2 (and `colour.*` codes from
    section 03). Every result of an editing command includes `{drawingId, rev}` and the IDs created, modified and
    removed.
12. **Long-running work** (vectorization, vector-file import, feathering many shapes) runs as a job: the command
    returns `{jobId}` immediately; progress, cancellation and the final result use `job_status` / `job_cancel`.
13. **Batching.** Any sequence of `draw.*` commands inside `batch` (section 11) is one undo step.

### 13.2 Dialogs and panels as commands

| GUI surface | Commands |
|---|---|
| Pen panel (list, new, delete, rename, settings, preview) | `tool.pen.list/create/delete/rename/set/select`; preview via `draw.drawing.render` of a scratch stroke is not needed — `tool.pen.list` returns all values. |
| Pen panel advanced options (symmetry, Draw Behind) | `tool.symmetry.set`, `tool.placeBehind.set` |
| Text panel | `tool.text.options.set` (defaults for new text), `draw.textFormat.set` (existing text) |
| Feather Edges dialog | `draw.feather.apply {stepWidth, steps, confirm}` |
| Import-and-vectorize settings dialog and preview | `draw.vectorize.run`, `draw.vectorize.preview` |
| Drawing inspector | `draw.drawing.rename`, `draw.drawing.render` (thumbnail), `draw.inspectorPreview.set` |
| Static light-table panel and its context menus | `drawAid.staticLightTable.*`, `drawAid.staticLightTablePanel.set` |
| Tools palette / group buttons | `tool.active.set`, `tool.group.select` |
| Context menu of the Drawing View | the corresponding `draw.*` commands and `edit.*` (section 01) |

### 13.3 Gesture equivalence

**Rules.**
- `pointer` gestures deliver exactly the given samples (no resampling) with position (view pixels), pressure (0–1,
  default 1), pointer kind (`mouse`, `pen`, `penEraser`) and modifiers. Each sample is mapped to drawing space with the
  view mapping in force at the press (`draw.viewMapping.get`); view navigation during a gesture is not allowed.
- A pointer kind of `penEraser` activates the temporary Eraser for that gesture (§5.2).
- **Same result** means: `draw.drawing.get` of both outcomes is identical (geometry and pressure values equal within
  1e-6 du, same colours, same topology, same IDs when both are run from the same starting state), and both create one
  undo step with the same label.
- For the equivalent command, current settings are made explicit: current pen, current colour, symmetry, Draw Behind,
  Draw on Top Layer, gap-closing mode, the view's zoom for tolerances (`view` parameter).

| Tool | Gesture | Equivalent structured command |
|---|---|---|
| Brush | press p₀, moves p₁…pₙ₋₁, release pₙ (pressure per sample) | `draw.stroke.add {kind:"brush", points: M(pᵢ)+pressure, fit:true, penId: current brush pen, colourId: current, layer:"auto", symmetry:true}` |
| Pencil | same | `draw.stroke.add {kind:"pencil", …, thickness: current brush pen max}` |
| Stroke | same | `draw.stroke.add {kind:"invisible", …}` |
| Rectangle | press p₀, release p₁; constrain → square; alternate → from centre | `draw.rectangle.add {from: M(p₀), to: M(p₁), square, fromCentre}` |
| Ellipse | same | `draw.ellipse.add {from, to, circle, fromCentre}` |
| Line | press p₀, release p₁; constrain → 15° steps; alternate → start at previous end | `draw.line.add {from, to, constrainAngle, continuePrevious}` |
| Polyline | clicks / click-drags, closed by clicking the first point or ended by Enter / double-click | `draw.polyline.add {points:[{x,y,outHandle?,inHandle?}], closed, snap:true}` |
| Paint | click p / drag path | `colour.paint.apply {points:[M(p)]}` / `{path}`; gradient drag → `{points, mapping:{from,to}}` |
| Unpaint | click p | `colour.paint.remove {points:[M(p)]}` |
| Paint Unpainted | click p / lasso drag | `colour.paintUnpainted.apply {points}` / `{lasso}` |
| Dropper | click p | `colour.pick {point}` then `colour.current.set {colourId}` |
| Close Gap | drag p₀ → pₙ | `colour.gap.close {from: M(p₀), to: M(pₙ)}` |
| Edit Texture | handle drags on an object | `colour.fillMapping.set {objectIds:[id], mapping}` |
| Eraser | drag with pressure | `draw.erase.apply {path: M(pᵢ)+pressure, penId: current eraser pen}` |
| Cutter | closed lasso | `draw.cut.apply {polygon: M(pᵢ)}` |
| Scissor | rectangle drag; alternate → lasso | `draw.cut.apply {rect}` / `{polygon}` |
| Select | click / marquee / lasso (alternate) / constrain-add | `draw.selection.set {by:"point"\|"rect"\|"lasso", mode}` |
| Select | drag selection, handle drags, alternate-drag copy, arrow nudge | `draw.transform.apply {translate \| scale+pivot \| rotate+pivot, copy}` |
| Select | drag drawing pivot | `draw.pivot.set {point}` |
| Contour Editor | point drag / segment drag / add point / reset or break handles / delete | `draw.point.move` / `draw.segment.move` / `draw.point.add` / `draw.point.set {handleMode}` / `draw.point.delete` |
| Perspective | corner / side handle drags | `draw.envelope.apply {corners}` |
| Reposition All Drawings | drag / handles | `draw.repositionAll.apply {translate, scale, rotate, corners}` |
| Text | click (or drag for fixed width), then `type_text` | `draw.text.add {position, text, width?}`; later edits `draw.text.set` / `draw.textFormat.set` |
| Zoom, Grabber | click / drag | `view.*` commands (section 01) |

### 13.4 JSON shapes

**Conventions.** Points are `{x, y}` in du. Affine matrices are `[a, b, c, d, e, f]` mapping (x, y) to
(a·x + c·y + e, b·x + d·y + f). A **Bézier list** is a flat list of control points: an open path of n segments has
3n + 1 points (anchor, control, control, anchor, …); a closed path has 3n points and its last segment returns to point
0. Anchors are at indices 0, 3, 6, …; an **anchor index** k refers to point 3k. A pressure map is `[{t, p}]` (§3.2).

**`draw.drawing.get` result:**

```json
{
  "drawingId": "d_81f2", "elementId": "e_12", "name": "3",
  "rev": 42, "locked": false,
  "space": {"units": "du", "duPerField": 100, "yAxis": "up", "origin": "drawingCentre"},
  "pivot": {"x": 0, "y": 0},
  "bounds": {"x": -120.5, "y": -80.0, "width": 241.0, "height": 190.2},
  "nodes": [
    {
      "type": "layer", "id": "l_1", "layerKind": "pencil",
      "strokes": [
        {"id": "s_1", "kind": "pencil", "closed": false,
         "bezier": [{"x": 0, "y": 0}, {"x": 10, "y": 5}, {"x": 20, "y": 5}, {"x": 30, "y": 0}],
         "colourId": "c_ink", "lineStyle": {"thickness": 4, "lineType": "solid"}, "penId": "p_3",
         "pressure": null, "leftZoneId": "z_2", "rightZoneId": "z_1"},
        {"id": "s_2", "kind": "invisible", "closed": false, "bezier": ["…"],
         "colourId": null, "leftZoneId": "z_2", "rightZoneId": "z_3"},
        {"id": "b_7", "kind": "brush", "closed": false,
         "bezier": ["… spine, or null once the outline was edited …"],
         "pressure": [{"t": 0, "p": 0.2}, {"t": 0.5, "p": 1.0}, {"t": 1, "p": 0.4}],
         "penId": "p_3", "pen": {"tipShape": "round", "minSize": 2, "maxSize": 8, "tipAngle": 0},
         "colourId": "c_ink", "textureMatrix": null,
         "outline": [["… outer contour, closed Bézier list …"], ["… hole contour …"]]}
      ],
      "zones": [
        {"id": "z_1", "colourId": "c_skin", "textureMatrix": null,
         "boundaryStrokeIds": ["s_1", "s_2", "b_7"],
         "holes": [{"boundaryStrokeIds": ["s_9"], "zoneId": "z_4"}],
         "parentZoneId": null, "area": 5321.7,
         "bounds": {"x": -40, "y": -30, "width": 80, "height": 60},
         "interiorPoint": {"x": 2.5, "y": -1.0}}
      ]
    },
    {"type": "group", "id": "g_1", "children": ["… layer / text / bitmap / group nodes …"]},
    {"type": "text", "id": "t_1", "text": "Hello", "transform": [1, 0, 0, 1, -50, 120],
     "widthMode": "auto", "width": null,
     "runs": [{"start": 0, "length": 5, "font": "Sans", "size": 36, "bold": false, "italic": false,
               "baseline": "normal", "kerning": 0, "autoKern": true, "colourId": "c_ink",
               "outlineColourId": null}],
     "paragraphs": [{"start": 0, "length": 5, "align": "left", "leftMargin": 0, "rightMargin": 0,
                     "indent": 0, "lineSpacing": 0}],
     "bounds": {"x": -50, "y": 84, "width": 92, "height": 36}},
    {"type": "bitmap", "id": "i_1", "mediaId": "m_5", "pixelWidth": 1024, "pixelHeight": 768,
     "transform": [1.17, 0, 0, 1.17, 0, 0]}
  ]
}
```

- `nodes` is ordered back-to-front. Object kinds and ID prefixes are illustrative; IDs are opaque.
- Stroke `kind` ∈ `brush`, `pencil`, `invisible`. For `brush`, `bezier` is the spine (null after outline edits),
  `outline` the authoritative closed contours (first = outer, rest = holes); the brush stroke's own interior is its
  fill and is **not** listed as a zone. For `pencil`, `bezier` is the centre line and `lineStyle` applies.
- Zones are the paintable regions of the layer bounded by strokes (brush outlines count as boundaries via the brush
  stroke's ID). `colourId: null` = unpainted. `interiorPoint` is guaranteed inside the zone (useful for point-based
  painting). `textureMatrix` is set only for gradient/texture fills.
- Options: `include?: [enum "zones"|"pressure"|"outline"|"bounds"]` (default all), `flatten: bool = false` (return a
  flat object list with `parentId` instead of nesting), `ids?: [id]` (return only those objects).

**Hit-test result** (`draw.hitTest.get`): `{point, hits: [{objectId, objectKind: "pencil"|"brush"|"invisible"|"zone"|
"text"|"bitmap", layerId, groupPath: [id], colourId}] (front to back), zoneId|null, revision}`. Invisible strokes are
reported only with `includeInvisible: true`.

**Render result** (`draw.drawing.render`): `{png: base64 | path, width, height, drawingToPixel: [6], rev}`.

### 13.5 Command table

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `draw.current.get` | query | Current drawing target and context | `view?: string` | `{drawingId\|null, elementId, sceneId, frame, cellEmpty: bool, locked: bool, activeView}` | n/a |
| `draw.drawing.get` | query | Full vector content as JSON (§13.4) | `drawingId?: id; elementId?: id; frame?: int; include?: [enum "zones"\|"pressure"\|"outline"\|"bounds"]; flatten: bool = false; ids?: [id]` | drawing JSON (§13.4) | n/a |
| `draw.drawing.render` | query | Render a drawing to PNG at a requested size | `drawingId?: id; width: int (1–8192); height?: int (1–8192); region: enum "bounds"\|"field12"\|"field16"\|"camera" \| object {x,y,width,height} = "bounds"; padding: number = 0; background: colour = #00000000; styleId?: id; showStrokes: bool = false; showSelection: bool = false; path?: path` | `{png?, path?, width, height, drawingToPixel: [number], rev}` | n/a |
| `draw.bounds.get` | query | Bounding box of the drawing or of given objects | `drawingId?: id; ids?: [id]; includeInvisible: bool = false` | `{bounds: {x,y,width,height}\|null}` | n/a |
| `draw.hitTest.get` | query | Objects and zone under a point, front to back | `drawingId?: id; point: point; tolerance: number = 3 (view px); includeInvisible: bool = false` | `{hits: [object], zoneId\|null, rev}` | n/a |
| `draw.viewMapping.get` | query | Mapping between a view's pixels and drawing space | `view: string = "drawing"` | `{drawingId, drawingToView: [number], viewToDrawing: [number], zoom, rotation, viewSize: {width,height}}` | n/a |
| `draw.drawing.rename` | action | Rename a drawing ("Rename Drawing") | `drawingId?: id; name: string` | `{drawingId, name}` | yes |
| `draw.lock.set` | toggle | Lock or unlock a drawing ("Lock Drawing"/"Unlock Drawing") | `drawingId?: id; locked: bool` | `{drawingId, locked}` | yes |
| `draw.pivot.set` | action | Move the drawing pivot ("Change Drawing Pivot") | `drawingId?: id; point: point` | `{pivot, rev}` | yes |
| `draw.stroke.add` | action | Add a brush stroke, pencil line or invisible stroke ("Brush Tool"/"Pencil Tool"/"Stroke Tool") | `drawingId?: id; elementId?: id; frame?: int; createDrawing: bool = true; kind: enum "brush"\|"pencil"\|"invisible" = "brush"; points?: [{x,y,pressure?}]; bezier?: [point]; closed: bool = false; pressure?: [{t,p}]; fit: bool = true; smoothness?: number (0–10) = pen's; penId?: id = current pen; pen?: object {tipShape,minSize,maxSize,tipAngle}; colourId?: id = current colour; thickness?: number (0–100) = pen max; layer: enum "auto"\|"new"\|"top"\|"behind" \| id = "auto"; symmetry: bool = true; expectRev?: int` | `{drawingId, createdDrawing: bool, strokeIds: [id], zoneIds: [id], layerId, removedIds: [id], rev, hints: [code]}` | yes |
| `draw.rectangle.add` | action | Add a rectangle as a closed pencil path ("Rectangle Tool") | `drawingId?: id; from: point; to: point; square: bool = false; fromCentre: bool = false; colourId?: id; thickness?: number; penId?: id; layer: enum "auto"\|"new"\|"top"\|"behind" \| id = "auto"; symmetry: bool = true` | `{strokeIds: [id], zoneIds: [id], layerId, rev}` | yes |
| `draw.ellipse.add` | action | Add an ellipse as a closed pencil path ("Ellipse Tool") | `drawingId?: id; from: point; to: point; circle: bool = false; fromCentre: bool = false; colourId?: id; thickness?: number; penId?: id; layer: enum "auto"\|"new"\|"top"\|"behind" \| id = "auto"; symmetry: bool = true` | `{strokeIds: [id], zoneIds: [id], layerId, rev}` | yes |
| `draw.line.add` | action | Add a straight pencil line ("Line Tool") | `drawingId?: id; from?: point; to: point; continuePrevious: bool = false; constrainAngle: bool = false; colourId?: id; thickness?: number; penId?: id; layer: enum "auto"\|"new"\|"top"\|"behind" \| id = "auto"; symmetry: bool = true` | `{strokeIds: [id], zoneIds: [id], layerId, rev}` | yes |
| `draw.polyline.add` | action | Add a polyline of straight/curved segments ("Polyline Tool") | `drawingId?: id; points: [{x,y,inHandle?: point,outHandle?: point}]; closed: bool = false; snap: bool = true; snapRadius: number = 8 (view px); colourId?: id; thickness?: number; penId?: id; layer: enum "auto"\|"new"\|"top"\|"behind" \| id = "auto"; symmetry: bool = true` | `{strokeIds: [id], zoneIds: [id], layerId, rev}` | yes |
| `draw.text.add` | action | Add a text object ("New Text Object") | `drawingId?: id; position: point; text: string; width?: number (fixed width; omit = auto); format?: object (Text panel fields); colourId?: id` | `{textId, bounds, rev}` | yes |
| `draw.text.set` | action | Replace or edit text content, width mode or position ("Typing"/"Set Text Width"/"Move Text Object") | `textId: id; text?: string; insert?: {at: int, text: string}; delete?: {start: int, length: int}; widthMode?: enum "auto"\|"fixed"; width?: number; position?: point` | `{textId, bounds, rev}` | yes |
| `draw.textFormat.set` | action | Set character/paragraph formatting of a text range ("Text Property Modified") | `textId: id; start: int = 0; length?: int (default to end); font?: string; size?: number (1–512); bold?: bool; italic?: bool; baseline?: enum "normal"\|"subscript"\|"superscript"; kerning?: int (−60–60); autoKern?: bool; align?: enum "left"\|"centre"\|"right"\|"justify"; leftMargin?: number (0–1000); rightMargin?: number (0–1000); indent?: number (−99–1000); lineSpacing?: number (−99–1000); colourId?: id; outlineColourId?: id\|null` | `{textId, rev}` | yes |
| `draw.stroke.set` | action | Replace a stroke's geometry, pressure, thickness or visibility ("Edit Stroke") | `strokeId: id; bezier?: [point]; closed?: bool; pressure?: [{t,p}]; thickness?: number (0–100); kind?: enum "pencil"\|"invisible"; expectRev?: int` | `{strokeIds: [id], removedIds: [id], rev}` | yes |
| `draw.content.add` | action | Insert drawing content given in the §13.4 JSON shape (IDs reassigned; colour recovery applies) ("Paste") | `drawingId?: id; content: object; offset: point = {0,0}; layer: enum "top"\|"behind" = "top"; select: bool = true` | `{idMap: object, rev}` | yes |
| `draw.object.delete` | action | Delete objects ("Delete Object") | `drawingId?: id; ids?: [id] = selection` | `{removedIds: [id], rev}` | yes |
| `draw.selection.set` | ui | Select drawing objects by IDs, point, rectangle, lasso, all or none | `drawingId?: id; by: enum "ids"\|"point"\|"rect"\|"lasso"\|"all"\|"none" = "ids"; ids?: [id]; point?: point; rect?: {x,y,width,height}; polygon?: [point]; rectRule: enum "inside"\|"intersect" = "inside"; mode: enum "replace"\|"add"\|"toggle"\|"remove" = "replace"` | `{selectedIds: [id], bounds}` | no |
| `draw.selection.get` | query | Current drawing selection | `drawingId?: id` | `{selectedIds: [id], bounds, selectedPoints: [{strokeId, anchor}]}` | n/a |
| `draw.pointSelection.set` | ui | Select anchor points (Contour Editor selection) | `drawingId?: id; points?: [{strokeId, anchor: int}]; rect?: {x,y,width,height}; mode: enum "replace"\|"add"\|"toggle"\|"remove" = "replace"` | `{selectedPoints: [{strokeId, anchor}]}` | no |
| `draw.transform.apply` | action | Move/scale/rotate/skew objects numerically, optionally as a copy ("Move/Scale/Rotate Object", "Copy Object") | `drawingId?: id; ids?: [id] = selection; translate?: point; scale?: {x,y}; rotate?: number (degrees, CCW); skew?: {x,y} (degrees); pivot?: point = selection bounds centre; matrix?: [number] (overrides the others); copy: bool = false` | `{ids: [id], removedIds: [id], rev}` | yes |
| `draw.flip.apply` | action | Flip selection ("Flip Horizontal"/"Flip Vertical") | `drawingId?: id; ids?: [id] = selection; axis: enum "horizontal"\|"vertical"` | `{ids: [id], rev}` | yes |
| `draw.rotate.apply` | action | Rotate selection by a fixed step ("Rotate 90° CW"/"Rotate 90° CCW"/"Rotate 180°") | `drawingId?: id; ids?: [id] = selection; by: enum "90cw"\|"90ccw"\|"180"` | `{ids: [id], rev}` | yes |
| `draw.envelope.apply` | action | Perspective/envelope distortion of the selection ("Perspective Tool") | `drawingId?: id; ids?: [id] = selection; corners: [point] (4 targets for the bounds corners: bottom-left, bottom-right, top-right, top-left); mapping: enum "auto"\|"projective"\|"bilinear" = "auto"` | `{ids: [id], rev}` | yes |
| `draw.repositionAll.apply` | action | Apply one transform to every drawing of an element ("Reposition All Drawings") | `elementId?: id = current; translate?: point; scale?: {x,y}; rotate?: number; pivot?: point = {0,0}; matrix?: [number]; corners?: [point]` | `{drawingIds: [id]}` | yes |
| `draw.point.add` | action | Add an anchor on a stroke without changing its shape ("Contour Editing") | `strokeId: id; at?: point (nearest location); segment?: int; t?: number (0–1)` | `{strokeId, anchor: int, rev}` | yes |
| `draw.point.move` | action | Move anchors (shared junction anchors move together) ("Contour Editing") | `points?: [{strokeId, anchor: int}] = point selection; delta: point; snap?: bool = Snap to Contour setting` | `{modifiedIds: [id], rev}` | yes |
| `draw.point.set` | action | Set an anchor's position and handles ("Contour Editing") | `strokeId: id; anchor: int; position?: point; inHandle?: point; outHandle?: point; handleMode?: enum "symmetric"\|"broken"\|"reset"` | `{strokeId, rev}` | yes |
| `draw.point.delete` | action | Delete anchors; strokes rebuilt or removed ("Contour Editing") | `points?: [{strokeId, anchor: int}] = point selection` | `{modifiedIds: [id], removedIds: [id], rev}` | yes |
| `draw.segment.move` | action | Reshape a segment so its point at t moves by delta ("Contour Editing") | `strokeId: id; segment: int; t: number = 0.5 (0–1); delta: point` | `{strokeId, rev}` | yes |
| `draw.erase.apply` | action | Erase along a path with the eraser tip, or everything inside a region ("Eraser Tool") | `drawingId?: id; path?: [{x,y,pressure?}]; penId?: id = current eraser pen; pen?: object; region?: [point] (closed polygon); layers: enum "all" \| [id] = "all"` | `{removedIds: [id], createdIds: [id], rev}` | yes |
| `draw.cut.apply` | action | Cut artwork along a polygon or rectangle; inside part selected or deleted ("Cutter Tool"/"Scissor Tool") | `drawingId?: id; polygon?: [point]; rect?: {x,y,width,height}; then: enum "select"\|"delete" = "select"` | `{insideIds: [id], removedIds: [id], rev}` | yes |
| `draw.group.create` | action | Group objects ("Group Selection") | `drawingId?: id; ids?: [id] = selection` | `{groupId, rev}` | yes |
| `draw.group.delete` | action | Ungroup: dissolve groups, keep members ("Ungroup Selection") | `drawingId?: id; groupIds?: [id] = selected groups` | `{memberIds: [id], rev}` | yes |
| `draw.arrange.apply` | action | Change stacking order ("Bring to Front", "Bring Forward", "Send Backward", "Send to Back") | `drawingId?: id; ids?: [id] = selection; to: enum "front"\|"forward"\|"backward"\|"back"` | `{order: [id], rev}` | yes |
| `draw.flatten.apply` | action | Merge selected layers into one ("Flatten Drawing") | `drawingId?: id; ids?: [id] = selection` | `{layerId, removedIds: [id], rev}` | yes |
| `draw.optimize.apply` | action | Merge layers only where appearance is unchanged ("Optimize Drawing") | `drawingId?: id; ids?: [id] = selection` | `{layerIds: [id], merged: int, rev}` | yes |
| `draw.smooth.apply` | action | Remove unnecessary points ("Smooth Selection") | `drawingId?: id; ids?: [id] = selection; points?: [{strokeId, anchor}]; tolerance: number = 1 (du)` | `{removedPointCount: int, rev}` | yes |
| `draw.centreLine.apply` | action | Convert brush strokes to pencil lines ("Extract Centre Line") | `drawingId?: id; ids?: [id] = selection; thickness?: number (default: average source width)` | `{strokeIds: [id], removedIds: [id], warnings: [code], rev}` | yes |
| `draw.linesToBrush.apply` | action | Convert centre lines to brush strokes ("Convert Lines to Brush") | `drawingId?: id; ids?: [id] = selection; penId?: id = current brush pen` | `{strokeIds: [id], rev}` | yes |
| `draw.breakApart.apply` | action | Convert text to per-character vector groups ("Break Text Apart") | `drawingId?: id; textIds?: [id] = selected text` | `{groupIds: [id], rev}` | yes |
| `draw.feather.apply` | job | Feather edges of filled shapes ("Feather Edges") | `drawingId?: id; ids?: [id] = selection; stepWidth: int = 10 (5–100); steps: int = 5 (3–100); confirm: bool = false` | `{jobId}` → `{layerIds: [id], skippedTextured: int, warnings: [code], rev}` or `{needs_confirmation}` | yes |
| `draw.vectorize.run` | job | Import and vectorize image files, or convert an image element, into drawings ("Import and Vectorize"); omitted options default to `import.vectorize.settings.get` (09) | `files?: [path]; imageElementId?: id; targetElementId?: id (default: selected; new element if none); startFrame?: int = selected cell; type: enum "blackAndWhite"\|"withTexture" = "blackAndWhite"; filter: enum "none"\|"smooth"\|"smoothMore"\|"edgeEnhance"\|"sharpen" = "none"; threshold: int = 70 (0–100); onConflict: enum "ask"\|"replace"\|"skip" = "ask"` | `{jobId}` → `{elementId, drawingIds: [id], completed: int, cancelled: bool}` | yes |
| `draw.vectorize.preview` | query | Preview vectorization of one image as PNG | `file?: path; imageElementId?: id; frame?: int; type: enum "blackAndWhite"\|"withTexture" = "blackAndWhite"; filter: enum "none"\|"smooth"\|"smoothMore"\|"edgeEnhance"\|"sharpen" = "none"; threshold: int = 70 (0–100); width: int = 512` | `{png, strokeCount: int}` | n/a |
| `draw.vectorFile.import` | job | Import an Illustrator/PDF file into a drawing ("Import Vector File") | `file: path; drawingId?: id; elementId?: id; frame?: int; page: int = 1` | `{jobId}` → `{drawingId, objectIds: [id], addedColourIds: [id], warnings: [string], rev}` | yes |
| `draw.inspectorPreview.set` | ui | Show/hide the drawing thumbnail in the inspector | `visible: bool` | `{visible}` | no |
| `draw.drawing.changed` | event | A drawing's content changed | — | `{drawingId, rev, commandId, createdIds: [id], modifiedIds: [id], removedIds: [id]}` | n/a |
| `draw.selection.changed` | event | Drawing selection changed | — | `{drawingId, selectedIds: [id], selectedPoints: [object]}` | n/a |
| `tool.list` | query | All drawing tools with group, option schema and current options | — | `{tools: [{toolId, group, options: object, optionSchema: object}], activeToolId}` | n/a |
| `tool.active.set` | ui | Activate a tool | `toolId: enum "select"\|"contourEditor"\|"perspective"\|"repositionAll"\|"brush"\|"pencil"\|"rectangle"\|"ellipse"\|"polyline"\|"line"\|"paint"\|"unpaint"\|"paintUnpainted"\|"dropper"\|"closeGap"\|"stroke"\|"editTexture"\|"eraser"\|"cutter"\|"scissor"\|"zoom"\|"grabber"\|"text"` | `{toolId}` | no |
| `tool.active.get` | query | Active tool (and temporary override, if any) | — | `{toolId, temporaryToolId\|null, group}` | n/a |
| `tool.group.select` | ui | Select a tool group; advance within it if already active (group cycling) | `group: enum "select"\|"brush"\|"paint"\|"eraser"\|"navigate"\|"text"; advance: bool = true` | `{toolId}` | no |
| `tool.options.get` | query | Options of a tool | `toolId: string` | `{toolId, options: object}` | n/a |
| `tool.options.set` | ui | Set any option(s) of any tool (generic form of the rows below) | `toolId: string; options: object` | `{toolId, options}` | no |
| `tool.select.options.set` | ui | Select tool options | `rectRule: enum "inside"\|"intersect" = "inside"` | `{options}` | no |
| `tool.brush.options.set` | ui | Brush options (current brush pen) | `penId: id` | `{options}` | no |
| `tool.pencil.options.set` | ui | Pencil and shape-tool line thickness source | `thicknessSource: enum "penMax"\|"fixed" = "penMax"; thickness?: number (0–100)` | `{options}` | no |
| `tool.polyline.options.set` | ui | Polyline snapping | `snap: bool = true; snapRadius: number = 8 (1–50 view px)` | `{options}` | no |
| `tool.eraser.options.set` | ui | Eraser options (current eraser pen) | `penId: id` | `{options}` | no |
| `tool.text.options.set` | ui | Text panel defaults for new text | `font?: string; size?: number (1–512) = 36; bold?: bool = false; italic?: bool = false; baseline?: enum "normal"\|"subscript"\|"superscript" = "normal"; kerning?: int (−60–60) = 0; autoKern?: bool = true; align?: enum "left"\|"centre"\|"right"\|"justify" = "left"; leftMargin?: number = 0; rightMargin?: number = 0; indent?: number = 0; lineSpacing?: number = 0; outlineColourId?: id\|null` | `{options}` | no |
| `tool.pen.list` | query | Pens of the project with all settings | `type?: enum "brush"\|"eraser"` | `{pens: [{penId, name, type, tipShape, minSize, maxSize, smoothness, tipAngle}], currentBrushPenId, currentEraserPenId}` | n/a |
| `tool.pen.create` | action | New pen from current or given pen ("New Pen") | `type: enum "brush"\|"eraser" = "brush"; fromPenId?: id = current; name?: string; settings?: object` | `{penId, name}` | yes |
| `tool.pen.delete` | action | Delete a pen ("Delete Pen") | `penId: id` | `{removedPenId, currentPenId}` | yes |
| `tool.pen.rename` | action | Rename a pen ("Rename Pen") | `penId: id; name: string` | `{penId, name}` | yes |
| `tool.pen.set` | action | Change pen settings ("Set Pen Style") | `penId: id; tipShape?: enum "round"\|"oval"\|"square"\|"flat"; minSize?: number (0–100); maxSize?: number (0–100); smoothness?: number (0–10); tipAngle?: number (0–360)` | `{pen}` | yes |
| `tool.pen.select` | ui | Make a pen current for its tool family | `penId: id` | `{penId, type}` | no |
| `tool.symmetry.set` | ui | Symmetry drawing mode | `mode: enum "none"\|"mirror"\|"kaleidoscope" = "none"; angle?: number (0–360) = 0; mirrors?: int (2–16) = 6` | `{mode, angle, mirrors}` | no |
| `tool.placeBehind.set` | toggle | Draw Behind | `enabled: bool` | `{enabled}` | no |
| `tool.drawTopLayer.set` | toggle | Draw on Top Layer | `enabled: bool` | `{enabled}` | no |
| `tool.snapToContour.set` | toggle | Snap to Contour (Contour Editor) | `enabled: bool` | `{enabled}` | no |
| `tool.tablet.set` | ui | Tablet input settings | `ignoreTablet?: bool = false; portraitCorrection?: bool = false; compatibilityMode?: bool = false; inputInterface?: string` | `{settings, restartRequired: bool}` | no |
| `tool.settings.get` | query | All global drawing-tool settings | — | `{placeBehind, drawTopLayer, snapToContour, symmetry, tablet, gapClosing}` | n/a |
| `tool.active.changed` | event | Active tool changed (including temporary overrides) | — | `{toolId, temporary: bool}` | n/a |
| `tool.options.changed` | event | A tool option or global drawing setting changed | — | `{toolId\|null, options}` | n/a |
| `tool.pen.changed` | event | Pen list or pen settings changed | — | `{penId, change: enum "created"\|"modified"\|"deleted"\|"selected"}` | n/a |
| `drawAid.state.get` | query | State of all drawing aids | — | `{grid, onionSkin, onionSkinStyle, autoLightTable, staticLightTable, lightTableShade, strokes, currentOnTop, selectionDisplay, background}` | n/a |
| `drawAid.grid.set` | ui | Grid visibility, type and overlay | `visible?: bool; type?: enum "normal"\|"field12"\|"field16"; overlay?: bool` | `{visible, type, overlay}` | no |
| `drawAid.onionSkin.set` | ui | Onion skin on/off and counts | `enabled?: bool; previous?: int (0–3); next?: int (0–3)` | `{enabled, previous, next}` | no |
| `drawAid.onionSkinStyle.set` | ui | Onion-skin appearance | `outline?: bool; transparency?: bool; colourShading?: bool; previousColour?: colour; nextColour?: colour` | `{style}` | no |
| `drawAid.onionSkinCycle.run` | ui | Cycle previous or next onion-skin count 0→1→2→3→0 | `direction: enum "previous"\|"next"` | `{previous, next}` | no |
| `drawAid.autoLightTable.set` | ui | Auto light table | `enabled: bool` | `{enabled}` | no |
| `drawAid.lightTableShade.set` | ui | Shading of auto and static light tables | `auto?: bool; static?: bool` | `{auto, static}` | no |
| `drawAid.staticLightTable.add` | action | Send a drawing or image to the static light table | `drawingId?: id; elementId?: id; frame?: int; placement: enum "overlay"\|"underlay" = "underlay"; visible: bool = true` | `{entryId}` | no |
| `drawAid.staticLightTable.remove` | action | Delete entries (sources untouched) | `entryIds?: [id]; all: bool = false` | `{removedEntryIds: [id]}` | no |
| `drawAid.staticLightTable.set` | action | Show/hide, hide all others, overlay/underlay entries | `entryIds?: [id]; all: bool = false; visible?: bool; placement?: enum "overlay"\|"underlay"; onlyVisibleEntryId?: id` | `{entries: [object]}` | no |
| `drawAid.staticLightTable.list` | query | Static light-table entries | `sceneId?: id` | `{entries: [{entryId, sourceId, sourceKind, visible, placement}]}` | n/a |
| `drawAid.staticLightTablePanel.set` | ui | Show/hide the static light-table panel | `visible: bool` | `{visible}` | no |
| `drawAid.strokes.set` | ui | Show Strokes (invisible strokes and open ends) | `visible: bool` | `{visible}` | no |
| `drawAid.currentOnTop.set` | ui | Show current drawing on top in the Camera View | `enabled: bool` | `{enabled}` | no |
| `drawAid.selectionDisplay.set` | ui | Camera View selection display options | `outlineOnSelection?: bool; boxHighlighting?: bool; keepSelectionVisible?: bool` | `{settings}` | no |
| `drawAid.background.set` | ui | Drawing View background | `colour?: colour; useSceneColour?: bool` | `{colour, useSceneColour}` | no |
| `drawAid.changed` | event | A drawing aid setting changed | — | `{aid, state: object}` | n/a |

### 13.6 Queries and events

- `draw.drawing.get` is the primary read model: drawing → nodes (layers, groups, text, bitmaps) → per layer strokes
  (brush / pencil / invisible with Bézier lists, pressure map, pen or line style, colour ID, side zones) and zones
  (colour ID, texture matrix, boundary stroke IDs, holes, interior point), plus a drawing-level `rev`. Every object
  is also reachable through generic `get_object` / `set_properties` (section 11); `set_properties` on drawing objects
  MUST go through the same validation and topology update as the `draw.*` commands.
- Verification loop recommended to agents: edit → read `rev` and created IDs from the result → `draw.drawing.get`
  or `draw.hitTest.get` → `draw.drawing.render` for a visual check.
- Events: `draw.drawing.changed` (every content change, including undo/redo, with the new revision),
  `draw.selection.changed`, `tool.active.changed`, `tool.options.changed`, `tool.pen.changed`, `drawAid.changed`;
  vectorization and feathering progress via job status.

---

## 14. Design decisions / open questions

Each item is a decision for the implementation team; the recommended default is what this section specifies.

1. **Drawing units.** 1 field = 100 du, origin at the drawing centre, y up. Alternative: pixel-based units tied to
   project resolution (rejected: drawings must be resolution independent).
2. **Brush spine retention.** Keep spine + pressure + pen on unedited brush strokes so they can be re-derived and read
   back; discard on direct outline edits.
3. **Default pens.** Brush pens max 2/4/8/16/32 du (min 25 %), eraser pens 10/20/40 du, round, smoothness 3; default
   brush pen "Pen 3", eraser "Eraser 2".
4. **Smoothness mapping.** Fitting tolerance 0.25 du + 0.5 du × smoothness; pressure smoothing window grows with
   smoothness. Tune with users.
5. **Symmetry.** Mirror axis angle measured from +y; kaleidoscope = dihedral symmetry with n mirror lines (2n images).
   Symmetry settings are global user settings, not stored per pen.
6. **Marquee rule.** Objects wholly inside are selected (option `intersect` available).
7. **Pencil and shape thickness** come from the current brush pen's maximum size (option for a fixed thickness).
8. **Unpaint on lines** turns the line into an invisible stroke (aligned with section 03); brush strokes and text
   cannot be unpainted.
9. **Draw Behind vs Draw on Top Layer.** Draw Behind wins when both are on.
10. **Stroke tool target layer.** The top-most layer whose zones the stroke crosses, else a new top layer.
11. **Feather Edges.** Outward bands with linear opacity ramp, placed in a layer directly behind the shape; hard step
    maximum 100 with confirmation above 30.
12. **Extract Centre Line.** Thickness = average source width; branches shorter than the local width are removed.
13. **Vectorization.** Cancelling keeps completed drawings; converting an image element produces a new drawing element;
    results are fitted to the 12-field width.
14. **Static light table.** Saved with the scene; its edits are not undoable.
15. **Onion-skin defaults.** Off, previous 1, next 0, transparency and colour shading on, red/green tints, opacity
    falloff 50/30/15 %.
16. **Text units.** Size, margins, indent and line spacing in du; kerning in 1/100 em.
17. **Paste offset.** +10 du x, −10 du y per repeated paste into the same drawing.
18. **Object identity.** Revision counter per drawing; stroke/text/group IDs stable when unchanged; zone IDs derived
    from boundaries (consistent with section 03's staleness rule).
19. **Grid spacing.** Normal grid 100 du major / 10 du minor; field guides 4:3.
20. **Envelope mapping.** Projective for convex quadrilaterals, bilinear otherwise.
21. **Default background colour** of the Drawing View: `#F0F0F0`.
22. **Open question:** whether drawing-view navigation state (zoom/pan/rotation) should be saved per scene; recommended:
    per user and per view, not in the project.
