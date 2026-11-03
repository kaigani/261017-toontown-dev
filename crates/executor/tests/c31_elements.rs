//! M3 C3.1: element structure, hierarchy, flags, timing and read model (04 §3–§10, §17.4–§17.5) through the
//! executor: creation names and placement, parameter rows, clone groups, duplicate banks, delete with re-attached
//! children, both orders, parenting rules, toggles, bar timing and key re-timing, `element.get`/`element.list`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

const INTRO: &str = "scene:\"Intro\"";
const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
const PEG: &str = "scene:\"Intro\"/element:\"Bob-P\"";
const BG: &str = "scene:\"Intro\"/element:\"Background\"";
const FADE: &str = "scene:\"Intro\"/element:\"Fade\"";

fn id(h: &ExecutorHandle, r: &str) -> String {
    get(h, r)["id"].as_str().unwrap().to_owned()
}

fn names(h: &ExecutorHandle, order: &str) -> Vec<String> {
    ok(h, "element.list", json!({ "sceneId": INTRO, "order": order }))["elements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap().to_owned())
        .collect()
}

fn root_level(h: &ExecutorHandle) -> Vec<String> {
    ok(h, "element.list", json!({ "sceneId": INTRO }))["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["depth"] == 0)
        .map(|e| e["name"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn create_names_validation_and_placement() {
    let h = fixture();
    let r = ok(&h, "element.create", json!({ "type": "peg", "name": "Leg", "count": 3, "sceneId": INTRO }));
    assert_eq!(r["names"], json!(["Leg", "Leg_1", "Leg_2"]));
    // Nothing selected: top of the root level, in order.
    assert_eq!(root_level(&h)[..3], ["Leg", "Leg_1", "Leg_2"]);
    let r = ok(&h, "element.create", json!({ "type": "drawing", "sceneId": INTRO, "count": 2 }));
    assert_eq!(r["names"], json!(["Drawing", "Drawing_1"]));
    // Cell-based elements join the Exposure Sheet at the right (none selected).
    assert_eq!(names(&h, "xsheet").last().unwrap(), "Drawing_1");
    for (params, code) in [
        (json!({ "type": "peg", "name": "", "sceneId": INTRO }), "element.nameEmpty"),
        (json!({ "type": "peg", "name": "a/b", "sceneId": INTRO }), "element.nameInvalid"),
        (json!({ "type": "peg", "name": "a.b", "sceneId": INTRO }), "element.nameInvalid"),
        (json!({ "type": "peg", "name": "Bob", "sceneId": INTRO }), "element.nameDuplicate"),
        (json!({ "type": "peg", "name": "x".repeat(65), "sceneId": INTRO }), "element.nameTooLong"),
        (json!({ "type": "peg", "count": 0, "sceneId": INTRO }), "element.countOutOfRange"),
        (json!({ "type": "peg", "count": 100, "sceneId": INTRO }), "element.countOutOfRange"),
        (json!({ "type": "peg", "parentId": BOB }), "element.notParentable"),
    ] {
        assert_eq!(err(&h, "element.create", params.clone()).code, code, "{params}");
    }
    // Sound elements: a zero-padded counter.
    let r = ok(&h, "element.create", json!({ "type": "sound", "sceneId": INTRO, "count": 2 }));
    assert_eq!(r["names"], json!(["Sound-01", "Sound-02"]));
    // Under a parent and above a sibling.
    let r = ok(&h, "element.create", json!({ "type": "drawing", "name": "Hat", "parentId": PEG }));
    let hat = r["elementIds"][0].as_str().unwrap().to_owned();
    assert_eq!(ok(&h, "element.hierarchy.get", json!({ "elementId": hat }))["parentId"], id(&h, PEG));
    let r = ok(&h, "element.create", json!({ "type": "drawing", "name": "Scarf", "above": BOB }));
    let hier = ok(&h, "element.hierarchy.get", json!({ "elementId": r["elementIds"][0] }));
    assert_eq!(hier["nextSiblingId"], id(&h, BOB));
}

#[test]
fn create_is_one_undo_step_selects_and_makes_current() {
    let h = fixture();
    let before = names(&h, "timeline");
    let r = ok(&h, "element.create", json!({ "type": "clipping", "sceneId": INTRO }));
    let clip = r["elementIds"][0].as_str().unwrap().to_owned();
    // The clipping effect gets its mask parameter row (04 §2.9, 10 §4.6).
    let kids = ok(&h, "element.list", json!({ "parentId": clip, "recursive": false }))["elements"].clone();
    assert_eq!(kids[0]["type"], "effectParameter");
    assert_eq!(kids[0]["name"], "Clipping-mask");
    let state = ok(&h, "cell.selection.get", json!({}));
    assert_eq!(state["currentElementId"], clip);
    assert_eq!(state["rowsSelected"], 1);
    ok(&h, "edit.undo", json!({}));
    assert_eq!(names(&h, "timeline"), before);
    // Particles get a sprites row with a sprite drawing element; skeletons bones and skin rows.
    let r = ok(&h, "element.create", json!({ "type": "particleNature", "sceneId": INTRO }));
    let p = r["elementIds"][0].as_str().unwrap().to_owned();
    let all = ok(&h, "element.list", json!({ "parentId": p }))["elements"].clone();
    let types: Vec<&str> = all.as_array().unwrap().iter().map(|e| e["type"].as_str().unwrap()).collect();
    assert_eq!(types, ["effectParameter", "drawing"]);
    let r = ok(&h, "element.create", json!({ "type": "skeleton", "sceneId": INTRO }));
    let kids =
        ok(&h, "element.list", json!({ "parentId": r["elementIds"][0], "recursive": false }))["elements"].clone();
    assert_eq!(kids.as_array().unwrap().len(), 2);
}

#[test]
fn drawing_mode_warning_is_non_blocking() {
    let h = fixture();
    ok(&h, "app.mode.set", json!({ "mode": "drawing" }));
    let env = run(&h, "element.create", json!({ "type": "peg", "sceneId": INTRO }));
    assert!(env.is_ok());
    let r = env.result.unwrap();
    assert_eq!(r["warnings"][0]["code"], "element.hiddenInDrawingMode");
    assert!(env.warnings.iter().any(|w| w.code == "element.hiddenInDrawingMode"));
    let r2 = ok(&h, "element.create", json!({ "type": "drawing", "sceneId": INTRO }));
    assert_eq!(r2["warnings"], json!([]));
}

#[test]
fn rename_rules() {
    let h = fixture();
    let r0 = rev(&h);
    ok(&h, "element.rename", json!({ "elementId": BOB, "name": "Bob" }));
    assert_eq!(rev(&h), r0, "renaming to the current name is a silent no-op");
    assert_eq!(
        err(&h, "element.rename", json!({ "elementId": BOB, "name": "Background" })).code,
        "element.nameDuplicate"
    );
    ok(&h, "element.rename", json!({ "elementId": BOB, "name": "Robert" }));
    let hist = ok(&h, "edit.history.get", json!({}));
    assert!(hist.to_string().contains("Rename Drawing Element"), "the undo label names the type: {hist}");
    // Drawing references are unaffected: the exposure still resolves.
    assert_eq!(get(&h, "scene:\"Intro\"/element:\"Robert\"")["exposure"][0][1], 6);
}

#[test]
fn clone_shares_the_bank_and_duplicate_copies_it() {
    let h = fixture();
    let r = ok(&h, "element.clone", json!({ "elementId": BOB }));
    assert_eq!(r["name"], "Bob_1");
    let clone = r["elementId"].as_str().unwrap().to_owned();
    let bob = get(&h, BOB);
    assert_eq!(r["cloneGroupId"], bob["drawingBankId"]);
    let info = ok(&h, "element.get", json!({ "elementId": clone }));
    assert_eq!(info["cloneGroupId"], bob["drawingBankId"]);
    assert_eq!(info["cloneIds"], json!([bob["id"]]));
    // A clone of a clone joins the group; timing stays independent.
    let r2 = ok(&h, "element.clone", json!({ "elementId": clone }));
    assert_eq!(r2["cloneGroupId"], bob["drawingBankId"]);
    ok(&h, "cell.exposure.set", json!({ "elementIds": [clone], "from": 1, "to": 50, "length": 1 }));
    assert_eq!(get(&h, BOB)["exposure"][0][1], 6);
    // A new drawing in one clone is available to all (shared bank).
    ok(&h, "element.drawing.create", json!({ "elementId": clone, "name": "Z" }));
    let list = ok(&h, "element.drawing.list", json!({ "elementId": BOB }));
    assert!(list["drawings"].as_array().unwrap().iter().any(|d| d["name"] == "Z"));
    assert_eq!(err(&h, "element.clone", json!({ "elementId": PEG })).code, "element.notCloneable");

    let h = fixture();
    let r = ok(&h, "element.duplicate", json!({ "elementIds": [BOB] }));
    let dup = get(&h, r["elementIds"][0].as_str().unwrap());
    assert_ne!(dup["drawingBankId"], get(&h, BOB)["drawingBankId"]);
    let a = ok(&h, "element.drawing.list", json!({ "elementId": BOB }))["drawings"].clone();
    let b = ok(&h, "element.drawing.list", json!({ "elementId": dup["id"] }))["drawings"].clone();
    let strip = |v: &Value| {
        v.as_array()
            .unwrap()
            .iter()
            .map(|d| (d["name"].clone(), d["locked"].clone(), d["useCount"].clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(strip(&a), strip(&b), "same names, locks and timing in the copy");
    assert_eq!(dup["transform"], get(&h, BOB)["transform"], "the copy appears at exactly the same place");
}

#[test]
fn delete_reattaches_children_and_drops_unshared_banks() {
    let h = fixture();
    let bob = id(&h, BOB);
    let r = ok(&h, "element.delete", json!({ "elementIds": [PEG] }));
    assert_eq!(r["reparentedIds"], json!([bob]));
    assert_eq!(ok(&h, "element.hierarchy.get", json!({ "elementId": bob }))["parentId"], Value::Null);
    ok(&h, "edit.undo", json!({}));
    assert_eq!(ok(&h, "element.hierarchy.get", json!({ "elementId": bob }))["parentId"], id(&h, PEG));
    // A clone keeps the shared bank; deleting the last user deletes the bank and its drawings.
    let clone = ok(&h, "element.clone", json!({ "elementId": BOB }))["elementId"].as_str().unwrap().to_owned();
    let r = ok(&h, "element.delete", json!({ "elementIds": [BOB] }));
    assert_eq!(r["deletedDrawingCount"], 0);
    let r = ok(&h, "element.delete", json!({ "elementIds": [clone] }));
    assert_eq!(r["deletedDrawingCount"], 3);
    // Effects take their parameter rows; the rows alone are refused.
    let h = fixture();
    let r = ok(&h, "element.delete", json!({ "elementIds": ["scene:\"Intro\"/element:\"Clip\""] }));
    assert_eq!(r["deletedIds"].as_array().unwrap().len(), 2);
    assert_eq!(
        err(&h, "element.delete", json!({ "elementIds": ["scene:\"Intro\"/element:\"Rig-bones\""] })).code,
        "arg.invalid"
    );
}

#[test]
fn stacking_orders_are_independent() {
    let h = fixture();
    let cols = names(&h, "xsheet");
    let r = ok(&h, "element.order.move", json!({ "elementIds": [BG], "to": "front" }));
    assert!(r["changed"].as_bool().unwrap());
    assert_eq!(root_level(&h)[0], "Background");
    assert_eq!(names(&h, "xsheet"), cols, "Timeline reorder leaves the columns");
    let r = ok(&h, "element.order.move", json!({ "elementIds": [BG], "to": "front" }));
    assert!(!r["changed"].as_bool().unwrap(), "disabled at the boundary");
    ok(&h, "element.order.move", json!({ "elementIds": [BOB], "to": "front", "order": "xsheet" }));
    assert_eq!(names(&h, "xsheet").last().unwrap(), "Bob", "front = rightmost column");
    ok(&h, "element.order.move", json!({ "elementIds": [BOB], "to": "backward", "order": "xsheet" }));
    let x = names(&h, "xsheet");
    assert_eq!(x[x.len() - 2], "Bob");
    assert_eq!(
        err(&h, "element.order.move", json!({ "elementIds": [PEG], "to": "front", "order": "xsheet" })).code,
        "element.notCellBased"
    );
    // order.set is a permutation of the level.
    let ids: Vec<String> = ok(&h, "element.list", json!({ "sceneId": INTRO, "order": "xsheet" }))["elements"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .map(|e| e["id"].as_str().unwrap().to_owned())
        .collect();
    ok(&h, "element.order.set", json!({ "order": "xsheet", "elementIds": ids }));
    assert_eq!(err(&h, "element.order.set", json!({ "order": "xsheet", "elementIds": [BOB] })).code, "arg.invalid");
}

#[test]
fn parenting_rules() {
    let h = fixture();
    ok(&h, "element.parent.set", json!({ "elementIds": [BG], "parentId": PEG }));
    assert_eq!(ok(&h, "element.hierarchy.get", json!({ "elementId": BG }))["parentId"], id(&h, PEG));
    assert_eq!(
        err(&h, "element.parent.set", json!({ "elementIds": [PEG], "parentId": BOB })).code,
        "element.notParentable"
    );
    assert_eq!(
        err(&h, "element.parent.set", json!({ "elementIds": [PEG], "parentId": PEG })).code,
        "element.hierarchyCycle"
    );
    let child_peg =
        ok(&h, "element.create", json!({ "type": "peg", "name": "Inner", "parentId": PEG }))["elementIds"][0].clone();
    assert_eq!(
        err(&h, "element.parent.set", json!({ "elementIds": [PEG], "parentId": child_peg })).code,
        "element.hierarchyCycle"
    );
    // Detach: to the parent's level, directly above the parent.
    let r = ok(&h, "element.parent.remove", json!({ "elementIds": [BG] }));
    assert_eq!(r["parentId"], Value::Null);
    let hier = ok(&h, "element.hierarchy.get", json!({ "elementId": BG }));
    assert_eq!(hier["nextSiblingId"], id(&h, PEG));
    // Attach to the nearest peg/effect above at the same level.
    let h = fixture();
    ok(&h, "element.order.move", json!({ "elementIds": [PEG], "to": "front" }));
    ok(&h, "element.parent.set", json!({ "elementIds": [BG], "nearest": true }));
    assert_eq!(ok(&h, "element.hierarchy.get", json!({ "elementId": BG }))["parentId"], id(&h, PEG));
    // Parent peg: inserted where the element was, covering its range.
    let h = fixture();
    let r = ok(&h, "element.parentPeg.create", json!({ "elementIds": [BG] }));
    assert_eq!(r["name"], "Background-P");
    let peg = get(&h, r["pegId"].as_str().unwrap());
    assert_eq!((peg["start"].as_u64(), peg["duration"].as_u64()), (Some(1), Some(48)));
    assert_eq!(ok(&h, "element.hierarchy.get", json!({ "elementId": BG }))["parentId"], peg["id"]);
}

#[test]
fn flags_visibility_lock_annotation_collapse_solo() {
    let h = fixture();
    let r = ok(&h, "element.visibility.set", json!({ "elementIds": [PEG], "scope": "scene", "visible": false }));
    assert_eq!(r["changedIds"].as_array().unwrap().len(), 1);
    let info = ok(&h, "element.get", json!({ "elementId": BOB }));
    assert!(info["hiddenByParent"].as_bool().unwrap(), "a child hidden only through its parent");
    assert!(info["visibleInScene"].as_bool().unwrap());
    ok(&h, "element.visibility.set", json!({ "scope": "scene", "visible": true, "apply": "all" }));
    let r = ok(
        &h,
        "element.visibility.set",
        json!({ "elementIds": [BOB], "scope": "scene", "visible": false, "apply": "others" }),
    );
    assert!(r["changedIds"].as_array().unwrap().len() > 10);
    let r = ok(
        &h,
        "element.visibility.set",
        json!({ "elementIds": [BOB], "scope": "scene", "visible": true, "apply": "all", "types": ["annotation"] }),
    );
    assert_eq!(r["changedIds"].as_array().unwrap().len(), 1);
    assert_eq!(
        err(&h, "element.visibility.set", json!({ "elementIds": [PEG], "scope": "xsheet", "visible": false })).code,
        "element.notCellBased"
    );
    // Hiding the current element in the Exposure Sheet moves the current element on (04 §10.2).
    ok(&h, "element.current.set", json!({ "elementId": BOB }));
    ok(&h, "element.visibility.set", json!({ "elementIds": [BOB], "scope": "xsheet", "visible": false }));
    assert_ne!(ok(&h, "cell.selection.get", json!({}))["currentElementId"], id(&h, BOB));
    // Lock all others / unlock all.
    let r = ok(&h, "element.lock.set", json!({ "elementIds": [BOB], "locked": true, "apply": "others" }));
    assert!(!r["changedIds"].as_array().unwrap().iter().any(|i| *i == id(&h, BOB)));
    ok(&h, "element.lock.set", json!({ "locked": false, "apply": "all" }));
    // Annotation toggle round trip.
    assert_eq!(ok(&h, "element.annotation.set", json!({ "elementId": BOB, "annotation": true }))["type"], "annotation");
    assert!(ok(&h, "element.get", json!({ "elementId": BOB }))["annotation"].as_bool().unwrap());
    ok(&h, "element.annotation.set", json!({ "elementId": BOB, "annotation": false }));
    assert_eq!(
        err(&h, "element.annotation.set", json!({ "elementId": BG, "annotation": true })).code,
        "element.notDrawing"
    );
    // Collapse is saved with the scene but is not an undo step.
    let steps = ok(&h, "edit.history.get", json!({}))["entries"].as_array().map_or(0, Vec::len);
    ok(&h, "element.collapsed.set", json!({ "elementIds": [PEG], "collapsed": true }));
    assert!(get(&h, PEG)["collapsed"].as_bool().unwrap());
    assert_eq!(ok(&h, "edit.history.get", json!({}))["entries"].as_array().map_or(0, Vec::len), steps);
    // Solo is view state only.
    let r0 = rev(&h);
    ok(&h, "element.solo.set", json!({ "enabled": true, "elementIds": [BOB] }));
    assert_eq!(rev(&h), r0);
}

#[test]
fn colours_notes_properties() {
    let h = fixture();
    ok(&h, "element.colour.set", json!({ "elementIds": [PEG], "marker": "#ff0000" }));
    let info = ok(&h, "element.get", json!({ "elementId": PEG }));
    assert_eq!(info["markerColour"], "#FF0000FF");
    assert_eq!(info["effectiveMarkerColour"], "#FF0000FF");
    ok(&h, "element.colour.set", json!({ "elementIds": [PEG], "marker": "" }));
    assert_eq!(ok(&h, "element.get", json!({ "elementId": PEG }))["effectiveMarkerColour"], "#C9A0DCFF");
    ok(&h, "element.typeColour.set", json!({ "category": "pegs", "colour": "#010203" }));
    assert_eq!(ok(&h, "element.get", json!({ "elementId": PEG }))["effectiveMarkerColour"], "#010203FF");
    assert_eq!(ok(&h, "element.typeColour.get", json!({}))["colours"]["pegs"], "#010203FF");
    ok(&h, "element.note.set", json!({ "elementId": PEG, "text": "rig" }));
    assert_eq!(get(&h, PEG)["note"], "rig");
    ok(&h, "element.note.set", json!({ "elementId": PEG, "text": "" }));
    assert!(get(&h, PEG).get("note").is_none());
    ok(&h, "element.properties.set", json!({ "elementId": BOB, "role": "foreground", "deformable": true }));
    let b = get(&h, BOB);
    assert_eq!((b["role"].as_str(), b["deformable"].as_bool()), (Some("foreground"), Some(true)));
}

#[test]
fn bar_timing_and_key_retiming() {
    let h = fixture();
    // Bob-P: bar 1-48, skew keys at local 1 and 48.
    let r = ok(&h, "element.timing.set", json!({ "elementIds": [PEG], "start": 5 }));
    assert_eq!(r["elements"][0]["start"], 5);
    assert_eq!(get(&h, PEG)["duration"], 48);
    let ch = |h: &ExecutorHandle| {
        let p = get(h, PEG);
        let c = get(h, p["channels"]["skew"]["id"].as_str().unwrap());
        c["keys"]
            .as_array()
            .unwrap()
            .iter()
            .map(|k| get(h, k["id"].as_str().unwrap())["frame"].as_u64().unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(ch(&h), [1, 48], "keys are local frames: moving the bar moves them");
    ok(&h, "element.timing.set", json!({ "elementIds": [PEG], "duration": 24, "keyframes": "scale" }));
    assert_eq!(ch(&h), [1, 24], "round(1 + (48 − 1)(24 − 1)/(48 − 1)) = 24");
    ok(&h, "element.timing.set", json!({ "elementIds": [PEG], "duration": 12 }));
    assert_eq!(ch(&h), [1], "keys beyond the new end are removed");
    // Stretch from the start keeps the end.
    let r = ok(&h, "element.timing.set", json!({ "elementIds": [PEG], "duration": 4, "anchor": "end" }));
    assert_eq!((r["elements"][0]["start"].as_u64(), r["elements"][0]["duration"].as_u64()), (Some(13), Some(4)));
    // Loops: count multiplies the bar.
    let r = ok(&h, "element.timing.set", json!({ "elementIds": [FADE], "loops": 2 }));
    assert_eq!(r["elements"][0]["duration"], 96);
    assert_eq!(
        err(&h, "element.timing.set", json!({ "elementIds": ["scene:\"Intro\"/element:\"Snow\""], "loops": 2 })).code,
        "element.notLoopable"
    );
    assert_eq!(err(&h, "element.timing.set", json!({ "elementIds": [BOB], "start": 3 })).code, "cell.notApplicable");
    assert_eq!(
        err(&h, "element.timing.set", json!({ "elementIds": [PEG], "start": 99_999, "duration": 10 })).code,
        "cell.frameOutOfRange"
    );
    assert_eq!(
        err(&h, "element.timing.set", json!({ "elementIds": [PEG], "start": 2, "startDelta": 1 })).code,
        "arg.invalid"
    );
}

#[test]
fn child_exposure_and_media_length() {
    let h = fixture();
    let r = ok(&h, "element.childExposure.apply", json!({ "pegId": PEG, "toFrame": 60 }));
    assert_eq!(r["changed"][0]["to"], 60);
    assert_eq!(get(&h, BOB)["exposure"].as_array().unwrap().last().unwrap()[1], 40);
    assert_eq!(
        err(&h, "element.childExposure.apply", json!({ "pegId": BOB, "toFrame": 60 })).code,
        "cell.notApplicable"
    );
    let r = ok(
        &h,
        "element.mediaLength.set",
        json!({ "elementId": "scene:\"Intro\"/element:\"Reference\"", "length": 30 }),
    );
    assert_eq!((r["from"].as_u64(), r["to"].as_u64()), (Some(1), Some(30)));
    let e = err(&h, "element.mediaLength.set", json!({ "elementId": "scene:\"Intro\"/element:\"Reference\"" }));
    assert_eq!(e.details["owner"], "M8 C8.3");
}

#[test]
fn read_model_and_lists() {
    let h = fixture();
    let info = ok(&h, "element.get", json!({ "elementId": BOB }));
    for k in [
        "id",
        "path",
        "name",
        "elementType",
        "sceneId",
        "parentId",
        "childIds",
        "depth",
        "timelineIndex",
        "xsheetIndex",
        "start",
        "duration",
        "loops",
        "annotation",
        "role",
        "deformable",
        "visibleInScene",
        "hiddenByParent",
        "visibleInXsheet",
        "locked",
        "collapsed",
        "markerColour",
        "backgroundColour",
        "effectiveMarkerColour",
        "note",
        "columnWidth",
        "cloneGroupId",
        "cloneIds",
        "drawingCount",
        "media",
    ] {
        assert!(info.get(k).is_some(), "element.get lacks `{k}`: {info}");
    }
    assert_eq!((info["start"].as_u64(), info["duration"].as_u64()), (Some(1), Some(44)));
    assert_eq!(info["path"], BOB);
    assert_eq!(info["depth"], 1);
    let media = ok(&h, "element.get", json!({ "elementId": "scene:\"Intro\"/element:\"Reference\"" }));
    assert_eq!(media["media"]["length"], 24);
    let list = ok(&h, "element.list", json!({ "sceneId": INTRO, "types": ["peg", "camera"] }))["elements"].clone();
    assert_eq!(list.as_array().unwrap().len(), 2);
    let kids = ok(&h, "element.list", json!({ "parentId": PEG }))["elements"].clone();
    assert_eq!(kids[0]["name"], "Bob");
    let hier = ok(&h, "element.hierarchy.get", json!({ "elementId": "scene:\"Intro\"/element:\"Rig-bones\"" }));
    assert_eq!(hier["nextSiblingId"], id(&h, "scene:\"Intro\"/element:\"Rig-skin\""));
    assert_eq!(hier["previousSiblingId"], Value::Null);
}

#[test]
fn selection_commands_are_not_undoable() {
    let h = fixture();
    let r0 = rev(&h);
    let r = ok(&h, "element.select", json!({ "elementIds": [BOB, PEG] }));
    assert_eq!(r["selectedIds"].as_array().unwrap().len(), 2);
    let r = ok(&h, "element.select", json!({ "elementIds": [BOB], "mode": "toggle" }));
    assert_eq!(r["selectedIds"], json!([id(&h, PEG)]));
    let r = ok(&h, "element.hierarchy.select", json!({ "relation": "children", "elementId": PEG }));
    assert_eq!(r["selectedIds"], json!([id(&h, PEG), id(&h, BOB)]));
    let r = ok(
        &h,
        "element.hierarchy.select",
        json!({ "relation": "skeletonRoot", "elementId": "scene:\"Intro\"/element:\"Arm\"" }),
    );
    assert_eq!(r["selectedIds"], json!([id(&h, "scene:\"Intro\"/element:\"Rig\"")]));
    assert_eq!(
        err(&h, "element.hierarchy.select", json!({ "relation": "parent", "elementId": PEG })).code,
        "command.disabled"
    );
    ok(&h, "element.current.set", json!({ "elementId": BOB }));
    let r = ok(&h, "element.current.move", json!({ "direction": "next" }));
    assert_eq!(r["elementId"], id(&h, BG));
    assert_eq!(rev(&h), r0);
}

#[test]
fn patch_applies_the_owning_command_rules() {
    // 10 AG-02: patching an element's name ≡ element.rename (normalised, 04 codes, unique in the scene).
    let h = fixture();
    ok(&h, "doc.object.patch", json!({ "targets": [BOB], "props": { "name": "  Robert " } }));
    assert_eq!(get(&h, "scene:\"Intro\"/element:\"Robert\"")["name"], "Robert");
    let bob = "scene:\"Intro\"/element:\"Robert\"";
    assert_eq!(
        err(&h, "doc.object.patch", json!({ "targets": [bob], "props": { "name": "Background" } })).code,
        "element.nameDuplicate"
    );
    assert_eq!(
        err(&h, "doc.object.patch", json!({ "targets": [bob], "props": { "name": "a/b" } })).code,
        "element.nameInvalid"
    );
    // Two targets cannot take one name.
    let e = err(&h, "doc.object.patch", json!({ "targets": [bob, PEG], "props": { "name": "Twin" } }));
    assert_eq!(e.code, "element.nameDuplicate");
    // Swapping two names in one patch is allowed: every target is renamed together.
    ok(
        &h,
        "doc.object.patch",
        json!({ "targets": [bob], "props": { "note": "n", "markerColour": "#00FF00FF", "locked": true, "visibleInScene": false } }),
    );
    let r = get(&h, bob);
    assert_eq!(
        (r["note"].as_str(), r["locked"].as_bool(), r["visibleInScene"].as_bool()),
        (Some("n"), Some(true), Some(false))
    );
    // Drawings: element.drawing.rename and element.drawingLock.set rules; unlocking a locked drawing is allowed.
    let d2 = "scene:\"Intro\"/element:\"Robert\"/drawing:\"2\"";
    ok(&h, "doc.object.patch", json!({ "targets": [d2], "props": { "locked": false } }));
    assert_eq!(
        err(&h, "doc.object.patch", json!({ "targets": [d2], "props": { "name": "1" } })).code,
        "drawing.nameDuplicate"
    );
    ok(&h, "doc.object.patch", json!({ "targets": [d2], "props": { "name": "B" } }));
    assert_eq!(get(&h, "scene:\"Intro\"/element:\"Robert\"/drawing:\"B\"")["locked"], false);
    // Properties of commands not yet implemented still name their owner.
    let e = err(
        &h,
        "doc.object.patch",
        json!({ "targets": [bob], "ops": [{ "op": "replace", "path": "/transform/offset/x", "value": 1 }] }),
    );
    assert_eq!(e.details["commandId"], "anim.transform.set");
}

#[test]
fn current_position_changes_are_announced() {
    let h = fixture();
    let rx = h.subscribe();
    ok(&h, "element.current.set", json!({ "elementId": BOB }));
    ok(&h, "element.current.move", json!({ "direction": "next" }));
    let ev: Vec<Value> = rx.try_iter().filter(|e| e.name == "cell.selection.changed").map(|e| e.payload).collect();
    assert_eq!(ev.len(), 2, "{ev:?}");
    assert_eq!(ev[0]["currentElementId"], id(&h, BOB));
    assert_eq!(ev[1]["currentElementId"], id(&h, BG));
}
