# ADR-0016 — Vector engine: strokes, fitting, brush outlines, text and the C2.1 drawing commands

Status: accepted (M2 C2.1, 2026-10-30; revised in C2.1 round 1, `reviews/M2-C2.1-r1-response.md`). Scope: plan §4 row M2, checkpoint C2.1; the twelve C2.1 obligations of
`reviews/M1-C1.4-r4-closure.md`. Spec: 02 §2–§3, §6, §13; 10 §4.8–§4.11; 03 §2.4.

## Context

M1 delivered the drawing entities as validated, persisted data (10 §4.8–§4.11) with no geometry: strokes, nodes and
text could be loaded, saved and patched, but nothing built or checked their geometry. 02 §3 defines the drawing model
in motion: freehand fitting that never depends on timing, brush outlines swept from spine, pressure and pen, text with
runs and paragraphs, groups, bitmaps, a pivot, and the identity and revision rules of §3.12. 02 §13 makes every one
of these a command. The planar map (zones) is C2.2, drawing operations and rendering C2.3, vectorization C2.4.

## Decision

1. **Crate split.** `tt-vector` is a pure library over the `tt-model` entities: geometry, fitting, outlines,
   symmetry, text content and layout, the §13.4 read-back, content input and the §3.12 invariants. It does not stage
   anything. The `draw.*` handlers live in `tt-executor` (`commands/draw*.rs`) and stage every change through
   `Cx::patch_entries`, so each commit runs the complete C1.1 integrity gate; no unchecked assembly is added.

2. **Targeting (02 §13.1 item 4; SI-056).** Every C2.1 drawing command takes `drawingId`, else `elementId` (default:
   current element) and `frame` (default: current frame), else the current drawing. Creation commands take
   `createDrawing` (default true): an empty addressed cell gets a new drawing — next unused integer name in the bank,
   the cell's exposure changed — in the same transaction and undo step (02 §1.1). A locked drawing refuses every edit
   (`drawing.locked`). Commands that edit objects by ID accept `expectRev`, the drawing revision; a mismatch is
   `drawing.staleRevision` (§3.12). IDs may be local or `<drawingId>/<localId>` (ID-03). The object setters
   (`draw.stroke.set`, `draw.text.set`, `draw.textFormat.set`) resolve their drawing through the same common target
   resolution; a compound ID that names a different drawing than the addressed one is `arg.invalid` (C21-R1-03).
   Every edit result carries `{drawingId, rev}` and `createdIds`, `modifiedIds`, `removedIds` (local IDs of the
   nodes and strokes the edit created, changed and removed; §13.1 item 11, C21-R1-08), next to the row's own members.
   Every commit that changes drawing content — commands, `doc.object.patch`, batches, undo, redo and checkpoint
   restore — emits `draw.drawing.changed` `{drawingId, rev, commandId, createdIds, modifiedIds, removedIds}` from
   the common commit path, derived from the transaction's change records (§13.6, C21-R1-07).

3. **Determinism (ADR-0004).** Geometry uses only `+ − × ÷ sqrt` in a fixed order. Sines, cosines and arc cosines come
   from the pure-software `libm` port (`force-soft-floats`; pinned by a golden bit-pattern test); multiples of 90° are
   exact. Unions run on an integer grid. IDs come from `Cx::new_id` (seeded by project and revision). No iteration over
   hash maps affects output. Command-created geometry and canonical entries are compared across runs and against a
   committed golden digest on Windows and macOS (`c21_determinism`). Text layout bounds depend on the installed font
   files and are derived data; persisted text is identical everywhere.

4. **Curve fitting (02 §3.3).** Consecutive samples within tolerance/16 of the last kept one are merged first
   (spatial dedupe; pressures averaged); corners (turns sharper than 60° between neighbours at least one tolerance
   away, searched within 64 samples each way, so the search is linear) split the samples; each run is fitted by least
   squares on chord-length parameters with up to four Newton reparameterisations, and split at the worst point when
   the error exceeds the tolerance `0.25 du + 0.5 du × smoothness` (less the merge distance). The error is measured
   against the input polyline in both directions — samples and segment midpoints to the curve, interior curve points
   to the polyline — and a two-sample run shrinks its handles until the curve stays within the tolerance of its
   chord (C21-R1-01). Runs deeper than 40 splits become straight segments. A product fit is bounded to 400 million
   sample visits (each about one cubic evaluation; the adversarial 100 000-sample corpus needs at most 201 million
   and 0.5 s in release, `crates/vector/tests/c21_worst_case.rs`); input that needs more is `limit.exceeded`, so the
   document worker is never held past the watchdog (obligation 11, C21-R1-06). Pressures are smoothed with a centred moving average of half-width
   `round(smoothness)` and stored as `(t, p)` at each sample's normalised arc length along the fitted spine, then
   simplified within 0.001 (Douglas–Peucker). `fit: false` joins the samples with straight segments; `bezier` is used
   verbatim. A single sample is a dab.

5. **Brush outlines (02 §3.4; SI-053, SI-054).** The tip is placed at every pressure-map knot (with that knot's
   pressure; a step in pressure places both sides) and adaptively along the spine: each segment is flattened by its
   curvature (Wang's bound) within 1/64 du, one grid step. Between consecutive placements the spine is then a chord
   and the width linear in arc length, so no pressure extremum or bend falls between two tips; at most 100 000
   placements, else `limit.exceeded` (C21-R1-02). Width is `min + p × (max − min)`; each pair
   of consecutive placements contributes the convex hull of the two tips (the exact sweep of a convex tip scaled
   linearly along the chord). Round tips are polygons circumscribing the circle (so the union contains every swept
   disk); oval tips 2:1 ellipses (also circumscribed) and flat tips 5:1 rectangles at the tip angle; square tips
   squares. The hulls are united by `i_overlay` (MIT OR Apache-2.0) with integer arithmetic on a 1/64 du
   grid (coordinates are bounded to ±10⁷ du so the engine's i32 range holds). Contours are joined by zero-width bridges
   into the one closed path 10 §4.9 stores, then fitted within min(0.1 du, max/16). The spine, its pressure map and
   pen are kept (a closed spine stored open with its first anchor repeated); a non-zero tip angle of a non-round tip
   goes to `ext["dev.toontown.brush"].tipAngle`. A spine or pressure edit sweeps the outline again; an outline edit
   discards the spine (§3.2) — through `draw.stroke.set` and through `doc.object.patch` alike (§13.6). One edit that
   changes both the outline and the spine is refused with `arg.invalid`, `details.pointers: ["/points", "/spine"]`
   and `reason: "outlineAndSpine"`; there is no silent precedence (orchestrator ruling on C21-R1-04).

6. **Text (02 §3.7, §7.6; SI-055).** Content edits work on code points: typing continues the run before the cursor,
   deletions merge paragraphs into the first, formats split and merge runs, and there is one paragraph per line.
   After every edit the text is normalised to NFC first (10 §1.1) and the runs and paragraphs are then remapped onto
   the composed code points: a combining mark joins the run of the code point it composes with, also across a run
   boundary or an insertion, and an insertion that composes with the code point before it extends that code point's
   run (C21-R1-09). The model's integrity gate requires the paragraphs to cover the text, one per line, in order, for
   every writer including `doc.object.patch` (C21-R1-05).
   Layout: the transform's origin is the box's top-left corner and lines run downwards; advances and `kern` pairs come
   from the font (`skrifa`; `ttf-parser` was declared unmaintained, RUSTSEC-2026-0192), line height from ascender, descender and line gap plus the paragraph's line spacing;
   fixed width wraps greedily at spaces. System fonts are listed once per process (`fontdb`, read on a background thread
   at start; files are memory-mapped while listed, so the scan touches only their tables, not whole files). A missing font keeps its name and is laid out with the platform's default sans-serif face (warning
   `drawing.fontMissing`); `doc.validate` reports it as `missingFonts`. New text uses the generic family `sans-serif`.

7. **Layer policies and settings (02 §3.1, §7.3.6, §8.11; SI-056).** `layer: auto` puts brush, pencil and shape strokes
   in a new top layer (a new back layer under Draw Behind) and an invisible stroke into the top-most layer with a
   stroke it crosses, else a new top layer; `new`, `top`, `behind` and a layer ID as named. Symmetry copies (Mirror,
   Kaleidoscope with 2n images) are built in the same step into the same layer. The current pen, colour, symmetry and
   Draw Behind are defaults until their commands (M4 C4.2, M7 C7.1) exist; the engine paths are live.

8. **Groups, delete, bitmaps, content (02 §3.6, §3.9, §8.14; SI-052).** Grouping takes objects of one node list;
   selected strokes move into a new layer inside the group; the group takes the place of the front-most member.
   Ungrouping puts the members back in place and order. Deleting every stroke of a layer removes the layer.
   `draw.content.add` takes the §13.4 shape (a `draw.drawing.get` result is accepted as is), assigns new IDs, keeps
   colour IDs (unknown ones are reported as `colour.unresolved` until recovery, C2.2) and inserts bitmaps that name an
   image of the document.

9. **Read-back (02 §13.4; SI-057).** `draw.drawing.get` returns the §13.4 shape with a closed, hand-written result
   schema; `include`, `flatten` and `ids` select parts. Zones and side zone IDs are absent until C2.2 and every such
   result carries `doc.derivedUnavailable`. Answers above 32 MiB are `limit.exceeded` with a hint (bounded payloads).

10. **Executed examples (obligation 1).** Every example of every implemented command is executed on the fixture and its
    result retained (`crates/registry/src/example-results.json`, shown by `describe_command`); the kernel namespaces
    have worked end-to-end examples through the MCP server (`crates/mcp/src/worked-examples.json`, rendered in
    `app://docs/<ns>`). Both are re-executed and compared on every OS; values that differ between runs (IDs, times,
    tokens) and font-dependent bounds are recorded as varying.

## Consequences

- C2.2 replaces the `doc.derivedUnavailable` zone warnings with the planar map and must treat outline bridges as
  coincident edges.
- C2.3 renders from this model (`render_drawing` over ADR-0006), adds the remaining `draw.*` commands and the gesture
  equivalence of §13.3 on top of the same `tt-vector` builders.
- The spec owner may amend 10 §4.9 (several outline contours, spine tip angle and `closed`, outline colours); the
  persisted form stays migratable through the C1.3 chain.
