"""Independently compare the actual generated TypeScript tables with the fresh runtime catalogue.

Node strips only TypeScript syntax. Bigints are tagged for lossless comparison in Python.
"""
import copy
import hashlib
import json
from pathlib import Path
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
module = ROOT / 'desktop/src/ipc/generated/commands.ts'
script = ('import {COMMAND_INDEX,COMMAND_DEFS} from ' + json.dumps(module.as_uri()) + ';'
          'process.stdout.write(JSON.stringify({index:COMMAND_INDEX,defs:COMMAND_DEFS},'
          '(k,v)=>typeof v === "bigint" ? {__review_bigint:v.toString()} : v));')
p = subprocess.run(['node', '--experimental-strip-types', '--input-type=module', '-e', script], cwd=ROOT, capture_output=True, check=True)
(HERE / 'frontend-schemas-dump.json').write_bytes(p.stdout)
def lossless(v):
    if isinstance(v, dict):
        if set(v) == {'__review_bigint'}:
            return int(v['__review_bigint'])
        return {k: lossless(x) for k, x in v.items()}
    if isinstance(v, list):
        return [lossless(x) for x in v]
    return v
frontend = lossless(json.loads(p.stdout))
runtime = json.loads((HERE / 'registry.json').read_text(encoding='utf-8'))
assert set(frontend['index']) == {r['id'] for r in runtime}
def qualify(v, cid, part):
    if isinstance(v, dict):
        for k, x in v.items():
            if k == '$ref' and x.startswith('#/$defs/'):
                v[k] = '#/$defs/' + cid + '.' + x[len('#/$defs/'):]
            elif k == '$ref' and x == '#':
                v[k] = '#/$defs/' + cid + '.' + part
            else:
                qualify(x, cid, part)
    elif isinstance(v, list):
        for x in v:
            qualify(x, cid, part)
checked = []
for row in runtime:
    cid = row['id']
    assert frontend['index'][cid]['kind'] == row['kind']
    for part in ['params', 'result']:
        original = copy.deepcopy(row[part + 'Schema'])
        qualify(original, cid, part)
        original.pop('$schema', None)
        definitions = original.pop('$defs', {})
        root_key = frontend['index'][cid][part]
        assert frontend['defs'][root_key] == original, (cid, part)
        for name, definition in definitions.items():
            assert frontend['defs'][cid + '.' + name] == definition, (cid, name)
        checked.append(root_key)
out = {'entries': len(runtime), 'rootSchemasMatched': len(checked), 'allNestedDefinitionsMatched': True,
       'registrySha256': hashlib.sha256((HERE / 'registry.json').read_bytes()).hexdigest(),
       'typescriptSha256': hashlib.sha256(module.read_bytes()).hexdigest(),
       'extractedTablesSha256': hashlib.sha256(p.stdout).hexdigest(),
       'nodeStderr': p.stderr.decode('utf-8'), 'bigintBoundsComparedLosslessly': True}
(HERE / 'frontend-registry-match.json').write_text(json.dumps(out, indent=2) + '\n', encoding='utf-8')
print(json.dumps(out, indent=2))
