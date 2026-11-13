import json,subprocess,os
from pathlib import Path
base=Path('target/debug'); fp=json.loads((base/'.fingerprint/tt-executor-99253635ef97485b/test-integration-test-c33_r2_depth.json').read_text()); args=['rustc','--edition','2024','--test','crates/executor/tests/c33_r2_depth.rs','-L','dependency=target/debug/deps','-o',str(Path(os.environ['TEMP'])/'c33-r2-depth.exe')]
for _,name,_,fingerprint in fp['deps']:
 for p in (base/'.fingerprint').glob('*/lib-'+name):
  if int.from_bytes(bytes.fromhex(p.read_text().strip()),'little')==fingerprint:
   suffix=p.parent.name.rsplit('-',1)[1]; lib=base/'deps'/f'lib{name}-{suffix}.rlib'
   if lib.exists(): args+=['--extern',f'{name}={lib}'];break
out=Path('reviews/evidence/M3-C3.3-r2'); (out/'depth-diagnostic-command.json').write_text(json.dumps(args,indent=2))
with (out/'depth-fingerprint-compile.log').open('w') as f:r=subprocess.run(args,stdout=f,stderr=subprocess.STDOUT)
(out/'depth-fingerprint-compile.exit').write_text(str(r.returncode))
if not r.returncode:
 with (out/'depth-fingerprint.log').open('w') as f:r=subprocess.run([args[args.index('-o')+1],'--nocapture'],stdout=f,stderr=subprocess.STDOUT)
 (out/'depth-fingerprint.exit').write_text(str(r.returncode))
