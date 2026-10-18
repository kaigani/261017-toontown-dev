# 11 — Agent Interface (MCP)

**Scope.** This section specifies how an AI agent controls the application completely, without a human operating the
GUI, through a **Model Context Protocol (MCP)** server built into the application. It defines the architecture that
makes full control possible (a single command layer shared by the GUI and the agent), the MCP surface (tools,
resources, prompts, notifications), object addressing, coordinate spaces, safety and permissions, undo integration,
long-running jobs, visual verification, error handling, and conformance tests.

The area-specific commands live in the **"Agent interface"** section at the end of each of sections 01–10. All
commands are compiled in **`A_command_registry.md`**. This section owns the namespace `agent` and the generic MCP
tools and the namespaces `agent`, `inspect`, `checkpoint` and `job`; it does not redefine area commands.

---

## 1. Goals and principles

| # | Principle | Requirement |
|---|---|---|
| P1 | **Full parity** | Every capability a user can reach through the GUI (menu item, toolbar button, tool gesture, dialog field, context-menu entry, keyboard shortcut, drag-and-drop, panel action, preference) MUST be reachable through a registered command. Everything a user can *see* (document content, current state, rendered output, warnings) MUST be readable through queries, resources or renders. A GUI feature without a command is a defect. |
| P2 | **One command layer** | The GUI and the MCP server are both clients of the same command registry and document model. The GUI MUST NOT mutate the document by any path that bypasses the registry. This is what guarantees P1. |
| P3 | **Structured first, gestural second** | Every gestural operation (drawing, painting, dragging, transforming) MUST have a structured equivalent that takes explicit data (points, ids, numbers). Raw input simulation (§7.6) also exists for tool-faithful reproduction and UI testing, but no task SHOULD require it. |
| P4 | **Explicit targets** | Commands that act on "the current selection / current element / current frame" in the GUI MUST also accept explicit target parameters. When targets are omitted the current UI context is used, and the result MUST report what was actually targeted. |
| P5 | **Never block** | No command invoked by an agent may open a modal dialog, file picker, colour picker or confirmation box, or wait for user input. Choices become parameters; confirmations become a structured protocol (§6.3). |
| P6 | **Verifiable** | After any change an agent MUST be able to verify the outcome: each mutating call returns the list of changed objects and their new revisions; queries return complete JSON; renders return images; `describe_frame` returns a structured description of what is visible. |
| P7 | **Deterministic** | Given the same document and the same command sequence, the resulting document and rendered pixels MUST be identical. Randomness (particles, jitter, auto-naming) MUST be seeded and recorded in the document. |
| P8 | **Safe and attributable** | Agent sessions run under explicit permission scopes. Destructive, file-overwriting and outward-facing (network) actions require confirmation according to policy. Every agent action is logged, attributed in undo history, and can be undone. The user can pause or disconnect an agent at any time. |
| P9 | **Discoverable and economical** | An agent with no prior knowledge MUST be able to discover all commands, their schemas, examples and the object model from the server itself. Responses MUST be compact by default (projections, pagination, summaries), because agents work with limited context. |

---

## 2. Architecture

```
            ┌──────────── GUI ─────────────┐      ┌──────── MCP server ────────┐
            │ menus · tools · dialogs ·    │      │ stdio / Streamable HTTP     │
            │ shortcuts · panels           │      │ tools · resources · prompts │
            └──────────────┬───────────────┘      └─────────────┬──────────────┘
                           │  command invocations (id + JSON params)  │
                           ▼                                          ▼
             ┌───────────────────────── Command registry ─────────────────────────┐
             │ id · kind · JSON Schema (params, result) · enablement rule ·        │
             │ undo label · permission scope · examples                            │
             └───────────────────────────────┬─────────────────────────────────────┘
                                             ▼
             ┌──────────────── Serialized command executor (document thread) ─────┐
             │ validate → check enablement → check permission/confirmation →       │
             │ execute as transaction → record undo step → emit change events      │
             └───────────────┬───────────────────────────────┬─────────────────────┘
                             ▼                               ▼
                 Document model (section 10)          Job runner (renders, exports,
                 + revision counters                   imports, lip-sync, vectorize)
                             │
                             ▼
                 Renderer (interactive and offscreen — same pipeline)
```

- **Command registry.** Each command declares: `id`, `kind` (`action`, `toggle`, `query`, `job`, `ui`, `event`),
  human label and description, parameter JSON Schema, result JSON Schema, enablement rule (with a human-readable
  reason when disabled), undo label, permission scope(s) (§6.1), whether confirmation may be required, and at least
  one example invocation. Registration happens at startup; plug-ins MAY register further commands under their
  own namespace.
- **GUI binding.** Menus, toolbars, context menus, shortcuts and dialogs are bound to command ids. A dialog collects
  parameters and invokes the command once on OK; a tool gesture builds the parameter set of a structured command
  (e.g. a brush gesture produces a `draw.stroke.add` call with the sampled points and pressures). This binding is
  what makes P1 testable (§14).
- **Executor.** All document mutations from all clients go through one serialized queue on the document thread.
  GUI input and agent calls are interleaved at command granularity; a command or batch is never interleaved with
  another mutation.
- **Renderer.** Interactive views and offscreen renders (agent `render_frame`, previews, export) MUST share the same
  scene evaluation and rasterisation rules, so an agent's verification render matches what the user sees and what
  export produces (differences allowed only for explicitly requested quality settings).

---

## 3. Run modes

| Mode | How started | Behaviour |
|---|---|---|
| **Attached** | GUI running; the user enables the agent server in Preferences › Agent access, or launches with `--mcp-http` | The agent controls the live session the user is looking at. GUI updates live as the agent works. A persistent **agent indicator** in the main window shows that an agent is connected and whether it is acting; it offers **Pause**, **Stop** and **Activity…** (§11). |
| **Headless** | `app --headless --mcp-stdio [--project <path>]` | No window. The full engine (document, commands, renderer, jobs) runs offscreen. Views exist as **virtual views** (with a size, zoom, pan and display toggles) so that view-dependent commands, `pointer` input, and `screenshot` still work. Intended for agent pipelines, CI, batch production. |
| **Batch script** | `app --headless --run <batch.json> [--project <path>] [--out <log.jsonl>]` | Executes a JSON file with the same structure as the `batch` tool (§7.3) without an MCP client, then exits with status 0 (all steps ok) or non-zero. Lets an agent-authored procedure be replayed in CI. |

Requirements:
- One application instance holds at most one open project (consistent with section 01). In attached mode the agent
  shares that project with the user. In headless mode an agent MAY ask to open, close, or switch projects.
- Several MCP clients MAY connect to one instance (HTTP transport). They share the document and the single
  executor; each has its own session id, permission grant, subscriptions and audit trail.
- Starting in headless mode MUST NOT require a display server or GPU; a software rasteriser fallback MUST produce
  pixel-identical output to the GPU path for the offscreen renderer, or the offscreen renderer is always the software one.
  **Design decision:** recommended — offscreen renders always use the deterministic software rasteriser.

---

## 4. Transport, connection and protocol

- **Protocol.** The server implements MCP (target: the current stable protocol revision at implementation time; minimum
  the 2025-06-18 revision). It MUST support the server features **tools** (with `inputSchema`, `outputSchema`,
  `structuredContent` and tool annotations), **resources** (including resource templates and subscriptions),
  **prompts**, **logging**, **completions** (argument completion for ids and names), progress notifications and
  cancellation. It SHOULD use the client features **elicitation** (confirmations, §6.3) and **roots** (file-system
  scope, §6.2) when the client offers them, and MUST work correctly when the client offers neither.
- **Transports.**
  - **stdio** for headless mode (the app is launched by the client).
  - **Streamable HTTP** for attached mode, bound to `127.0.0.1` only by default, on a configurable port (default:
    a fixed, documented port; fall back to the next free port and report it in the agent indicator and in a
    discovery file in the per-user settings folder).
  - The HTTP endpoint MUST validate the `Origin` header, reject non-local origins, and require a bearer token.
    **Design decision:** recommended — a random per-installation token, shown and regenerable in Preferences ›
    Agent access and written to the discovery file readable only by the user. Remote (non-loopback) binding
    MUST be off by default and, if enabled, MUST require the standard MCP authorization flow.
- **Off by default.** The agent server is disabled until the user enables it (attached mode) or the app is started
  with an explicit flag (headless). Enabling it is a preference (section 01 `prefs` key `agent.server.enabled`).
- **Server info.** The MCP `initialize` result reports the application name and version, the **command-registry
  version** (semver; minor = additive, major = breaking), the **document schema version** (section 10), and the
  run mode. `instructions` in the initialize result contain a short orientation for agents (how to discover
  commands, address objects, verify work), which MUST match the `app://docs/agent-guide` resource.
- **Change of tool list.** If the tool list changes (plug-in loaded, typed-tool mode toggled), the server emits the
  MCP tools-list-changed notification.

---

## 5. Addressing, units and coordinate spaces

### 5.1 Object identity

- Every document object (project, scene, element, drawing, layer, stroke, zone, palette, style, swatch, peg key,
  motion point, function, camera, bone, effect, sound placement, template, transition, …) has a **stable opaque id**
  (string, unique in the project, preserved by save/load, never reused). Section 10 defines id rules and object types.
- Wherever an id is accepted, a **path** is also accepted:
  `scene:"Intro"/element:"Bob"/drawing:"3"`, `palette:"Main"/swatch:"Skin light"`, `library:global/template:"Walk"`.
  Path segments are `type:"name"` or `type:#index` (1-based) or `type:@id`. Names are matched exactly, then
  case-insensitively; an ambiguous match is an error (`ref.ambiguous`) listing the candidates.
- Special references: `@current` (current scene / element / frame / drawing as appropriate), `@selection`,
  `@activeView`, `@activeCamera`, `$<n>.<field>` (result of step *n* inside a `batch`, §7.3).
- Each object carries a revision number `rev` that increments on every change (section 10 defines `rev` and the
  document-wide `documentRev`). Mutating commands accept `expectRev` for optimistic concurrency (error `rev.conflict`
  if the object changed since the agent read it). In `run_command` it is a map `{objectId: rev}`; area commands that
  declare `expectRev?: int` apply it to their primary target (e.g. the drawing for `draw.*` and `colour.paint.*`, the
  scene for `xsheet.exposures.set`).

### 5.2 Units and conventions

| Quantity | Convention |
|---|---|
| Frames | 1-based integers; ranges are inclusive `{from, to}` |
| Time | Frames are authoritative; commands that accept seconds or timecode (`hh:mm:ss:ff`) convert at the project frame rate and report the frame used |
| Colours | `#RRGGBB` or `#RRGGBBAA` (sRGB, straight alpha); document colours are referenced by **colour id** (section 03), not by value |
| Angles | Degrees, counter-clockwise positive (as seen in Camera view) — section 05 is normative |
| Booleans | JSON `true/false`; toggles are set explicitly (`.set {enabled}`), never flipped blindly |
| Strings | UTF-8; names are trimmed; length limits per section 10 |

### 5.3 Coordinate spaces

Every command or tool taking coordinates states its space; tools that accept several take a `space` parameter.

| Space | Definition | Normative section |
|---|---|---|
| `drawing` | Local space of a drawing, independent of pegs and camera: origin at the drawing centre, y up, 1 field = 100 drawing units | 02 |
| `scene` | 3-D staging space of a scene (east-west, north-south, front-back axes) in fields (1 field = 1 inch; the default camera frame is 12 fields wide) | 05 |
| `camera` | The active camera's frame: x, y in field units from the camera centre, plus `normalized` variant 0..1 from top-left | 05 |
| `view` | Pixels of a named view (`drawingView`, `cameraView`, `topView`, `sideView`, …) with origin top-left, y down, at the view's current zoom/pan | 01 |
| `image` | Pixels of an image returned by `render_frame` / `screenshot` (origin top-left); results include the transform from `image` to `camera`/`scene` so an agent can act on what it sees | 09, 11 |

The server MUST provide `convert_coordinates {from, to, points, context}` (§7.5) so that agents never have to
reimplement the transform chain.

---

## 6. Permissions, confirmation and safety

### 6.1 Scopes

Each MCP session holds a set of granted scopes. Every command declares the scopes it needs.

| Scope | Allows |
|---|---|
| `read` | Queries, resources, renders, `describe_frame`, `inspect_ui`, screenshots |
| `edit` | All undoable document changes |
| `ui` | Changing UI state: views, panels, workspaces, current frame/element, active tool, playback |
| `input` | Raw input simulation (`pointer`, `key`, `type_text`) |
| `files.read` | Reading files from disk (open project, import) |
| `files.write` | Writing files (save, save as, export, render to file); overwriting is additionally confirmable |
| `prefs` | Changing user preferences and key bindings (persisted beyond the project) |
| `library.global` | Changing the user-wide library |
| `network` | Any outward action: publishing/uploading, fetching remote media |
| `print` | Printing to a physical printer (printing to a file needs only `files.write`) |
| `app` | Quit, close project with discard, change agent-server settings (never grantable to an agent by itself) |

**Policy presets** (chosen by the user per connection in the connection prompt, or as a default in Preferences ›
Agent access):

| Preset | Scopes | Confirmation |
|---|---|---|
| Observe | `read` | — |
| Assist (default) | all except `network`, `print`, `prefs`, `app` | Confirm destructive and overwrite actions |
| Autonomous | all except `app`; `network` and `print` optional | Confirm only `network`/`print` actions and discarding unsaved work |

A command outside the granted scopes fails with `permission.denied` naming the missing scope. Granting more scope
is a user action in the GUI (or a launch flag in headless mode: `--grant read,edit,files.write`).

### 6.2 File-system boundaries

- File paths are absolute or relative to the **project folder**. In headless mode, the allowed roots are the project
  folder, explicitly granted folders (`--allow-path`), and the MCP client's roots if offered. In attached mode the
  user's grant dialog lists the allowed roots.
- Any read or write outside the allowed roots fails with `fs.outsideRoots`. Symbolic links are resolved before the check.
- Writes never silently overwrite: every writing command has an `onExists` (file targets) or `onConflict` (file or
  name targets) parameter with values from `"fail" | "overwrite" | "rename"` (some commands add `"replace"` or
  `"skip"`; `"ask"` means "return `needs_confirmation`"), default `"fail"`; `"overwrite"` is a confirmable action
  under Assist.
- Media can also be passed **inline** (base64 with a MIME type, size-limited, default limit 50 MB) or as an MCP
  resource URI, so an agent without file access can still import an image or sound.

### 6.3 Confirmation protocol

Commands declare which conditions require confirmation (deleting a scene, deleting unused drawings, discarding
unsaved changes, overwriting a file, uploading). Under the session's policy:

1. If the client supports **elicitation**, the server sends an elicitation request summarising the action
   (what will be lost or written, object counts, paths) and waits for accept/decline. Decline → error
   `confirmation.declined`.
2. Otherwise the command returns, without side effects, the result `{status: "needs_confirmation", confirmToken,
   summary, consequences[]}`. The agent re-invokes the same command with `confirmToken` (valid 5 minutes, single
   use, bound to the same parameters and document revision) to proceed. **In attached mode** the user may instead be
   shown the confirmation in the GUI (preference: *Ask me in the app when an agent needs confirmation*); the
   agent receives the user's decision.
3. Commands accept `confirm: true` only when the session policy pre-authorises that category.

Warnings that the GUI shows as informational message boxes (including "don't show again" tips) are never shown for
agent calls; they are returned in the result's `warnings[]` with codes (§10).

### 6.4 User control and coexistence (attached mode)

- The **agent indicator** shows: connected client name(s), current activity (last command label), and buttons
  **Pause** (agent calls queue and return `agent.paused` after a timeout), **Stop** (disconnect the session) and
  **Activity…** (opens the activity log, §11).
- **User priority.** While the user is mid-gesture (pointer button held, text field being edited, modal dialog open),
  agent mutations wait (up to `waitForUserMs`, default 10 000) then fail with `ui.busy`. An agent MAY query
  `get_state.ui.userBusy`.
- If a modal dialog opened by the *user* is up, agent `ui`-kind commands that would change focus fail with
  `ui.modalOpen`; document commands that do not conflict still run.
- Agent-originated changes are visually indicated (brief highlight of the changed objects/cells, optional preference).
- The agent MAY move the user's current frame, element, selection and view only with the `ui` scope; a preference
  *Let the agent move my view* (default on) controls whether agent `ui` commands affect the user's visible views
  or only virtual views owned by the agent session (see §7.4 `viewId`).

### 6.5 Rate and resource limits

Per session: maximum concurrent jobs (default 2), maximum batch length (default 1 000 steps), maximum inline image
size returned (default 2048 px on the long side and 8 MB encoded), maximum inline payload accepted (default 50 MB). Exceeding a limit returns
`limit.exceeded` with the limit's name and value. Limits are configurable in Preferences › Agent access.

---

## 7. MCP tools

The server exposes a **small, generic tool set** (this section). All area functionality is reached through
`run_command` / `batch` using the ids in `A_command_registry.md`. Optionally (Preferences › Agent access ›
*Expose commands as individual tools*, off by default) the server additionally exposes each command, or each
command in selected namespaces, as its own MCP tool named `<namespace>_<object>_<verb>` with the command's schemas,
for clients that work better with typed tools.

Every tool result carries `structuredContent` (JSON matching the tool's `outputSchema`) and a short text summary
in `content`; image-producing tools add an image content item. Tool annotations (`readOnlyHint`, `destructiveHint`,
`idempotentHint`, `openWorldHint`) MUST be set accurately.

### 7.1 Discovery and state

| Tool | Input | Output | Notes |
|---|---|---|---|
| `get_state` | `include?: ["project","scene","frame","selection","tool","views","playback","jobs","ui","agent"]` | Current project (id, path, dirty, rev), current scene, frame, element, drawing, active view and its type, working mode (drawing / staging), active tool and its options, selection (by kind, ids), playback state, running jobs, open dialogs, `ui.userBusy`, session scopes and policy | Read-only. The first call an agent should make. |
| `list_commands` | `namespace?; filter? (substring or glob); kind?; enabledOnly?: bool = false; includeSchema?: bool = false; page?` | `[{id, kind, label, summary, enabled, disabledReason?, scopes[], undoable}]` | `enabled` evaluated against the current context. |
| `describe_command` | `id` | Full declaration: description, parameter and result JSON Schemas, defaults, enablement rule text, scopes, confirmation conditions, undo label, examples, related commands, equivalent GUI locations | |
| `get_docs` | `topic?: "agent-guide" \| "object-model" \| "coordinates" \| "errors" \| <namespace>` | Markdown | Same content as the `app://docs/*` resources. |

### 7.2 Commands

| Tool | Input | Output |
|---|---|---|
| `run_command` | `id; params?: object; dryRun?: bool = false; expectRev?: {objectId: rev}; confirmToken?; requestId?` | **Command result envelope** (below) |

**Command result envelope** (all mutating calls, including `batch` and `set_properties`):

```json
{
  "status": "ok" | "needs_confirmation" | "error",
  "result": { ... command-specific, per its result schema ... },
  "targets": { "sceneId": "...", "elementIds": ["..."], "frame": 12 },
  "changes": [ { "id": "...", "type": "stroke", "op": "created" | "modified" | "deleted", "rev": 4 } ],
  "undo": { "stepId": "u-381", "label": "Add Stroke" },
  "warnings": [ { "code": "colour.unknownId", "message": "...", "objectIds": ["..."] } ],
  "documentRev": 1042,
  "error": { "code": "...", "message": "...", "details": { }, "hint": "..." }
}
```

- `dryRun: true` validates, checks enablement and permissions, and returns the `changes` that *would* be made
  (best-effort for operations whose outcome depends on geometry computation; such commands return
  `"dryRun": "approximate"`), without modifying anything or creating an undo step.
- `requestId` makes a call **idempotent**: a repeated `requestId` within the session returns the original result
  instead of executing again (protects against client retries).
- `changes` is capped (default 500 entries, then `changesTruncated: true` and a `changesSince` revision to query
  with `doc.diff`).

### 7.3 Batch (transactions)

| Tool | Input | Output |
|---|---|---|
| `batch` | `label: string; steps: [{id, params, as?: string}]; atomic?: bool = true; stopOnError?: bool = true; dryRun?: bool` | Envelope with `steps: [envelope…]` |

- With `atomic: true` all steps form **one transaction and one undo step** named `label`; if any step fails, the
  whole batch is rolled back and the failing step index is reported.
- Steps may reference earlier results: `"$2.result.strokeId"` or a named step `"$walk.result.elementId"` (`as`).
- Query steps are allowed inside a batch and see the uncommitted state of earlier steps.
- `job` commands are not allowed inside an atomic batch (error `batch.jobNotAllowed`).

### 7.4 Reading the document

| Tool | Input | Output |
|---|---|---|
| `get_object` | `ref (id or path); depth?: int = 1; fields?: [string]; page?` | The object as JSON (section 10 schema), children to `depth` (children beyond depth as `{id, type, name}` stubs) |
| `find_objects` | `type?; within?: ref; name?: string \| glob; where?: {field: value \| {op, value}}; limit?: int = 100; page?` | `[{id, type, name, path}]` |
| `set_properties` | `targets: [ref]; props: object; expectRev?` | Envelope. Generic validated patch for any writable property listed in the type's schema; one undo step |
| `doc_diff` | `sinceRev? \| sinceCheckpoint?` | Changes (created / modified / deleted with JSON-Patch style field changes) |

These are thin wrappers over the `doc.*` commands of section 10 and return identical data.

### 7.5 Seeing the result

| Tool | Input | Output |
|---|---|---|
| `render_frame` | `scene?: ref = @current; frame?: int = @current; camera?: ref = @activeCamera; width?: int = 960; height?: int; background?: "transparent" \| "sceneColour" \| colour; include?: [elementRef]; exclude?: [elementRef]; effects?: bool = true; overlays?: ["onionSkin","guides","fieldGuide","selection","pegs","motionPaths","safeArea"]; quality?: "draft" \| "final"; output?: "inline" \| path` | PNG image content + `{imageToCamera transform, width, height, frame, sha256, renderMs}` |
| `render_drawing` | `drawing: ref; width?; background?; showStrokes?: bool; showZoneIds?: bool` | PNG of one drawing in drawing space, optionally with invisible strokes and zone-id labels drawn, + `imageToDrawing` transform |
| `render_contact_sheet` | `scene?; frames: [int] \| {from,to,step}; columns?: int; cellWidth?: int = 240; labels?: bool = true` | One PNG grid of frames — lets an agent review motion in a single image |
| `screenshot` | `target?: "window" \| viewName \| panelName \| dialog; region?: {x,y,w,h}` | PNG of the actual GUI (attached mode) or of a virtual view (headless) |
| `describe_frame` | `scene?; frame?; camera?; detail?: "summary" \| "full"` | Structured description: visible elements in draw order with exposed drawing, world transform, bounding box in camera space (normalized and field units), opacity/visibility, active effects, audible sounds with levels, current mouth shape, out-of-frame elements, warnings (e.g. element exposed but empty) |
| `hit_test` | `space; point; scene?; frame?; view?` | Objects under the point, top-most first: element, drawing, layer, stroke/zone ids, colour id |
| `convert_coordinates` | `from; to; points: [{x,y,z?}]; scene?; frame?; element?; view?` | Converted points |
| `inspect_ui` | `target?: "window" \| viewName \| panelName \| "dialogs"; depth?` | UI tree: panels (open/visible/docked), views (zoom, pan, display toggles), widgets with role, label, value, enabled state and the **command id they invoke**; open dialogs with their fields |

- Renders are **deterministic** (P7). `quality: "final"` uses the final-quality renderer (all effects, antialiasing;
  ignores display-only switches) and matches export. `quality: "draft"` reproduces what the interactive views show,
  honouring display-only settings such as the all-effects switch and display quality (06); it MUST also be stable.
- `render_frame` honours visibility settings of the scene unless `include`/`exclude` override them. It never
  changes the user's views or the current frame.
- Images returned inline are limited by the session's maximum image size; larger renders must go to a file.

### 7.6 Simulated input

For tool-faithful reproduction, UI testing, and rare cases where the agent prefers to "use the tool".

| Tool | Input | Output |
|---|---|---|
| `pointer` | `view: viewName \| viewId; space?: "view" \| "drawing" \| "scene" \| "camera" = "view"; pointerType?: "mouse" \| "pen" \| "penEraser" = "mouse"; events: [{type: "down"\|"move"\|"up"\|"hover"\|"wheel"\|"doubleClick", x, y, pressure?: 0..1 = 1, tiltX?, tiltY?, button?: "left"\|"right"\|"middle", modifiers?: ["shift","alt","ctrl","meta"], dt?: ms, wheelDelta?}]; tool?: toolId` | Envelope of the command(s) the gesture produced (e.g. the `draw.stroke.add` it resulted in), plus the final hover info |
| `key` | `keys: string (e.g. "ctrl+z", "shift+F5"); view?` | Envelope of the command the key invoked (or `{handled: false}`) |
| `type_text` | `text; target?: widget path from inspect_ui` | Envelope |

Requirements:
- `tool` selects the tool first (same as the corresponding `tool.*.select` command).
- Input is delivered through the same path as real input, including pressure/tilt handling, smoothing, snapping and
  modifier semantics, so the outcome is identical to a human gesture with the same samples. `dt` drives timing-
  sensitive behaviour (e.g. motion recording) deterministically — simulated time, not wall-clock time.
- **Equivalence rule:** for each gestural tool the registry documents its structured equivalent; a conformance test
  verifies that a recorded gesture replayed through `pointer` and the structured command produce identical documents.
- In headless mode, `pointer` targets virtual views; the agent can create them with `ui.view.create {type, width,
  height}` (section 01) and set zoom/pan by command.

### 7.7 History, checkpoints and jobs

| Tool | Input | Output |
|---|---|---|
| `undo` | `steps?: int = 1; ifTop?: stepId` | Envelope. `ifTop` makes the undo conditional on the given step still being the most recent one (protects the user's own later edits); otherwise error `undo.notTop` |
| `redo` | `steps?: int = 1` | Envelope |
| `history` | `limit?: int = 20` | `[{stepId, label, origin: "user" \| "agent:<client>", time, objectCount}]`, top first |
| `checkpoint` | `action: "create" \| "restore" \| "list" \| "delete"; name?` | Named document snapshot independent of the undo stack. `restore` is itself one undoable step ("Restore checkpoint <name>") |
| `job_status` | `jobId?` (all when omitted) | `[{jobId, commandId, state: "queued"\|"running"\|"succeeded"\|"failed"\|"cancelled", progress 0..1, message, startedAt, result?, error?}]` |
| `job_wait` | `jobId; timeoutMs?: int = 60000` | Final job record, or current record with `timedOut: true` |
| `job_cancel` | `jobId` | Final job record |

### 7.8 Generic tools and area commands

Several generic tools are convenience wrappers over registered area commands and MUST return the same data:

| Tool | Wraps | Notes |
|---|---|---|
| `get_state` | `app.state.get` (01) | |
| `get_object` / `find_objects` / `set_properties` / `doc_diff` | `doc.object.get` / `doc.object.find` / `doc.object.patch` / `doc.diff` (10) | |
| `render_frame` | `render.frame.run` (09) with `composition: "camera"` | Inline/file rules of 09 §11 and the session limits of §6.5 |
| `render_drawing` | `draw.drawing.render` (02) | |
| `undo` / `redo` / `history` | `edit.undo` / `edit.redo` / `edit.history.get` (01) | |
| `inspect_ui` / `screenshot` | `inspect.ui.tree` / `inspect.screenshot` (§15.2) | |
| `describe_frame` / `hit_test` / `convert_coordinates` / `render_contact_sheet` | `inspect.frame.describe` / `inspect.hitTest` / `inspect.coordinates.convert` / `inspect.contactSheet.render` (§15.2) | |
| `checkpoint` | `checkpoint.create` / `.restore` / `.list` / `.delete` (§15.2) | |
| `job_status` / `job_wait` / `job_cancel` | `job.list` / `job.wait` / `job.cancel` (§15.2) | |
| `pointer` | The active tool of the target view; produces the structured command listed for that tool in its section's gesture-equivalence table (02, 03, 05) | |

The generic tools exist so that an agent can work with a dozen tools; everything they do is also reachable with
`run_command`.

---

## 8. Commands and kinds (semantics)

- **Kinds.** `action` — undoable document change; `toggle` — boolean state set explicitly; `query` — read-only;
  `job` — long-running, returns `{jobId}` immediately; `ui` — changes UI/view state only (not undoable, not marking
  the document dirty); `event` — documented notification, not callable.
- **Validation.** Parameters are validated against the JSON Schema before execution: unknown parameters are an error
  (`arg.unknown`), missing required parameters `arg.missing`, type/range violations `arg.invalid` with the JSON
  pointer of the offending field and the allowed range.
- **Enablement.** The same rule that greys out a menu item is evaluated for agent calls; a disabled command returns
  `command.disabled` with the reason (e.g. "no drawing exposed at frame 12 of element Bob").
- **Selection-based commands.** Each GUI command that works on the selection also accepts explicit targets
  (`targets`, `elementIds`, `frames`, `objectIds` …, per command). Agents SHOULD pass explicit targets. When the
  command uses the selection, `targets` in the envelope lists it.
- **Settings vs. document.** Tool options and drawing aids are per-user settings (not undoable); document properties
  are undoable. Each command's declaration states which.
- **Defaults.** Omitted optional parameters take the same default the GUI dialog would show initially (including
  "last used" values where the GUI remembers them). The envelope's `result.effectiveParams` lists the values
  actually used, so agent runs are reproducible.
- **Auto-naming.** Where the GUI auto-names (new elements, drawings, scenes), commands accept an explicit `name`,
  and results always return the name assigned.

---

## 9. Resources, subscriptions and events

### 9.1 Resources

| URI (template) | Content |
|---|---|
| `app://state` | Same as `get_state` (JSON) |
| `app://project` | Project object with scene list (JSON) |
| `app://object/{id}` | Any object as JSON (section 10 schema), depth 1 |
| `app://scene/{sceneId}/xsheet{?from,to}` | Run-length exposure grid (section 04 format) |
| `app://drawing/{drawingId}` | Full vector content (section 02 format) |
| `app://drawing/{drawingId}.png{?w}` | Rendered drawing |
| `app://scene/{sceneId}/frame/{frame}.png{?w,camera}` | Rendered frame |
| `app://palette/{paletteId}` | Palette with styles and swatches |
| `app://library/{libraryId}` | Library tree (templates with metadata and thumbnail URIs) |
| `app://command/{commandId}` | Command declaration (same as `describe_command`) |
| `app://schema/{type}` | JSON Schema of an object type |
| `app://docs/agent-guide`, `app://docs/object-model`, `app://docs/coordinates`, `app://docs/errors`, `app://docs/{namespace}` | Agent documentation generated from the registry and schemas (Markdown) |
| `app://events` | Event stream resource (§9.2) |
| `app://activity` | This session's audit log (JSONL) |

Resources that change (state, project, objects, xsheet, drawings, palettes, events) support **subscription**; the
server sends resource-updated notifications, coalesced to at most one per resource per 100 ms.

### 9.2 Events

Events are delivered as `app://events` updates (read the resource with `?after=<eventSeq>` to fetch new events)
and, if the client enabled logging at level `info`, mirrored as log notifications. Each event:
`{seq, time, type, origin: "user" | "agent:<client>" | "system", payload}`.

| Event type | Payload |
|---|---|
| `doc.changed` | `{undoStepId, label, changes[]}` (same shape as the envelope) |
| `doc.undone` / `doc.redone` | `{undoStepId, label}` |
| `project.opened` / `project.closed` / `project.saved` | `{projectId, path}` |
| `state.frameChanged` / `state.sceneChanged` / `state.elementChanged` | `{sceneId, frame, elementId}` |
| `state.selectionChanged` | `{kind, ids[]}` |
| `state.toolChanged` | `{toolId}` |
| `playback.started` / `playback.stopped` | `{sceneId, frame}` |
| `job.progress` / `job.finished` | Job record |
| `ui.dialogOpened` / `ui.dialogClosed` | `{dialogId, commandId}` |
| `agent.paused` / `agent.resumed` | `{by: "user"}` |
| Area events | As defined in each section's Agent interface (e.g. `draw.drawing.changed`) |

An agent working alongside a user SHOULD subscribe to `doc.changed` with `origin: user` to notice and respect the
user's concurrent edits.

---

## 10. Errors and warnings

Errors use stable dotted codes `area.reason`. The generic codes are:

| Code | Meaning |
|---|---|
| `command.unknown` | No such command id |
| `command.disabled` | Command not enabled in the current context (`details.reason`) |
| `arg.missing` / `arg.unknown` / `arg.invalid` | Parameter validation failure (`details.pointer`, `details.allowed`) |
| `ref.notFound` / `ref.ambiguous` / `ref.wrongType` | Object reference problems (`details.candidates`) |
| `rev.conflict` | `expectRev` mismatch (`details.currentRev`) |
| `project.none` | Command needs an open project |
| `project.unsavedChanges` | Closing/opening would discard changes; pass `unsaved: "save" \| "discard"` |
| `permission.denied` | Missing scope (`details.scope`) |
| `confirmation.required` / `confirmation.declined` / `confirmation.expired` | Confirmation protocol (§6.3) |
| `fs.outsideRoots` / `fs.invalidPath` / `fs.exists` / `fs.notFound` / `fs.unreadable` / `fs.unsupportedFormat` | File problems |
| `job.failed` / `job.cancelled` / `batch.jobNotAllowed` / `batch.stepFailed` | Jobs and batches (`details.stepIndex`) |
| `ui.busy` / `ui.modalOpen` / `ui.viewNotFound` | Attached-mode coexistence |
| `undo.notTop` / `undo.empty` | History |
| `agent.paused` | User paused the agent |
| `limit.exceeded` | Session limit (`details.limit`, `details.value`) |
| `internal` | Unexpected failure; includes a correlation id that appears in the application log |

Area-specific codes are defined in each section (e.g. `drawing.locked`, `element.wrongType`). Every error carries a
human-readable `message` and, where possible, a `hint` stating how to fix the call (e.g. "use `element.drawing.create`
first; frame 12 of element Bob is empty"). Warnings use the same code space and never abort the command.

---

## 11. Audit log and activity panel

- Every agent call (tool name, command id, parameters with inline media replaced by a hash, result status, changed
  object count, undo step id, duration, session/client name) is appended to an **activity log** kept per project
  session and, optionally, persisted next to the project as `agent-activity.jsonl` (preference, default off).
- The **Activity** panel lists agent calls live, groups them by batch, and lets the user: select an entry to
  highlight its changed objects; undo back to before an entry (if the steps after it can be undone in order);
  copy an entry as a replayable batch JSON; export the log.
- Undo history labels agent steps with the client's name (e.g. "Add Stroke — agent: Claude"), and `history`
  reports `origin`.

---

## 12. MCP prompts (workflow templates)

The server provides prompts that teach an agent the recommended command sequence for common tasks. Each prompt
takes arguments and returns a short task-specific procedure that references command ids and verification steps.
Required prompts:

| Prompt | Arguments | Procedure covers |
|---|---|---|
| `orient` | — | Read state, list scenes and elements, render current frame, summarise project |
| `draw_and_paint` | `element, frame, description` | Create drawing in cell → add strokes/shapes structurally → close gaps → paint zones by id → render_drawing to verify |
| `traditional_animation` | `element, frames, description` | Key drawings → in-betweens → exposures → onion-skin check via contact sheet |
| `cutout_rig` | `character` | Elements per body part, pegs and hierarchy, pivots, bones → test poses → evaluate transforms |
| `animate_motion` | `element, path, timing` | Peg keyframes, motion path points, velocity/ease, verify with `anim.evaluate` and contact sheet |
| `lip_sync` | `soundElement, mouthElement, transcript?` | Import sound → detect (with transcript) → map shapes to drawings → apply with overwrite policy → verify per-frame mouth track |
| `storyboard_to_animatic` | `panels: [image paths, durations, captions]` | Scenes per panel, imported images, timing, transitions, sound, preview |
| `effects_pass` | `scene, effect, targets` | Add effect, set parameters, seed, render frames to verify |
| `export_deliverable` | `format, path, range` | Validate project → render check frames → export job → verify manifest |
| `review_and_fix` | `scene` | `doc.validate` → fix issues → re-validate |

Prompt texts are generated from the registry so they never reference commands that do not exist (enforced by test).

---

## 13. Agent ergonomics requirements

- **Compact defaults.** Queries return depth 1 and the most useful fields; `fields` and `depth` expand.
  Large collections paginate (`page: {cursor, limit}`; default limit 100; results include `nextCursor`).
- **Summaries.** Object JSON includes computed summary fields where cheap and useful (element: drawing count,
  exposed frame range; drawing: stroke/zone counts, bounds, unpainted zone count; scene: length, element count).
- **Names over ids in summaries.** Text summaries in `content` use names; `structuredContent` uses ids and names.
- **Examples everywhere.** `describe_command` includes at least one example request and its result; docs resources
  include a worked end-to-end example per namespace.
- **Stable ordering.** Lists are returned in document order (z-order, exposure order, palette order) and say so.
- **Self-describing errors** with hints (§10).
- **No hidden state traps.** Commands never depend on a UI state that cannot be read through `get_state` or set by
  a command.
- **Localization independence.** Command ids, parameter names, enum values, error codes and object type names are
  language-independent; labels and messages follow the UI language, with an `en` copy always available through
  `get_docs`.

---

## 14. Conformance and testing (requirements for the implementation)

1. **Parity test (build gate).** Every GUI menu item, toolbar button, context-menu item, shortcut-bindable action,
   tool, tool option, dialog and preference is bound to a registered command id. The build fails if any GUI action
   has no binding or a binding refers to an unregistered id. The test also checks that every command in the registry
   is reachable from at least one GUI location or is marked `agentOnly` with a justification.
2. **Schema tests.** Every command has parameter and result schemas, at least one example, and the example
   validates and runs successfully on a fixture project.
3. **Round-trip tests.** For each object type: create via commands → save → reopen → `get_object` returns identical
   JSON (excluding volatile fields).
4. **Gesture equivalence tests** (§7.6) for every gestural tool.
5. **Determinism tests.** Golden-image renders of fixture scenes at fixed frames (including particle effects and
   lip-sync results) match byte-for-byte across runs and between attached and headless modes.
6. **Never-block test.** A headless run executes every command with valid example parameters under a watchdog;
   any attempt to show a modal UI fails the test.
7. **Protocol tests.** MCP compliance (initialize, tools, resources, subscriptions, prompts, progress, cancellation,
   elicitation fallback) against the reference MCP inspector/clients.
8. **End-to-end agent scenarios** (run as scripted batches and, periodically, with a live agent):
   - **E1 Bouncing ball:** new project 1920×1080 at 24 fps → drawing element "Ball" → circle shape → paint → peg →
     keyframes at frames 1, 12, 24 with ease → render contact sheet → export MP4 → manifest lists one file of 24 frames.
   - **E2 Paint-by-id:** import a line-art image → vectorize → `doc.validate` reports unpainted zones → agent paints
     each zone by id → validation clean.
   - **E3 Lip-sync:** import a WAV and a transcript → detection job → mouth track has a shape for every frame →
     apply to mouth element with `overwrite: "replace"` → `describe_frame` at sampled frames reports matching shapes.
   - **E4 Cut-out rig:** create body-part elements, pegs and bones, pose at three keyframes → `anim.evaluate` world
     transforms match the requested angles within 1e-6.
   - **E5 Storyline:** three scenes with a dissolve transition → render a frame inside the transition → both scenes
     contribute pixels.
   - **E6 Coexistence:** while a scripted "user" drags an element in attached mode, the agent's mutating call waits
     and then succeeds; `undo ifTop` refuses to undo the user's later step.
   - **E7 Safety:** under Assist, overwriting an export file returns `needs_confirmation`; with the token it proceeds;
     a `network` command returns `permission.denied`.

---

## 15. Commands owned by this section

### 15.1 Agent-session commands (namespace `agent`)

These are user-facing controls (Preferences › Agent access, agent indicator, Activity panel) also available to
the GUI's own command layer. Commands marked `app` scope cannot be invoked by an agent session on itself.

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `agent.server.set` | ui | Enable/disable the MCP server and set transport options (scope `app`) | `enabled: bool; http?: bool = true; port?: int (1024..65535); allowRemote?: bool = false` | `{enabled, endpoint?, discoveryFile?}` | no |
| `agent.server.status` | query | Server status, endpoint, connected sessions | — | `{enabled, endpoint, sessions: [{sessionId, client, policy, scopes[], connectedAt, lastActivity}]}` | n/a |
| `agent.token.regenerate` | ui | Regenerate the HTTP bearer token, disconnecting HTTP sessions (scope `app`) | — | `{discoveryFile}` | no |
| `agent.session.grant` | ui | Change a session's policy preset or scopes (scope `app`) | `sessionId; policy?: enum "observe"\|"assist"\|"autonomous"; scopes?: [string]; roots?: [path]` | `{sessionId, policy, scopes[], roots[]}` | no |
| `agent.session.pause` | ui | Pause an agent session (its calls wait, then fail with `agent.paused`) | `sessionId? (all when omitted)` | `{paused: [sessionId]}` | no |
| `agent.session.resume` | ui | Resume a paused session | `sessionId?` | `{resumed: [sessionId]}` | no |
| `agent.session.disconnect` | ui | Disconnect a session | `sessionId` | `{}` | no |
| `agent.policy.defaults.set` | ui | Default policy and limits for new sessions (scope `app`) | `policy?: enum "observe"\|"assist"\|"autonomous"; maxJobs?: int (1..8) = 2; maxBatchSteps?: int (1..100000) = 1000; maxInlineImagePx?: int (256..8192) = 2048; maxInlineBytes?: int = 52428800; askInApp?: bool = true; letAgentMoveView?: bool = true; highlightAgentChanges?: bool = true` | Effective settings | no |
| `agent.typedTools.set` | ui | Expose commands as individual MCP tools (scope `app`) | `enabled: bool; namespaces?: [string]` | `{toolCount}` | no |
| `agent.activity.list` | query | Activity log entries | `sessionId?; since?: time; limit?: int = 100` | `[{seq, time, sessionId, tool, commandId, status, undoStepId, changedCount, durationMs}]` | n/a |
| `agent.activity.export` | action | Write the activity log to a file | `path; format?: enum "jsonl"\|"batch" = "jsonl"; onExists?: enum "fail"\|"overwrite"\|"rename" = "fail"` | `{path, entries}` | no |
| `agent.activity.persist.set` | toggle | Persist the activity log beside the project | `enabled: bool` | `{enabled}` | no |
| `agent.confirmation.respond` | ui | The user's answer to a pending agent confirmation shown in the app | `confirmationId; accept: bool` | `{confirmationId, accepted}` | no |
| `agent.session.connected` | event | A session connected | — | `{sessionId, client, policy}` | n/a |
| `agent.session.disconnected` | event | A session disconnected | — | `{sessionId, reason}` | n/a |
| `agent.confirmation.requested` | event | A confirmation is waiting for the user (attached mode) | — | `{confirmationId, sessionId, summary, consequences[]}` | n/a |

### 15.2 Inspection, checkpoint and job commands (namespaces `inspect`, `checkpoint`, `job`)

These back the generic tools of §7.5 and §7.7 so that they can also be used in `batch` steps and batch scripts. They
have no GUI equivalent requirement except where noted in section 01 (history, activity panel).

| Command ID | Kind | Summary | Parameters | Result | Undo |
|---|---|---|---|---|---|
| `inspect.ui.tree` | query | UI tree of the window, a view, a panel or the open dialogs (same as `inspect_ui`) | `target?: string = "window"; depth?: int = 3` | `{nodes: [{role, id, label, value?, enabled, visible, commandId?, children[]}]}` | n/a |
| `inspect.screenshot` | query | PNG of the real GUI (attached) or a virtual view (headless) | `target?: string = "window"; region?: {x,y,w,h}` | `{png, width, height, sha256}` | n/a |
| `inspect.frame.describe` | query | Structured description of what is visible and audible at a frame (same as `describe_frame`) | `sceneId?: id; frame?: int; cameraId?: id; detail?: enum "summary"\|"full" = "summary"` | `{elements: [{elementId, name, drawingId?, worldTransform, cameraBounds, opacity, effects[]}], sounds: [{elementId, level}], mouthShape?, warnings[]}` | n/a |
| `inspect.hitTest` | query | Objects under a point, top-most first | `space: enum "view"\|"drawing"\|"scene"\|"camera"\|"image"; point: point; sceneId?: id; frame?: int; view?: string` | `{hits: [{elementId, drawingId?, layerId?, objectId?, kind, colourId?}]}` | n/a |
| `inspect.coordinates.convert` | query | Convert points between coordinate spaces | `from: string; to: string; points: [{x,y,z?}]; sceneId?: id; frame?: int; elementId?: id; view?: string` | `{points: [{x,y,z?}]}` | n/a |
| `inspect.contactSheet.render` | query | Grid image of several frames (same as `render_contact_sheet`) | `sceneId?: id; frames: [int] \| {from,to,step}; columns?: int; cellWidth?: int = 240; labels?: bool = true` | `{png, width, height, cells: [{frame, x, y, w, h}]}` | n/a |
| `checkpoint.create` | ui | Create a named in-memory snapshot of the document | `name: string` | `{name, documentRev}` | no |
| `checkpoint.restore` | action | Restore a checkpoint (one undo step "Restore checkpoint") | `name: string` | `{name, documentRev}` | yes |
| `checkpoint.list` | query | List checkpoints | — | `[{name, createdAt, documentRev, origin}]` | n/a |
| `checkpoint.delete` | ui | Delete a checkpoint | `name: string` | `{}` | no |
| `job.list` | query | Jobs of this session (or all, for the GUI) | `jobId?: string; all?: bool = false` | `[{jobId, commandId, state, progress, message, result?, error?}]` | n/a |
| `job.wait` | query | Wait for a job to finish or a timeout | `jobId: string; timeoutMs?: int = 60000` | Job record, `timedOut?: bool` | n/a |
| `job.cancel` | ui | Cancel a job; partial results are discarded unless the command documents otherwise | `jobId: string` | Job record | no |
| `job.progress` | event | Job progress changed | — | Job record | n/a |
| `job.finished` | event | Job reached a final state | — | Job record | n/a |

---

## 16. Design decisions / open questions

1. **Generic vs typed tools.** Recommended: generic tool set by default (keeps the tool list small for agents),
   typed tools optional per namespace. Revisit when clients handle large tool lists well (e.g. with tool search).
2. **MCP tasks.** If the negotiated protocol revision supports task-augmented tool calls, `job` commands SHOULD
   also be exposed through that mechanism; the `job_*` tools remain for older clients.
3. **Offscreen renderer.** Recommended: software rasteriser for all agent and export renders (determinism), GPU for
   interactive views, with golden tests keeping them visually equivalent.
4. **Concurrent multi-agent editing.** Recommended: allowed, serialized at command granularity, with `expectRev` for
   agents that need safety; no object locking in version 1.
5. **Checkpoint storage.** Recommended: copy-on-write snapshots of the in-memory document; not saved with the project
   unless the user chooses *Save checkpoint as version*.
6. **In-app assistant.** The application MAY later embed its own assistant panel that acts as an MCP *client* of this
   same server (so it gets exactly the agent capabilities described here). Not required for version 1.
