//! Parameter and result types shared by several commands.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tt_registry::{CommandError, codes};

/// No parameters.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct NoParams {}

/// An empty result object.
#[derive(Debug, Clone, Default, Serialize, JsonSchema)]
pub(crate) struct Empty {}

/// What to do with unsaved changes (01 §11.2 rule 3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OnUnsaved {
    /// Fail with `project.unsavedChanges` when something is modified.
    #[default]
    Fail,
    /// Save first.
    Save,
    /// Discard the changes.
    Discard,
}

/// What to do when a target file exists (01 §11.2 rule 4).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OnExists {
    /// Fail with `fs.exists` / `project.exists`.
    #[default]
    Fail,
    /// Overwrite.
    Overwrite,
}

/// Previous / next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Direction {
    /// Backwards.
    Previous,
    /// Forwards.
    Next,
}

/// Selection / clipboard context (01 §6.2); `auto` follows the focused panel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum EditContext {
    /// The focused panel's context.
    #[default]
    Auto,
    /// Drawing objects of the current drawing.
    Drawing,
    /// Elements in the camera/top/side views.
    Staging,
    /// Exposure Sheet.
    Xsheet,
    /// Timeline.
    Timeline,
    /// Library.
    Library,
    /// Function editor.
    FunctionEditor,
}

impl EditContext {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Drawing => "drawing",
            Self::Staging => "staging",
            Self::Xsheet => "xsheet",
            Self::Timeline => "timeline",
            Self::Library => "library",
            Self::FunctionEditor => "functionEditor",
        }
    }

    /// Resolves `auto` from the focused panel (focus-aware editing, readiness bullet 4).
    pub(crate) fn resolve(self, focused: Option<&str>, central: &str) -> EditContext {
        if self != Self::Auto {
            return self;
        }
        match focused.unwrap_or(central) {
            "drawingView" => Self::Drawing,
            "xsheet" | "cells" => Self::Xsheet,
            "timeline" => Self::Timeline,
            "library" => Self::Library,
            "functionEditor" => Self::FunctionEditor,
            _ => Self::Staging,
        }
    }
}

/// A 2-D point.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Point {
    /// X.
    pub x: f64,
    /// Y.
    pub y: f64,
}

/// A rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Rect {
    /// Left.
    pub x: f64,
    /// Top.
    pub y: f64,
    /// Width.
    pub w: f64,
    /// Height.
    pub h: f64,
}

/// The body of a handler whose command is an event (never invoked: the executor refuses events first).
pub(crate) fn event_not_invocable(id: &str) -> CommandError {
    CommandError::new(codes::COMMAND_DISABLED, format!("`{id}` is an event and cannot be invoked"))
}

/// An `arg.invalid` with a pointer.
pub(crate) fn invalid(pointer: &str, message: impl Into<String>) -> CommandError {
    CommandError::with_details(codes::ARG_INVALID, message, serde_json::json!({ "pointer": pointer }))
}
