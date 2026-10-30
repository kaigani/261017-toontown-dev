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

/// `{jobId}` of a job command, with the job's final result `R` declared under `x-jobResult` (11 §15.2; the job
/// table checks every final result against it, C12-R1-13).
#[derive(Debug, Clone)]
pub(crate) struct Job<R> {
    /// The job (`dryRun` for a dry run, which starts nothing).
    pub job_id: String,
    _result: std::marker::PhantomData<R>,
}

impl<R> Job<R> {
    pub(crate) fn new(job_id: String) -> Self {
        Self { job_id, _result: std::marker::PhantomData }
    }

    /// The answer of a dry run: the call was validated, nothing was started.
    pub(crate) fn dry() -> Self {
        Self::new("dryRun".to_owned())
    }
}

impl<R> Serialize for Job<R> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(Some(1))?;
        m.serialize_entry("jobId", &self.job_id)?;
        m.end()
    }
}

impl<R: JsonSchema> JsonSchema for Job<R> {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        format!("Job_{}", R::schema_name()).into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let result = generator.subschema_for::<R>();
        schemars::json_schema!({
            "type": "object",
            "properties": { "jobId": { "type": "string", "minLength": 1, "description": "The job (11 §7.7)." } },
            "required": ["jobId"],
            "additionalProperties": false,
            "x-jobResult": result,
        })
    }
}

/// A warning returned in a result (01 §12.1 rule 6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct ResultWarning {
    /// Code.
    pub id: String,
    /// Message.
    pub message: String,
    /// Details.
    pub details: serde_json::Value,
}
