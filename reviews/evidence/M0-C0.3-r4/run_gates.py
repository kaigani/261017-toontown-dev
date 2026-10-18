"""Independent baseline gates; capture exact commands, outputs and exit codes."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
GROUPS = {
    'rust': [
        ('fmt', 'cargo fmt --all --check'),
        ('clippy', 'cargo clippy --workspace --all-targets --locked -- -D warnings'),
        ('rust-tests', 'cargo test --workspace --locked'),
        ('deny', 'cargo deny --locked check'),
        ('headless-build', 'cargo build --release --locked -p tt-headless'),
        ('headless-1', 'target\\release\\toontown-headless.exe --selftest --json'),
        ('headless-2', 'target\\release\\toontown-headless.exe --selftest --json'),
        ('headless-guard', 'python scripts/check-headless-deps.py target/release/toontown-headless.exe'),
        ('tauri-build', 'pnpm --dir desktop tauri build --no-bundle -- --locked'),
    ],
    'frontend': [('frontend-build','pnpm --dir desktop build')] + [(x.replace(':', '-'), 'pnpm '+x) for x in ['lint', 'typecheck', 'test', 'format:check', 'licenses:check']],
    'ci': [(f'ci-{r}-logs',f'gh run view {r} --log') for r in [39131376596]] + [(f'artifacts-{r}',f'gh run download {r} --pattern c03-raster-* --dir target/review-r4/artifacts/{r}') for r in [39131376596]],
    'raster': [
        ('raster-default', 'cargo run --release --locked -p tt-render --example c03_raster -- --repeat 3 --out target/review-r4/default --report reviews/evidence/M0-C0.3-r4/raster-default.json --check crates/render/examples/c03_raster/hashes.json'),
    ],
}
for name, command in GROUPS[sys.argv[1]]:
    start = time.time()
    with (OUT / (name+'.log')).open('wb') as f:
        p = subprocess.run(['cmd','/d','/c',command], cwd=ROOT, stdout=f, stderr=subprocess.STDOUT, env={**os.environ,'PYTHONIOENCODING':'utf-8','C03_REVIEW_EVIDENCE':str(OUT)}, timeout=2400)
    receipt = dict(command=command,exitCode=p.returncode,seconds=round(time.time()-start,3))
    (OUT/(name+'.receipt.json')).write_text(json.dumps(receipt,indent=2)+'\n')
    print(name,receipt,flush=True)
