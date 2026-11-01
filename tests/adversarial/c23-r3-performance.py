"""Recompute final-round timing deltas from observations, retaining the input files."""
import argparse,importlib.util,json,math,pathlib,shutil,statistics,unittest
ROOT=pathlib.Path(__file__).resolve().parents[2]
def load(p):return json.loads(p.read_bytes())
def finite(values):
 if not values or any(isinstance(x,bool) or not isinstance(x,(int,float)) or not math.isfinite(x) or x<0 for x in values):raise ValueError('expected nonempty finite nonnegative numeric observations')
 return values
def summarize(values):
 values=sorted(finite(values));return {'n':len(values),'p50':values[math.floor((len(values)-1)*0.5+0.5)],'max':max(values)}
def collect(out):
 out.mkdir(parents=True,exist_ok=True);raw=out/'measurements';raw.mkdir(exist_ok=True);prior=ROOT/'reviews/evidence/M2-C2.3-r2/measurements';results={}
 for name in ['render-100k','flatten-100k','centre-line-large']:
  p=out/'measurements'/(name+'.json');v=load(p);old=load(prior/(name+'.json'));finite([v['seconds'],old['seconds']]);results[name]={'seconds':v['seconds'],'priorSeconds':old['seconds'],'ratio':v['seconds']/old['seconds'],'error':v['error']}
 spec=importlib.util.spec_from_file_location('analyzer',ROOT/'scripts/analyse-measurements.py');analyzer=importlib.util.module_from_spec(spec);spec.loader.exec_module(analyzer)
 for source,name in [(ROOT/'target/evidence/c21/strokes-100k-windows.json','c21-strokes-100k-windows.json'),(ROOT/'target/evidence/c22-planar/planar-worst-case-windows.json','c22-planar-worst-case-windows.json')]:shutil.copy2(source,raw/name)
 current=raw/'c21-strokes-100k-windows.json';previous=prior/current.name
 assert analyzer.check_file(current)['ok'] and analyzer.check_file(previous)['ok']
 a,b=load(previous),load(current);series=[]
 for name,s in b['series'].items():
  before=summarize([x['value'] for x in a['series'][name]['samples']]);after=summarize([x['value'] for x in s['samples']]);series.append({'name':name,'prior':before,'current':after,'p50Ratio':after['p50']/before['p50'] if before['p50'] else None})
 results['100k']={'series':series,'inputsValidated':True}
 a=load(prior/'c22-planar-worst-case-windows.json');b=load(raw/'c22-planar-worst-case-windows.json');cases=[]
 assert len(a['cases'])==len(b['cases'])==9
 for old,new in zip(a['cases'],b['cases']):
  assert old['name']==new['name'];same=[(r['units'],r['outcome'],r['zones'],r['strokes']) for r in old['runs']]==[(r['units'],r['outcome'],r['zones'],r['strokes']) for r in new['runs']]
  before=max(finite([r['wallMs'] for r in old['runs']]));after=max(finite([r['wallMs'] for r in new['runs']]));cases.append({'name':new['name'],'priorMaxMs':before,'currentMaxMs':after,'ratio':after/before,'workAndOutcomeUnchanged':same})
 results['planar']=cases
 for os in ['Windows','macOS']:
  source=ROOT/'target/c23-r3-ci'/('headless-'+os)
  dest=out/'ci-observations'/os
  for p in source.rglob('*.json'):
   t=dest/p.relative_to(source);t.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(p,t)
 (out/'performance-recomputed.json').write_text(json.dumps(results,indent=2)+'\n',encoding='utf-8')
class InputValidation(unittest.TestCase):
 def test_rejects_bad_observations(self):
  for value in [[],[True],[float('inf')],[float('nan')],['1'],[-1]]:
   with self.assertRaises(ValueError):summarize(value)
 def test_nearest_rank(self):self.assertEqual(summarize([4,1,3,2])['p50'],3)
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--out',type=pathlib.Path);a,rest=p.parse_known_args()
 if a.out:collect(a.out.resolve())
 unittest.main(argv=[__file__,*rest],verbosity=2)
