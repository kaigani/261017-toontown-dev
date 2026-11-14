# M4 C4.2 design note 4: the `pointer` tool

Before the code (plan §4; ruling D-3: the headless half in C4.2a, the attached input queue in C4.2b). Requirements:
11 §7.6 (tool definition, real input path, simulated `dt`, equivalence), §7.8 (the active tool's structured command),
§6.4 (coexistence), §6.5 (limits); 02 §13.3; SI-112, SI-124, SI-125; rulings D-4, D-7.

## 1. Shape

`pointer {view, space?, pointerType?, events[], tool?}` (11 §7.6). The MCP tool calls one executor entry point,
`ExecutorHandle::pointer(origin, request)`, which runs on the document thread as one request; it is not a registry
command (11 maps `pointer` to the active tool's commands, §7.8).

1. `tool` given: `tool.active.set` first (agent origin; SI-124).
2. Events are grouped into gestures: `down` starts one (the view mapping is captured), `move` adds samples, `up` ends
   it; `hover` updates the pointer sensor; `wheel` outside a gesture zooms by the ladder (SI-122) through
   `view.zoom.set`; `doubleClick` ends a polyline. A `wheel` inside a gesture refuses (`arg.invalid`, 02 §13.3).
3. Each gesture's samples go through `gesture::to_command` (design note 3) and the result is invoked with the caller's
   origin and scopes; the envelopes are returned in order, with the final hover information (view point, drawing point,
   the objects under it as `inspect.hitTest` reports them).
4. `space` per SI-125: `drawing` samples are converted to view pixels at the press; `scene` and `camera` refuse in the
   Drawing view.
5. `dt` adds to a simulated clock; nothing reads the wall clock.

## 2. Coexistence

- Headless (C4.2a): one session's `pointer` call is one document-thread request, so gestures of different calls never
  interleave.
- Attached (C4.2b): a per-window input queue serialises agent and user gestures: an agent gesture never interleaves
  with a user's; a user press during an agent gesture cancels the agent's (typed error, nothing committed); while the
  user is mid-gesture, the agent's `pointer` waits up to `waitForUserMs` and then answers `ui.busy` (11 §6.4).

## 3. Limits

At most 10,000 events per call (11 §6.5 caps batches at 1,000 steps; a stroke can hold thousands of samples, so the
event cap is stated separately and refused with `limit.exceeded`, `details.limit: "pointerEvents"`; SI-130 records the
number). Events must be finite numbers within ±10,000,000 view pixels (`arg.invalid` otherwise).

## 4. Invariants (one builder test each)

- **P1** Gestures of one call never interleave with another call's (headless: serial document-thread requests;
  attached: the input queue, C4.2b).
- **P2** A user press during an agent gesture cancels the agent's with a typed error and leaves the document unchanged
  (C4.2b).
- **P3** `dt` never reads the wall clock: the same events give the same commands on every run.
- **P4** Agent mutations during a user gesture wait at most `waitForUserMs`, then `ui.busy` (C4.2b; headless has no
  user gestures).
- **P5** The envelopes returned are those of the commands invoked, in order; a refused gesture's envelope is its typed
  refusal.
