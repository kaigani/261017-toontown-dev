"""Run prior reviewer probes unchanged except for their receipt destinations."""
import importlib.machinery
from pathlib import Path
import sys

ROOT=Path(__file__).resolve().parents[2]
OLD='reviews/evidence/M4-C4.2b'
NEW='reviews/evidence/M4-C4.3b-r1/case7/legacy'
for suffix in ('-r1','-r2','-r3','-r4/live'):
    (ROOT/(NEW+suffix)).mkdir(parents=True,exist_ok=True)
read_text=Path.read_text
def redirected(path,*args,**kwargs):
    text=read_text(path,*args,**kwargs)
    return text.replace(OLD,NEW) if path.suffix=='.py' and path.name.startswith('c42b-') else text
Path.read_text=redirected
get_code=importlib.machinery.SourceFileLoader.get_code
def code(loader,fullname):
    path=Path(loader.path)
    if path.suffix=='.py' and path.name.startswith('c42b-'):
        return compile(path.read_text(encoding='utf-8'),str(path),'exec')
    return get_code(loader,fullname)
importlib.machinery.SourceFileLoader.get_code=code
path=Path(sys.argv[1]);sys.argv=sys.argv[1:]
exec(compile(path.read_text(encoding='utf-8'),str(path),'exec'),{'__file__':str(path),'__name__':'__main__'})
