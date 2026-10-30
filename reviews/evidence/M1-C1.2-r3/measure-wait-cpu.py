"""Measure child process CPU during the real sequential-batch wait (Windows process accounting)."""
import ctypes
import json
import subprocess
import time
from pathlib import Path
from ctypes import wintypes

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
exe = max((ROOT / "target/debug/deps").glob("c12_r3_contracts-*.exe"), key=lambda p: p.stat().st_mtime)
command = [str(exe), "sequential_batch_wait_uses_elapsed_time_and_reaches_completion", "--exact", "--nocapture"]
start = time.perf_counter()
process = subprocess.Popen(command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
output, _ = process.communicate(timeout=20)
elapsed = time.perf_counter() - start
kernel = ctypes.WinDLL("kernel32", use_last_error=True)
kernel.GetProcessTimes.argtypes = [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4
kernel.GetProcessTimes.restype = wintypes.BOOL
created, ended, kern, user = [wintypes.FILETIME() for _ in range(4)]
assert kernel.GetProcessTimes(int(process._handle), ctypes.byref(created), ctypes.byref(ended), ctypes.byref(kern), ctypes.byref(user))
ticks = lambda ft: (ft.dwHighDateTime << 32) | ft.dwLowDateTime
cpu = (ticks(kern) + ticks(user)) / 10_000_000
result = {"command": command, "exitCode": process.returncode, "wallSeconds": elapsed, "cpuSeconds": cpu, "oneCoreFraction": cpu / elapsed, "expectedHeldWaitMs": 1250, "method": "GetProcessTimes of whole child, includes fixture/setup; no sampling loss"}
(HERE / "wait-cpu.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
(HERE / "wait-cpu.log").write_bytes(output)
print(json.dumps(result, indent=2))
assert process.returncode == 0 and elapsed >= 1.0
