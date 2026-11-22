//! The scene-planning tool (05 §5.0, §19.3; M4 C4.3a): `anim.tool.set`, `anim.tool.get`, `anim.tool.changed`. The
//! Select and Transform tools are C4.3's (C4.3 plan answer 2); the others of 05 §5.0's table, their tools, and the
//! *cycle rotate tools* command are M6's.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tt_registry::{CommandError, codes, command};

use super::common::NoParams;
use crate::cx::Cx;

/// A scene-planning tool (05 line 1217).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SceneTool {
    /// Select (05 §5.1): static values.
    Select,
    /// Transform (05 §5.2): keyed at the current frame.
    Transform,
    /// Motion recording.
    Record,
    /// Rotate.
    Rotate,
    /// Skew.
    Skew,
    /// Scale.
    Scale,
    /// Motion.
    Motion,
    /// Bone tools (§13.4).
    BoneCreate,
    BoneSetup,
    BoneManipulate,
    BoneInverse,
    BoneForward,
    BoneInfluence,
    BoneParent,
}

impl SceneTool {
    fn name(self) -> String {
        serde_json::to_value(self).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default()
    }
}

/// The scene tool ids `anim.tool.set` takes (SI-148: a Camera view request's `tool` among them is a scene tool).
pub(crate) const SCENE_TOOLS: &[&str] = &[
    "select",
    "transform",
    "record",
    "rotate",
    "skew",
    "scale",
    "motion",
    "boneCreate",
    "boneSetup",
    "boneManipulate",
    "boneInverse",
    "boneForward",
    "boneInfluence",
    "boneParent",
];

/// `anim.tool.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ToolParams {
    /// The tool.
    tool: SceneTool,
    /// Cycle between Rotate and Skew (05 §5.0's *cycle rotate tools*).
    #[serde(default)]
    cycle_rotate: bool,
}

/// `{tool}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ToolResult {
    /// The scene-planning tool.
    tool: String,
}

fn owned_by_m6(what: &str) -> CommandError {
    CommandError::with_details(
        codes::COMMAND_NOT_IMPLEMENTED,
        format!("{what} arrives with M6 (C4.3 builds Select and Transform)"),
        json!({ "owner": "M6" }),
    )
}

/// `anim.tool.set`: the tools C4.3 builds (the precheck and the handler give the same refusal, ruling O-24).
pub(crate) fn precheck_tool(_cx: &Cx, v: &serde_json::Value) -> Result<(), CommandError> {
    if v.get("cycleRotate").and_then(serde_json::Value::as_bool) == Some(true) {
        return Err(owned_by_m6("the cycle rotate tools command"));
    }
    match v.get("tool").and_then(serde_json::Value::as_str) {
        Some("select" | "transform") | None => Ok(()),
        Some(t) if SCENE_TOOLS.contains(&t) => Err(owned_by_m6(&format!("the `{t}` scene tool"))),
        Some(_) => Ok(()),
    }
}

#[command(
    id = "anim.tool.set",
    precheck = precheck_tool,
    kind = ui,
    label = "Select a scene tool",
    scopes = [ui],
    needs_project,
    example(title = "The Transform tool", params = r#"{"tool": "transform"}"#),
    gui = "the Camera view's scene tools (05 §5); wired in M4 C4.3",
)]
pub(crate) fn anim_tool_set(cx: &mut Cx, p: ToolParams) -> Result<ToolResult, CommandError> {
    if p.cycle_rotate {
        return Err(owned_by_m6("the cycle rotate tools command"));
    }
    if !matches!(p.tool, SceneTool::Select | SceneTool::Transform) {
        return Err(owned_by_m6(&format!("the `{}` scene tool", p.tool.name())));
    }
    let tool = p.tool.name();
    let shell = cx.ui_mut().shell_mut();
    let changed = shell.scene_tool != tool;
    shell.scene_tool.clone_from(&tool);
    // A Camera view now runs the scene tools (SI-148).
    shell.scene_family = true;
    if changed {
        cx.emit("anim.tool.changed", json!({ "tool": tool }));
    }
    Ok(ToolResult { tool })
}

#[command(
    id = "anim.tool.get",
    kind = query,
    label = "Get the scene tool",
    scopes = [read],
    needs_project,
    example(title = "The scene-planning tool", params = "{}"),
    gui = "the Camera view's scene tools (05 §5); wired in M4 C4.3",
)]
pub(crate) fn anim_tool_get(cx: &mut Cx, _p: NoParams) -> Result<ToolResult, CommandError> {
    Ok(ToolResult { tool: cx.ui().shell().scene_tool.clone() })
}

/// `anim.tool.changed` payload.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ToolChanged {
    /// The scene-planning tool now.
    tool: String,
}

#[command(
    id = "anim.tool.changed",
    kind = event,
    label = "Scene tool changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn anim_tool_changed(_cx: &mut Cx, _p: NoParams) -> Result<ToolChanged, CommandError> {
    Err(super::common::event_not_invocable("anim.tool.changed"))
}
