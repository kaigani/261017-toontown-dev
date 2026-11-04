//! Motion paths (05 §9, §10.5, §19.3; M3 C3.2): `anim.path.get`, `.set`, `.point.add`, `.point.set`, `.point.remove`,
//! `.lockDepth.set`. Keyframes pin the velocity channel; adding, removing or moving one renormalises the intermediate
//! velocity keys, so their timing is preserved (05 §10.5).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tt_anim::function::KeyView;
use tt_anim::node::NodeData;
use tt_anim::num::tidy;
use tt_anim::tcb::{self, Path, Velocity};
use tt_model::document::{Document, EntryKey};
use tt_model::entities::channel::{CurveType, Key};
use tt_model::{Id, ObjectType};
use tt_registry::{CommandError, command};

use super::anim_common::{
    ChannelEdit, Xy, Xyz, check_value, constant_default, data_of, default_shape, emit_changed, frame_invalid, frame_of,
    key_frame, node, node_id, not_found, tcb_defaults, transformable,
};
use super::timing::{Budget, Changes, as_u32, err};
use crate::cx::Cx;

/// `anim.keyExists`.
pub(crate) fn key_exists(frame: f64) -> CommandError {
    err("anim.keyExists", format!("a key already exists at frame {frame}"), json!({ "frame": frame }))
}

/// The path as the curve reads it.
pub(crate) fn path_of(points: &[Value], lock: bool) -> Result<Path, CommandError> {
    let mut out = Vec::with_capacity(points.len());
    for p in points {
        let q: tt_model::entities::channel::MotionPoint =
            serde_json::from_value(p.clone()).map_err(super::timing::internal)?;
        out.push(tcb::Point::from(&q));
    }
    Ok(Path::new(out, lock))
}

/// The velocity channel as the curve reads it.
pub(crate) fn velocity_of(edit: &ChannelEdit) -> Result<Velocity, CommandError> {
    let Some(c) = edit.channels.get("velocity") else { return Ok(Velocity::default()) };
    let curve = c
        .get("curveType")
        .cloned()
        .and_then(|v| serde_json::from_value::<CurveType>(v).ok())
        .unwrap_or(CurveType::Interpolation);
    let mut keys = Vec::new();
    for k in edit.keys("velocity") {
        let k: Key = serde_json::from_value(k).map_err(super::timing::internal)?;
        keys.push(KeyView::from(&k));
    }
    Ok(Velocity { curve, keys })
}

/// A new motion point.
pub(crate) fn point_json(cx: &mut Cx, frame: Option<u32>, pos: [f64; 3]) -> Result<Value, CommandError> {
    let id = cx.new_id(ObjectType::MotionPoint)?;
    let (t, c, b) = tcb_defaults(cx);
    Ok(json!({
        "id": id.as_str(), "type": "motionPoint", "rev": 0, "frame": frame,
        "position": { "x": pos[0], "y": pos[1], "z": pos[2] },
        "tension": t, "continuity": c, "bias": b,
        "constantSegment": frame.is_some() && constant_default(cx), "lockDepth": false,
    }))
}

fn ratio(a: f64, b: f64) -> f64 {
    if b == 0.0 { 0.0 } else { a / b }
}

/// Renormalises velocity keys for a pin inserted at local frame `f`, arc `arc`, inside the keyframe segment `j`.
pub(crate) fn split_velocity(
    cx: &mut Cx,
    edit: &mut ChannelEdit,
    path: &Path,
    j: usize,
    f: u32,
    arc: f64,
) -> Result<(), CommandError> {
    let mut keys = edit.keys("velocity");
    if keys.is_empty() {
        return Ok(());
    }
    let (f0, f1) = (path.key_frame(j), path.key_frame(j + 1));
    let (s0, s1) = (path.key_arc(j), path.key_arc(j + 1));
    let snew = arc.clamp(s0, s1.max(s0));
    for k in &mut keys {
        let Some(frame) = frame_of(k) else { continue };
        let at = f64::from(frame);
        if at <= f0 || at >= f1 {
            continue;
        }
        let v = s0 + k["value"].as_f64().unwrap_or(0.0) * (s1 - s0);
        let u = if frame < f {
            ratio(v - s0, snew - s0)
        } else if frame > f {
            ratio(v - snew, s1 - snew)
        } else {
            0.0
        };
        k["value"] = json!(u);
    }
    edit.set_keys(cx, "velocity", keys)
}

/// Velocity keys after the pin at keyframe index `j` is removed (05 §10.5): an interior pin becomes an ordinary key at
/// its normalised position in the merged segment; keys outside the remaining span go.
pub(crate) fn merge_velocity(cx: &mut Cx, edit: &mut ChannelEdit, path: &Path, j: usize) -> Result<(), CommandError> {
    let keys = edit.keys("velocity");
    let m = path.key_indices().len();
    let f = path.key_frame(j);
    let interior = j > 0 && j + 1 < m;
    if !interior && keys.is_empty() {
        return Ok(());
    }
    let mut out = Vec::with_capacity(keys.len() + 1);
    let mut record = None;
    for mut k in keys {
        let Some(frame) = frame_of(&k).map(f64::from) else { continue };
        if frame == f {
            record = Some(k);
            continue;
        }
        if interior {
            let (fa, fb) = (path.key_frame(j - 1), path.key_frame(j + 1));
            let (sa, sf, sb) = (path.key_arc(j - 1), path.key_arc(j), path.key_arc(j + 1));
            if frame > fa && frame < f {
                let v = sa + k["value"].as_f64().unwrap_or(0.0) * (sf - sa);
                k["value"] = json!(ratio(v - sa, sb - sa));
            } else if frame > f && frame < fb {
                let v = sf + k["value"].as_f64().unwrap_or(0.0) * (sb - sf);
                k["value"] = json!(ratio(v - sa, sb - sa));
            }
        } else {
            let keep = if j == 0 { frame > f } else { frame < f };
            if !keep {
                continue;
            }
        }
        out.push(k);
    }
    if interior {
        let (sa, sf, sb) = (path.key_arc(j - 1), path.key_arc(j), path.key_arc(j + 1));
        let u = ratio(sf - sa, sb - sa);
        let local = f as u32;
        let shape = default_shape(cx);
        let mut k = match record {
            Some(k) => k,
            None => super::anim_common::new_key(cx, local, u, shape)?,
        };
        k["value"] = json!(u);
        k["constantSegment"] = json!(false);
        let at = out.partition_point(|o| frame_of(o).is_some_and(|x| x < local));
        out.insert(at, k);
    }
    edit.set_keys(cx, "velocity", out)
}

/// Maps velocity key frames when keyframes move from `old` to `new` (same length, both increasing): piecewise linear
/// between the pins, rounded half up; a later key wins a collision.
pub(crate) fn retime_velocity(
    cx: &mut Cx,
    edit: &mut ChannelEdit,
    old: &[f64],
    new: &[f64],
) -> Result<(), CommandError> {
    let keys = edit.keys("velocity");
    if keys.is_empty() {
        return Ok(());
    }
    let mut out: std::collections::BTreeMap<u32, Value> = std::collections::BTreeMap::new();
    for mut k in keys {
        let Some(f) = frame_of(&k).map(f64::from) else { continue };
        let i = old.partition_point(|o| *o <= f);
        let nf = match (i.checked_sub(1).and_then(|a| old.get(a).zip(new.get(a))), old.get(i).zip(new.get(i))) {
            (Some((oa, na)), _) if *oa == f => *na,
            (Some((oa, na)), Some((ob, nb))) => (na + (f - oa) * (nb - na) / (ob - oa) + 0.5).floor(),
            _ => f,
        };
        let nf = as_u32(nf as usize);
        k["frame"] = json!(nf);
        out.insert(nf, k);
    }
    edit.set_keys(cx, "velocity", out.into_values().collect())
}

/// Adds a keyframe at a free local frame, or moves the point of an existing keyframe. `pos` components left `None` take
/// the node's current animated position at that frame.
pub(crate) fn put_motion_key(
    cx: &mut Cx,
    edit: &mut ChannelEdit,
    data: &NodeData,
    local: u32,
    pos: [Option<f64>; 3],
) -> Result<String, CommandError> {
    let lock = data.statics.as_ref().is_some_and(|s| s.lock_depth);
    let points = edit.points();
    let path = path_of(&points, lock)?;
    let velocity = velocity_of(edit)?;
    let f = f64::from(local);
    let current = tcb::travel(&path, &velocity, lock, f).position;
    let target = [pos[0].unwrap_or(current[0]), pos[1].unwrap_or(current[1]), pos[2].unwrap_or(current[2])];
    edit.touch(local);
    let existing = points.iter().position(|p| frame_of(p) == Some(local));
    if let Some(i) = existing {
        let mut points = points;
        points[i]["position"] = json!({ "x": target[0], "y": target[1], "z": target[2] });
        let id = points[i]["id"].as_str().unwrap_or_default().to_owned();
        edit.set_points(cx, points)?;
        return Ok(id);
    }
    let point = point_json(cx, Some(local), target)?;
    let id = point["id"].as_str().unwrap_or_default().to_owned();
    let mut points = points;
    let n = path.key_indices().len();
    let index = if n == 0 || f < path.key_frame(0) {
        0
    } else if f > path.key_frame(n - 1) {
        points.len()
    } else {
        // Inside a keyframe segment: after the control points the node has already passed at this frame.
        let j = tcb::key_segment(&path, f);
        let arc = velocity.value(&path, f).clamp(path.key_arc(j), path.key_arc(j + 1).max(path.key_arc(j)));
        let (a, b) = (path.key_indices()[j], path.key_indices()[j + 1]);
        let mut index = a + 1;
        for c in a + 1..b {
            if path.arc_of_point(c) <= arc {
                index = c + 1;
            }
        }
        split_velocity(cx, edit, &path, j, local, arc)?;
        index
    };
    points.insert(index, point);
    edit.set_points(cx, points)?;
    Ok(id)
}

/// The compound ID of a motion point (`<channelId>/<pointId>`, 10 ID-03).
fn compound(channel: &str, point: &str) -> String {
    format!("{channel}/{point}")
}

/// A motion point reference: its element and local point ID.
fn point_ref(cx: &Cx, doc: &Document, r: &str) -> Result<(Id, String), CommandError> {
    let key = cx.resolve_as(r, ObjectType::MotionPoint).map_err(|_| not_found("motion point", r))?;
    let l = doc.locate(&key).ok_or_else(|| not_found("motion point", r))?;
    match &l.entry {
        EntryKey::Element(e) => Ok((e.clone(), l.id.as_str().to_owned())),
        _ => Err(not_found("motion point", r)),
    }
}

// ----- anim.path.get ---------------------------------------------------------------------------------------------

/// `anim.path.get` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PathGetParams {
    /// The node.
    node_id: String,
}

/// A motion point (05 §19.4).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PathPoint {
    /// `<channelId>/<pointId>`.
    id: String,
    /// `key` or `control`.
    kind: String,
    /// Keyframe: its scene frame. Control point: the derived passage frame, 3 decimals.
    frame: f64,
    x: f64,
    y: f64,
    z: f64,
    tension: f64,
    continuity: f64,
    bias: f64,
    /// Keyframes only.
    #[serde(skip_serializing_if = "Option::is_none")]
    constant_segment: Option<bool>,
    /// Keyframes only.
    #[serde(skip_serializing_if = "Option::is_none")]
    lock_depth: Option<bool>,
    /// Arc length at this point.
    arc_length: f64,
}

/// `anim.path.get` result (05 §19.4).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PathResult {
    node_id: String,
    lock_depth: bool,
    length: f64,
    points: Vec<PathPoint>,
}

#[command(
    id = "anim.path.get",
    kind = query,
    label = "Get motion path",
    scopes = [read],
    needs_project,
    example(title = "Bob's motion path", params = r#"{"nodeId": "scene:\"Intro\"/element:\"Bob\""}"#),
    agent_only = "agents read paths numerically (05 §19.1.5); the Motion tool and motion point properties show them (M6 C6.2)",
)]
pub(crate) fn anim_path_get(cx: &mut Cx, p: PathGetParams) -> Result<PathResult, CommandError> {
    let doc = cx.document()?.clone();
    let id = node_id(cx, &p.node_id)?;
    let e = transformable(&doc, &id)?;
    let mut budget = Budget::new("reading a motion path");
    let data = data_of(&doc, &id, &mut budget)?;
    let lock_depth = data.statics.as_ref().is_some_and(|s| s.lock_depth);
    let Some(path) = &data.path else {
        return Ok(PathResult { node_id: id.as_str().to_owned(), lock_depth, length: 0.0, points: Vec::new() });
    };
    let channel = e.channels().as_ref().and_then(|m| m.get("position"));
    let cid = channel.map_or("", |c| c.id().as_str());
    let mut spend = |n: u64| budget.spend(usize::try_from(n).unwrap_or(usize::MAX));
    let mut points = Vec::with_capacity(path.points.len());
    let stored = channel.and_then(|c| c.points().as_ref());
    for (i, q) in path.points.iter().enumerate() {
        let pid = stored.and_then(|s| s.get(i)).map_or(String::new(), |m| compound(cid, m.id().as_str()));
        let arc = path.arc_of_point(i);
        let frame = match q.frame {
            Some(f) => data.bar.scene(f),
            None => {
                let j = control_segment(path, i);
                let start = path.key_indices().get(j).and_then(|k| path.points.get(*k));
                let local = if start.is_some_and(|s| s.constant) {
                    path.key_frame(j)
                } else {
                    data.velocity.passage(path, j, arc, &mut spend)?
                };
                tcb::round3(data.bar.scene(local))
            }
        };
        let key = q.frame.is_some();
        points.push(PathPoint {
            id: pid,
            kind: if key { "key" } else { "control" }.to_owned(),
            frame: tidy(frame),
            x: tidy(q.pos[0]),
            y: tidy(q.pos[1]),
            z: tidy(q.pos[2]),
            tension: tidy(q.t),
            continuity: tidy(q.c),
            bias: tidy(q.b),
            constant_segment: key.then_some(q.constant),
            lock_depth: key.then_some(q.lock_depth),
            arc_length: tidy(arc),
        });
    }
    Ok(PathResult { node_id: id.as_str().to_owned(), lock_depth, length: tidy(path.length()), points })
}

/// The keyframe segment holding the control point at index `i`.
fn control_segment(path: &Path, i: usize) -> usize {
    path.key_indices().partition_point(|k| *k < i).saturating_sub(1)
}

// ----- anim.path.set -----------------------------------------------------------------------------------------------

/// A point of `anim.path.set`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PointKind {
    /// Keyframe.
    #[default]
    Key,
    /// Control point.
    Control,
}

/// A point of `anim.path.set`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SetPoint {
    /// Keyframe or control point.
    kind: PointKind,
    /// Keyframe: its scene frame.
    #[serde(default)]
    frame: Option<f64>,
    x: f64,
    y: f64,
    z: f64,
    #[serde(default)]
    tension: Option<f64>,
    #[serde(default)]
    continuity: Option<f64>,
    #[serde(default)]
    bias: Option<f64>,
    #[serde(default)]
    constant_segment: Option<bool>,
    #[serde(default)]
    lock_depth: Option<bool>,
}

/// `anim.path.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PathSetParams {
    /// The node.
    node_id: String,
    /// The points in path order; first and last are keyframes.
    #[schemars(length(max = 100_000))]
    points: Vec<SetPoint>,
    /// Constant Z of the whole node.
    #[serde(default)]
    lock_depth: Option<bool>,
}

/// `anim.path.set` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PathSetResult {
    node_id: String,
    point_ids: Vec<String>,
    warnings: Vec<String>,
}

fn check_tcb(name: &str, v: Option<f64>) -> Result<(), CommandError> {
    if let Some(v) = v
        && !(-1.0..=1.0).contains(&v)
    {
        return Err(super::anim_common::out_of_range(name, v, -1.0, 1.0));
    }
    Ok(())
}

#[command(
    id = "anim.path.set",
    kind = action,
    label = "Set motion path",
    scopes = [edit],
    undo = "Set Motion Path",
    needs_project,
    example(title = "Move Bob from left to right over frames 1-12", params = r#"{"nodeId": "scene:\"Intro\"/element:\"Bob\"", "points": [{"kind": "key", "frame": 1, "x": -4, "y": 0, "z": 0}, {"kind": "control", "x": 0, "y": 3, "z": 0}, {"kind": "key", "frame": 12, "x": 4, "y": 0, "z": 0}]}"#),
    agent_only = "agents replace a whole path (05 §19.3); the Motion tool edits points (M6 C6.2)",
)]
pub(crate) fn anim_path_set(cx: &mut Cx, p: PathSetParams) -> Result<PathSetResult, CommandError> {
    let doc = cx.document()?.clone();
    let id = node_id(cx, &p.node_id)?;
    let e = transformable(&doc, &id)?;
    if !e.element_type().allowed_channels().contains(&"position") {
        return Err(super::anim_common::unsupported("position", e, "not a channel of this node kind"));
    }
    let mut budget = Budget::new("setting a motion path");
    let data = data_of(&doc, &id, &mut budget)?;
    budget.spend(p.points.len())?;
    let mut edit = ChannelEdit::read(&doc, &id)?;
    let mut warnings = Vec::new();
    // Validate before building anything: the whole list or nothing (05 §19.1.7).
    let mut last: Option<u32> = None;
    let mut out = Vec::with_capacity(p.points.len());
    for q in &p.points {
        check_value("x", q.x)?;
        check_value("y", q.y)?;
        check_value("z", q.z)?;
        check_tcb("tension", q.tension)?;
        check_tcb("continuity", q.continuity)?;
        check_tcb("bias", q.bias)?;
        let frame = match q.kind {
            PointKind::Key => {
                let scene = q.frame.ok_or_else(|| frame_invalid(0.0, "a keyframe needs its frame"))?;
                let local = key_frame(&data, scene)?;
                if let Some(l) = last {
                    if l == local {
                        return Err(super::anim_path::key_exists(scene));
                    }
                    if l > local {
                        return Err(frame_invalid(scene, "keyframes run in increasing frame order along the path"));
                    }
                }
                last = Some(local);
                edit.touch(local);
                Some(local)
            }
            PointKind::Control => None,
        };
        let mut v = point_json(cx, frame, [q.x, q.y, q.z])?;
        for (k, x) in [("tension", q.tension), ("continuity", q.continuity), ("bias", q.bias)] {
            if let Some(x) = x {
                v[k] = json!(x);
            }
        }
        if let Some(c) = q.constant_segment {
            v["constantSegment"] = json!(c && frame.is_some());
        }
        if let Some(l) = q.lock_depth {
            v["lockDepth"] = json!(l && frame.is_some());
        }
        out.push(v);
    }
    // The integrity rule (05 §9.6): control points beyond the end keyframes become keyframes.
    settle_ends(&mut out, &data, &mut warnings, &mut edit)?;
    let ids: Vec<String> = out.iter().map(|v| v["id"].as_str().unwrap_or_default().to_owned()).collect();
    // Velocity keys outside the new span go; those inside keep their normalised timing.
    let lo = out.first().and_then(frame_of);
    let hi = out.last().and_then(frame_of);
    let kept: Vec<Value> = edit
        .keys("velocity")
        .into_iter()
        .filter(|k| match (frame_of(k), lo, hi) {
            (Some(f), Some(a), Some(b)) => f >= a && f <= b,
            _ => false,
        })
        .collect();
    if edit.channels.contains_key("velocity") {
        edit.set_keys(cx, "velocity", kept)?;
    }
    edit.set_points(cx, out)?;
    super::anim_common::drop_absent_removals(&doc, &mut edit)?;
    let mut changes = Changes::default();
    edit.stage(&mut changes);
    if let Some(lock) = p.lock_depth {
        changes.push(
            EntryKey::Element(id.clone()),
            vec![super::timing::op("replace", "/transform/lockDepth", Some(json!(lock)))],
        );
    }
    changes.stage(cx)?;
    emit_changed(cx, &data, &edit);
    let cid = cx
        .document()?
        .element(&id)
        .and_then(|e| e.channels().as_ref())
        .and_then(|m| m.get("position"))
        .map_or(String::new(), |c| c.id().as_str().to_owned());
    for w in &warnings {
        cx.warn("anim.pathEndConverted", w.clone(), vec![id.as_str().to_owned()]);
    }
    Ok(PathSetResult {
        node_id: id.as_str().to_owned(),
        point_ids: ids.iter().map(|i| compound(&cid, i)).collect(),
        warnings,
    })
}

/// Converts control points at the ends of a point list into keyframes (05 §9.6) at the first free frames of the bar.
fn settle_ends(
    points: &mut [Value],
    data: &NodeData,
    warnings: &mut Vec<String>,
    edit: &mut ChannelEdit,
) -> Result<(), CommandError> {
    let taken: Vec<u32> = points.iter().filter_map(frame_of).collect();
    let free = |from: u32, up: bool, taken: &[u32]| -> Option<u32> {
        let mut f = from;
        let last = data.bar.cycle.unwrap_or(data.bar.duration) as u32;
        loop {
            if !taken.contains(&f) {
                return Some(f);
            }
            if up {
                if f >= last {
                    return None;
                }
                f += 1;
            } else {
                if f <= 1 {
                    return None;
                }
                f -= 1;
            }
        }
    };
    let n = points.len();
    if n == 0 {
        return Ok(());
    }
    if frame_of(&points[0]).is_none() {
        let first_key = taken.first().copied().unwrap_or(2).saturating_sub(1).max(1);
        let f =
            free(first_key, false, &taken).ok_or_else(|| frame_invalid(1.0, "no free frame for the first keyframe"))?;
        points[0]["frame"] = json!(f);
        edit.touch(f);
        warnings.push(format!("the first motion point became a keyframe at local frame {f}"));
    }
    if frame_of(&points[n - 1]).is_none() {
        let taken: Vec<u32> = points.iter().filter_map(frame_of).collect();
        let last_key = taken.last().copied().unwrap_or(0).saturating_add(1);
        let f =
            free(last_key, true, &taken).ok_or_else(|| frame_invalid(1.0, "no free frame for the last keyframe"))?;
        points[n - 1]["frame"] = json!(f);
        edit.touch(f);
        warnings.push(format!("the last motion point became a keyframe at local frame {f}"));
    }
    Ok(())
}

// ----- anim.path.point.add -------------------------------------------------------------------------------------------

/// A curve location.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CurveAt {
    /// Curve segment index.
    segment: u32,
    /// Parameter, 0…1.
    #[schemars(range(min = 0.0, max = 1.0))]
    s: f64,
}

/// `anim.path.point.add` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PointAddParams {
    /// The node.
    node_id: String,
    /// Keyframe or control point.
    kind: PointKind,
    /// Scene frame; a keyframe's must be an integer, a control point's may be fractional. Default: the current frame.
    #[serde(default)]
    frame: Option<f64>,
    /// A curve location instead of a frame (control points).
    #[serde(default)]
    at: Option<CurveAt>,
    /// The point's position; default: where the node is at that frame or location.
    #[serde(default)]
    position: Option<Xyz>,
}

/// `anim.path.point.add` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PointAddResult {
    /// The new point.
    point_id: String,
    /// Its frame (a control point's derived passage frame).
    frame: f64,
}

#[command(
    id = "anim.path.point.add",
    kind = action,
    label = "Add motion point",
    scopes = [edit],
    undo = "Add Motion Point",
    needs_project,
    example(title = "Add a keyframe to Bob at frame 6", params = r#"{"nodeId": "scene:\"Intro\"/element:\"Bob\"", "kind": "key", "frame": 6, "position": {"x": 1, "y": 2, "z": 0}}"#),
    gui = "Motion tool, motion point properties: add point (M6 C6.2)",
)]
pub(crate) fn anim_path_point_add(cx: &mut Cx, p: PointAddParams) -> Result<PointAddResult, CommandError> {
    let doc = cx.document()?.clone();
    let id = node_id(cx, &p.node_id)?;
    let e = transformable(&doc, &id)?;
    if !e.element_type().allowed_channels().contains(&"position") {
        return Err(super::anim_common::unsupported("position", e, "not a channel of this node kind"));
    }
    if let Some(q) = &p.position {
        q.check()?;
    }
    let mut budget = Budget::new("adding a motion point");
    let data = data_of(&doc, &id, &mut budget)?;
    let mut edit = ChannelEdit::read(&doc, &id)?;
    let lock = data.statics.as_ref().is_some_and(|s| s.lock_depth);
    let parts = p.position.map_or([None; 3], |q| q.parts());
    let (pid, frame) = match p.kind {
        PointKind::Key => {
            if p.at.is_some() {
                return Err(frame_invalid(0.0, "a keyframe is added by frame, not by curve location"));
            }
            let scene = super::anim_common::scene_frame(cx, p.frame)?;
            let local = key_frame(&data, scene)?;
            if edit.points().iter().any(|q| frame_of(q) == Some(local)) {
                return Err(key_exists(scene));
            }
            let pid = put_motion_key(cx, &mut edit, &data, local, parts)?;
            (pid, scene)
        }
        PointKind::Control => {
            let points = edit.points();
            let path = path_of(&points, lock)?;
            let velocity = velocity_of(&edit)?;
            if path.key_indices().len() < 2 {
                return Err(frame_invalid(p.frame.unwrap_or(0.0), "control points lie between keyframes"));
            }
            let (index, pos) = match p.at {
                Some(at) => {
                    let seg = at.segment as usize;
                    if seg >= path.segments() || !(0.0..=1.0).contains(&at.s) {
                        return Err(frame_invalid(f64::from(at.segment), "no such curve location"));
                    }
                    (seg + 1, path.curve(seg, at.s))
                }
                None => {
                    let scene = super::anim_common::scene_frame(cx, p.frame)?;
                    let f = scene - data.bar.start + 1.0;
                    let m = path.key_indices().len();
                    if f <= path.key_frame(0) || f >= path.key_frame(m - 1) {
                        return Err(frame_invalid(scene, "control points lie between keyframes"));
                    }
                    if path.key_indices().iter().any(|k| path.points[*k].frame == Some(f)) {
                        return Err(key_exists(scene));
                    }
                    let j = tcb::key_segment(&path, f);
                    let arc = velocity.value(&path, f).clamp(path.key_arc(j), path.key_arc(j + 1).max(path.key_arc(j)));
                    let (a, b) = (path.key_indices()[j], path.key_indices()[j + 1]);
                    let mut index = a + 1;
                    for c in a + 1..b {
                        if path.arc_of_point(c) <= arc {
                            index = c + 1;
                        }
                    }
                    let (seg, s) = path.locate(arc, a, b);
                    (index, path.curve(seg, s))
                }
            };
            let target = [parts[0].unwrap_or(pos[0]), parts[1].unwrap_or(pos[1]), parts[2].unwrap_or(pos[2])];
            let point = point_json(cx, None, target)?;
            let pid = point["id"].as_str().unwrap_or_default().to_owned();
            let mut next = points;
            next.insert(index.min(next.len().saturating_sub(1)), point);
            edit.set_points(cx, next.clone())?;
            // The reported frame is the new point's derived passage frame.
            let path2 = path_of(&next, lock)?;
            let i = next.iter().position(|q| q["id"] == json!(pid)).unwrap_or(0);
            let j = control_segment(&path2, i);
            let mut spend = |n: u64| budget.spend(usize::try_from(n).unwrap_or(usize::MAX));
            let local = velocity.passage(&path2, j, path2.arc_of_point(i), &mut spend)?;
            edit.touch(local.floor() as u32);
            (pid, tcb::round3(data.bar.scene(local)))
        }
    };
    let mut changes = Changes::default();
    edit.stage(&mut changes);
    changes.stage(cx)?;
    emit_changed(cx, &data, &edit);
    let cid = cx
        .document()?
        .element(&id)
        .and_then(|e| e.channels().as_ref())
        .and_then(|m| m.get("position"))
        .map_or(String::new(), |c| c.id().as_str().to_owned());
    Ok(PointAddResult { point_id: compound(&cid, &pid), frame: tidy(frame) })
}

// ----- anim.path.point.set -------------------------------------------------------------------------------------------

/// `anim.path.point.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PointSetParams {
    /// The points.
    #[schemars(length(min = 1, max = 100_000))]
    point_ids: Vec<String>,
    #[serde(default)]
    x: Option<f64>,
    #[serde(default)]
    y: Option<f64>,
    #[serde(default)]
    z: Option<f64>,
    /// Moves a keyframe in time (scene frame).
    #[serde(default)]
    frame: Option<f64>,
    /// On = keyframe, off = control point.
    #[serde(default)]
    lock_in_time: Option<bool>,
    #[serde(default)]
    tension: Option<f64>,
    #[serde(default)]
    continuity: Option<f64>,
    #[serde(default)]
    bias: Option<f64>,
    #[serde(default)]
    constant_segment: Option<bool>,
    /// Keyframes only: the `rotation` key at that frame.
    #[serde(default)]
    rotation: Option<f64>,
    /// Keyframes only: the `scale.*` keys at that frame.
    #[serde(default)]
    scale: Option<Xy>,
    /// Keyframes only: the `pivot.*` keys at that frame.
    #[serde(default)]
    pivot: Option<Xy>,
}

/// `anim.path.point.set` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PointSetResult {
    point_ids: Vec<String>,
    warnings: Vec<String>,
}

#[command(
    id = "anim.path.point.set",
    kind = action,
    label = "Edit motion point",
    scopes = [edit],
    undo = "Change Motion Point",
    needs_project,
    example(title = "Move Bob's last keyframe and sharpen it", params = r#"{"pointIds": ["scene:\"Intro\"/element:\"Bob\"/channel:\"position\"/key:\"24\""], "x": 5, "tension": 0.5}"#),
    agent_only = "agents edit points by id (05 §9.4); the motion point properties do the same (M6 C6.2)",
)]
pub(crate) fn anim_path_point_set(cx: &mut Cx, p: PointSetParams) -> Result<PointSetResult, CommandError> {
    let doc = cx.document()?.clone();
    check_tcb("tension", p.tension)?;
    check_tcb("continuity", p.continuity)?;
    check_tcb("bias", p.bias)?;
    for (n, v) in [("x", p.x), ("y", p.y), ("z", p.z)] {
        if let Some(v) = v {
            check_value(n, v)?;
        }
    }
    let mut budget = Budget::new("editing motion points");
    budget.spend(p.point_ids.len())?;
    let mut by_node: std::collections::BTreeMap<Id, Vec<String>> = std::collections::BTreeMap::new();
    for r in &p.point_ids {
        let (e, pid) = point_ref(cx, &doc, r)?;
        by_node.entry(e).or_default().push(pid);
    }
    let mut changes = Changes::default();
    let mut edits = Vec::new();
    for (id, pids) in &by_node {
        let data = data_of(&doc, id, &mut budget)?;
        let lock = data.statics.as_ref().is_some_and(|s| s.lock_depth);
        let mut edit = ChannelEdit::read(&doc, id)?;
        for pid in pids {
            let points = edit.points();
            let Some(i) = points.iter().position(|q| q["id"] == json!(pid)) else {
                return Err(not_found("motion point", pid));
            };
            let path = path_of(&points, lock)?;
            let velocity = velocity_of(&edit)?;
            let key_index = path.key_indices().iter().position(|k| *k == i);
            let mut next = points.clone();
            // Unlocking and locking in time (05 §9.4).
            match (p.lock_in_time, key_index) {
                (Some(false), Some(j)) => {
                    if j == 0 || j + 1 == path.key_indices().len() {
                        return Err(err(
                            "anim.pathEndKey",
                            "the first and last keyframes of a path cannot be unlocked",
                            json!({ "pointId": pid }),
                        ));
                    }
                    merge_velocity(cx, &mut edit, &path, j)?;
                    next[i]["frame"] = Value::Null;
                    next[i]["constantSegment"] = json!(false);
                    next[i]["lockDepth"] = json!(false);
                }
                (Some(true), None) => {
                    let j = control_segment(&path, i);
                    let mut spend = |n: u64| budget.spend(usize::try_from(n).unwrap_or(usize::MAX));
                    let derived = velocity.passage(&path, j, path.arc_of_point(i), &mut spend)?;
                    let local = (derived + 0.5).floor() as u32;
                    if points.iter().any(|q| frame_of(q) == Some(local)) {
                        return Err(key_exists(data.bar.scene(f64::from(local))));
                    }
                    split_velocity(cx, &mut edit, &path, j, local, path.arc_of_point(i))?;
                    next[i]["frame"] = json!(local);
                    edit.touch(local);
                }
                _ => {}
            }
            // Moving a keyframe in time.
            if let Some(scene) = p.frame {
                let local = key_frame(&data, scene)?;
                let Some(j) = key_index else {
                    return Err(frame_invalid(scene, "a control point's frame is derived"));
                };
                if points.iter().enumerate().any(|(o, q)| o != i && frame_of(q) == Some(local)) {
                    return Err(key_exists(scene));
                }
                let lo = j.checked_sub(1).map(|a| path.key_frame(a));
                let hi = (j + 1 < path.key_indices().len()).then(|| path.key_frame(j + 1));
                let f = f64::from(local);
                if lo.is_some_and(|l| f <= l) || hi.is_some_and(|h| f >= h) {
                    return Err(frame_invalid(scene, "between its neighbouring keyframes"));
                }
                let old: Vec<f64> = (0..path.key_indices().len()).map(|k| path.key_frame(k)).collect();
                let mut new = old.clone();
                new[j] = f;
                edit.touch(path.key_frame(j) as u32);
                edit.touch(local);
                retime_velocity(cx, &mut edit, &old, &new)?;
                next[i]["frame"] = json!(local);
            }
            let m = next[i].as_object_mut().ok_or_else(|| super::timing::internal("motion point"))?;
            set_point_fields(m, &p);
            if let Some(f) = frame_of(&next[i]) {
                edit.touch(f);
            }
            let key_frame_now = frame_of(&next[i]);
            edit.set_points(cx, next)?;
            // Per-key channel values (05 §9.4).
            let mut sets: Vec<(&str, f64)> = Vec::new();
            if let Some(v) = p.rotation {
                sets.push(("rotation", v));
            }
            if let Some(v) = p.scale {
                if let Some(a) = v.x {
                    sets.push(("scale.x", a));
                }
                if let Some(a) = v.y {
                    sets.push(("scale.y", a));
                }
            }
            if let Some(v) = p.pivot {
                if let Some(a) = v.x {
                    sets.push(("pivot.x", a));
                }
                if let Some(a) = v.y {
                    sets.push(("pivot.y", a));
                }
            }
            if !sets.is_empty() {
                let Some(local) = key_frame_now else {
                    return Err(frame_invalid(0.0, "rotation, scale and pivot apply to keyframes"));
                };
                for (channel, v) in sets {
                    check_value(channel, v)?;
                    super::anim_function::put_key(cx, &mut edit, node(&doc, id)?, channel, local, v)?;
                }
            }
        }
        edit.stage(&mut changes);
        edits.push((data, edit));
    }
    changes.stage(cx)?;
    for (data, edit) in &edits {
        emit_changed(cx, data, edit);
    }
    Ok(PointSetResult { point_ids: p.point_ids, warnings: Vec::new() })
}

fn set_point_fields(m: &mut Map<String, Value>, p: &PointSetParams) {
    if let Some(pos) = m.get_mut("position") {
        for (k, v) in [("x", p.x), ("y", p.y), ("z", p.z)] {
            if let Some(v) = v {
                pos[k] = json!(v);
            }
        }
    }
    for (k, v) in [("tension", p.tension), ("continuity", p.continuity), ("bias", p.bias)] {
        if let Some(v) = v {
            m.insert(k.to_owned(), json!(v));
        }
    }
    if let Some(c) = p.constant_segment
        && m.get("frame").is_some_and(|f| !f.is_null())
    {
        m.insert("constantSegment".to_owned(), json!(c));
    }
}

// ----- anim.path.point.remove ----------------------------------------------------------------------------------------

/// `anim.path.point.remove` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PointRemoveParams {
    /// The points.
    #[schemars(length(min = 1, max = 100_000))]
    point_ids: Vec<String>,
}

/// `anim.path.point.remove` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct PointRemoveResult {
    /// How many points went.
    removed: u32,
    warnings: Vec<String>,
}

/// Removes one point from an edit: the pin becomes a velocity key, the end rule applies (05 §9.6, §10.5).
pub(crate) fn remove_point(
    cx: &mut Cx,
    edit: &mut ChannelEdit,
    data: &NodeData,
    pid: &str,
    warnings: &mut Vec<String>,
    budget: &mut Budget,
) -> Result<(), CommandError> {
    let lock = data.statics.as_ref().is_some_and(|s| s.lock_depth);
    let points = edit.points();
    let Some(i) = points.iter().position(|q| q["id"] == json!(pid)) else {
        return Err(not_found("motion point", pid));
    };
    let path = path_of(&points, lock)?;
    let velocity = velocity_of(edit)?;
    let mut next = points.clone();
    next.remove(i);
    if let Some(f) = frame_of(&points[i]) {
        edit.touch(f);
    }
    if let Some(j) = path.key_indices().iter().position(|k| *k == i) {
        merge_velocity(cx, edit, &path, j)?;
    }
    if !next.iter().any(|q| frame_of(q).is_some()) {
        // Deleting the last remaining keyframe removes the animation entirely (05 §9.6).
        edit.set_points(cx, Vec::new())?;
        if edit.channels.contains_key("velocity") {
            edit.set_keys(cx, "velocity", Vec::new())?;
        }
        return Ok(());
    }
    // Control points left beyond an end become keyframes at round(derived frame), the next free frame (05 §9.6).
    let last_local = data.bar.cycle.unwrap_or(data.bar.duration) as u32;
    let mut spend = |n: u64| budget.spend(usize::try_from(n).unwrap_or(usize::MAX));
    for end in [0, 1] {
        let pos = if end == 0 { 0 } else { next.len() - 1 };
        if frame_of(&next[pos]).is_some() {
            continue;
        }
        let old_index = points.iter().position(|q| q["id"] == next[pos]["id"]).unwrap_or(0);
        let j = control_segment(&path, old_index);
        let derived = velocity.passage(&path, j, path.arc_of_point(old_index), &mut spend)?;
        let taken: Vec<u32> = next.iter().filter_map(frame_of).collect();
        let mut f = ((derived + 0.5).floor() as u32).clamp(1, last_local);
        while taken.contains(&f) {
            if end == 0 && f > 1 {
                f -= 1;
            } else if f < last_local {
                f += 1;
            } else {
                return Err(frame_invalid(f64::from(f), "no free frame for the converted end point"));
            }
        }
        next[pos]["frame"] = json!(f);
        edit.touch(f);
        warnings.push(format!("a motion point became a keyframe at local frame {f} to end the path"));
    }
    edit.set_points(cx, next)
}

#[command(
    id = "anim.path.point.remove",
    kind = action,
    label = "Remove motion points",
    scopes = [edit],
    undo = "Remove Motion Point",
    needs_project,
    example(title = "Remove Bob's last keyframe", params = r#"{"pointIds": ["scene:\"Intro\"/element:\"Bob\"/channel:\"position\"/key:\"24\""]}"#),
    agent_only = "agents remove points by id (05 §9.6); the motion point properties and Delete do the same (M6 C6.2)",
)]
pub(crate) fn anim_path_point_remove(cx: &mut Cx, p: PointRemoveParams) -> Result<PointRemoveResult, CommandError> {
    let doc = cx.document()?.clone();
    let mut budget = Budget::new("removing motion points");
    budget.spend(p.point_ids.len())?;
    let mut by_node: std::collections::BTreeMap<Id, Vec<String>> = std::collections::BTreeMap::new();
    for r in &p.point_ids {
        let (e, pid) = point_ref(cx, &doc, r)?;
        let list = by_node.entry(e).or_default();
        if !list.contains(&pid) {
            list.push(pid);
        }
    }
    let mut changes = Changes::default();
    let mut warnings = Vec::new();
    let mut removed = 0usize;
    let mut edits = Vec::new();
    for (id, pids) in &by_node {
        let data = data_of(&doc, id, &mut budget)?;
        let mut edit = ChannelEdit::read(&doc, id)?;
        for pid in pids {
            if !edit.points().iter().any(|q| q["id"] == json!(pid)) {
                // Gone with the last keyframe.
                continue;
            }
            remove_point(cx, &mut edit, &data, pid, &mut warnings, &mut budget)?;
            removed += 1;
        }
        edit.stage(&mut changes);
        edits.push((data, edit, id.clone()));
    }
    changes.stage(cx)?;
    for (data, edit, id) in &edits {
        emit_changed(cx, data, edit);
        for w in &warnings {
            cx.warn("anim.pathEndConverted", w.clone(), vec![id.as_str().to_owned()]);
        }
    }
    Ok(PointRemoveResult { removed: as_u32(removed), warnings })
}

// ----- anim.path.lockDepth.set ---------------------------------------------------------------------------------------

/// `anim.path.lockDepth.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct LockDepthParams {
    /// The node.
    node_id: String,
    /// Scene frame of the keyframe starting the segment.
    frame: i64,
    /// Constant Z on or off.
    enabled: bool,
}

/// `anim.path.lockDepth.set` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LockDepthResult {
    node_id: String,
    frame: i64,
}

#[command(
    id = "anim.path.lockDepth.set",
    kind = action,
    label = "Constant Z",
    scopes = [edit],
    undo = "Toggle Constant Z",
    needs_project,
    example(title = "Keep Z constant in the segment starting at Bob's frame 1", params = r#"{"nodeId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1, "enabled": true}"#),
    gui = "Motion point properties, Animation menu: Toggle Constant Z (M6 C6.2)",
)]
pub(crate) fn anim_path_lock_depth_set(cx: &mut Cx, p: LockDepthParams) -> Result<LockDepthResult, CommandError> {
    let doc = cx.document()?.clone();
    let id = node_id(cx, &p.node_id)?;
    transformable(&doc, &id)?;
    let mut budget = Budget::new("toggling constant Z");
    let data = data_of(&doc, &id, &mut budget)?;
    let local = key_frame(&data, p.frame as f64)?;
    let mut edit = ChannelEdit::read(&doc, &id)?;
    let mut points = edit.points();
    let Some(q) = points.iter_mut().find(|q| frame_of(q) == Some(local)) else {
        return Err(frame_invalid(p.frame as f64, "no motion keyframe at this frame"));
    };
    q["lockDepth"] = json!(p.enabled);
    edit.touch(local);
    edit.set_points(cx, points)?;
    let mut changes = Changes::default();
    edit.stage(&mut changes);
    changes.stage(cx)?;
    emit_changed(cx, &data, &edit);
    Ok(LockDepthResult { node_id: id.as_str().to_owned(), frame: p.frame })
}
