"""Hash final review artifacts and recompute test summaries from raw logs."""
from pathlib import Path
import datetime
import hashlib
import json
import re
import subprocess
import statistics

OUT=Path(__file__).resolve().parent
ROOT=OUT.parents[2]
def text(name):
    data=(OUT/name).read_bytes()
    return data.decode('utf-16' if data.startswith(b'\xff\xfe') else 'utf-8-sig',errors='replace')
def count(name):
    rows=re.findall(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored',text(name))
    return [sum(int(r[i]) for r in rows) for i in range(3)]

paths=[ROOT/'reviews/M1-C1.3-r3.md']
paths+=list((ROOT/'crates/executor/tests').glob('c13_r3*.rs'))
paths+=[ROOT/'crates/persist/tests/c13_r3_format.rs']
paths += [p for p in OUT.rglob('*') if p.is_file() and p.name!='receipt.json']
latency=json.loads(text('open-latency.json'))['milliseconds']
stress=json.loads(text('lock-stress.json'))
receipt={
 'productHead':'4333ff13fc12db1e871d0b3cd088a88b4f11adb6',
 'recordedAt':datetime.datetime.now(datetime.timezone.utc).isoformat(),
 'workspaceExit':json.loads(text('workspace-status.json')),
 'workspaceRust':count('workspace.log'),
 'newTargetExit':json.loads(text('final-status.json')),
 'newTargetRust':count('final-tests.log'),
 'diagnosticsRust':count('diagnostics.log'),
 'newTargetClippyExit':json.loads(text('clippy-status.json')),
 'fmtLogEmpty':not text('fmt.log').strip(),
 'lockStressRuns':len(stress),'lockStressFailures':sum(r['exit']!=0 for r in stress),
 'openLatencyMilliseconds':{'count':len(latency),'min':min(latency),'median':statistics.median(latency),'max':max(latency)},
 'expectedFailingRegressions':{
  'C13-R3-01':['a_live_writer_paused_beyond_two_seconds_is_not_taken_over','heartbeat_refuses_an_unreadable_replacement_lock'],
  'C13-R3-02':['asset_replace_preserves_unrelated_strings_equal_to_the_old_id'],
  'C13-R3-03':['repeated_exports_do_not_duplicate_omitted_media_in_session_cache']},
 'sha256':{p.relative_to(ROOT).as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(paths)},
 'gitStatus':subprocess.check_output(['git','status','--short'],cwd=ROOT,text=True),
}
(OUT/'receipt.json').write_text(json.dumps(receipt,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps({k:receipt[k] for k in ['workspaceRust','newTargetRust','newTargetExit','lockStressRuns','lockStressFailures']},indent=2))
