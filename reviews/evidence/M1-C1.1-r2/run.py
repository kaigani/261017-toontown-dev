"""Run a review command with immutable raw output and a content-hashed receipt."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
name, *args = sys.argv[1:]
name = name.replace(":", "-")
out = HERE / "logs" / (name + ".log")
receipt = HERE / "receipts" / (name + ".json")
out.parent.mkdir(exist_ok=True)
receipt.parent.mkdir(exist_ok=True)
if receipt.exists():
    raise SystemExit("Refusing to overwrite an existing receipt: " + name)
env = dict(os.environ, PYTHONIOENCODING="utf-8", CARGO_TERM_COLOR="never", PROPTEST_RNG_SEED="1182712")
start = datetime.datetime.now(datetime.timezone.utc).isoformat()
t = time.monotonic()
with out.open("wb") as f:
    p = subprocess.run(["cmd.exe", "/d", "/s", "/c", subprocess.list2cmdline(args)], cwd=ROOT, env=env, stdout=f, stderr=subprocess.STDOUT)
record = dict(command=args, cwd=str(ROOT), base=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(), started_utc=start, elapsed_seconds=time.monotonic()-t, exit_code=p.returncode, log=str(out.relative_to(HERE)), sha256=hashlib.sha256(out.read_bytes()).hexdigest(), proptest_seed=env["PROPTEST_RNG_SEED"])
receipt.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
print(json.dumps(record))
print(out.read_text(encoding="utf-8", errors="replace")[-6000:])
sys.exit(p.returncode)
