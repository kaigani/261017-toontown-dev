"""Compact receipts for the scoped review; preserve useful emitted code and gate results."""
import hashlib
import json
from pathlib import Path
import re
import subprocess

out = Path(__file__).resolve().parent
repo = out.parents[2]
def run(command):
    p = subprocess.run(['cmd', '/c', command], cwd=repo, capture_output=True)
    return dict(command=command, exit=p.returncode, stdout=p.stdout.decode('utf-8', errors='replace'), stderr=p.stderr.decode('utf-8', errors='replace'))
rows = []
for command in [
    'git rev-parse HEAD', 'git branch --show-current', 'git log --oneline 94d4b63..d54f0b7',
    'rustc --version', 'cargo deny --version', 'node --version', 'pnpm --version', 'python --version',
    'pnpm lint', 'node desktop/boundary/check-dist.mjs desktop/dist',
]:
    receipt = run(command)
    assert receipt['exit'] == 0, receipt
    rows.append(receipt)
diff = subprocess.check_output(['git', 'diff', '94d4b63..d54f0b7', '--', 'tests/adversarial', 'desktop/src/test/c01-r*.test.ts', 'desktop/src/test/adversarial.test.ts'], cwd=repo)
assert diff == b''
(out / 'reviewer-tests.diff').write_bytes(diff)
rows.append(dict(reviewerTestsDiffBytes=len(diff)))
frontend = (out / 'test.log').read_text(encoding='utf-8')
assert re.search(r'Tests\s+198 passed \| 3 expected fail \(201\)', frontend)
assert re.search(r'Test Files\s+15 passed \(15\)', frontend)
assert '# pass 9' in frontend and '# fail 0' in frontend
rust = (out / 'rust-test.log').read_text(encoding='utf-8')
count = sum(int(n) for n in re.findall(r'test result: ok\. (\d+) passed;', rust))
assert count == 53
rows.append(dict(vitestPass=198, vitestExpectedFailures=3, vitestFiles=15, nodePass=9, rustPass=count))
dist = repo / 'desktop/dist'
files = []
for file in sorted(dist.rglob('*')):
    if not file.is_file():
        continue
    data = file.read_bytes()
    entry = dict(file=str(file.relative_to(dist)), bytes=len(data), sha256=hashlib.sha256(data).hexdigest())
    if file.suffix == '.js':
        code = data.decode('utf-8')
        entry['literalTauri'] = bool(re.search('__tauri|@tauri-apps', code, re.I))
        entry['transportReferences'] = re.findall(r'.{0,60}tauri-transport.{0,80}', code)
        assert not entry['literalTauri'] or file.name.startswith('tauri-transport-')
    files.append(entry)
assert len(list(dist.rglob('tauri-transport-*.js'))) == 1
(out / 'production-dist.json').write_text(json.dumps(files, indent=2)+'\n', encoding='utf-8')
probes = json.loads((out / 'closure-probes.json').read_text(encoding='utf-8'))
summary = []
for r in probes:
    if 'accepted' not in r:
        continue
    for f in r['written']:
        if f['exists']:
            g = next(g for g in r['generated'] if g['fileName'] == f['fileName'])
            assert g['sha256'] == f['sha256'], (r['name'], f['fileName'])
    summary.append(dict(name=r['name'], lintErrors=r['lint']['errors'], accepted=r['accepted'],
        generatedChunks=len([f for f in r['generated'] if not f.get('asset')]),
        writtenChunks=len([f for f in r['written'] if f['exists']]), runtime=r.get('runtime'),
        generatedTauri=[f['fileName'] for f in r['generated'] if f.get('literalTauri')],
        writtenTauri=[f['fileName'] for f in r['written'] if f.get('literalTauri')]))
(out / 'probe-summary.json').write_text(json.dumps(summary, indent=2)+'\n', encoding='utf-8')
emitted = out / 'emitted'
emitted.mkdir(exist_ok=True)
for name in ['constant-reduce', 'constant-json', 'constant-decoder', 'constant-object', 'const-object']:
    (emitted / (name+'.js')).write_bytes((out / 'outputs' / name / 'dist/probe.js').read_bytes())
(out / 'audit.json').write_text(json.dumps(rows, indent=2)+'\n', encoding='utf-8')
print(json.dumps(dict(gates='pass', counts=rows[-1], probes=summary), indent=2))
