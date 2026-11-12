import subprocess, pathlib, os, json, time
root=pathlib.Path.cwd(); e=root/'reviews/evidence/M3-C3.4-r4'
jobs=[('panic-1',['test','--locked','-p','tt-executor','-j','4','--test','c34_r3_probes','panicking_probe_leaves_worker_alive','--','--exact','--nocapture'],{'RUST_BACKTRACE':'1'}),('panic-0',['test','--locked','-p','tt-executor','-j','4','--test','c34_r3_probes','panicking_probe_leaves_worker_alive','--','--exact','--nocapture'],{'RUST_BACKTRACE':'0'}),('reviewer-release',['test','--release','--locked','-p','tt-executor','-j','4']+sum((['--test',p.stem] for p in sorted((root/'crates/executor/tests').glob('c34_r[23]_*.rs'))),[])+['--test','c34_round2','--','--nocapture','--test-threads=1'],{}),('stress-release',['test','--release','--locked','-p','tt-mcp','-j','4','--test','c34_r3_stress','--','--nocapture'],{}),('executor',['test','--locked','-p','tt-executor','-j','4','--no-fail-fast'],{}),('mcp',['test','--locked','-p','tt-mcp','-j','4','--features','tt-persist/test-hooks','--no-fail-fast'],{}),('vector',['test','--locked','-p','tt-vector','-j','4','--features','test-hooks','--no-fail-fast'],{}),('anim',['test','--locked','-p','tt-anim','-j','4','--features','test-hooks','--no-fail-fast'],{})]
for name,args,env in jobs:
 if __import__('shutil').disk_usage(root).free < 120*1024**3: raise RuntimeError('disk floor')
 (e/(name+'.command.json')).write_text(json.dumps({'command':['cargo']+args,'env':env}))
 print('START',name,flush=True)
 with (e/(name+'.log')).open('w') as f: result=subprocess.run(['cargo']+args,stdout=f,stderr=subprocess.STDOUT,env={**os.environ,**env})
 (e/(name+'.exit')).write_text(str(result.returncode)); print('DONE',name,result.returncode,flush=True)
(e/'sweep.done').write_text('complete')
