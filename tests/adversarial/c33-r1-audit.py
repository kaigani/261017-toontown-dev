"""Fixed-head source and downloaded artifact audit. Receipts go outside the repository."""
import hashlib
import json
import tempfile
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
OUT = Path(tempfile.gettempdir()) / 'toontown-c33-r1'


def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    base = OUT / 'artifacts'
    comparisons = []
    for family in ['c33', 'e2', 'e4', 'c03-raster']:
        a, b = base / (family + '-Windows'), base / (family + '-macOS')
        for f in sorted(a.rglob('*')):
            if f.is_file() and (f.suffix == '.png' or f.name in ['manifest.json', 'gate-path.json', 'evaluate.json', 'hashes.json']):
                other = b / f.relative_to(a)
                byte_equal = other.is_file() and f.read_bytes() == other.read_bytes()
                # E2's hash receipt uses host text newlines; it is not a rendered output or a manifest.
                equal = byte_equal
                if f.name == 'hashes.json' and other.is_file():
                    equal = json.loads(f.read_text()) == json.loads(other.read_text())
                comparisons.append(dict(family=family, file=f.relative_to(a).as_posix(),
                                        sha256=sha(f.read_bytes()),
                                        otherSha256=sha(other.read_bytes()) if other.is_file() else None,
                                        byteEqual=byte_equal, equal=equal))
    index = json.loads(git('show', 'e61c2651feb83c7e03174bedd10fa2cf563c4ad8:crates/executor/tests/golden/c33/index.json'))
    goldens = []
    for name, entry in index.items():
        data = git('show', 'e61c2651feb83c7e03174bedd10fa2cf563c4ad8:crates/executor/tests/golden/c33/' + name + '.png')
        goldens.append(dict(name=name, equal=sha(data) == entry['sha256'], sha256=sha(data)))
    gate_index = json.loads(git('show', 'e61c2651feb83c7e03174bedd10fa2cf563c4ad8:crates/executor/tests/golden/c33/gate-index.json'))
    for platform in ['Windows', 'macOS']:
        for name, entry in gate_index.items():
            f = base / ('c33-' + platform) / (name + '.png')
            if f.exists():
                assert sha(f.read_bytes()) == entry['sha256'], str(f)
    ownership = git('diff', '--stat', '2184ae8..e61c265', '--',
                    'crates/*/tests/c*_r[0-9]_*', 'tests/adversarial', 'desktop/src/test',
                    'reviews/evidence/M3-C3.2-r*').decode()
    m2 = git('diff', '--name-only', '2184ae8..e61c265', '--',
             'crates/executor/tests/goldens/c23_render.json', 'reviews/evidence/M2-C2.3/goldens',
             'crates/render/examples/c03_raster/hashes.json')
    jobs = json.loads((OUT / 'ci.json').read_text(encoding='utf-8-sig')) if (OUT / 'ci.json').exists() else None
    prior_e2 = []
    prior_head = '8c640d18e8b1d8562145f6aa88dc75fd6dee1b25'
    if (OUT / 'prior-ci.json').exists():
        raw = (OUT / 'prior-ci.json').read_bytes()
        prior_ci = json.loads(raw.decode('utf-16' if raw.startswith(b'\xff\xfe') else 'utf-8-sig'))
        assert prior_ci['headSha'] == prior_head
    for platform in ['Windows', 'macOS']:
        old = OUT / 'prior-artifacts' / ('e2-' + platform)
        for f in sorted(old.glob('*.png')):
            current = base / ('e2-' + platform) / f.name
            prior_e2.append(dict(platform=platform, file=f.name, oldSha256=sha(f.read_bytes()),
                                 equal=current.is_file() and f.read_bytes() == current.read_bytes()))
    result = dict(reviewedHead='e61c2651feb83c7e03174bedd10fa2cf563c4ad8',
                  runnerHead=git('rev-parse', 'HEAD').decode().strip(), artifactComparisons=comparisons,
                  goldens=goldens, ownership=ownership, m2Changes=m2.decode(), priorE2=prior_e2,
                  historicalE2Run=42762495633, historicalE2Head=prior_head,
                  ciHead=jobs['headSha'] if jobs else None, jobs=[dict(name=j['name'], conclusion=j['conclusion'],
                                                   startedAt=j['startedAt'], completedAt=j['completedAt'])
                                              for j in jobs['jobs']] if jobs else [])
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / 'audit.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8', newline='\n')
    assert all(c['equal'] for c in comparisons)
    assert all(g['equal'] for g in goldens)
    assert all(c['equal'] for c in prior_e2)
    assert not ownership and not m2
    if jobs:
        assert jobs['headSha'] == 'e61c2651feb83c7e03174bedd10fa2cf563c4ad8'
        assert len(jobs['jobs']) == 28 and all(j['conclusion'] == 'success' for j in jobs['jobs'])
    print(f"{len(comparisons)} cross-OS files, {len(goldens)} indexed goldens; CI receipt present: {bool(jobs)}")


if __name__ == '__main__':
    main()
