//! Ruling D-58 (R-a), the MCP half: attached, a window present and `askInApp` on, a confirmation goes to the app only;
//! the agent's call is held until the person answers, and the agent receives the outcome: the command's result after an
//! accept, `confirmation.declined` after a decline, `confirmation.expired` after a document change.
#![allow(clippy::unwrap_used, missing_docs, reason = "review tests")]
mod support;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use support::*;

/// Answers the first in-app request the app shows (as the person would in the sheet), from another thread.
fn answer_when_shown(
    h: &tt_executor::ExecutorHandle,
    act: impl FnOnce(&tt_executor::ExecutorHandle, &str) + Send + 'static,
) -> std::thread::JoinHandle<()> {
    let h = h.clone();
    std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            let s = h.invoke(tt_executor::Invocation::user("agent.server.status", json!({}))).result.unwrap();
            if let Some(p) = s["pendingConfirmations"].as_array().and_then(|a| a.iter().find(|p| p["inApp"] == true)) {
                let id = p["confirmationId"].as_str().unwrap().to_owned();
                act(&h, &id);
                return;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        panic!("no in-app request was shown");
    })
}

fn respond(h: &tt_executor::ExecutorHandle, id: &str, accept: bool) {
    let e = h.invoke(tt_executor::Invocation::user(
        "agent.confirmation.respond",
        json!({ "confirmationId": id, "accept": accept }),
    ));
    assert!(e.is_ok(), "{e:?}");
}

#[test]
fn an_in_app_confirmation_holds_the_call_until_the_persons_answer() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    h.report_shell(tt_executor::host::ShellReport { attached: Some(true), ..Default::default() });
    assert!(h.invoke(tt_executor::Invocation::user("project.create", json!({ "name": "Held" }))).is_ok());
    let mut options = opts(&dir);
    options.policy = Some(tt_executor::agents::Policy::Assist);
    let (mut c, _) = Client::connect(&h, options, json!({}));
    let file = dir.join("log.jsonl");
    let export = || json!({ "path": file.to_string_lossy(), "format": "jsonl", "onExists": "overwrite" });
    std::fs::write(&file, "x").unwrap();
    // Accepted in the app: the held call returns the command's result.
    let t = answer_when_shown(&h, |h, id| respond(h, id, true));
    let r = c.run("agent.activity.export", export());
    t.join().unwrap();
    assert_eq!(r["status"], "ok", "{r}");
    // Declined: confirmation.declined.
    let t = answer_when_shown(&h, |h, id| respond(h, id, false));
    let r = c.run("agent.activity.export", export());
    t.join().unwrap();
    assert_eq!(
        (r["status"].as_str(), r["error"]["code"].as_str()),
        (Some("error"), Some("confirmation.declined")),
        "{r}"
    );
    // The document changes while the request is open: confirmation.expired, and nothing is left to answer.
    let t = answer_when_shown(&h, |h, _| {
        let scene = h
            .invoke(tt_executor::Invocation::user("doc.object.find", json!({ "type": "scene" })))
            .result
            .unwrap()["items"][0]["id"]
            .clone();
        assert!(
            h.invoke(tt_executor::Invocation::user(
                "doc.object.patch",
                json!({ "targets": [scene], "props": { "meta": { "n": 2 } } })
            ))
            .is_ok()
        );
    });
    let r = c.run("agent.activity.export", export());
    t.join().unwrap();
    assert_eq!(
        (r["status"].as_str(), r["error"]["code"].as_str()),
        (Some("error"), Some("confirmation.expired")),
        "{r}"
    );
    let s: Value = h.invoke(tt_executor::Invocation::user("agent.server.status", json!({}))).result.unwrap();
    assert!(s["pendingConfirmations"].as_array().unwrap().is_empty(), "{s}");
    c.close().unwrap();
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}
