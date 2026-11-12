import subprocess,pathlib,os,json,time,shutil
root=pathlib.Path.cwd(); e=root/'reviews/evidence/M3-C3.4-r4'
while not (e/'sweep.done').exists(): time.sleep(15)
def run(name,args):
 if shutil.disk_usage(root).free<120*1024**3: raise RuntimeError('disk floor')
 print('START',name,flush=True);(e/(name+'.command.json')).write_text(json.dumps(args))
 with (e/(name+'.log')).open('w',encoding='utf-8') as f: r=subprocess.run(args,stdout=f,stderr=subprocess.STDOUT,env={**os.environ,'PYTHONIOENCODING':'utf-8'})
 (e/(name+'.exit')).write_text(str(r.returncode));print('DONE',name,r.returncode,flush=True)
run('r4-executor',['cargo','test','--locked','-p','tt-executor','-j','4','--test','c34_r4_closure','--','--nocapture','--test-threads=1'])
run('r4-anim',['cargo','test','--locked','-p','tt-anim','-j','4','--features','test-hooks','--test','c34_r4_passage','--','--nocapture'])
for pkg in ['tt-executor','tt-mcp','tt-vector','tt-anim']: run('fmt-'+pkg,['cargo','fmt','-p',pkg,'--check'])
run('clippy-product',['cargo','clippy','--locked','-j','4','-p','tt-executor','-p','tt-mcp','-p','tt-vector','-p','tt-anim','--lib','--bins','--examples','--','-D','warnings'])
run('clippy-targets',['cargo','clippy','--locked','-j','4','-p','tt-executor','-p','tt-mcp','-p','tt-vector','-p','tt-anim','--all-targets','--features','tt-persist/test-hooks','--','-D','warnings','-A','clippy::empty_line_after_doc_comments'])
run('features',['python','scripts/check-release-features.py'])
run('reviewer-python',['python','scripts/run-reviewer-python-suites.py'])
run('headless',['cargo','build','--release','--locked','-p','tt-headless','-j','4'])
run('e1',[str(root/'.venv/Scripts/python.exe'),'scripts/e1-bouncing-ball.py','target/release/toontown-headless.exe',str(e/'E1')])
(e/'gates.done').write_text('complete')
