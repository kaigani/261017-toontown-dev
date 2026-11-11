import importlib.util,json
from pathlib import Path
r=Path.cwd()
def load(n,p):
 s=importlib.util.spec_from_file_location(n,p);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
ours=load('independent',r/'tests/adversarial/c34-r1-numeric.py');builder=load('builder',r/'scripts/e1-bouncing-ball.py')
error=max(abs(ours.position(f)-builder.expected_y(f)) for f in range(1,31));assert error<1e-9
print('Independent vs script maximum field error:',error)
