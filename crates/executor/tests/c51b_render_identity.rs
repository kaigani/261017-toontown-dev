//! M5 C5.1b (builder), ruling E-43 item 1: once the system font list is read, every frame command answers byte for
//! byte what it answered before (b) (the render list computing every entry's text bounds eagerly). The dev-only switch
//! `tt_vector::fonts::set_eager_text_bounds` restores the pre-(b) render list; the sample project's frame, hit tests and
//! full description are compared with it on and off, with Bob (whose drawing holds the caption) shown and hidden.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};
use std::time::Duration;
use tt_executor::ExecutorHandle;

const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";

/// Everything the frame commands answer for frame 1, in a form compared byte for byte.
fn answers(h: &ExecutorHandle) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    for (name, command, params) in [
        ("frame", "render.frame.run", json!({"frame": 1, "scale": 0.25})),
        ("frame 12", "render.frame.run", json!({"frame": 12, "scale": 0.25})),
        ("describe", "inspect.frame.describe", json!({"frame": 1, "detail": "full"})),
        ("describe summary", "inspect.frame.describe", json!({"frame": 1})),
        ("hit: camera centre", "inspect.hitTest", json!({"space": "camera", "point": {"x": 0, "y": 0}})),
        ("hit: on Bob", "inspect.hitTest", json!({"space": "image", "point": {"x": 960, "y": 520}, "frame": 1})),
        ("hit: corner", "inspect.hitTest", json!({"space": "image", "point": {"x": 5, "y": 5}, "frame": 1})),
        ("contact sheet", "inspect.contactSheet.render", json!({"frames": [1, 12], "cellWidth": 60})),
    ] {
        let mut env = serde_json::to_value(run(h, command, params)).unwrap();
        // The measured render time is the one field that differs between any two runs.
        if let Some(r) = env.get_mut("result").and_then(Value::as_object_mut) {
            r.remove("renderMs");
        }
        out.push((name.to_owned(), env));
    }
    out
}

/// The first JSON pointer at which two answers differ.
fn first_difference(a: &Value, b: &Value, at: &str) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let keys: std::collections::BTreeSet<&String> = x.keys().chain(y.keys()).collect();
            keys.into_iter().find_map(|k| {
                first_difference(
                    x.get(k).unwrap_or(&Value::Null),
                    y.get(k).unwrap_or(&Value::Null),
                    &format!("{at}/{k}"),
                )
            })
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            x.iter().zip(y).enumerate().find_map(|(i, (u, v))| first_difference(u, v, &format!("{at}/{i}")))
        }
        _ if a == b => None,
        _ => Some(format!(
            "{at}: {} | {}",
            a.to_string().chars().take(120).collect::<String>(),
            b.to_string().chars().take(120).collect::<String>()
        )),
    }
}

#[test]
fn frame_commands_answer_as_before_once_the_font_list_is_read() {
    let h = fixture();
    assert!(tt_vector::fonts::ready_within(Duration::from_secs(60)).is_ok());
    for bob_hidden in [false, true] {
        ok(&h, "element.visibility.set", json!({"elementIds": [BOB], "scope": "scene", "visible": !bob_hidden}));
        tt_vector::fonts::set_eager_text_bounds(true);
        let before = answers(&h);
        tt_vector::fonts::set_eager_text_bounds(false);
        let after = answers(&h);
        for ((name, b), (_, a)) in before.iter().zip(&after) {
            assert_eq!(
                serde_json::to_string(a).unwrap(),
                serde_json::to_string(b).unwrap(),
                "{name} (Bob hidden: {bob_hidden}): {:?}",
                first_difference(b, a, "")
            );
            assert!(b["status"] == "ok", "{name}: {b}");
        }
    }
}
