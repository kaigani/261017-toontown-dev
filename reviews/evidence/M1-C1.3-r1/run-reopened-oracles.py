"""Materialize evidence-only copies of the unchanged R1/R3 10k-step oracles, save/reopen their initial fixture.

One saved fixture is reused for both deterministic replays, including its modifiedAt stamp and persisted revisions.
All oracle assertions and seeds are byte-preserved. Temporary Cargo targets are removed.
"""
import hashlib
import json
import os
import pathlib
import subprocess

OUT = pathlib.Path(__file__).resolve().parent
ROOT = OUT.parents[2]
helper = '''
mod c13_support;
fn reopened_fixture() -> tt_executor::ExecutorHandle {
    static SAVED: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    let path = SAVED.get_or_init(|| {
        let initial = common::fixture();
        let path = c13_support::tmp().join("oracle.animpkg");
        c13_support::job_ok(&initial,"project.saveAs",serde_json::json!({"path":path}));
        common::ok(&initial,"project.close",serde_json::json!({}));
        path
    });
    let h = tt_executor::ExecutorHandle::spawn(tt_executor::ExecutorConfig {budget:std::time::Duration::from_secs(60)}).unwrap();
    c13_support::job_ok(&h,"project.open",serde_json::json!({"path":path,"readOnly":true}));
    assert_eq!(common::ok(&h,"doc.state.get",serde_json::json!({}))["dirty"],false);
    h
}
'''
receipts = []
for round in (1,3):
    original = ROOT / f"crates/executor/tests/c12_r{round}_stress.rs"
    target = ROOT / f"crates/executor/tests/c13_r1_evidence_oracle{round}.rs"
    source = original.read_text(encoding="utf-8")
    patched = source.replace("let h = fixture();", "let h = reopened_fixture();").replace("let replay = fixture();", "let replay = reopened_fixture();") + helper
    artifact_dir = OUT / f"oracle-r{round}"
    artifact_dir.mkdir(exist_ok=True)
    env = dict(os.environ, TT_C12_REVIEW_ARTIFACT_DIR=str(artifact_dir))
    target.write_text(patched,encoding="utf-8")
    (OUT / f"oracle-r{round}-source.rs.txt").write_text(patched,encoding="utf-8")
    cmd = ["cargo","test","-p","tt-executor","--release","--locked","--test",target.stem,"ten_thousand_history_operations","--","--nocapture"]
    try:
        log_path = OUT / f"oracle-r{round}.log"
        if log_path.exists():
            log_path.replace(OUT / f"oracle-r{round}-initial-setup.log")
        with log_path.open("wb") as log:
            result = subprocess.run(cmd,cwd=ROOT,stdout=log,stderr=subprocess.STDOUT,env=env,timeout=900)
        receipts.append({"original":str(original.relative_to(ROOT)),"sha256":hashlib.sha256(original.read_bytes()).hexdigest(),"changes":["fixture -> saved/reopened fixture", "replay fixture -> saved/reopened fixture", "append helper"],"command":cmd,"exitCode":result.returncode})
    finally:
        target.unlink(missing_ok=True)
    print(round,receipts[-1]["exitCode"],flush=True)
(OUT / "reopened-oracles.json").write_text(json.dumps(receipts,indent=2),encoding="utf-8")
