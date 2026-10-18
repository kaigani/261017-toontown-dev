# Spec issues

Ambiguities, contradictions or gaps found in `spec/` during implementation. One entry per issue:
section, quote, problem, decision taken (ADR link) or question for the spec owner.

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
  `envelope`, so 04/05's roles can be stored. Commands that accept only two values keep their narrower parameter.
- **Question:** confirm the four roles belong in 10 §4.5 and `anim.node.options.set`.

## SI-004 — properties named `type` collide with the envelope `type`; pen fields (found in C1.1; decided)

- **Sections:** 10 §4.4 / 08 §13.1 transition `type`; 06 §8.4 block `type` (particle type); 02 §6.1 pen "Type"
  (brush/eraser) and "Tip angle", absent from 10 §4.2; 10 §4.2 names the tip `tip`, 02 §6.1's command `tipShape`;
  smoothness is `int` in 10 §4.2 and `number` in 02 §6.1.
- **Decision taken (ADR-0010, following D6 for `elementType`):** persisted names `transitionType`, `particleType`,
  pen `kind` (`brush`/`eraser`) and pen `tipAngle` (0–360, default 0); `tip` as in 10; smoothness `int` 0–10 as in 10
  (10 owns the persisted format). Command parameters may keep the short names.

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

## SI-007 — effect naming and the drop-shadow transform shape (found in C1.1; decided)

- **Sections:** 06 §3.8.1 / §8.4 call the particle effects `natureParticles` / `actionParticles`; 10 §4.5 uses
  `particleNature` / `particleImpact`. 06 §3.4 gives the drop-shadow transform as `{xAxis, yAxis, origin}`, 06 §7 as
  a "2×3 matrix".
- **Decision taken:** 10's element type names are persisted; `params.transform` uses 06 §3.4's parameter shape
  (a 2×3 affine written as axes and origin). Particle effects have no effect-level parameters (`params: {}`).

## SI-008 — `importOptions` of image elements is undefined (found in C1.1; open)

- **Section:** 10 §4.6 ("plus `importOptions` (section 09)"); section 09 defines no such object.
- **Decision taken:** an opaque JSON object, preserved verbatim, until section 09 (M10) defines it.

## SI-009 — camera transform pivots (found in C1.1; decided)

- **Section:** 10 §4.6 assigns `rotationPivot`/`scalePivot`/`rotateAboutPivot` to "Elements" and `pivot`/
  `applyPivotTo` to pegs and skeletons; cameras (05 §12.3: offset, roll, FOV) are not covered.
- **Decision taken:** camera transforms carry no pivot fields.

## SI-010 — swatch ID prefix example (found in C1.1; decided)

- **Section:** 10 ID-02 example `sw_…`; 10 §4.13 and 03 §12.3 examples `col_…`.
- **Decision taken (ADR-0010):** generated colour IDs use `col_` (IDs are opaque either way).

## SI-011 — meaning of `depth` in `doc.object.get` (found in C1.1; decided)

- **Section:** 10 §10.2 (`depth: int = 1 (-1 = all, 0–20)`), 11 §7.4; only the depth-1 example of 10 §10.4 defines it.
- **Decision taken (ADR-0010):** depth counts object levels returned in full: 1 = the object with child objects as
  stubs (matches the example), n = n levels, 0 = the object itself as a stub, -1 = all. Value objects are complete
  at every depth.

## SI-012 — two commands per drawing rename and lock (found in C1.1; decided for `x-command`)

- **Sections:** `element.drawing.rename` (04) and `draw.drawing.rename` (02); `element.drawingLock.set` (04) and
  `draw.lock.set` (02).
- **Decision taken:** the drawing schema's `x-command` names the 04 commands, which own drawing management; both
  stay registered (C1.2 decides whether one is an alias).
