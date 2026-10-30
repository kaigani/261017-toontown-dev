"""Recompute reviewer statistics and verify baseline CI coverage and artifact provenance."""
import collections
import hashlib
import json
import pathlib
import re
import subprocess

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def read(path):
    data = path.read_bytes()
    return data.decode("utf-16" if data.startswith((b'\xff\xfe', b'\xfe\xff')) else "utf-8-sig", errors="replace")


def stats(values):
    values = sorted(values)
    return {"n": len(values), "p50": values[round((len(values)-1)*.5)],
            "p99": values[round((len(values)-1)*.99)], "max": values[-1]} if values else None


report = {}
ci = json.loads(read(HERE / "ci.json"))
log = re.sub(r"(?:\x1b|\^\[)\[[0-9;]*m", "", read(HERE / "ci.log"))
report["ci"] = {"head": ci["headSha"], "conclusion": ci["conclusion"], "jobs": {}}
baseline_paths = subprocess.check_output(["git", "ls-tree", "-r", "--name-only", "3906001"], cwd=ROOT, text=True).splitlines()
python_suites = [p for p in baseline_paths if p.startswith("tests/adversarial/") and p.endswith(".py") and not p.endswith("collect_runtime_evidence.py")]
rust_suites = [p for p in baseline_paths if re.search(r"/tests/c\d+_r\d+[^/]*\.rs$", p)]
for job in ci["jobs"]:
    name = job["name"]
    if not (name.startswith("check (") or name == "headless without a display (ubuntu)"):
        continue
    lines = [line for line in log.splitlines() if line.startswith(name + "\t")]
    text = "\n".join(lines)
    results = re.findall(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored", text)
    report["ci"]["jobs"][name] = {
        "conclusion": job["conclusion"], "durationSeconds": (str(job["completedAt"]), str(job["startedAt"])),
        "successfulSteps": [s["name"] for s in job["steps"] if s["conclusion"] == "success"],
        "rustSummarySumAllSteps": [sum(int(r[i]) for r in results) for i in range(3)],
        "workspaceSummarySum": [sum(int(r[i]) for r in re.findall(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored", "\n".join(l for l in lines if "\tRun cargo test --workspace --locked\t" in l))) for i in range(3)],
        "pythonReviewerSuites": {p: pathlib.Path(p).name in text for p in python_suites},
        "rustReviewerSuites": {p: (re.search(r"Running\s+tests[/\\]" + re.escape(pathlib.Path(p).stem) + r"\.rs", text) is not None) for p in rust_suites},
        "sdkCheckSummary": re.findall(r"24/24[^\n]*", text),
    }
report["baselineLocalWorkspace"] = {"summarySum": [sum(int(r[i]) for r in re.findall(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored", read(HERE / "workspace-rerun.log"))) for i in range(3)],
                                    "hasFailedSummary": "test result: FAILED" in read(HERE / "workspace-rerun.log")}
report["progress50"] = read(HERE / "progress-50.log").count("test progress_and_cancellation ... ok")
raw = json.loads((HERE / "protocol-raw.json").read_text())
for variant in ["example", "malformed"]:
    calls = [r for r in raw if r.get("probe") == "command" and r.get("variant") == variant]
    report[variant + "Sweep"] = {"calls": len(calls), "latencyMs": stats([c["elapsedMs"] for c in calls]),
                                "statusCounts": dict(collections.Counter(c["result"]["status"] for c in calls)),
                                "errorCounts": dict(collections.Counter(c["result"].get("error", {}).get("code") for c in calls))}
report["flood"] = [r for r in raw if r["probe"] == "flood"]
flood = json.loads(read(HERE / "flood-raw.json"))
report["floodRecomputed"] = {"responses": len(flood), "uniqueIds": len({r["id"] for r in flood}),
    "errors": sum(r["error"] is not None for r in flood),
    "lastResponseMs": max(r["elapsedMs"] for r in flood)}
for key in ["responses", "uniqueIds", "errors"]:
    assert report["floodRecomputed"][key] == report["flood"][0]["result"][key]
sample_file = ROOT / "target/evidence/c14-r1/autosave-protocol-raw.json"
if sample_file.exists():
    # Preserve the raw result bytes rather than parse/reserialize a measurement artifact.
    (HERE / "autosave-protocol-raw.json").write_bytes(sample_file.read_bytes())
    samples = json.loads(sample_file.read_text())
    report["protocolAutosave"] = {
        "snapshotMs": stats([s["snapshotMs"] for s in samples]),
        "schedulingDelayMs": stats([s["maintenanceElapsedMs"] - sum(s[k] or 0 for k in ["snapshotMs", "heartbeatMs", "watchMs"]) for s in samples]),
        "exportFinalStates": [s["export"]["result"]["state"] for s in samples],
    }
measurements = []
artifact_hashes = {}
for directory in [ROOT / "reviews/evidence/M1-C1.4/measurements", HERE / "ci-headless-Windows", HERE / "ci-headless-macOS"]:
    for path in sorted(directory.rglob("*")):
        if not path.is_file():
            continue
        artifact_hashes[str(path.relative_to(ROOT)).replace("\\", "/")] = hashlib.sha256(path.read_bytes()).hexdigest()
        if path.suffix == ".json" and any(x in path.name for x in ["autosave", "fingerprint", "never-block", "lock-liveness", "cache-bounds", "queued-calls"]):
            value = json.loads(path.read_text())
            def find_samples(v):
                if isinstance(v, dict):
                    return any((k.lower().endswith(("samples", "raw", "observations")) and isinstance(x, list) and len(x)>1) or find_samples(x) for k, x in v.items())
                if isinstance(v, list):
                    return any(find_samples(x) for x in v)
                return False
            measurements.append({"file": str(path.relative_to(ROOT)).replace("\\", "/"), "recomputableSamples": find_samples(value), "retainedMeasurement": value})
report["builderMeasurements"] = measurements
report["measurementRecomputationLimit"] = "Builder percentiles lack original observations on both OSes; values are retained claims, not independently recomputed statistics."
report["focusedRuntimeDeclarations"] = len(json.loads((HERE / "declarations-runtime.json").read_text()))
report["freshArtifactDownloads"] = json.loads((HERE / "artifact-download-verification.json").read_text())
errors = set()
for item in raw:
    result = item.get("result")
    if isinstance(result, dict):
        error = result.get("error")
        if isinstance(error, dict) and isinstance(error.get("code"),str): errors.add(error["code"])
report["observedDottedCodes"] = sorted(errors)
report["freshWorkspace"] = report.pop("baselineLocalWorkspace")
report["freshWorkspace"]["hasFailedSummary"] = "test result: FAILED" in read(HERE / "workspace-rerun.log")
report["freshWorkspace"]["complete"] = False
workspace_results = re.findall(r"test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored",
    read(HERE / "workspace-rerun.log"))
report["freshWorkspace"]["summarySum"] = [sum(int(r[i]) for r in workspace_results) for i in range(3)]
register = json.loads(read(HERE / "conformance.json"))
report["register"] = {}
for key, count_key in [("requirements", "counts"), ("declarations", "declarationCounts")]:
    counts = dict(collections.Counter(r["status"] for r in register[key]))
    assert counts == register[count_key], (key, counts, register[count_key])
    report["register"][key] = {"total": len(register[key]), **counts}
review = read(ROOT / "reviews/M1-C1.4-r1.md")
findings = re.findall(r"^### (C14-R1-\d+) — (P[012])", review, re.M)
report["findingCounts"] = {p: sum(level == p for _, level in findings) for p in ["P0", "P1", "P2"]}
assert len(findings) == 17
report["obligations"] = [{"id": int(n), "decision": decision} for n, decision in
    re.findall(r"^\| (\d) — [^|]+\| \*\*([^*]+)\*\*", review, re.M)]
assert len(report["obligations"]) == 8
final_ci = json.loads(read(HERE / "ci-final.json"))
assert final_ci["headSha"] == ci["headSha"] and final_ci["conclusion"] == "success"
report["finalCi"] = {"head": final_ci["headSha"], "conclusion": final_ci["conclusion"],
    "jobs": {j["name"]: j["conclusion"] for j in final_ci["jobs"]}}
report["sdk"] = json.loads(read(HERE / "sdk-local.json"))
report["baselineReviewerSuites"] = {"rust": len(rust_suites), "python": len(python_suites)}
report["finalVerification"] = json.loads(read(HERE / "final-verification.json"))
report["finalVerification"]["runtime"]["failureCount"] = len(report["finalVerification"]["runtime"]["failures"])
report["finalVerification"]["runtime"]["passCount"] = len(report["finalVerification"]["runtime"]["passes"])
report["artifactSha256"] = artifact_hashes
receipt = json.loads((ROOT / "reviews/evidence/M1-C1.4/receipt.json").read_text())
report["builderReceipt"] = {"exitCodes": receipt.get("status", {}),
    "sha256Matches": {p: hashlib.sha256((ROOT / "reviews/evidence/M1-C1.4" / p).read_bytes()).hexdigest() == expected for p, expected in receipt["sha256"].items()}}
(HERE / "verification.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
print(json.dumps(report, indent=2))
