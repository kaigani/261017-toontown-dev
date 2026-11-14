import subprocess,time,json,sys
from pathlib import Path
out=Path('reviews/evidence/M4-C4.1-r6'); results=json.loads((out/'b1-results.json').read_text())
def run(name,args):
 t=time.monotonic()
 with (out/(name+'.txt')).open('w',encoding='utf-8',newline='\n') as f:
  p=subprocess.run(args,stdout=f,stderr=subprocess.STDOUT)
 results.append(dict(name=name,code=p.returncode,seconds=time.monotonic()-t)); (out/'b1-results.json').write_text(json.dumps(results,indent=2),encoding='utf-8'); print(results[-1],flush=True)
for p in sorted(Path('tests/adversarial').glob('c41-r*s-*.py')): run(p.stem,[sys.executable,str(p)])
run('inventory',[sys.executable,'scripts/check-availability-inventory.py'])
for test in ['every_gui_reference_is_enabled_exactly_when_it_runs','every_precheck_pair_refuses_and_passes_as_the_table_says']:
 run(test,['cargo','test','--locked','-p','tt-executor','--features','tt-persist/test-hooks','--test','c41',test,'--','--nocapture'])
(out/'b1-exit.txt').write_text('0\n',encoding='utf-8')
