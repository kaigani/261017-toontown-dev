//! Independent visibility and metadata parity.
mod c13_support;
mod common;
use common::{fixture, ok};
use serde_json::json;
#[test]
fn visibility_overrides_are_read_only_and_match_hiding() {
    let h = fixture();
    let bob = "scene:\"Intro\"/element:\"Bob\"";
    let rev = common::rev(&h);
    let history = ok(&h, "edit.history.get", json!({}));
    let events = h.subscribe();
    let frame = ok(&h, "render.frame.run", json!({"width":64,"exclude":[bob]}));
    assert!(frame["renderMs"].is_u64());
    let sheet = ok(&h, "inspect.contactSheet.render", json!({"frames":[1,3],"cellWidth":64,"exclude":[bob]}));
    let desc = ok(&h, "inspect.frame.describe", json!({"exclude":[bob]}));
    let dir = c13_support::tmp();
    let range = c13_support::job_ok(
        &h,
        "render.range.run",
        json!({"from":1,"to":1,"width":64,"directory":dir,"exclude":[bob]}),
    );
    assert_eq!(rev, common::rev(&h));
    assert_eq!(history, ok(&h, "edit.history.get", json!({})));
    let emitted: Vec<_> = events.try_iter().collect();
    assert!(
        !emitted.iter().any(|e| e.name == "doc.changed" || e.name == "anim.changed" || e.name == "app.state.changed")
    );
    eprintln!("query/job events: {emitted:?}");
    ok(&h, "element.visibility.set", json!({"elementIds":[bob],"scope":"scene","visible":false}));
    assert_eq!(frame["sha256"], ok(&h, "render.frame.run", json!({"width":64}))["sha256"]);
    assert_eq!(
        sheet["sha256"],
        ok(&h, "inspect.contactSheet.render", json!({"frames":[1,3],"cellWidth":64}))["sha256"]
    );
    assert_eq!(range["files"][0]["sha256"], frame["sha256"]);
    assert_eq!(desc["elements"], ok(&h, "inspect.frame.describe", json!({}))["elements"]);
}

#[test]
fn unknown_override_refs_are_refused_on_each_command() {
    let h = fixture();
    let dir = c13_support::tmp();
    let unknown = "scene:\"Intro\"/element:\"No such element\"";
    for (command, params) in [
        ("render.frame.run", json!({"width":32,"include":[unknown]})),
        ("render.range.run", json!({"from":1,"to":1,"width":32,"directory":dir,"exclude":[unknown]})),
        ("inspect.contactSheet.render", json!({"frames":[1],"cellWidth":32,"include":[unknown]})),
        ("inspect.frame.describe", json!({"exclude":[unknown]})),
    ] {
        assert_eq!(common::err(&h, command, params).code, "arg.invalid", "{command}");
    }
}

#[test]
fn include_overrides_hidden_ancestor_and_exclusion_wins() {
    let h = fixture();
    let bob = "scene:\"Intro\"/element:\"Bob\"";
    let shown = ok(&h, "render.frame.run", json!({"width":64}));
    ok(
        &h,
        "element.visibility.set",
        json!({"elementIds":["scene:\"Intro\"/element:\"Bob-P\""],"scope":"scene","visible":false}),
    );
    let hidden = ok(&h, "render.frame.run", json!({"width":64}));
    let included = ok(&h, "render.frame.run", json!({"width":64,"include":[bob]}));
    assert_eq!(included["sha256"], shown["sha256"]);
    let both = ok(&h, "render.frame.run", json!({"width":64,"include":[bob],"exclude":[bob]}));
    assert_eq!(both["sha256"], hidden["sha256"]);
}
