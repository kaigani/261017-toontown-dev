//! Independent pointer scope review (11 section 6.1).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod support;
use serde_json::json;
use support::*;
#[test]
fn pointer_requires_input_scope_even_when_command_scopes_are_granted() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    assert!(
        h.invoke(tt_executor::Invocation::user(
            "app.context.set",
            json!({"elementId":"scene:#1/element:\"Bob\"","frame":1})
        ))
        .is_ok()
    );
    let before = h.snapshot().unwrap().unwrap().document_rev();
    let mut o = opts(&dir);
    o.scopes = Some([tt_registry::Scope::Read, tt_registry::Scope::Ui, tt_registry::Scope::Edit].into_iter().collect());
    let (mut c, _) = Client::connect(&h, o, json!({}));
    let r=c.call("pointer",json!({"view":"drawing","tool":"brush","events":[{"type":"down","x":1000,"y":500},{"type":"up","x":1030,"y":480}]}));
    let after = h.snapshot().unwrap().unwrap().document_rev();
    assert_eq!(r["structuredContent"]["error"]["code"], "permission.denied", "{r}; revision {before} -> {after}");
    assert_eq!(before, after);
}
