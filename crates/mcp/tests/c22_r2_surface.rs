#![allow(clippy::unwrap_used, missing_docs, reason = "independent QA variants")]
mod support;
use support::*;
use serde_json::json;
#[test]
fn prefs_agent_failed_atomic_batch_rolls_back_gap_setting() {
    let dir=tmp();let h=executor(&dir.join("prefs"));
    let mut options=opts(&dir);options.policy=Some(tt_executor::agents::Policy::Autonomous);
    options.scopes=Some([tt_registry::Scope::Read,tt_registry::Scope::Ui,tt_registry::Scope::Prefs].into_iter().collect());
    let(mut c,_)=Client::connect(&h,options,json!({}));
    assert_eq!(c.run("colour.gapClosing.set",json!({"mode":"off"}))["status"],"ok");
    let r=c.call("batch",json!({"atomic":true,"label":"R2 rollback","steps":[{"id":"colour.gapClosing.set","params":{"mode":"large"}},{"id":"colour.gapClosing.set","params":{"mode":"invalid"}}]}));
    assert_ne!(r["structuredContent"]["status"],"ok","{r}");
    assert_eq!(c.run("colour.gapClosing.get",json!({}))["result"]["mode"],"off");
    c.close().unwrap();drop(h);let _=std::fs::remove_dir_all(dir);
}
#[test]
fn all_four_deferred_contracts_and_generated_types_name_c23() {
    let registry=tt_executor::registry().unwrap();
    let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for id in ["colour.fillMapping.get","colour.fillMapping.set","colour.selection.apply","colour.recolour.apply"] {
        let d=registry.entries().find(|e|e.id()==id).unwrap().describe();
        assert_eq!(d["milestone"],"M2");assert_eq!(d["checkpoint"],"C2.3","{d}");
    }
    let generated=std::fs::read_to_string(root.join("desktop/src/ipc/generated/commands.ts")).unwrap();
    assert!(generated.contains("colour.fillMapping.get"));
}
