"""Scoped closure receipts. Run a named group; no product/source changes."""
from pathlib import Path
import datetime
import hashlib
import json
import os
import secrets
import subprocess
import sys
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
BASE = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
FAILED = []
for folder in ['logs', 'receipts']:
    (HERE / folder).mkdir(exist_ok=True)

def run(name, command, env=None):
    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    tick = time.monotonic()
    process_env = dict(os.environ, CARGO_TERM_COLOR='never', PYTHONIOENCODING='utf-8')
    process_env.update(env or {})
    print(f'START {name}: {command}', flush=True)
    with (HERE / 'logs' / (name + '.log')).open('wb') as log:
        result = subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, env=process_env)
    data = (HERE / 'logs' / (name + '.log')).read_bytes()
    receipt = dict(command=command, cwd=str(ROOT), base=BASE, started_utc=started,
                   elapsed_seconds=round(time.monotonic()-tick, 3), exit_code=result.returncode,
                   log='logs/' + name + '.log', sha256=hashlib.sha256(data).hexdigest(), env=env or {})
    (HERE / 'receipts' / (name + '.json')).write_text(json.dumps(receipt, indent=2)+'\n', encoding='utf-8')
    print(f'END {name}: exit={result.returncode}, seconds={receipt["elapsed_seconds"]}', flush=True)
    if result.returncode:
        FAILED.append(name)
        print(data.decode('utf-8', errors='replace')[-4000:], flush=True)
    return result.returncode

def main(group):
    if group == 'frontend':
        run('install', ['cmd', '/c', 'pnpm', 'install', '--frozen-lockfile'])
        for gate in ['lint', 'typecheck', 'test', 'format:check', 'licenses:check']:
            run('pnpm-' + gate.replace(':', '-'), ['cmd', '/c', 'pnpm', gate])
        for gate in ['build:diagnostics', 'build']:
            run('frontend-' + gate.replace(':', '-'), ['cmd', '/c', 'pnpm', '--dir', 'desktop', gate])
        run('review-policy', ['python', 'tests/adversarial/c11-r2-review-policy.py'])
        run('reviewer-python', ['python', 'scripts/run-reviewer-python-suites.py'])
        for case in ['licensing', 'allowlist', 'imports']:
            run('node-guards-' + case, ['node', 'tests/adversarial/guards.mjs', case])
    elif group == 'node-guards':
        for case in ['licensing', 'allowlist', 'imports']:
            run('node-guards-' + case, ['node', 'tests/adversarial/guards.mjs', case])
    elif group == 'rust':
        run('r4-regressions', ['cargo', 'test', '-p', 'tt-model', '--test', 'c11_r4_exposure_bounds', '--locked', '--', '--nocapture'])
        run('fmt', ['cargo', 'fmt', '--all', '--check'])
        run('clippy', ['cargo', 'clippy', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings'])
        run('workspace', ['cargo', 'test', '--workspace', '--locked'])
        run('clippy-diagnostics', ['cargo', 'clippy', '-p', 'tt-desktop', '--all-targets', '--locked', '--features', 'm0-diagnostics', '--', '-D', 'warnings'])
        run('test-diagnostics', ['cargo', 'test', '-p', 'tt-desktop', '--locked', '--features', 'm0-diagnostics'])
        run('deny', ['cargo', 'deny', '--locked', 'check'])
        seed = str(secrets.randbits(48))
        (HERE / 'fresh-seed.txt').write_text(seed+'\n', encoding='utf-8')
        for name, s in [('recorded', '202610180311'), ('fresh', seed)]:
            run('random-' + name, ['cargo', 'test', '-p', 'tt-model', '--test', 'c11_r3_random', '--locked', '--', '--nocapture'], {'PROPTEST_RNG_SEED': s})
        run('rustdoc', ['cargo', 'doc', '-p', 'tt-model', '--no-deps', '--locked'])
    elif group == 'release':
        run('tauri-release', ['cmd', '/c', 'pnpm', '--dir', 'desktop', 'tauri', 'build', '--no-bundle', '--', '--locked'])
        run('diagnostics-gate', ['python', 'scripts/check-diagnostics-gating.py', 'target/release/toontown.exe', '--expect', 'default'])
        run('headless-build', ['cargo', 'build', '--release', '--locked', '-p', 'tt-headless'])
        main('post-release')
    elif group == 'post-release':
        for n in [1, 2]:
            run('selftest-' + str(n), [str(ROOT / 'target/release/toontown-headless.exe'), '--selftest', '--json'])
        run('headless-guard', ['python', 'scripts/check-headless-deps.py', 'target/release/toontown-headless.exe'])
        run('raster', ['cargo', 'run', '--release', '--locked', '-p', 'tt-render', '--example', 'c03_raster', '--', '--out', 'target/review-m1-c11-r5-raster', '--report', 'reviews/evidence/M1-C1.1-r5/raster.json', '--check', 'crates/render/examples/c03_raster/hashes.json'])
    elif group == 'ci':
        run('ci-metadata', ['gh', 'run', 'view', '39180390724', '--json', 'headSha,status,conclusion,url,jobs'])
        run('ci-full', ['gh', 'run', 'view', '39180390724', '--log'])
        run('issue-state', ['gh', 'issue', 'list', '--state', 'all', '--json', 'number,title,state,url', '--limit', '100'])
        run('reviewed-history', ['git', 'log', '--oneline', 'ccdb9de..9c3f586'])
        run('reviewed-diff', ['git', 'diff', 'ccdb9de..9c3f586'])
        run('test-edit-diff', ['git', 'diff', 'ccdb9de..9c3f586', '--', 'crates/*/tests', 'desktop/src/ipc/c11-r*', 'desktop/src/test', 'tests/'])
    elif group == 'final':
        run('final-fmt', ['cargo', 'fmt', '--all', '--check'])
        run('final-clippy', ['cargo', 'clippy', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings'])
        run('new-regressions', ['cargo', 'test', '-p', 'tt-model', '--test', 'c11_r5_write_bounds', '--locked', '--', '--nocapture'])
        run('final-workspace', ['cargo', 'test', '--workspace', '--locked', '--no-fail-fast'])
    else:
        raise SystemExit('Unknown group: ' + group)

if __name__ == '__main__':
    main(sys.argv[1])
    sys.exit(1 if FAILED else 0)
