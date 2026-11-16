"""The assessment of scripts/c43-camera-measure.py's record (M4 C4.3b; plan §6, rulings D-5, D-33; C43B-R1S-08).

Pure, so its rules are tested on fixtures (scripts/test_c43_measure_assess.py): `assess(record, bounds, drags)` returns
every check as `(passed, name, detail)`. A measurement that is incomplete fails; it is never assessed on what it has:

- the back end is the one requested (`--canvas-backend`), with no fallback;
- the control has its 300 frames on the start screen;
- the 1,000-element scene has every drawing resident (1,000 elements and the sample's two: 1,002);
- on each scene, every drag committed (one release-to-committed sample per drag), and the canvas's rings are full:
  300 idle frame intervals and 300 idle frames' work, 300 frame intervals and 300 frames' work drawn during the drags
  (the work during the drags is that of the frames drawn while a gesture was in progress, not of idle frames between
  drags), and 300 drag feedback samples with OS input.

Then the budgets: plan §6's frame time, or, when the control shows an external cap (median above 17 ms), D-33's work
per frame and share of long intervals; release-to-committed on the sample scene; drag feedback with OS input.
"""

from __future__ import annotations

#: The canvas keeps the last 300 frames of each kind (desktop/src/canvas/metrics.ts).
FULL = 300
#: The 1,000-element scene's drawings and the sample scene's two (Bob and Ann).
RESIDENT_1K = 1002


def p95(values: list) -> float | None:
    s = sorted(values)
    return s[max(0, -(-len(s) * 95 // 100) - 1)] if s else None


def _n(stat) -> int:
    return (stat or {}).get("n", 0) if isinstance(stat, dict) else 0


def assess(record: dict, bounds: dict, drags: int) -> list:
    checks: list = []

    def check(ok, name, detail):
        checks.append((bool(ok), name, detail))

    requested = record.get("requestedBackend")
    check(record.get("backend") is not None and (requested is None or record.get("backend") == requested)
          and record.get("fallback") is None,
          "the canvas runs the requested back end with no fallback",
          {"requested": requested, "backend": record.get("backend"), "fallback": record.get("fallback")})
    control = record.get("control") or {}
    check(_n(control.get("frameMs")) >= FULL and control.get("startScreen") is True,
          "D-33 control: 300 frames of the webview's cadence on the start screen", control.get("frameMs"))
    resident = (record.get("groups1k") or {}).get("resident", 0)
    check(resident >= RESIDENT_1K, f"the 1,000-element scene has its {RESIDENT_1K} drawings resident", resident)

    median = (control.get("frameMs") or {}).get("p50")
    capped = median is not None and median > 17.0
    record["assessment"] = ("D-33 external cap: work p95 <= 8 ms, <= 1 % of intervals > 2 x the control median" if capped
                            else "plan §6: frame time p95 <= 17.5 ms")
    for scene in ("sample", "groups1k"):
        r = record.get(scene)
        if not isinstance(r, dict):
            check(False, f"{scene}: measured", None)
            continue
        # Completeness first: an incomplete scene fails.
        commits = (r.get("commits") or {}).get("n", 0)
        check(r.get("drags") == drags and commits == drags,
              f"{scene}: every one of the {drags} drags committed and was measured", {"drags": r.get("drags"), "commits": commits})
        # C43B-R1-05: the bounds name their least number of actual samples (workstation: 30, plan §6), whatever
        # `--drags` asked for.
        least = int(bounds.get("minSamples", 30))
        check(commits >= least, f"{scene}: at least {least} actual release-to-committed samples", commits)
        full = {
            "idle frame intervals": len(r.get("idleIntervals") or []),
            "idle frames' work": _n(r.get("idleWork")),
            "frame intervals during the drags": len(r.get("dragIntervals") or []),
            "work of the frames drawn during the drags": _n(r.get("dragWork")),
        }
        if r.get("input") == "os":
            full["drag feedback samples"] = _n(r.get("feedback"))
        for what, n in full.items():
            check(n >= FULL, f"{scene}: {FULL} {what}", n)
        if capped:
            long = 2 * median
            for phase, intervals, work in (("idle", r.get("idleIntervals") or [], r.get("idleWork") or {}),
                                           ("during scene drags", r.get("dragIntervals") or [], r.get("dragWork") or {})):
                share = sum(1 for v in intervals if v > long) / len(intervals) if intervals else None
                r.setdefault("d33", {})[phase] = {"workP95": work.get("p95"), "longShare": share,
                                                  "intervals": len(intervals), "longMs": long}
                check(work.get("p95") is not None and work["p95"] <= 8.0, f"{scene}: work per frame p95 {phase} <= 8 ms (D-33)", work)
                check(share is not None and share <= 0.01,
                      f"{scene}: at most 1 % of frame intervals {phase} longer than {long:.1f} ms (twice the control median; D-33)",
                      {"share": share, "intervals": len(intervals)})
            budget = []
        else:
            budget = [
                ("frame time p95, idle", (r.get("idle") or {}).get("p95"), bounds["frameMs"]),
                ("frame time p95, during scene drags", (r.get("dragFrames") or {}).get("p95"), bounds["dragFrameMs"]),
            ]
        # Plan §6 names the 1,000-element scene for the frame time; release-to-committed is the sample scene's bound,
        # and the large scene's figure is recorded with its parts (the kernel's answer, the redraw).
        if scene == "sample":
            budget.append(("release-to-committed p95 of a scene drag", (r.get("commits") or {}).get("p95"), bounds["commitMs"]))
        if r.get("input") == "os":
            budget.append(("drag feedback p95 (a sample to the frame drawing the preview)", (r.get("feedback") or {}).get("p95"),
                           bounds["feedbackMs"]))
        for name, value, bound in budget:
            check(value is not None and value <= bound, f"{scene}: {name} <= {bound} ms", value)
    return checks
