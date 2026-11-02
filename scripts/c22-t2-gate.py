#!/usr/bin/env python3
"""The C22-T2 completion gate (orchestrator ruling on the M2 C2.4 plan), from raw observations of one CI run.

Usage: python scripts/c22-t2-gate.py <strokes-100k-<os>.json> <flatten-100k-<os>.json> <out.json>

A 100 000-stroke Flatten counts as completed when its unbudgeted release wall time (p50 of at least 5 repetitions)
is at most the same run's 100k `strokeAdd` p50 and under the 30 s watchdog, and every unbudgeted repetition
committed. The product's budgeted Flatten is reported beside it (refused before the watchdog while the merge exceeds
its work budget). This is the C22-T2 gate only; the interactive latency budget (R1-12) is the M11 C11.1 scale gate.
Always exits 0 (the verdict is evidence, reported in the handoff); malformed input exits 2.
"""
import json
import sys


def values(series):
    if isinstance(series, list):
        return [float(x) for x in series]
    if isinstance(series, dict):
        for key in ("values", "samples", "observations"):
            if isinstance(series.get(key), list):
                return [float(x["value"]) if isinstance(x, dict) else float(x) for x in series[key]]
    raise ValueError("unreadable series")


def p50(v):
    s = sorted(v)
    return s[(len(s) - 1) // 2]


def main():
    if len(sys.argv) != 4:
        print(__doc__)
        return 2
    strokes = json.load(open(sys.argv[1], encoding="utf-8"))
    flatten = json.load(open(sys.argv[2], encoding="utf-8"))
    add = values(strokes["series"]["strokeAdd.wallMs"])
    unb = [float(u["seconds"]) for u in flatten["unbudgeted"]]
    bud = [float(b["seconds"]) for b in flatten["budgeted"]]
    if len(add) < 5 or len(unb) < 5:
        print("fewer than 5 repetitions")
        return 2
    add_p50 = p50(add) / 1000.0
    unb_p50 = p50(unb)
    met = unb_p50 <= add_p50 and unb_p50 < 30.0 and flatten.get("unbudgetedAllCompleted") is True
    out = {
        "gate": "C22-T2: 100k Flatten completes within the same run's 100k strokeAdd p50 and under the 30 s watchdog",
        "os": flatten.get("os"), "strokeAddP50Seconds": add_p50, "flattenUnbudgetedP50Seconds": unb_p50,
        "flattenUnbudgetedSeconds": unb, "flattenBudgetedSeconds": bud,
        "flattenBudgetedOutcomes": [b.get("error", {}) or "committed" for b in flatten["budgeted"]],
        "met": met, "gapSeconds": max(0.0, unb_p50 - add_p50),
        "note": "the interactive latency budget (R1-12) is the separate M11 C11.1 scale gate",
    }
    json.dump(out, open(sys.argv[3], "w", encoding="utf-8"), indent=2)
    print(json.dumps(out))
    return 0


if __name__ == "__main__":
    sys.exit(main())
