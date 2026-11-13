//! C3.1 independent protocol checks through the generic MCP tool.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod support;
use serde_json::{Value, json};
const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
fn call(c: &mut support::Client, id: &str, p: Value) -> Value {
    c.call("run_command", json!({"id":id,"params":p}))["structuredContent"].clone()
}
fn client() -> (tt_executor::ExecutorHandle, support::Client) {
    let dir = support::tmp();
    let h = support::executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let mut opts = support::opts(&dir);
    opts.policy = Some(tt_executor::agents::Policy::Autonomous);
    let (c, _) = support::Client::connect(&h, opts, json!({}));
    (h, c)
}
#[test]
fn drawing_in_use_parameter_confirmation_is_read_only_and_retry_applies() {
    let (h, mut c) = client();
    let r = h.snapshot().unwrap().unwrap().document_rev();
    let p = json!({"drawingIds":["scene:\"Intro\"/element:\"Bob\"/drawing:\"1\""]});
    let e = call(&mut c, "element.drawing.delete", p.clone());
    assert_eq!(e["status"], "needs_confirmation", "{e}");
    assert_eq!(e["result"]["code"], "drawing.inUse");
    assert_eq!(h.snapshot().unwrap().unwrap().document_rev(), r);
    assert_eq!(e["result"]["retryWith"]["ifInUse"], "clearCells");
    let mut retry = p;
    retry["ifInUse"] = json!("clearCells");
    let done = call(&mut c, "element.drawing.delete", retry);
    assert_eq!(done["status"], "ok", "{done}");
    assert_eq!(h.snapshot().unwrap().unwrap().document_rev(), r + 1);
    c.close().unwrap();
}
#[test]
fn generic_tool_dry_run_does_not_leak_category_colour() {
    let (_h, mut c) = client();
    let before = call(&mut c, "element.typeColour.get", json!({}));
    let e = c.call(
        "run_command",
        json!({"id":"element.typeColour.set","params":{"category":"pegs","colour":"#123456"},"dryRun":true}),
    );
    assert_eq!(e["structuredContent"]["status"], "ok", "{e}");
    let after = call(&mut c, "element.typeColour.get", json!({}));
    c.close().unwrap();
    assert_eq!(after["result"], before["result"], "ADR-0015 acceptance includes new session state");
}
#[test]
fn explicit_cell_edit_undo_and_invalid_reference_through_mcp() {
    let (_h, mut c) = client();
    let params = json!({"elementIds":[BOB],"from":1,"to":10});
    let before = call(&mut c, "xsheet.exposures.get", params.clone())["result"]["columns"].clone();
    let e = call(&mut c, "cell.exposure.add", json!({"elementIds":[BOB],"from":1,"to":1}));
    assert_eq!(e["status"], "ok", "{e}");
    assert_eq!(call(&mut c, "edit.undo", json!({}))["status"], "ok");
    assert_eq!(call(&mut c, "xsheet.exposures.get", params)["result"]["columns"], before);
    let bad = call(&mut c, "element.get", json!({"elementId":"scene:\"Intro\"/element:\"Absent\""}));
    assert_eq!(bad["error"]["code"], "element.notFound", "{bad}");
    c.close().unwrap();
}
