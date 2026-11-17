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

import math

#: The canvas keeps the last 300 frames of each kind (desktop/src/canvas/metrics.ts).
FULL = 300
#: A plausible control median frame interval, ms: a 333 Hz display at the fastest, 10 frames a second at the slowest
#: (C43B-R2-05).
CONTROL_MIN_MS = 3.0
CONTROL_MAX_MS = 100.0
#: The 1,000-element scene's drawings and the sample scene's two (Bob and Ann).
RESIDENT_1K = 1002


def p95(values: list) -> float | None:
    s = sorted(values)
    return s[max(0, -(-len(s) * 95 // 100) - 1)] if s else None


def _n(stat) -> int:
    return (stat or {}).get("n", 0) if isinstance(stat, dict) else 0


#: C43B-R3S-05 (ruling D-48): each scene summary and what its values must be: durations of work, feedback and commits
#: are finite and non-negative; frame times are finite and positive; every summary is ordered (p50 <= p95 <= max).
SUMMARIES = {"idle": "positive", "dragFrames": "positive", "idleWork": "nonNegative", "dragWork": "nonNegative",
             "feedback": "nonNegative", "commits": "nonNegative"}
#: The sample populations: every interval finite and positive; every commit sample finite and non-negative.
POPULATIONS = {"idleIntervals": "positive", "dragIntervals": "positive"}


def _ok_value(v, rule: str) -> bool:
    if isinstance(v, bool) or not isinstance(v, (int, float)) or not math.isfinite(v):
        return False
    return v > 0 if rule == "positive" else v >= 0


def scene_problems(r: dict) -> list:
    """Every impossible value of one scene's measurements (C43B-R3S-05), before any budget is applied."""
    out = []
    for key, rule in SUMMARIES.items():
        stat = r.get(key)
        if stat is None:
            continue  # its absence is the completeness checks' failure
        if not isinstance(stat, dict):
            out.append(f"{key}: not a summary")
            continue
        vals = [stat.get(k) for k in ("p50", "p95", "max")]
        bad = [k for k, v in zip(("p50", "p95", "max"), vals) if v is not None and not _ok_value(v, rule)]
        if bad:
            out.append(f"{key}: {', '.join(bad)} not finite and {'positive' if rule == 'positive' else 'non-negative'}")
        elif all(v is not None for v in vals) and not (vals[0] <= vals[1] <= vals[2]):
            out.append(f"{key}: p50 <= p95 <= max does not hold")
        samples = stat.get("samples")
        if isinstance(samples, list) and any(not _ok_value(v, rule) for v in samples):
            out.append(f"{key}: a sample is not finite and {'positive' if rule == 'positive' else 'non-negative'}")
    for key, rule in POPULATIONS.items():
        values = r.get(key)
        if isinstance(values, list) and any(not _ok_value(v, rule) for v in values):
            out.append(f"{key}: an interval is not finite and positive")
    return out


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
    # C43B-R3S-05 (ruling D-48): every scene's populations and summaries are physically possible before any budget.
    valid = True
    for scene in ("sample", "groups1k"):
        r = record.get(scene)
        if isinstance(r, dict):
            bad = scene_problems(r)
            valid = valid and not bad
            check(not bad, f"{scene}: every measurement finite, durations non-negative, intervals positive, "
                           "p50 <= p95 <= max", bad)
    resident = (record.get("groups1k") or {}).get("resident", 0)
    check(resident >= RESIDENT_1K, f"the 1,000-element scene has its {RESIDENT_1K} drawings resident", resident)

    # C43B-R2-05: the control's cadence must be a display's before it chooses the budget: positive, finite, ordered
    # (p50 <= p95 <= max) and between CONTROL_MIN_MS and CONTROL_MAX_MS (a 333 Hz display to 10 frames a second); a
    # zero or absent cadence fails here instead of selecting plan §6's ordinary budget.
    frame = control.get("frameMs") or {}
    median = frame.get("p50")
    stats = [frame.get(k) for k in ("p50", "p95", "max")]
    plausible = (all(isinstance(v, (int, float)) and math.isfinite(v) and v > 0 for v in stats)
                 and stats[0] <= stats[1] <= stats[2] and CONTROL_MIN_MS <= stats[0] <= CONTROL_MAX_MS)
    check(plausible, f"D-33 control: a plausible cadence (median {CONTROL_MIN_MS:g}-{CONTROL_MAX_MS:g} ms, "
          "positive, p50 <= p95 <= max)", {k: frame.get(k) for k in ("p50", "p95", "max")})
    if not plausible:
        median = None
    capped = median is not None and median > 17.0
    record["assessment"] = ("D-33 external cap: work p95 <= 8 ms, <= 1 % of intervals > 2 x the control median" if capped
                            else "plan §6: frame time p95 <= 17.5 ms")
    for scene in ("sample", "groups1k"):
        r = record.get(scene)
        if not valid:
            break  # no budget is assessed on impossible measurements (the validation check above failed)
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
