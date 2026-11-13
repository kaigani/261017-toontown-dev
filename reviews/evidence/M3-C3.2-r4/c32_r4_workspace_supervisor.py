import subprocess,pathlib,datetime,json,os,platform,hashlib
root=pathlib.Path(r'C:\Users\kaiga\work\review\m3-c32-r4')
out=root/'reviews/evidence/M3-C3.2-r4'
cmd=['cargo','test','--workspace','--locked','-j','4','--no-fail-fast']
assert not (out/'workspace.exit.txt').exists()
assert os.environ.get('TT_COMMIT_REVIEW_RECEIPTS') is None
meta={'command':cmd,'cwd':str(root),'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'started_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'supervisor_pid':os.getpid(),'platform':platform.platform(),'TT_COMMIT_REVIEW_RECEIPTS':None,'test_sha256':{p:hashlib.sha256((root/p).read_bytes()).hexdigest() for p in ['crates/executor/tests/c32_r4_closure.rs','crates/mcp/tests/c32_r4_delivery.rs']}}
with (out/'workspace.txt').open('wb') as log:
 p=subprocess.Popen(cmd,cwd=root,stdout=log,stderr=subprocess.STDOUT)
 meta['cargo_pid']=p.pid
 (out/'workspace-metadata.json').write_text(json.dumps(meta,indent=2),encoding='utf-8')
 code=p.wait()
meta.update(native_exit=code,finished_utc=datetime.datetime.now(datetime.timezone.utc).isoformat())
(out/'workspace-metadata.json').write_text(json.dumps(meta,indent=2),encoding='utf-8')
(out/'workspace.exit.txt').write_text(str(code)+'\n',encoding='utf-8')
