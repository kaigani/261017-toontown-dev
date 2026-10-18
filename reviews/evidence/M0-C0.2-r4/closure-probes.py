"""Scoped CLI closure probes; no product imports or committed fixture mutations."""
import copy
import gzip
import itertools
import json
import os
from pathlib import Path
import subprocess
import struct
import sys

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[2]
WORK = ROOT / "target/review-r4/probes"
WORK.mkdir(parents=True, exist_ok=True)
ENV = dict(os.environ, PYTHONIOENCODING="utf-8")
original = json.loads(gzip.decompress((ROOT / "reviews/evidence/M0-C0.2/tablet-spike-windows-runner-r2-matrix5.json.gz").read_bytes()))
cases = []

def add(name, mutate, expected=2, message=None):
    log = copy.deepcopy(original)
    mutate(log)
    cases.append((name, log, expected, message))

def manual(log):
    log.update(source="manual", runs=[], simulations=[])
    log["environment"]["mode"]["auto"] = False

add("intact-matrix5-no-source", lambda d: None, 0, "10 synthetic runs, 10 ok")
add("manual-retains-simulations", lambda d: d.update(source="manual", runs=[]), message="simulation entries")
add("auto-empty-runs-retains-sims", lambda d: d.update(source="auto", runs=[]), message="simulation entries")
add("auto-empty-runs-no-sims", lambda d: d.update(source="auto", runs=[], simulations=[]), message="--spike-auto")
add("unknown-source-with-runs", lambda d: d.update(source="unexpected"), message="unknown log source")
add("manual-with-runs", lambda d: d.update(source="manual"), message="contains synthetic runs")
add("manual-auto-environment", lambda d: d.update(source="manual", runs=[], simulations=[]), message="--spike-auto")
add("declared-manual", manual, 0, "manual/hardware log; loss unknown")
for version in (2, 3):
    def no_source(d, version=version):
        manual(d)
        d.pop("source")
        d["version"] = version
    add(f"v{version}-manual-without-source", no_source, message="does not declare")
    for absent in (False, True):
        def orphan(d, version=version, absent=absent):
            d["version"] = version
            if absent:
                d.pop("runs")
            else:
                d["runs"] = []
        add(f"v{version}-orphan-runs-{'absent' if absent else 'empty'}", orphan, message="simulation entries")

def focus_case(d, focus):
    # Remove the duplicate summaries so they cannot mask focus validation.
    d["simulations"] = []
    d["runs"][0]["report"]["focus"] = focus

focus = original["runs"][0]["report"]["focus"]
add("v3-focus-empty", lambda d: focus_case(d, []), message="focus snapshot")
add("v3-focus-one", lambda d: focus_case(d, focus[:1]), message="focus snapshot")
add("v3-focus-two-missing-fields", lambda d: focus_case(d, [{"when":"start"},{"when":"end"}]), message="lacks valid fields")
add("v3-focus-duplicate-role", lambda d: focus_case(d, [focus[0],focus[0]]), message="focus snapshot")
for field in ("ownPid", "targetHwnd", "foregroundHwnd", "foregroundPid", "focusHwnd", "focusPid", "activeHwnd", "captureHwnd", "foregroundClass", "focusClass"):
    damaged = copy.deepcopy(focus)
    damaged[0].pop(field)
    add(f"v3-focus-missing-{field}", lambda d, f=damaged: focus_case(d, f), message="lacks valid fields")
def v2_focus(d):
    d["version"] = 2
    focus_case(d, [])
add("v2-focus-empty-control", v2_focus, 0, "version 2 log; focus snapshots were added in version 3")

legacy = json.loads((ROOT / "reviews/evidence/M0-C0.2-r3/manual-page-log.json").read_text(encoding="utf-8"))
for key in ("version", "source", "runs", "simulations"):
    legacy.pop(key, None)
cases.append(("versionless-pure-manual", legacy, 0, "manual/hardware log; loss unknown"))
for field, value in (("runs", []), ("simulations", []), ("source", "manual"), ("runs", original["runs"])):
    bad = copy.deepcopy(legacy)
    bad[field] = value
    cases.append((f"versionless-{field}-{'populated' if value else 'empty'}", bad, 2, "no `version` field"))
bad = copy.deepcopy(legacy)
bad["environment"]["mode"]["auto"] = True
cases.append(("versionless-auto-flag", bad, 2, "no `version` field"))

def invoke(name, log):
    path = WORK / f"{name}.json"
    path.write_text(json.dumps(log), encoding="utf-8")
    result_path = WORK / f"{name}.analysis.json"
    result_path.unlink(missing_ok=True)
    r = subprocess.run([sys.executable, "scripts/analyse-tablet-log.py", str(path), "--json", str(result_path)], cwd=ROOT, capture_output=True, text=True, encoding="utf-8", env=ENV)
    (OUT / f"analyzer-{name}.txt").write_text(r.stdout+r.stderr, encoding="utf-8")
    return r, json.loads(result_path.read_text()) if result_path.exists() else None

results = []
for name, log, expected, message in cases:
    r, analysis = invoke(name, log)
    row = dict(case=name, exit=r.returncode, expected=expected, stdoutBytes=len(r.stdout.encode()), stderr=r.stderr.strip())
    row["pass"] = r.returncode == expected and (expected == 0 or not r.stdout) and (not message or message in r.stdout+r.stderr)
    results.append(row)
    print(name, row["pass"], r.returncode)
(OUT / "closure-probes.json").write_text(json.dumps(results, indent=2), encoding="utf-8")
assert all(r["pass"] for r in results)

# Independent group construction from R3's raw missing sets; compare full arrays AND printed text.
prior = json.loads((ROOT / "reviews/evidence/M0-C0.2-r3/independent-matrix.json").read_text())
range_rows = []
rts_rows = []
for matrix in prior:
    n = matrix["matrix"]
    log = json.loads(gzip.decompress((ROOT / f"reviews/evidence/M0-C0.2/tablet-spike-windows-runner-r2-matrix{n}.json.gz").read_bytes()))
    r, analysis = invoke(f"matrix{n}", log)
    assert r.returncode == 0, r.stderr
    for old, current in zip(matrix["runs"], analysis["runs"], strict=True):
        assert old["label"] == current["label"]
        for channel in ("web", "wmPointer"):
            if channel not in old:
                continue
            missing = old[channel]["missing"]
            # Independently recompute frame identities from the original raw streams as well.
            raw_run = next(x for x in log["runs"] if x["label"] == old["label"])
            if channel == "web":
                stream = log["web"]["samples"][slice(*raw_run["webRange"])]
                seen = {round(s["pressure"]*1024)-1 for s in stream if s["pressure"] > 0 and s["kind"] != "up"}
                total = raw_run["report"]["frames"]
            else:
                stream = raw_run["probe"]["listeners"]["wmPointer"]["entries"]
                seen = {s["pressure"]-1 for s in stream if s["inContact"]}
                total = raw_run["probe"]["report"]["frames"]
            assert sorted(set(range(total))-seen) == missing
            groups = [[x[1] for x in g] for _, g in itertools.groupby(enumerate(missing), key=lambda p:p[1]-p[0])]
            ranges = sorted([[g[0], g[-1], len(g)] for g in groups], key=lambda g:(-g[2],g[0]))
            assert current[channel]["missingFrames"] == missing
            assert current[channel]["missingRuns"] == ranges
            limit = 5 if channel == "web" else 3
            shown = ", ".join(f"{a}-{b} ({c})" if c > 1 else f"{a} (1)" for a,b,c in ranges[:limit]) or "none"
            if len(ranges) > limit:
                shown += f", ... {len(ranges)-limit} more"
            prefix = "longest runs of consecutive missing frames (first-last (count)): " if channel == "web" else "(longest runs "
            line = next(s for s in r.stdout.splitlines() if s.startswith(f"- **{old['label']}**:") and prefix in s)
            assert prefix + shown in line, (n, old['label'], channel, shown, line)
            full = ", ".join(str(g[0]) if len(g)==1 else f"{g[0]}-{g[-1]}" for g in groups) or "none"
            name = "web" if channel == "web" else "WM_POINTER"
            assert f"{name} missing frames: {full}" in line
            range_rows.append(dict(matrix=n, label=old["label"], channel=channel, missingCount=len(missing), ranges=ranges, printed=shown))
        if "rts" in old:
            missing = old["rts"]["missingAt1025"]
            raw_run = next(x for x in log["runs"] if x["label"] == old["label"])
            raw = raw_run["probe"]
            f32 = lambda value: struct.unpack('f', struct.pack('f', value))[0]
            expected = {f32(p/1025): i for i,p in enumerate(raw["report"]["pressure"])}
            seen = {expected[f32(p)] for p in raw["listeners"]["rts"]["pressures"] if f32(p) in expected}
            assert sorted(set(range(raw["report"]["frames"]))-seen) == missing
            assert current["rts"]["frames"]["missingFrames"] == missing
            groups = [[x[1] for x in g] for _, g in itertools.groupby(enumerate(missing), key=lambda p:p[1]-p[0])]
            full = ", ".join(str(g[0]) if len(g)==1 else f"{g[0]}-{g[-1]}" for g in groups) or "none"
            line = next(s for s in r.stdout.splitlines() if s.startswith(f"- **{old['label']}**:"))
            assert f"RTS missing frames (pressure scale 1025): {full}" in line
            rts_rows.append(dict(matrix=n,label=old['label'],missing=missing,printed=full))
(OUT / "range-audit.json").write_text(json.dumps(range_rows, indent=2), encoding="utf-8")
(OUT / "rts-range-audit.json").write_text(json.dumps(rts_rows, indent=2), encoding="utf-8")
first = next(r for r in range_rows if r["matrix"]==5 and r["label"]=="webview sine 240 Hz")
assert first["ranges"][:3] == [[906,1004,99],[867,904,38],[1006,1022,17]]
handoff = (ROOT / "reviews/M0-C0.2-handoff.md").read_text(encoding="utf-8")
assert "154" in handoff and "905" in handoff and "1005" in handoff
print(f"PASS: {len(cases)} classification/focus probes, {len(range_rows)} independent web/WM_POINTER range comparisons, {len(rts_rows)} RTS missing-range comparisons")
