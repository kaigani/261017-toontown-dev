"""Permanent C0.3 evidence regressions. Expected to fail on 11247ff; no xfails."""
import copy
import gzip
import importlib.util
import json
import re
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
BASE = json.loads(gzip.decompress((ROOT/'reviews/evidence/M0-C0.3/canvas-local-windows-rtx4090-auto.json.gz').read_bytes()))
SCRIPT = ROOT/'scripts/analyse-canvas-log.py'

class CanvasEvidence(unittest.TestCase):
    def rejected(self, change):
        doc = copy.deepcopy(BASE)
        change(doc)
        with tempfile.TemporaryDirectory() as d:
            path = Path(d)/'input.json'
            path.write_text(json.dumps(doc), encoding='utf-8')
            p = subprocess.run([sys.executable, str(SCRIPT), str(path)], capture_output=True, text=True, encoding='utf-8')
        self.assertEqual(p.returncode, 2, p.stdout[:800]+p.stderr)
        self.assertEqual(p.stdout, '', 'malformed evidence must not produce a success table')

    def test_c03_r1_01_partial_auto_matrix(self):
        def mutate(d):
            d['runs'] = d['runs'][:1]
            d['pixelRuns'] = []
        self.rejected(mutate)

    def test_c03_r1_01_contradictory_paths(self):
        self.rejected(lambda d: d['runs'][0]['decode'].update(paths=1))

    def test_c03_r1_01_missing_environment(self):
        self.rejected(lambda d: d.update(environment={'timeOrigin': d['environment']['timeOrigin']}))

    def test_c03_r1_01_negative_timings(self):
        self.rejected(lambda d: d['runs'][0]['frames'][0].update(cpuMs=-100, gpuMs=-100))

    def test_c03_r1_01_unmeasured_gpu_completion(self):
        self.rejected(lambda d: [f.update(gpuMs=None) for f in d['runs'][0]['frames']])

    def test_c03_r1_01_duplicate_transfer_ids(self):
        self.rejected(lambda d: [s.update(i=0, frame=0) for s in d['runs'][0]['transfers']])

    def test_c03_r1_01_zero_scene_bytes(self):
        self.rejected(lambda d: [s.update(bytes=1) for s in d['runs'][0]['transfers']])

    def test_c03_r1_01_wrong_spike_control(self):
        self.rejected(lambda d: d.update(spike='tablet'))

    def test_c03_r1_01_missing_samples_control(self):
        self.rejected(lambda d: d['runs'][0].update(frames=[]))

    def test_c03_r1_01_truncated_frames_control(self):
        self.rejected(lambda d: d['runs'][0].update(frames=d['runs'][0]['frames'][:100]))

    def test_c03_r1_03_committed_fixture_pngs(self):
        hashes = json.loads((ROOT/'crates/render/examples/c03_raster/hashes.json').read_text())
        tracked = subprocess.check_output(['git', 'ls-files', '*.png'], cwd=ROOT, text=True).splitlines()
        import hashlib
        actual = {hashlib.sha256((ROOT/f).read_bytes()).hexdigest() for f in tracked}
        missing = [f'{r}/{s}' for r, scenes in hashes['renderers'].items() for s, h in scenes.items() if h['png'] not in actual]
        self.assertEqual(missing, [], 'checkpoint requires committed fixture PNGs, not only expiring CI artifacts')

    def test_c03_r1_06_dependency_audit_covers_locked_graph(self):
        tree = subprocess.check_output(['cargo', 'tree', '--locked', '-p', 'vello_cpu', '--prefix', 'none'], cwd=ROOT, text=True)
        graph = set(re.findall(r'^(\S+) v([\d.]+)', tree, re.M))
        audit = (ROOT/'reviews/evidence/M0-C0.3/vello-dependency-audit.txt').read_text(encoding='utf-8')
        recorded = set(re.findall(r'^(\S+) v([\d.]+)', audit, re.M))
        self.assertEqual(recorded, graph, 'the dependency audit must account for build dependencies, or declare its narrower edge set')

if __name__ == '__main__':
    unittest.main()
