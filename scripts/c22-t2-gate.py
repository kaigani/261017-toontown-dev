#!/usr/bin/env python3
"""The C22-T2 completion gate (orchestrator ruling on the M2 C2.4 plan), from raw observations of one CI run.

Usage: python scripts/c22-t2-gate.py <strokes-100k-<os>.json> <flatten-100k-<os>.json> <out.json>

A 100 000-stroke Flatten counts as completed when its unbudgeted release wall time (p50 of at least 5 repetitions)
is at most the same run's 100k `strokeAdd` p50 and under the 30 s watchdog, and every unbudgeted repetition
completed. The product's budgeted Flatten is reported beside it (refused before the watchdog while the merge exceeds
its work budget). This is the C22-T2 gate only; C22-T2 and the interactive latency budget (R1-12) are the M11 C11.1
scale gate (accepted re-scope, M2 C2.4 round 1).

Validation (C24-R1-03): both files must have the documented shape; every duration is a finite, non-negative number
(booleans are not numbers); at least 5 stroke-add samples and 5 observations of each Flatten kind; both files come
from the same OS, architecture and release profile, and the stroke-add series from the same run's measurement
(`environment.startedAt` present); completion and refusal are derived from each record (a completed repetition is
committed with status `Ok` and no error; a refused one has an error code, did not commit and left the document
unchanged), and a stored `unbudgetedAllCompleted` must agree. The percentile is the one rule of
`scripts/analyse-measurements.py` (upper nearest rank, `sorted[round((n - 1) * q)]` with halves rounded up).

Exit status: 0 with the verdict written (a valid missed gate is evidence: `met: false`); 2 on malformed input, with
nothing written.
"""
import importlib.util
import json
import math
import pathlib
import sys

_SPEC = importlib.util.spec_from_file_location(
    "analyse_measurements", pathlib.Path(__file__).resolve().parent / "analyse-measurements.py"
)
_ANALYSER = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_ANALYSER)

WATCHDOG_SECONDS = 30.0
MIN_REPETITIONS = 5


class Malformed(ValueError):
    """The evidence does not have the documented shape."""


def number(x, what):
    """A finite, non-negative duration (booleans are rejected)."""
    if isinstance(x, bool) or not isinstance(x, (int, float)):
        raise Malformed(f"{what} is not a number: {x!r}")
    x = float(x)
    if not math.isfinite(x) or x < 0.0:
        raise Malformed(f"{what} is not a finite non-negative duration: {x!r}")
    return x


def values(series):
    """The observations of a series: a list of numbers, or `{samples|values|observations: [...]}`."""
    if isinstance(series, list):
        items = series
    elif isinstance(series, dict):
        items = next((series[k] for k in ("samples", "values", "observations") if isinstance(series.get(k), list)), None)
        if items is None:
            raise Malformed("unreadable series")
    else:
        raise Malformed("unreadable series")
    return [number(x["value"] if isinstance(x, dict) and "value" in x else x, "a stroke-add sample") for x in items]


def p50(v):
    """The p50 by the analyser's one nearest-rank rule."""
    if not v:
        raise Malformed("no observations")
    return _ANALYSER.nearest(sorted(v), 0.5)


def record(r, kind, i):
    if not isinstance(r, dict):
        raise Malformed(f"{kind}[{i}] is not an object")
    seconds = number(r.get("seconds"), f"{kind}[{i}].seconds")
    error = r.get("error")
    code = error.get("code") if isinstance(error, dict) else None
    if error is not None and not isinstance(code, str):
        raise Malformed(f"{kind}[{i}].error has no code")
    committed = r.get("committed")
    if not isinstance(committed, bool):
        raise Malformed(f"{kind}[{i}].committed is not a boolean")
    completed = committed and error is None and r.get("status") == "Ok"
    refused = code is not None and not committed and r.get("unchangedOnRefusal") is True
    return seconds, completed, refused, code


def evaluate(strokes, flatten):
    if not isinstance(strokes, dict) or not isinstance(flatten, dict):
        raise Malformed("both files must be JSON objects")
    env = strokes.get("environment")
    if not isinstance(env, dict) or not isinstance(env.get("startedAt"), str):
        raise Malformed("the stroke-add file has no environment of its run")
    for key in ("os", "arch", "profile"):
        if env.get(key) != flatten.get(key):
            raise Malformed(f"the files differ in {key}: {env.get(key)!r} vs {flatten.get(key)!r}")
    if flatten.get("profile") != "release":
        raise Malformed("the gate is measured in release")
    series = strokes.get("series")
    if not isinstance(series, dict) or "strokeAdd.wallMs" not in series:
        raise Malformed("no strokeAdd.wallMs series")
    add = values(series["strokeAdd.wallMs"])
    unb_raw, bud_raw = flatten.get("unbudgeted"), flatten.get("budgeted")
    if not isinstance(unb_raw, list) or not isinstance(bud_raw, list):
        raise Malformed("unbudgeted and budgeted must be lists of observations")
    if len(add) < MIN_REPETITIONS or len(unb_raw) < MIN_REPETITIONS or len(bud_raw) < MIN_REPETITIONS:
        raise Malformed(f"fewer than {MIN_REPETITIONS} repetitions")
    unb = [record(r, "unbudgeted", i) for i, r in enumerate(unb_raw)]
    bud = [record(r, "budgeted", i) for i, r in enumerate(bud_raw)]
    all_completed = all(c for _, c, _, _ in unb)
    stored = flatten.get("unbudgetedAllCompleted")
    if stored is not None and stored is not all_completed:
        raise Malformed("unbudgetedAllCompleted disagrees with the records")
    for i, (_, completed, refused, _) in enumerate(bud):
        if not completed and not refused:
            raise Malformed(f"budgeted[{i}] is neither a completion nor an atomic refusal")
    add_p50 = p50(add) / 1000.0
    unb_seconds = [s for s, _, _, _ in unb]
    unb_p50 = p50(unb_seconds)
    met = all_completed and unb_p50 <= add_p50 and unb_p50 < WATCHDOG_SECONDS
    return {
        "gate": "C22-T2: 100k Flatten completes within the same run's 100k strokeAdd p50 and under the 30 s watchdog",
        "os": flatten.get("os"), "arch": flatten.get("arch"), "runStartedAt": env.get("startedAt"),
        "percentileRule": "upper nearest rank, as scripts/analyse-measurements.py",
        "strokeAddP50Seconds": add_p50, "flattenUnbudgetedP50Seconds": unb_p50,
        "flattenUnbudgetedSeconds": unb_seconds, "flattenUnbudgetedAllCompleted": all_completed,
        "flattenBudgetedSeconds": [s for s, _, _, _ in bud],
        "flattenBudgetedOutcomes": [("committed" if c else {"code": code}) for _, c, _, code in bud],
        "met": met, "gapSeconds": max(0.0, unb_p50 - add_p50),
        "note": "C22-T2 and the interactive latency budget (R1-12) are the M11 C11.1 scale gate",
    }


def main():
    if len(sys.argv) != 4:
        print(__doc__)
        return 2
    try:
        strokes = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
        flatten = json.loads(pathlib.Path(sys.argv[2]).read_text(encoding="utf-8"))
        out = evaluate(strokes, flatten)
        text = json.dumps(out, indent=2, allow_nan=False)
    except (OSError, ValueError) as e:
        print(f"malformed C22-T2 evidence: {e}")
        return 2
    pathlib.Path(sys.argv[3]).write_text(text + "\n", encoding="utf-8")
    print(json.dumps(out))
    return 0


if __name__ == "__main__":
    sys.exit(main())
