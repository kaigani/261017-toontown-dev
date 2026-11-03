# M2 merge note — vector engine and drawing commands

Branch `m2-vector-engine`, merge candidate: the head named in the closing message, with its green CI run. Plan §4 M2:
02, 03 (model and paint). Not merged and not tagged by the builder: the orchestrator merges to `main`, verifies `main`
on both OSes and tags `m2`.

## 1. Checkpoints and closure

| Checkpoint | Scope | Closure | Final register |
|---|---|---|---|
| C2.1 | strokes, curve fitting, brush outline, pencil/invisible, text and bitmap nodes | round 4 scoped closure **pass** ([M2-C2.1-r4-closure.md](M2-C2.1-r4-closure.md); builder `4e304cf`, review `812554b`) | 134 implemented / 0 divergent / 0 missing / 231 deferred with owner (365 source rows) |
| C2.2 | planar map: noding, zones, sides, gap closing, paint by ID/point, colour IDs, recovery | round 4 scoped closure **pass** ([M2-C2.2-r4-closure.md](M2-C2.2-r4-closure.md); builder `62c428d`, review `9c42610`) | 187 implemented / 0 divergent / 0 missing / 158 deferred with owner (345 units) |
| C2.3 | flatten/optimize/smooth/centre line/convert/arrange/group, `draw.*`, `colour.paint.*`, `render_drawing` | round 4 scoped closure, closed by orchestrator ruling on C23-R4-01 (reviewer ownership is by file; [M2-C2.3-r4-closure.md](M2-C2.3-r4-closure.md), [M2-C2.3-handoff.md](M2-C2.3-handoff.md); builder `fc25eae`, review `1df274b`) | 98 implemented / 0 divergent / 0 missing / 25 deferred with owner (123 units); commands 37/37 |
| C2.4 | vectorize (trace + fit), topology property tests, E2 headless, M2 performance tickets | round 4 scoped closure **pass** ([M2-C2.4-r4-closure.md](M2-C2.4-r4-closure.md); builder `463158d`, review `8a61c77`) | requirements 72 total, 62 implemented, 10 deferred with owner; readiness 9 total, 7 implemented, 2 deferred with owner; commands 10 total, 6 implemented, 4 deferred with owner; 0 divergent, 0 missing |

## 2. Merge-readiness conditions ([M2-C2.3-r3.md](M2-C2.3-r3.md#m2-merge-readiness)) at the candidate

1. **C2.4 and the M2 subset pass; M2-owned tickets implemented or explicitly re-scoped.**
   - C2.4 is closed (round 4).
   - C22-T2 and R1-12 are re-scoped to M11 C11.1 by accepted rulings.
   - Feather (C23-T2/SI-077) is re-scoped by the orchestrator's scope decision at the merge (§3). It is synchronized in
     traceability, the generated declaration owner, SI-077, ADR-0018 history and handoff §5/§7.
   - SI-077 is not unowned: M7 C7.2, or the first checkpoint after the spec owner's decision.
2. **No open P0/P1:**
   - every round-1 to round-3 finding of C2.1 to C2.4 is closed;
   - all original proving tests, every round's variants and the closure evidence are retained.
3. **Green Windows and macOS CI at the candidate**, all 20 jobs (run ID in the closing message):
   - release history/geometry oracles;
   - deterministic artifact comparisons;
   - official-SDK E2/replay;
   - examples/generator/IPC-schema freshness;
   - fmt/clippy;
   - frontend/permission/headless guards;
   - licences/deny;
   - product-only feature proof.

   The round-4 addendum is met: `c24_r4_closure`'s two seeded properties share names with `check`'s skip list, so
   `review properties release` now runs that file explicitly on both OSes (declared CI edit).

   A second declared CI edit was needed. The reviewer-owned `c24_r4_closure.rs` trips one clippy style lint (blank
   lines after doc comments, five places), and reviewer-owned files are not edited. So `check` now runs clippy twice:
   - over product code (libraries, binaries, examples) with every warning denied;
   - over all targets with every warning denied except `clippy::empty_line_after_doc_comments`.

   That `-A` should be dropped once the reviewer removes the blank lines.
4. **Receipts and ownership preserved:**
   - raw receipts are kept under `reviews/evidence/`;
   - reviewer ownership is by file, and every reviewer-owned edit was ruled and declared (`scripts/check-reviewer-ownership.py` checks the C2.4 ones live);
   - the M1-C1.3 historical receipts are unchanged and `spec/` is read-only;
   - accepted later-milestone risks are documented (§3);
   - C22-T2 and R1-12 status is explicit;
   - safe refusal and optimized test setup are never represented as performance compliance.
5. **Orchestrator:** merge, verify `main` buildable on both OSes, tag the verified commit `m2`. This is not done by the builder.

## 3. Declared later-milestone deferrals (named owners)

| Item | Owner | State |
|---|---|---|
| C22-T2 — 100k-stroke Flatten within the run's 100k `strokeAdd` p50 | M11 C11.1, merged into the R1-12 scale gate (accepted ruling, C2.4 round 1) | Gate missed on both OSes (raw files, run 41425059602): the unbudgeted Flatten exceeds the run's 100k `strokeAdd` p50 by 17.3 s (Windows) and 4.3 s (macOS). The budgeted product refuses atomically with `limit.exceeded` before the 30 s watchdog (about 1.6 s), never presenting a refusal as completion. The bounded pass made settled layers fixpoints of noding; incremental noding could save at most 0.27 s / 0.11 s (handoff §6). `scripts/c22-t2-gate.py` runs every CI build and reports `met: false`. |
| R1-12 binding interactive budgets | M11 C11.1 | Measured baselines and proposed budgets on both OSes in handoff §6a (edit/undo/redo/dry-run, allocation, RSS, queued-read latency, maintenance delay). |
| Feather (`draw.feather.apply`, ticket C23-T2, SI-077) | M7 C7.2 (effects), or the first checkpoint after the spec owner decides SI-077, whichever comes first | Orchestrator scope decision at the M2 merge, because the spec owner has not answered SI-077. Feather stays declared and refuses with `command.notImplemented`; the generated owner is "M7 C7.2 (or the first checkpoint after the SI-077 decision)". The spec owner may reverse this. |
| `draw.vectorFile.import` (02 §11.4) | M10 C10.2 (ruling, SI-082) | Declared, refuses with `command.notImplemented`. |
| SI-084 — vectorization colours and palettes | Spec owner (03 amendment) | 02 §11.2 precedence adopted: generated black in the reused `Vectorization` palette. The 03 §11.6/§7.4 wording is the amendment candidate. |

## 4. Open spec issues for the spec owner

`plan/spec-issues.md`:

| Issue | Subject | Register state |
|---|---|---|
| SI-044 | "Repair on Open" cannot be undone into an invalid document | decided |
| SI-047 | RO-02 cannot see an edit that keeps size and modification time | interim ruling, open |
| SI-049 | MCP revision 2026-07-28 removes resource subscriptions and deprecates logging | open |
| SI-050 | RO-02's "same choice" for agents meets 11 §6.1's `app` scope | decided |
| SI-077 | feather colours | deferred by orchestrator, awaiting spec owner; Design B (band opacity) recommended |
| SI-084 | 02 §11.2 vs 03 §11.6/§7.4 | decided, amendment candidate |

The Harmony data-model amendments are carried as listed by the orchestrator. No entry under that name exists in this
branch's `plan/spec-issues.md`.

## 5. Reviewer-owned tests, permanent in CI

**Rust:** every reviewer file from C2.1 to C2.4 runs in CI:
- **C2.1** (`c21_r1_*` … `c21_r4_closure`);
- **C2.2** (`c22_r1_*` … `c22_r4_closure`);
- **C2.3** (`c23_r1_*` … `c23_r4_closure`);
- **C2.4** (`c24_r1_*`, `c24_r2_*`, `c24_r3_*`, `c24_r4_closure`).

Where each set runs:
- the fast tests run in `check`;
- the seeded topology properties run in `check properties`;
- the round-2, round-3 and round-4 seed variants run in `review properties release`;
- the 10 000-operation history oracles run in `history oracles release`.

**Python:** 41 of the 43 `tests/adversarial/*.py` files run as suites through `scripts/run-reviewer-python-suites.py` in `check` (41/41 pass).
- Two files are not suites: the M0 evidence collector `collect_runtime_evidence.py`, and the round receipt `c23-r3-finalize.py` (orchestrator ruling).
- One pinned test (`c24-r2-audit.py` `Ownership` receipt) is excluded by name, its live counterpart being
  `scripts/check-reviewer-ownership.py` (C2.4 edit 57, confirmed in round 3).

## 6. CI job layout and budgets

Times from run 41570855156; the candidate run is in the closing message.

| Job (Windows / macOS unless noted) | Budget (min) | Last run, Windows / macOS (min) |
|---|---|---|
| `check` (fmt, clippy, all tests except the heavy properties, reviewer Python suites, ownership check, frontend, generators) | 60 | 40.9 / 29.5 |
| `check properties` (seeded topology properties, 256 cases each, debug) | 45 | 35.2 / 24.9 |
| `review properties release` (reviewer round-2/3/4 seed variants) | 45 | 30.3 / 16.1 |
| `headless release` (release binary, self-test, SDK conformance, measurements, C22-T2 gate) | 55 | 35.5 / 37.1 |
| `history oracles release` | 40 | 9.4 / 10.5 |
| `measurements release` (R1-12 baselines, aggregate corpus) | 45 | 13.4 / 10.4 |
| `e2 release` (official-SDK paint-by-ID chain) | 30 | 6.1 / 4.0 |
| `bundle` (test-only asset check in the bundle) | 30 | 10.7 / 7.4 |
| `raster determinism` (Windows, macOS, Linux) | 20 | 3.3 / 2.6 / 1.6 |
| `headless without a display` (Linux) | 15 | 3.2 |

## 7. Test counts

- **Local workspace at the candidate's code** (`cargo test --workspace --locked --no-fail-fast`, Windows, debug, the
  six 10 000-operation history oracles excluded as in `check`): see the closing message for the count at the
  candidate.
- Round 4's independent full run counted **1290 passed / 0 failed / 22 ignored**, which is 1280 CI-equivalent tests
  plus the six history-oracle functions.
- Frontend: 333 tests in 40 files.
- Reviewer Python suites: 41/41.

## 8. Carried to M3

The M3 handoff list in [M2-C2.4-r3.md](M2-C2.4-r3.md#m3-handoff-list) is carried unchanged: retained regressions,
scene/timeline boundaries, fidelity risks and the M11 performance obligations.

The plan's §4 milestone table has no status column, so it is unchanged. M2's status is this note plus the C2.4 handoff
§7.
