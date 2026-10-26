# Spec issues

Ambiguities, contradictions or gaps found in `spec/` during implementation. One entry per issue:
section, quote, problem, decision taken (ADR link) or question for the spec owner.

Review C1.1 round 1 found SI-009, SI-010 and SI-012 to be misreadings and they were removed (their numbers are not
reused); the decisions they described are now justified from the spec directly in ADR-0010 §1 and §14.

## SI-001 — 09 §4.4 "≈ CRF 18 equivalent" under platform H.264 encoders (found in C0.4; open)

- **Section:** 09 §4.4 Video settings.
- **Quote:** "Quality | high (≈ CRF 18 equivalent) | low / medium / high / lossless where supported, or numeric
  0–100".
- **Problem:** user decision 3 (plan §6) ships only platform H.264 encoders (`h264_mf`, `h264_videotoolbox`) in an LGPL
  FFmpeg sidecar. CRF is a rate-control mode of x264, which is GPL and excluded. Neither platform encoder implements
  CRF, so "≈ CRF 18 equivalent" can only be met as a quality target, not as x264 CRF semantics.
- **Decision taken (ADR-0008):** M10 C10.1 maps low / medium / high / 0–100 to each encoder's quality-based mode
  (Media Foundation quality, VideoToolbox constant quality or bit rate) and measures the mapping in experiment
  E-FF1.
- **Question for the spec owner (open):** confirm that "≈ CRF 18 equivalent" means a visually comparable quality
  target.
- **Settled by ruling C04-R1-03 (not part of the open question):** "lossless where supported" is met by PNG image
  sequences and by FFV1 (FFmpeg's native LGPL encoder) in MKV, and in MOV once verified. ProRes variants, 4444
  included, are labelled "visually lossless". Lossless output is verified in M10 by decoded-pixel comparison against
  the rendered frames (ADR-0008 §1).

## SI-002 — reserved

Reserved by the M0 closure for "measured deterministic-format incompatibility" (ADR-0008, M10 C10.1).

## SI-003 — element `role` values disagree (found in C1.1; decided)

- **Sections:** 10 §4.5 (`role`: `normal`, `background`); 04 §6.1 and 05 §8.1 (Normal, Foreground, Background,
  Envelope); `anim.node.options.set` in A_command_registry (`role?: enum "normal"|"background"`).
- **Decision taken (ADR-0010):** the persisted enum is the superset `normal`, `foreground`, `background`,
  `envelope`, so 04/05's roles can be stored. Commands that accept only two values keep their narrower parameter
  until the owner rules; how a two-value command parameter maps onto the four persisted roles must then be stated
  explicitly (review C1.1 r1).
- **Question:** confirm the four roles belong in 10 §4.5 and `anim.node.options.set`.

## SI-004 — properties named `type` collide with the envelope `type`; pen fields (found in C1.1; decided)

- **Sections:** 10 §4.4 / 08 §13.1 transition `type`; 06 §8.4 block `type` (particle type); 02 §6.1 pen "Type"
  (brush/eraser) and "Tip angle", absent from 10 §4.2; 10 §4.2 names the tip `tip`, 02 §6.1's command `tipShape`;
  smoothness is `int` in 10 §4.2 and `number` in 02 §6.1.
- **Decision taken (ADR-0010, following D6 for `elementType`):** persisted names `transitionType`, `particleType`,
  pen `kind` (`brush`/`eraser`) and pen `tipAngle` (0–360, default 0); `tip` as in 10; smoothness `int` 0–10 as in 10
  (10 owns the persisted format). Command parameters may keep the short names; constructor and command tests (C1.2
  on) must keep the persisted and the parameter names distinct (review C1.1 r1).

## SI-005 — persisted properties defined outside section 10 (found in C1.1; decided)

- **Sections:** 03 §1.3 palette `hidden` (internal palettes); 03 §1.2 project-level automatic colour update flag.
- **Decision taken:** palettes persist `hidden` (bool, default false) and the project `autoColourUpdate` (bool,
  default false); otherwise they would be lost on save.

## SI-006 — motion points are objects with IDs but missing from 10 §2.1 (found in C1.1; decided)

- **Sections:** 10 §4.12 (`motionPoint: { id, frame, … }`), 11 §5.1 (motion points among objects with stable ids),
  10 §2.1 (no `motionPoint` row).
- **Decision taken (ADR-0010):** `motionPoint` is an addressable type below the `position` channel (container-local
  IDs like keys); `motionPoint:#n` / `motionPoint:@id`; `key:"n"` on the position channel selects the keyframe
  motion point at frame n.

## SI-007 — particle effect names (found in C1.1; decided)

- **Sections:** 06 §3.8.1 / §8.4 call the particle effects `natureParticles` / `actionParticles`; 10 §4.5 uses
  `particleNature` / `particleImpact`.
- **Decision taken:** 10's element type names are persisted. Particle effects have no effect-level parameters
  (`params: {}`).
- **Not a conflict (review C1.1 r1):** 06 §3.4's drop-shadow transform `{xAxis, yAxis, origin}` and 06 §7's "2×3
  matrix" describe the same affine transform; `params.transform` uses the §3.4 shape.

## SI-008 — `importOptions` of image elements is undefined (found in C1.1; open)

- **Section:** 10 §4.6 ("plus `importOptions` (section 09)"); section 09 defines no such object.
- **Decision taken:** an opaque JSON object until section 09 (M10) defines it: never interpreted, no string inside it
  normalised, only its keys written in canonical order (ADR-0011 §3; review C1.1 r1 finding 03).

## SI-011 — meaning of `depth` in `doc.object.get` (found in C1.1; decided)

- **Section:** 10 §10.2 (`depth: int = 1 (-1 = all, 0–20)`), 11 §7.4; only the depth-1 example of 10 §10.4 defines it.
- **Problem (refined by review C1.1 r1):** the depth-1 example and the stub rules support a counting convention, but
  depth 0 is not clearly defined.
- **Decision taken (ADR-0010):** depth counts object levels returned in full: 1 = the object with child objects as
  stubs (matches the example), n = n levels, 0 = the object itself as a stub, -1 = all. Value objects are complete
  at every depth. Every result, the depth-0 stub included, validates against the type's schema (finding 11).

## SI-013 — brush strokes with `closed: false` in section 02's example (found in C1.1 r1; decided)

- **Sections:** 10 §4.9 stroke table: "`closed` | bool | false | Brush outlines are closed."; 02's internal stroke
  example (02 §13.4 "JSON shapes") shows `{"id": "b_7", "kind": "brush", "closed": false, …}`.
- **Problem:** 10 states the invariant for brush strokes; 02's example contradicts it (there the brush points are
  the spine, and `bezier` may be `null` once the outline was edited).
- **Decision taken (ADR-0010 §15):** 10 owns the persisted format, so a persisted brush stroke has `closed: true`;
  `closed: false` with `kind: "brush"` is rejected. 02's spine is the stroke's `spine`.
- **Question:** confirm 02's example predates the 10 §4.9 stroke format.

## SI-014 — order of map keys in canonical JSON (found in C1.1 r1; decided)

- **Sections:** 10 JSON-02 (keys in schema order; unknown keys last in lexicographic order), JSON-05, AG-06; maps
  such as 10 §4.3 `paletteStyles`, 10 §4.13 swatch `values` and the content of `meta`/`ext`.
- **Problem:** JSON-02 orders an object's properties by its schema; a map's keys have no schema order, yet JSON-05/
  AG-06 require equal documents to give identical bytes.
- **Decision taken (ADR-0011 §3):** map keys and the keys of objects inside opaque values are written in
  lexicographic (code point) order, the order JSON-02 already uses for unknown keys.

## SI-015 — `include: ["stats"]` of `doc.object.get` is undefined (found in C1.2; decided)

- **Section:** 10 §10.2 `doc.object.get` (`include?: [enum "derived"|"stats"]`); no section defines "stats".
- **Problem:** the result is object-model JSON that validates against the type schema (10 §2.8), so statistics cannot
  be added inside the object.
- **Decision taken (ADR-0013 §9):** with `stats`, the result carries `stats: [{id, jsonBytes, objectsByType}]`, one
  per returned object, beside `object`/`objects`.
- **Question:** confirm or define the intended statistics.

## SI-016 — event entries put their payload in different columns (found in C1.2; decided)

- **Section:** A_command_registry: `app.state.changed`, `project.*` and `lipsync.*` events list the payload under
  *Parameters* with *Result* "—"; `doc.changed`, `job.*`, `agent.*` events list it under *Result* with *Parameters*
  "—".
- **Decision taken (ADR-0013 §1):** every event is registered with no parameters and its payload as the result
  schema; `describe_command` shows the spec's columns verbatim.
- **Question:** normalise the columns in spec A.

## SI-017 — base and form of batch step references (found in C1.2; decided)

- **Section:** 11 §7.3 ("Steps may reference earlier results: `"$2.result.strokeId"` or a named step
  `"$walk.result.elementId"`"), 11 §5.1 (`$<n>.<field>`).
- **Problem:** whether `n` counts from 0 or 1, what a path segment may be, and whether a reference may be part of a
  longer string are not stated.
- **Decision taken (ADR-0013 §6):** `n` is 1-based (the batch error reports `stepIndex` 1-based too); the path is
  dotted and walks the step's envelope (`result`, `targets`, `changes.0.id`, …; numeric segments index arrays); only a
  string that is exactly a reference is replaced; a reference to a missing step or value is `arg.invalid`.
- **Question:** confirm 1-based numbering.

## SI-018 — `@current` as the start of a path (found in C1.2; decided)

- **Section:** 11 §5.1 ("`@current` (current scene / element / frame / drawing as appropriate)"), 10 §2.4 (special
  references MAY be the first segment).
- **Problem:** "as appropriate" depends on the parameter's type, which a path start does not have.
- **Decision taken (ADR-0013 §7):** a bare `@current` given to a typed parameter is the current object of that type;
  as a path start it is the current element, else the current scene. `@selection` is the single selected object, and
  list parameters expand it; `@activeView` is what the central view shows.
- **Question:** confirm.

## SI-019 — main-toolbar item names (found in C1.2; decided)

- **Section:** 01 §5.7 ("Every toolbar item is a command reference (command ID plus optional fixed parameters)"),
  01 §12.1 `window.toolbar.items.set` (`items: [string] (command references, group IDs, "separator", "space",
  "flexibleSpace")`).
- **Problem:** group IDs are not listed and the string form of a command reference with fixed parameters is not
  defined; several default items (grid type, overlay, first/last frame, play/stop, the drop-downs) are not single
  commands.
- **Decision taken (ADR-0013 §8):** items are registered command ids or the group ids `gridType`, `gridOverlay`,
  `onionSkinPrevious`, `onionSkinNext`, `firstFrame`, `previousFrame`, `playStop`, `nextFrame`, `lastFrame`,
  `cameraList`, `sceneList`, `workspaceList`, plus the three spacers. Fixed parameters are not expressible yet.
- **Question:** define group IDs and the fixed-parameter form.

## SI-020 — error codes the executor needs that 11 §10 does not list (found in C1.2; decided)

- **Section:** 11 §10 (generic codes), 11 §7.7 (checkpoints, jobs), 11 §14.6 (never-block watchdog).
- **Decision taken (ADR-0013):** `command.notImplemented` (registered command implemented by a later milestone;
  `details.milestone`), `command.timeout` (watchdog budget exceeded; the outcome is indeterminate: the call may still complete, nothing is rolled back or cancelled, and the caller reads `doc.state.get` before retrying),
  `history.diverged` (a replayed undo step does not match the document; nothing applied), `checkpoint.exists`,
  `checkpoint.notFound`, `job.notFound`; round 1 (ruling R1-22): `request.conflict` (a `requestId` reused with a
  different command, parameters or options; `{requestId, originalCommand}`; never executed) and `request.unknownId`
  (an id whose remembered envelope was evicted from the bounded per-session cache; `{requestId, reason:
  "evicted"}`; never re-executed); warning `edit.optionNotApplied` (a cut/paste-special option that does not apply
  or needs a later milestone). 11 §7.2 says only that a repeated id returns the original result.
- **Question:** add them to 11 §10, and state in 11 §7.2 what a reused id with other parameters and an id beyond the
  remembered window return.

## SI-021 — `elements` clipboard items by reference (found in C1.2 r1; decided)

- **Section:** 01 §12.4 (`Clipboard`: "objects in the section-10 schema for this kind"), 01 §12.1
  (`edit.clipboard.set` validates against the schema of its kind).
- **Decision taken (ADR-0013 §19):** an `elements` item is either a full `element` object (what cut and copy put
  there) or a reference `{elementId}` to an element of the open project, which pastes a copy of that element. Agents
  can thus place elements on the clipboard without sending whole objects. Every other kind takes the section-10 (or
  §12.4) shapes only.
- **Question:** allow references for other kinds too, or forbid them for elements.

## SI-022 — references to deleted elements (found in C1.2 r1; decided)

- **Section:** 04 §8.3 (delete element: drawings and children), 10 §7.2 (dangling-reference repair on load).
- **Decision taken (ADR-0013 §19):** deleting elements (`edit.delete`, `edit.cut`) repairs references other elements
  of the scene hold to them as the load repair of 10 §7.2 does: a reference inside a list item (e.g. a lip-sync
  mapping) removes the item, an ID listed directly is removed, any other reference becomes `null`; the repair is part
  of the same undo step. Drawing banks stay in the document until save purges unused ones (04 §8.3, "purged when the
  project is saved"), so a cut element pastes back with its drawings.
- **Question:** confirm the repair rule for deletions, or name per-reference behaviour.

<!-- SI-023..: generated from crates/registry/src/declared.json placeholders (target of scripts/gen-command-declarations.py) -->

## SI-023 — `anim.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 05 and spec A (`anim.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `anim.function.preset.apply` `/result/segments`: a member whose type is not stated (`segments`)
  - `anim.node.get` `/result/static`: a member whose type is not stated (`static`)
  - `anim.prefs.set` `/result/prefs`: a member whose type is not stated (`prefs`)
  - `anim.projection.get` `/params/points/*`: an object whose fields are not listed
  - `anim.view.coords.get` `/params/points/*`: an object whose fields are not listed
  - `anim.view.coords.get` `/result/points/*`: an object whose fields are not listed
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-024 — `capture.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 09 and spec A (`capture.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `capture.device.get` `/result/settingsSchema`: a member whose type is not stated (`settingsSchema`)
  - `capture.device.settings.set` `/params/values`: an object whose fields are not listed
  - `capture.device.settings.set` `/result/applied`: an object whose fields are not listed
  - `capture.preview.get` `/result/capturedAt`: a member whose type is not stated (`capturedAt`)
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-025 — `colour.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 03 and spec A (`colour.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `colour.fillMapping.set` `/params/mapping`: an object whose fields are not listed
  - `colour.palette.create` `/params/swatches/*`: an object whose fields are not listed
  - `colour.palette.duplicate` `/result/idMap`: an object whose fields are not listed
  - `colour.palette.set` `/params/palette`: an object whose fields are not listed
  - `colour.swatch.create` `/params/value`: an object whose fields are not listed
  - `colour.swatch.create` `/params/values`: an object whose fields are not listed
  - `colour.swatch.list` `/result/swatches/*/value`: an object whose fields are not listed
  - `colour.swatch.set` `/params/values`: an object whose fields are not listed
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-026 — `draw.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 02 and spec A (`draw.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `draw.content.add` `/params/content`: an object whose fields are not listed
  - `draw.content.add` `/result/idMap`: an object whose fields are not listed
  - `draw.current.get` `/result/activeView`: a member whose type is not stated (`activeView`)
  - `draw.drawing.get` `/$defs/spec.DrawingReadback/nodes/*/strokes/*/textureMatrix`: a member whose type is not stated (`textureMatrix`)
  - `draw.drawing.get` `/$defs/spec.DrawingReadback/nodes/*/zones/*/textureMatrix`: a member whose type is not stated (`textureMatrix`)
  - `draw.erase.apply` `/params/pen`: an object whose fields are not listed
  - `draw.hitTest.get` `/result/hits/*`: an object whose fields are not listed
  - `draw.text.add` `/params/format`: an object whose fields are not listed
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-027 — `drawAid.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 02 and spec A (`drawAid.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `drawAid.changed` `/result/state`: an object whose fields are not listed
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-028 — `element.*`: shapes the owning section leaves undefined (found in C1.2 r2; resolved)

- **Resolved in C1.2 r3 (C12-R3-01):** every position of `element.*` is now typed from its owning section or its sibling commands; no placeholder remains.

## SI-029 — `export.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 09 and spec A (`export.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `export.formats.list` `/result/formats/*/video`: a member whose type is not stated (`video`)
  - `export.formats.list` `/result/formats/*/audio`: a member whose type is not stated (`audio`)
  - `export.formats.list` `/result/formats/*/settingsSchema`: a member whose type is not stated (`settingsSchema`)
  - `export.movie.defaults.get` `/result`: a type the section does not define (ExportMovieParams)
  - `export.movie.defaults.set` `/params/values`: an object whose fields are not listed
  - `export.movie.defaults.set` `/result`: a type the section does not define (ExportMovieParams)
  - `export.movie.run` `/params/settings`: an object whose fields are not listed
  - `export.movie.validate` `/params/settings`: an object whose fields are not listed
  - `export.publish.run` `/params/export`: an object whose fields are not listed
  - `export.publish.validate` `/params/export`: an object whose fields are not listed
  - `export.publish.validate` `/params/metadata`: an object whose fields are not listed
  - `export.report.get` `/result/cameraSize`: a member whose type is not stated (`cameraSize`)
  - `export.settings.get` `/result/settings`: an object whose fields are not listed
  - `export.settings.set` `/params/settings`: an object whose fields are not listed
  - `export.settings.set` `/result/settings`: an object whose fields are not listed
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-030 — `fx.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 06 and spec A (`fx.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `fx.block.list` `/result/blocks/*`: a type the section does not define (BlockSummary)
  - `fx.block.set` `/params/values`: an object whose fields are not listed
  - `fx.create` `/params/params`: an object whose fields are not listed
  - `fx.dropShadow.transform.set` `/result/transform`: a member whose type is not stated (`transform`)
  - `fx.keyframe.add` `/params/values`: an object whose fields are not listed
  - `fx.keyframe.list` `/result/keyframes/*/values`: an object whose fields are not listed
  - `fx.params.get` `/result/values`: an object whose fields are not listed
  - `fx.params.get` `/result/derived`: an object whose fields are not listed
  - `fx.params.set` `/params/values`: an object whose fields are not listed
  - `fx.params.set` `/result/values`: an object whose fields are not listed
  - `fx.preset.apply` `/result/values`: an object whose fields are not listed
  - `fx.preset.create` `/$defs/spec.Preset/static`: an object whose fields are not listed
  - `fx.preset.create` `/$defs/spec.Preset/dynamic`: an object whose fields are not listed
  - `fx.preset.get` `/$defs/spec.Preset/static`: an object whose fields are not listed
  - `fx.preset.get` `/$defs/spec.Preset/dynamic`: an object whose fields are not listed
  - `fx.renderList.get` `/result/items/*`: a type the section does not define (RenderItem)
  - `fx.renderer.options.changed` `/result/values`: an object whose fields are not listed
  - `fx.renderer.options.get` `/result/values`: an object whose fields are not listed
  - `fx.renderer.options.set` `/params/values`: an object whose fields are not listed
  - `fx.renderer.options.set` `/result/values`: an object whose fields are not listed
  - `fx.types.get` `/$defs/spec.TypeDescriptor/particleTypes`: a member whose type is not stated (`particleTypes`)
  - `fx.types.list` `/$defs/spec.TypeDescriptor/particleTypes`: a member whose type is not stated (`particleTypes`)
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-031 — `import.*`: shapes the owning section leaves undefined (found in C1.2 r2; resolved)

- **Resolved in C1.2 r3 (C12-R3-01):** every position of `import.*` is now typed from its owning section or its sibling commands; no placeholder remains.

## SI-032 — `keymap.*`: shapes the owning section leaves undefined (found in C1.2 r2; resolved)

- **Resolved in C1.2 r3 (C12-R3-01):** every position of `keymap.*` is now typed from its owning section or its sibling commands; no placeholder remains.

## SI-033 — `library.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 08 and spec A (`library.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `library.item.get` `/result`: a type the section does not define (ItemDetails)
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-034 — `lipsync.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 07 and spec A (`lipsync.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `lipsync.apply` `/params/mapping`: an object whose fields are not listed
  - `lipsync.detect` `/params/phonemeMap`: an object whose fields are not listed
  - `lipsync.settings.get` `/result/phonemeMap`: an object whose fields are not listed
  - `lipsync.settings.set` `/params/phonemeMap`: an object whose fields are not listed
  - `lipsync.settings.set` `/result/phonemeMap`: an object whose fields are not listed
  - `lipsync.shapes.list` `/result/shapes/*/referenceImage`: a member whose type is not stated (`referenceImage`)
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-035 — `playback.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 09 and spec A (`playback.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `playback.player.get` `/result`: a type the section does not define (PlayerState)
  - `playback.player.set` `/result`: a type the section does not define (PlayerState)
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-036 — `prefs.*`: shapes the owning section leaves undefined (found in C1.2 r2; resolved)

- **Resolved in C1.2 r3 (C12-R3-01):** every position of `prefs.*` is now typed from its owning section or its sibling commands; no placeholder remains.

## SI-037 — `print.*`: shapes the owning section leaves undefined (found in C1.2 r2; resolved)

- **Resolved in C1.2 r3 (C12-R3-01):** every position of `print.*` is now typed from its owning section or its sibling commands; no placeholder remains.

## SI-038 — `sound.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 07 and spec A (`sound.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `sound.scrubbing.set` `/result/effective`: a member whose type is not stated (`effective`)
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-039 — `storyline.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 08 and spec A (`storyline.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `storyline.selection.set` `/result/selection`: a member whose type is not stated (`selection`)
  - `storyline.transition.types.list` `/result/types/*/parameters/*/default`: a member whose type is not stated (`default`)
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-040 — `tool.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 02 and spec A (`tool.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `tool.list` `/result/tools/*/options`: an object whose fields are not listed
  - `tool.list` `/result/tools/*/optionSchema`: an object whose fields are not listed
  - `tool.options.changed` `/result/options`: an object whose fields are not listed
  - `tool.options.get` `/result/options`: an object whose fields are not listed
  - `tool.options.set` `/params/options`: an object whose fields are not listed
  - `tool.options.set` `/result/options`: an object whose fields are not listed
  - `tool.pen.create` `/params/settings`: an object whose fields are not listed
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-041 — `ui.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 01 and spec A (`ui.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `ui.message.shown` `/result/details`: a member whose type is not stated (`details`)
  - `ui.statusBar.get` `/result/pointer`: a member whose type is not stated (`pointer`)
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-042 — `view.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 01 and spec A (`view.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `view.list` `/result/*/transform`: a member whose type is not stated (`transform`)
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-043 — `xsheet.*`: shapes the owning section leaves undefined (found in C1.2 r2; resolved)

- **Resolved in C1.2 r3 (C12-R3-01):** every position of `xsheet.*` is now typed from its owning section or its sibling commands; no placeholder remains.

<!-- end of generated placeholder issues -->

## SI-044 — "Repair on Open" cannot be undone into an invalid document (found in C1.3; decided)

- **Section:** 10 §7.2 REP-01: "Repairs form one transaction with origin `load`, undoable as "Repair on Open"; the
  project is then modified".
- **Problem:** undoing the repairs would put back the document as read, which holds the errors the repairs removed
  (dangling references, duplicate IDs, invalid trees). 11 P2 and the executor's integrity gate never install a
  document with model errors, and 10 §5 makes those states invalid.
- **Decision taken (ADR-0014 §4):** the repaired project opens modified. Its history starts with the step
  "Repair on Open" (origin `load`, listed by `edit.history.get`, reported by `doc.changed`). That step is the base of
  the history and cannot be undone (`edit.nothingToUndo`). The user keeps the repaired version by saving, or drops it
  by closing without saving. Nothing is lost: every removed value is in the load report (REP-02).
- **Question:** confirm, or define what undoing the repair step should produce.

## SI-045 — the agent confirmation of RO-02 before the confirmation protocol exists (found in C1.3; decided)

- **Section:** 10 §8.5 RO-02: "Agents receive the same choice as a `needs_confirmation` result on the next save";
  11 §6.4 (the confirmation protocol, plan M1 C1.4).
- **Problem:** the confirmation protocol is C1.4's. In C1.3 the executor has no `needs_confirmation` result to
  return.
- **Decision taken (ADR-0014 §12):** after an external change (`doc.package.changedOnDisk`), a plain `project.save`
  fails with `project.changedOnDisk {path, detectedAt}`, with a hint naming both choices: reload (`project.open` with
  `onUnsaved`) or keep mine (`project.saveAs {path, onExists: "overwrite"}`). C1.4 turns the error into the
  `needs_confirmation` result.
- **Question:** none for the spec; recorded so the C1.4 change is traceable.

## SI-046 — codes C1.3 needs that the spec does not list (found in C1.3; decided)

- **Sections:** 10 §7.2, §8, §10; 11 §10; spec A (`project.*`, `doc.*`).
- **Decision taken (ADR-0014 §15):**
  - `project.backupNotFound` and `project.recoveryNotFound`: an unknown `backupId` or `recoveryId`.
  - `project.recoveryInUse`: recovery data whose owner still runs.
  - `asset.inUse`: `doc.asset.remove` of an asset still used by placements or media sequences, without `force`.
  - `fs.ioError`: an I/O failure outside a project save (settings, exports, cache).
  - `colour.unresolved`: the load-report and lint code for a colour ID nothing defines, until colour recovery
    (03 §11.2, M2 C2.2) replaces it with `colour.recovered` / `colour.placeholderCreated`.
  - Result warnings: `project.migrated` (`project.open` of an older file, migrated in memory) and
    `project.defaultsChanged` (`setAsDefault: true` of `project.properties.set`: the values become the defaults of new
    projects).
  - `project.invalidFormat` `details.reason` values beyond `unsafeEntry` and `sizeLimit` (ZIP-01, ZIP-06):
    `corrupt`, `encrypted`, `notZip`, `noMimetype`, `wrongMimetype`, `duplicateEntry`, `noManifest`, `manifest`,
    `project`, `library`, `palette`, `unrepairable`, `migration`, `wrongFormat`.
- **Question:** add them to 11 §10 and the owning sections.
