from pathlib import Path
import subprocess,time,json
ROOT=Path(__file__).resolve().parents[3]; HERE=Path(__file__).resolve().parent
commands=[
 ('closure-debug','cargo test --locked -p tt-vector --test c21_r4_closure -- --nocapture'),
 ('closure-release','cargo test --release --locked -p tt-vector --test c21_r4_closure -- --nocapture'),
 ('golden','cargo test --locked -p tt-executor --test c21_determinism -- --nocapture'),
 ('locks','cargo test --locked -p tt-persist --lib lock -- --nocapture'),
 ('c13-locks','cargo test --locked -p tt-persist --test c13_r4_lock -- --nocapture'),
 ('c14-locks','cargo test --locked -p tt-executor --test c14_lock_races -- --nocapture'),
 ('fmt','cargo fmt --all -- --check'),
]
out=[]
for name,cmd in commands:
 start=time.monotonic()
 with (HERE/(name+'.log')).open('wb') as log: code=subprocess.call(cmd,cwd=ROOT,shell=True,stdout=log,stderr=subprocess.STDOUT)
 out.append(dict(name=name,command=cmd,exit=code,wallSeconds=time.monotonic()-start))
 (HERE/'extra-gates.json').write_text(json.dumps(out,indent=2)+'\n')
 print(out[-1],flush=True)
