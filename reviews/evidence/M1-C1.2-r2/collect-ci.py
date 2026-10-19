"""Fetch the two cited Actions runs and retain exact logs compressed losslessly."""
import concurrent.futures
import gzip
import hashlib
import json
from pathlib import Path
import re
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def fetch(run):
    raw = subprocess.check_output(["gh", "run", "view", str(run), "--json", "databaseId,headSha,conclusion,event,createdAt,updatedAt,url,jobs"], cwd=ROOT)
    metadata = json.loads(raw)
    (HERE / f"ci-{run}.json").write_bytes(raw)
    logs = subprocess.check_output(["gh", "run", "view", str(run), "--log"], cwd=ROOT)
    (HERE / f"ci-{run}.log.gz").write_bytes(gzip.compress(logs, mtime=0))
    text = logs.decode("utf-8", errors="replace")
    selected = [line for line in text.splitlines() if re.search(r"test result:|Test Files|Tests  |Tests\s+\d+|11/11 reviewer|exclusive_mutation_capability|c11_r[1-7]|c04-r[1-4]|staged_inverse|staged_commit", line)]
    (HERE / f"ci-{run}-counts.log").write_text("\n".join(selected)+"\n", encoding="utf-8")
    return {"run":run, "head":metadata["headSha"], "conclusion":metadata["conclusion"], "jobs":[{"name":j["name"],"conclusion":j["conclusion"]} for j in metadata["jobs"]], "rawLogBytes":len(logs), "rawLogSha256":hashlib.sha256(logs).hexdigest()}


if __name__ == "__main__":
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        results=list(pool.map(fetch,[39215102172,39217799686]))
    (HERE / "ci-summary.json").write_text(json.dumps(results,indent=2)+"\n",encoding="utf-8")
    print(json.dumps(results,indent=2))

