"""Audit downloaded gh metadata and full logs; retain compressed originals."""
import gzip
import json
import pathlib
import re

OUT = pathlib.Path(__file__).resolve().parent

def read(path):
    data = path.read_bytes()
    return data.decode('utf-16' if data.startswith(b'\xff\xfe') else 'utf-8-sig')

audit = {}
for run in ('39134580275', '39133650053'):
    meta_path = OUT / f'ci-{run}.json'
    meta = json.loads(read(meta_path))
    meta_path.write_text(json.dumps(meta, indent=2)+'\n', encoding='utf-8')
    log_path = OUT / f'ci-{run}.log'
    log = read(log_path) if log_path.exists() else gzip.decompress((OUT/f'ci-{run}.log.gz').read_bytes()).decode('utf-8')
    (OUT/f'ci-{run}.log.gz').write_bytes(gzip.compress(log.encode('utf-8'), mtime=0))
    rows = {}
    for job in meta['jobs']:
        name = job['name']
        lines = [l for l in log.splitlines() if l.startswith(name+'\t')]
        joined = '\n'.join(lines)
        rows[name] = {
            'conclusion': job['conclusion'],
            'steps': {s['name']: s['conclusion'] for s in job['steps']},
            'test_summaries': [l for l in lines if re.search(r'test result:|Test Files|Tests\s+\x1b|# pass |# fail ', l)],
            'frontend_test_files': sorted(set(re.findall(r'src/[^\s\x1b]+\.test\.[jt]sx?', joined))),
            'gating_and_hash_results': [l for l in lines if re.search(r'diagnostics gating:|diagnostic [0-9]+/|M0 diagnostic and this build has none|IPC boundary \(dist check\):|hash check|234|mismatches', l)],
        }
    audit[run] = {'headSha':meta['headSha'], 'conclusion':meta['conclusion'], 'jobs':rows}
    # Only remove the known, successfully compressed log in this evidence directory.
    assert gzip.decompress((OUT/f'ci-{run}.log.gz').read_bytes()).decode('utf-8') == log
    if log_path.exists():
        log_path.unlink()
(OUT/'ci-audit.json').write_text(json.dumps(audit, indent=2)+'\n', encoding='utf-8')
for run, result in audit.items():
    print(run, result['headSha'], result['conclusion'])
    for name, job in result['jobs'].items():
        print(name, job['conclusion'], 'frontend files',len(job['frontend_test_files']))
