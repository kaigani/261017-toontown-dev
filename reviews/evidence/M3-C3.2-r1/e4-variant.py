"""Reviewer variant of E4: changed pose plus a second current camera; preserves the independent builder oracle."""
import importlib.util,sys
from pathlib import Path
root=Path(__file__).resolve().parents[3]
spec=importlib.util.spec_from_file_location('e4',root/'scripts/e4-cutout-numeric.py')
m=importlib.util.module_from_spec(spec)
source=Path(spec.origin).read_text(encoding='utf-8')
needle='            request = {"nodeIds": [ids[n] for n in NAMES], "frames": KEY_FRAMES + BETWEEN}'
assert source.count(needle)==1
source=source.replace(needle,'            await cmd("anim.camera.create", {"name":"Reviewer second", "fov":90,"position":{"x":2,"y":-3,"z":20},"makeCurrent":True})\n'+needle)
exec(compile(source,spec.origin,'exec'),m.__dict__)
m.POSES['Master']['rotation'][12]=45
m.POSES['Head']['skew'][12]=15
sys.exit(m.main())
