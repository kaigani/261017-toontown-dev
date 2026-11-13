//! Final-round independent closure variants, new data seed 0x313928.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod c13_support;
mod common;
use c13_support::{job_ok, tmp};
use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Status};
const B: &str = "scene:\"Intro\"/element:\"Bob\"";
const BG: &str = "scene:\"Intro\"/element:\"Background\"";
fn seeded() -> ExecutorHandle {
    let h = ExecutorHandle::spawn(Default::default()).unwrap();
    assert!(h.open_document(tt_model::fixture::sample_document(0x313928).unwrap(), None, false).is_ok());
    h
}

#[test]
fn delegated_moves_and_image_vector_copy_lock() {
    let h = seeded();
    ok(&h, "element.note.set", json!({"elementId":B,"text":"Rename928"}));
    let key = get(&h, B)["id"].as_str().unwrap().to_owned();
    ok(&h, "doc.object.patch", json!({"targets":[key],"ops":[{"op":"move","from":"/note","path":"/name"}]}));
    assert_eq!(get(&h, &key)["name"], "Rename928");
    assert!(get(&h, &key).get("note").is_none_or(|v| v == ""));
    ok(&h, "element.note.set", json!({"elementId":key,"text":"ToPlain928"}));
    ok(&h, "doc.object.patch", json!({"targets":[key],"ops":[{"op":"move","from":"/note","path":"/meta/r3"}]}));
    assert_eq!(get(&h, &key)["meta"]["r3"], "ToPlain928");
    for (element, image) in [(key.as_str(), false), (BG, true)] {
        let drawing = get(&h, element)["exposure"][0][0].as_str().unwrap().to_owned();
        ok(&h, "doc.object.patch", json!({"targets":[drawing],"props":{"meta":{"lock928":true}}}));
        // M2's separately ruled writer seeds the pre-existing lock, including on the image drawing.
        ok(&h, "draw.lock.set", json!({"drawingId":drawing,"locked":true}));
        let before = get(&h, &drawing);
        let r = run(
            &h,
            "doc.object.patch",
            json!({"targets":[drawing],"ops":[{"op":"copy","from":"/meta/lock928","path":"/locked"}]}),
        );
        if image {
            assert_eq!(r.error.unwrap().code, "cell.notApplicable");
            assert_eq!(get(&h, &drawing), before);
        } else {
            assert_eq!(r.status, Status::Ok);
            assert_eq!(get(&h, &drawing)["locked"], true);
        }
    }
}

#[test]
fn four_paste_forms_both_types_after_delete_and_project_replacement() {
    for image_source in [false, true] {
        for cross_project in [false, true] {
            let h = seeded();
            let src = if image_source { BG } else { B };
            ok(&h, "cell.clipboard.set", json!({"elementIds":[src],"from":1,"to":2}));
            ok(&h, "element.delete", json!({"elementIds":[src]}));
            if cross_project {
                ok(&h, "project.close", json!({"onUnsaved":"discard"}));
                assert!(h.open_document(tt_model::fixture::sample_document(0x313929).unwrap(), None, false).is_ok());
            }
            let target =
                ok(&h, "element.create", json!({"type":if image_source {"drawing"}else{"image"},"name":"Opposite928"}))
                    ["elementIds"][0]
                    .clone();
            let revision = rev(&h);
            let before = get(&h, target.as_str().unwrap());
            for (cmd, p) in [
                ("cell.clipboard.apply", json!({"elementId":target,"frame":19})),
                ("cell.clipboard.apply", json!({"elementId":target,"frame":19,"mode":"newDrawings"})),
                ("edit.paste", json!({"context":"xsheet","destination":{"elementId":target,"frame":19}})),
                (
                    "edit.pasteSpecial",
                    json!({"context":"xsheet","destination":{"elementId":target,"frame":19},"options":{"overwriteDrawings":false}}),
                ),
            ] {
                assert_eq!(
                    err(&h, cmd, p).code,
                    "cell.typeMismatch",
                    "{cmd} image={image_source} cross={cross_project}"
                );
                assert_eq!(rev(&h), revision);
                assert_eq!(get(&h, target.as_str().unwrap()), before);
            }
        }
    }
}

#[test]
fn generic_delete_and_cut_special_notes_and_boundary_refusal_save_open() {
    for (command, options) in [
        ("edit.delete", Value::Null),
        ("edit.cutSpecial", json!({"createDeleteExposure":false})),
        ("edit.cutSpecial", json!({"createDeleteExposure":true})),
    ] {
        let h = seeded();
        let e = ok(&h, "element.create", json!({"type":"drawing","name":"Notes928"}))["elementIds"][0].clone();
        for (f, t) in [(7, "removed928"), (11, "tail928"), (100000, "edge928")] {
            ok(&h, "cell.note.set", json!({"elementId":e,"frame":f,"text":t}));
        }
        let mut p = json!({"context":"xsheet","objectIds":[format!("element:@{}/cell:#7",e.as_str().unwrap())]});
        if !options.is_null() {
            p["options"] = options.clone();
        }
        ok(&h, command, p);
        let notes = get(&h, e.as_str().unwrap())["cellNotes"].clone();
        assert!(notes.get("7").is_none());
        assert_eq!(notes[if options["createDeleteExposure"] == true { "10" } else { "11" }], "tail928");
    }
    let h = seeded();
    let e = ok(&h, "element.create", json!({"type":"drawing","name":"Edge928"}))["elementIds"][0].clone();
    ok(&h, "cell.note.set", json!({"elementId":e,"frame":100000,"text":"exact928"}));
    // Moving no cell: changing a label at frame 2 keeps the edge note in place.
    ok(&h, "cell.label.set", json!({"elementIds":[e],"from":2,"to":2,"drawing":"held928","createMissing":true}));
    assert_eq!(get(&h, e.as_str().unwrap())["cellNotes"]["100000"], "exact928");
    let before = get(&h, e.as_str().unwrap());
    let revision = rev(&h);
    let history = ok(&h, "edit.history.get", json!({}))["position"].clone();
    assert_eq!(
        err(&h, "cell.range.add", json!({"elementIds":[e],"frame":1,"count":1,"position":"before"})).code,
        "cell.frameOutOfRange"
    );
    assert_eq!(get(&h, e.as_str().unwrap()), before);
    assert_eq!(rev(&h), revision);
    assert_eq!(ok(&h, "edit.history.get", json!({}))["position"], history);
    let out = tmp().join("refused928.animpkg");
    job_ok(&h, "project.saveAs", json!({"path":out}));
    ok(&h, "project.close", json!({"onUnsaved":"discard"}));
    job_ok(&h, "project.open", json!({"path":out}));
    assert_eq!(get(&h, e.as_str().unwrap())["cellNotes"], before["cellNotes"]);
}

#[test]
fn every_companion_and_name_counter_width_is_bounded() {
    let h = seeded();
    for (i, kind) in ["clipping", "skeleton", "particleNature", "particleImpact"].iter().enumerate() {
        let name = format!("{}{}", "界".repeat(63), i);
        let r = run(&h, "element.create", json!({"type":kind,"name":name}));
        assert_eq!(r.status, Status::Ok, "{kind}: {r:?}");
    }
    for row in ok(&h, "element.list", json!({}))["elements"].as_array().unwrap() {
        assert!(row["name"].as_str().unwrap().chars().count() <= 64);
    }
    for sep in ["_", "-"] {
        let base = "幅".repeat(64);
        let taken: Vec<_> = (1..=99)
            .map(|i| format!("{}{sep}{i}", "幅".repeat(64 - sep.len() - i.to_string().len())))
            .chain([base.clone()])
            .collect();
        let out = tt_anim::exposure::plan_names(&base, "", sep, 1, &|n| taken.iter().any(|x| x == n), 2, false);
        assert_eq!(out, vec![format!("{}{sep}100", "幅".repeat(60)), format!("{}{sep}101", "幅".repeat(60))]);
    }
}
