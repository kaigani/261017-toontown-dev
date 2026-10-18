"""Run reviewer-owned TSX inside the configured Vitest tree, then remove the temporary copy."""
import pathlib
import subprocess
import sys

ROOT=pathlib.Path(__file__).resolve().parents[2]
target=ROOT/'desktop/src/test/c04-r3-actions.test.tsx'
assert not target.exists()
try:
    target.write_bytes((ROOT/'tests/adversarial/c04-r3-actions.tsx').read_bytes())
    p=subprocess.run(['cmd','/d','/c','pnpm --dir desktop exec vitest run --config vite.config.ts src/test/c04-r3-actions.test.tsx'],cwd=ROOT)
finally:
    target.unlink(missing_ok=True)
sys.exit(p.returncode)
