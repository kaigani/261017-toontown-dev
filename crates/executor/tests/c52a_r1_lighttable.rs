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
