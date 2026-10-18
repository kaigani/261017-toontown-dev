"""Run evidence-only page probes without registering permanent passing tests."""
import os
import gzip
from pathlib import Path
import subprocess

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[2]
probe = ROOT / "desktop/src/test/__r4-evidence.test.tsx"
assert not probe.exists()
try:
    probe.write_bytes((OUT / "page-probe.tsx").read_bytes())
    with (OUT / "page-probe.txt").open("w", encoding="utf-8") as f:
        r = subprocess.run("pnpm --dir desktop exec vitest run --config vite.config.ts src/test/__r4-evidence.test.tsx", shell=True, cwd=ROOT, stdout=f, stderr=subprocess.STDOUT, env=dict(os.environ, NO_COLOR="1"))
    print("Page probe exit:", r.returncode)
finally:
    probe.unlink()
if r.returncode == 0:
    for path in OUT.glob('*-page-log.json'):
        raw = path.read_bytes()
        compressed = path.with_suffix('.json.gz')
        compressed.write_bytes(gzip.compress(raw, mtime=0))
        assert gzip.decompress(compressed.read_bytes()) == raw
        path.unlink()
raise SystemExit(r.returncode)
