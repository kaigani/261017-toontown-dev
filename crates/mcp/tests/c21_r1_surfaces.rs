//! Independent C2.1 surface, persistence and malformed-content review checks.
#![allow(clippy::unwrap_used, missing_docs, reason = "review tests")]
mod support;
use serde_json::{Value,json};
use support::*;
use tt_executor::{Invocation,agents::Policy};

const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const D2: &str = "dr_01K742SG0M5J3BAJSW8241WA87";

fn ok(c: &mut Client,id: &str,p:Value)->Value {
    let e = c.run(id,p); assert_eq!(e["status"],"ok","{id}: {e}"); e["result"].clone()
}
fn wait(c:&mut Client,r:&Value) {
    let job = ok(c,"job.wait",json!({"jobId":r["jobId"],"timeoutMs":60000}));
    assert_eq!(job["state"],"succeeded","{job}");
}

#[test]
fn typed_generic_batches_scripts_and_both_container_forms() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(),None,false).is_ok());
    assert!(h.invoke(Invocation::user("agent.typedTools.set",json!({"enabled":true,"namespaces":["draw"]}))).is_ok());
    let mut options = opts(&dir); options.policy=Some(Policy::Autonomous);
    let (mut c,_) = Client::connect(&h,options.clone(),json!({}));
    let r = c.call("draw_stroke_add",json!({"drawingId":D,"kind":"pencil","points":[{"x":0,"y":0},{"x":100,"y":0}]}));
    assert_eq!(r["structuredContent"]["status"],"ok","{r}");
    let stroke = r["structuredContent"]["result"]["strokeIds"][0].clone();
    let text = ok(&mut c,"draw.text.add",json!({"drawingId":D,"position":{"x":0,"y":0},"text":"AV é\nאבג مرحبا","width":40}));
    ok(&mut c,"draw.textFormat.set",json!({"drawingId":D,"textId":text["textId"],"start":1,"length":3,"bold":true}));
    let bitmap = ok(&mut c,"draw.drawing.get",json!({"drawingId":D2}))["nodes"][0].clone();
    ok(&mut c,"draw.content.add",json!({"drawingId":D,"content":{"nodes":[bitmap]}}));
    let group = ok(&mut c,"draw.group.create",json!({"drawingId":D,"ids":[text["textId"]]}));
    ok(&mut c,"draw.group.delete",json!({"drawingId":D,"groupIds":[group["groupId"]]}));
    let before = ok(&mut c,"draw.drawing.get",json!({"drawingId":D}));
    let bad = c.call("batch",json!({"label":"review rollback","atomic":true,"steps":[
        {"id":"draw.pivot.set","params":{"drawingId":D,"point":{"x":8,"y":9}}},
        {"id":"draw.stroke.set","params":{"drawingId":D,"strokeId":"missing","thickness":2}}
    ]}));
    assert_eq!(bad["structuredContent"]["status"],"error");
    assert_eq!(ok(&mut c,"draw.drawing.get",json!({"drawingId":D})),before);
    let partial = c.call("batch",json!({"label":"review partial","atomic":false,"steps":[
        {"id":"draw.pivot.set","params":{"drawingId":D,"point":{"x":8,"y":9}}},
        {"id":"draw.stroke.set","params":{"drawingId":D,"strokeId":"missing","thickness":2}}
    ]}));
    assert_eq!(partial["structuredContent"]["status"],"error");
    assert_eq!(ok(&mut c,"draw.drawing.get",json!({"drawingId":D}))["pivot"],json!({"x":8.0,"y":9.0}));
    let script = serde_json::from_value(json!({"label":"review script","atomic":false,"steps":[{"atomic":true,"label":"review group","steps":[
        {"id":"draw.stroke.set","params":{"drawingId":D,"strokeId":stroke,"thickness":4}},
        {"id":"draw.pivot.set","params":{"drawingId":D,"point":{"x":2,"y":3}}}
    ]}]})).unwrap();
    let out = tt_mcp::script::run(&h,&options,script,None).unwrap(); assert!(out.ok,"{:?}",out.envelope.error);
    for form in ["package","folder"] {
        let p = if form=="folder" {format!("{}/",path(&dir.join("RoundtripFolder.animpkg")))} else {path(&dir.join("Roundtrip.animpkg"))};
        let r = ok(&mut c,"project.saveAs",json!({"path":p})); wait(&mut c,&r);
        let expected = ok(&mut c,"draw.drawing.get",json!({"drawingId":D}));
        ok(&mut c,"project.close",json!({}));
        let r = ok(&mut c,"project.open",json!({"path":p})); wait(&mut c,&r);
        assert_eq!(ok(&mut c,"draw.drawing.get",json!({"drawingId":D})),expected,"{form}");
    }
    // Duplicate IDs and malformed recursive group shapes must fail without changing the document.
    let before = ok(&mut c,"draw.drawing.get",json!({"drawingId":D}));
    for nodes in [json!([{"type":"group","id":"g","children":[]},{"type":"group","id":"g","children":[]}]),
        json!([{"type":"group","id":"g","children":["g"]}])] {
        let e = c.run("draw.content.add",json!({"drawingId":D,"content":{"nodes":nodes}}));
        assert_eq!(e["status"],"error","{e}");
        assert_eq!(ok(&mut c,"draw.drawing.get",json!({"drawingId":D})),before);
    }
    ok(&mut c,"project.close",json!({})); c.close().unwrap(); drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}
