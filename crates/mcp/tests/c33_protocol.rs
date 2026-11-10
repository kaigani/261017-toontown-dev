//! M3 C3.3 (builder): the generic tools of 11 §7.5 through MCP — `render_frame` returns what `render.frame.run` with
//! `composition: "camera"` returns (inline, and with an output path the file), `describe_frame`, `hit_test`,
//! `convert_coordinates` and `render_contact_sheet` route to their `inspect.*` commands.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod support;

use serde_json::{Value, json};

fn tool(c: &mut support::Client, name: &str, args: Value) -> Value {
    c.call(name, args)["structuredContent"].clone()
}

#[test]
fn generic_render_and_inspect_tools_match_their_commands() {
    let dir = support::tmp();
    let h = support::executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let mut opts = support::opts(&dir);
    opts.policy = Some(tt_executor::agents::Policy::Autonomous);
    let (mut c, _) = support::Client::connect(&h, opts, json!({}));
    let by_tool = tool(&mut c, "render_frame", json!({ "frame": 12, "width": 240 }));
    let by_command = c.run("render.frame.run", json!({ "frame": 12, "width": 240, "composition": "camera" }));
    assert_eq!(by_tool["status"], "ok", "{by_tool}");
    assert_eq!(by_tool["result"]["sha256"], by_command["result"]["sha256"]);
    assert_eq!(by_tool["result"]["data"], by_command["result"]["data"]);
    // `output` as a path writes the file (09 §11.1 item 3); `"inline"` stays inline.
    let path = dir.join("frame12.png");
    let file = tool(&mut c, "render_frame", json!({ "frame": 12, "width": 240, "output": path.to_string_lossy() }));
    assert_eq!(file["status"], "ok", "{file}");
    assert_eq!(file["result"]["path"].as_str(), Some(path.to_string_lossy().as_ref()));
    let png = std::fs::read(&path).unwrap();
    assert_eq!(png, tt_persist::b64::decode(by_command["result"]["data"].as_str().unwrap()).unwrap());
    let inline = tool(&mut c, "render_frame", json!({ "frame": 12, "width": 240, "output": "inline" }));
    assert_eq!(inline["result"]["data"], by_command["result"]["data"]);
    // The inspection tools.
    let d = tool(&mut c, "describe_frame", json!({ "frame": 12 }));
    assert_eq!(d["result"], c.run("inspect.frame.describe", json!({ "frame": 12 }))["result"]);
    let hit = tool(&mut c, "hit_test", json!({ "space": "camera", "point": { "x": -4, "y": 0 }, "frame": 1 }));
    assert_eq!(hit["status"], "ok", "{hit}");
    let conv =
        tool(&mut c, "convert_coordinates", json!({ "from": "field", "to": "image", "points": [{ "x": 0, "y": 0 }] }));
    assert_eq!(conv["result"]["points"][0], json!({ "x": 960.0, "y": 540.0 }));
    let sheet = tool(&mut c, "render_contact_sheet", json!({ "frames": [1, 12], "cellWidth": 48 }));
    assert_eq!(sheet["result"]["cells"].as_array().unwrap().len(), 2, "{sheet}");
    c.close().unwrap();
    drop(h);
    let _ = std::fs::remove_dir_all(dir);
}
