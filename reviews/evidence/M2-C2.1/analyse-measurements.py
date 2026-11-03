#!/usr/bin/env python3
"""Recomputes the C1.4 measurement summaries from their observations and rejects malformed evidence (C14-R1-07).

Usage: python scripts/analyse-measurements.py <dir-or-file> [...] [--out report.json]

Every measurement file written by `crates/executor/tests/c14_evidence` holds an `environment` (OS, architecture, CPUs,
profile, start time and the time origin of `tMs`) and `series`, each with its `samples` (`tMs`, `value`, optional
`label`) and a `summary`. This script recomputes every summary with the same nearest-rank rule
(`sorted[round((n - 1) * q)]`) and fails (exit 1) when a file is malformed, the series collection is missing or empty,
a series has no observations, a sample is not a finite number, times go backwards within a series, a stored summary
field has the wrong type (an integer `n`, finite numbers for the quantiles and `max`; C14-R3-04), or a stored summary
differs from the recomputed one (C14-R2-06). Files that hold only
facts (no percentiles) are listed separately and must be declared in SCALAR_ONLY.
"""
import json
import math
import pathlib
import sys

SCALAR_ONLY = {"cache-bounds", "sdk-conformance"}
QUANTILES = {"p50": 0.5, "p90": 0.9, "p99": 0.99}


def nearest(sorted_values, q):
    # Rust's f64::round rounds half away from zero; Python's round() rounds half to even.
    i = (len(sorted_values) - 1) * q
    return sorted_values[int(math.floor(i + 0.5))]


def summarise(values):
    v = sorted(values)
    if not v:
        return {"n": 0}
    out = {"n": len(v)}
    for k, q in QUANTILES.items():
        out[k] = nearest(v, q)
    out["max"] = v[-1]
    return out


def check_file(path):
    problems = []
    try:
        doc = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as e:
        return {"file": path.name, "ok": False, "problems": [f"not JSON: {e}"]}
    stem = path.stem.rsplit("-", 1)[0]
    if "series" not in doc:
        ok = stem in SCALAR_ONLY
        return {"file": path.name, "ok": ok, "kind": "facts",
                "problems": [] if ok else ["no series of observations (percentile evidence must keep its samples)"]}
    env = doc.get("environment")
    for key in ("os", "arch", "cpus", "profile", "startedAt", "timeOrigin"):
        if not isinstance(env, dict) or key not in env:
            problems.append(f"environment.{key} missing")
    series_out = {}
    series = doc.get("series")
    if not isinstance(series, dict) or not series:
        problems.append("series must be a non-empty object of named series")
        series = {}
    for name, s in series.items():
        if not isinstance(s, dict):
            problems.append(f"{name}: a series must be an object")
            continue
        samples = s.get("samples")
        if not isinstance(samples, list) or not samples:
            problems.append(f"{name}: no samples")
            continue
        values, last_t = [], None
        for i, x in enumerate(samples):
            if not isinstance(x, dict):
                problems.append(f"{name}[{i}]: a sample must be an object")
                continue
            t, v = x.get("tMs"), x.get("value")
            if not isinstance(t, (int, float)) or not isinstance(v, (int, float)) or isinstance(t, bool) or isinstance(v, bool):
                problems.append(f"{name}[{i}]: tMs and value must be numbers")
                continue
            if not math.isfinite(t) or not math.isfinite(v):
                problems.append(f"{name}[{i}]: tMs and value must be finite")
                continue
            if t < 0:
                problems.append(f"{name}[{i}]: negative tMs")
            if last_t is not None and t < last_t:
                problems.append(f"{name}[{i}]: tMs {t} goes back before {last_t} (observations are kept in time order)")
            values.append(float(v))
            last_t = float(t)
        recomputed = summarise(values)
        stored = s.get("summary")
        if not isinstance(stored, dict):
            problems.append(f"{name}: summary is not an object")
            stored = {}
        for k, v in recomputed.items():
            got = stored.get(k)
            # Type before value (C14-R3-04): `True == 1` in Python, so a boolean would otherwise pass as a count.
            if k == "n":
                if not isinstance(got, int) or isinstance(got, bool):
                    problems.append(f"{name}: summary.n={got!r} is not an integer")
                    continue
            elif not isinstance(got, (int, float)) or isinstance(got, bool) or not math.isfinite(got):
                problems.append(f"{name}: summary.{k}={got!r} is not a finite number")
                continue
            if got != v:
                problems.append(f"{name}: stored {k}={stored.get(k)} but the samples give {v}")
        series_out[name] = {"unit": s.get("unit"), **recomputed}
    return {"file": path.name, "ok": not problems, "kind": "series", "environment": env, "series": series_out,
            "problems": problems}


def main(argv):
    out = None
    if "--out" in argv:
        i = argv.index("--out")
        out = argv[i + 1]
        argv = argv[:i] + argv[i + 2:]
    if not argv:
        print(__doc__)
        return 2
    files = []
    for a in argv:
        p = pathlib.Path(a)
        files.extend(sorted(p.glob("*.json")) if p.is_dir() else [p])
    results = [check_file(f) for f in files]
    for r in results:
        status = "ok" if r["ok"] else "FAIL"
        print(f"{status:4} {r['file']} ({r.get('kind', '?')})")
        for name, s in r.get("series", {}).items():
            print(f"       {name}: n={s['n']} p50={s.get('p50')} p99={s.get('p99')} max={s.get('max')} {s.get('unit')}")
        for p in r["problems"]:
            print(f"       problem: {p}")
    if out:
        pathlib.Path(out).write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")
    bad = [r for r in results if not r["ok"]]
    print(f"{len(results)} file(s), {len(bad)} rejected")
    return 1 if bad or not results else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
