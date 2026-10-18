"""CLI-level R3 re-break receipts, including legacy/manual and failed-run controls."""
import copy
import gzip
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
original = json.loads(gzip.decompress((ROOT / "reviews/evidence/M0-C0.2/tablet-spike-windows-runner-r2-matrix5.json.gz").read_bytes()))
work = ROOT / "target/review-r3/attacks"
work.mkdir(parents=True, exist_ok=True)
cases = []
def add(name, mutate):
    log = copy.deepcopy(original)
    mutate(log)
    cases.append((name, log))
add("v1", lambda d: d.update(version=1))
add("versionless-synthetic", lambda d: d.pop("version"))
add("truncated-report-array", lambda d: d["runs"][0]["report"]["injectEpochMs"].pop())
add("tampered-achieved", lambda d: d["runs"][0]["report"].update(achievedHz=123456789))
add("tampered-endpoint", lambda d: d["runs"][0]["report"].update(contactEndNs=d["runs"][0]["report"]["contactEndNs"]+1000000000))
add("reversed-endpoints", lambda d: d["runs"][0]["report"].update(contactEndNs=0))
add("failed-run", lambda d: d.update(runs=[dict(label="refused",target="webview",error="spike.notForeground: fixture")],simulations=[]))
add("orphaned-simulations", lambda d: d.update(runs=[]))
add("missing-run-list", lambda d: d.pop("runs"))
add("empty-focus", lambda d: d["runs"][0]["report"].update(focus=[]))
for name in ("manual-page-log", "auto-refusal-page-log"):
    cases.append((name, json.loads((OUT / f"{name}.json").read_text(encoding="utf-8"))))
manual = copy.deepcopy(cases[-2][1])
manual.pop("version")
cases.append(("versionless-manual", manual))
results = []
for name, log in cases:
    path = work / f"{name}.json"
    path.write_text(json.dumps(log), encoding="utf-8")
    analysis = OUT / f"{name}.analysis.json" if "manual" in name or name=="auto-refusal-page-log" else work / f"{name}.analysis.json"
    p = subprocess.run([sys.executable,"scripts/analyse-tablet-log.py",str(path),"--json",str(analysis)],cwd=ROOT,capture_output=True,text=True,encoding="utf-8",env=dict(os.environ,PYTHONIOENCODING="utf-8"))
    (OUT / f"attack-{name}.txt").write_text(p.stdout+p.stderr,encoding="utf-8")
    results.append(dict(case=name,exit=p.returncode,stdoutBytes=len(p.stdout.encode()),stderr=p.stderr.strip(),outcome=[s for s in p.stdout.splitlines() if "outcome:" in s or "No synthetic runs" in s]))
    print(name,p.returncode,results[-1]["stderr"] or results[-1]["outcome"])
(OUT/"attacks.json").write_text(json.dumps(results,indent=2),encoding="utf-8")
