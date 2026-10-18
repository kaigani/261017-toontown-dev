# 10 — Data Model and Project Format

This section defines the application's **document object model** and the **project format** used to persist it: the
containment tree of a project; every persisted entity with its fields, types, defaults and constraints; how objects are
identified, addressed, versioned and how their changes are described; the project package (`.animpkg`) and its unpacked
folder form; the palette exchange file (`.animpal`), the template file (`.animtpl`) and the library folder; and the load,
validation, save, backup, autosave and recovery behaviour.

**Single source of truth.** The JSON object model defined here is shared by three consumers: the files inside the
project package, the in-memory document used by every view, and the agent interface (`11_agent_interface_mcp.md`).
The JSON returned by `doc.object.get` / `get_object`, `doc.object.find` / `find_objects` and `query` is **the same schema**
as the persisted JSON, minus properties classified as derived or transient (§2.7). One serializer and one set of JSON
Schemas (`doc.schema.get`) serve all three.

Behavioural semantics belong to the owning sections and are only summarised here: project commands, autosave and
backup commands (01); drawing content (02); palettes and painting (03); elements, exposures and drawing management (04);
transforms, channels, cameras and bones (05); effects (06); sound, lip-sync and media (07); libraries, templates and
storyline (08); import and export (09); agent tools (11). Where a field is defined in detail elsewhere this section
gives its persisted name and type and points there.

---

## 1. Conventions

### 1.1 Value types

| Type | JSON representation | Notes |
|---|---|---|
| `id` | string | Stable opaque identifier (§2.3). Clients MUST NOT parse it. |
| `string` | string | Unicode, stored as UTF-8, normalised to NFC on input. |
| `int` | number (integer) | 32-bit signed range unless stated. |
| `number` | number | IEEE-754 double. NaN and infinities are invalid. |
| `bool` | `true` / `false` | — |
| `enum` | string | Lower camelCase literal from the listed set (letters such as mouth shapes are upper case). |
| `colour` | string `#RRGGBBAA` | sRGB, straight (non-premultiplied) alpha. `#RRGGBB` is accepted on input and stored with `FF` alpha. Written upper case. Document artwork refers to palette colours by **colour ID**, never by value (section 03). |
| `point2` | `{ "x": number, "y": number }` | — |
| `point3` | `{ "x": number, "y": number, "z": number }` | — |
| `matrix2d` | `[a, b, c, d, tx, ty]` | 2×3 affine matrix: (x, y) ↦ (a·x + c·y + tx, b·x + d·y + ty). |
| `frame` | int ≥ 1 | Frame numbers are **1-based** everywhere: in files, queries and commands. Ranges are inclusive `{from, to}`. |
| `duration` | int ≥ 0 | A count of frames. |
| `seconds` | number ≥ 0 | Only for positions inside sound and movie media. |
| `angle` | number | Degrees, counter-clockwise positive (section 05 is normative). |
| `timestamp` | string | ISO-8601 UTC with milliseconds, e.g. `2026-10-17T09:30:12.345Z`. |
| `ref<T>` | `id` or `null` | Reference to an object of type T; `null` = none. |
| `[T]` | array | Ordered unless stated otherwise. |
| `blob` | `{ "entry": string, "mediaType": string, "size": int, "sha256": string }` | Binary payloads (pixels, audio, video) are never inlined in object JSON. `entry` is the package entry path. Read with `doc.blob.get`. |

### 1.2 Coordinate spaces and units

The model stores geometry in two canonical spaces; all other spaces are display or input spaces (sections 05, 11).

| Space | Origin and axes | Unit | Persisted in |
|---|---|---|---|
| **Drawing space** | Drawing centre; +x right, +y up | drawing unit (du); **1 field = 100 du** (section 02) | Strokes, text, embedded bitmaps, drawing pivot |
| **Scene space** (and node-local / parent spaces derived from it) | Centre of the default camera frame on the table plane (z = 0); +x right (east), +y up (north), +z towards the default camera (front) | **field** (1 field = 1 inch = 2.54 cm; section 05) | Offsets, motion points, pivots, camera position, bone values, effect regions |
| Camera output space | Top-left of the camera frame, y down | output pixel | Never persisted (rendering only) |
| View space | Top-left of a view canvas, y down | view pixel | Never persisted (input simulation only) |

Rules:
- Through the default camera the camera frame spans 12 fields horizontally on the table plane (section 05); at identity
  element transform the 12-field guide of a drawing (x ∈ [−600, 600] du) coincides with it.
- Inches, centimetres and the `E/W`, `N/S`, `F/B` direction suffixes are display conventions only; persisted values are
  signed numbers in du or fields.
- Angles are degrees; time is frames (integers for keyframes; real numbers only for derived passage times).

### 1.3 Names

All names are trimmed of leading and trailing whitespace on input and MUST NOT be empty or contain control characters
(U+0000–U+001F, U+007F). Per-entity rules:

| Entity | Forbidden characters | Max length | Uniqueness scope | Default pattern |
|---|---|---|---|---|
| Project | `\ / : * ? " < > \|`, trailing `.` | 100 | — | `Untitled` |
| Scene | `\ / : * ? " < > \|` | 128 | project | `Scene-<n>` |
| Element | `\ / : * ? " < > \|` and `.` | 64 | scene | per element type (§4.5) |
| Drawing | `\ / : * ? " < > \|` and `.` | 64 | drawing bank | next unused integer `1`, `2`, … |
| Palette, palette style | `\ / : * ? " < > \|` | 128 | project / palette | section 03 |
| Swatch | — | 128 | none (names may repeat) | section 03 |
| Pen | `\ / : * ? " < > \|` | 64 | project | `Pen <n>` |
| Library folder, library item | `\ / : * ? " < > \|` | 128 | siblings, case-insensitive (section 08) | — |

The default pattern uses the smallest positive integer that makes the name unique. Name comparison for uniqueness is
case-sensitive except where stated.

---

## 2. Object model fundamentals

### 2.1 Object types

Every addressable object has a `type`. Type names are lower camelCase and are the same strings used in paths (§2.4),
schemas (`doc.schema.get`), `doc.object.find` filters, command results and change events.

| Type | Parent | Path selector by name | Behaviour owner |
|---|---|---|---|
| `project` | — (root) | — | 01 |
| `palette` | project | `palette:"Name"` (unique) | 03 |
| `paletteStyle` | palette | `paletteStyle:"Name"` (unique in palette); alias `style` | 03 |
| `swatch` | palette | `swatch:"Name"` (first match; ambiguity is an error) | 03 |
| `pen` | project | `pen:"Name"` (unique) | 02 |
| `asset` | project | `asset:"Name"` | 07, 09, §4.16 |
| `drawingBank` | project | id only | 04 |
| `drawing` | drawingBank (addressed through any element that uses the bank) | `drawing:"Name"` (unique in bank) | 02, 04 |
| `drawingNode` | drawing or group node | `drawingNode:"Name"` or `#index`; alias `layer` | 02 |
| `stroke` | layer node | `#index` or `@id` | 02 |
| `scene` | project | `scene:"Name"` (unique) | 04, 08 |
| `transition` | project (start edit point) or scene (outgoing edit point) | `transition` (singleton below a scene; `project/transition` = start) | 08 |
| `element` | scene (hierarchy parent may be another element) | `element:"Name"` (unique in scene) | 04 |
| `channel` | element | `channel:"rotation"` (channel id, §4.12) | 05 |
| `key` | channel | `key:"24"` (frame number as name) or `@id`; alias `keyframe` | 05 |
| `particleBlock` | element (particle effect) | `#index` or `@id` | 06 |
| `soundPlacement` | element (sound) | `#index` or `@id`; alias `placement` | 07 |
| `mediaSequence` | element (media) | `#index` or `@id` | 07 |
| `library` | project (local) or application (global, opened) | `library:local`, `library:global`, `library:"Name"` | 08 |
| `libraryFolder` | library or folder | `libraryFolder:"Name"`; alias `folder` | 08 |
| `libraryItem` | folder or library | `libraryItem:"Name"`; aliases `template`, `item` | 08 |

Cells are not objects with IDs: `cell:#12` below a cell-based element addresses the cell at frame 12 (§4.7).
Value objects that are not separately addressable (they are properties of their owner, changed by `doc.object.patch` on
the owner) include: exposure runs, cell notes, the static transform block, effect `params`, envelopes, mouth tracks,
lip-sync mappings, bone influence, text runs, colour-recovery entries.

### 2.2 Object envelope and stubs

Every object's JSON begins with the same envelope:

```json
{ "id": "el_01JA8Q6V8Z3K7M2N5P4R9S0T1W", "type": "element", "rev": 418, "name": "Bob", "meta": {}, "ext": {} }
```

| Field | Type | Rules |
|---|---|---|
| `id` | id | Immutable (§2.3). For `swatch` objects the identifier field is named **`colourId`** (section 03 §12.3); `id` is accepted as an alias on input and both names refer to the same value. |
| `type` | string | Immutable; one of §2.1. |
| `rev` | int | Document revision at which the object last changed (§2.5). Read-only. |
| `name` | string | Present for named types (§1.3). |
| `meta` | object | Free-form metadata for users, agents and tools (tags, review status, notes). Never interpreted by the application; preserved on copy, duplicate, export and template creation. Default `{}`. |
| `ext` | object | Extension data keyed by a reverse-DNS namespace (`"com.example.tool": {…}`). Unknown namespaces MUST be preserved verbatim through load, edit and save. Default `{}`. |

**Stubs.** Child collections contain either full child objects or **stubs** `{ "id", "type", "name" }` (11 §7.4). A full
object always contains `rev`; an object without `rev` is a stub. Both forms validate against the schema. In query results
the `depth` parameter decides which children are expanded; in persisted files, stubs appear exactly where the package
splits an object into a separate entry (§6.3).

### 2.3 Identifiers

- **ID-01** Every object listed in §2.1 has an `id` that is unique within the project across all types, assigned at
  creation and never changed or reused for the lifetime of the project, including after deletion.
- **ID-02** IDs are opaque strings of at most 64 characters from `[A-Za-z0-9_-]`. **Design decision:** a short type
  prefix, an underscore and a 26-character time-ordered random identifier (ULID-style, 128 bits), e.g.
  `sw_01JA8Q6V8Z3K7M2N5P4R9S0T1W`. With 128 random bits IDs are also unique across projects, which colour identity and
  library links rely on.
- **ID-03** Objects inside a drawing (`drawingNode`, `stroke`) and keys inside a channel MAY carry short IDs unique only
  within their container (e.g. `s42`). Their project-wide form is `<containerId>/<localId>` (e.g. `dr_01JA…/s42`);
  every interface that accepts an `id` accepts the compound form. Zones are derived and have **transient** IDs that are
  valid only for the drawing revision at which they were read (section 02 §3.12).
- **ID-04** IDs survive save and load, undo and redo (undoing a deletion restores the original ID), and moves within
  the project (re-parenting, re-ordering, moving a drawing between cells).
- **ID-05** Copy/paste, duplicate, template insertion and `doc.import.json` with `idPolicy: "new"` assign **new** IDs to
  every created object and rewrite internal references consistently — **except colour IDs**:
  - pasted or inserted artwork keeps the colour IDs it uses; when the target project lacks a colour ID it is recovered
    from the drawing's colour-recovery data (§4.11; rules in section 03);
  - duplicating a palette style keeps colour IDs (a style is another value set for the same swatches); duplicating a
    palette creates new colour IDs (section 03);
  - importing a `.animpal` keeps palette, style and colour IDs, with the conflict policy of section 03.
- **ID-06** Library, folder and item IDs are globally unique and stable across renames and moves (section 08 LIB-002);
  linked usages refer to `libraryId` + `itemId`.
- **ID-07** "No colour" is `null`; there are no reserved ID values.

### 2.4 Path addressing

Wherever an `id` is accepted, a **path** string is also accepted (11 §5.1):

```
path       := segment ( "/" segment )*
segment    := type ":" quotedName       element:"Bob"
            | type ":#" int              drawingNode:#2       1-based position in an ordered collection
            | type ":@" id               stroke:@s42          by id (local or project-wide)
            | type ":" keyword           library:global       reserved keywords: local, global (libraries)
            | type                       transition, camera    singleton shortcuts (see rules)
quotedName := '"' ( any character except '"' and '\' | '\"' | '\\' )* '"'
```

Rules:
- Paths start at the project root; a leading `project` segment is optional. Special references of 11 §5.1
  (`@current`, `@selection`, `@activeView`, `@activeCamera`) MAY be used as the first segment.
- Examples: `scene:"Intro"/element:"Bob"/drawing:"3"`, `palette:"Main"/swatch:"Skin light"`,
  `palette:"Main"/style:"Night"`, `scene:"Intro"/element:"Bob-P"/channel:"rotation"/key:"24"`,
  `scene:"Intro"/element:"Bob"/drawing:"3"/layer:#2/stroke:@s42`, `scene:"Intro"/element:"Bob"/cell:#12`,
  `pen:"Pen 1"`, `library:global/folder:"Props"/template:"Walk"`, `scene:"Intro"/camera`, `scene:"Intro"/transition`.
- `element:"Name"` searches the whole element tree of the scene (names are unique per scene). `drawing:"Name"` below an
  element resolves through that element's drawing bank. `camera` below a scene is the scene's current camera.
  `cell:#n` and `key:"n"` select by frame number.
- Names are matched exactly first, then case-insensitively. More than one match is `ref.ambiguous` (with candidate IDs
  and canonical paths); no match is `ref.notFound`; a match of the wrong type for the parameter is `ref.wrongType`.
- Every object has a **canonical path** (quoted names where the type is name-addressable, otherwise `:@id`), returned by
  `doc.object.resolve` and included in query results as the transient field `path`.
- Paths change when objects are renamed or moved; agents SHOULD keep IDs.

### 2.5 Revisions and optimistic concurrency

- **REV-01** The document has a monotonically increasing integer **`documentRev`**. Every committed transaction (one
  command, one atomic `batch`, one undo or redo step, one load-time repair set) increments it by exactly 1. Queries never
  change it.
- **REV-02** Each object's `rev` is the `documentRev` of the last transaction that changed one of its own persisted
  properties or the membership or order of its own child collections. A change to a child's properties does not change
  the parent's `rev`. Content edits of a drawing (strokes, nodes) change the drawing's `rev` as well as the affected
  node's (section 02 §3.12 calls this the drawing revision).
- **REV-03** Undo and redo are new transactions: they increment `documentRev` and stamp the affected objects with the
  new value. Revisions never decrease.
- **REV-04** `documentRev` and every object's `rev` are persisted, so `doc.diff` and `expectRev` work across save and
  reopen. The project's clean marker (01 §3.5) is the undo position at the last save; `savedRev` (the `documentRev`
  written by the last save) is reported by `doc.state.get`.
- **REV-05** Every mutating command that targets existing objects accepts `expectRev` (an int for a single target, or a
  map `{id: rev}`; 11 §7.2). If any listed object's current `rev` differs, the command fails with `rev.conflict`
  (`details.currentRev`, and the object's current JSON when it is smaller than 64 KiB) and changes nothing.
- **REV-06** GUI and agent edits share one serialised transaction queue. A GUI gesture in progress (e.g. a stroke being
  drawn) is committed as one transaction when it ends; agent commands arriving meanwhile wait or fail with `ui.busy`
  as defined in 11 §6.4.

### 2.6 Change descriptions

Every committed transaction produces a **change set**. It is the payload of the `doc.changed` event (11 §9.2), the
`changes` list of the command result envelope (11 §7.2), the unit of `doc.diff`, and the record used for undo and
autosave.

```json
{
  "undoStepId": "u-381",
  "documentRev": 1043,
  "label": "Rename Element",
  "origin": "agent:claude",
  "time": "2026-10-17T09:30:12.345Z",
  "changes": [
    { "id": "el_01JA…", "type": "element", "op": "modified", "rev": 1043,
      "patch": [ { "op": "replace", "path": "/name", "value": "Robert", "oldValue": "Bob" } ] },
    { "id": "dr_01JB…", "type": "drawing", "op": "created", "rev": 1043, "parentId": "db_01J9…", "index": 4 },
    { "id": "dr_01J7…", "type": "drawing", "op": "deleted", "rev": 1043, "parentId": "db_01J9…" },
    { "id": "sc_01J2…", "type": "scene", "op": "modified", "rev": 1043,
      "patch": [ { "op": "move", "from": "/tree/3", "path": "/tree/0/children/0" } ] },
    { "id": "el_01JC…", "type": "element", "op": "modified", "rev": 1043,
      "patch": [ { "op": "replace", "path": "/exposure", "range": { "from": 10, "to": 24 },
                   "value": [ ["dr_01JB…", 15] ], "oldValue": [ ["dr_01J7…", 15] ] } ] }
  ]
}
```

Rules:
- Object-level `op` ∈ `created`, `modified`, `deleted` (the vocabulary of 11 §7.2). `created` carries `parentId` and
  `index`; `deleted` carries `parentId`. Re-parenting and re-ordering appear as `modified` on the owning collection
  (scene `tree`, `columnOrder`, a bank's `drawings`, a drawing's `nodes`).
- `patch` is a list of RFC 6902-style operations (`add`, `remove`, `replace`, `move`) whose `path` is a JSON Pointer
  into the object's JSON, extended with `oldValue` so every change set can be inverted, and with `range` for exposure
  and mouth-track changes so that long scenes do not produce large patches.
- In events and `doc.diff` results any `value` or `oldValue` larger than 64 KiB is omitted and replaced by
  `"elided": true`; the consumer re-reads the object. The command result envelope MAY omit `patch` entirely (11 §7.2
  caps the list and points to `doc.diff`).
- `origin` is `user`, `agent:<client>`, `system` (automatic actions such as migration or background repair) or `load`.
- Derived data (zones, thumbnails, waveform peaks) never appears in change sets.

### 2.7 Persisted, derived and transient properties

| Class | Schema marker | Persisted | In query results | Writable through `doc.object.patch` |
|---|---|---|---|---|
| Persisted | — | yes | yes | yes, subject to validation |
| Persisted read-only | `readOnly: true` | yes | yes | no (`id`, `type`, `rev`, `createdAt`, `elementType` except via conversion commands) |
| Derived | `x-derived: true` | no | only with `include: ["derived"]` (cheap ones always: see per type) | no |
| Transient | `x-transient: true` | no | yes | no (changed only by UI/state commands) |

- **Derived** data is recomputable from persisted data: zones and contours, scene length, `parentId`/`childIds`,
  clone-group membership, usage and reference lists, natural end frames, asset media info, thumbnails, waveform peaks.
- **Transient** data is session state: canonical `path`, missing-media status, selection, current frame/scene/element,
  view state, playback, job state.
- **Rule DM-1:** the persisted JSON of an object is exactly its query JSON (at the depth matching the package split)
  with derived and transient properties removed. Implementations MUST use one serializer for both.
- The undo history is not persisted. Last-session UI state MAY be stored separately (§6.12) and is not part of the
  object model.

### 2.8 Schemas

- Every type has a JSON Schema (draft 2020-12) returned by `doc.schema.get`. Schemas carry the markers of §2.7 and the
  annotations `x-unit` (`du`, `field`, `degree`, `frame`, `second`, `percent`, `pixel`), `x-command` (the dedicated
  command whose semantics a patch of this property follows, e.g. `element.rename` for element `name`), `x-ref` (target
  type of a reference) and `x-since` (format version that introduced the property).
- The schema version equals the project format version (§6.6) and is reported in the MCP handshake (11 §4).
- Schemas are generated from the same definitions the application uses for validation; a conformance test MUST check
  that every persisted file of a test corpus validates and that every `doc.object.get` result validates.

---

## 3. Containment tree

```
project
├── settings: name, frame rate, camera size, startTransition (edit point before the first scene)
├── palette*  (ordered; ≥ 1)                                           — section 03
│     ├── paletteStyle* (ordered; ≥ 1)
│     └── swatch*  keyed by colourId; one value per style (solid | linear gradient | radial gradient | texture)
├── pen*  (ordered; ≥ 1)
├── asset*  (media files: sound, image, movie, vector file; embedded or linked)
├── drawingBank*  (one per clone group)
│     └── drawing*  (name unique in bank; pivot; lock; optional mouth shape)
│           ├── drawingNode*  (back-to-front): layer ── stroke*  |  group ── drawingNode*  |  text  |  bitmap
│           │       derived: zones and contours per layer
│           └── colourRecovery entries
├── scene*  (ordered = storyline order; ≥ 1)
│     ├── name, visible, background colour, current camera, palette style overrides; derived length
│     ├── outTransition (edit point after this scene)
│     ├── tree  (Timeline order and hierarchy)        ── columnOrder (Exposure Sheet order)
│     └── element*  — elementType: drawing | annotation | image | sound | media | peg | camera | skeleton | bone |
│           │          effectParameter | colourTransform | clipping | dropShadow | blur | shake | particleNature | particleImpact
│           ├── common: name, timing (start, duration, loops), visibility, lock, colours, note, column width
│           ├── cell-based: drawingBankId, exposure runs, cell notes
│           ├── transformable: static transform + channel* ── key*
│           └── type-specific: camera settings, effect params, particleBlock*, bone rest data, soundPlacement*,
│               lip-sync mappings, mediaSequence*
└── library (local) ── libraryFolder* ── libraryItem* (template | image | movie | sound)
```

Outside the project: global and opened **libraries** (library folders, §6.11), **palette files** (`.animpal`, §6.10)
and **template files** (`.animtpl`, §6.9).

---

## 4. Entity reference

Each table lists the persisted fields of the type in addition to the envelope (§2.2). "Default" is the value of a newly
created object. Ranges are inclusive. A command given a value outside its range fails with the owning area's
out-of-range code (e.g. `anim.valueOutOfRange`) or the generic `arg.invalid`; a value outside its range found in a file
is clamped and reported (§7.2).

### 4.1 Project

| Field | Type | Default | Constraints / notes |
|---|---|---|---|
| `name` | string | `Untitled` | §1.3. Also the default package file stem. |
| `createdAt`, `modifiedAt` | timestamp | creation time | Read-only; `modifiedAt` is set by each save. |
| `frameRate` | number (fps) | new-project form (01 §3.1) | 1–120; the NTSC rates 23.976, 29.97 and 59.94 MUST be representable exactly as written. |
| `cameraWidth`, `cameraHeight` | int (pixels) | new-project form | 16–8192 each. Output resolution and camera aspect (01 §3.8). |
| `startTransition` | transition | type `none` | Edit point before the first scene (08 §13). |
| `palettes` | [palette] | one default palette (section 03) | ≥ 1. Order = display order. |
| `pens` | [pen] | default pens (section 02) | ≥ 1. |
| `assets` | [asset] | [] | §4.16. |
| `drawingBanks` | [drawingBank] | one per cell-based element | §4.8. Unordered. |
| `scenes` | [scene] | one scene with a camera and an empty drawing element (01 §3.1) | ≥ 1. Order = Scene Manager / storyline order. |
| `library` | library | empty, kind `local` | The project-local library (08 §1). |

Derived: `sceneStartFrames` (global first frame of each visible scene, 08 §16.4), `globalLength`, `thumbnail` (blob).

### 4.2 Pen

| Field | Type | Default | Constraints / notes |
|---|---|---|---|
| `name` | string | `Pen <n>` | Unique in project. |
| `minSize`, `maxSize` | number (du) | per default pen set | 0–100 each; `minSize ≤ maxSize`. |
| `smoothness` | int | 0 | 0–10. |
| `tip` | enum | `round` | Tip shapes of section 02. |
| `colourId` | ref&lt;swatch&gt; | null | Colour bound to the pen; `null` = use the current colour. |

The default pen set (count, sizes, smoothness) is defined in section 02.

### 4.3 Scene

| Field | Type | Default | Constraints / notes |
|---|---|---|---|
| `name` | string | `Scene-<n>` | §1.3 (08 STL-002). |
| `visible` | bool | true | Hidden scenes are skipped by storyline playback and export (08). |
| `backgroundColour` | colour | `#FFFFFFFF` | — |
| `currentCameraId` | ref&lt;element&gt; | the scene's first camera | MUST reference a camera of this scene when the scene has one. |
| `paletteStyles` | map paletteId → paletteStyleId | first style of each palette | The active style per palette for this scene (03 §1). Exactly one entry per palette. |
| `outTransition` | transition | type `none` | Edit point after this scene (08 §13). |
| `tree` | [treeNode] | — | `treeNode = { "id": elementId, "children": [treeNode] }`. **Timeline order**: siblings top-to-bottom as displayed in the Timeline; compositing order is derived as in 04 §1.2 and 05 §2.8. Every element of the scene appears exactly once. |
| `columnOrder` | [id] | creation order | **Exposure Sheet order** of the cell-based elements, left (back) to right (front). Each cell-based element appears exactly once; other elements do not appear. |
| `elements` | [element] | — | Unordered set; order comes from `tree` and `columnOrder`. |

Derived: `length` (the last frame used by any element's exposures, bars or sound placements; 04 §11.7; minimum 1),
`paddedLength` (with storyline padding, 08 §16.4).

### 4.4 Transition

Fields as defined in 08 §13.1 and §16.2: `type` (`none`, `fade`, `crossDissolve`, `wipe`, `slide`, `clockWipe`,
`checker`), `durationLeft` (0–1000, default 12), `blankDuration` (0–1000, default 0), `durationRight` (0–1000,
default 12), `colour` (default `#000000FF`), `angle` (0–360, default 0), `direction` (`in`, `out`, `clockwise`,
`counterclockwise`; type-dependent), `size` (5–100, step 5, default 25). Fields not used by the current type are kept so
that switching back restores them. An unreadable transition loads as `none` with warning `storyline.transitionInvalid`.

### 4.5 Element — common fields

Field names follow the element read model of 04 §17.5.

| Field | Type | Default | Constraints / notes |
|---|---|---|---|
| `elementType` | enum | — | `drawing`, `annotation`, `image`, `sound`, `media`, `peg`, `camera`, `skeleton`, `bone`, `effectParameter`, `colourTransform`, `clipping`, `dropShadow`, `blur`, `shake`, `particleNature`, `particleImpact`. Read-only except through conversion commands (`drawing` ↔ `annotation`, `image` → `drawing`; section 04). The 04 read model reports this value as `type`. |
| `name` | string | per type (section 04) | Unique in scene; §1.3. |
| `start` | frame | 1 | Non-cell elements: first frame of the bar. Cell-based elements: derived from the exposure. |
| `duration` | duration | scene length at creation | Non-cell elements (pegs, cameras, effects, skeletons, bones): length of the bar. Derived for cell-based elements. |
| `loops` | int | 1 | ≥ 1; 1 = not looped (05 §8.4). |
| `loopType` | enum | `count` | `count`, `fill` (05 §8.4). |
| `positionLoop` | enum | `cycle` | `cycle`, `offset` (05 §8.4). |
| `role` | enum | `normal` | `normal`, `background` (05 §8.1, 04). |
| `deformable` | bool | false (true for skin-group members) | 05 §13.3. |
| `visibleInScene` | bool | true | 04 §1.3. |
| `visibleInXsheet` | bool | true | 04 §1.3. Cell-based elements only. |
| `locked` | bool | false | 04 §10.4. |
| `collapsed` | bool | false | Timeline display state of parents, persisted with the document. |
| `markerColour`, `backgroundColour` | colour \| null | null | `null` = category default from preferences (04 §5). |
| `note` | string | "" | Element note; ≤ 10 000 characters. |
| `columnWidth` | int (view pixels) | 80 | Exposure Sheet column width (04 §1.3). 20–1000. |
| `createdAt` | timestamp | — | Read-only. |

Derived: `parentId`, `childIds`, `depth`, `timelineIndex`, `xsheetIndex`, `cloneGroupId` (= `drawingBankId` when the
bank is shared), `cloneIds`, `drawingCount`, `hiddenByParent`, `effectiveMarkerColour`.

### 4.6 Element types

| elementType | Cell-based | Children | Type-specific fields |
|---|---|---|---|
| `drawing`, `annotation` | yes | yes (cut-out hierarchies) | `drawingBankId`, `exposure`, `cellNotes`, `transform`, `channels` |
| `image` | yes | yes | as `drawing` (drawings hold one bitmap node each) plus `importOptions` (section 09) |
| `sound` | yes (sound column) | no | `streamed`, `showMouthShapes`, `placements`, `lipSyncMappings` (§4.14) |
| `media` | yes | no | `sequences` (§4.15), `transform`, `channels` |
| `peg` | no | yes | `transform` (with peg fields), `channels`, `exportGroup` |
| `camera` | no | yes (camera peg) | `camera`, `transform`, `channels` |
| `skeleton` | no | yes (`bones` and `skin` parameter elements) | `skeleton`, `transform`, `channels` |
| `bone` | no | yes (chained bones) | `bone`, `channels` |
| `effectParameter` | no | yes | `slot`: `mask`, `sprites`, `bones`, `skin` (06 §2.3). Created and deleted only with its effect or skeleton. |
| `colourTransform`, `clipping`, `dropShadow`, `blur`, `shake` | no | yes | `params`, `channels` (06) |
| `particleNature`, `particleImpact` | no | yes | `params`, `blocks` (06 §3.8, §8.4) |

**Static transform block** (`transform`, 05 §1.5, §8):

| Field | Type | Default | Notes |
|---|---|---|---|
| `offset` | point3 (field) | (0, 0, 0) | Static position. Camera default (0, 0, 16). |
| `rotation` | angle | 0 | Camera: roll. |
| `skew` | angle | 0 | −89…89. |
| `scale` | `{ x, y }` | (1, 1) | \|s\| ≥ 0.001; negative values flip. |
| `keepProportions` | bool | false | — |
| `rotationPivot`, `scalePivot` | point2 (field) | (0, 0) | Elements. |
| `rotateAboutPivot` | bool | false | Elements. |
| `pivot` | point2 (field) | (0, 0) | Pegs and skeletons. |
| `applyPivotTo` | enum | `rotation` | Pegs and skeletons: `rotation`, `scaling`, `both`. |
| `useChildDrawingPivot` | bool | false | Pegs (05 §2.3). |
| `lockDepth` | bool | false | Nodes with a motion path (05 §9.3). |
| `orientToPath` | bool | false | 05 §8.3. |

**Type-specific blocks:**

| Block | Fields (defaults) | Owner |
|---|---|---|
| `camera` | `fovMode`: `static` \| `dynamic` (`static`); `fov`: angle 1–179 (41.1121) | 05 §12.3 |
| `exportGroup` | `groupOnVectorExport`: bool (false); `addComposite`: bool (false) | 05 §8.2, 09 |
| `skeleton` | `boneColour`: colour \| null (null = preference); `showTrack` (false); `showRoot` (true); `showBones` (true); `showInfluence` (false); `vectorQuality`: `low` \| `medium` \| `high` \| `highest` (`medium`); `deformationEnabled` (true) | 05 §13.6 |
| `bone` | `rest`: `{x, y, rotation, length}` (fields, degrees; length > 0.01); `influence`: `{ type: "infinite" \| "elliptical", lengthRadius: number, widthRadius: number, region: [point2] \| null }` (infinite; radii default 0.75 × and 0.35 × rest length) | 05 §13.1–13.3 |
| `params` | Static parameter values of the effect, validated against the effect type's `paramsSchema` (06 §8.4); persisted fields per 06 §7 | 06 |
| `blocks` | [particleBlock] — the `Block` shape of 06 §8.4 (`type`, `variant`, `presetId`, `seed`, `spriteSetId`, `static{…}`, `dynamic{…}`), each an addressable object | 06 |

### 4.7 Exposure and cells

Cell-based elements (`drawing`, `annotation`, `image`) store their timing as **runs**:

```json
"exposure": [ ["dr_01JA…", 6], [null, 2], ["dr_01JB…", 12], ["dr_01JA…", 40] ]
```

- Each run is `[drawingId | null, count]` starting at frame 1; `null` is a blank cell; `count` ≥ 1.
- Canonical form (MUST be written; normalised on input and on load): adjacent runs with equal values are merged and a
  trailing blank run is omitted. Frames after the last run are blank.
- Drawing IDs MUST belong to the element's drawing bank.
- `cellNotes`: map from frame (as a decimal string) to note text (≤ 10 000 characters).
- A cell is read as `{ "frame": 12, "drawingId": "dr_…" | null, "drawingName": "3" | null, "note": string | null }`
  via `cell:#12`. Windowed run reads and writes use the `xsheet.exposures.get/set` shapes of 04 §17.2.
- Sound and media columns have no `exposure`; their cells derive from placements (§4.14) and sequences (§4.15).

### 4.8 Drawing banks and drawings

**drawingBank**: `drawings: [drawing]` (unordered; displayed in natural name order). Derived: `elementIds` (the clone
group). A bank is created with its first element; clones share it (04 §8.1); when the last element using a bank is
deleted, the bank and its drawings are deleted in the same transaction.

**drawing**

| Field | Type | Default | Constraints / notes |
|---|---|---|---|
| `name` | string | next unused integer | Unique in bank; §1.3. Shared by all clones. |
| `locked` | bool | false | Locked drawings reject content edits with `drawing.locked`. |
| `pivot` | point2 (du) | (0, 0) | Drawing pivot (02 §3.8). |
| `mouthShape` | enum \| null | null | `A`, `B`, `C`, `D`, `E`, `F`, `G`, `X`. Marks the drawing as a mouth shape for automatic lip-sync mapping (07 §10.8). Settable with `doc.object.patch`. Values need not be unique within a bank. |
| `nodes` | [drawingNode] | [] | Back-to-front (02 §3.1). Stored in the drawing's own package entry (§6.3). |
| `colourRecovery` | [colourRecoveryEntry] | [] | §4.11. |
| `createdAt` | timestamp | — | Read-only. |

Derived: `thumbnail` (blob), `usage` (`[{elementId, ranges: [{from, to}]}]`), `isEmpty`, and with
`include: ["derived"]`, `zones` per layer: `[{zoneId, layerId, colourId | null, matrix | null, bounds, area, boundingStrokeIds}]`
(zone IDs transient, ID-03).

### 4.9 Vector content (drawing nodes)

Every node has a local `id`, a `kind` and an optional `name`. Semantics in section 02 §3.

| kind | Fields |
|---|---|
| `layer` | `hint`: `brush` \| `pencil`; `strokes`: [stroke] |
| `group` | `children`: [drawingNode] (back-to-front) |
| `text` | `transform`: matrix2d; `widthMode`: `auto` \| `fixed`; `width`: number (du, fixed mode); `text`: string (≤ 100 000 characters); `runs`: [{`start`, `length` (code points), `font`, `size` (du), `bold`, `italic`, `script`: `normal` \| `subscript` \| `superscript`, `kerning`, `autoKerning`, `colourId`}] covering the text; `paragraphs`: [{`start`, `length`, `alignment`: `left` \| `centre` \| `right` \| `justify`, `marginLeft`, `marginRight`, `indent`, `lineSpacing`}] |
| `bitmap` | `image`: blob (PNG, 8-bit RGBA, straight alpha); `width`, `height` (pixels, 1–16384); `transform`: matrix2d mapping pixel space (origin top-left, y down) to drawing space |

**stroke**

| Field | Type | Default | Constraints / notes |
|---|---|---|---|
| `kind` | enum | — | `brush`, `pencil`, `invisible` (02 §3.2). |
| `closed` | bool | false | Brush outlines are closed. |
| `points` | [[x, y, inX, inY, outX, outY]] | — | Piecewise cubic Bézier in drawing space: each anchor (x, y) with its incoming and outgoing handle **offsets** relative to the anchor (0, 0 = no handle). ≥ 2 anchors (≥ 1 for a closed dab). |
| `colourId` | ref&lt;swatch&gt; | current colour | Brush: fill colour of the outline; pencil: line colour; `null` for invisible strokes. |
| `fillMatrix` | matrix2d \| null | null | Brush strokes with gradient or texture colours: placement of the colour space (02 §3.5). |
| `lineStyle` | `{ thickness: number (du), lineType: "solid" }` | from the pen | Pencil strokes only. |
| `spine` | `{ points: [[…6 numbers]], pressure: [[t, p]], pen: { penId, tip, minSize, maxSize } }` \| null | set by the brush | Brush strokes while unedited (02 §3.2); `null` once the outline was edited directly. |
| `sides` | `{ left: zoneFill \| null, right: zoneFill \| null }` | both null | Fill of the zone on each side of the stroke (relative to point order). `zoneFill = { colourId: ref<swatch> \| null, matrix: matrix2d \| null }`. |

Zone fills are persisted on the bounding strokes' sides; zones themselves are derived on load. All strokes bounding the
same zone MUST carry the same fill for that side; on load, disagreement is resolved in favour of the fill held by the
longest bounding length and reported (`drawing.fillInconsistent`, repaired).

### 4.10 Image drawings

A drawing of an `image` element contains exactly one `bitmap` node (pixels plus placement). Import placement rules are
in section 09; conversion to vector replaces the node by vector layers (04 §2.3).

### 4.11 Colour recovery entries

`colourRecovery` = `[{ "colourId", "paletteId", "paletteName", "styles": [{ "styleId", "styleName" }], "swatch": {…} }]`
where `swatch` is the full swatch object of section 03 §12.3 (all style values; texture images referenced as entries of
the containing package).

- On save, each drawing's list MUST hold exactly one entry for every colour ID its strokes, sides and text use,
  reflecting the current definitions.
- Recovery itself (when entries are applied, where swatches go, placeholders) is defined in 03 §11.2; it runs on load,
  on paste, on template insertion and on `doc.import.json`.

### 4.12 Channels and keys

`channels` is a map from channel id to a **channel** object. Channel ids and owners are those of 05 §1.6: `position`
(motion path), `velocity`, `pivot.x`, `pivot.y`, `rotation`, `scale.x`, `scale.y`, `skew`, `fov`, `bone.x`, `bone.y`,
`bone.length`, plus the animatable parameters of effects (06), named by their parameter id. `position.x/y/z` are
projections and are not stored. A channel exists only once it has at least one key; otherwise the static value applies.

**1-D channel**

| Field | Type | Default | Notes |
|---|---|---|---|
| `curveType` | enum | per preference (05 §10.2) | `spline`, `interpolation`, `velocityBased` (05 §10.3). |
| `keys` | [key] | [] | Sorted by frame; frames unique (`anim.keyExists`). |

**key**: `{ id, frame: frame, value: number, constantSegment: bool, handles: { left: {dt, dv}, right: {dt, dv}, mode: "smooth" | "broken" }, easing: string | null }`.
`frame` MUST be an integer (`anim.frameNotInteger`). `handles` apply to `spline`; `easing` (a preset id of 05 §10.4)
applies to `interpolation`. Handle offsets are clamped as in 05 §10.3.

**Motion path channel** (`position`):

| Field | Type | Notes |
|---|---|---|
| `points` | [motionPoint] | Ordered along the path; first and last are keyframes (05 §9.6). |

**motionPoint**: `{ id, frame: frame | null, position: point3 (field), tension, continuity, bias: number (−1…1), constantSegment: bool, lockDepth: bool }`.
`frame` set = keyframe (locked in time, integer, unique); `frame: null` = control point (its passage frame is derived
from `velocity`, 05 §10.5). `constantSegment` and `lockDepth` apply to keyframes and govern the segment that starts there.

### 4.13 Palettes, styles and swatches

The palette object is defined in **section 03 §12.3** and is used unchanged inside the project package and by the agent
interface (`colour.palette.get` and `doc.object.get` return the same shape plus the envelope of §2.2):

```json
{ "id": "pal_…", "type": "palette", "rev": 12, "name": "Main",
  "styles": [ { "id": "sty_…", "name": "Day" } ],
  "swatches": [ { "colourId": "col_…", "name": "Skin", "kind": "solid", "deformable": false,
                  "values": { "sty_…": { "rgba": "#F2D3B4FF" } } } ] }
```

- `kind` ∈ `solid`, `linearGradient`, `radialGradient`, `texture`; values are `{rgba}`, `{stops: [{position, rgba}]}` or
  `{image, tile}` per style. Every swatch MUST have exactly one value per style of its palette.
- Texture `image` values are package entry paths under `textures/` (§6.3), the same paths an `.animpal` uses.
- Palette, style and swatch constraints (last palette, last style, name rules, value ranges) are in section 03.
- Inside the object model, styles and swatches are addressable objects (`paletteStyle`, `swatch`) with their own `rev`.

### 4.14 Sound elements, placements and lip-sync

**Sound element fields**: `streamed` (bool, false; 07 §1.1), `showMouthShapes` (bool, false; Exposure Sheet shows mouths,
07 §7.3), `placements` ([soundPlacement], sorted by `startFrame`, non-overlapping), `lipSyncMappings`
(`[{ "drawingElementId": id, "mapping": { "A".."G","X": drawingId | null }, "autoReapply": bool }]`, one entry per
mapped drawing element; 07 §10.8).

**soundPlacement** (07 §1.2):

| Field | Type | Default | Constraints |
|---|---|---|---|
| `assetId` | ref&lt;asset&gt; | — | A `sound` asset, or a `movie` asset with audio. |
| `startFrame` | frame | — | — |
| `stopFrame` | frame | natural end | `startFrame ≤ stopFrame ≤` natural end (derived `naturalEndFrame`). |
| `inPoint`, `outPoint` | seconds | 0, source duration | `0 ≤ in < out ≤ duration`. |
| `loops` | int | 1 | ≥ 1. |
| `volume` | int | 0 | −50…+50. |
| `envelope` | [{ `time`: seconds, `left`: number 0..1, `right`: number 0..1 }] | [] | Times within the source, ascending. |
| `mouthTrack` | mouthTrack \| null | null | The placement's lip-sync track (07 §10.3). |

**mouthTrack**:

| Field | Type | Notes |
|---|---|---|
| `mouths` | string | One letter (`A`–`G`, `X`) per frame of the placement, starting at the placement's first frame — the compact form of `lipsync.track.get {format: "string"}`. |
| `manual` | [{ `from`, `to` }] | Placement-relative frame ranges set by hand or explicitly (`manual[f]`). |
| `labels` | map frame (string, placement-relative) → string | Optional display labels. |
| `source` | enum | `detected`, `transcript`, `phonemes`, `explicit`. |
| `producedFor` | `{ assetId, inPoint, outPoint, loops, stopFrame, frameRate }` | Snapshot of the timing the track was produced for. |

Derived: `stale` (true when `producedFor` no longer matches the placement or the project frame rate; 07 §10.3).
Mouth-track changes are reported in change sets with `range` patches on `/mouthTrack/mouths`.

### 4.15 Media elements

`sequences`: [mediaSequence], each `{ id, assetId, startFrame, stopFrame, linkOffset }` (07 §1.1; `linkOffset` = frame
of the linked source shown at `startFrame`, ≥ 1). Sequences of one element MUST NOT overlap. The linked source is an
`asset` (§4.16) with `linked` storage — an external file or a library item. Derived: `sourceLength`, `missing`.

### 4.16 Assets

An asset is the project-wide registry entry for one media file (07's *sound asset* and *media object*).

| Field | Type | Default | Constraints / notes |
|---|---|---|---|
| `name` | string | file name | Informational; made unique when files are extracted (07 §2.3). |
| `kind` | enum | detected | `sound`, `image`, `movie`, `vector` (vector files linked by media elements). |
| `mediaType` | string | detected | IANA media type, e.g. `audio/wav`. |
| `storage` | enum | `embedded` for imported sound and images; `linked` for media-element sources | §6.7. |
| `entry` | string | — | Package entry path when `embedded`. |
| `link` | `{ kind: "file" \| "libraryItem", path: string, absolutePath: string, libraryId: id, itemId: id, versionId: string }` | — | When `linked`: for files, path relative to the folder containing the project plus the absolute path at link time; for library items, the library and item IDs (08 LIB-002) and the template version seen at link time (LIB-007). |
| `size`, `sha256` | int, string | computed | Of the file bytes. Identical content is stored once (07 §2.1). |
| `importedAt` | timestamp | — | Read-only. |
| `sourcePath` | string \| null | — | Where the file was imported from (informational). |

Derived: `info` (`durationSeconds`, `sampleRate`, `channels`, `bitDepth`, `width`, `height`, `frameCount`, `fps`),
`refs` (referencing object IDs). Transient: `status` ∈ `ok`, `missing`, `changed` (linked file content differs from
`sha256`), `unsupported`.

### 4.17 Libraries and templates

| Type | Fields |
|---|---|
| `library` | `name`; `kind`: `local` \| `global` \| `opened` \| `starter`; `formatVersion`; `children`: [libraryFolder \| libraryItem]. Transient: `readOnly`, `writeAccessHeldBy`, `location`. |
| `libraryFolder` | `name`; `children`: [libraryFolder \| libraryItem] |
| `libraryItem` | `name` (display name); `itemKind`: `template` \| `image` \| `movie` \| `sound`; `file` (stored file name, relative to the library's `files/` folder); `thumbnail` (blob \| null); for templates `versionId` and the metadata of 08 §6.3 (`artist`, `copyright`, `createdAt`, `modifiedAt`, `sizeBytes`, `formatVersion`, `source`, `summary`). Transient: `broken` (missing or unreadable file). |

Display names and stored file names are independent (08 LIB-003). Library behaviour is section 08; storage is §6.11.

---

## 5. Global constraints and limits

| Rule | Limit / behaviour | Code |
|---|---|---|
| Names | §1.3 | area codes of the owning section (e.g. `element.nameInvalid`, `storyline.nameInUse`, `library.nameInvalid`) |
| Elements created by one command | 1–99 | `element.tooMany` (04) |
| Scenes, palettes, styles, pens | ≥ 1 each; deleting the last is refused | owning section |
| Scene length | ≤ 100 000 frames | `doc.limitExceeded` |
| Elements per scene | no fixed limit; MUST handle ≥ 1 000 | — |
| Drawings per bank | no fixed limit; MUST handle ≥ 10 000 | — |
| Strokes per drawing | no fixed limit; MUST handle ≥ 100 000 | — |
| Swatches per palette | MUST handle ≥ 10 000 | — |
| Keys | integer frames, unique per channel | `anim.frameNotInteger`, `anim.keyExists` |
| References | every reference resolves to an object of the expected type, or is `null` where allowed | `ref.dangling` (load), `ref.notFound` / `ref.wrongType` (commands) |
| Hierarchy | the scene tree is acyclic; parameter elements stay under their effect | `anim.hierarchyCycle` |
| Exposure | canonical runs; drawing IDs from the element's bank | `xsheet.runInvalid` (commands), `exposure.invalid` (load) |
| Package size | ZIP64; MUST handle packages and entries larger than 4 GiB | — |

There are no edition- or licence-based limits.

---

## 6. Project format

### 6.1 Forms

A project is stored in one of two equivalent forms with **identical entry trees**:

| Form | On disk | Intended use |
|---|---|---|
| **Package** | One zip file `<Name>.animpkg` | Default; sharing, e-mail, backups, recovery copies |
| **Folder** | A directory (recommended name `<Name>.animpkg`) holding the same entries as plain files | Version control (text diffs, per-entity files), external tooling |

- **FMT-01** Opening a path that is a file reads the package form; opening a path that is a directory reads the folder
  form. Both forms MUST load to the identical document, and saving either form produces entries with identical bytes
  for the same document (except zip framing).
- **FMT-02** A project remembers its form; Save writes the same form. Converting between forms is a save-as-copy in the
  other form (`doc.package.export`) followed by opening the copy, or Save As (01) with a folder destination
  (see design decisions).
- **FMT-03** When a folder-form project is created, the application writes a `.gitignore` (only if none exists) listing
  `cache/`, `session.json` and `*.lock`, and a `.gitattributes` marking `*.png`, `media/**` and `textures/**` as binary.

Extensions and media types (placeholders, to be registered by the implementation team):

| File | Extension | `mimetype` entry |
|---|---|---|
| Project package | `.animpkg` | `application/x-animpkg+zip` |
| Template | `.animtpl` | `application/x-animtpl+zip` |
| Palette exchange | `.animpal` | `application/x-animpal+zip` |
| Interchange JSON bundle | `.animjson` (plain JSON) | — |

### 6.2 Container rules (package form; also apply to `.animtpl` and `.animpal`)

- **ZIP-01** Standard zip (PKWARE APPNOTE) with ZIP64 support. Entry names are UTF-8 (language-encoding flag set), use
  `/` separators, are relative, contain no `..` segment, no drive letters and no leading `/`. A package violating this is
  rejected (`project.invalidFormat`, detail `unsafeEntry`) without extracting anything.
- **ZIP-02** The first entry is `mimetype`, stored uncompressed, without extra fields, containing exactly the media type
  string (no newline), so the type can be identified from the first bytes of the file.
- **ZIP-03** JSON entries are deflate-compressed. Already-compressed media (PNG, MP3, AAC, MP4, MOV) SHOULD be stored.
- **ZIP-04** Readers MUST ignore unknown entries outside the reserved names of §6.3 and MUST preserve them on save
  (copy unchanged), so that other tools can add data.
- **ZIP-05** No encryption, no signatures. Integrity is provided by the SHA-256 list in the manifest (§6.4).
- **ZIP-06** Readers MUST NOT trust declared sizes: decompression is bounded by the manifest sizes plus a margin and by a
  global limit (protection against zip bombs); exceeding them aborts the load (`project.invalidFormat`, `sizeLimit`).

### 6.3 Entry layout

```
mimetype                                         media type (§6.2)
manifest.json                                    format version, generator, revision, entry list (§6.4)
project.json                                     project object: settings, startTransition, pens, assets,
                                                 stubs of palettes, drawing banks, scenes and the local library
palettes/<paletteId>.json                        one palette object (03 §12.3 shape + envelope)
textures/<file>.png                              texture images referenced by palette values
scenes/<sceneId>/scene.json                      scene object: settings, outTransition, tree, columnOrder, element stubs
scenes/<sceneId>/elements/<elementId>.json       one element object: common fields, type-specific blocks, exposure,
                                                 cell notes, channels and keys, placements, sequences, particle blocks
banks/<bankId>/bank.json                         drawing bank with drawing stubs (id, name)
banks/<bankId>/<drawingId>.json                  one drawing object: name, lock, pivot, mouthShape, nodes, colourRecovery
banks/<bankId>/<drawingId>/<nodeId>.png          pixels of a bitmap node of that drawing
media/<assetId>.<ext>                            embedded asset files, original bytes (no re-encoding)
library/library.json                             the project-local library index (§6.11 format)
library/files/<file>                             its template files and media items
library/thumbnails/<itemId>.png                  item thumbnails
thumbnail.png                                    project thumbnail (derived; regenerated on save)
cache/...                                        optional derived caches (waveform peaks, drawing thumbnails); may be
                                                 deleted at any time; never listed as required in the manifest
session.json                                     optional last-session UI state (§6.12); not part of the object model
```

- **LAY-01** Every object lives in exactly one entry. Entries split the model at: project, palette, scene, element,
  drawing bank and drawing. All other objects are inline in their parent's entry.
- **LAY-02** Entry names use object IDs, never display names, so renames do not move files. Texture file names are
  generated unique names (e.g. `<swatchName>-<styleName>.png` made unique) and stay stable once written.
- **LAY-03** Entries of deleted objects disappear on the next save; until then the deletion is undoable.
- **LAY-04** A clean (unmodified) project saves without rewriting any entry. In folder form, only entries whose bytes
  changed are rewritten.

### 6.4 Manifest

```json
{
  "format": "animpkg",
  "formatVersion": "1.0",
  "minReaderVersion": "1.0",
  "requiredFeatures": [],
  "generator": { "name": "<application name>", "version": "1.4.2", "platform": "windows-x64" },
  "projectId": "pr_01JA…",
  "documentRev": 1042,
  "savedAt": "2026-10-17T09:30:12.345Z",
  "entries": [ { "path": "project.json", "size": 18234, "sha256": "9f2c…" } ]
}
```

| Field | Rules |
|---|---|
| `format` | `animpkg` (`animtpl` for templates, §6.9). |
| `formatVersion` | `MAJOR.MINOR` of the writer's format (§6.6). |
| `minReaderVersion` | Lowest reader format version able to load the file without losing meaning. |
| `requiredFeatures` | Feature ids a reader MUST understand (for optional future capabilities); empty in 1.0. |
| `generator` | Informational; never used for behaviour decisions. |
| `projectId` | ID of the project object; identifies the project for backups and recovery (01 §3.7). |
| `documentRev` | `documentRev` at save time (the persisted `savedRev`). |
| `entries` | Package form: MUST list every entry except `mimetype`, `manifest.json`, `cache/` and `session.json`, with size and SHA-256. Folder form: SHOULD be omitted (reduces version-control churn); integrity then relies on the version-control system. |

### 6.5 Canonical JSON

To make saves deterministic and diffs readable:
- **JSON-01** UTF-8 without BOM, `\n` line endings, two-space indentation, final newline.
- **JSON-02** Object keys in schema order: envelope (`id`/`colourId`, `type`, `rev`, `name`) first, then the type's
  properties in the order of its schema, then `meta`, then `ext`; unknown keys (from newer minor versions) last in
  lexicographic order.
- **JSON-03** Numbers in the shortest form that round-trips to the same double; `-0` written as `0`; integers without
  a fraction. Arrays of small numeric tuples (stroke points, exposure runs, pressure samples) are written on one line
  per tuple.
- **JSON-04** Default-valued optional properties MAY be omitted only where the schema marks them `x-omitDefault`
  (`meta`, `ext`, notes, empty maps); everything else is written explicitly.
- **JSON-05** Saving the same document twice produces byte-identical entries.

### 6.6 Versioning and migration

- **VER-01** The format version is `MAJOR.MINOR`. A **minor** increment only adds optional properties, types or enum
  values that older readers may ignore; a **major** increment is any other change.
- **VER-02** On open: if `minReaderVersion` is newer than the reader, or the major version is newer, or a
  `requiredFeatures` entry is unknown, the open fails with `project.unsupportedVersion` (01 §3.2), naming the versions
  or features. If only the minor version is newer, the project opens with warning `project.newerMinorVersion`; unknown
  properties, types and enum values are preserved verbatim (as opaque JSON) and written back unchanged on save; objects
  with unknown enum values are read-only (`doc.unknownValue` on edit attempts).
- **VER-03** Older versions are **migrated in memory** on open by a chain of migration steps (N → N+1), each a pure
  function from the old JSON to the new JSON. Migration never deletes information: anything without a new home is kept
  under `ext["app.migrated"]`. Migration does not mark the project modified. The original file is not touched until
  the user saves; the first save of a migrated project requires confirmation of the upgrade (`project.formatUpgrade`,
  01 §3.2, parameter `allowFormatUpgrade` of `project.save`). The load report records `migratedFrom`.
- **VER-04** Every migration step has fixture tests (old file → expected new JSON) that MUST pass before release.
- **VER-05** Libraries (§6.11), templates (§6.9) and palette files carry their own `formatVersion` and follow the same
  rules (08 §11 for the library upgrade confirmation).

### 6.7 Media storage

- **MED-01 Embedded** assets (default for imported sound and images) are copied byte-for-byte into `media/<assetId>.<ext>`
  (`ext` from the source file, lower case). Identical content (same SHA-256) is stored once and shared (07 §2.3).
- **MED-02 Linked** assets (media-element sources, 04 §2.5, 07 §1.1) are not copied. The link stores a path relative to
  the folder containing the project and the absolute path at link time, or a library item reference. Resolution order
  on load: (1) relative path, (2) absolute path, (3) a file with the same name and size in the project's folder,
  (4) folders added by the user or agent via `doc.asset.relink {searchFolder}`. Library links resolve through the
  library ID and item ID; a newer `versionId` marks the usage for refresh (08 §7.2).
- **MED-03** Missing assets never prevent opening: affected objects keep their timing and show placeholders; one
  summary warning per open (`project.mediaMissing`, 01 §3.2) lists all missing items; `doc.asset.list {status:"missing"}`
  and `doc.validate` report them.
- **MED-04** Embedded assets no longer referenced by any object of the project (or its local library) are removed from
  the package on save (07 §2.3); while the project is open they stay in memory so undo can restore references.
- **MED-05** Bitmap node pixels are stored as PNG entries next to their drawing (§6.3), not as assets.
- **MED-06** Waveform peak caches and thumbnails go under `cache/`; a reader MUST work without them and regenerate them
  in the background.

### 6.8 Interchange JSON bundle

The bundle is the self-contained JSON form of a subtree (whole project, one or more scenes, or a set of elements) used
by `doc.export.json`, `doc.import.json`, `.animjson` files and as the content of template files.

```json
{
  "format": "animjson",
  "formatVersion": "1.0",
  "generator": { "name": "…", "version": "…" },
  "exportedAt": "2026-10-17T09:30:12.345Z",
  "sourceProjectId": "pr_01JA…",
  "scope": { "kind": "scenes", "ids": ["sc_01J2…"] },
  "settings": { "frameRate": 24, "cameraWidth": 1920, "cameraHeight": 1080 },
  "scenes":   [ { "…full scene objects, elements inline…" } ],
  "elements": [ ],
  "drawingBanks": [ { "…banks with full drawing objects…" } ],
  "palettes": [ { "…palette objects reduced to the colour IDs used…" } ],
  "pens": [ ],
  "assets": [ { "…asset objects…" } ],
  "blobs": { "media/as_01JD….wav": { "mediaType": "audio/wav", "base64": "…" } }
}
```

- Objects are the exact object-model JSON at full depth (DM-1), so one schema validates both.
- `scope.kind` ∈ `project`, `scenes`, `elements`. For `elements`, `elements` holds the selected elements with their
  descendants, plus `tree` and `columnOrder` fragments.
- `palettes` contains every palette that holds a used colour ID, reduced to the used swatches (all styles).
- Binary payloads are either embedded in `blobs` (base64) or referenced (`"blobs": {}` and `blob.entry` paths that the
  importer resolves against a sibling folder or the source package) per the export's `binaries` option.

### 6.9 Template file (`.animtpl`)

A template is a zip following §6.2 with:

```
mimetype                    application/x-animtpl+zip
manifest.json               as §6.4 with "format": "animtpl"
template.json               metadata: id, versionId, name, artist, copyright, createdAt, modifiedAt, formatVersion,
                            source, summary (element tree: type, name, frame count, hierarchy) — 08 §6.3
content.json                an interchange bundle (§6.8) with scope "elements" (or "scenes"), frames relative to the
                            template start (frame 1)
banks/…, media/…, textures/…   binary entries referenced by content.json, laid out as in §6.3
thumbnail.png               representative frame (08 §6.2)
```

`versionId` changes whenever the content changes (08 LIB-007). Artist and copyright are stamped at creation from the
user preferences (08 LIB-010).

### 6.10 Palette exchange file (`.animpal`)

Defined in section 03 §12: an archive whose root holds `palettes.json` (format `animpal`, `formatVersion`, `author`,
`copyright`, `palettes` in the 03 §12.3 shape) and a `textures/` folder; a palette set without textures MAY be a bare
JSON file with the same content. Archive variants follow §6.2 (the `mimetype` entry SHOULD be written; readers MUST NOT
require it).

### 6.11 Library folders

A library (global, opened or starter; the project-local library uses the same layout under `library/` in the package)
is a folder:

```
<library folder>/
  library.json            { "format": "animlib", "formatVersion": "1.0", "id", "name", "kind", "root": [folders and items] }
  files/<file>            template files (.animtpl) and imported media items, flat (08 LIB-001); unique names per LIB-005
  thumbnails/<itemId>.png item thumbnails
  .lock                   write-access lock (08 LIB-006), see §8.6
```

- The hierarchy exists only in `library.json`; moving or renaming items never moves files.
- `library.json` is written atomically (§8.1) and in canonical JSON (§6.5).
- Items whose file is missing or unreadable load as `broken` (08 §1.1) and are reported by `doc.validate`.

### 6.12 Session state

`session.json` MAY store last-session UI state of the project (current scene, frame, element, Exposure Sheet scroll,
open drawing). It is written on save and close, ignored if invalid, never part of the object model, change sets or
undo, and is excluded from the manifest's integrity list and from version control (FMT-03).

---

## 7. Load behaviour and validation

### 7.1 Load sequence

`project.open` (01 §3.2) runs as a job with these phases (reported as progress steps):

1. **Container** — identify the form; for the package, check `mimetype` and entry-name safety (ZIP-01, ZIP-02).
2. **Manifest** — parse; check versions and required features (VER-02); in package form verify entry sizes and
   SHA-256 hashes (a mismatch is a warning `integrity.hashMismatch`, the entry is still read).
3. **Migrate** — apply migration steps if the version is older (VER-03).
4. **Parse and schema-validate** each entry: project, palettes, scenes, elements, banks, drawings, library index.
5. **Resolve** references and build the scene trees; resolve assets (MED-02).
6. **Repair** what can be repaired (§7.2) and run colour recovery for unresolved colour IDs (03 §11.2).
7. **Derive** — rebuild zones, scene lengths and other derived data; schedule caches and thumbnails in the background.
8. **Report** — publish the load report (§7.3), emit `project.opened` (01) and, if repairs were made, one `doc.changed`
   with origin `load`.

- **LOAD-01** Drawings MAY be loaded lazily (content parsed on first access) provided every query and command behaves
  exactly as if they were loaded; `doc.validate` and `doc.object.find` with content predicates force loading.
- **LOAD-02** Cancelling the job leaves no project open (01 §3.2).
- **LOAD-03** Loading MUST be deterministic: the same files produce the same document, the same IDs, revisions and
  repairs.

### 7.2 Validation and repair on load

| Problem | Severity | Load behaviour (repair) | Code |
|---|---|---|---|
| Not a package / no `mimetype` / unsafe entry names / unreadable zip | fatal | Not opened | `project.invalidFormat` |
| Newer major version, `minReaderVersion` too new, unknown required feature | fatal | Not opened | `project.unsupportedVersion` |
| `project.json` missing or invalid | fatal | Not opened | `project.invalidFormat` |
| Entry hash or size mismatch | warning | Entry read anyway | `integrity.hashMismatch` |
| Entry not valid JSON / fails its schema: palette, element | error | Object skipped; references to it treated as dangling; the raw entry is preserved under `ext["app.unreadable"]` of the parent and written back on save | `doc.schemaViolation` |
| Drawing entry missing or unreadable | error | Drawing kept with empty `nodes`, flagged `contentMissing`; exposures unchanged | `drawing.contentMissing` |
| Scene entry unreadable | error | Scene kept empty with its stub's name, flagged | `doc.schemaViolation` |
| Duplicate ID | error | The later object (in entry-path order) gets a new ID; references inside its own entry are rewritten | `id.duplicate` |
| Dangling reference | error | Set to `null`; exposure runs → blank; tree nodes dropped; placements/sequences removed and listed in the report | `ref.dangling` |
| Element missing from the scene tree / listed twice / cycle | error | Appended at top level / first occurrence kept / cycle broken at the deepest link | `tree.invalid` |
| Exposure not canonical, foreign drawing IDs | warning | Normalised; foreign IDs → blank | `exposure.invalid` |
| Value out of range | warning | Clamped | `doc.valueClamped` |
| Invalid or duplicate name | warning | Forbidden characters replaced by `_`, too-long names truncated, duplicates suffixed `-2`, `-3`, … | `doc.nameRepaired` |
| Inconsistent zone fills | warning | §4.9 rule | `drawing.fillInconsistent` |
| Unresolved colour ID | warning | Colour recovery (03 §11.2) | `colour.recovered` / `colour.placeholderCreated` |
| Missing asset | warning | Placeholder; one summary warning per open | `project.mediaMissing` |
| Unreadable transition | warning | Type `none` | `storyline.transitionInvalid` |
| Missing font | info | Fallback for display; name kept | `text.fontMissing` |
| Unknown properties (newer minor version) | info | Preserved | `project.newerMinorVersion` |

- **REP-01** Repairs form one transaction with origin `load`, undoable as "Repair on Open"; the project is then
  modified (so the user can save the repaired version or discard it).
- **REP-02** No repair may silently lose user content: anything removed is listed in the load report with its old JSON.

### 7.3 Load report

The load report is `{ "projectId", "path", "form": "package" | "folder", "formatVersion", "migratedFrom": string | null,
"readOnly": bool, "durationMs", "issues": [issue], "repairedCount" }` where `issue` is the structure of §10.4. It is
returned in the `project.open` job result (`warnings[]`), available afterwards through `doc.loadReport.get`, and shown
to the user once as a non-modal summary (never one message per item).

---

## 8. Save, backups, autosave, recovery and locking

Commands and user-facing behaviour are in 01 §3.5–§3.7 (`project.save`, `project.saveAs`, `project.recovery.*`,
`project.backup.*`, preferences `files.autosave.*`, `files.backups.count`). This section defines the storage behaviour.

### 8.1 Atomic save

- **SAVE-01 Package form.** Write the complete new package to a temporary file in the destination folder
  (`.<name>.animpkg.<random>.tmp`), flush it to stable storage, re-open it and verify the manifest hashes, then
  replace the destination atomically (rename-over on POSIX; replace-file with write-through on Windows). Unchanged
  entries MAY be copied from the old package without recompression. On any failure the temporary file is deleted and
  the destination is untouched (01 §3.6).
- **SAVE-02 Folder form.** Write each changed entry to a temporary sibling file, flush, then rename over the target;
  write a journal `.save-journal.json` (list of pending renames and deletions) before the first rename and delete it
  after the last. `manifest.json` is written last. On open, a leftover journal means an interrupted save: the
  application completes the listed renames if every temporary file is present and valid, otherwise rolls back by
  deleting the temporaries, and reports `project.saveInterrupted` (warning).
- **SAVE-03** Saving is not cancellable once writing has started (01 §3.6); progress is reported per phase
  (serialise, write media, write drawings, verify, replace).
- **SAVE-04** A save serialises a consistent snapshot taken between transactions; editing MAY continue while the
  snapshot is written, and the clean marker is set to the snapshot's undo position.
- **SAVE-05** Failures are reported with the codes of 01 §3.6 (`project.saveFailed`, `project.permissionDenied`,
  `project.readOnly`) plus `details.reason` ∈ `diskFull`, `fileInUse`, `verifyFailed`, `ioError`.

### 8.2 Backups

- **BAK-01** After a successful save that replaces an existing version, the previous version is moved (not copied, where
  the file system allows) into the per-user backups folder: `<backups>/<projectId>/<savedAt>.animpkg`, plus
  `backup.json` listing `{backupId, originalPath, projectName, savedAt, documentRev, sizeBytes}` (01 §3.7).
- **BAK-02** Folder-form projects are backed up as a package-form snapshot of the previous version.
- **BAK-03** `files.backups.count` versions are kept per project (default 3, 0 = off), oldest removed first.
- **BAK-04** Opening a backup (`project.backup.open`) loads it as an untitled copy; the backup itself is never modified.

### 8.3 Autosave

- **AUTO-01** Every `files.autosave.intervalMinutes` (default 5) while the project is modified, a recovery copy is
  written to `<recovery>/<recoveryId>/` in **folder form** (so only changed entries are rewritten), together with
  `recovery.json` `{recoveryId, projectId, projectName, originalPath, savedAt, documentRev, appVersion, processId}`.
- **AUTO-02** Autosave MUST NOT write to the user's project, change the clean marker, the undo history or the project
  location (01 §3.7). It serialises a snapshot between transactions on the main thread (copy-on-write; budget ≤ 50 ms
  for typical projects) and writes it on a background thread; it is postponed during gestures and modal operations.
- **AUTO-03** Untitled (never saved) projects are autosaved too (`originalPath: null`).
- **AUTO-04** The recovery folder is deleted after a successful save, a discard-close or a clean quit (01 §3.7).

### 8.4 Recovery

- **REC-01** At launch, recovery folders whose `processId` is not a running instance of the application are offered
  (01 §1.1, `project.recovery.list`). Recovery data is validated like a project (§7) before being offered; unusable
  data is listed with a reason.
- **REC-02** Recovering opens the copy as a **modified** project whose location is `originalPath`, so Save writes to the
  original location (01 §3.7). If the original file was saved after the recovery copy (its `savedAt` is newer), the
  user is warned before saving over it (`project.recoveryOlderThanFile`).

### 8.5 Read-only and external modification

- **RO-01** A project on a read-only location opens read-only (01 §3.2).
- **RO-02** While a project is open the application watches its file (or folder). If it is modified by another process,
  the user is warned (`project.changedOnDisk`) and offered *reload* (discarding in-memory changes) or *keep mine* (the
  next save overwrites, after confirmation). Agents receive the same choice as a `needs_confirmation` result on the
  next save.

### 8.6 Locking

- **LOCK-01** Opening a project for writing creates a lock file next to it (`<package path>.lock`, or `.lock` inside a
  folder-form project) containing `{user, host, processId, appVersion, openedAt, heartbeatAt}`; the heartbeat is
  refreshed every 60 s and the lock is removed on close.
- **LOCK-02** If a live lock held by someone else exists, the project opens read-only and the lock holder is reported
  (`project.lockedByOther`, warning; 01 §3.2). A lock whose heartbeat is older than 5 minutes, or whose process is
  known dead on the same host, is stale and is taken over with a warning.
- **LOCK-03** Libraries use the same mechanism for write access (08 LIB-006).

---

## 9. Error and issue codes defined here

| Code | Kind | Meaning |
|---|---|---|
| `doc.schemaViolation` | error / issue | An object or patch does not satisfy its schema (`details.pointer`, `details.schemaPath`). |
| `doc.readOnlyProperty` | error | Patch targets a read-only, derived or transient property. |
| `doc.structuralChange` | error | Patch would create, delete or move objects or change a child collection; use the owning area command (`details.command`). |
| `doc.patchTestFailed` | error | A `test` operation in a patch did not match. |
| `doc.limitExceeded` | error | A global limit of §5 would be exceeded. |
| `doc.unknownValue` | error | Edit of an object carrying values from a newer format minor version. |
| `doc.importConflict` | error | `doc.import.json` with `onConflict: "fail"` met an ID or name conflict (`details.conflicts`). |
| `doc.exportTooLarge` | error | Inline export over the session's size limit; write to a file instead. |
| `doc.blobTooLarge` | error | `doc.blob.get` result over `maxBytes`. |
| `doc.diffUnavailable` | error | `doc.diff` base revision older than the retained history (`details.oldestRev`); re-read objects instead. |
| `doc.valueClamped`, `doc.nameRepaired` | issue | Load-time repairs (§7.2). |
| `id.duplicate`, `ref.dangling`, `tree.invalid`, `exposure.invalid` | issue | Load-time integrity problems (§7.2). |
| `drawing.contentMissing`, `drawing.fillInconsistent` | issue | Drawing storage problems (§7.2). |
| `integrity.hashMismatch` | warning | Manifest hash or size differs from the entry. |
| `project.newerMinorVersion`, `project.saveInterrupted`, `project.lockedByOther`, `project.changedOnDisk`, `project.recoveryOlderThanFile` | warning | §6.6, §8. |
| `asset.relinkFailed` | error | No file matched for a relink request. |

Codes used here but owned elsewhere: `project.invalidFormat`, `project.unsupportedVersion`, `project.formatUpgrade`,
`project.mediaMissing`, `project.saveFailed`, `project.permissionDenied`, `project.readOnly` (01); `ref.notFound`,
`ref.ambiguous`, `ref.wrongType`, `rev.conflict`, `arg.invalid` (11); colour, element, drawing and animation codes of
sections 02–08.

---

## 10. Agent interface

### 10.1 Agent-specific requirements

- **AG-01 One model, one schema.** The generic MCP tools of section 11 are thin wrappers over the `doc` commands and
  return identical data: `get_object` ≡ `doc.object.get`, `find_objects` / `query` ≡ `doc.object.find`,
  `set_properties` ≡ `doc.object.patch` (with `props`), `doc_diff` ≡ `doc.diff`. Their results are object-model JSON
  (§2.2, DM-1). Area read models (e.g. `element.get`, `colour.palette.get`, `xsheet.exposures.get`) MUST be projections
  of the same data with the same field names, except where a section documents a renamed field (04 reports
  `elementType` as `type`).
- **AG-02 Generic patch.** `doc.object.patch` changes any writable persisted property of any object:
  - it accepts a JSON merge patch (`props`, RFC 7386 semantics) applied to every target, and/or JSON-Patch operations
    (`ops`, RFC 6902: `add`, `remove`, `replace`, `move`, `copy`, `test`) with JSON Pointers into the object;
  - the result is validated against the type's schema **and** the semantic rules of the owning section (ranges, name
    uniqueness, reference types, locks: `element.locked`, `drawing.locked`) before anything changes;
  - a property annotated with `x-command` is applied with exactly the semantics and side effects of that command
    (patching an element's `name` ≡ `element.rename`; patching a camera's `camera.fov` ≡ `anim.camera.set`);
  - creating, deleting, re-parenting or re-ordering objects and editing child collections is refused with
    `doc.structuralChange` naming the area command to use; read-only, derived and transient properties are refused with
    `doc.readOnlyProperty`;
  - value-typed collections (gradient stops, envelopes, exposure runs, `meta`, `ext`, mapping tables) are patchable;
    exposure runs are normalised (§4.7) after the patch;
  - all targets change atomically in **one undo step** labelled `label` or "Set <property>"; `dryRun` returns the change
    set without applying it; `expectRev` gives optimistic concurrency (REV-05).
- **AG-03 Addressing.** All `ref` parameters accept IDs, compound IDs and paths (§2.4) and the special references of 11.
  Results always include `id` and the canonical `path`.
- **AG-04 Read-back completeness.** Everything persisted is readable through `doc.object.get`; everything derived that a
  user can see (scene length, zones, usage, clone groups, missing status, stale lip-sync) is readable through
  `include: ["derived"]` or the area read models. Visual verification uses the render tools of 11 §7.5.
- **AG-05 Large objects.** `doc.object.get` supports `page: {pointer, offset, limit}` to page through a large array
  (strokes of a layer, keys of a channel, exposure runs) and `fields` to project only needed properties; a result that
  would exceed the session limit fails with `limit.exceeded` and a hint to use `depth`, `fields` or `page`.
- **AG-06 Determinism.** Identical documents yield identical JSON (canonical key order, §6.5). `doc.object.find` returns
  items in canonical-path order unless `sort` is given. Revisions, IDs and repairs are deterministic (LOAD-03).
- **AG-07 No modal paths.** Every decision the GUI asks for is a parameter: format upgrade (`allowFormatUpgrade` of
  `project.save`, 01), overwrite (`onExists`), import conflicts (`onConflict`, `idPolicy`, `colours`), repair selection
  (`doc.repair`), relinking (`doc.asset.relink`). Load and validation problems are returned as structured issues, never
  as message boxes.
- **AG-08 Verification loop.** `doc.validate` returns issues with machine-applicable `fixes` (a command id and params);
  agents fix by running those commands or `doc.repair`, then re-validate (11 §12 `review_and_fix`).
- **AG-09 Change tracking.** Agents read `documentRev` from every envelope, subscribe to `doc.changed` (filtering
  `origin`), and use `doc.diff {sinceRev}` to catch up after reconnecting. The server retains change history at least
  back to the last save and at least the last 10 000 transactions; older bases fail with `doc.diffUnavailable`.
- **AG-10 Files.** Paths given to `doc.export.json`, `doc.import.json`, `doc.asset.*` and `doc.package.*` are subject to
  the file-system roots of 11 §6.2.

### 10.2 Command table

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `doc.object.get` | query | Read one or more objects as object-model JSON (= `get_object`) | `ref?: id; refs?: [id]; depth: int = 1 (-1 = all, 0–20); fields?: [string]; include?: [enum "derived"\|"stats"]; page?: object {pointer: string, offset: int = 0, limit: int = 1000}` | `{object}` or `{objects: [object]}`; paged arrays carry `{total, nextOffset}` | n/a |
| `doc.object.find` | query | Find objects by type, name and predicate (= `find_objects` / `query`) | `type?: string; within?: id; name?: string (exact or glob); where?: object (§10.3); limit: int = 100 (1–1000); offset: int = 0; fields?: [string]; sort?: enum "path"\|"name"\|"rev"\|"createdAt" = "path"` | `{items: [{id, type, name, path, rev, …fields}], total, nextOffset}` | n/a |
| `doc.object.resolve` | query | Resolve paths to IDs and IDs to canonical paths | `refs: [string]` | `{items: [{ref, id, type, name, path, rev} \| {ref, error}]}` | n/a |
| `doc.object.refs` | query | List objects that reference an object (incoming) or that it references (outgoing) | `ref: id; direction: enum "incoming"\|"outgoing" = "incoming"; limit: int = 500` | `{refs: [{id, type, path, property}], total}` | n/a |
| `doc.object.patch` | action | Generic validated property patch on one or more objects (= `set_properties`) | `targets: [id] (1–1000); props?: object; ops?: [object {op: enum "add"\|"remove"\|"replace"\|"move"\|"copy"\|"test", path: string, value?: any, from?: string}]; expectRev?: int \| object; label?: string; dryRun: bool = false` | `{changes: [change], documentRev}` | yes |
| `doc.schema.list` | query | List object types with their parents and addressing | — | `{formatVersion, types: [{type, aliases[], parents[], nameAddressable, description}]}` | n/a |
| `doc.schema.get` | query | JSON Schema of an object type (or of one property) | `type: string; property?: string (JSON Pointer)` | `{type, formatVersion, schema}` | n/a |
| `doc.schema.validate` | query | Validate a candidate object or bundle against the schema and semantic rules without applying it | `type: string; object: object; mode: enum "create"\|"replace" = "replace"` | `{valid: bool, errors: [{pointer, code, message}]}` | n/a |
| `doc.state.get` | query | Document state | — | `{projectId, path, form, formatVersion, migratedFrom, documentRev, savedRev, dirty, readOnly, lock, historyFromRev}` | n/a |
| `doc.diff` | query | Net changes since a revision or checkpoint (= `doc_diff`) | `sinceRev?: int; sinceCheckpoint?: id; untilRev?: int; within?: id; types?: [string]; detail: enum "summary"\|"full" = "full"; limit: int = 1000` | `{fromRev, toRev, changes: [change], truncated}` | n/a |
| `doc.validate` | query | Lint the project or a subtree (§10.3) | `within?: id; checks?: [string] (default all); minSeverity: enum "info"\|"warning"\|"error" = "info"; limit: int = 500` | `{documentRev, issues: [issue], counts: {error, warning, info}, truncated}` | n/a |
| `doc.repair` | action | Apply the default fix of selected issues ("Repair Project") | `issueIds?: [id]; codes?: [string]; within?: id; all: bool = false; dryRun: bool = false` | `{fixed: [issueId], skipped: [{issueId, reason}], changes: [change]}` | yes |
| `doc.loadReport.get` | query | The report of the last open (§7.3) | — | `LoadReport` | n/a |
| `doc.export.json` | query | Export the project, scenes or elements as an interchange bundle (§6.8), inline or to a file | `scope: enum "project"\|"scenes"\|"elements"; refs?: [id]; path?: path; binaries: enum "embed"\|"reference"\|"omit" = "reference"; pretty: bool = true; onExists: enum "fail"\|"overwrite" = "fail"` | `{bundle?: object, path?: path, bytes, objectCount}` | n/a |
| `doc.import.json` | action | Import an interchange bundle as new scenes or into a scene ("Import JSON") | `source: path \| object; target?: id (scene; omit = new scenes); index?: int (scene position); atFrame: int = 1; idPolicy: enum "new"\|"preserve" = "new"; onConflict: enum "rename"\|"replace"\|"fail" = "rename"; colours: enum "keepExisting"\|"updateExisting" = "keepExisting"; dryRun: bool = false` | `{created: {scenes[], elements[], drawings[], palettes[], assets[]}, renamed: [{id, from, to}], recoveredColourIds[], issues[]}` | yes |
| `doc.blob.get` | query | Read a binary payload (asset file, bitmap node pixels, texture) as base64 | `ref?: id; entry?: string; maxBytes: int = 8388608 (1–67108864)` | `{mediaType, size, sha256, base64}` | n/a |
| `doc.asset.list` | query | List assets with storage, status and references | `kind?: enum "sound"\|"image"\|"movie"\|"vector"; status: enum "all"\|"missing"\|"changed"\|"linked"\|"embedded"\|"unused" = "all"` | `{assets: [{id, name, kind, storage, status, path?, size, refs: int}]}` | n/a |
| `doc.asset.relink` | action | Point linked assets to a new file, or search a folder for missing ones ("Relink Media"); generic form of `sound.asset.relink` / `media.object.relink` (07) | `assetId?: id; path?: path; searchFolder?: path; recursive: bool = true; matchBy: enum "name"\|"nameAndSize"\|"hash" = "nameAndSize"` | `{relinked: [{assetId, path}], stillMissing: [id]}` | yes |
| `doc.asset.embed` | action | Copy linked files into the package ("Embed Media") | `assetIds?: [id]; all: bool = false` | `{embedded: [id], bytes}` | yes |
| `doc.asset.export` | query | Write an asset's file to disk (e.g. to share or edit externally) | `assetId: id; path: path; onExists: enum "fail"\|"overwrite" = "fail"` | `{path, bytes}` | n/a |
| `doc.asset.remove` | action | Remove assets; only unreferenced ones unless `force` ("Remove Media") | `assetIds?: [id]; unused: bool = false; force: bool = false` | `{removed: [id], bytes, detachedRefs: [id]}` | yes |
| `doc.package.export` | job | Write a copy of the current project in package or folder form without changing the open project's location | `path: path; form: enum "package"\|"folder" = "package"; onExists: enum "fail"\|"overwrite" = "fail"` | `{jobId}` → `{path, form, bytes, entries}` | n/a |
| `doc.package.get` | query | Inspect a package, folder, template or palette file on disk without opening it | `path: path; validate: bool = false` | `{kind: enum "project"\|"template"\|"palette"\|"library", form, formatVersion, minReaderVersion, readable, projectId?, name, savedAt, entryCount, issues[]}` | n/a |
| `doc.changed` | event | A transaction was committed (payload defined in 11 §9.2 and §2.6) | — | `{undoStepId, documentRev, label, origin, changes: [change]}` | n/a |
| `doc.asset.status.changed` | event | Assets became missing, changed or available again | — | `{assets: [{assetId, status}]}` | n/a |
| `doc.package.changedOnDisk` | event | The open project's file or folder was modified by another process | — | `{path, detectedAt}` | n/a |

Project open, save, save-as, recovery and backup commands are `project.*` (01); palette import/export are `colour.*`
(03); template and library commands are `library.*` (08).

### 10.3 Queries and events

**Find predicates.** `where` is an object whose keys are dotted property paths (relative to the object, e.g.
`elementType`, `transform.offset.z`, `camera.fov`, `meta.status`) and whose values are either a literal (equality) or
`{op, value}` with `op` ∈ `eq`, `ne`, `lt`, `lte`, `gt`, `gte`, `in`, `nin`, `glob`, `regex`, `exists`, `contains`
(array membership). All keys must match (AND); `anyOf: [where, …]` expresses OR. Examples:
`{type: "element", where: {elementType: {op: "in", value: ["drawing", "image"]}, locked: false}}`;
`{type: "drawing", within: "scene:\"Intro\"", where: {mouthShape: "A"}}`;
`{type: "stroke", within: "…/drawing:\"3\"", where: {colourId: "col_skin"}}`.

**Lint checks** (`doc.validate`; each issue carries `fixes` where an automatic or suggested fix exists):

| Check | Severity | Finds | Suggested fix |
|---|---|---|---|
| `missingMedia` | warning | Linked or embedded assets that cannot be read | `doc.asset.relink` |
| `missingDrawingContent` | error | Drawings whose content entry was missing or unreadable | — (re-import or delete: `element.drawing.delete`) |
| `emptyExposure` | warning | Cell-based elements with no exposed cell in the scene | `element.delete` or expose drawings (04) |
| `exposedEmptyDrawing` | info | Exposed drawings with no content | — |
| `unusedDrawings` | info | Drawings exposed by no element of their clone group and not used by a lip-sync mapping | `element.drawing.delete` |
| `unresolvedColours` | error | Colour IDs used by strokes, zone fills, text or pens that no palette defines | `colour.recovery.run` |
| `unpaintedZones` | info | Closed zones bounded by visible strokes with no fill | `colour.paint.apply` (agent chooses `colourId`) |
| `unusedSwatches` | info | Swatches used by no drawing, pen or template in the project | `colour.swatch.delete` |
| `unusedAssets` | info | Assets referenced by nothing | `doc.asset.remove {unused: true}` |
| `danglingReferences` | error | References that do not resolve | `doc.repair` |
| `duplicateNames` | error | Names violating uniqueness (e.g. after a version-control merge) | rename commands of the owning area |
| `noCamera` | warning | Scenes without a camera or with no current camera | `anim.camera.create` / `anim.camera.current.set` |
| `staleLipSync` | warning | Mouth tracks whose placement timing or frame rate changed | `lipsync.detect` |
| `incompleteLipSyncMapping` | info | Mappings with unmapped mouth shapes | `lipsync.mapping.set` |
| `transitionPadding` | warning | Scenes padded with blank frames to fit their transitions (08 §16.5) | change the transition or scene length |
| `keysOutsideDuration` | warning | Keys beyond an element's bar | `anim` timing commands (05) |
| `missingFonts` | info | Text using fonts not installed | — |
| `brokenLibraryItems` | warning | Library items whose files are missing | `library` commands (08) |
| `outdatedTemplateLinks` | info | Linked template usages whose library version is newer | template refresh (08 §7.2) |

**Events.** `doc.changed` is emitted once per committed transaction (including load repairs, undo and redo; 11 also
emits `doc.undone` / `doc.redone`). Area events (e.g. `element.drawing.changed`, `colour.recovery.performed`) are
emitted in addition, after `doc.changed` for the same transaction.

### 10.4 JSON shapes

```json
Change = {
  "id": "el_01JA…", "type": "element", "op": "created" | "modified" | "deleted", "rev": 1043,
  "parentId": "sc_01J2…", "index": 0,
  "patch": [ { "op": "replace", "path": "/name", "value": "Robert", "oldValue": "Bob" } ]
}

Issue = {
  "issueId": "is_7", "code": "unusedDrawings", "severity": "info",
  "objectId": "dr_01JB…", "path": "scene:\"Intro\"/element:\"Bob\"/drawing:\"7\"",
  "message": "Drawing 7 of Bob is not exposed in any cell",
  "details": { "bankId": "db_01J9…" },
  "fixes": [ { "fixId": "fx_1", "description": "Delete the drawing",
               "command": "element.drawing.delete", "params": { "elementId": "el_01JA…", "drawingIds": ["dr_01JB…"] },
               "default": true } ]
}

LoadReport = {
  "projectId": "pr_01JA…", "path": "C:/Projects/Short.animpkg", "form": "package",
  "formatVersion": "1.0", "migratedFrom": null, "readOnly": false, "durationMs": 840,
  "repairedCount": 2, "issues": [ /* Issue */ ]
}
```

Example `doc.object.get {ref: "scene:\"Intro\"/element:\"Bob\"", depth: 1}` (abridged):

```json
{ "id": "el_01JA…", "type": "element", "rev": 418, "name": "Bob", "path": "scene:\"Intro\"/element:\"Bob\"",
  "elementType": "drawing", "start": 1, "duration": 48, "loops": 1, "loopType": "count", "positionLoop": "cycle",
  "role": "normal", "deformable": false, "visibleInScene": true, "visibleInXsheet": true, "locked": false,
  "collapsed": false, "markerColour": null, "backgroundColour": null, "note": "", "columnWidth": 80,
  "drawingBankId": "db_01J9…",
  "exposure": [ ["dr_01JA…", 6], ["dr_01JB…", 6], ["dr_01JA…", 36] ],
  "cellNotes": { "3": "fix hand" },
  "transform": { "offset": {"x": 0, "y": -1.5, "z": 0}, "rotation": 0, "skew": 0, "scale": {"x": 1, "y": 1},
                 "keepProportions": false, "rotationPivot": {"x": 0, "y": 0}, "scalePivot": {"x": 0, "y": 0},
                 "rotateAboutPivot": false, "lockDepth": false, "orientToPath": false },
  "channels": { "rotation": { "id": "ch_01JC…", "type": "channel", "name": "rotation" } },
  "createdAt": "2026-10-17T08:12:00.000Z", "meta": {}, "ext": {} }
```

---

## 11. Design decisions / open questions

| # | Topic | Decision to make | Recommended default |
|---|---|---|---|
| D1 | ID format | Shape of opaque IDs | Type prefix + ULID (128 bits), unique across projects (ID-02). |
| D2 | Persisting zone fills | Store fills on stroke sides or as zone records with seed points | Stroke sides with the consistency rule of §4.9; zones derived. Revisit if section 02 requires stable zone IDs. |
| D3 | Exposure encoding | Per-frame list or runs | Runs with canonical form (§4.7); per-frame access via `cell:#n`. |
| D4 | Scene length | Stored or derived | Derived from content (04 §11.7); no stored length. |
| D5 | Per-element display state | Marker/background colours, column width and collapsed state in the document or per user | In the document (they travel with the project and are undoable). |
| D6 | Element `type` naming | The object envelope uses `type` for the object type | Resolved: envelope keeps `type: "element"`; the element kind is `elementType` everywhere, including 04's `element.list` / `element.get` read models. Command parameters (e.g. `element.create {type}`) may keep the short name. |
| D7 | Converting forms | How a user turns a package into a folder project | Add a `form: "package" \| "folder"` parameter to `project.saveAs` (01); until then `doc.package.export` + open. Coordinate with 01. |
| D8 | Binary sidecars for large drawings | Allow binary point arrays next to drawing JSON | Not in 1.0: JSON only (deflate compresses it well); reconsider with profiling (a minor version can add an optional sidecar encoding). |
| D9 | Numeric precision | Quantise geometry to reduce diff noise | No quantisation in the format; tools MAY round generated geometry to 1/1024 du. |
| D10 | Version-control merges | Merging two edited copies of a folder-form project | File-level merges only; after a merge the application validates and repairs on open (duplicate IDs, dangling references, duplicate names). No built-in three-way merge. |
| D11 | Change-history retention for `doc.diff` | How far back diffs are available | At least to the last save and the last 10 000 transactions (AG-09); diffs across sessions from persisted `rev` values (objects changed since a revision), without patch details older than the session. |
| D12 | Unknown enum values from newer minor versions | Read-only objects vs. dropping | Objects stay read-only and are preserved (VER-02). |
| D13 | Lock heartbeat | Interval and staleness | 60 s heartbeat, stale after 5 minutes (LOCK-02). |
| D14 | Media type strings | Registered media types | Placeholder `application/x-anim…+zip` until registered. |
| D15 | Autosave snapshot cost | Main-thread budget for very large projects | Copy-on-write document model so snapshots are O(changed objects); if the budget is exceeded, autosave postpones and retries. |
| D16 | Embedded vs. linked default for imported movies | Copy movies into the package or link them | Link (movies are large; 04 §2.5); `doc.asset.embed` available. Sound and images are embedded (07 §2.3). |
