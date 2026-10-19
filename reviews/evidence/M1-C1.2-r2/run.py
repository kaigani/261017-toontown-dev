"""Record a bounded command, exact output, exit code, worktree and hashes."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
name, *cmd = sys.argv[1:]
(HERE / "logs").mkdir(exist_ok=True)
(HERE / "receipts").mkdir(exist_ok=True)
env = dict(os.environ, PYTHONIOENCODING="utf-8", TT_C12_REVIEW_ARTIFACT_DIR=str(HERE))
started = datetime.datetime.now(datetime.timezone.utc).isoformat()
t = time.monotonic()
log = HERE / "logs" / f"{name}.log"
with log.open("wb") as f:
    p = subprocess.run(cmd, cwd=ROOT, stdout=f, stderr=subprocess.STDOUT, env=env, timeout=3600, check=False)
receipt = {"command": cmd, "exit": p.returncode, "started": started, "seconds": time.monotonic()-t,
           "head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
           "dirty": subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).splitlines(),
           "log": str(log.relative_to(HERE)), "sha256": hashlib.sha256(log.read_bytes()).hexdigest()}
(HERE / "receipts" / f"{name}.json").write_text(json.dumps(receipt, indent=2)+"\n", encoding="utf-8")
print(json.dumps(receipt))
sys.exit(p.returncode)
