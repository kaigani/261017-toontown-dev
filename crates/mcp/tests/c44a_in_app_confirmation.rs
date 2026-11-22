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
        assert!(Instant::now() < deadline, "no in-app request was shown");
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

/// The in-app requests the app shows now (`inApp`, undecided).
fn shown(h: &tt_executor::ExecutorHandle) -> Vec<String> {
    let s = h.invoke(tt_executor::Invocation::user("agent.server.status", json!({}))).result.unwrap();
    s["pendingConfirmations"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["inApp"] == true && p["decision"].is_null())
        .map(|p| p["confirmationId"].as_str().unwrap().to_owned())
        .collect()
}

/// Waits until the app shows `n` in-app requests (at most 10 s).
fn until_shown(h: &tt_executor::ExecutorHandle, n: usize) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let ids = shown(h);
        if ids.len() >= n || Instant::now() >= deadline {
            return ids;
        }
        std::thread::sleep(Duration::from_millis(30));
    }
}

fn held_fixture(name: &str) -> (std::path::PathBuf, tt_executor::ExecutorHandle, Client) {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    h.report_shell(tt_executor::host::ShellReport { attached: Some(true), ..Default::default() });
    assert!(h.invoke(tt_executor::Invocation::user("project.create", json!({ "name": name }))).is_ok());
    let mut options = opts(&dir);
    options.policy = Some(tt_executor::agents::Policy::Assist);
    let (c, _) = Client::connect(&h, options, json!({}));
    (dir, h, c)
}

fn export_call(file: &std::path::Path) -> Value {
    std::fs::write(file, "x").unwrap();
    json!({ "name": "run_command", "arguments": { "id": "agent.activity.export",
        "params": { "path": file.to_string_lossy(), "format": "jsonl", "onExists": "overwrite" } } })
}

/// Ruling D-63 item 1 (C44A-R1-60): a call held for the person's answer does not occupy the session: a read of the
/// same session is answered while the request is still open, before any decision.
#[test]
fn a_held_call_leaves_the_sessions_other_calls_answered() {
    let (dir, h, mut c) = held_fixture("Held read");
    let held = c.send("tools/call", export_call(&dir.join("a.jsonl")));
    let ids = until_shown(&h, 1);
    assert_eq!(ids.len(), 1, "the request is shown in the app");
    let read = c.send("tools/call", json!({ "name": "get_state", "arguments": {} }));
    let answered = c.wait(read, Duration::from_secs(10));
    assert!(answered.is_some_and(|m| m.get("error").is_none()), "the read is answered while the call is held");
    assert_eq!(shown(&h), ids, "and the request is still open, undecided");
    respond(&h, &ids[0], true);
    let m = c.wait(held, Duration::from_secs(20)).expect("the held call is answered after the decision");
    assert_eq!(m["result"]["structuredContent"]["status"], "ok", "{m}");
    c.close().unwrap();
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

/// Ruling D-63 item 1: a second confirmable call of the same session joins the app's queue at once: both requests are
/// shown before either is answered, and each held call receives its own outcome.
#[test]
fn two_held_calls_of_one_session_are_both_shown_at_once() {
    let (dir, h, mut c) = held_fixture("Held twice");
    let first = c.send("tools/call", export_call(&dir.join("a.jsonl")));
    assert_eq!(until_shown(&h, 1).len(), 1);
    let second = c.send("tools/call", export_call(&dir.join("b.jsonl")));
    let ids = until_shown(&h, 2);
    assert_eq!(ids.len(), 2, "both requests are open at once: {ids:?}");
    // Accept one, decline the other: each call gets its own outcome.
    respond(&h, &ids[0], true);
    respond(&h, &ids[1], false);
    // Either answer may come first: both are collected.
    let mut got = std::collections::HashMap::new();
    let deadline = Instant::now() + Duration::from_secs(20);
    while got.len() < 2 && Instant::now() < deadline {
        if let Some(m) = c.next(Duration::from_millis(500))
            && m.get("method").is_none()
            && let Some(id) = m.get("id").and_then(Value::as_u64)
            && (id == first || id == second)
        {
            got.insert(id, m);
        }
    }
    assert_eq!(got.len(), 2, "both held calls are answered");
    let mut outcomes = got
        .values()
        .map(|m| m["result"]["structuredContent"]["error"]["code"].as_str().unwrap_or("ok").to_owned())
        .collect::<Vec<_>>();
    outcomes.sort();
    assert_eq!(outcomes, ["confirmation.declined", "ok"]);
    c.close().unwrap();
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

/// Ruling D-65 item 1 (C44A-R1-60): a held call never waits on the session's request path. With four calls held (every
/// waiting thread taken) a fifth confirmable call is refused at once with the job waits' limit error, its confirmation
/// retired (the app shows four requests, not five), and a read that follows is answered at once.
#[test]
fn a_fifth_held_call_is_refused_and_the_request_path_stays_free() {
    let (dir, h, mut c) = held_fixture("Held five");
    let mut held = Vec::new();
    for i in 0..4 {
        held.push(c.send("tools/call", export_call(&dir.join(format!("held-{i}.jsonl")))));
        assert_eq!(until_shown(&h, i + 1).len(), i + 1, "request {i} shown");
    }
    let bound = Duration::from_secs(5);
    let asked = Instant::now();
    let fifth = c.send("tools/call", export_call(&dir.join("fifth.jsonl")));
    let m = c.wait(fifth, bound).expect("the fifth call is answered at once");
    // D-69 item 3 (D-45): the figure against its bound, recorded for the runners.
    record_timing("c44a_in_app_confirmation fifth held call refused", asked.elapsed(), bound);
    let env = &m["result"]["structuredContent"];
    assert_eq!(env["error"]["code"], "limit.exceeded", "{m}");
    assert_eq!(env["error"]["details"]["limit"], "concurrentJobWaits", "{m}");
    let asked = Instant::now();
    let read = c.send("tools/call", json!({ "name": "get_state", "arguments": {} }));
    let answered = c.wait(read, bound);
    record_timing("c44a_in_app_confirmation read with four held calls", asked.elapsed(), bound);
    assert!(answered.is_some_and(|m| m.get("error").is_none()), "the read is answered while four calls are held");
    assert_eq!(shown(&h).len(), 4, "the refused call's confirmation is not shown");
    // Retired, not merely hidden: the pending list holds the four held requests only, and the fifth export ran nothing.
    let status = h.invoke(tt_executor::Invocation::user("agent.server.status", json!({}))).result.unwrap();
    assert_eq!(status["pendingConfirmations"].as_array().unwrap().len(), 4, "{status}");
    assert_eq!(std::fs::read_to_string(dir.join("fifth.jsonl")).unwrap(), "x", "the refused export wrote nothing");
    for id in shown(&h) {
        respond(&h, &id, false);
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut answered = 0;
    while answered < 4 && Instant::now() < deadline {
        if let Some(m) = c.next(Duration::from_millis(500))
            && m.get("method").is_none()
            && m.get("id").and_then(Value::as_u64).is_some_and(|id| held.contains(&id))
        {
            answered += 1;
        }
    }
    assert_eq!(answered, 4, "each held call receives its decision");
    c.close().unwrap();
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}
