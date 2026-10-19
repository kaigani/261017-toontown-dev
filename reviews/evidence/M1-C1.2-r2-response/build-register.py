"""Round-2 response register (builder): re-applies the reviewer's R2 criteria to the current registry.

Inputs: registry.json (dumped by `c12_r2_contracts::export_registry_for_independent_audit` with
TT_C12_REVIEW_ARTIFACT_DIR set to this directory), the reviewer's register (../M1-C1.2-r2/command-register.json),
crates/registry/src/declared.json (placeholders) and plan/spec-issues.md (SI numbers).

The hole criterion `holes()` and the metadata checks are copied verbatim from the reviewer's
../M1-C1.2-r2/prepare-audit.py and build-registers.py, so a row counts as fixed only by the reviewer's own test.
Outputs: command-register.json (every row, old and new status, remaining issues, placeholders with their spec issue)
and command-register.md (the table of every row the reviewer classed divergent).
"""
import json
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def holes(s, at=''):  # verbatim from the reviewer's prepare-audit.py
    out = []
    if not isinstance(s, dict):
        return out
    if not set(s).intersection({'type', '$ref', 'enum', 'const', 'oneOf', 'anyOf', 'allOf', 'not', 'required'}):
        out.append({'pointer': at or '/', 'schema': s})
    if 'x-specType' in s:
        out.append({'pointer': at or '/', 'schema': s})
    for k in ('properties', '$defs'):
        for n, v in s.get(k, {}).items():
            out += holes(v, at + '/' + k + '/' + n)
    if isinstance(s.get('items'), dict):
        out += holes(s['items'], at + '/items')
    for k in ('oneOf', 'anyOf', 'allOf'):
        for i, v in enumerate(s.get(k, [])):
            out += holes(v, at + '/' + k + '/' + str(i))
    if isinstance(s.get('x-jobResult'), dict):
        out += holes(s['x-jobResult'], at + '/x-jobResult')
    return out


rows = {r['id']: r for r in json.loads((HERE / 'registry.json').read_text(encoding='utf-8'))}
r2 = {r['id']: r for r in json.loads((HERE.parent / 'M1-C1.2-r2' / 'command-register.json').read_text(encoding='utf-8'))}
declared = {e['id']: e for e in json.loads((ROOT / 'crates/registry/src/declared.json').read_text(encoding='utf-8'))}
issues_md = (ROOT / 'plan/spec-issues.md').read_text(encoding='utf-8')
si_of = {m.group(2): f"SI-{m.group(1)}" for m in re.finditer(r"^## SI-(\d+) — `(\w+)\.\*`", issues_md, re.M)}

# The reviewer's per-entry checks (build-registers.py), unchanged.
bare_results = {'draw.drawing.get', 'element.get', 'lipsync.settings.set', 'sound.editor.state.set',
                'timeline.rows.list', 'xsheet.exposures.get'}
global_writes = {'library.item.delete', 'library.item.duplicate', 'library.item.move'}
no_project = {'library.open', 'library.close', 'library.create', 'library.list', 'library.tree.get'}
# Round-2 findings per hand-written command; each is closed by the named reviewer regressions (all pass).
fixed_findings = {
    'checkpoint.create': 'C12-R2-03', 'checkpoint.list': 'C12-R2-03', 'checkpoint.delete': 'C12-R2-03',
    'doc.diff': 'C12-R2-12', 'doc.object.patch': 'C12-R2-11', 'job.wait': 'C12-R2-06/07',
    'edit.clipboard.set': 'C12-R2-10', 'edit.copy': 'C12-R2-05/08/13', 'edit.cut': 'C12-R2-05/08/13',
    'edit.delete': 'C12-R2-05/08', 'edit.cutSpecial': 'C12-R2-05/08', 'edit.paste': 'C12-R2-04/05/08/09/13/14',
    'edit.pasteSpecial': 'C12-R2-04/05/08/09/13/14', 'edit.pasteAsNewElement': 'C12-R2-04/08',
}

register = []
for cid, r in sorted(rows.items()):
    # As the reviewer's build-registers.py: the hole criterion applies to generated declarations; hand-written
    # contracts that permit any JSON value at a position are listed separately (`anyJson`).
    found = [{'field': 'params' + h['pointer'], 'reason': 'hole'} for h in holes(r['paramsSchema'])]
    found += [{'field': 'result' + h['pointer'], 'reason': 'hole'} for h in holes(r['resultSchema'])]
    issues = found if cid in declared else []
    any_json = [] if cid in declared else [f['field'] for f in found]
    if cid in bare_results and r['resultSchema'].get('type') == 'object' and not r['resultSchema'].get('properties') \
            and '$ref' not in r['resultSchema'] and 'allOf' not in r['resultSchema']:
        issues.append({'field': 'resultSchema', 'reason': 'whole named result is only type:object'})
    if cid in global_writes and 'library.global' not in r['scopes']:
        issues.append({'field': 'scopes', 'reason': 'global-library writes lack library.global'})
    if cid in no_project and not r['needsProject']:
        issues.append({'field': 'needsProject', 'reason': '01 §1.3 only exempts saving the global library'})
    if cid == 'xsheet.columnWidth.set' and not isinstance(
            r['resultSchema'].get('properties', {}).get('widths', {}).get('additionalProperties'), dict):
        issues.append({'field': 'resultSchema/properties/widths', 'reason': 'map values unconstrained'})
    if 'see spec' in (r.get('enablement') or ''):
        issues.append({'field': 'enablement', 'reason': 'generic enablement text'})
    placeholders = declared.get(cid, {}).get('placeholders', [])
    status = 'divergent' if issues else ('implemented' if r['status'] == 'implemented' else 'deferred-with-owner')
    register.append({
        'id': cid, 'r2Status': r2[cid]['status'], 'status': status, 'declaredStatus': r['status'],
        'owner': r.get('milestone', 'M1 C1.2'), 'issues': issues,
        'r2Findings': r2[cid]['findings'], 'fixedBy': fixed_findings.get(cid),
        'r2SchemaIssues': len(r2[cid]['schemaIssues']),
        'placeholders': [dict(p, issue=si_of.get(cid.split('.')[0])) for p in placeholders],
        'anyJson': any_json,
    })
(HERE / 'command-register.json').write_text(json.dumps(register, indent=1, ensure_ascii=False) + '\n', encoding='utf-8')

counts = {}
for x in register:
    counts[x['status']] = counts.get(x['status'], 0) + 1
lines = [
    '# Inventory register, round-2 response',
    '',
    f"Every row the reviewer classed `divergent` in round 2 ({sum(1 for x in register if x['r2Status'] == 'divergent')}), "
    f"re-checked with the reviewer's own criteria (`build-register.py`). Now: {counts}.",
    '',
    '| Command | R2 | Now | R2 schema issues | Closed by | Strict placeholders (spec issue) |',
    '|---|---|---|---:|---|---|',
]
for x in register:
    if x['r2Status'] != 'divergent':
        continue
    closed = x['fixedBy'] or ('C12-R2-02 (generator)' if 'C12-R2-02' in x['r2Findings'] else '—')
    ph = f"{len(x['placeholders'])} ({x['placeholders'][0]['issue']})" if x['placeholders'] else '—'
    lines.append(f"| `{x['id']}` | divergent | {x['status']} | {x['r2SchemaIssues']} | {closed} | {ph} |")
(HERE / 'command-register.md').write_text('\n'.join(lines) + '\n', encoding='utf-8')
print(counts)
print('remaining issues:', [(x['id'], x['issues']) for x in register if x['issues']][:20])
print('hand-written any-JSON positions:', [(x['id'], x['anyJson']) for x in register if x['anyJson']])
