# M4 design note — availability by precheck (ruling O-23)

Status: decided before implementation (M4 C4.1 closure, ruling O-23), committed before the code. Owner: builder.
Answers: C41-R3-03 (availability and execution must agree) without the cost the round-3 answer paid. Spec and plan:
01 §1.3, 11 §8 (the pipeline's checks), plan §3.2 and 11 P5 (never block), ADR-0021 decision 9 (ruling O-10: one
availability for every GUI surface).

## 1. Problem

Round 3 made availability ask each handler itself: every statically accepted reference cloned the session and ran
the whole transaction pipeline as a dry run. Availability then equalled execution by construction, but its cost grew
with the document: 3.15 s for one pass over the 269 GUI references with 990 pegs, as one request on the document
thread, during which a user command waited 3.03 s. That breaks the never-block rule on every state change.

## 2. The rule

- **A precheck per command that can refuse for a state reason with no parameter missing.** The `#[command]`
  attribute gains `precheck = PATH`: a function `fn(&Cx, &serde_json::Value) -> Result<(), CommandError>` of the
  session state and the reference's raw parameters, returning exactly the refusal the handler would give (same
  code). A precheck is pure and cheap: it reads the session (current scene, element, frame, drawing, selection lists,
  clipboard, layout, history flags) and the parameters; it makes no scratch copy, runs no pipeline, no integrity gate,
  no rendering, and allocates nothing proportional to the document (lookups by id; no walk over the document's
  objects).
- **One predicate by construction.** The transaction pipeline runs the declared precheck immediately before every
  handler (11 §8 order: existence, kind, permission, parameters, project, enablement, implementation status,
  `expectRev`, **precheck**, handler), where the handler gave the same refusal before, so the order of refusals is
  unchanged. (Correction during implementation: this note first placed the precheck after enablement, ahead of
  `expectRev`; that changed the refusal a stale `expectRev` gets, which the reviewer's M3 test `c31_r1_mutations`
  caught on CI. Availability passes no `expectRev`, so the predicate it evaluates is the same either way.) The refusal an invocation gets is therefore the
  precheck's whenever the precheck refuses, and availability evaluates the same function.
- **Availability evaluates the static checks and the precheck only**: registered, not an event, a project when the
  declaration needs one, the declared enablement rule, implemented, then the precheck. Nothing else of the pipeline
  runs. References whose required parameters are missing are completed by a dialog (01 §0) and get the static checks
  only (their refusals depend on what the user enters).
- **No clone.** Availability lends the session to one read-only context for the length of a chunk of references and
  takes it back (`Cx` owns no copy of anything; prechecks and enablement rules only read). Nothing is staged, emitted
  or written.
- **Two refusals ask the user** rather than disable the reference, and have no precheck: `project.pathRequired`
  (Save asks for a location) and `project.unsavedChanges` (01 §3.5).

## 3. The dry run becomes the test oracle

The builder sweep (`crates/executor/tests/c41/availability.rs`) takes every GUI reference (the native menu, the default
keymap, every implemented command that is not agent-only: 269) and, in every constructed state (no project, an empty
project, nothing selected with an empty clipboard, a staging selection, a filled clipboard, an undoable step, a
redoable step, a cell selection), asserts that availability equals the outcome of a dry run of the same reference
through the pipeline, in both directions, except the two "ask the user" refusals. It runs in `check`. A handler state
refusal without a precheck shows up there as "available, but the dry run refuses".

**What a precheck may read, exactly.** Lookups by id and the session's small state (current scene, element, frame,
drawing; selection lists; the clipboard's kind and, for a cell paste, its items' ids; the layout). Two prechecks read a
bounded part of the document, stated here: `cell.blank.remove` and `cell.drawing.duplicate` read the exposure column of
each target element (the handler's own `Column::read`, one element, not the document); `element.delete` counts the
cameras of the target's scene only when a target is a camera (the handler's own last-camera rule). The 57 prechecks are
listed in the round-4 response.

**Commands whose state refusal is not expressed as a precheck:** none of the state refusals the sweep meets. The one
class left to the handler is refusals that need the document walked or a computation run (for example a vector edit
whose result would be degenerate, an export whose target cannot be written): they are not state refusals with no
parameter missing, the sweep meets none of them in its states, and availability answers "available" for them; the
handler's refusal is the command's typed error when it runs.

## 4. When passes run, and their cost

- **Triggers.** The host's native menu asks for its references once per burst of state-changing events (coalesced
  over 100 ms); the toolbar asks when it renders; the key dispatch asks for one reference per key press; the MCP `key`
  tool likewise. There is no cache per state revision: a pass costs a constant per reference, so caching would add
  invalidation risk for no gain.
- **Chunks.** One availability call is served in chunks of at most 32 references, each its own request on the document
  thread, so a user command waits at most one chunk.
- **Budgets (tests, monotonic clock, generous margins):** a full pass over every GUI reference on a project of 10,000
  command-created elements costs no more than the same pass on an empty project plus a small constant (20 ms) and
  under 50 ms on a CI runner; no single request holds the document thread for more than 20 ms on account of
  availability (measured per chunk on the document thread); a user command submitted during continuous availability
  passes on that project completes within 100 ms.

**Measured (debug build, Windows workstation; `passes_stay_constant_on_a_large_project`):** the fixture of 10,101
elements is built through 102 `element.create` commands in about 27 s (debug) or 20 s (release); a full pass over the
269 references takes 5.4 ms on the empty project and 5.7 ms on the large one; the longest chunk holds the document
thread 1.1 ms; the slowest of 20 user commands (`app.frame.set`, session state only) took 6 to 10 ms during
continuous passes, the same as without them.

## 5. Isolation

A precheck that panics is contained: availability catches the unwind for that reference and reports it unavailable
with an internal reason; the pipeline catches it likewise and answers `internal`. The session (lent, never moved into
the closure that unwinds) and the document thread survive. Tested with a test-only command whose precheck panics, in
a test registry.

## 6. Tests

| Property | Test |
|---|---|
| availability equals the dry run, every GUI reference, every constructed state, both directions | `availability::every_gui_reference_is_enabled_exactly_when_it_runs` |
| 10,000 elements: pass ≤ empty pass + 20 ms and < 50 ms; chunk ≤ 20 ms; a user command within 100 ms | `availability::passes_stay_constant_on_a_large_project` |
| a panicking precheck is contained | `availability::a_panicking_precheck_is_contained` (test registry) |
| the reviewer's parity and round-2 availability tests | unchanged |

## 7. Closure round 5: completeness by inventory, large values read once (ruling O-24)

The eight-state oracle found what its states contained; each review found the next state (C41-R5-01, -02). Ruling
O-24 makes completeness mechanical:

- **The inventory.** `scripts/check-availability-inventory.py` (in the `lint` job) collects every refusal code a GUI
  command's handler can return, from its body and the executor functions it calls, and `docs/availability-inventory.md`
  classifies every (command, code) pair: 1,090 pairs of 228 commands, `parameter` 607, `precheck` 214, `execution` 236,
  `asks-user` 13, `unreachable` 19, `deferred` 1. A missing pair, a stale row or a `precheck` row whose code the
  precheck cannot reach fails the job. The scanner's limits are in its header; the inventory's header defines the
  classes and the states the sweep builds.
- **Every state refusal C4.1 can build is a precheck.** The drawing family shares one precheck built from the
  handler's own steps (`draw::Steps`: the target drawing, read-only like `target`; the lock; the paint colour; the
  objects: given, else the selection, else the anchor selection); the cell family wraps the handler's `cell_target`
  and kind checks; the element and anim families wrap `targets`; cut, copy and delete run `pick` and the deletion's
  refusals (`deletion_refusal`, now shared with the handler's `delete_with`); paste runs the refusals `paste` gives
  before it reads the copied items (`paste_refusal`). The two findings: `draw.object.delete` refuses an empty
  selection, `element.drawingContent.delete` a locked drawing.
- **The second sweep.** `every_precheck_pair_refuses_and_passes_as_the_table_says` builds, for each of the 156
  `precheck` rows with states, the state in which the refusal occurs and one in which it does not, on the sample
  document, and checks availability against a dry run in each. The other 58 rows say why their state cannot be built
  through commands (a project always has a current element and a current scene; no command deletes a scene or empties
  a palette; an image clipboard or drawing needs an import).
- **What stays the handler's (P3 under O-24 item 4).** Where the refusal depends on the content of the selected
  objects (a group to ungroup, a line to convert, a text to break apart) the precheck tests that the selection holds
  objects of the drawing, and the row says so; reading every selected object's kind is the walk the budget forbids.

**Large values read once.** The prechecks that need facts about a selection, the clipboard or an exposure column (the
kinds of the selected elements, a locked selected element, a copied element still in the project, the cells of the
current column) compute them once per value and document revision (`crate::memo`: keyed by the shared `Arc` of the
selection list or clipboard, holding a weak reference so the key cannot be reused; the clipboard is now shared
`Arc<Value>`), during availability only: the pipeline runs prechecks inside transactions, whose working document
changes between the steps of a batch before its revision does, and computes directly. The last-camera rule scans the
scene's elements up to the first camera that stays instead of placing the tree (the handler gains the same). A chunk
ends after 32 references or once it has held the document thread 2 ms, so the first pass after a change holds the
thread at most 2 ms plus one precheck; what a selection picks is computed once for cut, copy and delete, so each of
them adds one walk to its own request.

**Measured (debug build, Windows workstation; `passes_stay_constant_on_a_large_project`):** best full pass 5.5 ms on
the empty project and on 10,101 elements; with 10,101 elements selected the first pass after the change takes 53 ms in
all, its longest request 9.5 ms, then 5.6 ms; with them on the clipboard 6.9 ms (1.2 ms), then 5.6 ms; with a current
column of 10,000 runs 9.0 ms (2.9 ms), then 5.5 ms; with 991 cameras and one current 7.0 ms (1.1 ms), then 5.4 ms. The
slowest of 20 user commands during continuous passes: 7 to 10 ms, as without passes.
