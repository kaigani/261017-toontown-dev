from pathlib import Path
import json
p=Path('reviews/evidence/M3-C3.3-r1/register.json'); data=json.loads(p.read_text()); rows=data['rows']
rows.append(dict(id='SPEC-WIDTH-DEFAULT',requirement='disposition of conflicting generic render width defaults',status='missing',clause='09 11.1 item 1 versus 11 7.5 table',evidence='default-width.json; default-width.py raw SDK receipt',note='Tool and owning command both return 1920 on the fresh default project, with identical PNGs. This honors explicit default parity but conflicts with the generic table default 960. No precedence ruling is made.'))
data['counts']={s:sum(row['status']==s for row in rows) for s in ['implemented','divergent','missing','deferred-with-owner','pending']}
p.write_text(json.dumps(data,indent=2)+'\n',encoding='utf-8',newline='\n')
p.with_suffix('.md').write_text('# C3.3 round-one requirement register\n\n'+json.dumps(data['counts'])+'\n\nImplemented entries use the named source review and executed suites; missing entries record acceptance evidence or specification disposition not established.\n\n| ID | Requirement | Disposition | Evidence |\n|---|---|---|---|\n'+'\n'.join(f"| {row['id']} | {row['requirement']} | {row['status']} {row.get('owner','')} | {row['evidence']} |" for row in rows)+'\n',encoding='utf-8',newline='\n')
print(data['counts'])
