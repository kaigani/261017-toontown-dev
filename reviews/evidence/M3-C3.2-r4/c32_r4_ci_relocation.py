"""Verify every relocated case in both OS logs; run from the repository root."""
from pathlib import Path
import json,re,subprocess,gzip
out=Path('reviews/evidence/M3-C3.2-r4')
p=out/'ci-42762495633.log'; raw=p.read_bytes() if p.exists() else gzip.decompress(Path(str(p)+'.gz').read_bytes()); lines=raw.decode('utf-16').splitlines()
expected=[str(p.relative_to('crates/executor')).replace('\\','/') for folder in ['ui','ui-pass','c12-r1-ui'] for p in Path('crates/executor/tests',folder).glob('*.rs')]
checks={}
for os in ['windows-latest','macos-latest']:
 selected=[l.split('\t',2)[2] for l in lines if l.startswith(f'history oracles release ({os})\tCompile-time proofs (trybuild, release)\t')]
 text=re.sub(r'\x1b\[[0-9;]*m|\^\[\[[0-9;]*m','', '\n'.join(selected)).replace('\\','/')
 found=[p for p in expected if re.search(r'test '+re.escape(p)+r'(?: \[.*?\])? \.\.\. ok',text)]
 named=['duplicate_ids_and_inventory_kind_drift_are_rejected','missing_example_is_rejected_at_compile_time','exclusive_mutation_capability']
 checks[os]={'expected_cases':expected,'matched_cases':found,'all_cases_matched':set(found)==set(expected),'named_tests':{n:bool(re.search(r'test '+n+r' \.\.\. ok',text)) for n in named},'result_summaries':re.findall(r'test result: .*',text)}
 assert checks[os]['all_cases_matched'] and all(checks[os]['named_tests'].values())
checks['bodies_unchanged']=not subprocess.check_output(['git','diff','fd14a8e..8c640d1','--','crates/executor/tests/compile_fail*','crates/executor/tests/c12_r1_macro*'])
assert checks['bodies_unchanged']
(out/'ci-relocation-verification.json').write_text(json.dumps(checks,indent=2),encoding='utf-8')
print('Verified',len(expected),'individual trybuild cases and all 3 named tests on each OS')
