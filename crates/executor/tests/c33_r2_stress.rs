//! Deep and broad scene measurements and caller-side waiting.
mod c13_support;
mod common;
use common::{fixture, ok};
use serde_json::json;
#[test]
fn deep_and_broad_scene() -> Result<(), Box<dyn std::error::Error>> {
    let h = fixture();
    let mut parent = String::new();
    for i in 0..24 {
        let p = ok(&h, "element.create", json!({"type":"peg","name":format!("Depth {i}"),"sceneId":"scene:\"Intro\""}));
        let id = p["elementIds"][0].as_str().ok_or("id")?.to_owned();
        if !parent.is_empty() {
            ok(&h, "element.parent.set", json!({"elementIds":[id],"parentId":parent}));
        }
        ok(&h, "anim.transform.set", json!({"nodeIds":[id],"target":"static","scale":{"x":-0.99,"y":1.01},"skew":3}));
        parent = id;
    }
    ok(&h, "element.parent.set", json!({"elementIds":["scene:\"Intro\"/element:\"Bob\""],"parentId":parent}));
    let a = ok(&h, "render.frame.run", json!({"width":64}));
    let b = ok(&h, "render.frame.run", json!({"width":64}));
    assert_eq!(a["sha256"], b["sha256"]);
    for i in 0..1000 {
        ok(&h, "element.create", json!({"type":"drawing","name":format!("Broad {i}"),"sceneId":"scene:\"Intro\""}));
    }
    let start = std::time::Instant::now();
    ok(&h, "inspect.frame.describe", json!({"detail":"full"}));
    eprintln!("1000-element describe elapsed {:?}", start.elapsed());
    Ok(())
}
#[test]
fn last_target_readonly_preserves_originals() -> Result<(), Box<dyn std::error::Error>> {
    let h = fixture();
    let dir = c13_support::tmp();
    for i in 1..=3 {
        std::fs::write(dir.join(format!("shot-{i}.png")), format!("original-{i}"))?;
    }
    let last = dir.join("shot-3.png");
    let mut perm = std::fs::metadata(&last)?.permissions();
    perm.set_readonly(true);
    std::fs::set_permissions(&last, perm)?;
    let started = ok(
        &h,
        "render.range.run",
        json!({"from":1,"to":3,"width":32,"directory":dir,"namePattern":"shot-{frame}","onConflict":"overwrite"}),
    );
    let rec = c13_support::wait(&h, &started);
    assert_eq!(rec["error"]["code"], "export.targetInvalid");
    for i in 1..=3 {
        assert_eq!(std::fs::read(dir.join(format!("shot-{i}.png")))?, format!("original-{i}").as_bytes());
    }
    assert_eq!(std::fs::read_dir(&dir)?.count(), 3);
    Ok(())
}
