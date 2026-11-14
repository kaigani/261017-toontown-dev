//! Static and keyed transforms, loops (05 §1.5, §8.4, §19.3; M3 C3.2): `anim.transform.set`, `anim.loop.set`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_anim::exposure as x;
use tt_anim::timing::cycle_length;
use tt_model::document::EntryKey;
use tt_model::entities::element::{ElementType, LoopType, PivotSet, PositionLoop};
use tt_registry::{CommandError, codes, command};

use super::anim_common::{
    ChannelEdit, MAX_DISTANCE, MAX_VALUE, Xy, Xyz, check_channel, check_value, data_of, emit_changed, emit_static,
    eval_error, evaluator, frame_invalid, key_frame, node, node_id, nodes, out_of_range, scene_frame, transformable,
    unsupported,
};
use super::anim_function::put_key;
use super::anim_path::put_motion_key;
use super::timing::{Budget, Changes, err, op};
use crate::cx::Cx;

/// What `anim.transform.set` writes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Target {
    /// Static (layout) values: all frames, no keys.
    Static,
    /// Animated values, keyed at `frame`.
    #[default]
    Key,
}

/// The space of `position`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PositionSpace {
    /// Parent space.
    #[default]
    Parent,
    /// World space: converted through the parent's world matrix at `frame`.
    World,
}

/// `anim.transform.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct TransformParams {
    /// Nodes; default: the selected rows, else the current element.
    #[serde(default)]
    node_ids: Option<Vec<String>>,
    /// Static values or keys.
    #[serde(default)]
    target: Target,
    /// Scene frame of the keys; default: the current frame.
    #[serde(default)]
    frame: Option<i64>,
    /// Position; omitted axes keep their value.
    #[serde(default)]
    position: Option<Xyz>,
    /// Parent or world space.
    #[serde(default)]
    position_space: PositionSpace,
    /// Rotation, degrees.
    #[serde(default)]
    rotation: Option<f64>,
    /// Scale; omitted axes keep their value.
    #[serde(default)]
    scale: Option<Xy>,
    /// Skew, degrees, −89…89.
    #[serde(default)]
    skew: Option<f64>,
    /// Peg and skeleton pivot (static), or the animated pivot offset of any node (key).
    #[serde(default)]
    pivot: Option<Xy>,
    /// Element rotation pivot (static).
    #[serde(default)]
    rotation_pivot: Option<Xy>,
    /// Element scale pivot (static).
    #[serde(default)]
    scale_pivot: Option<Xy>,
    /// Add to (position, rotation, skew, pivots) or multiply (scale) the current values.
    #[serde(default)]
    relative: bool,
}

/// `anim.transform.set` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TransformResult {
    node_ids: Vec<String>,
    /// The channels keyed (`target: "key"`).
    keyed_channels: Vec<String>,
    warnings: Vec<String>,
}

fn add_or(relative: bool, current: f64, v: f64) -> f64 {
    if relative { current + v } else { v }
}

fn mul_or(relative: bool, current: f64, v: f64) -> f64 {
    if relative { current * v } else { v }
}

#[command(
    id = "anim.transform.set",
    precheck = super::anim_key::precheck_transformable,
    kind = action,
    label = "Set transform",
    scopes = [edit],
    undo = "Transform",
    needs_project,
    example(title = "Offset Bob statically", params = r#"{"nodeIds": ["scene:\"Intro\"/element:\"Bob\""], "target": "static", "position": {"x": 2, "y": -1, "z": 0}}"#),
    example(title = "Key the peg's rotation at frame 12", params = r#"{"nodeIds": ["scene:\"Intro\"/element:\"Bob-P\""], "frame": 12, "rotation": 45}"#),
    example(title = "Place Bob at a world position, keyed", params = r#"{"nodeIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 6, "position": {"x": 1, "y": 1, "z": 0}, "positionSpace": "world"}"#),
    gui = "Select, Transform, Rotate, Scale, Skew tools; Properties panel (M6 C6.1)",
)]
pub(crate) fn anim_transform_set(cx: &mut Cx, p: TransformParams) -> Result<TransformResult, CommandError> {
    let doc = cx.document()?.clone();
    let ids = nodes(cx, p.node_ids.as_deref())?;
    let frame = scene_frame(cx, p.frame.map(|f| f as f64))?;
    if p.relative && p.position_space == PositionSpace::World && p.position.is_some() {
        return Err(err(
            codes::ARG_INVALID,
            "a relative position is in parent space",
            json!({ "pointer": "/positionSpace" }),
        ));
    }
    let mut budget = Budget::new("setting a transform");
    let mut ev = evaluator(&doc, "setting a transform");
    let mut changes = Changes::default();
    let mut keyed: Vec<String> = Vec::new();
    let mut edits = Vec::new();
    let mut statics = Vec::new();
    for id in &ids {
        let e = transformable(&doc, id)?;
        if *e.element_type() == ElementType::Bone {
            return Err(err(
                "anim.notTransformable",
                "bones are posed with anim.bone.* (M6 C6.3)",
                json!({ "elementId": id.as_str() }),
            ));
        }
        let camera = *e.element_type() == ElementType::Camera;
        let set = PivotSet::of(*e.element_type());
        if camera && (p.scale.is_some() || p.skew.is_some()) {
            let c = if p.scale.is_some() { "scale" } else { "skew" };
            return Err(unsupported(c, e, "a camera is rigid (05 §2.6)"));
        }
        if camera && (p.pivot.is_some() || p.rotation_pivot.is_some() || p.scale_pivot.is_some()) {
            return Err(unsupported("pivot", e, "a camera has no pivots (05 §2.3)"));
        }
        let element_pivots = set == Some(PivotSet::Element);
        if p.target == Target::Key && (p.rotation_pivot.is_some() || p.scale_pivot.is_some()) {
            return Err(unsupported("rotationPivot", e, "these pivots are static; key `pivot` (05 §1.5)"));
        }
        if !element_pivots && (p.rotation_pivot.is_some() || p.scale_pivot.is_some()) {
            return Err(unsupported("rotationPivot", e, "this node kind has one `pivot` (05 §2.3)"));
        }
        if p.target == Target::Static && element_pivots && p.pivot.is_some() {
            return Err(unsupported("pivot", e, "elements have rotationPivot and scalePivot (05 §2.3)"));
        }
        let data = data_of(&doc, id, &mut budget)?;
        let l = ev.local(id, frame).map_err(eval_error)?;
        // The requested position, in the terms of the target layer.
        let mut position = p.position.map_or([None; 3], |q| q.parts());
        if let Some(q) = &p.position {
            q.check()?;
        }
        let other = match p.target {
            Target::Static => l.animated.position,
            Target::Key => l.statics.position,
        };
        if p.position_space == PositionSpace::World && p.position.is_some() {
            let inverse = ev.parent_world(id, frame).map_err(eval_error)?.inverse().ok_or_else(|| {
                err("anim.valueOutOfRange", "the parent's world matrix is singular", json!({ "nodeId": id.as_str() }))
            })?;
            let world = p.position.map_or([None; 3], |q| q.parts());
            let here = ev.world(id, frame).map_err(eval_error)?.translation();
            let want = [world[0].unwrap_or(here[0]), world[1].unwrap_or(here[1]), world[2].unwrap_or(here[2])];
            let parent = inverse.apply(want);
            for i in 0..3 {
                let v = parent[i] - other[i];
                check_value("x", v)?;
                position[i] = Some(v);
            }
        }
        match p.target {
            Target::Static => {
                let mut t = doc
                    .entry_raw_json(&EntryKey::Element(id.clone()))
                    .map_err(super::timing::internal)?
                    .get("transform")
                    .cloned()
                    .unwrap_or(Value::Null);
                for (i, axis) in ["x", "y", "z"].iter().enumerate() {
                    if let Some(v) = position[i] {
                        let current = l.statics.position[i];
                        let next = add_or(p.relative, current, v);
                        if next.abs() > MAX_DISTANCE {
                            return Err(out_of_range("position", next, -MAX_DISTANCE, MAX_DISTANCE));
                        }
                        t["offset"][axis] = json!(next);
                    }
                }
                if let Some(v) = p.rotation {
                    let next = add_or(p.relative, l.statics.rotation, v);
                    if next.abs() > MAX_VALUE {
                        return Err(out_of_range("rotation", next, -MAX_VALUE, MAX_VALUE));
                    }
                    t["rotation"] = json!(next);
                }
                if let Some(v) = p.skew {
                    let next = add_or(p.relative, l.statics.skew, v);
                    check_value("skew", next)?;
                    t["skew"] = json!(next);
                }
                if let Some(s) = p.scale {
                    for (i, (axis, v)) in [("x", s.x), ("y", s.y)].iter().enumerate() {
                        if let Some(v) = v {
                            let next = mul_or(p.relative, l.statics.scale[i], *v);
                            check_value("scale.x", next)?;
                            t["scale"][axis] = json!(next);
                        }
                    }
                }
                let mut pivot = |name: &str, q: Option<Xy>, current: [f64; 2]| -> Result<(), CommandError> {
                    if let Some(q) = q {
                        for (i, (axis, v)) in [("x", q.x), ("y", q.y)].iter().enumerate() {
                            if let Some(v) = v {
                                let next = add_or(p.relative, current[i], *v);
                                check_value("pivot.x", next)?;
                                t[name][axis] = json!(next);
                            }
                        }
                    }
                    Ok(())
                };
                pivot("pivot", p.pivot, l.statics.pivot)?;
                pivot("rotationPivot", p.rotation_pivot, l.statics.rotation_pivot)?;
                pivot("scalePivot", p.scale_pivot, l.statics.scale_pivot)?;
                changes.push(EntryKey::Element(id.clone()), vec![op("replace", "/transform", Some(t))]);
                statics.push(id.clone());
            }
            Target::Key => {
                let local = key_frame(&data, frame)?;
                let mut edit = ChannelEdit::read(&doc, id)?;
                if position.iter().any(Option::is_some) {
                    check_channel(e, "position")?;
                    let animated = l.animated.position;
                    let mut pos = [None; 3];
                    for i in 0..3 {
                        pos[i] = position[i].map(|v| add_or(p.relative, animated[i], v));
                    }
                    for v in pos.into_iter().flatten() {
                        check_value("x", v)?;
                    }
                    put_motion_key(cx, &mut edit, &data, local, pos)?;
                    if !keyed.iter().any(|c| c == "position") {
                        keyed.push("position".to_owned());
                    }
                }
                let mut one =
                    |cx: &mut Cx, edit: &mut ChannelEdit, channel: &str, value: f64| -> Result<(), CommandError> {
                        check_value(channel, value)?;
                        put_key(cx, edit, e, channel, local, value)?;
                        if !keyed.iter().any(|c| c == channel) {
                            keyed.push(channel.to_owned());
                        }
                        Ok(())
                    };
                if let Some(v) = p.rotation {
                    one(cx, &mut edit, "rotation", add_or(p.relative, l.animated.rotation, v))?;
                }
                if let Some(v) = p.skew {
                    one(cx, &mut edit, "skew", add_or(p.relative, l.animated.skew, v))?;
                }
                if let Some(s) = p.scale {
                    if let Some(v) = s.x {
                        one(cx, &mut edit, "scale.x", mul_or(p.relative, l.animated.scale[0], v))?;
                    }
                    if let Some(v) = s.y {
                        one(cx, &mut edit, "scale.y", mul_or(p.relative, l.animated.scale[1], v))?;
                    }
                }
                if let Some(q) = p.pivot {
                    if let Some(v) = q.x {
                        one(cx, &mut edit, "pivot.x", add_or(p.relative, l.animated.pivot[0], v))?;
                    }
                    if let Some(v) = q.y {
                        one(cx, &mut edit, "pivot.y", add_or(p.relative, l.animated.pivot[1], v))?;
                    }
                }
                edit.stage(&mut changes);
                edits.push((data, edit));
            }
        }
    }
    changes.stage(cx)?;
    for (data, edit) in &edits {
        emit_changed(cx, data, edit);
    }
    for id in &statics {
        emit_static(cx, id, &["transform"]);
    }
    Ok(TransformResult {
        node_ids: ids.iter().map(|i| i.as_str().to_owned()).collect(),
        keyed_channels: keyed,
        warnings: Vec::new(),
    })
}

// ----- anim.loop.set -----------------------------------------------------------------------------------------------

fn one() -> u32 {
    1
}

/// `anim.loop.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct LoopParams {
    /// The node.
    node_id: String,
    /// `count`: the number of cycles. `fill`: ignored, the cycles follow from the duration.
    #[serde(default = "one")]
    #[schemars(range(min = 1, max = 10_000))]
    loops: u32,
    /// `count` or `fill`.
    #[serde(default = "count")]
    loop_type: LoopType,
    /// `cycle` or `offset`.
    #[serde(default = "cycle")]
    position_loop: PositionLoop,
}

fn count() -> LoopType {
    LoopType::Count
}

fn cycle() -> PositionLoop {
    PositionLoop::Cycle
}

/// `anim.loop.set` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoopResult {
    node_id: String,
    /// L, in frames.
    cycle_length: u32,
    /// The bar after the change.
    duration: u32,
}

#[command(
    id = "anim.loop.set",
    kind = action,
    label = "Set loop",
    scopes = [edit],
    undo = "Change Loop",
    needs_project,
    example(title = "Loop Bob-P twice, advancing", params = r#"{"nodeId": "scene:\"Intro\"/element:\"Bob-P\"", "loops": 2, "positionLoop": "offset"}"#),
    gui = "Timeline: loop controls (M5); Properties panel (M6 C6.1)",
)]
pub(crate) fn anim_loop_set(cx: &mut Cx, p: LoopParams) -> Result<LoopResult, CommandError> {
    let doc = cx.document()?.clone();
    let id = node_id(cx, &p.node_id)?;
    let e = node(&doc, &id)?;
    if !super::element::loopable(*e.element_type()) {
        return Err(err(
            "element.notLoopable",
            format!("{} elements cannot loop", e.element_type().as_str()),
            json!({ "elementId": id.as_str(), "elementType": e.element_type().as_str() }),
        ));
    }
    if !(1..=10_000).contains(&p.loops) {
        return Err(out_of_range("loops", f64::from(p.loops), 1.0, 10_000.0));
    }
    let (Some(start), Some(duration)) = (*e.start(), *e.duration()) else {
        return Err(frame_invalid(0.0, "this node has no bar"));
    };
    let l = cycle_length(e, duration);
    let (next_duration, loops, stored_cycle) = match p.loop_type {
        LoopType::Count => {
            let d = l.checked_mul(p.loops).filter(|d| u64::from(*d) + u64::from(start) - 1 <= x::MAX_FRAMES as u64);
            let Some(d) = d else {
                let max = (x::MAX_FRAMES as u64 + 1 - u64::from(start)) / u64::from(l);
                return Err(out_of_range("loops", f64::from(p.loops), 1.0, max as f64));
            };
            (d, p.loops, None)
        }
        LoopType::Fill => (duration, duration.div_ceil(l).max(1), Some(l)),
    };
    let mut ops = vec![
        op("replace", "/loops", Some(json!(loops))),
        op("replace", "/loopType", Some(json!(p.loop_type))),
        op("replace", "/positionLoop", Some(json!(p.position_loop))),
        op("replace", "/duration", Some(json!(next_duration))),
    ];
    match stored_cycle {
        Some(c) => ops.push(op("add", "/cycleLength", Some(json!(c)))),
        None if e.cycle_length().is_some() => ops.push(op("remove", "/cycleLength", None)),
        None => {}
    }
    let mut changes = Changes::default();
    changes.push(EntryKey::Element(id.clone()), ops);
    changes.stage(cx)?;
    emit_static(cx, &id, &["loop"]);
    Ok(LoopResult { node_id: id.as_str().to_owned(), cycle_length: l, duration: next_duration })
}
