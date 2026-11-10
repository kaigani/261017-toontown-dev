import hashlib,json,subprocess,os
from pathlib import Path
p=Path('reviews/evidence/M3-C3.3-r2')
files=sorted([*Path('crates/executor/tests').glob('c33_r2_*.rs'),*Path('crates/render/tests').glob('c33_r2_*.rs')])
r={'head':subprocess.check_output(['git','rev-parse','HEAD']).decode().strip(),'branch':subprocess.check_output(['git','branch','--show-current']).decode().strip(),'reviewSources':{str(f):hashlib.sha256(f.read_bytes()).hexdigest() for f in files},'commitReviewReceipts':os.environ.get('TT_COMMIT_REVIEW_RECEIPTS'),'sdkVersion':__import__('importlib.metadata').metadata.version('mcp'),'goldenChanges':subprocess.check_output(['git','diff','--name-only','e61c265..669ddc3','--','crates']).decode().splitlines()}
r['goldenChanges']=[s for s in r['goldenChanges'] if 'golden' in s.lower()]
r['priorRegistersDiff']=subprocess.check_output(['git','diff','--stat','825b1e1..669ddc3','--','reviews/evidence/M3-C3.1*','reviews/evidence/M3-C3.2*']).decode()
(p/'provenance.json').write_text(json.dumps(r,indent=2)+'\n')
