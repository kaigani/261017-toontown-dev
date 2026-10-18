"""Run evidence-only page probes without registering permanent passing tests."""
import os
from pathlib import Path
import subprocess

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[2]
probe = ROOT / "desktop/src/test/__r3-evidence.test.tsx"
assert not probe.exists()
try:
    probe.write_bytes((OUT / "page-probe.tsx").read_bytes())
    with (OUT / "page-probe.txt").open("w", encoding="utf-8") as f:
        r = subprocess.run("pnpm --dir desktop exec vitest run --config vite.config.ts src/test/__r3-evidence.test.tsx", shell=True, cwd=ROOT, stdout=f, stderr=subprocess.STDOUT, env=dict(os.environ, NO_COLOR="1"))
    print("Page probe exit:", r.returncode)
finally:
    probe.unlink()
raise SystemExit(r.returncode)
