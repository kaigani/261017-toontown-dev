"""Recompute review evidence, historical receipt hashes, CI coverage, and test totals."""
from pathlib import Path
from collections import Counter
import hashlib
import json
import re
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]

def read(path):
    return path.read_text(encoding="utf-8", errors="replace")

def rust_totals(text):
    counts = [tuple(map(int, m)) for m in re.findall(r"test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored", text)]
    return dict(zip(("passed", "failed", "ignored"), map(sum, zip(*counts)))) if counts else {}

def tracked(path):
    return subprocess.check_output(["git", "show", "eb54b55:" + path], cwd=ROOT)

receipts = []
for f in sorted((HERE / "receipts").glob("*.json")):
    r = json.loads(read(f))
    actual = hashlib.sha256((HERE / r["log"]).read_bytes()).hexdigest()
    assert actual == r["sha256"], f
    receipts.append(dict(name=f.stem, command=r["command"], exit_code=r["exit_code"], elapsed_seconds=r["elapsed_seconds"], sha256=actual))

historical = []
prefix = "reviews/evidence/M1-C1.1-r2-response/"
for name, expected in json.loads(read(ROOT / prefix / "receipt.json"))["sha256"].items():
    data = tracked(prefix + name)
    historical.append(dict(file=prefix+name, expected=expected, actual=hashlib.sha256(data).hexdigest()))
for name, expected in {
    "rust-10mb-drawing-release.log": "80dfe26e72ad72c35ca85be9a2de3456c3ab0b7a7b78482a7e245b55b8fe59ff",
    "ts-10mb-drawing-node.log": "f4ee7b610fc9040bb7c9cc8b65afaa3d51cde3f21dfcf175a188b90d36ccd0e7",
    "gates/test.log": "46264458dfbb64b30f410b1c78c4391e75a4d89f03f0cec9dd128597393c984f",
    "gates/pnpm.log": "596549d761ac0ac3f7da6c4df7f7ef9aa5ba35ff3b80f8acb8aed0642cdaa7b7",
}.items():
    path = "reviews/evidence/M1-C1.1-r1-response/" + name
    historical.append(dict(file=path, expected=expected, actual=hashlib.sha256(tracked(path)).hexdigest()))

ci = json.loads(read(HERE / "logs/ci-metadata.log"))
assert ci["headSha"] == "eb54b55b44d6e744ab5f2b0d74b4213b604798f3"
ci_log = read(HERE / "logs/ci-full.log")
jobs = []
for job in ci["jobs"]:
    lines = [l for l in ci_log.splitlines() if l.startswith(job["name"] + "\t")]
    text = re.sub(r"(?:\x1b|\^\[)\[[0-9;]*m", "", "\n".join(lines))
    evidence = [l for l in text.splitlines() if any(p in l for p in (
        "Test Files", "Tests ", "# pass ", "test result:", "Running tests", "c11-r1-adversarial", "c11-r2-adversarial",
        "IPC boundary", "advisories ok", "Headless dependency", "headless dependencies", "all emitted", "sha256",
        "no display", "DISPLAY", "--selftest", "diagnostic command", "check-diagnostics-gating", "c11-r2-review-policy"))]
    (HERE / ("ci-" + re.sub(r"[^a-z0-9]+", "-", job["name"].lower()).strip("-") + ".txt")).write_text("\n".join(evidence) + "\n", encoding="utf-8")
    workspace_text = "\n".join(l for l in lines if "\tRun cargo test --workspace --locked\t" in l)
    jobs.append(dict(name=job["name"], conclusion=job["conclusion"], steps=[dict(name=s["name"], conclusion=s["conclusion"]) for s in job["steps"]], rust_test_summaries=rust_totals(text), workspace_rust=rust_totals(workspace_text), vitest=re.findall(r"Tests\s+\d+ passed[^\n]*", text), node=re.findall(r"# pass \d+", text)))

historical_totals = {}
for name in ["M1-C1.1-r1-response", "M1-C1.1-r2-response"]:
    historical_totals[name] = rust_totals(read(ROOT / "reviews/evidence" / name / "gates/test.log"))

local_totals = {}
for name in ["baseline-rust", "rust-final", "final-workspace", "rust-diagnostics"]:
    path = HERE / "logs" / (name+".log")
    if path.exists(): local_totals[name] = rust_totals(read(path))

scale = [json.loads(read(ROOT / prefix / "measurements" / ("scale-" + str(i) + ".json")))["measurements"] for i in [1,2]]
result = dict(base=ci["headSha"], receipts=receipts, historical_hashes=historical,
              historical_hash_mismatches=[h for h in historical if h["expected"]!=h["actual"]],
              historical_rust_totals=historical_totals, scale_measurements=scale,
              ci=dict(run=ci["databaseId"], url=ci["url"], conclusion=ci["conclusion"], jobs=jobs), local_rust_totals=local_totals)
old_suites={}
for chunk in re.split(r"(?=\s+Running tests[\\/])",read(HERE/"logs/baseline-rust.log")):
    match=re.search(r"Running tests[\\/](c11_r[12]_(?:adversarial|wire))\.rs",chunk)
    if match: old_suites[match[1]]=rust_totals(chunk.split("Doc-tests")[0])
result["old_reviewer_cargo_suites"]=old_suites
result["ci_python_policy_test_executed"]="c11-r2-review-policy.py" in ci_log
for name in ["headless-1", "headless-2"]:
    path = HERE / "logs" / (name+".log")
    if path.exists() and (HERE / "receipts" / (name+".json")).exists():
        result[name] = [(c["id"], c["ok"]) for c in json.loads(read(path))["checks"]]
if "headless-2" in result:
    assert result["headless-1"] == result["headless-2"]
    assert all(ok for _,ok in result["headless-1"])
(HERE / "audit.json").write_text(json.dumps(result, ensure_ascii=False, indent=2)+"\n", encoding="utf-8")
print(json.dumps(dict(receipts=len(receipts), historical_hash_mismatches=result["historical_hash_mismatches"], historical_rust_totals=historical_totals, local_rust_totals=local_totals, ci=[dict(name=j["name"], result=j["conclusion"], rust=j["rust_test_summaries"], vitest=j["vitest"], node=j["node"]) for j in jobs]), indent=2))
