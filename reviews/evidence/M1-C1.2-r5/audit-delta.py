"""Check the full generated schema delta separately from regenerated sample values."""
import json
from pathlib import Path
import subprocess
from jsonschema import Draft202012Validator as Validator

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
old = {r['id']: r for r in json.loads(subprocess.check_output(['git', 'show', 'c25c9d8:crates/registry/src/declared.json'], cwd=ROOT))}
now = {r['id']: r for r in json.loads((ROOT / 'crates/registry/src/declared.json').read_text(encoding='utf-8'))}
changes = []
def diff(a, b, path):
    if isinstance(a, dict) and isinstance(b, dict):
        for k in sorted(a.keys() | b.keys()):
            if k not in b:
                changes.append({'path': path + [k], 'kind': 'removed', 'old': a[k]})
            elif k not in a:
                changes.append({'path': path + [k], 'kind': 'added', 'new': b[k]})
            else:
                diff(a[k], b[k], path + [k])
    elif isinstance(a, list) and isinstance(b, list) and len(a) == len(b):
        for i, (x, y) in enumerate(zip(a, b)):
            diff(x, y, path + [i])
    elif a != b:
        changes.append({'path': path, 'kind': 'changed', 'old': a, 'new': b})

samples = []
unsampleable = []
for cid, row in now.items():
    for part in ('params', 'result'):
        diff(old[cid][part], row[part], [cid, part])
        if 'x-unsampleable' in row['samples'][part]:
            assert row['samples'][part] == old[cid]['samples'][part]
            assert row[part] == old[cid][part]
            unsampleable.append({'id': cid, 'part': part, 'sentinelAndSchemaUnchanged': True})
        else:
            assert Validator(row[part]).is_valid(row['samples'][part]), (cid, part)
    for key in row.keys() - {'params', 'result', 'samples'}:
        assert old[cid][key] == row[key], (cid, key)
    if old[cid]['samples'] != row['samples']:
        samples.append(cid)
assert all((c['kind'] == 'removed' and c['path'][-1] in ('enum', 'const')) or
           (c['kind'] == 'added' and c['path'][-1] == 'type') for c in changes)
out = {'onlyRemovedSetsAndAddedTypes': True, 'schemaLeafChanges': len(changes), 'changes': changes,
       'regeneratedSampleRows': samples, 'validSamples': 1324 - len(unsampleable),
       'unchangedUnsampleableSentinels': unsampleable,
       'otherDeclarationMetadataUnchanged': True}
(HERE / 'schema-delta.json').write_text(json.dumps(out, indent=2) + '\n', encoding='utf-8')
print(json.dumps({k: v for k, v in out.items() if k != 'changes'}, indent=2))
