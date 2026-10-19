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

## SI-025 — `cell.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 04 and spec A (`cell.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `cell.clipboard.apply` `/params/options`: an object whose fields are not listed
  - `cell.clipboard.set` `/params/options`: an object whose fields are not listed
  - `cell.range.move` `/params/options`: an object whose fields are not listed
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-026 — `colour.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 03 and spec A (`colour.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `colour.fillMapping.set` `/params/mapping`: an object whose fields are not listed
  - `colour.palette.create` `/params/swatches/*`: an object whose fields are not listed
  - `colour.palette.duplicate` `/result/idMap`: an object whose fields are not listed
  - `colour.palette.set` `/params/palette`: an object whose fields are not listed
  - `colour.swatch.create` `/params/value`: an object whose fields are not listed
  - `colour.swatch.create` `/params/values`: an object whose fields are not listed
  - `colour.swatch.set` `/params/values`: an object whose fields are not listed
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-027 — `draw.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 02 and spec A (`draw.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `draw.content.add` `/params/content`: an object whose fields are not listed
  - `draw.content.add` `/result/idMap`: an object whose fields are not listed
  - `draw.current.get` `/result/activeView`: a member whose type is not stated (`activeView`)
  - `draw.erase.apply` `/params/pen`: an object whose fields are not listed
  - `draw.hitTest.get` `/result/hits/*`: an object whose fields are not listed
  - `draw.selection.changed` `/result/selectedPoints/*`: an object whose fields are not listed
  - `draw.text.add` `/params/format`: an object whose fields are not listed
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-028 — `drawAid.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 02 and spec A (`drawAid.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `drawAid.changed` `/result/state`: an object whose fields are not listed
  - `drawAid.lightTableShade.set` `/result/auto`: a member whose type is not stated (`auto`)
  - `drawAid.lightTableShade.set` `/result/static`: a member whose type is not stated (`static`)
  - `drawAid.staticLightTable.set` `/result/entries/*`: an object whose fields are not listed
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

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
  - `export.settings.set` `/params/settings`: an object whose fields are not listed
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
  - `fx.renderer.options.get` `/result/values`: an object whose fields are not listed
  - `fx.renderer.options.set` `/params/values`: an object whose fields are not listed
  - `fx.renderer.options.set` `/result/values`: an object whose fields are not listed
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-031 — `keymap.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 01 and spec A (`keymap.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `keymap.set` `/params/params`: an object whose fields are not listed
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-032 — `library.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 08 and spec A (`library.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `library.item.get` `/result`: a type the section does not define (ItemDetails)
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-033 — `lipsync.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 07 and spec A (`lipsync.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `lipsync.apply` `/params/mapping`: an object whose fields are not listed
  - `lipsync.detect` `/params/phonemeMap`: an object whose fields are not listed
  - `lipsync.mapping.set` `/result/mapping`: a member whose type is not stated (`mapping`)
  - `lipsync.settings.get` `/result/phonemeMap`: a member whose type is not stated (`phonemeMap`)
  - `lipsync.settings.set` `/params/phonemeMap`: an object whose fields are not listed
  - `lipsync.settings.set` `/result/phonemeMap`: a member whose type is not stated (`phonemeMap`)
  - `lipsync.shapes.list` `/result/shapes/*/referenceImage`: a member whose type is not stated (`referenceImage`)
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-034 — `playback.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 09 and spec A (`playback.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `playback.player.get` `/result`: a type the section does not define (PlayerState)
  - `playback.player.set` `/result`: a type the section does not define (PlayerState)
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-035 — `prefs.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 01 and spec A (`prefs.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `prefs.get` `/result/values`: an object whose fields are not listed
  - `prefs.list` `/result/*/default`: a member whose type is not stated (`default`)
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-036 — `print.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 09 and spec A (`print.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `print.settings.set` `/params/values`: an object whose fields are not listed
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-037 — `sound.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 07 and spec A (`sound.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `sound.scrubbing.set` `/result/effective`: a member whose type is not stated (`effective`)
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-038 — `storyline.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 08 and spec A (`storyline.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `storyline.selection.set` `/result/selection`: a member whose type is not stated (`selection`)
  - `storyline.transition.types.list` `/result/types/*/parameters/*/default`: a member whose type is not stated (`default`)
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-039 — `timeline.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 04 and spec A (`timeline.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `timeline.rows.list` `/result`: a type the section does not define (timeline object)
  - `timeline.transferOptions.set` `/params/options`: an object whose fields are not listed
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-040 — `tool.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 02 and spec A (`tool.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `tool.list` `/result/tools/*/options`: an object whose fields are not listed
  - `tool.list` `/result/tools/*/optionSchema`: an object whose fields are not listed
  - `tool.options.get` `/result/options`: an object whose fields are not listed
  - `tool.options.set` `/params/options`: an object whose fields are not listed
  - `tool.pen.create` `/params/settings`: an object whose fields are not listed
  - `tool.pen.set` `/result/pen`: a member whose type is not stated (`pen`)
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

## SI-043 — `xsheet.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 04 and spec A (`xsheet.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `xsheet.exposures.get` `/result`: a type the section does not define (exposure-sheet object)
  - `xsheet.exposures.set` `/params/columns/*`: an object whose fields are not listed
  - `xsheet.exposures.set` `/result/createdDrawings/*`: a member whose type is not stated (`createdDrawing`)
  - `xsheet.lightTable.add` `/params/items/*`: an object whose fields are not listed
  - `xsheet.panel.set` `/result/lightTable`: a member whose type is not stated (`lightTable`)
  - `xsheet.panel.set` `/result/toolbar`: a member whose type is not stated (`toolbar`)
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

<!-- end of generated placeholder issues -->
