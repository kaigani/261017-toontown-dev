# Availability inventory

The completeness rule of command availability (ruling O-24; ADR-0021 decision 9; reviews/M4-design-availability.md).
Availability evaluates a command's static checks and its declared precheck; it is complete when every refusal a
GUI command's handler can give for a state reason is also returned by its precheck. This table classifies every
(command, refusal code) pair that `scripts/check-availability-inventory.py` finds in the handler of every command a GUI
surface can offer (every implemented command that is not agent-only) and in the executor functions it calls. The
script runs in the `lint` job and fails when a pair is missing here, when a row is stale, or when a pair marked
`precheck` cannot be reached from the command's declared precheck; its header states the scanner's limits.

Pairs whose code the command's static checks already return (`project.none` of a command that needs a project; the
codes of its enablement rule) are answered before the handler runs and are not listed.

## Classes

- **parameter**: the refusal depends on what the caller supplies (an explicit ID, a value out of range, the caller's
  role: an agent's confirmations, permissions and file-system roots).
- **precheck**: the declared precheck returns it. The states column names the state in which the refusal occurs and one
  in which it does not; `crates/executor/tests/c41/availability.rs` (`every_precheck_pair_refuses_and_passes_as_the_table_says`)
  builds both and checks availability against a dry run in each, or the column says why the state cannot be built.
- **asks-user**: one of the two refusals that keep the reference enabled because the user is asked (`project.pathRequired`,
  `project.unsavedChanges`; 01 §3.5).
- **execution**: an outcome of doing the work that no state check can predict (a file system failure, an internal
  invariant, a cancelled job, a work budget reached during the work, a refusal of a job after the call returned).
- **deferred &lt;checkpoint&gt;**: a refusal whose state needs a view or a facility that does not exist yet; the owner is the
  checkpoint that builds it. The M4 merge note lists these rows.
- **unreachable**: the scanner over-approximates (it follows calls by name); the reason names the guard that makes the
  path impossible.

Where a precheck covers a pair's state refusal in part (it tests that the selection holds objects, while the handler
also reads the selected objects' kinds, which is not constant cost), the reason says so: those remaining cases are
"reported available but the handler refuses for a state reason", P3 under ruling O-24 item 4.

## States the sweep builds

All on the sample document (scene `Intro`, drawing element `Bob`) unless named otherwise:
`drawing` (Bob current at frame 1, which exposes a drawing); `no-drawing` (a new project: no element, no drawing);
`locked-drawing` (`drawing`, its drawing locked); `drawing-selected` (`drawing`, one of its objects selected);
`peg-current` (a peg made current); `element` (Bob current); `no-element` (a new project: no element);
`blank-cell` (Bob current at a frame past its exposure, after a blank insertion there); `element-selected` (Bob
selected in the Timeline); `peg-selected` (a peg selected in the Timeline); `locked-element-selected` (Bob locked and
selected); `camera-selected` (the scene's only camera selected); `sound-current` (a sound element current, which has no
transformation); `no-camera` (a new project with a drawing element and no camera); `clipboard-empty` (nothing
copied); `clipboard-cells` (Bob's frame-1 cell copied, Bob current); `clipboard-cells-no-element` (cells copied, then
the Camera view focused); `frame-3` (Bob current at frame 3); `no-project` (no project open); `drawing-view` (the
Drawing view window made active, M4 C4.2); `painted-zone-selected` (a closed rectangle on a new layer, its zone
painted and selected, no brush stroke; M4 C4.2). A JSON object after a state name gives the reference's parameters in that state.

## Every command

| command | code | class | reason |
|---|---|---|---|
| `*` | `command.timeout` | execution | the caller's watchdog budget passed while the work ran |
| `*` | `confirmation.declined` | parameter | only agent callers are asked to confirm (11 §6.3); the GUI calls as the user |
| `*` | `confirmation.expired` | parameter | only agent callers are asked to confirm (11 §6.3); the GUI calls as the user |
| `*` | `confirmation.required` | parameter | only agent callers are asked to confirm (11 §6.3); the GUI calls as the user |
| `*` | `doc.limitExceeded` | execution | a document limit the change itself would pass, known once the change is computed: the revision counter exhausted (10 REV-01; cannot happen in practice), or an element made longer than a scene's maximum frame count (`timing::check_length`, the frame-array bound) |
| `*` | `fs.outsideRoots` | parameter | an agent's file-system roots (11 §6.2); the GUI calls as the user |
| `*` | `history.diverged` | execution | replaying an undo step met a document that diverged (an internal invariant) |
| `*` | `internal` | execution | an internal invariant failed (never a state the GUI can see) |
| `*` | `job.cancelled` | execution | the job the call waited for was cancelled |
| `*` | `limit.exceeded` | execution | a work or size budget reached while the work runs (11 §8 never-block rule) |
| `*` | `permission.denied` | parameter | an agent's permissions (11 §6.2); the GUI calls as the user |
| `*` | `project.pathRequired` | asks-user | Save asks the user for a location (01 §3.5) |
| `*` | `project.unsavedChanges` | asks-user | the user is asked to save, discard or cancel (01 §3.5) |
| `*` | `render.failed` | execution | rasterising or encoding failed |
| `*` | `text.glyphsUnavailable` | execution | the system fonts lack the glyphs (a property of the machine) |

## By command

| command | code | class | reason | states |
|---|---|---|---|---|
| `agent.session.disconnect` | `agent.sessionNotFound` | parameter | the agent session the caller names does not exist |  |
| `agent.session.grant` | `agent.sessionNotFound` | parameter | the agent session the caller names does not exist |  |
| `agent.session.grant` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `agent.session.pause` | `agent.sessionNotFound` | parameter | the agent session the caller names does not exist |  |
| `agent.session.resume` | `agent.sessionNotFound` | parameter | the agent session the caller names does not exist |  |
| `anim.camera.create` | `anim.valueOutOfRange` | parameter | a value is out of range |  |
| `anim.camera.create` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `anim.camera.create` | `command.disabled` | precheck | nothing selected, or no current scene | untestable: a project always has a current scene: it is created with one, and no implemented command deletes a scene or clears the current one |
| `anim.camera.create` | `element.countOutOfRange` | parameter | the count is out of range |  |
| `anim.camera.create` | `element.nameDuplicate` | parameter | the name the caller gives is taken |  |
| `anim.camera.create` | `element.nameTooLong` | parameter | the name is too long |  |
| `anim.camera.create` | `element.notCellBased` | parameter | a cell parameter given for an element type without cells (`element.create`) |  |
| `anim.camera.create` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `anim.camera.create` | `element.notParentable` | parameter | the parent the caller names cannot take children |  |
| `anim.camera.current.get` | `command.disabled` | precheck | nothing selected, or no current scene | untestable: a project always has a current scene: it is created with one, and no implemented command deletes a scene or clears the current one |
| `anim.camera.current.set` | `anim.notFound` | parameter | the camera the caller names does not exist |  |
| `anim.camera.list` | `command.disabled` | precheck | nothing selected, or no current scene | untestable: a project always has a current scene: it is created with one, and no implemented command deletes a scene or clears the current one |
| `anim.function.curveType.set` | `anim.channelUnsupported` | parameter | the channel the caller names does not apply to the node |  |
| `anim.function.curveType.set` | `anim.curveTypeUnsupported` | parameter | the curve type the caller names does not apply |  |
| `anim.function.curveType.set` | `anim.notFound` | parameter | the function the caller names does not exist |  |
| `anim.function.key.add` | `anim.channelUnsupported` | parameter | the channel the caller names does not apply to the node |  |
| `anim.function.key.add` | `anim.frameInvalid` | parameter | a frame value is outside 1-100000 |  |
| `anim.function.key.add` | `anim.frameNotInteger` | parameter | a key frame must be an integer |  |
| `anim.function.key.add` | `anim.keyExists` | parameter | a key exists at the frame the caller names |  |
| `anim.function.key.add` | `anim.notFound` | parameter | the function the caller names does not exist |  |
| `anim.function.key.add` | `anim.valueOutOfRange` | parameter | a value is out of range |  |
| `anim.function.key.duplicate` | `anim.channelUnsupported` | parameter | the channel the caller names does not apply to the node |  |
| `anim.function.key.duplicate` | `anim.frameInvalid` | parameter | a frame value is outside 1-100000 |  |
| `anim.function.key.duplicate` | `anim.frameNotInteger` | parameter | a key frame must be an integer |  |
| `anim.function.key.duplicate` | `anim.keyExists` | parameter | a key exists at the frame the caller names |  |
| `anim.function.key.duplicate` | `anim.notFound` | parameter | the function or key the caller names does not exist |  |
| `anim.function.key.remove` | `anim.channelUnsupported` | parameter | the channel the caller names does not apply to the node |  |
| `anim.function.key.remove` | `anim.frameInvalid` | parameter | a frame value is outside 1-100000 |  |
| `anim.function.key.remove` | `anim.frameNotInteger` | parameter | a key frame must be an integer |  |
| `anim.function.key.remove` | `anim.notFound` | parameter | the function or key the caller names does not exist |  |
| `anim.function.key.set` | `anim.channelUnsupported` | parameter | the channel the caller names does not apply to the node |  |
| `anim.function.key.set` | `anim.frameInvalid` | parameter | a frame value is outside 1-100000 |  |
| `anim.function.key.set` | `anim.frameNotInteger` | parameter | a key frame must be an integer |  |
| `anim.function.key.set` | `anim.keyExists` | parameter | a key exists at the frame the caller names |  |
| `anim.function.key.set` | `anim.notFound` | parameter | the function or key the caller names does not exist |  |
| `anim.function.key.set` | `anim.pathEndKey` | parameter | the motion-path point the caller names is an end key |  |
| `anim.function.key.set` | `anim.valueOutOfRange` | parameter | a value is out of range |  |
| `anim.function.list` | `anim.notFound` | parameter | the node the caller names does not exist |  |
| `anim.function.preset.apply` | `anim.channelUnsupported` | parameter | the channel the caller names does not apply to the node |  |
| `anim.function.preset.apply` | `anim.curveTypeUnsupported` | parameter | the curve type the caller names does not apply |  |
| `anim.function.preset.apply` | `anim.frameInvalid` | parameter | a frame value is outside 1-100000 |  |
| `anim.function.preset.apply` | `anim.notFound` | parameter | the function or key the caller names does not exist |  |
| `anim.key.add` | `anim.channelUnsupported` | parameter | the channel the caller names does not apply to the node |  |
| `anim.key.add` | `anim.frameInvalid` | parameter | a frame value is outside 1-100000 |  |
| `anim.key.add` | `anim.frameNotInteger` | parameter | a key frame must be an integer |  |
| `anim.key.add` | `anim.notFound` | parameter | the node, function or key the caller names does not exist |  |
| `anim.key.add` | `anim.notTransformable` | precheck | the target element has no transformation | refuse: sound-current · pass: element |
| `anim.key.add` | `element.notFound` | precheck | no element given, selected or current | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `anim.key.move` | `anim.channelUnsupported` | parameter | the channel the caller names does not apply to the node |  |
| `anim.key.move` | `anim.frameInvalid` | parameter | a frame value is outside 1-100000 |  |
| `anim.key.move` | `anim.frameNotInteger` | parameter | a key frame must be an integer |  |
| `anim.key.move` | `anim.keyExists` | parameter | a key exists at the frame the caller names |  |
| `anim.key.move` | `anim.notFound` | parameter | the node or key the caller names does not exist |  |
| `anim.key.remove` | `anim.channelUnsupported` | parameter | the channel the caller names does not apply to the node |  |
| `anim.key.remove` | `anim.frameInvalid` | parameter | a frame value is outside 1-100000 |  |
| `anim.key.remove` | `anim.frameNotInteger` | parameter | a key frame must be an integer |  |
| `anim.key.remove` | `anim.notFound` | parameter | the node, function or key the caller names does not exist |  |
| `anim.key.remove` | `element.notFound` | precheck | no element given, selected or current | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `anim.loop.set` | `anim.frameInvalid` | parameter | a frame value is outside 1-100000 |  |
| `anim.loop.set` | `anim.notFound` | parameter | the function the caller names does not exist |  |
| `anim.loop.set` | `anim.valueOutOfRange` | parameter | a value is out of range |  |
| `anim.loop.set` | `element.notLoopable` | parameter | the element the caller names cannot loop |  |
| `anim.path.lockDepth.set` | `anim.frameInvalid` | parameter | a frame value is outside 1-100000 |  |
| `anim.path.lockDepth.set` | `anim.frameNotInteger` | parameter | a key frame must be an integer |  |
| `anim.path.lockDepth.set` | `anim.notFound` | parameter | the path the caller names does not exist |  |
| `anim.path.lockDepth.set` | `anim.notTransformable` | parameter | the node the caller names has no transformation |  |
| `anim.path.point.add` | `anim.channelUnsupported` | parameter | the channel the caller names does not apply to the node |  |
| `anim.path.point.add` | `anim.frameInvalid` | parameter | a frame value is outside 1-100000 |  |
| `anim.path.point.add` | `anim.frameNotInteger` | parameter | a key frame must be an integer |  |
| `anim.path.point.add` | `anim.keyExists` | parameter | a key exists at the frame the caller names |  |
| `anim.path.point.add` | `anim.notFound` | parameter | the path the caller names does not exist |  |
| `anim.path.point.add` | `anim.notTransformable` | parameter | the node the caller names has no transformation |  |
| `anim.path.point.add` | `anim.valueOutOfRange` | parameter | a value is out of range |  |
| `anim.path.sample` | `anim.frameInvalid` | parameter | a frame value is outside 1-100000 |  |
| `anim.path.sample` | `anim.notFound` | parameter | the path the caller names does not exist |  |
| `anim.path.sample` | `anim.notTransformable` | parameter | the node the caller names has no transformation |  |
| `anim.path.sample` | `anim.valueOutOfRange` | parameter | a value is out of range |  |
| `anim.segment.set` | `anim.channelUnsupported` | parameter | the channel the caller names does not apply to the node |  |
| `anim.segment.set` | `anim.frameInvalid` | parameter | a frame value is outside 1-100000 |  |
| `anim.segment.set` | `anim.frameNotInteger` | parameter | a key frame must be an integer |  |
| `anim.segment.set` | `anim.notFound` | parameter | the function or key the caller names does not exist |  |
| `anim.segment.set` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `anim.segment.uniform.apply` | `anim.channelUnsupported` | parameter | the channel the caller names does not apply to the node |  |
| `anim.segment.uniform.apply` | `anim.frameInvalid` | parameter | a frame value is outside 1-100000 |  |
| `anim.segment.uniform.apply` | `anim.frameNotInteger` | parameter | a key frame must be an integer |  |
| `anim.segment.uniform.apply` | `anim.notFound` | parameter | the function the caller names does not exist |  |
| `anim.transform.set` | `anim.channelUnsupported` | parameter | the channel the caller names does not apply to the node |  |
| `anim.transform.set` | `anim.frameInvalid` | parameter | a frame value is outside 1-100000 |  |
| `anim.transform.set` | `anim.frameNotInteger` | parameter | a key frame must be an integer |  |
| `anim.transform.set` | `anim.notFound` | parameter | the node, function or key the caller names does not exist |  |
| `anim.transform.set` | `anim.notTransformable` | precheck | the target element has no transformation | refuse: sound-current · pass: element |
| `anim.transform.set` | `anim.valueOutOfRange` | parameter | a value is out of range |  |
| `anim.transform.set` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `anim.transform.set` | `element.notFound` | precheck | no element given, selected or current | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `app.frame.set` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `app.mode.set` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `app.quit` | `project.none` | unreachable | `resolve_unsaved` returns at once without a project; nothing else reads it |  |
| `cell.blank.add` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `cell.blank.add` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `cell.blank.add` | `cell.noTarget` | precheck | no cell selection and no current element (or a blank current cell) | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `cell.blank.add` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `cell.blank.remove` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `cell.blank.remove` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `cell.blank.remove` | `cell.noTarget` | precheck | no cell selection and no current element (or a blank current cell) | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `cell.blank.remove` | `cell.notBlank` | precheck | the cells to remove are not blank | refuse: element · pass: blank-cell |
| `cell.blank.remove` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `cell.clipboard.apply` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `cell.clipboard.apply` | `cell.clipboardEmpty` | precheck | the clipboard holds no cells | refuse: clipboard-empty · pass: clipboard-cells |
| `cell.clipboard.apply` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `cell.clipboard.apply` | `cell.noTarget` | precheck | no cell selection and no current element (or a blank current cell) | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `cell.clipboard.apply` | `cell.notApplicable` | precheck | the target element is not of a kind the command edits | refuse: clipboard-cells+sound-current · pass: clipboard-cells |
| `cell.clipboard.apply` | `cell.typeMismatch` | parameter | the copied cells do not fit the target columns (the copied items) |  |
| `cell.clipboard.apply` | `command.disabled` | precheck | nothing selected, or no current scene | untestable: a project always has a current scene: it is created with one, and no implemented command deletes a scene or clears the current one |
| `cell.clipboard.apply` | `command.notImplemented` | unreachable | `paste_elements` pastes an elements clipboard: this is a refusal of another kind's branch of `paste` |  |
| `cell.clipboard.apply` | `drawing.locked` | unreachable | `paste_elements` pastes an elements clipboard: this is a refusal of another kind's branch of `paste` |  |
| `cell.clipboard.apply` | `edit.clipboardEmpty` | unreachable | `paste_elements` runs only when the clipboard holds elements |  |
| `cell.clipboard.apply` | `edit.pasteNotAllowed` | parameter | clipboard content an agent set (`edit.clipboard.set`) naming an element that is not in the project; a copy carries the elements themselves |  |
| `cell.clipboard.apply` | `edit.wrongElementType` | unreachable | `paste_elements` pastes an elements clipboard: this is a refusal of another kind's branch of `paste` |  |
| `cell.clipboard.apply` | `element.notCellBased` | precheck | the target element has no cells | refuse: clipboard-cells+peg-current · pass: clipboard-cells |
| `cell.clipboard.apply` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `cell.clipboard.set` | `anim.lastCamera` | precheck | the scene's only camera would go | refuse: camera-current {"wholeElements": true, "cut": true} · pass: element {"wholeElements": true, "cut": true} |
| `cell.clipboard.set` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `cell.clipboard.set` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `cell.clipboard.set` | `cell.noTarget` | precheck | no cell selection and no current element (or a blank current cell) | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `cell.clipboard.set` | `cell.notApplicable` | precheck | the target element is not of a kind the command edits | refuse: sound-current · pass: element |
| `cell.clipboard.set` | `command.disabled` | unreachable | the targets are never empty (`cell_target` refuses first with `cell.noTarget`) |  |
| `cell.clipboard.set` | `drawing.locked` | unreachable | whole elements are cut as elements: the drawing-object branch of the deletion refusals does not apply |  |
| `cell.clipboard.set` | `edit.clipboardInvalid` | execution | a consistency check of content copied from a valid document |  |
| `cell.clipboard.set` | `element.locked` | precheck | a selected element is locked | refuse: locked-element {"wholeElements": true, "cut": true} · pass: element {"wholeElements": true, "cut": true} |
| `cell.clipboard.set` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `cell.columnate.apply` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `cell.columnate.apply` | `cell.drawingNotFound` | parameter | the drawing name the caller gives is not in the column's bank |  |
| `cell.columnate.apply` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `cell.columnate.apply` | `cell.noTarget` | precheck | no cell selection and no current element (or a blank current cell) | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `cell.columnate.apply` | `cell.notApplicable` | precheck | the target element is not of a kind the command edits | refuse: peg-current · pass: element |
| `cell.columnate.apply` | `element.notCellBased` | unreachable | `applicable` admits only element kinds with a drawing bank, before `bank_of_element` |  |
| `cell.columnate.apply` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `cell.cycle.create` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `cell.cycle.create` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `cell.cycle.create` | `cell.noTarget` | precheck | no cell selection and no current element (or a blank current cell) | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `cell.cycle.create` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `cell.drawing.duplicate` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `cell.drawing.duplicate` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `cell.drawing.duplicate` | `cell.noTarget` | precheck | no cell selection and no current element (or a blank current cell) | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `cell.drawing.duplicate` | `cell.notApplicable` | precheck | the target element is not of a kind the command edits | refuse: peg-current · pass: element |
| `cell.drawing.duplicate` | `drawing.nameDuplicate` | parameter | the name the caller gives is taken |  |
| `cell.drawing.duplicate` | `element.notCellBased` | unreachable | `applicable` admits only element kinds with a drawing bank, before `bank_of_element` |  |
| `cell.drawing.duplicate` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `cell.exposure.add` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `cell.exposure.add` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `cell.exposure.add` | `cell.noTarget` | precheck | no cell selection and no current element (or a blank current cell) | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `cell.exposure.add` | `cell.notApplicable` | precheck | the target element is not of a kind the command edits | refuse: sound-current · pass: element |
| `cell.exposure.add` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `cell.exposure.remove` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `cell.exposure.remove` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `cell.exposure.remove` | `cell.noTarget` | precheck | no cell selection and no current element (or a blank current cell) | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `cell.exposure.remove` | `cell.notApplicable` | precheck | the target element is not of a kind the command edits | refuse: sound-current · pass: element |
| `cell.exposure.remove` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `cell.exposure.set` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `cell.exposure.set` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `cell.exposure.set` | `cell.noTarget` | precheck | no cell selection and no current element (or a blank current cell) | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `cell.exposure.set` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `cell.fill.apply` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `cell.fill.apply` | `cell.frameBeforeSelection` | precheck | the target frame is before the first target cell | refuse: frame-3 {"toFrame": 2} · pass: frame-3 {"toFrame": 5} |
| `cell.fill.apply` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `cell.fill.apply` | `cell.noTarget` | precheck | no cell selection and no current element (or a blank current cell) | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `cell.fill.apply` | `cell.notApplicable` | precheck | the target element is not of a kind the command edits | refuse: peg-current · pass: element |
| `cell.fill.apply` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `cell.label.set` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `cell.label.set` | `cell.drawingNotFound` | parameter | the drawing name the caller gives is not in the column's bank |  |
| `cell.label.set` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `cell.label.set` | `cell.noTarget` | precheck | no cell selection and no current element (or a blank current cell) | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `cell.label.set` | `cell.notApplicable` | precheck | the target element is not of a kind the command edits | refuse: peg-current {"step": 1} · pass: element {"step": 1} |
| `cell.label.set` | `drawing.notFound` | parameter | a drawing the caller names does not exist (the current drawing always exists: the context is repaired) |  |
| `cell.label.set` | `element.notCellBased` | unreachable | `applicable` admits only element kinds with a drawing bank, before `bank_of_element` |  |
| `cell.label.set` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `cell.note.set` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `cell.note.set` | `cell.notApplicable` | parameter | the element the caller names is not of a kind that takes notes |  |
| `cell.note.set` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `cell.range.add` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `cell.range.add` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `cell.range.add` | `cell.noTarget` | precheck | no cell selection and no current element (or a blank current cell) | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `cell.range.add` | `cell.notApplicable` | precheck | the target element is not of a kind the command edits | refuse: peg-current · pass: element |
| `cell.range.add` | `element.notCellBased` | unreachable | `applicable` admits only element kinds with a drawing bank, before `bank_of_element` |  |
| `cell.range.add` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `cell.range.delete` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `cell.range.delete` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `cell.range.delete` | `cell.noTarget` | precheck | no cell selection and no current element (or a blank current cell) | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `cell.range.delete` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `cell.range.move` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `cell.range.move` | `cell.clipboardEmpty` | parameter | the source cells the caller names hold a drawing without content to move |  |
| `cell.range.move` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `cell.range.move` | `cell.linkNotPossible` | parameter | the columns the caller names cannot be linked |  |
| `cell.range.move` | `cell.noTarget` | parameter | the target the caller names has no scene |  |
| `cell.range.move` | `cell.notApplicable` | parameter | a column the caller names is not of a kind the move edits |  |
| `cell.range.move` | `cell.typeMismatch` | parameter | the copied cells do not fit the target columns (the copied items) |  |
| `cell.range.move` | `drawing.locked` | parameter | a drawing of the columns the caller names is locked |  |
| `cell.range.move` | `element.notCellBased` | parameter | the target the caller names is not an Exposure Sheet column |  |
| `cell.range.move` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `cell.select` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `cell.select` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `cell.select` | `cell.noTarget` | parameter | the elements and frames the caller names select no cell |  |
| `cell.select` | `cell.notApplicable` | parameter | an element the caller names has no cells |  |
| `cell.select` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `colour.fillMapping.get` | `colour.notMappable` | parameter | the object the caller names has no texture mapping |  |
| `colour.fillMapping.get` | `colour.staleZoneId` | parameter | a zone ID the caller names no longer exists |  |
| `colour.fillMapping.get` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `colour.fillMapping.get` | `drawing.notFound` | parameter | a drawing the caller names does not exist (the current drawing always exists: the context is repaired) |  |
| `colour.fillMapping.get` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `colour.fillMapping.get` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `colour.fillMapping.set` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `colour.fillMapping.set` | `colour.clipboardEmpty` | precheck | the mapping `clipboard` without a copied mapping | untestable: a copied mapping needs a texture fill in a drawing, which the sweep's states do not build |
| `colour.fillMapping.set` | `colour.staleZoneId` | parameter | a zone ID the caller names no longer exists |  |
| `colour.fillMapping.set` | `drawing.locked` | precheck | the target drawing is locked | untestable: the command maps a texture fill, which the sweep's states do not build (a stroke refuses `colour.notMappable`) |
| `colour.fillMapping.set` | `drawing.noTarget` | precheck | no current drawing | untestable: the command maps a texture fill, which the sweep's states do not build (a stroke refuses `colour.notMappable`) |
| `colour.fillMapping.set` | `drawing.notFound` | parameter | a drawing the caller names does not exist (the current drawing always exists: the context is repaired) |  |
| `colour.fillMapping.set` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `colour.fillMapping.set` | `drawing.nothingToDo` | precheck | nothing selected in the drawing | untestable: the command maps a texture fill, which the sweep's states do not build (a stroke refuses `colour.notMappable`) |
| `colour.fillMapping.set` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `colour.fillMapping.set` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `colour.gap.close` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `colour.gap.close` | `colour.noGapFound` | parameter | no stroke ends near both ends of the drag the caller gives |  |
| `colour.gap.close` | `command.notImplemented` | parameter | view space of a view window that does not show the Drawing view: the caller's `space: "view"` or `view` (M4 C4.3 builds the other types) |  |
| `colour.gap.close` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `colour.gap.close` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `colour.gap.close` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `colour.gap.close` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `colour.missing.list` | `colour.notFound` | parameter | a colour the caller names is not in any palette |  |
| `colour.paint.apply` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `colour.paint.apply` | `colour.noCurrentColour` | precheck | the project has no colour to paint with | untestable: no implemented command removes palette colours, and a project is created with the default palette |
| `colour.paint.apply` | `colour.noTarget` | parameter | the given targets cover nothing to paint |  |
| `colour.paint.apply` | `colour.notFound` | parameter | a colour the caller names is not in any palette |  |
| `colour.paint.apply` | `colour.staleZoneId` | parameter | a zone ID the caller names no longer exists |  |
| `colour.paint.apply` | `command.notImplemented` | parameter | view space of a view window that does not show the Drawing view: the caller's `space: "view"` or `view` (M4 C4.3 builds the other types) |  |
| `colour.paint.apply` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `colour.paint.apply` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `colour.paint.apply` | `drawing.notFound` | parameter | a drawing the caller names does not exist (the current drawing always exists: the context is repaired) |  |
| `colour.paint.apply` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `colour.paint.apply` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `colour.paint.remove` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `colour.paint.remove` | `colour.noTarget` | parameter | the given targets cover nothing to paint |  |
| `colour.paint.remove` | `colour.staleZoneId` | parameter | a zone ID the caller names no longer exists |  |
| `colour.paint.remove` | `command.notImplemented` | parameter | view space of a view window that does not show the Drawing view: the caller's `space: "view"` or `view` (M4 C4.3 builds the other types) |  |
| `colour.paint.remove` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `colour.paint.remove` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `colour.paint.remove` | `drawing.notFound` | parameter | a drawing the caller names does not exist (the current drawing always exists: the context is repaired) |  |
| `colour.paint.remove` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `colour.paint.remove` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `colour.paintUnpainted.apply` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `colour.paintUnpainted.apply` | `colour.noCurrentColour` | precheck | the project has no colour to paint with | untestable: no implemented command removes palette colours, and a project is created with the default palette |
| `colour.paintUnpainted.apply` | `colour.noTarget` | parameter | the given targets cover nothing to paint |  |
| `colour.paintUnpainted.apply` | `colour.notFound` | parameter | a colour the caller names is not in any palette |  |
| `colour.paintUnpainted.apply` | `colour.staleZoneId` | parameter | a zone ID the caller names no longer exists |  |
| `colour.paintUnpainted.apply` | `command.notImplemented` | parameter | view space of a view window that does not show the Drawing view: the caller's `space: "view"` or `view` (M4 C4.3 builds the other types) |  |
| `colour.paintUnpainted.apply` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `colour.paintUnpainted.apply` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `colour.paintUnpainted.apply` | `drawing.notFound` | parameter | a drawing the caller names does not exist (the current drawing always exists: the context is repaired) |  |
| `colour.paintUnpainted.apply` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `colour.paintUnpainted.apply` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `colour.pick` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `colour.pick` | `command.notImplemented` | parameter | view space of a view window that does not show the Drawing view: the caller's `space: "view"` or `view` (M4 C4.3 builds the other types) |  |
| `colour.pick` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing {"point": {"x": 0, "y": 0}, "space": "drawing"} · pass: drawing {"point": {"x": 0, "y": 0}, "space": "drawing"} |
| `colour.pick` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `colour.recolour.apply` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `colour.recolour.apply` | `colour.notFound` | parameter | a colour the caller names is not in any palette |  |
| `colour.recolour.apply` | `colour.protected` | parameter | the colour the caller names is protected |  |
| `colour.recolour.apply` | `scene.noTarget` | precheck | no current scene | untestable: a project always has a current scene: it is created with one, and no implemented command deletes a scene or clears the current one |
| `colour.recovery.run` | `colour.notFound` | parameter | a colour the caller names is not in any palette |  |
| `colour.selection.apply` | `colour.noCurrentColour` | precheck | the project has no colour to paint with | untestable: no implemented command removes palette colours, and a project is created with the default palette |
| `colour.selection.apply` | `colour.notFound` | parameter | a colour the caller names is not in any palette |  |
| `colour.selection.apply` | `colour.protected` | parameter | the colour the caller names is protected |  |
| `colour.selection.apply` | `colour.staleZoneId` | parameter | a zone ID the caller names no longer exists |  |
| `colour.selection.apply` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `colour.selection.apply` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `colour.selection.apply` | `drawing.notFound` | parameter | a drawing the caller names does not exist (the current drawing always exists: the context is repaired) |  |
| `colour.selection.apply` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `colour.selection.apply` | `drawing.nothingToDo` | precheck | nothing selected in the drawing | refuse: drawing · pass: drawing-selected |
| `colour.selection.apply` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `colour.selection.apply` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `doc.asset.embed` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `doc.asset.export` | `doc.blobTooLarge` | parameter | the file the caller names is too large |  |
| `doc.asset.export` | `ref.notFound` | parameter | an object the caller names does not exist |  |
| `doc.asset.relink` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `doc.asset.relink` | `asset.relinkFailed` | parameter | the file the caller names cannot be relinked |  |
| `doc.asset.remove` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `doc.asset.remove` | `asset.inUse` | parameter | an asset the caller names is in use |  |
| `doc.asset.remove` | `ref.notFound` | parameter | an object the caller names does not exist |  |
| `doc.repair` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `doc.repair` | `asset.inUse` | parameter | an asset the caller names is in use |  |
| `doc.repair` | `ref.notFound` | parameter | an object the caller names does not exist |  |
| `doc.validate` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.arrange.apply` | `colour.staleZoneId` | parameter | a zone ID the caller names no longer exists |  |
| `draw.arrange.apply` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `draw.arrange.apply` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `draw.arrange.apply` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.arrange.apply` | `drawing.nothingToDo` | precheck | nothing selected in the drawing | refuse: drawing · pass: drawing-selected |
| `draw.arrange.apply` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.arrange.apply` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.breakApart.apply` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `draw.breakApart.apply` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `draw.breakApart.apply` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.breakApart.apply` | `drawing.nothingToDo` | precheck | nothing selected in the drawing | refuse: drawing · pass: text-selected |
| `draw.breakApart.apply` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.breakApart.apply` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.centreLine.apply` | `colour.staleZoneId` | parameter | a zone ID the caller names no longer exists |  |
| `draw.centreLine.apply` | `drawing.centreLineOnFill` | precheck | the selection holds painted zones but no brush strokes | refuse: drawing+painted-zone-selected · pass: drawing-selected |
| `draw.centreLine.apply` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `draw.centreLine.apply` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `draw.centreLine.apply` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.centreLine.apply` | `drawing.nothingToDo` | precheck | nothing selected in the drawing | refuse: drawing · pass: drawing-selected |
| `draw.centreLine.apply` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.centreLine.apply` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.content.add` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.content.add` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `draw.content.add` | `drawing.noTarget` | precheck | no current drawing | refuse: blank-cell {"createDrawing": false} · pass: blank-cell {} |
| `draw.content.add` | `drawing.notVectorElement` | precheck | the addressed element is not a drawing or annotation element | refuse: peg-current · pass: drawing |
| `draw.content.add` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.cut.apply` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.cut.apply` | `drawing.invalidGeometry` | parameter | the caller's geometry is degenerate or out of range |  |
| `draw.cut.apply` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing {"rect": {"x": -1000, "y": -1000, "width": 2000, "height": 2000}} · pass: drawing {"rect": {"x": -1000, "y": -1000, "width": 2000, "height": 2000}} |
| `draw.cut.apply` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing {"rect": {"x": -1000, "y": -1000, "width": 2000, "height": 2000}} · pass: drawing {"rect": {"x": -1000, "y": -1000, "width": 2000, "height": 2000}} |
| `draw.cut.apply` | `drawing.notFound` | parameter | a drawing the caller names does not exist (the current drawing always exists: the context is repaired) |  |
| `draw.cut.apply` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.cut.apply` | `drawing.nothingToDo` | parameter | the region the caller gives cuts nothing |  |
| `draw.cut.apply` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.drawing.rename` | `drawing.nameExists` | parameter | the name the caller gives is taken |  |
| `draw.drawing.rename` | `drawing.nameInvalid` | parameter | the name the caller gives is invalid |  |
| `draw.drawing.rename` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `draw.drawing.rename` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.drawing.render` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.drawing.render` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `draw.drawing.render` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.ellipse.add` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.ellipse.add` | `arg.missing` | parameter | a parameter the call needs is missing |  |
| `draw.ellipse.add` | `colour.notFound` | parameter | a colour the caller names is not in any palette |  |
| `draw.ellipse.add` | `drawing.invalidGeometry` | parameter | the caller's geometry is degenerate or out of range |  |
| `draw.ellipse.add` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `draw.ellipse.add` | `drawing.noTarget` | precheck | no current drawing | refuse: blank-cell {"createDrawing": false} · pass: blank-cell {} |
| `draw.ellipse.add` | `drawing.notVectorElement` | precheck | the addressed element is not a drawing or annotation element | refuse: peg-current · pass: drawing |
| `draw.ellipse.add` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.ellipse.add` | `pen.notFound` | parameter | the pen the caller names does not exist |  |
| `draw.ellipse.add` | `tool.valueOutOfRange` | parameter | a tool value is out of range |  |
| `draw.envelope.apply` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.envelope.apply` | `colour.staleZoneId` | parameter | a zone ID the caller names no longer exists |  |
| `draw.envelope.apply` | `drawing.invalidGeometry` | parameter | the caller's geometry is degenerate or out of range |  |
| `draw.envelope.apply` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `draw.envelope.apply` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `draw.envelope.apply` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.envelope.apply` | `drawing.nothingToDo` | precheck | nothing selected in the drawing | refuse: drawing · pass: drawing-selected |
| `draw.envelope.apply` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.envelope.apply` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.erase.apply` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.erase.apply` | `drawing.invalidGeometry` | parameter | the caller's geometry is degenerate or out of range |  |
| `draw.erase.apply` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing {"region": [{"x": -1000, "y": -1000}, {"x": 1000, "y": -1000}, {"x": 1000, "y": 1000}, {"x": -1000, "y": 1000}]} · pass: drawing {"region": [{"x": -1000, "y": -1000}, {"x": 1000, "y": -1000}, {"x": 1000, "y": 1000}, {"x": -1000, "y": 1000}]} |
| `draw.erase.apply` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing {"region": [{"x": -1000, "y": -1000}, {"x": 1000, "y": -1000}, {"x": 1000, "y": 1000}, {"x": -1000, "y": 1000}]} · pass: drawing {"region": [{"x": -1000, "y": -1000}, {"x": 1000, "y": -1000}, {"x": 1000, "y": 1000}, {"x": -1000, "y": 1000}]} |
| `draw.erase.apply` | `drawing.notFound` | parameter | a drawing the caller names does not exist (the current drawing always exists: the context is repaired) |  |
| `draw.erase.apply` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.erase.apply` | `drawing.nothingToDo` | parameter | the region or path the caller gives erases nothing |  |
| `draw.erase.apply` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.erase.apply` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.erase.apply` | `pen.notFound` | parameter | the pen the caller names does not exist |  |
| `draw.flatten.apply` | `colour.staleZoneId` | parameter | a zone ID the caller names no longer exists |  |
| `draw.flatten.apply` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `draw.flatten.apply` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `draw.flatten.apply` | `drawing.notFound` | parameter | a drawing the caller names does not exist (the current drawing always exists: the context is repaired) |  |
| `draw.flatten.apply` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.flatten.apply` | `drawing.nothingToDo` | precheck | nothing selected in the drawing | refuse: drawing · pass: drawing-selected |
| `draw.flatten.apply` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.flatten.apply` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.flip.apply` | `colour.staleZoneId` | parameter | a zone ID the caller names no longer exists |  |
| `draw.flip.apply` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `draw.flip.apply` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `draw.flip.apply` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.flip.apply` | `drawing.nothingToDo` | precheck | nothing selected in the drawing | refuse: drawing · pass: drawing-selected |
| `draw.flip.apply` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.flip.apply` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.group.create` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.group.create` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `draw.group.create` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `draw.group.create` | `drawing.notFound` | parameter | a drawing the caller names does not exist (the current drawing always exists: the context is repaired) |  |
| `draw.group.create` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.group.create` | `drawing.nothingToDo` | precheck | nothing selected in the drawing | refuse: drawing · pass: drawing-selected |
| `draw.group.create` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.group.create` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.group.delete` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.group.delete` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `draw.group.delete` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `draw.group.delete` | `drawing.notFound` | parameter | a drawing the caller names does not exist (the current drawing always exists: the context is repaired) |  |
| `draw.group.delete` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.group.delete` | `drawing.nothingToDo` | precheck | nothing selected in the drawing | refuse: drawing · pass: group-selected |
| `draw.group.delete` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.group.delete` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.line.add` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.line.add` | `arg.missing` | parameter | a parameter the call needs is missing |  |
| `draw.line.add` | `colour.notFound` | parameter | a colour the caller names is not in any palette |  |
| `draw.line.add` | `drawing.invalidGeometry` | parameter | the caller's geometry is degenerate or out of range |  |
| `draw.line.add` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing {"from": {"x": 0, "y": 0}, "to": {"x": 50, "y": 50}} · pass: drawing {"from": {"x": 0, "y": 0}, "to": {"x": 50, "y": 50}} |
| `draw.line.add` | `drawing.noTarget` | precheck | no current drawing | refuse: blank-cell {"from": {"x": 0, "y": 0}, "to": {"x": 50, "y": 50}, "createDrawing": false} · pass: blank-cell {"from": {"x": 0, "y": 0}, "to": {"x": 50, "y": 50}} |
| `draw.line.add` | `drawing.notVectorElement` | precheck | the addressed element is not a drawing or annotation element | refuse: peg-current {"from": {"x": 0, "y": 0}, "to": {"x": 50, "y": 50}} · pass: drawing {"from": {"x": 0, "y": 0}, "to": {"x": 50, "y": 50}} |
| `draw.line.add` | `drawing.nothingToDo` | precheck | nothing selected in the drawing | refuse: drawing {"to": {"x": 50, "y": 50}, "continuePrevious": true} · pass: drawing {"from": {"x": 0, "y": 0}, "to": {"x": 50, "y": 50}} |
| `draw.line.add` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.line.add` | `pen.notFound` | parameter | the pen the caller names does not exist |  |
| `draw.line.add` | `tool.valueOutOfRange` | parameter | a tool value is out of range |  |
| `draw.linesToBrush.apply` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.linesToBrush.apply` | `colour.staleZoneId` | parameter | a zone ID the caller names no longer exists |  |
| `draw.linesToBrush.apply` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `draw.linesToBrush.apply` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `draw.linesToBrush.apply` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.linesToBrush.apply` | `drawing.nothingToDo` | precheck | nothing selected in the drawing | refuse: drawing · pass: line-selected |
| `draw.linesToBrush.apply` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.linesToBrush.apply` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.linesToBrush.apply` | `pen.notFound` | parameter | the pen the caller names does not exist |  |
| `draw.linesToBrush.apply` | `tool.valueOutOfRange` | parameter | a tool value is out of range |  |
| `draw.lock.set` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `draw.lock.set` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.object.delete` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `draw.object.delete` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `draw.object.delete` | `drawing.notFound` | parameter | a drawing the caller names does not exist (the current drawing always exists: the context is repaired) |  |
| `draw.object.delete` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.object.delete` | `drawing.nothingToDo` | precheck | nothing selected in the drawing | refuse: drawing · pass: drawing-selected |
| `draw.object.delete` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.object.delete` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.optimize.apply` | `colour.staleZoneId` | parameter | a zone ID the caller names no longer exists |  |
| `draw.optimize.apply` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `draw.optimize.apply` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `draw.optimize.apply` | `drawing.notFound` | parameter | a drawing the caller names does not exist (the current drawing always exists: the context is repaired) |  |
| `draw.optimize.apply` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.optimize.apply` | `drawing.nothingToDo` | precheck | nothing selected in the drawing | refuse: drawing · pass: drawing-selected |
| `draw.optimize.apply` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.optimize.apply` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.pivot.set` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `draw.pivot.set` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `draw.pivot.set` | `drawing.notFound` | parameter | a drawing the caller names does not exist (the current drawing always exists: the context is repaired) |  |
| `draw.pivot.set` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.pivot.set` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.point.add` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.point.add` | `drawing.invalidGeometry` | parameter | the caller's geometry is degenerate or out of range |  |
| `draw.point.add` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-stroke {"strokeId": "$stroke", "at": {"x": 50, "y": 50}} · pass: stroke {"strokeId": "$stroke", "at": {"x": 50, "y": 50}} |
| `draw.point.add` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing {"strokeId": "s_local", "at": {"x": 50, "y": 50}} · pass: stroke {"strokeId": "$stroke", "at": {"x": 50, "y": 50}} |
| `draw.point.add` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.point.add` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.point.add` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.point.delete` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `draw.point.delete` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `draw.point.delete` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.point.delete` | `drawing.nothingToDo` | precheck | nothing selected in the drawing | refuse: drawing · pass: anchors-selected |
| `draw.point.delete` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.point.delete` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.point.move` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `draw.point.move` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `draw.point.move` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.point.move` | `drawing.nothingToDo` | precheck | nothing selected in the drawing | refuse: drawing · pass: anchors-selected |
| `draw.point.move` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.point.move` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.point.set` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-stroke {"strokeId": "$stroke"} · pass: stroke {"strokeId": "$stroke"} |
| `draw.point.set` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing {"strokeId": "s_local"} · pass: stroke {"strokeId": "$stroke"} |
| `draw.point.set` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.point.set` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.point.set` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.pointSelection.set` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `draw.pointSelection.set` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.pointSelection.set` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.polyline.add` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.polyline.add` | `arg.missing` | parameter | a parameter the call needs is missing |  |
| `draw.polyline.add` | `colour.notFound` | parameter | a colour the caller names is not in any palette |  |
| `draw.polyline.add` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `draw.polyline.add` | `drawing.noTarget` | precheck | no current drawing | refuse: blank-cell {"createDrawing": false} · pass: blank-cell {} |
| `draw.polyline.add` | `drawing.notVectorElement` | precheck | the addressed element is not a drawing or annotation element | refuse: peg-current · pass: drawing |
| `draw.polyline.add` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.polyline.add` | `pen.notFound` | parameter | the pen the caller names does not exist |  |
| `draw.polyline.add` | `tool.valueOutOfRange` | parameter | a tool value is out of range |  |
| `draw.rectangle.add` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.rectangle.add` | `arg.missing` | parameter | a parameter the call needs is missing |  |
| `draw.rectangle.add` | `colour.notFound` | parameter | a colour the caller names is not in any palette |  |
| `draw.rectangle.add` | `drawing.invalidGeometry` | parameter | the caller's geometry is degenerate or out of range |  |
| `draw.rectangle.add` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `draw.rectangle.add` | `drawing.noTarget` | precheck | no current drawing | refuse: blank-cell {"createDrawing": false} · pass: blank-cell {} |
| `draw.rectangle.add` | `drawing.notVectorElement` | precheck | the addressed element is not a drawing or annotation element | refuse: peg-current · pass: drawing |
| `draw.rectangle.add` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.rectangle.add` | `pen.notFound` | parameter | the pen the caller names does not exist |  |
| `draw.rectangle.add` | `tool.valueOutOfRange` | parameter | a tool value is out of range |  |
| `draw.repositionAll.apply` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.repositionAll.apply` | `drawing.invalidGeometry` | parameter | the caller's geometry is degenerate or out of range |  |
| `draw.repositionAll.apply` | `drawing.noTarget` | precheck | no current drawing | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `draw.repositionAll.apply` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.repositionAll.apply` | `drawing.nothingToDo` | precheck | nothing selected in the drawing | refuse: empty-drawing-element · pass: drawing |
| `draw.rotate.apply` | `colour.staleZoneId` | parameter | a zone ID the caller names no longer exists |  |
| `draw.rotate.apply` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `draw.rotate.apply` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `draw.rotate.apply` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.rotate.apply` | `drawing.nothingToDo` | precheck | nothing selected in the drawing | refuse: drawing · pass: drawing-selected |
| `draw.rotate.apply` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.rotate.apply` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.segment.move` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.segment.move` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-stroke {"strokeId": "$stroke"} · pass: stroke {"strokeId": "$stroke"} |
| `draw.segment.move` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing {"strokeId": "s_local"} · pass: stroke {"strokeId": "$stroke"} |
| `draw.segment.move` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.segment.move` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.segment.move` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.selection.set` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.selection.set` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing {"by": "all"} · pass: drawing {"by": "all"} |
| `draw.selection.set` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.selection.set` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.smooth.apply` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.smooth.apply` | `colour.staleZoneId` | parameter | a zone ID the caller names no longer exists |  |
| `draw.smooth.apply` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `draw.smooth.apply` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `draw.smooth.apply` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.smooth.apply` | `drawing.nothingToDo` | precheck | nothing selected in the drawing | refuse: drawing · pass: drawing-selected |
| `draw.smooth.apply` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.smooth.apply` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.stroke.add` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.stroke.add` | `arg.missing` | parameter | a parameter the call needs is missing |  |
| `draw.stroke.add` | `colour.notFound` | parameter | a colour the caller names is not in any palette |  |
| `draw.stroke.add` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing {"points": [{"x": 0, "y": 0}, {"x": 100, "y": 100}]} · pass: drawing {"points": [{"x": 0, "y": 0}, {"x": 100, "y": 100}]} |
| `draw.stroke.add` | `drawing.noTarget` | precheck | no current drawing | refuse: blank-cell {"points": [{"x": 0, "y": 0}, {"x": 100, "y": 100}], "createDrawing": false} · pass: blank-cell {"points": [{"x": 0, "y": 0}, {"x": 100, "y": 100}]} |
| `draw.stroke.add` | `drawing.notVectorElement` | precheck | the addressed element is not a drawing or annotation element | refuse: peg-current {"points": [{"x": 0, "y": 0}, {"x": 100, "y": 100}]} · pass: drawing {"points": [{"x": 0, "y": 0}, {"x": 100, "y": 100}]} |
| `draw.stroke.add` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.stroke.add` | `pen.notFound` | parameter | the pen the caller names does not exist |  |
| `draw.stroke.add` | `tool.valueOutOfRange` | parameter | a tool value is out of range |  |
| `draw.stroke.set` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.stroke.set` | `arg.missing` | parameter | a parameter the call needs is missing |  |
| `draw.stroke.set` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-stroke {"strokeId": "$stroke", "thickness": 3} · pass: stroke {"strokeId": "$stroke", "thickness": 3} |
| `draw.stroke.set` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing {"strokeId": "s_local", "thickness": 3} · pass: stroke {"strokeId": "$stroke", "thickness": 3} |
| `draw.stroke.set` | `drawing.notFound` | parameter | a drawing the caller names does not exist (the current drawing always exists: the context is repaired) |  |
| `draw.stroke.set` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.stroke.set` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.stroke.set` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.stroke.set` | `pen.notFound` | parameter | the pen the caller names does not exist |  |
| `draw.stroke.set` | `tool.valueOutOfRange` | parameter | a tool value is out of range |  |
| `draw.text.add` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.text.add` | `colour.notFound` | parameter | a colour the caller names is not in any palette |  |
| `draw.text.add` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `draw.text.add` | `drawing.noTarget` | precheck | no current drawing | refuse: blank-cell {"createDrawing": false} · pass: blank-cell {} |
| `draw.text.add` | `drawing.notVectorElement` | precheck | the addressed element is not a drawing or annotation element | refuse: peg-current · pass: drawing |
| `draw.text.add` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.text.set` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.text.set` | `arg.missing` | parameter | a parameter the call needs is missing |  |
| `draw.text.set` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-text {"textId": "$text", "text": "B"} · pass: text {"textId": "$text", "text": "B"} |
| `draw.text.set` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing {"textId": "n_local", "text": "B"} · pass: text {"textId": "$text", "text": "B"} |
| `draw.text.set` | `drawing.notFound` | parameter | a drawing the caller names does not exist (the current drawing always exists: the context is repaired) |  |
| `draw.text.set` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.text.set` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.text.set` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.textFormat.set` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.textFormat.set` | `arg.missing` | parameter | a parameter the call needs is missing |  |
| `draw.textFormat.set` | `colour.notFound` | parameter | a colour the caller names is not in any palette |  |
| `draw.textFormat.set` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-text {"textId": "$text", "start": 0, "size": 20} · pass: text {"textId": "$text", "start": 0, "size": 20} |
| `draw.textFormat.set` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing {"textId": "n_local", "start": 0, "size": 20} · pass: text {"textId": "$text", "start": 0, "size": 20} |
| `draw.textFormat.set` | `drawing.notFound` | parameter | a drawing the caller names does not exist (the current drawing always exists: the context is repaired) |  |
| `draw.textFormat.set` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.textFormat.set` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.textFormat.set` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.transform.apply` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.transform.apply` | `colour.staleZoneId` | parameter | a zone ID the caller names no longer exists |  |
| `draw.transform.apply` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `draw.transform.apply` | `drawing.noTarget` | precheck | no current drawing | refuse: no-drawing · pass: drawing |
| `draw.transform.apply` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.transform.apply` | `drawing.nothingToDo` | precheck | nothing selected in the drawing | refuse: drawing · pass: drawing-selected |
| `draw.transform.apply` | `drawing.objectNotFound` | parameter | an object ID the caller names is not in the drawing (selections are repaired to existing objects) |  |
| `draw.transform.apply` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.vectorize.preview` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.vectorize.preview` | `vectorize.thresholdOutOfRange` | parameter | the threshold is out of range |  |
| `draw.vectorize.preview` | `vectorize.unsupportedFile` | parameter | the file the caller names is not a supported bitmap |  |
| `draw.vectorize.run` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `draw.vectorize.run` | `drawing.locked` | execution | refused by the job when it commits, after the call returned its job (`commit_image`) |  |
| `draw.vectorize.run` | `drawing.noTarget` | execution | refused by the job when it commits, after the call returned its job (`commit_image`) |  |
| `draw.vectorize.run` | `drawing.notVectorElement` | parameter | the element the caller names is not a drawing or annotation element |  |
| `draw.vectorize.run` | `drawing.nothingToDo` | parameter | the given files or image element yield no paths |  |
| `draw.vectorize.run` | `drawing.staleRevision` | parameter | the caller's `expectRev` is older than the drawing |  |
| `draw.vectorize.run` | `vectorize.thresholdOutOfRange` | parameter | the threshold is out of range |  |
| `draw.vectorize.run` | `vectorize.unsupportedFile` | parameter | the file the caller names is not a supported bitmap |  |
| `drawAid.background.set` | `arg.invalid` | parameter | the colour the caller gives is not `#RRGGBB` or `#RRGGBBAA` |  |
| `drawAid.onionSkin.set` | `tool.valueOutOfRange` | parameter | an onion-skin count the caller gives is outside 0-3 |  |
| `drawAid.onionSkinStyle.set` | `arg.invalid` | parameter | a colour the caller gives is not `#RRGGBB` or `#RRGGBBAA` |  |
| `drawAid.staticLightTable.add` | `drawing.noTarget` | precheck | no current drawing (the current element's cell at the frame is empty) | refuse: no-drawing · pass: drawing |
| `drawAid.staticLightTable.add` | `drawing.notFound` | parameter | the drawing the caller names is not used by any element |  |
| `drawAid.staticLightTable.add` | `element.notFound` | parameter | the element the caller names does not exist or has no scene |  |
| `drawAid.staticLightTable.list` | `command.disabled` | precheck | no current scene | untestable: a project always has a current scene: it is created with one, and no implemented command deletes a scene or clears the current one |
| `drawAid.staticLightTable.remove` | `arg.invalid` | parameter | neither `entryIds` nor `all` is given |  |
| `drawAid.staticLightTable.remove` | `command.disabled` | precheck | no current scene | untestable: a project always has a current scene: it is created with one, and no implemented command deletes a scene or clears the current one |
| `drawAid.staticLightTable.set` | `arg.invalid` | parameter | neither `entryIds`, `all` nor `onlyVisibleEntryId` is given |  |
| `drawAid.staticLightTable.set` | `command.disabled` | precheck | no current scene | untestable: a project always has a current scene: it is created with one, and no implemented command deletes a scene or clears the current one |
| `edit.copy` | `command.disabled` | precheck | nothing selected, or no current scene | refuse: element · pass: element-selected |
| `edit.copy` | `edit.clipboardInvalid` | execution | a consistency check of content copied from a valid document |  |
| `edit.copy` | `edit.wrongElementType` | parameter | a cell the caller names in `objectIds` belongs to an element without exposure; the selection setter refuses such cells (`ref.wrongType`) and an element's type never changes, so the selection cannot hold one |  |
| `edit.cut` | `anim.lastCamera` | precheck | the scene's only camera would go | refuse: camera-selected · pass: element-selected |
| `edit.cut` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `edit.cut` | `command.disabled` | precheck | nothing selected, or no current scene | refuse: element · pass: element-selected |
| `edit.cut` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing-selected {"context": "drawing"} · pass: drawing-selected {"context": "drawing"} |
| `edit.cut` | `edit.clipboardInvalid` | execution | a consistency check of content copied from a valid document |  |
| `edit.cut` | `edit.wrongElementType` | parameter | a cell the caller names in `objectIds` belongs to an element without exposure; the selection setter refuses such cells (`ref.wrongType`) and an element's type never changes, so the selection cannot hold one |  |
| `edit.cut` | `element.locked` | precheck | a selected element is locked | refuse: locked-element-selected · pass: element-selected |
| `edit.cut` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `edit.cutSpecial` | `anim.lastCamera` | precheck | the scene's only camera would go | refuse: camera-selected · pass: element-selected |
| `edit.cutSpecial` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `edit.cutSpecial` | `command.disabled` | precheck | nothing selected, or no current scene | refuse: element · pass: element-selected |
| `edit.cutSpecial` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing-selected {"context": "drawing"} · pass: drawing-selected {"context": "drawing"} |
| `edit.cutSpecial` | `edit.clipboardInvalid` | execution | a consistency check of content copied from a valid document |  |
| `edit.cutSpecial` | `edit.wrongElementType` | parameter | a cell the caller names in `objectIds` belongs to an element without exposure; the selection setter refuses such cells (`ref.wrongType`) and an element's type never changes, so the selection cannot hold one |  |
| `edit.cutSpecial` | `element.locked` | precheck | a selected element is locked | refuse: locked-element-selected · pass: element-selected |
| `edit.cutSpecial` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `edit.delete` | `anim.lastCamera` | precheck | the scene's only camera would go | refuse: camera-selected · pass: element-selected |
| `edit.delete` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `edit.delete` | `command.disabled` | precheck | nothing selected, or no current scene | refuse: element · pass: element-selected |
| `edit.delete` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing-selected {"context": "drawing"} · pass: drawing-selected {"context": "drawing"} |
| `edit.delete` | `edit.wrongElementType` | parameter | a cell the caller names in `objectIds` belongs to an element without exposure; the selection setter refuses such cells (`ref.wrongType`) and an element's type never changes, so the selection cannot hold one |  |
| `edit.delete` | `element.locked` | precheck | a selected element is locked | refuse: locked-element-selected · pass: element-selected |
| `edit.delete` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `edit.history.move` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `edit.paste` | `cell.clipboardEmpty` | parameter | clipboard content an agent set (`edit.clipboard.set`) without a copied drawing; a copy always carries its drawings |  |
| `edit.paste` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `edit.paste` | `cell.typeMismatch` | parameter | the copied cells do not fit the target columns (the copied items) |  |
| `edit.paste` | `command.disabled` | precheck | nothing selected, or no current scene | untestable: a project always has a current scene: it is created with one, and no implemented command deletes a scene or clears the current one |
| `edit.paste` | `command.notImplemented` | precheck | an image on the clipboard | untestable: an image clipboard comes from the system clipboard, which no command fills (image import: M10) |
| `edit.paste` | `drawing.locked` | precheck | the target drawing is locked | refuse: clipboard-objects+locked-drawing {"context": "drawing"} · pass: clipboard-objects {"context": "drawing"} |
| `edit.paste` | `edit.clipboardEmpty` | precheck | the clipboard is empty | refuse: clipboard-empty {"context": "xsheet"} · pass: clipboard-cells {"context": "xsheet"} |
| `edit.paste` | `edit.pasteNotAllowed` | precheck | the clipboard's kind does not paste here | refuse: clipboard-cells · pass: clipboard-cells {"context": "xsheet"} |
| `edit.paste` | `edit.wrongElementType` | precheck | the target or a selected element has no exposure | refuse: clipboard-cells+peg-current {"context": "xsheet"} · pass: clipboard-cells {"context": "xsheet"} |
| `edit.paste` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `edit.pasteAsNewElement` | `command.disabled` | precheck | nothing selected, or no current scene | untestable: a project always has a current scene: it is created with one, and no implemented command deletes a scene or clears the current one |
| `edit.pasteAsNewElement` | `edit.clipboardEmpty` | precheck | the clipboard is empty | refuse: clipboard-empty · pass: clipboard-cells |
| `edit.pasteAsNewElement` | `edit.pasteNotAllowed` | precheck | the clipboard's kind does not paste here | refuse: clipboard-elements · pass: clipboard-cells |
| `edit.pasteSpecial` | `cell.clipboardEmpty` | parameter | clipboard content an agent set (`edit.clipboard.set`) without a copied drawing; a copy always carries its drawings |  |
| `edit.pasteSpecial` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `edit.pasteSpecial` | `cell.typeMismatch` | parameter | the copied cells do not fit the target columns (the copied items) |  |
| `edit.pasteSpecial` | `command.disabled` | precheck | nothing selected, or no current scene | untestable: a project always has a current scene: it is created with one, and no implemented command deletes a scene or clears the current one |
| `edit.pasteSpecial` | `command.notImplemented` | precheck | an image on the clipboard | untestable: an image clipboard comes from the system clipboard, which no command fills (image import: M10) |
| `edit.pasteSpecial` | `drawing.locked` | precheck | the target drawing is locked | refuse: clipboard-objects+locked-drawing {"context": "drawing"} · pass: clipboard-objects {"context": "drawing"} |
| `edit.pasteSpecial` | `edit.clipboardEmpty` | precheck | the clipboard is empty | refuse: clipboard-empty {"context": "xsheet"} · pass: clipboard-cells {"context": "xsheet"} |
| `edit.pasteSpecial` | `edit.noPreviousPasteSpecial` | precheck | no earlier paste special in the session | refuse: clipboard-cells {"context": "xsheet", "useLast": true} · pass: clipboard-cells {"context": "xsheet"} |
| `edit.pasteSpecial` | `edit.pasteNotAllowed` | precheck | the clipboard's kind does not paste here | refuse: clipboard-cells · pass: clipboard-cells {"context": "xsheet"} |
| `edit.pasteSpecial` | `edit.wrongElementType` | precheck | the target or a selected element has no exposure | refuse: clipboard-cells+peg-current {"context": "xsheet"} · pass: clipboard-cells {"context": "xsheet"} |
| `edit.pasteSpecial` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `edit.undo` | `undo.notTop` | parameter | the step the caller names (`ifTop`) is not the most recent one |  |
| `element.annotation.set` | `element.notDrawing` | parameter | the element the caller names is not a drawing element |  |
| `element.annotation.set` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `element.childExposure.apply` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `element.childExposure.apply` | `cell.notApplicable` | parameter | the element the caller names is not a peg |  |
| `element.childExposure.apply` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `element.clone` | `element.nameDuplicate` | parameter | the name the caller gives is taken |  |
| `element.clone` | `element.notCloneable` | parameter | the element the caller names cannot be cloned |  |
| `element.clone` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `element.collapsed.set` | `element.notFound` | precheck | no element given, selected or current | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `element.colour.set` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `element.colour.set` | `element.notFound` | precheck | no element given, selected or current | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `element.create` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `element.create` | `command.disabled` | precheck | nothing selected, or no current scene | untestable: a project always has a current scene: it is created with one, and no implemented command deletes a scene or clears the current one |
| `element.create` | `element.countOutOfRange` | parameter | the count is out of range |  |
| `element.create` | `element.nameDuplicate` | parameter | the name the caller gives is taken |  |
| `element.create` | `element.nameTooLong` | parameter | the name is too long |  |
| `element.create` | `element.notCellBased` | parameter | a cell parameter given for an element type without cells |  |
| `element.create` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `element.create` | `element.notParentable` | parameter | the parent the caller names cannot take children |  |
| `element.current.set` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `element.delete` | `anim.lastCamera` | precheck | the scene's only camera would go | refuse: camera-selected · pass: element-selected |
| `element.delete` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `element.delete` | `element.notFound` | precheck | no element given, selected or current | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `element.drawing.create` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `element.drawing.create` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `element.drawing.create` | `cell.notApplicable` | parameter | the element the caller names is not a drawing element |  |
| `element.drawing.create` | `drawing.nameDuplicate` | parameter | the name the caller gives is taken |  |
| `element.drawing.create` | `element.notCellBased` | unreachable | `applicable` admits only element kinds with a drawing bank, before `bank_of_element` |  |
| `element.drawing.create` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `element.drawing.delete` | `drawing.inUse` | parameter | a drawing the caller names is exposed in a cell |  |
| `element.drawing.delete` | `drawing.notFound` | parameter | a drawing the caller names does not exist (the current drawing always exists: the context is repaired) |  |
| `element.drawing.delete` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `element.drawing.duplicate` | `drawing.nameDuplicate` | parameter | the name the caller gives is taken |  |
| `element.drawing.duplicate` | `drawing.notFound` | parameter | a drawing the caller names does not exist (the current drawing always exists: the context is repaired) |  |
| `element.drawing.rename` | `drawing.nameDuplicate` | parameter | the name the caller gives is taken |  |
| `element.drawing.rename` | `drawing.notFound` | parameter | a drawing the caller names does not exist (the current drawing always exists: the context is repaired) |  |
| `element.drawingContent.delete` | `cell.noTarget` | precheck | no cell selection and no current element (or a blank current cell) | refuse: blank-cell · pass: element |
| `element.drawingContent.delete` | `cell.notApplicable` | precheck | the target element is not of a kind the command edits | untestable: the current drawing of an image element comes from an import the sweep's states do not make |
| `element.drawingContent.delete` | `drawing.locked` | precheck | the target drawing is locked | refuse: locked-drawing · pass: drawing |
| `element.drawingContent.delete` | `drawing.notFound` | parameter | a drawing the caller names does not exist (the current drawing always exists: the context is repaired) |  |
| `element.drawingContent.delete` | `element.notFound` | unreachable | `bank_users` lists existing elements |  |
| `element.drawingLock.set` | `cell.noTarget` | precheck | no cell selection and no current element (or a blank current cell) | refuse: blank-cell · pass: element |
| `element.drawingLock.set` | `cell.notApplicable` | precheck | the target element is not of a kind the command edits | untestable: the current drawing of an image element comes from an import the sweep's states do not make |
| `element.drawingLock.set` | `drawing.notFound` | parameter | a drawing the caller names does not exist (the current drawing always exists: the context is repaired) |  |
| `element.drawingLock.set` | `element.notFound` | unreachable | `bank_users` lists existing elements |  |
| `element.duplicate` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `element.duplicate` | `element.notFound` | precheck | no element given, selected or current | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `element.hierarchy.select` | `command.disabled` | parameter | the relative the caller asks for does not exist (a root has no parent) |  |
| `element.hierarchy.select` | `element.notFound` | precheck | no element given, selected or current | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `element.lock.set` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `element.lock.set` | `command.disabled` | precheck | nothing selected, or no current scene | untestable: a project always has a current scene: it is created with one, and no implemented command deletes a scene or clears the current one |
| `element.lock.set` | `element.notFound` | precheck | no element given, selected or current | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `element.mediaLength.set` | `cell.notApplicable` | parameter | the element the caller names is not a media element |  |
| `element.mediaLength.set` | `command.disabled` | deferred M8 C8.3 | without `length`, the natural length is the linked source's, read by the media decoder of M8 C8.3 |  |
| `element.mediaLength.set` | `drawing.fileMissing` | parameter | the media file of the element the caller names is missing |  |
| `element.mediaLength.set` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `element.note.set` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `element.order.move` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `element.order.move` | `element.notCellBased` | precheck | the target element has no cells | refuse: peg-current {"order": "xsheet", "to": "front"} · pass: element {"order": "xsheet", "to": "front"} |
| `element.order.move` | `element.notFound` | precheck | no element given, selected or current | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `element.order.set` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `element.order.set` | `command.disabled` | precheck | nothing selected, or no current scene | untestable: a project always has a current scene: it is created with one, and no implemented command deletes a scene or clears the current one |
| `element.order.set` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `element.parent.remove` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `element.parent.remove` | `element.notFound` | precheck | no element given, selected or current | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `element.parent.set` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `element.parent.set` | `element.hierarchyCycle` | parameter | the parent the caller names would make a cycle |  |
| `element.parent.set` | `element.notFound` | precheck | no element given, selected or current | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `element.parent.set` | `element.notParentable` | parameter | the parent the caller names cannot take children |  |
| `element.parentPeg.create` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `element.parentPeg.create` | `element.nameDuplicate` | parameter | the name the caller gives is taken |  |
| `element.parentPeg.create` | `element.notFound` | precheck | no element given, selected or current | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `element.properties.set` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `element.rename` | `element.nameDuplicate` | parameter | the name the caller gives is taken |  |
| `element.rename` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `element.select` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `element.solo.set` | `element.notFound` | precheck | no element given, selected or current | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `element.timing.set` | `anim.notFound` | unreachable | `data_of` reads targets `targets` resolved (they exist) |  |
| `element.timing.set` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `element.timing.set` | `cell.frameOutOfRange` | parameter | a frame value is outside 1-100000 |  |
| `element.timing.set` | `cell.notApplicable` | precheck | the target element is not of a kind the command edits | refuse: element · pass: peg-current |
| `element.timing.set` | `element.notFound` | precheck | no element given, selected or current | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `element.timing.set` | `element.notLoopable` | parameter | a loop given for an element that cannot loop |  |
| `element.typeColour.set` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `element.visibility.set` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `element.visibility.set` | `command.disabled` | precheck | nothing selected, or no current scene | untestable: a project always has a current scene: it is created with one, and no implemented command deletes a scene or clears the current one |
| `element.visibility.set` | `element.notCellBased` | precheck | the target element has no cells | refuse: peg-current {"scope": "xsheet", "visible": false} · pass: element {"scope": "xsheet", "visible": false} |
| `element.visibility.set` | `element.notFound` | precheck | no element given, selected or current | untestable: a project always has a current element: a new project makes its first element current, another becomes current when it goes, and a scene's last camera cannot be deleted |
| `job.cancel` | `job.notFound` | parameter | the job the caller names does not exist |  |
| `keymap.export` | `fs.exists` | parameter | the file the caller names exists |  |
| `keymap.get` | `arg.missing` | parameter | a parameter the call needs is missing |  |
| `keymap.import` | `fs.notFound` | parameter | the file the caller names does not exist |  |
| `keymap.import` | `keymap.invalidFile` | parameter | the file the caller names is not a keymap |  |
| `keymap.reset` | `arg.missing` | parameter | a parameter the call needs is missing |  |
| `prefs.reset` | `arg.missing` | parameter | a parameter the call needs is missing |  |
| `prefs.set` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `project.backup.list` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `project.backup.list` | `project.none` | precheck | no project is open | refuse: no-project · pass: drawing |
| `project.backup.open` | `project.backupNotFound` | parameter | the backup the caller names does not exist |  |
| `project.backup.open` | `project.none` | unreachable | `resolve_unsaved` returns at once without a project; nothing else reads it |  |
| `project.create` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `project.create` | `name.empty` | parameter | the name the caller gives is empty |  |
| `project.create` | `name.invalidCharacter` | parameter | the name the caller gives has an invalid character |  |
| `project.create` | `name.tooLong` | parameter | the name the caller gives is too long |  |
| `project.create` | `project.none` | unreachable | `resolve_unsaved` returns at once without a project; nothing else reads it |  |
| `project.open` | `project.none` | unreachable | `resolve_unsaved` returns at once without a project; nothing else reads it |  |
| `project.recent.remove` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `project.recovery.delete` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `project.recovery.delete` | `project.recoveryInUse` | parameter | the recovery entry the caller names belongs to a running engine |  |
| `project.recovery.delete` | `project.recoveryNotFound` | parameter | the recovery entry the caller names does not exist |  |
| `project.recovery.open` | `project.none` | unreachable | `resolve_unsaved` returns at once without a project; nothing else reads it |  |
| `project.recovery.open` | `project.recoveryInUse` | parameter | the recovery entry the caller names belongs to a running engine |  |
| `project.recovery.open` | `project.recoveryNotFound` | parameter | the recovery entry the caller names does not exist |  |
| `project.saveAs` | `project.pathInsideProject` | parameter | the path the caller gives is inside the project |  |
| `render.frame.run` | `anim.notFound` | parameter | the camera the caller names does not exist |  |
| `render.frame.run` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `render.frame.run` | `command.notImplemented` | unreachable | `active_view` runs only while the active view window shows the Drawing view, the window `drawing_view(None)` then returns |  |
| `render.frame.run` | `drawing.noTarget` | precheck | `composition: "activeView"` with the Drawing view active and no current drawing | refuse: no-drawing+drawing-view {"composition": "activeView"} · pass: drawing+drawing-view {"composition": "activeView"} |
| `render.frame.run` | `export.fileExists` | parameter | the output file the caller names exists |  |
| `render.frame.run` | `render.compositionUnavailable` | precheck | `xsheet` (M5 C5.1), or `activeView` while the active view window is not the Drawing view (M4 C4.3) | refuse: drawing {"composition": "activeView"} · pass: drawing+drawing-view {"composition": "activeView"} |
| `render.frame.run` | `render.emptyDrawing` | parameter | the drawing the caller names is empty |  |
| `render.frame.run` | `render.frameInvalid` | parameter | a frame value is outside 1-100000 |  |
| `render.frame.run` | `render.inlineTooLarge` | parameter | the inline image the caller asks for is too large |  |
| `render.frame.run` | `render.noScene` | precheck | the project has no scene | untestable: no implemented command deletes a scene, and a project is created with one |
| `render.range.run` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `render.range.run` | `export.fileExists` | parameter | the output file the caller names exists |  |
| `render.range.run` | `render.compositionUnavailable` | parameter | the composition the caller names arrives later |  |
| `render.range.run` | `render.noScene` | precheck | the project has no scene | untestable: no implemented command deletes a scene, and a project is created with one |
| `tool.active.set` | `arg.invalid` | parameter | the tool id the caller gives is not a tool |  |
| `tool.brush.options.set` | `pen.notFound` | parameter | the pen the caller names does not exist |  |
| `tool.brush.options.set` | `tool.notApplicable` | parameter | the pen the caller names is an eraser pen |  |
| `tool.eraser.options.set` | `pen.notFound` | parameter | the pen the caller names does not exist |  |
| `tool.eraser.options.set` | `tool.notApplicable` | parameter | the pen the caller names is a brush pen |  |
| `tool.options.get` | `arg.invalid` | parameter | the tool id the caller gives is not a tool |  |
| `tool.pen.create` | `arg.invalid` | parameter | the name the caller gives is empty or longer than 128 characters, or a setting is unknown |  |
| `tool.pen.create` | `pen.nameExists` | parameter | the name the caller gives is taken |  |
| `tool.pen.create` | `pen.notFound` | parameter | the `fromPenId` the caller names does not exist |  |
| `tool.pen.create` | `tool.notApplicable` | parameter | the `fromPenId` the caller names is of the other family |  |
| `tool.pen.delete` | `pen.lastPen` | parameter | the pen the caller names is the last of its family (02 §6.1) |  |
| `tool.pen.delete` | `pen.notFound` | parameter | the pen the caller names does not exist |  |
| `tool.pen.rename` | `arg.invalid` | parameter | the name the caller gives is empty or longer than 128 characters |  |
| `tool.pen.rename` | `pen.nameExists` | parameter | the name the caller gives is taken by another pen |  |
| `tool.pen.rename` | `pen.notFound` | parameter | the pen the caller names does not exist |  |
| `tool.pen.select` | `pen.notFound` | parameter | the pen the caller names does not exist |  |
| `tool.pen.select` | `tool.notApplicable` | unreachable | `tool.pen.select` passes no family, so `select_pen` never compares one |  |
| `tool.pen.set` | `pen.notFound` | parameter | the pen the caller names does not exist |  |
| `tool.symmetry.set` | `tool.valueOutOfRange` | parameter | a tool value is out of range |  |
| `tool.tablet.set` | `arg.invalid` | parameter | the input interface name the caller gives is empty or longer than 64 characters |  |
| `ui.help.get` | `command.unknown` | parameter | the command the caller names does not exist |  |
| `ui.help.get` | `ui.unknownMessage` | parameter | the message the caller names does not exist |  |
| `ui.help.get` | `window.panelNotFound` | parameter | the panel the caller names does not exist |  |
| `ui.preferences.open` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `ui.tips.set` | `ui.unknownMessage` | parameter | the message the caller names does not exist |  |
| `view.coordinates.convert` | `anim.notFound` | parameter | scene space (the caller's `from` or `to`) evaluates the current element, which the evaluator does not know |  |
| `view.coordinates.convert` | `anim.valueOutOfRange` | parameter | scene space (the caller's `from` or `to`) evaluates a transform out of range |  |
| `view.coordinates.convert` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `view.pan.set` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `view.pan.set` | `arg.missing` | parameter | none of `centre`, `by` and `atPointer` is given |  |
| `view.rotation.set` | `arg.invalid` | parameter | both `degrees` and `steps` are given, or a value is not finite |  |
| `view.rotation.set` | `arg.missing` | parameter | neither `degrees` nor `steps` is given |  |
| `view.zoom.set` | `arg.invalid` | parameter | both `zoom` and `steps` are given, or a value is not finite |  |
| `view.zoom.set` | `arg.missing` | parameter | neither `zoom` nor `steps` is given |  |
| `window.toolbar.items.set` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `workspace.export` | `fs.exists` | parameter | the file the caller names exists |  |
| `workspace.import` | `fs.notFound` | parameter | the file the caller names does not exist |  |
| `workspace.import` | `workspace.invalidFile` | parameter | the file the caller names is not a workspace |  |
| `xsheet.exposures.set` | `arg.invalid` | parameter | a parameter value or combination is invalid |  |
| `xsheet.exposures.set` | `cell.drawingNotFound` | parameter | the drawing name the caller gives is not in the column's bank |  |
| `xsheet.exposures.set` | `command.disabled` | precheck | nothing selected, or no current scene | untestable: a project always has a current scene: it is created with one, and no implemented command deletes a scene or clears the current one |
| `xsheet.exposures.set` | `drawing.notFound` | parameter | a drawing the caller names does not exist (the current drawing always exists: the context is repaired) |  |
| `xsheet.exposures.set` | `element.notFound` | parameter | an element the caller names does not exist (selected and current elements always exist: the context is repaired) |  |
| `xsheet.exposures.set` | `xsheet.columnReadOnly` | parameter | the column the caller names is read-only |  |
| `xsheet.exposures.set` | `xsheet.revisionMismatch` | parameter | the caller's revision is older than the column |  |
| `xsheet.exposures.set` | `xsheet.runInvalid` | parameter | a run the caller gives is invalid |  |
| `xsheet.exposures.set` | `xsheet.runOutOfRange` | parameter | a run the caller gives is out of range |  |
| `xsheet.exposures.set` | `xsheet.runOverlap` | parameter | runs the caller gives overlap |  |
