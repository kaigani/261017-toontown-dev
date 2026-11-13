# M3 merge note — elements, timing, staging maths, final renderer

Merge candidate: branch `m3-elements-timing` into `main`, then tag `m3`. **Every M3 checkpoint has passed**; C3.4 passed
at its round-6 finalize `944c54d` on 2026-11-13. Builder: Claude Opus 5.5. The candidate is the status commit that
records this closure. Its own green run and counts are in its closing message, since a commit cannot name its own run.
The orchestrator merges and tags; the builder does not.

## 1. Checkpoints and closures

| Checkpoint | Scope (plan §4 row M3) | Closure | Final register |
|---|---|---|---|
| C3.1 | element types, exposure runs, cells, clone/duplicate, hierarchy, `element.*`, `cell.*`, `xsheet.*` JSON | round 4 scoped closure **pass** ([M3-C3.1-r4-closure.md](M3-C3.1-r4-closure.md); builder `864b70d`, review `ac47998`) | source units 467 implemented / 0 divergent / 0 missing / 227 deferred; commands and events 63 / 0 / 0 / 1 |
| C3.2 | transforms, pivots, TCB paths, velocity, channels, easing, cameras, projection, `anim.evaluate`, E4 numeric | round 4 scoped closure **pass** ([M3-C3.2-r4-closure.md](M3-C3.2-r4-closure.md); builder `8c640d1`, review `2184ae8`) | 36 rows / 383 units: 36 rows implemented; 293 units implemented, 90 deferred with owner; 0 divergent, 0 missing |
| C3.2 correction (C3.4 round 3) | passage frames at a tangent touch | `tcb::PASSAGE_TOUCH` (1e-12 relative): a velocity extremum equal to the arc is the passage frame, found by CI's 4096-case property in C3.4 round 3 (builder `2c7e592`); adjudicated in C3.4 round 4 against an 80-digit oracle (no at-or-above-arc maximum missed; below-arc touches only inside the documented band) | ADR-0019 constants table; builder `c32_passage::a_tangent_touch_at_an_extremum_is_the_passage`, reviewer `c34_r4_passage` |
| C3.3 | compositor, render list, deterministic `render_frame`, contact sheet, `describe_frame`, `hit_test`, `convert_coordinates`, PNG sequence export, golden images | round 4 scoped closure **pass** ([M3-C3.3-r4-closure.md](M3-C3.3-r4-closure.md); builder `4984c25`, review `2daaf99`) | 183 implemented / 0 divergent / 0 missing / 14 deferred with owner / 1 closed (198 units) |
| C3.4 | E1 headless (minus MP4) | round 6 finalize **pass**, 2026-11-13 ([M3-C3.4-r6-closure.md](M3-C3.4-r6-closure.md); builder `f0e538a`, review `944c54d`; handoff [M3-C3.4-handoff.md](M3-C3.4-handoff.md), `45b41da`) | 45 adjudications: 42 implemented / 0 divergent / 3 deferred with owner; every round-2 to round-5 finding closed; open P0 0 / P1 0 / P2 0 / P3 0 |
| C3.4 round 1 | merge-note inventory; request cancellation reaches the executor (O-1) | response ([M3-C3.4-r1-response.md](M3-C3.4-r1-response.md); builder `967229a`, review `1cbd190`) | 42 implemented / 0 divergent / 3 deferred with owner |
| C3.4 round 2 | the cancellation contract at the worker's boundaries; direct `job.wait`; honest discard; token hygiene; SI-111 | response ([M3-C3.4-r2-response.md](M3-C3.4-r2-response.md); builder `0146e1a`, review `a2253cb`) | the four round-2 findings implemented |
| C3.4 round 3 | bounded probe questions; refusal causes; positive discard notice; CI budget routing | response ([M3-C3.4-r3-response.md](M3-C3.4-r3-response.md); builder `1ef439b`, review `0a374c9`) | the three round-3 findings implemented |
| C3.4 round 4 | cause carried into every copy of a cancellation refusal; late answers latch; cumulative question bound; CI backtraces restored; one more release routing | response ([M3-C3.4-r4-response.md](M3-C3.4-r4-response.md); builder `8312694`, product-code head `1a19692`; review `62d21ea`, closure [M3-C3.4-r4-closure.md](M3-C3.4-r4-closure.md): C34-R3-01..03 closed) | C34-R4-01, C34-R4-02 and O-6 implemented |
| C3.4 round 5 | the activity entry declared as `agent.activity.list`'s typed result (optional `errorReason`); receipt corrections; `desktop diagnostics` job | response ([M3-C3.4-r5-response.md](M3-C3.4-r5-response.md); builder `f0e538a`; review `9f1a76f`, closure [M3-C3.4-r5-closure.md](M3-C3.4-r5-closure.md): C34-R4-01, C34-R4-02 closed) | C34-R5-01 implemented |
| C3.4 round 6 | finalize: C34-R5-01 closed; one authorized reviewer test amendment | **pass** ([M3-C3.4-r6-closure.md](M3-C3.4-r6-closure.md); review `944c54d`; CI run 43330225505 at `f0e538a` green on all 30 jobs) | open P0 0 / P1 0 / P2 0 / P3 0 |

**What M3 delivered:**
- The element, timing and staging maths, and the deterministic final renderer.
- E1 end to end headless, with every output byte-identical on Windows and macOS.
- A request-cancellation contract (ADR-0020 decision 8):
  - an already-cancelled request never begins;
  - the worker asks a probed caller directly, each question bounded by min(250 ms, budget/4); silence and a late
    answer count as not cancelled and latch the link, and all questions together hold the document thread for at most
    the request's budget plus one bound;
  - a commit that got there first stands;
  - request cancellation never cancels a job;
  - every copy of a caller-cancellation refusal carries `reason: "cancelled"` (SI-111).

## 2. Merge-readiness conditions at the candidate

- [x] **Every M3 checkpoint passes.** C3.1-C3.3 passed at their round-4 closures; C3.4 passed at its round-6 finalize
  `944c54d` (2026-11-13).
- [x] **No open P0/P1/P2.** Every finding of C3.1-C3.4 is closed (C3.4: open P0 0 / P1 0 / P2 0 / P3 0). All original
  proving tests, every round's variants and the closure evidence are retained (`reviews/evidence/M3-*`).
- [x] **Green Windows and macOS CI at the candidate**, all 30 jobs (28 until C3.4 round 5 added `desktop
  diagnostics`). Run 43330225505 at `f0e538a` is green on all 30, and the status commit's own green run is in its
  closing message. These cover:
  - debug `check` (fmt per package, both clippy invocations, workspace tests, reviewer Python suites, ownership,
    front end, generator and schema freshness, examples);
  - release history oracles, properties, measurements, E2, E4 and E1;
  - golden and cross-OS byte gates;
  - bundles, the desktop host with M0 diagnostics, raster determinism and headless without a display.

  Windows `check` stays under 55 min (34.6 min at run 43330225505).
- [x] **Receipts and ownership preserved:**
  - raw receipts under `reviews/evidence/`;
  - reviewer ownership by file (§5): the only reviewer-file edits of M3 are three authorized amendments, all made
    by the reviewer except the first: the C3.3 round-3 sequencing edit (`c3ce737`), the C3.4 round-4 amendment of
    `c34_r3_probes::panicking_probe_leaves_worker_alive` (`62d21ea`), and the C3.4 round-6 amendment of
    `c34_r5_deadlines::queued_job_wait_step_uses_enqueue_deadline` (`e97bf69`);
  - `spec/` read-only;
  - the allow inventory at 97 lints / 40 files, 0 new.
- [ ] **Orchestrator (not the builder):** merge into `main`, verify that `main` builds on both OSes, and tag the
  verified commit `m3`.

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
| Probed-call overhead in a product build | M11 C11.1 (orchestrator item O-10, C3.4 round 4) | C3.4 round-4 closure, release with test hooks on: probed versus plain calls +10 % for 10,000 actions (4805 / 5291 ms) and +13 % for a 1,000-step batch (422 / 477 ms). To be re-measured in a product build with the attached UI. |
| O-6 cumulative-bound test: a measurement limit (C3.4 round 5 closure) | M11 C11.1, with the other timing re-measurements | The builder test discards a trial whose answer reached 225 ms (0.9 × the 250 ms bound) and asserts the looser limit of budget + 2 × bound. It shows the guarantee with margin, not at its exact edge. |
| The waiting-caller late-wake path (C3.4 round 4) | M11 C11.1 | At its deadline a waiting caller takes a reply already delivered. No deterministic hook can force the late wake; reviewed by reading only. |

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
- **C3.4:** `c34_r1_contract`, `c34_r2_*`, `c34_r3_*`, `c34_r4_*` (executor, MCP `c34_r2_protocol` and
  `c34_r3_stress`, animation `c34_r4_passage`).

Authorized reviewer amendments:
- C3.3 round 3: the sequencing edit of the `c33_r2_wait` injection (`c3ce737`).
- C3.4 round 4: the body of `c34_r3_probes::panicking_probe_leaves_worker_alive` accepts an unchanged document or
  exactly +1 revision with exactly one undo step, and passes with backtraces on and off (made by the reviewer in
  `62d21ea`; CI's `check` runs it with `RUST_BACKTRACE=1` again).
- C3.4 round 6: `c34_r5_deadlines::queued_job_wait_step_uses_enqueue_deadline` runs with a 6000 ms budget and a
  4000 ms hold measured by `Instant`, for headroom on loaded CI runners; the product margin is unchanged (made by the
  reviewer in `e97bf69`, by orchestrator ruling; 30/30 debug and 30/30 release under load).

Placements outside debug `check`:
- `c31_r3_sequences` and `c31_r3_hierarchy` run in `review properties release`.
- `c33_r2_depth` and `c33_r2_stress` run in `c33 render release`.
- Six request-cancellation loops run whole-binary in release in `c33 render release`, bodies unchanged:
  `c34_r3_stress::concurrent_wire_cancellation_sixty_seconds`,
  `c34_r3_probes::overhead_ten_thousand_small_actions_and_thousand_step_batch`,
  `c34_r2_measurements::one_hundred_wait_cancellations_settle_jobs_and_leave_handle_usable`,
  `c34_r2_cancellation::fast_commit_cancel_retry_matrix_preserves_one_change_and_one_answer`,
  `c34_round2::a_cancellation_between_two_steps_refuses_the_second_every_time` (builder) (C3.4 round 3, O-4), and
  `c34_r4_late_probe::two_hundred_answers_after_the_bound_must_latch_after_one_question` (C3.4 round 4, O-8).
- The trybuild suites run in `history oracles release`.
- For `check`'s budget (C3.4 round 4), three builder binaries leave debug `check`: `c32_round3` and
  `c12_kernel_state` run in debug in `check properties`, and `cancel_phases` runs in release in `headless release`.
  The C0.4 step "Desktop host with the M0 diagnostics feature" runs in its own job, `desktop diagnostics` (C3.4
  round 5, after it overran `bundle`'s 30 minutes on Windows).
- `check` skips by exact name only what those jobs run, plus `c33_r1_crash::killed_sequence_leaves_no_staged_files`
  (premise closed by ruling C33-R1-08; body unchanged).

**Python:** `tests/adversarial/c31-*.py`, `c33-r1-audit.py` and `c34-*.py` (except the two round-2 collectors
`c34-r2-sdk.py` and `c34-r2-inventory.py`, listed as not suites), through `scripts/run-reviewer-python-suites.py`.

## 6. CI job layout and budgets

30 jobs. Times are from run 43330225505 at `f0e538a`, the last head before this status commit; the round-6 finalize
adjudicated it green on all 30 jobs, all on the first attempt. That run is used here to avoid a second docs commit.
The status commit's own green run is in its closing message.

| Job (Windows / macOS unless noted) | Budget (min) | Run 43330225505, Windows / macOS (min) |
|---|---|---|
| `check` | 60 (ceiling 55 by ruling; 47 target in C3.4 round 4) | 34.6 / 28.4 |
| `check properties` (with `c32_round3` and `c12_kernel_state` from C3.4 round 4) | 45 | 29.1 / 32.8 |
| `review properties release` (with `c31_r3_hierarchy`) | 45 | 40.1 / 29.9 |
| `headless release` (with `cancel_phases` from C3.4 round 4) | 55 | 50.5 / 42.2 |
| `history oracles release` | 40 | 18.8 / 9.9 |
| `measurements release` | 45 | 18.1 / 12.3 |
| `m3 properties release` | 20 | 3.1 / 1.6 |
| `e2 release` | 30 | 6.9 / 6.3 |
| `e4 release`, `e4 cross-OS` | 30, 10 | 7.4 / 7.7; 0.1 |
| `c33 render release` (goldens, range, bitmap, render gate, reviewer corpora, E1, request-cancellation loops and stress from C3.4 rounds 3 and 4), `c33 cross-OS` | 30, 10 | 17.0 / 22.2; 0.2 |
| `bundle` | 30 | 12.5 / 7.7 |
| `desktop diagnostics` (C0.4 desktop host with M0 diagnostics; new in C3.4 round 5) | 40 | 5.0 / 4.5 |
| `raster determinism` (Windows, macOS, Linux) | 20 | 2.9 / 2.6 / 2.9 |
| `headless without a display` (Linux) | 15 | 6.4 |

`check` at that run: Windows 372 test binaries, 1638 passed, 0 failed, 24 ignored; macOS 372, 1633, 0, 24. `check
properties`: 8 binaries, 60 passed, on each OS. Windows `check` ranged from 34.6 to 49.6 min across C3.4 rounds 4-5;
Windows `headless release` (50.5 of 55 min) is the closest to its budget.

## 7. User-only items

- **Spec owner:** the open issues and questions in §4, SI-096 (D-5) first, because M6 C6.2 depends on it.
- **Orchestrator:** the merge and the `m3` tag (§2.5).
- Settled by the orchestrator at C3.4 round 1, so no longer requests:
  - The MP4 export owner is **M10 C10.1** (ADR-0008; sidecar M8 C8.3).
  - The lock-file host comparison stays case-sensitive; the stale-same-machine gap is owned by **M11 C11.1** (§3).
- No hardware, credential or licence action is needed for M3.

## 8. M4 readiness (carry-forward)

Verbatim from the C3.4 round-6 finalize report ([M3-C3.4-r6-closure.md](M3-C3.4-r6-closure.md), "Final register and M4
readiness"):

Inherited round-1 register: 45 adjudications, 42 implemented / 0 divergent / 3 deferred with owners. All round-2 and round-3 findings remain closed; C34-R4-01 and C34-R4-02 remain closed; C34-R5-01 closes here. Open P0 0 / P1 0 / P2 0 / P3 0. No new finding is opened by this finalize check.

1. M4 C4.1: actual-shell command/GUI bindings, typed errors, request cancellation, settings persistence and IPC freshness, with both-platform receipts.
2. M4 C4.2: attached/headless parity for identical fixture, revision and integer frames on both OSes; fixed PNG/metadata parity and explicit volatile fields; retain goldens.
3. M4 C4.2: view/draft transitions, visibility overrides, ancestors, coordinate/roll/default-FOV agreement.
4. M5 C5.2 thumbnails; M5 C5.1 xsheet; M6 C6.3 bones; M7 effects; M8 sounds; M10 C10.1 MP4 with M8 C8.3 sidecar; retain import/view owners. Spec owner resolves SI-096 before M6 C6.2; SI-111 remains interim, next free SI-112.
5. M11 C11.1: lock-host case and safe stale-local refusal, batch-wait remeasurement with attached UI (off-thread continuations not planned), probed-call overhead in a product build, inherited C22-T2/R1-12 performance.
6. Preserve six release routings, three moved builder binaries and the separate desktop diagnostics job; accepted killed-sequence skip; both-OS E1/cross-OS goldens and Windows check below 55 minutes.
7. Retain timing-index risks, c14 autosave noise, O-6 retry/filter limitations, absolute throughput/overhead samples and scheduler-dependent deadline ambiguity. Add deterministic caller-wake instrumentation before claiming exact reply-arrival timing coverage. Retain this round's larger-budget deadline probe and its finite-scheduler limitation.
8. Finalize merge-note closure receipt and record the authorized round-6 test amendment in the builder status commit including this branch; require all jobs green on both OSes before merge. Merge/tag and spec decisions remain user-owned.
