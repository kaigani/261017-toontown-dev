# ADR-0017 — Planar map: noding, zones, sides, fills, gap closing, painting and colour recovery

Status: accepted (M2 C2.2, 2026-10-31). Scope: plan §4 row M2, checkpoint C2.2; the C2.2 readiness list of
`reviews/M2-C2.1-r3.md` (confirmed by `reviews/M2-C2.1-r4-closure.md`). Spec: 02 §3.5, §3.11, §3.12, §8.8, §13.4–§13.6;
03 §1, §7, §8, §10, §11, §15, §16; 10 §4.8–§4.11; spec A (`colour.paint.*`, `colour.pick`, `colour.gap.*`,
`colour.recovery.*`, `colour.missing.list`, `draw.hitTest.get`). Rulings: SI-059 … SI-062 (orchestrator, C2.2 plan).

## Context

C2.1 delivered strokes, text, groups and bitmaps as validated, persisted geometry, with zones reported as unavailable
(SI-057). 02 §3.12 requires every edit to re-node the affected layers, recompute their zones and carry colours over;
03 §8 requires painting by ID and by point with gap closing; 03 §11.2 requires colour recovery on paste and import;
10 §4.9 persists zone fills on the sides of the bounding strokes and derives zones on load.

## Decision

1. **Persistent noding (`tt_vector::planar::node`; SI-059).** Within a layer, pencil and invisible strokes are split
   where they cross any stroke (themselves included) and where an open stroke's end meets another stroke within the
   junction tolerance (1/32 du). Pieces meet at one shared coordinate (shared anchors) and get new IDs. A junction
   within the tolerance of an existing anchor uses it, so a noded layer stays noded and unchanged strokes keep their
   IDs; only strokes the edit changed have their ends moved. **Brush outlines are never split or moved**: their
   crossings are vertices of the derived graph only, so a spine never goes stale through noding (SI-053/SI-054
   intact). Pieces are not merged back when a crossing stroke is deleted.

2. **Derived graph (`planar::graph`).** Strokes are flattened within 1/128 du (vertices carry their path parameter) and
   put on the 1/64 du integer grid of the brush union. The arrangement is snap rounded (every end and proper crossing is
   a hot pixel; edges through a hot pixel break there), so edges meet only at vertices and every predicate is exact
   i128 arithmetic — identical on every OS. Coincident pieces of several strokes share an edge. Faces are traced with the
   face on the left; counter-clockwise cycles bound faces; each component's outside is a hole of the smallest
   containing face of another component (grid index of face boxes, exact point in polygon). A face inside a brush
   outline (non-zero winding of its edges) is that stroke's fill; every other bounded face is a **zone** with area,
   bounds, holes (with the zone inside each), parent zone, an exactly verified interior point and its boundary pieces.

3. **Zone IDs** are `zn_` + 26 Crockford base-32 characters of a SHA-256 of the zone's canonical boundary (bounding
   stroke IDs with sides, and for brush outlines the bounding stretches by path parameter). They change exactly when the
   boundary changes (02 §3.12 item 4) and are identical across OSes (`c22_determinism`).

4. **Fills (`planar::fill`; SI-060).** A zone's fill (colour ID + mapping matrix, never baked RGBA) is stored on the
   matching side of every pencil and invisible piece bounding it; disagreement resolves to the longest bounding length
   with `drawing.fillInconsistent`. A side bounding no zone keeps what it holds; an unchanged meaning keeps its stored
   form; brush strokes' own sides are never rewritten. A zone bounded only by brush-outline stretches stores its fill in
   the brush stroke's `ext["dev.toontown.zones"].exterior` records (side, parameter range, fill), rewritten
   deterministically on every update and absent when not needed; without that key a brush stroke's own side fills are
   read (other writers). **Carry-over:** a zone keeping its ID keeps its fill; any other new zone takes the fill of the
   old zone it overlaps most (exact grid polygon intersection), ties to the lower old zone ID — splits keep the colour
   on both parts, merges take the larger part's.

5. **Settling (`planar::layer`, `Cx::settle`).** Every transaction runs the settle pass before any preview or commit
   (and handlers call it before reporting IDs): each layer whose stroke identity, kind, closed flag, points, sides or
   exterior records changed since the last pass — or that a command painted — is noded, its map built, fills carried
   (or read from the staged sides when only fills changed) and painted fills applied; the result is staged in the same
   transaction. Undo, redo and checkpoint restore install already settled states. Paint commands register fills by zone
   ID; the pass stores them. The pass of an atomic batch starts from the previous pass's result.

6. **One work budget (`work::PLANAR_WORK_BUDGET`, 1.2 × 10⁹ units)** covers noding, the graph, carry-over and gap
   closing of all layers of a pass, charged before each step (pair tests, pieces, edges, half-edges, zones, hole tests,
   stored strokes). Past it the call fails with `limit.exceeded` and a hint; nothing is staged, no revision, undo step or
   event. Calibrated on `crates/vector/tests/c22_worst_case.rs`: worst accepted ≈ 2.5 s (one stroke added to a
   100 000-stroke layer), worst refusal ≈ 5 s (a 600 × 600 grid of crossing lines, 360 000 zones) in local release;
   every observation retained, both OSes in CI's headless job.

7. **Derived-map cache.** Maps are cached per session by the SHA-256 of a layer's geometry (fills do not change a map),
   least recently used out, bounded to 512 maps and 8 × 10⁶ vertices plus half-edges; results never depend on it.

8. **Painting (03 §8, §16).** `colour.paint.apply` targets zone, line, brush stroke and text IDs, points (front-most
   object: layers front to back, lines before brush strokes before zones), crossing paths, and (SI-061) lassos and rects
   (zones whose interior point lies inside); `onlyUnpainted`, repaint `onlyPainted`, protected colours (swatch
   `ext["dev.toontown.paint"].protected`, refused per target with `colour.protected` unless `respectProtected: false`),
   explicit `colourId` defaulting to the current colour, gradient/texture mapping by drag or default box (SI-064).
   `colour.paintUnpainted.apply` (IDs, points, lasso, all) and `colour.paint.remove` (zones unpainted; lines become
   invisible strokes; brush strokes and text `colour.cannotUnpaint`). A call that paints nothing fails with its first
   refusal; partial success lists the rest in `skipped`. Zone-ID targets with a stale `expectRev` →
   `colour.staleZoneId`; point targets are re-resolved. `colour.pick` reads the colour, palette and fill under a point;
   `draw.hitTest.get` lists hits front to back with the zone.

9. **Gap closing (03 §8.6, §8.8; SI-062, SI-063).** The mode is the persisted user setting `paint.gapClosing`
   (`colour.gapClosing.set/get`) with the per-command `gapClosing` override; tolerances 3/8/16 view px, 1 px = 1 du at
   100 % zoom. Candidates join open stroke ends to the nearest end and the nearest stroke point within the tolerance;
   the region under the point must become a zone of at least 4 × tolerance²; only candidates on its boundary are kept
   as invisible strokes (`addedStrokeIds`). `colour.gap.close` joins the end (else stroke point) nearest each end of a
   drag within max(16 du, half the drag), else `colour.noGapFound`.

10. **Colour recovery (03 §11.2; 10 §4.11).** Missing colours are recreated with the same ID from recovery copies —
    into the visible palette with the recorded name (styles added by name, every swatch given a value), else a new
    palette of that name (shown in every scene with its first style), or `Recovered` — else as `Deleted colour`
    placeholders (opaque mid-grey). `draw.content.add` recovers in the paste's own transaction from the content's
    `colourRecovery` copies (else the project's); `colour.recovery.run` on demand ("Colour Recovery"),
    `colour.missing.list` reports, `colour.recovery.performed` announces. Zone fills travel with pasted content (each
    zone's fill painted at its interior point). Each drawing's recovery list is refreshed on save to one entry per
    colour it uses.

11. **Read-back and events.** `draw.drawing.get` lists each layer's zones in the §13.4 shape and pencil/invisible
    strokes' `leftZoneId`/`rightZoneId`; zone IDs select zones in `ids`; `doc.object.get` derives drawing `zones`
    (10 §4.8); `doc.validate` reports `unpaintedZones` (zones with a visible bounding stroke and no fill) with a
    `colour.paint.apply` fix. `draw.drawing.changed` adds derived zone changes — new zone IDs created, vanished ones
    removed, repainted ones modified — for every commit, undo, redo and restore included. Edit results report the
    pieces of split strokes and the zones created and removed.

## Consequences

- C2.3 builds rendering, flatten/optimize and the remaining `draw.*` commands on the same maps and fills; C2.4 owns
  faster graph building at hundreds of thousands of strokes (R1-12 budgets).
- The spec owner may add a zone-fill side table to 10 §4.9 (replacing SI-060's extension key), a protected flag to
  03 §1.4 (SI-061), and decide a per-drawing gap setting (SI-062, M4 C4.2).
- Recovery while loading (03 §11.2 "loading drawings") and the load-time `drawing.fillInconsistent` repair run on
  paste, on demand, on save and on the first edit of a layer in C2.2; automatic recovery inside persistence's load
  repair is ticketed (handoff §6).
