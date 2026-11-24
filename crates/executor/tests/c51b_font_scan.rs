//! M5 C5.1b (builder), ruling E-43: every path into the system font list (`reviews/M5-design-font-scan.md` §1.2) waits
//! for it at most the bound and then answers `command.timeout` with `details.reason: "fontsLoading"` before it changes
//! anything, or does not wait at all because no text is involved; the window's state reads never wait; a retry after
//! the list is read succeeds. The font read is held with the dev-only `test-hooks` hold, so this binary is one test (the
//! list is read once per process).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use tt_executor::{ExecutorHandle, Origin};

/// The test bound (the product's is 20 s, `tt_vector::fonts::WAIT_BOUND`).
const BOUND: Duration = Duration::from_millis(1500);
/// Bob's first drawing: the caption (a text node) and a pencil line.
const D1: &str = "drawingBank:#1/drawing:#1";
const CAPTION: &str = "nd_01K742SG0WFAQJFKAZKV4CD56C";
const PENCIL: &str = "st_01K742SG0R61SYK9GG0HGYVNBT";
const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";

#[derive(Clone, Copy, PartialEq, Debug)]
enum Expect {
    /// Involves text: waits the bound, then `fontsLoading`, nothing changed.
    Waits,
    /// No text involved: answers without waiting.
    NoWait,
}

/// One entry of the design note's §1.2 table: what it is, the command and its parameters.
struct Case {
    entry: &'static str,
    command: &'static str,
    params: Value,
    expect: Expect,
}

fn case(entry: &'static str, command: &'static str, params: Value, expect: Expect) -> Case {
    Case { entry, command, params, expect }
}

fn reason(env: &tt_executor::Envelope) -> Option<String> {
    env.error.as_ref().and_then(|e| e.details["reason"].as_str().map(str::to_owned))
}

/// A drawing without text: a new element with one rectangle (made while the list is held: no text involved).
fn plain_drawing(h: &ExecutorHandle) -> String {
    let el = ok(h, "element.create", json!({"type": "drawing", "name": "PlainE43"}))["elementIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    ok(h, "element.drawing.create", json!({"elementId": el, "name": "1", "exposeFrom": 1, "exposeTo": 2}));
    ok(h, "app.context.set", json!({"elementId": el, "frame": 1}));
    ok(
        h,
        "draw.rectangle.add",
        json!({"elementId": el, "frame": 1, "from": {"x": -10, "y": -10}, "to": {"x": 10, "y": 10}, "layer": "new"}),
    );
    ok(h, "draw.drawing.get", json!({"elementId": el, "frame": 1}))["drawingId"].as_str().unwrap().to_owned()
}

#[test]
fn every_path_into_the_font_list_waits_at_most_the_bound_or_not_at_all() {
    tt_vector::fonts::hold_scan();
    tt_vector::fonts::set_wait_bound(Some(BOUND));
    let h = fixture();
    assert!(!tt_vector::fonts::is_ready(), "the font read is held");
    let plain = plain_drawing(&h);
    ok(&h, "app.context.set", json!({"elementId": BOB, "frame": 1}));
    let hide_bob = |h: &ExecutorHandle, hidden: bool| {
        ok(h, "element.visibility.set", json!({"elementIds": [BOB], "scope": "scene", "visible": !hidden}));
    };
    let notices = h.subscribe();
    use Expect::{NoWait, Waits};
    let cases = vec![
        // Text layout and bounds, through the render list (b): a frame that draws text waits, one that does not never.
        case("render list: a frame drawing text", "render.frame.run", json!({"scale": 0.1}), Waits),
        case("render list: a frame description", "inspect.frame.describe", json!({"detail": "full"}), Waits),
        case(
            "render list: a hit test",
            "inspect.hitTest",
            json!({"space": "camera", "point": {"x": 0, "y": 0}}),
            Waits,
        ),
        case(
            "render list: a contact sheet",
            "inspect.contactSheet.render",
            json!({"frames": [1], "cellWidth": 40}),
            Waits,
        ),
        case(
            "render: a drawing in drawing space",
            "render.frame.run",
            json!({"composition": "element", "drawingId": D1, "scale": 0.1}),
            Waits,
        ),
        case(
            "render: a drawing without text in drawing space",
            "render.frame.run",
            json!({"composition": "element", "drawingId": plain, "scale": 0.1}),
            NoWait,
        ),
        // Drawing bounds and the selection.
        case("drawing bounds", "draw.bounds.get", json!({"drawingId": D1}), Waits),
        case("drawing bounds without text", "draw.bounds.get", json!({"drawingId": plain}), NoWait),
        case("drawing read with bounds", "draw.drawing.get", json!({"drawingId": D1, "include": ["bounds"]}), Waits),
        case(
            "drawing read with bounds, flat",
            "draw.drawing.get",
            json!({"drawingId": D1, "flatten": true, "include": ["bounds"]}),
            Waits,
        ),
        case("drawing read without bounds", "draw.drawing.get", json!({"drawingId": D1, "include": []}), NoWait),
        case(
            "selection by rectangle",
            "draw.selection.set",
            json!({"drawingId": D1, "by": "rect", "rect": {"x": -200, "y": -200, "width": 400, "height": 400}}),
            Waits,
        ),
        // Transforms, cut, colour and stroke operations over a drawing with text.
        case(
            "transform of the caption",
            "draw.transform.apply",
            json!({"drawingId": D1, "ids": [CAPTION], "rotate": 15}),
            Waits,
        ),
        case(
            "rotate of the caption",
            "draw.rotate.apply",
            json!({"drawingId": D1, "ids": [CAPTION], "by": "90cw"}),
            Waits,
        ),
        case(
            "flip of the caption",
            "draw.flip.apply",
            json!({"drawingId": D1, "ids": [CAPTION], "axis": "horizontal"}),
            Waits,
        ),
        case(
            "transform of a line (no text)",
            "draw.transform.apply",
            json!({"drawingId": D1, "ids": [PENCIL], "translate": {"x": 1, "y": 0}}),
            NoWait,
        ),
        case(
            "envelope of the caption",
            "draw.envelope.apply",
            json!({"drawingId": D1, "ids": [CAPTION], "corners": [{"x": -50, "y": -50}, {"x": 50, "y": -50}, {"x": 30, "y": 50}, {"x": -30, "y": 50}]}),
            Waits,
        ),
        case(
            "reposition all with corners",
            "draw.repositionAll.apply",
            json!({"elementId": BOB, "corners": [{"x": -50, "y": -50}, {"x": 50, "y": -50}, {"x": 30, "y": 50}, {"x": -30, "y": 50}]}),
            Waits,
        ),
        case(
            "cut with nodes",
            "draw.cut.apply",
            json!({"drawingId": D1, "rect": {"x": -300, "y": -300, "width": 900, "height": 900}}),
            Waits,
        ),
        case("paint by point", "colour.paint.apply", json!({"drawingId": D1, "points": [{"x": 500, "y": 500}]}), Waits),
        case("break apart", "draw.breakApart.apply", json!({"drawingId": D1, "textIds": [CAPTION]}), Waits),
        // Glyphs and text editing.
        case(
            "text add",
            "draw.text.add",
            json!({"elementId": BOB, "frame": 1, "position": {"x": -100, "y": 300}, "text": "Hi"}),
            Waits,
        ),
        case(
            "text set",
            "draw.text.set",
            json!({"drawingId": D1, "textId": CAPTION, "insert": {"at": 5, "text": "!"}}),
            Waits,
        ),
        case(
            "text format",
            "draw.textFormat.set",
            json!({"drawingId": D1, "textId": CAPTION, "start": 0, "length": 2, "bold": true}),
            Waits,
        ),
        // The animation evaluator's boxes.
        case("projection of Bob's box", "anim.projection.get", json!({"frame": 1, "nodeIds": [BOB]}), Waits),
        // The missing-fonts check.
        case("doc.validate with the font check", "doc.validate", json!({"checks": ["missingFonts"]}), Waits),
        case(
            "doc.validate without it",
            "doc.validate",
            json!({"minSeverity": "error", "checks": ["unusedAssets"]}),
            NoWait,
        ),
    ];
    let mut waited = Vec::new();
    let mut report = Vec::new();
    for c in &cases {
        let before = rev(&h);
        let t0 = Instant::now();
        let env = run(&h, c.command, c.params.clone());
        let took = t0.elapsed();
        let loading = reason(&env).as_deref() == Some("fontsLoading");
        report.push(format!(
            "{:<48} {:>6} ms {}",
            c.entry,
            took.as_millis(),
            if loading { "fontsLoading" } else { "answered" }
        ));
        match c.expect {
            Waits => {
                assert!(loading, "{}: {} answered {env:?}", c.entry, c.command);
                let e = env.error.as_ref().unwrap();
                assert_eq!(e.code, "command.timeout", "{}", c.entry);
                assert!(e.hint.as_deref().is_some_and(|h| h.contains("retry")), "{}: {e:?}", c.entry);
                assert!(took >= BOUND, "{}: answered after {took:?}, before the bound", c.entry);
                assert!(took < BOUND + Duration::from_secs(10), "{}: waited {took:?}", c.entry);
                assert_eq!(rev(&h), before, "{}: nothing changed", c.entry);
                waited.push(c);
            }
            NoWait => {
                assert!(!loading, "{}: {} waited for the fonts: {env:?}", c.entry, c.command);
                assert!(took < BOUND, "{}: took {took:?}", c.entry);
            }
        }
    }
    // (b) A frame that draws no text never waits: with Bob (the only element whose drawing holds text) hidden.
    hide_bob(&h, true);
    for (entry, command, params) in [
        ("frame without drawn text", "render.frame.run", json!({"scale": 0.1})),
        ("its description", "inspect.frame.describe", json!({"detail": "full"})),
        ("its hit test", "inspect.hitTest", json!({"space": "camera", "point": {"x": 0, "y": 0}})),
    ] {
        let t0 = Instant::now();
        let env = run(&h, command, params);
        assert!(reason(&env).as_deref() != Some("fontsLoading") && t0.elapsed() < BOUND, "{entry}: {env:?}");
        report.push(format!("{entry:<48} {:>6} ms answered", t0.elapsed().as_millis()));
    }
    hide_bob(&h, false);
    // The Text tool's gesture (a click in the Drawing view) waits; the window's state reads never do.
    ok(&h, "app.context.set", json!({"elementId": BOB, "frame": 1}));
    let t0 = Instant::now();
    let click = json!({"view": "drawingView", "tool": "text", "space": "drawing", "events": [
        {"type": "down", "x": 0, "y": 0}, {"type": "up", "x": 0, "y": 0}]});
    let env = h.pointer(Origin::User, click.clone());
    assert_eq!(reason(&env).as_deref(), Some("fontsLoading"), "the Text tool's click: {env:?}");
    assert!(t0.elapsed() >= BOUND);
    report.push(format!("{:<48} {:>6} ms fontsLoading", "Text tool click", t0.elapsed().as_millis()));
    for view in ["cameraView", "drawingView"] {
        let t0 = Instant::now();
        assert!(h.canvas_scene(view).is_ok(), "{view}");
        assert!(t0.elapsed() < BOUND, "the window's {view} state read waited {:?}", t0.elapsed());
        report.push(format!("{:<48} {:>6} ms answered", format!("state read: {view}"), t0.elapsed().as_millis()));
    }
    // The window is told while a call waits, and when the list is read (waits for the states, D-45; R1S-07).
    let mut seen: Vec<Value> = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(30);
    while !(seen.contains(&json!({"waiting": true})) && seen.contains(&json!({"waiting": false})))
        && Instant::now() < deadline
    {
        if let Ok(e) = notices.recv_timeout(Duration::from_millis(200))
            && e.name == "shell.fontsLoading"
        {
            seen.push(e.payload);
        }
    }
    assert!(seen.contains(&json!({"waiting": true})) && seen.contains(&json!({"waiting": false})), "{seen:?}");
    // Released: the list is read and every refused call succeeds on retry.
    let after = h.subscribe();
    tt_vector::fonts::release_scan();
    assert!(tt_vector::fonts::ready_within(Duration::from_secs(60)).is_ok(), "the font list is read once released");
    assert!(tt_vector::fonts::scan_time().is_some());
    for c in waited {
        let env = run(&h, c.command, c.params.clone());
        assert!(reason(&env).as_deref() != Some("fontsLoading"), "{}: retry: {env:?}", c.entry);
        assert!(env.error.as_ref().is_none_or(|e| e.code != "command.timeout"), "{}: retry: {env:?}", c.entry);
    }
    let env = h.pointer(Origin::User, click);
    assert!(reason(&env).as_deref() != Some("fontsLoading"), "the Text tool's click on retry: {env:?}");
    // The notice that the list is read: waited for on the receiver, not after a fixed sleep (D-45; R1S-07).
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut ready = false;
    while !ready && Instant::now() < deadline {
        ready = after
            .recv_timeout(Duration::from_millis(200))
            .is_ok_and(|e| e.name == "shell.fontsLoading" && e.payload == json!({"waiting": false}));
    }
    assert!(ready, "no shell.fontsLoading {{waiting: false}} within 30 s of the release");
    println!("{}", report.join("\n"));
}
