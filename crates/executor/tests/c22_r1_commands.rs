#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review regression tests")]
mod common;
use common::*;
use serde_json::{json,Value};
const D:&str="dr_01K742SG0KKMCAP9X1E6Z5P4SW";
fn narrow(h:&tt_executor::ExecutorHandle)->Value{
    ok(h,"draw.stroke.add",json!({"drawingId":D,"kind":"invisible","layer":"new","fit":false,"closed":true,
        "points":[{"x":1000.1,"y":1000.0},{"x":1000.2,"y":1000.0},{"x":1000.2,"y":1002.0},{"x":1000.1,"y":1002.0}]}))
}
#[test]
fn crossing_path_paints_a_zone_between_sample_positions(){
    let h=fixture(); let _r=narrow(&h);
    let result=run(&h,"colour.paint.apply",json!({"drawingId":D,"path":[{"x":999.0,"y":1001.0},{"x":1001.0,"y":1001.0}]}));
    assert!(result.is_ok(),"a continuous path crosses the 0.1 du wide zone: {:?}",result.error);
    assert!(result.result.unwrap()["painted"].as_array().unwrap().iter().any(|p|p["objectKind"]=="zone"));
}

mod c13_support;
const MISSING:&str="col_01K9YYYYYYYYYYYYYYYYYYYYYY";
#[test]
fn save_has_a_recovery_entry_for_every_used_colour_even_without_prior_copy(){
    let dir=c13_support::tmp();let h=fixture();
    ok(&h,"doc.object.patch",json!({"targets":[format!("{D}/st_01K742SG0R61SYK9GG0HGYVNBT")],"ops":[{"op":"replace","path":"/colourId","value":MISSING}]}));
    c13_support::job_ok(&h,"project.saveAs",json!({"path":dir.join("R.animpkg").to_string_lossy()}));
    let doc=h.snapshot().unwrap().unwrap();let d=doc.drawing(&tt_model::Id::parse(D).unwrap()).unwrap();
    let found=d.colour_recovery().iter().any(|e|e.colour_id.as_str()==MISSING);
    drop(doc);drop(h);std::fs::remove_dir_all(dir).unwrap();
    assert!(found,"03 1.6 / 10 4.11: every used ID must have a complete recovery copy on save");
}
#[test]
fn loading_a_drawing_recovers_its_missing_colour(){
    let dir=c13_support::tmp();let h=fixture();
    ok(&h,"doc.object.patch",json!({"targets":[format!("{D}/st_01K742SG0R61SYK9GG0HGYVNBT")],"ops":[{"op":"replace","path":"/colourId","value":MISSING}]}));
    let package=dir.join("Load.animpkg");
    c13_support::job_ok(&h,"project.saveAs",json!({"path":package.to_string_lossy()}));
    drop(h);
    let h=c13_support::spawn(&dir.join("prefs"));
    c13_support::job_ok(&h,"project.open",json!({"path":package.to_string_lossy()}));
    let missing=ok(&h,"colour.missing.list",json!({}));
    drop(h);std::fs::remove_dir_all(dir).unwrap();
    assert!(!missing["missing"].as_array().unwrap().iter().any(|x|x["colourId"]==MISSING),"03 11.2: load should create a placeholder: {missing}");
}
