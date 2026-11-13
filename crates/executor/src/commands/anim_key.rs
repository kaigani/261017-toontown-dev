//! Keyframes and segments across channels (05 §10.1, §19.3; M3 C3.2): `anim.key.add`, `.list`, `.move`, `.remove`,
//! `anim.segment.set`, `anim.segment.uniform.apply`.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_anim::hierarchy as h;
use tt_anim::num::tidy;
use tt_model::Id;
use tt_model::entities::channel::Easing;
use tt_model::entities::element::{Element, ElementType, FovMode};
use tt_registry::{CommandError, command};

use super::anim_common::{
    ChannelEdit, channel_names, check_channel, data_of, default_shape, emit_changed, frame_invalid, frame_of,
    key_frame, new_key, node, node_id, nodes, scene_frame, transformable, unsupported,
};
use super::anim_function::{edit_path, motion_frames, pose_value, put_key};
use super::anim_path::{key_exists, put_motion_key, remove_point, retime_velocity};
use super::timing::{Budget, Changes, as_u32, scene_of};
use crate::cx::Cx;

/// The channels of a full pose for a node kind (05 §10.1); the velocity pin comes with the motion key.
fn pose_channels(e: &Element) -> Vec<&'static str> {
    if *e.element_type() == ElementType::Camera {
        let dynamic = e.camera().as_ref().is_some_and(|c| c.fov_mode == FovMode::Dynamic);
        let mut v = vec!["position", "rotation"];
        if dynamic {
            v.push("fov");
        }
        return v;
    }
    vec!["position", "rotation", "scale.x", "scale.y", "skew", "pivot.x", "pivot.y"]
}

/// Every local frame a node has a key at, across channels.
fn keyed_frames(edit: &ChannelEdit) -> Vec<u32> {
    let mut out: Vec<u32> = edit.points().iter().filter_map(frame_of).collect();
    for name in edit.channels.keys() {
        out.extend(edit.keys(name).iter().filter_map(frame_of));
    }
    out.sort_unstable();
    out.dedup();
    out
}

// ----- anim.key.add -------------------------------------------------------------------------------------------------

/// Which bounds of the selected segment get keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Bounds {
    /// The key at or before the frame.
    Left,
    /// The key after the frame.
    Right,
    /// Both.
    Both,
}

/// `anim.key.add` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct KeyAddParams {
    /// Nodes; default: the selected rows, else the current element.
    #[serde(default)]
    node_ids: Option<Vec<String>>,
    /// Scene frame; default: the current frame.
    #[serde(default)]
    frame: Option<i64>,
    /// Channels; default: the full pose. `scale` and `pivot` name both axes.
    #[serde(default)]
    channels: Option<Vec<String>>,
    /// Also key every transformable descendant (a collapsed hierarchy) whose range holds the frame.
    #[serde(default)]
    include_descendants: bool,
    /// Key the bounding frames of the segment around `frame` instead.
    #[serde(default)]
    at_segment_bounds: Option<Bounds>,
}

/// Keys added on one node at one frame.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AddedKeys {
    node_id: String,
    /// Scene frame.
    frame: f64,
    channels: Vec<String>,
}

/// `anim.key.add` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct KeyAddResult {
    keys: Vec<AddedKeys>,
}

#[command(
    id = "anim.key.add",
    kind = action,
    label = "Add keyframe",
    scopes = [edit],
    undo = "Add Keyframe",
    needs_project,
    example(title = "Block Bob's full pose at frame 6", params = r#"{"nodeIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 6}"#),
    example(title = "Key only rotation and scale", params = r#"{"nodeIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 8, "channels": ["rotation", "scale"]}"#),
    gui = "Animation menu, Timeline: Add Keyframe (M6 C6.2)",
)]
pub(crate) fn anim_key_add(cx: &mut Cx, p: KeyAddParams) -> Result<KeyAddResult, CommandError> {
    let doc = cx.document()?.clone();
    let ids = nodes(cx, p.node_ids.as_deref())?;
    let scene = scene_frame(cx, p.frame.map(|f| f as f64))?;
    let mut budget = Budget::new("adding keyframes");
    let mut all: Vec<(Id, bool)> = Vec::new();
    for id in &ids {
        transformable(&doc, id)?;
        if !all.iter().any(|(x, _)| x == id) {
            all.push((id.clone(), true));
        }
        if p.include_descendants {
            let sc = scene_of(&doc, id)?;
            budget.spend(h::count(sc.tree()))?;
            for d in h::descendants(sc.tree(), id) {
                if node(&doc, &d)?.transform().is_some() && !all.iter().any(|(x, _)| *x == d) {
                    all.push((d, false));
                }
            }
        }
    }
    let wanted = p.channels.as_deref().map(channel_names);
    let mut changes = Changes::default();
    let mut out = Vec::new();
    let mut edits = Vec::new();
    for (id, explicit) in &all {
        let e = node(&doc, id)?;
        let data = data_of(&doc, id, &mut budget)?;
        let mut edit = ChannelEdit::read(&doc, id)?;
        let list: Vec<String> = match &wanted {
            Some(w) => {
                let usable: Vec<String> = if *explicit {
                    w.clone()
                } else {
                    w.iter().filter(|c| check_channel(e, c).is_ok()).cloned().collect()
                };
                usable
            }
            None => pose_channels(e).iter().map(|c| (*c).to_owned()).collect(),
        };
        let mut frames = vec![scene];
        if let Some(b) = p.at_segment_bounds {
            let local = data.bar.local(scene);
            let (fc, _) = data.bar.cycle_frame(local);
            let keyed = keyed_frames(&edit);
            let i = keyed.partition_point(|f| f64::from(*f) <= fc);
            let left = i.checked_sub(1).and_then(|a| keyed.get(a)).copied();
            let right = keyed.get(i).copied();
            frames = match b {
                Bounds::Left => left.into_iter().collect::<Vec<u32>>(),
                Bounds::Right => right.into_iter().collect(),
                Bounds::Both => left.into_iter().chain(right).collect(),
            }
            .into_iter()
            .map(|f| data.bar.scene(f64::from(f)))
            .collect();
        }
        for scene in frames {
            let local = match key_frame(&data, scene) {
                Ok(l) => l,
                Err(e) if *explicit => return Err(e),
                Err(_) => continue,
            };
            let f = f64::from(local);
            let mut keyed = Vec::new();
            for c in &list {
                check_channel(e, c)?;
                budget.spend(1)?;
                match c.as_str() {
                    "position" => {
                        if !edit.points().iter().any(|q| frame_of(q) == Some(local)) {
                            put_motion_key(cx, &mut edit, &data, local, [None; 3])?;
                            keyed.push(c.clone());
                        }
                    }
                    "velocity" => {
                        let path = edit_path(&edit, &data)?;
                        if !motion_frames(&path).contains(&f)
                            && !edit.keys("velocity").iter().any(|k| frame_of(k) == Some(local))
                            && path.key_indices().len() >= 2
                        {
                            let j = tt_anim::tcb::key_segment(&path, f);
                            let (s0, s1) = (path.key_arc(j), path.key_arc(j + 1));
                            let v = data.velocity_at(f);
                            let u = if s1 > s0 { (v - s0) / (s1 - s0) } else { 0.0 };
                            put_key(cx, &mut edit, e, "velocity", local, u)?;
                            keyed.push(c.clone());
                        }
                    }
                    channel => {
                        if !edit.keys(channel).iter().any(|k| frame_of(k) == Some(local)) {
                            let v = pose_value(&data, channel, f);
                            put_key(cx, &mut edit, e, channel, local, v)?;
                            keyed.push(c.clone());
                        }
                    }
                }
            }
            if !keyed.is_empty() {
                out.push(AddedKeys { node_id: id.as_str().to_owned(), frame: tidy(scene), channels: keyed });
            }
        }
        edit.stage(&mut changes);
        edits.push((data, edit));
    }
    changes.stage(cx)?;
    for (data, edit) in &edits {
        emit_changed(cx, data, edit);
    }
    Ok(KeyAddResult { keys: out })
}

// ----- anim.key.remove ----------------------------------------------------------------------------------------------

/// `anim.key.remove` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct KeyRemoveParams {
    /// Nodes; default: the selected rows, else the current element.
    #[serde(default)]
    node_ids: Option<Vec<String>>,
    /// Scene frame; default: the current frame.
    #[serde(default)]
    frame: Option<i64>,
    /// Channels; default: position, rotation, scale, skew (velocity and pivot keys are kept, 05 §10.1).
    #[serde(default)]
    channels: Option<Vec<String>>,
    /// Remove every key of every channel.
    #[serde(default)]
    all: bool,
}

/// `{removed}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct KeyRemoveResult {
    /// Keys removed.
    removed: u32,
}

#[command(
    id = "anim.key.remove",
    kind = action,
    label = "Remove keyframe",
    scopes = [edit],
    undo = "Remove Keyframe",
    needs_project,
    example(title = "Remove Bob's keys at frame 1", params = r#"{"nodeIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 1}"#),
    example(title = "Remove all keyframes of Bob", params = r#"{"nodeIds": ["scene:\"Intro\"/element:\"Bob\""], "all": true}"#),
    gui = "Animation menu, Timeline: Remove Keyframe, Remove All Keyframes (M6 C6.2)",
)]
pub(crate) fn anim_key_remove(cx: &mut Cx, p: KeyRemoveParams) -> Result<KeyRemoveResult, CommandError> {
    let doc = cx.document()?.clone();
    let ids = nodes(cx, p.node_ids.as_deref())?;
    let scene = scene_frame(cx, p.frame.map(|f| f as f64))?;
    let mut budget = Budget::new("removing keyframes");
    let wanted = p.channels.as_deref().map(channel_names).unwrap_or_else(|| {
        ["position", "rotation", "scale.x", "scale.y", "skew"].iter().map(|c| (*c).to_owned()).collect()
    });
    let mut changes = Changes::default();
    let mut removed = 0usize;
    let mut warnings = Vec::new();
    let mut edits = Vec::new();
    for id in &ids {
        let e = node(&doc, id)?;
        let data = data_of(&doc, id, &mut budget)?;
        let mut edit = ChannelEdit::read(&doc, id)?;
        if p.all {
            let names: Vec<String> = edit.channels.keys().cloned().collect();
            for name in names {
                removed += edit.keys(&name).len() + edit.channels[&name]["points"].as_array().map_or(0, Vec::len);
                edit.channels.remove(&name);
                edit.changed.insert(name);
            }
        } else {
            let local = key_frame(&data, scene)?;
            for c in &wanted {
                check_channel(e, c)?;
                budget.spend(1)?;
                if c == "position" {
                    let id_at = edit.points().iter().find(|q| frame_of(q) == Some(local)).map(|q| q["id"].clone());
                    if let Some(pid) = id_at.as_ref().and_then(Value::as_str) {
                        remove_point(cx, &mut edit, &data, pid, &mut warnings, &mut budget)?;
                        removed += 1;
                    }
                } else {
                    let mut keys = edit.keys(c);
                    let before = keys.len();
                    keys.retain(|k| frame_of(k) != Some(local));
                    if keys.len() != before {
                        removed += before - keys.len();
                        edit.touch(local);
                        edit.set_keys(cx, c, keys)?;
                    }
                }
            }
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
    Ok(KeyRemoveResult { removed: as_u32(removed) })
}

// ----- anim.key.list ------------------------------------------------------------------------------------------------

/// `anim.key.list` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct KeyListParams {
    /// The node.
    node_id: String,
    /// First scene frame.
    #[serde(default)]
    from: Option<i64>,
    /// Last scene frame.
    #[serde(default)]
    to: Option<i64>,
    /// Report whether a descendant has a key at each frame (a collapsed parent, 05 §1.7).
    #[serde(default)]
    include_descendants: bool,
}

/// One keyed frame.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListedKey {
    /// Scene frame.
    frame: f64,
    /// Channels of this node keyed there.
    channels: Vec<String>,
    /// Of those, the ones whose segment starting there is constant.
    constant: Vec<String>,
    /// A descendant has a key there.
    descendant_keyed: bool,
}

/// `anim.key.list` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct KeyListResult {
    keys: Vec<ListedKey>,
}

#[command(
    id = "anim.key.list",
    kind = query,
    label = "List keyframes",
    scopes = [read],
    needs_project,
    example(title = "Keyed frames of Bob", params = r#"{"nodeId": "scene:\"Intro\"/element:\"Bob\""}"#),
    example(title = "With descendants, over frames 1-24", params = r#"{"nodeId": "scene:\"Intro\"/element:\"Bob-P\"", "from": 1, "to": 24, "includeDescendants": true}"#),
    agent_only = "agents read keyed frames (05 §19.1.5); the Timeline shows them (M5, M6)",
)]
pub(crate) fn anim_key_list(cx: &mut Cx, p: KeyListParams) -> Result<KeyListResult, CommandError> {
    let doc = cx.document()?.clone();
    let id = node_id(cx, &p.node_id)?;
    let mut budget = Budget::new("listing keyframes");
    let data = data_of(&doc, &id, &mut budget)?;
    let edit = ChannelEdit::read(&doc, &id)?;
    let mut map: BTreeMap<u64, ListedKey> = BTreeMap::new();
    let entry = |map: &mut BTreeMap<u64, ListedKey>, scene: f64| -> u64 {
        let k = scene as u64;
        map.entry(k).or_insert(ListedKey {
            frame: tidy(scene),
            channels: Vec::new(),
            constant: Vec::new(),
            descendant_keyed: false,
        });
        k
    };
    for q in edit.points() {
        if let Some(f) = frame_of(&q) {
            let k = entry(&mut map, data.bar.scene(f64::from(f)));
            if let Some(row) = map.get_mut(&k) {
                row.channels.push("position".to_owned());
                if q["constantSegment"] == json!(true) {
                    row.constant.push("position".to_owned());
                }
            }
        }
    }
    for name in edit.channels.keys() {
        for key in edit.keys(name) {
            if let Some(f) = frame_of(&key) {
                budget.spend(1)?;
                let k = entry(&mut map, data.bar.scene(f64::from(f)));
                if let Some(row) = map.get_mut(&k) {
                    row.channels.push(name.clone());
                    if key["constantSegment"] == json!(true) {
                        row.constant.push(name.clone());
                    }
                }
            }
        }
    }
    if p.include_descendants {
        let sc = scene_of(&doc, &id)?;
        for d in h::descendants(sc.tree(), &id) {
            let bar = data_of(&doc, &d, &mut budget)?.bar;
            let child = ChannelEdit::read(&doc, &d)?;
            for f in keyed_frames(&child) {
                budget.spend(1)?;
                let k = entry(&mut map, bar.scene(f64::from(f)));
                if let Some(row) = map.get_mut(&k) {
                    row.descendant_keyed = true;
                }
            }
        }
    }
    let lo = p.from.map_or(f64::NEG_INFINITY, |f| f as f64);
    let hi = p.to.map_or(f64::INFINITY, |f| f as f64);
    Ok(KeyListResult { keys: map.into_values().filter(|k| k.frame >= lo && k.frame <= hi).collect() })
}

// ----- anim.key.move ------------------------------------------------------------------------------------------------

/// `anim.key.move` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct KeyMoveParams {
    /// The node.
    node_id: String,
    /// Scene frame of the keys.
    from_frame: i64,
    /// Scene frame to move them to.
    to_frame: i64,
    /// Channels; default: every channel keyed at `fromFrame`.
    #[serde(default)]
    channels: Option<Vec<String>>,
}

/// `anim.key.move` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KeyMoveResult {
    node_id: String,
    to_frame: i64,
}

#[command(
    id = "anim.key.move",
    kind = action,
    label = "Move keyframe",
    scopes = [edit],
    undo = "Move Keyframe",
    needs_project,
    example(title = "Move Bob's keys at frame 12 to frame 14", params = r#"{"nodeId": "scene:\"Intro\"/element:\"Bob\"", "fromFrame": 12, "toFrame": 14}"#),
    gui = "Timeline: drag keyframes; function editor: timing modifier (M5, M6 C6.2)",
)]
pub(crate) fn anim_key_move(cx: &mut Cx, p: KeyMoveParams) -> Result<KeyMoveResult, CommandError> {
    let doc = cx.document()?.clone();
    let id = node_id(cx, &p.node_id)?;
    let e = node(&doc, &id)?;
    let mut budget = Budget::new("moving keyframes");
    let data = data_of(&doc, &id, &mut budget)?;
    let from = key_frame(&data, p.from_frame as f64)?;
    let to = key_frame(&data, p.to_frame as f64)?;
    let mut edit = ChannelEdit::read(&doc, &id)?;
    let names: Vec<String> = match &p.channels {
        Some(c) => channel_names(c),
        None => {
            let mut v: Vec<String> = edit
                .channels
                .keys()
                .filter(|n| {
                    edit.keys(n).iter().any(|k| frame_of(k) == Some(from))
                        || (n.as_str() == "position" && edit.points().iter().any(|q| frame_of(q) == Some(from)))
                })
                .cloned()
                .collect();
            // A velocity record at a moved pin travels with the pin below.
            if v.iter().any(|c| c == "position") {
                v.retain(|c| c != "velocity");
            }
            v
        }
    };
    if names.is_empty() {
        return Err(frame_invalid(p.from_frame as f64, "no key at this frame"));
    }
    if from != to {
        for c in &names {
            check_channel(e, c)?;
            budget.spend(1)?;
            if c == "position" {
                let mut points = edit.points();
                let Some(i) = points.iter().position(|q| frame_of(q) == Some(from)) else {
                    return Err(frame_invalid(p.from_frame as f64, "no motion keyframe at this frame"));
                };
                let frames: Vec<u32> = points.iter().filter_map(frame_of).collect();
                if frames.contains(&to) {
                    return Err(key_exists(p.to_frame as f64));
                }
                let pos = frames.partition_point(|f| *f < from);
                let lo = pos.checked_sub(1).and_then(|a| frames.get(a));
                let hi = frames.get(pos + 1);
                if lo.is_some_and(|l| to <= *l) || hi.is_some_and(|h| to >= *h) {
                    return Err(frame_invalid(p.to_frame as f64, "the move would cross a neighbouring key"));
                }
                let old: Vec<f64> = frames.iter().map(|f| f64::from(*f)).collect();
                let new: Vec<f64> =
                    frames.iter().map(|f| if *f == from { f64::from(to) } else { f64::from(*f) }).collect();
                retime_velocity(cx, &mut edit, &old, &new)?;
                points[i]["frame"] = json!(to);
                edit.set_points(cx, points)?;
            } else {
                if c == "velocity"
                    && let Some(path) = &data.path
                    && motion_frames(path).contains(&f64::from(from))
                {
                    return Err(unsupported("velocity", e, "a pin moves with its keyframe (05 §10.5)"));
                }
                let mut keys = edit.keys(c);
                let frames: Vec<u32> = keys.iter().filter_map(frame_of).collect();
                let Some(i) = frames.iter().position(|f| *f == from) else {
                    return Err(frame_invalid(p.from_frame as f64, "no key at this frame"));
                };
                if frames.contains(&to) {
                    return Err(key_exists(p.to_frame as f64));
                }
                let lo = i.checked_sub(1).and_then(|a| frames.get(a));
                let hi = frames.get(i + 1);
                if lo.is_some_and(|l| to <= *l) || hi.is_some_and(|h| to >= *h) {
                    return Err(frame_invalid(p.to_frame as f64, "the move would cross a neighbouring key"));
                }
                // A velocity key is normalised to its keyframe segment and stays inside it (05 §10.5).
                if c == "velocity"
                    && let Some(path) = &data.path
                {
                    let pins = motion_frames(path);
                    let s = pins.partition_point(|q| *q <= f64::from(from));
                    let below = s.checked_sub(1).and_then(|a| pins.get(a));
                    if below.is_some_and(|a| f64::from(to) <= *a) || pins.get(s).is_some_and(|b| f64::from(to) >= *b) {
                        return Err(frame_invalid(
                            p.to_frame as f64,
                            "a velocity key stays inside its keyframe segment",
                        ));
                    }
                }
                keys[i]["frame"] = json!(to);
                edit.set_keys(cx, c, keys)?;
            }
        }
        edit.touch(from);
        edit.touch(to);
    }
    let mut changes = Changes::default();
    edit.stage(&mut changes);
    changes.stage(cx)?;
    emit_changed(cx, &data, &edit);
    Ok(KeyMoveResult { node_id: id.as_str().to_owned(), to_frame: p.to_frame })
}

// ----- anim.segment.set ---------------------------------------------------------------------------------------------

/// `anim.segment.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SegmentSetParams {
    /// Nodes; default: the selected rows, else the current element.
    #[serde(default)]
    node_ids: Option<Vec<String>>,
    /// Scene frame of the keyframe starting the segment.
    frame: i64,
    /// Channels; default: every channel keyed at `frame`, except velocity.
    #[serde(default)]
    channels: Option<Vec<String>>,
    /// Constant or tweened.
    constant: bool,
}

/// `anim.segment.set` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SegmentSetResult {
    node_ids: Vec<String>,
    channels: Vec<String>,
}

#[command(
    id = "anim.segment.set",
    kind = action,
    label = "Set segment constant",
    scopes = [edit],
    undo = "Change Segment",
    needs_project,
    example(title = "Hold Bob's rotation from frame 1", params = r#"{"nodeIds": ["scene:\"Intro\"/element:\"Bob\""], "frame": 1, "channels": ["rotation"], "constant": true}"#),
    gui = "Animation menu, Timeline: Set Constant / Non-Constant Segment (M6 C6.2)",
)]
pub(crate) fn anim_segment_set(cx: &mut Cx, p: SegmentSetParams) -> Result<SegmentSetResult, CommandError> {
    let doc = cx.document()?.clone();
    let ids = nodes(cx, p.node_ids.as_deref())?;
    let mut budget = Budget::new("changing segments");
    let wanted = p.channels.as_deref().map(channel_names);
    let mut changes = Changes::default();
    let mut touched: Vec<String> = Vec::new();
    let mut edits = Vec::new();
    for id in &ids {
        let e = node(&doc, id)?;
        let data = data_of(&doc, id, &mut budget)?;
        let local = key_frame(&data, p.frame as f64)?;
        let mut edit = ChannelEdit::read(&doc, id)?;
        let explicit = wanted.is_some();
        let names: Vec<String> = match &wanted {
            Some(w) => w.clone(),
            None => edit.channels.keys().filter(|n| n.as_str() != "velocity").cloned().collect(),
        };
        for c in &names {
            check_channel(e, c)?;
            budget.spend(1)?;
            if c == "velocity" {
                return Err(unsupported("velocity", e, "the velocity channel can never be constant (05 §10.2)"));
            }
            if c == "position" {
                let mut points = edit.points();
                match points.iter_mut().find(|q| frame_of(q) == Some(local)) {
                    Some(q) => {
                        q["constantSegment"] = json!(p.constant);
                        edit.set_points(cx, points)?;
                    }
                    None if explicit => return Err(frame_invalid(p.frame as f64, "no motion keyframe at this frame")),
                    None => continue,
                }
            } else {
                let mut keys = edit.keys(c);
                match keys.iter_mut().find(|k| frame_of(k) == Some(local)) {
                    Some(k) => {
                        k["constantSegment"] = json!(p.constant);
                        if !p.constant && k["easing"] == json!(Easing::Constant) {
                            k["easing"] = json!(Easing::Linear);
                        }
                        edit.set_keys(cx, c, keys)?;
                    }
                    None if explicit => return Err(frame_invalid(p.frame as f64, "no key at this frame")),
                    None => continue,
                }
            }
            edit.touch(local);
            if !touched.contains(c) {
                touched.push(c.clone());
            }
        }
        edit.stage(&mut changes);
        edits.push((data, edit));
    }
    changes.stage(cx)?;
    for (data, edit) in &edits {
        emit_changed(cx, data, edit);
    }
    Ok(SegmentSetResult { node_ids: ids.iter().map(|i| i.as_str().to_owned()).collect(), channels: touched })
}

// ----- anim.segment.uniform.apply -------------------------------------------------------------------------------------

/// A side of a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Side {
    /// The segment ending at the key.
    Left,
    /// The segment starting at the key.
    Right,
}

/// `anim.segment.uniform.apply` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct UniformParams {
    /// The node.
    node_id: String,
    /// Scene frame of the key.
    frame: i64,
    /// Which side.
    side: Side,
    /// Channels; default: every 1-D channel keyed at `frame`, and velocity at a motion keyframe.
    #[serde(default)]
    channels: Option<Vec<String>>,
}

/// `anim.segment.uniform.apply` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UniformResult {
    node_id: String,
    channels: Vec<String>,
}

/// Makes the 1-D segment starting at `keys[a]` straight (05 §10.1).
fn straighten(keys: &mut [Value], a: usize) {
    keys[a]["constantSegment"] = json!(false);
    keys[a]["easing"] = json!(Easing::Linear);
    keys[a]["handles"]["right"] = json!({ "dt": 0, "dv": 0 });
    if let Some(b) = keys.get_mut(a + 1) {
        b["handles"]["left"] = json!({ "dt": 0, "dv": 0 });
    }
}

#[command(
    id = "anim.segment.uniform.apply",
    kind = action,
    label = "Make segment uniform",
    scopes = [edit],
    undo = "Uniform Segment",
    needs_project,
    example(title = "Make the segment after Bob's frame 1 uniform", params = r#"{"nodeId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1, "side": "right"}"#),
    gui = "Animation menu: Uniform Left / Uniform Right (M6 C6.2)",
)]
pub(crate) fn anim_segment_uniform_apply(cx: &mut Cx, p: UniformParams) -> Result<UniformResult, CommandError> {
    let doc = cx.document()?.clone();
    let id = node_id(cx, &p.node_id)?;
    let e = node(&doc, &id)?;
    let mut budget = Budget::new("making a segment uniform");
    let data = data_of(&doc, &id, &mut budget)?;
    let local = key_frame(&data, p.frame as f64)?;
    let mut edit = ChannelEdit::read(&doc, &id)?;
    let at_motion_key = edit.points().iter().any(|q| frame_of(q) == Some(local));
    let names: Vec<String> = match &p.channels {
        Some(c) => channel_names(c),
        None => {
            let mut v: Vec<String> = edit
                .channels
                .keys()
                .filter(|n| n.as_str() != "position" && n.as_str() != "velocity")
                .filter(|n| edit.keys(n).iter().any(|k| frame_of(k) == Some(local)))
                .cloned()
                .collect();
            if at_motion_key {
                v.push("velocity".to_owned());
            }
            v
        }
    };
    let mut done = Vec::new();
    for c in &names {
        check_channel(e, c)?;
        budget.spend(1)?;
        if c == "velocity" {
            // Constant speed across the keyframe segment: no intermediate keys, straight shape.
            let path = edit_path(&edit, &data)?;
            let frames = motion_frames(&path);
            let f = f64::from(local);
            let Some(j) = frames.iter().position(|q| *q == f) else {
                return Err(frame_invalid(p.frame as f64, "no motion keyframe at this frame"));
            };
            let start = match p.side {
                Side::Right => j,
                Side::Left => match j.checked_sub(1) {
                    Some(a) => a,
                    None if p.channels.is_none() => continue,
                    None => return Err(frame_invalid(p.frame as f64, "no segment on this side")),
                },
            };
            let (Some(f0), Some(f1)) = (frames.get(start).copied(), frames.get(start + 1).copied()) else {
                if p.channels.is_none() {
                    continue;
                }
                return Err(frame_invalid(p.frame as f64, "no segment on this side"));
            };
            let mut keys = edit.keys("velocity");
            keys.retain(|k| frame_of(k).is_none_or(|x| f64::from(x) <= f0 || f64::from(x) >= f1));
            let at = keys.iter().position(|k| frame_of(k) == Some(f0 as u32));
            if let Some(a) = at {
                straighten(&mut keys, a);
            } else if edit.channels.contains_key("velocity") {
                let shape = default_shape(cx);
                let mut k = new_key(cx, f0 as u32, 0.0, shape)?;
                k["easing"] = json!(Easing::Linear);
                let pos = keys.partition_point(|o| frame_of(o).is_some_and(|x| f64::from(x) < f0));
                keys.insert(pos, k);
            }
            if edit.channels.contains_key("velocity") {
                edit.set_keys(cx, "velocity", keys)?;
            }
        } else if c == "position" {
            return Err(unsupported(c, e, "a motion path's timing is its velocity channel"));
        } else {
            let mut keys = edit.keys(c);
            let Some(i) = keys.iter().position(|k| frame_of(k) == Some(local)) else {
                return Err(frame_invalid(p.frame as f64, "no key at this frame"));
            };
            let a = match p.side {
                Side::Right if i + 1 < keys.len() => i,
                Side::Left if i > 0 => i - 1,
                // A channel named explicitly needs its segment; the default list takes those that have one.
                _ if p.channels.is_none() => continue,
                _ => return Err(frame_invalid(p.frame as f64, "no segment on this side")),
            };
            straighten(&mut keys, a);
            edit.set_keys(cx, c, keys)?;
        }
        edit.touch(local);
        done.push(c.clone());
    }
    let mut changes = Changes::default();
    edit.stage(&mut changes);
    changes.stage(cx)?;
    emit_changed(cx, &data, &edit);
    Ok(UniformResult { node_id: id.as_str().to_owned(), channels: done })
}
