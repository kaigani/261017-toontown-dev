"""Final checks after the one workspace invocation has exited."""
from pathlib import Path
import subprocess,os
E=Path('reviews/evidence/M3-C3.3-r4')
assert (E/'workspace.exit').exists(), 'workspace must finish before any further Cargo invocation'
os.environ.pop('TT_COMMIT_REVIEW_RECEIPTS',None)
def run(name,args):
 print(name,flush=True)
 p=E/(name+'.log');x=E/(name+'.exit')
 if p.exists(): p.rename(E/(name+'-initial.log'))
 if x.exists(): x.rename(E/(name+'-initial.exit'))
 with p.open('w',encoding='utf-8') as f: r=subprocess.run(args,stdout=f,stderr=subprocess.STDOUT)
 x.write_text(str(r.returncode)+'\n')
run('rollback-mcp',['cargo','test','-p','tt-mcp','--features','tt-persist/test-hooks','--locked','-j','4','--test','c33_r4_rollback'])
run('clippy-executor',['cargo','clippy','-p','tt-executor','--locked','-j','4','--test','c33_r4_closure','--test','c33_r4_rollback','--','-D','warnings'])
run('clippy-mcp',['cargo','clippy','-p','tt-mcp','--features','tt-persist/test-hooks','--locked','-j','4','--test','c33_r4_closure','--test','c33_r4_rollback','--','-D','warnings'])
run('fmt-executor',['cargo','fmt','-p','tt-executor','--check'])
run('fmt-mcp',['cargo','fmt','-p','tt-mcp','--check'])
run('sdk',[str(Path(os.environ['TEMP'])/'toontown-c33-r4-sdk/Scripts/python.exe'),'scripts/mcp-conformance.py','target/debug/toontown-headless.exe',str(E/'sdk-conformance.json')])
print('FINAL GATES DONE',flush=True)
