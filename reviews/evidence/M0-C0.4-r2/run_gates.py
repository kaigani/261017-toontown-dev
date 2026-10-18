"""Reproduce C0.4 review gates; run from the repository root."""
import datetime
import json
import pathlib
import subprocess
import sys
import time

OUT = pathlib.Path(__file__).resolve().parent
ROOT = OUT.parents[2]

def run(name, argv, timeout=1800):
    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    before = time.monotonic()
    with (OUT / (name + '.log')).open('wb') as log:
        try:
            result = subprocess.run(argv, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, timeout=timeout)
            code = result.returncode
        except subprocess.TimeoutExpired:
            code = 'timeout'
    receipt = dict(command=argv, cwd=str(ROOT), started=started, seconds=round(time.monotonic()-before, 3), exit=code)
    (OUT / (name + '.receipt.json')).write_text(json.dumps(receipt, indent=2)+'\n', encoding='utf-8')
    print(name, code, flush=True)
    return code

if __name__ == '__main__':
    suite = sys.argv[1]
    if suite == 'rust':
        for name, args in [
            ('fmt', ['fmt','--all','--check']),
            ('clippy', ['clippy','--workspace','--all-targets','--locked','--','-D','warnings']),
            ('rust-tests', ['test','--workspace','--locked']),
            ('clippy-diagnostics', ['clippy','-p','tt-desktop','--all-targets','--locked','--features','m0-diagnostics','--','-D','warnings']),
            ('rust-tests-diagnostics', ['test','-p','tt-desktop','--locked','--features','m0-diagnostics']),
            ('deny', ['deny','--locked','check']),
            ('headless-build', ['build','--release','--locked','-p','tt-headless']),
            ('raster', ['run','--release','--locked','-p','tt-render','--example','c03_raster','--','--repeat','3','--out','target/c04-review-raster','--report',str(OUT/'raster.json'),'--check','crates/render/examples/c03_raster/hashes.json']),
            ('features-headless', ['tree','--locked','-e','features','-p','tt-headless']),
            ('features-desktop', ['tree','--locked','-e','features','-p','tt-desktop','--features','m0-diagnostics']),
        ]:
            run(name, ['cargo'] + args)
    elif suite == 'frontend':
        for name in ['lint','typecheck','test','format:check','licenses:check']:
            run(name.replace(':','-'), ['cmd','/d','/c','pnpm '+name])
    elif suite in ('default','diagnostics'):
        suffix = '' if suite == 'default' else ':diagnostics'
        run('frontend-build-'+suite, ['cmd','/d','/c','pnpm --dir desktop build'+suffix])
        flags = '' if suite == 'default' else ' --config src-tauri/tauri.diagnostics.conf.json'
        run('tauri-build-'+suite, ['cmd','/d','/c','pnpm --dir desktop tauri build --no-bundle'+flags+' -- --locked'])
        run('binary-gating-'+suite, ['python','scripts/check-diagnostics-gating.py','target/release/toontown.exe','--expect',suite])
    if suite in ('rust', 'headless-runtime'):
        for i in (1, 2):
            run(f'headless-selftest-{i}', [str(ROOT/'target/release/toontown-headless.exe'),'--selftest','--json'])
        run('headless-guard', ['python','scripts/check-headless-deps.py','target/release/toontown-headless.exe'])
