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
   junction tolerance (1/32 du): with the nearest stroke and every stroke as near (an end where a split stroke's pieces
   meet and a line passes through is a junction with all of them). **Every crossing decision is exact on the snapped
   1/64 du grid** (round 2, C22-R2-01). Segment ends are snapped and classified by `i128` orientation signs:
   - strictly opposite signs on both segments: a proper crossing;
   - an end lying on the other segment: a vertex case;
   - both ends of one segment on the other's line: collinear, never a junction.

   A vertex case is a junction only when the second stroke's two directions away from the point lie strictly on
   different sides of the first's (exact sector tests on grid directions). A touch, a turn-back or any shared direction
   (overlap, retracing) is not a junction. Segments that are collinear on the grid are merged by the derived graph's
   snap rounding as well, so persistent noding and the graph agree. Floating point is used only to place a proper
   crossing's point on the stored geometry, with the exact grid fractions where floating point degenerates; there is no
   epsilon. The endpoint (T-junction) pass applies the same classification (round 3, C22-R3-03): an open end lying on
   another stroke within the tolerance is a junction only when its grid direction is not parallel to that stroke's
   segment there (exact `i128` sign). Collinear overlap, retracing and continuation never create junctions, from
   either pass. **Each junction is resolved once for all of its
   participants** (round 1, C22-R1-01): a crossing or T-junction within the tolerance of a junction's first record
   belongs to that junction (no chaining). Its coordinate is the nearest existing anchor of an unchanged participant,
   else the first record's point, and every participant's pieces meet exactly there. A junction within the tolerance
   of an existing anchor therefore uses it, so a noded layer stays noded and unchanged strokes keep their IDs. A cut at
   the same place as an open path's end becomes that end's junction, so no piece has zero length. **Brush outlines are never split or moved**: their
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

3. **Zone IDs** are `zn_` + 26 Crockford base-32 characters of a SHA-256 of the zone's canonical boundary. Each boundary
   cycle is read in traversal order: the snapped integer coordinate of each vertex, followed by the stroke IDs and sides
   of the edge leaving it. A vertex where the boundary goes straight on along the same strokes is left out, the cycle
   starts at its smallest token sequence, and hole cycles follow sorted (round 1 ruling, C22-R1-03). IDs change
   whenever the boundary's geometry or strokes change (02 §3.12 item 4, 03 §16.1 item 5), not under unrelated edits.
   They are identical across OSes (`c22_determinism`, the history oracles' independent computation).

4. **Fills (`planar::fill`; SI-060).** A zone's fill (colour ID + mapping matrix, never baked RGBA) is stored on the
   matching side of every pencil and invisible piece bounding it; disagreement resolves to the longest bounding length
   with `drawing.fillInconsistent`. A side bounding no zone keeps what it holds; an unchanged meaning keeps its stored
   form; brush strokes' own sides are never rewritten. A zone bounded only by brush-outline stretches stores its fill in
   the brush stroke's `ext["dev.toontown.zones"].exterior` records (side, parameter range, fill), rewritten
   deterministically on every update and absent when not needed; without that key a brush stroke's own side fills are
   read (other writers). **Carry-over:** a zone keeping its ID keeps its fill; any other new zone takes the fill of the
   old zone it overlaps most, ties to the lower old zone ID only on mathematically equal areas. Splits keep the colour
   on both parts; merges take the larger part's. Candidates are every old zone whose box overlaps the new zone's with
   positive area (C22-R1-02). An old zone with a new zone's ID has the same polygon and keeps its fill without an
   overlay. **Overlap areas are exact rationals** (round 2, C22-R2-04; `planar::exact`). By Green's theorem, twice
   the area of `A ∩ B` is the sum over the cut boundary of `(t₂ − t₁)·cross(p, q)`, where p → q is an edge and
   `[t₁, t₂]` a piece of it. Each edge of A is cut where it meets B's boundary, and its pieces count when they lie
   inside B, or along B's boundary with B's interior on the same side. Each edge of B is cut the same way, and its
   pieces count when they lie strictly inside A. Segment tests and cut parameters are `i128` ratios (they fit for
   coordinates within ±10⁷ du); midpoint inside-tests and sums use `num-bigint`/`num-rational`, charged to the pass's
   account. Holes and non-convex zones are handled by the same rule. A property test checks the areas against an
   exact Sutherland–Hodgman reference.

5. **Settling (`planar::layer`, `Cx::settle`).** Every transaction runs the settle pass before any preview or commit
   (and handlers call it before reporting IDs): each layer whose stroke identity, kind, closed flag, points, sides or
   exterior records changed since the last pass (compared as model values, so untouched layers are never serialised)
   — or that a command painted — is noded, its map built, fills carried
   (or read from the staged sides when only fills changed) and painted fills applied; the result is staged in the same
   transaction. Undo, redo and checkpoint restore install already settled states. Drawings an import creates are taken
   as they stand, like a load (no noding until their first edit; C1.3 import fidelity). Paint commands register fills by zone
   ID; the pass stores them. The pass of an atomic batch starts from the previous pass's result.

6. **One work budget (`work::PLANAR_WORK_BUDGET`, 1.2 × 10⁹ units)** covers noding, the graph, carry-over and gap
   closing of all layers of a pass, charged before each step (pair tests, pieces, edges, half-edges, zones, hole tests,
   stored strokes). Past it the call fails with `limit.exceeded` and a hint; nothing is staged, no revision, undo step or
   event. Calibrated on `crates/vector/tests/c22_worst_case.rs`: worst accepted ≈ 2.5 s (one stroke added to a
   100 000-stroke layer), worst refusal ≈ 2.3 s (a 600 × 600 grid of crossing lines, refused once noding has fixed its
   720 000 pieces, before the graph) in local release;
   every observation retained, both OSes in CI's headless job.

7. **Derived-map cache.** Maps are cached per session by the SHA-256 of a layer's geometry (fills do not change a map),
   least recently used out, bounded to 512 maps and 8 × 10⁶ vertices plus half-edges; results never depend on it.

8. **Painting (03 §8, §16).** `colour.paint.apply` targets zone, line, brush stroke and text IDs, points (front-most
   object: layers front to back, lines before brush strokes before zones), crossing paths, and (SI-061) lassos and rects
   (zones whose interior point lies inside); `onlyUnpainted`, repaint `onlyPainted`, protected colours (swatch
   `ext["dev.toontown.paint"].protected`, refused per target with `colour.protected` unless `respectProtected: false`),
   explicit `colourId` defaulting to the current colour, gradient/texture mapping by drag or default box (SI-064) for
   zones, brush strokes and lines (SI-068). A solid colour preserves the stored matrix (C22-R1-09). A crossing path is
   traversed continuously through each layer's map (`LayerMap::along`: cut at every edge crossing, the face between
   crossings). All targets of one call share one work account (C22-R1-04). When gap closing changes the topology,
   every earlier target is resolved again and applied with the closed ones in the same transaction (C22-R1-08).
   `colour.paintUnpainted.apply` (IDs, points, lasso, all) and `colour.paint.remove` (zones unpainted; lines become
   invisible strokes; brush strokes and text `colour.cannotUnpaint`). A call that paints nothing fails with its first
   refusal; partial success lists the rest in `skipped`. Zone-ID targets with a stale `expectRev` →
   `colour.staleZoneId`; point targets are re-resolved. `colour.pick` reads the colour, palette and fill under a point;
   `draw.hitTest.get` lists hits front to back with the zone.

9. **Gap closing (03 §8.6, §8.8; SI-062, SI-063).** The mode is the persisted user setting `paint.gapClosing`
   (`colour.gapClosing.set/get`; the `prefs` scope, written as a staged preference effect, C22-R1-07) with the
   per-command `gapClosing` override; tolerances 3/8/16 view px, 1 px = 1 du at
   100 % zoom. Candidates join open stroke ends to the nearest end and the nearest stroke point within the tolerance;
   the region under the point must become a zone of at least 4 × tolerance²; only candidates on its boundary are kept
   as invisible strokes (`addedStrokeIds`). `colour.gap.close` joins the end (else stroke point) nearest each end of a
   drag within max(16 du, half the drag), else `colour.noGapFound`.

10. **Colour recovery (03 §11.2; 10 §4.11).** Missing colours are recreated with the same ID from recovery copies —
    into the visible palette with the recorded name (styles added by name, every swatch given a value), else a new
    palette of that name (shown in every scene with its first style), or `Recovered` — else as `Deleted colour`
    placeholders (opaque mid-grey). `draw.content.add` recovers in the paste's own transaction from the content's
    `colourRecovery` copies (else the project's); `colour.recovery.run` on demand ("Colour Recovery"),
    `colour.missing.list` reports, `colour.recovery.performed` announces. `doc.import.json` recovers in the import's
    own transaction. **On open** (C22-R1-05), recovery and the repair of disagreeing side fills are part of the repair
    step: one "Repair on Open" step and a new revision; `colour.recovered`, `colour.placeholderCreated` and
    `drawing.fillInconsistent` appear in the load report; IDs come from a content-seeded generator (`load_repair.rs`).
    Zone fills travel with pasted content (each zone's fill painted at its interior point). Each drawing's recovery list
    is refreshed on save to exactly one entry per colour it uses. A colour with neither a definition nor a copy gets a
    `Deleted colour` placeholder entry (C22-R1-06).

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
- Recovery and side-fill repair run on load, paste, import, on demand and on the first edit of a layer (round 1 ruling
  withdrew ticket C22-T1).

## History

- **M2 C2.3 round 1 (C23-R1-08): one exact predicate module.** `tt_vector::planar::predicates` holds orientation,
  proper crossing and containment on the snapped grid in i128; the planar graph, noding and exact overlap use it
  instead of private copies, and the drawing operations that classify geometry against the map — the Cutter and
  Eraser (`ops::cut`), Optimize's overlap tests and lasso selection — decide through it too. Corpus:
  `crates/vector/tests/c23_predicates.rs` (determinant ±1 far from the origin, endpoint contact, collinear overlap,
  containment on and off the boundary).
- **M2 C2.4 (readiness item 2): defects found by the topology properties, fixed.** (1) A repeated anchor (zero-length
  segment) made the flattened edge after it span that segment's parameters, so a junction on the edge was cut inside
  the zero-length segment and the piece after it doubled back (a sliver zone): each flattened vertex now also records
  where it is last left (`Flat::out`), and edges run from there. (2) A piece between two cuts that resolve to one
  junction (a zero-length loop) is dropped; its neighbours meet at the junction. (3) A piece cut from a straight
  segment stays straight between its junction ends (corner handles): the junction parameter is interpolated along the
  flattened chord, which a straight cubic's parameter does not follow, so the de Casteljau handles overshot the moved
  end and folded back (micro-zones beside coincident edges). (4) A path passing through a junction more than once is
  cut at every pass. (5) Fill carry-over ignores an overlap no larger than the snapping bound — twice the length of
  the new zone's boundary inside the old zone's box, in grid units — so a sliver created by snapping a new junction
  never carries a neighbour's fill; a zone with no overlap above the bound takes the fill of the old zone holding its
  interior point. Larger overlaps are still compared exactly (C22-R2-04). Ticket C24-T1 — side fills of zones
  bounded by collinearly overlapping or retracing strokes (not split there, C22-R3-03) could read back differently after
  an unrelated edit — is resolved below (7, 8). Evidence: `crates/vector/tests/c24_properties.rs`, `crates/executor/tests/c24_topology.rs`.
- **M2 C2.4 (readiness item 4, C22-T2): self T-junctions; incremental noding not shipped.** (6) An open stroke's end
  touching its own path (outside the Bézier segment the end belongs to) is a junction like an end touching another
  stroke: before, the touch was found only after another crossing had split the stroke, so settling was not a fixpoint.
  Incremental noding (not re-testing pairs of strokes already noded together, e.g. the unchanged strokes of a layer or a
  whole layer moved in by Flatten) was implemented and tested against full noding; it is **not shipped**, because a
  settled layer is not always a fixpoint of full noding when its pieces are re-noded as edited strokes (end-to-vertex
  and end-to-piece touches resolve again), so the two disagree on such content. Making settled layers fixpoints is the
  remaining C22-T2 prerequisite; the seeds are in the C2.4 handoff.
- **M2 C2.4 (ticket C24-T1, orchestrator ruling): one owner per overlapped edge; stretch records for split sides.**
  Collinearly overlapping, retracing and folded segments stay unsplit (C22-R3-03 stands), and a stroke is not split at a
  vertex another stroke only touches (a fold's turn point on it), so one stored side could face two zones and a zone's
  fill read back differently after an unrelated edit. Now: (7) for every set of pieces on one edge, the side fill is
  owned by the piece of the stroke with the lowest stroke ID, then the lowest segment index (`fill::owner`), and every
  read of a side fill on that edge resolves through the owner; (8) every write — paint, repair, recovery, carry-over,
  all through `fill::write` — gives every piece's stretch its zone's fill, the owner's and the mirrored members' alike:
  as the stroke's side fill (the zone along its longest stretch, as before) or, where a stretch faces a zone with
  another fill, as a stretch record `{side, from, to, fill}` in the stroke's `ext["dev.toontown.zones"].exterior` (the
  SI-060 key, now on pencil and invisible strokes too; adjoining records of one side and fill merge). Strokes whose
  sides face one zone each carry no record, so content without overlaps or touches is unchanged. Folds are covered by
  the owner rule, not by splitting. Evidence: `crates/vector/tests/c24_properties.rs` —
  `side_fills_read_back_after_adding_strokes` (seeded property, 256 cases in `check`; 4096 run locally) and
  `side_fills_read_back_beside_collinear_overlaps` (the shrunk seed, now permanent, with retrace and fold variants).
- **M2 C2.4 (readiness item 4, C22-T2 bounded pass): settled layers are fixpoints of noding.** (9) A noding pass's
  splits create piece ends at its junctions, and an end resting on another stroke is itself a junction — e.g. two
  lines crossing exactly at a vertex of a third stroke that retraces both: the crossing was a junction of the two only,
  and the third was split at that vertex by the next edit's noding. `node::node` now repeats the pass on the pieces
  until a pass changes nothing (at most `MAX_PASSES` = 8, each charged to the work account; in the corpus a second
  pass finds the new ends and a third nothing), composing the outcomes per input stroke. A settled layer is therefore
  a fixpoint of noding with its strokes marked changed or unchanged. Evidence: `crates/vector/tests/c24_properties.rs`
  `a_settled_layer_is_a_fixpoint_of_noding` (seeded property, 512 cases in `check`; 2048 run locally) with the
  shrunk seed.
- **M2 C2.4 round 1 (C24-R1-04, accepted ruling): C22-T2 moves to M11 C11.1.** The C22-T2 gate missed on both OSes
  after the bounded pass (raw files `reviews/evidence/M2-C2.4/c22-t2/`, phase breakdown `c24_flatten_phases.rs`), so
  C22-T2 is owned by M11 C11.1 and merged into the R1-12 scale gate there. Kept in M2: the atomic, work-charged refusal
  before the watchdog, the fixpoint property (9), the phase breakdown and the gate (`c24_measure.rs`,
  `scripts/c22-t2-gate.py`) on every CI run, reporting `met: false` until C11.1 closes it.
- **M2 C2.4 round 1 (C24-R1-02 ruling): (10) exact crossing points.** A proper crossing's fractions along the snapped
  edges are ratios of exact grid orientations and its point is the rational crossing of the snapped edges rounded half
  to even onto the grid (SI-067). Before, floating-point fractions of the unsnapped geometry placed it; where snapped
  pieces of near-collinear strokes cross at a shallow angle they cancel differently once translated, so an integer-du
  translation moved a junction (reviewer seed: translation by (37, −11) changed two face areas). Persisted junction
  anchors of proper crossings are now grid points; the C21/C22 determinism goldens and the symmetry golden were
  re-recorded for that reason only (declared). Evidence: `crates/vector/tests/c24_r1_properties.rs`
  `shrunk_determinant_case` and the 4096-case translation property pass unchanged.
- **M2 C2.4 round 1 (C24-R1-01): defects found by the command-route properties, fixed.** (11) An open end resting on a
  stroke that passes the end more than once (a fold, a retrace) was a T-junction with one pass only (one record per
  stroke), so the other pass ran on between two rounded junctions and crossed the first at a shallow angle — a spurious
  junction and a 0.08 du² area error. The end now makes one record per stroke and Bézier segment, cutting every pass
  (`crates/vector/tests/c24_noding_regressions.rs`). (12) Exact overlap areas (`exact::overlap_area2`) counted a dangling
  stroke inside a zone (an antenna of its boundary, or a two-vertex hole ring) as boundary, so a merged zone sharing such a
  hole with an old zone scored the wrong overlap and took the smaller part's fill. Edges traversed both ways now cancel
  before the overlap is computed (unit test `a_dangling_edge_inside_both_zones_bounds_no_area`). Both were found by
  `crates/executor/tests/c24_oracle_routes.rs` at 4096 cases and are kept there as permanent seeds.
- **M2 C2.4 round 2 (C24-R2-07 ruling): (13) a closed retrace keeps its traversal.** Flattening a closed path whose
  last anchor repeats its first dropped the repeated vertex but not its parameter, so the closing edge ran from the
  last real vertex across the zero-length closing segment (parameters 1 to 3 of a three-segment fold instead of 1 to 2)
  and the pieces cut from it traversed one pass twice. `Flat` now keeps the reach parameter of the dropped vertex and
  closes there; repeated terminal anchors and zero-length closing segments keep their cyclic entry and exit
  parameters. Intentional retraces are not deduplicated, folds are not split and area allowances are unchanged. A
  debug-only invariant (`debug_check_cuts`, compiled out of release builds) checks before every cut that the cut
  parameters increase strictly inside the traversal, so the pieces partition it exactly once, and that each lies at
  its junction (on the chord of a straight segment, within twice the junction tolerance, the flattening tolerance and
  a grid cell). Evidence: `crates/vector/tests/c24_r2_noding.rs` (reviewer, unchanged, both controls) and unit test
  `a_repeated_closing_anchor_keeps_the_closing_edge_parameter`.
- **M2 C2.4 round 2 (C24-R2-06; found by the reviewer's XOR-5D point-move seed): (14) the carry-over bound as
  documented.** The snapping bound of (5) is twice the boundary length inside the old zone's box, in area; the product
  compared twice the overlap area (`overlap_area2`) with twice that length, half the documented bound, so an overlap
  between the length and twice it carried a neighbour's fill. The bound is now four times the length in twice-area
  units, compared exactly. The route oracle now offers a collapsed contour's retained anchors (zero-length segments) as
  snap candidates with the stroke, segment, parameter tie order while keeping zero-length edges out of the faces, and
  its interior-point fallback uses the zone's published interior point (checked to lie in the oracle face) rather than
  its own probe. Evidence: `c24_r2_routes.rs` `point_moves_with_and_without_snapping_match_the_oracle` (4096 cases,
  unchanged) and `collapsed_other_contour_is_a_valid_snap_target_product_control`; builder seed
  `c24_oracle_routes.rs` `a_snapping_sliver_below_the_documented_bound_carries_no_fill`.
