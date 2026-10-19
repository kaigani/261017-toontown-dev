"""Run the generator against scratch outputs and compare bytes, without rewriting product files."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
spec=importlib.util.spec_from_file_location('closure_generation',ROOT/'scripts/gen-command-declarations.py')
gen=importlib.util.module_from_spec(spec)
spec.loader.exec_module(gen)
with tempfile.TemporaryDirectory(prefix='generated-',dir=HERE) as directory:
    scratch=Path(directory).resolve()
    assert scratch.is_relative_to(HERE.resolve())
    paths={'OUT_RS':ROOT/'crates/registry/src/declared.rs','OUT_JSON':ROOT/'crates/registry/src/declared.json','ISSUES':ROOT/'plan/spec-issues.md'}
    for name,original in paths.items():
        target=scratch/original.name
        target.write_bytes(original.read_bytes())
        setattr(gen,name,target)
    assert gen.main()==0
    result=[]
    for name,original in paths.items():
        actual=getattr(gen,name).read_bytes()
        expected=original.read_bytes()
        result.append({'path':str(original.relative_to(ROOT)),'equal':actual==expected,'sha256':hashlib.sha256(actual).hexdigest()})
    (HERE/'generated-freshness.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
    assert all(x['equal'] for x in result),result
print(json.dumps(result,indent=2))
