//! M4 C4.4b, the Contour Editor's gestures (02 §7.1.2 lines 446-463, §8.9; design note 3 §2 and §4; SI-184 to SI-187):
//! a gesture made with `contourEditor` becomes the command an agent sends for the same edit, with every current setting
//! explicit (02 §13.3). The press classifies its target (an anchor, a handle of a selected anchor, a segment, nothing)
//! with hit radii in view pixels, the modifiers held at the press choose the row, and the release builds the command.
//! The point selection is the kernel's (`AppContext::points`).

use serde_json::{Value, json};
use tt_model::entities::drawing::{Drawing, StrokeKind};
use tt_registry::CommandError;
use tt_vector::drawing::{self as vd, ObjectKind};
use tt_vector::geom::{Cubic, P};
use tt_vector::path::Path;

use crate::cx::Cx;
use crate::gesture::{Gesture, Sample};
use crate::shell::Point;

/// The tool id.
pub const TOOL: &str = "contourEditor";
/// Hit radius of an anchor, in view pixels (SI-184: SI-141's handle radius).
pub const ANCHOR_PX: f64 = 6.0;
/// Hit radius of a handle end of a selected anchor, in view pixels (SI-184).
pub const HANDLE_PX: f64 = 5.0;
/// Hit radius of a segment, in view pixels (SI-184).
pub const SEGMENT_PX: f64 = 4.0;

/// What a press is on.
#[derive(Debug, Clone, PartialEq)]
enum Target {
    Anchor { stroke: String, anchor: usize },
    Handle { stroke: String, anchor: usize, out: bool },
    Segment { stroke: String, segment: usize, t: f64 },
    Nothing,
}

/// A stroke as the hit test sees it: its local id, its path, and whether its anchors are shown (in the object
/// selection, or holding a selected anchor).
struct Candidate {
    id: String,
    path: Path,
    shown: bool,
}

fn pt(p: Point) -> Value {
    json!({ "x": p.x, "y": p.y })
}

fn rect(a: Point, b: Point) -> Value {
    json!({ "x": a.x.min(b.x), "y": a.y.min(b.y), "width": (a.x - b.x).abs(), "height": (a.y - b.y).abs() })
}

fn no_drawing() -> CommandError {
    CommandError::with_details(
        "drawing.noTarget",
        "there is no current drawing to edit with the Contour Editor",
        json!({ "tool": TOOL }),
    )
}

fn empty() -> CommandError {
    CommandError::with_details("arg.invalid", "a gesture needs at least one sample", json!({ "pointer": "/events" }))
}

/// The current drawing.
fn current(cx: &Cx) -> Option<Drawing> {
    let doc = cx.document().ok()?;
    let id = cx.app().current_drawing(doc)?;
    doc.drawing(&id).cloned()
}

/// The Contour Editor's overlay for the scene answer (`docs/canvas-format.md` §11.8; design note 3 §4): `None` unless
/// the Contour Editor is active with a current drawing. The shown strokes are [`candidates`]' (the driver's hit test
/// reads the same), then each same-layer stroke with an anchor at a shown anchor's exact coordinate as a partner.
pub(crate) fn scene_contours(cx: &Cx) -> Option<crate::canvas::SceneContours> {
    if cx.ui().drawing().active_tool != TOOL {
        return None;
    }
    let d = current(cx)?;
    let objects = vd::objects(&d);
    let list_of = |id: &str| objects.iter().find(|o| o.id == id).map(|o| o.list.clone());
    let cands = candidates(cx, &d);
    let mut strokes = Vec::new();
    let mut at: Vec<(String, f64, f64)> = Vec::new();
    for c in cands.iter().filter(|c| c.shown) {
        let mut sel: Vec<u32> =
            selected_anchors(cx, &d, &c.id).into_iter().filter_map(|a| u32::try_from(a).ok()).collect();
        sel.sort_unstable();
        sel.dedup();
        let list = list_of(&c.id).unwrap_or_default();
        at.extend(c.path.anchors.iter().map(|a| (list.clone(), a.p.x, a.p.y)));
        strokes.push(crate::canvas::SceneContourStroke {
            stroke_id: c.id.clone(),
            layer: list.clone(),
            closed: c.path.closed,
            partner: false,
            anchors: c.path.anchors.iter().map(|a| [a.p.x, a.p.y, a.inh.x, a.inh.y, a.outh.x, a.outh.y]).collect(),
            selected: sel,
        });
    }
    for o in objects.iter().filter(|o| o.kind == ObjectKind::Stroke) {
        if strokes.iter().any(|s| s.stroke_id == o.id) {
            continue;
        }
        let Some(s) = vd::stroke_at(&d, &o.pointer) else { continue };
        let joins = s.points().iter().any(|p| at.iter().any(|(l, x, y)| *l == o.list && *x == p[0] && *y == p[1]));
        if joins {
            strokes.push(crate::canvas::SceneContourStroke {
                stroke_id: o.id.clone(),
                layer: o.list.clone(),
                closed: *s.closed(),
                partner: true,
                anchors: s.points().to_vec(),
                selected: Vec::new(),
            });
        }
    }
    Some(crate::canvas::SceneContours { drawing_id: d.id().as_str().to_owned(), locked: *d.locked(), strokes })
}

/// The strokes of `d` in drawing order (back to front), with their paths and whether their anchors are shown.
fn candidates(cx: &Cx, d: &Drawing) -> Vec<Candidate> {
    let selected: std::collections::BTreeSet<String> =
        crate::commands::draw::selection_ids(cx, d).into_iter().collect();
    let prefix = format!("{}/", d.id().as_str());
    let with_points: std::collections::BTreeSet<String> =
        cx.app().points.iter().filter_map(|(k, _)| k.strip_prefix(&prefix).map(str::to_owned)).collect();
    let show_invisible = cx.ui().drawing().aids.strokes;
    vd::objects(d)
        .into_iter()
        .filter(|o| o.kind == ObjectKind::Stroke)
        .filter_map(|o| {
            let s = vd::stroke_at(d, &o.pointer)?;
            if *s.kind() == StrokeKind::Invisible && !show_invisible {
                return None;
            }
            // Closed as the contour commands read it (their segment numbering; brush outlines are stored closed).
            let shown = selected.contains(&o.id) || with_points.contains(&o.id);
            Some(Candidate { id: o.id, path: Path::from_points(s.points(), *s.closed()), shown })
        })
        .collect()
}

/// The selected anchors of stroke `id` in `d`.
fn selected_anchors(cx: &Cx, d: &Drawing, id: &str) -> Vec<usize> {
    let key = format!("{}/{id}", d.id().as_str());
    cx.app().points.iter().filter(|(k, _)| *k == key).map(|(_, a)| *a as usize).collect()
}

/// The point of segment `c` nearest to view point `q`, measured in view pixels through `view` (an affine map keeps the
/// Bezier parameter, so the parameter is the drawing-space one): `(distance, t)`. Sampled, then refined.
fn nearest_in_view(c: &Cubic, q: Point, view: &dyn Fn(Point) -> Point) -> (f64, f64) {
    let at = |t: f64| {
        let p = c.eval(t);
        let v = view(Point { x: p.x, y: p.y });
        (v.x - q.x).hypot(v.y - q.y)
    };
    let mut t = (0..=64).map(|k| f64::from(k) / 64.0).min_by(|a, b| at(*a).total_cmp(&at(*b))).unwrap_or(0.0);
    let mut step = 1.0 / 64.0;
    for _ in 0..40 {
        step /= 2.0;
        let (lo, hi) = ((t - step).max(0.0), (t + step).min(1.0));
        t = [lo, t, hi].into_iter().min_by(|a, b| at(*a).total_cmp(&at(*b))).unwrap_or(t);
    }
    (at(t), t)
}

/// The target of a press at view point `q` (SI-184): the nearest anchor of a shown stroke or handle end of a selected
/// anchor within its radius (an anchor on a tie); else the nearest segment within `SEGMENT_PX` of a shown stroke (of
/// any stroke when `any_segment`, the temporary-select click); ties between strokes go to the front-most.
fn target_at(
    cands: &[Candidate],
    selected: &dyn Fn(&str) -> Vec<usize>,
    q: Point,
    view: &dyn Fn(Point) -> Point,
    any_segment: bool,
) -> Target {
    let dist = |p: P| {
        let v = view(Point { x: p.x, y: p.y });
        (v.x - q.x).hypot(v.y - q.y)
    };
    // (distance, priority: 0 anchor, 1 handle, -index for front-most, target)
    let mut best: Option<(f64, u8, usize, Target)> = None;
    let mut keep = |d: f64, prio: u8, order: usize, t: Target| {
        let better = match &best {
            None => true,
            Some((bd, bp, bo, _)) => {
                d < *bd - 1e-12 || ((d - *bd).abs() <= 1e-12 && (prio < *bp || (prio == *bp && order > *bo)))
            }
        };
        if better {
            best = Some((d, prio, order, t));
        }
    };
    for (order, c) in cands.iter().enumerate() {
        if !c.shown {
            continue;
        }
        for (k, a) in c.path.anchors.iter().enumerate() {
            let d = dist(a.p);
            if d <= ANCHOR_PX {
                keep(d, 0, order, Target::Anchor { stroke: c.id.clone(), anchor: k });
            }
        }
        for k in selected(&c.id) {
            let Some(a) = c.path.anchors.get(k) else { continue };
            for (out, h) in [(false, a.inh), (true, a.outh)] {
                if h == P::default() {
                    continue;
                }
                let d = dist(a.p + h);
                if d <= HANDLE_PX {
                    keep(d, 1, order, Target::Handle { stroke: c.id.clone(), anchor: k, out });
                }
            }
        }
    }
    if let Some((_, _, _, t)) = best {
        return t;
    }
    let mut seg: Option<(f64, usize, Target)> = None;
    for (order, c) in cands.iter().enumerate() {
        if !(c.shown || any_segment) {
            continue;
        }
        for (i, cubic) in c.path.segments().iter().enumerate() {
            let (d, t) = nearest_in_view(cubic, q, view);
            if d <= SEGMENT_PX && t > 0.0 && t < 1.0 {
                let better =
                    seg.as_ref().is_none_or(|(bd, bo, _)| d < *bd - 1e-12 || ((d - *bd).abs() <= 1e-12 && order > *bo));
                if better {
                    seg = Some((d, order, Target::Segment { stroke: c.id.clone(), segment: i, t }));
                }
            }
        }
    }
    seg.map_or(Target::Nothing, |(_, _, t)| t)
}

/// The equivalent commands of a Contour Editor gesture (design note 3 §2), in order. `Ok(vec![])`: the gesture does
/// nothing (a temporary-select click on nothing).
///
/// # Errors
/// `drawing.noTarget` without a current drawing; `arg.invalid` for a gesture without samples; the view's refusal.
pub fn commands(cx: &Cx, g: &Gesture) -> Result<Vec<(String, Value)>, CommandError> {
    let Some(st) = g.strokes.first() else { return Err(empty()) };
    let (Some(p0), Some(pn)) = (st.samples.first().copied(), st.samples.last().copied()) else { return Err(empty()) };
    let d = current(cx).ok_or_else(no_drawing)?;
    let w = cx.ui().shell().view(&g.view_id)?;
    // Drawing units -> view pixels where the view shows them, as the Select tool's handles (SI-156).
    let camera = w.ty == crate::shell::ViewType::Camera;
    let inv = crate::camera_view::invert(&g.press);
    let view = move |p: Point| match inv {
        Some(m) if camera => crate::shell::apply(&m, p),
        _ => crate::commands::view::to_view(w, p),
    };
    let unit = (crate::commands::view::pixels_per_du(w) - 1.0).abs() <= 1e-12;
    let drawing = d.id().as_str().to_owned();
    let cands = candidates(cx, &d);
    let selected = |id: &str| selected_anchors(cx, &d, id);
    let mods = st.mods;
    let target = target_at(&cands, &selected, p0.v, &view, mods.temp_select && st.is_click());
    let snap = cx.ui().drawing().snap_to_contour;
    let delta = Point { x: pn.d.x - p0.d.x, y: pn.d.y - p0.d.y };
    let mode = if mods.constrain { "add" } else { "replace" };
    let with_view = |mut v: Value| {
        if !unit {
            v["view"] = json!(g.view_id);
        }
        v
    };
    let point_move = |points: Vec<Value>| {
        (
            "draw.point.move".to_owned(),
            with_view(json!({ "drawingId": drawing, "points": points, "delta": pt(delta), "snap": snap })),
        )
    };
    let selection_points = |cands: &[Candidate]| -> Vec<Value> {
        let mut out = Vec::new();
        for c in cands {
            for a in selected(&c.id) {
                out.push(json!({ "strokeId": c.id, "anchor": a }));
            }
        }
        out
    };
    let handle_offset = |stroke: &str, anchor: usize, out: bool| -> Option<P> {
        let c = cands.iter().find(|c| c.id == stroke)?;
        let a = c.path.anchors.get(anchor)?;
        Some(if out { a.outh } else { a.inh })
    };
    if st.is_click() {
        return Ok(click(&drawing, &target, mods, mode, p0, &with_view));
    }
    let out = match target {
        Target::Handle { stroke, anchor, out } => {
            let h = handle_offset(&stroke, anchor, out).unwrap_or_default();
            let moved = json!({ "x": h.x + delta.x, "y": h.y + delta.y });
            let key = if out { "outHandle" } else { "inHandle" };
            let mut v = json!({ "drawingId": drawing, "strokeId": stroke, "anchor": anchor,
                                "handleMode": if mods.alternate { "broken" } else { "symmetric" } });
            v[key] = moved;
            vec![("draw.point.set".to_owned(), v)]
        }
        Target::Anchor { stroke, anchor } if mods.alternate => {
            // SI-185 (D-40 answer 7, interim): both handles follow the pointer, the anchor stays.
            let i = handle_offset(&stroke, anchor, false).unwrap_or_default();
            let o = handle_offset(&stroke, anchor, true).unwrap_or_default();
            vec![(
                "draw.point.set".to_owned(),
                json!({ "drawingId": drawing, "strokeId": stroke, "anchor": anchor,
                        "inHandle": { "x": i.x + delta.x, "y": i.y + delta.y },
                        "outHandle": { "x": o.x + delta.x, "y": o.y + delta.y }, "handleMode": "broken" }),
            )]
        }
        Target::Anchor { stroke, anchor } => {
            let in_selection = selected(&stroke).contains(&anchor);
            let points = if in_selection {
                selection_points(&cands)
            } else {
                vec![json!({ "strokeId": stroke, "anchor": anchor })]
            };
            vec![point_move(points)]
        }
        Target::Segment { .. } if mods.alternate => vec![point_move(selection_points(&cands))],
        Target::Segment { stroke, segment, t } => vec![(
            "draw.segment.move".to_owned(),
            json!({ "drawingId": drawing, "strokeId": stroke, "segment": segment, "t": t, "delta": pt(delta) }),
        )],
        Target::Nothing => vec![(
            "draw.pointSelection.set".to_owned(),
            json!({ "drawingId": drawing, "rect": rect(p0.d, pn.d), "mode": mode }),
        )],
    };
    Ok(out)
}

/// The commands of a click (design note 3 §2).
fn click(
    drawing: &str,
    target: &Target,
    mods: crate::gesture::Mods,
    mode: &str,
    p0: Sample,
    with_view: &dyn Fn(Value) -> Value,
) -> Vec<(String, Value)> {
    match target {
        // Temporary-select: insert an anchor on the segment (02 line 452); on nothing, no command.
        _ if mods.temp_select => match target {
            Target::Segment { stroke, segment, t } => vec![(
                "draw.point.add".to_owned(),
                json!({ "drawingId": drawing, "strokeId": stroke, "segment": segment, "t": t }),
            )],
            _ => Vec::new(),
        },
        // Alternate + click an anchor: reset its handles (02 line 453; SI-186 records what reset does).
        Target::Anchor { stroke, anchor } if mods.alternate => vec![(
            "draw.point.set".to_owned(),
            json!({ "drawingId": drawing, "strokeId": stroke, "anchor": anchor, "handleMode": "reset" }),
        )],
        Target::Anchor { stroke, anchor } => vec![(
            "draw.pointSelection.set".to_owned(),
            json!({ "drawingId": drawing, "points": [{ "strokeId": stroke, "anchor": anchor }], "mode": mode }),
        )],
        // A click on a stroke body or on nothing: the Select click, then (without constrain) the point selection
        // cleared (SI-184: two `ui` commands; the second is not sent if the first is refused).
        _ => {
            let mut out = vec![(
                "draw.selection.set".to_owned(),
                with_view(json!({ "by": "point", "point": pt(p0.d), "mode": mode })),
            )];
            if !mods.constrain {
                out.push((
                    "draw.pointSelection.set".to_owned(),
                    json!({ "drawingId": drawing, "points": [], "mode": "replace" }),
                ));
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line() -> Path {
        Path::from_points(&[[0.0, 0.0, 0.0, 0.0, 0.0, 0.0], [100.0, 0.0, 0.0, 0.0, 0.0, 0.0]], false)
    }

    /// SI-184: the anchor wins within its radius; a segment within 4 px but not 4.01; nothing beyond.
    #[test]
    fn hit_order_and_radii_in_view_pixels() {
        let cands = vec![Candidate { id: "a".into(), path: line(), shown: true }];
        let none = |_: &str| Vec::new();
        let id = |p: Point| p;
        assert_eq!(
            target_at(&cands, &none, Point { x: 3.0, y: 2.0 }, &id, false),
            Target::Anchor { stroke: "a".into(), anchor: 0 }
        );
        match target_at(&cands, &none, Point { x: 50.0, y: 4.0 }, &id, false) {
            Target::Segment { segment, t, .. } => {
                assert_eq!(segment, 0);
                assert!((t - 0.5).abs() < 1e-6);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(target_at(&cands, &none, Point { x: 50.0, y: 4.01 }, &id, false), Target::Nothing);
        // At zoom 2 the segment lies along view y = 0 and an anchor sits at view (200, 0): radii stay in view pixels.
        let zoom2 = |p: Point| Point { x: p.x * 2.0, y: p.y * 2.0 };
        assert!(matches!(target_at(&cands, &none, Point { x: 100.0, y: 4.0 }, &zoom2, false), Target::Segment { .. }));
        assert_eq!(target_at(&cands, &none, Point { x: 100.0, y: 4.5 }, &zoom2, false), Target::Nothing);
        assert_eq!(
            target_at(&cands, &none, Point { x: 205.0, y: 0.0 }, &zoom2, false),
            Target::Anchor { stroke: "a".into(), anchor: 1 }
        );
    }

    /// A stroke whose anchors are not shown is not a drag target; its segment is a temporary-select click's.
    #[test]
    fn hidden_strokes_are_only_temporary_select_targets() {
        let cands = vec![Candidate { id: "a".into(), path: line(), shown: false }];
        let none = |_: &str| Vec::new();
        let id = |p: Point| p;
        assert_eq!(target_at(&cands, &none, Point { x: 1.0, y: 1.0 }, &id, false), Target::Nothing);
        assert!(matches!(target_at(&cands, &none, Point { x: 50.0, y: 1.0 }, &id, true), Target::Segment { .. }));
    }

    /// The handle end of a selected anchor is a target; an anchor at the same distance wins the tie.
    #[test]
    fn handles_of_selected_anchors_and_the_anchor_tie() {
        let path = Path::from_points(&[[0.0, 0.0, 0.0, 0.0, 20.0, 0.0], [100.0, 0.0, -20.0, 0.0, 0.0, 0.0]], false);
        let cands = vec![Candidate { id: "a".into(), path, shown: true }];
        let sel = |_: &str| vec![0usize];
        let id = |p: Point| p;
        assert_eq!(
            target_at(&cands, &sel, Point { x: 21.0, y: 1.0 }, &id, false),
            Target::Handle { stroke: "a".into(), anchor: 0, out: true }
        );
        let none = |_: &str| Vec::new();
        assert!(matches!(target_at(&cands, &none, Point { x: 21.0, y: 1.0 }, &id, false), Target::Segment { .. }));
    }
}
