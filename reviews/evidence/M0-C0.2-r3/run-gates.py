"""Run review gates with individual exit codes and UTF-8 receipts."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
GROUPS = {
    "rust": [
        "cargo fmt --all --check",
        "cargo clippy --workspace --all-targets --locked -- -D warnings",
        "cargo test --workspace --locked",
        "cargo deny --locked check",
        "cargo build --release --locked -p tt-headless",
    ],
    "frontend": ["pnpm " + s for s in ("lint", "typecheck", "test", "format:check", "licenses:check")],
    "release": ['cmd /d /c "pnpm --dir desktop tauri build --no-bundle -- --locked"'],
}
group = sys.argv[1]
results = []
for i, cmd in enumerate(GROUPS[group]):
    started = time.monotonic()
    print(cmd, flush=True)
    env = dict(os.environ, PYTHONIOENCODING="utf-8", NO_COLOR="1", CARGO_TERM_COLOR="never")
    with (OUT / f"gate-{group}-{i+1}.txt").open("w", encoding="utf-8") as f:
        result = subprocess.run(cmd, shell=True, cwd=ROOT, stdout=f, stderr=subprocess.STDOUT, env=env)
    results.append(dict(command=cmd, exitCode=result.returncode, seconds=round(time.monotonic()-started, 2)))
    (OUT / f"gate-{group}.json").write_text(json.dumps(results, indent=2), encoding="utf-8")
    print(results[-1], flush=True)
sys.exit(int(any(r["exitCode"] for r in results)))
