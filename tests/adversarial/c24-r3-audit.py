"""Independent round-3 closure, malformed-input and ownership controls; temp receipts only."""
import copy
from fractions import Fraction as F
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


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, ROOT / path)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


BUILDER = module('c24_r3_gate_builder', 'scripts/test_c22_t2_gate.py')


class GateBattery(unittest.TestCase):
    def test_fresh_malformed_metadata_identity_and_outcome_battery(self):
        mutations = {
            'arch missing': lambda s, f: (s['environment'].pop('arch'), f.pop('arch')),
            'arch wrong type': lambda s, f: f.__setitem__('arch', ['x86_64']),
            'run missing': lambda s, f: s['environment'].pop('runId'),
            'run wrong type': lambda s, f: f.__setitem__('runId', 41546035007),
            'run mismatch': lambda s, f: f.__setitem__('runId', 'other-run'),
            'no timezone': lambda s, f: f.__setitem__('startedAt', '2026-11-02T10:20:00'),
            'seven hour gap': lambda s, f: f.__setitem__('startedAt', '2026-11-02T17:00:01Z'),
            'backwards start': lambda s, f: f.__setitem__('startedAt', '2026-11-02T09:59:59Z'),
            'sampleId bool': lambda s, f: f['unbudgeted'][2].__setitem__('sampleId', True),
            'sampleId float': lambda s, f: f['budgeted'][2].__setitem__('sampleId', 2.0),
            'sampleId negative': lambda s, f: f['budgeted'][2].__setitem__('sampleId', -2),
            'duplicate budgeted': lambda s, f: f['budgeted'].__setitem__(4, copy.deepcopy(f['budgeted'][0])),
            'duplicate add record': lambda s, f: s['series']['strokeAdd.wallMs']['samples'].__setitem__(5, {'tMs': 0, 'value': 1000.0}),
            'invalid unbudgeted refusal': lambda s, f: f['unbudgeted'][0].update(status='Error', error={'code': 'limit.exceeded'}, committed=False, unchangedOnRefusal=False),
            'false completion': lambda s, f: f['unbudgeted'][0].update(error={'code': 'limit.exceeded'}),
            'NaN time': lambda s, f: f['budgeted'][0].__setitem__('seconds', float('nan')),
            'enormous add': lambda s, f: s['series']['strokeAdd.wallMs']['samples'][0].__setitem__('value', 10 ** 401),
            'bad timestamp': lambda s, f: f.__setitem__('startedAt', 'not-a-date'),
        }
        for name, change in mutations.items():
            with self.subTest(name=name):
                s, f = BUILDER.valid()
                change(s, f)
                rc, out, message = BUILDER.run_full(s, f)
                self.assertEqual((rc, out), (2, None), message)
                self.assertIn('malformed', message)

    def test_expected_run_and_legitimate_equal_times(self):
        s, f = BUILDER.valid()
        self.assertEqual(BUILDER.run(s, f, {'TT_EXPECT_RUN_ID': 'another-run'}), (2, None))
        for x in s['series']['strokeAdd.wallMs']['samples']:
            x['value'] = 1000
        rc, out = BUILDER.run(s, f)
        self.assertEqual(rc, 0)
        self.assertTrue(out['met'])

    def test_identityless_duplicate_numeric_observations_are_rejected(self):
        # C24-R3-01 proving failure: objects enforce identity; a numeric list bypasses it.
        s, f = BUILDER.valid()
        s['series']['strokeAdd.wallMs']['samples'] = [1000] * 5
        rc, out = BUILDER.run(s, f)
        self.assertEqual((rc, out), (2, None), 'five copies without sample provenance cannot establish five observations')

    def test_conflicting_declared_units_are_rejected(self):
        # C24-R3-01 proving failure: a seconds series is silently interpreted as milliseconds.
        s, f = BUILDER.valid()
        s['series']['strokeAdd.wallMs']['unit'] = 's'
        rc, out = BUILDER.run(s, f)
        self.assertEqual((rc, out), (2, None), 'explicit incompatible units must not yield a gate verdict')


class Ownership(unittest.TestCase):
    def test_every_original_route_test_function_is_byte_identical(self):
        path = 'crates/executor/tests/c24_r2_routes.rs'
        old = subprocess.check_output(['git', 'show', '9b37e9d:' + path], cwd=ROOT)
        new = subprocess.check_output(['git', 'show', '871e8bd:' + path], cwd=ROOT)
        def functions(data):
            # Top-level test functions end at a column-zero closing brace; preserve raw bytes.
            return dict((m[1], m[0]) for m in re.finditer(
                rb'^#\[test\]\n(?:#\[[^\n]*\]\n)*fn (\w+)\([^\n]*\n.*?^\}\n', data, re.M | re.S))
        a, b = functions(old), functions(new)
        self.assertEqual(len(a), 13)
        self.assertEqual(a, b)

    def test_live_checker_and_file_owned_inventory(self):
        p = subprocess.run([sys.executable, 'scripts/check-reviewer-ownership.py'], cwd=ROOT, capture_output=True)
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        changed = subprocess.check_output(['git', 'diff', '--name-only', '9b37e9d..871e8bd', '--',
            'tests/adversarial', 'crates/*/tests/c0*_r*', 'crates/*/tests/c1*_r*',
            'crates/*/tests/c21_r*', 'crates/*/tests/c22_r*', 'crates/*/tests/c23_r*',
            'crates/*/tests/c24_r1_*', 'crates/*/tests/c24_r2_*'], cwd=ROOT).decode().splitlines()
        self.assertEqual(changed, ['crates/executor/tests/c24_r1_topology.rs',
                                  'crates/executor/tests/c24_r2_routes.rs',
                                  'crates/executor/tests/c24_r2_topology.rs'])


def shoelace(poly):
    return abs(sum(a[0] * b[1] - a[1] * b[0] for a, b in zip(poly, poly[1:] + poly[:1]))) / 2


def intersect(a, b, c, d):
    cross = lambda v, w: v[0] * w[1] - v[1] * w[0]
    v, w, off = (b[0]-a[0], b[1]-a[1]), (d[0]-c[0], d[1]-c[1]), (c[0]-a[0], c[1]-a[1])
    t = F(cross(off, w), cross(v, w))
    return (F(a[0]) + t*v[0], F(a[1]) + t*v[1])


def cutter_receipt():
    a, b, c, d = (0, 0), (20, 40), (80, 20), (0, 40)
    cut = ((80, 60), (0, 0))
    p = intersect(a, b, c, d)
    x = intersect(b, c, *cut)
    y = intersect(c, d, *cut)
    pre, post = [[p, b, c]], [[p, b, x, y], [x, c, y]]
    snap = lambda q: tuple(F(round(v * 64), 64) for v in q)
    def info(faces):
        exact = sum(shoelace(poly) for poly in faces)
        snapped = [shoelace([snap(q) for q in poly]) for poly in faces]
        delta = max(math.dist(q, snap(q)) for poly in faces for q in poly)
        perimeter = sum(sum(math.dist(q, r) for q, r in zip(poly, poly[1:] + poly[:1])) for poly in faces)
        bound = perimeter * delta + sum(map(len, faces)) * delta ** 2
        return {'exactArea': str(exact), 'snappedAreas': [str(v) for v in snapped],
                'deltaDu': delta, 'perimeterDu': perimeter, 'boundDu2': bound,
                'signedSnappingErrorDu2': float(sum(snapped)-exact)}
    return {'intersections': [[str(v) for v in q] for q in [p, x, y]],
            'before': info(pre), 'after': info(post),
            'areaChangeDu2': float(sum(shoelace([snap(q) for q in poly]) for poly in post) - shoelace([snap(q) for q in pre[0]]))}


class Cutter(unittest.TestCase):
    def test_rational_area_change_is_entirely_vertex_snapping(self):
        r = cutter_receipt()
        self.assertEqual(r['before']['exactArea'], r['after']['exactArea'])
        self.assertEqual(r['before']['snappedAreas'], ['2485/16'])
        self.assertEqual(sorted(r['after']['snappedAreas']), ['3945/64', '6015/64'])
        self.assertEqual(r['areaChangeDu2'], .3125)
        for kind in ('before', 'after'):
            self.assertLessEqual(abs(r[kind]['signedSnappingErrorDu2']), r[kind]['boundDu2'])
        self.assertLessEqual(.3125, r['before']['boundDu2'] + r['after']['boundDu2'])


class GoldenAndRecords(unittest.TestCase):
    def test_by_id_comparison_descends_into_inserted_array_and_handles(self):
        m = module('r3_golden', 'scripts/golden-structural-diff.py')
        old = {'nodes': [{'id': 's1', 'type': 'stroke', 'points': [[0, 0, 0, 0, 0, 0], [1, 1, 0, 0, 0, 0]]}]}
        new = copy.deepcopy(old)
        new['nodes'][0]['points'][1] = [1.25, 1.5, 0, 0, -0.25, -0.5]
        new['nodes'].insert(0, {'id': 's2', 'type': 'stroke', 'points': [[9, 9, 0, 0, 0, 0]]})
        d = m.Diff()
        d.compare(old, new, '')
        self.assertEqual(len(d.pieces['added']), 1)
        self.assertEqual(len(d.pieces['moved']), 1)
        self.assertEqual(d.handle_changes, 1)
        self.assertAlmostEqual(d.anchor_displacements[0][0], math.hypot(.25, .5))

    def test_duplicate_ids_cannot_hide_geometry_in_structural_receipt(self):
        # C24-R3-03 proving failure: by_id silently overwrites all but the last duplicate object.
        with tempfile.TemporaryDirectory(prefix='c24-r3-golden-') as tmp:
            tmp = Path(tmp)
            old = [{'id': 'same', 'type': 'stroke', 'points': [[1, 2, 0, 0, 0, 0]]},
                   {'id': 'same', 'type': 'stroke', 'points': [[9, 9, 0, 0, 0, 0]]}]
            new = copy.deepcopy(old)
            new[0]['points'][0][0] = 100
            for name, value in [('old', old), ('new', new)]:
                (tmp / (name + '.json')).write_text(json.dumps(value))
            p = subprocess.run([sys.executable, str(ROOT / 'scripts/golden-structural-diff.py'),
                str(tmp / 'old.json'), str(tmp / 'new.json'), str(tmp / 'out.json')], capture_output=True)
            self.assertNotEqual(p.returncode, 0, 'duplicate identities must be rejected before alignment')
            self.assertFalse((tmp / 'out.json').exists())

    def test_handoff_is_current_and_spec_owner_obligation_is_explicit(self):
        text = (ROOT / 'reviews/M2-C2.4-handoff.md').read_text(encoding='utf-8')
        section = text.split('## 7.')[1].split('## 8.')[0]
        self.assertIn('round 3', section)
        self.assertIn('explicit milestone-scope decision', section)
        self.assertNotIn('no reviewer test body changed in C2.4', section)
        for edit in ('44', '46', '49', '53', '57'):
            self.assertIn(edit, section)


if __name__ == '__main__':
    if len(sys.argv) == 3 and sys.argv[1] == '--cutter-receipt':
        p = Path(sys.argv[2])
        if p.exists():
            raise RuntimeError('refusing to overwrite receipt')
        p.write_text(json.dumps(cutter_receipt(), indent=2) + '\n', encoding='utf-8')
    else:
        unittest.main()
