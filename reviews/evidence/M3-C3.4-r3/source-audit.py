"""Round-specific ownership and product-diff receipt; does not alter source."""
import subprocess,json,re
from pathlib import Path
def git(*a):return subprocess.check_output(['git',*a],text=True,encoding='utf-8')
paths=['crates/*/tests/c34_r[0-9]_*','tests/adversarial/c34-r*','reviews/M3-C3.4-r1.md','reviews/M3-C3.4-r2.md','reviews/evidence/M3-C3.4-r1','reviews/evidence/M3-C3.4-r2']
assert not git('diff','a2253cb..0146e1a','--',*paths)
assert not git('diff','0146e1a','--','crates/*/tests/c34_r[12]_*','tests/adversarial/c34-r[12]-*','reviews/M3-C3.4-r1.md','reviews/M3-C3.4-r2.md','reviews/evidence/M3-C3.4-r1','reviews/evidence/M3-C3.4-r2')
assert not git('diff','0146e1a','--','crates/*/src','plan','docs','Cargo.lock','**/Cargo.toml')
assert not git('diff','a2253cb..0146e1a','--','Cargo.lock','**/Cargo.toml')
diff=git('diff','a2253cb..0146e1a','--','crates/*/src')
added=[l[1:] for l in diff.splitlines() if l.startswith('+') and not l.startswith('+++')]
for l in added:
 assert not re.search(r'\b(?:unwrap|expect)\s*\(|\bpanic!|\bunsafe\s*\{|#\[allow',l),l
r={'candidate':git('rev-parse','0146e1a').strip(),'reviewer_files_byte_identical':True,'product_tree_unchanged_by_reviewer':True,'manifest_lock_unchanged':True,'added_product_forbidden_constructs':[],'added_unsafe_anywhere':[l for l in git('diff','a2253cb..0146e1a').splitlines() if l.startswith('+') and re.search(r'\bunsafe\b',l)]}
Path('reviews/evidence/M3-C3.4-r3/source-audit.json').write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps(r))
