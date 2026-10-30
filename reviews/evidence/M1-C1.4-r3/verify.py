"""Independent round-three receipt, CI, ownership and policy evidence; read-only product checks."""
import collections
import hashlib
import json
import pathlib
import re
import subprocess

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[2]
def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT).decode('utf-8', errors='replace')
def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()
def totals(text):
    matches = re.findall(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;', text)
    return dict(zip(('passed','failed','ignored'), map(sum, zip(*(map(int,v) for v in matches))))) if matches else {}

base = ROOT / 'reviews/evidence/M1-C1.4'
receipt = json.loads((base / 'receipt.json').read_text(encoding='utf-8'))
review_paths = ['tests/adversarial', 'crates/*/tests/*_r[0-9]*', 'bins/headless/tests/c14_r1.rs',
                'bins/headless/tests/c14_r2.rs', 'reviews/M1-C1.4-r1.md', 'reviews/M1-C1.4-r2.md',
                'reviews/evidence/M1-C1.4-r1', 'reviews/evidence/M1-C1.4-r2',
                'reviews/evidence/M1-C1.3-r2/inspection-memory.json',
                'reviews/evidence/M1-C1.3-r3/open-latency.json']
diff = git('diff','0e7d02e..39ea1b2','--',*review_paths)
(HERE / 'reviewer-path-diff.txt').write_text(diff,encoding='utf-8')
product = git('diff','--unified=0','0e7d02e..39ea1b2','--','crates/*/src/**','bins/*/src/**','desktop/src/**')
added = [s[1:] for s in product.splitlines() if s.startswith('+') and not s.startswith('+++')]
flags = [s for s in added if re.search(r'\b(?:unwrap|expect)\s*\(|\bpanic!|\bunsafe\b|#\[allow',s)]
ci = {}
for name in ['ci-green','ci-failed']:
    d = json.loads((HERE / (name+'.json')).read_bytes())
    ci[name] = {'head':d['headSha'],'conclusion':d['conclusion'],'jobs':[
        {'name':j['name'],'conclusion':j['conclusion'],'failedSteps':[s['name'] for s in j['steps'] if s['conclusion']=='failure'],
         'sdkSteps':[{k:s[k] for k in ('name','conclusion')} for s in j['steps'] if 'SDK' in s['name']]}
        for j in d['jobs']]}
ticket = (ROOT/'crates/mcp/tests/c14_tickets.rs').read_text(encoding='utf-8')
declared = git('diff','0e7d02e..39ea1b2','--','crates/executor/tests','crates/mcp/tests/fixtures')
(HERE/'builder-test-diff.txt').write_text(declared,encoding='utf-8')
report = {
    'product':'39ea1b2','receiptBase':receipt['baseCommit'],
    'receiptHashes':{p:sha(base/p)==v for p,v in receipt['sha256'].items()},
    'receiptStatuses':receipt['status'],
    'receiptWorkspaceTotals':totals((base/'gates/test.log').read_text(encoding='utf-8')),
    'reviewerDiffEmpty':not diff,'productLintCandidates':flags,
    'dependencyDiff':git('diff','0e7d02e..39ea1b2','--','Cargo.lock','deny.toml'),
    'generatedDiff':git('diff','39ea1b2','--','crates/registry/src/declared.rs','crates/registry/src/declared.json','desktop/src/ipc/generated','plan/spec-issues.md'),
    'c14TicketFileUnchangedSinceIntroduction':git('show','5983f29:crates/mcp/tests/c14_r1_16_tickets.rs').replace('\r\n','\n')==ticket,
    'ci':ci,
}
(HERE/'verification.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
print(json.dumps({k:report[k] for k in ['receiptWorkspaceTotals','reviewerDiffEmpty','productLintCandidates','dependencyDiff','c14TicketFileUnchangedSinceIntroduction']},indent=2))
print('receipt hashes all match:',all(report['receiptHashes'].values()))
