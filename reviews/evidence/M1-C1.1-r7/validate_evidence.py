"""Verify receipts, compare selftests and extract local and non-check CI results."""
from pathlib import Path
import hashlib
import json
import platform
import re
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
BASE = '01591796452bbea5333cc9121c00857456519daf'

def save(name, value):
    (HERE/name).write_text(json.dumps(value, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')

def log(name):
    return (HERE/'logs'/f'{name}.log').read_text(encoding='utf-8')

def totals(text):
    rows = [tuple(map(int, row)) for row in re.findall(
        r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored', text)]
    return [sum(row[i] for row in rows) for i in range(3)]

required = ['install', 'fmt', 'clippy', 'workspace', 'deny', 'clippy-diagnostics', 'test-diagnostics',
            'pnpm-lint', 'pnpm-typecheck', 'pnpm-test', 'pnpm-format-check', 'pnpm-licenses-check',
            'frontend-build', 'frontend-build-diagnostics', 'tauri-release', 'diagnostics-gate',
            'headless-build', 'selftest-1', 'selftest-2', 'headless-guard', 'raster',
            'r4-regressions', 'r5-regressions', 'r6-regressions', 'reviewer-python',
            'random-recorded', 'random-r4-recorded', 'random-r5-recorded', 'random-r6-recorded', 'random-fresh',
            'c11_r7_closure_probe', 'c11_r7_staged_commit_10000', 'c11_r7_staged_inverse_10000',
            'ci-metadata', 'ci-full', 'test-edit-diff', 'rustdoc']
verified = {}
for file in sorted((HERE/'receipts').glob('*.json')):
    receipt = json.loads(file.read_text(encoding='utf-8'))
    assert receipt['base'] == BASE, file
    assert receipt['exit_code'] == 0, (file, receipt['exit_code'])
    data = (HERE/receipt['log']).read_bytes()
    assert hashlib.sha256(data).hexdigest() == receipt['sha256'], file
    verified[file.stem] = dict(exit_code=0, sha256=receipt['sha256'],
                               elapsed_seconds=receipt['elapsed_seconds'])
assert not set(required) - set(verified), sorted(set(required)-set(verified))

assert totals(log('workspace')) == [292, 0, 1]
assert totals(log('test-diagnostics')) == [49, 0, 0]
assert '326 passed' in log('pnpm-test') and '# pass 9' in log('pnpm-test')
assert '11/11 reviewer Python suites passed' in log('reviewer-python')
for stem in ['staged_commit', 'staged_inverse']:
    metadata = json.loads((HERE/f'{stem}-property.json').read_text(encoding='utf-8'))
    source = (ROOT/f'crates/model/tests/{stem}.rs').read_bytes()
    assert hashlib.sha256(source).hexdigest() == metadata['builder_source_sha256']
    expanded = source.replace(b'cases: 256,', b'cases: 10_000,')
    assert hashlib.sha256(expanded).hexdigest() == metadata['expanded_source_sha256']
    receipt = json.loads((HERE/f'receipts/c11_r7_{stem}_10000.json').read_text(encoding='utf-8'))
    assert receipt['env']['PROPTEST_RNG_SEED'] == metadata['seed']
    assert totals(log(f'c11_r7_{stem}_10000')) == [1, 0, 0]
    assert not (ROOT/f'crates/model/tests/c11_r7_{stem}_10000.rs').exists()

selftests = [json.loads(log(f'selftest-{n}')) for n in [1, 2]]
checks = [[(c['id'], c['ok']) for c in r['checks']] for r in selftests]
assert checks[0] == checks[1] and len(checks[0]) == 5 and all(ok for _, ok in checks[0])
assert all(r['ok'] for r in selftests)
save('selftest-comparison.json', dict(compared='check IDs and ok flags, excluding timing',
                                     identical=True, checks=checks[0]))

commands = {'rustc': ['rustc', '-Vv'], 'cargo': ['cargo', '-V'],
            'cargo_deny': ['cargo', 'deny', '--version'], 'node': ['node', '--version'],
            'pnpm': ['cmd', '/c', 'pnpm', '--version'], 'python': ['python', '--version']}
environment = {name: subprocess.check_output(cmd, cwd=ROOT, text=True).strip()
               for name, cmd in commands.items()}
environment.update(platform=platform.platform(), reviewed_commit=BASE)
save('environment.json', environment)

ci = json.loads(log('ci-metadata'))
assert ci['headSha'] == BASE and ci['conclusion'] == 'success' and ci['status'] == 'completed'
raw = re.sub(r'(?:\x1b|\^\[)\[[0-9;]*[a-zA-Z]', '', log('ci-full'))
details = []
for job in ci['jobs']:
    lines = [s for s in raw.splitlines() if s.startswith(job['name']+'\t')]
    text = '\n'.join(lines)
    evidence = []
    if job['name'].startswith('headless without'):
        assert 'test -z "${DISPLAY:-}"' in text and 'test -z "${WAYLAND_DISPLAY:-}"' in text
        assert 'selftest-1.json' in text and 'selftest-2.json' in text
        assert "('determinism.ieee754', True)" in text and 'headless dependency check passed' in text
        evidence = [s for s in lines if 'Self-test with no display server\t' in s and
                    ('True)' in s or 'linked libraries (linux)' in s or 'dependency check passed' in s)]
    elif job['name'].startswith('raster determinism'):
        assert 'for run in 1 2; do' in text
        assert '--check crates/render/examples/c03_raster/hashes.json' in text
        assert 'test result: ok. 2 passed; 0 failed; 0 ignored' in text
        evidence = [s for s in lines if 'Render fixtures and check committed hashes (two processes)\t' in s and
                    ('for run in 1 2' in s or '--check crates/render' in s or 'test result:' in s)]
    elif job['name'].startswith('bundle ('):
        assert 'diagnostics gating: ok' in text and '"ok": true' in text
        evidence = [s for s in lines if 'diagnostics gating: ok' in s]
    else:
        continue
    assert job['conclusion'] == 'success'
    details.append(dict(job=job['name'], url=job['url'], verified=True, relevant_log_lines=evidence))
save('ci-linux-raster-bundle-audit.json', details)
save('evidence-validation.json', dict(base=BASE, receipts=verified, workspace=[292, 0, 1],
    diagnostics=[49, 0, 0], vitest=326, node=9, python_suites=11,
    provenance='Interrupted attempt left completed receipts and two live Python processes in this worktree. '
    'All receipts were checked against their raw-log SHA-256 and reviewed commit; both processes completed. '
    'CI was fetched again; scoped probes and static audits were independently rerun. '
    'No product code, historical reviewer tests or spec files were edited.'))
print(json.dumps(dict(receipts_verified=len(verified), workspace=[292,0,1],
                      selftests_identical=True, ci_jobs=len(ci['jobs'])), indent=2))
