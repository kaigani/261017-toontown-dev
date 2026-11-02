#!/usr/bin/env python3
"""The C22-T2 completion gate (orchestrator ruling on the M2 C2.4 plan), from raw observations of one CI run.

Usage: python scripts/c22-t2-gate.py <strokes-100k-<os>.json> <flatten-100k-<os>.json> <out.json>

A 100 000-stroke Flatten counts as completed when its unbudgeted release wall time (p50 of at least 5 repetitions)
is at most the same run's 100k `strokeAdd` p50 and under the 30 s watchdog, and every unbudgeted repetition
completed. The product's budgeted Flatten is reported beside it (refused before the watchdog while the merge exceeds
its work budget). This is the C22-T2 gate only; C22-T2 and the interactive latency budget (R1-12) are the M11 C11.1
scale gate (accepted re-scope, M2 C2.4 round 1).

Validation (C24-R1-03, C24-R2-03): both files must have the documented shape; every duration is a finite,
non-negative number (booleans are not numbers; a number too large for a float is malformed too); at least 5 stroke-add
samples and 5 observations of each Flatten kind.

- Environment: `os`, `arch` and `profile` are present non-empty strings in both files (two missing values are not a
  match) and equal; the profile is `release`.
- One run: both files carry the same non-empty `runId` (`environment.runId`, `runId`), and when `TT_EXPECT_RUN_ID` is
  set (CI: `<GITHUB_RUN_ID>-<attempt>-<os>`) it must be that value. Both carry `startedAt` (ISO 8601 with a zone); the
  Flatten measurement starts after the stroke-add measurement and within 6 hours of it (the same job).
- Identity: every Flatten record has an integer `sampleId`, distinct within its series; stroke-add samples given as
  objects are distinct `(tMs, value)` observations. A duplicated record is malformed (equal durations alone are
  legitimate).
- Outcomes, for both series: a record is either a completion (status `Ok`, no error, committed) or an atomic refusal
  (status `Error`, an error code, not committed, document unchanged); any other combination is malformed. A stored
  `unbudgetedAllCompleted` must agree with the records.

The percentile is the one rule of `scripts/analyse-measurements.py` (upper nearest rank, `sorted[round((n - 1) * q)]`
with halves rounded up).

Exit status: 0 with the verdict written (a valid missed gate is evidence: `met: false`); 2 on malformed input, with
nothing written.
"""
import datetime
import importlib.util
import json
import math
import os
import pathlib
import sys

_SPEC = importlib.util.spec_from_file_location(
    "analyse_measurements", pathlib.Path(__file__).resolve().parent / "analyse-measurements.py"
)
_ANALYSER = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_ANALYSER)

WATCHDOG_SECONDS = 30.0
MIN_REPETITIONS = 5
SAME_JOB = datetime.timedelta(hours=6)


class Malformed(ValueError):
    """The evidence does not have the documented shape."""


def number(x, what):
    """A finite, non-negative duration (booleans are rejected)."""
    if isinstance(x, bool) or not isinstance(x, (int, float)):
        raise Malformed(f"{what} is not a number: {x!r}")
    try:
        x = float(x)
    except OverflowError:
        raise Malformed(f"{what} is too large for a duration") from None
    if not math.isfinite(x) or x < 0.0:
        raise Malformed(f"{what} is not a finite non-negative duration: {x!r}")
    return x


def values(series):
    """The observations of a series: a list of numbers, or `{samples|values|observations: [...]}`; samples given as
    objects must be distinct `(tMs, value)` observations."""
    if isinstance(series, list):
        items = series
    elif isinstance(series, dict):
        items = next((series[k] for k in ("samples", "values", "observations") if isinstance(series.get(k), list)), None)
        if items is None:
            raise Malformed("unreadable series")
    else:
        raise Malformed("unreadable series")
    out, seen = [], set()
    for x in items:
        v = number(x["value"] if isinstance(x, dict) and "value" in x else x, "a stroke-add sample")
        if isinstance(x, dict):
            t = number(x.get("tMs"), "a stroke-add sample's tMs")
            if (t, v) in seen:
                raise Malformed(f"duplicated stroke-add sample at tMs {t}")
            seen.add((t, v))
        out.append(v)
    return out


def text(x, what):
    """A present, non-empty string."""
    if not isinstance(x, str) or not x:
        raise Malformed(f"{what} is missing or not a string: {x!r}")
    return x


def instant(x, what):
    """An ISO 8601 timestamp with a zone."""
    x = text(x, what)
    try:
        t = datetime.datetime.fromisoformat(x.replace("Z", "+00:00"))
    except ValueError:
        raise Malformed(f"{what} is not an ISO 8601 timestamp: {x!r}") from None
    if t.tzinfo is None:
        raise Malformed(f"{what} has no time zone: {x!r}")
    return t


def p50(v):
    """The p50 by the analyser's one nearest-rank rule."""
    if not v:
        raise Malformed("no observations")
    return _ANALYSER.nearest(sorted(v), 0.5)


def record(r, kind, i):
    if not isinstance(r, dict):
        raise Malformed(f"{kind}[{i}] is not an object")
    seconds = number(r.get("seconds"), f"{kind}[{i}].seconds")
    sample = r.get("sampleId")
    if isinstance(sample, bool) or not isinstance(sample, int) or sample < 0:
        raise Malformed(f"{kind}[{i}].sampleId is not a non-negative integer: {sample!r}")
    error = r.get("error")
    code = error.get("code") if isinstance(error, dict) else None
    if error is not None and not isinstance(code, str):
        raise Malformed(f"{kind}[{i}].error has no code")
    committed = r.get("committed")
    if not isinstance(committed, bool):
        raise Malformed(f"{kind}[{i}].committed is not a boolean")
    status = r.get("status")
    completed = status == "Ok" and error is None and committed
    refused = status == "Error" and code is not None and not committed and r.get("unchangedOnRefusal") is True
    if not completed and not refused:
        raise Malformed(
            f"{kind}[{i}] is neither a completion nor an atomic refusal: status {status!r}, error {error!r}, "
            f"committed {committed!r}, unchangedOnRefusal {r.get('unchangedOnRefusal')!r}"
        )
    return seconds, completed, code, sample


def distinct(records, kind):
    ids = [r[3] for r in records]
    if len(set(ids)) != len(ids):
        raise Malformed(f"duplicated {kind} records (sampleId {sorted(ids)})")


def evaluate(strokes, flatten):
    if not isinstance(strokes, dict) or not isinstance(flatten, dict):
        raise Malformed("both files must be JSON objects")
    env = strokes.get("environment")
    if not isinstance(env, dict):
        raise Malformed("the stroke-add file has no environment of its run")
    for key in ("os", "arch", "profile"):
        a, b = text(env.get(key), f"stroke-add environment.{key}"), text(flatten.get(key), f"Flatten {key}")
        if a != b:
            raise Malformed(f"the files differ in {key}: {a!r} vs {b!r}")
    if flatten.get("profile") != "release":
        raise Malformed("the gate is measured in release")
    run = text(env.get("runId"), "stroke-add environment.runId")
    if text(flatten.get("runId"), "Flatten runId") != run:
        raise Malformed(f"the files come from different runs: {run!r} vs {flatten.get('runId')!r}")
    expected = os.environ.get("TT_EXPECT_RUN_ID")
    if expected and run != expected:
        raise Malformed(f"the evidence is from run {run!r}, not this run {expected!r}")
    began = instant(env.get("startedAt"), "stroke-add environment.startedAt")
    flat_began = instant(flatten.get("startedAt"), "Flatten startedAt")
    if not began <= flat_began <= began + SAME_JOB:
        raise Malformed(f"contradictory start times: stroke-add {began.isoformat()}, Flatten {flat_began.isoformat()}")
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
    distinct(unb, "unbudgeted")
    distinct(bud, "budgeted")
    all_completed = all(c for _, c, _, _ in unb)
    stored = flatten.get("unbudgetedAllCompleted")
    if stored is not None and stored is not all_completed:
        raise Malformed("unbudgetedAllCompleted disagrees with the records")
    add_p50 = p50(add) / 1000.0
    unb_seconds = [s for s, _, _, _ in unb]
    unb_p50 = p50(unb_seconds)
    met = all_completed and unb_p50 <= add_p50 and unb_p50 < WATCHDOG_SECONDS
    return {
        "gate": "C22-T2: 100k Flatten completes within the same run's 100k strokeAdd p50 and under the 30 s watchdog",
        "os": flatten.get("os"), "arch": flatten.get("arch"), "runId": run, "runStartedAt": env.get("startedAt"),
        "flattenStartedAt": flatten.get("startedAt"),
        "percentileRule": "upper nearest rank, as scripts/analyse-measurements.py",
        "strokeAddP50Seconds": add_p50, "flattenUnbudgetedP50Seconds": unb_p50,
        "flattenUnbudgetedSeconds": unb_seconds, "flattenUnbudgetedAllCompleted": all_completed,
        "flattenBudgetedSeconds": [s for s, _, _, _ in bud],
        "flattenBudgetedOutcomes": [("committed" if c else {"code": code}) for _, c, code, _ in bud],
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
    except (OSError, ValueError, OverflowError) as e:
        print(f"malformed C22-T2 evidence: {e}")
        return 2
    pathlib.Path(sys.argv[3]).write_text(text + "\n", encoding="utf-8")
    print(json.dumps(out))
    return 0


if __name__ == "__main__":
    sys.exit(main())
