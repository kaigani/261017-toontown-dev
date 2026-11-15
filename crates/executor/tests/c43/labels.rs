//! `anim.transform.set`'s undo labels (05 §17 line 1135: "the operation and the object kind"; SI-144), as a full
//! table over the changed groups and the node kinds (ruling D-27: each cell states the expected label and would fail
//! if the rule were ignored, the earlier fixed label being "Transform").

use serde_json::json;

use crate::common::*;

const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
const PEG: &str = "scene:\"Intro\"/element:\"Bob-P\"";
const CAMERA: &str = "scene:\"Intro\"/element:\"Camera\"";

fn label_of(h: &tt_executor::ExecutorHandle) -> serde_json::Value {
    let hist = ok(h, "edit.history.get", json!({}));
    let p = hist["position"].as_u64().unwrap() as usize;
    hist["entries"][p - 1]["label"].clone()
}

#[test]
fn every_operation_and_node_kind_names_its_label() {
    let rows = [
        (json!({ "nodeIds": [BOB], "target": "static", "position": { "x": 1 } }), "Move Element"),
        (json!({ "nodeIds": [BOB], "target": "static", "rotation": 10 }), "Rotate Element"),
        (json!({ "nodeIds": [BOB], "target": "static", "scale": { "x": 1.5 } }), "Scale Element"),
        (json!({ "nodeIds": [BOB], "target": "static", "skew": 5 }), "Skew Element"),
        (json!({ "nodeIds": [BOB], "target": "static", "rotationPivot": { "x": 1, "y": 0 } }), "Move Pivot Element"),
        (json!({ "nodeIds": [BOB], "target": "static", "position": { "x": 1 }, "rotation": 10 }), "Transform Element"),
        (json!({ "nodeIds": [PEG], "frame": 3, "rotation": 15 }), "Rotate Peg"),
        (json!({ "nodeIds": [PEG], "target": "static", "pivot": { "x": 0.5, "y": 0 } }), "Move Pivot Peg"),
        (json!({ "nodeIds": [CAMERA], "target": "static", "position": { "x": 0.5 } }), "Move Camera"),
        (json!({ "nodeIds": [BOB, PEG], "target": "static", "position": { "x": 1 } }), "Move Nodes"),
    ];
    for (params, want) in rows {
        let h = fixture();
        ok(&h, "anim.transform.set", params.clone());
        assert_eq!(label_of(&h), json!(want), "{params}");
    }
    // Several nodes of one kind: plural.
    let h = fixture();
    let a = ok(&h, "element.create", json!({ "type": "drawing", "name": "A" }))["elementIds"][0].clone();
    ok(&h, "anim.transform.set", json!({ "nodeIds": [BOB, a], "target": "static", "scale": { "x": 2 } }));
    assert_eq!(label_of(&h), json!("Scale Elements"));
}
