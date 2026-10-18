"""Independent CI/artifact and corrected C0.4 numerical audit."""
import gzip
import hashlib
import json
from pathlib import Path
import re
import statistics
import subprocess

ROOT=Path(__file__).resolve().parents[3]
OUT=Path(__file__).resolve().parent
BASE=ROOT/'crates/render/examples/c03_raster'
H=json.loads((BASE/'hashes.json').read_text())
sha=lambda b: hashlib.sha256(b).hexdigest()
goldens={}
for config,scenes in H['renderers'].items():
    for name,h in scenes.items():
        b=(BASE/'goldens'/config/(name+'.png')).read_bytes()
        assert sha(b)==h['png']
        goldens[(config,name)]=b
artifacts=[]
for osname in ['Windows','macOS','Linux']:
    base=ROOT/f'target/review-r4/artifacts/39131376596/c03-raster-{osname}'
    for proc in [1,2]:
        raw=(base/f'report-run{proc}.json').read_bytes()
        r=json.loads(raw)
        assert r['hashes']==H and not r['repeatFailures'] and not r['checkMismatches']
        for (config,name),b in goldens.items():
            assert (base/f'run{proc}'/config/(name+'.png')).read_bytes()==b
        artifacts.append(dict(os=osname,process=proc,detected=r['detectedSimdLevel'],configurations=r['configurations'],entries=len(goldens)))
        (OUT/f'artifact-39131376596-{osname}-report{proc}.json.gz').write_bytes(gzip.compress(raw,mtime=0))
costs=[]
provenance=[]
for run in [39126106552,39126796883]:
    for osname in ['Windows','macOS','Linux']:
        p=ROOT/f'reviews/evidence/M0-C0.3-r2/artifact-{run}-{osname}-report1.json.gz'
        r=json.loads(gzip.decompress(p.read_bytes()))
        assert r['hashes']==H
        provenance.append(dict(run=run,os=osname,detected=r['detectedSimdLevel']))
        if run==39126106552:
            assert gzip.decompress(p.read_bytes())==(ROOT/f'reviews/evidence/M0-C0.3/raster-r1-report-ci-{osname}-{run}.json').read_bytes()
            for level in ['fallback','simd']:
                t=r['timingsMs']; scene='perf_2000_cubics_512'
                u=statistics.median(t['vello_cpu_'+level][scene]['msAll'])
                f=statistics.median(t['vello_cpu_f32_'+level][scene]['msAll'])
                costs.append(dict(os=osname,level=level,u8Ms=u,f32Ms=f,percent=100*(f/u-1)))
ci_raw=(OUT/'ci-39131376596.json').read_bytes()
ci=json.loads(ci_raw.decode('utf-16' if ci_raw[:2]==b'\xff\xfe' else 'utf-8-sig'))
assert ci['headSha']=='18c57fe7c096d9a3162772681feeaedccc83ad93'
assert ci['conclusion']=='success' and len(ci['jobs'])==8
assert all(j['conclusion']=='success' for j in ci['jobs'])
(OUT/'ci-39131376596.json').write_text(json.dumps(ci,indent=2)+'\n')
logpath=OUT/'ci-39131376596-logs.log'
logbytes=logpath.read_bytes() if logpath.exists() else gzip.decompress(logpath.with_suffix('.log.gz').read_bytes())
log=re.sub(r'(?:\x1b|\^\[)\[[0-9;]*m','',logbytes.decode('utf-8'))
ci_checks=[]
for osname in ['windows-latest','macos-latest']:
    lines=[s for s in log.splitlines() if s.startswith(f'check ({osname})\t')]
    text='\n'.join(lines)
    suites=['c03-r3-page.test.tsx','c03-r1-binary.test.ts','c03-r1-tooling.test.ts','c03-r2-tooling.test.ts','c03-r2-page.test.tsx','c03-r2-binary.test.ts','c03_r2_encoder','c03_r1_stress','c03_fixed_configurations','c03_r2_quality']
    assert all(s in text for s in suites)
    counts=re.findall(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;',text)
    ci_checks.append(dict(os=osname,suites=suites,rustPassed=sum(int(a) for a,b in counts),rustIgnored=sum(int(b) for a,b in counts),frontendSummary=[s for s in lines if 'Test Files ' in s or 'Tests ' in s]))
selected=[s for s in log.splitlines() if any(k in s for k in ['c03-r3-page.test','c03-r1-binary.test','c03-r1-tooling.test','c03-r2-tooling.test','c03-r2-page.test','c03-r2-binary.test','c03_r2_encoder','c03_r1_stress','c03_fixed_configurations','c03_r2_quality','fixed_scalar_configuration_hashes','c03_r1_04','c03_r1_05','u8_pipeline_loses','Test Files ','Tests ','hashes match','test result: ok. 2 passed'])]
(OUT/'ci-test-excerpts.txt').write_text('\n'.join(selected)+'\n',encoding='utf-8')
result=dict(goldens=len(goldens),artifactPngsCompared=len(artifacts)*len(goldens),artifacts=artifacts,priorRunSimd=provenance,f32Costs=costs,areaExtrapolation={f'{w}x{h}':[t*w*h/512**2 for t in [6.9,4.8,18.5]] for w,h in [(1280,720),(1920,1080),(3840,2160)]},ci=ci_checks)
(OUT/'evidence-audit.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k!='artifacts'},indent=2))
