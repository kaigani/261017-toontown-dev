//! M5 C5.1b round 2, ruling E-52 (C51B-R2S-01, C51B-R2S-02): an agent's Shift+F10 through the `key` tool on a real MCP
//! stream takes the path of every key, then answers what the window did. A fake window (the host's `AttachedIo`) plays
//! the window's half: it records every routed request and answers as `openContextMenuFor` does (`opened`, `noMenu`,
//! `notFound`). The real window's half runs in `scripts/c51b-context-menu-key.py` with real input.
//!
//! Rows: a dialog open (`ui.modalOpen`, nothing routed); a text field being edited (`ui.busy`,
//! nothing routed: the field keeps its Shift+F10); a non-string `view` (`arg.invalid`, nothing routed); an unknown
//! control and a stale row id (`view.notFound` from the window); a control without a menu (`handled: false`); the
//! passing case; the focused control (no `view`); no window (the headless answer).
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs, reason = "test")]
mod support;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use support::*;
use tt_executor::{AttachedIo, CapturedPng, CommandError, Envelope, Origin};

/// The fake window: the controls it shows, which of them offer a context menu (and its id), and every request routed.
#[derive(Debug, Default)]
struct FakeWindow {
    controls: Vec<(String, Option<String>)>,
    focused: Option<String>,
    routed: Mutex<Vec<Option<String>>>,
}

impl AttachedIo for FakeWindow {
    fn route_pointer(&self, _view: &str, _origin: &Origin, _request: &Value, _budget: Duration) -> Option<Envelope> {
        None
    }

    fn route_context_menu(&self, target: Option<&str>, _origin: &Origin, _budget: Duration) -> Option<Envelope> {
        self.routed.lock().unwrap().push(target.map(str::to_owned));
        let name = target.map(str::to_owned).or_else(|| self.focused.clone());
        let control = name.as_ref().and_then(|n| self.controls.iter().find(|(id, _)| id == n));
        Some(match control {
            Some((id, Some(menu))) => Envelope::ok(json!({ "handled": true, "contextMenu": id, "menuId": menu }), None),
            Some((id, None)) => {
                Envelope::ok(json!({ "handled": false, "reason": "noContextMenu", "control": id }), None)
            }
            None => Envelope::error(
                CommandError::with_details(
                    "view.notFound",
                    "the window has no such control",
                    json!({ "view": target, "reason": "noControl", "pointer": "/view" }),
                ),
                None,
            ),
        })
    }

    fn capture(&self, _t: &str, _r: Option<[f64; 4]>, _m: u32, _b: Duration) -> Result<CapturedPng, CommandError> {
        Err(CommandError::new("view.notFound", "no captures in this test"))
    }
}

fn key(c: &mut Client, args: Value) -> Value {
    c.call("key", args)["structuredContent"].clone()
}

fn code(r: &Value) -> &str {
    r["error"]["code"].as_str().unwrap_or("")
}

#[test]
fn shift_f10_takes_every_keys_checks_then_answers_what_the_window_did() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    let window = Arc::new(FakeWindow {
        controls: vec![
            ("timeline.tracks".into(), Some("timeline.cellMenu".into())),
            ("timeline.row.A.label".into(), Some("timeline.rowMenu".into())),
            ("timeline.add".into(), None),
        ],
        focused: Some("timeline.tracks".into()),
        routed: Mutex::new(Vec::new()),
    });
    h.set_attached_io(Some(window.clone()));
    let mut options = opts(&dir);
    options.policy = Some(tt_executor::agents::Policy::Autonomous);
    let (mut c, _) = Client::connect(&h, options, json!({}));
    let routed = |w: &FakeWindow| w.routed.lock().unwrap().clone();

    // A dialog the person opened has the keyboard: `ui.modalOpen`, and the window is never asked (no focus moved).
    h.report_shell(
        serde_json::from_value(json!({ "modal": { "kind": "dialog", "title": "Change Duration" } })).unwrap(),
    );
    let r = key(&mut c, json!({ "keys": "shift+F10", "view": "timeline.row.A.label" }));
    assert_eq!(code(&r), "ui.modalOpen", "{r}");
    let r = key(&mut c, json!({ "keys": "shift+F10" }));
    assert_eq!(code(&r), "ui.modalOpen", "{r}");
    assert!(routed(&window).is_empty(), "a key under a dialog reached the window: {:?}", routed(&window));

    // A text field being edited keeps its Shift+F10 (the shared text-field rule): `ui.busy`, nothing routed.
    h.report_shell(serde_json::from_value(json!({ "modal": null, "textFocus": true })).unwrap());
    let r = key(&mut c, json!({ "keys": "shift+F10", "view": "timeline.tracks" }));
    assert_eq!(code(&r), "ui.busy", "{r}");
    assert!(routed(&window).is_empty(), "a key over a text field reached the window: {:?}", routed(&window));
    h.report_shell(serde_json::from_value(json!({ "modal": null, "textFocus": false })).unwrap());

    // A `view` that is not a string: `arg.invalid` at `/view`, nothing routed.
    let r = key(&mut c, json!({ "keys": "shift+F10", "view": 42 }));
    assert_eq!(code(&r), "arg.invalid", "{r}");
    assert_eq!(r["error"]["details"]["pointer"], "/view", "{r}");
    assert!(routed(&window).is_empty(), "{:?}", routed(&window));

    // An unknown control and a stale row id: the window's `view.notFound`, never a success.
    let r = key(&mut c, json!({ "keys": "shift+F10", "view": "timeline.row.unknown.label" }));
    assert_eq!(code(&r), "view.notFound", "{r}");
    assert_eq!(r["error"]["details"]["reason"], "noControl", "{r}");
    let r = key(&mut c, json!({ "keys": "shift+F10", "view": "timeline.row.Gone" }));
    assert_eq!(code(&r), "view.notFound", "{r}");

    // A control that exists and offers no menu: `handled: false`.
    let r = key(&mut c, json!({ "keys": "shift+F10", "view": "timeline.add" }));
    assert_eq!(r["status"], "ok", "{r}");
    assert_eq!((r["result"]["handled"].clone(), r["result"]["reason"].clone()), (json!(false), json!("noContextMenu")));

    // The passing case, named and focused: the menu the window opened.
    let r = key(&mut c, json!({ "keys": "shift+F10", "view": "timeline.row.A.label" }));
    assert_eq!(
        (r["result"]["handled"].clone(), r["result"]["menuId"].clone()),
        (json!(true), json!("timeline.rowMenu"))
    );
    let r = key(&mut c, json!({ "keys": "Shift+F10" }));
    assert_eq!(
        (r["result"]["handled"].clone(), r["result"]["menuId"].clone()),
        (json!(true), json!("timeline.cellMenu"))
    );

    // Every request that passed the checks reached the window once, with its target.
    assert_eq!(
        routed(&window),
        vec![
            Some("timeline.row.unknown.label".to_owned()),
            Some("timeline.row.Gone".to_owned()),
            Some("timeline.add".to_owned()),
            Some("timeline.row.A.label".to_owned()),
            None
        ]
    );
}

#[test]
fn shift_f10_without_a_window_finds_no_menu() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    let mut options = opts(&dir);
    options.policy = Some(tt_executor::agents::Policy::Autonomous);
    let (mut c, _) = Client::connect(&h, options, json!({}));
    // No window attached: a named control is not found, the focused one handles nothing.
    let r = key(&mut c, json!({ "keys": "shift+F10", "view": "timeline.tracks" }));
    assert_eq!(code(&r), "view.notFound", "{r}");
    let r = key(&mut c, json!({ "keys": "shift+F10" }));
    assert_eq!((r["status"].clone(), r["result"]["handled"].clone()), (json!("ok"), json!(false)), "{r}");
}
