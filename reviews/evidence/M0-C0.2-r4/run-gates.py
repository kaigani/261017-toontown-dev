"""Independent closure gate receipts. Run from any directory with group argument."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[2]
GROUPS = {
    "rust": [
        "cargo fmt --all --check",
        "cargo clippy --workspace --all-targets --locked -- -D warnings",
        "cargo test --workspace --locked",
        "cargo deny --locked check",
        "cargo build --release --locked -p tt-headless",
    ],
    "frontend": [
        "pnpm lint", "pnpm typecheck", "pnpm test", "pnpm format:check",
        "pnpm licenses:check", "pnpm --dir desktop build",
    ],
    "regressions": [f"python tests/adversarial/c02-r{n}-tooling.py" for n in (3, 1, 2)],
    "release": ['cmd /d /c "pnpm --dir desktop tauri build --no-bundle -- --locked"'],
}
group = sys.argv[1]
results = []
for i, command in enumerate(GROUPS[group]):
    start = time.time()
    with (OUT / f"gate-{group}-{i+1}.txt").open("w", encoding="utf-8") as output:
        output.write(command + "\n")
        output.flush()
        result = subprocess.run(command, shell=True, cwd=ROOT, stdout=output, stderr=subprocess.STDOUT,
                                env=dict(os.environ, NO_COLOR="1", PYTHONIOENCODING="utf-8"))
    row = dict(command=command, exit=result.returncode, seconds=round(time.time()-start, 2))
    results.append(row)
    (OUT / f"gate-{group}.json").write_text(json.dumps(results, indent=2), encoding="utf-8")
    print(row, flush=True)
sys.exit(any(r["exit"] != 0 for r in results))
