"""Run one review command, retaining exact output and a hashed receipt."""
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
if any(c not in "abcdefghijklmnopqrstuvwxyz0123456789-_" for c in name):
    raise SystemExit("Use a portable log name")
out = HERE / "logs" / (name + ".log")
receipt = HERE / "receipts" / (name + ".json")
out.parent.mkdir(exist_ok=True)
receipt.parent.mkdir(exist_ok=True)
if receipt.exists():
    raise SystemExit("Refusing to overwrite receipt: " + name)
env = dict(os.environ, PYTHONIOENCODING="utf-8", CARGO_TERM_COLOR="never")
env.setdefault("PROPTEST_RNG_SEED", "202610180311")
start = datetime.datetime.now(datetime.timezone.utc).isoformat()
t = time.monotonic()
with out.open("wb") as f:
    # Native executables receive argv directly; cmd is needed only for .cmd shims / an explicitly requested shell.
    command = ["cmd.exe", "/d", "/s", "/c", subprocess.list2cmdline(args)] if args[0] in ("pnpm", "cmd") else args
    p = subprocess.run(command, cwd=ROOT, env=env, stdout=f, stderr=subprocess.STDOUT)
record = dict(command=args, cwd=str(ROOT), base=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(), started_utc=start, elapsed_seconds=time.monotonic()-t, exit_code=p.returncode, log=str(out.relative_to(HERE)).replace("\\", "/"), sha256=hashlib.sha256(out.read_bytes()).hexdigest(), proptest_seed=env["PROPTEST_RNG_SEED"], proptest_cases=env.get("PROPTEST_CASES"))
receipt.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
print(json.dumps(record))
print(out.read_text(encoding="utf-8", errors="replace")[-2200:])
sys.exit(p.returncode)
