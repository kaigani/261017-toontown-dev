#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason="review tests")]
mod common;
use common::*;
use serde_json::{Value,json};
use tt_executor::ExecutorHandle;
const BOB:&str="scene:\"Intro\"/element:\"Bob\"";
fn lists(h:&ExecutorHandle)->Vec<Value>{
 let a=ok(h,"xsheet.lightTable.list",json!({}))["items"].as_array().unwrap().clone();
 let b=ok(h,"drawAid.staticLightTable.list",json!({}))["entries"].as_array().unwrap().clone();
 assert_eq!(a.len(),b.len());
 for (x,y) in a.iter().zip(b.iter()) {
 assert_eq!(x["itemId"],y["entryId"]);
 assert_eq!(x["visible"],y["visible"]);
 assert_eq!(if x["placement"]=="over" {"overlay"} else {"underlay"},y["placement"].as_str().unwrap());
 }
 a
}
#[test]
fn a2_seeded_two_family_trace(){
 let h=fixture();
 let d=ok(&h,"element.drawing.create",json!({"elementId":BOB,"name":"trace"}))["drawingId"].clone();
 let r=rev(&h); let hist=ok(&h,"edit.history.get",json!({"limit":1000}));
 let mut seed=0x52a_u64;
 for i in 0..200 {
 seed=seed.wrapping_mul(6364136223846793005).wrapping_add(1);
 let a=lists(&h);
 let op=if a.is_empty(){0}else{(seed>>32)%7};
 match op {
 0=>{ok(&h,"xsheet.lightTable.add",json!({"items":[{"elementId":BOB,"drawingId":d}]}));},
 1=>{ok(&h,"xsheet.lightTable.move",json!({"itemId":a[0]["itemId"],"index":a.len()-1}));},
 2=>{ok(&h,"xsheet.lightTable.remove",json!({"itemIds":[a[0]["itemId"]]}));},
 3=>{ok(&h,"xsheet.lightTable.set",json!({"apply":"all","visible":i%2==0,"placement":if i%3==0{"over"}else{"under"}}));},
 4=>{ok(&h,"drawAid.staticLightTable.add",json!({"elementId":BOB,"drawingId":d}));},
 5=>{ok(&h,"drawAid.staticLightTable.remove",json!({"entryIds":[a[0]["itemId"]]}));},
 _=>{ok(&h,"drawAid.staticLightTable.set",json!({"all":true,"visible":i%2==0,"placement":if i%3==0{"overlay"}else{"underlay"}}));}
 }
 lists(&h);assert_eq!(rev(&h),r);assert_eq!(ok(&h,"edit.history.get",json!({"limit":1000})),hist);
 }
}
#[test]
fn a3_limits_and_notices(){
 let h=fixture();
 for w in [19,20,1000,1001] {
 let r=run(&h,"xsheet.columnWidth.set",json!({"elementId":BOB,"width":w}));
 if w==19 || w==1001 {assert_eq!(r.error.unwrap().code,"arg.invalid");}else{assert!(r.is_ok());}
 ok(&h,"doc.state.get",json!({}));
 }
 assert_eq!(ok(&h,"xsheet.columnWidth.set",json!({"reset":true}))["widths"],json!({}));
 let id=ok(&h,"xsheet.lightTable.add",json!({"items":[{"elementId":BOB,"frame":1}]}))["itemIds"][0].clone();
 for (item,index,code) in [(id,1000,"arg.invalid"),(json!("lt999999"),0,"ref.notFound")] {
 assert_eq!(err(&h,"xsheet.lightTable.move",json!({"itemId":item,"index":index})).code,code);
 ok(&h,"doc.state.get",json!({}));
 }
 for (cmd,key) in [("xsheet.display.set","thumbnails"),("xsheet.panel.set","elementList")] {
 let initial=ok(&h,cmd,json!({}))[key].as_bool().unwrap();
 let rx=h.subscribe();
 ok(&h,cmd,json!({key:!initial}));
 assert_eq!(rx.try_iter().filter(|e|e.name=="shell.xsheetView").count(),1);
 ok(&h,cmd,json!({key:!initial}));
 assert_eq!(rx.try_iter().filter(|e|e.name=="shell.xsheetView").count(),0);
 ok(&h,"doc.state.get",json!({}));
 }
}
mod c13_support;
#[test]
fn p2_scenes_save_and_exact_cleanup(){
 let dir=c13_support::tmp();let h=c13_support::spawn(&dir.join("user"));
 assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(),None,false).is_ok());
 let path=dir.join("Scenes.animpkg");
 c13_support::job_ok(&h,"project.saveAs",json!({"path":path.to_string_lossy()}));
 let a=first(&h,"scene");
 let e=ok(&h,"element.create",json!({"type":"drawing","name":"A"}))["elementIds"][0].clone();
 let mut ds=Vec::new();for i in 0..3 {ds.push(ok(&h,"element.drawing.create",json!({"elementId":e,"name":format!("D{i}")}))["drawingId"].clone());}
 for d in &ds {ok(&h,"xsheet.lightTable.add",json!({"items":[{"elementId":e,"drawingId":d}]}));}
 let ids=lists(&h);ok(&h,"xsheet.lightTable.move",json!({"itemId":ids[2]["itemId"],"index":0}));
 ok(&h,"xsheet.lightTable.set",json!({"itemIds":[ids[1]["itemId"]],"visible":false,"placement":"over"}));
 let before_a=lists(&h);
 let b=get(&h,"scene:\"Scene-2\"")["id"].clone();
 ok(&h,"app.context.set",json!({"sceneId":b}));
 assert!(lists(&h).is_empty());
 let f=ok(&h,"element.create",json!({"type":"drawing","name":"B"}))["elementIds"][0].clone();
 let d=ok(&h,"element.drawing.create",json!({"elementId":f,"name":"B1"}))["drawingId"].clone();
 ok(&h,"xsheet.lightTable.add",json!({"items":[{"elementId":f,"drawingId":d}]}));let before_b=lists(&h);
 c13_support::job_ok(&h,"project.save",json!({}));ok(&h,"project.close",json!({}));
 c13_support::job_ok(&h,"project.open",json!({"path":path.to_string_lossy()}));
 ok(&h,"app.context.set",json!({"sceneId":b}));assert_eq!(lists(&h),before_b);
 ok(&h,"app.context.set",json!({"sceneId":a}));assert_eq!(lists(&h),before_a);
 ok(&h,"element.drawing.delete",json!({"drawingIds":[ds[1]],"ifInUse":"clearCells"}));
 assert_eq!(lists(&h),before_a.iter().filter(|x|x["drawingId"]!=ds[1]).cloned().collect::<Vec<_>>());
 ok(&h,"element.delete",json!({"elementIds":[e]}));assert!(lists(&h).is_empty());
 assert_eq!(err(&h,"storyline.scene.delete",json!({"sceneId":a})).code,"command.notImplemented");
 ok(&h,"app.context.set",json!({"sceneId":b}));assert_eq!(lists(&h),before_b);
}
#[test]
fn p3_add_types_empty_frame_and_no_selection(){
 let h=fixture();
 let all=ok(&h,"doc.object.find",json!({"type":"element","limit":1000}))["items"].as_array().unwrap().clone();
 for item in all {
 let id=item["id"].clone();let obj=get(&h,id.as_str().unwrap());
 if !obj["path"].as_str().unwrap().starts_with("scene:\"Intro\""){continue;}
 let ty=obj["elementType"].as_str().unwrap();
 let before=lists(&h);
 if ["drawing","image","annotation"].contains(&ty) {
 let mut d=ok(&h,"element.drawing.list",json!({"elementId":id}))["drawings"][0]["id"].clone();
 if d.is_null(){d=ok(&h,"element.drawing.create",json!({"elementId":id,"name":"Matrix"}))["drawingId"].clone();}
 ok(&h,"xsheet.lightTable.add",json!({"items":[{"elementId":id,"drawingId":d}]}));
 assert_eq!(lists(&h).len(),before.len()+1);
 }else{
 assert_eq!(err(&h,"xsheet.lightTable.add",json!({"items":[{"elementId":id,"frame":1}]})).code,"cell.notApplicable","{ty}");
 assert_eq!(lists(&h),before);
 }
 }
 let before=lists(&h);
 assert_eq!(err(&h,"xsheet.lightTable.add",json!({"items":[{"elementId":BOB,"frame":999}]})).code,"drawing.noTarget");
 assert_eq!(lists(&h),before);
 ok(&h,"edit.deselectAll",json!({}));
 assert_eq!(err(&h,"xsheet.lightTable.add",json!({})).code,"drawing.noTarget");
 assert_eq!(lists(&h),before);
}
