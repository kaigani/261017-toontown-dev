//! Fill-mapping clipboard survives a save in-session but is absent after reopening.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod c13_support;
mod common;
use common::*;
use serde_json::json;
const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const P: &str = "st_01K742SG0R61SYK9GG0HGYVNBT";
#[test]
fn mapping_clipboard_is_not_package_content() {
    let h = fixture();
    let colours: Vec<_> = h
        .snapshot()
        .unwrap()
        .unwrap()
        .palettes()
        .flat_map(|p| p.swatches().iter().map(|s| s.colour_id().to_string()))
        .collect();
    ok(&h, "colour.paint.apply", json!({"drawingId":D,"objectIds":[P],"colourId":colours[1]}));
    ok(&h, "colour.fillMapping.set", json!({"drawingId":D,"objectIds":[P],"mapping":{"matrix":[37,2,4,31,2000,2000]}}));
    let mapping = ok(&h, "colour.fillMapping.get", json!({"drawingId":D,"objectId":P,"toClipboard":true}));
    let dir = c13_support::tmp();
    let path = dir.join("clipboard.animpkg");
    c13_support::job_ok(&h, "project.saveAs", json!({"path":path.to_string_lossy()}));
    ok(&h, "colour.fillMapping.set", json!({"drawingId":D,"objectIds":[P],"mapping":"clipboard"}));
    let next = c13_support::spawn(&dir.join("next-user"));
    c13_support::job_ok(&next, "project.open", json!({"path":path.to_string_lossy()}));
    let back = ok(&next, "colour.fillMapping.get", json!({"drawingId":D,"objectId":P}));
    assert_eq!(back, mapping);
    assert_eq!(
        err(&next, "colour.fillMapping.set", json!({"drawingId":D,"objectIds":[P],"mapping":"clipboard"})).code,
        "colour.clipboardEmpty"
    );
    drop(next);
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}
