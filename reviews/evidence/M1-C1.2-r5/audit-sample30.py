"""Scoped independent set audit: reproducible random declarations, complete fix delta, and adversarial context input.

Own-column domains are read from spec A, not inferred from the generator. Named definitions must be unchanged
from the accepted baseline. All inherited context schemas are observed during scratch-only generation.
"""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import random
import re
import subprocess
import tempfile

from jsonschema import Draft202012Validator as Validator

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SEED = 0xC12520261019

def save(name, value):
    (HERE / name).write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')

def load_rows(data):
    return {r['id']: r for r in json.loads(data)}

now = load_rows((ROOT / 'crates/registry/src/declared.json').read_text(encoding='utf-8'))
old = load_rows(subprocess.check_output(['git', 'show', 'c25c9d8:crates/registry/src/declared.json'], cwd=ROOT))
ids = random.Random(SEED).sample(sorted(now), 30)
assert ids == json.loads((HERE / 'sample30.json').read_text())['ids']

appendix = {}
for number, line in enumerate((ROOT / 'spec/A_command_registry.md').read_text(encoding='utf-8').splitlines(), 1):
    if re.match(r'\| `[^`]+` \| (action|toggle|query|job|ui|event) \|', line):
        cells = [c.strip().replace(r'\|', '|') for c in re.split(r'(?<!\\)\|', line)[1:-1]]
        appendix[cells[0].strip('`')] = {'params': cells[3], 'result': cells[4], 'line': number}

def walk(v, path=()):
    if isinstance(v, dict):
        yield path, v
        for k, x in v.items():
            yield from walk(x, path + (k,))
    elif isinstance(v, list):
        for i, x in enumerate(v):
            yield from walk(x, path + (i,))

def sets(v):
    return {p + (k,): x[k] for p, x in walk(v) for k in ('enum', 'const') if k in x}

def at(v, path):
    for p in path:
        v = v[p]
    return v

# The complete actual change removes exactly the foreign sets and otherwise preserves each schema's structure.
# Every old member still validates against the widened node; explicit domain preservation is checked below.
removed = []
for cid, row in now.items():
    for part in ('params', 'result'):
        a, b = sets(old[cid][part]), sets(row[part])
        assert all(a.get(p) == value for p, value in b.items()), (cid, part, 'new or modified set')
        for p, values in a.items():
            if p in b:
                continue
            node = at(row[part], p[:-1])
            members = values if p[-1] == 'enum' else [values]
            assert all(Validator(node).is_valid(v) for v in members), (cid, p, node)
            removed.append({'id': cid, 'part': part, 'path': list(p), 'before': values, 'after': node})
        assert old[cid][part].get('$defs') == row[part].get('$defs'), (cid, part, 'named definition changed')
assert len(removed) == 69 and len({r['id'] for r in removed}) == 62

# Run the real context-selection function with nested enum/const sources and inspect every context produced
# during generation. Only evidence scratch files are writable.
spec = importlib.util.spec_from_file_location('r5_generator', ROOT / 'scripts/gen-command-declarations.py')
gen = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gen)
original = gen.field_index
contexts = {}
def capture(entry, raw):
    result = original(entry, raw)
    assert not sets(result), (entry['id'], sets(result))
    contexts[entry['id']] = copy.deepcopy(result)
    return result
gen.field_index = capture
with tempfile.TemporaryDirectory(prefix='sample30-', dir=HERE) as folder:
    scratch = Path(folder).resolve()
    assert scratch.is_relative_to(HERE.resolve())
    for attr, rel in [('OUT_RS', 'crates/registry/src/declared.rs'), ('OUT_JSON', 'crates/registry/src/declared.json'), ('ISSUES', 'plan/spec-issues.md')]:
        target = scratch / Path(rel).name
        target.write_bytes((ROOT / rel).read_bytes())
        setattr(gen, attr, target)
    assert gen.main() == 0
    assert gen.OUT_JSON.read_bytes() == (ROOT / 'crates/registry/src/declared.json').read_bytes()

sample = []
for cid in ids:
    own_sets = []
    for part in ('params', 'result'):
        source = appendix[cid][part]
        assert now[cid][part]['x-spec'] == source
        # All non-named sample sets are directly written by their owning command column.
        for path, value in sets(now[cid][part]).items():
            if '$defs' in path:
                continue
            vals = value if path[-1] == 'enum' else [value]
            assert all(json.dumps(v, ensure_ascii=False) in source for v in vals), (cid, path, source)
            own_sets.append({'part': part, 'path': list(path), 'set': value, 'source': f"spec/A_command_registry.md:{appendix[cid]['line']}"})
        # Conversely, every explicitly enumerated field in this sample must retain the exact set.
        for m in re.finditer(r'(\w+)\??:\s*(?:enum\s+)?((?:"[^"]+"\|)+"[^"]+")', source):
            key, enum = m.group(1), re.findall(r'"([^"]+)"', m.group(2))
            prop = now[cid][part]['properties'][key]
            assert prop.get('enum') == enum, (cid, key, enum, prop)
            assert all(Validator(prop).is_valid(v) for v in enum)
            assert not Validator(prop).is_valid('__outside_own_domain__')
    sample.append({'id': cid, 'contextObserved': cid in contexts,
                   'contextHasNoSets': not sets(contexts[cid]), 'ownSets': own_sets,
                   'namedDefinitionsUnchanged': True,
                   'removedForeignSets': [r for r in removed if r['id'] == cid]})

controls = []
for cid, field, expected in [('export.publish.account.close', 'connected', False),
                              ('storyline.play.start', 'playing', True), ('storyline.play.stop', 'playing', False)]:
    node = now[cid]['result']['properties'][field]
    assert node == {'const': expected}
    assert Validator(node).is_valid(expected) and not Validator(node).is_valid(not expected)
    controls.append({'id': cid, 'field': field, 'constRetained': expected, 'oppositeRejected': True,
                     'source': f"spec/A_command_registry.md:{appendix[cid]['line']}"})

def obj(**properties):
    return {'type': 'object', 'properties': properties}
source = obj(items={'type': 'array', 'items': obj(mode={'enum': ['only-a', 'only-b']}, connected={'const': False},
                                               numeric={'const': 3}, mixed={'enum': [None, 'one']})})
raw = {'review.same.first': ({}, source), 'review.same.next': (obj(items={}), {})}
synthetic = original({'id': 'review.same.next'}, raw)['items']
assert not sets(synthetic)
assert Validator(synthetic).is_valid([{'mode': 'independent', 'connected': True, 'numeric': 0.625, 'mixed': None}])
assert not Validator(synthetic).is_valid([{'mode': 42, 'connected': 'yes', 'numeric': '3', 'mixed': {}}])

summary = {'seed': hex(SEED), 'sampleSize': len(sample), 'contextDeclarationsAudited': len(contexts),
           'foreignSetsRemoved': len(removed), 'affectedDeclarations': len({r['id'] for r in removed}),
           'newOrModifiedSets': 0, 'sampleDirectOwnSets': sum(len(r['ownSets']) for r in sample),
           'allNamedDefinitionsUnchanged': True, 'controls': controls,
           'syntheticContext': synthetic,
           'qualification': 'Scope is fix-induced narrowing and preservation of accepted owning domains, not reopening prior schema rulings.'}
save('sample30-audit.json', {'summary': summary, 'rows': sample, 'removedSets': removed})
print(json.dumps(summary, indent=2))
