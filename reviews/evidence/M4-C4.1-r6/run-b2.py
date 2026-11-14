import subprocess,time,json,sys
from pathlib import Path
root=Path.cwd(); out=root/'reviews/evidence/M4-C4.1-r6'; app=str(root/'target/debug/toontown.exe'); results=[]
jobs=[('gui','scripts/c41-gui-e2e.py',[app,str(out/'gui')]),('lifecycle','tests/adversarial/c41-r1-http-lifecycle.py',[app,str(out/'lifecycle.json')]),('floats','tests/adversarial/c41-r2-desktop-floats.py',[app,str(out/'floats.json')]),('keys','tests/adversarial/c41-r2-desktop-keys.py',[app,str(out/'keys.json')])]
for i in range(3): jobs.append(('changes-'+str(i+1),'tests/adversarial/c41-r3-desktop-changes.py',[app,str(out/('changes-'+str(i+1)+'.json'))]))
jobs.append(('conformance','scripts/mcp-conformance.py',['--http',app,str(out/'conformance.json')]))
for name,script,args in jobs:
 t=time.monotonic()
 with (out/(name+'.txt')).open('w',encoding='utf-8',newline='\n') as f: p=subprocess.run([sys.executable,script,*args],stdout=f,stderr=subprocess.STDOUT)
 results.append(dict(name=name,code=p.returncode,seconds=time.monotonic()-t)); (out/'b2-results.json').write_text(json.dumps(results,indent=2),encoding='utf-8',newline='\n'); print(results[-1],flush=True)
(out/'b2-exit.txt').write_text(str(int(any(r['code'] for r in results)))+'\n',encoding='utf-8',newline='\n')
