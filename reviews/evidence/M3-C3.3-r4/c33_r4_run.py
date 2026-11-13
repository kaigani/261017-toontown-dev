"""Sequential closure gates; native exit sidecars, no concurrent Cargo processes."""
from pathlib import Path
import subprocess,os,json,shutil,datetime
E=Path('reviews/evidence/M3-C3.3-r4')
os.environ.pop('TT_COMMIT_REVIEW_RECEIPTS',None)
def run(name,args):
 print(datetime.datetime.now().isoformat(),name,flush=True)
 with (E/(name+'.log')).open('w',encoding='utf-8') as f:
  result=subprocess.run(args,stdout=f,stderr=subprocess.STDOUT)
 (E/(name+'.exit')).write_text(str(result.returncode)+'\n')
 return result.returncode
run('rollback-executor',['cargo','test','-p','tt-executor','--locked','-j','4','--test','c33_r4_rollback'])
run('rollback-mcp',['cargo','test','-p','tt-mcp','--locked','-j','4','--test','c33_r4_rollback'])
run('fmt-executor',['cargo','fmt','-p','tt-executor','--check'])
run('fmt-mcp',['cargo','fmt','-p','tt-mcp','--check'])
run('clippy-executor',['cargo','clippy','-p','tt-executor','--locked','-j','4','--test','c33_r4_closure','--test','c33_r4_rollback','--','-D','warnings'])
run('clippy-mcp',['cargo','clippy','-p','tt-mcp','--locked','-j','4','--test','c33_r4_closure','--test','c33_r4_rollback','--','-D','warnings'])
run('deny',['cargo','deny','--locked','check'])
run('python',['python','scripts/run-reviewer-python-suites.py'])
free=shutil.disk_usage('C:/').free
(E/'workspace-storage.json').write_text(json.dumps(dict(freeBytes=free,minimumBytes=120_000_000_000,checkedAt=datetime.datetime.now().isoformat()),indent=2)+'\n')
run('df',['C:/Program Files/Git/bin/bash.exe','-lc','df -h /c'])
if free<120_000_000_000:
 (E/'workspace.exit').write_text('SKIPPED: below 120 GB\n')
else:
 run('workspace',['cargo','test','--workspace','--locked','-j','4','--no-fail-fast'])
print('DONE',flush=True)
