#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review regression tests")]
mod support;
use support::*;
use serde_json::json;
#[test]
fn an_agent_with_preferences_permission_can_set_gap_closing(){
    let dir=tmp();let h=executor(&dir.join("prefs"));
    let mut options=opts(&dir); options.policy=Some(tt_executor::agents::Policy::Autonomous);
    options.scopes=Some([tt_registry::Scope::Read,tt_registry::Scope::Ui,tt_registry::Scope::Prefs].into_iter().collect());
    let (mut c,_)=Client::connect(&h,options,json!({}));
    let result=c.run("colour.gapClosing.set",json!({"mode":"small"}));
    c.close().unwrap();drop(h);std::fs::remove_dir_all(dir).unwrap();
    assert_eq!(result["status"],"ok","the persisted paint preference must not require the agent-forbidden app scope: {result}");
}
