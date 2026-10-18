import importlib.util
import pathlib
import sys

OUT=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('gates', OUT/'run_gates.py')
g=importlib.util.module_from_spec(spec)
spec.loader.exec_module(g)
cases=[('r2-amended',['python','-X','utf8','tests/adversarial/c04-r2-decisions.py'],0),
       ('builder-decisions',['python','-X','utf8','scripts/test_decision_rules.py'],0),
       ('r3-decisions',['python','-X','utf8','tests/adversarial/c04-r3-decisions.py'],1),
       ('r3-page',['python','-X','utf8','tests/adversarial/c04-r3-page.py'],1)]
for name,args,expected in cases:
    assert g.run(name,args)==expected,name
for name in ('c02-r1-tooling','c02-r2-tooling','c02-r3-tooling','c03-r1-tooling','c03-r2-tooling'):
    assert g.run(name,['python','-X','utf8','tests/adversarial/'+name+'.py'])==0,name
