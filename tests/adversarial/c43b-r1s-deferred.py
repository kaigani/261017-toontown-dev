"""Read-only deferred-section audit; run from any directory on either platform."""
from pathlib import Path
import re, json, collections
root = Path(__file__).resolve().parents[2]
d = json.loads((root/'desktop/src/ui-bindings/deferred.json').read_text(encoding='utf-8'))['deferred']
errors, rows = [], []
for command, v in d.items():
    current, refs = None, []
    for token in re.finditer(r'(?:(\d{2})\s*)?§(\d+(?:\.\d+)*)', v['section']):
        if token[1]: current = token[1]
        files = list((root/'spec').glob(str(current)+'_*.md'))
        matches = []
        if files:
            matches = [(i,l) for i,l in enumerate(files[0].read_text(encoding='utf-8').splitlines(),1) if re.match(r'^#+\s+'+re.escape(token[2])+r'(?:\.|\s)',l)]
        refs.append({'file':str(files[0].relative_to(root)) if files else current,'section':token[2],'headings':matches})
        if not matches: errors.append(command+': missing heading '+str(current)+' §'+token[2])
    if not refs: errors.append(command+': no section reference')
    rows.append({'command':command,**v,'references':refs})
print(json.dumps({'counts':dict(collections.Counter(v['owner'] for v in d.values())),'entries':rows,'errors':errors},ensure_ascii=False,indent=2))
raise SystemExit(bool(errors))
