//! 1-D channels (05 §10.3, §10.4, §19.3; M3 C3.2): `anim.function.*`. The `velocity` channel is stored normalised to its
//! keyframe segment and reported in fields, with implicit pins (05 §10.5); `position.x/y/z` are projections of the
//! motion path, whose keys are edited through the path (05 §11.4, SI-096).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_anim::function::preset_points;
use tt_anim::node::NodeData;
use tt_anim::num::tidy;
use tt_anim::tcb::{self, Path};
use tt_model::Id;
use tt_model::entities::channel::{CurveType, Easing, HandleMode};
use tt_model::entities::element::Element;
use tt_registry::{CommandError, command};

use super::anim_common::{
    ChannelEdit, check_channel, check_value, data_of, default_shape, emit_changed, frame_invalid, frame_of, key_frame,
    new_key, node, node_id, unsupported,
};
use super::anim_path::{key_exists, path_of};
use super::timing::{Budget, Changes, as_u32, err};
use crate::cx::Cx;

/// `anim.curveTypeUnsupported`.
fn curve_unsupported(channel: &str, what: &str) -> CommandError {
    err(
        "anim.curveTypeUnsupported",
        format!("`{what}` is not allowed for channel `{channel}`"),
        json!({ "channel": channel, "what": what }),
    )
}

/// The curve types a channel allows (05 §10.3).
fn allowed_curves(channel: &str, has_path: bool) -> Vec<CurveType> {
    match channel {
        "position" => Vec::new(),
        "rotation" | "scale.x" | "scale.y" | "skew" if has_path => {
            vec![CurveType::Spline, CurveType::Interpolation, CurveType::VelocityBased]
        }
        _ => vec![CurveType::Spline, CurveType::Interpolation],
    }
}

/// Whether the channel id names a path projection.
fn is_projection(channel: &str) -> bool {
    matches!(channel, "position.x" | "position.y" | "position.z")
}

/// Refuses the motion path itself and, for mutations, its projections (05 §11.4: value edits go through the path).
fn one_d(e: &Element, channel: &str) -> Result<(), CommandError> {
    check_channel(e, channel)?;
    if channel == "position" || is_projection(channel) {
        return Err(unsupported(channel, e, "a motion-path channel: use anim.path.* (05 §9, §11.4)"));
    }
    Ok(())
}

/// Adds or overwrites the key of a 1-D channel at a local frame, keeping an existing key's ID and shape.
pub(crate) fn put_key(
    cx: &mut Cx,
    edit: &mut ChannelEdit,
    e: &Element,
    channel: &str,
    local: u32,
    value: f64,
) -> Result<(), CommandError> {
    check_channel(e, channel)?;
    let mut keys = edit.keys(channel);
    edit.touch(local);
    match keys.iter_mut().find(|k| frame_of(k) == Some(local)) {
        Some(k) => k["value"] = json!(value),
        None => {
            let shape = default_shape(cx);
            let k = new_key(cx, local, value, shape)?;
            let at = keys.partition_point(|o| frame_of(o).is_some_and(|f| f < local));
            keys.insert(at, k);
        }
    }
    edit.set_keys(cx, channel, keys)
}

/// The pin frames and arcs of a node's path, in local frames.
fn pins(path: &Path) -> Vec<(f64, f64)> {
    (0..path.key_indices().len()).map(|j| (path.key_frame(j), path.key_arc(j))).collect()
}

/// The fields value of a stored velocity key at a local frame.
fn velocity_fields(pins: &[(f64, f64)], frame: f64, u: f64) -> f64 {
    let i = pins.partition_point(|p| p.0 <= frame);
    match (i.checked_sub(1).and_then(|a| pins.get(a)), pins.get(i)) {
        (Some(a), _) if a.0 == frame => a.1,
        (Some(a), Some(b)) => a.1 + u * (b.1 - a.1),
        (Some(a), None) => a.1,
        (None, Some(b)) => b.1,
        (None, None) => 0.0,
    }
}

/// The stored `u` of a velocity value in fields at a local frame.
fn velocity_u(pins: &[(f64, f64)], frame: f64, value: f64) -> f64 {
    let i = pins.partition_point(|p| p.0 <= frame);
    match (i.checked_sub(1).and_then(|a| pins.get(a)), pins.get(i)) {
        (Some(a), Some(b)) if b.1 > a.1 => (value - a.1) / (b.1 - a.1),
        _ => 0.0,
    }
}

/// The node's path, required for the velocity channel.
fn velocity_path<'a>(e: &Element, data: &'a NodeData) -> Result<&'a Path, CommandError> {
    data.path.as_ref().filter(|p| p.key_indices().len() >= 2).ok_or_else(|| {
        unsupported("velocity", e, "the velocity channel needs a motion path with at least two keyframes")
    })
}

// ----- anim.function.list ------------------------------------------------------------------------------------------

/// A node-and-channel parameter.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct NodeParams {
    /// The node.
    node_id: String,
}

/// One channel of `anim.function.list`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListedChannel {
    channel: String,
    /// Curve type (`null` for the motion path).
    curve_type: Option<CurveType>,
    /// Keys, or motion keyframes for the path.
    key_count: u32,
    allowed_curve_types: Vec<CurveType>,
}

/// `anim.function.list` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct FunctionListResult {
    channels: Vec<ListedChannel>,
}

#[command(
    id = "anim.function.list",
    kind = query,
    label = "List channels",
    scopes = [read],
    needs_project,
    example(title = "Channels of Bob", params = r#"{"nodeId": "scene:\"Intro\"/element:\"Bob\""}"#),
    gui = "Function editor: channel picker (M6 C6.2)",
)]
pub(crate) fn anim_function_list(cx: &mut Cx, p: NodeParams) -> Result<FunctionListResult, CommandError> {
    let doc = cx.document()?.clone();
    let id = node_id(cx, &p.node_id)?;
    let e = node(&doc, &id)?;
    let stored = e.channels().clone().unwrap_or_default();
    let has_path = stored.get("position").is_some();
    let channels = e
        .element_type()
        .allowed_channels()
        .iter()
        .map(|c| {
            let ch = stored.get(*c);
            let key_count = match *c {
                "position" => ch
                    .and_then(|x| x.points().as_ref())
                    .map_or(0, |ps| ps.iter().filter(|q| q.frame().is_some()).count()),
                _ => ch.and_then(|x| x.keys().as_ref()).map_or(0, Vec::len),
            };
            ListedChannel {
                channel: (*c).to_owned(),
                curve_type: if *c == "position" {
                    None
                } else {
                    Some(ch.and_then(|x| *x.curve_type()).unwrap_or(CurveType::Interpolation))
                },
                key_count: as_u32(key_count),
                allowed_curve_types: allowed_curves(c, has_path),
            }
        })
        .collect();
    Ok(FunctionListResult { channels })
}

// ----- anim.function.get -------------------------------------------------------------------------------------------

/// A handle.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Handle {
    /// Frames from the key (left ≤ 0, right ≥ 0).
    dt: f64,
    /// Value units from the key.
    dv: f64,
}

/// A key point of `Function`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FunctionKey {
    /// Scene frame.
    frame: f64,
    /// Value.
    value: f64,
    constant_segment: bool,
    handle_mode: HandleMode,
    left: Handle,
    right: Handle,
    /// The easing preset of the segment starting here.
    preset: Option<Easing>,
}

/// A velocity pin.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Pin {
    /// Scene frame of the motion keyframe.
    frame: f64,
    /// Arc length, fields.
    value: f64,
}

/// `anim.function.get` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ChannelParams {
    /// The node.
    node_id: String,
    /// The channel (05 §1.6).
    #[schemars(length(min = 1, max = 256))]
    channel: String,
}

/// `anim.function.get` result (05 §19.4).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FunctionResult {
    node_id: String,
    channel: String,
    curve_type: CurveType,
    keys: Vec<FunctionKey>,
    pins: Vec<Pin>,
}

fn key_of(v: &Value, bar: &tt_anim::timing::Bar, value: f64) -> Result<FunctionKey, CommandError> {
    let k: tt_model::entities::channel::Key = serde_json::from_value(v.clone()).map_err(super::timing::internal)?;
    let h = k.handles();
    Ok(FunctionKey {
        frame: tidy(bar.scene(f64::from(*k.frame()))),
        value: tidy(value),
        constant_segment: *k.constant_segment(),
        handle_mode: h.mode,
        left: Handle { dt: tidy(h.left.dt), dv: tidy(h.left.dv) },
        right: Handle { dt: tidy(h.right.dt), dv: tidy(h.right.dv) },
        preset: *k.easing(),
    })
}

#[command(
    id = "anim.function.get",
    kind = query,
    label = "Get function curve",
    scopes = [read],
    needs_project,
    example(title = "Bob's rotation keys", params = r#"{"nodeId": "scene:\"Intro\"/element:\"Bob\"", "channel": "rotation"}"#),
    agent_only = "agents read curves as key lists (05 §19.4); the function editor shows them (M6 C6.2)",
)]
pub(crate) fn anim_function_get(cx: &mut Cx, p: ChannelParams) -> Result<FunctionResult, CommandError> {
    let doc = cx.document()?.clone();
    let id = node_id(cx, &p.node_id)?;
    let e = node(&doc, &id)?;
    check_channel(e, &p.channel)?;
    if p.channel == "position" {
        return Err(unsupported(&p.channel, e, "the motion path is 3-D: anim.path.get, or position.x, .y or .z"));
    }
    let mut budget = Budget::new("reading a channel");
    let data = data_of(&doc, &id, &mut budget)?;
    let edit = ChannelEdit::read(&doc, &id)?;
    let mut out = FunctionResult {
        node_id: id.as_str().to_owned(),
        channel: p.channel.clone(),
        curve_type: edit
            .channels
            .get(&p.channel)
            .and_then(|c| c.get("curveType"))
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or(CurveType::Interpolation),
        keys: Vec::new(),
        pins: Vec::new(),
    };
    if is_projection(&p.channel) {
        // Keyframes are the key points of a projection (05 §11.4).
        let axis = match p.channel.as_str() {
            "position.x" => 0,
            "position.y" => 1,
            _ => 2,
        };
        out.curve_type = CurveType::Spline;
        if let Some(path) = &data.path {
            for q in &path.points {
                if let Some(f) = q.frame {
                    out.keys.push(FunctionKey {
                        frame: tidy(data.bar.scene(f)),
                        value: tidy(q.pos[axis]),
                        constant_segment: q.constant,
                        handle_mode: HandleMode::Smooth,
                        left: Handle::default(),
                        right: Handle::default(),
                        preset: None,
                    });
                }
            }
        }
        return Ok(out);
    }
    if p.channel == "velocity" {
        if let Some(path) = &data.path {
            let pinned = pins(path);
            out.pins = pinned.iter().map(|(f, s)| Pin { frame: tidy(data.bar.scene(*f)), value: tidy(*s) }).collect();
            for k in edit.keys("velocity") {
                let f = frame_of(&k).map_or(1.0, f64::from);
                if pinned.iter().any(|q| q.0 == f) {
                    continue;
                }
                let u = k["value"].as_f64().unwrap_or(0.0);
                out.keys.push(key_of(&k, &data.bar, velocity_fields(&pinned, f, u))?);
            }
        }
        return Ok(out);
    }
    for k in edit.keys(&p.channel) {
        let v = k["value"].as_f64().unwrap_or(0.0);
        out.keys.push(key_of(&k, &data.bar, v)?);
    }
    Ok(out)
}

// ----- anim.function.set -------------------------------------------------------------------------------------------

/// A key of `anim.function.set`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SetKey {
    /// Scene frame (an integer: `anim.frameNotInteger`).
    frame: f64,
    /// Value.
    value: f64,
    #[serde(default)]
    constant_segment: Option<bool>,
    #[serde(default)]
    handle_mode: Option<HandleMode>,
    #[serde(default)]
    left: Option<Handle>,
    #[serde(default)]
    right: Option<Handle>,
    #[serde(default)]
    preset: Option<Easing>,
}

/// `anim.function.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct FunctionSetParams {
    /// The node.
    node_id: String,
    /// The channel.
    #[schemars(length(min = 1, max = 256))]
    channel: String,
    /// The curve type.
    #[serde(default)]
    curve_type: Option<CurveType>,
    /// The whole key list; empty removes the channel.
    #[schemars(length(max = 100_000))]
    keys: Vec<SetKey>,
}

/// `anim.function.set` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FunctionSetResult {
    node_id: String,
    channel: String,
    key_count: u32,
}

/// Clamps handles per 05 §10.3 against neighbouring frames.
fn clamp_handles(keys: &mut [Value]) {
    let frames: Vec<f64> = keys.iter().map(|k| frame_of(k).map_or(0.0, f64::from)).collect();
    for (i, k) in keys.iter_mut().enumerate() {
        let next = frames.get(i + 1).map_or(f64::INFINITY, |n| n - frames[i]);
        let prev = i.checked_sub(1).and_then(|a| frames.get(a)).map_or(f64::INFINITY, |p| frames[i] - p);
        if let Some(h) = k.get_mut("handles") {
            let r = h["right"]["dt"].as_f64().unwrap_or(0.0).clamp(0.0, next);
            let l = h["left"]["dt"].as_f64().unwrap_or(0.0).clamp(-prev, 0.0);
            h["right"]["dt"] = json!(r);
            h["left"]["dt"] = json!(l);
        }
    }
}

fn check_curve(channel: &str, curve: CurveType, has_path: bool) -> Result<(), CommandError> {
    if !allowed_curves(channel, has_path).contains(&curve) {
        return Err(curve_unsupported(channel, &serde_json::to_string(&curve).unwrap_or_default()));
    }
    Ok(())
}

#[command(
    id = "anim.function.set",
    kind = action,
    label = "Set function curve",
    scopes = [edit],
    undo = "Set Function",
    needs_project,
    example(title = "Rotate Bob 45 degrees over frames 1-12, easing in and out", params = r#"{"nodeId": "scene:\"Intro\"/element:\"Bob\"", "channel": "rotation", "curveType": "interpolation", "keys": [{"frame": 1, "value": 0, "preset": "easeInOut"}, {"frame": 12, "value": 45}]}"#),
    agent_only = "agents replace a key list (05 §19.3); the function editor edits points (M6 C6.2)",
)]
pub(crate) fn anim_function_set(cx: &mut Cx, p: FunctionSetParams) -> Result<FunctionSetResult, CommandError> {
    let doc = cx.document()?.clone();
    let id = node_id(cx, &p.node_id)?;
    let e = node(&doc, &id)?;
    one_d(e, &p.channel)?;
    let mut budget = Budget::new("setting a channel");
    let data = data_of(&doc, &id, &mut budget)?;
    budget.spend(p.keys.len())?;
    let is_velocity = p.channel == "velocity";
    let pinned = if is_velocity { pins(velocity_path(e, &data)?) } else { Vec::new() };
    if let Some(c) = p.curve_type {
        check_curve(&p.channel, c, data.path.is_some())?;
    }
    let mut edit = ChannelEdit::read(&doc, &id)?;
    let existing: BTreeMap<u32, Value> =
        edit.keys(&p.channel).into_iter().filter_map(|k| frame_of(&k).map(|f| (f, k))).collect();
    let mut out: BTreeMap<u32, Value> = BTreeMap::new();
    for k in &p.keys {
        let local = key_frame(&data, k.frame)?;
        if out.contains_key(&local) {
            return Err(key_exists(k.frame));
        }
        let mut stored = k.value;
        if is_velocity {
            let f = f64::from(local);
            if let Some(pin) = pinned.iter().find(|q| q.0 == f)
                && (pin.1 - k.value).abs() > 1e-9
            {
                return Err(unsupported("velocity", e, "a pin's value follows its keyframe (05 §10.5)"));
            }
            if k.constant_segment == Some(true) || k.preset == Some(Easing::Constant) {
                return Err(unsupported("velocity", e, "the velocity channel can never be constant (05 §10.2)"));
            }
            stored = velocity_u(&pinned, f, k.value);
        } else {
            check_value(&p.channel, k.value)?;
        }
        let shape = default_shape(cx);
        let mut v = match existing.get(&local) {
            Some(old) => old.clone(),
            None => new_key(cx, local, stored, shape)?,
        };
        v["value"] = json!(stored);
        if let Some(c) = k.constant_segment {
            v["constantSegment"] = json!(c);
        }
        if let Some(e2) = k.preset {
            v["easing"] = json!(e2);
            if e2 == Easing::Constant {
                v["constantSegment"] = json!(true);
            }
        }
        if let Some(m) = k.handle_mode {
            v["handles"]["mode"] = json!(m);
        }
        if let Some(l) = k.left {
            v["handles"]["left"] = json!({ "dt": l.dt, "dv": l.dv });
        }
        if let Some(r) = k.right {
            v["handles"]["right"] = json!({ "dt": r.dt, "dv": r.dv });
        }
        edit.touch(local);
        out.insert(local, v);
    }
    let mut keys: Vec<Value> = out.into_values().collect();
    clamp_handles(&mut keys);
    let count = keys.len();
    edit.set_keys(cx, &p.channel, keys)?;
    if let (Some(c), Some(ch)) = (p.curve_type, edit.channels.get_mut(&p.channel)) {
        ch["curveType"] = json!(c);
    }
    super::anim_common::drop_absent_removals(&doc, &mut edit)?;
    let mut changes = Changes::default();
    edit.stage(&mut changes);
    changes.stage(cx)?;
    emit_changed(cx, &data, &edit);
    Ok(FunctionSetResult { node_id: id.as_str().to_owned(), channel: p.channel, key_count: as_u32(count) })
}

// ----- anim.function.key.add ---------------------------------------------------------------------------------------

/// `anim.function.key.add` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct KeyAddParams {
    /// The node.
    node_id: String,
    /// The channel.
    #[schemars(length(min = 1, max = 256))]
    channel: String,
    /// Scene frame (an integer: `anim.frameNotInteger`).
    frame: f64,
    /// Value; default: the curve's current value.
    #[serde(default)]
    value: Option<f64>,
}

/// `{frame, value}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct FrameValue {
    /// Scene frame.
    frame: f64,
    /// Value.
    value: f64,
}

/// The current value of a channel at a cycle frame (fields for `velocity`).
fn current_value(data: &NodeData, channel: &str, f: f64) -> f64 {
    match channel {
        "velocity" => data.velocity_at(f),
        c => data.curve_at(c, f).unwrap_or(match c {
            "scale.x" | "scale.y" => 1.0,
            _ => 0.0,
        }),
    }
}

#[command(
    id = "anim.function.key.add",
    kind = action,
    label = "Add function key",
    scopes = [edit],
    undo = "Add Key",
    needs_project,
    example(title = "Add a rotation key on Bob at frame 6", params = r#"{"nodeId": "scene:\"Intro\"/element:\"Bob\"", "channel": "rotation", "frame": 6, "value": 10}"#),
    gui = "Function editor: add key (M6 C6.2)",
)]
pub(crate) fn anim_function_key_add(cx: &mut Cx, p: KeyAddParams) -> Result<FrameValue, CommandError> {
    let doc = cx.document()?.clone();
    let id = node_id(cx, &p.node_id)?;
    let e = node(&doc, &id)?;
    one_d(e, &p.channel)?;
    let mut budget = Budget::new("adding a key");
    let data = data_of(&doc, &id, &mut budget)?;
    let local = key_frame(&data, p.frame)?;
    let mut edit = ChannelEdit::read(&doc, &id)?;
    if edit.keys(&p.channel).iter().any(|k| frame_of(k) == Some(local)) {
        return Err(key_exists(p.frame));
    }
    let f = f64::from(local);
    let value = p.value.unwrap_or_else(|| current_value(&data, &p.channel, f));
    if p.channel == "velocity" {
        let path = velocity_path(e, &data)?;
        let pinned = pins(path);
        if pinned.iter().any(|q| q.0 == f) {
            return Err(key_exists(p.frame));
        }
        check_value("velocity", value)?;
        put_key(cx, &mut edit, e, "velocity", local, velocity_u(&pinned, f, value))?;
    } else {
        check_value(&p.channel, value)?;
        put_key(cx, &mut edit, e, &p.channel, local, value)?;
    }
    let mut changes = Changes::default();
    edit.stage(&mut changes);
    changes.stage(cx)?;
    emit_changed(cx, &data, &edit);
    Ok(FrameValue { frame: tidy(p.frame), value: tidy(value) })
}

// ----- anim.function.key.set ---------------------------------------------------------------------------------------

/// `anim.function.key.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct KeySetParams {
    /// The node.
    node_id: String,
    /// The channel.
    #[schemars(length(min = 1, max = 256))]
    channel: String,
    /// Scene frame of the key (a control point's passage frame on a path projection).
    frame: f64,
    /// Moves the key in time.
    #[serde(default)]
    new_frame: Option<f64>,
    /// New value.
    #[serde(default)]
    value: Option<f64>,
    #[serde(default)]
    left: Option<Handle>,
    #[serde(default)]
    right: Option<Handle>,
    #[serde(default)]
    handle_mode: Option<HandleMode>,
    #[serde(default)]
    constant_segment: Option<bool>,
    /// Path projections: lock a control point in time, or unlock a keyframe.
    #[serde(default)]
    lock_in_time: Option<bool>,
}

/// A smooth handle pair: the other handle turns to stay collinear through the key, keeping its length (05 §10.3).
fn smooth_other(moved: Handle, other: Handle) -> Handle {
    let m = (moved.dt * moved.dt + moved.dv * moved.dv).sqrt();
    let o = (other.dt * other.dt + other.dv * other.dv).sqrt();
    if m == 0.0 || o == 0.0 {
        return other;
    }
    Handle { dt: -moved.dt / m * o, dv: -moved.dv / m * o }
}

fn handle_of(v: &Value) -> Handle {
    Handle { dt: v["dt"].as_f64().unwrap_or(0.0), dv: v["dv"].as_f64().unwrap_or(0.0) }
}

/// A key edit on a path projection goes through the path (05 §11.4).
fn projection_key_set(cx: &mut Cx, p: &KeySetParams, id: &Id, data: &NodeData) -> Result<FrameValue, CommandError> {
    let doc = cx.document()?.clone();
    let e = node(&doc, id)?;
    let axis = match p.channel.as_str() {
        "position.x" => "x",
        "position.y" => "y",
        _ => "z",
    };
    if p.left.is_some() || p.right.is_some() || p.handle_mode.is_some() {
        return Err(unsupported(&p.channel, e, "motion paths have TCB parameters, not handles"));
    }
    let channel = e.channels().as_ref().and_then(|m| m.get("position"));
    let (Some(channel), Some(path)) = (channel, &data.path) else {
        return Err(frame_invalid(p.frame, "this node has no motion path"));
    };
    let stored = channel.points().clone().unwrap_or_default();
    let local = p.frame - data.bar.start + 1.0;
    let mut budget = Budget::new("editing a path projection");
    let mut spend = |n: u64| budget.spend(usize::try_from(n).unwrap_or(usize::MAX));
    let mut found = None;
    for (i, q) in path.points.iter().enumerate() {
        let at = match q.frame {
            Some(f) => f,
            None => {
                let j = path.key_indices().partition_point(|k| *k < i).saturating_sub(1);
                data.velocity.passage(path, j, path.arc_of_point(i), &mut spend)?
            }
        };
        if (at - local).abs() < 0.001 {
            found = stored.get(i).map(|m| (m.id().as_str().to_owned(), q.pos));
            break;
        }
    }
    let Some((pid, pos)) = found else { return Err(frame_invalid(p.frame, "no motion point at this frame")) };
    let mut params = json!({ "pointIds": [format!("{}/{pid}", channel.id())] });
    if let Some(v) = p.value {
        params[axis] = json!(v);
    }
    if let Some(v) = p.new_frame {
        params["frame"] = json!(v);
    }
    if let Some(v) = p.constant_segment {
        params["constantSegment"] = json!(v);
    }
    if let Some(v) = p.lock_in_time {
        params["lockInTime"] = json!(v);
    }
    let parsed = serde_json::from_value(params).map_err(super::timing::internal)?;
    super::anim_path::anim_path_point_set(cx, parsed)?;
    let index = match axis {
        "x" => 0,
        "y" => 1,
        _ => 2,
    };
    Ok(FrameValue { frame: tidy(p.new_frame.unwrap_or(p.frame)), value: tidy(p.value.unwrap_or(pos[index])) })
}

#[command(
    id = "anim.function.key.set",
    kind = action,
    label = "Edit function key",
    scopes = [edit],
    undo = "Change Key",
    needs_project,
    example(title = "Move Bob's first rotation key value and give it a right handle", params = r#"{"nodeId": "scene:\"Intro\"/element:\"Bob\"", "channel": "rotation", "frame": 1, "value": 5, "right": {"dt": 2, "dv": 1}}"#),
    gui = "Function editor: values panel, handles, lock in time (M6 C6.2)",
)]
pub(crate) fn anim_function_key_set(cx: &mut Cx, p: KeySetParams) -> Result<FrameValue, CommandError> {
    let doc = cx.document()?.clone();
    let id = node_id(cx, &p.node_id)?;
    let e = node(&doc, &id)?;
    check_channel(e, &p.channel)?;
    let mut budget = Budget::new("editing a key");
    let data = data_of(&doc, &id, &mut budget)?;
    if is_projection(&p.channel) {
        return projection_key_set(cx, &p, &id, &data);
    }
    one_d(e, &p.channel)?;
    if p.lock_in_time.is_some() {
        return Err(unsupported(&p.channel, e, "lock in time applies to motion points"));
    }
    let local = key_frame(&data, p.frame)?;
    let mut edit = ChannelEdit::read(&doc, &id)?;
    let mut keys = edit.keys(&p.channel);
    let is_velocity = p.channel == "velocity";
    let pinned = if is_velocity { pins(velocity_path(e, &data)?) } else { Vec::new() };
    let f = f64::from(local);
    let on_pin = pinned.iter().any(|q| q.0 == f);
    if is_velocity {
        if p.constant_segment == Some(true) {
            return Err(unsupported("velocity", e, "the velocity channel can never be constant (05 §10.2)"));
        }
        if on_pin && (p.value.is_some() || p.new_frame.is_some()) {
            return Err(unsupported("velocity", e, "a pin moves with its keyframe; it cannot be changed directly"));
        }
        if on_pin && !keys.iter().any(|k| frame_of(k) == Some(local)) {
            // The pin's shape record.
            let shape = default_shape(cx);
            let k = new_key(cx, local, 0.0, shape)?;
            let at = keys.partition_point(|o| frame_of(o).is_some_and(|x| x < local));
            keys.insert(at, k);
        }
    }
    let Some(i) = keys.iter().position(|k| frame_of(k) == Some(local)) else {
        return Err(frame_invalid(p.frame, "no key at this frame"));
    };
    let mut value = if is_velocity {
        velocity_fields(&pinned, f, keys[i]["value"].as_f64().unwrap_or(0.0))
    } else {
        keys[i]["value"].as_f64().unwrap_or(0.0)
    };
    if let Some(v) = p.value {
        check_value(if is_velocity { "velocity" } else { &p.channel }, v)?;
        value = v;
        keys[i]["value"] = json!(if is_velocity { velocity_u(&pinned, f, v) } else { v });
    }
    if let Some(c) = p.constant_segment {
        keys[i]["constantSegment"] = json!(c);
    }
    if let Some(m) = p.handle_mode {
        keys[i]["handles"]["mode"] = json!(m);
    }
    let smooth = keys[i]["handles"]["mode"].as_str() == Some("smooth");
    if let Some(l) = p.left {
        keys[i]["handles"]["left"] = json!({ "dt": l.dt, "dv": l.dv });
        if smooth && p.right.is_none() {
            let r = smooth_other(l, handle_of(&keys[i]["handles"]["right"]));
            keys[i]["handles"]["right"] = json!({ "dt": r.dt, "dv": r.dv });
        }
    }
    if let Some(r) = p.right {
        keys[i]["handles"]["right"] = json!({ "dt": r.dt, "dv": r.dv });
        if smooth && p.left.is_none() {
            let l = smooth_other(r, handle_of(&keys[i]["handles"]["left"]));
            keys[i]["handles"]["left"] = json!({ "dt": l.dt, "dv": l.dv });
        }
    }
    edit.touch(local);
    let mut frame = p.frame;
    if let Some(nf) = p.new_frame {
        let to = key_frame(&data, nf)?;
        if to != local {
            if keys.iter().any(|k| frame_of(k) == Some(to)) || pinned.iter().any(|q| q.0 == f64::from(to)) {
                return Err(key_exists(nf));
            }
            let lo = i.checked_sub(1).and_then(|a| keys.get(a)).and_then(frame_of);
            let hi = keys.get(i + 1).and_then(frame_of);
            if lo.is_some_and(|l| to <= l) || hi.is_some_and(|h| to >= h) {
                return Err(frame_invalid(nf, "a key cannot pass its neighbours"));
            }
            if is_velocity {
                let s = pinned.partition_point(|q| q.0 <= f);
                let cross = pinned.get(s.wrapping_sub(1)).is_some_and(|a| f64::from(to) <= a.0)
                    || pinned.get(s).is_some_and(|b| f64::from(to) >= b.0);
                if cross {
                    return Err(frame_invalid(nf, "an intermediate velocity key stays inside its keyframe segment"));
                }
            }
            keys[i]["frame"] = json!(to);
            edit.touch(to);
            frame = nf;
        }
    }
    clamp_handles(&mut keys);
    edit.set_keys(cx, &p.channel, keys)?;
    let mut changes = Changes::default();
    edit.stage(&mut changes);
    changes.stage(cx)?;
    emit_changed(cx, &data, &edit);
    Ok(FrameValue { frame: tidy(frame), value: tidy(value) })
}

// ----- anim.function.key.remove ------------------------------------------------------------------------------------

/// `anim.function.key.remove` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct KeyRemoveParams {
    /// The node.
    node_id: String,
    /// The channel.
    #[schemars(length(min = 1, max = 256))]
    channel: String,
    /// Scene frames.
    #[schemars(length(min = 1, max = 100_000))]
    frames: Vec<f64>,
}

/// `{removed}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct RemovedResult {
    /// How many keys went.
    removed: u32,
}

#[command(
    id = "anim.function.key.remove",
    kind = action,
    label = "Remove function keys",
    scopes = [edit],
    undo = "Remove Key",
    needs_project,
    example(title = "Remove Bob's rotation key at frame 1", params = r#"{"nodeId": "scene:\"Intro\"/element:\"Bob\"", "channel": "rotation", "frames": [1]}"#),
    gui = "Function editor: delete key (M6 C6.2)",
)]
pub(crate) fn anim_function_key_remove(cx: &mut Cx, p: KeyRemoveParams) -> Result<RemovedResult, CommandError> {
    let doc = cx.document()?.clone();
    let id = node_id(cx, &p.node_id)?;
    let e = node(&doc, &id)?;
    one_d(e, &p.channel)?;
    let mut budget = Budget::new("removing keys");
    let data = data_of(&doc, &id, &mut budget)?;
    budget.spend(p.frames.len())?;
    let mut edit = ChannelEdit::read(&doc, &id)?;
    let mut keys = edit.keys(&p.channel);
    let mut removed = 0usize;
    for frame in &p.frames {
        let local = key_frame(&data, *frame)?;
        if p.channel == "velocity"
            && data.path.as_ref().is_some_and(|q| pins(q).iter().any(|a| a.0 == f64::from(local)))
        {
            return Err(unsupported("velocity", e, "pins are implicit: they go with their keyframe (05 §10.5)"));
        }
        let Some(i) = keys.iter().position(|k| frame_of(k) == Some(local)) else {
            return Err(frame_invalid(*frame, "no key at this frame"));
        };
        keys.remove(i);
        edit.touch(local);
        removed += 1;
    }
    edit.set_keys(cx, &p.channel, keys)?;
    let mut changes = Changes::default();
    edit.stage(&mut changes);
    changes.stage(cx)?;
    emit_changed(cx, &data, &edit);
    Ok(RemovedResult { removed: as_u32(removed) })
}

// ----- anim.function.key.duplicate ---------------------------------------------------------------------------------

/// `anim.function.key.duplicate` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct KeyDuplicateParams {
    /// The node.
    node_id: String,
    /// The channel.
    #[schemars(length(min = 1, max = 256))]
    channel: String,
    /// Scene frame of the key.
    frame: i64,
    /// Scene frame of the copy.
    to_frame: i64,
}

/// `{frame}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct FrameResult {
    /// The copy's scene frame.
    frame: i64,
}

#[command(
    id = "anim.function.key.duplicate",
    kind = action,
    label = "Duplicate function key",
    scopes = [edit],
    undo = "Duplicate Key",
    needs_project,
    example(title = "Copy Bob's rotation key at frame 1 to frame 6", params = r#"{"nodeId": "scene:\"Intro\"/element:\"Bob\"", "channel": "rotation", "frame": 1, "toFrame": 6}"#),
    gui = "Function editor: drag a key with the duplicate modifier (M6 C6.2)",
)]
pub(crate) fn anim_function_key_duplicate(cx: &mut Cx, p: KeyDuplicateParams) -> Result<FrameResult, CommandError> {
    let doc = cx.document()?.clone();
    let id = node_id(cx, &p.node_id)?;
    let e = node(&doc, &id)?;
    one_d(e, &p.channel)?;
    if p.channel == "velocity" {
        return Err(unsupported("velocity", e, "velocity keys are normalised to their segment: use key.add"));
    }
    let mut budget = Budget::new("duplicating a key");
    let data = data_of(&doc, &id, &mut budget)?;
    let local = key_frame(&data, p.frame as f64)?;
    let to = key_frame(&data, p.to_frame as f64)?;
    let mut edit = ChannelEdit::read(&doc, &id)?;
    let mut keys = edit.keys(&p.channel);
    let Some(source) = keys.iter().find(|k| frame_of(k) == Some(local)).cloned() else {
        return Err(frame_invalid(p.frame as f64, "no key at this frame"));
    };
    if keys.iter().any(|k| frame_of(k) == Some(to)) {
        return Err(key_exists(p.to_frame as f64));
    }
    let mut copy = source;
    copy["id"] = json!(cx.new_id(tt_model::ObjectType::Key)?.as_str());
    copy["frame"] = json!(to);
    let at = keys.partition_point(|o| frame_of(o).is_some_and(|f| f < to));
    keys.insert(at, copy);
    clamp_handles(&mut keys);
    edit.touch(to);
    edit.set_keys(cx, &p.channel, keys)?;
    let mut changes = Changes::default();
    edit.stage(&mut changes);
    changes.stage(cx)?;
    emit_changed(cx, &data, &edit);
    Ok(FrameResult { frame: p.to_frame })
}

// ----- anim.function.curveType.set ---------------------------------------------------------------------------------

/// `anim.function.curveType.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CurveTypeParams {
    /// The node.
    node_id: String,
    /// The channel.
    #[schemars(length(min = 1, max = 256))]
    channel: String,
    /// The curve type.
    curve_type: CurveType,
}

/// `anim.function.curveType.set` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurveTypeResult {
    channel: String,
    curve_type: CurveType,
}

#[command(
    id = "anim.function.curveType.set",
    kind = action,
    label = "Set curve type",
    scopes = [edit],
    undo = "Change Curve Type",
    needs_project,
    example(title = "Make Bob's rotation a spline", params = r#"{"nodeId": "scene:\"Intro\"/element:\"Bob\"", "channel": "rotation", "curveType": "spline"}"#),
    gui = "Function editor: curve type selector (M6 C6.2)",
)]
pub(crate) fn anim_function_curve_type_set(cx: &mut Cx, p: CurveTypeParams) -> Result<CurveTypeResult, CommandError> {
    let doc = cx.document()?.clone();
    let id = node_id(cx, &p.node_id)?;
    let e = node(&doc, &id)?;
    one_d(e, &p.channel)?;
    let has_path = e.channels().as_ref().is_some_and(|m| m.contains_key("position"));
    check_curve(&p.channel, p.curve_type, has_path)?;
    let mut budget = Budget::new("changing a curve type");
    let data = data_of(&doc, &id, &mut budget)?;
    let mut edit = ChannelEdit::read(&doc, &id)?;
    if let Some(c) = edit.channels.get_mut(&p.channel) {
        c["curveType"] = json!(p.curve_type);
        edit.changed.insert(p.channel.clone());
    }
    let mut changes = Changes::default();
    edit.stage(&mut changes);
    changes.stage(cx)?;
    emit_changed(cx, &data, &edit);
    Ok(CurveTypeResult { channel: p.channel, curve_type: p.curve_type })
}

// ----- anim.function.preset.apply ----------------------------------------------------------------------------------

/// Which segments a preset applies to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PresetScope {
    /// The segment at `frame`.
    #[default]
    Segment,
    /// Every segment.
    All,
}

/// `anim.function.preset.apply` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PresetParams {
    /// The node.
    node_id: String,
    /// The channel.
    #[schemars(length(min = 1, max = 256))]
    channel: String,
    /// The preset.
    preset: Easing,
    /// A scene frame in the segment; default: the current frame.
    #[serde(default)]
    frame: Option<i64>,
    /// One segment or the whole curve.
    #[serde(default)]
    scope: PresetScope,
}

/// `{segments}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct PresetResult {
    /// How many segments changed.
    segments: u32,
}

#[command(
    id = "anim.function.preset.apply",
    kind = action,
    label = "Apply easing preset",
    scopes = [edit],
    undo = "Reshape Curve",
    needs_project,
    example(title = "Ease Bob's rotation in and out", params = r#"{"nodeId": "scene:\"Intro\"/element:\"Bob\"", "channel": "rotation", "preset": "easeInOut", "scope": "all"}"#),
    gui = "Function editor: Reshape (M6 C6.2)",
)]
pub(crate) fn anim_function_preset_apply(cx: &mut Cx, p: PresetParams) -> Result<PresetResult, CommandError> {
    let doc = cx.document()?.clone();
    let id = node_id(cx, &p.node_id)?;
    let e = node(&doc, &id)?;
    one_d(e, &p.channel)?;
    let mut budget = Budget::new("applying a preset");
    let data = data_of(&doc, &id, &mut budget)?;
    let is_velocity = p.channel == "velocity";
    if is_velocity && p.preset == Easing::Constant {
        return Err(curve_unsupported("velocity", "constant"));
    }
    let mut edit = ChannelEdit::read(&doc, &id)?;
    let curve: CurveType = edit
        .channels
        .get(&p.channel)
        .and_then(|c| c.get("curveType"))
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(CurveType::Interpolation);
    if curve == CurveType::VelocityBased {
        return Err(curve_unsupported(&p.channel, "an easing preset on a velocityBased curve"));
    }
    let mut keys = edit.keys(&p.channel);
    // The node list of the curve: a velocity curve's pins are nodes too (their shape records are created on demand).
    if is_velocity {
        let path = velocity_path(e, &data)?;
        for (f, _) in pins(path) {
            let local = f as u32;
            if !keys.iter().any(|k| frame_of(k) == Some(local)) {
                let shape = default_shape(cx);
                let k = new_key(cx, local, 0.0, shape)?;
                let at = keys.partition_point(|o| frame_of(o).is_some_and(|x| x < local));
                keys.insert(at, k);
            }
        }
    }
    if keys.len() < 2 {
        return Err(frame_invalid(p.frame.map_or(0.0, |f| f as f64), "this channel has no segment"));
    }
    let n = keys.len();
    let chosen: Vec<usize> = match p.scope {
        PresetScope::All => (0..n - 1).collect(),
        PresetScope::Segment => {
            let scene = super::anim_common::scene_frame(cx, p.frame.map(|f| f as f64))?;
            let local = data.bar.local(scene);
            let (fc, _) = data.bar.cycle_frame(local);
            let i = keys.partition_point(|k| frame_of(k).is_some_and(|f| f64::from(f) <= fc));
            match i.checked_sub(1).filter(|a| *a < n - 1) {
                Some(a) => vec![a],
                None => return Err(frame_invalid(scene, "no segment starts at or before this frame")),
            }
        }
    };
    let pinned = if is_velocity { data.path.as_ref().map(pins).unwrap_or_default() } else { Vec::new() };
    for &a in &chosen {
        budget.spend(1)?;
        let (fa, fb) = (frame_of(&keys[a]).unwrap_or(1), frame_of(&keys[a + 1]).unwrap_or(1));
        edit.touch(fa);
        edit.touch(fb);
        let t = f64::from(fb - fa.min(fb));
        let value_of = |k: &Value, f: u32| -> f64 {
            if pinned.iter().any(|q| q.0 == f64::from(f)) { 0.0 } else { k["value"].as_f64().unwrap_or(0.0) }
        };
        let dv = if is_velocity {
            // Normalised: the end pin of a segment is u = 1.
            let end = if pinned.iter().any(|q| q.0 == f64::from(fb)) { 1.0 } else { value_of(&keys[a + 1], fb) };
            end - value_of(&keys[a], fa)
        } else {
            keys[a + 1]["value"].as_f64().unwrap_or(0.0) - keys[a]["value"].as_f64().unwrap_or(0.0)
        };
        keys[a]["constantSegment"] = json!(p.preset == Easing::Constant);
        match curve {
            CurveType::Spline => {
                if let Some(((x1, y1), (x2, y2))) = preset_points(p.preset) {
                    keys[a]["handles"]["right"] = json!({ "dt": x1 * t, "dv": y1 * dv });
                    keys[a + 1]["handles"]["left"] = json!({ "dt": (x2 - 1.0) * t, "dv": (y2 - 1.0) * dv });
                }
            }
            _ => keys[a]["easing"] = json!(p.preset),
        }
    }
    edit.set_keys(cx, &p.channel, keys)?;
    let mut changes = Changes::default();
    edit.stage(&mut changes);
    changes.stage(cx)?;
    emit_changed(cx, &data, &edit);
    Ok(PresetResult { segments: as_u32(chosen.len()) })
}

/// The cycle frame of a local frame for key defaults (used by [`super::anim_key`]).
pub(crate) fn pose_value(data: &NodeData, channel: &str, f: f64) -> f64 {
    current_value(data, channel, f)
}

/// The motion keyframes' local frames.
pub(crate) fn motion_frames(path: &Path) -> Vec<f64> {
    pins(path).into_iter().map(|q| q.0).collect()
}

/// Rebuilds the path of an edit (kept next to [`pins`] for the key commands).
pub(crate) fn edit_path(edit: &ChannelEdit, data: &NodeData) -> Result<Path, CommandError> {
    let lock = data.statics.as_ref().is_some_and(|s| s.lock_depth);
    path_of(&edit.points(), lock)
}
