"""Inventory all 51 SI numbers and all ignored test bodies, preserving reserved/removed numbers."""
import hashlib
import json
import pathlib
import re
import subprocess
HERE=pathlib.Path(__file__).resolve().parent
ROOT=HERE.parents[2]
text=(ROOT/'plan/spec-issues.md').read_text(encoding='utf-8')
sections={int(m[1]):m[2] for m in re.finditer(r'^## SI-(\d+) (.*)$',text,re.M)}
open_issues={1:('M10 C10.1','No: encoder quality mapping is M10; M1 has no encoder.'),
             8:('M10 C10.2','Yes: M1 preserves opaque importOptions without interpreting it; explicit interim contract.'),
             47:('M1 C1.4 / M4 C4.1','Yes: binding interim ruling requires content hashes at save; watch remains size+mtime.'),
             49:('M1 C1.4 / M4 C4.1','Yes: supported older revisions are explicit; spec owner must decide the newer subscription protocol.')}
md=['## Spec issues SI-001–051','',
    '“Decided” is an implementation decision recorded in plan/spec-issues.md, not a claim that the frozen spec was edited or the spec owner ratified it. The four open issues remain with the spec owner. SI-051’s representation now also needs the orchestrator’s C14-R3-02 ruling.','',
    '| Issue | Status | M1 dependency / next owner |','|---|---|---|']
rows=[]
for n in range(1,52):
    title=sections.get(n,'Removed misreading; number not reused')
    if n in (9,10,12): status='closed (removed)'
    elif n==2: status='closed (reserved; no active issue)'
    elif n in open_issues: status='open with spec owner'
    elif 'resolved)' in title or 'closed in' in title: status='closed'
    else: status='decided'
    dependency='Recorded decision applies where referenced; no undecided M1 dependency.'
    if n in open_issues:
        owner,note=open_issues[n]; dependency=f'{note} Owner: spec owner with builder {owner}.'
    elif n in (9,10,12): dependency='No active obligation; removed by C1.1 round-one ruling.'
    elif n==2: dependency='Reserved for measured deterministic-format incompatibility, M10 C10.1.'
    elif n==44: dependency='M1 uses an un-undoable repair base to preserve integrity. Spec-owner ratification remains; builder M1 C1.3 decision stands.'
    elif n==45: dependency='M1 confirmation delivered; C14-R2-07 closes save-boundary bypass. Reload is a user action under SI-050.'
    elif n==50: dependency='M1 keeps app scope unavailable to agents; reload belongs to the user; keep-mine uses a bound token.'
    elif n==51: dependency='M1 depends on replay semantics. C14-R3-01/02 block implementation closure; builder/orchestrator C1.4.'
    elif 23<=n<=43 and status=='decided': dependency='Strict placeholders remain the interim declaration contract; owning domain checkpoint must replace them before implementing the handler.'
    md.append(f'| SI-{n:03} | {status} | {dependency} |')
    rows.append({'id':f'SI-{n:03}','status':status,'title':title,'dependency':dependency})

tickets=[('crates/mcp/tests/c14_tickets.rs','every_result_lists_its_effective_params','5983f29','crates/mcp/tests/c14_r1_16_tickets.rs','C14-R1-16 effectiveParams','builder M4 C4.1; before dialogs/result contracts are completed'),
('crates/mcp/tests/c14_tickets.rs','namespace_docs_carry_a_worked_example_with_executed_results','5983f29','crates/mcp/tests/c14_r1_16_tickets.rs','C14-R1-16 executed examples','builder before M2 C2.1 begins'),
('crates/persist/tests/zip64.rs','an_entry_over_4_gib_round_trips_with_zip64_sizes','04cc687','crates/persist/tests/zip64.rs','large ZIP64 release-only scenario (not a defect ticket)','builder M1 C1.3; release scenario retained, rerun before changing ZIP64 writer'),
('crates/executor/tests/c14_lock_races.rs','child_lock_holder','4e34520','crates/executor/tests/c14_lock_races.rs','crash child process (not a defect ticket)','builder M1 C1.4; parent must execute it in every ordinary workspace run'),
('crates/render/tests/c03_r1_stress.rs','c03_r1_04_simd_dispatch_must_not_change_final_pixels','f0f55d8','crates/render/tests/c03_r1_stress.rs','C0.4 SIMD ticket (outside C1.1–C1.4)','builder M3 C3.3; before final-renderer SIMD dispatch')]
ticket_rows=[]
def body(text,name):
    start=text.find('fn '+name+'(')
    if start<0:return None
    end=text.find('\n}',start)
    return text[start:end+2]
md+=['','## Ignored tests and carried tickets','',
     '| Test / ticket | Owner and deadline | Body unchanged since introduction |','|---|---|---|']
for path,name,commit,oldpath,ticket,owner in tickets:
    old=subprocess.check_output(['git','show',f'{commit}:{oldpath}'],cwd=ROOT).decode('utf-8').replace('\r\n','\n')
    current=(ROOT/path).read_text(encoding='utf-8')
    a,b=body(old,name),body(current,name)
    same=a==b and a is not None
    ticket_rows.append({'path':path,'test':name,'introduction':commit,'unchanged':same,'sha256':hashlib.sha256((b or '').encode()).hexdigest(),'owner':owner})
    md.append(f'| `{name}` — {ticket} | {owner} | {same} (`{commit}`) |')
(HERE/'readiness.md').write_text('\n'.join(md)+'\n',encoding='utf-8')
(HERE/'readiness.json').write_text(json.dumps({'specIssues':rows,'ignoredTests':ticket_rows},indent=2)+'\n',encoding='utf-8')
print([(r['test'],r['unchanged']) for r in ticket_rows])

