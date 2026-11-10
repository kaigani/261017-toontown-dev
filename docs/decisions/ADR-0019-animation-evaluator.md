# ADR-0019 — Animation evaluator

Status: accepted (M3 C3.2).

## Context

05 §2 fixes the transform mathematics "so that an implementation and an AI agent … obtain identical, deterministic
results": within 1e-6 field and degree across platforms, bit for bit on one platform (05 §19.1.6). E4 compares against
independent analytic values and across operating systems byte for byte. ADR-0004 restricts persisted and rendered
numerics to IEEE `+ − × ÷ sqrt` in a fixed order, with transcendental functions from a pinned implementation. Long
evaluations must not block the document thread (the never-block rule): they are charged and refused or cancelled before
the 30-second watchdog.

## Decision

1. **Where.** `tt_anim` holds the pure maths (`num`, `matrix`, `tcb`, `function`, `timing`, `node`, `eval`, `camera`), with
   no dependency on the executor; `tt-executor` (`commands/anim_*.rs`) holds schemas, targets, staging and events.
2. **Numbers.** `f64` throughout. `+ − × ÷` and `f64::sqrt`, `floor`; `sin`, `cos`, `tan`, `atan`, `atan2`, `fmod` come
   from `libm` with `force-soft-floats`, the pin `tt-vector` already uses. Multiples of 90° are exact in `sin_deg` and
   `cos_deg`. No `mul_add`, no platform maths. A 2-D norm (`matrix::norm`, used by the §2.5 decomposition) is
   `√(a² + b²)` while that sum is a finite normal number, and otherwise `k · √((a/k)² + (b/k)²)` with `k` the larger
   magnitude, so every finite matrix decomposes to finite values and ordinary results keep their bits (C32-R1-06).
3. **Fixed evaluation order.** Scene frame → local frame (`F − start + 1`, held to the bar) → cycle frame
   (`((f − 1) mod L) + 1`) → `V(f)` → clamped arc length → bracketing samples → `C(s)`; 1-D channels at the cycle frame;
   static + animated (§1.5); `M_local` multiplied left to right exactly as §2.2 prints it; `W(parent) · M_local` from the
   topmost ancestor down. Matrix products sum `a[r0]·b[0c] + a[r1]·b[1c] + a[r2]·b[2c] + a[r3]·b[3c]` in that order.
4. **Exactness where it is available.** A keyframe's own frame returns its stored point or value without going through a
   table; zero handles and the `linear` preset are straight lines computed directly; `C(0)` and `C(1)` are the end points
   exactly. So exact inputs give exact outputs, and rounding cannot accumulate along a path.
5. **Output.** One serialisation path; `-0.0` is normalised to `0` (`num::tidy`). Non-finite results are refused
   (`anim.valueOutOfRange`, `reason: nonFinite`), never returned as `null` and never as an encoding failure: the
   evaluator refuses a world matrix that left the finite range, `anim.evaluate` checks the decomposition and the world
   corners of each node (with its `nodeId`), and the result encoder refuses any other non-finite number of an `anim.*`
   result with the same code (`tt_registry::encode_command_result`, C32-R1-06).
6. **Explicit convergence budgets.**

   | Constant | Value | Why |
   |---|---|---|
   | `function::BEZIER_MAX_ITERATIONS` | 52 | Halving `u` 52 times reaches one unit in the last place of values near 1, inside the spec's 1e-9; the loop is fixed, so the count is the same on every platform |
   | `tcb::SAMPLES_PER_SEGMENT` | 64 | 05 §9.2 |
   | `tcb::PASSAGE_TOLERANCE` | 1e-4 frame | 05 §10.5's required accuracy; the bisection goes past it, to the bracket's floating-point resolution, so the 3-decimal rounding is that of the crossing itself (a crossing within 1e-4 below a rounding boundary is not carried across it, C32-R1-01) |
   | `tcb::PASSAGE_MAX_ITERATIONS` | 64 | The bracket is one monotone piece of a keyframe segment, at most 100 000 frames; halved 64 times it is under 1e-14 frame, below the spacing of doubles there; the bisection stops earlier when the midpoint is no longer strictly inside the bracket |
   | `tcb::MIN_TANGENT` | 1e-9 | 05 §8.3 |
   | `eval::NODE_FRAME_COST` | 16 units | Per node-frame evaluated |
   | `anim_eval::MAX_RESULTS` | 1 000 000 | Frame × node results or samples of one query |

7. **Never block.** `Evaluator` charges every node read (`NodeData::cost`: keys + 65 per motion point), node-frame,
   hierarchy read, passage-frame step and orient-to-path search step to a meter closure. The executor passes the same
   300-million-unit work account as C3.1 (`timing::TIMING_WORK_BUDGET`), which observes the caller's cancellation at each
   charge: an oversized request answers `limit.exceeded`, a cancelled one `command.timeout`, both before the watchdog.
   Paths are read once per call, world matrices memoised per scene frame. `c32_budgets` records the worst cases. Undo
   and redo charge an unlimited work account once per replayed entry (`Cx::replay`), so a long replay of a large
   animation step observes the caller's cancellation while it runs and commits nothing (C32-R3-01); no step becomes
   impossible to undo.
   A passage frame (05 §10.5) is bracketed without sampling: each velocity sub-segment's value extrema are the roots of
   its cubic's derivative, a quadratic solved in closed form in its cancellation-free form (`function::extremum_frames`),
   so nodes and extrema split the segment into monotone pieces; the candidates are evaluated in order up to the first
   with `V >= arc`, and the piece before it is bisected to resolution and rounded (C32-R1-01). A crossing exactly at a
   candidate frame reports that frame. Each candidate evaluation and each bisection step is one passage-frame step.
8. **Channel model.** As 05 and 10 §4.12 stand: `position` is a motion path, `position.x/y/z` projections, the rest 1-D.
   Keys are stored in local frames; every command frame is a scene frame in the first cycle. D-5 (per-axis position) is
   unresolved: SI-096 records the migration and the IPC types it would break.
9. **Velocity storage.** Intermediate keys store `u`, normalised to their keyframe segment (05 §10.5). A key record at a
   pin frame carries shape only (easing, handles). Inserting or removing a pin renormalises, with the old arc lengths.
10. **AG-02.** `doc.object.patch` applies `/transform/offset`, `/rotation`, `/skew`, `/scale`, `/camera/fov`, `/camera/fovMode`
    and a scene's `/currentCameraId` through the owning handlers, in order (ADR-0013 §2).
11. **Default field of view.** The stored and schema default is the exact 2·atan(0.375) in degrees,
    `41.11209043916693` (`tt_model::defaults::DEFAULT_FOV`, computed through `num::atan_deg`; a test checks the bits), so
    the 12-field width fills the frame at distance 16 to rounding (05 §2.7). The tables' 41.1121 is its 4-decimal
    display. Every FOV, an explicitly set 41.1121 included, is stored, reported and projected as itself: the reported
    `effectiveFov` is the projected value, so an explicit 41.1121 spans about 12.0000031 fields and only the exact
    default spans 12 (SI-102; SI-104 ruled "no alias", C32-R2-01). Files that store 41.1121 need no migration.
12. **Events.** `anim.changed` carries `frames` only when the change is one finite scene range: the touched frames
    widened to the neighbouring keys (two motion keyframes for `position`), inside the bar, on an unlooped node. Otherwise
    it omits `frames`, the all-frame form, and in the per-transaction union an all-frame event absorbs any range
    (05 §19.4, C32-R1-05). The undo step keeps that union (`history::Step::anim`); each undo and redo of the step emits it
    again, after the replay's `doc.changed`, so a replayed step invalidates exactly what the original did (C32-R2-02).
    History lives with the open project: saving keeps it, opening starts a fresh one.

## Consequences

- Cross-OS byte identity is a CI gate (`e4 cross-OS`), not a tolerance. A libm or compiler change that alters a bit fails it.
- `anim.evaluate` is a pure read: no revision, undo step, selection, zone ID or saved default changes.
- Bones and skinning (M6 C6.3) add to `node` and `eval`; effects pass the transform through until C7.
- Render-list evaluation (bitmap bounds at native size, roles, annotation policy per export) is C3.3's.
