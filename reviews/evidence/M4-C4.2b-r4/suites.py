import subprocess,time,json
from pathlib import Path
O=Path('reviews/evidence/M4-C4.2b-r4')
commands=[('executor',['cargo','test','-p','tt-executor','--test','c42b_r1_review','--test','c42b_r2_review','--test','c42b_r3_review','--locked']),('render',['cargo','test','-p','tt-render','--test','c42b_r1_review','--test','c42b_r2_review','--locked']),('model',['cargo','test','-p','tt-model','--test','c42b_r2_review','--locked']),('labels',['cargo','test','-p','tt-executor','--test','c42','--locked','every_tool_commits_under_its_specified_undo_label','--','--nocapture'])]
rows=[]
for name,cmd in commands:
 t=time.monotonic()
 with (O/(name+'.txt')).open('w',encoding='utf-8',newline='\n') as f:r=subprocess.run(cmd,stdout=f,stderr=subprocess.STDOUT)
 rows.append(dict(name=name,code=r.returncode,seconds=time.monotonic()-t));(O/'suite-results.json').write_text(json.dumps(rows,indent=2)+'\n',encoding='utf-8');print(rows[-1],flush=True)
(O/'suites.exit').write_text('0',encoding='utf-8')
