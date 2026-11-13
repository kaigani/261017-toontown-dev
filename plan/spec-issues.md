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
  `details.milestone`), `command.timeout` (watchdog budget exceeded; the outcome is indeterminate: the call may still complete, nothing is rolled back or cancelled, and the caller reads `doc.state.get` before retrying; superseded in M2 C2.3 by ADR-0015 history C23-R2-01/C23-R3-01: the timed-out request is cancelled and not accepted unless it had already committed),
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
  - `anim.node.get` `/result/static`: a member whose type is not stated (`static`)
  - `anim.prefs.set` `/result/prefs`: a member whose type is not stated (`prefs`)
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
  - `colour.palette.create` `/params/swatches/*`: an object whose fields are not listed
  - `colour.palette.duplicate` `/result/idMap`: an object whose fields are not listed
  - `colour.palette.set` `/params/palette`: an object whose fields are not listed
  - `colour.swatch.create` `/params/value`: an object whose fields are not listed
  - `colour.swatch.create` `/params/values`: an object whose fields are not listed
  - `colour.swatch.list` `/result/swatches/*/value`: an object whose fields are not listed
  - `colour.swatch.set` `/params/values`: an object whose fields are not listed
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-026 — `draw.*`: shapes the owning section leaves undefined (found in C1.2 r2; resolved)

- **Resolved:** no placeholder remains in `draw.*`. Its positions are typed from the owning section or the sibling commands (C1.2 r3, C12-R3-01), or by the hand-written declarations of the checkpoint that implemented the commands (for example `ui.*` in M4 C4.1).

## SI-027 — `drawAid.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 02 and spec A (`drawAid.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `drawAid.changed` `/result/state`: an object whose fields are not listed
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-028 — `element.*`: shapes the owning section leaves undefined (found in C1.2 r2; resolved)

- **Resolved:** no placeholder remains in `element.*`. Its positions are typed from the owning section or the sibling commands (C1.2 r3, C12-R3-01), or by the hand-written declarations of the checkpoint that implemented the commands (for example `ui.*` in M4 C4.1).

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

- **Resolved:** no placeholder remains in `import.*`. Its positions are typed from the owning section or the sibling commands (C1.2 r3, C12-R3-01), or by the hand-written declarations of the checkpoint that implemented the commands (for example `ui.*` in M4 C4.1).

## SI-032 — `keymap.*`: shapes the owning section leaves undefined (found in C1.2 r2; resolved)

- **Resolved:** no placeholder remains in `keymap.*`. Its positions are typed from the owning section or the sibling commands (C1.2 r3, C12-R3-01), or by the hand-written declarations of the checkpoint that implemented the commands (for example `ui.*` in M4 C4.1).

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

- **Resolved:** no placeholder remains in `prefs.*`. Its positions are typed from the owning section or the sibling commands (C1.2 r3, C12-R3-01), or by the hand-written declarations of the checkpoint that implemented the commands (for example `ui.*` in M4 C4.1).

## SI-037 — `print.*`: shapes the owning section leaves undefined (found in C1.2 r2; resolved)

- **Resolved:** no placeholder remains in `print.*`. Its positions are typed from the owning section or the sibling commands (C1.2 r3, C12-R3-01), or by the hand-written declarations of the checkpoint that implemented the commands (for example `ui.*` in M4 C4.1).

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

## SI-041 — `ui.*`: shapes the owning section leaves undefined (found in C1.2 r2; resolved)

- **Resolved:** no placeholder remains in `ui.*`. Its positions are typed from the owning section or the sibling commands (C1.2 r3, C12-R3-01), or by the hand-written declarations of the checkpoint that implemented the commands (for example `ui.*` in M4 C4.1).

## SI-042 — `view.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 01 and spec A (`view.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `view.list` `/result/*/transform`: a member whose type is not stated (`transform`)
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

## SI-043 — `xsheet.*`: shapes the owning section leaves undefined (found in C1.2 r2; decided)

- **Section:** 04 and spec A (`xsheet.*` command rows).
- **Positions** (in `crates/registry/src/declared.json`, `placeholders`):
  - `xsheet.lightTable.add` `/params/items/*`: an object whose fields are not listed
- **Decision taken (ADR-0013 §1, C12-R2-02):** each position is a strict placeholder: an object accepts only `{}`, a member accepts only a scalar (never an object or a list), so nothing undefined is accepted silently. The implementing checkpoint replaces it with the shape the section gives.
- **Question:** define these shapes in the owning section (field names and types).

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

## SI-045 — the agent confirmation of RO-02 before the confirmation protocol exists (found in C1.3; closed in C1.4)

- **Section:** 10 §8.5 RO-02: "Agents receive the same choice as a `needs_confirmation` result on the next save";
  11 §6.4 (the confirmation protocol, plan M1 C1.4).
- **Problem:** the confirmation protocol is C1.4's. In C1.3 the executor has no `needs_confirmation` result to
  return.
- **Decision taken (ADR-0014 §12):** after an external change (`doc.package.changedOnDisk`), a plain `project.save`
  fails with `project.changedOnDisk {path, detectedAt}`, with a hint naming both choices: reload (`project.open` with
  `onUnsaved`) or keep mine (`project.saveAs {path, onExists: "overwrite"}`). C1.4 turns the error into the
  `needs_confirmation` result.
- **Question:** none for the spec; recorded so the C1.4 change is traceable.
- **C1.4:** done. Agents get `needs_confirmation` (category `externalChange`) bound to the file's identity and the
  lock; see SI-050 for the reload choice.

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
    - `corrupt`, `encrypted`, `unsupportedCompression`, `notZip`, `noMimetype`, `wrongMimetype`, `duplicateEntry`;
    - `noManifest`, `manifest`, `project`, `library`, `palette`;
    - `unrepairable`, `migration`, `wrongFormat`.

    Added in round 1:
    - `journal`: a malformed folder-save journal;
    - `unrecoverableSave`: an interrupted folder save that left neither version complete;
    - `revisionExhausted`: repairs needed at `documentRev` 2^64 − 1.
- **Question:** add them to 11 §10 and the owning sections.

## SI-047 — RO-02 cannot see an edit that keeps size and modification time (found in C1.4; interim ruling, open)

- **Section:** 10 §8.5 RO-02 ("the application detects that the open project file changed on disk").
- **Problem:** detection compares sizes and modification times (a package; for a folder, every entry file's). An
  external edit that keeps a file's size and restores its modification time is not detected. Measured with a
  10,000-file folder (`reviews/evidence/M1-C1.4/measurements/fingerprint-*.json`: `sizeMtimePreservingEditDetected:
  false`). Hashing contents at the 2 s watch interval would cost a full read of the project every 2 s.
- **Interim ruling (orchestrator, C1.4 round 1, binding until the spec owner decides; C14-R1-09):**
  - the watch tick keeps size + modification time;
  - every save that replaces the project verifies its on-disk content against the content hash recorded at open or
    at the last save (`project.save`, Save As over the current path, `project.close {onUnsaved: "save"}`);
  - a mismatch is `needs_confirmation` for agents and `project.changedOnDisk` for users.

  As implemented (ADR-0014 §12):
  - package: the SHA-256 of the file;
  - folder: the SHA-256 of every entry file. The folder manifest has no per-entry hash list (only packages carry
    one), so the entry files are hashed directly.

  The cost on the 10,000-file folder is recorded per save (`measurements/fingerprint-*.json`, `saveContentCheckMs`).
- **Question for the user / spec owner:** is the cost of hashing the whole project at every save acceptable? A
  cheaper policy would hash only at the first save after an idle period, or only entries whose size or time changed
  plus a sample. Should RO-02 state the size + modification-time limit of the watch explicitly?

## SI-048 — codes, reasons and lock fields C1.4 adds (found in C1.4; decided)

- **Sections:** 11 §6, §10, §15.1; 10 §8.6.
- **Decision taken (ADR-0015, ADR-0014 §11.4):**
  - `agent.sessionNotFound`: an `agent.*` command naming an unknown session.
  - `agent.disconnected`: a call of a session the user disconnected.
  - `agent.httpUnavailable` (warning): `agent.server.set {http: true}` before the HTTP transport (M4).
  - `confirmation.expired`, `details.reason`: `expired`, `unknown or already used`, `bound to another call`, `the
    document changed since the confirmation was requested`, `the condition changed since the confirmation was
    requested`.
  - `limit.exceeded`, `details.limit`: `maxBatchSteps`, `maxJobs`, `maxInlineBytes`.
  - `project.saveFailed`, `details.reason: "nameCollision"` with `details.names`: a folder-form save of entry names
    that NFC or case folding would merge.
  - The lock file gains `osProtection: "shareMode" | "flock"`, which says that its holder protects it with an open
    handle (readers that do not know the key ignore it).
  - Round 1:
    - `project.saveFailed` `details.reason: "ownershipLost"`: a save wrote the project but the session lost its lock
      meanwhile;
    - `agent.activity.export` results gain `omitted: [{seq, tool, commandId, reason}]`;
    - activity entries gain `dryRun`;
    - `app.state.get` gains `agent` (the calling session: policy, scopes, paused, limits), which 11 §7.1 lists under
      `get_state`.
  - Round 2:
    - `fs.ioError` with `details.reason: "publish"` and `details.committed`: a file the call produces could not be
      moved into place after the commit;
    - activity entries gain `cachedRetry` and, for batches, `committedSteps`;
    - export omissions gain the reason `cachedRetry`.
- **Question:** add them to 11 §10, 11 §15.1 and 10 §8.6.

## SI-049 — MCP revision 2026-07-28 removes resource subscriptions and deprecates logging (found in C1.4; open)

- **Section:** 11 §4: "target: the current stable protocol revision at implementation time; minimum 2025-06-18";
  "MUST support ... resources (including resource templates and subscriptions) ... logging".
- **Problem:** the official Python SDK 2.0 warns that `resources/subscribe` is removed as of revision 2026-07-28 (a
  `listen` stream replaces it) and that the logging capability is deprecated (SEP-2577). The server speaks 2025-06-18
  and 2025-11-25 and passes the SDK's checks there (`reviews/evidence/M1-C1.4/sdk-conformance-*.json`).
- **Decision taken:** none; 11 §4's MUSTs are implemented as written.
- **Question:** should the implementation follow 2026-07-28 (the subscription replacement), and with what
  milestone? 11 §4 and §9.1 would need rewording.

## SI-050 — RO-02's "same choice" for agents meets 11 §6.1's `app` scope (found in C1.4; decided)

- **Sections:** 10 §8.5 RO-02 ("Agents receive the same choice as a `needs_confirmation` result"); 11 §6.1
  (discarding unsaved work is the `app` scope's, never an agent's).
- **Problem:** the user's choices are *reload* (discards unsaved work) and *keep mine*. An agent cannot discard.
- **Decision taken (ADR-0014 §12):** the agent's `needs_confirmation` offers keep mine (accepting overwrites) and
  names reload as the user's choice (`by: "user"`); its other choice is `project.saveAs` to another path.
- **Question:** confirm, or let the confirmation itself authorise the discard of a reload.

## SI-051 — what a replayable activity export contains (found in C1.4 round 1; decided)

- **Sections:** 11 §11 ("Exportable as a replayable batch JSON"), §3 (batch-script mode), §7.2 (dry runs have no
  effect).
- **Problem:** an activity log holds calls that a replay must not repeat as written:
  - dry runs;
  - queries and discovery tools (`list_commands` and the other tools that are not command ids);
  - failed or unconfirmed calls;
  - calls whose inline data were redacted to a hash;
  - batches, whose `$<n>` references are relative to the batch.
- **Decision taken (ADR-0015 §11, C14-R1-15):** the export contains committed edits only, in order, as one sequential
  batch:
  - a committed batch contributes its steps, with references renumbered and clashing names renamed;
  - everything else is listed under `omitted` with its reason in the command result.
- **Round 2 (C14-R2-02, -03):**
  - batch entries record `committedSteps`, so a sequential batch that failed after committing some steps contributes
    exactly those;
  - a `requestId` retry answered from the cache is recorded with `cachedRetry: true` and left out (reason
    `cachedRetry`);
  - replay through batch-script mode into the same initial document reproduces every document entry
    (`crates/mcp/tests/c14_r2_replay.rs`).
- **Round 3 (C14-R3-01, -02; orchestrator ruling on the representation):**
  - commit facts in the audit record come from the actual commit, not the answer's status: a batch that committed and
    then failed to publish is audited `error` with its committed steps, and replay includes them;
  - the batch-script format gains an **atomic group step** (`{"id": "batch", "params": {label, steps, expectRev}}`
    inside a sequential batch), run as one transaction with the semantics of `batch {atomic: true}`; nested groups are
    refused with `batch.nestedGroup` (ADR-0015 §13);
  - the export writes every committed atomic batch as one group, so replay reproduces revisions and undo boundaries
    byte for byte (`crates/mcp/tests/c14_r3_replay.rs`, `crates/mcp/tests/c14_r3_groups.rs`).
- **Round 4 (C14-R4-01):** the group's `expectRev` is implemented. A batch request carries an optional `expectRev`,
  checked once before an atomic batch's or group's first step; a mismatch refuses the whole transaction with
  `rev.conflict`. The export keeps the original call's `expectRev` on its group. A sequential batch refuses
  `expectRev` (ADR-0015 §14).
- **Question:** confirm, or define the representation in 11 §11. The orchestrator's ruling (atomic group step) is the
  M1 representation until the spec owner decides otherwise.

## SI-052 — bitmaps in drawings: no `draw.bitmap.*` commands, and `mediaId` vs `image` (found in M2 C2.1; decided)

- **Sections:** 02 §3.9 (embedded bitmaps), §13.4 bitmap node `{mediaId, pixelWidth, pixelHeight, transform}`, §13.5
  (no `draw.bitmap.*` rows); 10 §4.9 bitmap node `{image: blob, width, height, transform}`; spec A.
- **Problem:** spec A has no command that creates a bitmap node; 02's read-back names a `mediaId` that 10 does not
  persist (10 stores the PNG as a blob entry).
- **Decision taken (ADR-0016 §8):** bitmaps enter a drawing through `draw.content.add` (and `edit.paste`) in the
  §13.4 shape; `mediaId` is the package entry of an image the document already holds (the read-back reports the
  bitmap's blob entry as `mediaId` and the blob itself as `image`). New pixels arrive with image import (09, M10) and
  vectorization "with texture" (M2 C2.4). Bitmaps are read back, bounded, deleted, grouped and saved like any node.
- **Question:** confirm, or add a `draw.bitmap.add` command and a media reference to 10 §4.9.

## SI-053 — brush outlines with holes in a one-path stroke (found in M2 C2.1; decided)

- **Sections:** 02 §3.4 ("Self-overlaps of one stroke form a single clean outline"), §13.4 (`outline: [[outer],
  [hole]]`); 10 §4.9 (a stroke has one `points` path).
- **Problem:** a stroke that loops sweeps an outline with a hole (and a zero-width pressure section can split it into
  pieces); 10 stores one closed path per stroke.
- **Decision taken (ADR-0016 §4):** the outline is the union of the swept tips; its contours are joined into one closed
  path by zero-width bridges (each hole or piece spliced in at its nearest vertex). Filled with the non-zero rule the
  bridge edges cancel, so the shape renders as the union. `draw.drawing.get` reports `outline` with that one contour;
  `draw.content.add` accepts one contour. The planar map (C2.2) treats a bridge as two coincident edges.
- **Question:** amendment candidate for the spec owner: allow several contours per brush stroke in 10 §4.9.
- **Round 1 adjudication (`reviews/M2-C2.1-r1.md`):** accepted as an interim representation, but C2.2 may not rely on
  it before a larger corpus of holes, disconnected pieces and bridges is retained. C2.1 round 1 adds the property
  `outlines_contain_every_swept_tip_including_holes_and_separate_pieces` (loops and zero-pressure gaps, winding
  containment of every swept tip); the larger bridge corpus that the planar map consumes is ticketed to M2 C2.2 in
  `plan/traceability.csv` (SI-053 row).

## SI-054 — the persisted brush spine: tip angle, tip proportions and closed spines (found in M2 C2.1; decided)

- **Sections:** 02 §3.2 (spine keeps "the pen (tip shape, minimum/maximum size)"), §6.1 (tip shapes round, oval,
  square, flat; tip angle), §13.4 (`pen: {tipShape, minSize, maxSize, tipAngle}`); 10 §4.9 (`spine: {points, pressure,
  pen: {penId, tip, minSize, maxSize}}`, no `closed`).
- **Problem:** (a) 10's spine pen has no tip angle, so a non-round outline could not be swept again after a spine or
  pressure edit; (b) 02 does not give the proportions of oval and flat tips; (c) a spine has no `closed` flag.
- **Decision taken (ADR-0016 §3–§5):** (a) a non-zero tip angle of a non-round tip is kept in the stroke's
  `ext["dev.toontown.brush"].tipAngle` (the reverse-DNS extension slot of 10 §2.2, preserved by every reader) and read
  back as `pen.tipAngle`; (b) oval tips are 2:1 ellipses and flat tips 5:1 rectangles with the long axis at the tip
  angle, square tips have side = width; (c) a closed spine is stored open with its first anchor repeated at the end.
- **Question:** add `tipAngle` (and `closed`) to 10 §4.9's spine; state the tip proportions in 02 §6.1.

## SI-055 — text property names and units differ between 02 and 10 (found in M2 C2.1; decided)

- **Sections:** 02 §7.6.1 (Text panel: kerning int 1/100 em; line spacing "Additional spacing between lines", du;
  outline colour), §13.4 runs `{baseline, autoKern, colourId, outlineColourId}` and paragraphs `{align, leftMargin,
  rightMargin}`; 10 §4.9 runs `{script, kerning, autoKerning, colourId}` and paragraphs `{alignment, marginLeft,
  marginRight, indent, lineSpacing}`.
- **Problem:** the read-back and the persisted form name the same fields differently; 10 has no outline colour; the
  model's documentation called `lineSpacing` a factor.
- **Decision taken (ADR-0016 §6):** 10 owns the persisted names, 02 the command and read-back names (mapped one to
  one); units are 02's (sizes, margins, indent and line spacing in du, kerning 1/100 em, §14 decision 16).
  `outlineColourId` is accepted only as `null` (`arg.invalid` otherwise) and read back as `null`. New text uses the
  generic family `sans-serif` (resolved to the platform's default sans-serif face), so the persisted name is the same on
  every OS.
- **Question:** add `outlineColourId` to 10 §4.9 runs, or remove it from 02.

## SI-056 — targeting, revisions and settings of the C2.1 drawing commands (found in M2 C2.1; decided)

- **Sections:** 02 §13.1 items 4 and 5, §3.12 ("Commands that address objects by ID MAY pass the revision they
  read"), §13.5 rows; 10 REV-05; 02 §6.1 (current pen), §6.2/§8.11 (symmetry, Draw Behind), §7.3.6 (Stroke tool
  layer); 03 §2.4 (current colour).
- **Problem:** the rows list `drawingId?` only on some commands and `expectRev` on two; the current pen, colour,
  symmetry and Draw Behind settings have no setters before M4/M7; the Stroke tool's layer rule needs zones.
- **Decision taken (ADR-0016 §2, §7):** every C2.1 drawing command accepts `drawingId`, `elementId` and `frame`
  (§13.1 item 4); creation commands (`draw.stroke.add`, `draw.text.add`, `draw.content.add`) accept `createDrawing`;
  commands that edit objects by ID accept `expectRev` (the drawing revision) and fail with `drawing.staleRevision`.
  Until their commands exist: the current brush pen is the project's "Pen 3", else its first brush pen; the current
  colour is the pen's colour, else the first swatch of the first visible palette; symmetry is none and Draw Behind
  off (the engine applies both when set). `layer: "auto"` for an invisible stroke uses the top-most layer with a stroke
  the new stroke crosses (zones arrive with C2.2). Point-taking parameters are in drawing space; `space: scene|view`
  needs the M3 transforms and M4 views.
- **Question:** confirm the parameter superset, or list the addressing parameters in each row.
- **Round 1 adjudication (`reviews/M2-C2.1-r1.md`):** common addressing is a reading of the explicit 02 §13.1 item 4,
  not an ambiguity that permits omission. Round 1 applies it to the three object setters that lacked it
  (`draw.stroke.set`, `draw.text.set`, `draw.textFormat.set`; C21-R1-03); the revision check runs on the resolved
  drawing. The scene/view deferral (M3 C3.2 / M4 C4.2) and the settings deferral (M4 C4.2 / M7 C7.1) stand.

## SI-057 — zones in the C2.1 read-back (found in M2 C2.1; decided)

- **Sections:** 02 §13.4 (`zones`, stroke `leftZoneId`/`rightZoneId`), §13.5 (`zoneIds` results), §3.12 item 1; 10
  §4.8 (derived `zones`).
- **Problem:** zones are derived by the planar map, which is M2 C2.2.
- **Decision taken (ADR-0016 §9):** until C2.2, `draw.drawing.get` leaves `zones` and the side zone IDs out (the
  closed result schema has no such members), commands report `zoneIds: []`, and every such result carries the warning
  `doc.derivedUnavailable` naming M2 C2.2. Zone fills persisted on stroke sides are preserved untouched. Nothing
  reports an empty zone list as computed.
- **Question:** none; C2.2 replaces the warning with computed zones.

## SI-058 — edit results and the drawing event: §13.1 item 11 against the shorter §13.5 rows (found in M2 C2.1 round 1; decided)

- **Sections:** 02 §13.1 item 11 ("every edit result includes `{drawingId, rev}`" and the changed IDs), §13.5 result
  columns (e.g. `draw.stroke.set → {strokeIds, removedIds, rev}`), §13.6 and spec A `draw.drawing.changed`
  (`{drawingId, rev, commandId, createdIds, modifiedIds, removedIds}`); 02 §3.2 and §14 decision 2 (outline edit
  discards the spine).
- **Problem:** the §13.5 rows list shorter results than the common rule; the event row names fields but not which
  commits produce it; §3.2 does not say what one edit of both the outline and the spine means.
- **Decision taken (ADR-0016 §2, §5; C21-R1-04, -07, -08):** every edit result is the row's members plus `drawingId`,
  `rev`, `createdIds`, `modifiedIds` and `removedIds` (local IDs; the rows' own `strokeIds`/`removedIds` keep their
  meaning). `draw.drawing.changed` is emitted by the common commit path for every commit that changes drawing content
  (commands, generic patches, batches, undo, redo, checkpoint restore) with `commandId` the committing command
  (`batch` for a batch). An edit that changes both a stroke's outline and its spine is refused (`arg.invalid`,
  `details.pointers: ["/points", "/spine"]`) by orchestrator ruling; an outline edit alone discards the spine.
- **Question:** state in §13.5 that the rows extend §13.1 item 11's common members, and add the both-authorities
  rejection to §3.2.

## SI-059 — brush outlines and persistent noding (found in M2 C2.2; ruled)

- **Sections:** 02 §3.2 ("Within a layer, strokes are split at intersections and junctions"), §3.12 item 1; 10 §4.9 (a
  brush stroke has one closed outline path); SI-053, SI-054.
- **Problem:** splitting a brush outline would break its single closed path and stale its spine.
- **Decision (orchestrator ruling, ADR-0017 §1):** brush outlines are never split or moved by noding; their crossings
  are vertices of the derived graph only. Pencil and invisible strokes are split at the outline. Tests:
  `c22_settle::a_pencil_crossing_a_brush_splits_at_the_outline_and_leaves_the_brush_byte_identical`,
  `c22_planar::a_pencil_line_across_a_brush_stroke_splits_and_the_brush_entry_is_untouched`.
- **Question:** state in 02 §3.2 that brush outlines take part in topology without being split.

## SI-060 — fills of zones bounded only by brush-outline stretches (found in M2 C2.2; ruled, interim)

- **Sections:** 10 §4.9 (fills on stroke sides; zones derived), 02 §3.5; SI-059.
- **Problem:** one brush outline may border several zones (the hole of a ring and the space outside it); its two sides
  cannot hold their different fills.
- **Decision (orchestrator ruling, ADR-0017 §4):** such fills are kept in the brush stroke's
  `ext["dev.toontown.zones"].exterior` = `[{side, from, to, fill}]` (path-parameter ranges), rewritten deterministically
  on every re-node (same input → same bytes, `c22_settle`, `c22_properties`), round-tripping through both container
  forms (`ext` is preserved by every reader) and `draw.content.add` (fills travel at interior points), and listed in
  the drawing's `colourRecovery` like any fill (`colour_recovery::used_colours`).
- **Question (amendment proposal for the spec owner):** add a zone-fill side table to the drawing in 10 §4.9 (e.g.
  `zoneFills: [{layerId, boundary, fill}]`), from which the `ext` key would be migrated.

## SI-061 — region targets, repaint, protected colours (found in M2 C2.2; ruled)

- **Sections:** 03 §8.2, §8.4, §16.3 (`colour.paint.apply` parameters), §1.4 (swatch fields).
- **Decision (orchestrator ruling, ADR-0017 §8):** `colour.paint.apply` also takes `lasso` and `rect` (zones whose
  interior point lies inside), `onlyPainted` (repaint), and `respectProtected` (default true); a swatch is protected by
  `ext["dev.toontown.paint"].protected: true`; refusals are reported per target with `colour.protected`.
- **Question (amendment proposal):** add a `protected` flag to 03 §1.4 swatches and these parameters to the §16.3 row.

## SI-062 — per-drawing gap-closing setting (found in M2 C2.2; ruled)

- **Sections:** 03 §8.8 (a persisted user setting), §16.3 (`gapClosing` per command).
- **Decision (orchestrator ruling):** the spec as written — user setting (`paint.gapClosing` preference) plus the
  per-command override; no per-drawing field.
- **Question (amendment proposal, owner M4 C4.2):** whether the paint tools' UI wants a per-drawing setting.

## SI-063 — view pixels at 100 % zoom (found in M2 C2.2; decided)

- **Sections:** 03 §8.8 ("tolerance at 100 % zoom"), 02 §13.1 item 5; 03 §7.2 ("1 image pixel = 1 drawing unit at
  100 %").
- **Decision (accepted by ruling):** at 100 % zoom one view pixel is one drawing unit; gap tolerances are 3/8/16 du and
  the hit-test tolerance is in du for commands without a view.

## SI-064 — the unit space of fill mappings (found in M2 C2.2; decided)

- **Sections:** 03 §7.2 (default and drag mappings), §16.3 (`Mapping`).
- **Problem:** the unit space the 2×3 matrix maps from is not stated.
- **Decision (ADR-0017 §8):** unit square `[0, 1]²`: linear axis along x (0 → 1), radial centred at (½, ½) with radius ½,
  texture image over the square. Default mapping: the square onto the object's bounds (radial: radius to the corner);
  drag: linear 0 at the press and 1 at the release, radial centre at the press with the drag as radius, texture origin
  at the press, width = drag length, rotated to the drag.
- **Question:** confirm, or define the unit space in 03 §7.2.

## SI-065 — `colour.pick` before the renderer and the views (found in M2 C2.2; decided)

- **Sections:** 03 §16.3 (`colour.pick`: `space` defaults to `view`, `rgba` is the rendered pixel).
- **Decision:** until the desktop views (M4 C4.2) and the renderer (C2.3/M3), points are given in drawing space
  (`space: "drawing"`; `scene`/`view` are `arg.invalid` with a hint, in every paint command), and `rgba` is the swatch's
  value for the palette's first style (first stop of a gradient; `#00000000` for textures, unpainted and empty canvas).
- **Question:** none; M4 C4.2 and C2.3 complete the view space and the composited pixel.

## SI-066 — zone reporting (found in M2 C2.2; decided)

- **Sections:** 02 §13.4 (zones), §13.5 (`zoneIds`), §13.6 (`draw.drawing.changed`); 10 §10.3 (`unpaintedZones`).
- **Decision (ADR-0017 §3, §11):** zone IDs are `zn_` + 26 base-32 characters of a SHA-256 of the canonical boundary;
  `draw.drawing.changed` and edit results list zones as derived objects (new IDs created, vanished ones removed,
  repainted ones modified); `zoneIds` of `draw.stroke.add` are the zones the edit created; a hole's `zoneId` is the zone
  across its longest boundary stretch; `unpaintedZones` reports zones with at least one visible bounding stroke and no
  fill (a zone closed only by invisible strokes is not line art to paint).
- **Question:** confirm the event semantics for derived objects in 02 §13.6.

## SI-067 — noding and recovery details (found in M2 C2.2; decided)

- **Sections:** 02 §3.12 item 1, §3.11; 03 §11.2; 10 §4.11.
- **Decision (ADR-0017 §1, §10):** junction tolerance 1/32 du; pieces are not merged back after a delete; recovered
  swatches without a recorded palette name and placeholders go to one palette `Recovered` (created once, reused);
  missing styles are added by name with every swatch receiving its first style's value; recovery lists are refreshed on
  save (10 §4.11's "on save"), not on every edit; texture images referenced by recovered swatches keep their package
  entry (a missing image is reported by `missingMedia`).
- **Question:** confirm where recovered swatches go when the recorded palette exists but is hidden (C2.2: a visible
  palette of that name, else a new one).
- **Round 1 (C22-R1-01, R1-03, R1-05, R1-06; orchestrator rulings):**
  - *Junction rule.* Every crossing and T-junction is resolved once for all participants. A record within the
    tolerance of a junction's first record belongs to it (no chaining). Its coordinate is the nearest existing anchor
    of an unchanged pencil or invisible participant, else the first record's point, and every participant meets that
    exact coordinate. An open end is a junction with the nearest stroke and with every stroke as near (ties). A
    stroke crossing another exactly at a vertex of either is a junction at exactly that vertex. A touch, retracing, or
    any shared direction (collinear overlap) is not a crossing. Round 2 (C22-R2-01 ruling): every crossing,
    vertex, collinear, touch and turn-back decision uses `i128` orientation signs on the snapped 1/64 du grid, with no
    epsilon. Floating point only places a proper crossing's point. Round 3 (C22-R3-03 ruling): the endpoint pass uses the
    same classification. An open end on another stroke is a junction only when its grid direction is not parallel to
    that stroke's segment (exact sign), so collinear overlap is excluded by both passes alike. A cut at the same place as an open path's end becomes that end's junction.
  - *Imports.* Drawings `doc.import.json` creates are persisted content, taken as they are like a load: their fills
    are read and repaired (round 2, C22-R2-02: `repair_drawing_sides` in the import's own transaction, reported as
    `drawing.fillInconsistent`), but they are not re-noded. Their first edit nodes them. This keeps C1.3's import fidelity
    and `idPolicy: preserve`.
  - *Zone-ID input (ruling).* A zone ID is SHA-256 over each boundary cycle in traversal order: the snapped integer
    coordinate of each vertex with the stroke IDs and sides of the edge leaving it. A vertex where the boundary goes
    straight on along the same strokes is left out, the cycle starts at its smallest token sequence, and hole cycles
    are sorted. Carry-over scores by exact overlap. An old zone with a new zone's ID has the same polygon and keeps its
    fill directly.
  - *Recovery.* Recovery and side-fill repair run on load as part of the repair step (C22-T1 withdrawn). Placeholder
    recovery entries are written on save for used colours without a definition or a copy, in the `Recovered` palette.
- **M2 C2.4 (ticket C24-T1, orchestrator ruling): side fills on overlapped edges.** Overlapping or retracing collinear
  segments stay unsplit (C22-R3-03 stands). For any set of mutually overlapping segments the side fill is owned by one
  deterministic owner: the segment of the stroke with the lowest stroke ID, then the lowest segment index. Every read of
  a side fill on an overlapped segment resolves through the owner; every write (paint, repair, recovery, carry-over)
  writes through to the owner and mirrors to the other members. Folds are covered by the owner rule, not by splitting.
  Where a stroke side faces zones with different fills along its length (an overlap, a fold, or a vertex another stroke
  only touches), the stretches that differ from its side fill are stored as `{side, from, to, fill}` records in the
  SI-060 extension key (ADR-0017 history 7–8). The shrunk property case is a permanent regression with retrace and
  fold variants.
- **M2 C2.4 round 1 (C24-R1-04, confirmed ruling): the junction coordinate has three steps.** A junction's coordinate
  is the nearest existing anchor of an unchanged pencil or invisible participant; **else the one point where every
  participant already has an anchor**; else the first record's point. The middle step keeps re-noding settled pieces
  that are marked changed from moving them (C22-T2 fixpoint; ADR-0017 history 9). This supersedes the two-step wording
  of the round-1 junction rule above.
- **M2 C2.4 round 1 (C24-R1-02 ruling): a proper crossing's point is exact.** The fractions along both snapped edges
  are ratios of the exact grid orientations, and the crossing point is the rational crossing of the snapped edges
  rounded half to even onto the 1/64-du grid. Floating point no longer places it (this supersedes "Floating point only
  places a proper crossing's point" in round 2), so an integer-du translation of the input translates the snapped
  arrangement exactly (ADR-0017 history 10).

## SI-068 — mappings of lines (found in M2 C2.2 round 1; decided)

- **Sections:** 03 §7.2, §1.6; 10 §4.9 (stroke `fillMatrix`).
- **Issue:** 03 §7.2 says each painted zone, brush stroke **or line** that uses a gradient or texture stores its mapping
  matrix. The note on `fillMatrix` in the 10 §4.9 stroke table names only brush strokes, and C2.1's schema rule kept
  `fillMatrix` null for pencils.
- **Decision (C22-R1-09 ruling):** pencil strokes store `fillMatrix` when painted with a gradient or texture (drag
  mapping or the default box). Invisible strokes keep it null. A solid colour ignores but preserves the stored matrix
  (03 §1.6). Stored matrices serialise integral entries as integers.
- **Proposal:** amend the 10 §4.9 note to "brush strokes and lines with gradient or texture colours".


## SI-069 — headless view, rename and polyline handles (found in M2 C2.3; decided)

- **Sections:** 02 §9, §7.2.6, §13.5 (`draw.viewMapping.get`, `draw.drawing.rename`, `draw.polyline.add`).
- **Decisions:** headless runs have one virtual Drawing view of 1920 x 1080 px at zoom 1 with the drawing origin at its
  centre (`drawingToView [1,0,0,-1,960,540]`); any other view is `view.notFound` until M4 C4.2 owns views. Drawing names
  refuse `\ / : * ? < > " | .` (`drawing.nameInvalid`) and duplicates within the bank (`drawing.nameExists`); rename
  and lock are allowed on a locked drawing (metadata, not artwork). Polyline `inHandle`/`outHandle` are offsets from
  the point; snapping joins existing anchors within `snapRadius`.

## SI-070 — transforms: spines, envelopes, point subsets (found in M2 C2.3; decided)

- **Sections:** 02 §3.2, §7.1.1–§7.1.4, §8.13, §13.5.
- **Decisions:** rigid motions (translation, rotation, reflection) move a brush spine with its outline and turn a
  non-round tip angle; scale, skew and envelopes drop the spine (the outline becomes authoritative, as for a direct
  outline edit). Envelopes map control points as points (projective when the target quadrilateral is convex, else
  bilinear); fill matrices, text and bitmap transforms take the affine fit through three corners. Reflections swap side
  fills and brush exterior records. `draw.transform.apply` also takes `points` (or the Contour Editor's point
  selection) to move anchors only (orchestrator ruling: point-subset transforms for M4). Layers moved as a whole repaint
  their zones with the old fills and mapped matrices, so gradients and textures stay attached.

- **Round 1 additions (C23-R1-01/02/03):** every geometric transform route repaints moved layers the same way: each
  zone takes the fill of the source zone at its interior point mapped back through the map's affine part (envelopes:
  their three-corner fit; a parallelogram target is mapped exactly as an affine map), the matrix mapped forward;
  Reposition All does the same for every drawing. Point-subset transforms move the whole same-layer junction class of
  each requested anchor, with the pivot from the requested subset. **Snap to Contour reading:** candidates are the
  other strokes of the moved stroke's own layer (layers are independent planar maps, 02 §3.1), by their whole contour
  — anchors and segment interiors, curved segments included — within 8 du; ties go to the lower stroke ID, then the
  lower segment and parameter.

## SI-071 — the Cutter's segment primitive and brush strokes (found in M2 C2.3; decided)

- **Sections:** 02 §7.2.2, §7.2.7, §13.5 (`draw.cut.apply`, `draw.erase.apply`); spec A (closed command list).
- **Decisions:** the cut-by-polygon and cut-by-segment primitives (orchestrator ruling) are `draw.cut.apply` with
  `polygon`/`rect` or the extension `segment` (spec A admits no new command ID). A cut stages its boundary as an
  invisible stroke; pieces through a pre-existing zone stay as cut edges. Brush strokes are never split (SI-059): one
  wholly inside is inside, one crossing is kept whole and reported in `keptBrushIds`.
- **Question:** amendment candidate: a boolean outline cut for brush strokes.

## SI-072 — explicit eraser pen (found in M2 C2.3; decided)

- **Sections:** 02 §13.5 `draw.erase.apply` `pen?: object` (SI-026 placeholder).
- **Decision:** `pen` is a round eraser `{minSize, maxSize}`; `penId` must name an eraser pen; the default is the
  project's first eraser pen.

## SI-073 — flatten and optimize details (found in M2 C2.3; decided)

- **Sections:** 02 §3.5, §8.1, §8.2, §8.10, §8.11.
- **Decisions:** after re-noding, lower pencil and invisible pieces under opaque upper cover (opaque zone fills, opaque
  brush strokes) are discarded like wholly hidden strokes; partly covered lower brush strokes stay whole (SI-059).
  "Opaque" means a solid colour or gradient with alpha 255 in the interim style; textures count as not opaque. Zones of
  the result take the fill of the winning source layer painting their interior. `policy {precedence: upper|lower,
  position: top|bottom}` (orchestrator ruling: one command for M4 auto-flatten and Draw Behind). Optimize merges
  adjacent siblings only; it also declines when an upper painted zone covers a lower line or brush stroke (the merged
  layer would draw it above the fill). Overlap is tested on zone outer rings, outlines and centre paths.

## SI-074 — drawing renders: region, text, texture orientation (found in M2 C2.3; decided)

- **Sections:** 02 §13.5 `draw.drawing.render`; 03 §7.2; ADR-0006.
- **Decisions:** `padding` is in drawing units; without `height` the region keeps its aspect, with both it is fitted and
  centred; `camera` is 12 fields wide at the project aspect; an empty drawing's `bounds` is the 12-field guide. Results
  add `pixelsSha256` and `warnings`. Text pixels come from the machine's fonts, so cross-platform goldens avoid text.
  Superscripts rise a third of the run size, subscripts drop a sixth; `centre`/`right` alignments shift lines.
- **Texture orientation (orchestrator ruling on the C2.3 handoff): the upright convention everywhere.** The renderer
  keeps image row 0 at unit y = 0 (the standard texture convention). Stored mappings therefore show the image upright
  in drawing space: the tile-off click default is `[w, 0, 0, -h, minX, maxY]` like tile-on (`[W, 0, 0, -H, minX,
  maxY]` at native size); a texture drag mapping takes the image's x axis along the drag and its y axis as the x axis
  turned clockwise, `[dx, dy, dy, -dx, fromX, fromY]` (negative determinant in y-up drawing space); gradients are
  unaffected. `c22_r3_commands::texture_click_uses_palette_first_style_and_drag_and_tile_off_stay_unchanged` carries
  the new tile-off and drag values (declared edit under the ruling); the `c23_render` goldens show the asymmetric
  3 x 2 fixture upright under tile-on, tile-off and a drag.

## SI-075 — fill mapping objects (found in M2 C2.3; decided)

- **Sections:** 03 §8.7, §13 (`colour.fillMapping.get/set`; SI-025 placeholder).
- **Decisions:** `mapping` is `"default"`, `"clipboard"` or exactly one of `{matrix}`, `{linear: {start, end}}`,
  `{radial: {centre, radius}}`, `{texture: {origin, xAxis, yAxis}}`. `get` reports handles for gradients and textures
  and, for a solid colour, the matrix it keeps (or the box mapping); unpainted zones, invisible strokes and text refuse
  with `colour.notMappable` and a reason. `set` skips solid, unpainted, invisible, text and mismatched-kind objects with
  a reason. The fill-mapping clipboard is session state.

## SI-076 — centre line without a spine (found in M2 C2.3; resolved in M2 C2.4, ticket C23-T1)

- **Sections:** 02 §8.5.
- **Decision (orchestrator ruling: accepted as interim):** brush strokes that keep their spine become pencil lines
  along it at the pressure-average width; outlines edited directly (no spine) are kept and reported
  (`drawing.centreLineNoSpine`). The triangulated medial axis with branch pruning and colour splitting for spineless
  outlines is ticket **C23-T1**, owner M2 C2.4.
- **Resolution (M2 C2.4, C23-T1):** spineless outlines get the chordal axis of a constrained Delaunay triangulation
  (ADR-0016 history). `drawing.centreLineNoSpine` remains only for outlines the algorithm refuses: fewer than three
  distinct points, crossing contours (self-intersection), no enclosed area, or no axis left after pruning.

## SI-077 — feather colours (found in M2 C2.3; deferred by orchestrator, awaiting spec owner; ticket C23-T2, owner M7 C7.2)

- **Sections:** 02 §8 (Feather Edges), 03 §1.1 (indirect colour), 10 §4.9.
- **Problem:** feather bands need each source colour at decreasing opacity ((N-1)/N down to 1/N), but artwork
  references colour IDs only and has no object opacity.
- **Ruling:** `draw.feather.apply` moves to M2 C2.4 (generated declaration owner M2 C2.4, ticket **C23-T2**); the colour
  question goes to the spec owner; neither design is implemented.
- **Design A — per-band swatches in a dedicated system palette.** Feather creates (or reuses) swatches
  `<source name> 1/N … N-1/N` in a hidden system palette (like the hidden palette of 03 §1.2), one per source colour and
  band, holding the source's value in every style with the alpha scaled. *For:* no model change; renders, exports and
  recolouring work today. *Against:* palette growth (colours × steps, up to 100 per colour); editing the source swatch
  does not update the bands unless a link is added; Replace Colour must learn the family.
- **Design B — band-opacity model amendment.** A stroke or zone fill gains an opacity factor (`fill.opacity`, 0–1) applied
  on top of the referenced colour; feather bands reference the source colour ID with opacity k/N. *For:* bands follow
  swatch edits, style switches and Replace Colour automatically; no palette growth; reusable for other effects.
  *Against:* a 10 §4.9 schema amendment (migration, renderer and paint semantics, `colour.pick` reporting).
- **Recommendation:** Design B. Feather bands are derived appearance of the source colour, and indirect colour (03 §1.1)
  is the reason colours are IDs; a per-object opacity keeps that property, where Design A copies colour values and
  would drift on the first swatch edit.
- **Status at the M2 merge (orchestrator scope decision): deferred by orchestrator, awaiting the spec owner.** The spec
  owner has not answered, so Feather is re-scoped out of M2 as an explicit declared deferral: `draw.feather.apply`
  stays declared and refuses with `command.notImplemented`. Ticket C23-T2 is owned by **M7 C7.2 (effects), or the
  first checkpoint after the spec owner decides SI-077, whichever comes first** (generated declaration owner
  "M7 C7.2 (or the first checkpoint after the SI-077 decision)"). The spec owner may reverse this.

## SI-078 — the Cutter and text or bitmap nodes (found in M2 C2.3 round 1; decided)

- **Sections:** 02 §7.2.7 ("everything enclosed becomes selected"), §3.7, §3.9.
- **Decision (orchestrator ruling C23-R1-11):** text and bitmap nodes wholly enclosed by the cut boundary (their
  bounds' corners inside) are selected or deleted with the rest and reported by ID; partly enclosed ones are left whole
  and reported in `skipped` with reason `notClippable` (no glyph or bitmap clipping in M2). Brush strokes keep the
  SI-071 treatment. The Eraser leaves text and bitmap nodes alone.
- **Candidate:** clipping bitmaps (and glyph outlines) for M3/M4.

## SI-079 — selection and document history (found in M2 C2.3 round 3; decided)

- **Sections:** 02 §7.1, §13.1.10; 10 §2.7.
- **Decision (reviewer interpretation adopted as the ruling, M2-C2.3-r3):** the drawing selection (zones included) is
  transient session state, not part of document history. Undo and redo restore document geometry and zone identity;
  a selection pruned because its zone was re-identified stays pruned when undo brings the zone back, and the restored
  zone can be selected again by its original ID. Selection readback returns only valid IDs (§13.1.10). A call whose
  caller timed out leaves the selection as it was (ADR-0015 history, C23-R3-01).

## SI-080 — `VectorizeSettings` example versus the command rows (found in M2 C2.4; interim decided)

- **Sections:** 09 §11.2 (`VectorizeSettings` example `{"type": "blackWhite", "filter": "none", "threshold": 50}`);
  02 §11.2 and A (`draw.vectorize.*`: `type` `blackAndWhite` | `withTexture`, threshold default 70).
- **Interim decision (builder, C2.4):** the command rows govern (`blackAndWhite`, default 70); the 09 example is read
  as illustrative. `import.vectorize.settings.*` stay M10 C10.2 and will store the command-row values.
- **Question:** align the 09 example with the command rows.

## SI-081 — the `draw.vectorize.preview` PNG result limit (found in M2 C2.4; interim decided)

- **Sections:** A (`draw.vectorize.preview` result `{png, strokeCount}`, generated schema `png` max 65 536
  characters); 02 §11.2 (preview width default 512).
- **Interim decision (builder, C2.4):** the implemented result does not cap `png`: a 512-px preview of detailed line
  art exceeds 64 KiB of base64. The width parameter (1–4096) bounds the output.
- **Question:** raise or remove the limit, or return a `path` like `draw.drawing.render`.

## SI-082 — owner of `draw.vectorFile.import` (found in M2 C2.4; decided)

- **Decision (orchestrator ruling, C2.4 plan):** the registry row's original owner (M2 C2.4) disagreed with plan §4,
  which places vector import (PDF/AI/SVG) in M10 C10.2; plan §4 governs, and the declaration's owner is M10 C10.2.

## SI-083 — the `draw.vectorize.run` result reports skipped images (found in M2 C2.4; decided)

- **Sections:** A (`draw.vectorize.run` → `{elementId, drawingIds, completed, cancelled}`); 02 §11.2 (cancelling keeps
  completed drawings).
- **Decision (orchestrator ruling, C2.4 plan):** each image is its own undo step; the result also lists the images not
  committed, `skipped: [{index, source, reason: "occupied" | "cancelled"}]`, beside the spec's fields.
- **Question:** add `skipped` to the A result column.

## SI-084 — vectorization colours and palettes: 02 §11.2 versus 03 §11.6 and §7.4 (found in M2 C2.4 round 1; decided, amendment candidate)

- **Sections:** 02 §11.2 (Vectorize); 03 §11.6 (vectorization colours), §7.4 (texture swatch names); ADR-0018.
- **Problem:** 02 §11.2 gives black-and-white vectorization generated black in a reused palette named `Vectorization`,
  and texture vectorization swatches named from the file. 03 §11.6 instead uses the current colour for
  black-and-white and a new uniquely suffixed `Vectorization Palette` per import for texture, and 03 §7.4 names texture
  swatches `1`, `2`, ….
- **Decision (orchestrator ruling, C24-R1-05): 02 §11.2 takes precedence, as implemented** — generated black in the
  reused `Vectorization` palette; texture swatches named from the file (`draw_vectorize.rs::{black, texture}`;
  `c24_vectorize.rs` palette assertions). No behaviour change.
- **Question (amendment candidate for the spec owner):** align 03 §11.6 and §7.4 with 02 §11.2 (colour choice, palette
  naming and lifetime, swatch naming), or rule the other way.

## SI-085 — where the static light table is stored (found in M3 C3.1; interim decided)

- **Sections:** 04 §15.3 ("the panel contents are saved with the scene"), §18 #13 (light-table changes are not
  undoable); 10 §4.3 (the scene object has no light-table field), §6.12 (session state).
- **Problem:** "saved with the scene" and "not undoable" cannot both hold for a document property: every persisted
  property of the object model is part of revisions, change sets and undo (10 §2.5–§2.6).
- **Interim decision (builder, C3.1, the spec's recommended default "not undoable"):** the light table is per-scene
  view state of the session (`TimelineState.light_tables`, keyed by scene ID), outside revisions and undo;
  `xsheet.lightTable.list` reads it and items whose drawing or element no longer exists drop out (04 §15.3). Writing
  it into the project's session state (10 §6.12) arrives with `xsheet.lightTable.add/.remove/.set/.move` (M5 C5.2).
- **Question:** add a `lightTable` field to the scene (10 §4.3) and accept undo, or confirm session state.

## SI-086 — the hierarchy-cycle error code (found in M3 C3.1; decided by section ownership)

- **Sections:** 04 §9.2, §17.6 (`element.hierarchyCycle`); 10 §5 (`anim.hierarchyCycle`).
- **Decision (builder, C3.1):** each command uses its owning section's code: `element.parent.set` refuses with
  `element.hierarchyCycle`; the `anim.*` hierarchy commands of C3.2 with `anim.hierarchyCycle`.
- **Question:** name one code for both.

## SI-087 — the batch-count error code (found in M3 C3.1; decided by section ownership)

- **Sections:** 04 §3.2, §17.6 (`element.countOutOfRange`); 10 §5 (`element.tooMany`).
- **Decision (builder, C3.1):** `element.create` refuses counts outside 1-99 with 04's `element.countOutOfRange`
  (the count is validated by the handler, so the code is reachable; the schema states the range in its description).
- **Question:** drop `element.tooMany` from 10 §5 or rename.

## SI-088 — media and sound cells before M8 (found in M3 C3.1; interim decided, declared deferral)

- **Sections:** 04 §13.20 (media and sound columns accept several cell commands), §12.8; 10 §4.14–§4.15 (their cells
  derive from sound placements and media sequences, M8's model).
- **Interim decision (builder, C3.1):** cell commands that 13.20 allows on media or sound columns refuse them with
  `cell.notApplicable` naming the owner (`details.owner`: media → M8 C8.3, sound → M8 C8.1; `details.specIssue`).
  Two exceptions are implemented now: `cell.range.add {allElements: true}` moves sound placements and media
  sequences that start at or after the insertion point (and their lip-sync `producedFor.stopFrame`), so all columns
  stay in sync; one spanning the point is not split. `element.mediaLength.set {length}` sets the last sequence's
  length; without `length` (the media's natural length) it refuses with `command.disabled`
  (`reason: sourceLengthUnknown`, owner M8 C8.3). `xsheet.exposures.set` treats media columns as read-only
  (`xsheet.columnReadOnly`), like sound columns. `xsheet.exposures.get` reports media sequences as runs (drawing =
  the asset name, drawingId = the sequence ID) and every column's type as `elementType` (the 17.2 example writes
  `type` for the sound column only).
- **Question:** none for the spec owner; M8 C8.1/C8.3 implement the remaining sound/media cell edits.

## SI-089 — key re-timing when a bar's duration changes (found in M3 C3.1; decided by 05 §8.4 and the orchestrator's C3.2 ruling)

- **Sections:** 04 §11.10 (change duration removes keys beyond the end; stretching scales keys); 05 §8.4 (keys are
  local frames; proportional re-timing maps local frame k to round(1 + (k − 1)(D′ − 1)/(D − 1)), the later key wins
  a collision; truncation holds the value at the new end); 06 §2.6; 10 §4.5.
- **Decision (orchestrator ruling, C3.2 readiness item 7):** before truncation, evaluate the original curve at the new
  local end and preserve that held value there; moving the start must not shift local key coordinates.
  `element.timing.set {keyframes: "keep"}`: a 1-D channel with a key beyond the new cycle gets a key at the new end
  with the original curve's value there (also when every key lay beyond it); a motion path gets a keyframe at the
  position evaluated there, keeps the control points the node has already passed, drops the rest, and its velocity keys
  are renormalised to the shorter segment. `{keyframes: "scale"}` applies 05 §8.4's mapping with half-up rounding and
  later-wins collisions. Keys are local frames, so moving the start moves them with the bar.
- **Cycle length (decided, C3.2, readiness item 8):** `count`: L = ⌊duration ÷ loops⌋ (at least 1), a remainder plays as a
  partial cycle; `anim.loop.set` makes the bar `loops × L`, refusing past frame 100 000. `fill`: 10 §4.5 has only
  `duration` and `loops`, which cannot express L with a partial last cycle, so `Element.cycleLength` is added (optional,
  written only for `fill`, omitted otherwise; a `fill` element without it uses ⌊duration ÷ loops⌋). A new duration keeps L
  and `loops` becomes ⌈duration ÷ L⌉; an explicit `loops` in `element.timing.set` keeps the bar and L follows.
- **Question:** add `cycleLength` to 10 §4.5.

## SI-090 — drawing-name codes of 02 and 04 (found in M3 C3.1; decided by section ownership)

- **Sections:** 02 §9 (`draw.drawing.rename`: `drawing.nameExists`, `drawing.nameInvalid`); 04 §13.14, §17.6
  (`element.drawing.rename`: `drawing.nameEmpty`, `drawing.nameInvalid`, `drawing.nameDuplicate`).
- **Decision (builder, C3.1):** each command keeps its owning section's codes; `draw.drawing.rename` (M2, declared as
  delegating to `element.drawing.rename`) keeps `drawing.nameExists`, which its M2 tests pin.
- **Question:** unify the duplicate-name code.

## SI-091 — default drawing names: 10 §1.3 versus 04 §14.2 (found in M3 C3.1; decided for 04)

- **Sections:** 10 §1.3 (default pattern "the smallest positive integer that makes the name unique"); 04 §14.2 ("the
  next integer after the highest purely numeric drawing name in the bank"), 02 §1.1 (drawing into a blank cell uses
  14.2's name).
- **Decision (builder, C3.1):** the C3.1 commands (named cells, `element.drawing.create/.duplicate`,
  `cell.drawing.duplicate`, paste as new drawings) follow 04 §14.2 (`tt_anim::exposure::next_drawing_name`; after
  `1`, `3` comes `4`). M2's drawing-into-a-blank-cell path (`draw.rs::next_name`) keeps 10 §1.3 until the spec
  owner rules, so no M2 behaviour changes in C3.1.
- **Question:** which rule governs; then align the remaining path.

## SI-092 — which elements may have children: 04 §9.2 versus 10 §4.6 (found in M3 C3.1; decided for 04)

- **Sections:** 04 §2.1, §9.2 (drawing, image, media, sound and annotation elements cannot have children,
  `element.notParentable`); 10 §4.6 (drawing, annotation and image elements have children "yes (cut-out
  hierarchies)").
- **Decision (builder, C3.1):** the 04 commands (`element.parent.set`, `element.create {parentId}`) accept only pegs,
  cameras, skeletons, bones and effects as parents; documents whose trees already hang elements under drawings stay
  valid (the integrity gate follows 10 §4.6).
- **Question:** whether cut-out rigs (M6) parent under drawings; then align 04 §9.2 or 10 §4.6.

## SI-093 — cell notes through exposure edits (found in M3 C3.1 round 1; decided by orchestrator ruling)

- **Sections:** 04 §7.2 (notes stored per element per frame), §13 (cells shift), §13.16 (cut/copy/paste carry
  "labels, drawings, properties"); 10 §4.7 (`cellNotes`).
- **Decision (orchestrator ruling, C31-R1-05):** notes are frame-keyed element data that follow their cells. Every
  edit that moves cells moves their notes by the same mapping (`tt_anim::exposure::CellMap`: add/remove/set exposure,
  blank cells, insert/remove cells, cycles, inserted columnate, moves); notes of removed cells (cut, shifting delete,
  shortened exposures) go; clearing cells (`cell.range.delete {shift: false}`, cut without exposure change) clears
  their notes; copy/paste and copying moves carry notes and **overwrite** the destination's notes in the pasted range
  (an overwriting paste clears destination notes the copy does not carry); overwriting edits that move no cell
  (extend exposure, change label, `xsheet.exposures.set`, overwriting columnate) keep notes where they are. Each edit
  is one transaction with its exposure change; drawing and zone revisions are unchanged.
- **Extension (orchestrator ruling, C31-R2-04):** the generic `edit.cut`, `edit.cutSpecial` and `edit.delete` of
  cells use the same note edit as `cell.clipboard.set {cut: true}` (notes of cut or cleared cells go; a shifting cut
  moves later notes with the cells), in the same transaction. **Overflow:** an edit that would move a note it does
  not remove beyond frame 100 000 is refused as a whole (`cell.frameOutOfRange`, details `frame`, `movedTo`); the
  note and the exposure stay as they were. A shift never discards a note silently.
- **Question:** none for the spec owner beyond confirming the overwrite/clear rule.

## SI-094 — what duplicating an element copies (found in M3 C3.1 round 1; decided by orchestrator ruling)

- **Sections:** 04 §8.2 (an independent copy of the element, its drawings, exposure and transformation), §2.9–§2.10
  (parameter rows required by effects and skeletons); 10 §4.6 (`effectParameter` created and deleted with its owner).
- **Decision (orchestrator ruling, C31-R1-06):** `element.duplicate` copies the element's whole subtree: mandatory
  parameter rows (clipping masks, particle sprite rows with their sprite drawing element, skeleton bones and skin
  with their bones) and ordinary user children alike, each with a new ID, a unique `_n` name and, for cell-based
  elements, a new bank with verbatim drawing copies (banks shared inside the subtree stay shared between the copies);
  `x-ref: element` references between copied elements follow the copies. An element listed together with one of its
  ancestors is copied once, with that ancestor. A parameter row alone is refused (`arg.invalid`): it never becomes an
  unowned root.
- **Question:** confirm that ordinary children are duplicated with their parent (04 §8.2 is silent on children).

## SI-095 — columnate: total frames versus "expose last" (found in M3 C3.1 round 1; decided by orchestrator ruling)

- **Sections:** 04 §13.10 (total frames recompute the loops; the last loop is truncated to the requested count;
  "expose last drawing for" the final drawing of the whole cycle).
- **Decision (orchestrator ruling, C31-R1-01):** total frames take precedence. With `totalFrames`, loops =
  ceil(frames ÷ frames-per-loop) (frames-per-loop uses `exposeEach` for every drawing) and the sequence is generated
  far enough to reach the requested count before the last loop is truncated, so a short `exposeLast` never leaves a
  shortfall; `exposeLast` applies only when the requested count ends exactly where the untruncated cycle ends. A
  single pass (`loopMode: none`) is never extended: a larger `totalFrames` is refused (`arg.invalid`, the pass length
  stated), a smaller one truncates it.
- **Question:** confirm, or specify how `exposeLast` combines with a truncated cycle.

## SI-096 — per-axis position channels, D-5 (raised in the M3 plan; unresolved, spec owner)

- **Sections:** 05 §1.6, §9, §11.4; 10 §4.12.
- **Decision (orchestrator ruling, C3.2):** build to 05 as it stands: `position` is a motion path with TCB points,
  `position.x/y/z` are projections, not stored. M6 C6.2 is the second owner.
- **Migration if the amendment is adopted:** a format step (versioned manifest, a fixture folder under
  `crates/persist/tests/fixtures/migrations/`) rewrites each `position` channel into `position.x`, `.y`, `.z` 1-D channels.
  Each keyframe becomes one key per axis at the same local frame, with an ID derived deterministically from the point's;
  control points and non-linear velocity are baked into sampled per-axis keys at each frame of the affected segment
  (TCB parameters have no per-axis form, so a segment with non-default TCB is baked too); `constantSegment` and
  `lockDepth` map to `constantSegment` per axis; `meta`/`ext` move to the `x` channel. `velocity` keys are dropped once baked.
- **IPC types the amendment would break (not frozen; repeated in the C3.2 handoff):** `anim.path.get/.set/.point.*`,
  `anim.path.lockDepth.set`, the `position.*` projection form of `anim.function.*`, `NodeEval.animated.velocity`.
- **Question:** D-5 itself.

## SI-097 — E4's bones (found in M3 C3.2; decided by the plan's milestones)

- **Sections:** 11 §14 E4 ("create body-part elements, pegs and bones"); plan §4 (C3.2 "E4 numeric"; C6.3 bones, "E4
  interactive"); generator owners (`anim.bone.*`, `anim.skeleton.*`: M6 C6.3).
- **Decision (builder, C3.2):** E4 numeric poses pegs and elements. The bone part of E4 runs with M6 C6.3.
  `anim.evaluate` refuses a bone (`anim.notTransformable`, hint names C6.3).
- **Question:** none.

## SI-098 — `anim.evaluate` frames and frame reporting (found in M3 C3.2; decided by the readiness list)

- **Sections:** 05 §19.1.4 ("fractional only where stated"), §19.3 (`frames?: [int]`).
- **Decision (builder, C3.2; readiness item 10):** `anim.evaluate {frames}` also accepts fractional scene frames, a superset
  of the table. Every other frame parameter is as its table row says. All results report scene frames, including
  `anim.path.get`, `anim.function.get`, `anim.key.list`; `NodeEval.localFrame` is the local frame before the cycle mapping.
- **Question:** confirm the superset.

## SI-099 — Constant Z, constant segments and timing (found in M3 C3.2; decided by 05 §9.3)

- **Sections:** 05 §9.2, §9.3 ("The Z of control points is ignored where Constant Z applies"), §10.5.
- **Decision (builder, C3.2):** the arc length of a locked keyframe segment is measured in its X/Y plane, so control-point Z
  does not change timing there. A control point inside a constant segment is "ignored for timing": its reported passage
  frame is the segment's start frame.
- **Question:** confirm the plane measure.

## SI-100 — visibility, clipped points and frame corners (found in M3 C3.2; decided for agents)

- **Sections:** 05 §2.6, §19.3, §19.4 (the `NodeEval` example gives a peg `bounds` and a visible `screen`; the text below it
  says pegs and cameras report only their origin).
- **Decision (builder, C3.2):** the text wins: pegs and cameras have no `bounds`; their `screen` box is their origin.
  `visible` = in front of the near plane and overlapping the frame; for content nodes also not hidden (itself or an
  ancestor), not an annotation (04 §2.6, final render excludes them by default) and with content at that frame. In
  `anim.projection.get` a clipped point reports `x`, `y` as `null`. In `anim.camera.get`, `frameCornersAtZ0` is `null` when
  the table plane is at or behind the near plane. `bounds` come from the exposed drawing at 1 field = 100 du (02 §3);
  bitmap native-size mapping (05 §1.3) arrives with C3.3's render list.
- **Question:** confirm `visible` for pegs (origin in frame).

## SI-101 — numeric bounds of animation values (found in M3 C3.2; decided by the builder)

- **Sections:** 05 §2.1, §8, §18 (`anim.valueOutOfRange`); C31-R1-10.
- **Decision (builder, C3.2):** beyond the spec's ranges (skew ±89, |scale| ≥ 0.001, FOV 1–179, TCB ±1): positions and pivots
  ±1 000 000 fields, |scale| ≤ 1 000 000, rotation and generic 1-D values ±1e9. All `anim.valueOutOfRange` with limits, so
  finite documents cannot overflow casually; an evaluation that still overflows (a deep chain of large scales) is refused
  with `reason: nonFinite`.
- **Question:** none beyond confirming the limits.

## SI-102 — default FOV and "exactly 12 fields" (found in M3 C3.2; ruled: exact expression)

- **Sections:** 05 §2.7 (2·atan(0.375) ≈ 41.1121°, "exactly fills"), §12.3, §19.3, 10 §4.6 (default 41.1121).
- **Ruling (orchestrator, M3 C3.2 round 1, C32-R1-03; spec-owner confirmation pending):** exact expression; the tables'
  41.1121 is its display rounding; the spec owner may amend the prose to ≈. The stored and schema default is
  2·atan(0.375) in degrees at full double precision, `41.11209043916693`, computed once through the pinned soft-float
  path (`tt_model::defaults::DEFAULT_FOV`, bits checked by `crates/anim/tests/c32_round1.rs`); tables and the property
  panel display it to 4 decimals (`DEFAULT_FOV_DISPLAY`); an explicitly set 41.1121 is stored and read back as 41.1121.
  New cameras, new projects and the M1 sample fixture carry the exact value; schemas, generated IPC and example
  results are regenerated. Files storing 41.1121 stay valid: it is a migratable number, not a format change. (Until
  round 1 the builder kept 41.1121, which filled the frame to about 3e-6 field.)
- **Question:** none; the projection of an explicit 41.1121 is SI-104.

## SI-103 — default curve types (found in M3 C3.2; decided by 05 §10.2)

- **Sections:** 05 §10.2, §19.4 (the `Function` example is a `spline`); 10 §4.12 ("per preference").
- **Decision (builder, C3.2):** a new channel is `interpolation`, its keys eased `linear`, or `easeInOut` when
  `staging.keyframes.linearVelocity` is off. A later `anim.function.curveType.set` to `spline` starts with zero handles,
  which are straight lines. `anim.function.*` on `position.x/y/z` reads keyframes and edits values through the path
  (05 §11.4); key addition, removal and duplication there are refused (`anim.channelUnsupported`, use `anim.path.*`).
- **Question:** none.

## SI-104 — an explicit 41.1121 is not an alias of the default (found in M3 C3.2 round 1; ruled: no alias)

- **Sections:** 05 §2.7, §12.3, §19.1.6; SI-102's ruling; the round-1 reviewer's `c32_r1_default_frame`, which projects
  a camera at the tables' 41.1121 and requires 12 fields within 1e-6.
- **Decision (builder, C3.2 round 1):** the ruling asks both that an explicitly set 41.1121 stays 41.1121 and that the
  reviewer's oracle, which projects exactly that value, passes unchanged. A double at 41.1121 is 9.6e-6° wider than
  the exact default, 3.0e-6 field at distance 16, so projecting it as itself cannot meet the oracle. The projection
  therefore resolves the one value 41.1121, the spec's own written default, to the exact expression
  (`tt_anim::camera::projection_fov`); the stored `fov` and the reported `fov` and `effectiveFov` keep 41.1121, and
  every other value, 41.11211 included, projects as itself. A user who types the tables' default gets the default frame.
- **Ruling (orchestrator, 2026-11-09, C32-R2-01):** no alias; the reported value is the projected value. An explicit
  41.1121 is stored, reported and projected as itself (about 12.0000031 fields at distance 16); only the default
  camera, which carries the exact `DEFAULT_FOV`, spans exactly 12 fields. Round 2 removed
  `tt_anim::camera::projection_fov`; `effectiveFov` and every projection path use the same number. The reviewer amended
  `c32_r1_default_frame` to project `DEFAULT_FOV` and added `explicit_display_fov_is_not_aliased`.
- **Question:** none. (The round-1 builder decision above is superseded.)

## SI-105 — bitmap placement and native size (found in M3 C3.3; decided by 10 §4.9 and 05 §1.3)

- **Sections:** 10 §4.9 (`transform` maps pixel space, origin top-left, y down, to drawing space), 05 §1.3 (native size:
  one image pixel per output pixel through the default camera, 12 ÷ camera width fields per pixel), 06 §6.8.
- **Decision (builder, C3.3):** the renderer and hit test honour any stored transform; *native size* is
  `[s, 0, 0, −s, tx, ty]` with `s = 1200 / cameraWidth` du per pixel (the y flip keeps the image upright; 100 du per
  field unchanged). Importers (M10) write that placement. The M1 fixture's sample node `[1, 0, 0, 1, −960, 540]` is
  schema sample data: read by 10 §4.9 it lies above the frame, upside down (the compositor reports it `outOfFrame`).
  Fully transparent pixels are not hit. `c33_bitmap` proves the pixel-for-pixel mapping and that bounds, render and hit
  test agree.
- **Question:** none.

## SI-106 — composite order with roles (found in M3 C3.3; decided by the builder)

- **Sections:** 05 §2.8 (depth sort, Timeline tie-break), 04 §6.1 (foreground/background composite in front of/behind
  all normal elements; background auto-extends), 04 §9.1, 10 §4.3 (tree children top to bottom as displayed).
- **Decision (builder, C3.3; depth ties by ruling C33-R1-01):** order = role tier (background; normal and envelope;
  foreground), then world Z of the origin (smaller first), then the Timeline order: back to front is the displayed rows
  bottom to top (index 0 of a level is the front; a parent's row is above its children, so it is in front of them at
  equal depth). Depths are not rounded: within a tier the nodes are sorted by Z and grouped from the smallest; a group
  takes each following node whose Z is less than 1e-6 field from the group's *first* (smallest-Z) member, and a group
  is ordered by the Timeline alone. Chain rule: a long run of near-equal depths splits where the distance from the
  group's anchor reaches 1e-6, even though each step is smaller (0, 0.6e-6, 1.2e-6 is two groups).
  A background element extends its last exposure to the scene length at render time. Envelope elements draw as normal
  until bones (SI-097).
- **Question:** confirm the role tiers outrank depth.

## SI-107 — `render.frame.run` coverage (found in M3 C3.3; decided by the builder)

- **Sections:** 09 §11.1–§11.4, 11 §7.5, §7.8, 06 §6.10.
- **Decision (builder, C3.3):** compositions `camera` and `element` (`elementIds` through the camera; `drawingId` in
  drawing space at the camera's table-plane scale, colours through the scene's styles); `xsheet` (owner M5 C5.1) and
  `activeView` (owner M4 C4.2) are refused `render.compositionUnavailable` with the owner. `quality: "draft"` renders as
  final with a `render.draftAsFinal` warning until the interactive renderer (M4 C4.2). `cameraFrame` has no effect on
  compositions that are the camera frame (warning `render.cameraFrameNotApplicable`). `cameraId` is accepted so the
  generic `render_frame` tool's `camera` keeps its meaning; the tool's `output` path writes a file and `sceneColour` is
  `scene`. Overlays are never drawn in final renders. `RenderResult` adds `frame`, `imageToCamera` and `warnings`, and
  returns `renderMs` (ruling C33-R1-06; also in the generic `render_frame` result). Volatile fields (`renderMs`, job
  timestamps, job IDs) are excluded from deterministic byte comparisons; the PNG bytes and their `sha256` are the
  determinism contract. `include`/`exclude` (element refs) are a render-time visibility override on
  `render.frame.run`, `render.range.run`, `inspect.contactSheet.render` and `inspect.frame.describe` (ruling C33-R1-05):
  an included element is drawn although hidden or an annotation, an excluded one is left out with its descendants
  (verdict `excluded`; exclusion wins), and the document is never changed. Known output sizes are checked before any
  evaluation, allocation or encoding: an inline image (a frame or a contact sheet) whose long side or pixel count is
  over the inline limit is `render.inlineTooLarge`, a general resource limit is `limit.exceeded`; the encoded-byte check
  stays after encoding (ruling C33-R1-04). `render.range.run` without `wait` answers `{jobId}` (`job.wait` waits); with
  `wait: true` the caller's request waits for the job off the document thread, bounded by the call's budget and
  cancellable, and answers the manifest with its `jobId`; at the timeout it answers `job.waitTimeout` with the `jobId`,
  and the job runs on (ruling C33-R1-02). Its companion manifest `<scene>_<from>-<to>.manifest.json` is written with the
  frames, only when every frame is published: publication is a transaction (ruling C33-R1-03; ADR-0020).
- **Question:** none.

## SI-108 — contact-sheet layout and labels (found in M3 C3.3; decided by the builder)

- **Sections:** 11 §7.5 `render_contact_sheet`, §15.2 `inspect.contactSheet.render`.
- **Decision (builder, C3.3):** columns default ⌈√n⌉ (at most 64, at most n); cell height by the camera aspect; 4-pixel
  gaps; a 14-pixel label band under each cell with the frame number drawn from built-in 3 × 5 digit bitmaps at 2 × 2
  pixels per dot (no fonts, so the bytes are the same on every OS); white sheet, cells with the scene background; each
  cell is exactly the frame rendered at the cell size. Results add `bytes`, `sha256`, `documentRevision`, `warnings`.
- **Question:** none.

## SI-109 — coordinate-space names (found in M3 C3.3; decided by the builder)

- **Sections:** 11 §5.3, §7.5, §15.2.
- **Decision (builder, C3.3):** `convert` accepts `drawing`, `local`, `parent` (these with `elementId`), `scene` (also
  `world`, `field`), `camera` (fields from the camera centre on the table plane, y up, camera roll applied),
  `cameraNormalized` (0..1 from the frame's top-left), `image` (pixels of a render at the camera size); image and camera
  points go onto the plane Z = `z` (default the table plane). `hitTest` accepts `image`, `camera` and `scene` (table
  plane); `drawing` names no element and is refused (`inspect.spaceUnavailable`, convert to `scene` first); `view` waits
  for views (owner M4 C4.2). Points at or behind the near plane are `inspect.behindCamera`; a singular transform
  (unreachable through the scale and skew limits) would be `inspect.singularTransform`. Inspection frames are integers,
  as the registry rows bind them (`frame?: int`); a fractional frame is a typed `arg.invalid` (C3.3 round 1).
  `inspect.frame.describe` takes the same `include`/`exclude` as `render.frame.run` and describes that render (SI-107).
- **Question:** none.

## SI-110 — generic `render_frame` default width (found in M3 C3.3 round 1; decided by ruling)

- **Sections:** 11 §7.5 (`render_frame`), 09 §11.1 item 1 (the generic tool equals the owning command).
- **Decision (ruling, C3.3 round 1):** parity wins: the generic `render_frame` tool maps to `render.frame.run` with
  `composition: "camera"`, and its default width is the owning command's, the camera width; the tool schema says
  "default = the camera width". No product change beyond the schema text.
- **Question:** none.

## SI-111 — a caller-cancelled request has no code in 11 §10 (found in M3 C3.4 round 2; interim, awaiting spec owner)

- **Sections:** 11 §4 (MCP cancellation: no answer to a cancelled request), 11 §10 (error codes), 09 §11.1 item 4
  (`wait: true`).
- **Interim decision (orchestrator ruling, C3.4 round 2):** reuse `command.timeout` with `details.reason: "cancelled"`
  (plus `details.jobId` on waited-job paths). An MCP client gets no answer to a cancelled request, so the code is seen
  only by in-process callers, batch step envelopes and the activity log. The contract is ADR-0020 decision 8.
- **Question:** add a code for a request its caller cancelled to 11 §10, or confirm `command.timeout` with
  `reason: "cancelled"`.

## SI-112 — virtual view creation and view ids (found in M4 C4.1; decided by the builder)

- **Sections:** 11 §7.6 ("the agent can create them with `ui.view.create {type, width, height}` (section 01)"), 01 §12.2
  (`view.*`), spec A.
- **Problem:** `ui.view.create` is named in 11 but exists in neither 01 §12.2 nor spec A; `viewId` has no defined
  value.
- **Decision taken:** the view windows are the four view panels of 01 §5.2 (`cameraView`, `drawingView`, `topView`,
  `sideView`), in attached and headless mode alike, and `viewId` is the panel id; each window can switch its type
  (01 §4). No creation command is added (the registry stays equal to spec A). Owner of any change: M4 C4.2.
- **Question:** confirm, or add `ui.view.create` to 01 §12.2 and spec A with its semantics.

## SI-113 — facts only the shell knows have no command (found in M4 C4.1; decided by the builder)

- **Sections:** 01 §12.1 rule 8, §12.4 (`modal`), §9.1 (description and pointer segments), 11 §6.4, §7.5
  (`inspect_ui`).
- **Problem:** modal state, a gesture in progress, text-field focus, hover description, pointer text, view canvas
  sizes, screens and the rendered UI tree are read by commands but set by nothing in spec A.
- **Decision taken:** the desktop host reports them through a kernel API (`ExecutorHandle::report_shell`), not a
  command; a report changes no document, setting, layout or history, and agents cannot send one (ADR-0021,
  `reviews/M4-design-busy-arbitration.md`).
- **Question:** confirm that these are shell inputs outside the command layer.

## SI-114 — two names for one user-busy state, and the main toolbar's visibility (found in M4 C4.1; decided by the builder)

- **Sections:** 01 §12.4 (`modal`), §12.1 rule 8 (`ui.busy`); 11 §6.4 (`get_state.ui.userBusy`, `ui.modalOpen`);
  01 §5.7 and §7.3 (`interface.mainToolbar.visible`, toolbar configuration per workspace).
- **Decisions taken:** `app.state.get` reports `modal` as 01 defines it (`ui.userBusy` is not added: it is `modal`
  being non-null); agent changes that conflict with a dialog or a gesture wait and fail with `ui.busy`, and
  focus-changing agent `ui` commands under a user dialog fail with `ui.modalOpen` (both codes, each where its section
  says). The main toolbar's visibility follows `interface.mainToolbar.visible` in both directions (`window.toolbar.set
  {toolbarId: "main"}` writes the preference); its items stay per workspace.
- **Question:** confirm, and say whether toolbar visibility is per workspace or global.

## SI-115 — UI conventions awaiting spec amendments where the spec is silent (found in M4 C4.1; defaults taken)

- **Context:** the orchestrator's design review lists conventions that wait for spec-owner approval (reviews/
  M4-C4.1-plan.md §12). Where 01 already requires a behaviour it is built; where it is silent, nothing is invented.
- **Silent in the spec, not built (attach points recorded in the plan §12):** an add-panel control on every tab bar,
  collapsible dock areas, maximise one panel, a panel menu in every header, a message log panel, preferences
  export/import, a shortcut sheet, user-saved resolution presets on the start screen, focus-follows-pointer, tap/hold
  tool keys, bundled alternative keymap sets.
- **Additive members taken now:** `ui.theme.list` items carry `colours` for user themes (the front end needs the
  tokens; 01 §12.2 lists `{themeId, name, builtIn, dark}`).
- **Question:** approve or reject each convention so the owning checkpoint can build it.

## SI-116 — `waitForUserMs` has no command, toggles bound without their state (found in M4 C4.1; decided by the builder)

- **Sections:** 01 §12.1 rule 8, 11 §6.4 (`waitForUserMs`, default 10 000); 01 §8.1 (a binding is a command reference
  with fixed parameters); 01 §8.2 (full screen F11 / Ctrl+Cmd+F toggles).
- **Problems:** no command sets `waitForUserMs`; the toggle commands take an explicit state (`window.fullScreen.set
  {enabled}`, never a toggle; readiness bullet 4), yet a key or a menu item toggles.
- **Decisions taken:** `waitForUserMs` is an executor setting (default 10 000 ms) the host and tests set; a binding or
  menu item that leaves a toggle's state parameter out gets the negation of the current state at dispatch (the kernel
  completes the reference; `window.panel.set {panelId}` shows a hidden or background panel and hides a front one).
- **Question:** add `waitForUserMs` to `agent.policy.defaults.set` or the session grant; confirm the toggle completion.

## SI-117 — preferences other sections use without a key in 01 §7.3 (found in M4 C4.1; decided by the builder)

- **Sections:** 05 §16 (depth-nudge step, peg-only mode), 03 (paint gap closing), 02 (drawing preview), 01 §7.3.
- **Decision taken:** keys in the 01 style: `staging.depthNudgeStep` (number 0.0001–1, default 0.01),
  `staging.pegOnlyMode` (bool, default off), `paint.gapClosing` (enum off/small/medium/large, the kernel key of
  `colour.gap.mode.set`), `drawing.inspectorPreview` (bool, the kernel key of `draw.inspectorPreview.set`).
  `interface.background.timeline`'s "theme default" is the empty string. Motion-recording options (05 §14.2) are
  registered by M6.
- **Conflicts noted:** "Selection always visible" defaults to on in 01 §7.3 and 05 §16, off in 02 §10.7; 01 governs.
- **Question:** add the keys to 01 §7.3 and settle the default of 02 §10.7.

## SI-118 — a splitter drag has no user command (found in M4 C4.1; interim, awaiting the spec owner)

- **Sections:** 01 §5.4 and §5.6 (a workspace stores "sizes" of the dock layout), 01 §12.2 (`window.panel.move
  {size?}`, `workspace.layout.set`), spec A.
- **Problem:** the only user command that carries a dock size is `window.panel.move`, whose `size` applies when a
  panel is docked. Resizing a dock area by dragging its splitter, with every panel staying where it is, has no
  command; `workspace.layout.set` is agent-only in the registry ("agents apply prepared layouts; users arrange panels
  by dragging"). The GUI never mutates state outside a registered command.
- **Interim decision (orchestrator rulings O-3 and O-3a, M4 C4.1 round 1):** at the end of a splitter drag the GUI
  runs the existing user command `window.panel.move` as a same-place call: `{panelId: P, area: <P's present area>,
  relativeTo: P, position: "tab", size}` (a panel tabbed with itself, a call the declaration already admits). The kernel
  leaves the arrangement exactly as it is (no re-insertion, no tab reorder, no front-tab change, no size migrating to
  another group) and applies `size` to P's area; without `size` it changes nothing and emits no `workspace.changed`.
  `relativeTo: P` with any other `position`, or an `area` that is not P's present area, stays an error. No command is
  added; the registry stays equal to spec A. (O-3 first used re-insertion calls, which existed for only 3 of the built-in
  workspaces' side areas; O-3a replaced them so every area behaves the same.)
- **Question:** add a user command for dock area sizes (for example `window.dock.size.set {area, size}`), or make
  `workspace.layout.set` a user command as well.

## SI-119 — no message id for a settings file that could not be saved (found in M4 C4.1 round 1; interim)

- **Sections:** 01 §11 (settings files, `prefs.corrupt`), 01 §12.2 message list.
- **Problem:** 01 §11 names `prefs.corrupt` for a file that cannot be read, but no id for a write that fails (an
  unwritable or full settings folder). Ruling O-12 requires the user to be told once per file per session, with the
  path and the operating system's reason, while the change stays in memory.
- **Interim decision:** `prefs.saveFailed` (severity warning; details `kind`, `path`, `reason`, `message`), also used
  when a per-user folder cannot be created. It has a help entry like every other message id.
- **Question:** add `prefs.saveFailed` (or another id) to 01 §11 and the message list.

## SI-120 — the answer to a request cut off by an engine restart (found in M4 C4.1 round 1; interim)

- **Sections:** 11 §10 (generic codes), 11 P5 (never block), reviews/M4-design-engine-restart.md.
- **Problem:** when the user restarts an engine that stopped answering, a request still in flight on the old engine
  has no specified answer. Its outcome is unknown (the old engine may have applied it, and may still apply it if it
  wakes up, though it is fenced from writing recovery data).
- **Interim decision (ruling O-14):** `command.timeout` with `details.reason: "engineRestarted"`,
  `details.generation` (the generation the request started on) and `details.currentGeneration`, by analogy with the
  caller-cancelled request of SI-111. The window shows it as a status line ("the engine restarted while it ran; check
  the result"), never as applied.
- **Question:** confirm, or define a dedicated code (for example `engine.restarted`) in 11 §10.
