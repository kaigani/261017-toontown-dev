"""Round-3 CLI probes; no product or prior reviewer files are modified."""
import copy
import gzip
import itertools
import json
import math
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
BASE = ROOT / 'reviews/evidence/M0-C0.3/canvas-r2-local-windows-rtx4090-auto.json.gz'
base = json.loads(gzip.decompress(BASE.read_bytes()))
results = []


def analyze(path):
    p = subprocess.run(['python', '-I', 'scripts/analyse-canvas-log.py', str(path)], cwd=ROOT,
                       capture_output=True, encoding='utf-8', env={**os.environ, 'PYTHONIOENCODING': 'utf-8'})
    return dict(exitCode=p.returncode, stdoutBytes=len(p.stdout.encode()), stderr=p.stderr.strip())


def probe(name, mutate, accepted=False):
    doc = copy.deepcopy(base)
    mutate(doc)
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / 'log.json'
        path.write_text(json.dumps(doc), encoding='utf-8')
        r = analyze(path)
    r.update(name=name, expected='accept' if accepted else 'reject-empty-stdout')
    r['pass'] = r['exitCode'] == 0 if accepted else r['exitCode'] != 0 and r['stdoutBytes'] == 0
    results.append(r)


weights = dict(groups=24, paths=28, points=8, verbs=1)
for a, b in itertools.combinations(weights, 2):
    amount = math.lcm(weights[a], weights[b], 4)
    for sign in [-1, 1]:
        def mutate(doc, a=a, b=b, amount=amount, sign=sign):
            # groups1k permits positive and negative changes in every field.
            for t in doc['runs'][1]['transfers']:
                t['header'][a] += sign * amount // weights[a]
                t['header'][b] -= sign * amount // weights[b]
        probe(f'compensating-{a}-{b}-{sign}', mutate)

for field in weights:
    def mutate(doc, field=field):
        for t in doc['runs'][0]['transfers']:
            h = t['header']
            h[field] += 1
            t['bytes'] = 72 + 24*h['groups'] + 28*h['paths'] + 8*h['points'] + 4*math.ceil(h['verbs']/4)
    probe(f'off-by-one-consistent-length-{field}', mutate)

def other_scene(doc):
    for t, other in zip(doc['runs'][0]['transfers'], doc['runs'][1]['transfers']):
        t['header'] = copy.deepcopy(other['header'])
        t['bytes'] = other['bytes']
probe('seed1-header-from-groups1k-in-paths100k', other_scene)
probe('unsupported-seed', lambda d: d['parameters'].update(seed=2))
probe('v1-current-fast-control', lambda d: d.update(version=1), True)
probe('missing-phases', lambda d: d['runs'][0].pop('phases'))
for phase in ['idle', 'panZoom']:
    probe(f'missing-{phase}', lambda d, phase=phase: d['runs'][0]['phases'].pop(phase))
    def reverse(d, phase=phase):
        s = d['runs'][0]['phases'][phase]
        s['start'], s['end'] = s['end'], s['start']
    probe(f'reversed-{phase}', reverse)
    probe(f'shorter-than-samples-{phase}', lambda d, phase=phase: d['runs'][0]['phases'][phase].update(end=d['runs'][0]['phases'][phase]['end']-100))

probe('early-frames-intact-10s-claim', lambda d: d['runs'][0].update(frames=d['runs'][0]['frames'][:100]))
def early_consistent(d):
    r = d['runs'][0]
    r['frames'] = r['frames'][:100]
    r['phases']['panZoom']['end'] = r['frames'][-1]['t']
probe('early-frames-and-early-span', early_consistent)

# Audit all tracked raw canvas logs, including deliberate old reviewer counterexamples.
logs = []
for name in subprocess.check_output(['git', 'ls-files', 'reviews/evidence'], cwd=ROOT, text=True).splitlines():
    if not name.endswith(('.json', '.json.gz')):
        continue
    p = ROOT / name
    try:
        data = json.loads(gzip.decompress(p.read_bytes()) if name.endswith('.gz') else p.read_bytes())
    except (ValueError, OSError):
        continue
    if isinstance(data, dict) and data.get('spike') == 'canvas':
        logs.append(dict(path=name, version=data.get('version'), **analyze(p)))
(OUT / 'analyzer-probes.json').write_text(json.dumps(results, indent=2)+'\n')
(OUT / 'committed-log-audit.json').write_text(json.dumps(logs, indent=2)+'\n')
print(json.dumps(dict(probes=len(results), passed=sum(r['pass'] for r in results), logs=len(logs),
                     rejectedLogs=[r for r in logs if r['exitCode']]), indent=2))
assert all(r['pass'] for r in results)
