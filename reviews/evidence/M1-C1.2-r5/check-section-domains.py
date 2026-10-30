"""Explicit owning-section domains must survive context widening; probe values on both sides of each domain."""
import json
from pathlib import Path
import re
from jsonschema import Draft202012Validator as Validator

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
rows = {r['id']: r for r in json.loads((ROOT / 'crates/registry/src/declared.json').read_text(encoding='utf-8'))}
source = (ROOT / 'spec/09_playback_export.md').read_text(encoding='utf-8')
definition = re.search(r'`PrintParams` = `(.*?)`\.', source, re.S).group(1)
expected = {m.group(1): re.findall(r'"([^"]+)"', m.group(2)) for m in
            re.finditer(r'(\w+)\??:\s*((?:"[^"]+"\|\s*)+"[^"]+")', definition)}
assert set(expected) == {'layout', 'type', 'range', 'orientation'}
checks = []
for cid, part, prefix in [('print.settings.get', 'result', []), ('print.settings.set', 'result', []),
                           ('print.settings.set', 'params', ['properties', 'values']),
                           ('print.preview.get', 'params', []), ('print.run', 'params', [])]:
    schema = rows[cid][part]
    for token in prefix:
        schema = schema[token]
    for field, values in expected.items():
        prop = schema['properties'][field]
        assert prop['enum'] == values, (cid, part, field)
        assert all(Validator(prop).is_valid(v) for v in values)
        assert not Validator(prop).is_valid('__r5_outside_set__')
        checks.append({'id': cid, 'part': part, 'path': prefix + ['properties', field], 'values': values,
                       'allMembersAccepted': True, 'outsideSetRejected': True,
                       'source': 'spec/09_playback_export.md:1077'})
prop = rows['xsheet.lightTable.set']['params']['properties']['placement']
assert prop['enum'] == ['over', 'under']
assert all(Validator(prop).is_valid(v) for v in ['over', 'under'])
assert not Validator(prop).is_valid('sideways')
checks.append({'id': 'xsheet.lightTable.set', 'part': 'params', 'field': 'placement', 'values': prop['enum'],
               'source': 'spec/04_timeline_xsheet.md:1460', 'outsideSetRejected': True})
out = {'owningSectionDomainsChecked': len(checks), 'checks': checks,
       'sampleWidening': {'id': 'xsheet.lightTable.add', 'path': 'params/items[]/placement',
                          'ownColumn': 'items?: [object]', 'formerSetOrigin': 'sibling xsheet.lightTable.set parameter',
                          'ruling': 'Untyped context-inferred field receives type/structure; the explicitly stated set on the owning .set field remains enforced.'}}
(HERE / 'section-domains.json').write_text(json.dumps(out, indent=2) + '\n', encoding='utf-8')
print(json.dumps(out, indent=2))
