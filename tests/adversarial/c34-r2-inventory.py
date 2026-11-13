from pathlib import Path
import re,json,subprocess
root=Path(__file__).resolve().parents[2]
e=root/'reviews/evidence/M3-C3.4-r2'
s=(root/'plan/spec-issues.md').read_text(encoding='utf-8')
n=(root/'reviews/M3-merge-note.md').read_text(encoding='utf-8')
section=n.split('## 4.')[1].split('## 5.')[0]
headings=re.findall(r'^## (SI-\d+).*$',s,re.M)
selected=[line for line in s.splitlines() if line.startswith('## SI-') and re.search(r'open|unresolved|awaiting|interim|pending confirmation',line,re.I)]
assert all(re.search(r'SI-\d+',line)[0] in section for line in selected)
assert headings[0]=="SI-001" and headings[-1]=="SI-110"
changes=subprocess.check_output(['git','diff','1cbd190..967229a','--','tests/adversarial','crates/*/tests/c34_r1_*'],text=True)
assert not changes
product=subprocess.check_output(['git','diff','--unified=0','1cbd190..967229a','--','crates/executor/src','crates/mcp/src'],text=True)
added=[l for l in product.splitlines() if l.startswith('+') and not l.startswith('+++')]
assert not any(re.search(r'\b(unwrap|expect)\s*\(|\bpanic!|\bunsafe\b|#\[allow',l) for l in added)
assert not subprocess.check_output(['git','diff','1cbd190..967229a','--','Cargo.lock','**/Cargo.toml'],text=True)
(e/'inventory.json').write_text(json.dumps({'headings':len(headings),'active_headings':selected,'unchanged_reviewer_files':True,'product_gate':True},indent=2)+'\n')
print('Register headings audited; active heading inventory, reviewer ownership and product gates pass')

