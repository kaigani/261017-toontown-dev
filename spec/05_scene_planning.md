# 05 — Scene Planning (Staging), Transformation and Animation

This section specifies how elements are positioned and animated in the three-dimensional scene space: the scene-planning
(staging) views (Camera, Top and Side), the animation tools, pegs and the element hierarchy, static and animated
transformations, motion paths and motion points, keyframes, segments and interpolation, velocity, function curves and
the function editor, cameras, bones and skeletons, motion recording, and the related display options, preferences and
error conditions. It fixes the transform mathematics completely, so that an implementation and an AI agent driving it
through the command layer obtain identical, deterministic results.

Related sections: element creation, the Timeline/exposure sheet, element visibility, locking, stacking order and
start/duration editing (`04_timeline_xsheet.md`); drawing content, drawing space, grids, current-drawing-on-top,
outline-on-selection and view background (`02_drawing.md`); effect elements and their handles (`06_effects.md`); the
library and scenes (`08_library_storyline.md`); playback and rendering (`09_playback_export.md`); persistence
(`10_data_model_project_format.md`); window management, generic view navigation and preferences (`01_application_shell.md`);
the command registry (`A_command_registry.md`) and the MCP server (`11_agent_interface_mcp.md`).

---

## 1. Concepts and scene model

### 1.1 Drawing versus scene planning

The application separates two activities:

- **Drawing** — editing the artwork inside a drawing cell (section 02). Drawing tools work in the Drawing view and also
  directly in the Camera view, where they edit the current cell of the selected element through its full scene transform.
- **Scene planning (staging)** — placing, layering and animating elements inside a 3-D scene seen through a camera.

Both are available at all times from one command set; there is no global mode switch. The scene-planning tool set and
the drawing tool set are independent: each remembers its own current tool, and selecting a scene-planning tool does not
change the remembered drawing tool (and vice versa).

The scene-planning workspace consists of the **Camera view**, **Top view**, **Side view** (§3), the Timeline (section 04),
the **Properties** panel (which shows the properties of the selected node, §8, §9.4, §12.3, §13.5–13.6), and the
**function editor** (§11). A default scene-planning workspace layout containing Camera view, Timeline, Side view, Top
view, scene manager and Properties MUST be provided (layout design per section 01).

### 1.2 Scene space, axes and units

The scene is a right-handed 3-D space ("scene space", also "field space"):

| Axis | Positive direction | Negative direction | Display labels (positive / negative) | Meaning |
|---|---|---|---|---|
| X | right | left | E / W | horizontal position |
| Y | up | down | N / S | vertical position |
| Z | toward the default camera (front) | away from the camera (back) | F / B | depth (multiplane) |

Numeric fields and the function editor display distances with a direction suffix (e.g. `3.5 E`, `1.25 S`, `2 B`) when
*Show Units* is on; the suffix replaces the minus sign. Agents always use signed numbers (§19.2).

**Design decision — units.** The canonical unit of scene space is the **field**. 1 field = 1 inch = 2.54 cm, isotropic
on X, Y and Z. Positions are stored in fields (double precision) and converted only for display. Display units are
selectable: fields (default), inches, centimetres. Angles are stored and displayed in degrees (the function editor MAY
offer radians as a display alternative). Time is measured in frames (1-based integers for keyframes; real numbers for
derived times such as control-point passage times).

**Design decision — table plane and field guide.** The plane Z = 0 is the *table plane*. Through the default camera
(§2.7) the camera frame covers exactly **12 fields horizontally** on the table plane, centred on the origin; its height
is 12 × (camera height ÷ camera width) fields (9 fields for 4:3, 6.75 fields for 16:9). The 12-field and 16-field guides
of section 02 are drawn against this convention.

### 1.3 Other coordinate spaces

| Space | Origin | Axes | Unit | Used by |
|---|---|---|---|---|
| Scene (world) space | centre of the default camera frame on the table plane | X right, Y up, Z toward camera | field | all world positions, `anim.evaluate` results |
| Node-local space | the node's own origin after its local transform | as its parent, transformed | field | child placement, pivots, bone rest data |
| Parent space | the parent node's local space (scene space for top-level nodes) | — | field | a node's position/rotation/scale/skew values |
| Drawing space | drawing centre | X right, Y up | field | drawing content (section 02) |
| Camera output (render) space | top-left of the camera frame | x right, y **down** | output pixel | rendering, `anim.projection.get`, screen bounds |
| View pixels | top-left of a view's canvas | x right, y down | screen pixel | pointer input simulation (section 11) |

**Design decision — drawing space mapping.** Drawing content is expressed in fields with the origin at the drawing
centre and Y up. An element with an identity transform at Z = 0, seen through the default camera, displays its drawing
exactly as the Drawing view shows it against the 12-field guide. If section 02 adopts a different drawing unit, a single
fixed conversion factor is applied before the element transform. A bitmap image imported at native size maps one image
pixel to one camera output pixel on the table plane through the default camera (i.e. 12 ÷ camera width fields per pixel).

### 1.4 Node kinds that take part in scene planning

All of the following are **transformable nodes** (also called "nodes" below). Each is an element of the scene's element
tree (section 04) and owns the transformation block described in §1.5.

| Node kind | Visible content | Scene role |
|---|---|---|
| Drawing element (vector), image element (bitmap), media element (video), annotation element | yes | Visible content. Can be animated directly or through a parent peg. |
| Peg | no | Invisible transformation carrier. Children inherit its transformation. |
| Camera | no (frame drawn in views) | Point of view; the scene's *current camera* defines the Camera view and the exported image. |
| Skeleton (with bone group, bones and skin group) | bones drawn in views | Bone rig that deforms the artwork placed in its skin group (§13). The skeleton node itself transforms like a peg. |
| Effect elements | depends on effect | Occupy a place in the hierarchy and can have children (section 06); they pass the transform through to their children. |

There is no limit on the number of pegs, cameras or skeletons in a scene other than available memory.

### 1.5 Static (layout) and animated transformation

Every transformable node has two transformation layers:

- **Static (layout) values** — offset (X, Y, Z), rotation, scale X/Y, skew, pivots. They apply to all frames and never
  create keyframes. They are edited with the Select tool (§5.1), the Properties panel and `anim.transform.set` with
  `target: "static"`.
- **Animated values** — channels that vary over time (§1.6), edited with the keyframing tools (Transform, Rotate, Skew,
  Scale, Motion), keyframe commands, the function editor and motion recording.

**Design decision — combination rule.** For a frame *f* the *effective local values* of a node are:

| Quantity | Effective value |
|---|---|
| position | static offset + animated position (motion path, §9–§10.5); animated position is (0,0,0) when the node has no motion keyframe |
| rotation | static rotation + animated rotation (default 0) + orient-to-path angle (§8.3, when enabled) |
| scale X, scale Y | static scale × animated scale (default 1 each) |
| skew | static skew + animated skew (default 0), clamped to −89…+89° |
| pivot(s) | static pivot + animated pivot offset (pivot path, default (0,0)) |

These rules are used everywhere: in rendering, in tool feedback, in the Properties panel, in `anim.evaluate`.

### 1.6 Channels (animation functions)

Each animatable parameter is a **channel**, a function of the node's local frame. Channel identifiers are part of the
agent interface:

| Channel id | Owner | Dimension | Unit | Model |
|---|---|---|---|---|
| `position` | peg, element, camera, skeleton | 3-D (X, Y, Z) | field | motion path: TCB spline through motion points (§9) |
| `position.x`, `position.y`, `position.z` | same | 1-D projections of `position` | field | edited as projections in the function editor (§11.4) |
| `velocity` | any node with a `position` channel | 1-D | field (distance travelled along the path) | timing along the path (§10.5) |
| `pivot.x`, `pivot.y` | peg, element, skeleton | 1-D each (together: the *pivot path*) | field | 1-D function (§10.3) |
| `rotation` | peg, element, camera (roll), skeleton, bone | 1-D | degree | 1-D function |
| `scale.x`, `scale.y` | peg, element, skeleton | 1-D | factor | 1-D function |
| `skew` | peg, element, skeleton | 1-D | degree | 1-D function |
| `fov` | camera (when FOV mode is dynamic) | 1-D | degree | 1-D function |
| `bone.x`, `bone.y` | bone (root bones only, §13.2) | 1-D | field | 1-D function |
| `bone.length` | bone | 1-D | field | 1-D function |

Every channel spans the node's duration, has keyframes, and follows the node's loop setting (§8.4).

### 1.7 Motion points: keyframes and control points

A **motion point** is a point of a motion path. It is either

- a **keyframe** — *locked in time*: tied to an integer frame at which the node is exactly at that point; or
- a **control point** — *not locked in time*: shapes the path; the (possibly fractional) frame at which the node passes
  it is derived from the velocity channel (§10.5).

*Lock in time* converts a control point into a keyframe (at the nearest integer frame, §9.4); unlocking converts a
keyframe into a control point. A motion path always begins and ends with a keyframe (§9.6).

Keyframes of 1-D channels (rotation, scale, skew, pivot, FOV, bone channels) are simply key points with an integer frame
and a value. A node "has a keyframe at frame *f*" when any of its channels has a key at *f*; the Timeline (section 04)
shows which channels are keyed at each frame (position, rotation, scale, skew, pivot, velocity, and for bones position,
rotation and length), and shows on a collapsed parent that one of its descendants has a keyframe.

### 1.8 Segments

The interval between two consecutive keyframes of a channel is a **segment**. A segment is either **tweened**
(interpolated) or **constant** (the value of the start keyframe holds until the next keyframe, then jumps). For the
`position` channel the depth (Z) component can additionally be made constant independently (*Constant Z*, §9.3). Whether
new keyframes start constant segments is governed by the *Create constant keyframes* preference (§16).

---

## 2. Transform mathematics

This chapter is normative. Implementations MUST produce results equal to these formulas within 1e-6 field / 1e-6 degree
(before display rounding).

### 2.1 Conventions

- Points are column vectors (x, y, z, 1)ᵀ; transforms are 4 × 4 affine matrices applied as p′ = M · p.
- Matrices are serialised row-major as 16 numbers `[m00, m01, m02, m03, m10, …, m33]` (translation in m03, m13, m23).
- Angles are degrees. **Positive rotation is counter-clockwise** as seen from the front (from +Z looking toward −Z,
  i.e. as seen through the default camera).
- Rotation, skew and scale act only in the X/Y plane; nodes are flat planes parallel to the table plane. Only
  translation changes Z.

Elementary matrices (only the upper-left 3 × 3 and translation column shown; all others identity):

- T(x, y, z): translation.
- R(θ): rotation in X/Y — `[[cos θ, −sin θ, 0], [sin θ, cos θ, 0], [0, 0, 1]]`.
- K(κ): horizontal skew (shear) — `[[1, tan κ, 0], [0, 1, 0], [0, 0, 1]]`, so x′ = x + y·tan κ. Positive skew leans the
  top of an upright shape to the right. Shear has determinant 1, so skew preserves area.
- S(sx, sy): scale — `[[sx, 0, 0], [0, sy, 0], [0, 0, 1]]`. Negative factors mirror (flips, §6.3). |sx|, |sy| MUST be
  ≥ 0.001; smaller magnitudes are rejected with `anim.valueOutOfRange`.

### 2.2 Local transform of a node

With effective values (§1.5) — position P = (px, py, pz), rotation θ, skew κ, scale (sx, sy), rotation pivot cr and scale
pivot cs (2-D points in parent space relative to the node origin, z = 0):

```
M_local = T(P) · T(cr) · R(θ) · T(−cr) · T(cs) · K(κ) · S(sx, sy) · T(−cs)
```

i.e. scale first, then skew (both about the scale pivot), then rotation about the rotation pivot, then translation.

### 2.3 Pivots

| Node kind | Rotation pivot cr | Scale/skew pivot cs |
|---|---|---|
| Element (drawing/image/media/annotation) | *rotationPivot* + animated pivot when *Use rotation pivot* is on; otherwise equal to cs | *scalePivot* + animated pivot |
| Peg, skeleton | *pivot* + animated pivot if *Apply pivot to* ∈ {rotation, both}; else (0, 0) | *pivot* + animated pivot if *Apply pivot to* ∈ {scaling, both}; else (0, 0) |
| Camera | (0, 0) | not applicable |

Defaults: element pivots (0, 0) (= drawing centre); peg pivot (0, 0); *Apply pivot to* = rotation (pegs) and rotation
(skeletons; skeletons offer rotation or scaling only).

**Design decision — drawing-pivot option for pegs.** A peg MAY have *Use child drawing pivot* on. When on and the peg has
exactly one drawing element as direct child, the peg's static pivot is taken as that child's static scale pivot plus the
child's static X/Y offset; otherwise the option has no effect.

### 2.4 Hierarchy and world transform

```
W(node, F) = W(parent, F) · M_local(node, f_node(F))       W(scene root) = identity
```

where *F* is the scene frame and f_node(F) the node's local frame (§2.10). This applies recursively through any number of
pegs, effects, skeletons and elements (drawings may be parented to drawings for cut-out puppets). The parent's channels
are evaluated at the same scene frame as the child.

### 2.5 Decomposition (read-back)

To report world (or combined) rotation, scale and skew, the X/Y part A = `[[a, c], [b, d]]` of a matrix is decomposed as
A = R(θ) · K(κ) · S(sx, sy):

```
sx = sqrt(a² + b²);   θ = atan2(b, a)
(c′, d′) = R(−θ) · (c, d)
sy = d′;   κ = atan(c′ / sy)          (sy = 0 is impossible because |scale| ≥ 0.001)
```

A mirrored matrix (determinant < 0) therefore reports a negative sy. Agents receive both the matrix and this
decomposition from `anim.evaluate`.

### 2.6 Camera and projection

- The camera looks along its local −Z axis with local +Y up. Its **view matrix** is the inverse of its *rigid world
  transform*: **Design decision** — only translation and Z rotation are taken from the camera's world matrix
  (position = W · (0,0,0,1); roll = θ from the decomposition of W); scale and skew inherited from ancestors are ignored.
- **FOV is the horizontal field of view** of the camera frame, in degrees, range 1…179.
- Projection of a scene point p: q = V · p (camera space). Let depth d = −q.z. Points with d ≤ 0.1 field (near plane)
  are not drawn; there is no far plane. With h = tan(FOV / 2), camera output size W × H pixels:

```
xn = q.x / (d · h)                     (−1 … +1 across the frame width)
yn = q.y / (d · h · H / W)             (−1 … +1 across the frame height)
pixel x = (xn + 1) / 2 · W
pixel y = (1 − yn) / 2 · H             (y down)
```

- Consequently an element at depth Z (camera on the Z axis at distance D from the table plane) appears scaled by
  D ÷ (D − Z) relative to the table plane: F values bring it closer and enlarge it; B values shrink it.
- The camera output size and frame rate are project settings (sections 01 and 09); the camera aspect ratio = W ÷ H.

### 2.7 Default camera

**Design decision.** Default FOV = 2·atan(0.375) ≈ **41.1121°**; default camera static offset = **(0, 0, 16)** fields,
roll 0. With these values the 12-field width of the table plane exactly fills the frame (6 ÷ tan(FOV/2) = 16). Every new
scene is created with one camera named "Camera" whose parent is a peg named "Camera Peg" (identity transform), and that
camera is the scene's current camera. Animating the camera is normally done on the camera peg.

### 2.8 Stacking order and depth sorting

**Design decision.** Visible nodes are composited back to front by the world Z of their origin (smaller Z drawn first).
Nodes whose world Z differs by less than 1e-6 field are ordered by the Timeline stacking order (section 04; the
front-most layer is drawn last). Consequences:

- Siblings at equal depth follow the Timeline order (bring to front / send backward act within one hierarchy level).
- A child can be placed just in front of or behind its parent with the *depth nudge* (§6.4), which changes its static Z
  offset by a small step (default 0.01 field, perceptually not a size change).
- The *current drawing on top* display option of section 02 overrides this order for the drawing being edited.

### 2.9 Per-frame evaluation pipeline

For each scene frame *F* to display or render:

1. Determine the scene's current camera; evaluate its world matrix and its FOV (static value, or `fov` channel at *F*).
2. Walk the element tree. For every node compute the local frame *f* (§2.10), the effective values (§1.5) — position from
   the motion path at the arc length given by the velocity channel (§10.5), respecting constant segments and Constant Z;
   pivots; rotation (+ orient-to-path angle); scale; skew — then M_local (§2.2) and W (§2.4).
3. For drawing, image and media elements fetch the cell exposed at *F* (section 04); skip hidden elements.
4. For skeletons, evaluate the bones (rest and current pose) and deform the skin-group content (§13.3).
5. Apply effects (section 06), project through the camera (§2.6), depth-sort (§2.8) and composite.
6. In the scene-planning views only, draw overlays (paths, motion points, frame markers, pivots, ghosts, arms, camera
   frames, bones, envelopes). Overlays never appear in renders or exports.

### 2.10 Node timing, holding and loops

- A node has a start frame (scene frame of its local frame 1) and a duration (section 04). Local frame f = F − start + 1.
- Before the first keyframe of a channel the channel holds the first keyframe's value; after the last keyframe it holds
  the last value. For scene frames outside the node's own range its channels are evaluated at the nearest end of the
  range (held), so a peg keeps transforming its children even where the peg itself has ended.
- Loops (§8.4): with a cycle length L, the local frame is mapped to ((f − 1) mod L) + 1 before evaluation.

---

## 3. Scene-planning views

### 3.1 Common behaviour

- The Camera, Top and Side views are canvas views of the same kind as the Drawing view. Any such view window can be
  switched between the types Drawing, Camera, Top and Side (`anim.view.type.set`); the switch MUST be reachable from the
  view itself.
- Generic navigation (pan, zoom in/out, reset zoom, rotate view, reset rotation, recentre, reset view to camera framing,
  centre on pointer, zoom and pan tools, temporary pan while a key is held) is specified in section 01 and applies to
  all three views. In scene views the pan and zoom tools operate on the scene canvas.
- **Switch active view.** Each view window keeps two view-state slots A and B; each slot stores view type, zoom, pan and
  view rotation. The *switch active view* command stores the current state in the active slot and restores the other
  slot (first use copies the current state to both).
- Opening/closing the Camera, Top and Side views is a window command (section 01).

### 3.2 Camera view

- Shows the scene through the **current camera**: the image that will be rendered and exported.
- Draws: all visible elements composited with hierarchy transforms, perspective from depth and a simplified version of
  effects suitable for interactive playback; the **camera frame** (rectangle of the render area; the area outside the
  frame is dimmed); motion paths, motion points, frame markers, pivots, ghosts and arms according to §4; selection
  highlighting; the grid and overlay/underlay of section 02; the view background (scene background colour or default
  colour, section 02).
- Selection highlighting: with the *Use box highlighting* preference on, a coloured box surrounds the selection (and with
  *show outline on selection*, section 02, the outlines/centre lines of selected artwork are drawn); with it off,
  unselected content is shown faded relative to the selection.
- Drawing tools work in the Camera view on the current cell of the selected element (section 02).
- Locked elements cannot be picked in the Camera view (section 04).
- Selecting in the view selects the node in the Timeline and shows its properties; the Timeline can be scrolled to the
  selected node (section 04).
- The context menu of the view MUST offer at least: cut, copy, paste, select all, deselect all, stacking order,
  collapse/expand, hierarchy navigation (§7.2), attach to next object, detach from parent, previous/next motion point,
  lock and unlock all, and show/hide of the display options of §4.

### 3.3 Top view

- Orthographic view from above. **Design decision:** screen right = +X (E), screen up = −Z (B); the camera therefore
  appears toward the bottom of the view looking up the screen.
- Elements appear as horizontal line segments covering their world X extent at their world Z depth (as if looking at
  cels edge-on). The camera is drawn as a camera symbol with its horizontal field-of-view wedge, whose edges carry
  handles that change the FOV (§12.4). Motion paths are drawn projected onto X/Z. The selection is highlighted.
- Typical uses: placing elements in depth, moving and animating the camera.

### 3.4 Side view

- Orthographic view from the right-hand side. **Design decision:** screen right = −Z (B), screen up = +Y (N); the camera
  appears at the left looking right.
- Same content as the Top view, projected onto Z/Y; the camera wedge shows the vertical field of view.

### 3.5 Interaction rules common to the scene views

- A drag changes only the two axes visible in the view: Camera view → X/Y; Top view → X/Z; Side view → Z/Y. In the
  Camera view the drag delta is converted from view pixels to the moved node's parent space at the node's depth, so the
  node stays under the pointer.
- Arrow keys nudge the selection along the view's axes: **Design decision:** 1 view pixel converted to fields per press,
  10× with the coarse modifier held. In the Camera view, the depth-nudge commands move the selection along Z (§6.4).
- A press becomes a drag only after the pointer moves more than **3 view pixels** (design decision).
- Hit testing MUST be precise for thin paths, small points and handles; **design decision:** pick tolerance 4 view
  pixels. When several pickable items overlap, priority is: handle of the current tool > motion point of the selected
  node > frame marker > motion point of other displayed nodes > path > bone > element content (front-most first) >
  camera.
- Pickable items: element content, pegs (via path or pivot), motion points, frame markers, peg ghosts, element arms,
  cameras (symbol and FOV handles), bones and their handles, effect handles (section 06).
- The pointer cursor MUST indicate the operation available under it (move, rotate, scale — oriented in 15° steps to the
  handle direction — skew, add keyframe, add control point, drag/copy, pan, zoom in/out).

---

## 4. Display options

Global display settings of the scene views, persisted with the user settings (not the project). Each is a boolean set
with `anim.display.<option>.set`.

| Option | Default | Effect |
|---|---|---|
| Show all pegs | off | Displays the motion paths and motion points of every peg regardless of each peg's own path-display toggle in the Timeline. When off, paths are shown for selected nodes and for nodes whose path display is switched on. |
| Pivot path | off | Displays the path followed by a peg's pivot over the frames (exists when the pivot is animated or offset while the peg moves along its path). |
| Peg ghosts | off | For each element attached to a displayed peg, displays the path that element's origin actually follows in the scene (the peg path combined with the element's offset from the peg). Ghosts are pickable; dragging a ghost changes the child's static X/Y offset by the drag delta expressed in the parent's space (one undo step). |
| Element arms | off | While rotating a peg with several attached elements, displays an arm (line from the rotation pivot to each attached element's origin, ending in a handle); any arm can be grabbed to rotate the peg. |
| Frame markers | on | Displays a tick on motion paths at the position of the node at every integer frame of its range. Close ticks show slow motion, distant ticks fast motion. Ticks are pickable: picking one sets the current frame to that frame. |
| Effects in views | on | Shows or suppresses the in-view display of effects (section 06). Never affects rendering/export. |
| Bone envelopes | off | Shows the influence zones of bones (§13.3) in the Camera view. |
| Smooth paths | on | Anti-aliases motion paths in the views. |
| Smooth tool furniture | on | Anti-aliases tool furniture (rotation circle, handles). |

Element show/hide, annotation visibility (section 04), the grid and field guides, current-drawing-on-top,
outline-on-selection and the view background colour (section 02) are owned by those sections. No title-safe/action-safe
overlay is required (it MAY be added as a further display option).

---

## 5. Animation tools

### 5.0 Rules common to all scene tools

| Rule | Requirement |
|---|---|
| Tool set | Select, Transform, Motion recording, Rotate, Skew, Scale, Motion, plus the bone tools (§13.4). Exactly one scene tool is active. A *cycle rotate tools* command alternates between Rotate and Skew. |
| Temporary select | While the *temporary-select modifier* is held with Rotate, Scale, Skew, Transform, Motion, pan or zoom active, the Select tool is active; releasing it restores the previous tool. |
| Temporary skew | While the *alternate modifier* is held with the Transform tool active, the Skew tool is active. |
| Selection sync | Selecting a node in a view selects it in the Timeline and shows its properties; selecting in the Timeline selects it in the views. |
| Multiple selection | Clicking with the *extend modifier* adds/removes nodes; dragging in empty space with Select or Transform draws a rubber-band rectangle selecting every pickable element touched. Select all selects all elements of the scene as one transform target. |
| Keyframe target | Keyframing tools (Transform, Rotate, Skew, Scale, Motion) write keys at the **current frame** on the selected node(s), or on their parent pegs in peg-only mode (§6.1). The Select tool never creates keys. |
| Keyed channels | **Design decision:** a keyframing drag keys only the channels it changes (translate → `position`; rotate → `rotation`; scale → `scale.x`/`scale.y`; skew → `skew`; pivot move by Rotate/Scale with an animated pivot → `pivot.x`/`pivot.y`). The *add keyframe* command keys all channels (full pose, §10.1). |
| Live feedback | Views update live during a drag; the whole drag commits as **one** undoable step on release. Cancelling (escape) during a drag restores the state before the drag. |
| Locked elements | Cannot be picked in the views. |

### 5.1 Select (layout)

- Purpose: select nodes and set their **static** position, rotation, scale and skew without creating keyframes.
- A bounding box with handles appears around the selection. Drag inside → move (view axes); drag a corner or side handle
  → scale (corner with the *constrain modifier* keeps the aspect ratio); drag the rotation handle → rotate (constrain
  modifier snaps to 15° steps); drag a side skew handle → skew.
- Works on elements, pegs (via path/pivot), cameras and skeletons: it edits their static values.

### 5.2 Transform

- Purpose: select, move, rotate, scale and skew nodes and **key the change at the current frame**.
- Furniture around the selection: bounding box (union of the selected nodes' boxes), scale handles at corners and side
  midpoints, a rotation handle, skew handles on the sides, and the pivot marker (draggable).
- Gestures: drag inside → translate (keys `position`); drag a scale handle → scale (constrain modifier keeps proportions);
  drag the rotation handle → rotate about the rotation pivot (constrain modifier snaps to 15°); drag a skew handle →
  skew; drag the pivot → moves the static pivot (no key); depth-nudge commands (§6.4) act on the selection.
- If a child inside a collapsed hierarchy is keyed, the Timeline shows the child-keyframe indicator on the collapsed
  ancestor (section 04).

### 5.3 Rotate

- Draws a rotation circle around the rotation pivot; dragging anywhere around it rotates and keys `rotation` at the
  current frame. Dragging the pivot marker relocates the pivot (static; or keyed `pivot.x`/`pivot.y` when the node
  already has pivot keys). With element arms displayed, any arm can be grabbed as the rotation handle.
- Moving the pivot of a peg at different frames while it travels along its path creates a pivot path, displayed with the
  *pivot path* option.

### 5.4 Skew

- Top/bottom side handles apply horizontal shear (the `skew` channel). Left/right side handles apply a vertical shear;
  **design decision:** a vertical shear is converted to the equivalent rotation + skew + scale using the decomposition of
  §2.5 and keys those channels. Skew preserves area. Keys at the current frame.

### 5.5 Scale

- Scale handles on the bounding box (cursor oriented to the handle in 15° steps); the constrain modifier keeps
  proportions; the scale pivot can be moved by dragging it (static, or keyed when pivot keys exist).
- On a bone, Scale grabs the bone tip and changes only that bone's length (`bone.length`); children keep their length
  and follow the moved tip.

### 5.6 Motion

- Purpose: add, delete and move motion points of the selected node's motion path (pegs, elements, cameras, skeletons).
- Displays for the selected node(s): the path, motion points (**keyframes as filled squares, control points as hollow
  smaller squares**), frame markers when enabled, and a current-frame indicator on the path.
- Gestures:
  - Click a motion point → select it (its properties are shown, §9.4). Previous/next motion point commands step through
    the points.
  - Drag a motion point → move it in the view's two axes (Top view X/Z, Side view Z/Y).
  - Click on the path between points → insert a **control point** at the clicked curve location (§9.5).
  - Click on the current-frame indicator, or click with the *add-key modifier* anywhere → add a **keyframe** at the
    current frame (at the clicked position, or at the current position for the indicator).
  - Drag the node itself (content, camera symbol) → if a motion keyframe exists at the current frame it is moved;
    otherwise a motion keyframe is created at the current frame at the new position.
  - Delete removes the selected motion point(s) (§9.6).
- While a motion point is selected, the generic edit commands cut/copy/paste/delete act on points (§15).

### 5.7 Pan and zoom in scene views

The pan tool (also temporarily while the pan key is held) and the zoom tool (click to zoom in, drag a rectangle to zoom
to it, with the alternate modifier to zoom out; mouse wheel zooms) are the generic tools of section 01. A *cycle zoom
tools* command alternates them.

---

## 6. Scene modes and related commands

### 6.1 Peg-only mode

- Toggle (default off, persisted with user settings). When on, picking an element in a scene view selects the element's
  **direct parent peg** as the transformation target (the element acts as a pick handle); keyframing tools key that peg;
  motion points of that peg can be selected through the child.
- **Design decision:** elements without a parent peg cannot be picked while the mode is on (the pick is ignored and the
  status area explains why).

### 6.2 Select current camera

Selects the current camera of the scene (in the views and the Timeline) and shows its properties, so its frame and
position can be modified.

### 6.3 Flip horizontal / vertical (scene)

- Mirrors the selected node(s) left↔right (resp. top↔bottom): negates static scale X (resp. Y) about the scale pivot.
  When a keyframing tool is active the flip is applied to the animated scale channel and keyed at the current frame
  instead. One undo step.
- Distinct from flipping drawing objects inside a drawing (section 02).

### 6.4 Depth nudge

Moves the selected node(s) forward (+Z) or backward (−Z) by the depth-nudge step (preference, default 0.01 field) in
their static Z offset, typically to place an attached piece in front of or behind its parent (§2.8). Repeating moves
further.

### 6.5 Snap last keyframe

Toggle (default off). **Design decision:** when on, whenever a node's channels are extended to a later end frame
(duration increase, extend-children-exposure, motion recording), the newly created last motion keyframe takes the same X
and Y as the keyframe before it, so the node holds still at the end.

### 6.6 Lock

A locked element cannot be picked or edited interactively in the Camera view. Commands addressing it by id still act on
it (locking is an interaction aid). Lock data and commands are in section 04.

---

## 7. Pegs and the element hierarchy

### 7.1 Pegs

- A **peg** is an invisible node whose purpose is to carry an animated transformation (motion path with velocity, pivot
  path, rotation, scale, skew) applied to everything attached under it. It has a start frame, a duration and a loop
  setting, and appears in the Timeline with its own icon and the peg element colour (section 04).
- A cut-out character normally uses a **master peg** above all its parts to move and scale the whole character; its
  pivot is placed between the feet.
- **Design decision — new peg defaults:** inserted directly above the selected node at the same hierarchy level (top of
  the scene if nothing is selected); start = first frame of the scene; duration = scene length; no keyframes; identity
  static transform; pivot (0, 0); *apply pivot to* rotation.

### 7.2 Hierarchy commands

| Capability | Behaviour |
|---|---|
| Add peg | Creates a peg (§7.1). |
| Add parent peg | Creates a peg that takes the place of the selected node(s) in the hierarchy and attaches them to it (one undo step). With several selected nodes at different levels, the peg is placed at the level of the top-most selected node. |
| Attach to next object | Attaches the selected node to the nearest peg or effect: **design decision:** the closest peg or effect node above it in the Timeline at the same hierarchy level. If none exists the command fails with `anim.noAttachTarget`. |
| Detach from parent | Detaches the selected node from its parent peg/effect; it moves up one hierarchy level, placed directly above its former parent. |
| Attach / reparent (generic) | Attaches nodes under any peg, effect, skeleton skin group or element that accepts children; dropping a layer *onto* another in the Timeline attaches, dropping *between* layers only reorders (section 04). Creating a cycle is refused with `anim.hierarchyCycle`. |
| Keep world placement | **Design decision:** interactive attach/detach does not change the child's local values, so its on-screen placement changes by the new parent's transform. The command layer additionally offers `keepWorld: true`, which recomputes the child's static values (and offsets every key of its position, rotation, scale and skew channels by the same correction) so that its world transform at the current frame is unchanged; a skew/scale combination that cannot be represented exactly is approximated with the decomposition of §2.5 and reported as `approximated: true`. |
| Select parent / children / first child / previous sibling / next sibling | Selection navigation through the hierarchy (selects the parent; the node and all its descendants; the first attached child; the previous or next node attached to the same parent). |
| Collapse / expand | Hides or shows the rows of attached children in the Timeline (section 04). |
| Extend children exposure | Extends the exposure of a peg's descendant drawing elements to a given frame (section 04 performs the exposure change). |

### 7.3 Inheritance rules

- World transform as in §2.4, recursively.
- A child's timing is independent of its parent's; both are evaluated at the same scene frame.
- To combine several motions (e.g. a trajectory plus a rotation plus a scale), stack pegs: the main trajectory goes on
  the top-most peg, rotation and scale on lower pegs, so that rotation/scale do not swing the whole trajectory.

### 7.4 Reference workflow (acceptance scenario: cut-out puppet)

An implementation MUST support this end-to-end workflow interactively and through commands:

1. Draw one drawing element per moving part (e.g. head, torso, two arms, two legs), each drawn as a complete shape
   overlapping its neighbours.
2. Reorder layers by dropping between layers; attach pieces by dropping onto a layer (e.g. forearm onto upper arm; the
   attached layer is indented). Use the depth nudge to put an attached piece in front of or behind its parent.
3. Add a parent peg as master peg, rename it, attach all parts to it.
4. With the Rotate tool, move each part's pivot to its articulation (shoulder, elbow, centre of eyes and mouth, between
   the feet for the master peg) and test by rotating.
5. With the Transform tool go to a frame, select parts (expanding the collapsed puppet), rotate/skew/scale/translate:
   keys are created on the changed parts and the collapsed master shows that a child is keyed. Repeat at other frames.
6. To block a pose, select a frame and *add keyframe* on the collapsed puppet: every node of the puppet gets a full-pose
   key at that frame.

---

## 8. Node properties

Fields in the Properties panel commit when confirmed or when the field loses focus; numeric fields display in the chosen
coordinate unit; invalid values are rejected with `anim.valueOutOfRange` (the permitted minimum/maximum is reported) and
the previous value is restored. Each commit is one undo step.

### 8.1 Element properties (drawing, image, media, annotation)

| Property | Type / range | Default | Effect |
|---|---|---|---|
| Name, start, duration | read-only here | — | Renaming and timing are section 04 commands. |
| Edit velocity | action | — | Opens the function editor on the element's `velocity` channel. |
| Element role | enum `normal` \| `foreground` \| `background` \| `envelope` | normal | Composite role; semantics in section 04 §6.1 (foreground/background composite in front of/behind all normal elements; background auto-extends to the scene length; envelope marks a bone deformation skin). |
| Rotation | degrees, any real | 0 | Static rotation. |
| Skew | degrees, −89…89 | 0 | Static skew. |
| Offset X / Y / Z | distance | 0, 0, 0 | Static position; Z = depth. |
| Scale X / Y | factor, \|s\| ≥ 0.001 | 1, 1 | Static scale. |
| Keep proportions | bool | off | When on, editing one scale value changes the other to keep their ratio. |
| Use rotation pivot | bool | off | When on, rotation uses the rotation pivot; otherwise the scale pivot. |
| Rotation pivot X / Y | distance | 0, 0 | Static rotation pivot. |
| Scale pivot X / Y | distance | 0, 0 | Static scale/skew pivot. |
| Orient to path | bool | off | §8.3. |
| Deformable | bool | on for skin-group members | Whether a skeleton deforms the element (§13.3). |

Every element also owns the animated channels of §1.6 (it can be animated directly like a peg).

### 8.2 Peg properties

| Property | Type / range | Default | Effect |
|---|---|---|---|
| Name, start, duration | read-only here | — | Section 04 commands. |
| Loop state | read-only | not looped | Shows "not looped" or the cycle length in frames (§8.4). |
| Edit velocity | action | — | Opens the function editor on the peg's `velocity` channel. |
| Offset X / Y / Z | distance | 0 | Static position (set by the Select tool or a command). |
| Scale X / Y + keep proportions | factor, bool | 1, 1, off | Static scale. |
| Skew | degrees −89…89 | 0 | Static skew. |
| Rotation | degrees | 0 | Static rotation. |
| Pivot X / Y | distance | 0, 0 | Static pivot. |
| Apply pivot to | enum `rotation` \| `scaling` \| `both` | rotation | §2.3. |
| Constant Z | bool | off | Z of the motion path steps at keyframes instead of interpolating (§9.3). |
| Orient to path | bool | off | §8.3. |
| Use child drawing pivot | bool | off | §2.3. |
| Group on vector export / add composite for export group | bool, bool | off, off | Export grouping options for the peg's subtree used by vector (SWF) export (section 09). |

Per-keyframe values (position, rotation, scale, pivot at a keyframe) are edited in the motion point properties (§9.4) and
the function editor (§11).

### 8.3 Orient to path

- When on, the node's effective rotation at frame *f* includes the angle α of the motion-path tangent at the position for
  *f*: α = atan2(dy, dx) of the path derivative in the X/Y plane (parent space; Z ignored).
- The artwork must face +X (point right) for the result to look correct.
- **Design decision — edge cases:** on a constant segment, or where |(dx, dy)| < 1e-9, α is the last valid angle found
  by searching backward in time along the path; if none exists, the next valid angle forward; if the path has no valid
  tangent at all, α = 0.
- Available on elements, pegs and skeletons; commonly used with motion recording (§14).

### 8.4 Duration, stretching and loops

- Start frame and duration of pegs, cameras, effects, skeletons and bones are edited with the section 04 timing
  commands. **Design decision:** changing a duration through the Timeline extends or truncates channels (keys beyond the
  new end are removed; the value at the new end is held). Proportional re-timing is a separate command
  (`anim.stretch.apply`): a key at local frame k maps to round(1 + (k − 1) · (newDuration − 1) ÷ (oldDuration − 1));
  when two keys of one channel collide, the key that was later before re-timing wins.
- **Loops.** A peg, effect, skeleton or bone can repeat its animation. Parameters: `loops` (integer ≥ 1; 1 = not looped)
  and `loopType`:
  - `count` — the cycle length L is the duration before looping; the node's duration becomes `loops × L`.
  - `fill` — the cycle length L is kept and the number of cycles follows from the node's current duration (duration ÷ L;
    the last cycle may be partial).
- Position looping mode `positionLoop`: `cycle` (default; every cycle restarts from the first position) or `offset`
  (cycle k ≥ 1 is shifted by k × (end position − start position) of the first cycle, so that e.g. a walk keeps
  advancing).
- The node's properties display "looped over L frames" when looped.

### 8.5 Reset and remove all keyframes

| Capability | Effect |
|---|---|
| Reset | Removes all keyframes of the selected nodes and resets their static values to defaults (identity, pivots 0, flags off). |
| Remove all keyframes | Removes all keys of all channels of the selected nodes; static values are kept. |

---

## 9. Motion paths

### 9.1 Curve model (Kochanek–Bartels / TCB spline)

A motion path is a 3-D curve through its ordered motion points P₀ … Pₙ (n ≥ 0); each point carries tension t, continuity
c and bias b, each in −1…+1 (defaults from preferences, §16). Meaning: tension +1 → sharp corner at the point (negative
values → rounder, overshooting); continuity 0 → smooth, toward ±1 → abrupt change of direction (jumps, punches); bias 0
→ neither side favoured, toward ±1 → the path leans toward one side of the point.

For a point Pᵢ with neighbours Pᵢ₋₁ and Pᵢ₊₁:

```
outgoing tangent  Dout(i) = (1−t)(1+b)(1+c)/2 · (P[i] − P[i−1]) + (1−t)(1−b)(1−c)/2 · (P[i+1] − P[i])
incoming tangent  Din(i)  = (1−t)(1+b)(1−c)/2 · (P[i] − P[i−1]) + (1−t)(1−b)(1+c)/2 · (P[i+1] − P[i])
```

**Design decision — end points:** phantom neighbours are reflected: P[−1] = 2·P[0] − P[1] and P[n+1] = 2·P[n] − P[n−1],
then the same formulas apply. A single-point path is a constant position; a two-point path with t = c = b = 0 is a
straight line.

Segment i (from P[i] to P[i+1]), parameter s ∈ [0, 1], cubic Hermite:

```
C(s) = (2s³ − 3s² + 1)·P[i] + (s³ − 2s² + s)·Dout(i) + (−2s³ + 3s²)·P[i+1] + (s³ − s²)·Din(i+1)
```

With t = c = b = 0 everywhere the curve is a Catmull–Rom spline. Motion-point coordinates are in the node's parent space
and are *animated* positions (the static offset is added per §1.5).

### 9.2 Arc length

**Design decision:** each segment is sampled at 64 uniform parameter steps (65 points); the cumulative chord length of
these samples defines arc length. To find the point at arc length s, locate the bracketing samples, interpolate the
parameter linearly between them, and evaluate C exactly at that parameter. The same sampling is used for display,
picking, frame markers and timing, so all clients agree. Paths are drawn as polylines of at least these samples.

### 9.3 Constant segments and Constant Z

- **Constant segment:** for f in [f_j, f_j+1) the position equals keyframe j's position, jumping to keyframe j+1 at
  f_j+1. The path segment is still drawn (dashed) but not travelled; control points inside it are ignored for timing.
- **Constant Z** (per node, or per keyframe segment via *toggle constant Z*): X and Y follow the path and timing
  normally; Z equals the Z of the segment's start keyframe until the next keyframe. The per-node flag applies to all
  segments, the per-segment flag to that segment only. The Z of control points is ignored where Constant Z applies.

### 9.4 Motion point properties

Shown when a motion point is selected; also fully available through `anim.path.point.set`.

| Property | Type / range | Default | Effect |
|---|---|---|---|
| Owner | read-only | — | The node owning the path. |
| Frame | number | — | Keyframe: its integer frame; entering a new value moves it in time. The value must be an integer (`anim.frameNotInteger`), inside the node's range and between its neighbouring keyframes (`anim.frameInvalid`) and not already keyed (`anim.keyExists`). Control point: the derived passage frame (read-only, real). |
| Lock in time | bool | — | On = keyframe; off = control point. Locking a control point makes it a keyframe at round(derived frame) (`anim.keyExists` if occupied). Unlocking the first or last keyframe of a path is refused (`anim.pathEndKey`). |
| Constant segment | bool | preference *Create constant keyframes* | Keyframes only: the segment starting here is constant. |
| Position X / Y / Z | distance | point position | Moves the point (animated position). |
| Scale X / Y | factor | value at that frame | Keyframes only: sets (creating if needed) the `scale.x` / `scale.y` keys at the keyframe's frame. |
| Rotation | degrees | value at that frame | Keyframes only: sets the `rotation` key at that frame. |
| Pivot X / Y | distance | value at that frame | Keyframes only: sets the `pivot.x` / `pivot.y` keys at that frame. |
| Tension, continuity, bias | −1…1 | preferences (0) | TCB parameters. Interactive sliders update the path live and commit one undo step on release. With several points selected the change applies to all of them. |
| Add point / remove point / previous / next | actions | — | §9.5, §9.6. |

### 9.5 Adding motion points

- **Add motion point** takes a point type (`control` or `key`) and a frame. Keyframe frames MUST be integers
  (`anim.frameNotInteger`), within the node's range (`anim.frameInvalid`) and not already keyed (`anim.keyExists`).
  Control-point frames may be fractional. The point is inserted at the position the node currently occupies at that
  frame, between the motion points it lies between; its TCB values come from the preferences.
- **Inserting on the path** (Motion tool click, or `anim.path.point.add` with `at: {segment, s}`) inserts a control
  point at that curve location. **Design decision:** the curve is not re-fitted, so neighbouring segments may change
  slightly; the frames of existing keyframes are unchanged.
- Adding a keyframe after the last keyframe appends a new end point; before the first, a new start point.

### 9.6 Navigating, deleting and path integrity

- Previous/next motion point select the adjacent point. **Design decision:** when the newly selected point is a
  keyframe, the current frame moves to its frame.
- Deleting a motion point removes it; the other points keep their positions; control-point passage frames are
  recomputed. Deleting the last remaining keyframe removes the `position` animation entirely.
- **Integrity rule:** a path always starts and ends with a keyframe, and control points lie only between keyframes. If
  an operation leaves control points after the last keyframe (or before the first), the last (first) motion point is
  converted into a keyframe at round(its derived frame) (moved to the next free frame if occupied) and the user is
  informed (warning `anim.pathEndConverted`; agents receive it in the result's `warnings`).
- Moving a keyframe in time in the Timeline (section 04) is the same operation as editing its frame.

---

## 10. Keyframes, segments and timing

### 10.1 Keyframe operations

| Capability | Behaviour |
|---|---|
| Add keyframe | Adds a key at the given frame to every channel of the selected node(s), storing the current evaluated values (motion position, rotation, scale, skew, pivot, velocity pin; for bones position, rotation and length), so that the pose at that frame is blocked. On a collapsed hierarchy every node of the hierarchy is keyed. Existing keys at that frame keep their values. |
| Remove keyframe | Removes the keys of the selected node at the given frame from the position, rotation, scale and skew channels. **Velocity and pivot keys at that frame are kept** (remove them in the function editor or by naming the channels explicitly). |
| Set constant / non-constant segment | Makes the segment starting at the selected keyframe constant / tweened, for all channels keyed at that frame (or a given channel list). |
| Toggle constant Z | Makes Z constant from the selected keyframe until the next keyframe; if already constant, Z interpolates again. |
| Uniform left / uniform right | Makes the segment left (right) of the selected keyframe uniform: its curve becomes linear (constant speed for velocity; a straight line for 1-D channels). |
| Add keys at segment bounds | Adds keyframes at the left, right or both bounding frames of the selected segment with the current values. |
| Move keyframe | Moves a node's keys from one frame to another (all channels or a list); refused if a key exists at the destination (`anim.keyExists`) or the move would cross a neighbouring key (`anim.frameInvalid`). |
| Remove all keyframes, reset | §8.5. |

Frames before the first keyframe hold the first keyframe's values (§2.10). Copying, pasting and dragging keyframes in the
Timeline with per-channel options (motion, rotation, scale, pivot, velocity and skew keyframes; extra keys at the
selection bounds; substitution mode) are specified in section 04 and implemented with these operations.

### 10.2 Defaults for new keys

- *Create linear spline* preference (default on): new segments of `velocity`, `rotation`, `scale.*`, `skew`, `pivot.*`,
  `fov` and bone channels are linear. When off they get the `easeInOut` preset (§10.4).
- *Create constant keyframes* preference (default off): when on, new keyframes start constant segments.
- The `velocity` channel can never be constant (it would stall the path); constant flags for it are refused with
  `anim.channelUnsupported`.

### 10.3 The 1-D function model

Every 1-D channel is a list of key points `{frame, value, constantSegment, handles}`; between two keys k0 = (t0, v0) and
k1 = (t1, v1) the shape depends on the channel's **curve type**:

| Curve type | Shape between keys |
|---|---|
| `spline` | Cubic Bézier in (frame, value) space with P0 = (t0, v0), P1 = (t0 + a, v0 + b) (right handle of k0), P2 = (t1 + c, v1 + d) (left handle of k1), P3 = (t1, v1). Handle frame offsets are clamped to 0 ≤ a ≤ t1 − t0 and −(t1 − t0) ≤ c ≤ 0, which keeps the curve a function of time. To evaluate frame f, solve x(u) = f for u ∈ [0, 1] (unique by monotonicity; to 1e-9) and return y(u). Handles are *smooth* (left and right handle collinear through the key; editing one rotates the other, lengths independent) or *broken* (independent); default smooth. |
| `interpolation` | The same Bézier, but handles are not free: each segment carries an easing preset (§10.4) that fixes P1 and P2 in normalised segment space. |
| `velocityBased` | Key values only; between keys value = v0 + (v1 − v0) · u with u = (V(f) − V(t0)) ÷ (V(t1) − V(t0)), V being the node's `velocity` channel (§10.5), so the change progresses with the movement along the path. If V(t1) = V(t0), u = (f − t0) ÷ (t1 − t0). Only for `rotation`, `scale.*` and `skew` of nodes that have a motion path. |

Constant segments override all curve types. `velocity` supports `spline` and `interpolation` only. Unsupported
combinations are refused with `anim.curveTypeUnsupported`. In the function editor handles are displayed as **angle**
(−90…+90°, computed from value change per frame in display units) and **length** (distance to the key point in the same
units); agents use the offset form {dt, dv} (§19.2).

### 10.4 Easing presets (reshape)

Applied to the selected segment, or — with the *whole-curve modifier* / `scope: "all"` — to every segment of the curve.
For `spline` channels a preset sets the two handles; for `interpolation` channels it sets the segment preset. P1 and P2
are given in normalised segment space (time 0…1, value 0…1 from start to end value):

| Preset id | Shape | P1 | P2 |
|---|---|---|---|
| `constant` | hold, then step (sets the constant flag) | — | — |
| `linear` | straight line | (1/3, 1/3) | (2/3, 2/3) |
| `easeInOut` | slow start, slow end | (1/3, 0) | (2/3, 1) |
| `fastInOut` | fast start, fast end | (0, 1/2) | (1, 1/2) |
| `extremeEaseInOut` | strong slow start and end | (1/2, 0) | (1/2, 1) |
| `extremeFastInOut` | strong fast start and end | (0, 1) | (1, 0) |
| `slowAcceleration` | starts slowly, accelerates (ease in) | (1/3, 0) | (1, 1) |
| `slowDeceleration` | starts fast, decelerates (ease out) | (0, 0) | (2/3, 1) |

`constant` is not available for `velocity` (§10.2).

### 10.5 Velocity and timing along the path

Let the motion keyframes of a node be K0 … Km with frames f0 < … < fm and arc-length positions S0 = 0 ≤ S1 ≤ … ≤ Sm along
the path (§9.2).

- The **velocity channel** V(f) is the distance travelled along the path at frame f (displayed in distance units).
- At every motion keyframe V is **pinned**: V(fj) = Sj. Pins are implicit; they move with their keyframe and cannot be
  deleted or changed in value directly.
- Between pins the segment shape is a 1-D curve (spline handles or easing preset; default per §10.2). The user may add
  intermediate velocity key points (e.g. to pause mid-segment). **Design decision — storage:** intermediate velocity
  values and handles are stored normalised to the segment (u = (V − Sj) ÷ (Sj+1 − Sj)), so reshaping the path does not
  disturb the timing; they are displayed in distance units.
- Evaluation for fj ≤ f ≤ fj+1: compute V(f), clamp it to [Sj, Sj+1] (no travelling outside the segment), and take the
  path point at that arc length (§9.2). The curve may be non-monotonic (the node then moves back along the path).
- **Control-point passage frame:** the smallest f in [fj, fj+1] with V(f) ≥ the control point's arc length, found by
  bisection to 1e-4 frame and reported rounded to 3 decimals. Changing the velocity changes these frames.
- **Removing a motion keyframe** turns its pin into an ordinary intermediate velocity key (same frame, same normalised
  position in the merged segment), preserving the timing as far as possible.
- Frame markers (§4) visualise the result.

---

## 11. The function editor

### 11.1 Purpose and access

- The function editor displays and edits channels (§1.6) as curves of value over frames: rotation, scale, skew, pivot,
  velocity, FOV, bone channels, and one projected coordinate at a time of a motion path.
- It is opened from the window commands (section 01), from the *edit velocity* action of element, peg and skeleton
  properties (opens on `velocity`) and from the bone properties (opens on the bone's channels).
- It always shows the channels of the currently selected node and follows selection and document changes. Its title or
  header MUST identify the channel and the node shown (or that nothing is selected).

### 11.2 Parts

| Part | Requirement |
|---|---|
| Channel picker | Lists the channels of the selected node: pegs, elements and skeletons → motion path, pivot path, velocity, rotation, skew, scale X, scale Y; cameras → motion path, velocity, rotation (roll), FOV (when dynamic); bones → position X, position Y (root bones), rotation, length. |
| Curve type selector | `spline`, `interpolation`, `velocityBased` (§10.3); only the types allowed for the channel are enabled. |
| Projection selector | For the motion path and pivot path: which coordinate is plotted and edited (X, Y or Z; pivot X or Y). Only one coordinate is edited at a time. |
| Tools | Select, pan, zoom, grid on/off, fit curve to window, add key, delete key, lock in time. |
| Reshape | The easing presets of §10.4 (with the whole-curve modifier: whole curve). Presets not allowed for the channel are disabled. |
| Values panel | Frame and value of the selected key point; left and right handle angle and length (enabled for `spline` only); for simple channels only frame and value. Values commit on confirm or focus loss. |
| Constant segment | Check box: the segment starting at the selected key is constant. |
| Sync current frame | Check box: keeps the editor's selected frame and the scene's current frame synchronised. |
| Plot | Horizontal axis = frames, vertical axis = value; rulers with labels; optional grid; curve, key points, control points (path projections) and handles; current-frame indicator; independent horizontal and vertical zoom and scrolling. Axis labels of path projections use direction names (W/E, S/N, B/F). Path projections plot the effective position (static offset + animated); edits subtract the static offset. |
| Readout | Live frame and value under the pointer / of the selection. The editing panel and the tool bars can be shown/hidden and the editing panel docked to different sides. |

Units follow the coordinate-unit preference (distances), degrees (angles) and frames (time).

### 11.3 Gestures in the plot

| Gesture | Behaviour |
|---|---|
| Select tool: click / drag a key | Selects; vertical drag changes the value. Dragging a handle changes its angle and length. |
| Click on the curve with the *add-key modifier* | Adds a key point at that frame (on the curve's current value). |
| Click on a key with the *remove modifier* | Removes the key point. |
| Horizontal drag of a key with the *duplicate modifier* | Copies the key to the frame where it is dropped. |
| Horizontal drag of a key with the *timing modifier* | Moves the key in time; a key cannot pass its neighbours (it stops one frame short). |
| Pan, zoom, mouse wheel, fit | View navigation of the plot (also the generic view commands of section 01). |
| Add / delete / lock in time buttons | Add a key at the selected frame; delete the selected key or control point; turn the selected control point into a keyframe. |

Undo and redo inside the editor use the global history.

### 11.4 Motion-path projections

With the motion path selected, the plot shows one coordinate against frames: keyframes are key points (frame and value
editable); control points are shown at their derived passage frames and only their value is editable (moving the point
along that axis). Constant segments and lock in time are available. Velocity is edited as its own channel.

---

## 12. Cameras

### 12.1 Concept

- A camera node defines the point of view (§2.6). A scene can contain several cameras; exactly **one is the current
  camera** of the scene. The current camera defines the Camera view and all renders of that scene.
- **Design decision:** the current camera is a per-scene setting and does not change over time within a scene (cuts
  between camera set-ups are made with separate scenes, section 08).
- A scene always contains at least one camera; deleting the last camera is refused (`anim.lastCamera`). Deleting the
  current camera makes the first remaining camera in Timeline order current.
- Adding cameras or skeletons is refused while a library template is being edited (`anim.templateEditMode`, section 08).

### 12.2 Capabilities

| Capability | Behaviour |
|---|---|
| Add camera | Creates a camera with the default FOV and static offset (0, 0, 16), at the top level (or under a given parent). It does not become current automatically. |
| Delete camera | §12.1. |
| Camera list / set current | Lists the scene's cameras; choosing one makes it current (one undo step). The list MUST be reachable from the scene-planning views. |
| Select current camera | §6.2. |
| Rename, change duration | Section 04 element commands. |

### 12.3 Camera properties

| Property | Type / range | Default | Effect |
|---|---|---|---|
| Name | read-only here | — | — |
| Resolution | read-only, W × H pixels | project camera size | From the project settings (sections 01, 09). |
| Offset X / Y / Z | distance | 0, 0, 16 | Static camera position. |
| Roll | degrees | 0 | Static rotation about the viewing axis. |
| FOV mode | enum `static` \| `dynamic` | static | `static`: a single FOV value. `dynamic`: the `fov` channel, edited in the function editor. |
| FOV | degrees 1…179 | 41.1121 | Static FOV (also the value used to initialise the channel when switching to dynamic). |

Cameras also own `position`, `velocity` and `rotation` channels (animated like a peg) and may be attached to pegs.

### 12.4 Cameras in the views

- **Camera view:** renders through the current camera (§2.6). When a non-current camera is selected its frame is drawn
  as a thin outline at the table plane.
- **Top and Side views:** the camera is drawn with its viewing wedge; dragging a wedge edge changes the FOV (static
  value, or keyed `fov` at the current frame in dynamic mode). The camera can be moved with Select (static), Transform
  (keyed) or Motion (path) like any node.
- Acceptance workflow (animating the camera): with the Motion tool select the camera peg at the first frame and move the
  camera to its start position in the Top view; select a later frame on the camera peg, add a keyframe, move the camera;
  a second keyframe and a tweened segment appear; frames before the first keyframe hold its position.

---

## 13. Bones and skeletons

### 13.1 Structure

- A **skeleton** node groups a bone rig and the artwork it deforms. In the Timeline it contains:
  - a **bone group** — one layer per bone; bones of a chain are nested (child under parent); freestanding bones sit
    directly under the bone group. Bones created earlier are listed first (front of the bone display order);
  - a **skin group** — the drawing and image elements deformed by the bones.
- The skeleton node itself transforms like a peg (motion path, velocity, rotation, scale, skew, pivot, Constant Z, orient
  to path, loops).
- Bones are nodes with start, duration and loops, and the channels `bone.x`, `bone.y`, `rotation`, `bone.length`.
- Each bone also has **rest (setup) values**: rest X, rest Y, rest rotation, rest length, and an **influence**
  (type `infinite` or `elliptical`, length radius, width radius, optional custom region polygon).
- *Select skeleton root* selects the skeleton (bone group root) of the current bone or skin element.

### 13.2 Bone kinematics

All bone values are in **skeleton space** (the skeleton node's local space).

- A **root bone** (no parent bone) has its own position: B = T(x, y, 0) · R(θ), θ measured from the skeleton's +X axis.
- A **chained bone** starts at its parent bone's tip: B = B_parent · T(L_parent, 0, 0) · R(θ), θ relative to the
  parent's direction. Its position channels are not used (`anim.boneChained` if set).
- The bone points along its local +X axis from root (0, 0) to tip (L, 0).
- **Design decision — absolute channels:** bone channels hold absolute values (not offsets from rest). A channel with no
  keys uses the rest value. *Copy rest pose to current frame* keys every bone's channels with its rest values;
  *copy rest pose to all keyframes* does so at every frame where any bone of the skeleton has a key.
- World transform of a bone = W(skeleton) · B.

### 13.3 Deformation (skinning)

For each frame, the skin content is deformed from the rest bone network to the current one. **Design decision —
algorithm (2-D linear-blend skinning):**

1. Per bone i, the deformation matrix in skeleton space is D_i = B_i,current · S(L_current ÷ L_rest, 1) · B_i,rest⁻¹
   (the bone's own stretch acts along its length only).
2. Weights are computed for each skin point p in its rest position (skeleton space), with d_i = distance from p to the
   rest segment of bone i:
   - `infinite`: w_i = 1 ÷ (d_i² + 0.01)².
   - `elliptical`: ellipse centred on the bone's rest midpoint, axis a = length radius along the bone and b = width radius
     across it (defaults 0.75 × rest length and 0.35 × rest length); with q = √((x′/a)² + (y′/b)²) in ellipse
     coordinates, w_i = (1 − q)² if q < 1, else 0.
   - custom region polygon (optional, replaces the ellipse): inside the polygon w_i = 1 ÷ (d_i² + 0.01)², outside 0.
3. Weights are normalised to sum 1. A point with total weight 0 is not deformed (moves rigidly with the skeleton).
4. Deformed point p′ = Σ w_i · D_i · p, then the skeleton's world transform applies.
5. Vector drawings are discretised before deformation; samples per Bézier segment by the skeleton's *vector quality*:
   low 4, medium 8 (default), high 16, highest 32. Bitmap images are triangulated on a regular grid whose cell size is
   1/8, 1/16, 1/32 or 1/64 of the image's larger side for the same four qualities, and drawn as textured meshes.
6. Elements with *deformable* off inside the skin group are transformed rigidly with the skeleton. Deformation can be
   disabled per skeleton.

Deformation is evaluated on demand per frame; when it takes long the application shows progress, and operations that
would modify a skeleton's skin content while its deformation is being computed are refused (`anim.busyDeforming`).

### 13.4 Bone tools

All bone tools act in the Camera view.

| Tool | Behaviour |
|---|---|
| Bone creator | Press-drag-release creates a bone from root (press) to tip (release). The new bone is selected and the **next bone created is chained to it** (its root is placed at the selected bone's tip; its tip at the release point). With the *new-chain modifier* held, the bone is freestanding (root at the press point). If no skeleton is active and a drawing or image element is selected, a skeleton is created automatically, placed at that element's position in the hierarchy, and the element is moved into its skin group. New bones get the rest values from the gesture, influence `infinite`, and default names "Bone 1", "Bone 2"… |
| Bone setup | Edits **rest** values without animation. Selecting a bone shows two handles: drag the bone body → rotate; drag the **round tip handle** → change length and rotation together; drag the **square root handle** (root bones only) → move the bone. |
| Bone manipulator | Animates bones at the current frame with the same handles: square handle (root bones) → move (keys `bone.x`/`bone.y`); round tip handle → rotate and stretch in one movement (keys `rotation` and `bone.length`; children follow the tip); drag the body → rotate (keys `rotation`). Collapsed skeletons show the child-keyed indicator in the Timeline. |
| Natural movement (inverse) | Dragging a bone's tip moves the chain so the tip follows the pointer. **Design decision:** cyclic-coordinate-descent IK over the bones from the dragged bone up to its chain root, at most 20 iterations, processing bones from the dragged bone toward the root, stopping when the tip is within 0.001 field of the target; lengths unchanged; keys `rotation` of every changed bone. |
| Natural movement (forward) | Rotating a bone also applies follow-through to its descendants. **Design decision:** a rotation change Δ of the dragged bone adds −Δ · 0.5ᵏ to the local rotation of each descendant at depth k (k = 1 for direct children), so the chain lags behind; keys `rotation` of all changed bones. |
| Bone influence | For a bone with `elliptical` influence: shows its zone with a bounding box and handles; dragging handles changes the length and width radii (or the vertices of a custom region). |
| Bone parent | Drag from a bone to another bone: the first bone's chain becomes a child of the target (its root snaps to the target's tip; its rest rotation is converted to relative so its direction is kept). Dropping in empty space unparents the bone (it becomes a root bone keeping its rest position and direction). Cycles are refused (`anim.hierarchyCycle`). |
| Envelope display | Toggle of the bone-envelope display option (§4). |

The Scale tool also works on bones (§5.5).

### 13.5 Bone properties

| Property | Type | Effect |
|---|---|---|
| Name, start, duration | — | Section 04 commands. |
| Open function editor | action | Opens the editor on the bone's channels. |
| Position X / Y | distance | Root bones only. |
| Length | distance > 0.01 | Bone length. |
| Rotation | degrees | Bone angle (§13.2). |
| Influence | enum `infinite` \| `elliptical` | Type of influence zone; length radius, width radius, region. |

**Design decision:** while the Bone setup tool is active these fields show and edit rest values; otherwise they show the
animated values at the current frame and editing keys them.

### 13.6 Skeleton properties

| Property | Type | Default | Effect |
|---|---|---|---|
| Name, start, duration, loop state | — | — | As for pegs. |
| Edit velocity | action | — | Opens the editor on the skeleton's `velocity`. |
| Bone colour | colour | interface preference for skeletons and bones | Colour used to draw the bones. |
| Show track | bool | off | Shows the skeleton's motion path. |
| Show root | bool | on | Shows the skeleton root marker. |
| Show bones | bool | on | Shows the bones in the views. |
| Show influence | bool | off | Shows this skeleton's influence zones. |
| Vector quality | enum `low` \| `medium` \| `high` \| `highest` | medium | §13.3. Higher is smoother and slower. |
| Deformation enabled | bool | on | Off: skin content is not deformed. |
| Scale X / Y + keep proportions, skew, rotation, pivot | as pegs | identity | Static transform. |
| Apply pivot to | enum `rotation` \| `scaling` | rotation | §2.3. |
| Constant Z, orient to path | bool | off | As for pegs. |
| Copy rest pose to current frame / to all keyframes | actions | — | §13.2. |

### 13.7 Workflow and content (acceptance)

1. In the Camera view select the artwork element. 2. With the Bone creator create bones, starting with the parts at the
back. 3. With Bone setup adjust rest rotation, length and position. 4. Set elliptical influence where needed and adjust
the zones with Bone influence. 5. With the Bone manipulator go to a frame and pose the bones; repeat at other frames;
keys appear on the bone layers.

The application SHOULD ship ready-made skeleton motion templates (library, section 08) for human and animal rigs in front,
side and three-quarter views, covering at least walking, running, jumping, dancing, waving, sitting, stretching and idle
head movements. The content is to be authored fresh.

---

## 14. Motion recording

### 14.1 Purpose and access

Motion recording animates an element or peg by moving, scaling or rotating it with the pointer while the application
records the movement in real time and turns it into keyframes. It is a scene tool; selecting it shows the motion
recording options panel (which can also be opened on its own; its screen position is remembered).

### 14.2 Options

| Option | Type / range | Default | Effect |
|---|---|---|---|
| Capture speed | number 0.1…10 | 1.0 | Frames recorded per second of real time = scene frame rate × capture speed. Higher values give more frames for the same gesture (slower resulting motion). |
| Smoothness | integer 0…100 | 50 | Amount of smoothing of the recorded values (removes hand jitter). |
| Auto-extend exposure | bool | on | Extends the exposure of the animated drawing element(s) (the target, or the drawing descendants of a target peg) so they cover all recorded frames (section 04). |

### 14.3 Behaviour

1. With a drawing element or peg selected, the tool shows a bounding box with scale handles and a rotation handle.
2. Each recording captures **one** kind of transformation: translate (drag from inside), scale (drag a box handle:
   corner = uniform, side = that axis only) or rotate (drag the rotation handle or a corner rotation zone).
3. Recording starts at the current frame. While dragging, dots appear along the recorded path, one per recorded keyframe
   (slow movement → dense dots), and the current frame advances as frames are captured.
4. On release the animation is created as keyframes; playback shows the motion.
5. Recording again continues from the current frame (by default the end of the previous recording), appending motion.
6. To **overwrite** part of a recording, move the current frame to where the new recording should start and record
   again: existing keys of the recorded channel(s) from that frame onward are replaced.
7. Several transformations are combined by recording them separately on stacked parent pegs, with the main trajectory on
   the top-most peg. Orient to path makes the drawing follow the recorded direction (§8.3).
8. One recording is one undoable step. During a recording other scene interaction is blocked.

### 14.4 Recording algorithm (normative)

Input: a sample list of pointer positions `(t, x, y)` — t in seconds from the press, x/y in the target's **parent
space** — starting with the press sample (t = 0) and ending with the release sample. Interactive use produces this list
from pointer events; agents supply it directly (§19).

1. Let r = scene frame rate × capture speed and N = floor(t_last × r). For k = 0 … N, the pointer position p_k at time
   k ÷ r is linearly interpolated from the samples.
2. Values per recorded frame (start values v_start are the target's evaluated values at the start frame F0):
   - translate: position_k = position_start + (p_k − p_0);
   - rotate: rotation_k = rotation_start + (unwrapped angle of p_k around the rotation pivot − angle of p_0), with the
     angle unwrapped so that it accumulates beyond ±180°;
   - scale: factor_k = |p_k − c| ÷ |p_0 − c| (c = scale pivot) for corner handles (both axes), or the ratio of the
     X (resp. Y) distances for side handles; scale_k = scale_start × factor_k (|scale| ≥ 0.001).
3. Smoothing: half-window h = round(smoothness ÷ 10) frames; each value k (except the first and last) is replaced by the
   mean of values k−h … k+h clipped to the recorded range.
4. Keys of the recorded channel(s) on frames ≥ F0 are removed; then one key per recorded frame F0 + k is written with
   linear segments (position keyframes as motion keyframes; rotation/scale as 1-D keys). The node's duration is extended
   if needed (and *snap last keyframe* applies, §6.5).
5. If auto-extend exposure is on, drawing exposures are extended to F0 + N.
6. The current frame becomes F0 + N.

---

## 15. Edit operations in the scene

| Capability | Behaviour |
|---|---|
| Cut / copy / paste nodes | Acts on the selected nodes together with all their descendants (section 01 generic clipboard). |
| Cut / copy / paste / delete point | With a motion point selected, the generic clipboard commands act on the point: copying stores its position, TCB values and (for keyframes) the rotation, scale and pivot values at its frame; pasting onto a selected point of the same or another node applies them. |
| Delete | Deletes the selected motion point(s) if any; otherwise the selected node(s) (section 04). |
| Paste special | Pastes only chosen properties of the copied motion point onto the selected point(s) (table below). |
| Paste special again | Repeats the last paste special with the same choices, without asking. |
| Cut special | Chooses which channels to cut from the selected pegs or elements (the keys of those channels are copied, then removed). |

Paste-special properties:

| Group | Property | Available when |
|---|---|---|
| Motion | position X, position Y, position Z, tension, continuity, bias | always |
| Rotation | angle | the copied point is a keyframe |
| Rotation | pivot | the copied point is a keyframe and the target's pivot applies to rotation |
| Scale | scale X, scale Y | the copied point is a keyframe |
| Scale | pivot | the copied point is a keyframe and the target's pivot applies to scaling |

Unavailable properties are disabled in the interface and, for commands, skipped and reported in the result. A paste whose
clipboard content does not fit the target is refused (`anim.pasteIncompatible`). When the clipboard holds Timeline cells
or layers instead of a motion point, the Timeline paste options of section 04 apply.

---

## 16. Preferences

Persisted with the user settings; editable in the preferences (section 01) and through `anim.prefs.set`.

| Preference | Type / range | Default | Effect |
|---|---|---|---|
| Default tension / continuity / bias | −1…1 each | 0, 0, 0 | TCB values of new motion points. |
| Coordinate units | enum `fields` \| `inches` \| `centimetres` | fields | Display unit for distances (§1.2). |
| Show units | bool | on | Shows unit and direction suffixes in fields. |
| Use box highlighting | bool | on | §3.2. |
| Selection always visible | bool | on | Shows an element selected in the Timeline or exposure sheet even when it is hidden. |
| Create linear spline | bool | on | §10.2. |
| Create constant keyframes | bool | off | §10.2. |
| Depth nudge step | distance 0.0001…1 | 0.01 field | §6.4. |
| Peg-only mode, snap last keyframe | bool | off, off | §6.1, §6.5. |
| Motion recording options | — | §14.2 | Persisted between sessions. |

Display options (§4) and element colours for pegs, cameras, skeletons and bones (section 01) are also persisted.

---

## 17. Undo granularity

- Every command of this section that changes the document is exactly one undo step; an interactive drag (tool gesture,
  slider, handle) is one step committed on release; a property-field commit is one step; one motion recording is one
  step; `batch` (section 11) groups several commands into one step.
- Undo labels name the operation and the object kind (e.g. move peg, rotate bone, change tension, add keyframe); wording
  is a UX-writing task.
- Display options, view state, tool selection and selection changes are not undoable.

---

## 18. Messages and errors

| Code | Kind | Condition |
|---|---|---|
| `anim.frameInvalid` | error | Frame missing, non-numeric, outside the node's range, or a key move would cross a neighbouring key. |
| `anim.frameNotInteger` | error | A keyframe frame with a fractional part. |
| `anim.keyExists` | error | Adding or moving a keyframe onto a frame that already has one in the same channel/path. |
| `anim.valueOutOfRange` | error | Value below minimum or above maximum (the limits are part of the error data and of the message). |
| `anim.pathEndConverted` | warning | Control points were left beyond the last (first) keyframe; the end point was converted to a keyframe to preserve the path. |
| `anim.pathEndKey` | error | Attempt to unlock the first or last keyframe of a path. |
| `anim.channelUnsupported` | error | Channel not available on this node kind, or constant flag on `velocity`. |
| `anim.curveTypeUnsupported` | error | Curve type or preset not allowed for the channel. |
| `anim.notTransformable` | error | Target cannot be transformed (e.g. sound element). |
| `anim.noAttachTarget` | error | *Attach to next object* found no peg/effect above. |
| `anim.hierarchyCycle` | error | Attaching/reparenting would create a cycle. |
| `anim.boneChained` | error | Setting position channels on a chained bone. |
| `anim.lastCamera` | error | Deleting the last camera of a scene. |
| `anim.templateEditMode` | error | Adding a camera or skeleton while editing a library template. |
| `anim.pasteIncompatible` | error | Clipboard content does not fit the paste target. |
| `anim.busyDeforming` | error | Modifying skin content while its deformation is being computed. |
| `anim.pegOnlyNoPeg` | info | Peg-only mode: the picked element has no parent peg. |
| `anim.samplesInvalid` | error | Motion-recording sample list empty, not starting at t = 0, or with decreasing t. |
| `anim.notFound` | error | Unknown node, point, bone, camera or view id. |

Message wording is a UX-writing task; it MUST state the problem and, where applicable, the permitted range or the way
to resolve it. Tools that have no options show an indication that there are none in the tool-options panel.

---

## 19. Agent interface

### 19.1 Agent-specific requirements

1. **Numeric equivalents for every gesture.** Every tool gesture of §5, §13.4 and §14 has a command equivalent taking
   numeric values: moving/rotating/scaling/skewing (`anim.transform.set`), motion-point editing (`anim.path.*`), handle
   editing in the function editor (`anim.function.key.set`), FOV wedge dragging (`anim.camera.set`), bone posing
   (`anim.bone.pose.set`, `anim.bone.reach.apply`), influence-zone editing (`anim.bone.influence.set`), ghost dragging
   (`anim.transform.set` with `target: "static"` and `relative: true` on the child), arms (rotation of the peg), and
   motion recording from a supplied sample list (`anim.record.run`). Interactive pointer simulation (section 11
   `pointer`) remains available but is never required.
2. **No modal paths.** The add-motion-point dialog maps to `anim.path.point.add` parameters; paste special maps to
   `anim.point.paste {properties}`; cut special maps to `anim.channel.cut {channels}`; confirmations do not exist in this
   area; warnings (e.g. `anim.pathEndConverted`) are returned in the result's `warnings` array and never block.
3. **Explicit targets.** Commands address nodes by id (or path, section 11) and never depend on the active tool, the
   view or peg-only mode. Commands that "act on the selection" in the GUI take an explicit `nodeIds` list; when it is
   omitted they use the current selection.
4. **Frames.** All `frame` parameters are **scene frames** (1-based integers; fractional only where stated). A frame
   inside a repeated loop cycle (not the first cycle) cannot be keyed (`anim.frameInvalid`). Omitted `frame` means the
   current frame.
5. **Read-back for verification.** `anim.evaluate` returns static, animated, local and world transforms, world bounds
   and output-pixel bounds of any node at any frames — agents verify placement without rendering. `anim.path.get`,
   `anim.path.sample`, `anim.function.get`, `anim.function.sample`, `anim.key.list`, `anim.skeleton.get`,
   `anim.skeleton.deform.get`, `anim.camera.get` and `anim.projection.get` expose every other model; `render_frame`
   (section 11) gives the visual check.
6. **Determinism.** Given the same document and commands, results are bit-for-bit reproducible across runs on the same
   platform and equal within 1e-6 across platforms (§2). Motion recording from samples does not depend on wall-clock
   time.
7. **Atomicity and undo.** Each action is one undo step; `batch` makes several one step. Invalid input changes nothing
   and returns an `anim.*` error with `{code, message, data}` where `data` carries limits or the conflicting frame/id.
8. **Events.** Changes are announced on `app://events` (§19.4) so agents can keep caches coherent.

### 19.2 Conventions for agents

- Distances in **fields**, angles in **degrees** (counter-clockwise positive), scale as factors, time in scene frames.
  Display units never affect command values.
- `object {x,y,z}` is a 3-D position; `point {x,y}` a 2-D position. Node channel values (`position`, pivots) are in the
  node's **parent space**; `positionSpace: "world"` converts a requested world position into the parent-space value.
- Matrices: 16 numbers row-major, column-vector convention (§2.1).
- Function key handles: `{dt, dv}` offsets in frames and value units from the key (left handle dt ≤ 0, right dt ≥ 0).
- Channel ids as in §1.6; preset ids as in §10.4; curve types `spline`, `interpolation`, `velocityBased`.
- Output pixels: origin top-left of the camera frame, y down (§2.6).

### 19.3 Command table

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `anim.tool.set` | ui | Select the active scene-planning tool (independent of the drawing tool) | `tool: enum "select"\|"transform"\|"record"\|"rotate"\|"skew"\|"scale"\|"motion"\|"boneCreate"\|"boneSetup"\|"boneManipulate"\|"boneInverse"\|"boneForward"\|"boneInfluence"\|"boneParent"; cycleRotate?: bool = false` | `{tool}` | no |
| `anim.tool.get` | query | Current scene-planning tool | — | `{tool}` | n/a |
| `anim.pegOnlyMode.set` | toggle | Peg-only mode on/off (§6.1) | `enabled: bool` | `{enabled}` | no |
| `anim.snapLastKey.set` | toggle | Snap-last-keyframe on/off (§6.5) | `enabled: bool` | `{enabled}` | no |
| `anim.display.allPegs.set` | toggle | Show all pegs' paths | `enabled: bool` | `{enabled}` | no |
| `anim.display.pivotPath.set` | toggle | Show pivot paths | `enabled: bool` | `{enabled}` | no |
| `anim.display.pegGhosts.set` | toggle | Show peg ghosts | `enabled: bool` | `{enabled}` | no |
| `anim.display.arms.set` | toggle | Show element arms while rotating pegs | `enabled: bool` | `{enabled}` | no |
| `anim.display.frameMarkers.set` | toggle | Show frame markers on paths | `enabled: bool` | `{enabled}` | no |
| `anim.display.envelopes.set` | toggle | Show bone influence envelopes | `enabled: bool` | `{enabled}` | no |
| `anim.display.smoothPaths.set` | toggle | Anti-alias motion paths | `enabled: bool` | `{enabled}` | no |
| `anim.display.antialiasToolOverlays.set` | toggle | Anti-alias tool furniture | `enabled: bool` | `{enabled}` | no |
| `anim.display.get` | query | All display options of §4 | — | `{allPegs, pivotPath, pegGhosts, arms, frameMarkers, effects, envelopes, smoothPaths, antialiasToolOverlays}` | n/a |
| `anim.node.pathDisplay.set` | ui | Per-node motion-path display switch (as in the Timeline) | `nodeIds: [id]; visible: bool` | `{nodeIds}` | no |
| `anim.view.type.set` | ui | Switch a view window between Drawing, Camera, Top and Side | `viewId: id; type: enum "drawing"\|"camera"\|"top"\|"side"` | `{viewId, type}` | no |
| `anim.view.slot.select` | ui | Switch active view: store current state and restore the other slot | `viewId: id; slot?: enum "A"\|"B"\|"other" = "other"` | `{viewId, slot, state}` | no |
| `anim.view.get` | query | View state of a scene view | `viewId: id` | `{type, zoom, pan {x,y}, rotation, visibleRect {minX,minY,maxX,maxY}, slot}` | n/a |
| `anim.view.coords.get` | query | Map points between scene space and view pixels (for pointer simulation) | `viewId: id; points: [object]; from: enum "scene"\|"view"; depth?: number = 0` | `{points: [object]}` | n/a |
| `anim.node.get` | query | All animation properties of a node | `nodeId: id; frame?: int` | `{id, kind, static, options, channels[], loop, parentId, childIds[]}` | n/a |
| `anim.transform.set` | action | Set static values or key animated values (Select/Transform/Rotate/Scale/Skew tool equivalent) | `nodeIds?: [id]; target: enum "static"\|"key" = "key"; frame?: int; position?: object {x,y,z}; positionSpace?: enum "parent"\|"world" = "parent"; rotation?: number; scale?: point; skew?: number (-89..89); pivot?: point; rotationPivot?: point; scalePivot?: point; relative?: bool = false` | `{nodeIds, keyedChannels[], warnings[]}` | yes |
| `anim.node.options.set` | action | Set non-transform node options | `nodeIds?: [id]; orientToPath?: bool; keepProportions?: bool; rotateAboutPivot?: bool; pivotAppliesTo?: enum "rotation"\|"scaling"\|"both"; lockDepth?: bool; useChildDrawingPivot?: bool; deformable?: bool; role?: enum "normal"\|"background"; groupOnExport?: bool; compositeForExportGroup?: bool` | `{nodeIds}` | yes |
| `anim.flip.apply` | action | Flip nodes horizontally or vertically | `nodeIds?: [id]; axis: enum "horizontal"\|"vertical"; target: enum "static"\|"key" = "static"; frame?: int` | `{nodeIds}` | yes |
| `anim.depth.move` | action | Depth nudge forward/backward | `nodeIds?: [id]; direction: enum "forward"\|"backward"; steps: int = 1 (1..1000)` | `{nodeIds, z[]}` | yes |
| `anim.reset.apply` | action | Reset nodes: remove keys and restore default static values | `nodeIds?: [id]` | `{nodeIds}` | yes |
| `anim.evaluate` | query | Evaluate transforms of nodes at frames (no rendering) | `nodeIds: [id]; frames?: [int]; include?: [enum "static"\|"animated"\|"local"\|"world"\|"bounds"\|"screen"] = all; cameraId?: id` | `{frames: [{frame, nodes: [NodeEval]}]}` (§19.4) | n/a |
| `anim.peg.create` | action | Add a peg; with `wrapIds`, add a parent peg around those nodes | `name?: string; parentId?: id; aboveId?: id; wrapIds?: [id]` | `{pegId}` | yes |
| `anim.hierarchy.attach` | action | Attach/reparent nodes under a parent (or the next peg/effect above) | `childIds: [id]; parentId: id \| "next"; index?: int; keepWorld: bool = false` | `{childIds, parentId, approximated}` | yes |
| `anim.hierarchy.detach` | action | Detach nodes from their parent (up one level) | `nodeIds: [id]; keepWorld: bool = false` | `{nodeIds, approximated}` | yes |
| `anim.hierarchy.get` | query | Scene node tree with kinds and attachment | `sceneId?: id; rootId?: id` | `{nodes: [{id, kind, name, parentId, childIds[]}]}` | n/a |
| `anim.hierarchy.select` | ui | Select parent / descendants / first child / previous or next sibling | `fromId?: id; relation: enum "parent"\|"children"\|"firstChild"\|"prevSibling"\|"nextSibling"` | `{selectedIds}` | no |
| `anim.loop.set` | action | Set looping of a peg, effect, skeleton or bone | `nodeId: id; loops: int = 1 (1..10000); loopType: enum "count"\|"fill" = "count"; positionLoop: enum "cycle"\|"offset" = "cycle"` | `{nodeId, cycleLength, duration}` | yes |
| `anim.stretch.apply` | action | Re-time a node's channels proportionally to a new duration | `nodeId: id; duration: int (1..)` | `{nodeId, movedKeys, mergedKeys}` | yes |
| `anim.path.get` | query | Motion path with all points, parameters and arc lengths | `nodeId: id` | `Path` (§19.4) | n/a |
| `anim.path.set` | action | Replace a node's whole motion path | `nodeId: id; points: [object {kind, frame?, x, y, z, tension?, continuity?, bias?, constantSegment?, lockDepth?}]; lockDepth?: bool` | `{nodeId, pointIds[], warnings[]}` | yes |
| `anim.path.point.add` | action | Add a keyframe or control point (by frame, or at a curve location) | `nodeId: id; kind: enum "key"\|"control"; frame?: number; at?: object {segment: int, s: number (0..1)}; position?: object {x,y,z}` | `{pointId, frame}` | yes |
| `anim.path.point.set` | action | Edit a motion point (position, frame, lock in time, TCB, constant, per-key rotation/scale/pivot) | `pointIds: [id]; x?: number; y?: number; z?: number; frame?: int; lockInTime?: bool; tension?: number (-1..1); continuity?: number (-1..1); bias?: number (-1..1); constantSegment?: bool; rotation?: number; scale?: point; pivot?: point` | `{pointIds, warnings[]}` | yes |
| `anim.path.point.remove` | action | Delete motion points | `pointIds: [id]` | `{removed, warnings[]}` | yes |
| `anim.path.point.select` | ui | Select a motion point, or the previous/next one | `pointIds?: [id]; nodeId?: id; relation?: enum "next"\|"prev"; moveCurrentFrame: bool = true` | `{pointIds, frame}` | no |
| `anim.path.sample` | query | Positions per frame (frame markers, ghosts, pivot path) | `nodeId: id; from?: int; to?: int; step: number = 1; what: enum "origin"\|"pivot" = "origin"; space: enum "parent"\|"world" = "world"` | `{samples: [{frame, x, y, z}]}` | n/a |
| `anim.path.lockDepth.set` | action | Constant Z for the segment starting at a keyframe | `nodeId: id; frame: int; enabled: bool` | `{nodeId, frame}` | yes |
| `anim.key.add` | action | Add keyframes (full pose or chosen channels); optionally on a whole hierarchy or at segment bounds | `nodeIds?: [id]; frame?: int; channels?: [string]; includeDescendants: bool = false; atSegmentBounds?: enum "left"\|"right"\|"both"` | `{keys: [{nodeId, frame, channels[]}]}` | yes |
| `anim.key.remove` | action | Remove keyframes at a frame (default channels position/rotation/scale/skew) or all keyframes | `nodeIds?: [id]; frame?: int; channels?: [string]; all: bool = false` | `{removed}` | yes |
| `anim.key.list` | query | Keyed frames and channels of a node | `nodeId: id; from?: int; to?: int; includeDescendants: bool = false` | `{keys: [{frame, channels[], constant[], descendantKeyed}]}` | n/a |
| `anim.key.move` | action | Move keys of a node to another frame | `nodeId: id; fromFrame: int; toFrame: int; channels?: [string]` | `{nodeId, toFrame}` | yes |
| `anim.segment.set` | action | Make segments starting at a keyframe constant or tweened | `nodeIds?: [id]; frame: int; channels?: [string]; constant: bool` | `{nodeIds, channels[]}` | yes |
| `anim.segment.uniform.apply` | action | Make the segment left/right of a key uniform (linear) | `nodeId: id; frame: int; side: enum "left"\|"right"; channels?: [string]` | `{nodeId, channels[]}` | yes |
| `anim.function.list` | query | Channels of a node with curve type and key count | `nodeId: id` | `{channels: [{channel, curveType, keyCount, allowedCurveTypes[]}]}` | n/a |
| `anim.function.get` | query | Function curve as a key-point list | `nodeId: id; channel: string` | `Function` (§19.4) | n/a |
| `anim.function.set` | action | Replace a 1-D channel's key list and curve type | `nodeId: id; channel: string; curveType?: enum "spline"\|"interpolation"\|"velocityBased"; keys: [object {frame, value, constantSegment?, handleMode?, left?, right?, preset?}]` | `{nodeId, channel, keyCount}` | yes |
| `anim.function.key.add` | action | Add a key point (value defaults to current curve value) | `nodeId: id; channel: string; frame: number; value?: number` | `{frame, value}` | yes |
| `anim.function.key.set` | action | Edit a key point: move in time, value, handles, handle mode, constant flag, lock in time | `nodeId: id; channel: string; frame: number; newFrame?: number; value?: number; left?: object {dt,dv}; right?: object {dt,dv}; handleMode?: enum "smooth"\|"broken"; constantSegment?: bool; lockInTime?: bool` | `{frame, value}` | yes |
| `anim.function.key.remove` | action | Remove key points | `nodeId: id; channel: string; frames: [number]` | `{removed}` | yes |
| `anim.function.key.duplicate` | action | Copy a key point to another frame | `nodeId: id; channel: string; frame: int; toFrame: int` | `{frame}` | yes |
| `anim.function.curveType.set` | action | Change a channel's curve type | `nodeId: id; channel: string; curveType: enum "spline"\|"interpolation"\|"velocityBased"` | `{channel, curveType}` | yes |
| `anim.function.preset.apply` | action | Apply an easing preset to a segment or the whole curve | `nodeId: id; channel: string; preset: enum "constant"\|"linear"\|"easeInOut"\|"fastInOut"\|"extremeEaseInOut"\|"extremeFastInOut"\|"slowAcceleration"\|"slowDeceleration"; frame?: int; scope: enum "segment"\|"all" = "segment"` | `{segments}` | yes |
| `anim.function.sample` | query | Evaluate a channel over a frame range | `nodeId: id; channel: string; from: number; to: number; step: number = 1` | `{samples: [{frame, value}]}` | n/a |
| `anim.functionEditor.open` | ui | Open the function editor on a node/channel/projection | `nodeId?: id; channel: string; projection?: enum "x"\|"y"\|"z"` | `{editorId}` | no |
| `anim.functionEditor.syncFrame.set` | ui | Sync the editor's selected frame with the current frame | `enabled: bool` | `{enabled}` | no |
| `anim.functionEditor.view.set` | ui | Function-editor view: fit, grid, zooms, panel and toolbar visibility, panel side | `fit?: bool; grid?: bool; zoomX?: number; zoomY?: number; showPanel?: bool; showToolbars?: bool; panelSide?: enum "left"\|"right"\|"top"\|"bottom"` | `{state}` | no |
| `anim.functionEditor.get` | query | Function-editor state | — | `{nodeId, channel, projection, selectedFrames[], syncFrame, view}` | n/a |
| `anim.camera.create` | action | Add a camera | `name?: string; parentId?: id; position?: object {x,y,z} = {0,0,16}; fov?: number = 41.1121 (1..179); makeCurrent: bool = false` | `{cameraId}` | yes |
| `anim.camera.delete` | action | Delete a camera (not the last one) | `cameraId: id` | `{currentCameraId}` | yes |
| `anim.camera.list` | query | Cameras of a scene | `sceneId?: id` | `{cameras: [{id, name, current}]}` | n/a |
| `anim.camera.get` | query | Camera properties and evaluated view at a frame | `cameraId?: id; frame?: int` | `{id, position, roll, fovMode, fov, effectiveFov, worldPosition, viewMatrix[16], frameCornersAtZ0[4], resolution {w,h}}` | n/a |
| `anim.camera.set` | action | Set camera position, roll, FOV mode and FOV (keyed at `frame` when dynamic) | `cameraId: id; position?: object {x,y,z}; roll?: number; fovMode?: enum "static"\|"dynamic"; fov?: number (1..179); frame?: int` | `{cameraId}` | yes |
| `anim.camera.current.set` | action | Make a camera the scene's current camera | `cameraId: id` | `{cameraId}` | yes |
| `anim.camera.current.get` | query | Current camera of a scene | `sceneId?: id` | `{cameraId}` | n/a |
| `anim.camera.select` | ui | Select the current camera | — | `{cameraId}` | no |
| `anim.projection.get` | query | Project world points or node bounds into output pixels | `frame?: int; cameraId?: id; points?: [object]; nodeIds?: [id]` | `{points: [{x, y, depth, visible}], nodes: [{id, bbox {x,y,width,height}, visible}]}` | n/a |
| `anim.skeleton.create` | action | Add a skeleton, optionally moving elements into its skin group | `name?: string; parentId?: id; skinIds?: [id]` | `{skeletonId, boneGroupId, skinGroupId}` | yes |
| `anim.skeleton.delete` | action | Delete a skeleton (skin elements are moved out to its level) | `skeletonId: id; keepSkin: bool = true` | `{skeletonId}` | yes |
| `anim.skeleton.get` | query | Bone tree with rest and current values | `skeletonId: id; frame?: int` | `Skeleton` (§19.4) | n/a |
| `anim.skeleton.set` | action | Skeleton display and quality options | `skeletonId: id; boneColour?: colour; showTrack?: bool; showRoot?: bool; showBones?: bool; showInfluence?: bool; vectorQuality?: enum "low"\|"medium"\|"high"\|"highest"; deformationEnabled?: bool` | `{skeletonId}` | yes |
| `anim.skeleton.skin.add` | action | Put elements into the skin group | `skeletonId: id; elementIds: [id]` | `{elementIds}` | yes |
| `anim.skeleton.skin.remove` | action | Take elements out of the skin group | `skeletonId: id; elementIds: [id]` | `{elementIds}` | yes |
| `anim.skeleton.restPose.apply` | action | Copy the rest pose to the current frame or to all keyframes | `skeletonId: id; scope: enum "frame"\|"allKeyframes" = "frame"; frame?: int` | `{frames[]}` | yes |
| `anim.skeleton.root.select` | ui | Select the skeleton root of the current bone/skin element | `fromId?: id` | `{skeletonId}` | no |
| `anim.skeleton.deform.get` | query | Deformed positions and weights of given rest-space points | `skeletonId: id; frame?: int; points: [point]` | `{points: [{x, y, weights: [{boneId, w}]}]}` | n/a |
| `anim.bone.create` | action | Create a bone (chained to a parent bone, or freestanding) | `skeletonId: id; parentBoneId?: id; root?: point; tip: point; name?: string` | `{boneId}` | yes |
| `anim.bone.delete` | action | Delete bones | `boneIds: [id]; children: enum "reparent"\|"delete" = "reparent"` | `{removed}` | yes |
| `anim.bone.rest.set` | action | Set rest (setup) values | `boneId: id; x?: number; y?: number; rotation?: number; length?: number (0.01..)` | `{boneId}` | yes |
| `anim.bone.pose.set` | action | Pose bones by angles/lengths at a frame (keys) | `frame?: int; bones: [object {boneId, rotation?, length?, x?, y?}]; rotationSpace: enum "local"\|"skeleton" = "local"` | `{keyed: [{boneId, channels[]}]}` | yes |
| `anim.bone.reach.apply` | action | Natural movement: IK reach to a target, or FK rotation with follow-through | `boneId: id; frame?: int; mode: enum "ik"\|"fk"; target?: point; rotationDelta?: number` | `{keyed: [{boneId, rotation}], reached: bool, error: number}` | yes |
| `anim.bone.influence.set` | action | Influence type and zone | `boneId: id; type?: enum "infinite"\|"elliptical"; lengthRadius?: number; widthRadius?: number; regionPath?: [point]` | `{boneId}` | yes |
| `anim.bone.parent.set` | action | Reparent a bone (null = make it a root bone) | `boneId: id; parentBoneId: id \| null` | `{boneId}` | yes |
| `anim.record.options.set` | ui | Motion-recording options (persisted user settings) | `captureSpeed?: number (0.1..10); smoothness?: int (0..100); autoExtendExposure?: bool` | `{options}` | no |
| `anim.record.options.get` | query | Motion-recording options | — | `{captureSpeed, smoothness, autoExtendExposure}` | n/a |
| `anim.record.run` | action | Record motion from a sample list (§14.4) | `nodeId: id; kind: enum "translate"\|"rotate"\|"scale"; handle?: enum "corner"\|"horizontal"\|"vertical" = "corner"; startFrame?: int; samples: [object {t, x, y}]; captureSpeed?: number; smoothness?: int; autoExtendExposure?: bool` | `{firstFrame, lastFrame, keyCount, channels[]}` | yes |
| `anim.point.copy` | ui | Copy a motion point to the clipboard | `pointId: id` | `{pointId, isKeyframe}` | no |
| `anim.point.paste` | action | Paste a copied point (all properties, or chosen ones = paste special; `again` repeats the last choice) | `targetPointIds: [id]; properties?: [enum "x"\|"y"\|"z"\|"tension"\|"continuity"\|"bias"\|"rotation"\|"rotationPivot"\|"scaleX"\|"scaleY"\|"scalePivot"]; again: bool = false` | `{applied[], skipped[]}` | yes |
| `anim.channel.copy` | ui | Copy chosen channels' keys of a node | `nodeId: id; channels: [string]; from?: int; to?: int` | `{keyCount}` | no |
| `anim.channel.cut` | action | Cut special: copy then remove chosen channels' keys | `nodeIds?: [id]; channels: [string]; from?: int; to?: int` | `{keyCount}` | yes |
| `anim.channel.paste` | action | Paste copied channel keys at a frame | `nodeIds: [id]; frame: int; channels?: [string]; mode: enum "merge"\|"replace" = "merge"` | `{keyCount}` | yes |
| `anim.prefs.get` | query | Scene-planning preferences (§16) | — | `{defaultTension, defaultContinuity, defaultBias, units, displayUnits, boxHighlighting, keepSelectionVisible, linearByDefault, constantKeysByDefault, depthNudgeStep}` | n/a |
| `anim.prefs.set` | ui | Set scene-planning preferences | `defaultTension?: number (-1..1); defaultContinuity?: number (-1..1); defaultBias?: number (-1..1); units?: enum "fields"\|"inches"\|"centimetres"; displayUnits?: bool; boxHighlighting?: bool; keepSelectionVisible?: bool; linearByDefault?: bool; constantKeysByDefault?: bool; depthNudgeStep?: number (0.0001..1)` | `{prefs}` | no |
| `anim.changed` | event | Node animation data changed | — | `{nodeIds[], channels[], frames?: {from, to}}` | n/a |
| `anim.hierarchy.changed` | event | Attachment/order of nodes changed | — | `{sceneId, nodeIds[]}` | n/a |
| `anim.camera.currentChanged` | event | The current camera of a scene changed | — | `{sceneId, cameraId}` | n/a |
| `anim.tool.changed` | event | Scene-planning tool changed | — | `{tool}` | n/a |
| `anim.record.completed` | event | A motion recording (interactive or by command) finished | — | `{nodeId, firstFrame, lastFrame}` | n/a |
| `anim.display.changed` | event | A display option or preference of this area changed | — | `{option, value}` | n/a |

### 19.4 Queries and events — result shapes

**NodeEval** (`anim.evaluate`, per node and frame):

```json
{
  "id": "n42", "kind": "peg", "localFrame": 12,
  "static":   {"position": {"x": 0, "y": 0, "z": 0}, "rotation": 0, "scale": {"x": 1, "y": 1}, "skew": 0,
               "rotationPivot": {"x": 0, "y": 0}, "scalePivot": {"x": 0, "y": 0}},
  "animated": {"position": {"x": 2.5, "y": -1, "z": 0}, "rotation": 15, "scale": {"x": 1, "y": 1}, "skew": 0,
               "pivot": {"x": 0, "y": 0}, "velocity": 3.2, "orientAngle": 0},
  "local":    {"position": {"x": 2.5, "y": -1, "z": 0}, "rotation": 15, "scale": {"x": 1, "y": 1}, "skew": 0,
               "rotationPivot": {"x": 0, "y": 0}, "scalePivot": {"x": 0, "y": 0}, "matrix": [16 numbers]},
  "world":    {"matrix": [16 numbers], "position": {"x": 2.5, "y": -1, "z": 0}, "rotation": 15,
               "scale": {"x": 1, "y": 1}, "skew": 0},
  "bounds":   {"minX": 0.1, "minY": -3, "maxX": 4.9, "maxY": 1.2, "z": 0},
  "screen":   {"visible": true, "bbox": {"x": 812, "y": 401, "width": 384, "height": 336}, "depth": 16}
}
```

`bounds` is the world-space axis-aligned box of the node's visible content at that frame (absent for pegs and cameras,
which report only their origin); `screen` is relative to the current (or given) camera.

**Path** (`anim.path.get`):

```json
{"nodeId": "n42", "lockDepth": false, "length": 14.37,
 "points": [
   {"id": "p1", "kind": "key", "frame": 1, "x": -4, "y": 0, "z": 0, "tension": 0, "continuity": 0, "bias": 0,
    "constantSegment": false, "lockDepth": false, "arcLength": 0},
   {"id": "p2", "kind": "control", "frame": 7.412, "x": 0, "y": 3, "z": 0, "tension": 0, "continuity": 0, "bias": 0,
    "arcLength": 6.02},
   {"id": "p3", "kind": "key", "frame": 24, "x": 4, "y": 0, "z": 0, "tension": 0, "continuity": 0, "bias": 0,
    "constantSegment": false, "lockDepth": false, "arcLength": 14.37}]}
```

**Function** (`anim.function.get`):

```json
{"nodeId": "n42", "channel": "rotation", "curveType": "spline",
 "keys": [{"frame": 1, "value": 0, "constantSegment": false, "handleMode": "smooth",
           "left": {"dt": 0, "dv": 0}, "right": {"dt": 3.67, "dv": 0}, "preset": null},
          {"frame": 12, "value": 45, "constantSegment": false, "handleMode": "smooth",
           "left": {"dt": -3.67, "dv": 0}, "right": {"dt": 0, "dv": 0}, "preset": null}],
 "pins": []}
```

For `velocity`, `pins` lists the implicit keyframe pins `{frame, value}` and `keys` the intermediate velocity keys
(values reported in fields); for `interpolation` curves `preset` names each segment's preset.

**Skeleton** (`anim.skeleton.get`):

```json
{"skeletonId": "s1", "boneGroupId": "g1", "skinGroupId": "g2", "skinIds": ["e7"], "vectorQuality": "medium",
 "bones": [{"id": "b1", "name": "Bone 1", "parentBoneId": null,
            "rest": {"x": 0, "y": -2, "rotation": 90, "length": 2},
            "current": {"x": 0, "y": -2, "rotation": 80, "length": 2,
                        "rootWorld": {"x": 0, "y": -2}, "tipWorld": {"x": 0.35, "y": -0.03}},
            "influence": {"type": "infinite"}}]}
```

**Events.** `anim.changed` is emitted once per undo step with the union of affected nodes, channels and frame range;
`anim.hierarchy.changed`, `anim.camera.currentChanged`, `anim.tool.changed`, `anim.record.completed` and
`anim.display.changed` as listed in the table. Agents re-query with `anim.evaluate` after these events.

---

## 20. Design decisions / open questions

Each item records a decision taken in this section (with its recommended default) or a point the implementation team
must confirm.

1. **Units and field geometry** (§1.2): 1 field = 1 inch = 2.54 cm, isotropic; the default camera frame spans 12 fields
   horizontally on the table plane. Confirm that sections 02 (drawing space) and 09 (pixel mapping) use the same
   convention.
2. **Transform order and signs** (§2.1–2.2): scale → skew → rotate (each about its pivot) → translate; counter-clockwise
   positive rotation; single horizontal skew channel; vertical shear gestures decomposed into rotation + skew + scale.
3. **Static/animated combination** (§1.5): additive for position, rotation, skew and pivots, multiplicative for scale.
4. **Camera model** (§2.6–2.7, §12): horizontal FOV, default 41.1121° at distance 16 fields; rigid camera (inherited
   scale/skew ignored); near plane 0.1 field; one current camera per scene, not animatable over time; at least one
   camera per scene.
5. **Top/Side view orientation** (§3.3–3.4): Top view right = +X, up = −Z; Side view right = −Z, up = +Y.
6. **Compositing order** (§2.8): world-Z depth sort with Timeline order as tie-breaker; depth nudge 0.01 field.
7. **Velocity semantics** (§10.5): distance-along-path channel pinned at motion keyframes, intermediate keys stored
   normalised per segment; removed keyframes become intermediate velocity keys.
8. **Spline details** (§9.1–9.2): reflected phantom end points; 64 samples per segment for arc length, display and
   picking; inserting a control point does not re-fit the curve.
9. **Easing presets** (§10.4): the Bézier control points given are normative; visual tuning may adjust them only before
   the first release (they affect saved documents).
10. **Motion tool gestures** (§5.6): click on path = control point; add-key modifier or current-frame indicator =
    keyframe; dragging the node creates/moves the keyframe at the current frame.
11. **Bones** (§13.2–13.4): absolute bone channels; chained bones start at the parent's tip; linear-blend skinning with
    the given weight functions and ellipse defaults; CCD inverse kinematics (20 iterations, 0.001 field tolerance);
    forward follow-through factor 0.5 per depth level; deleting a bone re-chains its children by default.
12. **Motion recording** (§14): capture speed 0.1–10 (default 1), smoothness 0–100 (default 50, half-window
    smoothness ÷ 10 frames), one key per recorded frame with linear segments. A later key-reduction option (keep only
    significant keys within a tolerance) MAY be added.
13. **Peg-only mode** (§6.1): unparented elements are not pickable while it is on; the nearest (direct) parent peg is
    the target.
14. **Reparenting** (§7.2): interactive attach/detach keeps local values; `keepWorld` is offered to commands only.
    Consider exposing it interactively (e.g. via a modifier).
15. **Duration changes** (§8.4): the Timeline extends/truncates; proportional re-timing is a separate command. Loop
    types `count`/`fill`; position looping `cycle` by default, `offset` on request.
16. **Element roles** (§8.1): the four roles and their semantics are fixed in section 04 §6.1 (design decision there).
17. **Display defaults** (§4): show all pegs, pivot path, ghosts, arms and envelopes off; frame markers, effects, smooth
    paths and smooth tools on.
18. **Locking** (§6.6): lock affects interactive picking only; commands by id still apply.
19. **Snap last keyframe** (§6.5): applies only when channels are extended to a later end frame.
20. **Safe-area overlays**: not required; MAY be added as display options.
21. **Relationship to other namespaces (decided).** Showing effects in the scene views is `fx.allEffects.set` (06).
    `anim.prefs.*` commands write the staging preference keys of section 01 (same values, typed access).
    `anim.peg.create`, `anim.camera.create` and `anim.skeleton.create` are typed conveniences equivalent to
    `element.create` (04) with the respective type plus staging parameters; both paths produce identical documents.
    `anim.channel.*` copies animation channels; the Timeline keyframe clipboard of section 04 uses the same data.
