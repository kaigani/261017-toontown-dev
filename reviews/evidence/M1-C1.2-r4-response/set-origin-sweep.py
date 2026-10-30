"""Round-4 response (builder; C12-R4-02): every `enum`/`const` set in the current declarations that the pre-round-3
declarations (`be5ef32`) did not have at the same position, with where it is stated. A set that context typing
inherited from a sibling would have no statement in its own entry; after the fix the generator inherits none.
"""
import json
import re
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
now = {r['id']: r for r in json.loads((ROOT / 'crates/registry/src/declared.json').read_text(encoding='utf-8'))}
text09 = (ROOT / 'spec').glob('09_*.md').__next__().read_text(encoding='utf-8')
print_params = re.search(r"`PrintParams` = `(.*?)`\.", text09, re.S).group(1)
old = {r['id']: r for r in json.loads(subprocess.check_output(['git', 'show', 'be5ef32:crates/registry/src/declared.json'], cwd=ROOT))}


def sets(v, path, out):
    if isinstance(v, dict):
        for k in ('enum', 'const'):
            if k in v:
                out[path] = v[k]
        for k, x in v.items():
            sets(x, f'{path}/{k}', out)
    elif isinstance(v, list):
        for i, x in enumerate(v):
            sets(x, f'{path}/{i}', out)


rows = []
for cid, row in now.items():
    for part in ('params', 'result'):
        a, b = {}, {}
        sets(row[part], part, a)
        sets(old.get(cid, {}).get(part, {}), part, b)
        spec = row[part].get('x-spec') or ''
        both = (row['params'].get('x-spec') or '') + ' ' + (row['result'].get('x-spec') or '')
        for path, values in a.items():
            if b.get(path) == values:
                continue
            vals = values if isinstance(values, list) else [values]
            text = [v if isinstance(v, str) else json.dumps(v) for v in vals]
            if '/$defs/' in path:
                origin = 'named type ($defs)'
            elif all(t in spec for t in text):
                origin = 'own spec column'
            elif all(t in both for t in text):
                origin = 'own entry (other column)'
            elif cid == 'print.settings.set' and all(t in print_params for t in text):
                origin = '09 PrintParams'
            else:
                origin = 'UNSTATED'
            rows.append({'id': cid, 'path': path, 'values': values, 'origin': origin, 'spec': spec})
summary = {'newSets': len(rows), 'byOrigin': {o: sum(r['origin'] == o for r in rows) for o in sorted({r['origin'] for r in rows})},
           'unstated': [r for r in rows if r['origin'] == 'UNSTATED']}
(HERE / 'set-origin-sweep.json').write_text(json.dumps({'summary': summary, 'rows': rows}, indent=2) + '\n', encoding='utf-8')
print(json.dumps(summary, indent=2))
