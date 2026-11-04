//! Shared parts of the `anim.*` commands (05 §19; M3 C3.2; ADR-0019): node and frame resolution, numeric bounds, the
//! staged edit of an element's `channels`, and evaluator set-up. Command frames are scene frames (05 §19.1.4); keys are
//! stored in local frames (05 §1.6).

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tt_anim::eval::{EvalError, Evaluator};
use tt_anim::exposure as x;
use tt_anim::node::NodeData;
use tt_model::document::{Document, EntryKey};
use tt_model::entities::channel::{CurveType, Easing, Key};
use tt_model::entities::element::{Element, ElementType};
use tt_model::{Id, ObjectType};
use tt_registry::CommandError;

use super::timing::{Budget, Changes, err, internal, op, to_json};
use crate::cx::Cx;

/// Largest position or pivot magnitude, in fields (SI-101).
pub(crate) const MAX_DISTANCE: f64 = 1e6;
/// Largest scale magnitude (SI-101).
pub(crate) const MAX_SCALE: f64 = 1e6;
/// Smallest scale magnitude (05 §2.1).
pub(crate) const MIN_SCALE: f64 = 0.001;
/// Largest rotation magnitude and largest generic 1-D value (SI-101).
pub(crate) const MAX_VALUE: f64 = 1e9;

/// `anim.notFound`.
pub(crate) fn not_found(what: &str, reference: &str) -> CommandError {
    err("anim.notFound", format!("no {what} `{reference}`"), json!({ "ref": reference, "what": what }))
}

/// `anim.valueOutOfRange`, with the limits.
pub(crate) fn out_of_range(what: &str, value: f64, minimum: f64, maximum: f64) -> CommandError {
    err(
        "anim.valueOutOfRange",
        format!("{what} {value} is outside {minimum}…{maximum}"),
        json!({ "what": what, "value": value, "minimum": minimum, "maximum": maximum }),
    )
}

/// `anim.frameInvalid`.
pub(crate) fn frame_invalid(frame: f64, why: &str) -> CommandError {
    err("anim.frameInvalid", format!("frame {frame}: {why}"), json!({ "frame": frame, "reason": why }))
}

/// `anim.channelUnsupported`.
pub(crate) fn unsupported(channel: &str, e: &Element, why: &str) -> CommandError {
    err(
        "anim.channelUnsupported",
        format!("`{channel}` on {} `{}`: {why}", e.element_type().as_str(), e.name()),
        json!({ "channel": channel, "elementId": e.id().as_str(), "elementType": e.element_type().as_str() }),
    )
}

/// A node reference; an unknown one is `anim.notFound`.
pub(crate) fn node_id(cx: &Cx, r: &str) -> Result<Id, CommandError> {
    let key = cx.resolve_as(r, ObjectType::Element).map_err(|_| not_found("node", r))?;
    Id::parse(&key).map_err(|_| not_found("node", r))
}

/// The nodes of a `nodeIds?` parameter: given, else the selected rows, else the current element (05 §19.1.3).
pub(crate) fn nodes(cx: &Cx, given: Option<&[String]>) -> Result<Vec<Id>, CommandError> {
    let Some(list) = given else { return super::element::targets(cx, None) };
    let mut out = Vec::new();
    for r in list {
        let id = node_id(cx, r)?;
        if !out.contains(&id) {
            out.push(id);
        }
    }
    Ok(out)
}

/// The element of a node ID.
pub(crate) fn node<'a>(doc: &'a Document, id: &Id) -> Result<&'a Element, CommandError> {
    doc.element(id).ok_or_else(|| not_found("node", id.as_str()))
}

/// Refuses nodes without a transform block (05 §18 `anim.notTransformable`).
pub(crate) fn transformable<'a>(doc: &'a Document, id: &Id) -> Result<&'a Element, CommandError> {
    let e = node(doc, id)?;
    if e.transform().is_none() {
        return Err(err(
            "anim.notTransformable",
            format!("{} element `{}` has no transformation", e.element_type().as_str(), e.name()),
            json!({ "elementId": id.as_str(), "elementType": e.element_type().as_str() }),
        ));
    }
    Ok(e)
}

/// A scene frame parameter, or the current frame: 1…100 000.
pub(crate) fn scene_frame(cx: &Cx, given: Option<f64>) -> Result<f64, CommandError> {
    let f = given.unwrap_or_else(|| f64::from(cx.app().frame));
    if !(1.0..=x::MAX_FRAMES as f64).contains(&f) {
        return Err(frame_invalid(f, "outside 1…100000"));
    }
    Ok(f)
}

/// The local frame a key at scene frame `frame` is stored at: an integer inside the bar's first cycle
/// (`anim.frameNotInteger`, `anim.frameInvalid`; 05 §19.1.4).
pub(crate) fn key_frame(data: &NodeData, frame: f64) -> Result<u32, CommandError> {
    if frame != frame.floor() {
        return Err(err(
            "anim.frameNotInteger",
            format!("keyframe frame {frame} has a fractional part"),
            json!({ "frame": frame }),
        ));
    }
    let local = frame - data.bar.start + 1.0;
    if !data.bar.keyable(local) {
        return Err(frame_invalid(frame, "outside the node's range or inside a repeated loop cycle"));
    }
    Ok(local as u32)
}

/// Checks a value of a channel against its domain (05 §2.1, §8, §10; SI-101).
pub(crate) fn check_value(channel: &str, v: f64) -> Result<(), CommandError> {
    let bound = |lo: f64, hi: f64| {
        if v < lo || v > hi { Err(out_of_range(channel, v, lo, hi)) } else { Ok(()) }
    };
    match channel {
        "skew" => bound(-89.0, 89.0),
        "fov" => bound(1.0, 179.0),
        "scale.x" | "scale.y" => {
            if v.abs() < MIN_SCALE || v.abs() > MAX_SCALE {
                Err(out_of_range(channel, v.abs(), MIN_SCALE, MAX_SCALE))
            } else {
                Ok(())
            }
        }
        "pivot.x" | "pivot.y" | "position.x" | "position.y" | "position.z" | "x" | "y" | "z" => {
            bound(-MAX_DISTANCE, MAX_DISTANCE)
        }
        _ => bound(-MAX_VALUE, MAX_VALUE),
    }
}

/// Maps a stop of the evaluator to a command error.
pub(crate) fn eval_error(e: EvalError<CommandError>) -> CommandError {
    match e {
        EvalError::Meter(c) => c,
        EvalError::NotFound(id) => not_found("node", id.as_str()),
        EvalError::NonFinite(id) => err(
            "anim.valueOutOfRange",
            format!("the transform of `{id}` left the finite range"),
            json!({ "nodeId": id.as_str(), "reason": "nonFinite" }),
        ),
    }
}

/// An evaluator charged to a work account named `what`.
pub(crate) fn evaluator<'d>(doc: &'d Document, what: &'static str) -> Evaluator<'d, CommandError> {
    let mut budget = Budget::new(what);
    Evaluator::new(doc, move |n| budget.spend(usize::try_from(n).unwrap_or(usize::MAX)))
}

/// A node's data, read outside an evaluator.
pub(crate) fn data_of(doc: &Document, id: &Id, budget: &mut Budget) -> Result<NodeData, CommandError> {
    let e = node(doc, id)?;
    let scene = super::timing::scene_of(doc, id)?;
    budget.spend(usize::try_from(NodeData::cost(e)).unwrap_or(usize::MAX))?;
    Ok(NodeData::read(doc, scene.tree(), e, doc.scene_length(scene)))
}

/// The curve type and easing new keys get (05 §10.2, *create linear spline*: `staging.keyframes.linearVelocity`).
pub(crate) fn default_shape(cx: &Cx) -> Easing {
    let linear = cx.session().env.pref("staging.keyframes.linearVelocity").and_then(|v| v.as_bool()).unwrap_or(true);
    if linear { Easing::Linear } else { Easing::EaseInOut }
}

/// Whether new keyframes start constant segments (05 §10.2).
pub(crate) fn constant_default(cx: &Cx) -> bool {
    cx.session().env.pref("staging.keyframes.constantByDefault").and_then(|v| v.as_bool()).unwrap_or(false)
}

/// The TCB values of new motion points (05 §16).
pub(crate) fn tcb_defaults(cx: &Cx) -> (f64, f64, f64) {
    let at = |k: &str| cx.session().env.pref(k).and_then(|v| v.as_f64()).unwrap_or(0.0);
    (
        at("staging.motionPoint.defaultTension"),
        at("staging.motionPoint.defaultContinuity"),
        at("staging.motionPoint.defaultBias"),
    )
}

/// JSON of a new key.
pub(crate) fn new_key(cx: &mut Cx, frame: u32, value: f64, easing: Easing) -> Result<Value, CommandError> {
    let id = cx.new_id(ObjectType::Key)?;
    let mut v = to_json(&Key::new(id, frame, value))?;
    v["easing"] = json!(easing);
    v["constantSegment"] = json!(constant_default(cx));
    Ok(v)
}

/// The frame of a key or motion point JSON.
pub(crate) fn frame_of(v: &Value) -> Option<u32> {
    v.get("frame").and_then(Value::as_u64).and_then(|f| u32::try_from(f).ok())
}

/// The staged edit of one element's `channels` map: channels are read, changed as JSON (keeping channel, key and point
/// IDs) and written back one channel at a time; a channel left with no keys is removed (10 §4.12).
pub(crate) struct ChannelEdit {
    pub id: Id,
    pub channels: Map<String, Value>,
    pub changed: BTreeSet<String>,
    /// The local frames touched.
    pub from: Option<u32>,
    pub to: Option<u32>,
}

impl ChannelEdit {
    /// Reads the channels of `id`.
    pub(crate) fn read(doc: &Document, id: &Id) -> Result<Self, CommandError> {
        let v = doc.entry_raw_json(&EntryKey::Element(id.clone())).map_err(internal)?;
        let channels = v.get("channels").and_then(Value::as_object).cloned().unwrap_or_default();
        Ok(Self { id: id.clone(), channels, changed: BTreeSet::new(), from: None, to: None })
    }

    /// Records a touched local frame.
    pub(crate) fn touch(&mut self, frame: u32) {
        self.from = Some(self.from.map_or(frame, |f| f.min(frame)));
        self.to = Some(self.to.map_or(frame, |f| f.max(frame)));
    }

    /// The keys of a 1-D channel (empty when it does not exist).
    pub(crate) fn keys(&self, name: &str) -> Vec<Value> {
        self.channels.get(name).and_then(|c| c.get("keys")).and_then(Value::as_array).cloned().unwrap_or_default()
    }

    /// The points of the motion path (empty when there is none).
    pub(crate) fn points(&self) -> Vec<Value> {
        self.channels
            .get("position")
            .and_then(|c| c.get("points"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    }

    /// Writes the keys of a 1-D channel, creating or removing it as needed.
    pub(crate) fn set_keys(&mut self, cx: &mut Cx, name: &str, keys: Vec<Value>) -> Result<(), CommandError> {
        self.changed.insert(name.to_owned());
        if keys.is_empty() {
            self.channels.remove(name);
            return Ok(());
        }
        match self.channels.get_mut(name) {
            Some(c) => c["keys"] = Value::Array(keys),
            None => {
                let cid = cx.new_id(ObjectType::Channel)?;
                self.channels.insert(
                    name.to_owned(),
                    json!({ "id": cid.as_str(), "type": "channel", "rev": 0, "name": name,
                            "curveType": CurveType::Interpolation, "keys": keys }),
                );
            }
        }
        Ok(())
    }

    /// Writes the motion path, creating or removing the channel as needed.
    pub(crate) fn set_points(&mut self, cx: &mut Cx, points: Vec<Value>) -> Result<(), CommandError> {
        self.changed.insert("position".to_owned());
        if points.is_empty() {
            self.channels.remove("position");
            return Ok(());
        }
        match self.channels.get_mut("position") {
            Some(c) => c["points"] = Value::Array(points),
            None => {
                let cid = cx.new_id(ObjectType::Channel)?;
                self.channels.insert(
                    "position".to_owned(),
                    json!({ "id": cid.as_str(), "type": "channel", "rev": 0, "name": "position", "points": points }),
                );
            }
        }
        Ok(())
    }

    /// Adds the changed channels to `changes`.
    pub(crate) fn stage(&self, changes: &mut Changes) {
        let mut ops = Vec::new();
        for name in &self.changed {
            let path = format!("/channels/{}", tt_model::pointer::escape(name));
            match self.channels.get(name) {
                Some(c) => ops.push(op("add", &path, Some(c.clone()))),
                None => ops.push(op("remove", &path, None)),
            }
        }
        if !ops.is_empty() {
            changes.push(EntryKey::Element(self.id.clone()), ops);
        }
    }
}

/// The original element JSON has its channel removed only if it existed: an absent channel stays absent.
pub(crate) fn drop_absent_removals(doc: &Document, edit: &mut ChannelEdit) -> Result<(), CommandError> {
    let before = ChannelEdit::read(doc, &edit.id)?;
    let gone: Vec<String> = edit
        .changed
        .iter()
        .filter(|n| !edit.channels.contains_key(*n) && !before.channels.contains_key(*n))
        .cloned()
        .collect();
    for n in gone {
        edit.changed.remove(&n);
    }
    Ok(())
}

/// `anim.changed` (05 §19.4): coalesced into one event per undo step when the call is accepted.
pub(crate) fn emit_changed(cx: &mut Cx, data: &NodeData, edit: &ChannelEdit) {
    if edit.changed.is_empty() {
        return;
    }
    let frames = match (edit.from, edit.to) {
        (Some(a), Some(b)) => json!({ "from": data.bar.scene(f64::from(a)), "to": data.bar.scene(f64::from(b)) }),
        _ => Value::Null,
    };
    let mut payload = json!({ "nodeIds": [edit.id.as_str()], "channels": edit.changed.iter().collect::<Vec<_>>() });
    if !frames.is_null() {
        payload["frames"] = frames;
    }
    cx.emit("anim.changed", payload);
}

/// `anim.changed` for static or structural changes.
pub(crate) fn emit_static(cx: &mut Cx, id: &Id, channels: &[&str]) {
    cx.emit("anim.changed", json!({ "nodeIds": [id.as_str()], "channels": channels }));
}

/// The channel names a `channels` parameter names: `scale` and `pivot` stand for their two axes (05 §19.3).
pub(crate) fn channel_names(given: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for c in given {
        match c.as_str() {
            "scale" => out.extend(["scale.x".to_owned(), "scale.y".to_owned()]),
            "pivot" => out.extend(["pivot.x".to_owned(), "pivot.y".to_owned()]),
            other => out.push(other.to_owned()),
        }
    }
    out.dedup();
    out
}

/// Checks a channel id against the node kind (05 §1.6); `position.x/y/z` count as `position`.
pub(crate) fn check_channel(e: &Element, channel: &str) -> Result<(), CommandError> {
    let stored = match channel {
        "position.x" | "position.y" | "position.z" => "position",
        c => c,
    };
    if !e.element_type().allowed_channels().contains(&stored) {
        return Err(unsupported(channel, e, "not a channel of this node kind"));
    }
    Ok(())
}

/// The world position of `id` as evaluated is unavailable for bones in C3.2 (M6 C6.3).
pub(crate) fn evaluable(doc: &Document, id: &Id) -> Result<(), CommandError> {
    let e = node(doc, id)?;
    if matches!(e.element_type(), ElementType::Sound | ElementType::Bone) {
        let mut error = err(
            "anim.notTransformable",
            format!("{} element `{}` is not evaluated", e.element_type().as_str(), e.name()),
            json!({ "elementId": id.as_str(), "elementType": e.element_type().as_str() }),
        );
        if *e.element_type() == ElementType::Bone {
            error = error.hint("bone kinematics are M6 C6.3");
        }
        return Err(error);
    }
    Ok(())
}

/// A partial 3-D point parameter.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Xyz {
    /// X.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    /// Y.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    /// Z.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z: Option<f64>,
}

impl Xyz {
    /// The components as `[Option; 3]`.
    pub(crate) fn parts(&self) -> [Option<f64>; 3] {
        [self.x, self.y, self.z]
    }

    /// Checks every component.
    pub(crate) fn check(&self) -> Result<(), CommandError> {
        for (n, v) in ["x", "y", "z"].iter().zip(self.parts()) {
            if let Some(v) = v {
                check_value(n, v)?;
            }
        }
        Ok(())
    }
}

/// A partial 2-D point parameter.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Xy {
    /// X.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    /// Y.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
}

/// A full 3-D result point.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
pub(crate) struct P3 {
    /// X.
    pub x: f64,
    /// Y.
    pub y: f64,
    /// Z.
    pub z: f64,
}

impl From<[f64; 3]> for P3 {
    fn from(p: [f64; 3]) -> Self {
        Self { x: tt_anim::num::tidy(p[0]), y: tt_anim::num::tidy(p[1]), z: tt_anim::num::tidy(p[2]) }
    }
}

/// A full 2-D result point.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
pub(crate) struct P2 {
    /// X.
    pub x: f64,
    /// Y.
    pub y: f64,
}

impl From<[f64; 2]> for P2 {
    fn from(p: [f64; 2]) -> Self {
        Self { x: tt_anim::num::tidy(p[0]), y: tt_anim::num::tidy(p[1]) }
    }
}

/// The current scene frame span of keys by name, for readers.
pub(crate) type KeysByChannel = BTreeMap<String, Vec<Value>>;
