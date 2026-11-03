"""Convert PowerShell text receipts to UTF-8 and retain a final verification index."""
import json,re,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
HERE=Path(__file__).resolve().parent
for path in HERE.rglob('*'):
    if path.is_file() and path.suffix in ('.log','.json','.jsonl','.txt'):
        raw=path.read_bytes()
        if raw.startswith(b'\xff\xfe'):
            path.write_text(raw.decode('utf16'),encoding='utf8',newline='')
def read(name): return (HERE/name).read_text(encoding='utf-8-sig')
def counts(name):
    return [dict(zip(['passed','failed','ignored'],map(int,m))) for m in
            re.findall(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;',read(name))]
workspace=counts('workspace.log')
totals={key:sum(row[key] for row in workspace) for key in ['passed','failed','ignored']}
assert 'Finished' in read('reviewer-clippy-final.log') and 'error:' not in read('reviewer-clippy-final.log')
assert not read('fmt-final.log').strip()
assert '15/15 reviewer Python suites passed' in read('python-suites.log')
assert not re.search(r'error TS|ELIFECYCLE|ERR_PNPM',read('frontend-checks.log'))
old=['reviews/evidence/M1-C1.3-r2/inspection-memory.json','reviews/evidence/M1-C1.3-r3/open-latency.json']
assert subprocess.run(['git','diff','--quiet','HEAD','--',*old],cwd=ROOT).returncode==0
assert subprocess.run(['git','diff','--quiet','0997205','--','spec'],cwd=ROOT).returncode==0
allowed={'crates/vector/tests/c21_r1_geometry.rs','crates/executor/tests/c21_r1_contracts.rs',
         'crates/executor/tests/c21_r1_retention.rs','crates/executor/tests/c21_r1_autosave.rs',
         'crates/mcp/tests/c21_r1_surfaces.rs'}
changed=subprocess.check_output(['git','diff','--name-only','0997205'],cwd=ROOT,text=True).splitlines()
assert all(p.startswith('reviews/') or p in allowed for p in changed),changed
ci=json.loads(read('ci.json'))
out={'builderHead':ci['headSha'],'verdict':'block','findings':{'P0':0,'P1':9,'P2Defects':1,'P2Followups':2},
     'conformance':json.loads(read('conformance.json'))['counts'],
     'workspace':{'summaries':len(workspace),**totals,'failedTargets':['tt-executor/c21_r1_contracts','tt-vector/c21_r1_geometry'],
                  'qualification':'Compiled an earlier reviewer-probe revision; all original targets pass. Final targeted logs cover all current regressions.'},
     'finalTargets':{n:counts(n) for n in ['contracts-final.log','geometry-final.log','surfaces-final.log','retention-final.log','autosave.log','history.log']},
     'checks':{'reviewerClippy':'finished without diagnostics','fmt':'clean','pythonSuites':'15/15',
               'frontend':'333/333 plus 9/9 tooling; lint/typecheck/format/licences passed',
               'cargoDeny':'advisories, bans, licences, sources OK','releaseGraphs':'no test-hooks'},
     'ci':'10/10 jobs successful at builder head; both check/release OSes plus Linux SDK verified',
     'headlessRun':{'status':json.loads(read('headless-result.json'))['status'],
                    'script':'headless-script.json','fixture':'Geometry.animpkg','log':'headless-script.jsonl'},
     'specUnchanged':True,'selfRewritingM1EvidenceRestored':True,'productChanges':False,
     'evidenceEncoding':'PowerShell UTF-16 text receipts converted losslessly to UTF-8; observations and intermediate failures retained.',
     'cleanup':'Automatic approval review rejected deletion with blocked by policy. Isolated target/review-c21-ci-release, target/review-c21-cli-user and target/review-c21-fixture.animpkg remain. Relevant receipts and fixture copied into evidence.'}
(HERE/'verification.json').write_text(json.dumps(out,indent=2)+'\n',encoding='utf8')
print(json.dumps(out,indent=2))
