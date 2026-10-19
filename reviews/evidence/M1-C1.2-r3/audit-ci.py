"""Verify actual job logs, not only the green check marks."""
import gzip
import json
import re
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
RUN = "39236678120"


def gh(*args):
    return subprocess.check_output(["gh", *args], cwd=ROOT)


run = json.loads(gh("run", "view", RUN, "--json", "headSha,conclusion,status,jobs,url,workflowName"))
(HERE / "ci-run.json").write_text(json.dumps(run, indent=2) + "\n", encoding="utf-8")
raw = gzip.decompress((HERE / "ci-full.log.gz").read_bytes()) if (HERE / "ci-full.log.gz").exists() else gh("run", "view", RUN, "--log")
(HERE / "ci-full.log.gz").write_bytes(gzip.compress(raw, mtime=0))
text = re.sub(r"\^\[\[[0-9;]*m|\x1b\[[0-9;]*m", "", raw.decode("utf-8", "replace"))
out = {"run": RUN, "head": run["headSha"], "conclusion": run["conclusion"], "jobs": []}
for job in run["jobs"]:
    lines = [line for line in text.splitlines() if line.startswith(job["name"] + "\t")]
    body = "\n".join(lines)
    targets = sorted(set(re.findall(r"[/\\]deps[/\\](\w+)-[a-f0-9]+", body)))
    results = re.findall(r"test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored", body)
    out["jobs"].append({"name": job["name"], "conclusion": job["conclusion"], "logLines": len(lines), "rustTargets": targets,
                        "rustTotals": {"passed": sum(int(r[1]) for r in results), "failed": sum(int(r[2]) for r in results), "ignored": sum(int(r[3]) for r in results)},
                        "suiteSummaries": [re.sub(r"^.*?\d{4}-\d\d-\d\dT\S+\s+", "", line) for line in lines if re.search(r"Test Files|Tests\s+\d+ passed|of 11|11/11|test result:|hash|self.test|display|headless-deps", line, re.I)][-150:],
                        "steps": [{"name": s["name"], "conclusion": s["conclusion"]} for s in job["steps"]]})
    if job["name"].startswith("check ("):
        required = ["c12_r1_contracts", "c12_r1_stress", "c12_r1_macro", "c12_r1_scale", "c12_r2_contracts", "c12_r2_registry", "c12_r2_scale", "c12_never_block", "c11_r1_adversarial", "c11_r2_adversarial", "c11_r3_adversarial", "c11_r3_random", "c11_r3_register", "c11_r4_exposure_bounds", "c11_r5_write_bounds", "c11_r6_staged_inverse", "c11_r1_wire", "c11_r2_wire", "c11_r3_wire", "adversarial_c01"]
        assert set(required) <= set(targets), (job["name"], set(required) - set(targets))
assert run["headSha"].startswith("2e0e5dd")
assert len(out["jobs"]) == 8 and all(j["conclusion"] == "success" and j["logLines"] for j in out["jobs"])
(HERE / "ci-audit.json").write_text(json.dumps(out, indent=2) + "\n", encoding="utf-8")
print(json.dumps({"run": RUN, "head": out["head"], "jobs": [{k: j[k] for k in ("name", "conclusion", "logLines", "rustTotals")} for j in out["jobs"]]}, indent=2))
