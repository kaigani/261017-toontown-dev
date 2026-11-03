//! Random command-level hierarchy oracle, including cycle refusals and deletion reattachment.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::json;
use std::collections::BTreeMap;
fn next(s: &mut u64, n: usize) -> usize {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s as usize % n
}
#[test]
fn random_hierarchy_edits_4096_keep_independent_parent_and_level_model() {
    let h = fixture();
    let scene = first(&h, "scene");
    let made = ok(&h, "element.create", json!({"type":"peg","name":"Tree928","count":12}))["elementIds"]
        .as_array()
        .unwrap()
        .clone();
    let mut ids: Vec<String> = made.iter().map(|v| v.as_str().unwrap().to_owned()).collect();
    let mut parents: BTreeMap<String, Option<String>> = ids.iter().map(|v| (v.clone(), None)).collect();
    let mut seed = 0x313950;
    for step in 0..4096 {
        let a = ids[next(&mut seed, ids.len())].clone();
        let b = ids[next(&mut seed, ids.len())].clone();
        if step % 17 == 0 {
            let parent = parents[&a].clone();
            ok(&h, "element.delete", json!({"elementIds":[a]}));
            for p in parents.values_mut() {
                if *p == Some(a.clone()) {
                    *p = parent.clone();
                }
            }
            parents.remove(&a);
            ids.retain(|v| v != &a);
            let new = ok(&h, "element.create", json!({"type":"peg","name":format!("New928-{step}")}))["elementIds"][0]
                .as_str()
                .unwrap()
                .to_owned();
            parents.insert(new.clone(), None);
            ids.push(new);
        } else if step % 5 == 0 {
            ok(&h, "element.parent.remove", json!({"elementIds":[a]}));
            parents.insert(a, None);
        } else {
            let mut cur = Some(b.clone());
            let mut cycle = false;
            while let Some(p) = cur {
                if p == a {
                    cycle = true;
                    break;
                }
                cur = parents[&p].clone();
            }
            let revision = rev(&h);
            let r = run(&h, "element.parent.set", json!({"elementIds":[a],"parentId":b}));
            if cycle {
                assert_eq!(r.error.unwrap().code, "element.hierarchyCycle");
                assert_eq!(rev(&h), revision);
            } else {
                assert_eq!(r.status, tt_executor::Status::Ok, "{r:?}");
                parents.insert(a, Some(b));
            }
        }
        let rows = ok(&h, "element.list", json!({"sceneId":scene}))["elements"].as_array().unwrap().clone();
        for row in rows {
            let id = row["id"].as_str().unwrap();
            if !parents.contains_key(id) {
                continue;
            }
            let read = ok(&h, "element.get", json!({"elementId":id}));
            assert_eq!(read["parentId"], json!(parents[id]), "step {step} id {id}");
            let mut depth = 0;
            let mut p = parents[id].clone();
            while let Some(v) = p {
                depth += 1;
                p = parents[&v].clone();
            }
            assert_eq!(read["depth"], depth, "step {step} id {id}");
        }
    }
}
