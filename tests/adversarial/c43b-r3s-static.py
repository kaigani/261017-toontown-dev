"""Read-only static regression probes; exit 1 while a finding reproduces."""
from pathlib import Path
import json
import re
ROOT = Path(__file__).resolve().parents[2]
failures = []
def check(name, ok):
    print(("PASS " if ok else "FAIL ") + name)
    if not ok: failures.append(name)
def read(path): return (ROOT / path).read_text(encoding="utf-8")
r = json.loads(read("desktop/src/ui-bindings/reads.json"))["surfaces"]
expected = {"About Toontown": "app.info.get", "Project Info": "project.info.get",
            "Check Project": "doc.validate", "Open Backup": "project.backup.list"}
check("S1 actual dialog read ownership", all(set(r["dialog:" + s]["reads"]) == {q} for s, q in expected.items()))
# The lifecycle accepts an unrelated settled load N+1 for a commit armed at N.
canvas = read("desktop/src/canvas/CanvasView.tsx")
life = read("desktop/src/canvas/gestureLifecycle.ts")
check("S2 commit identified beyond pre-release local counter",
      not ("version: before" in canvas and "settledVersion > c.version" in life))
flows = read("scripts/c43-d44-flows.py")
check("S5 Properties choice present in flow audit", "project.properties.set" in flows)
import importlib.util
import sys
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("assessment", ROOT / "scripts/c43_measure_assess.py")
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
def stat(n, value): return dict(n=n, p50=value, p95=value, max=value)
def scene():
    return dict(idle=stat(300,-1), idleWork=stat(300,-1), idleIntervals=[-1]*300,
                dragFrames=stat(300,-1), dragWork=stat(300,-1), dragIntervals=[-1]*300,
                feedback=stat(300,-1), commits=stat(30,-1), input="os", drags=30)
r = dict(requestedBackend="webgpu", backend="webgpu", fallback=None,
         control=dict(frameMs=stat(300,17.5),startScreen=True),
         sample=scene(), groups1k=dict(resident=1002,**scene()))
# Round-trip through strict standard JSON, no NaN or infinity.
r = json.loads(json.dumps(r, allow_nan=False))
bounds = dict(frameMs=17.5,dragFrameMs=17.5,feedbackMs=16.7,commitMs=50,minSamples=30)
check("S7 negative measured durations refused despite plausible control",
      any(not ok for ok, _, _ in m.assess(r,bounds,30)))
budget = read("crates/executor/tests/c32_budgets.rs")
check("S7 evaluation conversion includes a small-case shape assertion",
      not ('else { 35.0 }' in budget and not re.search(r'\bratio\b', budget.lower())))
deadline = read("crates/executor/tests/c34_round4.rs")
wrong_wait_ms, late_timestamp_ms, budget_ms = 1800, 2100, 1000
check("S7 wrong start-based wait cannot pass late caller timestamp",
      not ("(env, Instant::now())" in deadline and
           wrong_wait_ms < budget_ms + late_timestamp_ms / 2))
print("PASS source-only parser inspection recorded (desktop/node_modules absent)")
raise SystemExit(bool(failures))
