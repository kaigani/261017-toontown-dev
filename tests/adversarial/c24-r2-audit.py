"""Independent C2.4 round-2 controls. Finding assertions deliberately remain enabled.

Run without arguments for unittest, or --collect for read-only fixed-head/CI evidence.
All gate receipts use temporary directories. No historical receipt is regenerated.
"""
import copy
import datetime
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'reviews/evidence/M2-C2.4-r2'
BASE = '6f72ee7'
HEAD = '3c74c3432f2a1895b5aaca83e6ddf914cf6b4281'


def command(*args):
    p = subprocess.run(args, cwd=ROOT, capture_output=True)
    if p.returncode:
        raise RuntimeError(f'{args}: {p.stderr.decode("utf-8", errors="replace")}')
    return p.stdout.decode('utf-8', errors='replace')


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, ROOT / path)
    obj = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(obj)
    return obj


def valid():
    s = {'environment': {'os': 'windows', 'arch': 'x86_64', 'profile': 'release',
                         'startedAt': '2026-11-02T10:00:00Z'},
         'series': {'strokeAdd.wallMs': {'unit': 'ms', 'samples': [
             {'tMs': i, 'value': 1000 + i} for i in range(6)]}}}
    f = {'os': 'windows', 'arch': 'x86_64', 'profile': 'release', 'unbudgetedAllCompleted': True,
         'unbudgeted': [{'seconds': .9 + i / 1000, 'status': 'Ok', 'error': None, 'committed': True,
                         'unchangedOnRefusal': True, 'sampleId': i} for i in range(5)],
         'budgeted': [{'seconds': .5 + i / 1000, 'status': 'Error', 'error': {'code': 'limit.exceeded'},
                       'committed': False, 'unchangedOnRefusal': True, 'sampleId': i} for i in range(5)]}
    return s, f


def gate(s, f):
    with tempfile.TemporaryDirectory(prefix='c24-r2-gate-') as d:
        d = Path(d)
        (d / 's.json').write_text(json.dumps(s), encoding='utf-8')
        (d / 'f.json').write_text(json.dumps(f), encoding='utf-8')
        p = subprocess.run([sys.executable, str(ROOT / 'scripts/c22-t2-gate.py'),
                            str(d / 's.json'), str(d / 'f.json'), str(d / 'out.json')],
                           capture_output=True, text=True, encoding='utf-8')
        return p.returncode, (d / 'out.json').exists(), p.stdout, p.stderr


class GateValidation(unittest.TestCase):
    def reject(self, s, f):
        code, exists, stdout, stderr = gate(s, f)
        self.assertEqual((code, exists), (2, False), (stdout, stderr))

    def test_nan_string_missing_profile_four_samples_refusal_completed(self):
        mutations = [lambda s, f: f['unbudgeted'][0].update(seconds=float('nan')),
                     lambda s, f: f['unbudgeted'][0].update(seconds='0.9'),
                     lambda s, f: f.pop('profile'),
                     lambda s, f: f.update(unbudgeted=f['unbudgeted'][:4]),
                     lambda s, f: f['unbudgeted'][0].update(error={'code': 'limit.exceeded'}, committed=False)]
        for mutate in mutations:
            with self.subTest(mutation=mutate):
                s, f = copy.deepcopy(valid())
                mutate(s, f)
                self.reject(s, f)

    def test_missing_os_on_both_files_is_rejected(self):
        # C24-R2: equality of two missing values is not presence validation.
        s, f = valid()
        s['environment'].pop('os')
        f.pop('os')
        self.reject(s, f)

    def test_explicit_mismatched_run_ids_are_rejected(self):
        s, f = valid()
        s['environment']['runId'] = '41486947308'
        f['runId'] = '41425059602'
        f['startedAt'] = '2026-11-01T10:00:00Z'
        self.reject(s, f)

    def test_duplicated_observation_records_are_rejected(self):
        s, f = valid()
        f['unbudgeted'] = [copy.deepcopy(f['unbudgeted'][0]) for _ in range(5)]
        self.reject(s, f)

    def test_refusal_record_with_status_ok_is_rejected(self):
        s, f = valid()
        f['budgeted'][0]['status'] = 'Ok'
        self.reject(s, f)

    def test_huge_json_integer_duration_exits_2_without_receipt(self):
        s, f = valid()
        f['unbudgeted'][0]['seconds'] = 10 ** 400
        self.reject(s, f)

    def test_percentiles_match_shared_rule_for_even_and_odd_counts(self):
        g = module('gate_r2', 'scripts/c22-t2-gate.py')
        a = module('analyser_r2', 'scripts/analyse-measurements.py')
        for n in range(5, 16):
            xs = [float(i * i) for i in range(n)]
            self.assertEqual(g.p50(xs[::-1]), a.nearest(xs, .5))


class Ownership(unittest.TestCase):
    def test_edit38_changes_only_the_two_ruled_union_assertions(self):
        path = 'crates/executor/tests/c24_r1_topology.rs'
        old = command('git', 'show', f'{BASE}:{path}')
        new = command('git', 'show', f'{HEAD}:{path}')
        expected = old.replace('sa <= union + 1e-3 * union.max(1.0)',
                               'sa <= union + oracle::snapping_envelope(&segs, 64)')
        self.assertEqual(new, expected, 'Ruling permits two union bounds only; area-preservation must remain intact')

    def test_oracle_has_no_product_geometry_imports(self):
        for p in (ROOT / 'crates/vector/tests/oracle').rglob('*.rs'):
            text = p.read_text(encoding='utf-8')
            self.assertNotRegex(text, r'\btt_(?:vector|executor|model)::|\bcrate::|\bsuper::')
        text = (ROOT / 'crates/vector/tests/oracle/mod.rs').read_text(encoding='utf-8')
        for token in ('BigRational', 'fn snap_pt', 'fn zone_id', 'fn locate', 'fn snapping_envelope'):
            self.assertIn(token, text)


class Bookkeeping(unittest.TestCase):
    def test_owner_and_decisions_are_recorded(self):
        trace = (ROOT / 'plan/traceability.csv').read_text(encoding='utf-8')
        self.assertRegex(trace, r'C22-T2 \(ticket\),[^\n]*,M11,C11.1,')
        si = (ROOT / 'plan/spec-issues.md').read_text(encoding='utf-8')
        self.assertIn('SI-084', si)
        self.assertIn('middle', si)
        self.assertIn('SI-084', next((ROOT / 'docs/decisions').glob('ADR-0018*')).read_text(encoding='utf-8'))

    def test_si067_middle_step_and_si084_named_policy_remain_explicit(self):
        si = (ROOT / 'plan/spec-issues.md').read_text(encoding='utf-8')
        s67 = si.split('## SI-067')[1].split('## SI-068')[0]
        self.assertIn('participant already has an anchor', s67)
        self.assertIn('rounded half to even', s67)
        s84 = si.split('## SI-084')[1]
        for token in ('02 §11.2 takes precedence', 'generated black', 'reused `Vectorization` palette',
                      'texture swatches named from the file', 'spec owner'):
            self.assertIn(token, s84)

    def test_handoff_merge_readiness_no_longer_claims_round1_is_next(self):
        hand = (ROOT / 'reviews/M2-C2.4-handoff.md').read_text(encoding='utf-8')
        section = hand.split('## 7.')[1].split('## 8.')[0]
        self.assertNotIn('review round 1 next', section)
        self.assertNotIn('no reviewer test body changed in C2.4', section)


def collect():
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / 'ci-run.json').write_text(command('gh', 'run', 'view', '41486947308', '--json',
                                           'headSha,status,conclusion,jobs'), encoding='utf-8')
    ci = json.loads((OUT / 'ci-run.json').read_text(encoding='utf-8'))
    jobs = []
    for j in ci['jobs']:
        seconds = (datetime.datetime.fromisoformat(j['completedAt'].replace('Z', '+00:00')) -
                   datetime.datetime.fromisoformat(j['startedAt'].replace('Z', '+00:00'))).total_seconds()
        jobs.append({'name': j['name'], 'seconds': seconds, 'conclusion': j['conclusion'],
                     'above55Minutes': j['name'] == 'check (windows-latest)' and seconds > 3300})
        if j['name'].startswith(('check properties', 'history oracles', 'e2 release', 'check (')):
            name = re.sub('[^A-Za-z0-9]+', '-', j['name']).strip('-')
            (OUT / (name + '.log')).write_text(command('gh', 'run', 'view', '41486947308',
                                                      '--job', str(j['databaseId']), '--log'), encoding='utf-8')
    (OUT / 'ci-summary.json').write_text(json.dumps({'head': ci['headSha'], 'jobs': jobs}, indent=2), encoding='utf-8')
    for artifact in ('e2-Windows', 'e2-macOS', 'history-oracles-Windows', 'history-oracles-macOS'):
        command('gh', 'run', 'download', '41486947308', '-n', artifact, '-D', str(OUT / 'ci' / artifact))
    patterns = ['tests/adversarial', 'crates/*/tests/c0*_r*', 'crates/*/tests/c1*_r*',
                'crates/*/tests/c21_r*', 'crates/*/tests/c22_r*', 'crates/*/tests/c23_r*', 'crates/*/tests/c24_r1_*']
    (OUT / 'protected-diff.txt').write_text(command('git', 'diff', '--stat', f'{BASE}..{HEAD}', '--', *patterns) +
                                           command('git', 'diff', f'{BASE}..{HEAD}', '--', *patterns), encoding='utf-8')
    (OUT / 'spec-diff.txt').write_text(command('git', 'diff', f'{BASE}..{HEAD}', '--', 'spec/'), encoding='utf-8')
    leaves = []
    def diff(a, b, path, file):
        if type(a) is not type(b):
            leaves.append({'file': file, 'path': path, 'old': a, 'new': b})
        elif isinstance(a, dict):
            for k in sorted(a.keys() | b.keys()):
                diff(a.get(k), b.get(k), path + '/' + k, file)
        elif isinstance(a, list):
            if len(a) != len(b):
                leaves.append({'file': file, 'path': path + '/length', 'old': len(a), 'new': len(b)})
            for i, (x, y) in enumerate(zip(a, b)):
                diff(x, y, path + '/' + str(i), file)
        elif a != b:
            leaves.append({'file': file, 'path': path, 'old': a, 'new': b})
    for file in command('git', 'diff', '--name-only', f'{BASE}..{HEAD}', '--', 'crates/*/tests/golden/*').splitlines():
        diff(json.loads(command('git', 'show', f'{BASE}:{file}')),
             json.loads(command('git', 'show', f'{HEAD}:{file}')), '', file)
    (OUT / 'golden-structural.json').write_text(json.dumps(leaves, indent=2), encoding='utf-8')
    print(json.dumps({'jobs': jobs, 'goldenLeaves': len(leaves)}, indent=2))


def freshness():
    """Generate only into target/, compare bytes, and leave every pre-existing file untouched."""
    with tempfile.TemporaryDirectory(prefix='review-r2-gen-', dir=ROOT / 'target') as temp:
        temp = Path(temp)
        gen = module('gen_review_r2', 'scripts/gen-command-declarations.py')
        gen.OUT_RS, gen.OUT_JSON, gen.ISSUES = temp / 'declared.rs', temp / 'declared.json', temp / 'spec-issues.md'
        gen.ISSUES.write_bytes((ROOT / 'plan/spec-issues.md').read_bytes())
        assert gen.main() == 0
        for generated, original in [(gen.OUT_RS, ROOT / 'crates/registry/src/declared.rs'),
                                     (gen.OUT_JSON, ROOT / 'crates/registry/src/declared.json'),
                                     (gen.ISSUES, ROOT / 'plan/spec-issues.md')]:
            assert generated.read_bytes() == original.read_bytes(), str(original)
        print('598 current declarations: Rust, JSON and issue block byte-identical; temporary outputs only')


if __name__ == '__main__':
    if sys.argv[1:] == ['--collect']:
        collect()
    elif sys.argv[1:] == ['--freshness']:
        freshness()
    else:
        unittest.main()
