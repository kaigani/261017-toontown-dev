"""Seal the conclusive local/CI/register evidence, failing if any required result is absent or unsuccessful."""
import hashlib
import json
from pathlib import Path
import re
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
def load(name):
    return json.loads((HERE / name).read_text(encoding='utf-8'))
def text(name):
    return (HERE / name).read_text(encoding='utf-8')

rust = load('compact-gates/receipt.json')
assert all(code == 0 for code in rust['status'].values())
assert set(rust['status']) == {f'gates/{n}.log' for n in ['fmt', 'clippy', 'test', 'clippy-diag', 'test-diag', 'deny', 'headless']}
frontend = ['build', 'pnpm-lint', 'pnpm-typecheck', 'pnpm-test', 'pnpm-format', 'pnpm-licenses', 'reviewer-python-suites']
for name in frontend:
    log = text(f'gates-run/gates/{name}.log')
    assert log.rstrip().endswith('[exit 0]') and '[exit 1]' not in log
    assert all(x == '0' for x in re.findall(r'\[exit (\d+)\]', log))
required = ['static-audit', 'ci-audit', 'schema-closure', 'sample30-audit', 'generated-freshness',
            'schema-delta-v3', 'section-domains', 'scoped-probes', 'fresh-property', 'release-stress-scale',
            'tauri-release', 'raster', 'register-reconciliation', 'frontend-registry-match']
for name in required:
    receipt = load(f'receipts/{name}.json')
    assert receipt['exitCode'] == 0, (name, receipt)
    assert hashlib.sha256((HERE / receipt['log']).read_bytes()).hexdigest() == receipt['sha256']

def totals(log):
    groups = re.findall(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored', log)
    assert groups and all(x[0] == 'ok' and x[2] == '0' for x in groups)
    return {'passed': sum(int(x[1]) for x in groups), 'failed': sum(int(x[2]) for x in groups),
            'ignored': sum(int(x[3]) for x in groups)}

local = text('compact-gates/gates/test.log')
targets = set(re.findall(r'Running tests[/\\]([\w]+)\.rs', local))
ci = load('ci-audit.json')
assert set(ci['inheritedReviewerTargets']) <= targets
r4names = re.findall(r'#\[test\]\s*fn\s+(\w+)', (ROOT / 'crates/executor/tests/c12_r4_closure.rs').read_text(encoding='utf-8'))
assert len(r4names) == 4 and all(f'test {n} ... ok' in local for n in r4names)
assert '333 passed' in text('gates-run/gates/pnpm-test.log') and '# pass 9' in text('gates-run/gates/pnpm-test.log')

counts = load('register-counts.json')
assert counts['requirements'] == {'implemented': 65, 'divergent': 0, 'missing': 0, 'deferred-with-owner': 7}
assert counts['commands'] == {'implemented': 69, 'divergent': 0, 'missing': 0, 'deferred-with-owner': 727}
registry = HERE / 'registry.json'
raster = load('raster-report.json')
assert not raster['checkMismatches'] and not raster['repeatFailures']
assert sum(len(v) for v in raster['hashes']['renderers'].values()) == 234
changed = subprocess.check_output(['git', 'diff', '--name-only', 'ec4329c'], cwd=ROOT, text=True).splitlines()
assert all(p.startswith('reviews/') for p in changed), changed
assert not list((ROOT / 'crates/executor/tests').glob('c12_r5*.rs'))
out = {'verdict': 'pass', 'openP0': 0, 'openP1': 0, 'newFindings': [],
       'reviewedHead': 'ec4329cb1abb47821ada701175730a3e691d41d7',
       'registers': counts, 'workspace': totals(local),
       'diagnostics': totals(text('compact-gates/gates/test-diag.log')),
       'scopedProbes': totals(text('logs/scoped-probes.log')),
       'freshProperty': load('fresh-property.json'),
       'releaseStressScale': totals(text('logs/release-stress-scale.log')),
       'retainedReviewerSuiteTargets': sorted(set(ci['inheritedReviewerTargets'])),
       'fourR4Regressions': r4names, 'registrySha256': hashlib.sha256(registry.read_bytes()).hexdigest(),
       'frontendCommandsSha256': hashlib.sha256((ROOT / 'desktop/src/ipc/generated/commands.ts').read_bytes()).hexdigest(),
       'ci': {'run': ci['run'], 'head': ci['head'], 'jobs': len(ci['jobs'])},
       'raster': {'hashes': 234, 'checkMismatches': 0, 'repeatFailures': 0},
       'testEdits': 'No inherited or permanent test edits. Two temporary evidence-only integration targets removed after execution.',
       'environmentQualification': load('build-environment.json')}
(HERE / 'verification-summary.json').write_text(json.dumps(out, indent=2) + '\n', encoding='utf-8')
print(json.dumps({k: out[k] for k in ['verdict', 'workspace', 'diagnostics', 'scopedProbes', 'releaseStressScale', 'registrySha256', 'ci']}, indent=2))
