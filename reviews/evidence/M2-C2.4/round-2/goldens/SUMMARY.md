# C24-R2-05 — the goldens re-recorded in C2.4 round 1 (edit 35), compared structurally

Heads: `6f72ee7` (round-1 review) → `3c74c34` (round-1 response). Tool: `scripts/golden-structural-diff.py` (descends
through every array; arrays of objects with an `id` aligned by ID — `added`/`removed` pieces apart from `moved` ones of
the same ID and anchor count, whose corresponding anchors' displacements are measured, and `resplit` ones whose anchor
count changed; other arrays of unequal length aligned by longest common subsequence; an anchor of a moved piece that
moved more than 1 du is a rerouted end, not a displaced junction). Files here: both raw goldens of each head
(`<golden>.<head>.json`), the full diff (`goldens-6f72ee7-3c74c34.diff`), one structural comparison per golden
(`<golden>.structural.json`), and the symmetry corpus itself at both heads (`c24-symmetry-corpus.<head>.json`, written
by `TT_DUMP_SYMMETRY=<file>`; their SHA-256 are the goldens' digests `fc3116b0…` and `fcaf9b70…`) with its structural
comparison (`c24-symmetry-corpus.structural.json`).

| Golden | Pieces added / removed / moved / resplit | Corresponding anchors moved | Max displacement | Handle rows changed | Rerouted ends |
|---|---|---|---|---|---|
| `c21-determinism.json` | 0 / 1 stroke / 45 strokes / 0 | 82 | **0.393 du** | 44 | 1 |
| `c22-determinism.json` | 4 zones / 4 zones / 0 / 0 | 22 | 0.00695 du | 34 | 0 |
| `c24-symmetry.json` (corpus) | 126 zones / 126 zones / 0 / 0 | 744 | 0.0517 du | 744 | 0 |

**Cause (corrected).** Edit 32 (C24-R1-02 ruling, ADR-0017 history 10) places a proper crossing at the rational
crossing of the two *snapped* edges, rounded half to even onto the 1/64 du grid. Before, the junction was the
floating-point crossing of the *unsnapped* edges, left off the grid. The new point is within half a grid cell of the
new exact crossing, but the snapped edges are not the unsnapped ones: where two edges cross at a shallow angle, moving
their ends by up to half a cell slides their crossing along them by much more. Hence displacements far above the 1/128
du claimed in the round-1 response, which is withdrawn: up to 0.393 du in C21 (a junction of two near-parallel pieces at
(38.006, 3.942) → (37.797, 3.609), shared by four pieces), 0.052 du in the symmetry corpus, 0.007 du in C22. Moving a
junction moves the adjacent handles of curved pieces with it (the handle rows above), and the zones bounded by moved
junctions change ID (a zone ID hashes its snapped boundary): 4 zones in C22, 126 in the symmetry corpus (22 in
configuration 7, kaleidoscope 22.5° × 5; 104 in configuration 10, kaleidoscope 7.5° × 12; the other configurations are
byte-identical). In C21 two junctions on the line y = 0 lay 0.035 du apart, at (35.125, 0) and (35.160, 0), with a 0.035 du sliver
piece (`st_…C0KEK`) between them; the exact crossing of the snapped edges now rounds onto (35.125, 0), so the two
junctions are one: the piece beyond the second one (`st_…DBWN`, (35.160, 0) to (100, 0)) is gone and the sliver piece
now runs from (35.125, 0) to (100, 0) — the one rerouted end, not a displacement. No other value changed:
apart from the digest and byte count, every change is one of the above. The current goldens are deterministic on both
OSes (round-1 CI run 41486947308) and unchanged by every round-2 product change (carry bound, closed-retrace
parameter, exact T-junctions).
