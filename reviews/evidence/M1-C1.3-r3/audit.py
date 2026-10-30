"""Recompute immutable paths, gate hashes/counts, CI suite coverage, and source-policy deltas."""
import collections
import datetime
import hashlib
import json
from pathlib import Path
import re
import subprocess

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[2]

def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT).decode('utf-8', errors='replace')

def write(name, value):
    (OUT/name).write_text(json.dumps(value, indent=2, ensure_ascii=False)+'\n', encoding='utf-8')

protected = ['tests/adversarial', 'desktop/src/test', ':(glob)crates/**/tests/c0*',
             ':(glob)crates/**/tests/c11*', ':(glob)crates/**/tests/c12_r*',
             'crates/executor/tests/c13_r1_persistence.rs', 'crates/executor/tests/c13_r2_persistence.rs',
             'crates/persist/tests/c13_r1_format.rs', 'crates/persist/tests/c13_r2_format.rs',
             'reviews/M1-C1.3-r1.md', 'reviews/M1-C1.3-r2.md', 'reviews/evidence/M1-C1.3-r1',
             'reviews/evidence/M1-C1.3-r2', ':(glob)reviews/evidence/M0*', ':(glob)reviews/evidence/M1-C1.1*',
             ':(glob)reviews/evidence/M1-C1.2*', 'crates/model/tests/staged_commit.rs', 'crates/model/tests/staged_inverse.rs']
delta = git('diff', 'c3b650b', '--', *protected)
(OUT/'protected.diff').write_text(delta, encoding='utf-8')
receipt = json.loads((ROOT/'reviews/evidence/M1-C1.3/receipt.json').read_text(encoding='utf-8-sig'))
hashes = {name: hashlib.sha256((ROOT/'reviews/evidence/M1-C1.3'/name).read_bytes()).hexdigest() == h for name,h in receipt['sha256'].items()}
summary_re = re.compile(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored')
def counts(text):
    return [sum(int(m[i]) for m in summary_re.findall(text)) for i in range(3)]
gate = (ROOT/'reviews/evidence/M1-C1.3/gates/test.log').read_text(encoding='utf-8-sig')
diff = git('diff', '--unified=0', 'c3b650b', '4333ff1', '--', ':(glob)crates/**/src/*.rs')
policy = []
path = ''
for line in diff.splitlines():
    if line.startswith('+++ b/'): path = line[6:]
    if line.startswith('+') and not line.startswith('+++') and re.search(r'\.unwrap\s*\(|\.expect\s*\(|\bpanic!|\bunsafe\b|#\[allow',line):
        policy.append({'path':path,'line':line})
write('audit.json', {'protectedPaths':protected,'protectedDiffEmpty':not delta,
    'specDiffEmpty':not git('diff','c3b650b','--','spec'),
    'dependenciesDiff':git('diff','c3b650b','--','Cargo.lock','deny.toml'),
    'builderGateBase':receipt['baseCommit'],'builderGateHashes':hashes,'builderGateRustCounts':counts(gate),
    'builderGateStatuses':receipt['status'],'addedPolicyMatchesIncludingTests':policy})

ci = json.loads((OUT/'ci.json').read_text(encoding='utf-16'))
log = re.sub(r'(?:\x1b|\^\[)\[[0-9;]*m','',(OUT/'ci.log').read_text(encoding='utf-16'))
jobs = {}
for job in ci['jobs']:
    name = job['name']
    if not name.startswith('check ('): continue
    lines = [x.split('\t',3)[-1] for x in log.splitlines() if x.startswith(name+'\tRun cargo test --workspace --locked\t')]
    suites = {}; current = None; pending = None
    for line in lines:
        m = re.search(r'Running (.+?)(?: \(target| \([^)]*target)',line)
        if m: pending = m[1].replace('\\','/')
        if re.search(r'\brunning \d+ tests?$', line): current = pending
        m = summary_re.search(line)
        if m and current:
            suites[current] = [int(x) for x in m.groups()]
            current = None
    start = datetime.datetime.fromisoformat(job['startedAt'].replace('Z','+00:00'))
    end = datetime.datetime.fromisoformat(job['completedAt'].replace('Z','+00:00'))
    jobs[name] = {'minutes':(end-start).total_seconds()/60,'conclusion':job['conclusion'],'suites':suites,
                  'aggregate':counts('\n'.join(lines))}
write('ci-analysis.json', {'headSha':ci['headSha'],'conclusion':ci['conclusion'],'jobs':jobs})
print(json.dumps({'protectedDiffEmpty':not delta,'gateHashesAllMatch':all(hashes.values()),'gateCounts':counts(gate),
                 'ci':{k:{'minutes':v['minutes'],'aggregate':v['aggregate'],'suites':len(v['suites'])} for k,v in jobs.items()}},indent=2))
