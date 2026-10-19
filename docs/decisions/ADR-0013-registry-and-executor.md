# ADR-0013 — Command registry and serialized executor

Status: accepted (M1 C1.2, 2026-10-18). Scope: plan §4 row M1, checkpoint C1.2; the reviewer's final C1.1 ticket list
(`reviews/M1-C1.1-r7-closure.md`); M0 readiness bullets 2–4.

## Context

C1.1 delivered an immutable, copy-on-write object model with staged commits (`Document::with_patched_entry`), verified
canonical undo/redo patches (C11-R6-01), own views and change descriptions, but no revisions in motion, no
transactions, no commands. Spec 11 requires one serialized executor through which every mutation flows (P2), a
registry of every command with schemas, enablement, scopes and examples (§2, §8, §14), envelopes with `dryRun`,
`expectRev`, `requestId` (§7.2), batches (§7.3), undo/redo and checkpoints (§7.7), and a never-block guarantee (P5,
§14.6). Spec 10 fixes revisions (REV-01…06) and the generic patch (AG-02). Spec 01 makes window, panel and workspace
state commands (§5, §12).

## Decision

1. **Registry.** Commands are declared next to their handlers with the `#[tt_registry::command]` attribute (crate
   `tt-registry-macros`): id, kind, label, scopes, undo label, confirmation conditions, enablement rule, project
   requirement, examples (at least one), GUI location or `agentOnly` justification; the parameter and result schemas
   are derived from the handler's types (draft 2020-12, the model's integer bounds). Every command of the M1
   namespaces (`app`, `project`, `edit`, `doc`, `agent`, `inspect`, `checkpoint`, `job`) and the kernel-level
   `window`/`workspace` commands has a typed declaration (134: 62 implemented, 72 declared for later checkpoints). The
   remaining 662 entries of spec A are inventory
   stubs generated from the spec (`scripts/gen-command-inventory.py`, kept equal by `crates/registry/tests/inventory.rs`).
   A declaration whose implementation belongs to a later checkpoint names it (`not_implemented = "M1 C1.3"`); it
   validates and decodes its parameters and fails with **`command.notImplemented`** `{commandId, milestone}`. Stubs
   fail the same way with the owner from `owner_of` (plan §4: namespace default plus exact overrides). Events are
   registered with their payload as the result schema (the spec's columns differ, SI-016); invoking one is
   `command.disabled` (reason `event`). **Registry version** `0.1.0` (semver: minor = additive; major 0 until the
   surface is frozen for version 1; every checkpoint that implements commands raises the minor version).

2. **Generic patch and whole-object policy (AG-02; the deferred C1.1 decision).** `doc.object.patch` changes writable
   persisted properties only:
   - an operation on the object root (`""`: whole-object `add`/`replace`/`remove`) is **`doc.structuralChange`**
     (`reason: wholeObject`) — replacing an object would rewrite its identity, revision and children; agents patch
     properties (`props` or `ops`);
   - an operation on a child collection (a slot of 10 §3) is `doc.structuralChange` naming the owning area command;
   - `readOnly`, `x-derived` and `x-transient` properties (any prefix of the pointer) are **`doc.readOnlyProperty`**;
     an unknown property is `doc.schemaViolation`;
   - a property annotated `x-command` — on any prefix of the pointer or anywhere inside the replaced value — follows
     that command's semantics; until the command is implemented the patch fails with `command.notImplemented`
     naming the command and its owner (never a silent raw write);
   - `test` reads anything; `props` is RFC 7386 (null removes, nulls inside new values dropped); all targets commit
     atomically as one undo step labelled `label` or "Set <first property>"; edits of a locked drawing's content are
     `drawing.locked`; `expectRev` (int for every target, or per reference) and the parameter `dryRun` are honoured.

3. **One document thread.** `ExecutorHandle::spawn` starts the thread that owns the session (open project, history,
   checkpoints, revision log, application context, window/workspace state, clipboard). Every call is a message;
   the caller waits at most the **watchdog budget** (default 30 s) and otherwise gets **`command.timeout`**
   `{commandId, budgetMs}` — the work may still complete, and the hint says to read `doc.state.get` before retrying.
   Handlers never wait for input (P5). `job.wait` waits on the caller's thread against the job table, never on the
   document thread; inside a batch it only polls. Snapshots handed out are immutable `Document` values.

4. **Transactions and revisions.** A handler stages changes through `Cx` (`patch_objects` for property edits,
   `patch_entries` for structural edits from M2 on); staging commits through `Document::with_patched_entries_undo_only`
   — the C1.1 staged commit with the full input path of each touched entry and the complete `issues()` of the staged
   document — and keeps the verified canonical undo patch. Change records are the **touched objects' own-view
   diffs** (property edits) or the changed entries' diff (structural edits); an empty change set commits nothing (no
   revision, no step). A commit gets `documentRev + 1`, stamps every created or modified object (a drawing-local
   object also stamps its drawing, REV-02), pushes one undo step `u-<n>`, logs the revision and emits `doc.changed`.
   **Undo and redo are transactions** (REV-03): each replayed step gets the next revision and stamps what it changes.
   A replay first checks every stored `oldValue` precondition against the live entry **modulo the `rev` fields of
   model objects** (a content difference is `history.diverged`; nothing is applied), merges same-entry commits, and
   keeps the verified patch the replay returns as the opposite direction (the redo returned by undo). Step ids are
   stable across replays. **Checkpoints** are copy-on-write snapshots; `checkpoint.restore` is one undoable step whose
   objects with changed content get the new revision and whose content-equal objects keep the larger of their
   current and target revisions (no revision decreases). History keeps 1 000 steps (01 §6.1: at least 200) and a clean
   marker. `doc.diff` rebuilds earlier revisions from the revision log (the last 256 with their previous snapshot, the
   last 10 000 from back-patches; older bases are `doc.diffUnavailable {oldestRev}`).

5. **Envelope, dry runs, idempotency.** Every call returns the 11 §7.2 envelope; `changes` is capped at 500 with
   `changesTruncated` and `changesSince`. A dry run runs the handler on the staged state and reports the changes and
   the revisions they would get; the session's application, window and checkpoint state is restored afterwards.
   `requestId` is idempotent per session (`agent:<session>`, `user`, …): the first envelope — success or error — is
   returned again (10 000 remembered). A failing call restores the same session state.

6. **Batches.** Atomic batches run every step in one context: queries see the uncommitted state, the batch commits as
   one transaction and one undo step named `label`, and any failure rolls back the document and the session state
   (`batch.stepFailed {stepIndex, commandId, error}`). Job commands are `batch.jobNotAllowed`; history commands
   (undo, redo, history moves, checkpoint restore) are disabled inside atomic batches. References in parameters are
   strings that are exactly `$<n>.<path>` (`n` 1-based) or `$<name>.<path>` (`as`), resolved against the earlier
   steps' envelopes along a dotted path (SI-017). Non-atomic batches run steps as separate invocations;
   `stopOnError` decides whether to continue. A batch has 1–1 000 steps.

7. **Special references (11 §5.1).** `@current` as the first path segment is the current element, else the current
   scene; a bare `@current` given to a typed parameter is the current object of that type (scene, element, drawing,
   project). `@selection` is the single selected object; list parameters (`targets`, `refs`) expand it to the whole
   selection. `@activeView` is what the central view shows (the current drawing for the drawing view, else the
   scene); `@activeCamera` is the current scene's current camera (SI-018).

8. **Kernel window and workspace state (readiness bullet 4).** The executor keeps the arrangement the shell renders:
   the 21 panels of 01 §5.2, dock areas with tab groups, floating panels with geometry, tool palettes, the main
   toolbar, corner ownership, focus, the separate full-screen arrangement (01 §5.6) and the full-screen flag set
   explicitly by `window.fullScreen.set {enabled}` (never a toggle). The four built-in workspaces carry the bundled
   arrangements of 01 §5.5; `workspace.*` lists, reads, selects, saves, creates, renames, deletes (built-ins and the
   last workspace are protected), resets and applies layouts. `edit.*` contexts set to `auto` follow the focused
   panel. Main-toolbar items are registered command ids or the group ids `gridType`, `gridOverlay`,
   `onionSkinPrevious`, `onionSkinNext`, `firstFrame`, `previousFrame`, `playStop`, `nextFrame`, `lastFrame`,
   `cameraList`, `sceneList`, `workspaceList` and the spacers (SI-019). Per-user persistence follows with settings
   (C1.3/M4); native window geometry, minimise and raise belong to the shell (M4 C4.1).

9. **Derived projections (AG-04).** `doc.object.get {include: ["derived"]}` adds the properties the schemas mark
   `x-derived`/`x-transient` that can be computed from the model now: scene `length`; element `parentId`,
   `childIds`, `depth`, `timelineIndex`, `xsheetIndex`, `cloneGroupId`, `cloneIds`, `drawingCount`,
   `hiddenByParent`; bank `elementIds`; drawing `usage`, `isEmpty`; particle block `index`; library `readOnly`,
   `writeAccessHeldBy`, `location`. The others are left out and named with their owners in a
   `doc.derivedUnavailable` warning (thumbnails M3 C3.3, zones M2 C2.1, storyline timing M9, preferences M4 C4.1,
   media information M8/M10, asset references C1.3, broken library items M9). `include: ["stats"]` returns
   `{id, jsonBytes, objectsByType}` per object beside the object (SI-015).

10. **Drawing-command delegation (C1.1 handoff gap 8).** `draw.drawing.rename` and `draw.lock.set` (02) are not
    aliases of `element.drawing.rename` and `element.drawingLock.set` (04): their parameters differ (02 defaults to
    the current drawing; 04 takes ids and lists). Both stay registered; the 02 commands delegate to the 04 commands,
    whose semantics the drawing schema's `x-command` names (`tt_registry::DELEGATES`, reported as `delegatesTo` by
    `describe_command`).

11. **Exclusive mutation capability.** `Cx` has private fields, no public constructor, no `Clone`/`Default`; only
    the executor creates one, handlers get `&mut Cx`, enablement checks get `&dyn Any` (read only), jobs take
    `'static` closures (they cannot keep a context), the session module is private, and model documents and entities
    have no writable fields. `crates/executor/tests/compile_fail.rs` proves each property with trybuild.

12. **Workers.** The engine's background work runs on Rust threads (jobs). The front end needs no web workers, so the
    worker ban of the IPC boundary stays in all three layers (C01-R4-02 unchanged).

13. **Permissions.** Each declaration lists its scopes; stubs derive them from their kind (query → read, ui/toggle →
    ui, action/job → edit). `permission.denied {missingScopes}` precedes parameter validation. The `app` scope is
    never an agent's (11 §6.1).

14. **Front-end command schemas.** `tt_engine::commands::command_definitions` publishes every typed command's
    parameter and result schemas and the envelope as `desktop/src/ipc/generated/commands.ts`, names collision-free
    (`<commandId>.<Name>`); the front end validates them with the existing validator and wire controls (ADR-0012).

15. **List diffs by ID (model).** A list of full inline objects whose length changes is diffed by ID (one `remove`
    per deleted item, the kept items' content diffs, one `add` per created item) when IDs are unique and kept items
    keep their order; anything else is replaced as before, and every diff is still verified against the canonical
    snapshots. Inserting a stroke into a 100 000-stroke layer stores a 361-byte undo instead of a copy of the layer.

## Consequences

- Area milestones add behaviour by replacing `not_implemented` declarations with handlers that stage through `Cx`;
  the registry, envelope, history, events and conformance tests apply unchanged. The parity test covers the menu
  manifest now and every typed declaration's GUI location or justification; the GUI half grows with M4.
- At 100 000 strokes a one-property edit costs about 2.4–2.8 s end to end (release, Windows; `commit_scale`), almost
  all of it the full staged integrity check the C1.1 rulings require (the entry's input path ≈ 0.78 s, two entry
  serialisations ≈ 0.37 s, the verified undo diff ≈ 0.47 s, index rebuild). The touched-object change computation is
  7 ms against 1 045 ms for the whole-entry own-view diff. Interactive drawing at that size needs node-granular
  staging (validate and diff only the touched layer); that is drawing-engine work for M2 C2.1, which owns the
  stroke model in motion.
- New error codes: `command.notImplemented`, `command.timeout`, `history.diverged`, `checkpoint.exists`,
  `checkpoint.notFound`, `job.notFound`; 11 §10 does not list them (SI-020).
