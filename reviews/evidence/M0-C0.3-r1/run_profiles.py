import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
ROOT=Path(__file__).resolve().parents[3]
sys.path.insert(0,str(Path(__file__).parent))
from run_checks import run, OUT
expected=json.loads((ROOT/'crates/render/examples/c03_raster/hashes.json').read_text())['renderers']['ttraster']
profiles={
    'o0-generic':['-C','opt-level=0','-C','codegen-units=16'],
    'o3-generic':['-C','opt-level=3','-C','codegen-units=16'],
    'o3-native-lto':['-C','opt-level=3','-C','codegen-units=1','-C','lto=fat','-C','target-cpu=native'],
}
results={}
for name,flags in profiles.items():
    exe=ROOT/f'target/profile-{name}.exe'; directory=ROOT/f'target/profile-{name}'
    assert run('compile-'+name,['rustc','--edition','2024','reviews/evidence/M0-C0.3-r1/raster_profiles.rs','-o',str(exe),*flags])==0
    assert run('run-'+name,[str(exe),str(directory)])==0
    hashes={s:hashlib.sha256(b'TTPIX1'+struct.pack('>II',h['width'],h['height'])+(directory/(s+'.rgba')).read_bytes()).hexdigest() for s,h in expected.items()}
    results[name]={'flags':flags,'hashes':hashes,'all23Match':all(hashes[s]==h['pixels'] for s,h in expected.items())}
(OUT/'profile-audit.json').write_text(json.dumps(results,indent=2)+'\n')
print({name:r['all23Match'] for name,r in results.items()})
