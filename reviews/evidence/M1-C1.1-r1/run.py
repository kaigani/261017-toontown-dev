"""Capture review commands verbatim with exit codes, times and raw byte logs."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent

def run(name, command):
    start = time.perf_counter()
    stamp = datetime.datetime.now(datetime.timezone.utc).isoformat()
    env = {**os.environ, "CARGO_TERM_COLOR": "never", "PYTHONIOENCODING": "utf-8", "PROPTEST_RNG_SEED": "1182711"}
    with (OUT / (name + ".log")).open("wb") as log:
        p = subprocess.run(command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
    data = (OUT / (name + ".log")).read_bytes()
    receipt = {"command": command, "cwd": str(ROOT), "startedUtc": stamp, "seconds": time.perf_counter()-start,
               "exit": p.returncode, "logSha256": hashlib.sha256(data).hexdigest(), "proptestSeed": "1182711"}
    (OUT / (name + ".receipt.json")).write_text(json.dumps(receipt, indent=2)+"\n", encoding="utf-8")
    print(json.dumps(receipt), flush=True)
    return p.returncode

if __name__ == "__main__":
    sys.exit(run(sys.argv[1], sys.argv[2:]))
