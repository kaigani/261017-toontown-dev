from pathlib import Path
import tempfile
p=Path(tempfile.gettempdir())/'toontown-c33-r1/package-evidence.py'
s=p.read_text()
needle="ci_oracles=[]"
insert="local_comparisons=[]\nfor family,local_dir,ci_dir in [('c33','gate','c33-Windows'),('e2','e2','e2-Windows')]:\n for f in sorted((SRC/local_dir).rglob('*')):\n  if f.is_file() and (f.suffix=='.png' or f.name in ['manifest.json','gate-path.json']):\n   other=SRC/'artifacts'/ci_dir/f.relative_to(SRC/local_dir)\n   equal=other.is_file() and f.read_bytes()==other.read_bytes()\n   local_comparisons.append(dict(family=family,file=f.relative_to(SRC/local_dir).as_posix(),equal=equal,sha256=sha(f.read_bytes())))\nfor local,relative in [('e4','evaluate.json'),('e4-variant','variant/evaluate.json')]:\n f=SRC/local/'evaluate.json'; other=SRC/'artifacts/e4-Windows'/relative\n local_comparisons.append(dict(family=local,file='evaluate.json',equal=f.read_bytes()==other.read_bytes(),sha256=sha(f.read_bytes())))\nassert all(item['equal'] for item in local_comparisons),local_comparisons\nwrite(OUT/'local-ci-comparisons.json',json.dumps(local_comparisons,indent=2)+'\\n')\n"
s=s.replace(needle,insert+needle)
s=s.replace('ciReleaseOracleSummaries=ci_oracles,','ciReleaseOracleSummaries=ci_oracles,localCiComparisons=local_comparisons,')
p.write_text(s)
