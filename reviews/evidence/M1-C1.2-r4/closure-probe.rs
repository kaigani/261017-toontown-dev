//! Temporary closure QA, executed by run-probes.py; only genuine new P0/P1 cases are retained in the suite.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::print_stdout, missing_docs, reason = "independent review evidence")]
mod common;
use common::*;
use serde_json::{Value, json};
use std::sync::{Arc, Barrier, mpsc};
use std::time::Duration;
use tt_executor::{ExecutorConfig, ExecutorHandle, Invocation, InvokeOptions};

fn fresh() -> ExecutorHandle {
    let h = ExecutorHandle::spawn(ExecutorConfig { budget: Duration::from_secs(60) }).unwrap();
    assert!(h.open_document(tt_model::fixture::sample_document(0xc124_2026).unwrap(), None, false).is_ok());
    h
}

fn artifact(name: &str, value: Value) {
    let path = std::path::PathBuf::from(std::env::var("TT_C12_REVIEW_ARTIFACT_DIR").unwrap()).join(name);
    std::fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
}

#[test]
fn sixteen_identical_waiters_keep_the_first_envelope_and_conflict_on_different_params() {
    let h = fresh();
    let (release, hold) = mpsc::channel();
    let (started, ready) = mpsc::channel();
    let job = h.jobs().submit("review.r4", "user", move |_| {
        started.send(()).unwrap();
        hold.recv_timeout(Duration::from_secs(15)).unwrap();
        Ok(json!({"closure": 124, "payload": [4, 9, 16]}))
    }).unwrap();
    ready.recv_timeout(Duration::from_secs(5)).unwrap();
    let call = Invocation { command: "job.wait".into(), params: json!({"jobId":job,"timeoutMs":1500}),
        options: InvokeOptions { request_id: Some("closure-16-waiters".into()), ..Default::default() },
        origin: tt_executor::Origin::User };
    let a = h.clone(); let b = call.clone();
    let owner = std::thread::spawn(move || a.invoke(b));
    std::thread::sleep(Duration::from_millis(150));
    let mut other = call.clone(); other.params["timeoutMs"] = json!(1501);
    let conflict = h.invoke(other.clone());
    assert_eq!(conflict.error.as_ref().unwrap().code, "request.conflict");
    assert_eq!(conflict.error.as_ref().unwrap().details["inProgress"], true);
    let start = Arc::new(Barrier::new(16));
    let followers: Vec<_> = (0..15).map(|i| {
        let h = h.clone(); let call = call.clone(); let start = start.clone();
        std::thread::spawn(move || {
            start.wait(); std::thread::sleep(Duration::from_millis(i * 13)); h.invoke(call)
        })
    }).collect();
    start.wait();
    ok(&h, "doc.object.patch", json!({"targets":["scene:#1"],"props":{"meta":{"whileWaiting":124}}}));
    let first = serde_json::to_value(owner.join().unwrap()).unwrap();
    assert_eq!(first["result"]["timedOut"], true);
    release.send(()).unwrap();
    let replies: Vec<_> = followers.into_iter().map(|w| serde_json::to_value(w.join().unwrap()).unwrap()).collect();
    let complete = ok(&h, "job.wait", json!({"jobId":job,"timeoutMs":5000}));
    let cached: Vec<_> = (0..32).map(|_| serde_json::to_value(h.invoke(call.clone())).unwrap()).collect();
    let conflict_after = h.invoke(other);
    artifact("wait16.json", json!({"owner":first,"followers":replies,"cached":cached,"differentParams":conflict,
        "differentParamsAfter":conflict_after,"completedJob":complete}));
    assert!(replies.iter().chain(cached.iter()).all(|e| *e == first));
    assert_eq!(conflict_after.error.unwrap().code, "request.conflict");
}

#[test]
fn single_and_final_batch_checkpoints_are_coherent_before_and_after_later_edits() {
    let h = fresh(); let stroke = first(&h, "stroke");
    let drawing = stroke.split('/').next().unwrap();
    let mut evidence = Vec::new();
    for atomic in [false, true] {
        let name = format!("closure-final-{atomic}");
        if atomic {
            assert!(batch(&h, json!({"label":"closure final checkpoint","steps":[
                {"id":"doc.object.patch","params":{"targets":[stroke],"props":{"meta":{"closure":71}}}},
                {"id":"checkpoint.create","params":{"name":name}}]})).is_ok());
        } else {
            ok(&h, "doc.object.patch", json!({"targets":[stroke],"props":{"meta":{"closure":37}}}));
            ok(&h, "checkpoint.create", json!({"name":name}));
        }
        let immediate = ok(&h, "doc.diff", json!({"sinceCheckpoint":name}));
        assert_eq!(immediate["changes"], json!([]));
        assert_eq!(immediate["fromRev"], json!(rev(&h)));
        ok(&h, "doc.object.patch", json!({"targets":[stroke],"props":{"meta":{"later":atomic}}}));
        let later = ok(&h, "doc.diff", json!({"sinceCheckpoint":name}));
        assert!(later["changes"].as_array().unwrap().iter().any(|c| c["id"] == drawing));
        assert!(later["changes"].as_array().unwrap().iter().any(|c| c["id"] == stroke));
        evidence.push(json!({"atomic":atomic,"immediate":immediate,"later":later}));
    }
    artifact("checkpoint-standalone-final.json", json!(evidence));
}

#[test]
fn immediately_querying_a_staged_checkpoint_is_empty() {
    let h = fresh(); let stroke = first(&h, "stroke");
    let e = batch(&h, json!({"label":"closure immediate staged checkpoint","steps":[
        {"id":"doc.object.patch","params":{"targets":[stroke],"props":{"meta":{"closure":891}}}},
        {"id":"checkpoint.create","params":{"name":"closure-staged"}},
        {"id":"doc.diff","params":{"sinceCheckpoint":"closure-staged"}}
    ]}));
    artifact("checkpoint-staged-immediate.json", serde_json::to_value(&e).unwrap());
    assert!(e.is_ok(), "{e:?}");
    assert_eq!(e.steps.unwrap()[2].result.as_ref().unwrap()["changes"], json!([]),
        "a checkpoint of the current staged content must have an empty immediate diff");
}

#[test]
fn nested_checkpoints_do_not_reinvalidate_unchanged_drawing_content() {
    let h = fresh(); let stroke = first(&h, "stroke");
    let e = batch(&h, json!({"label":"closure nested checkpoint sequence","steps":[
        {"id":"doc.object.patch","params":{"targets":[stroke],"props":{"meta":{"closure":384}}}},
        {"id":"checkpoint.create","params":{"name":"outer"}},
        {"id":"doc.object.patch","params":{"targets":["scene:#1"],"props":{"meta":{"later":283}}}},
        {"id":"checkpoint.create","params":{"name":"inner"}},
        {"id":"doc.diff","params":{"sinceCheckpoint":"outer","types":["drawing"]}},
        {"id":"doc.diff","params":{"sinceCheckpoint":"inner"}}
    ]}));
    let after_inner = ok(&h, "doc.diff", json!({"sinceCheckpoint":"inner"}));
    let after_outer = ok(&h, "doc.diff", json!({"sinceCheckpoint":"outer","types":["drawing"]}));
    artifact("checkpoint-nested.json", json!({"batch":e,"afterInner":after_inner,"afterOuter":after_outer}));
    assert!(e.is_ok(), "{e:?}");
    let steps = e.steps.unwrap();
    assert_eq!(steps[4].result.as_ref().unwrap()["changes"], json!([]));
    assert_eq!(steps[5].result.as_ref().unwrap()["changes"], json!([]));
    assert_eq!(after_inner["changes"], json!([]));
    assert_eq!(after_outer["changes"], json!([]));
}

#[test]
fn drawing_invalidation_survives_the_filter_cartesian_product() {
    let h = fresh(); let stroke = first(&h, "stroke");
    let drawing = stroke.split('/').next().unwrap().to_owned();
    let bank = first(&h, "drawingBank"); let scene = first(&h, "scene");
    let mut evidence = Vec::new(); let mut counter = 0u64;
    // Every optional filter path: type inclusion/exclusion/empty, containment, detail, and truncation.
    for types in [None, Some(json!(["drawing"])), Some(json!(["drawing","stroke"])), Some(json!(["stroke"])), Some(json!([]))] {
        for within in [None, Some(drawing.clone()), Some(bank.clone()), Some(stroke.clone()), Some(scene.clone())] {
            for detail in ["summary", "full"] { for limit in [1, 1000] {
                for source in ["sinceRev", "sinceCheckpoint"] { for until_current in [false, true] {
                    counter += 1;
                    let base = rev(&h);
                    ok(&h, "checkpoint.create", json!({"name":"matrix-base"}));
                    let mut p = json!({"detail":detail,"limit":limit});
                    p[source] = if source == "sinceRev" { json!(base) } else { json!("matrix-base") };
                    if let Some(t) = &types { p["types"] = t.clone(); }
                    if let Some(w) = &within { p["within"] = json!(w); }
                    if until_current { p["untilRev"] = json!(base+1); }
                    let e = batch(&h, json!({"label":"closure filter matrix","steps":[
                        {"id":"doc.object.patch","params":{"targets":[stroke],"props":{"meta":{"matrix":counter}}}},
                        {"id":"doc.diff","params":p}
                    ]}));
                    assert!(e.is_ok(), "{p}: {e:?}");
                    let staged = e.steps.unwrap()[1].result.clone().unwrap();
                    let committed = ok(&h, "doc.diff", p.clone());
                    assert_eq!(staged, committed, "staged vs committed: {p}");
                    let include_type = types.as_ref().is_none_or(|t| t.as_array().unwrap().contains(&json!("drawing")));
                    let include_within = within.as_ref().is_none_or(|w| *w == drawing || *w == bank);
                    if include_type && include_within && (limit > 1 || types == Some(json!(["drawing"]))) {
                        assert!(staged["changes"].as_array().unwrap().iter().any(|c| c["id"] == drawing && c["rev"] == base+1), "{p}: {staged}");
                    }
                    if !include_type || !include_within {
                        assert!(!staged["changes"].as_array().unwrap().iter().any(|c| c["id"] == drawing), "{p}: {staged}");
                    }
                    evidence.push(json!({"params":p,"staged":staged,"committed":committed}));
                    ok(&h, "checkpoint.delete", json!({"name":"matrix-base"}));
                }}
            }}
        }
    }
    artifact("diff-filter-matrix.json", json!({"cases":counter,"rows":evidence}));
    assert_eq!(counter, 400);
}
