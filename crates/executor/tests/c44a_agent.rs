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
