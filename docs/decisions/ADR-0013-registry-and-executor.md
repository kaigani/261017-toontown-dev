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
   `window`/`workspace` commands has a hand-written declaration (134). **Every other entry of spec A (662) has a
   generated declaration** (round 1, C12-R1-12): `scripts/gen-command-declarations.py` derives from spec A's columns
   (inventory kept equal to the spec by `crates/registry/tests/inventory.rs`) the parameter and result schemas —
   types, ranges, enums, defaults, required parameters, unknown parameters rejected, a job's final result under
   `x-jobResult` — the scopes (11 §6.1: kind, plus `files.read`/`files.write` for parameters of the file `path` type,
   `prefs` for user-wide settings, `network` for publishing, `print`, `library.global`), `needsProject` (01 §1.3),
   the undo label, confirmation conditions, one example that validates, a GUI location (events: agent-only) and the
   owning checkpoint of plan §4. **Round 2 (C12-R2-02): the generated schemas are closed and fully typed.** A
   closing pass (`scripts/command_schemas.py`) closes every object level (`additionalProperties: false`, or a typed
   map for `object (K → V)`), types every leaf (from the spec text, else from the field name), and resolves every
   named type: to the owning section's shape (`scripts/command_types.py` reads the sections' JSON examples, `jsonc`
   type markers, `Field | Type` tables, parameter groups such as `PrintParams` and 04 §17's exposure-sheet, transfer
   options and timeline objects; embedded as `$defs/spec.<Name>`), or to the model schema (`$defs/<Name>`). A
   position the section leaves undefined is a **strict placeholder** — an object accepts only `{}`, a member only a
   scalar — recorded per entry in `declared.json` and, one per command family, as SI-023 onwards in
   `plan/spec-issues.md` (written by the generator). `library.*` needs a project except `library.save` (01 §1.3);
   every library write declares `library.global`. The enablement text is the predicate: the project rule, the
   context a command defaults to (`a current drawing exists or drawingId is given`), required targets, the
   selection, and the section's own disablement rules with their codes (e.g. `anim.camera.delete`: the scene keeps
   at least one camera, 05 §12.1, `anim.lastCamera`). Each entry stores samples using every property;
   `crates/registry/tests/declared.rs` mutates every leaf of them and adds unknown members, and the schema must
   reject each. **Round 3 (C12-R3-01): types come from context before names.** An untyped field takes the type of
   the same-named typed field in its family's results (`sound.envelope.get` for `sound.envelope.point.*`), then in
   the command's own parameters, then in its family's parameters; only then a name rule, and a name never narrows a
   number to an integer; a fraction in a section example makes a number. Shapes the sections define are used where
   spec A only names them: 02's drawing read-back for `draw.drawing.get`, 03 §12.3's palette object (style-keyed
   swatch values), 04 §17.2's exposure columns, 04 §17.3's transfer options, 04 §17.5's element `media`, 05
   §19.4's velocity `pins`, 09's PrintParams for print settings, 01 §7's preference values, the model `Pen`, and a
   tool's option object for `*.options.set`; a binding's `params` is open (`x-open`: the bound command's schema
   applies). The **positive counterpart** of the mutation test: every example the owning sections give for a type
   an entry uses, or for the command itself (04 §17.2's write-back), is stored with the entry and must validate
   (literal examples as written; `jsonc` examples with their placeholders read as values). Every placeholder node
   carries the id of the spec issue that justifies it (`x-specIssue`); the family-to-issue numbering is fixed, and a
   family left without placeholders keeps its issue, marked resolved. The registry therefore has no untyped entries; CI regenerates the files and fails on drift. A declaration whose implementation belongs to a later checkpoint names it (`not_implemented = "M1 C1.3"`);
   it validates its parameters and fails with **`command.notImplemented`** `{commandId, milestone}`. Owners follow
   plan §4 checkpoint by checkpoint (C12-R1-21): e.g. `import.movie.run` and `media.*` → M8 C8.3; the `anim.*`
   transform, channel, key, path, segment, camera and projection commands and `anim.evaluate` → M3 C3.2; motion-path
   and function-editor UI, motion recording → M6 C6.2; bones and skeletons → M6 C6.3; the other animation tools,
   views and hierarchy commands → M6 C6.1; real-time `playback.*` → M5 C5.1, rendered preview and player → M10
   C10.1 (the plan names no checkpoint for real-time playback; the Timeline playhead is C5.1's). Events are
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
   Handlers never wait for input (P5). A handler that does not return cannot block callers: each call still answers
   with `command.timeout` within the budget, and the executor serves again when the handler returns
   (`tests/c12_never_block.rs`); a handler that never returns cannot be preempted in-process (no safe thread
   termination), so recovery is an engine restart by the host (C1.4). `job.wait` runs through the common contract
   in two phases (C12-R1-04): the document thread checks scopes, parameters, `expectRev` and the request cache, the
   caller's thread waits on the job table, and the document thread finishes the envelope (revision, request cache).
   While it waits, its `requestId` is owned by it: another call with the same id is `request.conflict`
   (C12-R2-06), and an identical retry attaches to it and receives exactly its envelope; the cache keeps the first
   envelope of an id and is never overwritten (C12-R3-03). In
   a sequential batch a `job.wait` step waits on the job table's condition variable too, for its `timeoutMs` but
   never beyond the remaining watchdog budget of the batch call (C12-R2-07); in an atomic batch it only polls.
   Snapshots handed out are immutable `Document` values.

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
   last 10 000 from back-patches merged per entry and aligned like a replay, with the objects' earlier revisions
   restored; older bases are `doc.diffUnavailable {oldestRev}`); the record the last save needs is never dropped
   (AG-09), and inside an atomic batch `doc.diff` sees the staged state at its predicted revision (C12-R1-08/09). A
   change to drawing content (a stroke, a node) also reports the drawing in `changes` with its new revision (REV-02,
   11 P6; C12-R1-11) — in the envelope, in `doc.changed` and in `doc.diff`, which adds a drawing whose revision moved
   or which holds a changed object (C12-R2-12); the containing drawings are found from every change before the
   `types` and `within` filters apply, so a drawing-only or drawing-scoped diff keeps them (C12-R3-04). A revision that cannot grow is `doc.limitExceeded {limit: "documentRev"}`, never an overflow
   (C12-R1-19).

5. **Envelope, dry runs, idempotency.** Every call returns the 11 §7.2 envelope; `changes` is capped at 500 with
   `changesTruncated` and `changesSince`. **A dry run is the real call on a copy of the session** (C12-R1-02/03): it
   predicts the result exactly — including what earlier steps of a sequential batch changed — and nothing of it
   (document, history, clipboard, application, window and checkpoint state, events, job cancellation) reaches the
   live session. Inside an atomic batch a step whose own parameter `dryRun` is true runs to a savepoint and is
   rolled back. `expectRev` (options or the command's parameter; an integer guards every target, a map every listed
   object, targets or not) fails with `rev.conflict {ref, id, expectedRev, currentRev, documentRev, current?}`, the
   current object included when its JSON is under 64 KiB (REV-05; C12-R1-07). **`requestId`** (C12-R1-06/22): the
   cache is per session (`agent:<session>`, `user`, …) and bounded (100 000 envelopes or 64 MiB per session, oldest
   first); the same id with the same command, parameters and options returns the first envelope — success or error
   — without running again; the same id with anything different is **`request.conflict`** `{requestId,
   originalCommand}`; an id whose envelope was evicted is **`request.unknownId`** `{requestId, reason: "evicted"}` and
   never re-executes (the session keeps a hash of every id it has seen). Codes in SI-020.

6. **Batches.** Atomic batches run every step in one context: queries see the uncommitted state, the batch commits as
   one transaction and one undo step named `label`, and any failure rolls back the document, the session state and
   every staged effect (`batch.stepFailed {stepIndex, commandId, error}`). Job commands are `batch.jobNotAllowed`; history commands
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

13. **Permissions.** Each declaration lists its scopes (hand-written, or generated per §1).
    `permission.denied {missingScopes}` precedes parameter validation. The `app` scope is never an agent's (11 §6.1).

14. **Front-end command schemas.** `tt_engine::commands::command_definitions` publishes every command's parameter
    and result schemas and the envelope as `desktop/src/ipc/generated/commands.ts`, names collision-free
    (`<commandId>.<Name>`); the front end validates them with the existing validator and wire controls (ADR-0012).

15. **List diffs by ID (model).** A list of full inline objects whose length changes is diffed by ID (one `remove`
    per deleted item, the kept items' content diffs, one `add` per created item) when IDs are unique and kept items
    keep their order; anything else is replaced as before, and every diff is still verified against the canonical
    snapshots. Inserting a stroke into a 100 000-stroke layer stores a 361-byte undo instead of a copy of the layer.

16. **The load transaction (C12-R1-01).** Installing a document into the live executor is itself a guarded
    transaction: `ExecutorHandle::open_document` (and `open_document_with` with `OpenOptions {path, readOnly,
    onUnsaved}`) sends the document to the document thread, which checks the caller's `project.open` scopes and the
    unsaved-changes rule of 01 §11.2 (`fail` → `project.unsavedChanges`; `save` → `command.notImplemented` until
    `project.save` in C1.3; `discard` needs the `app` scope, 11 §6.1, so agents cannot discard), then runs the
    **full integrity gate** on the candidate — `issues()` with every error rule (schemas, references, REV-01/02,
    §5 limits, names, trees, exposures) — whatever API produced it (C12-R2-01), and only then installs it with a
    `LoadGrant` — a token with a private field that only that check creates — emitting `project.closed` and
    `project.opened` and starting a fresh history. `Document::at_earlier_revision` checks every stamp against the
    object it returns; a reconstruction whose other objects stay newer than its revision is reported by `issues()`
    and never installed. No other path reaches the live document: the core, the request
    type and the request channel are private (compile-fail cases in `tests/ui`).

17. **Effects commit with the transaction (C12-R1-02/10).** Everything a command does outside the document — job
    cancellation, checkpoints, events, the clipboard, remembered edit options, application and window state — is
    staged with the document and committed only when the call or batch commits; a failed batch or a dry run leaves
    no trace. **Checkpoints are staged session state** (C12-R2-03): `checkpoint.create` captures the content staged
    so far at the revision it has at that step (the predicted revision when something is staged), later steps of a
    batch see and can delete it, and it goes with the batch's rollback or the dry run's discarded copy. The captured
    document is stamped as the staged changes will commit it (C12-R3-02): its `documentRev` and the touched objects'
    revisions are the revision the checkpoint advertises, so `doc.diff {sinceCheckpoint}` right after it is empty
    (`tests/c12_checkpoint_property.rs` checks random edit/checkpoint/diff sequences).

18. **Jobs belong to their session (C12-R1-05/13).** An agent sees, waits for and cancels only its own session's
    jobs (`job.notFound` otherwise); `job.list {all: true}` is the GUI's (`permission.denied` for agents). A job's
    final result must satisfy its command's `x-jobResult` schema, or the job fails with `internal` and hands out no
    result. `timedOut` is present only when true (A/11 §15.2).

19. **Clipboard and the clipboard edits (C12-R1-14/17/18/20).** `edit.clipboard.get` returns the `Clipboard` itself
    (`null` when empty) and needs a project like every edit command (01 §1.3); `edit.clipboard.set` validates the
    items against their kind (`edit.clipboardInvalid`): `drawingObjects` are `stroke`/`drawingNode` objects,
    `elements` are `element` objects or `{elementId}` references (SI-021), `elementProperties` are maps of element
    properties, `cells` are `{elementId, frameOffset, drawingRef, drawingContent?}`, `keyframes`/`functionPoints`
    `{channel, elementId?, key}`, `motionPoint` one `{channel, elementId?, point}`, `templates` library folders and
    items, `image` `{mediaType, width, height, base64}`. `edit.cut/copy/paste/delete/cutSpecial/pasteSpecial/
    pasteAsNewElement` act on the selection of the edit context (or `objectIds`; a selected column means its cells):
    drawing objects (the layer of a stroke goes with it; locked drawings refuse), elements (**cut, copy and paste
    take the selected nodes with all their descendants and paste the copied forest whole**, remapping its internal
    parentage and element references, 05 §15, C12-R2-13; delete re-attaches children to the parent, 04 §8.3, and
    repairs references to the deleted elements as 10 §7.2 repairs dangling references, SI-022 — only at locations
    the element schema declares as element references, `x-ref: "element"`; banks stay until save purges them), cells (delete blanks; cut special with create/delete
    exposure removes frames; paste overwrites or, with create/delete exposure, inserts; drawings from another bank
    replace same-name drawings or are copied; extend exposure fills the gap), keys (pasted relative to the paste
    frame; key groups filter cut/paste special), motion points (paste special pastes the selected values), library
    templates (copies into a catalog, unique names) and element properties (each value validated against its
    property's schema on the clipboard, C12-R2-10). Copies get new IDs (§22) and unique names; new IDs are given only
    in the model's containment slots, never inside opaque `meta`/`ext` data, which is copied verbatim (C12-R2-05).
    The paste destination is a typed, closed parameter: a wrong-typed field is `arg.invalid`, never a fallback to the
    current target (C12-R2-09). A cross-bank paste onto a same-name locked drawing is `drawing.locked` (04 §13.16;
    C12-R2-14). Every edit envelope reports `targets`: the context and the objects or cells acted on, or the resolved
    destination (C12-R2-08).
    Paste-as-new-element makes a drawing element with a new bank from copied cells or drawing objects. Options are
    remembered per operation in the session; `pasteSpecial {useLast}` repeats (`edit.noPreviousPasteSpecial`
    otherwise). Options that need later work (`keyframeSubstitution`, `boundaryKeys` — M3 C3.2; motion-point
    rotation/scale values without time-locked keys) are reported as `edit.optionNotApplied` warnings; pasting an
    image is image import (M10). Area milestones refine these as additive behaviour.

20. **Mode, views and workspaces (C12-R1-15/16).** The mode is derived from the active view — the central view of
    the arrangement in use, the full-screen one while full screen is on (01 §4, §5.6); `app.mode.set` activates a view
    of the requested type. Selecting a workspace saves the current arrangement first by default (`autoSave` is on by
    default, 01 §7), then reads the target, so reselecting the current workspace keeps the layout; built-in
    workspaces keep their bundled identity, so a renamed one can still be reset. Each edit context keeps its own
    selection (01 §6.4).

21. **Omitted members are addressable (JSON-04).** A patch to `/meta/x` or `/ext/x` of an object whose `meta`/`ext`
    is empty (and therefore omitted in canonical JSON) materialises the member — only a model object's own member (its
    holder has a string `type`, and no earlier token is inside opaque data), so `/meta/custom/ext/x` on an empty
    `custom` is still "no container" (C12-R2-11). The inverse removes the materialised member, so the raw input is
    restored byte-exact; the canonical form is unchanged.

22. **IDs come from the transaction (C12-R2-04).** Commands that create objects draw IDs from `Cx::new_id`: a stream
    seeded from the project's ID and the live `documentRev` and advanced per ID (the ULID time part is a logical
    clock). The same document and the same calls produce the same IDs (11 P7, 10 AG-06), a dry run predicts exactly
    the IDs the real call creates, and a step rolled back to its savepoint returns its IDs to the stream. Seeds
    differ between projects and between revisions, so a committed ID is never produced again.

## Consequences

- Area milestones add behaviour by replacing `not_implemented` declarations with handlers that stage through `Cx`;
  the registry, envelope, history, events and conformance tests apply unchanged. A hand-written declaration replaces
  a generated one by re-running the generator (the registry refuses an id declared twice). The parity test covers
  the menu manifest and every entry's GUI location or justification, skipping nothing; the GUI half grows with M4.
- At 100 000 strokes a one-property edit costs about 2.4–2.8 s end to end (release, Windows; `commit_scale`), almost
  all of it the full staged integrity check the C1.1 rulings require (the entry's input path ≈ 0.78 s, two entry
  serialisations ≈ 0.37 s, the verified undo diff ≈ 0.47 s, index rebuild). The touched-object change computation is
  7 ms against 1 045 ms for the whole-entry own-view diff. Interactive drawing at that size needs node-granular
  staging (validate and diff only the touched layer); that is drawing-engine work for M2 C2.1, which owns the
  stroke model in motion.
- The generated declarations still hold strict placeholders where the owning sections leave shapes undefined
  (SI-023 onwards); a placeholder accepts nothing undefined, and the implementing checkpoint replaces it with the
  section's shape when the spec owner answers the issue.
- New error codes: `command.notImplemented`, `command.timeout`, `history.diverged`, `checkpoint.exists`,
  `checkpoint.notFound`, `job.notFound`, `request.conflict`, `request.unknownId`; warning `edit.optionNotApplied`;
  11 §10 does not list them (SI-020).
