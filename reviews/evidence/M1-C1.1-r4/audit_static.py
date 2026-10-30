"""Reproduce the scoped test-edit/API/CI audit without altering historical evidence."""
from pathlib import Path
from collections import Counter
import json
import re
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
BASE, HEAD = 'c38109b', '3b6179f'

def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT).decode('utf-8')

def canonical(source):
    # The added comments are standalone lines. Whitespace-only rustfmt changes are immaterial.
    return re.sub(r'\s+', '', '\n'.join(l for l in source.splitlines() if not l.lstrip().startswith('//')))

replacements = {
    'crates/model/tests/c11_r1_adversarial.rs': [
        ('change::invert(&patch)', 'change::invert_against(&a, &patch).unwrap()', 2)],
    'crates/model/tests/c11_r2_adversarial.rs': [
        ('change::invert(&patch)', 'change::invert_against(&a, &patch).unwrap()', 1),
        ('change::apply(&value, &change::invert(&[p])).is_err()', 'change::invert_against(&value, &[p]).is_err()', 1)],
    'crates/model/tests/c11_r3_adversarial.rs': [
        ('change::invert(&patch)', 'change::invert_against(&before, &patch).unwrap()', 1)],
}
edits = []
for file, changes in replacements.items():
    old = git('show', f'{BASE}:{file}')
    new = git('show', f'{HEAD}:{file}')
    expected = old
    for a, b, count in changes:
        assert expected.count(a) == count, (file, a)
        expected = expected.replace(a, b)
    assert canonical(expected) == canonical(new), file
    names = lambda source: re.findall(r'^fn ([a-zA-Z0-9_]+)\(', source, re.M)
    assert names(old) == names(new), file
    edits.append(dict(file=file, replacements=changes, exact_after_removing_comments_and_whitespace=True))

paths = ['crates/model/tests', 'crates/engine/tests', 'desktop/src/ipc', 'desktop/src/test', 'tests/adversarial']
changed = git('diff', '--name-only', f'{BASE}..{HEAD}', '--', *paths).splitlines()
other = ['crates/model/tests/conformance.rs', 'crates/model/tests/properties.rs',
         'desktop/src/ipc/generated/model.ts', 'desktop/src/ipc/wireJson.ts']
assert set(changed) == set(replacements) | set(other), changed
audit = dict(base=BASE, head=HEAD, reviewer_test_call_sites=5, reviewer_test_files=edits,
             all_requested_path_changes=changed, other_files=dict(zip(other, [
                 'Builder-owned inversion caller; equality expectations unchanged.',
                 'Builder-owned inversion callers/import qualification; property assertions unchanged.',
                 'Generated product schema; four x-since additions, no reviewer assertions.',
                 'Product numeric-coercion fix for R3-05, no reviewer assertions.'])))
(HERE / 'test-edit-audit.json').write_text(json.dumps(audit, indent=2)+'\n', encoding='utf-8')

api = {}
pattern = re.compile(r'^\s*pub (?!\()(?:const |async |unsafe )?(?:fn|struct|enum|trait|type|mod|use|const|static)\b[^\n]*', re.M)
for rev in [BASE, HEAD]:
    items = []
    for file in git('ls-tree', '-r', '--name-only', rev, '--', 'crates/model/src').splitlines():
        if file.endswith('.rs'):
            items.extend((file, s.strip()) for s in pattern.findall(git('show', f'{rev}:{file}')))
    api[rev] = Counter(items)
delta = dict(added=list((api[HEAD]-api[BASE]).elements()), removed=list((api[BASE]-api[HEAD]).elements()),
             method='Public source declarations across all tt-model src files; changed modules also inspected manually, including macro and re-export stability.')
# JSON converts tuple pairs to arrays. Removal is the only public item difference.
assert delta['added'] == []
assert delta['removed'] == [('crates/model/src/change.rs', 'pub fn invert(patch: &[PatchOp]) -> Vec<PatchOp> {')], delta
diff = git('diff', '386f032^', '386f032', '--', 'crates/model/src', 'desktop/src/ipc/wireJson.ts')
delta['added_allow_unsafe_mut_lines'] = [l for l in diff.splitlines() if l.startswith('+') and not l.startswith('+++') and re.search(r'#\[allow|\bunsafe\b|&(?:\x27\w+\s+)?mut\b', l)]
delta['interpretation'] = 'No new allow or unsafe. Added mutable references are private schema traversal over caller-owned schemas; new Issues mutation is test-only. No Document accessor or public signature changed other than invert removal.'
(HERE / 'api-audit.json').write_text(json.dumps(delta, indent=2)+'\n', encoding='utf-8')

metadata = json.loads((HERE / 'logs/ci-metadata.log').read_text(encoding='utf-8'))
raw = (HERE / 'logs/ci-full.log').read_text(encoding='utf-8')
text = re.sub(r'\x1b\[[0-9;]*[a-zA-Z]', '', raw)
assert metadata['headSha'] == git('rev-parse', HEAD).strip()
assert len(metadata['jobs']) == 8
assert all(j['conclusion'] == 'success' for j in metadata['jobs'])
jobs = []
for job in metadata['jobs']:
    lines = [l for l in text.splitlines() if l.startswith(job['name']+'\t')]
    log = '\n'.join(lines)
    is_check = job['name'].startswith('check (')
    item = dict(name=job['name'], conclusion=job['conclusion'], url=job['url'],
                steps=[dict(name=s['name'], conclusion=s['conclusion']) for s in job['steps']],
                reviewer_targets=sorted(set(re.findall(r'(c11_r[123]_(?:wire|adversarial|random|register))\.rs', log))),
                frontend_review_targets=sorted(set(re.findall(r'c11-r[123]-adversarial\.test\.ts', log))))
    for key, marker in [('workspace', 'Run cargo test --workspace --locked'), ('diagnostics', 'Desktop host with the M0 diagnostics feature')]:
        totals = [tuple(map(int, m)) for l in lines if marker in l for m in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored', l)]
        item[key+'_counts'] = [sum(x[i] for x in totals) for i in range(3)]
    if is_check:
        assert len(item['reviewer_targets']) == 8, item
        assert len(item['frontend_review_targets']) == 3, item
        assert '326 passed' in log and '# pass 9' in log
        item['vitest_passed'] = 326
        item['node_passed'] = 9
    jobs.append(item)
summary = dict(head=metadata['headSha'], url=metadata['url'], jobs=jobs,
               policy_suite_in_ci='c11-r2-review-policy.py' in text,
               limitation='Python policy suite is not invoked by this workflow; checked locally. Linux covers raster and no-display headless, not full desktop suites.')
(HERE / 'ci-audit.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
print(json.dumps(dict(test_edit_sites=5, public_api_delta=delta, ci_jobs=[{k:j[k] for k in ['name','conclusion','workspace_counts','diagnostics_counts']} for j in jobs]), indent=2))
