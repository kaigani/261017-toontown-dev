"""Scoped independent CLI closure checks, corpus audit and estimator regression comparison."""
import copy
import gzip
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
ENV = {**os.environ, 'PYTHONIOENCODING': 'utf-8'}
base = json.loads((ROOT / 'reviews/evidence/M0-C0.3-r3/page-r3-one-frame.json').read_bytes())
results = []


def analyze(path, script='scripts/analyse-canvas-log.py'):
    with tempfile.TemporaryDirectory() as tmp:
        output = Path(tmp) / 'analysis.json'
        p = subprocess.run(['python', '-I', str(script), str(path), '--json', str(output)],
                           cwd=ROOT, capture_output=True, encoding='utf-8', env=ENV)
        return p, json.loads(output.read_bytes()) if output.exists() else None


def probe(name, mutate, accepted=False):
    doc = copy.deepcopy(base)
    mutate(doc)
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / 'log.json'
        path.write_text(json.dumps(doc), encoding='utf-8')
        p, analysis = analyze(path)
    ok = p.returncode == 0 if accepted else p.returncode == 2 and p.stdout == ''
    if accepted:
        for r in analysis['runs']:
            assert r['complete'] is True and r['fps'] is None and r['meetsTarget'] is None
            assert r['intervalMs'] is None and r['cadence'] == 'insufficient samples (n=1)'
        assert 'not estimated: insufficient samples (n=1) (run complete)' in p.stdout
        (OUT / 'one-frame.analysis.json').write_text(json.dumps(analysis, indent=2) + '\n')
        (OUT / 'one-frame.analysis.md').write_text(p.stdout, encoding='utf-8')
    results.append(dict(name=name, exitCode=p.returncode, stderr=p.stderr.strip(),
                        stdoutBytes=len(p.stdout.encode()), passed=ok))


probe('complete-one-frame', lambda d: None, True)
probe('zero-frames-complete-span', lambda d: d['runs'][0].update(frames=[]))
probe('one-frame-before-span', lambda d: d['runs'][0]['frames'][0].update(t=d['runs'][0]['phases']['panZoom']['start']-1))
probe('one-frame-on-span-start', lambda d: d['runs'][0]['frames'][0].update(t=d['runs'][0]['phases']['panZoom']['start']))
probe('one-frame-after-span', lambda d: d['runs'][0]['frames'][0].update(t=d['runs'][0]['phases']['panZoom']['end']+1))
probe('span-shorter-than-declared', lambda d: d['runs'][0]['phases']['panZoom'].update(start=d['runs'][0]['phases']['panZoom']['start']+1))
probe('span-end-before-last-frame-full-duration', lambda d: d['runs'][0]['phases']['panZoom'].update(start=d['runs'][0]['phases']['panZoom']['start']-1, end=d['runs'][0]['phases']['panZoom']['end']-1))
probe('span-end-after-last-frame', lambda d: d['runs'][0]['phases']['panZoom'].update(end=d['runs'][0]['phases']['panZoom']['end']+1))
probe('missing-pan-span', lambda d: d['runs'][0]['phases'].pop('panZoom'))
probe('reversed-pan-span', lambda d: d['runs'][0]['phases']['panZoom'].update(end=d['runs'][0]['phases']['panZoom']['start']-1))
probe('version-1-one-frame', lambda d: d.update(version=1))
(OUT / 'closure-probes.json').write_text(json.dumps(results, indent=2) + '\n')
assert all(r['passed'] for r in results)

# Re-run every tracked raw canvas log, preserving the accepted historical v1 counterexample.
paths = subprocess.check_output(['git', 'ls-tree', '-r', '--name-only', '18c57fe', '--', 'reviews/evidence'], cwd=ROOT, text=True).splitlines()
paths += [p.relative_to(ROOT).as_posix() for p in (ROOT / 'target/review-r4/artifacts/39131376596').glob('desktop-*/canvas-spike-*.json')]
paths += [p.relative_to(ROOT).as_posix() for p in OUT.glob('page-*.json')]
logs = []
with tempfile.TemporaryDirectory() as tmp:
    old = Path(tmp) / 'analyzer-before.py'
    old.write_bytes(subprocess.check_output(['git', 'show', '47ebe83:scripts/analyse-canvas-log.py'], cwd=ROOT))
    for name in paths:
        if not name.endswith(('.json', '.json.gz')):
            continue
        path = ROOT / name
        try:
            raw = gzip.decompress(path.read_bytes()) if name.endswith('.gz') else path.read_bytes()
            doc = json.loads(raw)
        except (ValueError, OSError):
            continue
        if not isinstance(doc, dict) or doc.get('spike') != 'canvas':
            continue
        p, new_data = analyze(path)
        expected = name == 'reviews/evidence/M0-C0.3-r2/page-slow.json'
        assert (p.returncode == 2 and p.stdout == '') if expected else p.returncode == 0, (name, p.stderr)
        prior, old_data = analyze(path, old)
        comparison = 'not accepted by old analyzer'
        if prior.returncode == 0:
            # New successful-run metadata is additive; all prior statistics must be byte-value equivalent.
            normalized = copy.deepcopy(new_data)
            for run in normalized['runs']:
                run.pop('complete', None)
                run.pop('cadence', None)
            assert normalized == old_data, name
            comparison = 'all pre-existing JSON statistics equal'
        logs.append(dict(path=name, version=doc['version'], exitCode=p.returncode,
                         stderr=p.stderr.strip(), oldEstimatorComparison=comparison))
        if name.startswith('target/'):
            (OUT / (path.stem + '.json.gz')).write_bytes(gzip.compress(raw, mtime=0))
            (OUT / (path.stem + '.analysis.json')).write_text(json.dumps(new_data, indent=2) + '\n')
            (OUT / (path.stem + '.analysis.md')).write_text(p.stdout, encoding='utf-8')
(OUT / 'committed-log-audit.json').write_text(json.dumps(logs, indent=2) + '\n')
print(json.dumps(dict(probes=len(results), passed=sum(r['passed'] for r in results),
                     logs=len(logs), rejected=[r['path'] for r in logs if r['exitCode']],
                     unchangedEstimators=sum(r['oldEstimatorComparison'].startswith('all') for r in logs)), indent=2))
