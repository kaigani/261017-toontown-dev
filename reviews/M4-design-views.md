# M4 C4.2 design note 6: views and the mapping

Before the code (plan reviews/M4-C4.2-plan.md §4, ruling D-3: C4.2a). Requirements: 02 §1.2, §2.1, §2.2, §4.3; 01 §6.5,
§12.2 (`view.*`); SI-056, SI-063, SI-065, SI-069, SI-107, SI-109, SI-112, SI-122, SI-123; M3 merge note §8 item 3 and
§3 line 70; 11 §6.4 ("let the agent move my view").

## 1. State

- One view window per view panel (`cameraView`, `drawingView`, `topView`, `sideView`; SI-112: `viewId` = panel id),
  already modelled by `shell::ViewWindow` inside `UiState`, so rollback, savepoints and the atomic-batch rules of C4.1
  carry it. Per window: current type, previous type, one transform per type (zoom, rotation in degrees, document point
  at the view centre; switching type and back restores it, 01 §6.5), the canvas size reported by the host, the last
  pointer position.
- **Not undoable** (02 §12.1). **Not persisted** across sessions: the spec persists no view transform (02 §12.3 has no
  row for it; 01 §6.5 says "remembered per view type", within the session). Recorded as SI-128 with this default; the
  canvas size comes from the host each launch.
- Headless and before the host reports a size: the Drawing view is the SI-069 virtual view, 1920 × 1080 px; the other
  views keep 960 × 540.

## 2. The mapping (one function)

- Drawing view: at 100 % one view pixel is one drawing unit (SI-063); view pixels have y down, drawing space y up
  (02 §2.1-2.2); rotation is a display transform (02 §2.2), clockwise positive in view pixels. `view_to_doc` and
  `doc_to_view` in `shell.rs` are the only implementation; `pixels_per_unit` for the Drawing view becomes 1 (it reads
  the camera width today, which contradicts SI-063). Staging views keep their camera-based scale (C4.3).
- The affine matrices `drawingToView` and `viewToDrawing` are produced from that function and published:
  `draw.viewMapping.get` (now reading the real view state, `view` = `drawing` or a view id), `view.transform.get`,
  `view.coordinates.convert`, and the view's announcement to the canvas (C4.2b), which never derives a mapping itself.
- `view` space becomes available to `colour.pick`, `inspect.hitTest` and `view.coordinates.convert` (SI-065, SI-109)
  through the same matrices. Scene space in `view.coordinates.convert` for the Drawing view is the drawing's space under
  the current element's transform at the current frame (as `draw.*` scene-space conversions do today); camera and
  staging spaces are C4.3's.

## 3. Commands (01 §12.2)

`view.list`, `view.active.set`, `view.type.set` (with `"previous"`), `view.transform.get`, `view.zoom.set` (absolute
within 6.25 %–6400 %, or by table steps; the 02 §4.3 ladder for wheel and zoom-tool steps, SI-122; `anchor` keeps that
view point fixed), `view.rotation.set` (absolute or 30° steps), `view.pan.set` (`centre`, `by`, `atPointer`),
`view.transform.reset` (parts), `view.coordinates.convert`. Rotation is only through `view.rotation.set` (SI-123).
Every change announces `view.changed` with the view's new state (the canvas follows it).

## 4. Agents

With `agent.policy.defaults.letAgentMoveView` (default on) false, an agent's `view.*` change to a view refuses with
`permission.denied`, `details.preference: "letAgentMoveView"` (11 §6.4 lets such commands affect only session-owned
virtual views, and C4.2 has no command to create one, SI-128). While the user is mid-gesture, agent view changes wait
and then answer `ui.busy` (C4.1's arbitration).

## 5. Invariants (one builder test each, in `tests/c42.rs`)

- **V1** One mapping: for random transforms and sizes, `view_to_doc(doc_to_view(p)) = p` within float rounding, and the
  matrices published by `draw.viewMapping.get`, `view.transform.get` and `view.coordinates.convert` give the same
  points as the function for the same state.
- **V2** View state is not undoable (`edit.undo` after a view change does not restore it) and is per type (switching
  type and back restores the transform).
- **V3** With `letAgentMoveView` off, an agent's view change refuses with `permission.denied` and changes nothing; the
  user's (or with the preference on, the agent's) succeeds.
- **V4** The headless Drawing view is SI-069's: 1920 × 1080, zoom 1, `drawingToView [1,0,0,-1,960,540]`.
- **V5** Zoom stays within 6.25 %–6400 %; steps follow the table (`steps`) and the ladder (wheel, zoom tool).
