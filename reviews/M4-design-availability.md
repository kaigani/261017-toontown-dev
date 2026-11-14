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
