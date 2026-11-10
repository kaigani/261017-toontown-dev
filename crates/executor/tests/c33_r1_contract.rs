//! Independent C3.3 round-one contract probes. Finding tests deliberately remain failing.
mod common;

use common::{fixture, ok};
use serde_json::json;

#[test]
fn every_query_preserves_document_history_and_app_state() {
    let h = fixture();
    let before = ok(&h, "app.state.get", json!({}));
    let history = ok(&h, "edit.history.get", json!({}));
    let rev = common::rev(&h);
    let events = h.subscribe();
    for (command, params) in [
        ("render.frame.run", json!({"width":64})),
        ("render.limits.get", json!({})),
        ("inspect.frame.describe", json!({"detail":"full"})),
        ("inspect.hitTest", json!({"space":"image","point":{"x":960,"y":540}})),
        ("inspect.coordinates.convert", json!({"from":"image","to":"scene","points":[{"x":0,"y":0}]})),
        ("inspect.contactSheet.render", json!({"frames":[1,3],"cellWidth":32})),
    ] {
        ok(&h, command, params);
        assert_eq!(common::rev(&h), rev, "{command}");
        assert_eq!(ok(&h, "app.state.get", json!({})), before, "{command}");
        assert_eq!(ok(&h, "edit.history.get", json!({})), history, "{command}");
        assert!(events.try_iter().next().is_none(), "{command} emitted an event");
    }
}

#[test]
fn every_query_honors_stale_expect_rev() -> Result<(), Box<dyn std::error::Error>> {
    let h = fixture();
    let object = common::get(&h, "scene:\"Intro\"/element:\"Bob\"");
    let id = object["id"].as_str().ok_or("missing element id")?.to_owned();
    for (command, params) in [
        ("render.frame.run", json!({"width":32})),
        ("render.limits.get", json!({})),
        ("inspect.frame.describe", json!({})),
        ("inspect.hitTest", json!({"space":"image","point":{"x":0,"y":0}})),
        ("inspect.coordinates.convert", json!({"from":"image","to":"scene","points":[{"x":0,"y":0}]})),
        ("inspect.contactSheet.render", json!({"frames":[1],"cellWidth":32})),
    ] {
        let response = common::with(
            &h,
            command,
            params,
            tt_executor::InvokeOptions {
                expect_rev: [(id.clone(), 99999)].into_iter().collect(),
                ..Default::default()
            },
        );
        assert_eq!(response.error.as_ref().map(|e| e.code.as_str()), Some("rev.conflict"), "{command}: {response:?}");
    }
    Ok(())
}

#[test]
fn fractional_inspection_frame_is_a_typed_registry_refusal() {
    let h = fixture();
    let refusal = common::err(
        &h,
        "inspect.coordinates.convert",
        json!({"from":"scene","to":"image","frame":1.5,"points":[{"x":0,"y":0}]}),
    );
    assert_eq!(
        refusal.code, "arg.invalid",
        "registered frame is integer; fractional readiness wording is not established"
    );
}

#[test]
fn wait_true_returns_completed_manifest() {
    let h = fixture();
    let dir = std::env::temp_dir().join(format!("c33-r1-wait-{}", std::process::id()));
    let result = ok(
        &h,
        "render.range.run",
        json!({"from":1,"to":1,"width":32,"directory":dir,"createDirs":true,"onConflict":"rename","wait":true}),
    );
    // Settle the job even when the command incorrectly returned immediately.
    if result.get("jobId").is_some() {
        ok(&h, "job.wait", json!({"jobId":result["jobId"],"timeoutMs":60000}));
    }
    assert_eq!(result["frameCount"], 1, "09 section 11.1 item 4 requires the final result for wait:true: {result}");
}

#[test]
fn failed_sequence_publication_leaves_no_frames() -> Result<(), Box<dyn std::error::Error>> {
    let h = fixture();
    let dir = std::env::temp_dir().join(format!("c33-r1-publication-{}", tt_persist::fsx::random_token(6)));
    std::fs::create_dir_all(dir.join("shot-2.png"))?;
    std::fs::write(dir.join("shot-2.png/keep.txt"), b"existing unrelated content")?;
    let started = ok(
        &h,
        "render.range.run",
        json!({"from":1,"to":2,"width":32,"directory":dir,"namePattern":"shot-{frame}","onConflict":"overwrite"}),
    );
    let record = ok(&h, "job.wait", json!({"jobId":started["jobId"],"timeoutMs":60000}));
    assert_eq!(record["state"], "failed", "expected an unreplaceable directory to refuse publication: {record}");
    assert!(
        !dir.join("shot-1.png").exists(),
        "09 section 11.1 item 4 / ADR-0020 decision 8: failed delivery published frame 1 in {}",
        dir.display()
    );
    Ok(())
}

#[test]
fn range_snapshot_does_not_observe_a_later_edit() -> Result<(), Box<dyn std::error::Error>> {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            tt_vector::work::hooks::set_spin_micros(0);
        }
    }
    let h = fixture();
    let dir = std::env::temp_dir().join(format!("c33-r1-snapshot-{}", tt_persist::fsx::random_token(6)));
    let bob = "scene:\"Intro\"/element:\"Bob\"";
    let expected: Vec<_> =
        (1..=8).map(|frame| ok(&h, "render.frame.run", json!({"frame":frame,"width":64}))["sha256"].clone()).collect();
    let _reset = Reset;
    tt_vector::work::hooks::set_spin_micros(20_000);
    let started = ok(&h, "render.range.run", json!({"from":1,"to":8,"width":64,"directory":dir,"createDirs":true}));
    ok(&h, "anim.transform.set", json!({"nodeIds":[bob],"target":"static","position":{"x":100.0}}));
    let after = ok(&h, "render.frame.run", json!({"frame":1,"width":64}))["sha256"].clone();
    assert_ne!(expected[0], after, "edit must actually alter pixels");
    let id = started["jobId"].as_str().ok_or("missing job id")?;
    let (in_flight, _) = h.jobs().wait(id, std::time::Duration::ZERO).map_err(|e| format!("{e:?}"))?;
    assert!(
        matches!(in_flight.state, tt_executor::JobState::Queued | tt_executor::JobState::Running),
        "snapshot probe must edit before delivery: {in_flight:?}"
    );
    tt_vector::work::hooks::set_spin_micros(0);
    let record = ok(&h, "job.wait", json!({"jobId":started["jobId"],"timeoutMs":60000}));
    assert_eq!(record["state"], "succeeded", "{record}");
    let files = record["result"]["files"].as_array().ok_or("missing manifest files")?;
    for file in files {
        let index = file["frame"].as_u64().ok_or("missing frame")? as usize - 1;
        assert_eq!(file["sha256"], expected[index], "snapshot frame {}", file["frame"]);
    }
    Ok(())
}

#[test]
fn sub_microfield_depth_difference_uses_timeline_order() {
    let h = fixture();
    for name in ["Bob", "Background"] {
        ok(
            &h,
            "element.visibility.set",
            json!({"elementIds":[format!("scene:\"Intro\"/element:\"{name}\"")],"scope":"scene","visible":false}),
        );
    }
    let mut ids = Vec::new();
    for (name, z) in [("First", 0.00000051), ("Second", 0.00000049)] {
        let r = ok(&h, "element.create", json!({"type":"drawing","name":name,"sceneId":"scene:\"Intro\""}));
        let id = r["elementIds"][0].clone();
        ok(
            &h,
            "draw.rectangle.add",
            json!({"elementId":id,"frame":1,"from":{"x":-100,"y":-100},"to":{"x":100,"y":100},"layer":"new"}),
        );
        ok(&h, "anim.transform.set", json!({"nodeIds":[id],"target":"static","position":{"z":z}}));
        ids.push(id);
    }
    let first = ok(&h, "inspect.frame.describe", json!({"frame":1,"detail":"full"}));
    // Swapping depths across a rounding boundary, while remaining within the specified tie tolerance,
    // must not change draw order. Compare the entire visible-id sequence, independently of tree insertion.
    for (id, z) in ids.iter().zip([0.00000049, 0.00000051]) {
        ok(&h, "anim.transform.set", json!({"nodeIds":[id],"target":"static","position":{"z":z}}));
    }
    let second = ok(&h, "inspect.frame.describe", json!({"frame":1,"detail":"full"}));
    let visible_ids = |v: &serde_json::Value| -> Vec<serde_json::Value> {
        v["elements"].as_array().into_iter().flatten().map(|e| e["elementId"].clone()).collect()
    };
    assert!(!visible_ids(&first).is_empty(), "description shape: {first}");
    assert_eq!(visible_ids(&first), visible_ids(&second), "05 section 2.8: depths differ by less than 1e-6 field");
}
