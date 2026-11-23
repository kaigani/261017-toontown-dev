//! M5 C5.1b (builder): the Timeline's click on a cell follows an agent's change of `timing.timeline.singleCellSelection`
//! (ruling E-6; 04 §11.1; 01 §7.3 "a click selects one cell; when off, a click selects the whole track"). The window
//! sends the person's samples and the kernel's interpreter applies the setting, so the window follows the agent's
//! change with no state of its own: the same click selects one cell, then the whole track, then one cell again, after
//! the agent's `prefs.set` and its structured equivalent `timeline.singleCellSelection.set`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Invocation, Origin};

const RH: f64 = 22.0;
const CW: f64 = 16.0;

fn agent() -> Origin {
    Origin::Agent {
        session: "agent-1".into(),
        name: "Agent".into(),
        scopes: [tt_registry::Scope::Read, tt_registry::Scope::Ui, tt_registry::Scope::Prefs].into_iter().collect(),
    }
}

fn agent_ok(h: &ExecutorHandle, command: &str, params: Value) {
    let env = h.invoke(Invocation { origin: agent(), ..Invocation::user(command, params.clone()) });
    assert!(env.error.is_none(), "{command} {params}: {env:?}");
}

/// The first cell-based row of the fixture: its index and id.
fn drawing_row(h: &ExecutorHandle) -> (usize, String) {
    let rows = ok(h, "timeline.rows.list", json!({}))["rows"].as_array().unwrap().clone();
    let i = rows.iter().position(|r| r["type"] == "drawing").unwrap();
    (i, rows[i]["elementId"].as_str().unwrap().to_owned())
}

/// The selection the click on frame 5 of the row makes: the interpreter's plan, as the window's release commits it.
fn click_selects(h: &ExecutorHandle, row: usize) -> Value {
    let layout = json!({"surface": "tracks", "zoomLevel": 6, "firstFrame": 1, "scrollTop": 0, "platform": "windows"});
    let at = json!({"x": 4.0 * CW + 8.0, "y": row as f64 * RH + 11.0, "modifiers": []});
    let plan = h.timing_replay(Origin::User, layout, json!([at, at]), false).unwrap()["plan"].clone();
    assert_eq!(plan["kind"], "commands", "{plan}");
    let first = &plan["commands"][0];
    assert_eq!(first[0], "cell.select", "{plan}");
    first[1].clone()
}

#[test]
fn a_click_follows_the_agents_single_cell_setting() {
    let h = fixture();
    let (row, id) = drawing_row(&h);
    let len = ok(&h, "timeline.rows.list", json!({}))["sceneLength"].as_u64().unwrap();
    let one = json!({"elementIds": [id], "from": 5, "to": 5});
    let track = json!({"elementIds": [id], "from": 1, "to": len, "extent": "track"});
    // The default (01 §7.3): one cell.
    assert_eq!(click_selects(&h, row), one);
    agent_ok(&h, "prefs.set", json!({"values": {"timing.timeline.singleCellSelection": false}}));
    assert_eq!(click_selects(&h, row), track, "after the agent's prefs.set");
    agent_ok(&h, "timeline.singleCellSelection.set", json!({"enabled": true}));
    assert_eq!(click_selects(&h, row), one, "after the agent's timeline.singleCellSelection.set");
}
