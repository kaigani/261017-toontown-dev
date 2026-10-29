"""Compare lint cost in disposable, committed-source snapshots; never edit either product checkout."""
import io
import json
import pathlib
import shutil
import subprocess
import tarfile
import tempfile
import time

OUT=pathlib.Path(__file__).resolve().parent
ROOT=OUT.parents[2]
records=[]
for ref in ["67265b8","f119948"]:
    snapshot=pathlib.Path(tempfile.mkdtemp(prefix="tt-c13-lint-"))
    archive=subprocess.run(["git","archive",ref,"desktop","scripts","tests/adversarial","package.json","pnpm-lock.yaml","pnpm-workspace.yaml"],cwd=ROOT,capture_output=True,check=True).stdout
    tarfile.open(fileobj=io.BytesIO(archive)).extractall(snapshot,filter="data")
    for relative in ["node_modules","desktop/node_modules"]:
        target=snapshot/relative
        # Junctions retain the existing install; the source snapshot and cwd are independent.
        subprocess.run(["cmd","/c","mklink","/J",str(target),str(ROOT/relative)],capture_output=True,check=True)
    cli=ROOT/"desktop/node_modules/eslint/bin/eslint.js"
    generated=snapshot/"desktop/src/ipc/generated/commands.ts"
    runs=[]
    for index in range(3):
        start=time.monotonic()
        p=subprocess.run(["node",str(cli),".","--max-warnings","0"],cwd=snapshot/"desktop",capture_output=True,timeout=90)
        runs.append({"seconds":time.monotonic()-start,"exitCode":p.returncode})
        (OUT/f"lint-{ref}-{index}.log").write_bytes(p.stdout+p.stderr)
    records.append({"ref":ref,"snapshot":str(snapshot),"commandsTsBytes":generated.stat().st_size,"runs":runs})
    # Do not recursively delete junction targets. Remove the junctions first, then this checked temporary tree.
    for relative in ["desktop/node_modules","node_modules"]:
        (snapshot/relative).rmdir()
    assert snapshot.resolve().parent==pathlib.Path(tempfile.gettempdir()).resolve()
    shutil.rmtree(snapshot)
(OUT/"lint-comparison.json").write_text(json.dumps(records,indent=2),encoding="utf-8")
print(json.dumps(records))
