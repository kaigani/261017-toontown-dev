# ADR-0010 — Object model, identifiers and addressing

Status: accepted (M1 C1.1, 2026-10-18). Spec: 10 §1–§5, design decisions D1, D3, D4, D5, D6, D9; 11 §5.1.

## Context

Spec 10 defines one JSON object model shared by the files, the in-memory document and the agent interface (DM-1).
Several of its design decisions (§11) and a few conflicts between sections had to be settled before code could be
written. Everything later (executor, persistence, MCP, every area's commands) builds on these choices.

## Decision

1. **Identifiers (D1, ID-02).** A generated ID is `<prefix>_<ULID>`: a type prefix (`pr`, `pal`, `sty`, `col`, `pen`,
   `as`, `db`, `dr`, `nd`, `st`, `sc`, `tr`, `el`, `ch`, `ky`, `mp`, `blk`, `pl`, `ms`, `lib`, `lf`, `li`), an
   underscore and a 26-character ULID (48-bit millisecond time, 80 random bits from the OS, Crockford base 32). IDs
   from one generator are strictly increasing (monotonic ULIDs). Any string of 1–64 `[A-Za-z0-9_-]` is accepted on
   input; clients must not parse IDs. Swatches use `col_` (10 §4.13, 03 §12.3; the `sw_` in ID-02's example is an
   example only).
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
    same serialiser with inline children in full and split children as stubs (DM-1). SI-011.
12. **Paths (10 §2.4).** The grammar is implemented as written; aliases print as primary type names; a canonical
    path uses a quoted name only where that name selects exactly one object by exact match, otherwise `type:@id`.
    A drawing's canonical path goes through the first element (Timeline order) of its clone group. `drawing:#n` uses
    natural name order (10 §4.8), `element:#n` Timeline order. `motionPoint` is an addressable type below the
    `position` channel, and `key:"n"` there selects the keyframe at frame n (SI-006). Special references
    (`@current`, …) need application context and resolve from C1.2 on.
13. **Superset enums and extra properties where sections disagree**: element `role` has `normal`, `foreground`,
    `background`, `envelope` (SI-003); palettes carry `hidden` and the project `autoColourUpdate` (SI-005); effect
    `params` are typed per effect (06 §3), particle effects have none (SI-007); image `importOptions` is an opaque
    object until section 09 defines it (SI-008); cameras have no pivot fields (SI-009).

## Consequences

- Every crate reads the model through accessors and `&` borrows; mutation is the executor's capability, which C1.2
  proves with compile-fail tests.
- Seeded fixtures make golden files stable across machines (`crates/model/tests/golden/`).
- Readers must treat `rev`, sizes and seeds as 64-bit integers (ADR-0012 carries them losslessly to the front end).
