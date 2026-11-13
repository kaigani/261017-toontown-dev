from pathlib import Path
import json,re,hashlib,subprocess,shutil
E=Path('reviews/evidence/M3-C3.4-r4'); summary={}
for exit_file in E.glob('*.exit'):
 name=exit_file.stem; log=E/(name+'.log'); text=log.read_text(encoding='utf-8',errors='replace') if log.exists() else ''
 counts=[dict(zip(['passed','failed','ignored','measured','filtered'],map(int,m))) for m in re.findall(r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out',text)]
 summary[name]={'exit':int(exit_file.read_text()),'totals':{k:sum(c[k] for c in counts) for k in ['passed','failed','ignored','measured','filtered']},'failure_lines':[l for l in text.splitlines() if ' ... FAILED' in l or l.startswith('error:')],'test_summaries':len(counts)}
(E/'execution-summary.json').write_text(json.dumps(summary,indent=2))
if (E/'E1/report.json').exists():
 old=Path('reviews/evidence/M3-C3.4-r3/E1-final'); new=E/'E1'; paths=json.loads(Path('reviews/evidence/M3-C3.4-r3/e1-comparison.json').read_text())
 compare=[]
 for row in paths:
  p=row['file']; a=(old/p).read_bytes();b=(new/p).read_bytes();compare.append({'file':p,'equal':a==b,'sha256':hashlib.sha256(b).hexdigest()})
 (E/'e1-comparison.json').write_text(json.dumps(compare,indent=2));print('E1 equal',sum(r['equal'] for r in compare),'/',len(compare))
(E/'storage.json').write_text(json.dumps({'free_bytes':shutil.disk_usage(Path.cwd()).free,'minimum_bytes':120*1024**3},indent=2))
print(json.dumps(summary,indent=2))
