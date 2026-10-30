"""Archive the interrupted run's CI downloads and huge repetitive graph, preserving every byte."""
from pathlib import Path
import gzip
import hashlib
import json
import shutil
import zipfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
manifest = {}
with zipfile.ZipFile(HERE / 'ci-artifacts.zip', 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as z:
    for p in sorted((HERE / 'ci').rglob('*')):
        if p.is_file():
            rel = p.relative_to(HERE / 'ci').as_posix()
            z.write(p, rel)
            manifest[rel] = hashlib.sha256(p.read_bytes()).hexdigest()
with zipfile.ZipFile(HERE / 'ci-artifacts.zip') as z:
    assert z.testzip() is None
    assert all(hashlib.sha256(z.read(p)).hexdigest() == h for p, h in manifest.items())
p = HERE / 'dev-feature-tree.log'
with p.open('rb') as src, gzip.open(HERE / 'dev-feature-tree.log.gz', 'wb', compresslevel=9) as dst:
    shutil.copyfileobj(src, dst)
assert hashlib.sha256(p.read_bytes()).digest() == hashlib.sha256(gzip.decompress((HERE / 'dev-feature-tree.log.gz').read_bytes())).digest()
observations = HERE / 'reviewer-observations'
observations.mkdir(exist_ok=True)
for p in (ROOT / 'target/evidence/c14').glob('*.json'):
    shutil.copyfile(p, observations / p.name)
(HERE / 'ci-artifact-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
print('CI archived bytes:', (HERE / 'ci-artifacts.zip').stat().st_size, 'files:', len(manifest))
print('Dev graph compressed bytes:', (HERE / 'dev-feature-tree.log.gz').stat().st_size)
print('Archives fully byte-verified; source downloads are safe to remove explicitly.')
