"""Round-2 execution recorder; explicit phase avoids overlapping Cargo builds."""
import datetime
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'reviews/evidence/M2-C2.4-r2'
OUT.mkdir(parents=True, exist_ok=True)


def run(name, args, env=None, cwd=ROOT):
    name = name.replace(':', '-')
    start = time.monotonic()
    print(f'START {name}', flush=True)
    with (OUT / (name + '.txt')).open('w', encoding='utf-8') as log:
        log.write(json.dumps({'argv': args, 'cwd': str(cwd), 'env': env or {},
                              'startedAt': datetime.datetime.now(datetime.timezone.utc).isoformat()}) + '\n')
        log.flush()
        code = subprocess.run(args, cwd=cwd, env={**os.environ, **(env or {})}, stdout=log,
                              stderr=subprocess.STDOUT).returncode
    receipt = {'name': name, 'argv': args, 'exit': code, 'seconds': time.monotonic() - start}
    with (OUT / 'execution.jsonl').open('a', encoding='utf-8') as f:
        f.write(json.dumps(receipt) + '\n')
    print(json.dumps(receipt), flush=True)
    return code


if __name__ == '__main__' and len(sys.argv) > 1:
    if sys.argv[1] == 'per-crate':
        for crate in ('vector', 'executor', 'io', 'mcp'):
            run('crate-' + crate, ['cargo', 'test', '-p', 'tt-' + crate, '--locked', '-j', '4'])
    elif sys.argv[1] == 'properties':
        run('r2-variants', ['cargo', 'test', '--release', '-p', 'tt-executor', '--locked', '-j', '4',
                            '--test', 'c24_r2_variants', '--', '--nocapture'])
        run('routes-4096', ['cargo', 'test', '--release', '-p', 'tt-executor', '--locked', '-j', '4',
                           '--test', 'c24_r2_routes', '--', '--nocapture'], {'TT_ROUTE_CASES': '4096'})
        run('vector-properties-4096', ['cargo', 'test', '--release', '-p', 'tt-vector', '--locked', '-j', '4',
                                      '--test', 'c24_r2_properties', '--', '--nocapture'])
        run('executor-properties-4096', ['cargo', 'test', '--release', '-p', 'tt-executor', '--locked', '-j', '4',
                                        '--test', 'c24_r2_topology', '--', '--nocapture'])
        run('geometry-histories-10k', ['cargo', 'test', '--release', '-p', 'tt-mcp', '--locked', '-j', '4',
                                      '--test', 'c21_history', '--', '--ignored', '--nocapture'])
        run('headless-release-build', ['cargo', 'build', '--release', '-p', 'tt-headless', '--locked', '-j', '4'])
        run('e2-local-build', [sys.executable, 'scripts/e2-paint-by-id.py', 'target/release/toontown-headless.exe',
                             str(OUT / 'e2-local-build')], {'PYTHONIOENCODING': 'utf-8'})
    elif sys.argv[1] == 'closure-probes':
        run('r2-variants-corrected', ['cargo', 'test', '--release', '-p', 'tt-executor', '--locked', '-j', '4',
                                      '--test', 'c24_r2_variants', '--', '--nocapture'])
        run('closed-fold-noding', ['cargo', 'test', '--release', '-p', 'tt-vector', '--locked', '-j', '4',
                                   '--test', 'c24_r2_noding', '--', '--nocapture'])
        run('collapsed-snap-rust-control', ['cargo', 'test', '--release', '-p', 'tt-executor', '--locked', '-j', '4',
                                           '--test', 'c24_r2_routes', 'collapsed_other_contour', '--', '--nocapture'])
    elif sys.argv[1] == 'final-checks':
        run('fmt-final', ['cargo', 'fmt', '--all', '--check'])
        run('clippy-final', ['cargo', 'clippy', '--workspace', '--all-targets', '--locked', '-j', '4', '--', '-D', 'warnings'])
    elif sys.argv[1] == 'workspace':
        run('workspace-final', ['cargo', 'test', '--workspace', '--locked', '-j', '4', '--no-fail-fast'])
