import subprocess,time,json
from pathlib import Path
E=Path('reviews/evidence/M3-C3.4-r1')
assert (E/'workspace.exit').exists(), 'do not edit or start cargo while workspace run is active'
p=Path('crates/executor/tests/c34_r1_contract.rs');s=p.read_text(encoding='utf-8-sig')
s=s.replace('fn canonical_document(h: &tt_executor::ExecutorHandle) -> serde_json::Value {','fn canonical_document(h: &tt_executor::ExecutorHandle) -> Result<serde_json::Value, Box<dyn std::error::Error>> {')
s=s.replace('.as_object_mut().expect("bundle object")','.as_object_mut().ok_or("bundle object required")?')
s=s.replace('    exported\n}', '    Ok(exported)\n}')
s=s.replace('canonical_document(&h)', 'canonical_document(&h)?')
p.write_text(s,encoding='utf-8')
results=[]
def run(name,args):
 if args[0]=='cargo':
  for proc in ('cargo.exe','rustc.exe'):
   check=subprocess.check_output(['tasklist','/FI',f'IMAGENAME eq {proc}'],text=True)
   if proc.lower() in check.lower(): raise RuntimeError(check)
 start=time.time()
 with (E/(name+'.log')).open('w',encoding='utf-8') as f: result=subprocess.run(args,stdout=f,stderr=subprocess.STDOUT)
 (E/(name+'.exit')).write_text(str(result.returncode)+'\n')
 results.append({'name':name,'command':subprocess.list2cmdline(args),'exitCode':result.returncode,'durationSeconds':round(time.time()-start,3)})
 (E/'final-reviewer-runs.json').write_text(json.dumps(results,indent=2))
 assert result.returncode==0, name
run('fmt-reviewer-apply',['rustfmt','--edition','2024',str(p)])
run('fmt-reviewer',['rustfmt','--check','--edition','2024',str(p)])
run('clippy-reviewer',['cargo','clippy','--locked','-p','tt-executor','--tests','--','-D','warnings','-A','clippy::empty_line_after_doc_comments'])
run('contract-reviewer',['cargo','test','--locked','-p','tt-executor','-j','4','--test','c34_r1_contract'])
