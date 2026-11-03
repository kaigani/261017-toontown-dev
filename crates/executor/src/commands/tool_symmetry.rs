//! The symmetry drawing mode (02 §6.2; M2 C2.4 for ticket C21-R1-11): `tool.symmetry.set`. A global drawing-tool
//! setting kept in the session's tool state, read by every stroke command that takes `symmetry: true`, so mirrors and
//! kaleidoscopes are created through commands (the tools that show the axes arrive in M4 C4.2).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tt_registry::{CommandError, command};
use tt_vector::symmetry::Symmetry;

use super::draw::err;
use crate::cx::Cx;

/// The tool-state key of the symmetry setting.
pub(crate) const KEY: &str = "symmetry";

/// Symmetry mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Mode {
    /// No copies.
    #[default]
    None,
    /// One mirror axis.
    Mirror,
    /// Dihedral symmetry.
    Kaleidoscope,
}

fn six() -> i64 {
    6
}

/// `tool.symmetry.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SymmetryParams {
    /// Mode.
    #[serde(default)]
    mode: Mode,
    /// Axis angle from +y, counter-clockwise, degrees (0–360).
    #[serde(default)]
    angle: f64,
    /// Mirror lines of a kaleidoscope (2–16).
    #[serde(default = "six")]
    mirrors: i64,
}

/// `{mode, angle, mirrors}`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SymmetryResult {
    /// Mode.
    mode: Mode,
    /// Axis angle (degrees).
    angle: f64,
    /// Mirror lines.
    mirrors: i64,
}

#[command(
    id = "tool.symmetry.set",
    kind = ui,
    label = "Symmetry drawing mode",
    scopes = [ui],
    example(title = "A kaleidoscope of 8 mirrors at 15°", params = r#"{"mode": "kaleidoscope", "angle": 15, "mirrors": 8}"#),
    example(title = "Mirror about the vertical axis", params = r#"{"mode": "mirror"}"#),
    gui = "Drawing toolbar › Symmetry (M4 C4.2)",
)]
pub(crate) fn tool_symmetry_set(cx: &mut Cx, p: SymmetryParams) -> Result<SymmetryResult, CommandError> {
    if !(p.angle.is_finite() && (0.0..=360.0).contains(&p.angle)) {
        return Err(err("tool.valueOutOfRange", "the symmetry angle is 0-360 degrees", json!({ "pointer": "/angle" })));
    }
    if !(2..=16).contains(&p.mirrors) {
        return Err(err("tool.valueOutOfRange", "a kaleidoscope has 2-16 mirrors", json!({ "pointer": "/mirrors" })));
    }
    let r = SymmetryResult { mode: p.mode, angle: p.angle, mirrors: p.mirrors };
    let text = serde_json::to_string(&r).map_err(|e| CommandError::internal(e.to_string()))?;
    cx.app_mut().tool_memory.insert(KEY.to_owned(), text);
    Ok(r)
}

/// The symmetry setting in force.
pub(crate) fn setting(cx: &Cx) -> Symmetry {
    let Some(r) = cx.app().tool_memory.get(KEY).and_then(|t| serde_json::from_str::<SymmetryResult>(t).ok()) else {
        return Symmetry::None;
    };
    match r.mode {
        Mode::None => Symmetry::None,
        Mode::Mirror => Symmetry::Mirror { angle: r.angle },
        Mode::Kaleidoscope => Symmetry::Kaleidoscope { angle: r.angle, mirrors: u32::try_from(r.mirrors).unwrap_or(6) },
    }
}
