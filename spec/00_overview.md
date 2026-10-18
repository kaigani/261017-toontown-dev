# 00 — Overview: 2-D Animation Application, Functional Requirements

**Purpose.** This document set specifies the functional requirements of a desktop 2-D animation application: what it
does, how it behaves, what data it manages and how every capability can be driven by an AI agent. It is written
as a requirements specification for an implementation team that designs the user interface, architecture, file
format and code independently.
**Status:** version 2 (requirements-only, agent-controllable). Date: 2026-10-17.
**Working name:** "the application". The product name, file extensions (`.animpkg`, `.animpal`, `.animtpl`) and
visual design are placeholders to be decided by the implementation team.

---

## 1. How to read this specification

### 1.1 Document set

| # | File | Scope |
|---|---|---|
| 00 | `00_overview.md` | This file: scope, conventions, glossary, architecture, feature inventory, cross-cutting rules, key design decisions |
| 01 | `01_application_shell.md` | Application life-cycle, start screen, projects (new/open/save/autosave/backups), project properties, drawing vs staging modes, main window, panels, docking and workspaces, toolbars, themes, global commands, preferences, key-binding editor, status/help/tips, messages, undo/redo, clipboard, persisted settings |
| 02 | `02_drawing.md` | Vector drawing model (brush contours, pencil lines, invisible strokes, zones, layers, groups, text), drawing view, every drawing tool and option, pens and pressure, drawing commands, drawing aids (grids, onion skin, light tables), import and vectorize |
| 03 | `03_colour.md` | Palettes, styles, swatches (solid, gradients, textures), colour IDs, colour editing, tinting, painting tools, gap closing, default palette, palette exchange |
| 04 | `04_timeline_xsheet.md` | Element types and properties, element commands, Timeline, Exposure Sheet, cells and exposures, cycles, drawing management, static light table |
| 05 | `05_scene_planning.md` | 3-D staging space, Camera/Top/Side views, animation tools, pegs and hierarchy, motion paths, keyframes, velocity and interpolation, function curves, cameras, bones and skeletons, motion recording; normative transform maths |
| 06 | `06_effects.md` | Effect architecture, every effect type and its parameters, particle effects and engine, presets, effect preview, render pipeline |
| 07 | `07_sound_lipsync.md` | Sound elements and editing, playback and scrubbing, automatic lip-sync (detection, mapping, apply), media (video) elements |
| 08 | `08_library_storyline.md` | Template library (user-wide and per project), templates, reuse and editing; storyline of scenes and transitions |
| 09 | `09_playback_export.md` | Interactive playback, previews, rendering, export (video, image sequences, vector animation, PDF, print, publishing), all import paths, stop-motion capture |
| 10 | `10_data_model_project_format.md` | Conceptual data model, identifiers, limits, the open project package format (zip + JSON), palette/template exchange files, load/save behaviour, the JSON object model shared with the agent interface |
| 11 | `11_agent_interface_mcp.md` | Full agent control through MCP: command layer, run modes, transports, addressing and coordinate spaces, permissions and confirmation, tools, resources, events, jobs, errors, audit log, prompts, conformance tests |
| A | `A_command_registry.md` | Index of all 796 commands and events (compiled from the "Agent interface" section of 01–11): id, kind, parameters, result, undo |

Sections 01–11 are normative. Appendix A is a compiled index; where it disagrees with a section, the section wins.

### 1.2 Requirement language

- **MUST / SHOULD / MAY** follow RFC 2119. Plain declarative statements ("the tool closes the gap") are requirements too.
- **Design decision:** marks a point where the specification recommends a choice but the implementation team may choose
  differently, provided the choice is documented and applied consistently.
- Each section ends with **Design decisions / open questions** listing choices still to make, with recommended defaults.
- Labels in this specification (command names, panel names) are descriptive. Final UI wording, menu organisation,
  dialog layout, iconography and default key bindings are UX design work for the implementation team, subject to the
  behavioural requirements here.

### 1.3 Platform conventions

Platform-standard shortcuts (new, open, save, close, quit, undo, redo, cut, copy, paste, select all) follow the host OS.
All other commands MUST be bindable to keys by the user; the default key map is a design decision (01). Modifier keys
are written generically: *primary* (Command on macOS, Ctrl elsewhere), *alt* (Option/Alt), *shift*.

---

## 2. Product summary

The application is a single-user desktop tool for 2-D animation by hobbyists, schools and small studios, and a
production engine that AI agents can drive end to end. It combines:

1. **Vector drawing and painting** — pressure-sensitive brush, pencil and shape tools producing resolution-independent
   artwork made of strokes and fillable zones; colours referenced by ID from switchable palettes (02, 03).
2. **Frame-by-frame animation** — drawings exposed over frames in an **Exposure Sheet** and a **Timeline**, with onion
   skin and light tables (02, 04).
3. **Cut-out / puppet animation** — drawings attached to **pegs** in a hierarchy and animated with keyframed
   transformations, motion paths and velocity curves; **bones/skeletons** with deformation zones; **motion recording**
   of hand-performed motion (05).
4. **Staging in 3-D space** — elements placed along horizontal, vertical and depth axes, viewed through animated
   **cameras** in Camera, Top and Side views, giving multiplane parallax (05).
5. **Effects** — colour transform, clipping masks, drop shadow, blur, shake, and particle effects for natural
   phenomena and action/impact effects (06).
6. **Sound and automatic lip-sync** — sound tracks with trimming, fades and volume; automatic mouth-shape detection
   mapped to mouth drawings (07).
7. **Reuse** — a library of templates (user-wide and per project) that can be inserted into scenes (08).
8. **Storyline** — a movie made of several scenes joined by transitions (08).
9. **Stop-motion** — capture from cameras with onion skin and colour keying (09).
10. **Output** — video files, image sequences, vector animation, snapshots, PDF, print and publishing (09).
11. **Agent control** — every capability above is available to an AI agent through an MCP server, with structured
    commands, read-back of the full document, deterministic renders, permissions and an audit trail (11).

---

## 3. Glossary

| Term | Meaning |
|---|---|
| **Project** | A document (project package, 10) holding scenes, drawings, palettes, sounds and the project-local library |
| **Project properties** | Project-wide frame rate and camera (output) size (01) |
| **Scene** | A self-contained segment of the movie with its own elements, length and cameras; scenes are ordered in the **storyline** and joined by **transitions** (08) |
| **Element** | A layer-like object in a scene. Types include drawing, image, sound, media, annotation, peg, parent peg, camera, skeleton, bone, effect and effect-parameter elements (04) |
| **Drawing** | One vector (or bitmap) picture belonging to a drawing element |
| **Cell / exposure** | A frame slot of an element; exposing a drawing for N frames = N cells referencing it (04) |
| **Exposure Sheet** (X-sheet, dope sheet) | Grid view: one column per element, one row per frame (04) |
| **Timeline** | Horizontal view: one row per element with exposure bars, keyframes and hierarchy (04) |
| **Drawing mode / staging mode** | The two working contexts: drawing views edit artwork; staging (scene-planning) views edit placement and animation (01) |
| **Peg** | An invisible element carrying an animated transformation applied to everything attached below it (05) |
| **Motion path / control point / keyframe** | A spline path in 3-D space; control points on it; keyframes fix the position at a frame, with velocity controlling timing between them (05) |
| **Function** | An animated parameter curve (rotation, scale, skew, velocity, …) editable in the function editor (05) |
| **Skeleton / bone** | Hierarchical bones that deform attached drawings via zones of influence (05) |
| **Stroke / brush contour / pencil line / invisible stroke / zone** | Vector primitives: brush strokes are filled outlines; pencil lines have a centre line and width; invisible strokes only bound areas; zones are paintable regions closed by strokes (02) |
| **Palette / style / swatch / colour ID** | Palettes hold swatches; a palette has one or more styles giving each swatch a different value; artwork stores colour IDs so switching style recolours it (03) |
| **Onion skin / light table** | Semi-transparent display of neighbouring drawings (onion skin) or of other elements/selected drawings (light tables) while drawing (02) |
| **Template** | A reusable bundle (elements, drawings, timing, motion, colours, sound) stored in a library (08) |
| **Mouth chart** | The eight lip-sync mouth shapes A–G plus X (rest) (07) |
| **Field / field guide** | Traditional animation grid unit system (12-field, 16-field) used for staging coordinates and drawing guides (02, 05) |
| **Command** | A registered, schema-described operation with a stable id; the single path for all changes (11, A) |
| **Agent session** | An MCP client connection controlling the application under a permission policy (11) |

---

## 4. Reference architecture (functional decomposition)

A recommended decomposition. The implementation team may restructure it, but requirement P2 of section 11 (one
command layer for GUI and agents) is mandatory.

| Layer | Responsibility | Spec |
|---|---|---|
| **Document model** | Project → scenes → elements → cells/drawings; palettes; functions; effects; sounds; templates; ids and revisions | 10 |
| **Command layer** | Registry of every operation with JSON-schema parameters and results, enablement rules, undo labels, permission scopes; serialized executor; transactions | 11, A |
| **Persistence** | Project package read/write, atomic save, backups, autosave and recovery, palette/template exchange files | 10 |
| **Vector engine** | Bézier paths, stroke outlines from pressure, zone (planar region) computation, gap closing, flatten/optimize, bitmap tracing, text outlines | 02 |
| **Colour engine** | Palettes, styles, colour IDs, gradient/texture mapping, colour transforms | 03, 06 |
| **Animation engine** | Per-frame evaluation: exposures → drawing; peg hierarchy → matrices; splines + velocity → position; functions; bones → deformation; camera → projection | 04, 05 |
| **Effects & particles** | Effect elements applied to sub-trees; deterministic, seeded particle simulation | 06 |
| **Renderer** | Interactive renderer for views; deterministic offscreen renderer for previews, agent renders and export; onion-skin/light-table shading | 06, 09, 11 |
| **Audio** | Decoding, waveform cache, mixing, scrubbing, lip-sync detection | 07 |
| **Views & tools** | Drawing, Camera, Top, Side views; Timeline; Exposure Sheet; colour, properties, library, tool-options, function editor, sound editor, lip-sync, storyline, effect preview, capture panels; tool framework with modifier semantics | 01–09 |
| **I/O** | Import (images, layered images, vector art, sound, video, capture devices) and export (video, image sequences, vector animation, PDF, print, publishing) | 09 |
| **Agent server** | MCP server (stdio and local HTTP), sessions, permissions, confirmation, resources/subscriptions, jobs, audit log, headless mode | 11 |
| **Shell** | Start screen, project wizard, preferences, workspaces, themes, key bindings | 01 |

### 4.1 Per-frame evaluation (summary)

For frame *f* of the current scene: (1) for each visible element, find the cell exposed at *f* (drawing, image, media
frame, sound position); (2) walk the hierarchy from root pegs down, composing each peg's transformation at *f*
(position from motion path and velocity, rotation, scale, skew, pivot) into world matrices; (3) apply bone
deformations; (4) project through the active camera (with depth for multiplane parallax); (5) apply effects to their
sub-trees in hierarchy order; (6) draw back-to-front by depth and element order; (7) mix the sounds active at *f*.
Normative details: 05, 06, 07. The same evaluation serves interactive views, previews, export and agent renders.

---

## 5. Feature inventory (build checklist)

Use this list to scope a partial build. Each line points to the section with the full specification and the command
namespaces that expose it to agents.

**Shell & projects (01; `app`, `project`, `edit`, `view`, `window`, `workspace`, `prefs`, `keymap`, `ui`)** — start
screen with recent projects · new project with resolution and frame-rate presets · open/save/save as/close/recent ·
autosave and backups · project properties · drawing vs staging modes · dockable panels and workspaces · toolbar
customisation · themes · preferences · key-binding editor with conflict handling · undo/redo · clipboard and paste
special · status bar, tooltips, context help, tips with "don't show again".

**Drawing (02; `draw`, `tool`, `drawAid`)** — select, contour editor, brush, pencil, text, rectangle, ellipse, line,
polyline, eraser, cutter, perspective, view navigation tools, invisible-stroke tool · pens with size, smoothness,
tip shape, pressure · mirror drawing · draw behind / merge into top layer · flatten, optimize, smooth, extract centre
line, convert lines to brush, break text apart, feather edges · auto gap closing · snapping · show invisible strokes ·
grids and field guides · onion skin · auto and static light tables · import and vectorize.

**Colour (03; `colour`)** — palettes (create, duplicate, delete, rename, import/export) · styles (duplicate, rename,
delete, switch per scene) · swatches: solid, linear/radial gradient, texture · colour editor with alpha · tint
offset/blend · texture mapping · paint, paint unpainted, unpaint, dropper, close gap, edit texture · default palette.

**Elements, Timeline & Exposure Sheet (04; `element`, `cell`, `timeline`, `xsheet`)** — all element types · create,
rename, colour, notes · clone vs duplicate · ordering · hierarchy navigation · collapse/expand · show/hide, solo, lock
· element and cell properties · Timeline (ruler, playhead, frames or feet+frames, zoom, bar editing, keyframes,
playback range) · Exposure Sheet (thumbnails, display modes) · create/rename/delete cells, set/increase/decrease/extend
exposure, insert/delete blank cells, fill, cycles, columnate · drawing management · static light table.

**Staging & animation (05; `anim`)** — Camera/Top/Side views · display toggles (pegs, pivot paths, ghosts, arms, frame
markers) · select, transform, rotate/skew, scale, motion tools · peg-only mode · flips · pegs and parent pegs,
attach/detach · motion paths (TCB splines, velocity, orient to path, constant depth, loops, constant segments) ·
keyframes · function editor (curve types, reshape presets, values) · multiple cameras, active camera, field of view ·
bones (tools, zones of influence, natural movement) · motion recording.

**Effects (06; `fx`)** — effect elements over sub-trees · effect parameters (masks/mattes, sprites, bone groups) ·
colour transform · clipping · drop shadow · blur (radial/motion) · shake · natural-phenomena and action/impact particle
effects with presets and custom sprites · effect preview · all-effects toggle.

**Sound & lip-sync (07; `sound`, `lipsync`, `media`)** — import sound · placements with start/stop, loops, volume, fade
envelope · waveform display · playback and scrubbing · sound element editor · lip-sync detection (A–G, X) · mouth
mapping and apply · media (video) elements · movie import as image sequence.

**Library & storyline (08; `library`, `storyline`)** — library browser · user-wide, project and external libraries ·
folders and templates · create template from selection · insert templates (copy vs link, colour update) · template
editing and properties · import/export templates · storyline · scene add/delete/reorder/rename/show-hide ·
transitions with parameters · play storyline.

**Playback & output (09; `playback`, `render`, `import`, `export`, `capture`, `print`)** — play/stop/step/loop ·
frame-rate override · playback range · previews of movie/scene/selection · export movie (video, image sequences,
vector animation, ranges, scenes, web page, publishing) · snapshot · drawing to PDF · print and page setup · import
files (images, layered images, vector art, sound, movie) · stop-motion capture (devices, keying, light table,
auto-capture) · scanner/camera acquisition.

**Data (10; `doc`)** — entity model · ids and revisions · project package format · palette and template exchange ·
validation · JSON object model and schemas.

**Agent control (11; generic MCP tools, `agent`, `inspect`, `checkpoint`, `job`)** — headless and attached modes · command discovery and execution ·
atomic batches · object queries and patches · deterministic renders, contact sheets, frame descriptions · simulated
input · undo/checkpoints · jobs with progress · permissions, confirmations, file-system roots · events and
subscriptions · audit log and activity panel · workflow prompts · conformance tests.

---

## 6. Cross-cutting rules

- **Command layer.** Every document change, from the GUI or an agent, is a registered command executed through one
  serialized executor; every user-visible capability has a command and every visible state can be queried (11 §1–2).
- **Undo.** Every document edit is a named, undoable operation; a batch is one step. View/UI changes (zoom, pan, panel
  layout, playback, current frame) are not undoable. History is shared by the user and agents, with origin
  attribution (01, 11 §7.7).
- **Selection model.** Selection is per view type (artwork selection in drawing views; element/peg selection in
  staging views; cell selection in Exposure Sheet/Timeline) and drives command enablement and the properties panel.
  Every selection-based command also accepts explicit targets (11 §8).
- **Current frame and current element.** One global current frame and current element are shared by all views;
  changing either in one view updates the others (04, 05).
- **Messages.** Errors and warnings have stable dotted codes (`area.reason`). In the GUI, informational warnings may
  offer "don't show again", resettable in preferences (01). Agents receive the same conditions as structured
  errors/warnings and never see modal messages (11 §10).
- **Persisted settings.** User preferences, tool options, last-used folders and dialog choices are stored per user
  and are readable/settable by command (01, 02, 09).
- **Determinism.** Document evaluation and offscreen rendering are deterministic; random processes are seeded and the
  seed is stored in the document (06, 11).

---

## 7. Key design decisions (summary)

Each section lists its own decisions with recommended defaults. The most consequential:

1. **Numeric defaults** for element colours, effect presets, gap-closing distances, default pens, new-scene length:
   recommended values are given per section; tune by usability testing (02, 03, 04, 06, 08).
2. **Transform conventions** — transform order, axis signs, units between fields and scene units, default camera
   distance and field of view, velocity range: section 05 fixes these normatively so documents and agent results are
   reproducible.
3. **Bone weighting** for infinite vs elliptical zones of influence and natural-movement modes (05).
4. **Transitions** — whether transitions overlap adjacent scenes or insert frames; default type and duration (08).
5. **Export formats** — baseline MP4/H.264, PNG sequence and GIF; vector-animation export optional (09).
6. **Lip-sync detector** — algorithm choice and phoneme-to-mouth mapping; transcript-assisted detection recommended (07).
7. **Project format** — zip + JSON package with an equivalent folder form; versioned schemas with migrations (10).
8. **Agent surface** — generic MCP tools by default, typed per-command tools optional; software rasteriser for all
   offscreen renders (11).
