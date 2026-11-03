"""Explicit review execution phases. Never regenerate historical receipts."""
import datetime
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'reviews/evidence/M2-C2.4-r3'


def run(name, args, env=None, cwd=ROOT):
    OUT.mkdir(parents=True, exist_ok=True)
    logpath = OUT / (name + '.txt')
    if logpath.exists():
        raise RuntimeError(f'refusing to overwrite {logpath}')
    start = time.monotonic()
    print('START', name, flush=True)
    with logpath.open('w', encoding='utf-8') as log:
        log.write(json.dumps({'argv': args, 'cwd': str(cwd), 'env': env or {},
                              'startedAt': datetime.datetime.now(datetime.timezone.utc).isoformat()}) + '\n')
        log.flush()
        code = subprocess.run(args, cwd=cwd, env={**os.environ, 'PYTHONIOENCODING': 'utf-8', **(env or {})},
                              stdout=log, stderr=subprocess.STDOUT).returncode
    receipt = {'name': name, 'argv': args, 'exit': code, 'seconds': time.monotonic() - start}
    with (OUT / 'execution.jsonl').open('a', encoding='utf-8') as f:
        f.write(json.dumps(receipt) + '\n')
    print(json.dumps(receipt), flush=True)
    return code


def cargo(name, crate, targets, release=True, env=None):
    args = ['cargo', 'test', '-p', 'tt-' + crate, '--locked', '-j', '4']
    if release:
        args += ['--release']
    for t in targets:
        args += ['--test', t]
    run(name, args + ['--', '--nocapture'], env)


if __name__ == '__main__' and len(sys.argv) > 1:
    phase = sys.argv[1]
    if phase == 'per-crate':
        for c in ('executor', 'io', 'mcp'):
            run('crate-' + c, ['cargo', 'test', '-p', 'tt-' + c, '--locked', '-j', '4'])
    elif phase == 'properties':
        cargo('domains-4096', 'executor', ['c24_r3_domains'], env={'TT_ROUTE_CASES': '4096'})
        cargo('routes-4096', 'executor', ['c24_r3_routes'], env={'TT_ROUTE_CASES': '4096'})
        cargo('topology-4096', 'executor', ['c24_r3_topology'])
        cargo('vector-4096', 'vector', ['c24_r3_properties'])
        cargo('noding-variants', 'vector', ['c24_r2_noding', 'c24_r3_noding'])
        cargo('cutter-regression', 'executor', ['c24_cutter_snapping'])
        cargo('decode-30000', 'io', ['c24_r3_decoder'])
        run('headless-release-build', ['cargo', 'build', '--release', '-p', 'tt-headless', '--locked', '-j', '4'])
        run('e2-local-build', [sys.executable, 'scripts/e2-paint-by-id.py',
                             'target/release/toontown-headless.exe', str(OUT / 'e2-local-build')])
    elif phase == 'workspace':
        run('workspace-final', ['cargo', 'test', '--workspace', '--locked', '-j', '4', '--no-fail-fast'])
