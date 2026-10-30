"""Round-3 response (builder; re-run unchanged in the round-4 response): the reviewer's fourteen semantic cases (../M1-C1.2-r3/semantic-schemas.py, copied
verbatim) re-run against the current registry dump (`registry.json`, written by the reviewer's
`c12_r2_contracts::export_registry_for_independent_audit` with TT_C12_REVIEW_ARTIFACT_DIR set to this directory).
The reviewer's script asserted that every case is rejected; here every case must be accepted.
"""
import json
from pathlib import Path

from jsonschema import Draft202012Validator

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
rows = {r['id']: r for r in json.loads((HERE / 'registry.json').read_text(encoding='utf-8'))}


def example(file, heading):
    text = (ROOT / 'spec' / file).read_text(encoding='utf-8')
    return json.loads(text.split(heading, 1)[1].split('```json', 1)[1].split('```', 1)[0])


path = example('05_scene_planning.md', '**Path**')
func = example('05_scene_planning.md', '**Function**')
func.update(channel='velocity', pins=[{'frame': 1, 'value': 0}, {'frame': 24, 'value': 14.37}])
cases = [
    ('anim.path.get', 'result', path, '05 section 19.4 exact Path example'),
    ('anim.function.get', 'result', func, '05 section 19.4 Function example and explicit velocity pins prose'),
    ('draw.pointSelection.set', 'result', {'selectedPoints': [{'strokeId': 'stroke-r3', 'anchor': 2}]}, '02 and A: anchor is integer index'),
    ('xsheet.panel.set', 'result', {'elementList': True, 'lightTable': False, 'toolbar': True}, '04 and A: three boolean panels'),
    ('prefs.list', 'result', [{'key': 'general.workspace.autoSave', 'type': 'bool', 'default': True, 'value': True, 'applies': 'live', 'description': 'Save workspace automatically'}], '01 preference types include bool'),
    ('keymap.list', 'result', [{'commandId': 'app.frame.set', 'params': {'frame': 7}, 'keys': 'Primary+7', 'context': 'global', 'isDefault': False}], '01 section 12.4: arbitrary command parameter object'),
    ('tool.polyline.options.set', 'result', {'options': {'snap': True, 'snapRadius': 8}}, '02 polyline options and A parameter fields'),
    ('print.settings.set', 'params', {'values': {'copies': 2}}, '09 PrintParams copies plus A values object'),
    ('export.settings.get', 'result', {'preset': 'default', 'settings': {}}, '09 settings are an object in paired export.settings.set'),
    ('drawAid.staticLightTable.set', 'result', {'entries': [{'entryId': 'r3-entry', 'sourceId': 'r3-drawing', 'sourceKind': 'drawing', 'visible': True, 'placement': 'underlay'}]}, '02 section 13.5 entries defined by staticLightTable.list'),
    ('tool.pen.set', 'result', {'pen': {'id': 'pen_r3', 'type': 'pen', 'rev': 1, 'name': 'Pen 1', 'kind': 'brush', 'minSize': 0.5, 'maxSize': 2, 'smoothness': 3, 'tip': 'round', 'tipAngle': 0, 'colourId': None}}, '02 section 6.1 and 10 section 4.2: Pen is a model object'),
]
for cid in ['sound.envelope.set', 'sound.envelope.point.move', 'sound.envelope.point.remove']:
    cases.append((cid, 'result', {'points': [{'time': 0.25, 'left': 0.5, 'right': 0.75}]}, '07 envelope points: time, left, right'))
out = []
for cid, part, value, source in cases:
    errors = list(Draft202012Validator(rows[cid][part + 'Schema']).iter_errors(value))
    out.append({'id': cid, 'part': part, 'source': source, 'accepted': not errors,
                'violations': [{'path': list(e.path), 'message': e.message} for e in errors]})
(HERE / 'semantic-recheck.json').write_text(json.dumps(out, indent=2) + '\n', encoding='utf-8')
print(json.dumps({'cases': len(out), 'accepted': sum(r['accepted'] for r in out)}, indent=2))
assert len(out) == 14 and all(r['accepted'] for r in out), [r['id'] for r in out if not r['accepted']]
