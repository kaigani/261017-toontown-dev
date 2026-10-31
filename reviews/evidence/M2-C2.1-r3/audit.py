"""Recompute final-round receipts, CI observations, ownership and source-row register."""
from pathlib import Path
from collections import Counter
import difflib, gzip, hashlib, json, re, subprocess
ROOT=Path(__file__).resolve().parents[3]
HERE=Path(__file__).resolve().parent
TMP=ROOT/'.review-r3-scratch'
def git(*args):
    return subprocess.check_output(['git',*args],cwd=ROOT).decode('utf8')
def read(p):
    b=Path(p).read_bytes()
    return b.decode('utf-16' if b.startswith((b'\xff\xfe',b'\xfe\xff')) else 'utf-8-sig')
def totals(p):
    matches=re.findall(r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;',read(p))
    return list(map(sum,zip(*(map(int,m) for m in matches)))) if matches else []
base='9498cfe';head='354aae9'
paths=git('ls-tree','-r','--name-only',base).splitlines()
reviewer=[p for p in paths if p.startswith('tests/adversarial/') or re.search(r'/tests/(?:c\d+_r\d|c21_r[12]_)',p)]
changed=[p for p in reviewer if git('diff',base,head,'--',p)]
assert not changed,changed
assert not git('diff',base,'--',*reviewer),'pre-existing reviewer-owned paths changed in the review worktree'
prohibited=[]
for p in git('diff','--name-only',base,head).splitlines():
    if not p.endswith('.rs') or '/src/' not in p:continue
    a=git('show',base+':'+p).split('#[cfg(test)]')[0]
    b=git('show',head+':'+p).split('#[cfg(test)]')[0]
    for line in difflib.unified_diff(a.splitlines(),b.splitlines(),n=0):
        if line.startswith('+') and not line.startswith('+++') and re.search(r'\b(?:unwrap|expect)\s*\(|\bpanic!\s*\(|\bunsafe\s*(?:\{|fn|impl)|#\s*\[\s*allow',line):prohibited.append([p,line])
assert not prohibited,prohibited
ci=[];observations=[]
for label,run in [('first',41181900335),('second',41184912211)]:
    receipt=json.loads(read(HERE/f'ci-r3-{label}.json'))
    assert receipt['conclusion']=='success' and all(j['conclusion']=='success' for j in receipt['jobs'])
    ci.append({'run':run,**receipt})
    log=TMP/f'ci-{label}.log'
    compressed=HERE/f'ci-{run}.log.gz'
    if log.exists():
        compressed.write_bytes(gzip.compress(log.read_bytes(),mtime=0))
    raw=gzip.decompress(compressed.read_bytes())
    lines=raw.decode('utf-16' if raw.startswith((b'\xff\xfe',b'\xfe\xff')) else 'utf-8-sig').splitlines()
    selected=set()
    for i,line in enumerate(lines):
        if any(s in line for s in ['c21_determinism','golden digest','SDK conformance','c21_history','c21_r2_geometry']):
            selected.update(range(max(0,i-1),min(len(lines),i+9)))
    excerpts=[lines[i] for i in sorted(selected)]
    (HERE/f'ci-{run}-excerpts.txt').write_text('\n'.join(excerpts)+'\n',encoding='utf8')
    downloaded=sorted((TMP/f'ci-{label}-artifacts').rglob('*worst-case-*.json'))
    for path in downloaded or sorted(HERE.glob(f'ci-{run}-*worst-case-*.json')):
        d=json.loads(path.read_text(encoding='utf8'))
        filename=path.name.removeprefix(f'ci-{run}-')
        allruns=[(c['name'],r) for c in d['cases'] for r in c['runs']]
        name,worst=max(allruns,key=lambda item:item[1]['wallMs'])
        assert abs(worst['wallMs']-d['worst']['wallMs'])<1e-6
        fact={'run':run,'file':filename,'runs':len(allruns),'worstCase':name,'worstMs':worst['wallMs'],
              'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}
        if filename.startswith('brush'):
            accepted=[(n,r) for n,r in allruns if r['outcome']=='ok']
            refused=[(n,r) for n,r in allruns if r['outcome']!='ok']
            fact['acceptedWorstMs']=max(r['wallMs'] for n,r in accepted)
            fact['refusedWorstMs']=max(r['wallMs'] for n,r in refused)
            assert all(r['units']<=d['budgetUnits'] for n,r in accepted)
        observations.append(fact)
        destination=HERE/f'ci-{run}-{filename}'
        if destination != path: destination.write_bytes(path.read_bytes())
        retained=ROOT/'reviews/evidence/M2-C2.1'/f'ci-{run}-{filename}'
        if retained.exists():assert json.loads(retained.read_text(encoding='utf8'))==d
assert len(observations)==8,observations
out={'base':git('rev-parse',base).strip(),'builderHead':git('rev-parse',head).strip(),
     'reviewerPaths':len(reviewer),'changedReviewerPaths':changed,'prohibitedAdditions':prohibited,
     'lockUnchanged':not git('diff',base,head,'--','Cargo.lock','deny.toml'),
     'goldenUnchanged':not git('diff',base,head,'--','crates/executor/tests/golden/c21-determinism.json'),
     'generatedFresh':not git('diff','--','crates/registry/src/declared.rs','crates/registry/src/declared.json'),
     'builderGateTotals':totals(ROOT/'reviews/evidence/M2-C2.1/workspace-tests.log'),
     'ci':ci,'observations':observations}
assert out['builderGateTotals']==[826,0,12]
for name in ['workspace-tests','geometry','geometry-all-final','geometry-release','contracts-final','contracts-all-final','contracts-release','history-release','history-oracles-release','mcp-history-oracles-release','worst-case-release','malformed-mcp','closed-command-release']:
    path=HERE/f'{name}.log'
    if path.exists():out[name]=totals(path)
(HERE/'audit.json').write_text(json.dumps(out,indent=2)+'\n',encoding='utf8')

# Re-extract the frozen source inventory using the retained round-2 generator.
source=(HERE.parent/'M2-C2.1-r2/build-register.py').read_text(encoding='utf8')
exec(compile(source,'round2-register','exec'),{'__file__':str(HERE/'build-register.py'),'__name__':'inventory'})
rows=json.loads((HERE/'conformance.json').read_text(encoding='utf8'))['rows']
plan=(ROOT/'plan/IMPLEMENTATION_PLAN.md').read_text(encoding='utf8')
for row in rows:
    if 'coverage risk P2-09' in row['evidence']:
        row['evidence']=row['evidence'].replace('coverage risk P2-09','C21-R1-11: nonidentity cross-OS corpus and aggregate work, M2 C2.3 before M4 C4.2')
    if row['status']=='divergent':
        if '`draw.stroke.set`' in row['requirement']:
            row.update(status='implemented',evidence='c21_r2_contracts; c21_r1_contracts; c21_r3_contracts::over_limit_brush_refuses_atomically_and_worker_accepts_next_edit; c21_worst_case (R2-03 closed)')
        else:
            issue='C21-R3-03 (C2.1 builder): character formats change after canonical reordering; c21_r3_geometry::canonical_reordering_preserves_each_marks_own_format; c21_r3_contracts::deleting_a_separator_keeps_formats_on_reordered_hebrew_points'
            if row['section'] in ['3.2','3.3','3.4'] or '`draw.stroke.add`' in row['requirement'] or '`spine`' in row['requirement']:
                issue='C21-R3-01/02 (C2.1 builder): exact duplicates lose pressure / closing pressure moves to first edge; c21_r3_geometry; c21_r3_contracts::stationary_pressure_dab_is_not_refused_as_zero_area'
            row['evidence']=issue
    if row['status']=='implemented':
        assert re.search(r'c\d|test',row['evidence']),row
    if row['status']=='deferred-with-owner':
        assert re.search(r'C\d\.\d',row['evidence']),row
        for checkpoint in re.findall(r'C\d+\.\d+',row['evidence']):assert checkpoint in plan,checkpoint
counts=dict(Counter(r['status'] for r in rows))
assert len(rows)==365
(HERE/'conformance.json').write_text(json.dumps({'counts':counts,'rows':rows},indent=2,ensure_ascii=False)+'\n',encoding='utf8')
lines=['# C2.1 round 3 conformance register','',f'Frozen inventory re-extracted: {len(rows)} rows. Counts: {counts}. Missing: 0.',
       '', 'Implemented rows certify the engine subset with the test references shown. Deferred owners retain the plan section 4 checkpoint and qualifications from rounds 1–2. Open divergences are owned by the C2.1 builder before acceptance, with orchestrator adjudication after this final full round.',
       '', '| # | Source / clause | Status | Evidence or owner |','|---|---|---|---|']
for i,r in enumerate(rows,1):lines.append(f"| {i} | {r['source']} section {r['section']}: {r['requirement'].replace('|',' / ')} | {r['status']} | {r['evidence'].replace('|',' / ')} |")
(HERE/'conformance.md').write_text('\n'.join(lines)+'\n',encoding='utf8')
print(json.dumps({'counts':counts,'gate':out['builderGateTotals'],'reviewerPaths':len(reviewer),'observations':observations},indent=2))
