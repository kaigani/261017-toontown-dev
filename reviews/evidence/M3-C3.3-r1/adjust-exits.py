from pathlib import Path
import tempfile
p=Path(tempfile.gettempdir())/'toontown-c33-r1/package-evidence.py'; s=p.read_text()
s=s.replace('startedAt=datetime.datetime.fromtimestamp(path.stat().st_ctime','fileCreatedAt=datetime.datetime.fromtimestamp(path.stat().st_ctime').replace('lastWrite=datetime.datetime.fromtimestamp(path.stat().st_mtime','fileLastWrite=datetime.datetime.fromtimestamp(path.stat().st_mtime')
needle="exits={p.name:read(p).strip() for p in SRC.glob('*.exit')}"
insert="\nfor name in ['render.exit','clippy.exit','release.exit','gate.exit','deny.exit','e2.exit','e4.exit','e4-variant.exit','sdk.exit','release-goldens.exit','review-release-pixels.exit','fmt.exit','review-clippy.exit','python-final.exit','allows-final.exit','declarations-final.exit','post-test-tracked-diff.exit','artifact-download.exit','default-width.exit']:\n assert exits.get(name)=='0',(name,exits.get(name))\nfor name in ['executor.exit','mcp.exit','workspace.exit']: assert exits.get(name)=='101',(name,exits.get(name))"
s=s.replace(needle,needle+insert)
p.write_text(s)
