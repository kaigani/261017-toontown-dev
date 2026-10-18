"""Repeatable review runtime evidence; does not modify product code."""
import json
from pathlib import Path
import subprocess
import time

out = Path(__file__).resolve().parent
repo = out.parents[2]
runtime = []

def run(args, destination, timeout=120):
    start = time.perf_counter()
    p = subprocess.run(args, cwd=repo, capture_output=True, text=True, timeout=timeout)
    (out / destination).write_text(p.stdout, encoding="utf-8")
    runtime.append({"args": [str(a) for a in args], "exit": p.returncode,
                    "seconds": time.perf_counter() - start, "stderr": p.stderr})
    assert p.returncode == 0, p.stderr
    return p.stdout

exe = repo / "target/release/toontown-headless.exe"
a = json.loads(run([exe, "--selftest", "--json"], "selftest-1.json"))
b = json.loads(run([exe, "--selftest", "--json"], "selftest-2.json"))
def strip(value):
    if isinstance(value, dict):
        return {k: strip(v) for k, v in value.items() if k not in ("elapsedNs", "elapsedMs", "durationNs")}
    if isinstance(value, list):
        return [strip(v) for v in value]
    return value
# The checks themselves include timing; retain ids, status, and detailed golden evidence.
aa = [(c['id'], c['ok'], c.get('details')) for c in a['checks']]
bb = [(c['id'], c['ok'], c.get('details')) for c in b['checks']]
assert aa == bb and all(c['ok'] for c in a['checks'])
(out / 'headless-comparison.json').write_text(json.dumps(aa, indent=2), encoding='utf-8')
run(['python', 'scripts/check-headless-deps.py', str(exe)], 'headless-deps.log')
run([repo / 'target/release/toontown.exe', '--ipc-bench', out / 'ipc-bench.json', '--ipc-bench-count', '500'], 'ipc-bench-stdout.log')
bench = json.loads((out / 'ipc-bench.json').read_text(encoding='utf-8'))
assert bench['frontend']['ok'] is True
(out / 'runtime.json').write_text(json.dumps(runtime, indent=2), encoding='utf-8')
print(json.dumps(bench, indent=2))
