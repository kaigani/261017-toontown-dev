import pathlib, subprocess, json, time, sys
root=pathlib.Path('reviews/evidence/M4-C4.1-r4')
def run(name,args):
 t=time.monotonic()
 r=subprocess.run(args,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
 lines=r.stdout.splitlines()
 s=dict(command=args,exit=r.returncode,seconds=time.monotonic()-t,results=[x for x in lines if any(k in x for k in ['test result:','Running ','FAILED','error','references in','baseline='])])
 if r.returncode: s['tail']=lines[-25:]
 (root/(name+'.json')).write_text(json.dumps(s,indent=2)+'\n',encoding='utf-8')
 print(json.dumps(s),flush=True)
if sys.argv[1]=='python':
 for p in sorted(pathlib.Path('tests/adversarial').glob('c41-r*s-*.py')): run(p.stem,['python',str(p)])
else: run(sys.argv[1],['C:/Users/kaiga/.cargo/bin/cargo.exe']+sys.argv[2:])
