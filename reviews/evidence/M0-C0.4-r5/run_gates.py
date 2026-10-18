"""Reproducible closure gates; each command has a log and exit/time receipt."""
import datetime
import gzip
import json
import os
import pathlib
import subprocess
import sys
import time

OUT = pathlib.Path(__file__).resolve().parent
ROOT = OUT.parents[2]
os.environ['PYTHONIOENCODING'] = 'utf-8'
RESULTS = []

def run(name, command, expected=0):
    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    begin = time.monotonic()
    print('START', name, flush=True)
    with (OUT / (name + '.log')).open('wb') as stream:
        p = subprocess.run(command, cwd=ROOT, stdout=stream, stderr=subprocess.STDOUT)
    receipt = dict(command=command, cwd=str(ROOT), started=started,
                   seconds=round(time.monotonic()-begin,3), exit=p.returncode, expected=expected)
    (OUT/(name+'.receipt.json')).write_text(json.dumps(receipt,indent=2)+'\n')
    print('END', name, p.returncode, receipt['seconds'], flush=True)
    RESULTS.append(p.returncode == expected)
    return p.returncode == expected

def pnpm(name, command):
    return run(name, ['cmd','/d','/c',command])

def main():
    stage = sys.argv[1]
    if stage == 'frontend':
        for name, cmd in [('lint','lint'),('typecheck','typecheck'),('test','test'),
                          ('format-check','format:check'),('licenses-check','licenses:check')]:
            pnpm(name, 'pnpm '+cmd)
        pnpm('frontend-build-diagnostics','pnpm --dir desktop build:diagnostics')
        pnpm('frontend-build-default','pnpm --dir desktop build')
    elif stage == 'rust':
        run('fmt',['cargo','fmt','--all','--check'])
        run('clippy',['cargo','clippy','--workspace','--all-targets','--locked','--','-D','warnings'])
        run('rust-tests',['cargo','test','--workspace','--locked'])
        run('clippy-diagnostics',['cargo','clippy','-p','tt-desktop','--all-targets','--locked','--features','m0-diagnostics','--','-D','warnings'])
        run('rust-tests-diagnostics',['cargo','test','-p','tt-desktop','--locked','--features','m0-diagnostics'])
        run('deny',['cargo','deny','--locked','check'])
        run('headless-build',['cargo','build','--release','--locked','-p','tt-headless'])
        for i in (1,2):
            run('headless-selftest-'+str(i),[str(ROOT/'target/release/toontown-headless.exe'),'--selftest','--json'])
        run('headless-guard',['python','scripts/check-headless-deps.py','target/release/toontown-headless.exe'])
        run('raster',['cargo','run','--release','--locked','-p','tt-render','--example','c03_raster','--',
                     '--repeat','3','--out','target/c04-r5-raster','--report',str(OUT/'raster.json'),
                     '--check','crates/render/examples/c03_raster/hashes.json'])
        for mode in ('default','diagnostics'):
            config = '' if mode=='default' else ' --config src-tauri/tauri.diagnostics.conf.json'
            pnpm('tauri-build-'+mode,'pnpm --dir desktop tauri build --no-bundle'+config+' -- --locked')
            run('binary-gating-'+mode,['python','scripts/check-diagnostics-gating.py','target/release/toontown.exe','--expect',mode])
            run('independent-gating-'+mode,['python',str(OUT/'probe_gating.py'),mode])
    elif stage == 'reviewers':
        for script in ['scripts/test_decision_rules.py',*['tests/adversarial/'+n for n in
                       ['c02-r1-tooling.py','c02-r2-tooling.py','c02-r3-tooling.py','c03-r1-tooling.py',
                        'c03-r2-tooling.py','c04-r2-decisions.py','c04-r3-decisions.py','c04-r3-page.py']]]:
            run(pathlib.Path(script).stem,['python','-X','utf8',script])
    elif stage == 'ci':
        for suffix,args in [('json',['--json','headSha,status,conclusion,url,jobs']),('log',['--log'])]:
            p = subprocess.run(['gh','run','view','39147607699',*args],cwd=ROOT,capture_output=True,check=True)
            (OUT/('ci-39147607699.'+suffix+('.gz' if suffix=='log' else ''))).write_bytes(
                gzip.compress(p.stdout,mtime=0) if suffix=='log' else p.stdout)
            print('Fetched CI',suffix, len(p.stdout),flush=True)
    elif stage == 'scoped':
        for name, script in [('closure-probes', OUT/'closure_probes.py'), ('page-probe', OUT/'page_probe.py'),
                             ('rebreak', OUT/'rebreak.py'), ('copy-probe', OUT/'copy_probe.py'),
                             ('c04-r5-decisions', ROOT/'tests/adversarial/c04-r5-decisions.py')]:
            run(name, ['python', '-X', 'utf8', str(script)])
    else:
        raise ValueError('unknown stage: '+stage)
    return 0 if all(RESULTS) else 1

if __name__ == '__main__':
    sys.exit(main())
