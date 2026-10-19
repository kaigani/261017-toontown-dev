"""Insert the complete requirement closure table and verify report/test totals."""
import json,re
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
report=ROOT/'reviews/M1-C1.2-r2.md'
rows=json.loads((HERE/'requirements.json').read_text(encoding='utf-8'))
lines=['| Row / requirement | R1 → R2 | Current file:line | Test/evidence; new findings |','|---|---|---|---|']
for r in rows:
    lines.append(f"| {r['id']} {r['requirement']} | {r['priorStatus']} → **{r['status']}** | `{r['implementation']}` | {r['tests']}; {', '.join(r['findings']) or 'no additional defect in this row'} |")
text=report.read_text(encoding='utf-8')
table='<!-- REQUIREMENT_TABLE_BEGIN -->\n\n'+'\n'.join(lines)+'\n\n<!-- REQUIREMENT_TABLE_END -->'
if '<!-- REQUIREMENT_TABLE -->' in text:text=text.replace('<!-- REQUIREMENT_TABLE -->',table)
else:text=re.sub(r'<!-- REQUIREMENT_TABLE_BEGIN -->.*?<!-- REQUIREMENT_TABLE_END -->',lambda _:table,text,flags=re.S)
text=text.replace('\n<!-- REQUIREMENT_TABLE_APPENDIX -->\n','\n')
assert len(re.findall(r'^### C12-R2-\d+ — P1',text,re.M))==13
assert len(re.findall(r'^### C12-R2-\d+ — P2',text,re.M))==1
audit=json.loads((HERE/'ci-audit.json').read_text(encoding='utf-8'))
final=audit['final-complete']['rustSuites']
totals={k:sum(r[k] for r in final) for k in ['passed','failed','ignored']}
assert totals=={'passed':430,'failed':29,'ignored':1},totals
for os,want in [('windows-latest',411),('macos-latest',405)]:
    assert sum(x['passed'] for x in audit['39217799686'][f'check ({os})']['workspaceSuites'])==want
text=text.replace('review tree fails only the three R2 targets','review tree 430 pass / 29 fail / 1 standing ignore, only the three R2 targets fail')
text=text.replace('the final targeted assertion check covers the scheduling-order-neutral request-ID assertion.',
    'the final targeted run covers the scheduling-order-neutral request-ID assertion plus additional key/template/motion-point/property paste and cut-special undo/redo round trips; its totals remain 18 pass / 22 fail for the 40-test contract target.')
report.write_text(text,encoding='utf-8',newline='\n')
counts={'fullWorkspace':totals,'newReview':{'passed':19,'failed':29,'tests':48},'originalR1':{'passed':44,'failed':0},'originalC11':{'suites':11,'passed':114},'findings':{'P0':0,'P1':13,'P2':1}}
(HERE/'final-test-counts.json').write_text(json.dumps(counts,indent=2)+'\n',encoding='utf-8')
print(json.dumps(counts,indent=2))
