"""Retain independently executed closure gates and elapsed wall times."""
from pathlib import Path
import subprocess, time, json
ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
commands = [
 ('workspace', 'cargo test --workspace --locked --no-fail-fast'),
 ('frontend', 'pnpm test'),
 ('release-review', 'cargo test --release --locked -p tt-vector --test c21_r3_geometry --test c21_r2_geometry -- --nocapture'),
 ('release-contracts', 'cargo test --release --locked -p tt-executor --test c21_r3_contracts -- --nocapture'),
 ('brush-debug', 'cargo test --locked -p tt-vector --test c21_worst_case adversarial_brush -- --ignored --nocapture'),
 ('brush-release', 'cargo test --release --locked -p tt-vector --test c21_worst_case adversarial_brush -- --ignored --nocapture'),
 ('allows', 'python scripts/check-product-allows.py'),
 ('generation', 'python scripts/gen-command-declarations.py'),
 ('python', 'python scripts/run-reviewer-python-suites.py'),
]
results=[]
for name, command in commands:
    start=time.monotonic()
    with (HERE/(name+'.log')).open('wb') as log:
        code=subprocess.call(command,cwd=ROOT,shell=True,stdout=log,stderr=subprocess.STDOUT)
    results.append(dict(name=name,command=command,exit=code,wallSeconds=time.monotonic()-start))
    (HERE/'gates.json').write_text(json.dumps(results,indent=2)+'\n')
    print(results[-1],flush=True)
    if name.startswith('brush-'):
        import shutil
        p=ROOT/'target/evidence/c21-fit/brush-worst-case-windows.json'
        if p.exists(): shutil.copyfile(p,HERE/(name+'.json'))
