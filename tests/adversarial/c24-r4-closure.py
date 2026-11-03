"""Round-4 malformed variants; receipts always live in temporary directories."""
import importlib.util, json, pathlib, subprocess, sys, tempfile, unittest
ROOT=pathlib.Path(__file__).resolve().parents[2]
def load(path):
    spec=importlib.util.spec_from_file_location('r4helper',ROOT/path)
    m=importlib.util.module_from_spec(spec); spec.loader.exec_module(m); return m
B=load('scripts/test_c22_t2_gate.py')
class Closure(unittest.TestCase):
    def test_gate_nonfinite_identity(self):
        s,f=B.valid(); s['series']['strokeAdd.wallMs']['samples'][0]['tMs']=float('inf')
        self.assertEqual(B.run(s,f),(2,None))
    def test_gate_unit_object(self):
        s,f=B.valid(); s['series']['strokeAdd.wallMs']['unit']={'name':'ms'}
        self.assertEqual(B.run(s,f),(2,None))
    def test_nested_duplicate_in_old(self): self.duplicate('old')
    def test_nested_duplicate_in_new(self): self.duplicate('new')
    def duplicate(self,side):
        clean={'level1':{'level2':{'nodes':[{'id':'a'},{'id':'b'}]}}}
        bad={'level1':{'level2':{'nodes':[{'id':'a'},{'id':'a'}]}}}
        with tempfile.TemporaryDirectory(prefix='c24-r4-') as d:
            d=pathlib.Path(d)
            for name in ('old','new'): (d/(name+'.json')).write_text(json.dumps(bad if name==side else clean))
            p=subprocess.run([sys.executable,str(ROOT/'scripts/golden-structural-diff.py'),str(d/'old.json'),str(d/'new.json'),str(d/'out.json')],capture_output=True)
            self.assertEqual(p.returncode,2,p.stdout+p.stderr); self.assertFalse((d/'out.json').exists())
if __name__=='__main__': unittest.main()
