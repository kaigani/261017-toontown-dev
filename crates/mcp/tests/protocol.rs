//! MCP protocol conformance of the stdio server (11 §4, §6, §7, §9, §11, §12; 11 §14.7): lifecycle and errors, tools
//! with output schemas, resources and subscriptions, events, prompts, completions, logging, progress, cancellation,
//! elicitation and its fallback, scopes, confirmations, audit, pause and disconnect, typed tools, idempotent requests.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod support;

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use support::*;
use tt_executor::agents::Policy;
use tt_executor::{Invocation, InvokeOptions, Origin};

fn user(h: &tt_executor::ExecutorHandle, id: &str, params: Value) -> tt_executor::Envelope {
    h.invoke(Invocation { command: id.into(), params, options: InvokeOptions::default(), origin: Origin::User })
}

/// A connected client with a fresh project created by the agent at `<dir>/P.animpkg`.
fn with_project(caps: Value) -> (std::path::PathBuf, tt_executor::ExecutorHandle, Client, String) {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    let (mut c, _) = Client::connect(&h, opts(&dir), caps);
    let env = c.run("project.create", json!({ "name": "P", "path": path(&dir.join("P.animpkg")) }));
    assert_eq!(env["status"], "ok", "{env}");
    let scene = env["result"]["sceneId"].as_str().unwrap().to_owned();
    (dir, h, c, scene)
}

#[test]
fn lifecycle_negotiation_and_json_rpc_errors() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    let mut c = Client::start(&h, opts(&dir));
    // Before initialize: refused; ping always works.
    let early = c.message("tools/list", json!({}));
    assert_eq!(early["error"]["code"], -32600, "{early}");
    assert_eq!(c.request("ping", json!({})), json!({}));
    // An unknown revision gets the server's.
    let init = c.request(
        "initialize",
        json!({ "protocolVersion": "2099-01-01", "capabilities": {}, "clientInfo": { "name": "t", "version": "1" } }),
    );
    assert_eq!(init["protocolVersion"], "2025-06-18");
    for cap in ["tools", "resources", "prompts", "logging", "completions"] {
        assert!(init["capabilities"].get(cap).is_some(), "{cap}: {init}");
    }
    assert_eq!(init["capabilities"]["resources"]["subscribe"], true);
    assert_eq!(init["serverInfo"]["name"], "toontown");
    assert_eq!(init["_meta"]["toontown"]["runMode"], "headless");
    // The instructions are the agent guide resource, exactly.
    let guide = c.request("resources/read", json!({ "uri": "app://docs/agent-guide" }));
    assert_eq!(guide["contents"][0]["text"], init["instructions"]);
    let unknown = c.message("no/such/method", json!({}));
    assert_eq!(unknown["error"]["code"], -32601);
    let unknown_tool = c.message("tools/call", json!({ "name": "nope", "arguments": {} }));
    assert_eq!(unknown_tool["error"]["code"], -32602);
    let missing_uri = c.message("resources/read", json!({ "uri": "app://nope" }));
    assert_eq!(missing_uri["error"]["code"], -32002);
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn parse_and_invalid_request_errors_are_answered() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    let (mut c, _) = Client::connect(&h, opts(&dir), json!({}));
    c.raw("{not json");
    c.raw(r#"{"jsonrpc":"2.0","id":77,"method":5}"#);
    c.raw(r#"{"jsonrpc":"1.0","id":78,"method":"ping"}"#);
    let mut codes = Vec::new();
    while codes.len() < 3 {
        let m = c.next(Duration::from_secs(5)).expect("an error response");
        if let Some(code) = m["error"]["code"].as_i64() {
            codes.push((m["id"].clone(), code));
        }
    }
    assert_eq!(codes[0], (Value::Null, -32700), "{codes:?}");
    assert_eq!(codes[1], (json!(77), -32600), "{codes:?}");
    assert_eq!(codes[2], (json!(78), -32600), "{codes:?}");
    // The stream stays usable.
    assert_eq!(c.request("ping", json!({})), json!({}));
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tools_list_schemas_and_structured_content_validate() {
    let (dir, _h, mut c, scene) = with_project(json!({}));
    let tools = c.request("tools/list", json!({}))["tools"].as_array().unwrap().clone();
    assert_eq!(tools.len(), 28, "the generic tool set (11 §7)");
    let names: BTreeSet<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names.len(), 28);
    for t in &tools {
        assert_eq!(t["inputSchema"]["type"], "object", "{t}");
        assert!(t["outputSchema"].is_object(), "{t}");
        assert!(t["annotations"]["readOnlyHint"].is_boolean(), "{t}");
    }
    let schema = tools.iter().find(|t| t["name"] == "get_object").unwrap()["outputSchema"].clone();
    let validator = jsonschema::validator_for(&schema).unwrap();
    for (tool, args) in [
        ("get_state", json!({})),
        ("get_object", json!({ "ref": scene })),
        ("find_objects", json!({ "type": "element" })),
        ("set_properties", json!({ "targets": [scene], "props": { "meta": { "a": 1 } }, "dryRun": true })),
        ("history", json!({})),
        ("doc_diff", json!({ "sinceRev": 0 })),
        ("list_commands", json!({ "namespace": "doc" })),
        ("describe_command", json!({ "id": "doc.object.patch" })),
        ("get_docs", json!({ "topic": "errors" })),
        ("pointer", json!({ "action": "click", "view": "camera", "x": 1, "y": 1 })),
        ("run_command", json!({ "id": "no.such.command" })),
    ] {
        let r = c.call(tool, args);
        let sc = &r["structuredContent"];
        let errors: Vec<String> = validator.iter_errors(sc).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{tool}: {errors:?} {sc}");
        assert_eq!(r["isError"], sc["status"] == "error", "{tool}: {r}");
        assert!(r["content"][0]["text"].as_str().unwrap().starts_with(tool), "{tool}: {r}");
    }
    let pointer = c.call("pointer", json!({}));
    assert_eq!(pointer["structuredContent"]["error"]["code"], "command.notImplemented");
    assert_eq!(pointer["structuredContent"]["error"]["details"]["milestone"], "M2 C2.3");
    let state = c.call("get_state", json!({ "include": ["agent", "project"] }))["structuredContent"]["result"].clone();
    assert_eq!(state["agent"]["policy"], "assist", "{state}");
    assert!(state.get("selection").is_none(), "include projects the state: {state}");
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn generic_tools_wrap_commands_revisions_undo_and_idempotent_requests() {
    let (dir, h, mut c, scene) = with_project(json!({}));
    let rev = c.call("get_object", json!({ "ref": scene }))["structuredContent"]["documentRev"].as_u64().unwrap();
    let dry = c.call("set_properties", json!({ "targets": [scene], "props": { "meta": { "x": 1 } }, "dryRun": true }));
    assert_eq!(dry["structuredContent"]["dryRun"], true);
    assert_eq!(dry["structuredContent"]["documentRev"].as_u64(), Some(rev), "a dry run changes nothing");
    let conflict = c.call(
        "set_properties",
        json!({ "targets": [scene], "props": { "meta": { "x": 1 } }, "expectRev": { scene.clone(): 999 } }),
    );
    assert_eq!(conflict["structuredContent"]["error"]["code"], "rev.conflict", "{conflict}");
    let done = c.call("set_properties", json!({ "targets": [scene], "props": { "meta": { "x": 2 } } }));
    assert_eq!(done["structuredContent"]["status"], "ok");
    assert_eq!(done["structuredContent"]["documentRev"].as_u64(), Some(rev + 1));
    let diff = c.call("doc_diff", json!({ "sinceRev": rev }))["structuredContent"].clone();
    assert_eq!(diff["status"], "ok", "{diff}");
    assert_eq!(c.call("undo", json!({}))["structuredContent"]["status"], "ok");
    assert_eq!(c.call("redo", json!({}))["structuredContent"]["status"], "ok");
    let hist = c.call("history", json!({}))["structuredContent"].clone();
    assert_eq!(hist["status"], "ok", "{hist}");
    // checkpoint
    assert_eq!(c.call("checkpoint", json!({ "action": "create", "name": "cp" }))["structuredContent"]["status"], "ok");
    // Idempotent request envelopes: the same requestId answers the remembered envelope and commits once.
    let args = json!({ "id": "doc.object.patch", "params": { "targets": [scene], "props": { "meta": { "y": 1 } } }, "requestId": "req-1" });
    let first = c.call("run_command", args.clone())["structuredContent"].clone();
    let again = c.call("run_command", args)["structuredContent"].clone();
    assert_eq!(first["documentRev"], again["documentRev"], "{first} {again}");
    assert_eq!(first, again);
    let snap = h.snapshot().unwrap().unwrap();
    assert_eq!(Some(snap.document_rev()), first["documentRev"].as_u64(), "one commit");
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn atomic_batch_commits_one_revision_one_undo_step_and_one_audit_entry() {
    let (dir, h, mut c, scene) = with_project(json!({}));
    let before = h.snapshot().unwrap().unwrap().document_rev();
    let audit_before = c.request("resources/read", json!({ "uri": "app://activity" }))["contents"][0]["text"]
        .as_str()
        .unwrap()
        .lines()
        .count();
    let r = c.call(
        "batch",
        json!({ "label": "Two edits", "steps": [
            { "id": "doc.object.patch", "params": { "targets": [scene], "props": { "meta": { "a": 1 } } } },
            { "id": "doc.object.patch", "params": { "targets": [scene], "props": { "meta": { "b": 2 } } } },
        ] }),
    );
    let env = &r["structuredContent"];
    assert_eq!(env["status"], "ok", "{env}");
    assert_eq!(env["documentRev"].as_u64(), Some(before + 1), "one committed revision");
    assert_eq!(env["steps"].as_array().unwrap().len(), 2);
    let hist = c.call("history", json!({}))["structuredContent"]["result"].clone();
    let labels: Vec<&str> = hist["entries"].as_array().unwrap().iter().filter_map(|s| s["label"].as_str()).collect();
    assert_eq!(labels.iter().filter(|l| **l == "Two edits").count(), 1, "{hist}");
    let audit = c.request("resources/read", json!({ "uri": "app://activity" }))["contents"][0]["text"]
        .as_str()
        .unwrap()
        .to_owned();
    let new: Vec<Value> = audit.lines().skip(audit_before).map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(new.iter().filter(|e| e["tool"] == "batch").count(), 1, "{new:?}");
    // A failing step rolls everything back; jobs are refused inside atomic batches.
    let failed = c.call(
        "batch",
        json!({ "label": "Fails", "steps": [
            { "id": "doc.object.patch", "params": { "targets": [scene], "props": { "meta": { "c": 3 } } } },
            { "id": "doc.object.patch", "params": { "targets": ["el_nope"], "props": {} } },
        ] }),
    );
    assert_eq!(failed["structuredContent"]["status"], "error");
    assert_eq!(h.snapshot().unwrap().unwrap().document_rev(), before + 1, "rolled back");
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn session_limits_cap_batches_and_jobs() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(user(&h, "agent.policy.defaults.set", json!({ "maxBatchSteps": 2, "maxJobs": 1 })).is_ok());
    let (mut c, _) = Client::connect(&h, opts(&dir), json!({}));
    let steps: Vec<Value> = (0..3).map(|_| json!({ "id": "app.state.get", "params": {} })).collect();
    let r = c.call("batch", json!({ "label": "Big", "steps": steps }));
    assert_eq!(r["structuredContent"]["error"]["code"], "limit.exceeded");
    assert_eq!(r["structuredContent"]["error"]["details"]["limit"], "maxBatchSteps");
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn scopes_roots_and_policies_are_enforced() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    let mut o = opts(&dir);
    o.policy = Some(Policy::Observe);
    let (mut c, init) = Client::connect(&h, o, json!({}));
    assert_eq!(init["_meta"]["toontown"]["policy"], "observe");
    let denied = c.run("project.create", json!({ "name": "X", "path": path(&dir.join("X.animpkg")) }));
    assert_eq!(denied["error"]["code"], "permission.denied", "{denied}");
    let app = c.run("agent.server.set", json!({ "enabled": false }));
    assert_eq!(app["error"]["code"], "permission.denied", "the app scope is never an agent's");
    c.close().unwrap();
    // Roots: a path outside them is refused.
    let (mut c, _) = Client::connect(&h, opts(&dir), json!({}));
    let outside = std::env::temp_dir().join(format!("tt-c14-outside-{}.animpkg", tt_persist::fsx::random_token(4)));
    let env = c.run("project.create", json!({ "name": "O", "path": path(&outside) }));
    assert_eq!(env["error"]["code"], "fs.outsideRoots", "{env}");
    assert!(!outside.exists());
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn confirmation_protocol_with_token_and_with_elicitation() {
    let (dir, _h, mut c, _scene) = with_project(json!({}));
    let target = dir.join("Copy.animpkg");
    std::fs::write(&target, b"existing").unwrap();
    let params = json!({ "path": path(&target), "onExists": "overwrite" });
    let asked = c.call("run_command", json!({ "id": "project.saveAs", "params": params }));
    let env = &asked["structuredContent"];
    assert_eq!(env["status"], "needs_confirmation", "{env}");
    assert_eq!(asked["isError"], false);
    let token = env["result"]["confirmToken"].as_str().unwrap().to_owned();
    assert!(asked["content"][0]["text"].as_str().unwrap().contains(&token));
    assert_eq!(std::fs::read(&target).unwrap(), b"existing", "nothing changed while asking");
    // Another call (another existing file) cannot use the token.
    std::fs::write(dir.join("Other.animpkg"), b"other").unwrap();
    let other = c.call("run_command", json!({ "id": "project.saveAs", "params": { "path": path(&dir.join("Other.animpkg")), "onExists": "overwrite" }, "confirmToken": token }));
    assert_eq!(other["structuredContent"]["error"]["code"], "confirmation.expired", "{other}");
    // A fresh token, used by the same call, proceeds once.
    let t2 = c.call("run_command", json!({ "id": "project.saveAs", "params": params }))["structuredContent"]["result"]
        ["confirmToken"]
        .as_str()
        .unwrap()
        .to_owned();
    let done = c.call("run_command", json!({ "id": "project.saveAs", "params": params, "confirmToken": t2 }));
    assert_eq!(done["structuredContent"]["status"], "ok", "{done}");
    c.close().unwrap();
    // Elicitation: a client that offers it is asked; accept proceeds, decline refuses.
    for (answer, expect) in [
        (json!({ "action": "accept", "content": { "confirm": true } }), "ok"),
        (json!({ "action": "decline" }), "error"),
    ] {
        let (dir, _h, mut c, _scene) = with_project(json!({ "elicitation": {} }));
        let target = dir.join("Copy.animpkg");
        std::fs::write(&target, b"existing").unwrap();
        let a = answer.clone();
        c.respond_with(move |method, _| if method == "elicitation/create" { a.clone() } else { json!({}) });
        let r = c.call(
            "run_command",
            json!({ "id": "project.saveAs", "params": { "path": path(&target), "onExists": "overwrite" } }),
        );
        assert!(
            c.asked
                .iter()
                .any(|(m, p)| m == "elicitation/create" && p["requestedSchema"]["properties"]["confirm"].is_object()),
            "{:?}",
            c.asked
        );
        assert_eq!(r["structuredContent"]["status"], expect, "{r}");
        if expect == "error" {
            assert_eq!(r["structuredContent"]["error"]["code"], "confirmation.declined");
            assert_eq!(std::fs::read(&target).unwrap(), b"existing");
        }
        c.close().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_external_change_reaches_the_agent_as_a_confirmation() {
    let (dir, _h, mut c, scene) = with_project(json!({}));
    let pkg = dir.join("P.animpkg");
    c.call("set_properties", json!({ "targets": [scene], "props": { "meta": { "n": 1 } } }));
    let t = std::time::SystemTime::now() + Duration::from_secs(30);
    std::fs::File::options().write(true).open(&pkg).unwrap().set_modified(t).unwrap();
    let r = c.call("run_command", json!({ "id": "project.save" }));
    assert_eq!(r["structuredContent"]["status"], "needs_confirmation", "{r}");
    assert_eq!(r["structuredContent"]["result"]["category"], "externalChange");
    let token = r["structuredContent"]["result"]["confirmToken"].as_str().unwrap();
    let saved = c.call("run_command", json!({ "id": "project.save", "confirmToken": token }));
    assert_eq!(saved["structuredContent"]["status"], "ok", "{saved}");
    let job = saved["structuredContent"]["result"]["jobId"].as_str().unwrap();
    let waited = c.call("job_wait", json!({ "jobId": job, "timeoutMs": 30000 }));
    assert_eq!(waited["structuredContent"]["status"], "ok", "{waited}");
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resources_templates_subscriptions_coalescing_and_events() {
    let (dir, _h, mut c, scene) = with_project(json!({}));
    let list = c.request("resources/list", json!({}))["resources"].as_array().unwrap().len();
    assert_eq!(list, 8);
    assert_eq!(c.request("resources/templates/list", json!({}))["resourceTemplates"].as_array().unwrap().len(), 10);
    let obj = c.request("resources/read", json!({ "uri": format!("app://object/{scene}") }));
    let v: Value = serde_json::from_str(obj["contents"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(v["object"]["id"], scene.as_str());
    let schema = c.request("resources/read", json!({ "uri": "app://schema/scene" }));
    assert!(schema["contents"][0]["text"].as_str().unwrap().contains("properties"));
    let cmd = c.request("resources/read", json!({ "uri": "app://command/doc.object.patch" }));
    assert!(cmd["contents"][0]["text"].as_str().unwrap().contains("paramsSchema"));
    let docs = c.request("resources/read", json!({ "uri": "app://docs/doc" }));
    assert!(docs["contents"][0]["text"].as_str().unwrap().contains("## `doc.object.patch`"));
    // Subscriptions: updates for the subscribed object, coalesced to one per 100 ms.
    c.request("resources/subscribe", json!({ "uri": format!("app://object/{scene}") }));
    c.request("resources/subscribe", json!({ "uri": "app://object/el_unrelated" }));
    let start = Instant::now();
    for i in 0..30 {
        c.call("set_properties", json!({ "targets": [scene], "props": { "meta": { "i": i } } }));
    }
    let span = start.elapsed();
    c.drain(Duration::from_millis(400));
    let updates: Vec<Value> = c.notes_of("notifications/resources/updated");
    let mine = updates.iter().filter(|u| u["params"]["uri"] == format!("app://object/{scene}")).count();
    assert!(mine >= 1, "{updates:?}");
    let bound = usize::try_from(span.as_millis() / 100).unwrap() + 3;
    assert!(mine <= bound, "coalesced: {mine} updates in {span:?}");
    assert!(updates.iter().all(|u| u["params"]["uri"] != "app://object/el_unrelated"));
    c.request("resources/unsubscribe", json!({ "uri": format!("app://object/{scene}") }));
    c.notes.clear();
    c.call("set_properties", json!({ "targets": [scene], "props": { "meta": { "z": 1 } } }));
    c.drain(Duration::from_millis(300));
    assert!(c.notes_of("notifications/resources/updated").is_empty());
    // Events with sequence numbers and origins.
    let ev = c.request("resources/read", json!({ "uri": "app://events?after=0" }));
    let body: Value = serde_json::from_str(ev["contents"][0]["text"].as_str().unwrap()).unwrap();
    let events = body["events"].as_array().unwrap();
    assert!(events.iter().any(|e| e["type"] == "doc.changed"), "{body}");
    let last = body["lastSeq"].as_u64().unwrap();
    let after = c.request("resources/read", json!({ "uri": format!("app://events?after={last}") }));
    let none: Value = serde_json::from_str(after["contents"][0]["text"].as_str().unwrap()).unwrap();
    assert!(none["events"].as_array().unwrap().is_empty());
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn prompts_completions_and_logging() {
    let (dir, _h, mut c, scene) = with_project(json!({}));
    let prompts = c.request("prompts/list", json!({}))["prompts"].as_array().unwrap().clone();
    assert_eq!(prompts.len(), 10);
    let p = c.request(
        "prompts/get",
        json!({ "name": "draw_and_paint", "arguments": { "element": "E", "frame": "1", "description": "a ball" } }),
    );
    assert!(p["messages"][0]["content"]["text"].as_str().unwrap().contains("a ball"));
    let missing = c.message("prompts/get", json!({ "name": "draw_and_paint", "arguments": {} }));
    assert_eq!(missing["error"]["code"], -32602);
    let done = c.request(
        "completion/complete",
        json!({ "ref": { "type": "ref/prompt", "name": "orient" }, "argument": { "name": "commandId", "value": "doc.object.p" } }),
    );
    assert!(done["completion"]["values"].as_array().unwrap().iter().any(|v| v == "doc.object.patch"), "{done}");
    let scenes = c.request(
        "completion/complete",
        json!({ "ref": { "type": "ref/resource", "uri": "app://scene/{sceneId}/xsheet" }, "argument": { "name": "sceneId", "value": "" } }),
    );
    assert!(scenes["completion"]["values"].as_array().unwrap().iter().any(|v| *v == scene), "{scenes}");
    assert_eq!(c.message("logging/setLevel", json!({ "level": "loud" }))["error"]["code"], -32602);
    c.request("logging/setLevel", json!({ "level": "info" }));
    c.call("set_properties", json!({ "targets": [scene], "props": { "meta": { "log": 1 } } }));
    c.drain(Duration::from_millis(300));
    assert!(c.notes_of("notifications/message").iter().any(|m| m["params"]["data"]["type"] == "doc.changed"));
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn progress_and_cancellation() {
    let (dir, h, mut c, scene) = with_project(json!({}));
    c.call("set_properties", json!({ "targets": [scene], "props": { "meta": { "p": 1 } } }));
    let export = c.run("doc.package.export", json!({ "path": path(&dir.join("E.animpkg")) }));
    let job = export["result"]["jobId"].as_str().unwrap().to_owned();
    let r = c.request(
        "tools/call",
        json!({ "name": "job_wait", "arguments": { "jobId": job, "timeoutMs": 30000 }, "_meta": { "progressToken": "pt-1" } }),
    );
    assert_eq!(r["structuredContent"]["status"], "ok", "{r}");
    c.drain(Duration::from_millis(100));
    let progress = c.notes_of("notifications/progress");
    assert!(
        progress
            .iter()
            .any(|p| p["params"]["progressToken"] == "pt-1" && p["params"]["progress"].as_f64() == Some(1000.0)),
        "{progress:?}"
    );
    assert_eq!(
        progress.last().unwrap()["params"]["message"],
        "finished",
        "a final notification ends every wait: {progress:?}"
    );
    // Cancellation: a call waiting on a paused session is cancelled and never answered.
    let session = c.call("get_state", json!({}))["structuredContent"]["result"]["agent"]["sessionId"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(user(&h, "agent.session.pause", json!({ "sessionId": session })).is_ok());
    let id = c.send("tools/call", json!({ "name": "get_state", "arguments": {} }));
    std::thread::sleep(Duration::from_millis(50));
    c.notify("notifications/cancelled", json!({ "requestId": id, "reason": "test" }));
    assert!(c.wait(id, Duration::from_millis(800)).is_none(), "a cancelled request gets no response");
    assert_eq!(c.request("ping", json!({})), json!({}));
    // Pause: calls wait, then fail with agent.paused; resumed calls run.
    let paused = c.call("get_state", json!({}));
    assert_eq!(paused["structuredContent"]["error"]["code"], "agent.paused", "{paused}");
    assert!(user(&h, "agent.session.resume", json!({ "sessionId": session })).is_ok());
    assert_eq!(c.call("get_state", json!({}))["structuredContent"]["status"], "ok");
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn typed_tools_change_the_list_and_disconnect_ends_the_session() {
    let (dir, h, mut c, scene) = with_project(json!({}));
    assert!(user(&h, "agent.typedTools.set", json!({ "enabled": true, "namespaces": ["doc"] })).is_ok());
    c.drain(Duration::from_millis(600));
    assert!(!c.notes_of("notifications/tools/list_changed").is_empty());
    let tools = c.request("tools/list", json!({}))["tools"].as_array().unwrap().clone();
    assert!(tools.iter().any(|t| t["name"] == "doc_object_get"));
    assert!(!tools.iter().any(|t| t["name"] == "edit_undo"), "only the doc namespace");
    let r = c.call("doc_object_get", json!({ "ref": scene }));
    assert_eq!(r["structuredContent"]["status"], "ok", "{r}");
    let session = c.call("get_state", json!({}))["structuredContent"]["result"]["agent"]["sessionId"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(user(&h, "agent.session.disconnect", json!({ "sessionId": session })).is_ok());
    let deadline = Instant::now() + Duration::from_secs(5);
    while !c.server_finished() && Instant::now() < deadline {
        c.drain(Duration::from_millis(50));
    }
    assert!(c.server_finished(), "the server ends when the user disconnects the session");
    assert!(c.notes_of("notifications/message").iter().any(|m| m["params"]["level"] == "warning"));
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn roots_from_the_client_extend_the_grant() {
    let dir = tmp();
    let other = tmp();
    let h = executor(&dir.join("user"));
    let mut c = Client::start(&h, opts(&dir));
    let uri = format!("file:///{}", path(&other).replace('\\', "/").trim_start_matches('/'));
    let u = uri.clone();
    c.respond_with(
        move |m, _| if m == "roots/list" { json!({ "roots": [{ "uri": u, "name": "other" }] }) } else { json!({}) },
    );
    c.request("initialize", json!({ "protocolVersion": "2025-06-18", "capabilities": { "roots": {} }, "clientInfo": { "name": "t", "version": "1" } }));
    c.notify("notifications/initialized", json!({}));
    c.drain(Duration::from_millis(400));
    assert!(c.asked.iter().any(|(m, _)| m == "roots/list"), "{:?}", c.asked);
    let env = c.run("project.create", json!({ "name": "R", "path": path(&other.join("R.animpkg")) }));
    assert_eq!(env["status"], "ok", "{uri}: {env}");
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&other);
}
