"""Run the builder's actual consistency test against isolated document copies."""
import hashlib
import json
import pathlib
import shutil
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[3]
OUT = pathlib.Path(__file__).resolve().parent
TMP = ROOT / 'target/c04-r5-copy-probe'
TEST = ROOT / 'desktop/src/test/decisionTables.test.ts'
files = ['reviews/M0-C0.4-go-no-go.md', 'docs/decisions/ADR-0007-interactive-canvas.md',
         'docs/decisions/ADR-0009-tablet-input.md']
before = {p: hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in files}
for name in files:
    dst = TMP/name
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT/name, dst)
# Preserve the test body. Only redirect its repo variable to the isolated fixture root.
body = TEST.read_text(encoding='utf-8')
body = body.replace("const repo = fileURLToPath(new URL('../../../', import.meta.url));",
                    'const repo = ' + json.dumps(TMP.as_posix() + '/') + ';')
probe = ROOT/'desktop/src/test/c04-r5-copy-probe.test.ts'
assert not probe.exists()
result = []
try:
    probe.write_text(body, encoding='utf-8')
    for variant in ('baseline', 'tablet-divergence', 'canvas-divergence', 'restored'):
        report = (ROOT/files[0]).read_text(encoding='utf-8')
        if variant == 'tablet-divergence':
            report = report.replace('| 2 | Pressure ≥ 256 levels |', '| 2 | Pressure ≥ 257 levels |', 1)
        if variant == 'canvas-divergence':
            report = report.replace('fps ≥ 57 over the 10 s', 'fps ≥ 58 over the 10 s', 1)
        (TMP/files[0]).write_text(report, encoding='utf-8')
        proc = subprocess.run(['cmd','/d','/c',
            'pnpm --dir desktop exec vitest run --config vite.config.ts src/test/c04-r5-copy-probe.test.ts'],
            cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=120)
        (OUT/f'copy-{variant}.log').write_bytes(proc.stdout)
        result.append(dict(variant=variant, exit=proc.returncode))
        assert proc.returncode == (0 if variant in ('baseline', 'restored') else 1), result[-1]
finally:
    probe.unlink(missing_ok=True)
assert before == {p: hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in files}
(OUT/'copy-consistency.json').write_text(json.dumps(dict(results=result, originals=before), indent=2)+'\n')
print(result)
