"""Run the scoped closure gates and retain exit codes and complete logs."""
import json
from pathlib import Path
import subprocess
import time

out = Path(__file__).resolve().parent
repo = out.parents[2]
commands = [
    ('r4-boundary', 'node --test tests/adversarial/c01-r4-boundary.test.mjs'),
    ('r3-boundary', 'node --test tests/adversarial/c01-r3-boundary.test.mjs'),
    ('r2-node', 'node --test tests/adversarial/c01-r2-boundary.test.mjs tests/adversarial/c01-r2-licenses.test.mjs'),
    ('reviewer-frontend', 'pnpm --dir desktop exec vitest run --config vite.config.ts src/test/c01-r3-schema.test.ts src/test/c01-r2-schema.test.ts src/test/adversarial.test.ts'),
    ('build', 'pnpm --dir desktop build'),
    ('fmt', 'cargo fmt --all -- --check'),
    ('clippy', 'cargo clippy --workspace --all-targets --locked -- -D warnings'),
    ('rust-test', 'cargo test --workspace --locked'),
    ('deny', 'cargo deny --locked check'),
    ('lint', 'pnpm lint'),
    ('typecheck', 'pnpm typecheck'),
    ('test', 'pnpm test'),
    ('format', 'pnpm format:check'),
    ('licenses', 'pnpm licenses:check'),
    ('release-headless', 'cargo build --release --locked -p tt-headless'),
    ('release-desktop', 'pnpm --dir desktop tauri build --no-bundle -- --locked'),
    ('runtime', 'python reviews/evidence/M0-C0.1-r5/runtime.py'),
]
rows = []
for name, command in commands:
    start = time.perf_counter()
    with (out / (name + '.log')).open('w', encoding='utf-8') as log:
        p = subprocess.run(['cmd', '/c', command], cwd=repo, stdout=log, stderr=subprocess.STDOUT)
    row = dict(name=name, command=command, exit=p.returncode, seconds=round(time.perf_counter()-start, 3))
    rows.append(row)
    (out / 'gates.json').write_text(json.dumps(rows, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(row), flush=True)
