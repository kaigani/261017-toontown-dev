# 06 — Effects, Particles and Render Pipeline

This section specifies the application's **effect system** and the **render pipeline** that turns a scene into pixels.
It covers:

- the **effect element** model: how effects sit in the element hierarchy and apply to whole sub-trees, effect
  parameter elements (masks/mattes, sprite artwork, bone and skin groups), effect timing, keyframes and looping, and the
  global all-effects display switch;
- **every effect type** — Skeleton, Colour Transform, Clipping, Drop Shadow, Blur, Shake, the natural-phenomena particle
  effect and the action/impact particle effect — with all parameters, defaults, ranges and behaviours;
- **presets**: the preset data model (a preset is a named parameter set), built-in preset coverage and user presets;
- the **particle simulation engine**: emitters, particle types and variants, forces, lifetimes, floors and regions, and
  deterministic seeded simulation;
- the **effect preview** (final-quality preview of frames with all effects);
- the **render pipeline**: composition order, the interactive renderer and the final-quality renderer, colour-transform
  pipeline, clipping/flatten/blur scopes, hit-testing, antialiasing and display-quality options;
- the **agent interface** (`fx` namespace).

Cross-references: element hierarchy, Timeline rows and generic element commands (create, delete, rename, parent,
collapse, clone) are in `04_timeline_xsheet.md`. Pegs, cameras, bones, skeleton deformation, keyframe interpolation and
the function-curve editor are in `05_scene_planning.md`. Palettes, colour IDs, gradients and textures are in
`03_colour.md`. Playback, rendered previews and export (including SWF) are in `09_playback_export.md`. Persistence is in
`10_data_model_project_format.md`. Preferences storage is in `01_application_shell.md`. Generic agent tools are in
`11_agent_interface_mcp.md`; all command IDs are compiled into `A_command_registry.md`.

---

## 1. Concepts and terminology

| Term | Meaning |
|---|---|
| **Effect (effect element)** | An element (Timeline row) that changes how the elements attached beneath it are drawn, adds motion to them, or generates new imagery. Effects have no exposure-sheet cells and exist only in scene-planning (staging) mode. |
| **Effect sub-tree** | All elements attached beneath an effect, recursively, including those attached through intermediate pegs or effects. An effect applies to its whole sub-tree. |
| **Effect parameter element** | A special child row that is part of an effect's definition rather than an ordinary child: the **mask** of a Clipping effect, the **sprites** of a particle effect, the **bone group** and **skin group** of a Skeleton. Elements attached to a parameter element are used as inputs to the effect and are not drawn as ordinary content. |
| **Static parameter** | A parameter with one value for the whole effect (or particle block). |
| **Dynamic (animatable) parameter** | A parameter that can be keyframed; values are interpolated between keyframes. |
| **Effect keyframe** | A frame at which all animatable parameters of an effect (or particle block) are explicitly set. |
| **Particle block** | A time segment of a particle effect with its own independent simulation, particle type, preset, parameters and keyframes. |
| **Particle type / variant** | The simulation behaviour of a block (e.g. rain, snow, explosion). Some types have sub-behaviours (variants). |
| **Sprite / sprite set** | The artwork drawn for each particle. A sprite set is a named collection of drawings (variants) plus their colours. |
| **Preset** | A named, stored parameter set for an effect type (and, for particles, a particle type), applicable in one step. |
| **Interactive renderer** | The GPU-accelerated renderer used by the editing views for real-time display. |
| **Final-quality renderer** | The deterministic, fully antialiased renderer used by the effect preview, rendered previews, export and agent frame rendering. |
| **Seed** | An integer that, together with the parameters, fully determines a particle block's random choices. |

### 1.1 Coordinate spaces used in this section

| Space | Definition | Used by |
|---|---|---|
| **Scene space** | The 3-D staging space of section 05: X = east–west (positive east/right), Y = north–south (positive north/up), Z = front–back (positive towards the camera). Distances in **fields** (1 field = 1 inch; the default camera frame of a 4:3 scene spans 12 fields horizontally). | Drop shadow offset, shake amplitude, effect positions |
| **Effect-local space** | Scene space transformed by all pegs and transforms above the effect. The particle region and drop-shadow transform are expressed in this space, so moving a parent peg moves them. | Particle region, drop shadow |
| **Particle units** | Unit for particle region size, floor height and speeds. **100 particle units = 1 field.** Speeds are particle units per frame. | Particle parameters |
| **Output pixels** | Pixels of the rendered frame at the scene's full output resolution (section 09). Values given in output pixels scale proportionally when rendering at reduced size so the look is size-independent. | Blur radius |
| **View pixels** | Pixels of an on-screen view, origin top-left. | Hit-testing (§6.8) |

Frames are 1-based integers, consistent with all other sections.

---

## 2. Effect architecture

### 2.1 Effect catalogue

| Effect type (agent `type` id) | Purpose | Parameter elements created | Animatable parameters | Interactive views |
|---|---|---|---|---|
| Skeleton (`skeleton`) | Deforms drawings and bitmaps by a bone hierarchy (behaviour in section 05) | bone group, skin group | Bone poses (section 05) | Full |
| Colour Transform (`colourTransform`) | Multiplies and offsets colour and opacity of the sub-tree; tints, fades, flashes | none | 8 channels | Full (flatten may be approximated) |
| Clipping (`clipping`) | Shows the sub-tree only where the mask has coverage | mask | none | Full (hard edges allowed) |
| Drop Shadow (`dropShadow`) | Draws a flat-coloured, offset/distorted silhouette of the sub-tree behind it | none | none | Full |
| Blur (`blur`) | Softens the sub-tree uniformly (radial) or along a direction (motion) | none | radius, angle | Approximate; final look only in final-quality renders |
| Shake (`shake`) | Adds a decaying oscillating displacement to the sub-tree | none | amplitude | Full |
| Natural-phenomena particles (`natureParticles`) | Generates weather and nature particles (rain, snow, hail, fog, cloud, eruption) | sprites | per block: intensity, scale, bounciness, min/max speed, direction | Full (live particles) |
| Action/impact particles (`actionParticles`) | Generates action particles (shatter, magic, explosion, splash, fire, celestial) | sprites | as above | Full (live particles) |

There are no other effect types. Both particle effect types use the same particle engine (§4) and differ only in the
particle types they offer and their default type.

### 2.2 Effect data model

Each effect element consists of:

| Part | Content |
|---|---|
| **Element record** (shared with all elements, section 04) | id, name, parent and children, start frame, duration, loop count, collapsed/expanded state, visibility, lock, Timeline colour |
| **Effect record** | effect `type`; static parameter values; animatable parameters as function curves with keyframes (section 05 function model); for particle effects, the ordered list of blocks; references to its parameter elements |

Every effect type reports through its type descriptor (§8.4):

- its parameter schema (name, type, unit, range, default, static or animatable);
- which parameter-element slots it has;
- whether it is **animated** (has at least one keyframe) — animated effects show keyframe markers on their Timeline track;
- whether it is **compositing** (its sub-tree must be rendered into one intermediate image before the effect is applied:
  Clipping, Blur, and Colour Transform with *Flatten* on);
- whether it contributes a **transform** to its children (Shake);
- whether it is **loopable** (§2.6);
- whether it is **sequenced** (consists of independent blocks — only the particle effects);
- whether it may be cloned (all effect types may be cloned; cloning copies the effect record and parameter elements,
  section 04).

### 2.3 Effect parameter elements

- Parameter elements appear in the Timeline as rows directly beneath their effect, before its ordinary children, and use
  their own default Timeline colour (*Effect Parameters* colour, distinct from the *Effects* colour; the Skeleton uses
  the *Skeletons and Bones* colour; colours are preferences, section 01).
- Parameter elements are created automatically with the effect and cannot be deleted on their own; deleting the effect
  deletes them. Elements attached under them are ordinary elements (drawings, images, media, pegs) that the user or
  agent attaches by re-parenting.

| Effect | Slot | Role |
|---|---|---|
| Clipping | `mask` | Elements attached here define the clipping shape (matte). They are not drawn. |
| Natural-phenomena / action particles | `sprites` | Drawings or images attached here replace the built-in sprite set (§3.8.6). They are not drawn as ordinary content. |
| Skeleton | `bones` | The bone hierarchy (section 05). |
| Skeleton | `skin` | Drawing and image elements deformed by the bones (section 05). |

### 2.4 Effects in the hierarchy

- Effects can be both parents and children. An effect can be attached under a peg, another effect or a skeleton, and can
  have drawings, images, media, pegs and other effects attached beneath it.
- **Scope.** An effect applies to every element in its sub-tree, including elements reached through intermediate pegs or
  effects.
- **Nesting.** Effects compose from the inside out: a child effect is applied to its own sub-tree first, and the result
  is treated as a single child of the parent effect. Example: a Drop Shadow under a Colour Transform produces a shadowed
  group that is then colour-transformed (refined by the Drop Shadow's *parent colour transform mode*, §3.4).
- **Stacking.** Children of an effect are layered in Timeline order, exactly as children of a peg (section 04/05). Quick
  previews in the drawing view use exposure-sheet column order instead (section 04).
- **Attaching.** Elements are attached to and detached from an effect with the generic parenting commands of section 04
  (including "attach to the nearest peg or effect above" and "detach from parent"). When an effect is created with
  elements specified as children (`childIds`), those elements are re-parented under it in one undo step. In the GUI,
  creating an effect does **not** auto-parent selected elements, except for the Skeleton (section 05: the selected
  drawing goes into the skin group).
- Hierarchy navigation (select children / child / parent / previous / next sibling) works on effect hierarchies exactly
  as on peg hierarchies (section 04).

### 2.5 Creating effects

- An effect can be created from: the element-creation command (section 04, with any effect type), a dedicated
  create-effect command per type reachable from the main menu, an effects toolbar, the Timeline's add-element control,
  and the exposure-sheet context menu (at least for Drop Shadow). All are clients of `fx.create`.
- **Count and naming.** One creation request may create 1–99 effects of the same type. Names: 1–20 characters, no
  leading or trailing whitespace, none of `. * / \ < > : ? " |`, unique among the scene's elements. Errors:
  `element.nameEmpty`, `element.nameInvalid`, `element.nameExists`, `element.countOutOfRange`. Default names are the
  type's display name followed by a number that makes them unique.
- **Default timing.** A new effect starts at frame 1 and lasts the scene length. If children are given, it starts at the
  earliest start and ends at the latest end of those children.
- **Drawing mode.** Effects can be created while the application is in drawing mode, but they are not visible there. The
  application MUST warn the user (non-blocking) that the new elements are only visible and editable in scene-planning
  mode and how to switch. For agents this is a `warnings[]` entry `fx.notVisibleInDrawingMode`.
- **Template editing.** Creating or modifying effects is not allowed while editing a template in place (section 08);
  error `fx.notAllowedInTemplateEdit`.
- Creation is one undoable step ("Create Effect"; the Skeleton uses its own label).
- Type-specific creation:
  - **Clipping** creates the effect plus its `mask` parameter element.
  - **Natural-phenomena particles** creates the effect plus its `sprites` parameter element and one block spanning the
    effect, set to the **rain** type with the default rain preset; particles are immediately visible in the camera view.
  - **Action/impact particles** creates the effect plus its `sprites` parameter element and one block set to the
    **shatter** type with the default shatter preset (a masonry wall breaking apart); immediately visible.
  - **Skeleton** creates the effect plus bone and skin groups (section 05).
  - All other types are created with the defaults listed in §3.

### 2.6 Timing, duration and looping

- An effect occupies a frame span: *start frame* and *duration* (frames). On the Timeline it is drawn as a bar from start
  to end; dragging the bar moves the start; dragging either end changes the duration. A minimum duration of 1 frame is
  enforced.
- **Changing duration** stretches or crops all of the effect's animation functions. Recommended rule: keyframes are
  rescaled proportionally when the bar's end is dragged and when the duration is set numerically with
  `scaleKeys: true`; with `scaleKeys: false` keyframes stay at their frames and those beyond the new end are removed
  (with confirmation, §8.1).
- **Looping.** Colour Transform, Blur and Shake effects can be looped: with loop count *n*, the effect's keyframed
  animation over its duration is repeated *n* times in consecutive spans, extending the bar to *duration × n*. Clipping
  and Drop Shadow accept a loop count (it only extends their span). **Particle effects are not loopable**; their span is
  defined by their blocks (§3.8.2).
- Outside its span an effect does nothing: its sub-tree is drawn as if the effect did not exist, except that elements
  attached to parameter elements are never drawn as ordinary content.
- **Effect-local frame.** Within the span, parameters are evaluated at the local frame `(frame − start) mod duration + 1`
  when looped, else `frame − start + 1`.
- Start, duration and loop changes are each one undoable step.

### 2.7 Timeline representation

| Aspect | Requirement |
|---|---|
| Row | An effect row with a type-specific icon; a dimmed icon style when the effect or its parameter elements are inactive |
| Track | A bar from start to end (see §2.6) |
| Keyframes | Animated effects show a keyframe marker on each keyframe frame. A collapsed Skeleton shows its keyframe markers only when expanded (section 05). |
| Particle blocks | The bar is drawn as consecutive blocks; each block shows its own keyframe markers and its type icon |
| Loops | Loop repetitions are drawn as repeated segments |
| Collapse / expand | Collapsing hides all child and parameter rows (section 04) |
| Parameter rows | Shown or hidden independently of collapse (§2.8) |
| Colours | Effects, Effect Parameters and Skeletons-and-Bones default colours are preferences (section 01) |

### 2.8 Parameter-row display

Three commands control whether parameter rows (and the elements attached to them) are listed in the Timeline:

| Command | Behaviour |
|---|---|
| Show/hide parameters of the selected effects | Toggles parameter-row visibility for the selected effect(s). Enabled only when at least one selected effect has parameter elements. Shown as checked when the selected effect's parameter rows are visible. Applies to every effect type with parameter elements (Clipping, particles, Skeleton). |
| Show all effect parameters | Shows the parameter rows of every effect in the scene. |
| Hide all effect parameters | Hides the parameter rows, and the elements attached to them, of every effect in the scene. |

Hiding parameter rows only declutters the Timeline. It MUST NOT disable the effect or hide the mask or sprites in any
render. This state is view state (not undoable) but is saved with the scene's view settings.

### 2.9 All-effects display switch

- A global switch (default **on**) controls whether effects are drawn in the interactive views. When **off**, the
  interactive views draw every element as if no effects existed: no clipping (mask elements are drawn as ordinary
  drawings), no colour transform, no drop shadow, no blur, no shake displacement, no particles. Skeleton deformation is
  **not** affected (it is posing, not an effect look).
- The switch is display state only: it is not document data, not undoable, persisted as a preference, and it MUST NOT
  affect the effect preview, rendered previews, export or agent final-quality frame rendering, which always render all
  effects. (`render_frame` with `quality: "draft"` honours the switch, §8.1.)

### 2.10 Effect keyframes (generic rules)

- Effects with animatable parameters (Colour Transform, Blur, Shake, particle blocks) support **add keyframe** and
  **remove keyframe** at the current frame. A keyframe stores values for all animatable parameters of the effect (or
  block) together.
- Between keyframes values are interpolated using the interpolation of the underlying function curves (default per
  section 05 preferences for new functions); before the first keyframe and after the last the nearest keyframe's value
  is held.
- With no keyframes an effect is **static**: each animatable parameter has one constant value.
- **Editing a value at a frame** (rule `keyMode: "auto"`):
  1. no keyframes exist → the constant value is changed;
  2. the frame is a keyframe → that keyframe is changed;
  3. keyframes exist and the frame is not a keyframe → a keyframe is created at the frame (all parameters take their
     interpolated values there) and the edited parameter is set on it.
- Adding a keyframe at a frame that is already a keyframe is a no-op (result `created: false`). Removing the last
  keyframe makes the effect static at that keyframe's values.
- Keyframes can be added only within the effect's span (or the block's span for particle effects); error
  `fx.frameOutsideSpan`.
- Animatable parameters are ordinary function curves and MUST be editable in the function-curve editor of section 05
  (values, interpolation, ease).
- Each add, remove or edit is one undoable step.

### 2.11 Parameter editing in the GUI (generic)

- Each effect type with parameters has a properties panel that shows the selected effect's values at the current frame
  and updates when the frame changes.
- Numeric parameters are edited with a slider plus a numeric field. A slider drag commits one undoable step on release.
- Typed values are validated: text that does not parse as a number restores the previous value; out-of-range values are
  rejected with an indication of the allowed minimum or maximum (agent errors `fx.valueBelowMinimum` /
  `fx.valueAboveMaximum`, unless `clamp: true`).
- When the current frame is outside the effect's span the panel still shows the values at the nearest frame of the span
  and edits apply there. For particle effects, when the current frame is outside every block, the panel shows the
  nearest block disabled until the frame is moved into a block.
- Clipping has no parameters panel beyond the generic element properties (and its optional *inverted* switch, §3.3).

---

## 3. Effect types

### 3.1 Skeleton

The Skeleton is specified in section 05 (bones, bone tools, influence, deformation, resting position). Within the
effect model:

- It deforms the drawings and images attached to its **skin** group using the bones in its **bones** group.
- It has a properties panel with: name, start, duration and loop state (read-only; edited via timing commands), velocity
  curve, skeleton display colour, show track, show root, show influence, vector deformation quality (low, medium, high,
  highest), root transform (scale X/Y with keep-proportions, skew, rotation, pivot applies to rotation or scaling,
  constant Z, orient to path), and copy resting position to the current frame or to all keyframes. All of these are
  commands in the `anim` namespace (section 05).
- **Render:** deformed drawings are re-tessellated or warped per frame (section 05); the result is drawn like any other
  drawing.
- SWF export cannot deform bitmaps: exporting a scene with deformed bitmaps or deformed bitmap textures produces warnings
  (codes owned by section 09).
- The all-effects switch does not disable skeleton deformation.

### 3.2 Colour Transform

**Purpose:** change the colours and opacity of everything in the sub-tree, optionally animated over time (fade in/out,
tint, darken, flash).

**Parameters**

| Parameter (`name`) | Type | Range | Default | Animatable | Meaning |
|---|---|---|---|---|---|
| Multiply red (`multRed`) | number | 0.000–1.000 (3 decimals) | 1.000 | yes | Factor applied to the red channel |
| Multiply green (`multGreen`) | number | 0.000–1.000 | 1.000 | yes | Factor for green |
| Multiply blue (`multBlue`) | number | 0.000–1.000 | 1.000 | yes | Factor for blue |
| Multiply alpha (`multAlpha`) | number | 0.000–1.000 | 1.000 | yes | Factor for opacity (fade) |
| Add red (`addRed`) | int | −255…+255 | 0 | yes | Offset added to red after multiplication (0–255 scale) |
| Add green (`addGreen`) | int | −255…+255 | 0 | yes | Offset for green |
| Add blue (`addBlue`) | int | −255…+255 | 0 | yes | Offset for blue |
| Add alpha (`addAlpha`) | int | −255…+255 | 0 | yes | Offset for opacity |
| Flatten (`flatten`) | bool | — | false | no | Composite the sub-tree first, then transform (below) |

**Formula** (per pixel, channels in 0–1, applied to straight (un-premultiplied) colour):
`out = clamp(in × mult + add / 255, 0, 1)` for each of R, G, B, A.
Pixels with zero alpha are left unchanged: fully transparent areas stay transparent even with a positive additive alpha.

- **Animation.** Each of the 8 channels is a separate function curve; keyframes are added and removed for all 8 together.
- **Flatten off** (default): each drawable in the sub-tree is transformed individually, so overlapping semi-transparent
  children show through each other.
- **Flatten on:** the sub-tree is first composited into one image, the transform is applied to that image, and the
  result is composited. With alpha < 1 overlapping children fade as one unit. The interactive renderer MAY approximate
  this; the final-quality renderer MUST be exact.
- **Nesting.** For an inner transform (m₁, a₁) under an outer transform (m₂, a₂) the combined transform is
  `mult = m₂ × m₁`, `add = a₂ + m₂ × a₁` (per channel), with clamping applied after each stage only where a flatten
  scope forces an intermediate image.
- **Import/export.** SWF export maps it to SWF colour transforms (section 09). If SWF import is supported (section 09),
  SWF colour transforms are imported as Colour Transform effects.

### 3.3 Clipping

**Purpose:** mask the sub-tree by a shape. Ordinary children are visible only where the mask has coverage.

Structure:

```
Clipping effect
 ├── mask (parameter element)       ← attach matte drawings/images here; they are not drawn
 │     └── matte element(s)
 └── clipped element(s)             ← ordinary children; these are what you see
```

| Topic | Requirement |
|---|---|
| Mask source | Union of the coverage (alpha) of all elements attached under the mask parameter element at the current frame, with their own pegs and transforms |
| Visible result | Children are drawn only inside the mask; the mask artwork itself is never drawn |
| Inverted (`inverted`) | bool, default false, static. When true, children are drawn only **outside** the mask. |
| Edge quality | Final-quality renderer: antialiased; partially transparent mask pixels give proportional visibility. Interactive renderer MAY use hard (binary) edges. |
| Animation | No parameters of its own to animate (only `inverted`, static); animate the matte elements or their pegs instead |
| Empty mask | If no element is attached to the mask, or the mask has no coverage at a frame, the children are invisible (non-inverted) or fully visible (inverted) |
| All-effects off | Children drawn unclipped; mask elements drawn as ordinary drawings |
| SWF export | Mapped to SWF clip layers covering the masked objects (section 09) |

### 3.4 Drop Shadow

**Purpose:** draw a flat-coloured, semi-transparent silhouette of the sub-tree, offset or distorted, behind it.

**Parameters** (all static; Drop Shadow is not animatable)

| Parameter (`name`) | Type | Range / values | Default | Meaning |
|---|---|---|---|---|
| Parent colour transform mode (`parentColourTransformMode`) | enum | `drawingsOnly`, `drawingsAndShadow`, `shadowOnly` | `drawingsOnly` | How a Colour Transform effect **above** this Drop Shadow is applied: to the original drawings only, to both, or to the shadow only |
| Colour (`colour`) | colour (RGB; alpha ignored) | any | #000000 | Every shadow pixel takes this colour; the source alpha shape is kept |
| Opacity (`opacity`) | int % | 0–100 | 50 | Multiplies the silhouette's alpha |
| Hide elements (`hideElements`) | bool | — | false | When on, only the shadow is drawn (useful for stand-alone shadow layers) |
| Shadow transform (`transform`) | 2-D affine `{xAxis:{x,y}, yAxis:{x,y}, origin:{x,y}}` in effect-local space, fields | any | xAxis (1,0), yAxis (0,1), origin (0.2, −0.2) | Position, scale and skew of the shadow relative to the elements. Default = offset 0.2 field right and 0.2 field down |

- Convenience: the transform can also be set as `offset {x,y}`, `scale {x,y}` and `skew` (degrees), which the
  application converts to the matrix. **Reset position** restores the default transform.
- **Interactive manipulation.** When a Drop Shadow effect is selected in the camera view, a shadow manipulator shows
  handles on the shadow's bounding box: drag inside to move the shadow, drag handles to scale or skew it; with the
  constrain modifier held, movement is restricted to one axis; Escape or a secondary click cancels the drag in progress.
  Each completed drag is one undoable step.
- **Render model:**
  1. Render the sub-tree with the shadow transform appended, using a constant-colour transform
     (rgb = shadow colour, alpha = source alpha × opacity / 100). The silhouette is composited as one flattened image
     so overlapping parts do not accumulate opacity.
  2. Unless *hide elements* is on, render the sub-tree normally on top.
  3. An inherited parent Colour Transform is applied to the drawings and/or the shadow according to
     *parent colour transform mode*.
- All-effects off: no shadow is drawn.
- SWF export: shadows are exported as additional placed instances with the shadow matrix and a constant-colour
  transform (section 09).

### 3.5 Blur

**Purpose:** soften the sub-tree; animate the amount for soft focus and focus pulls; in motion mode, smear along a
direction.

**Parameters**

| Parameter (`name`) | Type | Range | Default | Animatable | Meaning |
|---|---|---|---|---|---|
| Radial (`radial`) | bool | — | true | no | true = uniform blur in all directions around each pixel (not a zoom blur); false = directional (motion) blur |
| Radius (`radius`) | number (decimals allowed) | 0–100 output pixels | 4 | yes | Blur strength; in motion mode, the smear length |
| Angle (`angle`) | number (degrees) | 0–360 | 0 | yes | Direction of the motion blur (0 = east, counter-clockwise positive). Ignored, and shown disabled, when radial is on |

- **Rendering:** the sub-tree is rendered into an intermediate image padded by the radius (blur extends the content's
  bounds), filtered, and composited. Radial mode: a Gaussian-like separable blur of the given radius on premultiplied
  colour. Motion mode: a directional blur along the angle with length = radius. Radius 0 = no blur.
- The radius is in output pixels at full output resolution and scales with the render size, so preview size does not
  change the look.
- **Interactive views:** blur MAY be omitted or approximated; the final look is shown by the effect preview, rendered
  previews, export and final-quality agent renders.
- SWF export: native SWF blur filter (radial) or directional blur filter (motion) (section 09).

### 3.6 Shake

**Purpose:** add a decaying oscillating jitter to the sub-tree (camera shake, impacts, earthquakes).

**Parameters**

| Parameter (`name`) | Type | Range | Default | Animatable | Meaning |
|---|---|---|---|---|---|
| Axis (`axis`) | enum | `x`, `y`, `xy`, `z`, `xyz` | `xy` | no | Directions along which the sub-tree is displaced (`z` = front–back) |
| Wave time (`waveTime`) | int frames | 1–999 | 4 | no | Oscillation period: frames per full back-and-forth cycle |
| Half-life (`decayHalfLifeFrames`) | number frames | 0.1–999 | 6 | no | Frames for the oscillation to decay to half its amplitude |
| Amplitude (`amplitude`) | number fields | 0–100 | 0.5 | yes | Peak displacement at each keyframe |

- **Motion model.** Shake contributes a translation to its children's transform (they are moved, not redrawn
  differently). Each amplitude keyframe acts as an **impulse** that restarts the oscillation at that keyframe's
  amplitude. With no keyframes the effect behaves as a single impulse at the effect's first frame. For each enabled axis:

  `d(t) = A × 2^(−(t − tₖ)/decayHalfLifeFrames) × sin(2π (t − tₖ)/waveTime + φ_axis)`

  where `t` is the effect-local frame, `tₖ` the most recent keyframe at or before `t` (or the first frame), `A` its
  amplitude, and `φ_axis` a fixed per-axis phase (`x` = 0, `y` = π/2, `z` = π/4) so that axes do not move in lockstep.
  Shake is fully deterministic.
- Shake renders identically in interactive views, preview and export (it is only a transform). SWF export bakes it into
  per-frame object matrices.
- **Presets.** At least three built-in shake presets: *small* (low amplitude, short wave time, short half-life),
  *medium* and *large* (high amplitude, longer half-life). Applying one sets axis, wave time, half-life and amplitude in
  one undoable step.

### 3.7 Presets (all effect types)

A **preset** is a named parameter set for one effect type. Applying a preset replaces the corresponding parameters of
an effect (or, for particle effects, of one block) in one undoable step. Presets exist for every effect type except
Skeleton; built-in presets ship for Shake (§3.6), the particle types (§3.8.5) and SHOULD ship for Colour Transform
(fade to transparent, fade from transparent, fade to black, fade to white, darken, warm tint, cool tint, flash) and Blur
(soft focus, strong blur, horizontal motion blur, focus pull).

**Preset data model**

| Field | Type | Meaning |
|---|---|---|
| `id` | id | Stable identifier. Built-in presets have fixed ids; user presets get generated ids. |
| `name` | string | Display name (1–60 characters, unique per effect type and particle type within its source) |
| `source` | enum `builtin`\|`user` | Built-in presets are read-only |
| `effectType` | enum (effect type id) | Which effect type it applies to |
| `particleType` | enum? | For particle effects: the particle type it sets |
| `variant` | enum? | For particle types with variants |
| `tags` | [string] | Coverage tags (e.g. `light`, `heavy`, `stylised`), used for filtering |
| `thumbnail` | image (PNG) | A representative rendered image or icon shown on the preset button |
| `static` | object | Values for static parameters (only those present are applied) |
| `dynamic` | object | For each animatable parameter either a constant value or a keyframe list `[{frame, value, interpolation?}]` with frames relative to the block or effect start (1 = first frame) |
| `spriteSetId` | id? | Particle presets: the built-in sprite set to use |
| `background` | enum? | Particle presets: background |
| `seed` | int? | Optional fixed seed; when absent the block keeps its current seed |
| `version` | int | Preset schema version |

Rules:

- Applying a preset to a particle block also sets the block's type (and variant) to the preset's; parameters absent from
  the preset are reset to the type's defaults. Custom sprites attached to the `sprites` parameter element still override
  the preset's sprite set.
- Keyframe lists longer than the target span are cropped; shorter ones end with a hold.
- **User presets** can be saved from any effect (or block), renamed, deleted, exported to and imported from a file
  (a JSON document following this model; file naming and location defined in section 10). Saving over an existing user
  preset name requires `onConflict: "replace"` (or `"rename"`); otherwise the result is `needs_confirmation`.
- In the GUI, presets appear as one-click buttons (with thumbnails and names) in the effect's properties panel, grouped
  by particle type for particle effects; at least 10 preset buttons per particle type must fit without scrolling.

### 3.8 Particle effects (natural-phenomena and action/impact)

#### 3.8.1 Purpose and categories

Both particle effect types create a **particle generator** that emits many copies (particles) of small drawings
(sprites) and moves them with a physics-like simulation in a 3-D region in effect-local space. The type decides the
available particle types and the default:

| Effect type | Particle types offered | Default type on creation |
|---|---|---|
| `natureParticles` | `rain`, `snow`, `hail`, `fog`, `cloud`, `eruption` | `rain` |
| `actionParticles` | `shatter`, `magic`, `explosion`, `splash`, `fire`, `celestial` | `shatter` |

Unlike other effects, a particle effect **draws new content**: its own output is the particles (plus an optional
background). Elements attached to its `sprites` parameter element are used only as sprite artwork. Ordinary elements
attached directly under the particle effect are drawn normally beneath the particles' parent transform (the effect acts
like a group for them); the particles are drawn in front of them.

#### 3.8.2 Blocks

- A particle effect's span is divided into one or more consecutive **blocks**. Each block is an independent simulation
  with its own particle type, variant, preset, seed, static parameters and keyframed dynamic parameters. The effect's
  span is the union of its blocks.
- **Add block at frame:** splits the block under the given frame at that frame (the new block begins there), or appends
  a new block if the frame is just after the last block. Not allowed outside the effect span (other than directly after
  it) or on an existing block start: `fx.blockStartExists`, `fx.frameOutsideSpan`. The new block starts with the
  defaults of the type of the block it was split from (or of the effect's default type) and a new seed.
- **Remove block:** removes the block under the given frame; its frames are merged into the preceding block (or the
  following block if it is the first). The last remaining block cannot be removed (`fx.lastBlock`); delete the effect
  instead.
- The properties panel always shows the block under the current frame and refreshes when the frame changes.
- Changing a block's duration resizes it; following blocks shift so blocks stay consecutive.

#### 3.8.3 Block static parameters

Constant for the whole block; editable from any frame of the block.

| Parameter (`name`) | Type | Range | Default | Meaning |
|---|---|---|---|---|
| Particle type (`type`) | enum | per effect type (§3.8.1) | effect default | Simulation behaviour. Changing it loads that type's defaults. |
| Variant (`variant`) | enum | per type (§3.8.4) | type default | Sub-behaviour |
| Background (`background`) | enum | `transparent`, `darkSkyGradient`, `lightSkyGradient` | `transparent` | Optional full-frame sky gradient (dark blue or light blue) drawn behind the particles; transparent draws none |
| Start of simulation (`simulationStart`) | int frame | ≤ `firstFrame`; ≥ `firstFrame` − 999 | `firstFrame` | First frame the simulation runs. Earlier than the first displayed frame = **pre-roll**, so particles are already in motion when the block appears |
| First frame (`firstFrame`) | int frame | within effect span | block start | First frame the block is displayed |
| Duration (`duration`) | int frames | ≥ 1 | block length | Frames the block lasts; changing it resizes the block |
| Max objects (`maxParticles`) | int | 0 – type cap | per type | Hard cap on particles alive at once; births beyond the cap are discarded. Type cap: 3000 for rain, snow, hail, eruption and all action types; 50 for fog and cloud. Typed values above the cap are clamped. |
| Lifetime (`lifetimeFrames`) | int frames | −1, or 0–512 | per type | Particles older than this are removed. −1 = unlimited (removed only when leaving the region). |
| Fade (`fade`) | bool | — | per type | Particles fade out as they approach their lifetime and after landing on the floor |
| Region width (`areaWidth`) | number particle units | 0–1999 | per type | Width (X) of the emission/simulation box |
| Region height (`areaHeight`) | number particle units | 0–1999 | per type | Height (Y) of the box |
| Region depth (`areaDepth`) | number particle units | 0–1999 | per type | Depth (Z) of the box; gives perspective scaling under the camera |
| Floor height (`floorLevel`) | number particle units | 0–1999 | per type | Height of the floor plane above the bottom of the region; particles stop, rest, fade or bounce there |
| Seed (`seed`) | int | 0 – 2³²−1 | random at block creation, then stored | Determines all random choices of the simulation (§4.6) |
| Sprite set (`spriteSetId`) | id | built-in sprite sets valid for the type | from preset | Built-in artwork used when no custom sprites are attached |

The region is centred horizontally and in depth on the effect's origin, with its bottom at the origin; pegs above the
effect move, rotate and scale the whole region.

#### 3.8.4 Block dynamic parameters

Keyframeable within the block; interpolated between keyframes (§2.10 rules apply per block).

| Parameter (`name`) | Type | Range | Meaning |
|---|---|---|---|
| Intensity (`intensity`) | number | 0–999 | Emission rate: particles born per frame |
| Scale (`scale`) | number % | 0–199 | Sprite scale relative to native size (100 = native) |
| Bounciness (`bounciness`) | number % | 0–99 | Restitution when striking the floor (0 = no bounce); used by bouncing types |
| Min speed (`speedMin`) | number particle units/frame | 0–199 | Minimum initial speed |
| Max speed (`speedMax`) | number particle units/frame | 0–199 | Maximum initial speed; each particle's speed is random between min and max. Min ≤ max is enforced: setting one past the other moves the other to the same value. |
| Direction east–west (`directionEW`) | number | −1000…+1000 | X component of the emission/wind direction (positive = east) |
| Direction north–south (`directionNS`) | number | −1000…+1000 | Y component (positive = north/up) |
| Direction front–back (`directionFB`) | number | −1000…+1000 | Z component (positive = towards the camera) |

The direction components form a vector that is normalised to give the emission/wind direction; its length relative to
1000 (capped at 1) scales the strength of lateral wind drift for falling types. A zero vector means the type's natural
direction (down for falling types, up for rising types, radial for burst types). Per-type defaults for all dynamic
parameters are part of the type descriptor.

#### 3.8.5 Particle types and behaviours

Each type is a behaviour script over the engine (§4): it chooses the emitter domain, the initial velocity rule, the
forces, collision and kill rules, sprite orientation, and optional per-birth customisation.

| Type | Variants | Behaviour |
|---|---|---|
| `rain` | — | Born across the top of the region; accelerated downward by gravity towards a terminal velocity, with wind; sprites oriented along velocity (streaks); die or fade at the floor (optional short splash bounce) |
| `snow` | — | Slow fall at near-constant terminal velocity with lateral drift and gentle sway; rest or fade at the floor |
| `hail` | — | Gravity fall; bounce off the floor with *bounciness* and friction until resting, then fade or die |
| `fog` | — | Few (≤ 50) large, slowly drifting, semi-transparent sprites filling the region box; particles that leave the box re-enter on the opposite side (wrap) |
| `cloud` | — | Few (≤ 50) large sprites drifting horizontally across the region; wrap at the sides |
| `eruption` | — | Emitted upward from a small source at the bottom centre, arcing under gravity, bouncing on the floor |
| `shatter` | — | A source image is broken into pieces at the first simulated frame; pieces fly apart from an impact point with gravity, floor bounce and friction; pieces below the floor or outside the region are removed. Uses the shatter sprite rule (§3.8.6) |
| `magic` | `glitter`, `bubbles`, `radiatingStars`, `spinningStars`, `circlingStars` | Glitter: sparkles falling under light gravity with random sprite variants. Bubbles: rising particles (negative gravity, terminal velocity) with sway. Radiating stars: stars emitted outward from a centre. Spinning stars: as radiating, with rotation. Circling stars: stars orbiting a centre |
| `explosion` | `blast`, `electricSparks`, `fireworks` | Blast: smoke and debris bursting outward from a point, slowing by drag, expanding and fading. Electric sparks: crackling short-lived sparks jumping along a path across the region. Fireworks: rockets rising then bursting into coloured star particles (secondary emission at burst) |
| `splash` | — | Liquid droplets ejected upward and outward from a point, falling back under gravity, fading on the floor |
| `fire` | `flame`, `smoke`, `sparks` | Flame: rising, shrinking and fading flame sprites from a base line. Smoke: rising, expanding, fading puffs drifting with wind. Sparks: fast, short-lived sparks |
| `celestial` | `fireBurst`, `lightBurst`, `meteorRain` | Fire burst: radial burst of fire and light sprites from a centre. Light burst: radial burst of light sprites. Meteor rain: diagonal falling meteors with trails |

Default variants: `glitter`, `blast`, `flame`, `fireBurst`.

**Built-in preset coverage** (content to be authored fresh):

- Every particle type MUST ship at least **9** built-in presets.
- Each type MUST include its standard look in three strengths (e.g. light / normal / heavy, or small / medium / large),
  one of which is the type's default preset.
- Natural-phenomena types MUST each also include at least 6 stylised alternatives using different sprite sets, together
  covering: realistic weather, cartoon/stylised weather, magical/whimsical objects falling or drifting (e.g. stars,
  petals, leaves, sweets, coins, sparkles), light effects (rays, glows), smoke/steam/mist, and landscape fillers
  (bushes, forest silhouettes for drifting fog/cloud layers).
- Action/impact types MUST cover each variant in three strengths: shatter for at least three source materials
  (masonry, glass, a bottle or ceramic object) in three fragment sizes; magic (glitter, bubbles, stars); explosion
  (blast, electric sparks, fireworks); splash for at least three liquids (water, slime/goo, mud); fire (flame, smoke,
  sparks); celestial (fire burst, light burst, meteor rain).
- Built-in sprite sets: every preset references a built-in sprite set authored for it; each sprite set has at least 3
  drawing variants (at least 20 for fog and cloud to avoid visible repetition) and its own colours (palette, section
  03).
- Every built-in preset has a thumbnail.
- If a saved file references an unknown particle type, the block is preserved, shown with a generic "unknown type"
  icon, renders nothing, and can be changed to a known type.

#### 3.8.6 Custom sprites

- Attaching a drawing or image element to the effect's `sprites` parameter element overrides the built-in sprite set for
  **all blocks** of the effect. With nothing attached, each block uses its sprite set.
- **Multiple drawings = variants.** Each distinct drawing (exposed cell) of the attached element(s) is a separate sprite
  variant. Each particle picks a variant at birth (seeded random); drawings are not played as an animation sequence.
  Example: three glitter drawings in three colours give multicoloured glitter.
- **Shatter rule:** the first drawing is the image to be broken; the second drawing defines the pieces (its closed
  shapes are the fragment outlines) and the area of the first drawing to break. With only one drawing, the image is
  broken into automatically generated fragments of a size set by the preset.
- Sprites are drawn with their own palettes; a sprite drawing's own pegs/transforms are ignored, only its artwork is
  used (centred on the drawing's origin).
- Elements attached under the `sprites` parameter element are never drawn as ordinary layers.
- SWF export writes particles as per-frame placed drawing or bitmap symbols (section 09).

#### 3.8.7 Particle drawing

- Each particle is drawn as its sprite with a per-particle transform: translation to its position, scale =
  particle size × *scale* / 100 × depth perspective, rotation (for rotating types, or orientation along velocity for
  streak types), and opacity (fading with age when *fade* is on, or as the type defines).
- Draw order within a block: background (if any), then particles sorted **back to front by depth** (farther first);
  equal depths keep birth order.
- Particles are drawn live in the interactive views and identically (with antialiasing) by the final-quality renderer.
- Particles are not individually selectable.

#### 3.8.8 Edge cases

- Invalid numeric entry is reverted (GUI) or rejected/clamped (agents, §2.11).
- Typing more than the type's object cap clamps to the cap.
- Intensity 0 at all frames, or max objects 0, produces no particles (background still drawn).
- If the first frame is before the start of simulation, start of simulation is moved to the first frame.
- Region size 0 on an axis collapses emission to a plane or line on that axis.

---

## 4. Particle simulation engine

The particle effects run on a particle engine with the functional model below. Any implementation is acceptable if it
meets these behaviours and the determinism requirements of §4.6. One simulation step is executed per frame; positions
are in particle units in effect-local space (X = east–west, Y = north–south, Z = front–back), velocities in particle
units per frame.

### 4.1 Particle state

| Field | Meaning |
|---|---|
| position (3-D) | Location within the region |
| previous position | For streak orientation and collision tests |
| up vector, rotation velocity | Sprite orientation and spin |
| velocity (3-D) | Units per frame |
| size | Base sprite scale (randomised per type, multiplied by *scale*) |
| colour tint, alpha | Tint and opacity (alpha used for fades) |
| age | Frames since birth |
| mass | Used by drag and some type behaviours |
| sprite variant index | Which sprite variant to draw |
| state flags | e.g. landed / resting, secondary emitter (firework rocket) |

Particles live in **particle groups** with a maximum size (= max objects). A block may use several groups (e.g. flames
and sparks); the cap applies to the block's total. A **birth hook** lets a type customise each new particle (shatter
fragment assignment, firework rockets, flame shapes, radiating stars).

### 4.2 Emitter state

| Setting | Meaning |
|---|---|
| Position domain | Where new particles are born (§4.3) |
| Velocity domain | Initial velocity sampled from a domain, typically a cone or box around the direction vector, magnitude between min and max speed |
| Rotation velocity | Initial spin, fixed or sampled from a domain |
| Starting age | Base ± spread, so particles do not all expire together |
| Size, tint, variant | Sampled per type |

### 4.3 Domains

Point, line, triangle, rectangle, disc, plane, box, cylinder, cone, sphere. Each domain can **generate** a random point
inside itself (using the block's random stream) and **test** whether a point is inside. Typical uses: rain, snow and
hail are born in a box or rectangle across the top of the region; eruption, splash and explosion from a point, disc or
sphere; the floor is a plane at floor height; the region kill/wrap test uses a box. Collision (bounce) domains MUST be
surfaces of zero thickness.

### 4.4 Actions (applied every step, in the type's order)

| Action | Parameters | Behaviour |
|---|---|---|
| **Emit** | rate, domain | Creates `rate` new particles this step (rate = intensity; fractional rates accumulate across frames), positions from the domain, initial state from the emitter |
| **Gravity** | acceleration vector | Adds a constant acceleration to every velocity (down for rain/hail, up for bubbles/smoke) |
| **Bounce** | friction, restitution, cutoff speed, surface domain | When a particle would cross the surface this step, reflect its normal velocity × restitution (= bounciness / 100) and reduce tangential velocity by friction; below the cutoff speed friction is not applied |
| **Drag** | factor per axis, low and high speed limits | Multiplies velocity by a factor when speed is between the limits (air drag) |
| **Expire** | age limit | Removes particles older than the limit (= lifetime; skipped when −1) |
| **Move** | — | position += velocity; orientation += rotation velocity; age += 1 |
| **Remove inside/outside** | domain, inside flag | Removes particles inside (or outside) a domain: region bounds, below the floor |
| **Wrap** | domain | Particles leaving a box re-enter on the opposite face (fog, cloud) |
| **Speed limit** | min, max | Clamps each particle's speed into [min, max] |
| **Target velocity** | velocity, blend factor | Blends velocity toward a target per step (terminal velocity, wind) |
| **Avoid** | look-ahead, strength, domain | Steers particles around an obstacle domain (available to type behaviours) |

Engine misuse (invalid group, invalid action order) is a developer error, never shown to users.

### 4.5 Per-frame pipeline for one block

1. **Initialise** (first request or after invalidation): type defaults → preset values → the block's own parameters;
   seed the random generator from the block's seed (§4.6).
2. **Step** for each frame `f` from *start of simulation* to the requested frame:
   1. evaluate the dynamic parameters at `f` (intensity, scale, bounciness, speeds, direction);
   2. run the type's actions: Emit → type forces (gravity, target velocity, drag, floor bounce, friction) → speed limit
      → Move → Expire → remove outside region / below floor (or Wrap);
   3. snapshot the particle list for `f` (cacheable).
3. **Draw** if `f` ≥ first frame and within the block: background, then particles back to front (§3.8.7).

Caching: snapshots MAY be cached per block; any parameter, seed, type, timing or sprite change invalidates that block's
cache. A cached result MUST be identical to a from-scratch simulation.

### 4.6 Determinism

- Each block has a stored integer **seed**. All random choices (birth positions, velocities, sizes, sprite variants,
  fragment shapes, spark paths) are drawn from a pseudo-random generator seeded from (seed, block index within the
  effect, particle-group index), consuming random numbers in a fixed order. The generator algorithm is fixed and
  implemented by the application itself (not a platform or runtime default whose output may change between versions).
- Frame N of a block MUST look identical regardless of the order in which frames are requested (scrubbing backwards,
  jumping, rendering a single frame, rendering a range) and across repeated renders, application restarts and saving
  and reloading the project.
- The simulation MUST be single-threaded per block, or use a reduction order independent of thread scheduling.
- Duplicating an effect or block copies its seed (identical particles); setting a new seed changes the particle layout
  without changing any other parameter.
- Rendering the same frame twice with the final-quality renderer at the same size MUST produce byte-identical pixels on
  the same machine and build (§6.7).

---

## 5. Effect preview

### 5.1 Fidelity levels

1. **Interactive views** (camera and other staging views): effects drawn in real time; blur and flatten may be
   approximated; clipping edges may be hard; the all-effects switch is honoured.
2. **Effect preview** (this section): final-quality renders of single frames or ranges.
3. **Rendered preview and export** (section 09): final-quality renders of the whole scene including all effects.

### 5.2 Effect preview panel

A dockable panel that renders frames with the final-quality renderer, identical to export output.

| Control | Values | Behaviour |
|---|---|---|
| Render (with range choice) | single frame (current), playback range, from current frame | Starts rendering the chosen range; the last choice is remembered (preference). While rendering, the control becomes **Stop** and progress (frame n of m) is shown. |
| Size | quarter, half, full (each labelled with the resulting pixel size of the scene's camera resolution) | Output resolution. Default **quarter**. Remembered (preference). |
| Clear cache (with usage %) | — | Empties the rendered-frame cache; the label shows how much of the cache budget is used |
| Follow current frame | on/off (default on) | When on, the panel shows the cached render of the current Timeline frame and follows the playhead |
| Image area | — | Shows the rendered frame, or an indication that no preview is available for the current frame |

Rules:

- **Playback range** uses the playback range of section 09; **from current frame** renders from the playhead to the end
  of the playback range (or of the scene if no range is set).
- **Cache budget:** 200 MB of rendered images by default (preference, 50–4000 MB) — roughly 25 full-HD frames, 100
  half-size or 400 quarter-size. When full, least-recently-used frames are evicted.
- **Invalidation:** any document change invalidates cached frames that depend on it. Required minimum: invalidate all
  frames of the scene on any edit that affects rendering; implementations MAY invalidate only frames within the changed
  element's span.
- Rendering is a background job; editing remains possible; edits made during a render invalidate affected frames, and
  the job renders remaining frames from the new state.
- Cached frames are kept per size; changing the size shows cached frames of that size only.

---

## 6. Render pipeline

### 6.1 Overview

```
Scene document (elements, pegs, cameras, bones, effects, cells)
        │ per frame
        ▼
Compositor — walks the element tree in Timeline order and produces a render list:
   for each drawable: world transform (camera, pegs, shake, bones, own transform),
   accumulated colour transform, clip scope, flatten/blur scope, particle batches
        │
        ▼
Renderer back ends
   ├── Interactive renderer (GPU-accelerated): staging and drawing views, playback
   ├── Hit-testing: identifies the object under a view position
   └── Final-quality renderer (deterministic): effect preview, rendered preview, export, agent render_frame
```

The interactive and final-quality renderers MUST consume the same render list so that they differ only in quality, not
in content or ordering. The render list for a frame is readable by agents (`fx.renderList.get`).

**Renderer operations** each back end MUST support: begin/end frame; draw vector drawing (fills, brush strokes,
centre-line strokes); draw bitmap image (or only its bounding box while loading); draw text; draw outline paths in a
colour; begin/end clip scope (with an *inverted* flag); begin/end flatten scope; begin/end blur scope (radial or
directional, radius, angle).

### 6.2 Composition rules

- **Visibility.** Hidden elements are skipped in all views, playback and export. Annotation elements are view-only
  (section 04). Elements attached under effect parameter elements are never drawn as ordinary content.
- **Order.** Children are drawn in Timeline stacking order (section 04). Drawing-view and exposure-sheet quick previews
  use exposure-sheet column order.
- **Transforms.** Drawable world transform = camera projection × peg chain × effect transforms (Shake) × bone
  deformation (section 05) × the drawing's own transform. Depth (Z) gives perspective scaling.
- **Effect scopes.** On reaching an effect within its active span, the compositor opens a scope; effects nest inside out
  (§2.4):

| Effect | Scope behaviour |
|---|---|
| Colour Transform | Pushes (mult, add) onto the colour-transform stack; children drawn with the combined transform. With *flatten*, children are drawn into a flatten scope and the transform applied once to the result |
| Clipping | Draws the mask sub-tree into the clip buffer, then draws ordinary children restricted to it (or outside it if inverted), then closes the clip scope |
| Drop Shadow | Shadow pass: children with the shadow transform appended and a constant-colour transform, flattened; then (unless *hide elements*) the normal pass. Parent colour transform applied per mode |
| Blur | Opens a blur scope: children render into an intermediate image which is blurred and composited |
| Shake | Pushes the shake translation onto the transform stack |
| Particle effects | Draws ordinary children, then for each active block: background (if any) and particles back to front, inside the effect's transform |
| Skeleton | Applies bone deformation to skin-group drawings (section 05) |

- **All effects off** (interactive views only): every scope except Skeleton is skipped; mask elements are drawn as
  ordinary drawings; particles are not generated.

### 6.3 Vector rendering

- Fills (closed contours) are tessellated for drawing; tessellations are cached per drawing and rebuilt lazily after
  edits.
- Brush strokes (variable-width outlines) are discretised from Bézier paths at a zoom-dependent tolerance so curves look
  smooth at every zoom level.
- Centre-line (pencil) strokes are drawn with their pen styles (section 02). The *fast centre-line rendering* option
  (§6.9) MAY switch to a faster method.
- **Colour references.** Geometry stores colour IDs (section 03); colours are resolved at draw time through the
  element's palettes, so editing a palette colour updates all drawings without re-tessellation.
- Fill kinds the renderers MUST support: solid colour; linear gradient; mirrored linear gradient; radial gradient;
  vector-drawing texture; bitmap texture; each optionally combined with an additional alpha (transparency) texture.
  Gradients and textures are anchored in drawing space through their own transform (section 03); textures tile where
  required.
- Deformed drawings (bones) are rendered through a deformation-aware path with a bounded deformation cache (section 05).

### 6.4 Bitmap rendering

- Bitmap images (image and media elements, imported pictures) are drawn as textured quads in scene space and cached.
- Images larger than the device's maximum texture size are split into tiles transparently.
- **Display resolution cap** (§6.9) limits the resolution at which bitmaps are held for interactive display; the
  setting shows the resulting maximum pixel size and estimated memory. Display only — final-quality renders always use
  full source resolution.
- **Bitmap memory budget** (§6.9): fraction of system memory allowed for bitmap caching; does not affect output quality.
- **Smooth textures and bitmaps** (§6.9): filtered (bilinear, with mipmaps for minification) versus nearest-neighbour
  sampling in the interactive views.
- **Compositing:** source-over with straight alpha; images imported with keying or transparency carry alpha (section
  09). Users only get normal alpha blending.

### 6.5 Colour-transform pipeline

Every draw carries a colour-transform context. Kinds the renderers MUST support:

| Kind | Used for |
|---|---|
| Identity | Normal drawing |
| Effect (multiply + add) | Colour Transform effect (§3.2) |
| Constant colour (keep alpha) | Drop Shadow silhouette, outlines |
| Wash (blend toward a colour by a factor) | Light-table shading, onion-skin shading (section 02) |
| Colour wash | Tinting toward the onion-skin previous/next colours (section 02) |
| Alpha wash (reduce opacity) | Onion-skin transparency, faded unselected elements |
| Highlight | Selection highlighting when box highlighting is off (section 05) |
| Identification colour | Hit-testing (§6.8) |

**Evaluation order** for a fragment: base colour (flat, gradient or texture) → `× mult + add` → wash
(`rgb × keep + washColour`) → `× fade` (overall fade factor) → clamp to [0,1]. The transform is skipped for fragments
with zero alpha. A CPU-side evaluation without the wash step MUST be available for export paths that need resolved
colours.

### 6.6 Clipping, flatten and blur per back end

| Feature | Interactive renderer | Final-quality renderer |
|---|---|---|
| Clipping | MAY use hard (binary) edges | Antialiased sub-pixel coverage masks; partial mask alpha gives partial visibility |
| Flatten | MAY approximate (children drawn with the combined transform) | Exact: children composited into a temporary image, transform applied once |
| Blur | MAY omit or approximate | Full radial / directional blur |
| Drop shadow | Two passes with constant-colour transform | Same, antialiased |
| Particles | Sprites drawn live | Same simulation, antialiased |

### 6.7 Final-quality renderer

- Produces final-quality frames for the effect preview, rendered previews, all bitmap and movie exports, and agent
  `render_frame` with quality `final`.
- **Layers.** A frame is rendered as a tree of layers: drawing layers, clip layers, flatten layers, blur layers and
  containers.
- **Antialiasing.** Fills and strokes are always antialiased with sub-pixel coverage (at least 16 coverage samples per
  pixel, or an exact-area equivalent). Overlapping semi-transparent fragments composite correctly, and adjacent fills of
  the same drawing show no seams.
- **Shading.** Solid, gradient (with colour stops) and texture/bitmap fills, each with multiplicative and additive colour
  transforms.
- **Texture sampling modes:** nearest, bilinear, pre-filtered (mipmapped) for minification, and box-filtered. *High
  quality textures* (§6.9) selects filtered sampling; off selects faster bilinear/nearest sampling.
- **Texture cache** with a memory budget (*texture cache size*, §6.9) and least-recently-used purge.
- **Output:** 32-bit RGBA images with the scene background colour (or transparent where the export format supports
  alpha, section 09); composition of panning (larger-than-frame) background images is supported.
- **Determinism:** identical inputs (document state, frame, size, options) MUST produce byte-identical pixels on the
  same machine and build. Implementations SHOULD produce identical output across machines; a GPU path may be used for
  final-quality rendering only if it meets this requirement.

### 6.8 Hit-testing

- Clicking in a view identifies the top-most object under the pointer by rendering (or scan-converting) the scene's
  pickable objects in unique identification colours, without blending or antialiasing, and reading the pixels within a
  small radius of the pointer (default 3 view pixels); the top-most identified object wins.
- Pickable objects: elements, drawing objects, strokes, motion points, handles, bones, peg paths and effect handles
  (e.g. the drop-shadow manipulator). Particles are not individually pickable; clicking a particle picks the particle
  effect.
- Fully transparent bitmap pixels are not pickable.
- Hit-test results MUST match the visible geometry.
- Selection display (box highlight or outline/fade) is specified in section 05.

### 6.9 Display-quality and renderer options

All are application preferences (persisted per section 01), not document data, and are readable and settable through
`fx.renderer.options.get` / `fx.renderer.options.set`.

| Option (`name`) | Type | Default | Effect |
|---|---|---|---|
| Interactive antialiasing (`interactiveAntialiasing`) | bool | false | Multisample antialiasing of everything drawn in the interactive views. Without it, interactive fills may be aliased. |
| Smooth tools (`antialiasToolOverlays`) | bool | true | Antialiased lines for tool handles and manipulators |
| Smooth motion paths (`smoothMotionPaths`) | bool | true | Antialiased motion paths in staging views |
| Smooth textures and bitmaps (`filterTextures`) | bool | true | Filtered sampling of bitmaps and texture fills in interactive views |
| Fast centre-line rendering (`fastCentrelineStrokes`) | bool | false | Faster method for centre-line strokes when they render slowly |
| Overlay buffering (`overlayBuffering`) | bool | true | Buffer the static scene so drawing-tool feedback redraws only an overlay; turn off if drawing tools respond slowly on a device |
| Display resolution cap (`displayBitmapMaxSize`) | int pixels, longest side | 2048 (choices 512, 1024, 2048, 4096, 8192, presented from faster display to better quality with estimated memory) | Maximum resolution of bitmaps held for interactive display |
| Bitmap memory budget (`bitmapMemoryPercent`) | int % of system RAM | 25 (5–75) | RAM allowed for bitmap caching |
| High quality textures (`finalHighQualityTextures`) | bool | true | Filtered texture sampling in the final-quality renderer |
| Texture cache size (`finalTextureCacheMB`) | int MB | the recommended value | Final-quality renderer texture cache budget. The application computes a **recommended maximum** from system RAM and shows it, with a hint to use a lower value if other applications are affected. |
| Effect preview cache (`previewCacheMB`) | int MB | 200 (50–4000) | §5.2 |

Options that the implementation can only apply after a restart report `restartRequired: true` when set, and the GUI
tells the user. The application MUST work when GPU acceleration is unavailable, falling back to a software path for the
interactive views with reduced performance.

### 6.10 Interactive vs final-quality renderer

| Aspect | Interactive (GPU-accelerated) | Final-quality |
|---|---|---|
| Purpose | Real-time editing and playback | Effect preview, rendered previews, export, agent final renders |
| Antialiasing | Off unless interactive antialiasing is on; tool lines optionally smoothed | Always, sub-pixel coverage |
| Blur | May be omitted/approximated | Full |
| Clipping edges | May be hard | Antialiased |
| Flatten | May be approximate | Exact |
| Bitmap resolution | Capped by display resolution cap | Full source resolution |
| Texture filtering | *Smooth textures and bitmaps* | *High quality textures* |
| Particles | Live | Same simulation, antialiased |
| All-effects switch | Honoured | Ignored (effects always rendered) |
| Overlays (grids, handles, onion skin, light table, guides) | Drawn | Never drawn |
| Determinism | Not required | Required (§6.7) |

### 6.11 Effects in SWF export

Summary; the export itself is specified in section 09.

| Effect | SWF mapping |
|---|---|
| Colour Transform | SWF colour transform on placed objects (flatten approximated per object) |
| Clipping | SWF clip layer covering the masked objects (inverted clipping is rasterised, with a warning) |
| Drop Shadow | Additional placed instances with the shadow matrix and a constant-colour transform |
| Blur | Native SWF blur filter; motion mode as a directional blur |
| Shake | Baked into per-frame object matrices |
| Particle effects | Per-frame particle instances placed as drawing or bitmap symbols |
| Skeleton | Deformed vectors per frame; deformed bitmaps not supported (warning) |

---

## 7. Persistence summary

Defined normatively in section 10; the effect-specific stored data is:

| Effect | Stored fields |
|---|---|
| Common | element record (section 04); `type`; parameter-element references; loop count |
| Colour Transform | 8 function curves (mult R,G,B,A; add R,G,B,A) with keyframes; `flatten` |
| Clipping | `inverted` |
| Drop Shadow | `colour`, `opacity`, `hideElements`, `parentColourTransformMode`, `transform` (2×3 matrix) |
| Blur | `radial`; function curves `radius`, `angle` |
| Shake | `axis`, `waveTime`, `decayHalfLifeFrames`; function curve `amplitude` |
| Particle effects | ordered blocks, each with `type`, `variant`, `presetId` (last applied, informational), `seed`, `spriteSetId`, `background`, `simulationStart`, `firstFrame`, `duration`, `maxParticles`, `lifetimeFrames`, `fade`, `areaWidth`, `areaHeight`, `areaDepth`, `floorLevel`, and function curves for the dynamic parameters |
| User presets | Stored outside projects in the user's preset library (preset data model, §3.7) |

Effects (with their parameter elements and attached elements) can be copied, dragged, stored in templates and in the
library (section 08). Simulation caches and the preview cache are never persisted.

---

## 8. Agent interface

### 8.1 Agent-specific requirements

1. **Everything through commands.** Every effect capability in this section — creating effects, every parameter of every
   effect type, keyframes, timing and loops, particle blocks, particle types and variants, seeds, sprite sets and custom
   sprites, presets, parameter-row display, the all-effects switch, the effect preview, renderer options and all
   read-backs — MUST be reachable through the `fx` commands below (plus the generic element commands of section 04 and
   the function-curve commands of section 05). No properties panel, preset button, drag gesture or preview window is
   the only path.
2. **Full parameter objects.** `fx.create` accepts a complete `params` object for the effect type (and, for particle
   effects, a `blocks` array with full block specifications including type, variant, seed, static values and dynamic
   values as constants or keyframe lists), so an agent can create a fully configured effect in one undoable step.
   `fx.params.set` accepts any subset. Unknown parameter names fail with `fx.unknownParameter`; out-of-range values fail
   with `fx.valueBelowMinimum` / `fx.valueAboveMaximum` (with `{param, min, max}` details) unless `clamp: true`, in
   which case they are clamped and listed in `clamped[]`.
3. **Discoverable schemas.** `fx.types.list` returns, for every effect type, a JSON Schema of its parameters (type,
   unit, minimum, maximum, default, enum values, `animatable` flag), its parameter-element slots, its flags (§2.2) and,
   for particle effects, every particle type with its variants, per-type defaults and caps, and its compatible sprite
   sets. The schemas returned are exactly the ones `fx.create` / `fx.params.set` / `fx.block.set` validate against.
4. **Gestures as structured commands.** The drop-shadow manipulator maps to `fx.dropShadow.transform.set` (matrix, or
   offset/scale/skew); dragging an effect bar or its ends maps to `fx.timing.set`; dragging elements onto a mask or
   sprites row maps to `fx.paramElement.add`; preset buttons map to `fx.preset.apply`; the type buttons map to
   `fx.block.type.set`.
5. **Keyframe semantics are explicit.** `fx.params.set` with `frame` follows the auto-key rule of §2.10
   (`keyMode: "auto"`) and reports `keyframeCreated`. `keyMode: "static"` removes all keyframes of the effect (or block) and sets
   constant values; because it discards animation it returns `needs_confirmation` unless `confirm: true`. Shortening a
   duration with `scaleKeys: false` that would delete keyframes behaves likewise.
6. **No blocking prompts.** Name conflicts, destructive operations and preset overwrites use `confirm` or `onConflict`
   parameters or return a structured `needs_confirmation` result. The drawing-mode visibility notice is returned as a
   warning (`fx.notVisibleInDrawingMode`), never as a modal.
7. **Deterministic particles.** Given the same document state (including each block's seed), `fx.particles.get` returns
   identical JSON and `render_frame` with `quality: "final"` returns byte-identical PNGs, regardless of request order,
   caching, restarts or save/reload (§4.6, §6.7). Agents can set seeds explicitly (`fx.block.seed.set`) to reproduce or
   vary a simulation.
8. **Preview rendering.** Agents preview effects with section 11's `render_frame`: `quality: "final"` uses the
   final-quality renderer (all effects, full blur, exact flatten, antialiasing; ignores the all-effects switch);
   `quality: "draft"` reproduces the interactive view (honours the all-effects switch). The effect preview panel
   is also drivable (`fx.preview.render` job, `fx.preview.frame.get` returns cached frames as PNG) and produces the same
   pixels as `render_frame` final at the same size.
9. **Read-back for verification.** `fx.get`, `fx.params.get` (resolved values at a frame, plus derived values such as
   the combined colour transform, shake displacement and live particle count), `fx.keyframe.list`, `fx.block.list`,
   `fx.particles.get`, `fx.shake.displacement.get` and `fx.renderList.get` (the compositor's ordered render list for a
   frame) let an agent verify ordering, scopes and values without reading pixels.
10. **Long operations as jobs.** Preview rendering of ranges and pre-simulation of long particle blocks
    (`fx.simulation.run`) are jobs with progress via `job_status` and cancellation via `job_cancel`.
11. **Errors** use `fx.*` codes (§8.5); element naming errors use the `element.*` codes of section 04.

### 8.2 Command table

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `fx.types.list` | query | List all effect types with parameter JSON schemas, slots, flags, particle types, variants, defaults, caps and sprite sets | `effectType?: enum "skeleton"\|"colourTransform"\|"clipping"\|"dropShadow"\|"blur"\|"shake"\|"natureParticles"\|"actionParticles"; includeDefaults?: bool = true` | `{types: [TypeDescriptor]}` | n/a |
| `fx.types.get` | query | Descriptor of one effect type, optionally narrowed to one particle type | `effectType: enum (as above); particleType?: string` | `TypeDescriptor` | n/a |
| `fx.create` | action | Create one or more effects with full parameters ("Create Effect") | `type: enum (effect type); name?: string (1–20 chars); count?: int = 1 (1–99); sceneId?: id; parentId?: id; index?: int; childIds?: [id]; startFrame?: int; duration?: int; loops?: int = 1 (1–999); params?: object; presetId?: id; blocks?: [BlockSpec]` | `{effectIds: [id], paramElementIds: {slot: id}, warnings: [code]}` | yes |
| `fx.list` | query | List effects in a scene | `sceneId?: id; type?: enum (effect type); underElementId?: id` | `{effects: [{id, name, type, parentId, startFrame, duration, loops, animated, paramElementIds}]}` | n/a |
| `fx.get` | query | Full effect record (static values, keyframes, blocks, slots, timing) | `effectId: id; includeCurves?: bool = false` | `Effect` | n/a |
| `fx.params.get` | query | Resolved parameter values at a frame plus derived values | `effectId: id; frame?: int = current; blockId?: id` | `{values: object, isKeyframe: bool, derived: object}` | n/a |
| `fx.params.set` | action | Set any subset of an effect's (or block's) parameters ("Change Effect Parameters") | `effectId: id; values: object; frame?: int = current; blockId?: id; keyMode?: enum "auto"\|"static" = "auto"; clamp?: bool = false; confirm?: bool = false` | `{values: object, keyframeCreated: bool, clamped: [string]}` or `needs_confirmation` | yes |
| `fx.timing.set` | action | Change start frame, duration and/or loop count ("Change Effect Timing") | `effectId: id; startFrame?: int; duration?: int (≥1); loops?: int (1–999); scaleKeys?: bool = true; confirm?: bool = false` | `{startFrame, duration, loops, removedKeyframes: [int]}` or `needs_confirmation` | yes |
| `fx.paramElement.list` | query | List an effect's parameter elements and the elements attached to each | `effectId: id` | `{slots: [{slot, elementId, attachedIds: [id]}]}` | n/a |
| `fx.paramElement.add` | action | Attach elements to a parameter slot (mask, sprites, bones, skin) | `effectId: id; slot: enum "mask"\|"sprites"\|"bones"\|"skin"; elementIds: [id]; index?: int` | `{attachedIds: [id]}` | yes |
| `fx.paramElement.remove` | action | Detach elements from a parameter slot | `effectId: id; slot: enum "mask"\|"sprites"\|"bones"\|"skin"; elementIds: [id]; newParentId?: id` | `{detachedIds: [id]}` | yes |
| `fx.paramRows.set` | ui | Show or hide parameter rows in the Timeline for given effects or all effects | `scope: enum "effects"\|"all" = "effects"; effectIds?: [id] (default: selection); visible: bool` | `{changedEffectIds: [id]}` | no |
| `fx.paramRows.get` | query | Parameter-row visibility per effect | `sceneId?: id` | `{effects: [{effectId, visible}]}` | n/a |
| `fx.allEffects.set` | toggle | Enable or disable effect display in interactive views | `enabled: bool` | `{enabled}` | no |
| `fx.allEffects.get` | query | Current all-effects display state | — | `{enabled}` | n/a |
| `fx.keyframe.add` | action | Add an effect (or block) keyframe, optionally with values ("Add Effect Keyframe") | `effectId: id; frame?: int = current; blockId?: id; values?: object` | `{frame, created: bool}` | yes |
| `fx.keyframe.remove` | action | Remove a keyframe ("Remove Effect Keyframe") | `effectId: id; frame: int; blockId?: id` | `{removed: bool, nowStatic: bool}` | yes |
| `fx.keyframe.list` | query | List keyframes with their values | `effectId: id; blockId?: id` | `{keyframes: [{frame, values}]}` | n/a |
| `fx.curve.get` | query | Function-curve ids of animatable parameters (for editing with section 05 commands) | `effectId: id; blockId?: id; param?: string` | `{curves: [{param, functionId}]}` | n/a |
| `fx.dropShadow.transform.set` | action | Set or reset the shadow transform ("Change Drop Shadow") | `effectId: id; matrix?: {xAxis: point, yAxis: point, origin: point}; offset?: point; scale?: point; skew?: number (−89–89); reset?: bool = false` | `{transform}` | yes |
| `fx.shake.displacement.get` | query | Shake displacement samples | `effectId: id; startFrame?: int; endFrame?: int` | `{samples: [{frame, dx, dy, dz}]}` (fields) | n/a |
| `fx.block.list` | query | List a particle effect's blocks | `effectId: id` | `{blocks: [BlockSummary]}` | n/a |
| `fx.block.get` | query | Full block record (static values, dynamic curves/keyframes, seed, sprite set) | `blockId: id` or `effectId: id; frame: int` | `Block` | n/a |
| `fx.block.add` | action | Add a block at a frame (split or append) ("Add Particle Block") | `effectId: id; frame?: int = current; type?: string; variant?: string; presetId?: id; seed?: int` | `{blockId, startFrame, duration}` | yes |
| `fx.block.remove` | action | Remove a block; frames merge into a neighbour ("Remove Particle Block") | `blockId?: id; effectId?: id; frame?: int` | `{mergedIntoBlockId}` | yes |
| `fx.block.set` | action | Set block static values (timing, background, region, floor, caps, lifetime, fade, sprite set) ("Change Particle Settings") | `blockId: id; values: object; clamp?: bool = false` | `{block: Block, clamped: [string]}` | yes |
| `fx.block.type.set` | action | Change a block's particle type and variant, loading that type's defaults ("Change Particle Type") | `blockId: id; type: string; variant?: string` | `{block: Block}` | yes |
| `fx.block.seed.set` | action | Set a block's seed, or draw a new random one | `blockId: id; seed?: int (0–4294967295)` | `{seed}` | yes |
| `fx.block.defaults.apply` | action | Reset a block's parameters to its type's defaults ("Reset to Defaults") | `blockId: id; keepTiming?: bool = true; keepSeed?: bool = true` | `{block: Block}` | yes |
| `fx.preset.list` | query | List presets | `effectType?: enum (effect type); particleType?: string; source?: enum "builtin"\|"user"\|"all" = "all"; tags?: [string]` | `{presets: [{id, name, source, effectType, particleType, variant, tags}]}` | n/a |
| `fx.preset.get` | query | Full preset body, optionally with thumbnail PNG | `presetId: id; includeThumbnail?: bool = false` | `Preset` | n/a |
| `fx.preset.apply` | action | Apply a preset to an effect or particle block ("Apply Preset") | `presetId: id; effectId: id; blockId?: id; frame?: int` | `{effectId, blockId?, values: object}` | yes |
| `fx.preset.create` | action | Save an effect's or block's current parameters (or an explicit body) as a user preset | `name: string (1–60); effectId?: id; blockId?: id; body?: Preset; tags?: [string]; includeSeed?: bool = false; onConflict?: enum "fail"\|"replace"\|"rename" = "fail"` | `{presetId}` or `needs_confirmation` | no |
| `fx.preset.rename` | action | Rename a user preset | `presetId: id; name: string; onConflict?: enum "fail"\|"rename" = "fail"` | `{name}` | no |
| `fx.preset.delete` | action | Delete a user preset | `presetId: id; confirm?: bool = false` | `{deleted: bool}` or `needs_confirmation` | no |
| `fx.preset.import` | action | Import user presets from a preset file | `path: path; onConflict?: enum "fail"\|"replace"\|"rename" = "rename"` | `{presetIds: [id], skipped: [string]}` | no |
| `fx.preset.export` | action | Export user (or built-in) presets to a preset file | `presetIds: [id]; path: path; overwrite?: bool = false` | `{path}` | no |
| `fx.spriteSet.list` | query | List built-in sprite sets | `particleType?: string` | `{spriteSets: [{id, name, particleTypes, variantCount}]}` | n/a |
| `fx.spriteSet.get` | query | Sprite-set details and variant thumbnails | `spriteSetId: id; includeThumbnails?: bool = false` | `{id, name, variants: [{index, thumbnail?}]}` | n/a |
| `fx.particles.get` | query | Deterministic particle snapshot of a block at a frame | `effectId: id; frame: int; blockId?: id; space?: enum "effect"\|"scene" = "effect"; limit?: int = 3000 (1–3000); fields?: [string]` | `{blockId, seed, frame, count, truncated: bool, particles: [{index, pos: {x,y,z}, vel: {x,y,z}, size, rotation, alpha, age, variant}]}` | n/a |
| `fx.simulation.run` | job | Pre-simulate a block up to a frame (warms the cache) | `effectId: id; blockId?: id; toFrame: int` | `{jobId}` → `{framesSimulated, peakCount}` | n/a |
| `fx.renderList.get` | query | Compositor output for a frame: ordered drawables with transforms, colour transforms and effect scopes | `sceneId?: id; frame: int; quality?: enum "draft"\|"final" = "final"; includeParticles?: bool = false` | `{items: [RenderItem]}` | n/a |
| `fx.hitTest.run` | query | Identify pickable objects under a view position | `viewId: id; x: number; y: number (view pixels); radius?: int = 3 (0–20)` | `{hits: [{objectId, kind, elementId}]}` (top-most first) | n/a |
| `fx.preview.open` | ui | Open (or focus) the effect preview panel | — | `{panelId}` | no |
| `fx.preview.close` | ui | Close the effect preview panel | — | `{}` | no |
| `fx.preview.options.get` | query | Effect preview settings | — | `{size, followCurrentFrame, defaultRange}` | n/a |
| `fx.preview.options.set` | ui | Change effect preview settings | `size?: enum "quarter"\|"half"\|"full"; followCurrentFrame?: bool; defaultRange?: enum "single"\|"playbackRange"\|"fromCurrent"` | `{size, followCurrentFrame, defaultRange}` | no |
| `fx.preview.render` | job | Render frames into the effect preview cache | `range?: enum "single"\|"playbackRange"\|"fromCurrent" = defaultRange; startFrame?: int; endFrame?: int; size?: enum "quarter"\|"half"\|"full"` | `{jobId}` → `{framesRendered}` | n/a |
| `fx.preview.stop` | ui | Stop the running preview render (same as `job_cancel`) | `jobId?: id` | `{stopped: bool}` | no |
| `fx.preview.cache.delete` | ui | Clear the rendered-frame cache | — | `{freedMB}` | no |
| `fx.preview.status.get` | query | Rendering state and cache usage | — | `{rendering, jobId?, cacheUsedMB, cacheBudgetMB, usedPercent, cachedFrames: [{frame, size}]}` | n/a |
| `fx.preview.frame.get` | query | Cached preview render of a frame as PNG | `frame?: int = current; size?: enum "quarter"\|"half"\|"full"` | `{cached: bool, width, height, png?}` | n/a |
| `fx.renderer.options.get` | query | Display-quality and renderer options (§6.9) | — | `{values: object}` | n/a |
| `fx.renderer.options.set` | ui | Change display-quality and renderer options | `values: object` | `{values: object, restartRequired: bool}` | no |
| `fx.renderer.capabilities.get` | query | Renderer capabilities and computed recommendations | — | `{gpuAccelerated, maxTextureSize, interactiveAntialiasingSupported, systemMemoryMB, recommendedTextureCacheMB}` | n/a |
| `fx.changed` | event | An effect's parameters, keyframes, timing or slots changed | — | `{effectId, blockId?, what: enum "params"\|"keyframes"\|"timing"\|"slots"\|"blocks"}` | n/a |
| `fx.allEffects.changed` | event | All-effects display switch changed | — | `{enabled}` | n/a |
| `fx.preset.changed` | event | User preset library changed | — | `{presetId, change: enum "created"\|"renamed"\|"deleted"\|"imported"}` | n/a |
| `fx.preview.frameRendered` | event | A preview frame finished rendering | — | `{frame, size, jobId}` | n/a |
| `fx.preview.invalidated` | event | Preview cache frames invalidated by an edit | — | `{frames: [int] \| "all"}` | n/a |
| `fx.renderer.options.changed` | event | Renderer options changed | — | `{values, restartRequired}` | n/a |

Effect deletion, renaming, duplication/cloning, parenting, selection, collapse and visibility use the generic element
commands of section 04; skeleton-specific properties and bone editing use the `anim` commands of section 05.

### 8.3 Queries and events

- `fx.get` returns the complete effect as JSON: element fields, `type`, static values, animatable parameters (constant
  value or keyframes `[{frame, value, interpolation}]`), parameter-element slots with attached element ids, and for
  particle effects the ordered `blocks[]`.
- `fx.params.get` `derived` contents per type: Colour Transform → combined `{mult[4], add[4]}` including ancestors;
  Drop Shadow → resolved shadow matrix in scene space; Blur → radius in pixels at the requested render size; Shake →
  `{dx, dy, dz}`; particles → `{blockId, liveCount}`.
- `fx.renderList.get` items: `{order, elementId, drawingId?, kind: "vector"|"bitmap"|"text"|"particles"|"scopeBegin"|"scopeEnd", scope?: "clip"|"flatten"|"blur"|"shadowPass", effectId?, worldMatrix, colourTransform: {mult[4], add[4]}}`.
- Events are delivered via section 11 resource subscriptions (`app://events`). Effect creation and deletion are
  reported by the element events of section 04.

### 8.4 JSON shapes

```json
TypeDescriptor = {
  "type": "shake", "displayName": "Shake",
  "flags": {"compositing": false, "transform": true, "loopable": true, "sequenced": false, "animatable": true},
  "slots": [],
  "paramsSchema": { "type": "object", "properties": {
      "axis":      {"enum": ["x","y","xy","z","xyz"], "default": "xy", "animatable": false},
      "waveTime":  {"type": "integer", "minimum": 1, "maximum": 999, "default": 4, "unit": "frames", "animatable": false},
      "decayHalfLifeFrames":  {"type": "number", "minimum": 0.1, "maximum": 999, "default": 6, "unit": "frames", "animatable": false},
      "amplitude": {"type": "number", "minimum": 0, "maximum": 100, "default": 0.5, "unit": "fields", "animatable": true}}},
  "particleTypes": null
}

Block = {
  "id": "blk_7", "index": 0, "type": "snow", "variant": null, "presetId": "builtin.snow.normal",
  "seed": 184467, "spriteSetId": "builtin.sprites.snow.flakes",
  "static": {"background": "transparent", "simulationStart": -20, "firstFrame": 1, "duration": 96,
             "maxParticles": 1500, "lifetimeFrames": -1, "fade": true, "areaWidth": 1400, "areaHeight": 1000,
             "areaDepth": 600, "floorLevel": 0},
  "dynamic": {"intensity": {"keyframes": [{"frame": 1, "value": 20}, {"frame": 48, "value": 60}]},
              "scale": {"value": 100}, "bounciness": {"value": 0}, "speedMin": {"value": 2},
              "speedMax": {"value": 5}, "directionEW": {"value": 150}, "directionNS": {"value": -1000},
              "directionFB": {"value": 0}}
}
```

`BlockSpec` (used by `fx.create`) has the same shape as `Block` without `id`/`index`, with every field optional.
`Preset` follows §3.7. Frames inside blocks and presets are absolute scene frames in `Block` and relative (1 = first
frame) in `Preset`.

### 8.5 Error codes

| Code | Meaning |
|---|---|
| `fx.notFound` | Effect, block or keyframe id does not exist |
| `fx.wrongType` | Command not applicable to this effect type (e.g. block commands on a blur) |
| `fx.unknownParameter` | Parameter name not in the type's schema |
| `fx.valueBelowMinimum` / `fx.valueAboveMaximum` | Value out of range (details: param, min, max) |
| `fx.notAnimatable` | Keyframe values given for a static parameter |
| `fx.frameOutsideSpan` | Frame outside the effect's or block's span |
| `fx.blockStartExists` | A block already starts at that frame |
| `fx.lastBlock` | Cannot remove the only block |
| `fx.particleTypeInvalid` | Particle type not offered by this effect type |
| `fx.variantInvalid` | Variant not offered by the particle type |
| `fx.spriteSetIncompatible` | Sprite set not valid for the particle type |
| `fx.presetNotFound` / `fx.presetTypeMismatch` / `fx.presetReadOnly` / `fx.presetNameExists` | Preset errors |
| `fx.presetFileInvalid` | Imported preset file cannot be read or fails validation |
| `fx.slotNotFound` | Effect has no such parameter slot |
| `fx.invalidAttachment` | Element kind cannot be attached to that slot (e.g. a sound), or attaching would create a cycle |
| `fx.noParameterElements` | Parameter-row command on effects without parameter elements |
| `fx.notAllowedInTemplateEdit` | Effect creation/modification while editing a template in place |
| `fx.notVisibleInDrawingMode` | Warning: created effects are not visible in drawing mode |
| `fx.previewSizeUnavailable` | Requested preview size exceeds memory limits |
| `fx.optionRequiresRestart` | Warning: option takes effect after restart |

---

## 9. Design decisions / open questions

1. **Colour Transform multiplicative range.** Decision: 0.000–1.000 (1.0 = identity). Optionally allow up to 10.0 for
   over-brightening in a later version; if adopted, the schema range changes and SWF export clamps.
2. **Shake axis set and defaults.** Decision: axes `x`, `y`, `xy`, `z`, `xyz`, default `xy`; wave time 4, half-life 6,
   amplitude 0.5 field; per-axis phases 0, π/2, π/4. Preset values (small/medium/large) to be authored and tuned
   visually.
3. **Drop Shadow defaults.** Decision: black, 50 % opacity, offset (0.2, −0.2) field, drawings-only parent colour
   transform mode.
4. **Blur defaults.** Decision: radial on, radius 4 output pixels, angle 0. Alternative: radius 0 (no visible change on
   creation) — rejected because users need immediate feedback.
5. **Particle unit mapping.** Decision: 100 particle units = 1 field; direction components −1000…+1000. Per-type
   default regions must cover the default camera frame (≥ 1400 × 1100 units) with margin.
6. **All particle preset values** (intensity, speeds, regions, lifetimes, sprite sets) are authored fresh to match each
   preset's described look; built-in preset ids are stable once released.
7. **All-effects switch scope.** Decision: affects every effect except Skeleton in interactive views only; never affects
   final-quality rendering or export.
8. **Clipping with an empty mask.** Decision: children invisible (non-inverted), fully visible (inverted).
9. **Inverted clipping.** Decision: provided (`inverted`, default false); SWF export rasterises inverted clips with a
   warning.
10. **Default effect timing.** Decision: frame 1 to scene end, or the span of the given children. Alternative: start at
    the current frame — consider as a preference.
11. **Duration change and keyframes.** Decision: dragging the end scales keyframes; numeric changes scale by default
    (`scaleKeys: true`).
12. **Particle looping.** Decision: particle effects are not loopable; repetition is achieved with additional blocks or
    presets with long durations.
13. **Cache invalidation granularity** (preview and simulation). Decision: whole-scene invalidation on any rendering
    edit is acceptable for the first version; per-span invalidation is an optimisation.
14. **Cross-machine determinism.** Required on the same machine and build; SHOULD hold across machines. Decision: the
    final-quality renderer and particle engine avoid platform-dependent maths (use the application's own PRNG and a
    fixed operation order; avoid fused or reordered floating-point operations where results could differ).
15. **Custom sprite override scope.** Decision: custom sprites apply to all blocks of the effect. Alternative:
    per-block sprite slots — not required.
16. **Preset file format and location** for user presets: JSON per §3.7; file naming and storage location to be fixed
    in section 10.
17. **Built-in non-particle presets** (Colour Transform, Blur, Drop Shadow): SHOULD ship (§3.7); content to be authored.
