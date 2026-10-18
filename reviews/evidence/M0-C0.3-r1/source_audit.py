import json
from pathlib import Path
import re
import subprocess
import tomllib
ROOT=Path(__file__).resolve().parents[3]
OUT=Path(__file__).resolve().parent
registry=Path.home()/'.cargo/registry/src/index.crates.io-1949cf8c6b5b557f'
tree=subprocess.check_output(['cargo','tree','--locked','-p','vello_cpu','--prefix','none'],cwd=ROOT,text=True)
(OUT/'vello-tree.txt').write_text(tree)
crates=sorted(set(re.findall(r'^([\w-]+) v([\d.]+)',tree,re.M)))
audit=[]
for name,version in crates:
    folder=registry/f'{name}-{version}'; data=tomllib.loads((folder/'Cargo.toml').read_text())
    lines=[line for file in (folder/'src').rglob('*.rs') for line in file.read_text(encoding='utf-8').splitlines()]
    audit.append({'name':name,'version':version,'license':data['package'].get('license'),'srcLines':len(lines),'unsafeWordLines':sum(bool(re.search(r'\bunsafe\b',line)) for line in lines if not line.lstrip().startswith('//'))})
(OUT/'dependency-audit.json').write_text(json.dumps({'crates':audit,'count':len(audit),'lines':sum(x['srcLines'] for x in audit),'unsafeWordLines':sum(x['unsafeWordLines'] for x in audit)},indent=2)+'\n')
selection={
 'vello_common-0.3.0/src/flatten_simd.rs':[(626,684),(246,286),(501,530)],
 'vello_common-0.3.0/src/filter/gaussian_blur.rs':[(137,158)],
 'vello_common-0.3.0/src/encode.rs':[(25,44),(911,932)],
 'vello_cpu-0.3.0/src/fine/common/rounded_blurred_rect.rs':[(165,187),(265,283)],
 'kurbo-0.13.1/src/stroke.rs':[(435,509)],
 'kurbo-0.13.1/src/offset.rs':[(342,358)],
 'vello_cpu-0.3.0/src/dispatch/multi_threaded.rs':[(40,46),(106,114),(202,244),(421,452)],
 'vello_cpu-0.3.0/src/fine/lowp/mod.rs':[(648,680)],
}
with (OUT/'source-excerpts.txt').open('w',encoding='utf-8') as f:
    for file,ranges in selection.items():
        lines=(registry/file).read_text(encoding='utf-8').splitlines()
        for start,end in ranges:
            for i in range(start,end+1): f.write(f'{file}:{i}: {lines[i-1]}\n')
            f.write('\n')

headroom=json.loads((OUT/'browser-headroom.json').read_text())
import math
summary=[]
for r in headroom['runs']:
    frames=r['frames']; ts=[f['t'] for f in frames]; gpu=sorted(f['gpuMs'] for f in frames if f['gpuMs'] is not None)
    assert r['drawCalls']==len(frames)+1
    assert r['totalInstances']==r['drawnSegments']*r['drawCalls']
    assert not r.get('error') and r.get('glError',0)==0
    summary.append({'backend':r['backend'],'multiple':r['multiple'],'fps':(len(ts)-1)*1000/(ts[-1]-ts[0]),'drawCalls':r['drawCalls'],'segmentsPerDraw':r['drawnSegments'],'gpuP50Ms':gpu[math.ceil(len(gpu)*.5)-1] if gpu else None,'gpuP95Ms':gpu[math.ceil(len(gpu)*.95)-1] if gpu else None,'gpuMaxMs':max(gpu) if gpu else None})
(OUT/'headroom-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
baseline=json.loads((ROOT/'crates/render/examples/c03_raster/hashes.json').read_text())['renderers']
config_differences={}
for config in ['vello_cpu_fallback','vello_cpu_mt3']:
    config_differences[config]=[s for s,h in baseline['vello_cpu_simd'].items() if h!=baseline[config][s]]
(OUT/'original-config-differences.json').write_text(json.dumps(config_differences,indent=2)+'\n')
print(json.dumps({'dependencies':len(audit),'sourceLines':sum(x['srcLines'] for x in audit),'unsafe':sum(x['unsafeWordLines'] for x in audit),'headroom':summary},indent=2))
