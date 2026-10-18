"""Compare native CPU flags for scalar vello; optimisation levels for ttraster."""
from pathlib import Path
import hashlib
import json
import os
import subprocess
import time

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
os.chdir(ROOT)
env = {**os.environ, 'RUSTFLAGS': '-C target-cpu=native', 'CARGO_TARGET_DIR': str(ROOT/'target/review-r2/native')}
cmd = ['cargo','run','--release','--locked','-p','tt-render','--example','c03_raster','--','--repeat','3','--out','target/review-r2/native-png','--report',str(OUT/'raster-native.json'),'--check','crates/render/examples/c03_raster/hashes.json']
t = time.time()
with (OUT/'raster-native.log').open('wb') as f:
    p = subprocess.run(cmd,env=env,stdout=f,stderr=subprocess.STDOUT)
(OUT/'raster-native.receipt.json').write_text(json.dumps(dict(command=cmd,flags=env['RUSTFLAGS'],exitCode=p.returncode,seconds=time.time()-t),indent=2)+'\n')
print('native completed',p.returncode,flush=True)
for opt in [1,3]:
    exe = ROOT/f'target/review-r2/tt-opt{opt}.exe'
    cmd=['rustc','--edition=2024',f'-Copt-level={opt}','reviews/evidence/M0-C0.3-r1/raster_profiles.rs','-o',str(exe)]
    with (OUT/f'tt-opt{opt}.log').open('wb') as f:
        subprocess.run(cmd,stdout=f,stderr=subprocess.STDOUT,check=True)
        subprocess.run([str(exe),f'target/review-r2/tt-opt{opt}'],stdout=f,stderr=subprocess.STDOUT,check=True)
baseline=json.loads((ROOT/'crates/render/examples/c03_raster/hashes.json').read_text())['renderers']['ttraster']
rows=[]
for name,h in baseline.items():
    a=(ROOT/f'target/review-r2/tt-opt1/{name}.rgba').read_bytes()
    b=(ROOT/f'target/review-r2/tt-opt3/{name}.rgba').read_bytes()
    ph=hashlib.sha256(b'TTPIX1'+h['width'].to_bytes(4,'big')+h['height'].to_bytes(4,'big')+a).hexdigest()
    rows.append(dict(scene=name,opt1EqualsOpt3=a==b,committedMatch=ph==h['pixels']))
(OUT/'ttraster-profiles.json').write_text(json.dumps(rows,indent=2)+'\n')
print('ttraster',len(rows),'scenes',all(r['opt1EqualsOpt3'] and r['committedMatch'] for r in rows),flush=True)
