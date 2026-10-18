"""Independent scene counts, timing recomputation, artifact packaging and source-change audit."""
import gzip
import hashlib
import json
import math
from pathlib import Path
import struct
import subprocess
import tarfile
import sys
ROOT=Path(__file__).resolve().parents[3]
OUT=Path(__file__).resolve().parent
sys.path.insert(0,str(OUT))
from run_checks import run

def stats(v):
    v=sorted(v)
    return {'p50':v[math.ceil(len(v)*.5)-1],'p95':v[math.ceil(len(v)*.95)-1],'max':max(v)}
scenes={}
for path in (ROOT/'target/review-scenes').glob('*.ttrl'):
    b=path.read_bytes(); G,P,V,N=struct.unpack_from('<4I',b,20)
    paths=72+G*24; points=paths+P*28; verbs=points+N*8
    count={'groups':G,'paths':P,'points':N,'verbs':V,'bytes':len(b),'strokePaths':0,'fillPaths':0,'cubics':0,'segments':0}
    for i in range(P):
        first,n,pt,group,color,style,width=struct.unpack_from('<6If',b,paths+i*28)
        assert group<G and math.isfinite(width) and width>=0 and first+n<=V
        count['strokePaths' if style==2 else 'fillPaths']+=1
        x0=y0=0.; segments=0
        for v in b[verbs+first:verbs+first+n]:
            if v in [0,1]:
                x0,y0=struct.unpack_from('<ff',b,points+pt*8); pt+=1
                if v==1: segments+=1
            elif v==2:
                x1,y1,x2,y2,x3,y3=struct.unpack_from('<6f',b,points+pt*8); pt+=3
                ddx=max(abs(x0-2*x1+x2),abs(x1-2*x2+x3)); ddy=max(abs(y0-2*y1+y2),abs(y1-2*y2+y3))
                segs=min(32,max(1,math.ceil(math.sqrt(.75*math.sqrt(ddx*ddx+ddy*ddy)/.5))))
                segments+=segs; count['cubics']+=1; x0,y0=x3,y3
            else: assert v==3
            assert pt<=N
        if style==2: count['segments']+=segments
    scenes[path.stem]=count

tables={}; matches=[]
for path in list((ROOT/'reviews/evidence/M0-C0.3').glob('canvas-*.json.gz'))+[OUT/'canvas-auto.json',OUT/'canvas-webgl2.json']:
    log=json.loads(gzip.decompress(path.read_bytes()) if path.suffix=='.gz' else path.read_bytes())
    rows=[]
    for r in log['runs']:
        if r['error']: rows.append({'backend':r['backend'],'scene':r['scene'],'error':r['error']}); continue
        t=[f['t'] for f in r['frames']]; iv=[b-a for a,b in zip(t,t[1:])]
        inv=[s['invokeMs'] for s in r['transfers']]
        assert all(s['bytes']==scenes[r['scene']]['bytes'] for s in r['transfers'])
        for k in ['paths','strokePaths','fillPaths','cubics','segments']: assert r['decode'][k]==scenes[r['scene']][k], (path,r['scene'],k)
        gpu=[f['gpuMs'] for f in r['frames'] if f['gpuMs'] is not None]
        rows.append({'backend':r['backend'],'scene':r['scene'],'fps':(len(t)-1)*1000/(t[-1]-t[0]),'intervalMs':stats(iv),'transferMs':stats(inv),'encodeMs':stats([s['encodeNs']/1e6 for s in r['transfers']]),'decodeMs':r['decode']['ms'],'upload':r['upload'],'gpuMs':stats(gpu) if gpu else None,'refreshMs':(r['idle'][-1]-r['idle'][0])/(len(r['idle'])-1),'durationMs':t[-1]-t[0]})
    tables[path.name]=rows
    if path.parent.name=='M0-C0.3':
        original=path.with_suffix('').with_suffix('.analysis.md')
        generated=OUT/original.name
        matches.append({'file':original.name,'exactTableTextMatches':original.read_text(encoding='utf-8')==generated.read_text(encoding='utf-8')})

tar=ROOT/'target/review-ci-current/desktop-macOS/Toontown-macos-arm64.app.tar.gz'
with tarfile.open(tar) as f:
    app=f.getmember('Toontown.app/Contents/MacOS/toontown')
    mac={'executableMode':oct(app.mode),'executableBytes':app.size,'analyzerMatchesSource':(ROOT/'target/review-ci-current/desktop-macOS/analyse-canvas-log.py').read_bytes()==(ROOT/'scripts/analyse-canvas-log.py').read_bytes()}

audit={'sceneCounts':scenes,'independentTables':tables,'committedAnalysisText':matches,'macArtifact':mac}
(OUT/'independent-audit.json').write_text(json.dumps(audit,indent=2)+'\n')
reports=sorted((ROOT/'reviews/evidence/M0-C0.3').glob('raster-report-*.json'))
run('raster-compare',['python','scripts/compare-raster-reports.py','crates/render/examples/c03_raster/hashes.json',*[str(p) for p in reports]])
for label,args in [
 ('reviewer-tests-unchanged',['git','diff','ac78acb..11247ff','--','tests/adversarial','desktop/src/test']),
 ('product-unchanged',['git','diff','ac78acb..11247ff','--','crates/render/src','deny.toml']),
 ('architecture-diff',['git','diff','ac78acb..11247ff','--','desktop/src/ipc','desktop/src-tauri/src/lib.rs','desktop/src-tauri/src/diagnostics.rs','desktop/src-tauri/src/spikes/tablet/mod.rs']),
 ('changed-files',['git','diff','--name-only','ac78acb..11247ff']),
 ('unsafe-and-boundary',['rg','-n','unsafe|@tauri-apps|__TAURI','desktop/src/spikes/canvas','desktop/src-tauri/src/spikes/canvas','crates/render/examples/c03_raster']),
]: run(label,args)
print(json.dumps({'scenes':scenes,'textMatches':matches,'mac':mac},indent=2))
