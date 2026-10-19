"""Run evidence-only Rust closure probes without leaving additions in the permanent suite."""
import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
target = ROOT / 'crates/executor/tests/c12_r5_probe.rs'
assert not target.exists(), target
env = os.environ.copy()
env['TT_C12_REVIEW_ARTIFACT_DIR'] = str(HERE)
try:
    target.write_bytes((HERE / 'closure-probe.rs').read_bytes())
    p = subprocess.run(['cargo', 'test', '-p', 'tt-executor', '--test', 'c12_r5_probe', '--locked', '--', '--nocapture'], cwd=ROOT, env=env)
finally:
    target.unlink()
sys.exit(p.returncode)
