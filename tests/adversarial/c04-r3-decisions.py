"""R3 regressions: correct behavior, intentionally failing until findings close."""
import gzip
import importlib.util
import json
import pathlib
import unittest

ROOT=pathlib.Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('tablet',ROOT/'scripts/analyse-tablet-log.py')
t=importlib.util.module_from_spec(spec)
spec.loader.exec_module(t)
OUT=ROOT/'reviews/evidence/M0-C0.4-r3'

def fixture(name):
    return json.loads(gzip.decompress((OUT/(name+'.json.gz')).read_bytes()))

def decide(log):
    w,n=t.web_analysis(log),t.native_analysis(log)
    a=t.action_analysis(log,w,n)
    return t.tablet_decision(log,w,n,a)

class Regressions(unittest.TestCase):
    def test_C04_R3_04_binding_score_rule_has_no_unruled_core_filter(self):
        ok={r:'pass' for r in range(1,10)}
        rows={'W':{**ok,2:'fail',3:'fail'},'R':{**ok,4:'fail',5:'fail',6:'n/a'}}
        d=t.aggregate('windows',rows)
        self.assertEqual(d['scores'],{'W':7,'R':6})
        self.assertEqual(d['selected'],'R','Final Addendum 2 disqualifies W before ranking by share')
        self.assertEqual(d['disqualified'],{'W':[2]})
        self.assertEqual(d['shares']['W'],'7/9')

    def test_C04_R3_01_outside_contact_must_not_flip_candidate(self):
        baseline=decide(fixture('coalescing-baseline'))
        outside=decide(fixture('coalescing-outside'))
        self.assertEqual(baseline['selected'],'H')
        self.assertEqual(outside['rows']['W']['6'],baseline['rows']['W']['6'],
                         'Row 6 belongs to A2, not an ignored outside-action contact')
        self.assertEqual(outside['selected'],baseline['selected'])

    def test_C04_R3_03_overlapping_native_marker_spans_must_be_rejected(self):
        log=fixture('epoch-overlap-accepted')
        with self.assertRaises(t.LogError):
            t.action_analysis(log,t.web_analysis(log),t.native_analysis(log))

if __name__=='__main__': unittest.main(verbosity=2)
