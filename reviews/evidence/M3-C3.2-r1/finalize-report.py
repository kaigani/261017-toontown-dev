"""Finalize receipts/report only after a completed workspace test execution."""
import json,re,os
from pathlib import Path
R=Path.cwd();E=R/'reviews/evidence/M3-C3.2-r1'
def read(p):
 b=p.read_bytes();s=b.decode('utf-16') if b.startswith((b'\xff\xfe',b'\xfe\xff')) else b.decode('utf-8-sig',errors='replace');return s.replace('\r\n','\n').replace('\r','\n')
exitcode=int(read(E/'workspace-final-exit.txt').strip())
log=read(E/'workspace-final.txt');binary='';rows=[]
for line in log.splitlines():
 if re.search(r'\bRunning |Doc-tests ',line):binary=line.strip()
 if line.startswith('test result:'):
  numbers=re.search(r'(\d+) passed; (\d+) failed; (\d+) ignored;',line)
  if numbers:rows.append(dict(binary=binary,result=line,passed=int(numbers[1]),failed=int(numbers[2]),ignored=int(numbers[3])))
assert rows,'No test results; compilation-only failure must not be reported as test completion'
failed=[x for x in rows if x['failed']]
expected=('c32_r1_passage','c32_r1_corpus','c32_r1_default_frame')
assert all(any(name in x['binary'] for name in expected) for x in failed),failed
summary=dict(command='cargo test --workspace --locked -j 4 --no-fail-fast',environment={'CARGO_INCREMENTAL':'0','TT_COMMIT_REVIEW_RECEIPTS':None},native_exit=exitcode,per_binary=rows,total_passed=sum(x['passed'] for x in rows),total_failed=sum(x['failed'] for x in rows),total_ignored=sum(x['ignored'] for x in rows),failed_binaries=[x['binary'] for x in failed],prior_attempt='workspace-disk-full.txt: OS error 112; exit 101 before test execution; cargo clean --profile dev recovered 168.4 GiB')
(E/'workspace-results.json').write_text(json.dumps(summary,indent=2)+'\n')
(E/'workspace-results.md').write_text('# Final workspace results\n\n'+f"Native exit {exitcode}; {len(rows)} binary results; {summary['total_passed']} passed / {summary['total_failed']} failed / {summary['total_ignored']} ignored.\n\n"+'\n'.join(f"- {x['binary']}: {x['result']}" for x in rows)+'\n')
receipt=f"The final `cargo test --workspace --locked -j 4 --no-fail-fast` completed with native exit **{exitcode}**: **{len(rows)} per-binary results, {summary['total_passed']} passed / {summary['total_failed']} failed / {summary['total_ignored']} ignored**. Failing binaries: "+'; '.join('`'+x['binary']+'`' for x in failed)+'. These retain the finding assertions for C32-R1-01 and C32-R1-03. No other test-result failure is reported. Existing ignored measurement cases are listed explicitly in [the per-binary receipt](evidence/M3-C3.2-r1/workspace-results.md); their CI release measurements are separate. The first attempt exited 101 during compilation after OS error 112 (disk full); Cargo-managed debug-cache cleanup freed 168.4 GiB, and the completed retry disabled incremental caching. Both attempt logs are retained.'
body=read(E/'report-template.txt').replace('__WORKSPACE_RECEIPT__',receipt)
assert not re.search(r'\b(interim|incomplete|pending)\b',body,re.I)
(R/'reviews/M3-C3.2-r1.md').write_text(body.rstrip()+'\n',encoding='utf-8',newline='\n')
register=json.loads((E/'register.json').read_text(encoding='utf-8'));register['complete']=True;register['workspace_receipt']='workspace-results.json';(E/'register.json').write_text(json.dumps(register,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps({k:v for k,v in summary.items() if k!='per_binary'}))
