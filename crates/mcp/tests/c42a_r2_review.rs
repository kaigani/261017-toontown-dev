//! Independent round-two scope probes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod support;
use serde_json::json;
use std::time::{Duration, Instant};
use support::*;
use tt_registry::Scope;
#[test]
fn raw_input_and_produced_command_scopes() {
    for scopes in [
        vec![Scope::Read, Scope::Ui, Scope::Edit],
        vec![Scope::Read, Scope::Input],
        vec![Scope::Read, Scope::Input, Scope::Ui, Scope::Edit],
    ] {
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
        assert!(h.invoke(tt_executor::Invocation::user("tool.active.set", json!({"toolId":"brush"}))).is_ok());
        let before = h.snapshot().unwrap().unwrap().document_rev();
        let mut o = opts(&dir);
        o.scopes = Some(scopes.iter().copied().collect());
        let (mut c, _) = Client::connect(&h, o, json!({}));
        let events = if scopes.contains(&Scope::Input) {
            json!([{"type":"down","x":1000,"y":500},{"type":"up","x":1030,"y":480}])
        } else {
            json!((0..10000).map(|_| json!({"type":"hover","x":1000,"y":500})).collect::<Vec<_>>())
        };
        let start = Instant::now();
        let r = c.call("pointer", json!({"view":"drawing","events":events}));
        assert!(start.elapsed() < Duration::from_secs(2), "scope gate took {:?}", start.elapsed());
        if scopes.contains(&Scope::Edit) && scopes.contains(&Scope::Input) {
            assert!(r["structuredContent"]["error"].is_null(), "{r}");
            assert!(h.snapshot().unwrap().unwrap().document_rev() > before);
        } else {
            assert_eq!(r["structuredContent"]["error"]["code"], "permission.denied", "{r}");
            let missing = if scopes.contains(&Scope::Input) { "edit" } else { "input" };
            assert!(
                r["structuredContent"]["error"]["details"]["missingScopes"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(missing)),
                "{r}"
            );
            assert_eq!(h.snapshot().unwrap().unwrap().document_rev(), before);
        }
        let key = c.call("key", json!({"keys":"F24"}));
        if !scopes.contains(&Scope::Input) {
            assert_eq!(key["structuredContent"]["error"]["code"], "permission.denied", "{key}");
            assert_eq!(key["structuredContent"]["error"]["details"]["missingScopes"], json!(["input"]));
        } else {
            assert_eq!(key["structuredContent"]["result"]["handled"], false, "{key}");
        }
    }
}
