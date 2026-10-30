"""Compare original Cargo targets with the completed scoped regression runs."""
from pathlib import Path
import json,re
E=Path(__file__).resolve().parent
R=E.parents[2]
def parse(text):
 text=re.sub(r'\x1b\[[0-9;]*m','',text); current=None; found={}
 for line in text.splitlines():
  match=re.search(r'Running .+\((?:.*[/\\])([^/\\()]+)-[a-f0-9]+(?:\.exe)?\)',line)
  if match: current=match[1]
  counts=re.search(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored',line)
  if current and counts:
   found[current]=tuple(map(int,counts.groups()));current=None
 return found
old=parse((E.parent/'M1-C1.4/gates/test.log').read_text(encoding='utf-8'))
new={}
for name in ['workspace.log','remaining-workspace.log','mcp-remaining.log']:
 new.update(parse((E/name).read_text(encoding='utf-16')))
missing=set(old)-set(new)
changes={key:(value,new[key]) for key,value in old.items() if key in new and value!=new[key]}
assert not missing and not changes,(missing,changes)
result={'originalTargets':len(old),'coveredOriginalTargets':len(set(old)&set(new)),'missing':sorted(missing),'failures':{key:value for key,value in new.items() if value[1]},'originalTargetCountChanges':changes}
(E/'workspace-coverage.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
print(json.dumps(result))
