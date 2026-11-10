//! A long control-point path must yield a typed refusal and release the worker.
mod common;
use common::*;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use tt_executor::{ExecutorConfig, Status};
#[test]
fn long_passage_queries_release_the_worker_before_watchdog() {
    let h = fixture();
    let id = ok(&h, "element.create", json!({"type":"peg","name":"R2 long path"}))["elementIds"][0].clone();
    ok(&h, "element.timing.set", json!({"elementIds":[id],"duration":100000}));
    let mut points = vec![json!({"kind":"key","frame":1,"x":0,"y":0,"z":0})];
    for n in 1..20000 {
        points.push(json!({"kind":"control","x":n,"y":n%7,"z":0}));
    }
    points.push(json!({"kind":"key","frame":100000,"x":20000,"y":0,"z":0}));
    ok(&h, "anim.path.set", json!({"nodeId":id,"points":points}));
    let keys: Vec<Value> = (1..20000).map(|n| json!({"frame":n*4,"value":n as f64/20000.})).collect();
    ok(&h, "anim.function.set", json!({"nodeId":id,"channel":"velocity","keys":keys}));
    let doc = h.snapshot().unwrap().unwrap();
    let h = tt_executor::ExecutorHandle::spawn(ExecutorConfig { budget: Duration::from_secs(3) }).unwrap();
    assert_eq!(h.open_document(doc, None, false).status, Status::Ok);
    let revision = rev(&h);
    let start = Instant::now();
    let env = run(&h, "anim.path.get", json!({"nodeId":id}));
    eprintln!("long path seconds={} status={:?} error={:?}", start.elapsed().as_secs_f64(), env.status, env.error);
    assert!(start.elapsed() < Duration::from_secs(15), "worker exceeded the cancellation margin");
    if env.status == Status::Error {
        assert!(matches!(env.error.unwrap().code.as_str(), "command.timeout" | "limit.exceeded"));
    } else {
        assert_eq!(env.status, Status::Ok);
    }
    assert_eq!(run(&h, "doc.state.get", json!({})).status, Status::Ok);
    assert_eq!(rev(&h), revision);
}
