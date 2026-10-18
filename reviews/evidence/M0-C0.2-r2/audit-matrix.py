"""Independent audit: no imports from production analysis, no saved summaries used.

Run from repository root: python reviews/evidence/M0-C0.2-r2/audit-matrix.py
"""
import gzip
import hashlib
import json
import math
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent


def quantile(xs, p):
    return sorted(xs)[max(0, math.ceil(len(xs) * p / 100) - 1)] if xs else None


def stats(xs):
    return {str(p): quantile(xs, p) for p in (50, 95, 99)} | {"max": max(xs) if xs else None}


def f32(n):
    return struct.unpack("<f", struct.pack("<f", n))[0]


def indices(values, report, denominator):
    # Match against the actual injected pressures, including float32 rounding in RTS.
    expected = {f32(p / denominator): i for i, p in enumerate(report["pressure"])}
    # serde_json -> browser JSON stringify -> Python can differ by a double ULP;
    # restore the original f32 before comparison to the expected f32.
    return {expected[f32(v)] for v in values if f32(v) in expected}


def audit(path):
    raw = gzip.decompress(path.read_bytes())
    log = json.loads(raw)
    output = {"file": path.name, "sha256": hashlib.sha256(raw).hexdigest(),
              "environment": log["environment"], "runs": []}
    for run in log["runs"]:
        probe = run.get("probe") or {}
        report = run.get("report") or probe.get("report")
        if report is None:
            output["runs"].append({"label": run["label"], "error": run.get("error") or probe.get("error")})
            continue
        expected = set(range(report["frames"]))
        repeated = [i for i in range(1, report["frames"])
                    if (report["screenX"][i], report["screenY"][i]) ==
                    (report["screenX"][i - 1], report["screenY"][i - 1])]
        times = report["injectEpochMs"]
        row = {"label": run["label"], "request": run["request"], "screenRect": report["screenRect"],
               "injectorHzFirstToLast": (len(times) - 1) * 1000 / (times[-1] - times[0]),
               "reportedAchievedHz_UNVERIFIED": report["achievedHz"],
               "injectCallMs": stats([b - a for a, b in zip(times, report["injectReturnEpochMs"])]),
               "repeatedPixelFrames": repeated,
               "varyingAxes": {k: len(set(report[k])) for k in ("pressure", "tiltX", "tiltY", "rotation")}}
        raw_input = (probe.get("listeners") or (run.get("rawInput") or {}).get("listeners") or {}).get("rawInput")
        if raw_input:
            row["raw"] = {k: v for k, v in raw_input.items() if k != "reports"}
            row["raw"]["savedReports"] = len(raw_input.get("reports", []))
            row["raw"]["savedBytes"] = sum(len(bytes.fromhex(s)) for s in raw_input.get("reports", []))
        if run["target"] == "webview":
            samples = log["web"]["samples"][slice(*run["webRange"])]
            points = [s for s in samples if s["kind"] != "up" and s["pressure"] > 0]
            ids = indices([s["pressure"] for s in points], report, 1024)
            ts = sorted(s["t"] for s in points)
            intervals = [b - a for a, b in zip(ts, ts[1:])]
            pres = sorted({s["pressure"] for s in points})
            first = {}
            for s in points:
                k = int(s["pressure"] * 1024) - 1
                first.setdefault(k, s)
            origin = log["environment"]["timeOrigin"]
            row["web"] = {
                "received": len(ids), "missing": sorted(expected - ids),
                "repeatedMissing": sorted(set(repeated) - ids),
                "rateHz": (len(ts) - 1) * 1000 / (ts[-1] - ts[0]) if len(ts) > 1 else None,
                "intervalMs": stats(intervals), "pressureDistinct": len(pres),
                "pressureImplied": 1 / min(b - a for a, b in zip(pres, pres[1:])) if len(pres) > 1 else None,
                "exactTiltMatches": sum(s["tiltX"] == report["tiltX"][k] and s["tiltY"] == report["tiltY"][k] for k, s in first.items()),
                "exactRotationMatches": sum(s["twist"] == report["rotation"][k] for k, s in first.items()),
                "eventLatencyMs": stats([origin + s["t"] - times[k] for k, s in first.items()]),
                "handlerLatencyMs": stats([origin + s["handlerT"] - times[k] for k, s in first.items()]),
                "allKinds": {kind: sum(s["kind"] == kind for s in samples) for kind in ("down", "move", "up")},
            }
            row["nativePackets"] = len(log["native"]["packets"][slice(*run["nativeRange"])])
        else:
            listeners = probe["listeners"]
            wm = listeners["wmPointer"]
            wm_ids = indices([e["pressure"] / 1024 for e in wm["entries"] if e["inContact"]], report, 1024)
            row["wmPointer"] = {k: v for k, v in wm.items() if k != "entries"}
            row["wmPointer"].update(received=len(wm_ids), missing=sorted(expected - wm_ids))
            if listeners["rts"]:
                pressures = listeners["rts"]["pressures"]
                fits = {scale: indices(pressures, report, scale) for scale in (1024, 1025)}
                row["rts"] = {"exactFloat32Matches": {str(k): len(v) for k, v in fits.items()},
                              "missingAt1025": sorted(expected - fits[1025]),
                              "unionWithWM": len(wm_ids | fits[1025])}
        output["runs"].append(row)
    # Whole-log estimators, including hover/down/up, same scope as the handoff.
    dispatches = {}
    for s in log["web"]["samples"]:
        key = ("move", s["dispatch"]) if s["kind"] == "move" else (s["kind"], s["t"], s["dispatch"])
        dispatches.setdefault(key, []).append(s)
    oldest, newest, every, counts = [], [], [], []
    for key, group in dispatches.items():
        ordered = sorted(group, key=lambda s: s["t"])
        oldest.append(ordered[0]["handlerT"] - ordered[0]["t"])
        newest.append(ordered[-1]["handlerT"] - ordered[-1]["t"])
        every.extend(s["handlerT"] - s["t"] for s in group)
        if key[0] == "move":
            counts.append(group[0]["coalescedCount"])
    output["handlerLatencyMs"] = {"oldest": stats(oldest), "newest": stats(newest), "all": stats(every)}
    output["coalescedCount"] = stats(counts)
    return output


if __name__ == "__main__":
    paths = sorted((ROOT / "reviews/evidence/M0-C0.2").glob("*matrix*.gz"))
    if (OUT / "tablet-spike-final-ci.json.gz").exists():
        paths.append(OUT / "tablet-spike-final-ci.json.gz")
    results = [audit(p) for p in paths]
    (OUT / "independent-matrix.json").write_text(json.dumps(results, indent=2), encoding="utf-8")
    for result in results:
        print(result["file"], result["handlerLatencyMs"])
        for row in result["runs"]:
            print(row["label"], "rect", row.get("screenRect"), "web", row.get("web", {}).get("received"),
                  "WM", row.get("wmPointer", {}).get("received"), "RTS", row.get("rts"),
                  "raw", row.get("raw", {}).get("hidReports"))
