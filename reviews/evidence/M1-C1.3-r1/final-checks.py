"""Record final checks with exact native exit status and raw, unmodified subprocess output."""
import datetime
import hashlib
import json
import os
import pathlib
import subprocess
import time

OUT=pathlib.Path(__file__).resolve().parent
ROOT=OUT.parents[2]
logs=OUT/"final-checks"; logs.mkdir(exist_ok=True)
env=dict(os.environ,CARGO_PROFILE_DEV_DEBUG="0",CARGO_PROFILE_TEST_DEBUG="0",CARGO_INCREMENTAL="0",CARGO_BUILD_JOBS="4",TT_C13_R1_ARTIFACT_DIR=str(OUT))
commands={
    "fmt":["cargo","fmt","--all","--check"],
    "clippy":["cargo","clippy","--workspace","--all-targets","--locked","--","-D","warnings"],
    "regressions":["cargo","test","-p","tt-executor","--test","c13_r1_persistence","-p","tt-persist","--test","c13_r1_format","--locked","--no-fail-fast"],
    "frontend-freshness":["cargo","test","-p","tt-engine","--test","frontend_schemas","--locked"],
    "declaration-generation":["python","scripts/gen-command-declarations.py"],
    "product-diff":["git","diff","--exit-code","f119948","--","spec","crates/*/src","desktop","Cargo.toml","Cargo.lock","deny.toml","crates/executor/tests/c12*","tests/adversarial","plan"],
}
results=[]
for name,cmd in commands.items():
    start=time.monotonic()
    with (logs/f"{name}.log").open("wb") as log:
        p=subprocess.run(cmd,cwd=ROOT,stdout=log,stderr=subprocess.STDOUT,env=env,timeout=600)
    results.append({"name":name,"command":cmd,"exitCode":p.returncode,"seconds":time.monotonic()-start,"sha256":hashlib.sha256((logs/f"{name}.log").read_bytes()).hexdigest()})
    print(name,p.returncode,flush=True)
(OUT/"final-checks.json").write_text(json.dumps({"head":subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip(),"recordedAt":datetime.datetime.now(datetime.timezone.utc).isoformat(),"environment":{k:env[k] for k in ["CARGO_PROFILE_DEV_DEBUG","CARGO_PROFILE_TEST_DEBUG","CARGO_INCREMENTAL","CARGO_BUILD_JOBS"]},"checks":results},indent=2),encoding="utf-8")
