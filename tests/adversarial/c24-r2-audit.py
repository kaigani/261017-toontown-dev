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


class BudgetPrecision(unittest.TestCase):
    def test_snapshot_and_observed_send_ranges_are_recomputed_before_rounding(self):
        hand = (ROOT / 'reviews/M2-C2.4-handoff.md').read_text(encoding='utf-8').split('## 6a.')[1].split('## 7.')[0]
        for label in ('Windows', 'macOS'):
            p = next((ROOT / 'reviews/evidence/M2-C2.4/budgets' / label).glob('budgets-*.json'))
            b = json.loads(p.read_text(encoding='utf-8'))
            xs = [r['autosaveSnapshotSeconds'] * 1000 for r in b['maintenance']]
            self.assertIn(f'snapshot {min(xs):.3f}–{max(xs):.3f} ms', hand)
            xs = [r['readSentAtSeconds'] for r in b['queuedRead']]
            self.assertIn(f'sent {min(xs):.3f}–{max(xs):.3f} s', hand)
            xs = [r['requestedAtSeconds'] for r in b['maintenance']]
            self.assertIn(f'requested {min(xs):.3f}–{max(xs):.3f} s', hand)


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


def register():
    from collections import Counter
    prior = json.loads((ROOT / 'reviews/evidence/M2-C2.4-r1/register.json').read_text(encoding='utf-8'))
    current = copy.deepcopy(prior)
    current['reviewedHead'] = HEAD
    updates = {
        'U019': ('implemented', 'SI-084 ruled 02 precedence; ADR-0018 records the 03 amendment candidate'),
        'U042': ('divergent', 'C24-R2-01/06/07: changed-route breadth incomplete; collapsed snap candidate omitted; Flatten exposes duplicated closed-fold geometry'),
        'U043': ('divergent', 'C24-R2-02: exact translation and two union-envelope substitutions accepted; protected preservation changed outside exception'),
        'U049': ('divergent', 'C24-R2-03: original three malformations fixed; metadata/run/duplicate/status/huge-integer variants fail'),
        'U050': ('divergent', 'C24-R2-04: owner, middle step and SI-084 synchronized; handoff section 7 remains stale'),
        'U063': ('implemented', 'R1-06 closed; budgets-recomputed.json independently confirms corrected raw ranges'),
    }
    for row in current['requirements']:
        if row['id'] in updates:
            row['state'], row['evidenceOrOwner'] = updates[row['id']]
    for id, requirement, state, evidence in [
        ('U064', 'independent arbitrary-precision intersections, snapping, point location and canonical ID computation',
         'implemented', 'oracle source audit; no product geometry imports; route comparisons'),
        ('U065', 'T-junction every-pass splitting of folds/retraces', 'implemented',
         'ADR-0017 history 11; c24_noding_regressions and new-seed routes'),
        ('U066', 'antenna/reverse-edge cancellation in exact fill overlap', 'implemented',
         'ADR-0017 history 12; exact unit test and new-seed routes'),
        ('U067', 'reviewer assertions preserved except explicitly ruled changes', 'divergent',
         'C24-R2-02; protected-diff.txt and fixed-head Ownership test'),
        ('U068', 'complete and accurate declared golden structural comparison/cause', 'divergent',
         'C24-R2-05; full recursive and object-ID comparisons; first-piece anchor example'),
        ('U069', 'nearest-contour route oracle includes degenerate anchors/zero-length contours', 'divergent',
         'C24-R2-06; XOR-5D shrunk point move predicts 460 vs correct product area 400; independent SDK control passes'),
        ('U070', 'cyclic noding preserves source traversal multiplicity across zero-length closing segments', 'divergent',
         'C24-R2-07; closed repeated-end fold crosses (10,72.5) twice before and four times after; closing edge spans 1->3 instead of 1->2'),
    ]:
        current['requirements'].append({'id': id, 'clause': 'C2.4 round-1 response; round-2 charter',
                                        'requirement': requirement, 'state': state, 'evidenceOrOwner': evidence})
    for row in current['readiness']:
        if row['id'] == '1':
            row.update(state='implemented', evidenceOrOwner='02 pipeline and SDK controls; SI-084 records ruled precedence')
        elif row['id'] == '2':
            row.update(evidenceOrOwner='C24-R2-01 independent changed-route breadth; C24-R2-02 protected assertion change')
        elif row['id'] == '4':
            row.update(evidenceOrOwner='M11 C11.1 accepted and synchronized; gate missed, refusal/phase/fixpoint controls retained')
    for row in current['commands']:
        if row['id'] == 'draw.vectorize.run':
            row.update(state='implemented', evidenceOrOwner='02 functional contract and E2 pass; SI-084 records ruled 02 precedence')
    current['commands'].append({'id': 'draw.flatten.apply', 'state': 'divergent',
                                'evidenceOrOwner': 'New related-surface row: C24-R2-07 closed-fold duplicate traversals; other route/variant controls retained'})
    for group in ('requirements', 'readiness', 'commands'):
        counts = Counter(row['state'] for row in current[group])
        current['counts'][group] = {'total': len(current[group]), **{
            k: counts[k] for k in ('implemented', 'divergent', 'missing', 'deferred-with-owner')}}
    deltas = []
    for group in ('requirements', 'readiness', 'commands'):
        old = {r['id']: r for r in prior[group]}
        for row in current[group]:
            if row != old.get(row['id']):
                deltas.append({'register': group, 'id': row['id'], 'before': old.get(row['id']), 'after': row})
    (OUT / 'register.json').write_text(json.dumps(current, indent=2) + '\n', encoding='utf-8')
    (OUT / 'register-deltas.json').write_text(json.dumps(deltas, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(current['counts'], indent=2))


def summarize():
    results = {}
    for p in OUT.glob('*.txt'):
        text = p.read_text(encoding='utf-8', errors='replace')
        summaries = re.findall(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;', text)
        if summaries:
            results[p.stem] = {'emittedSummaryCount': len(summaries),
                               **{k: sum(int(s[i]) for s in summaries) for i, k in enumerate(('passed', 'failed', 'ignored'))},
                               'failedTests': re.findall(r'^test (\S+) \.\.\. FAILED', text, flags=re.M),
                               'rawSha256': hashlib.sha256(p.read_bytes()).hexdigest()}
    results['commands'] = [json.loads(s) for s in (OUT / 'execution.jsonl').read_text(encoding='utf-8').splitlines()]
    (OUT / 'execution-final.json').write_text(json.dumps(results, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({k: v for k, v in results.items() if k != 'commands'}, indent=2))


def hygiene():
    import importlib.metadata
    import os
    base = set(command('git', 'ls-tree', '-r', '--name-only', HEAD).splitlines())
    changed = set(command('git', 'diff', '--name-only', HEAD).splitlines())
    protected = sorted(base & changed)
    assert not protected, protected
    receipts = []
    historical = '989233ec83b78d37f41052b57bf12ee17d132940'
    for path in ('reviews/evidence/M1-C1.3-r3/open-latency.json',
                 'reviews/evidence/M1-C1.3-r2/inspection-memory.json'):
        original = subprocess.check_output(['git', 'show', historical + ':' + path], cwd=ROOT)
        raw = (ROOT / path).read_bytes()
        assert raw == original, path
        receipts.append({'path': path, 'fixedSha': historical, 'byteIdentical': True,
                         'sha256': hashlib.sha256(raw).hexdigest()})
    assert os.environ.get('TT_COMMIT_REVIEW_RECEIPTS') is None
    result = {'reviewedHead': HEAD, 'preexistingFilesChanged': protected,
              'specDiff': command('git', 'diff', HEAD, '--', 'spec/'), 'm1Receipts': receipts,
              'TT_COMMIT_REVIEW_RECEIPTS': os.environ.get('TT_COMMIT_REVIEW_RECEIPTS'),
              'branch': command('git', 'branch', '--show-current').strip(),
              'mcpVersion': importlib.metadata.version('mcp'), 'rustc': command('rustc', '--version').strip(),
              'node': command('node', '--version').strip(), 'statusBeforeCommit': command('git', 'status', '--short')}
    (OUT / 'hygiene.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(result, indent=2))


async def snap_probe(binary, output):
    """A product control of the new-seed failure, through SDK 2.0.0 and the reviewed CI binary."""
    import asyncio
    from mcp import ClientSession, StdioServerParameters, types
    from mcp.client.stdio import stdio_client
    calls = []
    with tempfile.TemporaryDirectory(prefix='c24-r2-snap-', dir=ROOT / 'target') as temp:
        work = Path(temp).resolve()
        async def roots(ctx):
            return types.ListRootsResult(roots=[types.Root(uri=work.as_uri(), name='review')])
        params = StdioServerParameters(command=str(Path(binary).resolve()), args=[
            '--headless', '--mcp-stdio', '--user-dir', str(work / 'user'), '--policy', 'autonomous'])
        async with stdio_client(params) as (reader, writer):
            async with ClientSession(reader, writer, list_roots_callback=roots) as session:
                await session.initialize()
                await asyncio.sleep(.5)
                async def call(id, p):
                    raw = await session.call_tool('run_command', {'id': id, 'params': p})
                    env = getattr(raw, 'structuredContent', None) or getattr(raw, 'structured_content', None) or {}
                    calls.append({'id': id, 'params': p, 'envelope': env})
                    assert env.get('status') == 'ok', env
                    return env['result']
                await call('project.create', {'name': 'Snap oracle control', 'path': str(work / 'Snap.animpkg')})
                def bezier(points):
                    out = [dict(zip(('x', 'y'), points[0]))]
                    for a, b in zip(points, points[1:]):
                        out += [dict(zip(('x', 'y'), a)), dict(zip(('x', 'y'), b)), dict(zip(('x', 'y'), b))]
                    return out
                nodes = [{'type': 'layer', 'id': 'l_a', 'layerKind': 'pencil', 'strokes': [
                    {'id': 's' + str(i), 'kind': 'pencil', 'closed': False, 'colourId': None,
                     'lineStyle': {'thickness': 1, 'lineType': 'solid'}, 'bezier': bezier(ps)}
                    for i, ps in enumerate([[(40, 0), (40, 40)], [(80, 20), (80, 20)],
                                            [(40, 0), (0, 0), (60, 60)]])]}]
                added = await call('draw.content.add', {'elementId': 'scene:#1/element:"Drawing"', 'frame': 1,
                    'createDrawing': True, 'select': False, 'content': {'nodes': nodes}})
                d = added['drawingId']
                before = await call('draw.drawing.get', {'drawingId': d})
                layer = next(n for n in before['nodes'] if n['id'] == added['idMap']['l_a'])
                assert [z['area'] for z in layer['zones']] == [800.0], layer
                stroke = next(s for s in layer['strokes'] if s['bezier'][0] == {'x': 40, 'y': 40}
                              and s['bezier'][-1] == {'x': 60, 'y': 60})
                await call('draw.point.move', {'drawingId': d, 'points': [{'strokeId': stroke['id'], 'anchor': 0}],
                                              'delta': {'x': 37, 'y': -17}, 'snap': True})
                after = await call('draw.drawing.get', {'drawingId': d})
                layer = next(n for n in after['nodes'] if n['id'] == added['idMap']['l_a'])
                assert [z['area'] for z in layer['zones']] == [400.0], layer
                at = sum(p == {'x': 80, 'y': 20} for s in layer['strokes'] for p in s['bezier'][::3])
                assert at >= 3, layer
                result = {'reviewedHead': HEAD, 'sourceRun': 41486947308, 'sdk': '2.0.0', 'calls': calls,
                          'beforeArea': 800.0, 'afterArea': 400.0, 'shippedOraclePredictedArea': 460.0,
                          'requestedPoint': [77, 23], 'nearestOtherCollapsedContour': [80, 20],
                          'distance': math.sqrt(18), 'radius': 8, 'anchorsAtSnapPoint': at, 'passed': True}
                Path(output).write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
                print(json.dumps({k: v for k, v in result.items() if k != 'calls'}, indent=2))


async def flatten_probe(binary, output):
    import asyncio
    from mcp import ClientSession, StdioServerParameters, types
    from mcp.client.stdio import stdio_client
    calls = []
    with tempfile.TemporaryDirectory(prefix='c24-r2-flatten-', dir=ROOT / 'target') as temp:
        work = Path(temp).resolve()
        async def roots(ctx):
            return types.ListRootsResult(roots=[types.Root(uri=work.as_uri(), name='review')])
        params = StdioServerParameters(command=str(Path(binary).resolve()), args=[
            '--headless', '--mcp-stdio', '--user-dir', str(work / 'user'), '--policy', 'autonomous'])
        async with stdio_client(params) as (reader, writer):
            async with ClientSession(reader, writer, list_roots_callback=roots) as session:
                await session.initialize()
                await asyncio.sleep(.5)
                async def call(id, p):
                    raw = await session.call_tool('run_command', {'id': id, 'params': p})
                    env = getattr(raw, 'structuredContent', None) or getattr(raw, 'structured_content', None) or {}
                    calls.append({'id': id, 'params': p, 'envelope': env})
                    assert env.get('status') == 'ok', env
                    return env['result']
                def layer(specs, name):
                    strokes = []
                    for i, (closed, ps) in enumerate(specs):
                        point = lambda p: dict(zip(('x', 'y'), p))
                        bez = [point(ps[0])]
                        for a, b in zip(ps, ps[1:]):
                            bez += [point(a), point(b), point(b)]
                        if closed:
                            bez += [point(ps[-1]), point(ps[0])]
                        strokes.append({'id': name + str(i), 'kind': 'pencil', 'closed': closed, 'colourId': None,
                                        'lineStyle': {'thickness': 1, 'lineType': 'solid'}, 'bezier': bez})
                    return {'type': 'layer', 'id': name, 'layerKind': 'pencil', 'strokes': strokes}
                await call('project.create', {'name': 'Flatten oracle control', 'path': str(work / 'Flat.animpkg')})
                added = await call('draw.content.add', {'elementId': 'scene:#1/element:"Drawing"', 'frame': 1,
                    'createDrawing': True, 'select': False, 'content': {'nodes': [
                        layer([(True, [(0,80),(80,20),(0,80)])], 'l_a')]}})
                d = added['drawingId']
                other = await call('draw.content.add', {'drawingId': d, 'select': False, 'content': {'nodes': [
                    layer([(False, [(0,40),(80,80),(40,20)]),(False,[(20,40),(40,60)])], 'l_b')]}})
                before = await call('draw.drawing.get', {'drawingId': d})
                flat = await call('draw.flatten.apply', {'drawingId': d, 'ids': [added['idMap']['l_a'],other['idMap']['l_b']]})
                after = await call('draw.drawing.get', {'drawingId': d})
                node = next(n for n in after['nodes'] if n['id'] == flat['layerId'])
                areas = sorted(z['area'] for z in node['zones'])
                result = {'reviewedHead': HEAD, 'sourceRun': 41486947308, 'sdk': '2.0.0', 'calls': calls,
                          'actualAreas': areas, 'before': before, 'after': after}
                Path(output).write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
                print(json.dumps({'areas': areas, 'strokes': [s['bezier'][::3] for s in node['strokes']]}, indent=2))


if __name__ == '__main__':
    if sys.argv[1:] == ['--collect']:
        collect()
    elif sys.argv[1:] == ['--freshness']:
        freshness()
    elif sys.argv[1:] == ['--register']:
        register()
    elif sys.argv[1:] == ['--summarize']:
        summarize()
    elif sys.argv[1:] == ['--hygiene']:
        hygiene()
    elif sys.argv[1:2] == ['--snap-probe']:
        import asyncio
        asyncio.run(snap_probe(sys.argv[2], sys.argv[3]))
    elif sys.argv[1:2] == ['--flatten-probe']:
        import asyncio
        asyncio.run(flatten_probe(sys.argv[2], sys.argv[3]))
    else:
        unittest.main()
