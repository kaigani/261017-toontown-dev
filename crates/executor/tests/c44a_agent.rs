//! M4 C4.4a, agent surfaces (reviews/M4-design-agent-surfaces.md §2, §6): the agent settings are preferences, so the
//! Agent access category, the command layer and a restart see one state.
#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test fixtures")]

mod common;

use common::*;
use serde_json::{Value, json};

fn spawn_in(dir: &std::path::Path) -> tt_executor::ExecutorHandle {
    tt_executor::ExecutorHandle::spawn_with_options(tt_executor::ExecutorOptions {
        config: tt_executor::ExecutorConfig { budget: std::time::Duration::from_secs(60) },
        persist: tt_executor::PersistOptions {
            user_dirs: Some(tt_executor::UserDirs::under(dir)),
            ..Default::default()
        },
    })
    .unwrap()
}

fn changed(rx: &std::sync::mpsc::Receiver<tt_executor::Event>) -> Vec<Vec<String>> {
    rx.try_iter()
        .filter(|e| e.name == "prefs.changed")
        .map(|e| {
            e.payload["keys"].as_array().unwrap().iter().map(|k| k.as_str().unwrap().to_owned()).collect::<Vec<_>>()
        })
        .collect()
}

fn pref(h: &tt_executor::ExecutorHandle, k: &str) -> Value {
    ok(h, "prefs.get", json!({ "keys": [k] }))["values"][k].clone()
}

/// SI-179: `agent.activity.persist.set` and `agent.typedTools.set` stage preferences: `prefs.changed` names them, and
/// the next run starts with them (the typed tools reach the MCP server's status).
#[test]
fn the_activity_persistence_and_the_typed_tools_survive_a_restart() {
    let dir = std::env::temp_dir().join(format!("tt-c44a-agent-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    {
        let h = spawn_in(&dir);
        let rx = h.subscribe();
        ok(&h, "agent.activity.persist.set", json!({ "enabled": true }));
        let r = ok(&h, "agent.typedTools.set", json!({ "enabled": true, "namespaces": ["doc"] }));
        assert!(r["toolCount"].as_u64().unwrap() > 0, "{r}");
        assert_eq!(changed(&rx), [vec!["agent.activity.persist".to_owned()], vec!["agent.typedTools".to_owned()]]);
        // The same value again changes nothing and announces nothing.
        ok(&h, "agent.activity.persist.set", json!({ "enabled": true }));
        assert!(changed(&rx).is_empty());
        assert_eq!(h.agent_status("none").unwrap().typed_tools, Some(vec!["doc".to_owned()]));
    }
    let h = spawn_in(&dir);
    assert_eq!(pref(&h, "agent.activity.persist"), true);
    assert_eq!(pref(&h, "agent.typedTools"), json!({ "enabled": true, "namespaces": ["doc"] }));
    assert_eq!(h.agent_status("none").unwrap().typed_tools, Some(vec!["doc".to_owned()]));
    // `prefs.set` writes the same state the command does.
    ok(&h, "prefs.set", json!({ "values": { "agent.typedTools": { "enabled": false, "namespaces": [] } } }));
    assert_eq!(h.agent_status("none").unwrap().typed_tools, None);
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

/// SI-173: `agent.server.set` and `prefs.set` on the `agent.server.*` keys give one state and the same event, so the
/// Agent access category follows a change made through either; a port out of range is refused by both alike.
#[test]
fn the_server_command_and_the_server_preferences_are_one_state() {
    let h = fixture();
    let rx = h.subscribe();
    ok(&h, "agent.server.set", json!({ "enabled": true, "port": 47300, "http": false }));
    let mut keys = changed(&rx);
    for k in &mut keys {
        k.sort();
    }
    assert_eq!(keys, [vec!["agent.server.enabled".to_owned(), "agent.server.port".to_owned()]]);
    assert_eq!(pref(&h, "agent.server.port"), 47300);
    assert_eq!(ok(&h, "agent.server.status", json!({}))["enabled"], true);
    ok(&h, "prefs.set", json!({ "values": { "agent.server.enabled": false } }));
    assert_eq!(changed(&rx), [vec!["agent.server.enabled".to_owned()]]);
    assert_eq!(ok(&h, "agent.server.status", json!({}))["enabled"], false);
    assert_eq!(err(&h, "agent.server.set", json!({ "enabled": true, "port": 80 })).code, "arg.invalid");
    assert_eq!(err(&h, "prefs.set", json!({ "values": { "agent.server.port": 80 } })).code, "prefs.outOfRange");
}

/// D-62 item 1 (C44A-R1-40; 11 §15.1): while the host serves, the status carries the serving endpoint, whether the
/// preference or the launch option started it (here the preference stays off, as with `--mcp-http`); when the host
/// stops, the endpoint goes.
#[test]
fn the_status_carries_the_serving_endpoint_however_the_server_was_started() {
    let h = fixture();
    ok(&h, "prefs.set", json!({ "values": { "agent.server.enabled": false } }));
    assert_eq!(ok(&h, "agent.server.status", json!({}))["endpoint"], Value::Null);
    h.set_agent_serving(Some("http://127.0.0.1:47215/mcp".to_owned()));
    let st = ok(&h, "agent.server.status", json!({}));
    assert_eq!(st["endpoint"], "http://127.0.0.1:47215/mcp");
    assert_eq!(st["enabled"], true);
    assert_eq!(pref(&h, "agent.server.enabled"), false, "serving does not change the saved preference");
    h.set_agent_serving(None);
    let st = ok(&h, "agent.server.status", json!({}));
    assert_eq!((st["endpoint"].clone(), st["enabled"].clone()), (Value::Null, json!(false)));
}

fn agent(h: &tt_executor::ExecutorHandle, root: &std::path::Path) -> tt_executor::Origin {
    let s = h
        .connect_agent(tt_executor::agents::AgentConnect {
            client: "c44a".into(),
            policy: Some(tt_executor::agents::Policy::Assist),
            roots: vec![root.to_path_buf()],
            transport: "test".into(),
            ..Default::default()
        })
        .unwrap();
    tt_executor::Origin::Agent { session: s.session_id, name: s.client, scopes: s.scopes }
}

fn as_agent(
    h: &tt_executor::ExecutorHandle,
    o: &tt_executor::Origin,
    params: Value,
    token: Option<&str>,
) -> tt_executor::Envelope {
    let options = tt_executor::InvokeOptions { confirm_token: token.map(str::to_owned), ..Default::default() };
    h.invoke(tt_executor::Invocation { command: "agent.activity.export".into(), params, options, origin: o.clone() })
}

/// Ruling D-58 (R-a), the kernel half, first: a token asked in the app is refused until the person accepts (and stays
/// pending while refused); a decline is the agent's `confirmation.declined`; a document revision change ends the request
/// (`confirmation.expired`), and every decision and expiry is announced to the windows (`shell.agentConfirmation`).
#[test]
fn an_in_app_token_waits_for_the_persons_answer() {
    let dir = std::env::temp_dir().join(format!("tt-c44a-confirm-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let h = spawn_in(&dir.join("user"));
    ok(&h, "project.create", json!({ "name": "Confirm" }));
    let file = dir.join("log.jsonl");
    std::fs::write(&file, "x").unwrap();
    let a = agent(&h, &dir);
    let params = json!({ "path": file.to_string_lossy(), "format": "jsonl", "onExists": "overwrite" });
    let token = |env: &tt_executor::Envelope| -> String {
        assert_eq!(env.status, tt_executor::Status::NeedsConfirmation, "{env:?}");
        env.result.as_ref().unwrap()["confirmToken"].as_str().unwrap().to_owned()
    };
    let rx = h.subscribe();
    // Accepted.
    let t1 = token(&as_agent(&h, &a, params.clone(), None));
    assert!(h.hold_in_app(&t1).unwrap());
    assert!(h.confirmation_waiting(&t1).unwrap());
    let pending = ok(&h, "agent.server.status", json!({}))["pendingConfirmations"].clone();
    assert_eq!(pending[0]["inApp"], true, "{pending}");
    let refused = as_agent(&h, &a, params.clone(), Some(&t1));
    let e = refused.error.unwrap();
    assert_eq!(
        (e.code.as_str(), e.details["reason"].as_str()),
        ("confirmation.expired", Some("waiting for the user's answer in the app"))
    );
    assert!(h.confirmation_waiting(&t1).unwrap(), "a refused in-app token stays pending");
    ok(&h, "agent.confirmation.respond", json!({ "confirmationId": t1, "accept": true }));
    assert!(!h.confirmation_waiting(&t1).unwrap());
    assert!(as_agent(&h, &a, params.clone(), Some(&t1)).is_ok());
    // Declined.
    let t2 = token(&as_agent(&h, &a, params.clone(), None));
    assert!(h.hold_in_app(&t2).unwrap());
    ok(&h, "agent.confirmation.respond", json!({ "confirmationId": t2, "accept": false }));
    assert_eq!(as_agent(&h, &a, params.clone(), Some(&t2)).error.unwrap().code, "confirmation.declined");
    // The document changed while the request was open.
    let t3 = token(&as_agent(&h, &a, params.clone(), None));
    assert!(h.hold_in_app(&t3).unwrap());
    let scene = ok(&h, "doc.object.find", json!({ "type": "scene" }))["items"][0]["id"].as_str().map(str::to_owned);
    ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "n": 1 } } }));
    assert!(!h.confirmation_waiting(&t3).unwrap());
    assert!(ok(&h, "agent.server.status", json!({}))["pendingConfirmations"].as_array().unwrap().is_empty());
    let e = as_agent(&h, &a, params, Some(&t3)).error.unwrap();
    assert_eq!(e.code, "confirmation.expired", "{e:?}");
    let outcomes: Vec<String> = rx
        .try_iter()
        .filter(|e| e.name == "shell.agentConfirmation")
        .map(|e| e.payload["outcome"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(outcomes, ["requested", "accepted", "requested", "declined", "requested", "expired"]);
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Ruling D-59 (C44A-R1S-01): the first decision settles an in-app request. A second, conflicting answer (another
/// window whose sheet had not caught up) before the agent's call uses the token is refused with `confirmation.expired`
/// ("already decided") and changes nothing, in both orders; the token then gives the first decision's outcome.
#[test]
fn the_first_decision_settles_an_in_app_request() {
    let dir = std::env::temp_dir().join(format!("tt-c44a-decide-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let h = spawn_in(&dir.join("user"));
    ok(&h, "project.create", json!({ "name": "Decide" }));
    let file = dir.join("log.jsonl");
    std::fs::write(&file, "x").unwrap();
    let a = agent(&h, &dir);
    let params = json!({ "path": file.to_string_lossy(), "format": "jsonl", "onExists": "overwrite" });
    for (first, second) in [(false, true), (true, false)] {
        let asked = as_agent(&h, &a, params.clone(), None);
        assert_eq!(asked.status, tt_executor::Status::NeedsConfirmation, "{asked:?}");
        let t = asked.result.as_ref().unwrap()["confirmToken"].as_str().unwrap().to_owned();
        assert!(h.hold_in_app(&t).unwrap());
        ok(&h, "agent.confirmation.respond", json!({ "confirmationId": t, "accept": first }));
        let e = err(&h, "agent.confirmation.respond", json!({ "confirmationId": t, "accept": second }));
        assert_eq!(e.code, "confirmation.expired", "{e:?}");
        assert_eq!(e.details["reason"], "already decided", "{e:?}");
        assert_eq!(e.details["decision"], first, "{e:?}");
        let used = as_agent(&h, &a, params.clone(), Some(&t));
        if first {
            assert!(used.is_ok(), "the first answer, Accept, settles it: {used:?}");
        } else {
            assert_eq!(used.error.unwrap().code, "confirmation.declined", "the first answer, Decline, settles it");
        }
    }
    // An expired request (the document changed) refuses an answer too.
    let asked = as_agent(&h, &a, params.clone(), None);
    let t = asked.result.as_ref().unwrap()["confirmToken"].as_str().unwrap().to_owned();
    assert!(h.hold_in_app(&t).unwrap());
    let scene = ok(&h, "doc.object.find", json!({ "type": "scene" }))["items"][0]["id"].clone();
    ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "n": 1 } } }));
    let e = err(&h, "agent.confirmation.respond", json!({ "confirmationId": t, "accept": true }));
    assert_eq!((e.code.as_str(), e.details["reason"].as_str()), ("confirmation.expired", Some("the document changed")));
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}
