"""Independent artifact audit; PNG decoding uses Python's standard library."""
import base64
import gzip
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import zlib

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
os.environ['PYTHONIOENCODING'] = 'utf-8'
def digest(b): return hashlib.sha256(b).hexdigest()
def png(path):
    b=path.read_bytes(); pos=8; chunks=[]; data=b''
    while pos < len(b):
        n=struct.unpack('>I',b[pos:pos+4])[0]; kind=b[pos+4:pos+8]; payload=b[pos+8:pos+8+n]; pos+=12+n
        chunks.append(kind.decode())
        if kind==b'IHDR': w,h,depth,col,*_=struct.unpack('>IIBBBBB',payload)
        if kind==b'IDAT': data+=payload
    assert depth==8 and col==6
    raw=zlib.decompress(data); stride=w*4; prior=bytearray(stride); out=bytearray(); at=0
    for y in range(h):
        mode=raw[at]; row=bytearray(raw[at+1:at+1+stride]); at+=stride+1
        for x in range(stride):
            a=row[x-4] if x>=4 else 0; bb=prior[x]; c=prior[x-4] if x>=4 else 0
            if mode==1: v=a
            elif mode==2: v=bb
            elif mode==3: v=(a+bb)//2
            elif mode==4:
                p=a+bb-c; pa,pb,pc=abs(p-a),abs(p-bb),abs(p-c)
                v=a if pa<=pb and pa<=pc else bb if pb<=pc else c
            else: assert mode==0; v=0
            row[x]=(row[x]+v)%256
        out.extend(row); prior=row
    if not all(out[i]==255 for i in range(3,len(out),4)):
        print('nonopaque', path, sorted(set(out[3::4])), flush=True)
        for i in range(0, len(out), 4):
            a=out[i+3]
            for k in range(3): out[i+k]=(out[i+k]*a+127)//255
    return digest(b'TTPIX1'+struct.pack('>II',w,h)+out), digest(b), chunks

baseline=json.loads((ROOT/'crates/render/examples/c03_raster/hashes.json').read_text())
rows=[]; provenance=[]
for folder in ['review-ci-current','review-ci-historical']:
    for system in ['Windows','macOS','Linux']:
        base=ROOT/'target'/folder/('c03-raster-'+system)
        for run in [1,2]:
            report=base/f'report-run{run}.json'
            r=json.loads(report.read_text())
            assert r['hashes']==baseline and not r['repeatFailures'] and not r['checkMismatches']
            count=0; kinds=set()
            for renderer,scenes in baseline['renderers'].items():
                for name,h in scenes.items():
                    ph,fh,chunks=png(base/f'run{run}'/renderer/(name+'.png'))
                    assert ph==h['pixels'] and fh==h['png'], (folder,system,run,renderer,name)
                    count+=1; kinds.update(chunks)
            rows.append(dict(artifact=folder,os=system,run=run,entries=count,simd=r['detectedSimdLevel'],chunks=sorted(kinds),reportSha256=digest(report.read_bytes())))
            (OUT/f'{folder}-{system}-report{run}.json.gz').write_bytes(gzip.compress(report.read_bytes(),mtime=0))
        if folder=='review-ci-historical':
            committed=ROOT/f'reviews/evidence/M0-C0.3/raster-report-ci-{system}-39121780905.json'
            provenance.append(dict(file=str(committed.relative_to(ROOT)),byteIdentical=committed.read_bytes()==(base/'report-run1.json').read_bytes()))
    for system in ['Windows','macOS']:
        b=ROOT/'target'/folder/('desktop-'+system)/f'canvas-spike-{system}.json'
        if folder=='review-ci-historical':
            path=ROOT/f'reviews/evidence/M0-C0.3/canvas-ci-{system.lower()}-runner-39121780905.json.gz'
            provenance.append(dict(file=str(path.relative_to(ROOT)),byteIdentical=gzip.decompress(path.read_bytes())==b.read_bytes(),sha256=digest(b.read_bytes())))
        (OUT/f'{folder}-canvas-{system}.json.gz').write_bytes(gzip.compress(b.read_bytes(),mtime=0))

analyses=[]
for path in list((ROOT/'reviews/evidence/M0-C0.3').glob('canvas-*.json.gz'))+list(OUT.glob('*canvas-*.json.gz'))+list(OUT.glob('canvas-*.json')):
    if '.analysis.' in path.name or '.receipt.' in path.name: continue
    stem=path.name.removesuffix('.gz').removesuffix('.json')
    p=subprocess.run([sys.executable,str(ROOT/'scripts/analyse-canvas-log.py'),str(path),'--json',str(OUT/(stem+'.analysis.json')),'--snapshots',str(OUT/'snapshots'/stem)],capture_output=True,text=True,encoding='utf-8')
    (OUT/(stem+'.analysis.md')).write_text(p.stdout,encoding='utf-8')
    analyses.append(dict(file=str(path.relative_to(ROOT)),exitCode=p.returncode,stderr=p.stderr))
    assert p.returncode==0, (path, p.stderr)
jobs=json.loads((OUT/'ci-jobs.json').read_bytes())
job_summary=[dict(name=j['name'],conclusion=j['conclusion'],start=j['startedAt'],end=j['completedAt']) for j in jobs['jobs']]
(OUT/'artifact-audit.json').write_text(json.dumps(dict(raster=rows,provenance=provenance,analyses=analyses,jobs=job_summary),indent=2)+'\n')
print(json.dumps(dict(entriesRecomputed=sum(r['entries'] for r in rows),provenance=provenance,jobs=job_summary),indent=2))
