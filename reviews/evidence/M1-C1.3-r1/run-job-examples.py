"""Evidence-only copy of builder fixture runner; await and schema-check every C1.3 job example."""
import hashlib
import json
import pathlib
import subprocess

OUT=pathlib.Path(__file__).resolve().parent
ROOT=OUT.parents[2]
source=ROOT/"crates/executor/tests/c12_kernel_state.rs"
text=source.read_text(encoding="utf-8")
text=text.replace("mod common;","mod common;\nmod c13_support;")
text=text.replace("for e in registry.entries() {",'for e in registry.entries().filter(|e| e.id().starts_with("project.") || e.id().starts_with("doc.") || e.id()=="app.quit") {')
needle="let env = h.invoke(Invocation::user(e.id(), params));"
insert='''
            if let Some(started)=env.result.as_ref().filter(|r|r.get("jobId").is_some()) {
                let rec=c13_support::wait(&h,started);
                assert_eq!(rec["state"],"succeeded","{}: {}",e.id(),rec);
                let schema=e.result_schema();
                let mut final_schema=schema.get("x-jobResult").unwrap().clone();
                if let Some(defs)=schema.get("$defs") { final_schema.as_object_mut().unwrap().insert("$defs".into(),defs.clone()); }
                let validator=jsonschema::options().with_draft(jsonschema::Draft::Draft202012).build(&final_schema).unwrap();
                let errors:Vec<_>=validator.iter_errors(&rec["result"]).map(|e|e.to_string()).collect();
                assert!(errors.is_empty(),"{} final result: {:?}",e.id(),errors);
                println!("final job example succeeded and schema validated: {}",e.id());
            }
'''
assert text.count(needle)==2
text=text.replace(needle,needle+insert,1)
text=text.replace('assert!(ran >= 140, "examples: {ran}");','assert!(ran >= 48, "examples: {ran}");')
target=ROOT/"crates/executor/tests/c13_r1_evidence_examples.rs"
target.write_text(text,encoding="utf-8")
(OUT/"job-examples-source.rs.txt").write_text(text,encoding="utf-8")
cmd=["cargo","test","-p","tt-executor","--locked","--test",target.stem,"every_example_runs_on_the_fixture_under_the_watchdog","--","--nocapture"]
try:
    old=OUT/"job-examples.log"
    if old.exists():old.replace(OUT/"job-examples-initial-schema-context.log")
    with (OUT/"job-examples.log").open("wb") as log:
        p=subprocess.run(cmd,cwd=ROOT,stdout=log,stderr=subprocess.STDOUT,timeout=300)
    (OUT/"job-examples-receipt.json").write_text(json.dumps({"originalSha256":hashlib.sha256(source.read_bytes()).hexdigest(),"edits":["restrict to C1.3 namespace and app.quit","await every job result","validate x-jobResult schema","threshold 140 -> 48 for restricted scope"],"command":cmd,"exitCode":p.returncode},indent=2),encoding="utf-8")
finally:target.unlink(missing_ok=True)
print(p.returncode)
