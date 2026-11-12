# M3 merge note — elements, timing, staging maths, final renderer

Merge candidate: branch `m3-elements-timing` into `main`, then tag `m3`, after C3.4 passes. Builder: Claude Opus 5.5.
The candidate head, its green run and the counts are in the C3.4 closing message, and are added here at the closure.
The orchestrator merges and tags; the builder does not.

## 1. Checkpoints and closures

| Checkpoint | Scope (plan §4 row M3) | Closure | Final register |
|---|---|---|---|
| C3.1 | element types, exposure runs, cells, clone/duplicate, hierarchy, `element.*`, `cell.*`, `xsheet.*` JSON | round 4 scoped closure **pass** ([M3-C3.1-r4-closure.md](M3-C3.1-r4-closure.md); builder `864b70d`, review `ac47998`) | source units 467 implemented / 0 divergent / 0 missing / 227 deferred; commands and events 63 / 0 / 0 / 1 |
| C3.2 | transforms, pivots, TCB paths, velocity, channels, easing, cameras, projection, `anim.evaluate`, E4 numeric | round 4 scoped closure **pass** ([M3-C3.2-r4-closure.md](M3-C3.2-r4-closure.md); builder `8c640d1`, review `2184ae8`) | 36 rows / 383 units: 36 rows implemented; 293 units implemented, 90 deferred with owner; 0 divergent, 0 missing |
| C3.3 | compositor, render list, deterministic `render_frame`, contact sheet, `describe_frame`, `hit_test`, `convert_coordinates`, PNG sequence export, golden images | round 4 scoped closure **pass** ([M3-C3.3-r4-closure.md](M3-C3.3-r4-closure.md); builder `4984c25`, review `2daaf99`) | 183 implemented / 0 divergent / 0 missing / 14 deferred with owner / 1 closed (198 units) |
| C3.4 | E1 headless (minus MP4) | handed off ([M3-C3.4-handoff.md](M3-C3.4-handoff.md), `45b41da`); closure pending | — |
| C3.4 round 1 | merge-note inventory; request cancellation reaches the executor (O-1) | response ([M3-C3.4-r1-response.md](M3-C3.4-r1-response.md); builder `967229a`, review `1cbd190`) | 42 implemented / 0 divergent / 3 deferred with owner |
| C3.4 round 2 | the cancellation contract at the worker's boundaries; direct `job.wait`; honest discard; token hygiene; SI-111 | response ([M3-C3.4-r2-response.md](M3-C3.4-r2-response.md); builder `0146e1a`, review `a2253cb`) | the four round-2 findings implemented |
| C3.4 round 3 | bounded probe questions; refusal causes; positive discard notice; CI budget routing | response ([M3-C3.4-r3-response.md](M3-C3.4-r3-response.md); review `0a374c9`) | the three round-3 findings implemented |

**What M3 delivered:**
- The element, timing and staging maths, and the deterministic final renderer.
- E1 end to end headless, with every output byte-identical on Windows and macOS.
- A request-cancellation contract (ADR-0020 decision 8):
  - an already-cancelled request never begins;
  - the worker asks a probed caller directly, each question bounded by min(250 ms, budget/4), and silence counts as
    not cancelled;
  - a commit that got there first stands;
  - request cancellation never cancels a job;
  - refusals carry `reason: "cancelled"` (SI-111).

## 2. Merge-readiness conditions at the candidate

1. **Every M3 checkpoint passes.** C3.1-C3.3 are closed (round 4); C3.4 closes in its review rounds.
2. **No open P0/P1/P2.** Every finding of C3.1-C3.3 is closed. All original proving tests, every round's variants
   and the closure evidence are retained (`reviews/evidence/M3-*`).
3. **Green Windows and macOS CI at the candidate**, all 28 jobs. These cover:
   - debug `check` (fmt per package, both clippy invocations, workspace tests, reviewer Python suites, ownership,
     front end, generator and schema freshness, examples);
   - release history oracles, properties, measurements, E2, E4 and E1;
   - golden and cross-OS byte gates;
   - bundles, raster determinism and headless without a display.

   Windows `check` must stay under 55 min.
4. **Receipts and ownership preserved:**
   - raw receipts under `reviews/evidence/`;
   - reviewer ownership by file (§5): the only reviewer-file edit of M3 is the authorized C3.3 round-3 sequencing
     edit (`c3ce737`);
   - `spec/` read-only;
   - the allow inventory at 97 lints / 40 files, 0 new.
5. **Orchestrator:** merge, verify that `main` builds on both OSes, and tag the verified commit `m3`.

## 3. Declared later-milestone deferrals (named owners)

| Item | Owner | State |
|---|---|---|
| MP4/MOV/GIF export (E1's movie step) | M10 C10.1 (ADR-0008; sidecar M8 C8.3; settled at C3.4 round 1) | E1 delivers the PNG sequence and its manifest. |
| Attached-session frame rendering = headless (readiness item 12) | M4 C4.2 | Gate: the same saved fixture, revision and frames through the running app's adapter and headless on both OSes; PNG bytes and metadata compared, with the volatile fields identified; attached-session receipts retained. |
| Project and drawing thumbnails | M5 C5.2 | `derived.rs` names the owner. |
| Views, `activeView` composition, draft quality, `view` space | M4 C4.2 | refused with the owner |
| `xsheet` composition | M5 C5.1 | refused with the owner |
| Bones and envelopes | M6 C6.3 | SI-097 |
| Path/function/velocity IPC final shape (D-5) | M6 C6.2, SI-096 | spec owner |
| Effects in the render list | M7 | — |
| Sounds and mouth shapes in `describe_frame` | M8 | — |
| Importers writing native-size bitmaps | M10 | SI-105 |
| C22-T2, R1-12 (performance) | M11 C11.1 | inherited from M2 |
| Lock files compare host names case-sensitively (a stale lock of the same machine is not recognised) | M11 C11.1 (settled at C3.4 round 1) | found in C3.4 (handoff §5); stays case-sensitive until then, the safe direction |
| Off-thread continuations for a sequential batch's waited job step and `job.wait` step (they wait on the document thread, bounded by the batch's remaining budget, ending on request cancellation; ADR-0020 decision 8) | not planned; **M11 C11.1** re-measures the bound with the attached UI | measured in C3.4 round 2: a concurrent read waited 740 ms of a 1000 ms budget uncancelled, about 33 ms after cancellation |

## 4. Open spec issues for the spec owner

`plan/spec-issues.md` (next free id **SI-112**). This inventory follows every `## SI-` heading of the register, SI-001
to SI-111.

**Open, unresolved or awaiting the spec owner** (heading status in the register):

| Issue | Subject | Register disposition |
|---|---|---|
| SI-001 | 09 §4.4 "≈ CRF 18 equivalent" under platform H.264 encoders | **open**; decision taken in ADR-0008 (M10 C10.1 maps the quality levels to each encoder's quality mode); the spec owner is asked to confirm the meaning |
| SI-008 | `importOptions` of image elements is undefined | **open**, interim decision: an opaque JSON object, never interpreted, until section 09 (M10) defines it |
| SI-047 | RO-02 cannot see an edit that keeps size and modification time | **interim ruling, open** |
| SI-049 | MCP revision 2026-07-28 removes resource subscriptions and deprecates logging | **open** |
| SI-077 | feather colours | **deferred by orchestrator, awaiting spec owner**; ticket C23-T2, owner M7 C7.2 |
| SI-096 | per-axis position channels (D-5) | **unresolved, spec owner**; M6 C6.2 depends on it |
| SI-111 | a caller-cancelled request has no code in 11 §10 (found in M3 C3.4 round 2) | **interim, awaiting spec owner**; interim decision by orchestrator ruling: `command.timeout` with `details.reason: "cancelled"` (plus `details.jobId` on waited-job paths); seen only by in-process callers, batch step envelopes and the activity log |

**Interim decisions** (decided for now, with a question for the spec owner, or a declared deferral):

| Issue | Subject | Register disposition |
|---|---|---|
| SI-060 | fills of zones bounded only by brush-outline stretches | ruled, interim (ADR-0017 §4); amendment proposal |
| SI-080 | `VectorizeSettings` example versus the command rows | interim decided; align the 09 example |
| SI-081 | the `draw.vectorize.preview` PNG result limit | interim decided |
| SI-085 | where the static light table is stored | interim decided |
| SI-088 | media and sound cells before M8 | interim decided, declared deferral (M8 C8.1/C8.3); no question |

**M3 orchestrator rulings:** SI-102 (exact default FOV) is ruled and its register entry says **spec-owner confirmation
pending**. SI-104 (an explicit 41.1121 is no alias) and SI-110 (generic `render_frame` default width) are ruled with no
question in their entries, so they are not pending.

**Decided, with a confirmation or alignment asked of the spec owner:**
- SI-086, SI-087, SI-090 (error-code names).
- SI-089 (`cycleLength`).
- SI-091, SI-092 (drawing names, parents).
- SI-094, SI-095 (duplicating children, columnate).
- SI-098, SI-099, SI-100 (`anim.evaluate` frames, Constant Z, `visible` for pegs).
- SI-106 (role tiers outrank depth).
- The M1/M2 amendment candidates listed in the register (for example SI-044, SI-084).

Every other SI from SI-001 to SI-111 is decided, ruled, resolved or closed in the register, and SI-002 is reserved.

## 5. Reviewer-owned tests, permanent in CI

**Rust** (every file runs in CI):
- **C3.1:** `c31_r1_*`, `c31_r2_*`, `c31_r3_*`, `c31_r4_closure`, and the MCP `c31_r*_protocol`.
- **C3.2:** `c32_r1_*`, `c32_r2_*`, `c32_r3_*`, `c32_r4_*`.
- **C3.3:** `c33_r1_*`, `c33_r2_*`, `c33_r3_*`, `c33_r4_*` (executor, MCP, render).

Placements outside debug `check`:
- `c31_r3_sequences` and `c31_r3_hierarchy` run in `review properties release`.
- `c33_r2_depth` and `c33_r2_stress` run in `c33 render release`.
- The trybuild suites run in `history oracles release`.
- `check` skips by exact name only what those jobs run, plus `c33_r1_crash::killed_sequence_leaves_no_staged_files`
  (premise closed by ruling C33-R1-08; body unchanged).

**Python:** `tests/adversarial/c31-*.py` and `c33-r1-audit.py`, through `scripts/run-reviewer-python-suites.py`.

## 6. CI job layout and budgets

Times are from run 42968758950 at the C3.4 handoff head `45b41da` (after the C3.3 closure budget routing `7216cb9`).
The C3.4 closure row and the candidate run are added at the closure.

| Job (Windows / macOS unless noted) | Budget (min) | Run 42968758950, Windows / macOS (min) |
|---|---|---|
| `check` | 60 (ceiling 55 by ruling) | 42.5 / 37.4 |
| `check properties` | 45 | 27.5 / 21.8 |
| `review properties release` (with `c31_r3_hierarchy`) | 45 | 32.3 / 21.8 |
| `headless release` | 55 | 41.1 / 34.0 |
| `history oracles release` | 40 | 17.1 / 12.0 |
| `measurements release` | 45 | 13.4 / 9.9 |
| `m3 properties release` | 20 | 3.0 / 2.1 |
| `e2 release` | 30 | 6.9 / 4.3 |
| `e4 release`, `e4 cross-OS` | 30, 10 | 6.3 / 6.0; 0.0 |
| `c33 render release` (goldens, range, bitmap, render gate, reviewer corpora, E1, request-cancellation loops and stress from C3.4 round 3), `c33 cross-OS` | 30, 10 | 13.9 / 7.1; 0.1 |
| `bundle` | 30 | 10.1 / 8.3 |
| `raster determinism` (Windows, macOS, Linux) | 20 | 2.6 / 3.0 / 1.7 |
| `headless without a display` (Linux) | 15 | 3.7 |

## 7. User-only items

- **Spec owner:** the open issues and questions in §4, SI-096 (D-5) first, because M6 C6.2 depends on it.
- **Orchestrator:** the merge and the `m3` tag (§2.5).
- Settled by the orchestrator at C3.4 round 1, so no longer requests:
  - The MP4 export owner is **M10 C10.1** (ADR-0008; sidecar M8 C8.3).
  - The lock-file host comparison stays case-sensitive; the stale-same-machine gap is owned by **M11 C11.1** (§3).
- No hardware, credential or licence action is needed for M3.
