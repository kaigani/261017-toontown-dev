# ADR-0010 — Object model, identifiers and addressing

Status: accepted (M1 C1.1, 2026-10-18); amended after review C1.1 round 1 (2026-10-18: decisions 1, 4, 11, 12, 13,
14) and round 3 (decision 4). Spec: 10 §1–§5, design decisions D1, D3, D4, D5, D6, D9; 11 §5.1.

## Context

Spec 10 defines one JSON object model shared by the files, the in-memory document and the agent interface (DM-1).
Several of its design decisions (§11) and a few conflicts between sections had to be settled before code could be
written. Everything later (executor, persistence, MCP, every area's commands) builds on these choices.

## Decision

1. **Identifiers (D1, ID-02).** A generated ID is `<prefix>_<ULID>`: a type prefix (`pr`, `pal`, `sty`, `col`, `pen`,
   `as`, `db`, `dr`, `nd`, `st`, `sc`, `tr`, `el`, `ch`, `ky`, `mp`, `blk`, `pl`, `ms`, `lib`, `lf`, `li`), an
   underscore and a 26-character ULID (48-bit millisecond time, 80 random bits from the OS, Crockford base 32). IDs
   from one generator are strictly increasing (monotonic ULIDs). Any string of 1–64 `[A-Za-z0-9_-]` is accepted on
   input; clients must not parse IDs. Swatches use `col_` as 10 §4.13 and 03 §12.3 do (ID-02's `sw_…` is an example
   of an opaque ID, not a rule). Cloning an `IdGenerator` yields a handle to the **same** sequence: clones share one
   monotonic state behind a mutex, so no two clones can return the same ID (C11-R1-20).
2. **Container-local IDs (ID-03).** Drawing nodes and strokes are unique within their drawing, keys and motion
   points within their channel; their project-wide form is `<containerId>/<localId>`. New objects nevertheless get
   full ULIDs, so an ID is never reused after deletion (ID-01); short local IDs such as `s42` are accepted from files.
3. **Determinism vs. random IDs (P7).** The product generator uses OS randomness and the clock, so two runs of the
   same commands create different IDs (ID-02 requires uniqueness across projects). A seeded generator
   (`IdGenerator::seeded`) gives reproducible IDs for fixtures, golden files and replay tests; renders, canonical
   JSON and change sets are otherwise deterministic.
4. **Read-only model types.** Every entity field is crate-private with a read accessor; values are created only by
   validated parsing (`object::from_json`) or by constructors that return new values. No type has a public
   `&mut self` method. The open document is an immutable [`Document`] snapshot whose entries (the package split of
   LAY-01: project, palettes, drawing banks, drawings, scenes, elements, local library) are behind `Arc`s, so a
   snapshot clone costs pointers and two snapshots share unchanged entries (copy-on-write, D15, 11 §16.5). Only the
   executor (C1.2) holds the current snapshot and derives the next one inside a transaction.
   `Document::from_parts` is the validated constructor and does not trust its parts: entity types also have public
   `Deserialize` impls, so every entry is run through `object::from_json` again (depth limit, normalisation, schema,
   canonical form, object-local rules; C11-R1-02), and an entry ID given twice is `id.duplicate`, never a silent
   overwrite (C11-R1-01; `assemble` keeps the first entry and reports the dropped one for the C1.3 repair path). This
   costs a full validation of every entry (1.7–2.4 s for the 121 092-object scale document), so the executor must
   derive snapshots by revalidating only the entries a transaction touched (C1.2). Undo and redo invert a
   transaction's patches with `change::apply_with_inverse` / `invert_against`, against the state they applied to;
   there is no stateless inversion (C11-R3-02, ADR-0011 §7).
5. **One struct per object type, strict.** Serde with `deny_unknown_fields`; field declaration order is schema order
   (JSON-02). Type-specific element blocks (10 §4.6) are optional fields that are **absent** when they do not apply
   (never `null`); nullable properties are written explicitly as `null` (JSON-04). Unknown properties of newer minor
   versions are handled by the load path (C1.3), not by catch-all fields.
6. **`type` collisions.** The envelope owns `type`. Where another section names a property `type`, the persisted name
   follows D6 (`elementType`): transitions `transitionType` (08 §13.1), particle blocks `particleType` (06 §8.4), pens
   `kind` (`brush`/`eraser`, 02 §6.1). Command parameters may keep the short names (D6). SI-004.
7. **Exposure (D3).** Runs `[drawingId | null, count]`, canonical (merged, no trailing blank, count ≥ 1), normalised
   on every input; per-frame access via `cell:#n`.
8. **Scene length (D4)** is derived (`Document::scene_length`), never stored; cell-based elements' `start` and
   `duration` are derived too and only appear in query results.
9. **Per-element display state (D5)** (marker/background colours, column width, collapsed) is in the document.
10. **Numeric precision (D9).** No quantisation; numbers are IEEE doubles written in the shortest round-trip form
    (ADR-0011).
11. **Depth and stubs.** `depth` counts object levels returned in full: 1 = the object with its child objects as
    `{id, type, name}` stubs (the example of 10 §10.4), `n` = n levels, 0 = the object itself as a stub, -1 = all.
    Value objects (exposure, transform, params, envelopes) are always complete. The persisted JSON of an entry is the
    same serialiser with inline children in full and split children as stubs (DM-1). SI-011. Every result validates
    against its type's schema at every depth, the depth-0 stub included (10 §2.2, C11-R1-11; ADR-0011 §5).
12. **Paths (10 §2.4).** The grammar is implemented as written; aliases print as primary type names; a canonical
    path uses a quoted name only where that name selects exactly one object by exact match, otherwise `type:@id`.
    A drawing's canonical path goes through the first element (Timeline order) of its clone group. `drawing:#n` uses
    natural name order (10 §4.8), `element:#n` Timeline order. `motionPoint` is an addressable type below the
    `position` channel, and `key:"n"` there selects the keyframe at frame n (SI-006). Special references
    (`@current`, …) need application context and resolve from C1.2 on. Paths have at most 100 segments and 64 KiB,
    and objects at most 100 levels of nesting (`tt_model::input::MAX_MODEL_DEPTH`), so every object the model
    accepts has a canonical path the parser resolves (C11-R1-10).
13. **Superset enums and extra properties where sections disagree**: element `role` has `normal`, `foreground`,
    `background`, `envelope` (SI-003); palettes carry `hidden` and the project `autoColourUpdate` (SI-005); effect
    `params` are typed per effect (06 §3), particle effects have none (SI-007); image `importOptions` is an opaque
    object until section 09 defines it (SI-008).
14. **Values the spec fixes are not persisted.** Camera transforms carry no pivot fields: 05 §2.3 fixes the camera's
    rotation pivot at (0, 0) and makes scale and skew pivots not applicable. The drawing schema's `x-command`
    annotations name the 04 drawing-management commands (`element.drawing.rename`, `element.drawingLock.set`); the
    02 commands for the same operations stay registered, and whether one is an alias of the other is a registry
    decision for C1.2.
15. **Validated input (C11-R1-03/05/12/15).** Every object enters through `object::from_json`: a 100-level depth
    limit, then a schema-directed pass (`tt_model::input`) that NFC-normalises every string at a declared position
    (10 §1.1), trims model-object names (10 §1.3), maps a swatch's `id` to `colourId` (10 §2.2) and puts keys in
    canonical order; `meta`, `ext` namespaces and other undeclared (opaque) values are never interpreted — no string
    inside them changes, no embedded `type`/`name` is read. Then schema validation, typed parsing, canonicalisation
    (exposure runs; spline handle offsets clamped as 05 §10.3) and the object-local rules, which include the entity
    invariants of 10 §4: closed brush outlines (§4.9), a width for fixed-width text (§4.9), file links with both paths
    and library links with both IDs (§4.16), texture images under `textures/` (§4.13), manual mouth ranges within the
    placement (§4.14) and mouth-label and cell-note keys that are frames 1–2 147 483 647 (§1.1, §4.7, §4.14; labels
    also within the placement). Declared values of pattern maps (cell notes, labels) are NFC-normalised too
    (C11-R2-08/10). Document-level rules add: each drawing of an image element is exactly one bitmap node
    (§4.10), at least one visible palette (03 §1.2), the project library has kind `local` (§3, §4.1).

## Consequences

- Every crate reads the model through accessors and `&` borrows; mutation is the executor's capability, which C1.2
  proves with compile-fail tests.
- Seeded fixtures make golden files stable across machines (`crates/model/tests/golden/`).
- Readers must treat `rev`, sizes and seeds as 64-bit integers (ADR-0012 carries them losslessly to the front end).
