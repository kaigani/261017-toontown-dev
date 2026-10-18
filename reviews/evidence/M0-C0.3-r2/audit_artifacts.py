"""Audit all downloaded PNG bytes, report hashes, opacity, SIMD deltas and timings."""
import gzip
import hashlib
import json
from pathlib import Path
import statistics
import struct
import zlib

ROOT=Path(__file__).resolve().parents[3]
OUT=Path(__file__).resolve().parent
BASE=ROOT/'crates/render/examples/c03_raster'
H=json.loads((BASE/'hashes.json').read_text())
STRESS=json.loads((ROOT/'reviews/evidence/M0-C0.3-r1/stress-windows-baseline.json').read_text())
sha=lambda b:hashlib.sha256(b).hexdigest()

def rgba(b):
    pos=8; data=b''; chunks=[]
    while pos<len(b):
        n=struct.unpack('>I',b[pos:pos+4])[0]; kind=b[pos+4:pos+8]; payload=b[pos+8:pos+8+n]; pos+=12+n
        chunks.append(kind.decode())
        if kind==b'IHDR': w,h,depth,col,*_=struct.unpack('>IIBBBBB',payload)
        if kind==b'IDAT': data+=payload
    assert depth==8 and col==6 and chunks==['IHDR','IDAT','IEND']
    raw=zlib.decompress(data); stride=w*4; prior=bytearray(stride); out=bytearray(); at=0
    for _ in range(h):
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
    return w,h,out

opacity=[]; sizes=[]; golden={}
for config,scenes in H['renderers'].items():
    for name,hsh in scenes.items():
        b=(BASE/'goldens'/config/(name+'.png')).read_bytes(); golden[(config,name)]=b
        assert sha(b)==hsh['png']
        w,h,raw=rgba(b); alphas=raw[3::4]; opaque=not name.startswith('stress_')
        opacity.append(dict(config=config,scene=name,opaqueBackground=opaque,minAlpha=min(alphas),maxAlpha=max(alphas)))
        if opaque and config in ['ttraster','vello_cpu_f32_fallback','vello_cpu_f32_simd']: assert min(alphas)==255
        for i in range(0,len(raw),4):
            for k in range(3): raw[i+k]=(raw[i+k]*raw[i+3]+127)//255
        assert sha(b'TTPIX1'+struct.pack('>II',w,h)+raw)==hsh['pixels'], (config,name)
        if name.startswith('stress_') and config in ['ttraster','vello_cpu_fallback','vello_cpu_simd','vello_cpu_mt3']:
            assert sha(raw)==STRESS[f'{config}/{int(name[7:])}'], (config,name,'review corpus changed')
        sizes.append(len(b))
rows=[]; speed=[]; provenance=[]
for run in [39126106552,39126796883]:
    for osname in ['Windows','macOS','Linux']:
        base=ROOT/f'target/review-r2/artifacts/{run}/c03-raster-{osname}'
        for proc in [1,2]:
            reportpath=base/f'report-run{proc}.json'; r=json.loads(reportpath.read_text())
            assert r['hashes']==H and r['repeatFailures']==[] and r['checkMismatches']==[]
            for (config,name),b in golden.items():
                got=(base/f'run{proc}'/config/(name+'.png')).read_bytes()
                assert got==b,(run,osname,proc,config,name)
            rows.append(dict(run=run,os=osname,process=proc,entries=len(golden),detected=r['detectedSimdLevel'],configurations=r['configurations'],reportSha=sha(reportpath.read_bytes())))
            (OUT/f'artifact-{run}-{osname}-report{proc}.json.gz').write_bytes(gzip.compress(reportpath.read_bytes(),mtime=0))
            if run==39126106552 and proc==1:
                old=ROOT/f'reviews/evidence/M0-C0.3/raster-r1-report-ci-{osname}-{run}.json'
                provenance.append(dict(os=osname,committedReportIdentical=old.read_bytes()==reportpath.read_bytes()))
                for kind in ['fallback','simd']:
                    t=r['timingsMs']; sc='perf_2000_cubics_512'
                    u=statistics.median(t[f'vello_cpu_{kind}'][sc]['msAll']); f=statistics.median(t[f'vello_cpu_f32_{kind}'][sc]['msAll'])
                    upper=lambda values:sorted(values)[len(values)//2]
                    uu=upper(t[f'vello_cpu_{kind}'][sc]['msAll']); ff=upper(t[f'vello_cpu_f32_{kind}'][sc]['msAll'])
                    speed.append(dict(os=osname,kind=kind,u8Ms=u,f32Ms=f,statisticalMedianPercent=(f/u-1)*100,reportU8Ms=uu,reportF32Ms=ff,reportUpperMedianPercent=(ff/uu-1)*100))
diff={}
for prefix in ['vello_cpu','vello_cpu_f32']:
    a,b=H['renderers'][prefix+'_fallback'],H['renderers'][prefix+'_simd']
    diff[prefix]=[name for name in a if a[name]['pixels']!=b[name]['pixels']]
assert diff['vello_cpu']==diff['vello_cpu_f32'] and len(diff['vello_cpu'])==20
native=json.loads((OUT/'raster-native.json').read_text()); default=json.loads((OUT/'raster-default.json').read_text())
assert native['hashes']==default['hashes']==H
result=dict(goldens=len(golden),goldenBytes=sum(sizes),artifactPngsCompared=len(rows)*len(golden),artifacts=rows,opacity=opacity,simdDifferentScenes=diff,speed=speed,provenance=provenance,nativeEqualsDefault=True)
(OUT/'artifact-audit.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k not in ['opacity','artifacts']},indent=2))
