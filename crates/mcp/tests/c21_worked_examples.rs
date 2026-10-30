//! M2 C2.1 obligation 1 (C14-R1-16, 11 §13 "docs resources include a worked end-to-end example per namespace"):
//! a worked end-to-end example for each kernel namespace — project, doc, edit, checkpoint, job, agent, app — is run
//! through the real MCP server by an agent session, and every request with its executed response is retained in
//! `crates/mcp/src/worked-examples.json`, which `app://docs/<namespace>` renders under "## Worked example".
//!
//! Checking (the default): every scenario runs again; each `run_command` result must satisfy its command's result
//! schema and each response the envelope schema; each response must reproduce the retained one except at the
//! pointers recorded as varying (generated ids, times, tokens). CI runs this on Windows and macOS. Recording:
//! `TT_RECORD_EXAMPLES=1 cargo test -p tt-mcp --test c21_worked_examples`.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "test")]

#[path = "../../executor/tests/common/recorded.rs"]
mod recorded;
mod support;

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use support::*;

fn worked_file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/worked-examples.json")
}

/// One step: a tool call and its arguments (paths under the run's folder written as given; `{dir}` is replaced).
struct Step {
    note: &'static str,
    tool: &'static str,
    args: Value,
}

fn step(note: &'static str, tool: &'static str, args: Value) -> Step {
    Step { note, tool, args }
}

fn rc(note: &'static str, id: &str, params: Value) -> Step {
    step(note, "run_command", json!({ "id": id, "params": params }))
}

struct Scenario {
    ns: &'static str,
    title: &'static str,
    steps: Vec<Step>,
}

fn scenarios() -> Vec<Scenario> {
    let create = || {
        rc(
            "Create a project (it becomes the open project)",
            "project.create",
            json!({ "name": "Demo", "path": "{dir}/Demo.animpkg" }),
        )
    };
    vec![
        Scenario {
            ns: "project",
            title: "Create, inspect, save and close a project",
            steps: vec![
                create(),
                rc("Read its summary", "project.info.get", json!({})),
                rc("Change its frame rate", "project.properties.set", json!({ "frameRate": 25 })),
                rc("Save it (a job)", "project.save", json!({})),
                step("Wait for the save", "job_wait", json!({ "jobId": "$job", "timeoutMs": 30000 })),
                rc("Close it", "project.close", json!({})),
                rc("It is now in the recent list", "project.recent.list", json!({})),
            ],
        },
        Scenario {
            ns: "doc",
            title: "Find, read, patch, diff and validate objects",
            steps: vec![
                create(),
                rc("Find the scenes", "doc.object.find", json!({ "type": "scene" })),
                rc(
                    "Read the first scene with its elements as stubs",
                    "doc.object.get",
                    json!({ "ref": "scene:#1", "depth": 1 }),
                ),
                rc(
                    "Annotate it in meta",
                    "doc.object.patch",
                    json!({ "targets": ["scene:#1"], "props": { "meta": { "status": "layout" } } }),
                ),
                rc("What changed since revision 1", "doc.diff", json!({ "sinceRev": 1 })),
                rc("Validate the document (errors only)", "doc.validate", json!({ "minSeverity": "error" })),
            ],
        },
        Scenario {
            ns: "edit",
            title: "Undo and redo an edit, reading the history",
            steps: vec![
                create(),
                rc(
                    "Edit the first scene",
                    "doc.object.patch",
                    json!({ "targets": ["scene:#1"], "props": { "meta": { "take": 1 } }, "label": "Mark take" }),
                ),
                rc("Read the history", "edit.history.get", json!({})),
                rc("Undo the edit", "edit.undo", json!({})),
                rc("Redo it", "edit.redo", json!({})),
            ],
        },
        Scenario {
            ns: "checkpoint",
            title: "Try an edit and return to a checkpoint",
            steps: vec![
                create(),
                rc("Checkpoint before experimenting", "checkpoint.create", json!({ "name": "before" })),
                rc(
                    "Experiment",
                    "doc.object.patch",
                    json!({ "targets": ["scene:#1"], "props": { "meta": { "experiment": true } } }),
                ),
                rc("List checkpoints", "checkpoint.list", json!({})),
                rc("Restore the checkpoint (one undoable step)", "checkpoint.restore", json!({ "name": "before" })),
                rc("Delete it", "checkpoint.delete", json!({ "name": "before" })),
            ],
        },
        Scenario {
            ns: "job",
            title: "Start a job, wait for it and list jobs",
            steps: vec![
                create(),
                rc("Saving is a job: the call returns its id at once", "project.save", json!({})),
                rc("Wait for it", "job.wait", json!({ "jobId": "$job", "timeoutMs": 30000 })),
                rc("The caller's jobs", "job.list", json!({})),
            ],
        },
        Scenario {
            ns: "agent",
            title: "Inspect the server, then read and export this session's activity",
            steps: vec![
                create(),
                rc(
                    "Edit something",
                    "doc.object.patch",
                    json!({ "targets": ["scene:#1"], "props": { "meta": { "by": "agent" } } }),
                ),
                rc("Endpoint and sessions", "agent.server.status", json!({})),
                rc("The activity of this session", "agent.activity.list", json!({ "limit": 5 })),
                rc(
                    "Export the committed edits as a replayable batch",
                    "agent.activity.export",
                    json!({ "path": "{dir}/activity.json", "format": "batch" }),
                ),
            ],
        },
        Scenario {
            ns: "app",
            title: "Read the application state and move the context",
            steps: vec![
                create(),
                rc("Application information", "app.info.get", json!({})),
                rc(
                    "Go to frame 12 of the first scene",
                    "app.context.set",
                    json!({ "sceneId": "scene:#1", "frame": 12 }),
                ),
                rc("Step one frame back", "app.frame.move", json!({ "direction": "previous" })),
                rc("The state now", "app.state.get", json!({})),
            ],
        },
    ]
}

fn fill(v: &mut Value, dir: &Path, job: Option<&str>) {
    match v {
        Value::String(s) if s.contains("{dir}") => *s = s.replace("{dir}", &path(dir)),
        Value::String(s) if s == "$job" => {
            if let Some(j) = job {
                *s = j.to_owned();
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| fill(x, dir, job)),
        Value::Object(m) => m.values_mut().for_each(|x| fill(x, dir, job)),
        _ => {}
    }
}

/// Runs a scenario: per step `(tool, arguments, response)` with portable paths, and schema problems.
fn execute(s: &Scenario) -> (Vec<(Value, Value)>, Vec<String>) {
    let registry = tt_executor::registry().unwrap();
    let envelope = tt_registry::schema_of::<tt_executor::Envelope>();
    let dir = tmp();
    let h = executor(&dir.join("user"));
    let (mut c, _) = Client::connect(&h, opts(&dir), json!({}));
    let mut job: Option<String> = None;
    let mut out = Vec::new();
    let mut problems = Vec::new();
    for st in &s.steps {
        let mut args = st.args.clone();
        fill(&mut args, &dir, job.as_deref());
        let r = c.call(st.tool, args.clone());
        let mut env = r["structuredContent"].clone();
        if env["status"] != "ok" {
            problems.push(format!("{} / {}: {env}", s.ns, st.note));
        }
        if let Some(j) = env["result"]["jobId"].as_str() {
            job = Some(j.to_owned());
        }
        for e in recorded::schema_errors(&envelope, &env) {
            problems.push(format!("{} / {}: envelope {e}", s.ns, st.note));
        }
        if st.tool == "run_command" {
            let id = args["id"].as_str().unwrap();
            let schema = registry.get(id).unwrap().result_schema();
            for e in recorded::schema_errors(&schema, &env["result"]) {
                problems.push(format!("{} / {}: {id} result {e}", s.ns, st.note));
            }
        }
        recorded::portable_paths(&mut args, &dir);
        recorded::portable_paths(&mut env, &dir);
        out.push((json!({ "tool": st.tool, "arguments": args }), env));
    }
    c.close().unwrap();
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
    (out, problems)
}

#[test]
fn every_kernel_namespace_has_a_retained_worked_example() {
    let mut file = Map::new();
    let stored = if recorded::recording() { Value::Null } else { recorded::read_json(&worked_file()) };
    let mut problems = Vec::new();
    for s in scenarios() {
        let (first, p) = execute(&s);
        problems.extend(p);
        if recorded::recording() {
            let (second, p) = execute(&s);
            problems.extend(p);
            let steps: Vec<Value> = s
                .steps
                .iter()
                .zip(first.iter().zip(&second))
                .map(|(st, ((req, a), (_, b)))| {
                    let mut varies = Vec::new();
                    recorded::differences(a, b, "", &mut varies);
                    json!({ "note": st.note, "request": req, "response": a, "varies": varies })
                })
                .collect();
            file.insert(s.ns.to_owned(), json!({ "title": s.title, "steps": steps }));
            continue;
        }
        let Some(rec) = stored.get(s.ns) else {
            problems.push(format!("{}: no retained worked example", s.ns));
            continue;
        };
        let rec_steps = rec["steps"].as_array().unwrap();
        if rec_steps.len() != first.len() {
            problems.push(format!("{}: {} retained steps, {} executed", s.ns, rec_steps.len(), first.len()));
            continue;
        }
        for (i, ((req, live), r)) in first.iter().zip(rec_steps).enumerate() {
            let varies: Vec<String> =
                r["varies"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_owned()).collect();
            // A request carries a job id only where the previous step's (varying) answer gave it.
            let mut req_varies = Vec::new();
            recorded::differences(&r["request"], req, "", &mut req_varies);
            if req_varies.iter().any(|p| !p.ends_with("/jobId")) {
                problems.push(format!("{} step {i}: request differs at {req_varies:?}", s.ns));
            }
            if let Err(e) = recorded::matches(&r["response"], live, &varies) {
                problems.push(format!("{} step {i} ({}): {e}", s.ns, r["note"]));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    if recorded::recording() {
        recorded::write_json(&worked_file(), &Value::Object(file));
    }
}

/// The docs resource of each kernel namespace shows its worked example with requests and executed responses.
#[test]
fn namespace_docs_render_the_retained_worked_examples() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    let (mut c, _) = Client::connect(&h, opts(&dir), json!({}));
    for ns in ["project", "doc", "edit", "checkpoint", "job", "agent", "app"] {
        let doc = c.request("resources/read", json!({ "uri": format!("app://docs/{ns}") }));
        let text = doc["contents"][0]["text"].as_str().unwrap();
        assert!(text.contains("## Worked example"), "{ns}: no worked example");
        assert!(text.contains("\"status\": \"ok\""), "{ns}: no executed response");
    }
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
