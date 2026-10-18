"""Reproducible reviewer commands; records exact commands, clocks, output and exits."""
import datetime
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
os.chdir(ROOT)

def run(name, args, timeout=1200, env=None):
    start = datetime.datetime.now(datetime.timezone.utc).isoformat()
    t = time.perf_counter()
    with (OUT / (name + '.txt')).open('w', encoding='utf-8') as f:
        p = subprocess.run(args, stdout=f, stderr=subprocess.STDOUT, timeout=timeout, env=env)
    receipt = dict(command=args, startedUtc=start, elapsedSeconds=time.perf_counter()-t, exitCode=p.returncode)
    (OUT / (name + '.receipt.json')).write_text(json.dumps(receipt, indent=2)+'\n')
    print(name, p.returncode, round(receipt['elapsedSeconds'], 2), flush=True)
    return p.returncode

if __name__ == '__main__':
    group = sys.argv[1]
    if group == 'gates':
        for name, command in [
            ('frontend', ['cmd', '/d', '/c', 'pnpm test']),
            ('lint', ['cmd', '/d', '/c', 'pnpm lint']),
            ('typecheck', ['cmd', '/d', '/c', 'pnpm typecheck']),
            ('licenses', ['cmd', '/d', '/c', 'pnpm licenses:check']),
            ('rust-tests', ['cargo', 'test', '--workspace', '--locked']),
            ('deny', ['cargo', 'deny', '--locked', 'check']),
            ('headless-build', ['cargo', 'build', '--release', '--locked', '-p', 'tt-headless']),
            ('headless-guard', ['python', 'scripts/check-headless-deps.py', 'target/release/toontown-headless.exe']),
            ('headless-tree', ['cargo', 'tree', '--locked', '-p', 'tt-headless']),
        ]:
            run(name, command)
    elif group == 'raster':
        base = ['cargo', 'run', '--locked', '-p', 'tt-render', '--example', 'c03_raster']
        for name, release, repeats in [('raster-release-1', True, 50), ('raster-release-2', True, 3), ('raster-debug', False, 3)]:
            args = base + (['--release'] if release else []) + ['--', '--repeat', str(repeats), '--check', 'crates/render/examples/c03_raster/hashes.json', '--report', str(OUT/(name+'.json')), '--out', str(ROOT/'target'/name)]
            run(name, args)
    elif group == 'canvas':
        for backend in ['auto', 'webgl2']:
            run('canvas-'+backend, [str(ROOT/'target/release/toontown.exe'), '--spike', 'canvas', '--spike-log', str(OUT/('canvas-'+backend+'.json')), '--spike-auto', '--spike-renderer', backend], timeout=360)
