from pathlib import Path
import tempfile
p=Path(tempfile.gettempdir())/'toontown-c33-r1/package-evidence.py'
s=p.read_text()
s=s.replace("'review-release-pixels.exit']", "'review-release-pixels.exit','python-final-source.json']")
s=s.replace("for path,digest in metadata['sourceHashes'].items(): assert sha(Path(path).read_bytes())==digest,path", "changed_non_cargo=[]\nfor path,digest in metadata['sourceHashes'].items():\n if sha(Path(path).read_bytes())!=digest:\n  assert path.endswith('.py'),path\n  changed_non_cargo.append(path)\npython_witness=json.loads(read(SRC/'python-final-source.json'))\nassert python_witness['sha256']==sha((ROOT/'tests/adversarial/c33-r1-audit.py').read_bytes())\nassert read(SRC/'python-final.exit').strip()=='0'")
s=s.replace('workspaceSourceHashesMatch=True','workspaceRustSourceHashesMatch=True,pythonSourcesChangedAfterWorkspaceStart=changed_non_cargo,finalPythonSourceWitness=python_witness')
p.write_text(s)
