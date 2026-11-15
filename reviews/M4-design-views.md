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
- **The active view** (01 §4; amended in C4.2b round 3). The active view window is the one last *activated* while it
  is shown, else the central view. A view is activated by `view.active.set` (its GUI path, 01 §12: a click into the
  view window, not wired in the window yet, SI-142) and by `app.mode.set`, which activates a view of the requested type;
  a view that is hidden or closed stops being active and the central view is used until another is activated. Focus
  does not activate a view: a float's window taking the OS focus, or `window.panel.set {focus: true}`, moves
  `focusedPanel` only. The active view's type decides the mode (01 §4) and is what `render.frame.run {composition:
  "activeView"}` draws; its result does not name the view (SI-142: read `app.state.get` in the same batch).
  `inspect.screenshot` names the view it captured. A canvas and its gestures address their own view id, so a floated
  Drawing view draws and commits whether or not it is the active view. (`ui.rs` `active_view`.)

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
within 6.25 %–6400 %, or by 01 §6.5's table steps; `anchor` keeps that view point fixed; ruling D-15: the wheel
steps along 02 §4.3's ladder, a Zoom-tool click and the zoom keys send table steps, a Zoom marquee fits its rectangle,
SI-122), `view.rotation.set` (absolute or 30° steps), `view.pan.set` (`centre`, `by`, `atPointer`),
`view.transform.reset` (parts), `view.coordinates.convert`. Rotation is only through `view.rotation.set` (SI-123).
Every change announces the host-only event `shell.view` with the view's new state (`viewId`, type, size, transform and
both matrices; the canvas follows it). Spec A has no view event and the registry cannot grow (796 entries), so the
announcement is a `shell.*` event like the other host notifications: agents do not receive it and read views with
`view.list` and `view.transform.get` (corrected with the code; the first version of this note said `view.changed`).

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
- **V5** Zoom stays within 6.25 %–6400 %; `steps` (the zoom keys and a Zoom-tool click) follow the 01 §6.5 table, the
  wheel follows the 02 §4.3 ladder (ruling D-15, SI-122).

## 6. Amendment for C4.3: the Camera view (C4.3a; plan `reviews/M4-C4.3-plan.md` §3.3, answers 1 and 4)

**The Camera view's document space is camera space** (SI-109: fields from the camera centre on the table plane, roll
applied, y up). Its pixels per field at 100 % are the camera's output pixels per field, `1 / s` with `s` the scale of
`render::image_to_camera` (`2 · depth · tan(fov / 2) / width`), so at 100 % one view pixel is one output pixel; for the
default camera (16 fields, `tan(fov / 2) = 0.375`) that is `width / 12`, the value `shell::pixels_per_unit` already
uses. The view transform (zoom, rotation, centre in camera space) and `doc_to_view_matrix` are unchanged.

**The chain, one function per link** (V1 extended):
- view pixels and camera space: `doc_to_view_matrix` / `view_to_doc_matrix` with the Camera view's pixels per field;
- camera space and camera output pixels (`image` space, SI-109): `image_to_camera` and its inverse;
- output pixels and a drawing's units: the element's `to_pixel` from the render list at the frame (camera-view note
  §2) and its inverse;
- scene space and output pixels: `CameraView::project` and, at a given depth, `CameraView::unproject`;
- scene space and a drawing's units, for a point addressed to a drawing (02 §13.1 item 5): the element's world matrix
  at the frame restricted to its plane, and its inverse: a scene point means the point of the element's plane with
  those scene X and Y (SI-146). The Drawing view's `scene` space is unchanged: it was already the current element's
  transform at the current frame.

**Where it applies** (every declared use, ruling D-22): the addressing parameters of the `draw.*` and `colour.*`
commands (`space: scene`, and `space: view` with a Camera view; `addressing.rs`), `view.coordinates.convert` (spaces
`view`, `camera`, `image`, `scene`, `drawing`), `inspect.hitTest` (02 line 61: a view point maps back through the full
scene transform), `pointer` (`space: view|drawing|scene|camera`, 11 line 354), `anim.view.coords.get` (05 line 1234:
scene and view pixels with `depth`), `view.pan.set` centres in scene space for staging views (01 lines 1036-1041), and
the canvas's matrices. Top and Side views keep their refusals, re-owned to M6 C6.1 (answer 1).

**The active view** (answer 4): a press in a view window makes it active through `view.active.set` (user origin, UI
state, no undo step) and the same press is the tool's press; SI-142 records it as the interim.

**Invariants (C4.3, `tests/c43.rs`):**
- **V6** The Camera view's chain round-trips view, camera, output pixels and drawing units, and view and scene at a
  depth in front of the near plane, within 1e-9 relative, for random view transforms, cameras (position, roll, field of
  view) and element transforms; the published matrices give the same points as the functions.
- **V7** A drawing-space point mapped to view pixels by the Camera view's chain lands where the final renderer draws
  it: `to_pixel` then `cameraToView` equals the chain, for the camera-view note's CV1 fixtures.
- **V8** Every declared use takes scene and Camera-view spaces as a full table (command, space, view type): each
  Camera-view cell succeeds and gives the chain's point; each Top/Side cell refuses `command.notImplemented` with owner
  M6 C6.1; a cell that ignored the space would fail.
- **V9** A press in an inactive view window activates it (the window, C4.3b) and acts as the tool's press;
  `edit.history.get` shows no step for the activation.
