"""Exercise the real page with the amended R3 harness plus closure boundary probes."""
import pathlib
import subprocess
import sys

OUT=pathlib.Path(__file__).resolve().parent
ROOT=OUT.parents[2]
target=ROOT/'desktop/src/test/c04-r6-closure.test.tsx'
assert not target.exists()
try:
    source=(ROOT/'tests/adversarial/c04-r3-actions.tsx').read_text(encoding='utf-8')
    source+='\n'+(OUT/'page_cases.tsx').read_text(encoding='utf-8')
    target.write_text(source,encoding='utf-8')
    p=subprocess.run(['cmd','/d','/c','pnpm --dir desktop exec vitest run --config vite.config.ts src/test/c04-r6-closure.test.tsx'],cwd=ROOT)
finally:
    target.unlink(missing_ok=True)
sys.exit(p.returncode)
