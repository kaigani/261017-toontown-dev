"""Exact reviewed-tree, public API and CI log audit for the final scoped review."""
from pathlib import Path
from collections import Counter
import json
import re
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
HEAD = '0159179'

def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT).decode('utf-8')

def save(name, value):
    (HERE / name).write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')

metadata = json.loads((HERE / 'logs/ci-metadata.log').read_text(encoding='utf-8'))
raw = (HERE / 'logs/ci-full.log').read_text(encoding='utf-8')
text = re.sub(r'(?:\x1b|\^\[)\[[0-9;]*[a-zA-Z]', '', raw)
assert metadata['headSha'] == git('rev-parse', HEAD).strip()
assert len(metadata['jobs']) == 8 and all(j['conclusion'] == 'success' for j in metadata['jobs'])
rust_targets = ['c11_r1_adversarial', 'c11_r2_adversarial', 'c11_r3_adversarial', 'c11_r3_register',
                'c11_r3_random', 'c11_r4_exposure_bounds', 'c11_r5_write_bounds', 'c11_r6_staged_inverse', 'staged_inverse', 'staged_commit', 'c11_r1_wire', 'c11_r2_wire', 'c11_r3_wire',
                'c03_r2_encoder', 'adversarial_c01', 'c03_fixed_configurations', 'c03_r1_stress', 'c03_r2_quality']
ts_files = sorted([p.relative_to(ROOT / 'desktop').as_posix() for p in (ROOT / 'desktop/src/test').glob('*.test.*')]
                  + [p.relative_to(ROOT / 'desktop').as_posix() for p in (ROOT / 'desktop/src/ipc').glob('c11-r*-adversarial.test.ts')])
py_files = sorted(p.name for p in (ROOT / 'tests/adversarial').glob('*.py') if p.name != 'collect_runtime_evidence.py')
jobs = []
for job in metadata['jobs']:
    lines = [l for l in text.splitlines() if l.startswith(job['name'] + '\t')]
    item = dict(name=job['name'], conclusion=job['conclusion'], url=job['url'],
                steps=[dict(name=s['name'], conclusion=s['conclusion']) for s in job['steps']])
    if not job['name'].startswith('check ('):
        jobs.append(item)
        continue
    workspace = '\n'.join(l for l in lines if '\tRun cargo test --workspace --locked\t' in l)
    front = '\n'.join(l for l in lines if '\tFront end' in l and 'lint, type-check, test' in l)
    py = '\n'.join(l for l in lines if '\tReviewer Python suites' in l)
    diag = '\n'.join(l for l in lines if '\tDesktop host with the M0 diagnostics feature' in l)
    total_pattern = r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored'
    def totals(body):
        counts = [tuple(map(int, x)) for x in re.findall(total_pattern, body)]
        return [sum(x[i] for x in counts) for i in range(3)]
    item['workspace_counts'] = totals(workspace)
    item['diagnostics_counts'] = totals(diag)
    rust = {}
    # Cargo's stdout/stderr are interleaved in macOS logs: a later target's Running header can precede
    # the earlier target's results. Match the actual permanent test names instead of assigning the
    # next textual summary to the wrong binary.
    for target in rust_targets:
        if target == 'adversarial_c01':
            file = ROOT / 'tests/adversarial/c01.rs'
        elif target == 'c03_r2_encoder':
            file = ROOT / 'tests/adversarial/c03-r2-encoder.rs'
        elif target.startswith('c03_'):
            file = ROOT / 'crates/render/tests' / (target + '.rs')
        else:
            file = ROOT / ('crates/engine/tests' if target.endswith('_wire') else 'crates/model/tests') / (target + '.rs')
        assert re.search(r'Running[^\n]*' + target, workspace), (job['name'], target)
        source = file.read_text(encoding='utf-8')
        names = re.findall(r'#\[test\]\s*fn\s+(\w+)', source)
        assert names, target
        for name in names:
            assert re.search(r'test (?:\w+::)*' + name + r' \.\.\. ok', workspace), (job['name'], target, name)
        ignored = re.findall(r'#\[test\]\s*#\[ignore[^\n]*\]\s*fn\s+(\w+)', source)
        for name in ignored:
            assert f'test {name} ... ignored' in workspace
        rust[target] = [len(names), 0, len(ignored)]
    assert set(rust) == set(rust_targets), (job['name'], sorted(set(rust_targets) - set(rust)))
    item['reviewer_rust_counts'] = rust
    ts = {}
    for file in ts_files:
        found = re.search(re.escape(file) + r'\s+\((\d+) tests?\)', front)
        assert found, (job['name'], file)
        ts[file] = int(found.group(1))
    item['reviewer_vitest_file_counts'] = ts
    item['vitest_total'] = 326
    item['node_license_tests'] = 9
    assert '326 passed' in front and '# pass 9' in front
    # The sequential runner's stdout banners can overtake stderr summaries. Match ordered
    # unittest summaries to ordered unittest suites, with the page/Vitest shim separate.
    unittest_files = [name for name in py_files if name != 'c04-r3-page.py']
    summaries = re.findall(r'Ran (\d+) tests? in', py)
    assert len(summaries) == len(unittest_files)
    python_counts = dict(zip(unittest_files, map(int, summaries)))
    page = re.search(r'c04-r3-actions.test.tsx \((\d+) tests\)', py)
    assert page
    python_counts['c04-r3-page.py'] = int(page.group(1))
    for name in py_files:
        assert re.search(r'\bok\s+' + re.escape(name), py)
    assert '11/11 reviewer Python suites passed' in py
    item['python_suite_counts'] = python_counts
    item['nested_node_execution'] = {
        'guards.mjs': {'via': 'adversarial_c01', 'cases': ['c01_r1_01_license_grouping_preserves_mandatory_copyleft',
                    'c01_r1_01_license_allowlist_stays_within_user_policy', 'c01_r1_02_dynamic_import_cannot_bypass_ipc_boundary']},
        'c01-r2-boundary.test.mjs + c01-r2-licenses.test.mjs': {'via': 'src/test/c01-r2-node.test.ts', 'wrapper_tests': 2},
        'c01-r3-boundary.test.mjs': {'via': 'src/test/c01-r3-node.test.ts', 'wrapper_tests': 1},
        'c01-r4-boundary.test.mjs': {'via': 'src/test/c01-r4-node.test.ts', 'wrapper_tests': 1},
        'c03-r1-real-transport.mjs': {'via': 'src/test/c03-r1-binary.test.ts', 'wrapper_tests': ts['src/test/c03-r1-binary.test.ts']},
    }
    for case in item['nested_node_execution']['guards.mjs']['cases']:
        assert f'test {case} ... ok' in workspace
    jobs.append(item)
save('ci-audit.json', dict(head=metadata['headSha'], url=metadata['url'], jobs=jobs,
    assessment='All committed reviewer suites execute in both OS check jobs. Suite counts below are taken from logs. Nested Node stdout is piped by the existing wrappers: child TAP counts are not printed; wrapper counts and successful exit assertions are the evidence, not invented child counts.',
    other_rust_fixtures='c02-r1-native-timestamps.rs, c02-r1-macos-accessors.rs and c02-r2-macos-accessors.rs are compiled/exercised by the c02 Python suites, on both hosts using doubles.',
    excluded='collect_runtime_evidence.py is an evidence collector, not a suite.'))
print(json.dumps(dict(ci=[{k: j[k] for k in ['name', 'workspace_counts', 'diagnostics_counts', 'reviewer_rust_counts', 'python_suite_counts']} for j in jobs if 'workspace_counts' in j]), indent=2))
