"""Fetch raw gh output and verify exact-head suite coverage and raster artifacts."""
import gzip
import hashlib
import json
import pathlib
import re
import subprocess

OUT = pathlib.Path(__file__).resolve().parent
ROOT = OUT.parents[2]
RUN = '39137507872'


def gh(*args):
    return subprocess.check_output(['gh', *args], cwd=ROOT)


meta = json.loads(gh('run','view',RUN,'--json','headSha,conclusion,status,jobs,url,headBranch,createdAt'))
raw = gh('run','view',RUN,'--log')
(OUT/f'ci-{RUN}.json').write_text(json.dumps(meta,indent=2)+'\n',encoding='utf-8')
(OUT/f'ci-{RUN}.log.gz').write_bytes(gzip.compress(raw,mtime=0))
log = re.sub(r'(?:\x1b|\^\[)\[[0-9;]*m','',raw.decode('utf-8'))
assert meta['headSha'] == 'a89ca1b799dd5fa72878ed708f5562f98374cd42'
assert meta['conclusion'] == 'success' and len(meta['jobs']) == 8
local = (OUT/'test.log').read_text(encoding='utf-8')
test_pattern = r'src/[^\s\x1b]+\.test\.[jt]sx?'
local_files = {p.relative_to(ROOT/'desktop').as_posix() for p in (ROOT/'desktop/src').rglob('*')
               if re.search(r'\.test\.[jt]sx?$',p.name)}
assert len(local_files) == 32
audit = dict(headSha=meta['headSha'],conclusion=meta['conclusion'],jobs=[])
for job in meta['jobs']:
    assert job['conclusion'] == 'success'
    lines = [line for line in log.splitlines() if line.startswith(job['name']+'\t')]
    text = '\n'.join(lines)
    files = sorted(set(re.findall(test_pattern,text)))
    totals = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',text)
    summaries = [line for line in lines if re.search(r'Test Files|Tests\s+\d|test result:|# pass|# fail',line)]
    if job['name'].startswith('check '):
        assert set(files) == local_files, (job['name'],set(files)^local_files)
        assert '271 passed | 3 expected fail' in text
        assert '# pass 9' in text and '# fail 0' in text
    audit['jobs'].append(dict(name=job['name'],conclusion=job['conclusion'],
                              steps={s['name']:s['conclusion'] for s in job['steps']},
                              frontend_test_files=files, test_summaries=summaries,
                              rust_totals=[list(map(int,t)) for t in totals],
                              gate_lines=[line for line in lines if re.search(
                                  r'diagnostics gating:|M0 diagnostic and this build has none|IPC boundary \(dist check\):|hashes match|mismatches',line)]))
golden = json.loads((ROOT/'crates/render/examples/c03_raster/hashes.json').read_bytes())
artifacts = []
for path in sorted((ROOT/'target/c04-r2-ci').glob('*/report-run*.json')):
    data = path.read_bytes()
    report = json.loads(data)
    assert report['hashes'] == golden and not report['checkMismatches'] and not report['repeatFailures']
    label = path.parent.name
    run = path.stem.replace('report-', '')
    matches = []
    for png in sorted((path.parent/run).rglob('*.png')):
        reference = ROOT/'crates/render/examples/c03_raster/goldens'/png.relative_to(path.parent/run)
        assert png.read_bytes() == reference.read_bytes(), png
        matches.append(dict(path=png.relative_to(path.parent).as_posix(),sha256=hashlib.sha256(png.read_bytes()).hexdigest()))
    assert len(matches) == 234
    (OUT/f'{label}-{path.name}.gz').write_bytes(gzip.compress(data,mtime=0))
    artifacts.append(dict(artifact=label,report=path.name,sha256=hashlib.sha256(data).hexdigest(),
                          entries=sum(len(v) for v in golden['renderers'].values()),pngs=matches))
assert len(artifacts) == 6
audit['raster_artifacts'] = artifacts
(OUT/'ci-audit.json').write_text(json.dumps(audit,indent=2)+'\n',encoding='utf-8')
print('Exact-head CI: 8 jobs; 32/32 frontend files on both OSes; 6 x 234 hashes and PNG bytes match')
