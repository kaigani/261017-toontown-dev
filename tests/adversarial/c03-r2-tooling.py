"""Round-2 analyzer attacks. Assertions describe the required contract, without xfail."""
import copy
import gzip
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
BASE = json.loads(gzip.decompress((ROOT/'reviews/evidence/M0-C0.3/canvas-r1-local-windows-rtx4090-auto.json.gz').read_bytes()))

def run(log):
    with tempfile.TemporaryDirectory() as d:
        path = Path(d)/'log.json'
        path.write_text(json.dumps(log),encoding='utf-8')
        return subprocess.run([sys.executable,'-I',str(ROOT/'scripts/analyse-canvas-log.py'),str(path)],capture_output=True,text=True,encoding='utf-8')

class AnalyzerRound2(unittest.TestCase):
    def reject(self, mutate):
        log = copy.deepcopy(BASE)
        mutate(log)
        result = run(log)
        self.assertNotEqual(result.returncode,0,'contradictory evidence printed a success table')
        self.assertEqual(result.stdout,'')

    def test_valid_auto(self):
        self.assertEqual(run(BASE).returncode,0)

    def test_extra_backend(self):
        self.reject(lambda x: x['runs'].append(copy.deepcopy(x['runs'][0])))

    def test_missing_pixel_run(self):
        self.reject(lambda x: x['pixelRuns'].pop())

    def test_transfer_order(self):
        self.reject(lambda x: x['runs'][0]['transfers'].reverse())

    def test_header_paths_disagree(self):
        self.reject(lambda x: x['runs'][0]['transfers'][0]['header'].update(paths=1))

    def test_forced_renderer_mismatch(self):
        self.reject(lambda x: x['environment'].update(forcedRenderer='webgl2'))

    def test_auto_claiming_manual(self):
        self.reject(lambda x: x.update(source='manual'))

    def test_c03_r2_01_compensating_point_verb_counts(self):
        def mutate(x):
            # Each cubic needs 3 points. No points cannot decode to 200,000 cubics;
            # compensate the missing point bytes with verb bytes to keep total size exact.
            for t in x['runs'][0]['transfers']:
                h=t['header']
                h['verbs'] += 8*h['points']
                h['points']=0
        self.reject(mutate)

    def test_c03_r2_01_nondefault_seed_does_not_bypass_workload(self):
        def mutate(x):
            x['parameters']['seed']=2
            for r in x['runs']:
                r['seed']=2
                for t in r['transfers']:
                    t['header'].update(seed=2,paths=0,points=0,verbs=0,groups=0)
                    t['bytes']=72
        self.reject(mutate)

if __name__=='__main__':
    unittest.main()
