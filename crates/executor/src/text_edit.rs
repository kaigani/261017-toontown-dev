//! M4 C4.4b, the Text tool (02 §7.6 lines 664-681; design note 5 §2-§4; SI-191, SI-193, SI-194, SI-195): the edit
//! session, the hit zones of a press, the gesture rows, the one input path that typing, editing keys, `type_text` and
//! paste go through, and the settling that ends a session.
//!
//! The kernel owns the session ([`TextEdit`], in [`crate::app::AppContext`], transient like the point selection).
//! Opening it, placing the caret and highlighting are not commands; every document change is a command an agent can
//! send (`draw.text.add`, `draw.text.set`, `draw.textFormat.set`, `draw.object.delete`). A keystroke's commands run
//! with the typing key set, so they extend the open typing run (`History::extend_top`): one "Typing" step per run.

use serde_json::{Map, Value, json};
use tt_model::entities::drawing::Drawing;
use tt_registry::CommandError;
use tt_vector::drawing::{self as vd, ObjectKind};
use tt_vector::geom::{Affine, P};
use tt_vector::text::{self, Content};

use crate::cx::Cx;
use crate::gesture::{Gesture, Sample};
use crate::run::Core;
use crate::shell::Point;
use crate::types::{Envelope, Invocation, InvokeOptions, Origin};

/// The tool id.
pub const TOOL: &str = "text";
/// The width handles' hit radius, view pixels (SI-131's handle radius).
pub const HANDLE_PX: f64 = 6.0;
/// The frame band, view pixels either side of a box's outline (SI-194).
pub const FRAME_PX: f64 = 4.0;

/// The edit session (design note 5 §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextEdit {
    /// The drawing (`dr_...`).
    pub drawing: String,
    /// The text object's local id.
    pub text: String,
    /// The view the session was opened in.
    pub view_id: String,
    /// The origin that opened it (`Origin::label`).
    pub origin: String,
    /// Highlight anchor and caret, code points.
    pub anchor: usize,
    pub caret: usize,
    /// The x Up and Down keep (f64 bits), until a horizontal move.
    pub goal_x: Option<u64>,
    /// Character fields set at the caret with nothing highlighted (SI-195): the next keystroke applies them.
    pub pending: Map<String, Value>,
    /// The element and frame current when it opened: a change of either ends it.
    pub element: Option<String>,
    pub frame: u32,
}

impl TextEdit {
    /// `<drawingId>/<textId>`.
    pub fn qualified(&self) -> String {
        format!("{}/{}", self.drawing, self.text)
    }

    fn range(&self) -> (usize, usize) {
        (self.anchor.min(self.caret), self.anchor.max(self.caret))
    }
}

/// A text object of the current drawing as the hit test and the scene see it.
#[derive(Debug, Clone)]
pub(crate) struct TextBox {
    pub id: String,
    pub transform: Affine,
    pub width: Option<f64>,
    pub content: Content,
    /// The box's width and height (local du).
    pub size: (f64, f64),
}

impl TextBox {
    /// The box's corners in drawing units: top-left, top-right, bottom-right, bottom-left.
    pub fn quad(&self) -> [P; 4] {
        let (w, h) = self.size;
        [P::new(0.0, 0.0), P::new(w, 0.0), P::new(w, -h), P::new(0.0, -h)].map(|p| self.transform.apply(p))
    }

    /// The width handles in drawing units: the middle of the left and right edges.
    pub fn handles(&self) -> [P; 2] {
        let (w, h) = self.size;
        [P::new(0.0, -h / 2.0), P::new(w, -h / 2.0)].map(|p| self.transform.apply(p))
    }

    /// A drawing point in the box's local frame.
    fn local(&self, p: P) -> P {
        let [a, b, c, d, e, f] = self.transform.0;
        let det = a * d - b * c;
        if det.abs() < 1e-300 {
            return p;
        }
        let (x, y) = (p.x - e, p.y - f);
        P::new((d * x - c * y) / det, (-b * x + a * y) / det)
    }
}

/// The text objects of `d`, back to front.
pub(crate) fn boxes(d: &Drawing) -> Vec<TextBox> {
    vd::objects(d)
        .into_iter()
        .filter(|o| o.kind == ObjectKind::Text)
        .filter_map(|o| {
            let n = vd::node_at(d, &o.pointer)?;
            let content = vd::text_content(n)?;
            let width =
                if *n.width_mode() == Some(tt_model::entities::drawing::WidthMode::Fixed) { *n.width() } else { None };
            let t = n.transform().unwrap_or(tt_model::value::IDENTITY);
            let l = text::layout(&content, width);
            Some(TextBox { id: o.id, transform: Affine(t), width, content, size: (l.width, l.height) })
        })
        .collect()
}

fn current(cx: &Cx) -> Option<Drawing> {
    let doc = cx.document().ok()?;
    let id = cx.app().current_drawing(doc)?;
    doc.drawing(&id).cloned()
}

/// What a press is on (design note 5 §2), in this order.
#[derive(Debug, Clone, PartialEq)]
enum Zone {
    /// A width handle of the edited object (`right`: the right edge's).
    Handle {
        right: bool,
    },
    /// The frame band of a text object.
    Frame(String),
    /// Inside a text box.
    Inside(String),
    Empty,
}

fn dist_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((p.x - a.x) * dx + (p.y - a.y) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (p.x - (a.x + t * dx)).hypot(p.y - (a.y + t * dy))
}

fn inside_quad(p: Point, q: &[Point; 4]) -> bool {
    let mut sign = 0.0;
    for i in 0..4 {
        let (a, b) = (q[i], q[(i + 1) % 4]);
        let c = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
        if c == 0.0 {
            continue;
        }
        if sign == 0.0 {
            sign = c.signum();
        } else if c.signum() != sign {
            return false;
        }
    }
    true
}

fn zone_at(view: &dyn Fn(Point) -> Point, boxes: &[TextBox], edited: Option<&str>, q: Point) -> Zone {
    let vp = |p: P| view(Point { x: p.x, y: p.y });
    if let Some(e) = edited
        && let Some(b) = boxes.iter().find(|b| b.id == e)
    {
        let h = b.handles();
        for (right, hp) in [(false, h[0]), (true, h[1])] {
            let v = vp(hp);
            if (v.x - q.x).hypot(v.y - q.y) <= HANDLE_PX {
                return Zone::Handle { right };
            }
        }
    }
    // Front-most first.
    for b in boxes.iter().rev() {
        let v = b.quad().map(vp);
        if (0..4).any(|i| dist_to_segment(q, v[i], v[(i + 1) % 4]) <= FRAME_PX) {
            return Zone::Frame(b.id.clone());
        }
    }
    for b in boxes.iter().rev() {
        if inside_quad(q, &b.quad().map(vp)) {
            return Zone::Inside(b.id.clone());
        }
    }
    Zone::Empty
}

fn view_of<'a>(cx: &'a Cx, g: &Gesture) -> Result<impl Fn(Point) -> Point + 'a, CommandError> {
    let w = cx.ui().shell().view(&g.view_id)?;
    let camera = w.ty == crate::shell::ViewType::Camera;
    let inv = crate::camera_view::invert(&g.press);
    Ok(move |p: Point| match inv {
        Some(m) if camera => crate::shell::apply(&m, p),
        _ => crate::commands::view::to_view(w, p),
    })
}

/// The Text panel defaults as `draw.text.add`'s `format` (02 §7.6.1 line 1202; C4.4a's `tool.text.options.set`).
pub(crate) fn panel_format(cx: &Cx) -> Value {
    let o = cx.ui().drawing().options_of(TOOL);
    let mut f = Map::new();
    for k in [
        "font",
        "size",
        "bold",
        "italic",
        "baseline",
        "kerning",
        "autoKern",
        "align",
        "leftMargin",
        "rightMargin",
        "indent",
        "lineSpacing",
        "outlineColourId",
    ] {
        if let Some(v) = o.get(k).filter(|v| !v.is_null()) {
            f.insert(k.to_owned(), v.clone());
        }
    }
    Value::Object(f)
}

fn pt(p: Point) -> Value {
    json!({ "x": p.x, "y": p.y })
}

/// The commands of a Text tool gesture (design note 5 §2's table). A press inside a box changes only the session (no
/// command; [`after_gesture`] places the caret).
///
/// # Errors
/// The view's refusal; `arg.invalid` for a gesture without samples.
pub(crate) fn commands(cx: &Cx, g: &Gesture) -> Result<Vec<(String, Value)>, CommandError> {
    let Some(st) = g.strokes.first() else {
        return Err(CommandError::with_details(
            "arg.invalid",
            "the gesture has no samples",
            json!({ "pointer": "/events" }),
        ));
    };
    let (Some(p0), Some(pn)) = (st.samples.first().copied(), st.samples.last().copied()) else {
        return Err(CommandError::with_details(
            "arg.invalid",
            "the gesture has no samples",
            json!({ "pointer": "/events" }),
        ));
    };
    let d = current(cx);
    let all = d.as_ref().map(boxes).unwrap_or_default();
    let session = cx.app().text_edit.clone().filter(|s| d.as_ref().is_some_and(|d| d.id().as_str() == s.drawing));
    let view = view_of(cx, g)?;
    let zone = zone_at(&view, &all, session.as_ref().map(|s| s.text.as_str()), p0.v);
    let drawing = d.as_ref().map(|d| d.id().as_str().to_owned());
    match zone {
        Zone::Handle { right } if !st.is_click() => {
            let (Some(s), Some(drawing)) = (&session, &drawing) else { return Ok(Vec::new()) };
            let Some(b) = all.iter().find(|b| b.id == s.text) else { return Ok(Vec::new()) };
            Ok(vec![width_drag(b, drawing, right, p0, pn)])
        }
        Zone::Frame(id) if !st.is_click() => {
            let (Some(drawing), Some(b)) = (&drawing, all.iter().find(|b| b.id == id)) else { return Ok(Vec::new()) };
            let t = b.transform.0;
            let at = Point { x: t[4] + pn.d.x - p0.d.x, y: t[5] + pn.d.y - p0.d.y };
            Ok(vec![("draw.text.set".into(), json!({ "drawingId": drawing, "textId": id, "position": pt(at) }))])
        }
        Zone::Inside(id) | Zone::Frame(id) => {
            // A click (or drag) inside another object ends the session there first.
            Ok(ending(cx, session.as_ref(), Some(&id)).into_iter().collect())
        }
        Zone::Handle { .. } => Ok(Vec::new()),
        Zone::Empty => {
            let mut out: Vec<(String, Value)> = ending(cx, session.as_ref(), None).into_iter().collect();
            let mut v = json!({ "text": "", "format": panel_format(cx) });
            if st.is_click() {
                v["position"] = pt(p0.d);
            } else {
                v["position"] = pt(Point { x: p0.d.x.min(pn.d.x), y: p0.d.y.max(pn.d.y) });
                v["width"] = json!((pn.d.x - p0.d.x).abs().max(1.0));
            }
            if let Some(doc) = cx.document().ok()
                && let Some(c) = crate::commands::draw_stroke::current_colour(cx.app(), doc)
            {
                v["colourId"] = json!(c);
            }
            out.push(("draw.text.add".into(), v));
            Ok(out)
        }
    }
}

/// The deletion that ends a session on an empty object (02 line 681), unless the press is on that object.
fn ending(cx: &Cx, session: Option<&TextEdit>, keep: Option<&str>) -> Option<(String, Value)> {
    let s = session?;
    if keep == Some(s.text.as_str()) || !is_empty(cx, s) {
        return None;
    }
    Some(("draw.object.delete".into(), json!({ "drawingId": s.drawing, "ids": [s.text] })))
}

fn content_of(cx: &Cx, s: &TextEdit) -> Option<(TextBox, Drawing)> {
    let doc = cx.document().ok()?;
    let id = tt_model::Id::parse(&s.drawing).ok()?;
    let d = doc.drawing(&id)?.clone();
    let b = boxes(&d).into_iter().find(|b| b.id == s.text)?;
    Some((b, d))
}

fn is_empty(cx: &Cx, s: &TextEdit) -> bool {
    content_of(cx, s).is_some_and(|(b, _)| b.content.chars.is_empty())
}

/// A width handle drag (02 lines 672-673): the right handle sets the width, the left one the width and the position,
/// the width along the box's local x and at least 1 du.
fn width_drag(b: &TextBox, drawing: &str, right: bool, p0: Sample, pn: Sample) -> (String, Value) {
    let (w0, _) = b.size;
    let l0 = b.local(P::new(p0.d.x, p0.d.y));
    let l1 = b.local(P::new(pn.d.x, pn.d.y));
    let dx = l1.x - l0.x;
    if right {
        let w = (w0 + dx).max(1.0);
        ("draw.text.set".into(), json!({ "drawingId": drawing, "textId": b.id, "width": w }))
    } else {
        let w = (w0 - dx).max(1.0);
        let at = b.transform.apply(P::new(w0 - w, 0.0));
        (
            "draw.text.set".into(),
            json!({ "drawingId": drawing, "textId": b.id, "width": w, "position": { "x": at.x, "y": at.y } }),
        )
    }
}

/// After a Text tool gesture's commands ran (design note 5 §2): a new object opens a session at caret 0; a press
/// inside a box opens one there with the caret at the press and release (a double click: the word).
pub(crate) fn after_gesture(core: &mut Core, origin: &Origin, g: &Gesture, answered: &[(String, Envelope)]) {
    if g.tool != TOOL {
        return;
    }
    place_after(core, origin, g, answered);
    notify(core);
}

fn place_after(core: &mut Core, origin: &Origin, g: &Gesture, answered: &[(String, Envelope)]) {
    if let Some((_, env)) = answered.iter().rev().find(|(c, e)| c == "draw.text.add" && e.is_ok()) {
        let r = env.result.clone().unwrap_or_default();
        let (Some(d), Some(t)) = (r["drawingId"].as_str(), r["textId"].as_str()) else { return };
        open(core, origin, &g.view_id, d, t, 0, 0);
        return;
    }
    if answered.iter().any(|(_, e)| !e.is_ok()) {
        return;
    }
    let Some(st) = g.strokes.first() else { return };
    let (Some(p0), Some(pn)) = (st.samples.first().copied(), st.samples.last().copied()) else { return };
    let placed = core.with_lent_session(|cx| {
        let d = current(cx)?;
        let all = boxes(&d);
        let view = view_of(cx, g).ok()?;
        let edited = cx.app().text_edit.as_ref().map(|s| s.text.clone());
        let zone = zone_at(&view, &all, edited.as_deref(), p0.v);
        let id = match zone {
            Zone::Inside(id) => id,
            Zone::Frame(id) if st.is_click() => id,
            _ => return None,
        };
        {
            {
                let b = all.iter().find(|b| b.id == id)?;
                let a = text::caret_at(&b.content, b.width, b.local(P::new(p0.d.x, p0.d.y)));
                let c = text::caret_at(&b.content, b.width, b.local(P::new(pn.d.x, pn.d.y)));
                let (a, c) = if g.double_click { text::word_at(&b.content, a) } else { (a, c) };
                Some((d.id().as_str().to_owned(), id, a, c))
            }
        }
    });
    if let Some(Some((d, t, a, c))) = placed {
        open(core, origin, &g.view_id, &d, &t, a, c);
    }
}

/// Opens (or moves) the session; any typing run ends (a caret placement ends it).
fn open(core: &mut Core, origin: &Origin, view: &str, drawing: &str, text: &str, anchor: usize, caret: usize) {
    let Some(s) = core.session_mut() else { return };
    let element = s.app.element.as_ref().map(|e| e.as_str().to_owned());
    let frame = s.app.frame;
    s.app.text_edit = Some(TextEdit {
        drawing: drawing.to_owned(),
        text: text.to_owned(),
        view_id: view.to_owned(),
        origin: origin.label(),
        anchor,
        caret,
        goal_x: None,
        pending: Map::new(),
        element,
        frame,
    });
    if let Some(p) = s.project.as_mut() {
        p.history.end_run();
    }
}

fn invoke(core: &mut Core, origin: &Origin, command: &str, params: Value) -> Envelope {
    core.invoke(&Invocation {
        command: command.to_owned(),
        params,
        options: InvokeOptions::default(),
        origin: origin.clone(),
    })
}

/// Ends the session when its condition holds (design note 5 §2, "Ending the session"): another tool, another current
/// drawing, element or frame, the drawing locked, the object removed. Ending deletes the object when it exists and is
/// empty (02 line 681), with the origin of the call that ended it; a locked drawing or a removed object ends it without
/// a deletion. Called after every accepted invocation.
pub(crate) fn settle(core: &mut Core, origin: &Origin) {
    let Some(state) = core.with_lent_session(|cx| {
        let s = cx.app().text_edit.clone()?;
        let tool_left = cx.ui().drawing().active_tool != TOOL;
        let doc = cx.document().ok();
        let here = doc.and_then(|doc| cx.app().current_drawing(doc)).map(|d| d.as_str().to_owned());
        let element = cx.app().element.as_ref().map(|e| e.as_str().to_owned());
        let moved = here.as_deref() != Some(s.drawing.as_str()) || element != s.element || cx.app().frame != s.frame;
        let found = content_of(cx, &s);
        let locked = found.as_ref().is_some_and(|(_, d)| *d.locked());
        let gone = found.is_none();
        let empty = found.as_ref().is_some_and(|(b, _)| b.content.chars.is_empty());
        Some((s, tool_left || moved, locked, gone, empty))
    }) else {
        return;
    };
    let Some((s, left, locked, gone, empty)) = state else { return };
    if !(left || locked || gone) {
        // The session stays: its caret and highlight follow the text (an undo, a redo or another origin's edit can
        // shorten it), then the panel's shown values follow the call (an agent's `draw.textFormat.set`, D-40).
        reconcile(core);
        notify(core);
        return;
    }
    end(core, origin, &s, empty && !locked && !gone);
}

/// Reconciles the session's caret and highlight with its object's text (finding C44B-R1S-02): after an undo, a redo
/// or another origin's edit the text can be shorter than the indices; both are clamped to its length (and the goal
/// column dropped), so the next input inserts at a valid place and the scene answer finds the caret's stop.
fn reconcile(core: &mut Core) {
    let Some(Some(n)) = core.with_lent_session(|cx| {
        let s = cx.app().text_edit.clone()?;
        content_of(cx, &s).map(|(b, _)| b.content.chars.len())
    }) else {
        return;
    };
    set_session(core, |e| {
        if e.caret > n || e.anchor > n {
            e.caret = e.caret.min(n);
            e.anchor = e.anchor.min(n);
            e.goal_x = None;
        }
    });
}

/// Ends the session, deleting its object first when `delete`.
fn end(core: &mut Core, origin: &Origin, s: &TextEdit, delete: bool) {
    if let Some(sess) = core.session_mut() {
        sess.app.text_edit = None;
        if let Some(p) = sess.project.as_mut() {
            p.history.end_run();
        }
    }
    notify(core);
    if delete {
        let _ = invoke(core, origin, "draw.object.delete", json!({ "drawingId": s.drawing, "ids": [s.text] }));
    }
}

/// One input of the input path (design note 5 §4).
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    /// Typed characters (a composition's result counts once).
    Text(String),
    /// An editing key, canonical (`Left`, `Shift+Left`, `Backspace`, `Enter`, `Escape`, `Primary+A`, ...).
    Key(String),
    /// Pasted characters (one step).
    Paste(String),
    /// Cut the highlight (one step; the window wrote it to the clipboard).
    Cut,
}

/// The editing keys the input path takes before the keymap (design note 5 §4).
#[must_use]
pub fn is_editing_key(keys: &str) -> bool {
    matches!(
        keys,
        "Left"
            | "Right"
            | "Up"
            | "Down"
            | "Home"
            | "End"
            | "Shift+Left"
            | "Shift+Right"
            | "Shift+Up"
            | "Shift+Down"
            | "Shift+Home"
            | "Shift+End"
            | "Backspace"
            | "Delete"
            | "Enter"
            | "Escape"
            | "Tab"
            | "Primary+A"
    )
}

/// The session of `origin`, refused when there is none (`arg.missing`) or another origin's is open (`ui.busy`).
fn session_of(core: &mut Core, origin: &Origin) -> Result<TextEdit, CommandError> {
    let s = core.with_lent_session(|cx| cx.app().text_edit.clone()).flatten();
    match s {
        None => Err(CommandError::with_details(
            "arg.missing",
            "no text is being edited: click with the Text tool first (pointer), or give a target",
            json!({ "pointer": "/target" }),
        )),
        Some(s) if s.origin != origin.label() => {
            Err(CommandError::with_details("ui.busy", "another caller is editing text", json!({ "editing": s.origin })))
        }
        Some(s) => Ok(s),
    }
}

fn set_session(core: &mut Core, f: impl FnOnce(&mut TextEdit)) {
    if let Some(s) = core.session_mut()
        && let Some(t) = s.app.text_edit.as_mut()
    {
        f(t);
    }
}

fn end_run(core: &mut Core) {
    if let Some(p) = core.session_mut().and_then(|s| s.project.as_mut()) {
        p.history.end_run();
    }
}

/// The length of the session's text, code points.
fn length(core: &mut Core, s: &TextEdit) -> Option<usize> {
    core.with_lent_session(|cx| content_of(cx, s).map(|(b, _)| b.content.chars.len())).flatten()
}

/// Runs one input (design note 5 §3-§4); the answered commands, in order.
///
/// # Errors
/// `arg.missing` without a session, `ui.busy` for another origin's; a refused command's error.
pub(crate) fn input(
    core: &mut Core,
    origin: &Origin,
    i: Input,
) -> Result<Vec<(String, Value, Envelope)>, CommandError> {
    session_of(core, origin)?;
    // The indices are valid for the text as it is now, whatever changed it since the last call (C44B-R1S-02).
    reconcile(core);
    let s = session_of(core, origin)?;
    let mut out = Vec::new();
    run_input(core, origin, &s, i, &mut out);
    notify(core);
    Ok(out)
}

fn run_input(core: &mut Core, origin: &Origin, s: &TextEdit, i: Input, out: &mut Vec<(String, Value, Envelope)>) {
    let s = s.clone();
    match i {
        Input::Text(t) => type_text(core, origin, &s, &t, true, out),
        Input::Paste(t) => {
            end_run(core);
            type_text(core, origin, &s, &t, false, out);
            end_run(core);
        }
        Input::Cut => {
            let (a, b) = s.range();
            if b > a {
                end_run(core);
                let p = json!({ "drawingId": s.drawing, "textId": s.text, "delete": { "start": a, "length": b - a } });
                let env = invoke(core, origin, "draw.text.set", p.clone());
                let ok = env.is_ok();
                out.push(("draw.text.set".to_owned(), p, env));
                if ok {
                    set_session(core, |t| {
                        t.anchor = a;
                        t.caret = a;
                    });
                }
                end_run(core);
            }
        }
        Input::Key(k) => key(core, origin, &s, &k, out),
    }
}

/// The host-only notice of the session (design note 5 §5; SI-192): `shell.textEdit {drawingId, textId, viewId,
/// anchor, caret, format}` or `null`, for the Text panel. MCP clients never receive `shell.*` notices; it carries nothing
/// an agent cannot read through `inspect_ui` and `draw.drawing.get`.
pub(crate) fn notify(core: &mut Core) {
    let payload = core
        .with_lent_session(|cx| {
            let s = cx.app().text_edit.clone()?;
            let (b, _) = content_of(cx, &s)?;
            Some(json!({ "drawingId": s.drawing, "textId": s.text, "viewId": s.view_id, "anchor": s.anchor,
                         "caret": s.caret, "format": shown_format(&b.content, &s) }))
        })
        .flatten();
    if let Some(sess) = core.session_mut() {
        sess.emit("shell.textEdit", payload.unwrap_or(Value::Null));
    }
}

/// The values the Text panel shows (design note 5 §6): per field, the value over the range (character fields over the
/// runs it covers, paragraph fields over the paragraphs it touches), `null` where they differ; at a caret, the run
/// before it with the pending fields applied.
pub(crate) fn shown_format(c: &Content, s: &TextEdit) -> Value {
    let (lo, hi) = s.range();
    let n = c.chars.len();
    let runs: Vec<&text::Run> = if hi > lo {
        let mut pos = 0usize;
        c.runs
            .iter()
            .filter(|r| {
                let (a, b) = (pos, pos + r.length as usize);
                pos = b;
                b > lo && a < hi
            })
            .collect()
    } else {
        vec![c.run_at(lo.saturating_sub(1).min(n.saturating_sub(1)))]
    };
    let paras: Vec<&text::Para> = c
        .paras
        .iter()
        .filter(|p| {
            let (a, b) = (p.start as usize, (p.start + p.length) as usize);
            (hi > lo && b > lo && a < hi) || (a <= lo && (lo < b || b == n))
        })
        .collect();
    let same = |vals: Vec<Value>| -> Value {
        match vals.split_first() {
            Some((first, rest)) if rest.iter().all(|v| v == first) => first.clone(),
            Some(_) => Value::Null,
            None => Value::Null,
        }
    };
    let mut f = Map::new();
    f.insert("font".into(), same(runs.iter().map(|r| json!(r.font)).collect()));
    f.insert("size".into(), same(runs.iter().map(|r| json!(r.size)).collect()));
    f.insert("bold".into(), same(runs.iter().map(|r| json!(r.bold)).collect()));
    f.insert("italic".into(), same(runs.iter().map(|r| json!(r.italic)).collect()));
    f.insert("baseline".into(), same(runs.iter().map(|r| json!(r.script)).collect()));
    f.insert("kerning".into(), same(runs.iter().map(|r| json!(r.kerning)).collect()));
    f.insert("autoKern".into(), same(runs.iter().map(|r| json!(r.auto_kerning)).collect()));
    f.insert("colourId".into(), same(runs.iter().map(|r| json!(r.colour_id)).collect()));
    f.insert("align".into(), same(paras.iter().map(|p| json!(p.alignment)).collect()));
    f.insert("leftMargin".into(), same(paras.iter().map(|p| json!(p.margin_left)).collect()));
    f.insert("rightMargin".into(), same(paras.iter().map(|p| json!(p.margin_right)).collect()));
    f.insert("indent".into(), same(paras.iter().map(|p| json!(p.indent)).collect()));
    f.insert("lineSpacing".into(), same(paras.iter().map(|p| json!(p.line_spacing)).collect()));
    if hi == lo {
        for (k, v) in &s.pending {
            f.insert(k.clone(), v.clone());
        }
    }
    Value::Object(f)
}

/// Inserts `t` at the caret (over the highlight), then the pending format over it, both in the typing run when
/// `typing`; moves the caret after it.
fn type_text(
    core: &mut Core,
    origin: &Origin,
    s: &TextEdit,
    t: &str,
    typing: bool,
    out: &mut Vec<(String, Value, Envelope)>,
) {
    if t.is_empty() {
        return;
    }
    let (a, b) = s.range();
    let Some(before) = length(core, s) else { return };
    // 02 §7.6: typed text takes the current colour (design note 5 §6; finding C44B-R1S-03). The inserted characters
    // inherit the preceding run's format; when its colour is not the current one, the current colour is applied to them
    // with the rest of the pending format (an explicit pending colour wins).
    let colour = core
        .with_lent_session(|cx| {
            let doc = cx.document().ok()?;
            let current = crate::commands::draw_stroke::current_colour(cx.app(), doc)?;
            let (bx, _) = content_of(cx, s)?;
            let inherited =
                if bx.content.runs.is_empty() { None } else { bx.content.run_at(a.saturating_sub(1)).colour_id.clone() };
            (inherited.as_deref() != Some(current.as_str())).then_some(current)
        })
        .flatten();
    let mut format = s.pending.clone();
    if let Some(c) = colour
        && !format.contains_key("colourId")
    {
        format.insert("colourId".to_owned(), json!(c));
    }
    let mut p = json!({ "drawingId": s.drawing, "textId": s.text, "insert": { "at": a, "text": t } });
    if b > a {
        // Typing over a highlight: one call (the handler deletes first) beginning a new run.
        p["delete"] = json!({ "start": a, "length": b - a });
        end_run(core);
    }
    if typing {
        core.set_typing(Some(s.qualified()));
    }
    let env = invoke(core, origin, "draw.text.set", p.clone());
    let ok = env.is_ok();
    out.push(("draw.text.set".to_owned(), p, env));
    let after = length(core, s).unwrap_or(before);
    let n = (after + (b - a)).saturating_sub(before);
    if ok && !format.is_empty() {
        let mut f = Value::Object(format);
        f["drawingId"] = json!(s.drawing);
        f["textId"] = json!(s.text);
        f["start"] = json!(a);
        f["length"] = json!(n);
        let env = invoke(core, origin, "draw.textFormat.set", f.clone());
        out.push(("draw.textFormat.set".to_owned(), f, env));
    }
    core.set_typing(None);
    if ok {
        set_session(core, |e| {
            e.caret = a + n;
            e.anchor = a + n;
            e.goal_x = None;
            e.pending.clear();
        });
    }
}

fn key(core: &mut Core, origin: &Origin, s: &TextEdit, k: &str, out: &mut Vec<(String, Value, Envelope)>) {
    let n = length(core, s).unwrap_or(0);
    let (a, b) = s.range();
    let shift = k.starts_with("Shift+");
    let base = k.trim_start_matches("Shift+");
    let moved = |core: &mut Core, caret: usize, goal: Option<f64>| {
        end_run(core);
        set_session(core, |e| {
            e.caret = caret.min(n);
            if !shift {
                e.anchor = e.caret;
            }
            e.goal_x = goal.map(f64::to_bits);
            e.pending.clear();
        });
    };
    match base {
        "Left" => {
            let c = if !shift && b > a { a } else { s.caret.saturating_sub(1) };
            moved(core, c, None);
        }
        "Right" => {
            let c = if !shift && b > a { b } else { (s.caret + 1).min(n) };
            moved(core, c, None);
        }
        "Up" | "Down" | "Home" | "End" => {
            let geo = core.with_lent_session(|cx| content_of(cx, s)).flatten();
            let Some((bx, _)) = geo else { return };
            let goal = s.goal_x.map(f64::from_bits).unwrap_or_else(|| text::caret_x(&bx.content, bx.width, s.caret));
            let c = match base {
                "Up" => text::line_move(&bx.content, bx.width, s.caret, goal, -1),
                "Down" => text::line_move(&bx.content, bx.width, s.caret, goal, 1),
                _ => {
                    let stops = text::carets(&bx.content, bx.width);
                    let line = stops.iter().find(|c| c.index == s.caret).map_or(0, |c| c.line);
                    let on: Vec<usize> = stops.iter().filter(|c| c.line == line).map(|c| c.index).collect();
                    if base == "Home" { on.first().copied().unwrap_or(0) } else { on.last().copied().unwrap_or(n) }
                }
            };
            moved(core, c, matches!(base, "Up" | "Down").then_some(goal));
        }
        "Backspace" | "Delete" => {
            let del = if b > a {
                Some((a, b - a))
            } else if base == "Backspace" && s.caret > 0 {
                Some((s.caret - 1, 1))
            } else if base == "Delete" && s.caret < n {
                Some((s.caret, 1))
            } else {
                None
            };
            let Some((start, len)) = del else { return };
            if b > a {
                end_run(core);
            }
            let p = json!({ "drawingId": s.drawing, "textId": s.text, "delete": { "start": start, "length": len } });
            core.set_typing(Some(s.qualified()));
            let env = invoke(core, origin, "draw.text.set", p.clone());
            core.set_typing(None);
            let ok = env.is_ok();
            out.push(("draw.text.set".to_owned(), p, env));
            if ok {
                set_session(core, |e| {
                    e.caret = start;
                    e.anchor = start;
                    e.goal_x = None;
                });
            }
        }
        "Enter" => type_text(core, origin, s, "\n", true, out),
        "Escape" => {
            let empty = n == 0;
            end(core, origin, s, empty);
        }
        "Primary+A" => {
            end_run(core);
            set_session(core, |e| {
                e.anchor = 0;
                e.caret = n;
                e.goal_x = None;
                e.pending.clear();
            });
        }
        // Tab is consumed and inserts nothing (design note 5 §4).
        _ => {}
    }
}

/// `draw.textFormat.set` at the session's caret with nothing highlighted (SI-195): the session's own call with
/// character fields is kept as the pending format (no document change, no step); `true` when it was taken.
pub(crate) fn take_pending(cx: &mut Cx, params: &Value) -> bool {
    let Some(s) = cx.app().text_edit.clone() else { return false };
    if s.origin != cx.origin().label() || s.anchor != s.caret {
        return false;
    }
    let id = params.get("textId").and_then(Value::as_str).unwrap_or_default();
    let local = id.rsplit('/').next().unwrap_or(id);
    let at = params.get("start").and_then(Value::as_u64);
    let len = params.get("length").and_then(Value::as_u64).unwrap_or(0);
    if local != s.text || at != Some(s.caret as u64) || len != 0 {
        return false;
    }
    const CHARACTER: [&str; 9] =
        ["font", "size", "bold", "italic", "baseline", "kerning", "autoKern", "colourId", "outlineColourId"];
    let fields: Map<String, Value> = params
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(k, _)| CHARACTER.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let paragraph = params
        .as_object()
        .into_iter()
        .flatten()
        .any(|(k, _)| matches!(k.as_str(), "align" | "leftMargin" | "rightMargin" | "indent" | "lineSpacing"));
    if fields.is_empty() || paragraph {
        return false;
    }
    if let Some(t) = cx.app_mut().text_edit.as_mut() {
        t.pending.extend(fields);
    }
    // A format change ends the typing run (SI-191; design note 5 §3; finding C44B-R1S-01): the next keystroke opens a
    // new "Typing" step, so "A, bold, B" is two steps and an undo removes B only.
    if let Ok(p) = cx.project_mut() {
        p.history.end_run();
    }
    if let Some(s) = cx.app().text_edit.clone()
        && let Some((b, _)) = content_of(cx, &s)
    {
        let payload = json!({ "drawingId": s.drawing, "textId": s.text, "viewId": s.view_id, "anchor": s.anchor,
                              "caret": s.caret, "format": shown_format(&b.content, &s) });
        cx.emit("shell.textEdit", payload);
    }
    true
}

/// The session for the scene answer (design note 5 §5): the edited box, the caret's top and bottom, the highlight's
/// line quads and the width handles, in drawing units; and every text box of the current drawing.
pub(crate) fn scene_text(cx: &Cx) -> (Option<crate::canvas::SceneTextEdit>, Option<Vec<crate::canvas::SceneTextBox>>) {
    if cx.ui().drawing().active_tool != TOOL {
        return (None, None);
    }
    let Some(d) = current(cx) else { return (None, None) };
    let all = boxes(&d);
    let list: Vec<crate::canvas::SceneTextBox> = all
        .iter()
        .map(|b| crate::canvas::SceneTextBox { text_id: b.id.clone(), quad: b.quad().map(|p| [p.x, p.y]) })
        .collect();
    let edit = cx.app().text_edit.as_ref().filter(|s| s.drawing == d.id().as_str()).and_then(|s| {
        let b = all.iter().find(|b| b.id == s.text)?;
        let stops = text::carets(&b.content, b.width);
        let at = |i: usize| stops.iter().find(|c| c.index == i).copied();
        let k = at(s.caret)?;
        let caret = [b.transform.apply(P::new(k.x, k.top)), b.transform.apply(P::new(k.x, k.bottom))];
        let (lo, hi) = s.range();
        let mut highlight = Vec::new();
        if hi > lo {
            let mut line: Option<(usize, f64, f64, f64, f64)> = None;
            for c in stops.iter().filter(|c| c.index >= lo && c.index <= hi) {
                match &mut line {
                    Some((l, _, x1, _, _)) if *l == c.line => *x1 = c.x,
                    _ => {
                        if let Some((_, x0, x1, top, bottom)) = line.take() {
                            highlight.push([P::new(x0, top), P::new(x1, top), P::new(x1, bottom), P::new(x0, bottom)]);
                        }
                        line = Some((c.line, c.x, c.x, c.top, c.bottom));
                    }
                }
            }
            if let Some((_, x0, x1, top, bottom)) = line {
                highlight.push([P::new(x0, top), P::new(x1, top), P::new(x1, bottom), P::new(x0, bottom)]);
            }
        }
        let size = b.content.run_at(s.caret.saturating_sub(1)).size;
        Some(crate::canvas::SceneTextEdit {
            text_id: s.text.clone(),
            view_id: s.view_id.clone(),
            anchor: s.anchor,
            caret_index: s.caret,
            quad: b.quad().map(|p| [p.x, p.y]),
            caret: caret.map(|p| [p.x, p.y]),
            highlight: highlight
                .into_iter()
                .map(|q| {
                    q.map(|p| {
                        let p = b.transform.apply(p);
                        [p.x, p.y]
                    })
                })
                .collect(),
            handles: b.handles().map(|p| [p.x, p.y]),
            size,
        })
    });
    (edit, Some(list))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quads_and_segments() {
        let q = [
            Point { x: 0.0, y: 0.0 },
            Point { x: 10.0, y: 0.0 },
            Point { x: 10.0, y: 10.0 },
            Point { x: 0.0, y: 10.0 },
        ];
        assert!(inside_quad(Point { x: 5.0, y: 5.0 }, &q));
        assert!(!inside_quad(Point { x: 11.0, y: 5.0 }, &q));
        assert!((dist_to_segment(Point { x: 5.0, y: 3.0 }, q[0], q[1]) - 3.0).abs() < 1e-12);
        assert!(is_editing_key("Shift+Home") && is_editing_key("Primary+A") && !is_editing_key("Primary+Z"));
    }
}
