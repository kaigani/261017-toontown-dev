import subprocess,time,json,shutil,re
from pathlib import Path
E=Path('reviews/evidence/M3-C3.4-r1')
results=json.loads((E/'continuation-runs.json').read_text())
def run(name,args):
 if args[0]=='cargo':
  for proc in ('cargo.exe','rustc.exe'):
   check=subprocess.check_output(['tasklist','/FI',f'IMAGENAME eq {proc}'],text=True)
   if proc.lower() in check.lower(): raise RuntimeError(check)
 start=time.time()
 with (E/(name+'.log')).open('w',encoding='utf-8') as f:
  p=subprocess.run(args,stdout=f,stderr=subprocess.STDOUT)
 (E/(name+'.exit')).write_text(str(p.returncode)+'\n')
 results.append(dict(name=name,command=subprocess.list2cmdline(args),exitCode=p.returncode,durationSeconds=round(time.time()-start,3)))
 (E/'continuation-runs.json').write_text(json.dumps(results,indent=2))
 return p.returncode
run('fmt-reviewer',['rustfmt','--check','--edition','2024','crates/executor/tests/c34_r1_contract.rs'])
run('clippy-reviewer',['cargo','clippy','--locked','-p','tt-executor','--tests','--','-D','warnings','-A','clippy::empty_line_after_doc_comments'])
run('contract-reviewer',['cargo','test','--locked','-p','tt-executor','-j','4','--test','c34_r1_contract'])
run('numeric-reviewer',[str(Path('.venv/Scripts/python.exe').resolve()),'tests/adversarial/c34-r1-numeric.py'])
run('merge-note-probe',[str(Path('.venv/Scripts/python.exe').resolve()),'tests/adversarial/c34-r1-merge-note.py'])
free=shutil.disk_usage('C:/').free
(E/'workspace-storage.json').write_text(json.dumps({'freeBytes':free,'minimumBytes':120*1024**3},indent=2))
run('df-workspace',['C:/Program Files/Git/bin/bash.exe','-lc','df -h /c'])
assert free>120*1024**3
run('workspace',['cargo','test','--workspace','--locked','-j','4','--no-fail-fast'])
