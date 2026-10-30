"""Add MCP transports around the retained saved-project oracle; keep its oracle assertions and seeds."""
from pathlib import Path
ROOT = Path(__file__).resolve().parents[3]
helpers = r'''
mod support;
use std::cell::RefCell;
struct Protocol {
    handle: tt_executor::ExecutorHandle,
    client: RefCell<Option<support::Client>>,
}
impl std::ops::Deref for Protocol {
    type Target = tt_executor::ExecutorHandle;
    fn deref(&self) -> &Self::Target { &self.handle }
}
impl Drop for Protocol {
    fn drop(&mut self) {
        if let Some(c) = self.client.get_mut().take() { c.close().unwrap(); }
    }
}
fn run(h: &Protocol, id: &str, mut params: Value) -> Value {
    let dry = params.as_object_mut().and_then(|p| p.remove("dryRun")).unwrap_or(json!(false));
    h.client.borrow_mut().as_mut().unwrap().call("run_command", json!({"id":id,"params":params,"dryRun":dry}))["structuredContent"].clone()
}
fn ok(h: &Protocol, id: &str, params: Value) -> Value {
    let e = run(h,id,params);
    assert_eq!(e["status"], "ok", "{e}");
    e["result"].clone()
}
fn first(h: &Protocol, ty: &str) -> String {
    ok(h,"doc.object.find",json!({"type":ty,"limit":1}))["items"][0]["id"].as_str().unwrap().to_owned()
}
fn rev(h: &Protocol) -> u64 { h.snapshot().unwrap().unwrap().document_rev() }
fn saved_fixture() -> Protocol {
    static SAVED: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    let path = SAVED.get_or_init(|| {
        let dir = support::tmp();
        let h = support::executor(&dir.join("initial-user"));
        assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
        let path = dir.join("Oracle.animpkg");
        let e = h.invoke(tt_executor::Invocation::user("project.saveAs",json!({"path":path})));
        assert!(e.is_ok(),"{e:?}");
        let j = e.result.unwrap()["jobId"].clone();
        let done = h.invoke(tt_executor::Invocation::user("job.wait",json!({"jobId":j,"timeoutMs":60000})));
        assert_eq!(done.result.unwrap()["state"],"succeeded");
        assert!(h.invoke(tt_executor::Invocation::user("project.close",json!({}))).is_ok());
        path
    });
    let dir = support::tmp();
    let h = support::executor(&dir.join("user"));
    let mut opts = support::opts(path.parent().unwrap());
    opts.policy = Some(tt_executor::agents::Policy::Autonomous);
    let (c,_) = support::Client::connect(&h,opts,json!({}));
    let p = Protocol { handle:h, client:RefCell::new(Some(c)) };
    let e = ok(&p,"project.open",json!({"path":path,"readOnly":true}));
    let done = ok(&p,"job.wait",json!({"jobId":e["jobId"],"timeoutMs":60000}));
    assert_eq!(done["state"],"succeeded","{done}");
    assert_eq!(ok(&p,"doc.state.get",json!({}))["dirty"],false);
    p
}
'''
for seed in ('r1','r3'):
    old = ROOT / f'crates/executor/tests/c13_r3_history_{seed}.rs'
    text = old.read_text(encoding='utf-8').split('// Preserve every oracle assertion')[0]
    text = text.replace('mod common;\nuse common::*;', '')
    text = text.replace('use std::time::{Duration, Instant};', 'use std::time::Instant;')
    text = text.replace('assert!(env.is_ok(),', 'assert!(env["status"] == "ok",')
    text = text.replace('"random-history.json"', f'"mcp-history-{seed}.json"').replace('"random-history-commands.json"', f'"mcp-history-{seed}-commands.json"')
    (ROOT / f'crates/mcp/tests/c14_r3_history_{seed}.rs').write_text(text + helpers, encoding='utf-8')
