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
