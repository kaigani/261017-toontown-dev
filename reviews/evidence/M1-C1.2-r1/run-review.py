"""Reproduce review commands with raw output, exit codes, timings and SHA-256 receipts.

Run from the repository root: python reviews/evidence/M1-C1.2-r1/run-review.py <name> <command> [args...]
Commands are argv, never shell text. pnpm uses the Windows command shim via cmd /c only for execution, not file ops.
"""
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def main():
    name, *cmd = sys.argv[1:]
    if not name.replace("-", "").replace("_", "").isalnum() or not cmd:
        raise SystemExit("name must be simple; command is required")
    logs = HERE / "logs"
    logs.mkdir(exist_ok=True)
    receipts = HERE / "receipts"
    receipts.mkdir(exist_ok=True)
    env = os.environ.copy()
    env["TT_C12_REVIEW_ARTIFACT_DIR"] = str(HERE)
    t = time.monotonic()
    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    with (logs / (name + ".log")).open("wb") as out:
        p = subprocess.run(cmd, cwd=ROOT, env=env, stdout=out, stderr=subprocess.STDOUT, check=False)
    log = logs / (name + ".log")
    receipt = {
        "command": cmd, "cwd": str(ROOT), "startedAt": started,
        "durationSeconds": round(time.monotonic() - t, 3), "exitCode": p.returncode,
        "head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "dirtyFiles": subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).splitlines(),
        "machine": platform.platform(), "python": sys.version,
        "log": str(log.relative_to(HERE)), "sha256": hashlib.sha256(log.read_bytes()).hexdigest(),
        "extraEnv": {"TT_C12_REVIEW_ARTIFACT_DIR": str(HERE)},
    }
    (receipts / (name + ".json")).write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({k: receipt[k] for k in ("command", "exitCode", "durationSeconds", "log")}))
    return p.returncode


if __name__ == "__main__":
    sys.exit(main())
