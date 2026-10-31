//! Malformed-parameter sweep over every registered command through MCP.
#![allow(clippy::unwrap_used, missing_docs, reason = "review tests")]
mod support;
use support::*;
use serde_json::json;
use std::time::Duration;

#[test]
fn every_registered_command_answers_malformed_params_through_mcp() {
    let dir=tmp();
    let h=executor(&dir.join("user"));
    let (mut c,_)=Client::connect(&h,opts(&dir),json!({}));
    let registry=tt_executor::registry().unwrap();
    let mut count=0;
    for entry in registry.entries().filter(|e|e.spec.kind!=tt_registry::Kind::Event) {
        for params in [json!(null),json!([]),json!({"__review_malformed":true})] {
            let req=c.send("tools/call",json!({"name":"run_command","arguments":{"id":entry.id(),"params":params}}));
            let response=c.wait(req,Duration::from_secs(10)).unwrap();
            assert!(response.get("result").is_some()||response.get("error").is_some(),"{}: {response}",entry.id());
            count+=1;
        }
    }
    eprintln!("malformed MCP sweep: {count} responses");
    c.close().unwrap();
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}
