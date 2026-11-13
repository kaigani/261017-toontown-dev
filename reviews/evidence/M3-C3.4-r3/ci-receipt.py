"""Round-specific CI receipt collector; no product assertions."""
import re,os,json
from pathlib import Path
p=Path(os.environ['TEMP'])/'c34-r3-ci.log'
text=p.read_text(encoding='utf-16' if p.read_bytes()[:2]==b'\xff\xfe' else 'utf-8')
rows=[]; binary=None
for line in text.splitlines():
    line=re.sub(r'(?:\x1b|\^\[)\[[0-9;]*m','',line)
    if not line.startswith('check (windows-latest)\t'): continue
    m=re.search(r'Running (.+?) \(target',line)
    if m: binary=m[1]
    m=re.search(r'test result:.*finished in ([\d.]+)s',line)
    if m and binary: rows.append({'binary':binary,'seconds':float(m[1])}); binary=None
out=Path('reviews/evidence/M3-C3.4-r3/ci-timings.json')
out.write_text(json.dumps({'top10':sorted(rows,key=lambda r:r['seconds'],reverse=True)[:10],'c34':[r for r in rows if 'c34' in r['binary']],'binaries':len(rows)},indent=2)+'\n')
print(out.read_text())


