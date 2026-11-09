//! Independent AG-02 regression for the orchestrator's ownership ruling.
mod common;
use common::*;
use serde_json::json;
#[test]
fn rotation_patch_matches_owner_and_future_owner_is_named() {
    let a = fixture();
    let b = fixture();
    let ra = a.subscribe();
    let rb = b.subscribe();
    let node = "scene:\"Intro\"/element:\"Bob\"";
    ok(&a, "doc.object.patch", json!({"targets":[node],"ops":[{"op":"replace","path":"/transform/rotation","value":17}]}));
    ok(&b, "anim.transform.set", json!({"nodeIds":[node],"target":"static","rotation":17}));
    assert_eq!(get(&a,node), get(&b,node));
    let events = |rx: std::sync::mpsc::Receiver<tt_executor::Event>| rx.try_iter().map(|e| serde_json::to_value(e).unwrap()).collect::<Vec<_>>();
    assert_eq!(events(ra),events(rb));
    let e = err(&a,"doc.object.patch",json!({"targets":[first(&a,"scene")],"props":{"visible":false}}));
    assert_eq!(e.code,"command.notImplemented");
    assert_eq!(e.details["commandId"],"storyline.scene.visible.set");
}
