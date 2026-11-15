import importlib.util
import sys
from pathlib import Path

if len(sys.argv) < 2:
    print('skipped: no application')
    sys.exit(0)
R = Path(__file__).resolve().parents[2]
def load(name, path):
    s = importlib.util.spec_from_file_location(name, path)
    m = importlib.util.module_from_spec(s)
    s.loader.exec_module(m)
    return m
e = load('e', R/'scripts/c41-gui-e2e.py')
c = load('c', R/'scripts/mcp-conformance.py')
def launch(app, work):
    p = e.launch(app, Path(work)/'user', Path(work)/'discovery.json')
    d = e.wait_discovery(Path(work)/'discovery.json', p)
    c.HTTP.update(endpoint=d['endpoint'], token=d['token'])
    return p
c.launch_desktop = launch
sys.argv = ['mcp-conformance.py', '--http', str(Path(sys.argv[1]).resolve()), str(R/'reviews/evidence/M4-C4.2b-r1/c1-conformance.json')]
sys.exit(c.main())
